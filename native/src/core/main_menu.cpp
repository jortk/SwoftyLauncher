#include "runtime.hpp"

#include "main_menu.hpp"
#include "main_menu_logic.hpp"
#include "apply/moments.hpp"
#include <safetyhook.hpp>
#include "virtual_pad.hpp"
#include "json.hpp"

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <algorithm>
#include <atomic>
#include <thread>
#include <map>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <utility>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::InlineHook::Error &e);
int overlay_image(const std::string &id, const std::string &path, uint32_t *width, uint32_t *height);
}

bool ui_vm_set_path(const std::string &path, int type, const void *value);

// The main menu in rows. FeTile_Build_List's home case adds the stock tiles in order: Braking Point, Career,
// F1 The Movie, F1 World, the challenge tile, a divider, Local Multiplayer, League Racing, Player Hub, Theatre.
// site_FeHome_AfterCareerCall (swofty_core.cpp) runs right after the Career tile and calls add_tiles, which adds
// Grand Prix and Time Trial as two copies of the F1 World item (their own id, title and description over the
// stock populate), Split Screen as the stock Local Multiplayer item with its own title (its route opens the
// game's Local Multiplayer list, the lone Splitscreen tile), the challenge tile when the game builds it, Theatre,
// Showroom and Customisation (two copies of the Theatre item, each when the launcher's route for it is in the
// build) and a divider; swofty_core then adds Addon Settings last. arm_patch overwrites the first instruction of the F1 World
// block (lea r15d,[r12+1]) with a jmp to the block's final jmp, so F1 World, the divider, League Racing, Player Hub
// and the stock Theatre are never added. The block is matched byte for byte first and the adders are the call
// targets read from it; when anything does not match nothing is patched, no tile is added and the stock menu stays.
//
// The status strip, the sign-in state and the latest video card are painted by the overlay frame
// (paint) from swofty/home_menu.json while the home screen is open. Images (the Discord avatar and the video
// thumbnail) are decoded by the overlay's image loader and drawn as overlay images.
//
// Navigation (notes: main-menu NAV.md). The Time Trial card fires swofty_time_trial, a launcher route in the fe_home
// group (ui/home_nav.flag says the build has it): the game's own home_to_activity with a launcher f1_world_flow entry
// that opens the activity container with the picker's command (swofty_enter_time_trial). The Grand Prix card fires the
// game's own home_to_activity and raises solo_gp on the container once its tabs exist, the command the container's own
// Grand Prix card raises: the game then raises grand_prix_resume and opens grand_prix_continue (the saved seasons and
// New Grand Prix) through the front-end flow's enter_gp_resume, as F1 World > Play > Grand Prix does. The Back of the
// container (f1_world / home), of the Showroom (back_1) and of that saves list (back) is swapped where the game fires
// it for swofty_back_home (the container's home route behind the standard wipe) or swofty_home (the hub's back_1).
// The Customisation card fires swofty_customisation (ui/home_customisation.flag says the build has it), a copy of
// fe_home's own customisation route: close the home screen and its slices, then Migrate customisation_flow
// enter_f1_life_customisation. After that press, the Customisation screen's Back (back, which leads nowhere when the
// screen was opened from the main menu, back_1 to the F1 World hub, or back_2) is swapped for swofty_back_home in its
// group, customisation_f1_life_entry; a Customisation screen reached
// any other way keeps the game's Back. Every Migrate the game runs is logged with its flow and entry
// (nav:), the last accepted flow is kept, and a screen that does not open in time is recovered. A session that
// started from a card hands over (frontend_transition.ui, swofty_enter_return) to the bare activity container, where
// the card's command is raised; session_enter swaps the card's mode for the mode of the session that loaded, as the
// container's tabs can start a Time Trial after a Grand Prix card press (or the other way round).
//
// SWOFTY_MAIN_MENU=0 turns the whole feature off. SWOFTY_MAIN_MENU_NAV=single|rows picks the navigation
// layout; only single (one spawner, tile order) exists so far, and rows is logged as not built.
// SWOFTY_MAIN_MENU_ENTRY=chain opens the pickers the old way (home_to_activity, then the command 900 ms after
// the container is up); SWOFTY_MAIN_MENU_BACK=stock|home|twostep picks the game's Back, its home route or the two-step
// hub Back instead of the launcher's routes. 1.9.4 opened grand_prix_continue straight from the home screen
// (swofty_grand_prix_saves, a copy of the theatre route), where its slot list crashed the game in
// Game_CareerContinue_ListSlots for many players; neither that route nor swofty_grand_prix is fired any more;
// SWOFTY_MAIN_MENU_RETURN=0 leaves the session-end hand-over on the bare container; SWOFTY_MAIN_MENU_RECOVER=0 turns
// the recovery of a screen that did not open off; SWOFTY_MAIN_MENU_STATEFIX=0 leaves #ui_menu_data.frontend_transition_state
// alone when leaving the container; SWOFTY_MAIN_MENU_MIGRATELOG=0 leaves the Migrate command unhooked.

