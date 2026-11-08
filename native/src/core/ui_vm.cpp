#include <windows.h>

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>
#include <swofty/ui.hpp>

#include <cstdio>
#include <string>
#include <vector>

#include "json.hpp"

bool settings_command(uint32_t hash);

namespace swofty::ui {

struct Host {
    static ViewModel *&slot(void *native) { return *reinterpret_cast<ViewModel **>(static_cast<uint8_t *>(native) + 0x2f8); }
    static void bind(ViewModel &vm, const std::string &screen, const std::string &path, void *native)
    {
        vm.screen_ = screen;
        vm.path_ = path;
        vm.native_ = native;
    }
    static void *native(const ViewModel &vm) { return vm.native_; }
};

}

namespace {

using namespace swofty::game;
using swofty::ui::Host;
using swofty::ui::ViewModel;

struct Target {
    const char *what, *pattern;
    uintptr_t rva;
};

const Target T_REGVM{"Vm_Register_ViewModel", "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 88 02 00 00 4C 89 C3", 0x42bf450};
const Target T_FIXSTR{"Ne_Set_FixedString", "56 57 48 83 EC 28 48 89 D7 48 89 CE 0F B6 02 34 01", 0x1600d80};
const Target T_PATH_CALL{"Vm_Resolve_NodePath_Thunk call",
                         "E8 ?? ?? ?? ?? 4C 8D 2D ?? ?? ?? ?? 4C 89 6C 24 60 4C 8D 64 24 68 C5 F8", 0x42c07bb};
const Target T_SETINT_CALL{"Vm_Set_Int call in FeTile_ItemBase_Populate",
                           "E8 ?? ?? ?? ?? 4C 89 6C 24 30 C5 F8 57 C0 C5 F8 11 44 24 38 48 C7 44 24 48 00 00 00 00 48 8B 00 "
                           "48 89 44 24 38 48 85 C0 74 24 48 8B 48 28 48 89 4C 24 40 48 C7 44 24 48 00 00 00 00 48 85 C9 "
                           "74 04 48 89 59 10 48 89 58 28 66 F0 FF 40 24 48 8D 05 ?? ?? ?? ?? 48 89 44 24 30 48 8D 8F 00 01 "
                           "00 00",
                           0x1698223};
const Target T_REGNODE{"Vm_Register_BindingNode", "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 88 01 00 00 49 89 D6",
                       0x4284af0};
const Target T_VTABLES{"FrontendTransitionViewModel vtable stores",
                       "48 8D 05 ?? ?? ?? ?? 48 89 03 C5 C8 57 F6 C5 F8 11 73 10 48 C7 43 20 00 00 00 00 48 8D 05 ?? ?? ?? ?? "
                       "48 89 43 08 48 89 73 28 48 C7 43 30 00 00 00 00 48 8D 4B 38 48 8D 15 ?? ?? ?? ?? E8 ?? ?? ?? ?? "
                       "C7 83 40 01 00 00 2F ED 5D 10 48 8D 8B 48 01 00 00 48 8D 15 ?? ?? ?? ?? E8 ?? ?? ?? ?? C7 83 50 02 "
                       "00 00 05 15 00 00 C7 83 58 02 00 00 02 00 00 00 80 A3 5C 02 00 00 E0 48 8B 4B 28 48 8D 94 24 90 "
                       "00 00 00 49 89 D8 E8 ?? ?? ?? ?? 80 8B 5C 02 00 00 02 C5 F8 11 B3 80 02 00 00 C5 F8 11 B3 70 02 "
                       "00 00 C5 F8 11 B3 90 02 00 00 48 8D 05 ?? ?? ?? ?? 48 89 03",
                       0x149b53e};
constexpr size_t HELPER_LEA = 0x00, HANDLE_LEA = 0x1b, TRANSITION_LEA = 0xad;
constexpr size_t VM_SIZE = 0x300, VT_SLOTS = 18;

using RegVm_t = uint64_t(void *mgr, uint32_t *mode, void *vm);
using FixStr_t = void(void *dst, const char *src);
using Path_t = void **(void *registry, void *root, const char *path);
using SetInt_t = void *(void *registry, void *parent, const char *name, int64_t *value, int overwrite);
using Getter_t = void *(void *self);

struct Factory {
    std::string prefix;
    swofty::ui::Factory make;
};

struct Instance {
    std::string screen;
    void *native = nullptr;
    std::unique_ptr<ViewModel> vm;
};

uintptr_t base, a_regvm, a_fixstr, a_path, a_setint, transition_vt, helper_vt, handle_vt;
void **ui_system;
bool ready;
std::vector<Factory> &factories()
{
    static std::vector<Factory> f;
    return f;
}
std::vector<Instance *> instances;
void *vtable[VT_SLOTS + 1], *vt260[6], *vt268[3];

uintptr_t scan(const Target &t)
{
    uintptr_t at = 0;
    int hits = swofty::api->scan(".xtls", t.pattern, &at);
    if (hits != 1 || !at) {
        swofty::log("[ui] view models: %s signature matched %d time(s); view models stay off", t.what, hits);
        return 0;
    }
    return at;
}

uintptr_t rel32(uintptr_t insn)
{
    return *reinterpret_cast<const uint8_t *>(insn) == 0xe8 ? insn + 5 + *reinterpret_cast<const int32_t *>(insn + 1) : 0;
}

uintptr_t lea_target(uintptr_t insn)
{
    auto *b = reinterpret_cast<const uint8_t *>(insn);
    if (b[0] != 0x48 || b[1] != 0x8d || (b[2] & 0xc7) != 0x05)
        return 0;
    return insn + 7 + *reinterpret_cast<const int32_t *>(insn + 3);
}

uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    while (*s)
        h = h * 33 + uint8_t(*s++);
    return h;
}

