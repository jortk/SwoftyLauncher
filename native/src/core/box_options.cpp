#include "runtime.hpp"

#include <algorithm>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>

#include <array>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

#include "apply/content/box_options.hpp"
#include "car_category_logic.hpp"

// Box rules that are game options (content.package 1.5: rules.safety_car, rules.formation_lap, rules.penalties,
// rules.corner_cutting; apply/content/box_options.hpp). The era picker calls box_option_apply with the pressed box's
// setting for each rule it carries and box_option_put_back for each rule it does not (and for all of them when
// another box or a stock box is pressed). Each option is found and set the way car_category.cpp finds and sets the
// category option (Game_OptionsStore_FindOption on g_Game_OptionsStore +0x270, entry option at +8, values at
// +0x80..+0x88 of 0x50 bytes with the name holder at +0x38, index get/set through vtable +0x10/+0x18). Every step
// checks what it reads; a shape it does not know, a missing option or a value it cannot match sets nothing and says
// why.

namespace {

namespace mem = swofty::mem;
namespace logic = runtime::car_category;
namespace content = swofty_core::apply::content;
using swofty::game::Game_OptionsStore_FindOption_t;

constexpr size_t STORE_MAP = 0x270;
constexpr size_t STORE_ENTRIES = 0x288;
constexpr size_t STORE_END = 0x290;
constexpr size_t STORE_ENTRY = 0x10;
constexpr size_t OPT_VALUES = 0x80;
constexpr size_t OPT_VALUES_END = 0x88;
constexpr size_t VALUE_BYTES = 0x50;
constexpr size_t VALUE_NAME = 0x38;
constexpr size_t NAME_LEN = 0x10;
constexpr size_t NAME_SRC = 0x18;
constexpr size_t NAME_PLAIN = 0x40;
constexpr size_t NAME_BUF = 0x48;
constexpr size_t MAX_VALUES = 64;

std::mutex lock;
std::array<content::OptionForce, content::BOX_OPTION_COUNT> forced;

template <class T> T at(const void *base, size_t off)
{
    T v{};
    memcpy(&v, static_cast<const uint8_t *>(base) + off, sizeof v);
    return v;
}

bool code_ptr(const void *p)
{
    auto a = reinterpret_cast<uintptr_t>(p);
    uintptr_t base = swofty::api->module_base();
    return p && a >= base && a < base + 0x20000000 && mem::readable(p, 1);
}

template <class F> F *vfunc(const void *object, size_t slot)
{
    if (!mem::readable(object, 8))
        return nullptr;
    auto **vt = *static_cast<void **const *>(object);
    if (!mem::readable(vt, (slot + 1) * 8) || !code_ptr(vt[slot]))
        return nullptr;
    return reinterpret_cast<F *>(vt[slot]);
}

std::string text_at(const char *p)
{
    if (!p || !mem::readable(p, 1))
        return {};
    std::string out;
    for (size_t i = 0; i < logic::VALUE_NAME_MAX && mem::readable(p + i, 1) && p[i]; ++i)
        out.push_back(p[i]);
    return out;
}

void *find_option(const char *name, std::string &why)
{
    uintptr_t find = swofty::api ? swofty::api->resolve("Game_OptionsStore_FindOption") : 0;
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_OptionsStore") : 0;
    if (!find || !slot) {
        why = "this build does not resolve Game_OptionsStore_FindOption and g_Game_OptionsStore";
        return nullptr;
    }
    void *store = mem::readable(reinterpret_cast<void *>(slot), 8) ? *reinterpret_cast<void **>(slot) : nullptr;
    if (!mem::readable(store, STORE_END + 8)) {
        why = "the options store is not readable";
        return nullptr;
    }
    uint64_t index = reinterpret_cast<Game_OptionsStore_FindOption_t *>(find)(static_cast<uint8_t *>(store) + STORE_MAP, &name);
    auto *base = at<uint8_t *>(store, STORE_ENTRIES), *end = at<uint8_t *>(store, STORE_END);
    if (!base || end < base || index >= size_t(end - base) / STORE_ENTRY)
        return nullptr;
    uint8_t *entry = base + index * STORE_ENTRY;
    void *option = mem::readable(entry, STORE_ENTRY) ? at<void *>(entry, 8) : nullptr;
    return mem::readable(option, OPT_VALUES_END + 8) ? option : nullptr;
}

std::vector<std::string> value_names(void *option)
{
    std::vector<std::string> out;
    auto *v = at<uint8_t *>(option, OPT_VALUES), *e = at<uint8_t *>(option, OPT_VALUES_END);
    if (!v || e < v || size_t(e - v) % VALUE_BYTES || size_t(e - v) / VALUE_BYTES > MAX_VALUES || !mem::readable(v, size_t(e - v)))
        return out;
    uintptr_t key_at = swofty::api ? swofty::api->resolve("k_Gdb_StringXorKey") : 0;
    const auto *key = reinterpret_cast<const uint8_t *>(key_at);
    for (uint8_t *entry = v; entry < e; entry += VALUE_BYTES) {
        std::string name;
        auto *holder = at<uint8_t *>(entry, VALUE_NAME);
        auto *n = mem::readable(holder, 8) ? at<uint8_t *>(holder, 0) : nullptr;
        if (mem::readable(n, NAME_BUF + 8)) {
            if (at<int64_t>(n, NAME_PLAIN) == 1) {
                name = text_at(at<const char *>(n, NAME_BUF));
            } else {
                auto len = at<uint64_t>(n, NAME_LEN);
                auto *src = at<const uint8_t *>(n, NAME_SRC);
                if (key && len >= 2 && len <= logic::VALUE_NAME_MAX && mem::readable(src, size_t(len)) && mem::readable(key, 4))
                    name = logic::decode_name(src, size_t(len), key);
            }
        }
        out.push_back(name);
    }
    return out;
}

// The stock value labels of the box rule options in the order the database lists them (options_system.options.game
// options_folder_0: option_21 rules_and_penalties_mode, _22 corner_cutting_mode, _30 safety_car_frequency, _32
// formation_lap). In game (F1 25 build 25251167) every value name holder read empty while the value count matched, so
// a list whose names all read empty and whose count matches takes these labels (the option's values are built from the
// rows in this order).
struct StockValues {
    const char *option;
    std::array<const char *, 4> values;
    size_t count;
};
constexpr StockValues STOCK_VALUES[] = {
    {"rules_and_penalties_mode", {"off", "corner_cutting_only", "eSports", "on"}, 4},
    {"corner_cutting_mode", {"regular", "strict", nullptr, nullptr}, 2},
    {"safety_car_frequency", {"off", "reduced", "standard", "increased"}, 4},
    {"formation_lap", {"off", "on", nullptr, nullptr}, 2},
};

// Fills unreadable names from STOCK_VALUES. Returns true when it did.
bool stock_names(const char *option, std::vector<std::string> &names)
{
    if (names.empty() || std::any_of(names.begin(), names.end(), [](const std::string &n) { return !n.empty(); }))
        return false;
    for (const auto &s : STOCK_VALUES)
        if (!strcmp(s.option, option) && s.count == names.size()) {
            for (size_t i = 0; i < s.count; ++i)
                names[i] = s.values[i];
            return true;
        }
    return false;
}

int get_index(const void *option)
{
    auto *get = vfunc<int(const void *, void *)>(option, 2);
    if (!get)
        return -1;
    uint64_t scratch[2] = {};
    return get(option, scratch);
}

void set_index(const void *option, int index)
{
    if (auto *set = vfunc<void(const void *, int)>(option, 3))
        set(option, index);
}

std::string joined(const std::vector<std::string> &names)
{
    std::string s;
    for (const auto &n : names)
        s += (s.empty() ? "" : ", ") + (n.empty() ? std::string("?") : n);
    return s.empty() ? "none" : s;
}

}
namespace runtime {

// Sets the option of rule `rule` (an index into BOX_OPTION_RULES) to the box's setting. Returns the line to log.
std::string box_option_apply(size_t rule, const std::string &setting)
{
    if (rule >= content::BOX_OPTION_COUNT)
        return "not applied: unknown rule";
    const auto &r = content::BOX_OPTION_RULES[rule];
    std::lock_guard<std::mutex> g(lock);
    std::string why, tried;
    void *option = nullptr;
    const char *name = nullptr;
    for (const char *n : r.options) {
        if (!n)
            break;
        tried += (tried.empty() ? "" : ", ") + std::string(n);
        if ((option = find_option(n, why)) != nullptr) {
            name = n;
            break;
        }
        if (!why.empty())
            return "not applied: " + why;
    }
    if (!option)
        return "not applied: the options store has no option named " + tried;
    std::vector<std::string> names = value_names(option);
    bool stock = stock_names(name, names);
    int index = content::box_option_value(names, r.rule, setting);
    if (index < 0)
        return "not applied: option " + std::string(name) + " has no value for " + setting + " (values: " + joined(names) + ")";
    if (!vfunc<int(const void *, void *)>(option, 2) || !vfunc<void(const void *, int)>(option, 3))
        return "not applied: option " + std::string(name) + " has no index get/set in its vtable";
    int before = get_index(option);
    forced[rule].force(option, index, get_index, set_index);
    int now = get_index(option);
    return "option " + std::string(name) + " index " + std::to_string(before) + " -> " + std::to_string(now) + " (" +
           names[size_t(index)] + (stock ? ", stock value order" : "") + ")" + (now == index ? "" : "; the option did not take the index");
}

// Puts the player's index back where the option of rule `rule` still holds the box's. Returns 1 when it did.
int box_option_put_back(size_t rule)
{
    if (rule >= content::BOX_OPTION_COUNT)
        return 0;
    std::lock_guard<std::mutex> g(lock);
    if (!forced[rule].active())
        return 0;
    std::string why;
    void *option = nullptr;
    for (const char *n : content::BOX_OPTION_RULES[rule].options) {
        if (!n)
            break;
        if ((option = find_option(n, why)) != nullptr)
            break;
    }
    return forced[rule].put_back(option, get_index, set_index);
}

// Drops every remembered index without touching the options.
void box_options_forget()
{
    std::lock_guard<std::mutex> g(lock);
    for (auto &f : forced)
        f.forget();
}

}