namespace runtime::main_menu {

namespace {

using namespace swofty::game;
namespace logic = ::runtime::main_menu;

constexpr size_t OFF_F1WORLD = 11, OFF_CHALLENGE = 32, OFF_DIVIDER = 47, OFF_LOCAL_MP = 59, OFF_THEATRE = 98, OFF_END_JMP = 103;
constexpr size_t NESTR_CHARS = 9;

const runtime::Sig S_BLOCK{"FeHome_F1WorldBlock",
                           "45 8D 7C 24 01 4C 89 F1 44 89 FA E8 ?? ?? ?? ?? F6 46 08 01 74 12 41 80 C4 02 4C 89 F1 44 "
                           "89 E2 E8 ?? ?? ?? ?? 45 89 E7 41 8D 57 01 4C 89 F1 E8 ?? ?? ?? ?? 41 8D 57 02 4C 89 F1 E8 "
                           "?? ?? ?? ?? 41 8D 57 03 4C 89 F1 E8 ?? ?? ?? ?? 41 8D 57 04 4C 89 F1 E8 ?? ?? ?? ?? 41 80 "
                           "C7 05 4C 89 F1 44 89 FA E8 ?? ?? ?? ?? E9",
                           0x1742fee, runtime::Sig::addr, 0, 0, 0};

using Add_t = void (*)(TileList *, uint8_t);
using Populate_t = void (*)(HomeItem *, void *, uint32_t);
using GetAction_t = Action *(*)(HomeItem *, Action *);

struct Block {
    uint8_t *at = nullptr;
    Add_t f1world = nullptr, challenge = nullptr, divider = nullptr, local_mp = nullptr, theatre = nullptr;
} block;

std::atomic<bool> resolved{false}, patched{false}, home_open{false};
std::once_flag once;
void *f1w_populate = nullptr, *f1w_action = nullptr;
std::atomic<bool> tt_warned{false};
constexpr uint64_t PANEL_DELAY_MS = 450, PANEL_FADE_MS = 250;
std::atomic<int> rows_flag{-1}, menu_flag{-1}, customisation_flag{-1};
std::atomic<bool> customisation_armed{false};
std::atomic<int> via{0};
std::atomic<uint64_t> swap_at{0}, home_opened_at{0};
std::atomic<void *> last_router{nullptr};
std::atomic<uint64_t> hub_at{0};
std::mutex view_lock;
logic::HomeView home_view;
std::mutex swap_lock;
std::string swap_screens;
bool swap_reported = true;
std::atomic<bool> state_loaded{false}, panel_disabled{false}, panel_prepared{false}, bind_pad{false}, overlay_drawn{false};

struct Prepared {
    logic::State s;
    int avatar_slot = -1, video_slot = -1;
    uint32_t video_w = 0, video_h = 0;
    std::string version, discord, watch;
};
std::mutex prepared_lock;
std::shared_ptr<const Prepared> prepared;
uint64_t prepared_at = 0;
std::string prepared_key;
std::atomic<int> strip_wanted{-1}, strip_set{-1};
void *theatre_populate = nullptr, *theatre_action = nullptr, *local_mp_populate = nullptr;
std::mutex state_lock;
logic::State state;
uint64_t state_read_at = 0;

const char *env(const char *name)
{
    const char *v = std::getenv(name);
    return v ? v : "";
}

Add_t target(const uint8_t *at, size_t off)
{
    int32_t rel;
    std::memcpy(&rel, at + off + 1, 4);
    return reinterpret_cast<Add_t>(const_cast<uint8_t *>(at) + off + 5 + rel);
}

bool resolve()
{
    std::call_once(once, [] {
        if (!std::strcmp(env("SWOFTY_MAIN_MENU"), "0")) {
            swofty::log("[main-menu] off (SWOFTY_MAIN_MENU=0); the stock main menu stays");
            return;
        }
        const char *nav = env("SWOFTY_MAIN_MENU_NAV");
        swofty::log("[main-menu] navigation layout: %s%s", *nav ? nav : "single",
                    !std::strcmp(nav, "rows") ? " (rows is not built yet; single is used)" : "");
        auto at = reinterpret_cast<uint8_t *>(runtime::find(S_BLOCK, "home", "core"));
        if (!at) {
            swofty::log("[main-menu] the home tile block did not resolve on this build; the stock main menu stays");
            return;
        }
        if (at[OFF_END_JMP] != 0xE9) {
            swofty::log("[main-menu] the home tile block has no closing jmp; the stock main menu stays");
            return;
        }
        block.at = at;
        block.f1world = target(at, OFF_F1WORLD);
        block.challenge = target(at, OFF_CHALLENGE);
        block.divider = target(at, OFF_DIVIDER);
        block.local_mp = target(at, OFF_LOCAL_MP);
        block.theatre = target(at, OFF_THEATRE);
        swofty::log("[main-menu] home tile block at %p; adders F1 World %p, challenge %p, Local Multiplayer %p, Theatre %p", at,
                    reinterpret_cast<void *>(block.f1world), reinterpret_cast<void *>(block.challenge),
                    reinterpret_cast<void *>(block.local_mp), reinterpret_cast<void *>(block.theatre));
        resolved = true;
    });
    return resolved.load();
}

HomeItem *last_item(TileList *list)
{
    auto begin = reinterpret_cast<uintptr_t>(list->items_begin), end = reinterpret_cast<uintptr_t>(list->items_end);
    if (!begin || end <= begin || (end - begin) % sizeof(TileEntry))
        return nullptr;
    Tile *tile = list->items_end[-1].tile;
    return tile ? tile->item : nullptr;
}

void set_string(void *reg, void *parent, const char *name, const char *text)
{
    NeStr *s = nullptr;
    Ne_Make_StrFromCStr(&s, text, 0, 0);
    Vm_Set_String(reg, parent, name, &s, 1);
    Ne_Make_StrFromCStr(&s, nullptr, 0, 0);
}

const char *id_of(const HomeItem *item)
{
    return item && item->id ? reinterpret_cast<const char *>(item->id) + NESTR_CHARS : "";
}

bool rows_on()
{
    int f = rows_flag.load();
    if (f < 0) {
        f = runtime::read_file(runtime::under_swofty("ui\\home_rows.flag"), 16).empty() ? 0 : 1;
        rows_flag = f;
        swofty::log("[main-menu] the launcher's rows of cards %s", f ? "are in this build" : "are not in this build; the tiles stay in the stock list");
    }
    return f == 1;
}

bool menu_on()
{
    int f = menu_flag.load();
    if (f < 0) {
        f = runtime::read_file(runtime::under_swofty("ui\\home_menu.flag"), 16).empty() ? 0 : 1;
        menu_flag = f;
        swofty::log("[main-menu] the launcher's Showroom route %s", f ? "is in this build" : "is not in this build; no Showroom card");
    }
    return f == 1;
}

bool customisation_on()
{
    int f = customisation_flag.load();
    if (f < 0) {
        f = runtime::read_file(runtime::under_swofty("ui\\home_customisation.flag"), 16).empty() ? 0 : 1;
        customisation_flag = f;
        swofty::log("[main-menu] the launcher's Customisation routes %s", f ? "are in this build" : "are not in this build; no Customisation card");
    }
    return f == 1;
}

void style_item(HomeItem *item)
{
    const char *tpl = logic::template_for(id_of(item), rows_on());
    auto node = item ? static_cast<uint8_t *>(item->button_node.node) : nullptr;
    if (!*tpl)
        return;
    if (!node) {
        swofty::log("[main-menu] %s: no VM node yet, template %s not set", id_of(item), tpl);
        return;
    }
    set_string(*g_Vm_Registry, node + 8, "type", tpl);
    swofty::log("[main-menu] %s -> template %s", id_of(item), tpl);
}

std::map<void **, void *> typed_orig;
std::mutex typed_lock;

void typed_populate(HomeItem *item, void *node, uint32_t a7)
{
    void *orig = nullptr;
    {
        std::lock_guard<std::mutex> g(typed_lock);
        auto it = typed_orig.find(reinterpret_cast<void **>(item->vtable));
        orig = it == typed_orig.end() ? nullptr : it->second;
    }
    if (orig)
        reinterpret_cast<Populate_t>(orig)(item, node, a7);
    style_item(item);
}

void wrap_item(HomeItem *item)
{
    const char *id = id_of(item);
    if (!*logic::template_for(id, rows_on()))
        return;
    if (!std::strcmp(id, logic::ID_GRAND_PRIX) || !std::strcmp(id, logic::ID_TIME_TRIAL) ||
        !std::strcmp(id, logic::ID_LOCAL) || !std::strcmp(id, logic::ID_SHOWROOM) || !std::strcmp(id, logic::ID_CUSTOMISATION) ||
        !std::strcmp(id, logic::ID_SETTINGS))
        return;
    auto vt = reinterpret_cast<void **>(item->vtable);
    {
        std::lock_guard<std::mutex> g(typed_lock);
        if (typed_orig.count(vt))
            return;
    }
    void *orig = vt[4];
    void **nv = swofty::subclass(item, {{4, reinterpret_cast<void *>(typed_populate)}});
    std::lock_guard<std::mutex> g(typed_lock);
    typed_orig[nv] = orig;
    swofty::log("[main-menu] %s: populate wrapped to set its template", id);
}

void style_list(TileList *list)
{
    for (auto e = list->items_begin; e && e < list->items_end; e++)
        if (e->tile && e->tile->item)
            wrap_item(e->tile->item);
}

void retitle(HomeItem *item, const char *title, const char *description)
{
    auto vm_node = static_cast<uint8_t *>(item->button_node.node);
    if (!vm_node)
        return;
    void *reg = *g_Vm_Registry, *parent = vm_node + 8;
    uint8_t no = 0;
    set_string(reg, parent, "title", title);
    Vm_Set_Bool(reg, parent, "title_localised", &no, 1);
    set_string(reg, parent, "description", description);
    Vm_Set_Bool(reg, parent, "description_localised", &no, 1);
    style_item(item);
}

Action *route_action(Action *out, const char *route)
{
    out->type = 1;
    out->str = nullptr;
    Ne_Make_StrFromCStr(&out->str, route, 0, 0);
    return out;
}

void showroom_populate(HomeItem *item, void *node, uint32_t a7)
{
    reinterpret_cast<Populate_t>(theatre_populate)(item, node, a7);
    retitle(item, "SHOWROOM", "Look over every car in the game");
}

Action *showroom_action(HomeItem *item, Action *out)
{
    via = 2;
    swofty::log("[main-menu] showroom card pressed (item %p): route %s", item, logic::SHOWROOM_ROUTE);
    return route_action(out, logic::SHOWROOM_ROUTE);
}

void customisation_populate(HomeItem *item, void *node, uint32_t a7)
{
    reinterpret_cast<Populate_t>(theatre_populate)(item, node, a7);
    retitle(item, logic::CUSTOMISATION_TITLE, logic::CUSTOMISATION_DESCRIPTION);
}

Action *customisation_action(HomeItem *item, Action *out)
{
    customisation_armed = true;
    swofty::log("[main-menu] customisation card pressed (item %p): route %s; the Customisation screen's Back returns to the main menu",
                item, logic::CUSTOMISATION_ROUTE);
    return route_action(out, logic::CUSTOMISATION_ROUTE);
}

void add_theatre_copy(TileList *list, uint8_t index, const char *id, Populate_t populate, GetAction_t action)
{
    block.theatre(list, index);
    HomeItem *item = last_item(list);
    if (!item)
        return;
    auto vt = reinterpret_cast<void **>(item->vtable);
    if (!theatre_populate) {
        theatre_populate = vt[4];
        theatre_action = vt[5];
    }
    if (vt[4] != theatre_populate) {
        swofty::log("[main-menu] %s: the Theatre item's vtable changed; left as a Theatre tile", id);
        return;
    }
    swofty::subclass(item, {{4, reinterpret_cast<void *>(populate)}, {5, reinterpret_cast<void *>(action)}});
    Ne_Make_StrFromCStr(&item->id, id, 0, 0);
}

void split_screen_populate(HomeItem *item, void *node, uint32_t a7)
{
    reinterpret_cast<Populate_t>(local_mp_populate)(item, node, a7);
    retitle(item, logic::SPLIT_SCREEN_TITLE, logic::SPLIT_SCREEN_DESCRIPTION);
}

// The stock Local Multiplayer item, titled Split Screen: its id and its action (the local_multiplayer route, which
// opens the game's Local Multiplayer list, the lone Splitscreen tile) stay the game's.
void add_split_screen(TileList *list, uint8_t index)
{
    block.local_mp(list, index);
    HomeItem *item = last_item(list);
    if (!item) {
        swofty::log("[main-menu] split screen: the Local Multiplayer adder left no item");
        return;
    }
    auto vt = reinterpret_cast<void **>(item->vtable);
    if (!local_mp_populate)
        local_mp_populate = vt[4];
    if (vt[4] != local_mp_populate) {
        swofty::log("[main-menu] split screen: the Local Multiplayer item's vtable changed; left as a Local Multiplayer tile");
        return;
    }
    swofty::subclass(item, {{4, reinterpret_cast<void *>(split_screen_populate)}});
}

enum class Step { idle, wait_container, done };
struct Chain {
    Step step = Step::idle;
    const char *command = "";
    uint64_t started = 0, container_at = 0;
    bool handover = false;
} chain;
std::mutex chain_lock;

const char *route_name()
{
    const char *r = env("SWOFTY_MAIN_MENU_ROUTE");
    return *r ? r : "home_to_activity";
}

bool off(const char *name) { return !std::strcmp(env(name), "0"); }

bool nav_flag()
{
    static std::atomic<int> flag{-1};
    int f = flag.load();
    if (f < 0) {
        f = runtime::read_file(runtime::under_swofty("ui\\home_nav.flag"), 16).empty() ? 0 : 1;
        flag = f;
        swofty::log("[main-menu] nav: the launcher's Time Trial, Grand Prix and Back routes %s", f ? "are in this build" : "are not in this build; the old chain and the game's Back stay");
    }
    return f == 1;
}

bool entry_by_route() { return nav_flag() && std::strcmp(env("SWOFTY_MAIN_MENU_ENTRY"), "chain") != 0; }

logic::Back back_mode() { return logic::back_variant_of(env("SWOFTY_MAIN_MENU_BACK"), nav_flag()); }

struct NavState {
    std::atomic<int> mode{int(logic::Mode::none)}, expect{int(logic::Expect::none)};
    std::atomic<uint64_t> expect_at{0}, return_at{0};
    std::atomic<bool> was_in_session{false}, return_armed{false};
} nav;
std::mutex flow_lock;
std::string flow_now = logic::FRONTEND_FLOW;

std::string get_flow()
{
    std::lock_guard<std::mutex> g(flow_lock);
    return flow_now;
}

void set_flow(const std::string &f)
{
    std::lock_guard<std::mutex> g(flow_lock);
    flow_now = f;
}

void expect_screen(logic::Expect e, const char *why)
{
    nav.expect = int(e);
    nav.expect_at = GetTickCount64();
    swofty::log("[main-menu] nav: expecting %s (%s); flow now %s", e == logic::Expect::home ? "fe_home" : e == logic::Expect::saves ? "grand_prix_continue" : "f1_world_activity_container",
                why, get_flow().c_str());
}

std::string ne_text(NeStr *s)
{
    auto p = reinterpret_cast<uint8_t *>(s);
    if (!p || !runtime::readable(p, NESTR_CHARS))
        return {};
    uint32_t len = *reinterpret_cast<uint32_t *>(p + 6) & 0xfffff;
    if (len > 200 || !runtime::readable(p + NESTR_CHARS, len + 1))
        return {};
    return std::string(reinterpret_cast<const char *>(p) + NESTR_CHARS, len);
}

void reset_transition_state(const char *why)
{
    if (off("SWOFTY_MAIN_MENU_STATEFIX"))
        return;
    bool ok = ui_vm_set_path("ui_menu_data.frontend_transition_state", 0, "main_menu");
    swofty::log("[main-menu] nav: #ui_menu_data.frontend_transition_state set to main_menu before %s: %s", why, ok ? "done" : "could not be set");
}

const runtime::Sig S_CONTAINER{
    "Game_F1WorldActivityContainer_HandleCommand",
    "41 57 41 56 41 55 41 54 56 57 53 48 81 EC B0 00 00 00 48 8B 02 48 85 C0 0F 84 ?? ?? ?? ?? 4C 89 C7 49 89 D0 48 89 CE "
    "8B 00 3D 37 3B FC FC 0F 8E ?? ?? ?? ?? 3D BD FC E1 41",
    0x1a77d60};
safetyhook::InlineHook h_container;
std::atomic<void *> container_vm{nullptr}, container_third{nullptr};
std::atomic<uint64_t> container_vm_at{0};

uint64_t raise_delay_ms()
{
    const char *v = std::getenv("SWOFTY_MAIN_MENU_RAISE_MS");
    return v && *v ? uint64_t(std::atoll(v)) : 300;
}

bool fire_in_group(const char *group, const char *route, const char *why);

// Clears g_Game_F1WorldEntryPoint (how F1 World was entered) so the hub is not reopened after a Back. The address comes from
// the running build's binding set, never from a fixed offset: a build whose set does not resolve it keeps the game's
// own value and says so once.
std::atomic<uintptr_t> entry_point_addr{0};
std::atomic<bool> entry_point_told{false};

// The entry point is a global in the game image, whose protection never changes: it is queried once per address.
std::atomic<uintptr_t> entry_point_checked{0};
std::atomic<bool> entry_point_rw{false};

bool entry_point_writable(uintptr_t at)
{
    if (at && entry_point_checked.load() == at)
        return entry_point_rw.load();
    MEMORY_BASIC_INFORMATION m{};
    bool ok = at && VirtualQuery(reinterpret_cast<const void *>(at), &m, sizeof m) == sizeof m &&
              logic::region_writable(m.State == MEM_COMMIT, m.Protect, uintptr_t(m.BaseAddress), m.RegionSize, at, 4);
    entry_point_rw = ok;
    entry_point_checked = at;
    return ok;
}

void clear_entry_point(const char *why)
{
    uintptr_t at = entry_point_addr.load();
    if (!at && !entry_point_told.load()) {
        at = swofty::api->resolve("g_Game_F1WorldEntryPoint");
        entry_point_addr = at;
    }
    auto entry = reinterpret_cast<uint32_t *>(at);
    switch (logic::entry_point_write(at, at && entry_point_writable(at))) {
    case logic::EntryPointWrite::write:
        *entry = 0;
        return;
    case logic::EntryPointWrite::unresolved:
        if (!entry_point_told.exchange(true))
            swofty::log("[main-menu] back: g_Game_F1WorldEntryPoint is not resolved on this build (%s); the F1 World entry point keeps the game's value (%s)",
                        runtime::build_distribution().c_str(), why);
        return;
    case logic::EntryPointWrite::unwritable:
        if (!entry_point_told.exchange(true))
            swofty::log("[main-menu] back: g_Game_F1WorldEntryPoint at base+0x%llx is not writable memory; the F1 World entry point keeps the game's value (%s)",
                        (unsigned long long)(at - swofty::api->module_base()), why);
        return;
    }
}

void leave_picker_to_home(const char *why)
{
    clear_entry_point(why);
    {
        std::lock_guard<std::mutex> g(swap_lock);
        swap_screens.clear();
        swap_reported = false;
    }
    swap_at = GetTickCount64();
    reset_transition_state(logic::BACK_HOME_FROM_CONTAINER);
    expect_screen(logic::Expect::home, why);
    nav.return_armed = false;
    swofty::log("[main-menu] back: the game's handler fired route f1_world from its own view model (group 0x59ba71d1); it is replaced by swofty_back_home in the same group, so the F1 World hub is never opened (picker Back: %s)", why);
    fire_in_group("f1_world_activity_container", logic::BACK_HOME_FROM_CONTAINER, why);
}

uint64_t container_detour(void *container, NeStr **message, void *third)
{
    if (patched.load() && message && *message && back_mode() == logic::Back::route && !off("SWOFTY_MAIN_MENU_PICKERBACK")) {
        logic::Mode m = logic::Mode(nav.mode.load());
        if (m == logic::Mode::time_trial && ne_text(*message) == "entrant_goto_select" &&
            nav.expect.load() == int(logic::Expect::none)) {
            swofty::log("[main-menu] nav: picker Back intercepted (container command entrant_goto_select, which would select the default tab; the %s picker's Back leaves for the main menu at once)",
                        logic::return_command(m));
            leave_picker_to_home("picker Back");
            return 0;
        }
    }
    if (container_vm.load() != container) {
        container_vm = container;
        container_vm_at = GetTickCount64();
        swofty::log("[main-menu] chain: activity container view model %p seen", container);
    }
    container_third = third;
    if (nav.expect.load() != int(logic::Expect::none) || chain.step == Step::wait_container || nav.mode.load() != int(logic::Mode::none))
        swofty::log("[main-menu] nav: container command '%s'", message && *message ? ne_text(*message).c_str() : "");
    return h_container.call<uint64_t>(container, message, third);
}

std::atomic<uint64_t> saves_back_at{0}, saves_tabs_at{0};

void saves_back_tabs()
{
    if (saves_back_at.load() && !saves_tabs_at.load())
        saves_tabs_at = GetTickCount64();
}

std::atomic<bool> cover_on{false};
std::atomic<uint64_t> cover_since{0}, cover_frames{0}, cover_applied_frames{0}, applied_at{0}, frame_no{0}, open_frame{0};

void cover_up(const char *why)
{
    if (std::strcmp(env("SWOFTY_MAIN_MENU_COVER"), "1") != 0)
        return;
    cover_since = GetTickCount64();
    cover_frames = 0;
    applied_at = 0;
    cover_on = true;
    swofty::log("[main-menu] nav: cover up (%s) at frame %llu", why, (unsigned long long)frame_no.load());
}

void cover_down(const char *picker)
{
    if (!cover_on.exchange(false))
        return;
    swofty::log("[main-menu] nav: picker %s open; cover released after %llu ms / %llu frames", picker,
                (unsigned long long)(GetTickCount64() - cover_since.load()), (unsigned long long)cover_frames.load());
}

void raise_command(void *vm, const char *command, const char *where)
{
    static uint8_t zero[0x100];
    NeStr *name = nullptr;
    Ne_Make_StrFromCStr(&name, command, 0, 0);
    void *third = container_third.load() ? container_third.load() : zero;
    swofty::log("[main-menu] nav: raising '%s' on the activity container %p (%s) at frame %llu (container opened at frame %llu)", command, vm, where,
                (unsigned long long)frame_no.load(), (unsigned long long)open_frame.load());
    uint64_t r = h_container ? h_container.call<uint64_t>(vm, &name, third) : 0;
    Ne_Make_StrFromCStr(&name, nullptr, 0, 0);
    applied_at = GetTickCount64();
    cover_applied_frames = cover_frames.load();
    swofty::log("[main-menu] nav: command %s applied at frame %llu (returned 0x%llx)", command, (unsigned long long)frame_no.load(), (unsigned long long)r);
}

void apply_pending(void *vm, const char *where)
{
    const char *command = nullptr;
    {
        std::lock_guard<std::mutex> g(chain_lock);
        if (chain.step != Step::wait_container || !*chain.command)
            return;
        if (chain.handover) {
            swofty::log("[main-menu] nav: the hand-over's '%s' waits until the activity container is active (its own first command) (%s)", chain.command, where);
            return;
        }
        command = chain.command;
        chain.step = Step::done;
    }
    raise_command(vm, command, where);
}

const runtime::Sig S_GOHOME{"Game_F1WorldActivityContainer_GoHome",
                            "56 57 48 83 EC 28 48 89 CE 88 91 45 09 00 00 80 FA 06 74 34 0F B6 C2 83 F8 05 0F 85 2F 01 00 00 80 BE 42 09 00 00 00 0F 84 22 01 00 00",
                            0x1a79ee0};
safetyhook::InlineHook h_gohome, h_addtabs;

const runtime::Sig S_ADDTABS{"Game_F1WorldActivityContainer_AddTabs",
                             "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC F8 00 00 00 C5 F8 29 B4 24 E0 00 00 00 49 89 CF B1 02 E8 ?? ?? ?? ?? 48 8B 05 ?? ?? ?? ?? 80 78 38",
                             0x1a76130};

void addtabs_detour(void *vm)
{
    h_addtabs.call<void>(vm);
    swofty::log("[main-menu] nav: activity container tabs built (AddTabs returned) at frame %llu", (unsigned long long)frame_no.load());
    saves_back_tabs();
    apply_pending(vm, "the moment its tabs exist");
}

uint64_t gohome_detour(void *vm, uint8_t state)
{
    uint64_t r = h_gohome.call<uint64_t>(vm, state);
    if (state == 5) {
        swofty::log("[main-menu] nav: activity container tabs built at frame %llu", (unsigned long long)frame_no.load());
        saves_back_tabs();
        apply_pending(vm, "the moment its tabs exist");
    }
    return r;
}

const runtime::Sig S_MIGRATE{"Ui_RouteCommand_Migrate",
                             "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC C8 00 00 00 4C 89 C7 48 89 D6 48 89 CB 4D 8B 78 50 45 8B 70 40 48 8B 0D ?? ?? ?? ?? 48 8B 01 FF 50 48",
                             0x4186240};
safetyhook::InlineHook h_migrate;

NeStr **migrate_detour(void *cmd, NeStr **out, void *node)
{
    NeStr **r = h_migrate.call<NeStr **>(cmd, out, node);
    swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
    auto base = static_cast<uint8_t *>(cmd);
    std::string flow, entry, result, before = get_flow();
    if (runtime::readable(base + 0xc8, 16)) {
        flow = ne_text(*reinterpret_cast<NeStr **>(base + 0xc8));
        entry = ne_text(*reinterpret_cast<NeStr **>(base + 0xd0));
    }
    if (r && runtime::readable(r, sizeof(void *)))
        result = ne_text(*r);
    bool failed = logic::migrate_failed(result);
    if (!failed && !flow.empty())
        set_flow(flow);
    if (!failed && logic::customisation_disarms(flow) && customisation_armed.exchange(false))
        swofty::log("[main-menu] back: the %s was entered after the Customisation card press without its Back; the Customisation screen's Back is the game's again", flow.c_str());
    if (!failed && flow == logic::F1_WORLD_FLOW) {
        bool pending = false;
        {
            std::lock_guard<std::mutex> g(chain_lock);
            if (chain.step != Step::wait_container && nav.return_armed.exchange(false)) {
                chain = {};
                chain.step = Step::wait_container;
                chain.command = logic::return_command(logic::Mode(nav.mode.load()));
                chain.started = chain.container_at = GetTickCount64();
                chain.handover = true;
                container_vm = nullptr;
                swofty::log("[main-menu] nav: session-end hand-over migrates to the activity container; '%s' is raised on it once it is active", chain.command);
            }
            pending = chain.step == Step::wait_container;
        }
        if (pending)
            cover_up("the container is about to open");
    }
    if (logic::migrate_logged(failed, flow, before))
        swofty::log("[main-menu] nav: Migrate command: flow '%s' entry '%s' -> %s%s%s; the flow the game last accepted is now %s", flow.c_str(),
                    entry.empty() ? "(none)" : entry.c_str(), failed ? "FAILED: " : "accepted", failed ? result.c_str() : "", "", get_flow().c_str());
    return r;
}

const runtime::Sig S_FIRE{"Vm_Fire_Route", "48 8B 4E 28 48 89 F2 49 89 F8 45 31 C9 E8 ?? ?? ?? ?? E9 E4 00 00 00",
                          0x42c0e20, runtime::Sig::rel32, 13, 14, 5};
safetyhook::InlineHook h_fire;

bool fire_in_group(const char *group, const char *route, const char *why)
{
    void *router = last_router.load();
    if (!router || !h_fire) {
        swofty::log("[main-menu] nav: route '%s' in group %s (%s) not fired: no router captured yet", route, group, why);
        return false;
    }
    static uint8_t scratch[0x400];
    *reinterpret_cast<uint32_t *>(scratch + 0x250) = logic::djb2(group);
    NeStr *name = nullptr;
    Ne_Make_StrFromCStr(&name, route, 0, 0);
    swofty::log("[main-menu] nav: firing route '%s' in group %s (%s); flow now %s", route, group, why, get_flow().c_str());
    h_fire.call<void>(router, scratch, &name, 0);
    Ne_Make_StrFromCStr(&name, nullptr, 0, 0);
    return true;
}

void fire_detour(void *router, void *vm, NeStr **name, uint64_t a4)
{
    last_router = router;
    const char *text = name && *name ? reinterpret_cast<const char *>(*name) + NESTR_CHARS : "";
    uint32_t gid = 0;
    {
        swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
        if (!vm || !swofty::mem::read(reinterpret_cast<uintptr_t>(vm) + 0x250, gid))
            gid = 0;
    }
    if (home_open.load() && gid == logic::djb2("fe_home") && logic::hides_panel(text)) {
        std::lock_guard<std::mutex> g(view_lock);
        home_view.left();
        home_open = false;
        swofty::log("[main-menu] panel hidden: route %s fired from the home screen", text);
    }
    if (patched.load()) {
        if (gid == logic::djb2("f1_world_activity_container") || gid == logic::djb2("fe_showroom") || gid == logic::djb2("grand_prix_continue") ||
            gid == logic::djb2(logic::CUSTOMISATION_GROUP))
            swofty::log("[main-menu] nav: route '%s' fired in group 0x%08x (%s)", text, gid,
                        gid == logic::djb2("fe_showroom")           ? "fe_showroom"
                        : gid == logic::djb2("grand_prix_continue") ? "grand_prix_continue"
                        : gid == logic::djb2(logic::CUSTOMISATION_GROUP) ? logic::CUSTOMISATION_GROUP
                                                                    : "f1_world_activity_container");
        if (logic::saves_back_leaves_home(back_mode(), gid, text, logic::Mode(nav.mode.load()))) {
            saves_tabs_at = 0;
            saves_back_at = GetTickCount64();
            swofty::log("[main-menu] back: the saves list's Back (route back) migrates to the activity container as the game's own; the core leaves the container for the main menu once its tabs exist");
        }
        const char *to = logic::back_replacement(back_mode(), gid, text);
        if (!to && (to = logic::customisation_back(back_mode(), customisation_armed.load(), gid, text))) {
            customisation_armed = false;
            swofty::log("[main-menu] back: the Customisation screen was entered from the main menu card; its Back (route %s in %s) returns to the main menu",
                        text, logic::CUSTOMISATION_GROUP);
        }
        if (to) {
            clear_entry_point("Back");
            {
                std::lock_guard<std::mutex> g(swap_lock);
                swap_screens.clear();
                swap_reported = false;
            }
            swap_at = GetTickCount64();
            if (std::strcmp(to, logic::HOME_BACK_ROUTE) != 0)
                reset_transition_state(to);
            expect_screen(logic::Expect::home, "Back");
            nav.return_armed = false;
            swofty::log("[main-menu] back: the game's handler fired route %s from its own view model (group 0x%08x); it is replaced by %s in the same group, so the F1 World hub is never opened",
                        text, gid, to);
            NeStr *replacement = nullptr;
            Ne_Make_StrFromCStr(&replacement, to, 0, 0);
            h_fire.call<void>(router, vm, &replacement, a4);
            Ne_Make_StrFromCStr(&replacement, nullptr, 0, 0);
            return;
        }
    }
    if (via.load()) {
        swofty::log("[main-menu] route fired while back-to-menu is armed (%s): %s (route group id 0x%08x)", via.load() == 1 ? "card" : "showroom", text, gid);
        if (uint64_t at = swap_at.load(); at && GetTickCount64() - at < 5000)
            swofty::log("[main-menu] route %s fired %llu ms after the back step (vm %p, not replaced)", text, (unsigned long long)(GetTickCount64() - at), vm);
    }
    h_fire.call<void>(router, vm, name, a4);
}

void hook_fire()
{
    if (h_fire)
        return;
    auto at = runtime::find(S_FIRE, "home", "core");
    if (!at) {
        swofty::log("[main-menu] back: Vm_Fire_Route did not resolve; Back from the pickers and the Showroom stays stock");
        return;
    }
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(fire_detour));
    if (!h) {
        swofty::log("[main-menu] back: hooking Vm_Fire_Route failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_fire = std::move(*h);
    swofty::log("[main-menu] back: Vm_Fire_Route hooked at base+0x%llx", (unsigned long long)(at - swofty::api->module_base()));
}

void hook_migrate()
{
    if (h_migrate || off("SWOFTY_MAIN_MENU_MIGRATELOG"))
        return;
    auto at = runtime::find(S_MIGRATE, "home", "core");
    if (!at) {
        swofty::log("[main-menu] nav: the route Migrate command did not resolve; migrations are not logged and no flow is tracked");
        return;
    }
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(migrate_detour));
    if (!h) {
        swofty::log("[main-menu] nav: hooking the route Migrate command failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_migrate = std::move(*h);
    swofty::log("[main-menu] nav: route Migrate command hooked at base+0x%llx", (unsigned long long)(at - swofty::api->module_base()));
}

void hook_addtabs()
{
    if (h_addtabs)
        return;
    auto at = runtime::find(S_ADDTABS, "home", "core");
    if (!at) {
        swofty::log("[main-menu] nav: the container's AddTabs did not resolve; the command is raised %llu ms after its view model is seen", (unsigned long long)raise_delay_ms());
        return;
    }
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(addtabs_detour));
    if (!h) {
        swofty::log("[main-menu] nav: hooking the container's AddTabs failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_addtabs = std::move(*h);
    swofty::log("[main-menu] nav: container AddTabs hooked at base+0x%llx", (unsigned long long)(at - swofty::api->module_base()));
}

void hook_gohome()
{
    if (h_gohome)
        return;
    auto at = runtime::find(S_GOHOME, "home", "core");
    if (!at) {
        swofty::log("[main-menu] nav: the container's tabs-ready callback did not resolve; the command is raised %llu ms after its view model is seen", (unsigned long long)raise_delay_ms());
        return;
    }
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(gohome_detour));
    if (!h) {
        swofty::log("[main-menu] nav: hooking the container's tabs-ready callback failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_gohome = std::move(*h);
    swofty::log("[main-menu] nav: container tabs-ready callback hooked at base+0x%llx", (unsigned long long)(at - swofty::api->module_base()));
}

void hook_container()
{
    if (h_container)
        return;
    auto at = runtime::find(S_CONTAINER, "home", "core");
    if (!at) {
        swofty::log("[main-menu] chain: the activity container command handler did not resolve; the cards open the activity container only");
        return;
    }
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(container_detour));
    if (!h) {
        swofty::log("[main-menu] chain: hooking the activity container command handler failed: %s",
                    runtime::hook_error(h.error()).c_str());
        return;
    }
    h_container = std::move(*h);
    swofty::log("[main-menu] chain: activity container command handler hooked at base+0x%llx",
                (unsigned long long)(at - swofty::api->module_base()));
}

void card_populate(HomeItem *item, void *node, uint32_t a7)
{
    reinterpret_cast<Populate_t>(f1w_populate)(item, node, a7);
    auto vm_node = static_cast<uint8_t *>(item->button_node.node);
    if (!vm_node)
        return;
    bool gp = !std::strcmp(id_of(item), logic::ID_GRAND_PRIX);
    void *reg = *g_Vm_Registry, *parent = vm_node + 8;
    uint8_t no = 0;
    set_string(reg, parent, "title", gp ? "GRAND PRIX" : "TIME TRIAL");
    Vm_Set_Bool(reg, parent, "title_localised", &no, 1);
    set_string(reg, parent, "description",
               gp ? "Race a Grand Prix weekend" : "Set the fastest lap on any track");
    Vm_Set_Bool(reg, parent, "description_localised", &no, 1);
    style_item(item);
}

void gp_saves_probe();

Action *card_action(HomeItem *item, Action *out)
{
    if (uint64_t at = home_opened_at.load(); at && GetTickCount64() - at < 700) {
        out->type = 0;
        out->str = nullptr;
        swofty::log("[main-menu] %s: press ignored %llu ms after fe_home opened (carried-over input)", id_of(item),
                    (unsigned long long)(GetTickCount64() - at));
        return out;
    }
    bool gp = !std::strcmp(id_of(item), logic::ID_GRAND_PRIX);
    const char *route = route_name();
    if (entry_by_route()) {
        if (gp)
            gp_saves_probe();
        logic::Mode m = gp ? logic::Mode::grand_prix : logic::Mode::time_trial;
        nav.mode = int(m);
        nav.return_armed = false;
        route = logic::press_route(m);
        {
            std::lock_guard<std::mutex> g(chain_lock);
            chain = {};
            chain.step = Step::wait_container;
            chain.command = logic::return_command(m);
            chain.started = chain.container_at = GetTickCount64();
            container_vm = nullptr;
        }
        expect_screen(logic::Expect::container, route);
        swofty::log("[main-menu] chain: %s card pressed; route %s opens the activity container, then '%s' is raised on it (%s)", gp ? "Grand Prix" : "Time Trial", route,
                    logic::return_command(m),
                    m == logic::Mode::time_trial ? "the Time Trial picker" : "the container's own Grand Prix command: the saved seasons and New Grand Prix");
    } else {
        via = 1;
        swofty::log("[main-menu] %s card pressed: route %s (the old chain)", id_of(item), route);
    }
    out->type = 1;
    out->str = nullptr;
    Ne_Make_StrFromCStr(&out->str, route, 0, 0);
    return out;
}

void add_card(TileList *list, uint8_t index, const char *id)
{
    block.f1world(list, index);
    HomeItem *item = last_item(list);
    if (!item) {
        swofty::log("[main-menu] %s: the adder left no item", id);
        return;
    }
    auto vt = reinterpret_cast<void **>(item->vtable);
    if (!f1w_populate) {
        f1w_populate = vt[4];
        f1w_action = vt[5];
    }
    if (vt[4] != f1w_populate || vt[5] != f1w_action) {
        swofty::log("[main-menu] %s: the item's vtable is not the F1 World one; left as a plain F1 World tile", id);
        return;
    }
    swofty::subclass(item, {{4, reinterpret_cast<void *>(card_populate)}, {5, reinterpret_cast<void *>(card_action)}});
    Ne_Make_StrFromCStr(&item->id, id, 0, 0);
}

void set_offline_text(long long online)
{
    static long long last = -1;
    std::string text = logic::offline_text(online);
    if (online == last)
        return;
    if (ui_vm_set_path("ui_menu_data.swofty_offline_text", 0, text.c_str())) {
        last = online;
        swofty::log("[main-menu] Launched with panel text set: %s", logic::online_line(online).empty() ? "no online line" : logic::online_line(online).c_str());
    } else {
        static int fails = 0;
        if (fails++ % 60 == 0)
            swofty::log("[main-menu] could not set ui_menu_data.swofty_offline_text yet (try %d)", fails);
    }
}

void read_state(bool force = false)
{
    uint64_t now = GetTickCount64();
    std::lock_guard<std::mutex> g(state_lock);
    if (!force && state_read_at && now - state_read_at < 1000)
        return;
    state_read_at = now;
    auto next = logic::parse_state(runtime::read_file(runtime::under_swofty("home_menu.json"), size_t(1) << 20));
    if (const char *override_path = std::getenv("SWOFTY_HOME_VIDEO_JSON"); override_path && *override_path) {
        json::Value v;
        if (json::parse(runtime::read_file(runtime::wide(override_path), size_t(1) << 20), v)) {
            next.loaded = true;
            next.video_title = v["title"].str();
            next.video_url = v["url"].str();
            next.video_thumbnail = v["thumbnail"].str();
        }
    }
    if (next.loaded != state.loaded || next.signed_in != state.signed_in || next.video_url != state.video_url)
        swofty::log("[main-menu] state: %s, %s, video %s", next.loaded ? logic::version_line(next).c_str() : "no file",
                    logic::discord_line(next).c_str(), logic::has_video(next) ? "yes" : "none");
    state = next;
    state_loaded = next.loaded;
    set_offline_text(next.online);
}

struct PadSample {
    bool connected = false;
    uint32_t packet = 0;
    uint16_t buttons = 0;
};

PadSample sample_pad()
{
    using Get_t = DWORD(WINAPI *)(DWORD, void *);
    static Get_t get = [] {
        HMODULE m = LoadLibraryA("xinput1_4.dll");
        if (!m)
            m = LoadLibraryA("xinput1_3.dll");
        return m ? reinterpret_cast<Get_t>(GetProcAddress(m, "XInputGetState")) : nullptr;
    }();
    uint8_t buf[16] = {};
    PadSample s;
    s.connected = get && get(0, buf) == 0;
    std::memcpy(&s.packet, buf, 4);
    std::memcpy(&s.buttons, buf + 4, 2);
    return s;
}

bool any_key_down()
{
    for (int vk = 8; vk < 255; vk++)
        if (GetAsyncKeyState(vk) & 0x8000)
            return true;
    return false;
}

bool bind_pressed(bool *pad)
{
    static bool was = false;
    static uint32_t last_packet = 0;
    static uint64_t pad_at = 0, key_at = 0;
    uint64_t now = GetTickCount64();
    PadSample p = sample_pad();
    if (p.connected && p.packet != last_packet)
        pad_at = now;
    last_packet = p.packet;
    bool key_active = GetForegroundWindow() != nullptr && any_key_down();
    if (key_active)
        key_at = now;
    static bool shown_pad = false;
    if (pad_at != key_at)
        shown_pad = p.connected && pad_at > key_at;
    *pad = shown_pad;
    bool y = p.connected && (p.buttons & logic::WATCH_PAD_MASK);
    bool key = (GetAsyncKeyState(logic::WATCH_KEY_VK) & 0x8000) != 0 && GetForegroundWindow() != nullptr;
    bool down = y || key;
    bool edge = down && !was;
    was = down;
    return edge;
}

void open_video(const logic::State &s)
{
    std::string body = logic::open_request_json(s.video_url);
    bool ok = runtime::write_file(runtime::under_swofty("open_url.json"), body);
    swofty::log("[main-menu] watch pressed: %s open_url.json for %s", ok ? "wrote" : "could not write", s.video_url.c_str());
}

std::string host_path(const std::string &p)
{
    if (!p.empty() && p[0] == '/') {
        std::string w = "Z:" + p;
        for (auto &c : w)
            if (c == '/')
                c = '\\';
        return w;
    }
    if (!p.empty() && p.find(':') == std::string::npos) {
        std::string rel = "swofty:" + p;
        for (auto &c : rel)
            if (c == '/')
                c = '\\';
        return rel;
    }
    return p;
}

// The function's first 64 bytes are shared with a second lookup (0x1407c7280), so the pattern is taken from the first
// byte that differs (function + 0x31c: the lea/call displacements are wildcards, the short jump after them is not) and
// the anchor walks back to the entry.
const runtime::Sig S_MODE_BY_NAME{"Game_GameModes_FindByName",
                                  "44 24 5E C6 00 00 48 8D 05 ?? ?? ?? ?? 48 89 44 24 28 48 8D 54 24 28 E8 ?? ?? ?? ?? 48 C1 E0 03 48 03 46 58 48 3B 46 60 74 65 48 8B",
                                  0x7d0de0, runtime::Sig::addr, 0, uintptr_t(0) - 0x31c};
const runtime::Sig S_GAME_MANAGER{"game.manager",
                                  "48 8B B8 98 01 00 00 48 85 FF 0F 84 ?? ?? ?? ?? 48 8D 4C 24 70 E8 ?? ?? ?? ?? 48 8B 05", 0x604c2c8,
                                  runtime::Sig::riprel, 26, 29, 7};

// The stock Grand Prix entry (grand_prix_continue) shows its saved seasons when GrandPrixContinueViewModel's mode
// lookup (Game_GrandPrixContinueVm_FindModeEntity 0x141bccc00) finds a championship mode whose season registry
// (entity +0x1030, virtual +0x78) answers 1: F2_2026/F2_2025/F2_2024_Championship_Season, F1_Championship_Season,
// F1_2026_Championship_Season, in that order. The card does not decide on this lookup any more: in the field it found
// no entity for any name, F1_Championship_Season included, so the card never opened the saves screen (1.9.0 to 1.9.3).
// It is asked once per session and logged (the modes pointer and every answer) so the field says which step fails.
void gp_saves_probe()
{
    static std::atomic<bool> asked{false};
    if (asked.exchange(true))
        return;
    static const uintptr_t names[] = {0x5734370, 0x573438c, 0x57343a8, 0x570a6b2, 0x570a6c9};
    uintptr_t find = runtime::find(S_MODE_BY_NAME, "home", "core"), manager = runtime::find(S_GAME_MANAGER, "home", "core");
    if (!find || !manager) {
        swofty::log("[main-menu] probe: the Grand Prix season lookup did not resolve (mode lookup %p, game manager %p)",
                    reinterpret_cast<void *>(find), reinterpret_cast<void *>(manager));
        return;
    }
    void *mgr = *reinterpret_cast<void **>(manager);
    if (!mgr || !runtime::readable(static_cast<uint8_t *>(mgr) + 0xc0, 8)) {
        swofty::log("[main-menu] probe: the game manager %p is not readable", mgr);
        return;
    }
    void *modes = *reinterpret_cast<void **>(static_cast<uint8_t *>(mgr) + 0xc0);
    uintptr_t base = swofty::api->module_base();
    swofty::log("[main-menu] probe: game manager %p, game modes %p, mode lookup base+0x%llx", mgr, modes,
                (unsigned long long)(find - base));
    if (!modes || !runtime::readable(modes, 8))
        return;
    for (uintptr_t rva : names) {
        const char *name = reinterpret_cast<const char *>(base + rva);
        if (!runtime::readable(name, 8) || std::strlen(name) > 40 || std::strncmp(name, "F", 1)) {
            swofty::log("[main-menu] probe: mode name at base+0x%llx is not a name", (unsigned long long)rva);
            continue;
        }
        auto entity = reinterpret_cast<uint8_t *>(reinterpret_cast<void *(*)(void *, const char *)>(find)(modes, name));
        if (!entity) {
            swofty::log("[main-menu] probe: Grand Prix mode %s: no entity", name);
            continue;
        }
        if (!runtime::readable(entity + 0x1030, 8)) {
            swofty::log("[main-menu] probe: Grand Prix mode %s: entity %p has no readable season registry", name, entity);
            continue;
        }
        void **vt = *reinterpret_cast<void ***>(entity + 0x1030);
        if (!vt || !runtime::readable(vt + 0xf, 8)) {
            swofty::log("[main-menu] probe: Grand Prix mode %s: entity %p, registry vtable %p unreadable", name, entity, vt);
            continue;
        }
        int answer = reinterpret_cast<int (*)(void *)>(vt[0x78 / 8])(entity + 0x1030);
        swofty::log("[main-menu] probe: Grand Prix mode %s: entity %p, season registry answers %d", name, entity, answer);
    }
}

void start_chain(bool time_trial)
{
    if (!std::strcmp(env("SWOFTY_MAIN_MENU_CHAIN"), "0")) {
        swofty::log("[main-menu] chain off (SWOFTY_MAIN_MENU_CHAIN=0); the card opens the activity container");
        return;
    }
    std::lock_guard<std::mutex> g(chain_lock);
    chain = {};
    chain.step = Step::wait_container;
    if (!time_trial)
        gp_saves_probe();
    logic::Mode m = time_trial ? logic::Mode::time_trial : logic::Mode::grand_prix;
    nav.mode = int(m);
    chain.command = logic::return_command(m);
    chain.started = GetTickCount64();
    container_vm = nullptr;
    swofty::log("[main-menu] chain: %s card pressed; route %s, then the activity container's %s command", time_trial ? "Time Trial" : "Grand Prix",
                route_name(), chain.command);
}

}

bool enabled()
{
    return resolve();
}

bool drawing()
{
    return cover_on.load() || logic::panel_due(patched.load(), home_open.load() && !runtime::in_session_now(), state_loaded.load(),
                            panel_disabled.load(), panel_prepared.load());
}

bool wants_overlay()
{
    if (!std::strcmp(env("SWOFTY_MAIN_MENU"), "0") || !std::strcmp(env("SWOFTY_MAIN_MENU_PANEL"), "0"))
        return false;
    return !runtime::read_file(runtime::under_swofty("home_menu.json"), size_t(1) << 20).empty();
}

bool arm_patch()
{
    if (!resolve())
        return false;
    if (patched.load())
        return true;
    uint8_t jmp[5] = {0xE9, uint8_t(OFF_END_JMP - 5), 0, 0, 0};
    DWORD old;
    if (!VirtualProtect(block.at, sizeof jmp, PAGE_EXECUTE_READWRITE, &old)) {
        swofty::log("[main-menu] could not make the home tile block writable; the stock main menu stays");
        resolved = false;
        return false;
    }
    std::memcpy(block.at, jmp, sizeof jmp);
    VirtualProtect(block.at, sizeof jmp, old, &old);
    FlushInstructionCache(GetCurrentProcess(), block.at, sizeof jmp);
    patched = true;
    hook_container();
    hook_fire();
    hook_migrate();
    hook_gohome();
    hook_addtabs();
    swofty::log("[main-menu] stock F1 World, divider, League Racing, Player Hub and Theatre tiles skipped");
    return true;
}

uint8_t add_tiles(void *list_, uint8_t career_index, const uint8_t *flags)
{
    auto list = static_cast<TileList *>(list_);
    if (!arm_patch())
        return career_index;
    uint8_t index = career_index;
    add_card(list, ++index, logic::ID_GRAND_PRIX);
    add_card(list, ++index, logic::ID_TIME_TRIAL);
    add_split_screen(list, ++index);
    if (flags && (flags[8] & 1))
        block.challenge(list, ++index);
    block.theatre(list, ++index);
    bool showroom = menu_on();
    if (showroom)
        add_theatre_copy(list, ++index, logic::ID_SHOWROOM, showroom_populate, showroom_action);
    bool customisation = customisation_on();
    if (customisation)
        add_theatre_copy(list, ++index, logic::ID_CUSTOMISATION, customisation_populate, customisation_action);
    block.divider(list, ++index);
    if (HomeItem *divider = last_item(list)) {
        Ne_Make_StrFromCStr(&divider->id, logic::ID_DIVIDER, 0, 0);
        swofty::log("[main-menu] divider tile added before Addon Settings (index %u)", unsigned(index));
    }
    style_list(list);
    swofty::log("[main-menu] tiles added after Career: Grand Prix, Time Trial, Split Screen%s, Theatre%s%s (last index %u)",
                flags && (flags[8] & 1) ? ", challenge" : "", showroom ? ", Showroom" : "", customisation ? ", Customisation" : "", unsigned(index));
    return index;
}

void style_settings(void *item)
{
    if (rows_on())
        style_item(static_cast<HomeItem *>(item));
}

void prepare();
void poll_bind();

void recover(logic::Expect e)
{
    std::string flow = get_flow();
    switch (logic::recovery_for(e, flow, home_open.load())) {
    case logic::Recovery::none:
        swofty::log("[main-menu] nav: the expected screen did not open but the main menu is up; nothing to recover");
        break;
    case logic::Recovery::back_home_route:
        swofty::log("[main-menu] nav: the expected screen did not open within 4 s and the %s flow is current; migrating back to the main menu so the player is never left without a screen", flow.c_str());
        reset_transition_state("the recovery");
        if (fire_in_group("f1_world_activity_container", logic::BACK_HOME_FROM_CONTAINER, "recovery"))
            expect_screen(logic::Expect::home, "recovery");
        break;
    case logic::Recovery::open_home:
        swofty::log("[main-menu] nav: the expected screen did not open within 4 s in the %s flow; reopening the main menu screen so the player is never left without a screen", flow.c_str());
        fire_in_group("fe_home", "open", "recovery");
        break;
    }
}

void watch_cover()
{
    if (!cover_on.load())
        return;
    uint64_t now = GetTickCount64();
    if (uint64_t at = applied_at.load(); at && cover_frames.load() >= cover_applied_frames.load() + 3 && now - at >= 120) {
        cover_down("tab of the activity container (dynamic_era_select)");
        return;
    }
    if (now - cover_since.load() > 3000) {
        bool applied = applied_at.load() != 0;
        swofty::log("[main-menu] nav: cover safety timeout after 3 s (command %s)", applied ? "was applied" : "was never applied");
        cover_down("(timeout)");
        if (!applied && !off("SWOFTY_MAIN_MENU_RECOVER"))
            recover(logic::Expect::container);
    }
}

void watch_saves_back(uint64_t now)
{
    uint64_t back_at = saves_back_at.load(), tabs_at = saves_tabs_at.load();
    if (!back_at)
        return;
    if (tabs_at && now - tabs_at >= logic::SAVES_BACK_DELAY_MS) {
        saves_back_at = 0;
        saves_tabs_at = 0;
        swofty::log("[main-menu] back: the activity container is up %llu ms after the saves list's Back; leaving it for the main menu",
                    (unsigned long long)(now - back_at));
        leave_picker_to_home("saves list Back");
    } else if (!tabs_at && now - back_at > logic::SAVES_BACK_TIMEOUT_MS) {
        saves_back_at = 0;
        swofty::log("[main-menu] back: no activity container came up within %llu ms of the saves list's Back; the player continues by hand",
                    (unsigned long long)logic::SAVES_BACK_TIMEOUT_MS);
    }
}

void watch_nav()
{
    watch_cover();
    uint64_t now = GetTickCount64();
    watch_saves_back(now);
    bool in_session = runtime::in_session_now();
    int e = nav.expect.load();
    if (e != int(logic::Expect::none) && now - nav.expect_at.load() > 4000 && !in_session) {
        nav.expect = int(logic::Expect::none);
        if (off("SWOFTY_MAIN_MENU_RECOVER"))
            swofty::log("[main-menu] nav: the expected screen did not open within 4 s; recovery is off (SWOFTY_MAIN_MENU_RECOVER=0)");
        else
            recover(logic::Expect(e));
    }
}

void tick()
{
    if (patched.load())
        watch_nav();
    if (uint64_t at = hub_at.load(); at && GetTickCount64() - at >= 60) {
        hub_at = 0;
        void *router = last_router.load();
        if (router && h_fire) {
            static uint8_t scratch[0x400];
            *reinterpret_cast<uint32_t *>(scratch + 0x250) = logic::djb2("fe_f1_world");
            NeStr *route = nullptr;
            Ne_Make_StrFromCStr(&route, "back", 0, 0);
            {
                std::lock_guard<std::mutex> g(swap_lock);
                swap_screens.clear();
                swap_reported = false;
            }
            swap_at = GetTickCount64();
            clear_entry_point("hub Back");
            swofty::log("[main-menu] back: firing route 'back' in group fe_f1_world (0x%08x), the route the hub's own Back button fires (this build session15 reached the main menu with it)",
                        logic::djb2("fe_f1_world"));
            h_fire.call<void>(router, scratch, &route, 0);
            Ne_Make_StrFromCStr(&route, nullptr, 0, 0);
        } else {
            swofty::log("[main-menu] back: no router was captured; Back from the hub stays stock");
        }
    }
    if (uint64_t at = swap_at.load(); at && GetTickCount64() - at > 1000) {
        std::lock_guard<std::mutex> g(swap_lock);
        if (!swap_reported) {
            swap_reported = true;
            swofty::log("[main-menu] back: 1 s after the route swap the screens opened since were: %s; the F1 World hub (fe_f1_world) %s; fe_home %s",
                        swap_screens.empty() ? "(none)" : swap_screens.c_str(),
                        swap_screens.find("fe_f1_world") != std::string::npos ? "WAS OPENED" : "was never opened",
                        home_open.load() ? "is the last front-end screen opened" : "is NOT the last front-end screen opened");
        }
    }
    if (home_open.load()) {
        uint64_t t0 = GetTickCount64();
        read_state();
        uint64_t t1 = GetTickCount64(), t2 = t1, t3 = t1;
        if (patched.load() && !panel_disabled.load() && std::strcmp(env("SWOFTY_MAIN_MENU_PANEL"), "0") != 0) {
            prepare();
            t2 = GetTickCount64();
            poll_bind();
            t3 = GetTickCount64();
        }
        static uint64_t slow = 0;
        if (t3 - t0 > 100 && (slow++ < 10 || slow % 50 == 0))
            swofty::log("[main-menu] slow home tick: state read %llu ms, panel prepare %llu ms, video bind %llu ms", static_cast<unsigned long long>(t1 - t0),
                        static_cast<unsigned long long>(t2 - t1), static_cast<unsigned long long>(t3 - t2));
    }
    int want = strip_wanted.load();
    if (want >= 0 && want != strip_set.load() && patched.load()) {
        int v = want;
        bool ok = ui_vm_set_path("ui_menu_data.swofty_strip_visible", 1, &v);
        static int tries = 0;
        if (ok) {
            strip_set = want;
            swofty::log("[main-menu] stock top-right strip %s (ui_menu_data.swofty_strip_visible = %d)", want ? "shown" : "hidden", want);
        } else if (tries++ % 120 == 0) {
            swofty::log("[main-menu] could not set ui_menu_data.swofty_strip_visible yet (try %d)", tries);
        }
    }
    std::lock_guard<std::mutex> g(chain_lock);
    if (chain.step == Step::idle || chain.step == Step::done)
        return;
    uint64_t now = GetTickCount64();
    if (chain.step == Step::wait_container && !chain.container_at && now - chain.started > 3000) {
        void *router = last_router.load();
        chain.step = Step::done;
        if (router && h_fire) {
            static uint8_t scratch[0x400];
            *reinterpret_cast<uint32_t *>(scratch + 0x250) = logic::djb2("fe_home");
            NeStr *route = nullptr;
            Ne_Make_StrFromCStr(&route, "open", 0, 0);
            swofty::log("[main-menu] chain: the activity container did not open within 3 s; reopening the main menu (route 'open' in group fe_home) so the player is never left without a screen");
            h_fire.call<void>(router, scratch, &route, 0);
            Ne_Make_StrFromCStr(&route, nullptr, 0, 0);
        } else {
            swofty::log("[main-menu] chain: the activity container did not open within 3 s and no router was captured to reopen the main menu");
        }
        return;
    }
    if (now - chain.started > 20000) {
        swofty::log("[main-menu] chain: gave up at step %d after 20 s; the player continues by hand", int(chain.step));
        chain.step = Step::done;
        return;
    }
    if (chain.step == Step::wait_container && chain.container_at && container_vm.load() &&
        now - chain.container_at > 300 && now - container_vm_at.load() > raise_delay_ms()) {
        const char *command = chain.command;
        chain.step = Step::done;
        raise_command(container_vm.load(), command, chain.handover ? "the hand-over: the container is active, its own first command came" : "fallback: after its view model was seen");
        chain.step = Step::done;
    }
}

void session_enter()
{
    logic::Mode pressed = logic::Mode(nav.mode.load());
    if (pressed == logic::Mode::none)
        return;
    std::string mode = swofty_core::apply::moments::session_mode_track().first;
    logic::Mode driven = logic::driven_mode(pressed, mode);
    if (driven == pressed)
        return;
    nav.mode = int(driven);
    const char *card = pressed == logic::Mode::time_trial ? "Time Trial" : "Grand Prix";
    if (driven == logic::Mode::none)
        swofty::log("[main-menu] nav: the session that loaded (game mode %s) is not the %s the card opened, nor the other card's mode; the session-end hand-over raises no command",
                    mode.c_str(), card);
    else
        swofty::log("[main-menu] nav: the session that loaded is a %s (game mode %s), not the %s the card opened; the session-end hand-over raises '%s'",
                    driven == logic::Mode::time_trial ? "Time Trial" : "Grand Prix", mode.c_str(), card, logic::return_command(driven));
}

void screen(const char *name)
{
    if (!name)
        return;
    if (via.load() == 1 && !std::strcmp(name, "fe_f1_world") && back_mode() == logic::Back::twostep) {
        hub_at = GetTickCount64();
        swofty::log("[main-menu] back: the stock Back opened the F1 World hub; its own Back (route 'back') follows at once");
    }
    if (uint64_t at = swap_at.load(); at && GetTickCount64() - at < 5000)
    {
        swofty::log("[main-menu] back: screen %s opened %llu ms after the route swap", name,
                    (unsigned long long)(GetTickCount64() - at));
        std::lock_guard<std::mutex> g(swap_lock);
        swap_screens += (swap_screens.empty() ? "" : ", ") + std::string(name);
    }
    {
        std::lock_guard<std::mutex> g(chain_lock);
        uint64_t now = GetTickCount64();
        if (chain.step != Step::idle && chain.step != Step::done)
            swofty::log("[main-menu] chain: screen %s (step %d)", name, int(chain.step));
        if (!std::strcmp(name, "f1_world_activity_container")) {
            open_frame = frame_no.load();
            if (chain.step == Step::wait_container) {
                chain.container_at = now;
                container_vm = nullptr;
            } else if (nav.return_armed.exchange(false)) {
                logic::Mode m = logic::Mode(nav.mode.load());
                chain = {};
                chain.step = Step::wait_container;
                chain.command = logic::return_command(m);
                chain.started = chain.container_at = now;
                chain.handover = true;
                container_vm = nullptr;
                swofty::log("[main-menu] nav: session-end hand-over landed on the activity container (the F1 World hub was skipped); raising '%s' on it once it is active", chain.command);
            }
        }
    }
    if (!std::strcmp(name, "frontend_transition") && logic::Mode(nav.mode.load()) != logic::Mode::none && !off("SWOFTY_MAIN_MENU_RETURN") && entry_by_route()) {
        nav.return_armed = true;
        nav.return_at = GetTickCount64();
        swofty::log("[main-menu] nav: session-end hand-over: the front end is back (frontend_transition) after a session that the %s card started; the activity container the hand-over lands on gets that card's command; armed until it opens or fe_home does",
                    logic::return_command(logic::Mode(nav.mode.load())));
    }
    {
        logic::Expect e = logic::Expect(nav.expect.load());
        bool container = !std::strcmp(name, "f1_world_activity_container"), saves = !std::strcmp(name, "grand_prix_continue"), home = !std::strcmp(name, "fe_home");
        if (e != logic::Expect::none || nav.return_armed.load())
            swofty::log("[main-menu] nav: screen %s opened; flow now %s; expected %s", name, get_flow().c_str(),
                        e == logic::Expect::container ? "the activity container" : e == logic::Expect::saves ? "the saves list" : e == logic::Expect::home ? "fe_home" : "(nothing)");
        if ((container && e == logic::Expect::container) || (saves && e == logic::Expect::saves) || (home && e == logic::Expect::home)) {
            nav.expect = int(logic::Expect::none);
            swofty::log("[main-menu] nav: %s opened %llu ms after it was asked for", name, (unsigned long long)(GetTickCount64() - nav.expect_at.load()));
        }
        if (home) {
            set_flow(logic::FRONTEND_FLOW);
            saves_back_at = 0;
            saves_tabs_at = 0;
            nav.return_armed = false;
            nav.mode = int(logic::Mode::none);
            if (e != logic::Expect::none && e != logic::Expect::home)
                nav.expect = int(logic::Expect::none);
        }
    }
    logic::Screen kind = logic::classify_screen(name);
    if (kind != logic::Screen::overlay) {
        {
            std::lock_guard<std::mutex> g(view_lock);
            home_view.screen(name);
            home_open = home_view.home;
        }
        bool home = !std::strcmp(name, "fe_home");
        if (home)
            home_opened_at = GetTickCount64();
        if (home && via.exchange(0))
            swofty::log("[main-menu] fe_home screen opened; back-to-menu disarmed (a screen-open event, not proof it is the visible screen)");
        if (home && customisation_armed.exchange(false))
            swofty::log("[main-menu] back: fe_home opened after the Customisation card press; the Customisation screen's Back is the game's again");
        strip_wanted = home ? 0 : 1;
        if (home) {
            read_state(true);
            swofty::log("[main-menu] home screen open: state read, stock top-right strip %s", rows_on() ? "hidden" : "hidden");
        }
    }
}

void command(void *, uint32_t command_hash, uint32_t focused_hash)
{
    if (!patched.load() || command_hash != logic::djb2("select_pressed"))
        return;
    bool tt = focused_hash == logic::djb2(logic::ID_TIME_TRIAL);
    bool gp = focused_hash == logic::djb2(logic::ID_GRAND_PRIX);
    if (!tt && !gp)
        return;
    if (uint64_t at = home_opened_at.load(); at && GetTickCount64() - at < 700)
        return;
    if (!tt && !entry_by_route())
        start_chain(false);
    if (!tt)
        return;
    read_state();
    logic::State s;
    {
        std::lock_guard<std::mutex> g(state_lock);
        s = state;
    }
    if (!logic::time_trial_warning_due(s, true, tt_warned.load())) {
        if (!entry_by_route())
            start_chain(true);
        return;
    }
    tt_warned = true;
    int r = runtime::popup_show(logic::TT_TITLE, logic::TT_BODY, "swofty_core (main menu)");
    swofty::log("[main-menu] Time Trial while not signed in to Discord: warning %s; the pad chain is skipped this once",
                r == 0 ? "queued" : "refused");
}

void prepare()
{
    uint64_t now = GetTickCount64();
    logic::State s;
    {
        std::lock_guard<std::mutex> g(state_lock);
        s = state;
    }
    if (!s.loaded)
        return;
    std::string key = s.version + "|" + s.avatar + "|" + s.video_thumbnail + "|" + (s.signed_in ? "1" : "0") + "|" + s.name;
    if (key == prepared_key && now - prepared_at < 60000)
        return;
    prepared_key = key;
    prepared_at = now;
    auto p = std::make_shared<Prepared>();
    p->s = s;
    uint32_t w = 0, h = 0;
    if (logic::has_avatar(s))
        p->avatar_slot = runtime::overlay_image("core.main_menu\navatar", host_path(s.avatar), &w, &h);
    if (logic::has_video(s)) {
        p->video_slot = runtime::overlay_image("core.main_menu\nvideo", host_path(s.video_thumbnail), &w, &h);
        p->video_w = w;
        p->video_h = h;
    }
    p->version = logic::version_line(s);
    p->discord = logic::discord_line(s);
    {
        std::lock_guard<std::mutex> g(prepared_lock);
        prepared = p;
    }
    panel_prepared = true;
    swofty::log("[main-menu] panel prepared on the flow thread: avatar slot %d, video slot %d", p->avatar_slot, p->video_slot);
}

void poll_bind()
{
    static uint64_t last = 0;
    uint64_t now = GetTickCount64();
    if (now - last < 100)
        return;
    last = now;
    std::shared_ptr<const Prepared> p;
    {
        std::lock_guard<std::mutex> g(prepared_lock);
        p = prepared;
    }
    if (!p || p->s.video_url.empty())
        return;
    // XInputGetState and the key sweep can block for about a second each under Wine, so they are sampled on their own thread and the
    // flow thread only reads the result.
    static std::atomic<bool> pressed{false}, started{false};
    static std::atomic<uint64_t> wanted_at{0};
    wanted_at = now;
    if (!started.exchange(true))
        std::thread([] {
            for (;;) {
                Sleep(100);
                if (GetTickCount64() - wanted_at.load() > 500)
                    continue;
                uint64_t t0 = GetTickCount64();
                bool on_pad = false;
                if (bind_pressed(&on_pad))
                    pressed = true;
                bind_pad = on_pad;
                uint64_t took = GetTickCount64() - t0;
                static uint64_t slow = 0;
                if (took > 50 && slow++ % 100 == 0)
                    swofty::log("[main-menu] the video card's pad and key sample took %llu ms on its own thread (slow sample %llu)",
                                static_cast<unsigned long long>(took), static_cast<unsigned long long>(slow));
            }
        }).detach();
    if (pressed.exchange(false))
        open_video(p->s);
}

void fade_panel(hud::Frame &f, float x, float y, float w, float h, float fade_left, float fade_right, float k)
{
    const int slices = 48;
    auto smooth = [](float t) {
        t = t < 0 ? 0 : t > 1 ? 1 : t;
        return t * t * (3 - 2 * t);
    };
    for (int i = 0; i < slices; i++) {
        float t0 = float(i) / slices, t1 = float(i + 1) / slices, tm = (t0 + t1) * 0.5f;
        float a = (fade_left > 0 && tm < fade_left ? smooth(tm / fade_left) : 1.0f) *
                  (fade_right > 0 && tm > 1 - fade_right ? smooth((1 - tm) / fade_right) : 1.0f);
        uint32_t fill = 0x0b0d1200u | uint32_t(a * 0xe4);
        uint32_t line = 0xffffff00u | uint32_t(a * 0x50);
        float sx = x + t0 * w, sw = (t1 - t0) * w + 0.6f;
        hud::rect(f, sx, y, sw, h, fill);
        hud::rect(f, sx, y, sw, 1.3f * k, line);
        hud::rect(f, sx, y + h - 1.3f * k, sw, 1.3f * k, line);
    }
}

void key_glyph(hud::Frame &f, float x, float y, float h, float k, const std::string &key, bool pad_style)
{
    hud::TextStyle st;
    st.px = h * 0.62f;
    st.rgba = 0xf3f5f9ffu;
    float w = 0, hh = 0;
    hud::measure(st, key, &w, &hh);
    float bw = pad_style ? std::max(h * 1.5f, w + 18 * k) : std::max(h, w + 14 * k);
    float radius = pad_style ? h * 0.5f : 6 * k;
    hud::rect_rounded(f, x, y, bw, h, radius, 0xffffff1cu);
    hud::rect_rounded_outline(f, x, y, bw, h, radius, 1.4f * k, 0xf3f5f9d8u);
    hud::text(f, x + (bw - w) * 0.5f, y + (h - st.px) * 0.5f, st, key);
}

void image_rounded(hud::Frame &f, int slot, float x, float y, float w, float h, float r, float u0, float v0, float u1, float v1)
{
    if (slot < 0 || w <= 0 || h <= 0 || hud::clipped_out(f))
        return;
    std::vector<hud::Pt> pts;
    hud::rounded_local(pts, x, y, w, h, r, f.xform.scale());
    if (pts.size() < 3 || !hud::reserve(f, pts.size() * 3, slot + 1))
        return;
    uint32_t col = hud::pack(0xffffffff, f.opacity);
    auto uv = [&](const hud::Pt &p, float *u, float *v) {
        *u = u0 + (p.x - x) / w * (u1 - u0);
        *v = v0 + (p.y - y) / h * (v1 - v0);
    };
    hud::Pt c{x + w * 0.5f, y + h * 0.5f};
    for (size_t i = 0; i < pts.size(); i++) {
        const hud::Pt &a = pts[i], &b = pts[(i + 1) % pts.size()];
        float cu, cv, au, av, bu, bv;
        uv(c, &cu, &cv);
        uv(a, &au, &av);
        uv(b, &bu, &bv);
        hud::vtx(f, c.x, c.y, col, cu, cv, hud::IMAGE);
        hud::vtx(f, a.x, a.y, col, au, av, hud::IMAGE);
        hud::vtx(f, b.x, b.y, col, bu, bv, hud::IMAGE);
    }
}

std::vector<std::string> wrap_title(const hud::TextStyle &style, const std::string &title, float max_w, size_t max_lines)
{
    std::vector<std::string> lines;
    std::string line;
    size_t i = 0;
    auto width = [&](const std::string &t) {
        float w = 0, h = 0;
        hud::measure(style, t, &w, &h);
        return w;
    };
    while (i < title.size() && lines.size() < max_lines) {
        size_t j = title.find(' ', i);
        std::string word = title.substr(i, j == std::string::npos ? std::string::npos : j - i);
        std::string trial = line.empty() ? word : line + " " + word;
        if (width(trial) <= max_w || line.empty()) {
            line = trial;
            i = j == std::string::npos ? title.size() : j + 1;
        } else {
            lines.push_back(line);
            line.clear();
        }
    }
    if (!line.empty() && lines.size() < max_lines)
        lines.push_back(line);
    bool cut = i < title.size();
    if (!lines.empty()) {
        std::string &last = lines.back();
        if (cut)
            last += "...";
        while (width(last) > max_w && last.size() > 4) {
            last.erase(last.size() - (cut ? 4 : 1), cut ? 4 : 1);
            if (cut)
                last += "...";
        }
    }
    return lines;
}

void paint(hud::Frame &f, uint64_t now)
{
    (void)now;
    static bool told_first = false, told_blocked = false, told_noprep = false;
    if (!std::exchange(told_first, true))
        swofty::log("[main-menu] panel: paint called on a %.0fx%.0f frame (patched %d, home open %d, state %d, prepared %d, disabled %d)",
                    double(f.width), double(f.height), int(patched.load()), int(home_open.load()), int(state_loaded.load()),
                    int(panel_prepared.load()), int(panel_disabled.load()));
    frame_no++;
    if (cover_on.load()) {
        hud::reset_state(f);
        hud::rect(f, 0, 0, f.width, f.height, 0x000000ffu);
        cover_frames++;
    }
    if (!logic::panel_due(patched.load(), home_open.load() && !runtime::in_session_now(), state_loaded.load(), panel_disabled.load(), panel_prepared.load())) {
        if (home_open.load() && !std::exchange(told_blocked, true))
            swofty::log("[main-menu] panel: not due on an open home screen (patched %d, state %d, prepared %d, disabled %d)",
                        int(patched.load()), int(state_loaded.load()), int(panel_prepared.load()), int(panel_disabled.load()));
        return;
    }
    std::shared_ptr<const Prepared> p;
    {
        std::lock_guard<std::mutex> g(prepared_lock);
        p = prepared;
    }
    if (!p) {
        if (!std::exchange(told_noprep, true))
            swofty::log("[main-menu] panel: due but the prepared snapshot is empty");
        return;
    }
    size_t verts_before = f.verts.size();
    LARGE_INTEGER t0, t1, freq;
    QueryPerformanceCounter(&t0);
    static bool told_drawn = false;
    hud::reset_state(f);
    uint64_t shown_for = GetTickCount64() - home_opened_at.load();
    float fade = shown_for < PANEL_DELAY_MS ? 0.0f : std::min(1.0f, float(shown_for - PANEL_DELAY_MS) / float(PANEL_FADE_MS));
    if (fade <= 0.0f)
        return;
    f.opacity = fade;
    float W = f.width, H = f.height;
    if (!std::exchange(told_drawn, true))
        swofty::log("[main-menu] panel: first draw on a %.0fx%.0f frame; video card %s, avatar %s", double(W), double(H),
                    p->video_slot >= 0 ? "yes" : "no", p->avatar_slot >= 0 ? "yes" : "no");
    const float k = logic::panel_scale(W, H);
    const float band_top = logic::ui_band(W, H).top;
    auto anchor = logic::scene_to_px(logic::CARD_RIGHT_X, logic::CARD_BOTTOM_Y, W, H);
    const float right = anchor.x;
    {
        hud::TextStyle name_style;
        name_style.px = 26 * k;
        name_style.rgba = 0xf3f5f9ffu;
        std::string who = p->s.signed_in ? (p->s.name.empty() ? std::string("Discord") : p->s.name) : std::string("Not signed in");
        float tmp = 0, nw = 0;
        hud::measure(name_style, who, &nw, &tmp);
        nw = nw * 1.06f + 4 * k;
        float avatar = 44 * k, pad = 10 * k, gap = 12 * k;
        float block_h = name_style.px * 1.3f;
        float ph = avatar + 2 * pad;
        float pw = std::max(190 * k, pad + avatar + gap + 3 * k + 9 * k + nw + 20 * k);
        float px = right - pw, py = band_top + 44 * k;
        fade_panel(f, px - 110 * k, py, pw + 110 * k, ph, 0.45f, 0.04f, k);
        float ax = px + pad, ay = py + pad;
        if (p->avatar_slot >= 0) {
            image_rounded(f, p->avatar_slot, ax, ay, avatar, avatar, avatar * 0.5f, 0, 0, 1, 1);
        } else if (p->s.signed_in) {
            hud::rect_rounded(f, ax, ay, avatar, avatar, avatar * 0.5f, 0x5865f2ffu);
            hud::TextStyle initial;
            initial.px = 24 * k;
            initial.rgba = 0xffffffffu;
            std::string letter = p->s.name.empty() ? "D" : p->s.name.substr(0, 1);
            float lw = 0;
            hud::measure(initial, letter, &lw, &tmp);
            hud::text(f, ax + (avatar - lw) * 0.5f, ay + (avatar - initial.px) * 0.5f, initial, letter);
        } else {
            hud::rect_rounded(f, ax, ay, avatar, avatar, avatar * 0.5f, 0x3a3f4bffu);
            hud::rect_rounded(f, ax + avatar * 0.34f, ay + avatar * 0.18f, avatar * 0.32f, avatar * 0.32f, avatar * 0.16f, 0xb9bfcdffu);
            hud::rect_rounded(f, ax + avatar * 0.2f, ay + avatar * 0.56f, avatar * 0.6f, avatar * 0.26f, avatar * 0.13f, 0xb9bfcdffu);
        }
        float bx = ax + avatar + gap, by = py + (ph - block_h) * 0.5f;
        hud::rect_rounded(f, bx, by, 3 * k, block_h, 1.5f * k, 0xe10600ffu);
        hud::text(f, bx + 12 * k, by + (block_h - name_style.px) * 0.5f, name_style, who);
    }
    if (p->video_slot >= 0 && p->video_w && p->video_h) {
        hud::TextStyle cap_style;
        cap_style.px = 16 * k;
        cap_style.rgba = 0xa9afbfffu;
        cap_style.tracking = 16 * k * 0.12f;
        hud::TextStyle title_style;
        title_style.px = 24 * k;
        title_style.rgba = 0xf3f5f9ffu;
        hud::TextStyle watch_style;
        watch_style.px = 20 * k;
        watch_style.rgba = 0xd5d9e3ffu;
        const std::string caption = "LATEST VIDEO";
        const bool pad_dev = bind_pad.load();
        const std::string key = logic::button_name(pad_dev);
        float tmp_h = 0, cap_w = 0, watch_w = 0, key_w = 0;
        hud::TextStyle key_probe;
        key_probe.px = 26 * k * 0.62f;
        hud::measure(cap_style, caption, &cap_w, &tmp_h);
        hud::measure(watch_style, "Watch", &watch_w, &tmp_h);
        hud::measure(key_probe, key, &key_w, &tmp_h);
        const float row_h = 26 * k;
        const float glyph_w = pad_dev ? std::max(row_h * 1.5f, key_w + 18 * k) : std::max(row_h, key_w + 14 * k);
        auto plan0 = logic::plan_card(k, 0);
        auto title_lines = wrap_title(title_style, p->s.video_title, plan0.text_max, 2);
        float title_w = 0;
        for (auto &line : title_lines) {
            float w = 0;
            hud::measure(title_style, line, &w, &tmp_h);
            title_w = std::max(title_w, w);
        }
        float text_w = std::max({cap_w, title_w, glyph_w + 10 * k + watch_w}) * 1.06f + 4 * k;
        auto card = logic::plan_card(k, text_w);
        float cx = right - card.width, cy = anchor.y - card.height;
        fade_panel(f, cx - 110 * k, cy, card.width + 110 * k, card.height, 0.45f, 0.04f, k);
        float tx = cx + card.pad + 12 * k;
        float cap_h = cap_style.px * 1.25f, title_h = title_style.px * 1.2f * float(title_lines.size());
        float block_h = cap_h + 6 * k + title_h + 8 * k + row_h;
        float ty = cy + (card.height - block_h) * 0.5f;
        hud::rect_rounded(f, cx + card.pad, ty, 3 * k, block_h, 1.5f * k, 0xe10600ffu);
        hud::text(f, tx, ty, cap_style, caption);
        float yy = ty + cap_h + 6 * k;
        for (auto &line : title_lines) {
            hud::text(f, tx, yy, title_style, line);
            yy += title_style.px * 1.2f;
        }
        yy += 8 * k;
        key_glyph(f, tx, yy, row_h, k, key, pad_dev);
        hud::text(f, tx + glyph_w + 10 * k, yy + (row_h - watch_style.px) * 0.5f, watch_style, "Watch");
        float want = card.thumb_w / card.thumb_h, have = float(p->video_w) / float(p->video_h);
        float v0 = 0, v1 = 1, u0 = 0, u1 = 1;
        if (have < want) {
            float keep = have / want;
            v0 = (1 - keep) * 0.5f;
            v1 = 1 - v0;
        } else {
            float keep = want / have;
            u0 = (1 - keep) * 0.5f;
            u1 = 1 - u0;
        }
        image_rounded(f, p->video_slot, cx + card.width - card.pad - card.thumb_w, cy + card.pad, card.thumb_w, card.thumb_h,
                      8 * k, u0, v0, u1, v1);
    }
    overlay_drawn = true;
    static bool told_verts = false;
    if (!std::exchange(told_verts, true))
        swofty::log("[main-menu] panel: first draw added %zu vertices and %zu batch(es) (frame total %zu / %zu)",
                    f.verts.size() - verts_before, f.batches.size(), f.verts.size(), f.batches.size());
    QueryPerformanceCounter(&t1);
    QueryPerformanceFrequency(&freq);
    double ms = double(t1.QuadPart - t0.QuadPart) * 1000.0 / double(freq.QuadPart);
    if (logic::panel_too_slow(ms) && !panel_disabled.exchange(true))
        swofty::log("[main-menu] panel: one draw took %.2f ms (budget %.1f ms); the panel is disabled for this session", ms,
                    logic::PANEL_BUDGET_MS);
}

}

namespace runtime {

bool main_menu_account(std::string &name)
{
    if (!main_menu::state_loaded.load())
        return false;
    std::lock_guard<std::mutex> g(main_menu::state_lock);
    if (!main_menu::state.signed_in || main_menu::state.name.empty())
        return false;
    name = main_menu::state.name;
    return true;
}

}