ViewModel *owner(void *native)
{
    return native ? Host::slot(native) : nullptr;
}

void *destroy(void *self, int)
{
    return self;
}

uint64_t ret0(void *, ...)
{
    return 0;
}

void on_message(void *, void *) {}

void on_bound(void *self)
{
    if (ViewModel *vm = owner(self))
        vm->on_bound();
}

void on_activated(void *self)
{
    if (ViewModel *vm = owner(self)) {
        swofty::log("[ui] view model %s: screen activated", vm->screen().c_str());
        vm->on_activated();
    }
}

void on_deactivated(void *self)
{
    if (ViewModel *vm = owner(self)) {
        swofty::log("[ui] view model %s: screen deactivated", vm->screen().c_str());
        vm->on_deactivated();
    }
}

void on_update(void *self, float dt)
{
    if (ViewModel *vm = owner(self))
        vm->update(dt);
}

void on_command(void *self, NeStr **name, const void *data)
{
    ViewModel *vm = owner(self);
    if (!vm || !name || !*name)
        return;
    uint32_t hash = *reinterpret_cast<uint32_t *>(*name);
    const char *text = reinterpret_cast<const char *>(*name) + 9;
    bool handled = vm->on_command({hash, text, data});
    swofty::log("[ui] view model %s: command '%s' (0x%08x) %s", vm->screen().c_str(), text, hash,
                handled ? "handled" : "not handled");
}

bool build_vtables()
{
    if (!transition_vt || !helper_vt || !handle_vt) {
        swofty::log("[ui] view models: a vtable address did not resolve (transition base+0x%llx, helper base+0x%llx, "
                    "handle base+0x%llx); view models stay off",
                    (unsigned long long)(transition_vt ? transition_vt - base : 0),
                    (unsigned long long)(helper_vt ? helper_vt - base : 0),
                    (unsigned long long)(handle_vt ? handle_vt - base : 0));
        return false;
    }
    auto *src = reinterpret_cast<void **>(transition_vt), *helper = reinterpret_cast<void **>(helper_vt);
    for (size_t i = 0; i < VT_SLOTS; i++) {
        auto f = reinterpret_cast<uintptr_t>(src[i]);
        if (f < base || f > base + 0x6000000) {
            swofty::log("[ui] view models: template vtable slot %zu is not code (0x%llx); view models stay off", i,
                        (unsigned long long)f);
            return false;
        }
        vtable[1 + i] = src[i];
    }
    void **vt = vtable + 1;
    vt[0] = helper[0];
    vt[1] = helper[1];
    vt[2] = reinterpret_cast<void *>(&destroy);
    vt[5] = reinterpret_cast<void *>(&on_message);
    vt[6] = reinterpret_cast<void *>(&on_bound);
    vt[10] = reinterpret_cast<void *>(&on_command);
    vt[12] = reinterpret_cast<void *>(&on_update);
    vt[13] = reinterpret_cast<void *>(&on_activated);
    vt[15] = reinterpret_cast<void *>(&on_deactivated);
    for (auto &s : vt260)
        s = reinterpret_cast<void *>(&ret0);
    for (auto &s : vt268)
        s = reinterpret_cast<void *>(&ret0);
    return true;
}

void *binding_manager()
{
    if (!ui_system || !*ui_system)
        return nullptr;
    void *sys = *ui_system;
    auto fn = reinterpret_cast<Getter_t *>((*static_cast<void ***>(sys))[0x68 / 8]);
    return fn(sys);
}

void **data_node(const ViewModel &vm, const char *child)
{
    void *reg = *g_Vm_Registry;
    if (!reg || !a_path)
        return nullptr;
    auto *root = *reinterpret_cast<uint8_t **>(static_cast<uint8_t *>(reg) + 8);
    void *root_handle = root ? static_cast<void *>(root + 8) : nullptr;
    if (!root_handle)
        return nullptr;
    std::string path = vm.path();
    if (child && *child)
        path += std::string(".") + child;
    void **node = reinterpret_cast<Path_t *>(a_path)(reg, root_handle, path.c_str());
    return node && *node ? node : nullptr;
}

void *parent_of(const ViewModel &vm, const char *prop, std::string &leaf)
{
    std::string p(prop);
    size_t dot = p.find_last_of('.');
    std::string child = dot == std::string::npos ? "" : p.substr(0, dot);
    leaf = dot == std::string::npos ? p : p.substr(dot + 1);
    void **node = data_node(vm, child.c_str());
    return node ? static_cast<uint8_t *>(*node) + 8 : nullptr;
}

}

namespace swofty::ui {

bool ViewModel::set_text(const char *prop, const char *utf8)
{
    std::string leaf;
    void *parent = parent_of(*this, prop, leaf);
    if (!parent)
        return false;
    NeStr *s = nullptr;
    Ne_Make_StrFromCStr(&s, utf8, 0, 0);
    Vm_Set_String(*g_Vm_Registry, parent, leaf.c_str(), &s, 1);
    Ne_Make_StrFromCStr(&s, nullptr, 0, 0);
    return true;
}

bool ViewModel::set_bool(const char *prop, bool value)
{
    std::string leaf;
    void *parent = parent_of(*this, prop, leaf);
    if (!parent)
        return false;
    uint8_t v = value;
    Vm_Set_Bool(*g_Vm_Registry, parent, leaf.c_str(), &v, 1);
    return true;
}

bool ViewModel::set_int(const char *prop, int32_t value)
{
    std::string leaf;
    void *parent = parent_of(*this, prop, leaf);
    if (!parent || !a_setint)
        return false;
    int64_t v = value;
    reinterpret_cast<SetInt_t *>(a_setint)(*g_Vm_Registry, parent, leaf.c_str(), &v, 1);
    return true;
}

void register_view_model(const char *screen_prefix, Factory factory)
{
    factories().push_back({screen_prefix, factory});
}

}

namespace {

struct Option {
    std::string id, label, type, description;
    std::vector<std::string> commands, texts, values;
    int current = 0;
    std::string depend_id;
    std::string depend_unlocked_val;
    bool has_depend = false;
};

struct AddonInfo {
    std::string id, name;
    std::vector<Option> options;
};

std::vector<AddonInfo> &addons()
{
    static std::vector<AddonInfo> m;
    return m;
}

class AddonPage : public ViewModel {
public:
    void on_activated() override
    {
        AddonInfo *m = find_addon();
        if (!m) {
            set_text("title", screen().c_str());
            swofty::log("[ui] view model %s: no addon for this page in settings.json", screen().c_str());
            return;
        }
        set_text("title", m->name.c_str());
        for (size_t i = 0; i < m->options.size(); i++)
            publish(*m, i);
        swofty::log("[ui] view model %s: published title '%s' and %zu option row(s)", screen().c_str(), m->name.c_str(),
                    m->options.size());
    }

    bool on_command(const swofty::ui::Command &c) override
    {
        AddonInfo *m = find_addon();
        if (!m)
            return false;
        std::string name = c.name ? c.name : "";

        // Handle controller reset action / native F1 reset button commands
        if (name == "reset_to_defaults" || name == "reset_to_default" || name == "reset" ||
            name == "reset_pressed" || name == "restore_defaults" || name == "restore_default" ||
            name == "trigger_default" || name == "swofty_reset" ||
            c.hash == djb2("reset_to_defaults") || c.hash == djb2("reset_to_default") ||
            c.hash == djb2("reset") || c.hash == djb2("reset_pressed") ||
            c.hash == djb2("restore_defaults") || c.hash == djb2("restore_default") ||
            c.hash == djb2("trigger_default") || c.hash == djb2("swofty_reset")) {
            swofty::log("[ui] view model %s: reset defaults command received ('%s', 0x%08x)", screen().c_str(),
                        name.c_str(), c.hash);
            reset_addon_options_to_default(*m);
            for (size_t j = 0; j < m->options.size(); j++)
                publish(*m, j);
            return true;
        }

        for (size_t i = 0; i < m->options.size(); i++) {
            Option &o = m->options[i];
            int count = int(o.commands.size());
            if (!count)
                continue;
            int next = -1;
            for (int k = 0; k < count && next < 0; k++)
                if (djb2(o.commands[k].c_str()) == c.hash)
                    next = k;
            std::string tail = ":" + m->id + ":" + o.id;
            if (next < 0 && name == "swofty_next" + tail)
                next = o.type == "slider" ? (o.current + 1 < count ? o.current + 1 : o.current) : (o.current + 1) % count;
            if (next < 0 && name == "swofty_prev" + tail)
                next = o.type == "slider" ? (o.current > 0 ? o.current - 1 : 0) : (o.current + count - 1) % count;
            if (next < 0)
                continue;
            if (is_option_locked(*m, o)) {
                swofty::log("[ui] view model %s: option %s is locked; ignoring command", screen().c_str(), o.id.c_str());
                return true;
            }
            o.current = next;
            settings_command(djb2(o.commands[next].c_str()));
            if (is_master_option(o)) {
                for (size_t j = 0; j < m->options.size(); j++)
                    publish(*m, j);
            } else if (is_action_option(o)) {
                std::string val = get_option_value(o);
                if (val == "reset" || val == "Reset to default" || o.current == 1) {
                    swofty::log("[ui] view model %s: reset action activated for %s", screen().c_str(), m->id.c_str());
                    reset_addon_options_to_default(*m);
                    o.current = 0;
                    for (size_t j = 0; j < m->options.size(); j++)
                        publish(*m, j);
                } else {
                    publish(*m, i);
                }
            } else {
                publish(*m, i);
            }
            return true;
        }
        return false;
    }

private:
    AddonInfo *find_addon()
    {
        auto &list = addons();
        size_t at = screen().find_last_of('_');
        if (at == std::string::npos)
            return nullptr;
        int index = atoi(screen().c_str() + at + 1);
        return index >= 0 && size_t(index) < list.size() ? &list[size_t(index)] : nullptr;
    }

    bool is_master_option(const Option &o) const
    {
        return o.id == "override_mode" || o.id == "master_mode";
    }

    bool is_action_option(const Option &o) const
    {
        return o.id == "reset_to_default" || o.id == "reset" || o.id == "restore_defaults" ||
               (o.id.size() >= 6 && o.id.rfind("_reset") == o.id.size() - 6);
    }

    bool shares_choices(const Option &a, const Option &b) const
    {
        for (const auto &va : a.values) {
            if (va.empty() || va == "true" || va == "false" || va == "default" || va == "per_team" ||
                va == "individual" || va == "custom" || va == "manual")
                continue;
            for (const auto &vb : b.values) {
                if (va == vb)
                    return true;
            }
        }
        return false;
    }

    const Option *find_master_option(const AddonInfo &m) const
    {
        for (const auto &opt : m.options)
            if (is_master_option(opt))
                return &opt;
        return nullptr;
    }

    std::string get_option_value(const Option &opt) const
    {
        if (opt.current >= 0 && opt.current < int(opt.values.size()))
            return opt.values[size_t(opt.current)];
        if (opt.current >= 0 && opt.current < int(opt.texts.size()))
            return opt.texts[size_t(opt.current)];
        return "";
    }

    void reset_addon_options_to_default(AddonInfo &m)
    {
        for (size_t j = 0; j < m.options.size(); j++) {
            Option &opt = m.options[j];
            if (is_master_option(opt) || is_action_option(opt))
                continue;

            // Reset to default state (index 0)
            if (opt.current != 0 && !opt.commands.empty()) {
                opt.current = 0;
                settings_command(djb2(opt.commands[0].c_str()));
            }
        }
    }

    bool is_option_locked(const AddonInfo &m, const Option &o) const
    {
        // 1. Explicit dependency directive
        if (o.has_depend) {
            for (const auto &opt : m.options) {
                if (opt.id == o.depend_id) {
                    std::string cur_val = get_option_value(opt);
                    return cur_val != o.depend_unlocked_val;
                }
            }
        }

        // 2. Master override mode: only locks options that depend on the master override
        const Option *master = find_master_option(m);
        if (master && master->id != o.id) {
            std::string master_val = get_option_value(*master);
            bool is_custom_mode = (master_val == "per_team" || master_val == "individual" ||
                                   master_val == "custom" || master_val == "manual");

            // Action option (e.g. reset_to_default) is locked when not in custom/individual mode
            if (is_action_option(o))
                return !is_custom_mode;

            // Only lock options that share choices with the master override (or explicitly depend on it)
            if (!is_custom_mode && !master_val.empty() && shares_choices(*master, o))
                return true;
        }

        return false;
    }

    void publish(const AddonInfo &m, size_t i)
    {
        if (i >= m.options.size())
            return;
        const Option &o = m.options[i];
        char key[64];
        auto prop = [&](const char *leaf) {
            snprintf(key, sizeof key, "o%zu.%s", i, leaf);
            return key;
        };
        int count = int(o.commands.size());
        bool locked = is_option_locked(m, o);

        std::string raw_text = o.current >= 0 && o.current < int(o.texts.size()) ? o.texts[size_t(o.current)] : "";
        std::string disp_text;
        std::string disp_label;
        int disp_index = o.current;

        if (locked) {
            const Option *master = find_master_option(m);
            std::string target_val;

            // 1. Author-defined locked state text (available to any option)
            for (size_t s = 0; s < o.values.size(); s++) {
                if (o.values[s] == "locked" || o.values[s] == "disabled") {
                    if (s < o.texts.size()) {
                        target_val = o.texts[s];
                        disp_index = int(s);
                    }
                    break;
                }
            }

            // 2. Dynamic resolution if no explicit locked state choice was declared
            if (target_val.empty()) {
                if (is_action_option(o)) {
                    std::string mode_name;
                    if (master) {
                        for (size_t s = 0; s < master->values.size(); s++) {
                            const std::string &v = master->values[s];
                            if (v == "per_team" || v == "individual" || v == "custom" || v == "manual") {
                                if (s < master->texts.size())
                                    mode_name = master->texts[s];
                                break;
                            }
                        }
                    }
                    if (!mode_name.empty()) {
                        std::string lower = mode_name;
                        for (char &c : lower) c = tolower(uint8_t(c));
                        if (lower.size() >= 4 && lower.rfind("mode") == lower.size() - 4)
                            target_val = mode_name + " only";
                        else
                            target_val = mode_name + " mode only";
                    } else {
                        target_val = "Locked";
                    }
                } else if (master) {
                    std::string master_val = get_option_value(*master);
                    if (master_val == "default") {
                        if (!o.values.empty())
                            target_val = o.values[0];
                        else if (!o.texts.empty())
                            target_val = o.texts[0];
                    } else {
                        target_val = master_val;
                    }
                }
            }

            std::string override_text;
            if (!target_val.empty()) {
                if (is_action_option(o)) {
                    override_text = target_val;
                } else {
                    for (size_t s = 0; s < o.values.size(); s++) {
                        if (o.values[s] == target_val) {
                            disp_index = int(s);
                            if (s < o.texts.size())
                                override_text = o.texts[s];
                            break;
                        }
                    }
                    if (override_text.empty()) {
                        for (size_t s = 0; s < o.texts.size(); s++) {
                            if (o.texts[s].rfind(target_val, 0) == 0) {
                                disp_index = int(s);
                                override_text = o.texts[s];
                                break;
                            }
                        }
                    }
                }
            }

            if (override_text.empty())
                override_text = raw_text;

            disp_label = "{s:18_regular_upper_part_neg}" + o.label + "{/s}";
            disp_text = "{s:18_regular_upper_part_neg}" + override_text + "{/s}";
        } else {
            disp_label = o.label;
            disp_text = raw_text;
            disp_index = o.current;
        }

        set_text(prop("label"), disp_label.c_str());
        set_text(prop("text"), disp_text.c_str());
        set_int(prop("index"), disp_index);
        set_int(prop("value"), disp_index);
        set_int(prop("min"), locked ? disp_index : 0);
        set_int(prop("max"), locked ? disp_index : (count > 0 ? count - 1 : 0));
        set_int(prop("step"), 1);
        set_bool(prop("is_slider"), o.type == "slider");
        set_bool(prop("enabled"), !locked);
        set_bool(prop("disabled"), locked);
    }
};

SWOFTY_VIEW_MODEL("swofty_addon_", AddonPage)

void load_addons(const std::wstring &settings_path)
{
    HANDLE f = CreateFileW(settings_path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return;
    std::string text;
    LARGE_INTEGER size{};
    if (GetFileSizeEx(f, &size) && size.QuadPart > 0 && size.QuadPart < (16 << 20)) {
        text.resize(size_t(size.QuadPart));
        DWORD got = 0;
        if (!ReadFile(f, text.data(), DWORD(text.size()), &got, nullptr))
            text.clear();
    }
    CloseHandle(f);
    json::Value doc;
    if (text.empty() || !json::parse(text, doc))
        return;
    for (auto &jm : doc.field("addons", "mods").a) {
        AddonInfo m;
        m.id = jm["id"].str();
        m.name = jm["name"].str(m.id.c_str());
        for (auto &jo : jm["options"].a) {
            Option o;
            o.id = jo["id"].str();
            o.label = jo["label"].str(o.id.c_str());
            o.type = jo["type"].str();
            o.description = jo["description"].str();
            o.current = int(jo["current"].num(0));

            // Parse directive if present: e.g. [depends: mode=custom]
            size_t dep_pos = o.description.find("[depends:");
            if (dep_pos != std::string::npos) {
                size_t end_pos = o.description.find("]", dep_pos);
                if (end_pos != std::string::npos) {
                    std::string clause = o.description.substr(dep_pos + 9, end_pos - (dep_pos + 9));
                    size_t eq = clause.find('=');
                    if (eq != std::string::npos) {
                        o.depend_id = clause.substr(0, eq);
                        o.depend_unlocked_val = clause.substr(eq + 1);
                        auto trim = [](std::string &s) {
                            while (!s.empty() && (s.front() == ' ' || s.front() == '\t')) s.erase(s.begin());
                            while (!s.empty() && (s.back() == ' ' || s.back() == '\t')) s.pop_back();
                        };
                        trim(o.depend_id);
                        trim(o.depend_unlocked_val);
                        o.has_depend = !o.depend_id.empty();
                    }
                }
            }

            for (auto &st : jo["states"].a) {
                o.commands.push_back(st["command"].str());
                o.texts.push_back(st["text"].str());
                std::string val;
                if (st["value"].type == json::Value::Str)
                    val = st["value"].str();
                else if (st["value"].type == json::Value::Bool)
                    val = st["value"].b ? "true" : "false";
                else if (st["value"].type == json::Value::Num)
                    val = std::to_string(st["value"].num());
                o.values.push_back(std::move(val));
            }
            m.options.push_back(std::move(o));
        }
        addons().push_back(std::move(m));
    }
}

}

bool ui_vm_set_path(const std::string &path, int type, const void *value)
{
    if (!g_Vm_Registry.address() || !a_path)
        return false;
    void *reg = *g_Vm_Registry;
    if (!reg)
        return false;
    auto *root = *reinterpret_cast<uint8_t **>(static_cast<uint8_t *>(reg) + 8);
    if (!root)
        return false;
    size_t dot = path.find_last_of('.');
    if (dot == std::string::npos || dot == 0 || dot + 1 >= path.size())
        return false;
    std::string leaf = path.substr(dot + 1);
    void **node = reinterpret_cast<Path_t *>(a_path)(reg, root + 8, path.substr(0, dot).c_str());
    if (!node || !*node)
        return false;
    void *parent = static_cast<uint8_t *>(*node) + 8;
    switch (type) {
    case 0: {
        NeStr *s = nullptr;
        Ne_Make_StrFromCStr(&s, static_cast<const char *>(value), 0, 0);
        Vm_Set_String(reg, parent, leaf.c_str(), &s, 1);
        Ne_Make_StrFromCStr(&s, nullptr, 0, 0);
        return true;
    }
    case 1: {
        uint8_t v = *static_cast<const int32_t *>(value) != 0;
        Vm_Set_Bool(reg, parent, leaf.c_str(), &v, 1);
        return true;
    }
    case 2: {
        if (!a_setint)
            return false;
        int64_t v = *static_cast<const int32_t *>(value);
        reinterpret_cast<SetInt_t *>(a_setint)(reg, parent, leaf.c_str(), &v, 1);
        return true;
    }
    }
    return false;
}

void *ui_vm_message_system()
{
    if (!ui_system || !swofty::mem::readable(ui_system, sizeof(void *)) || !*ui_system)
        return nullptr;
    void *sys = *ui_system;
    void **vt = *static_cast<void ***>(sys);
    if (!swofty::mem::readable(vt + 0x58 / 8, sizeof(void *)) || !swofty::mem::in_image(uintptr_t(vt[0x58 / 8])))
        return nullptr;
    return reinterpret_cast<Getter_t *>(vt[0x58 / 8])(sys);
}

bool ui_vm_init(uintptr_t module_base, const std::wstring &swofty_dir)
{
    base = module_base;
    a_regvm = scan(T_REGVM);
    a_fixstr = scan(T_FIXSTR);
    if (uintptr_t path_call = scan(T_PATH_CALL))
        a_path = rel32(path_call);
    uintptr_t call = scan(T_SETINT_CALL);
    if (call)
        a_setint = rel32(call);
    if (uintptr_t stores = scan(T_VTABLES)) {
        helper_vt = lea_target(stores + HELPER_LEA);
        handle_vt = lea_target(stores + HANDLE_LEA);
        transition_vt = lea_target(stores + TRANSITION_LEA);
    }
    uintptr_t regnode = scan(T_REGNODE);
    if (regnode) {
        auto *insn = reinterpret_cast<const uint8_t *>(regnode + 0x16);
        if (insn[0] == 0x48 && insn[1] == 0x8b && insn[2] == 0x0d)
            ui_system = reinterpret_cast<void **>(regnode + 0x16 + 7 + *reinterpret_cast<const int32_t *>(insn + 3));
    }
    ready = a_regvm && a_fixstr && a_path && ui_system && Vm_Set_String.address() && Vm_Set_Bool.address() &&
            g_Vm_Registry.address() && build_vtables();
    if (call && a_setint != base + 0xd38fd0)
        swofty::log("[ui] view models: Vm_Set_Int resolved to base+0x%llx (KB build has base+0xd38fd0)",
                    (unsigned long long)(a_setint - base));
    load_addons(swofty_dir + L"settings.json");
    swofty::log("[ui] view models: %s (register base+0x%llx, path base+0x%llx, set_int base+0x%llx, vtables "
                "base+0x%llx/0x%llx/0x%llx, ui system %p, %zu factory(ies), %zu addon(s) from settings.json)",
                ready ? "ready" : "off", (unsigned long long)(a_regvm ? a_regvm - base : 0),
                (unsigned long long)(a_path ? a_path - base : 0), (unsigned long long)(a_setint ? a_setint - base : 0),
                (unsigned long long)(transition_vt ? transition_vt - base : 0),
                (unsigned long long)(helper_vt ? helper_vt - base : 0),
                (unsigned long long)(handle_vt ? handle_vt - base : 0), static_cast<void *>(ui_system),
                factories().size(), addons().size());
    return ready;
}

bool ui_vm_ready()
{
    return ready;
}

bool ui_vm_attach(const std::string &screen)
{
    if (!ready)
        return false;
    for (auto *i : instances)
        if (i->screen == screen)
            return true;
    const Factory *f = nullptr;
    for (auto &c : factories())
        if (screen.compare(0, c.prefix.size(), c.prefix) == 0)
            f = &c;
    if (!f) {
        swofty::log("[ui] view model for %s: no factory matches the screen name; not registered", screen.c_str());
        return false;
    }
    void *mgr = binding_manager();
    if (!mgr) {
        swofty::log("[ui] view model for %s: binding manager not available yet", screen.c_str());
        return false;
    }
    auto *native = static_cast<uint8_t *>(VirtualAlloc(nullptr, VM_SIZE, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    if (!native)
        return false;
    auto *inst = new Instance{screen, native, f->make(screen)};
    *reinterpret_cast<void ***>(native) = vtable + 1;
    *reinterpret_cast<uintptr_t *>(native + 0x08) = handle_vt;
    *reinterpret_cast<void **>(native + 0x28) = mgr;
    *reinterpret_cast<void **>(native + 0x30) = nullptr;
    reinterpret_cast<FixStr_t *>(a_fixstr)(native + 0x38, screen.c_str());
    *reinterpret_cast<uint32_t *>(native + 0x140) = djb2(screen.c_str());
    reinterpret_cast<FixStr_t *>(a_fixstr)(native + 0x148, "");
    *reinterpret_cast<uint32_t *>(native + 0x250) = 0x1505;
    *reinterpret_cast<uint32_t *>(native + 0x258) = 1;
    *(native + 0x25c) = 0;
    *reinterpret_cast<void ***>(native + 0x260) = vt260;
    *reinterpret_cast<void ***>(native + 0x268) = vt268;
    Host::slot(native) = inst->vm.get();
    Host::bind(*inst->vm, screen, "view_model_data." + screen, native);
    uint32_t mode = 2;
    reinterpret_cast<RegVm_t *>(a_regvm)(mgr, &mode, native);
    *(native + 0x25c) |= 2;
    instances.push_back(inst);
    uint32_t key = *reinterpret_cast<uint32_t *>(native + 0x250);
    swofty::log("[ui] view model for %s registered (native %p, binding key 0x%08x%s, path %s)", screen.c_str(),
                static_cast<void *>(native), key, key == djb2(screen.c_str()) ? " = djb2(screen)" : " != djb2(screen)",
                inst->vm->path().c_str());
    return true;
}
