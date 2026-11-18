#include "direct_start_tt.hpp"
#include "runtime.hpp"
#include "replay_grid_logic.hpp"

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <algorithm>
#include <array>
#include <atomic>
#include <cstddef>
#include <cstdio>
#include <cstring>
#include <iterator>
#include <map>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#include "apply/content/appliers.hpp"
#include "apply/moments.hpp"
#include "apply/scheduler.hpp"
#include "box_grid_logic.hpp"
#include "era_logic.hpp"
#include "tt_loaded_logic.hpp"

std::string settings_addon(const void *from, std::string &dll);
int settings_get(const char *id, uint32_t type, SwoftySettingValue *out);
namespace runtime {
std::string box_option_apply(size_t rule, const std::string &setting);
int box_option_put_back(size_t rule);
void box_options_forget();
void box_grid_set(const std::optional<box_grid::Rule> &rule, const std::string &box);
}

// Era picker addon row (swofty/era.hpp). The Grand Prix, Time Trial and split-screen category picker
// (the DynamicEraSelectTabViewModel, VM dynamic_era_select) shows its stock boxes in the VM tiles list;
// the launcher's UI build gives the screen a second row (scene slot addon_categories_row) whose spawner
// lists the VM addon_tiles list, selects into selected_addon_tile and raises addon_select_pressed.
// Addons register boxes with add_box; each names an entrant list of the host setup mode swofty_f1, which
// the core's own database rows add to every F1 mode and addons list-append their entrant lists onto.
//
// Build (site_FeTile_EraTilesBuilt, rsi = the view model, after the last stock box): the stock tiles stay
// in vm+0x690. The core keeps a second tile set laid out like it (era::Set) whose two handles hold the
// addon_tiles list (Vm_GetOrCreate_ListChild under the VM root) and the selected_addon_tile reference
// (Vm_Set_NodeRef). Each handle sits in its node's holder list and holds a reference. The Grand Prix and
// Time Trial each build their own picker view model, and before every build FeTile_Clear_EraTileSet_Thunk
// clears the set wherever its handles still hold nodes, another view model's too: it empties that
// addon_tiles list and unlinks and releases both handles. A handle overwritten while still linked would
// leave the old node's holder list running on into the new node's, so the game's teardown of the old view
// model would release the new nodes as well, and tearing the new picker down then made a pure call on its
// freed selected_addon_tile. A view model the game tears down unlinks and releases the handles itself.
// FeTile_Add_EraTile adds each box while vm+0x698 points at addon_tiles, so its VM node lands in the
// addon row and its Tile in the stock vector after the stock tiles, which the game frees with them.
// Boxes whose entrant list the screen's mode does not offer under swofty_f1 are skipped with one log line.
// On a Time Trial picker (the mode's name, *(*(mode+0x88)+0x278), is one the core reports as time_trial)
// a box with its own Time Trial entrant list gets that list as its tile's command, so its press selects
// and narrows to it; a box without one keeps its own list. Either way the box is skipped there, with one
// log line, when that list is not offered under swofty_f1 or holds more cars than a Time Trial session
// holds (era_logic.hpp); the car count is the list object's entrant records.
// The VM bool addon_tiles_visible says whether the addon row holds a box; the UI only moves Down from the
// stock row to the addon row while it is true.
//
// Press (FeTile_Press_EraTile, wrapped): addon_select_pressed runs the stock select_pressed path with
// vm+0x698, vm+0x6b8 and the vector start pointed at the addon row, so the pressed box raises
// entrant_list_selected with its entrant list exactly like a stock box. The stock handler does not
// bounds-check the child index against the vector, so the wrap refuses a press unless the vector still
// holds the stock and addon tiles of the last build of that view model.
//
// Narrowing (Game_F1WorldActivityContainer_SelectEntrantList, wrapped): every addon list shares the one
// host setup mode, whose entrant setup modes list all of them. Time Trial loads every car of every list
// the entrant setup mode offers and the Grand Prix takes its first list, so a press on an addon box
// leaves only that box's list in each swofty_f1 entrant setup mode until another box is pressed, and
// every press clears the setup's selected list, which the setup refill takes from the entrant setup
// mode's first list only while it is empty. A build of the boxes puts every list back for the
// availability checks, then narrows again to the addon list the setup has selected (setup +0xe18), or
// to the last addon box pressed while nothing is selected: a session can start from a rebuilt picker
// without a new press, and a Time Trial that loads all nine Classic Seasons lists overflows the fixed
// 43 MiB AUDIO_BUNDLE heap with their engine sound bundles.
//
// DRS per box: a narrowing also points every swofty_f1 game setup mode's DRS group (+0x1e0, a label holder) at the
// group the core added at boot for the box's rules.drs (content.package rules.drs). A box without a rule, a stock
// box press and a build that narrows to no addon list put the game's group back. The DRS system reads the current
// mode's group at session load, at a full safety car end and at a red flag restart; every other mode keeps its own.
// Points per box: the same narrowing points the race sessions of every swofty_f1 mode at the standings preset the
// boot apply added for the box's rules.points (logic::PointsSwaps); the same put backs restore the game's presets.
// Mandatory tyre change per box: the same narrowing writes the box's rules.mandatory_tyre_change into every
// swofty_f1 mode's 39 tyre regulation requirement flags (+0x8b) (logic::TyreChangeSwaps); the same put backs restore them.
// Tyre warmers per box: the same narrowing writes the box's rules.tyre_warmers into every swofty_f1 mode's
// has_tyre_warmers byte (+0x1c8) and the same put backs restore the game's value (logic::WarmerSwaps).
// Race time limit and weekend per box: the same narrowing writes the box's rules.race_time_limit_s into every swofty_f1
// mode's race_time_limit_in_seconds (+0xb4, logic::TimeLimitSwaps) and exchanges the weekend options (+0x190..+0x1a8)
// of the setup mode named by rules.weekend_mode with the first swofty_f1 mode (logic::WeekendSwaps); the same put
// backs undo both.
// Game options per box: the same narrowing sets the options store's safety car, formation lap, rules and penalties and
// corner cutting options to the box's rules.safety_car / formation_lap / penalties / corner_cutting (box_options.cpp)
// and the same put backs give the player's index back where an option still holds the box's.
// Grid rules per box: the same narrowing hands the box's rules.grid to box_grid.cpp, which re-ranks the weekend's
// qualifying classification; the same put backs clear it (the game's own grid).
// Qualifying tyres per box: the same narrowing hands the box's rules.qualifying_tyres option mask to the qualifying
// tyre hooks (quali_tyres_guard.cpp); the same put backs clear it.
//
// The three hooks are one core mixin set, offered at the first mixin commit after a box was registered,
// so a game without registered boxes runs none of them.
//
// SwoftyMod's split_season_picker off: the launcher's UI build leaves the screen with its single stock row,
// so the era picker is off from the start. available() says so and every box is refused as unavailable.
//
// Time Trial guard (era_logic.hpp): a stock box pressed on a Time Trial picker leaves swofty_f1 offering
// only its lists of at most 23 cars, none when no list fits; each Time Trial load counts the distinct cars of
// every list of the selected list's leaderboard that the Time Trial fill takes (era_logic.hpp) and leaves whole
// lists out, addon lists first, until they fit beside the game's 4 entries in the 26-slot table, and its unload
// puts them back; era_session_load (StateLoadSession, any mode) puts back what it held
// and, in Time Trial, logs the setup's resolved list and keeps a list past 23 cars out of the session;
// era_session_unloaded (StateUnloadSession Exiting) and every press put it back. The load's leaderboard cut is made on
// the swapped array, so each of them puts the cut back before the swap (logic::end_hold). The session vehicle guard is a
// separate set wrapping Sess_Setup_VehicleComponents, offered whether or not a box is registered.
//
// Setup mode guard: Sess_Apply_ContentAvailability rebuilds the game setup modes (it frees the old objects), so a
// set wrapping its entry drops the narrowing, DRS swaps and Time Trial hold the core keeps without writing them back.

namespace swofty_core::hooks {
void race_fuel_rules(const swofty_core::apply::content::FuelRules &rules);
}

namespace {

namespace mem = swofty::mem;
namespace logic = runtime::era;
using swofty::game::FeTile_Add_EraTile_t;
using swofty::game::FeTile_Clear_EraTileSet_Thunk_t;
using swofty::game::FeTile_Is_EraScenarioAvailable_Thunk_t;
using swofty::game::FeTile_Press_EraTile_t;
using swofty::game::Game_F1WorldActivityContainer_SelectEntrantList_t;
using swofty::game::Vm_GetOrCreate_ListChild_t;
using swofty::game::Vm_Set_Bool_t;
using swofty::game::Vm_Set_NodeRef_t;

constexpr const char *MIXIN = "swofty_core.dll (era picker)";
constexpr const char *SPLIT_OPTION = "split_season_picker";
constexpr uint32_t LOG_FIRST = 6;

constexpr size_t MODE_NAME = 0x278;
constexpr size_t VM_ROOT = 0x10;
constexpr size_t VM_ALLOCATOR = 0x270;
constexpr size_t VM_MODE = 0x670;
constexpr size_t VM_TILES = 0x690;
constexpr size_t VM_TILES_NODE = 0x698;
constexpr size_t VM_SELECTED_NODE = 0x6b8;
constexpr size_t VM_TILE_BEGIN = 0x6d8;
constexpr size_t VM_TILE_END = 0x6e0;
constexpr size_t TILE_ENTRY = 0x10;

constexpr size_t CONTAINER_MODE = 0x2f0;
constexpr size_t MODE_SETUP = 0x88;
constexpr size_t SETUP_MODES = 0x260;
constexpr size_t SETUP_MODES_VFUNC = 0x1d0;
constexpr size_t SELECTED_LIST = 0xe18;
constexpr size_t SETUP_ENTRANT_SETUP = 0xe10;
constexpr size_t SETUP_GAME_SETUP_MODE = 0x2b0;
using logic::ENTRANT_SETUP_MAX_ENTRANTS;
constexpr size_t SETUP_MODE_NAME = 0xd8;
constexpr size_t ENTRANT_SETUPS = 0x10;
constexpr size_t ENTRANT_SETUP_COUNT = 0x70;
constexpr size_t LISTS = logic::ESM_LISTS;
constexpr size_t LIST_COUNT = logic::ESM_LIST_COUNT;
constexpr size_t LIST_SLOTS = logic::ESM_LIST_SLOTS;
constexpr size_t LIST_NAME = 0x28;
constexpr int64_t ENTRANT_SETUP_SLOTS = 12;

using logic::Handle;

struct Set {
    void *vt0;
    Handle list;
    void *vt1;
    Handle selected;
    void *allocator;
    uint8_t *begin, *end, *cap;
};
static_assert(sizeof(Set) == 0x60);
static_assert(offsetof(Set, list) == VM_TILES_NODE - VM_TILES && offsetof(Set, selected) == VM_SELECTED_NODE - VM_TILES &&
              offsetof(Set, begin) == VM_TILE_BEGIN - VM_TILES);

struct Bound {
    uintptr_t site, press, select, add, available, list_child, node_ref, set_bool, clear, registry;
} b;

struct Built {
    uint8_t *vm = nullptr;
    size_t stock = 0, boxes = 0;
    std::vector<std::string> lists;
};

using Narrowed = logic::ListCut;

std::mutex lock;
std::vector<logic::Box> boxes;
std::vector<std::string> skipped;
std::atomic<bool> offered, settled, live, off;
std::string off_why;
Set set;
Built built;
Narrowed narrowed[16];
size_t narrowed_count;
std::string last_pick, last_press;
uint32_t logged_renarrow;
FeTile_Press_EraTile_t *press_orig;
Game_F1WorldActivityContainer_SelectEntrantList_t *select_orig;
uint32_t logged_builds, logged_narrow, logged_refused, logged_detached;
constexpr size_t BOX_TIME_TRIAL = offsetof(SwoftyEraBox, time_trial_entrant_list);
constexpr size_t BOX_ONLY = offsetof(SwoftyEraBox, only);
static_assert(BOX_TIME_TRIAL == 0x28 && BOX_ONLY == 0x30);
logic::DrsSwaps drs;
std::string last_drs_line;
logic::WarmerSwaps warmers;
std::string last_warmers_line;
logic::PointsSwaps points;
std::string last_points_line;
logic::TyreChangeSwaps tyre_change;
std::string last_tyre_change_line;
logic::TimeLimitSwaps time_limits;
logic::WeekendSwaps weekends;
std::string last_weekend_line;
std::string last_box_options_line;

template <class T> T at(const void *base, size_t off)
{
    return *reinterpret_cast<const T *>(static_cast<const uint8_t *>(base) + off);
}

template <class T> T &ref(void *base, size_t off)
{
    return *reinterpret_cast<T *>(static_cast<uint8_t *>(base) + off);
}

const char *name_of(const void *object, size_t off)
{
    auto *holder = object ? at<const uint8_t *>(object, off) : nullptr;
    return holder ? at<const char *>(holder, 0x10) : nullptr;
}

uint8_t *setup_owner(void *mode)
{
    auto *owner = mode ? at<uint8_t *>(mode, MODE_SETUP) : nullptr;
    return owner ? at<uint8_t *>(owner, SETUP_MODES) : nullptr;
}

template <class F> void for_each_host_setup(void *mode, F f)
{
    auto *modes = setup_owner(mode);
    if (!modes)
        return;
    auto get = reinterpret_cast<uint8_t *(*)(void *)>((*reinterpret_cast<void ***>(modes))[SETUP_MODES_VFUNC / 8]);
    uint8_t *vec = get(modes);
    if (!vec)
        return;
    for (auto **p = at<uint8_t **>(vec, 8), **e = at<uint8_t **>(vec, 0x10); p != e; ++p) {
        const char *name = name_of(*p, SETUP_MODE_NAME);
        if (!name || strcmp(name, logic::HOST))
            continue;
        int64_t n = at<int64_t>(*p, ENTRANT_SETUP_COUNT);
        auto **setups = reinterpret_cast<uint8_t **>(*p + ENTRANT_SETUPS);
        for (int64_t i = 0; i < n && i < ENTRANT_SETUP_SLOTS; ++i)
            f(setups[i]);
    }
}

template <class F> void for_each_host_mode(void *mode, F f)
{
    auto *modes = setup_owner(mode);
    if (!modes)
        return;
    auto get = reinterpret_cast<uint8_t *(*)(void *)>((*reinterpret_cast<void ***>(modes))[SETUP_MODES_VFUNC / 8]);
    uint8_t *vec = get(modes);
    if (!vec)
        return;
    for (auto **p = at<uint8_t **>(vec, 8), **e = at<uint8_t **>(vec, 0x10); p != e; ++p) {
        const char *name = name_of(*p, SETUP_MODE_NAME);
        if (name && !strcmp(name, logic::HOST))
            f(*p);
    }
}

void log_drs(const std::string &line)
{
    if (line == last_drs_line)
        return;
    last_drs_line = line;
    swofty::log("[drs] %s", line.c_str());
}

void apply_drs(void *mode, const char *list)
{
    auto box = swofty_core::apply::content::drs_for_box(list);
    if (box.label.empty()) {
        drs.put_back();
        std::string own;
        for_each_host_mode(mode, [&](uint8_t *m) {
            auto *holder = at<uint8_t *>(m, logic::MODE_DRS_GROUP);
            const char *text = holder && mem::readable(holder + logic::HOLDER_TEXT, 8) ? logic::holder_text(holder) : nullptr;
            if (own.empty() && text && mem::readable(text, 1))
                own = std::string(text, strnlen(text, logic::DRS_LABEL_MAX));
        });
        log_drs(box.line + (own.empty() ? std::string() : " (" + own + ")"));
        return;
    }
    logic::DrsHolder *holder = drs.holder(box.label);
    size_t modes = 0;
    for_each_host_mode(mode, [&](uint8_t *m) {
        drs.swap(m, holder);
        modes++;
    });
    log_drs(holder && modes ? box.line : box.line + "; not applied (no " + std::string(logic::HOST) + " mode found)");
}

void log_warmers(const std::string &line)
{
    if (line == last_warmers_line)
        return;
    last_warmers_line = line;
    swofty::log("[tyres] %s", line.c_str());
}

// Tyre warmers per box: the pressed box's rules.tyre_warmers goes into every swofty_f1 mode's has_tyre_warmers byte;
// a box without the rule puts the game's value back.
void apply_warmers(void *mode, const char *list)
{
    auto box = swofty_core::apply::content::tyre_warmers_for_box(list);
    if (!box.warmers) {
        warmers.put_back();
        log_warmers(box.line);
        return;
    }
    size_t modes = 0;
    for_each_host_mode(mode, [&](uint8_t *m) {
        warmers.swap(m, *box.warmers);
        modes++;
    });
    log_warmers(modes ? box.line : box.line + "; not applied (no " + std::string(logic::HOST) + " mode found)");
}

// Fuel rules per box: the pressed box's refuelling, race and qualifying fuel and parc ferme rule go to the per-frame fuel
// plan (hooks/car_live.cpp); a stock box press or leaving the boxes hands over no rules.
void apply_fuel(const char *list)
{
    swofty_core::hooks::race_fuel_rules(swofty_core::apply::content::fuel_rules_for_box(list));
}

void clear_fuel() { swofty_core::hooks::race_fuel_rules({}); }
void log_weekend(const std::string &line)
{
    if (line == last_weekend_line)
        return;
    last_weekend_line = line;
    swofty::log("[weekend] %s", line.c_str());
}

uint8_t *find_mode(void *mode, const std::string &want)
{
    auto *modes = setup_owner(mode);
    if (!modes || want.empty())
        return nullptr;
    auto get = reinterpret_cast<uint8_t *(*)(void *)>((*reinterpret_cast<void ***>(modes))[SETUP_MODES_VFUNC / 8]);
    uint8_t *vec = get(modes);
    if (!vec)
        return nullptr;
    for (auto **p = at<uint8_t **>(vec, 8), **e = at<uint8_t **>(vec, 0x10); p != e; ++p) {
        const char *name = name_of(*p, SETUP_MODE_NAME);
        if (name && want == name)
            return *p;
    }
    return nullptr;
}

// A weekend helper mode is listed in no game mode, so it is not in the game mode's list above: look it up in the
// store of every parsed setup mode (g_Game_Scenarios +0x40, logic::find_stored_mode). where says which list held it.
uint8_t *find_helper_mode(void *mode, const std::string &want, std::string &where)
{
    if (uint8_t *m = find_mode(mode, want)) {
        where = "the game mode's setup modes";
        return m;
    }
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_Scenarios") : 0, registry = 0;
    if (!slot || !mem::read(slot, registry) || !registry) {
        where = "the game mode's setup modes (the setup mode store needs g_Game_Scenarios)";
        return nullptr;
    }
    size_t seen = 0;
    std::vector<std::string> labels;
    uint8_t *m = logic::find_stored_mode(reinterpret_cast<const uint8_t *>(registry), want,
                                         [](const void *p, size_t n) { return mem::readable(p, n); }, &seen, &labels);
    if (m) {
        where = "the store of " + std::to_string(seen) + " parsed setup mode(s)";
        return m;
    }
    std::string all;
    for (auto &l : labels)
        all += (all.empty() ? "" : ", ") + l;
    where = "the game mode's setup modes and the store of " + std::to_string(seen) + " parsed setup mode(s): " +
            (all.empty() ? std::string("none") : all) +
            "; the game parses only rows with available true (the core sets it on a content.package's weekend_mode row at boot)";
    return m;
}

// Race time limit and weekend per box: rules.race_time_limit_s goes into every swofty_f1 mode's
// race_time_limit_in_seconds; rules.weekend_mode's weekend options are exchanged with the first swofty_f1 mode's. A
// box without a rule puts the game's value back.
void apply_weekend(void *mode, const char *list)
{
    auto box = swofty_core::apply::content::weekend_for_box(list);
    std::string line = box.line;
    if (!box.time_limit_s) {
        time_limits.put_back();
    } else {
        size_t modes = 0;
        uint32_t now = 0;
        for_each_host_mode(mode, [&](uint8_t *m) {
            time_limits.swap(m, *box.time_limit_s);
            if (!modes)
                now = at<uint32_t>(m, logic::MODE_RACE_TIME_LIMIT);
            modes++;
        });
        if (!modes)
            line += "; time limit not applied (no " + std::string(logic::HOST) + " mode found)";
        else
            line += "; time limit written into " + std::to_string(modes) + " " + std::string(logic::HOST) +
                    " mode(s) (race_time_limit_in_seconds now " + std::to_string(now) + " s)";
    }
    if (box.weekend_mode.empty()) {
        weekends.put_back();
    } else {
        uint8_t *host = nullptr;
        size_t hosts = 0;
        for_each_host_mode(mode, [&](uint8_t *m) {
            if (!host)
                host = m;
            hosts++;
        });
        std::string where;
        uint8_t *helper = find_helper_mode(mode, box.weekend_mode, where);
        if (!helper) {
            weekends.put_back();
            line += "; weekend not applied (no setup mode " + box.weekend_mode + " in " + where + ")";
        } else if (!weekends.swap(host, helper)) {
            line += "; weekend not applied (no " + std::string(logic::HOST) + " mode found)";
        } else {
            line += "; weekend applied (" + box.weekend_mode + " from " + where + ")";
            if (hosts > 1)
                line += "; weekend on the first of " + std::to_string(hosts) + " " + std::string(logic::HOST) + " modes";
        }
    }
    log_weekend(line);
}

void log_box_options(const std::string &line)
{
    if (line == last_box_options_line)
        return;
    last_box_options_line = line;
    swofty::log("[box-rules] %s", line.c_str());
}

// Game options per box: each of the pressed box's option rules goes into its options store option; a rule the box
// does not carry gives the player's index back.
void apply_box_options(const char *list)
{
    namespace content = swofty_core::apply::content;
    auto box = content::box_options_for_box(list);
    std::string line = box.line, put;
    for (size_t i = 0; i < content::BOX_OPTION_COUNT; ++i) {
        if (box.settings[i])
            line += "; " + std::string(content::BOX_OPTION_RULES[i].rule) + ": " + runtime::box_option_apply(i, *box.settings[i]);
        else if (runtime::box_option_put_back(i))
            put += (put.empty() ? "" : ", ") + std::string(content::BOX_OPTION_RULES[i].rule);
    }
    if (!put.empty())
        line += "; the player's " + put + " setting put back";
    log_box_options(line);
}

void put_back_box_options()
{
    namespace content = swofty_core::apply::content;
    std::string put;
    for (size_t i = 0; i < content::BOX_OPTION_COUNT; ++i)
        if (runtime::box_option_put_back(i))
            put += (put.empty() ? "" : ", ") + std::string(content::BOX_OPTION_RULES[i].rule);
    if (!put.empty())
        log_box_options("the player's " + put + " setting put back");
}

std::string last_grid_line;

void log_grid(const std::string &line)
{
    if (line == last_grid_line)
        return;
    last_grid_line = line;
    swofty::log("[grid] %s", line.c_str());
}

// Grid rules per box: the pressed box's rules.grid goes to box_grid.cpp; a box without the rule clears it.
void apply_grid(const char *list)
{
    auto box = swofty_core::apply::content::grid_for_box(list);
    runtime::box_grid_set(box.rule, list);
    log_grid(box.line);
}

// Qualifying tyres per box: the pressed box's rules.qualifying_tyres mask goes to the qualifying tyre hooks
// (quali_tyres_guard.cpp), which refuse those options in race sessions; a box without the rule stores 0.
void apply_quali_tyres(const char *list)
{
    auto box = swofty_core::apply::content::quali_tyres_for_box(list);
    runtime::quali_tyres_box(box.mask, box.mask ? box.line : std::string());
}

void log_points(const std::string &line)
{
    if (line == last_points_line)
        return;
    last_points_line = line;
    swofty::log("[points] %s", line.c_str());
}

// Points per box: the race sessions of every swofty_f1 mode point at the preset the boot apply added for the pressed
// box's rules.points, found among the parsed presets by its points; a box without the rule puts the game's back.
void apply_points(void *mode, const char *list)
{
    auto box = swofty_core::apply::content::points_for_box(list);
    if (!box.found) {
        points.put_back();
        log_points(box.line);
        return;
    }
    logic::Readable ok = [](const void *p, size_t n) { return mem::readable(p, n); };
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_Scenarios") : 0;
    const uint8_t *scenarios = nullptr, *begin = nullptr, *end = nullptr;
    logic::peek(ok, reinterpret_cast<const void *>(slot), 0, scenarios);
    logic::peek(ok, scenarios, logic::SCENARIOS_PRESETS_BEGIN, begin);
    logic::peek(ok, scenarios, logic::SCENARIOS_PRESETS_END, end);
    logic::PointsWant want{box.full, box.reduced, box.bonus, box.rule};
    const uint8_t *preset = logic::find_points_preset(ok, begin, end, want);
    if (!preset) {
        points.put_back();
        log_points(box.line + "; not applied (preset " + box.label + " not among the game's parsed presets)");
        return;
    }
    size_t sessions = 0;
    for_each_host_mode(mode, [&](uint8_t *m) {
        for (uint64_t *s : logic::race_sessions(ok, m)) {
            points.swap(s, reinterpret_cast<uint64_t>(preset));
            sessions++;
        }
    });
    log_points(sessions ? box.line + " on " + std::to_string(sessions) + " race session(s)"
                        : box.line + "; not applied (no race session in a " + std::string(logic::HOST) + " mode)");
}

void log_tyre_change(const std::string &line)
{
    if (line == last_tyre_change_line)
        return;
    last_tyre_change_line = line;
    swofty::log("[tyres] %s", line.c_str());
}

// Mandatory tyre change per box: the pressed box's rules.mandatory_tyre_change goes into every swofty_f1 mode's
// tyre regulation requirement flags; a box without the rule puts the game's flags back.
void apply_tyre_change(void *mode, const char *list)
{
    auto box = swofty_core::apply::content::tyre_change_for_box(list);
    if (!box.required) {
        tyre_change.put_back();
        log_tyre_change(box.line);
        return;
    }
    size_t modes = 0;
    for_each_host_mode(mode, [&](uint8_t *m) {
        tyre_change.swap(m, *box.required);
        modes++;
    });
    log_tyre_change(modes ? box.line : box.line + "; not applied (no " + std::string(logic::HOST) + " mode found)");
}

void put_back_drs()
{
    runtime::box_grid_set(std::nullopt, std::string());
    runtime::quali_tyres_box(0, std::string());
    if (size_t n = drs.put_back())
        log_drs(std::string(logic::HOST) + "'s DRS group put back on " + std::to_string(n) + " mode(s)");
    if (size_t n = warmers.put_back())
        log_warmers(std::string(logic::HOST) + "'s tyre warmer setting put back on " + std::to_string(n) + " mode(s)");
    if (size_t n = tyre_change.put_back())
        log_tyre_change(std::string(logic::HOST) + "'s tyre regulations put back on " + std::to_string(n) + " mode(s)");
    if (size_t n = time_limits.put_back())
        log_weekend(std::string(logic::HOST) + "'s race time limit put back on " + std::to_string(n) + " mode(s)");
    if (weekends.put_back())
        log_weekend(std::string(logic::HOST) + "'s own weekend put back");
    if (size_t n = points.put_back())
        log_points(std::string(logic::HOST) + "'s points put back on " + std::to_string(n) + " race session(s)");
    put_back_box_options();
}

logic::ListCars list_cars(const void *list);

void put_back_failed(const Narrowed &n)
{
    static uint32_t logged;
    if (logged++ >= LOG_FIRST)
        return;
    std::vector<std::string> names;
    for (void *list : logic::left_out(n)) {
        std::string name = list_cars(list).name;
        names.push_back(name.empty() ? "?" : name);
    }
    int64_t now = -1;
    if (n.setup)
        mem::read(reinterpret_cast<uintptr_t>(n.setup) + LIST_COUNT, now);
    swofty::log("[era] %s", logic::put_back_failed_line(names, now, n.kept, n.time_trial).c_str());
}

// Virtual entrant lists. The game loads the rows of entrants.entrant_lists up to the table's count as list objects,
// and a list's stat id is a byte (255) while an entrant setup mode holds 32 lists. A box can instead name a list
// whose rows lie above the count: the game never loads it, so it takes no id and no slot. On a press the core reads
// those rows with the game's own Sess_Parse_EntrantList (0x142453c40: int *ordinal, row key, list, driver
// registry; the row key is a GdbTableCursor {container char[256] at +0, row char[256] at +0x100, count at +0x200}
// that it joins as container.row; it pushes 0x88 EntrantRecords into list+8 = {allocator, begin, end, capacity}) and
// swaps the result into the host list VIRTUAL_HOST, which is in swofty_f1's entrant lists like any box list. The
// next press, or a press of a stock box, swaps the host's own entrants back. Parsed vectors stay cached per row
// path for the session's life: the game may still hold records of a list it copied, so nothing is freed.
constexpr const char *VIRTUAL_HOST = "swofty_virtual_list";
constexpr const char *PARSE_LIST_FN = "Sess_Parse_EntrantList";
constexpr const char *PARSE_LIST_MIXIN = "swofty_core.dll (virtual entrant lists)";
constexpr size_t LIST_VECTOR = 0x08, LIST_VECTOR_SIZE = 0x20;
constexpr size_t KEY_ROW = 0x100, KEY_COUNT = 0x200, KEY_SIZE = 0x210;
using ParseList_t = void(int *ordinal, const void *row_key, void *list, void *driver_registry);
ParseList_t *parse_list_orig;
std::atomic<void *> boot_registry;
std::atomic<bool> parse_list_bound;
std::map<std::string, std::array<uint8_t, LIST_VECTOR_SIZE>> parsed_lists;
struct Filled {
    void *host = nullptr;
    std::array<uint8_t, LIST_VECTOR_SIZE> own{};
    std::string row, label;
} filled;
uint32_t logged_virtual;

void parse_list_entry(int *ordinal, const void *row_key, void *list, void *registry)
{
    if (registry)
        boot_registry = registry;
    parse_list_orig(ordinal, row_key, list, registry);
}

// The row of entrants.entrant_lists labelled `label` that the game did not load: the build's virtual-list step puts
// those rows at VIRTUAL_BASE and up (packages/core/src/virtual_lists.rs, BASE).
constexpr int64_t VIRTUAL_BASE = 10000;
std::string virtual_row(const char *label)
{
    const SwoftyDb *db = runtime::db_table();
    if (!label || !db || !db->available || !db->available())
        return {};
    for (int64_t i = VIRTUAL_BASE; i < VIRTUAL_BASE + 4096; ++i) {
        std::string row = "entrants.entrant_lists.entrant_list_" + std::to_string(i);
        const char *name = nullptr;
        if (db->string_at((row + "._metadata_").c_str(), &name) != SWOFTY_ASSET_OK || !name)
            break;
        if (!strncmp(name, label, 128))
            return row;
    }
    return {};
}

void unfill_virtual()
{
    if (!filled.host)
        return;
    memcpy(static_cast<uint8_t *>(filled.host) + LIST_VECTOR, filled.own.data(), LIST_VECTOR_SIZE);
    filled = {};
}

// Parses `row` (once) and swaps its entrants into `host`; false leaves the host as it was.
bool fill_virtual(void *host, const std::string &row)
{
    if (!host || !parse_list_orig)
        return false;
    auto it = parsed_lists.find(row);
    if (it == parsed_lists.end()) {
        uintptr_t slot = swofty::api->resolve("g_Ne_DefaultAllocator"), heap = 0;
        if (!slot || !mem::read(slot, heap) || !heap)
            return false;
        alignas(16) uint8_t list[0x40] = {};
        alignas(16) uint8_t no_registry[0x20] = {}; // read as an empty {_, begin +8, end +0x10} vector
        memcpy(list + LIST_VECTOR, &heap, sizeof heap);
        // row is "entrants.entrant_lists.entrant_list_<n>": container and row name of the key the parser joins
        alignas(16) uint8_t key[KEY_SIZE] = {};
        size_t dot = row.rfind('.');
        if (dot == std::string::npos || dot >= KEY_ROW || row.size() - dot - 1 >= KEY_ROW)
            return false;
        memcpy(key, row.data(), dot);
        memcpy(key + KEY_ROW, row.data() + dot + 1, row.size() - dot - 1);
        int32_t uncounted = -1;
        memcpy(key + KEY_COUNT, &uncounted, sizeof uncounted);
        key[KEY_COUNT + 4] = 1;
        int ordinal = 1;
        void *registry = boot_registry.load();
        parse_list_orig(&ordinal, key, list, registry ? registry : no_registry);
        uintptr_t begin = 0, end = 0;
        memcpy(&begin, list + LIST_VECTOR + 8, 8);
        memcpy(&end, list + LIST_VECTOR + 16, 8);
        if (!begin || end <= begin)
            return false;
        std::array<uint8_t, LIST_VECTOR_SIZE> v{};
        memcpy(v.data(), list + LIST_VECTOR, LIST_VECTOR_SIZE);
        it = parsed_lists.emplace(row, v).first;
    }
    unfill_virtual();
    filled.host = host;
    filled.row = row;
    memcpy(filled.own.data(), static_cast<uint8_t *>(host) + LIST_VECTOR, LIST_VECTOR_SIZE);
    memcpy(static_cast<uint8_t *>(host) + LIST_VECTOR, it->second.data(), LIST_VECTOR_SIZE);
    return true;
}

// The list a press of `label` selects in the game: VIRTUAL_HOST for a virtual list, else the label itself.
std::string pressed_list(const char *label)
{
    if (!label)
        return {};
    if (!parse_list_bound || virtual_row(label).empty())
        return label;
    return VIRTUAL_HOST;
}

void restore_lists()
{
    unfill_virtual();
    logic::put_back_cuts(narrowed, narrowed_count, false, put_back_failed);
}

void restore()
{
    restore_lists();
    put_back_drs();
}

logic::GridLimit continue_grid;

void narrow(void *mode, const char *list)
{
    restore_lists();
    std::string row = parse_list_bound ? virtual_row(list) : std::string();
    const char *want = row.empty() ? list : VIRTUAL_HOST;
    for_each_host_setup(mode, [&](uint8_t *setup) {
        int64_t count = at<int64_t>(setup, LIST_COUNT);
        if (count <= 1 || count > int64_t(LIST_SLOTS) || narrowed_count == std::size(narrowed))
            return;
        auto **lists = reinterpret_cast<void **>(setup + LISTS);
        void *hit = nullptr;
        for (int64_t i = 0; i < count && !hit; ++i) {
            const char *name = name_of(lists[i], LIST_NAME);
            if (name && !strcmp(name, want))
                hit = lists[i];
        }
        if (!hit)
            return;
        if (!row.empty() && filled.host != hit) {
            bool ok = fill_virtual(hit, row);
            if (logged_virtual++ < LOG_FIRST)
                swofty::log("[era] %s pressed: virtual list %s %s %s", list, row.c_str(),
                            ok ? "read into" : "could not be read into", VIRTUAL_HOST);
            if (!ok)
                return;
            filled.label = list;
        }
        Narrowed &n = narrowed[narrowed_count++];
        n.setup = setup;
        n.count = count;
        n.kept = 1;
        n.time_trial = false;
        n.grid = {};
        std::string grid = n.grid.raise(setup, list_cars(hit).cars);
        if (!grid.empty())
            swofty::log("[era] %s pressed: %s", list, grid.c_str());
        n.kept_lists[0] = hit;
        memcpy(n.lists, lists, size_t(count) * sizeof(void *));
        lists[0] = hit;
        ref<int64_t>(setup, LIST_COUNT) = 1;
        if (logged_narrow++ < LOG_FIRST)
            swofty::log("[era] %s offers only %s (of %lld lists)", logic::HOST, list, (long long)count);
    });
    apply_drs(mode, list);
    apply_warmers(mode, list);
    apply_tyre_change(mode, list);
    apply_fuel(list);
    apply_weekend(mode, list);
    // After the weekend exchange: the points swap edits the race sessions the host mode holds now.
    apply_points(mode, list);
    apply_box_options(list);
    apply_grid(list);
    apply_quali_tyres(list);
}

bool time_trial(void *mode)
{
    uintptr_t holder = 0, name = 0;
    std::string label;
    if (!mode || !mem::read(reinterpret_cast<uintptr_t>(mode) + MODE_SETUP, holder) || !holder ||
        !mem::read(holder + MODE_NAME, name) || !mem::c_string(name, label, 64))
        return false;
    return logic::time_trial_mode(label);
}

int64_t cars_of(void *mode, const std::string &list)
{
    int64_t cars = -1;
    for_each_host_setup(mode, [&](uint8_t *setup) {
        int64_t count = at<int64_t>(setup, LIST_COUNT);
        auto **lists = reinterpret_cast<uint8_t **>(setup + LISTS);
        for (int64_t i = 0; i < count && i < int64_t(LIST_SLOTS) && cars < 0; ++i) {
            const char *name = name_of(lists[i], LIST_NAME);
            uintptr_t begin = 0, end = 0;
            if (!name || strcmp(name, list.c_str()) ||
                !mem::read(reinterpret_cast<uintptr_t>(lists[i]) + logic::LIST_ENTRANTS_BEGIN, begin) ||
                !mem::read(reinterpret_cast<uintptr_t>(lists[i]) + logic::LIST_ENTRANTS_END, end))
                continue;
            cars = logic::entrant_count(begin, end);
        }
    });
    return cars;
}

logic::ListCars list_cars(const void *list)
{
    logic::ListCars out;
    uintptr_t holder = 0, name = 0, begin = 0, end = 0;
    auto at_list = reinterpret_cast<uintptr_t>(list);
    if (!list || !mem::read(at_list + LIST_NAME, holder) || !holder || !mem::read(holder + 0x10, name) ||
        !mem::c_string(name, out.name, 64))
        out.name.clear();
    if (list && mem::read(at_list + logic::LIST_ENTRANTS_BEGIN, begin) && mem::read(at_list + logic::LIST_ENTRANTS_END, end))
        out.cars = logic::entrant_count(begin, end);
    return out;
}

std::vector<logic::ListCars> lists_of(const uint8_t *entrant_setup, std::vector<void *> &ptrs)
{
    std::vector<logic::ListCars> out;
    int64_t count = 0;
    if (!entrant_setup || !mem::read(reinterpret_cast<uintptr_t>(entrant_setup) + LIST_COUNT, count) || count <= 0 ||
        count > int64_t(LIST_SLOTS))
        return out;
    for (int64_t i = 0; i < count; ++i) {
        uintptr_t list = 0;
        if (!mem::read(reinterpret_cast<uintptr_t>(entrant_setup) + LISTS + size_t(i) * 8, list))
            list = 0;
        ptrs.push_back(reinterpret_cast<void *>(list));
        out.push_back(list_cars(reinterpret_cast<void *>(list)));
    }
    return out;
}

std::mutex held_lock;
logic::TtHold held;
bool holding;

// Ends the Time Trial hold, putting the Time Trial cuts back before the swap (logic::end_hold); `cuts` puts them back
// even without a swap held.
logic::CutsPutBack put_back(bool cuts = false)
{
    continue_grid.restore();
    return logic::end_hold(narrowed, narrowed_count, held, holding, cuts, put_back_failed);
}

constexpr const char *VEHICLE_MIXIN = "swofty_core.dll (session vehicle guard)";
constexpr const char *VEHICLE_FN = "Sess_Setup_VehicleComponents";
constexpr size_t MODULE_VEHICLES_BEGIN = 0x28;
constexpr size_t MODULE_VEHICLES_END = 0x30;
constexpr size_t MODULE_MODE_CONTEXT = 0x80;
using SetupVehicles_t = void(void *module);
SetupVehicles_t *vehicles_orig;
std::atomic<bool> vehicles_offered, vehicles_settled, vehicles_bound;
uint32_t logged_vehicles;

std::string session_list(uintptr_t ctx)
{
    uintptr_t holder = 0, setup = 0, list = 0, gsm = 0, gsm_holder = 0, gsm_name = 0;
    std::string mode = "?";
    if (!ctx || !mem::read(ctx + MODE_SETUP, holder) || !holder || !mem::read(holder + SETUP_MODES, setup) || !setup)
        return "an unread list";
    if (mem::read(setup + SETUP_GAME_SETUP_MODE, gsm) && gsm && mem::read(gsm + SETUP_MODE_NAME, gsm_holder) && gsm_holder &&
        mem::read(gsm_holder + 0x10, gsm_name))
        mem::c_string(gsm_name, mode, 64);
    logic::ListCars cars = mem::read(setup + SELECTED_LIST, list) && list ? list_cars(reinterpret_cast<void *>(list))
                                                                         : logic::ListCars{};
    return "list " + (cars.name.empty() ? std::string("?") : cars.name) + " (" + std::to_string(cars.cars) +
           " cars) of setup mode " + mode;
}

uint32_t trace_lines;
std::atomic<int64_t> tt_budget{-1};

int64_t descriptor_participants(uintptr_t holder, size_t off)
{
    uintptr_t desc = 0, begin = 0, end = 0;
    if (!holder || !mem::read(holder + off, desc) || !desc || !mem::read(desc + logic::DESC_PARTICIPANTS_BEGIN, begin) ||
        !mem::read(desc + logic::DESC_PARTICIPANTS_END, end))
        return -1;
    return logic::participant_count(begin, end);
}

void session_trace(const std::string &where, int64_t vehicles)
{
    if (trace_lines >= logic::TRACE_LINES)
        return;
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_Manager") : 0, gm = 0, ctx = 0, holder = 0, setup = 0;
    if (!slot || !mem::read(slot, gm) || !gm)
        return;
    logic::SessionTrace t;
    t.where = where;
    t.vehicles = vehicles;
    uintptr_t name = 0, module = 0;
    if (mem::read(gm + offsetof(swofty::game::GameManager, mode_context), ctx) && ctx && mem::read(ctx + MODE_SETUP, holder) &&
        holder) {
        if (mem::read(holder + MODE_NAME, name) && name)
            mem::c_string(name, t.mode, 64);
        if (mem::read(holder + SETUP_MODES, setup) && setup) {
            uintptr_t begin = 0, end = 0, list = 0, gsm = 0, gsm_holder = 0, gsm_name = 0;
            if (mem::read(setup + logic::MGR_SESSION_BEGIN, begin) && mem::read(setup + logic::MGR_SESSION_END, end)) {
                t.entrants = logic::entrant_count(begin, end);
                for (int64_t i = 0; i < t.entrants && i < int64_t(logic::MAX_PARTICIPANTS); ++i) {
                    uint16_t team = 0;
                    mem::read(begin + size_t(i) * logic::ENTRANT_RECORD + logic::ENTRANT_TEAM, team);
                    t.teams.push_back(team);
                }
            }
            if (mem::read(setup + SELECTED_LIST, list) && list) {
                logic::ListCars cars = list_cars(reinterpret_cast<void *>(list));
                t.list = cars.name;
                t.list_cars = cars.cars;
            }
            if (mem::read(setup + SETUP_GAME_SETUP_MODE, gsm) && gsm && mem::read(gsm + SETUP_MODE_NAME, gsm_holder) &&
                gsm_holder && mem::read(gsm_holder + 0x10, gsm_name) && gsm_name)
                mem::c_string(gsm_name, t.setup_mode, 64);
        }
    }
    t.budget = tt_budget.load();
    static int64_t last_participants = -1;
    t.previous = last_participants;
    t.gm_session = descriptor_participants(gm, logic::GM_SESSION);
    if (mem::read(gm + logic::GM_MODULE, module) && module) {
        t.module_session = descriptor_participants(module, logic::MODULE_SESSION);
        t.module_next = descriptor_participants(module, logic::MODULE_SESSION_NEXT);
    }
    trace_lines++;
    swofty::log("[era] %s", logic::session_trace_line(t).c_str());
    last_participants = t.gm_session;
    std::string mismatch = logic::budget_mismatch(t);
    if (!mismatch.empty())
        swofty::log("[era] %s", mismatch.c_str());
}

void vehicles_entry(void *module)
{
    auto *m = static_cast<uint8_t *>(module);
    uintptr_t begin = at<uintptr_t>(m, MODULE_VEHICLES_BEGIN), end = at<uintptr_t>(m, MODULE_VEHICLES_END);
    session_trace("Sess_Setup_VehicleComponents", int64_t(logic::session_vehicles(begin, end)));
    runtime::replay_grid::session_vehicles.store(int64_t(logic::session_vehicles(begin, end)));
    uintptr_t within = logic::vehicles_end_within_table(begin, end);
    if (within == end)
        return vehicles_orig(module);
    if (logged_vehicles++ < LOG_FIRST)
        swofty::log("[era] the session holds %zu vehicles and its vehicle table %zu; setting up the first %zu (the "
                    "session's %s)",
                    logic::session_vehicles(begin, end), logic::TT_TABLE, logic::TT_TABLE,
                    session_list(at<uintptr_t>(m, MODULE_MODE_CONTEXT)).c_str());
    ref<uintptr_t>(m, MODULE_VEHICLES_END) = within;
    vehicles_orig(module);
    if (at<uintptr_t>(m, MODULE_VEHICLES_END) == within)
        ref<uintptr_t>(m, MODULE_VEHICLES_END) = end;
}

std::vector<std::string> kept_out;
uint32_t logged_kept;

bool keep_time_trial_lists(void *mode)
{
    bool any = false;
    for_each_host_setup(mode, [&](uint8_t *setup) {
        std::vector<void *> ptrs;
        auto info = lists_of(setup, ptrs);
        auto keep = logic::time_trial_keep(info);
        if (!logic::time_trial_narrows(info, keep) || narrowed_count == std::size(narrowed))
            return;
        Narrowed &n = narrowed[narrowed_count++];
        n.setup = setup;
        n.count = int64_t(info.size());
        n.kept = int64_t(keep.size());
        n.time_trial = true;
        n.grid = {};
        memcpy(n.lists, ptrs.data(), info.size() * sizeof(void *));
        for (size_t i = 0; i < keep.size(); ++i)
            n.kept_lists[i] = ptrs[keep[i]];
        memcpy(setup + LISTS, n.kept_lists, keep.size() * sizeof(void *));
        ref<int64_t>(setup, LIST_COUNT) = n.kept;
        any = true;
        for (size_t i = 0; i < info.size(); ++i) {
            if (!logic::oversized(info[i].cars) ||
                std::find(kept_out.begin(), kept_out.end(), info[i].name) != kept_out.end())
                continue;
            kept_out.push_back(info[i].name);
            if (logged_kept++ < LOG_FIRST * 4)
                swofty::log("[era] %s keeps %s out of Time Trial while a stock box is picked: it holds %lld cars; Time "
                            "Trial holds at most %zu (the game adds %zu cars of its own to a table of %zu)%s",
                            logic::HOST, info[i].name.c_str(), (long long)info[i].cars, logic::TT_MAX_CARS,
                            logic::TT_EXTRA_CARS, logic::TT_TABLE,
                            keep.empty() ? "; no list of it fits, so it offers none until the next press" : "");
        }
    });
    return any;
}

struct ListSlot {
    uint8_t *setup;
    size_t index;
};

struct LeaderboardList {
    void *list;
    std::vector<ListSlot> slots;
};

bool leaderboard_of(const void *list, uint8_t &id)
{
    uint8_t set = 0;
    auto at_list = reinterpret_cast<uintptr_t>(list);
    return list && mem::read(at_list + logic::LIST_LEADERBOARD_SET, set) && set &&
           mem::read(at_list + logic::LIST_LEADERBOARD, id);
}

std::vector<uint32_t> vehicle_refs(const void *list)
{
    std::vector<uint32_t> refs;
    uintptr_t begin = 0, end = 0;
    auto at_list = reinterpret_cast<uintptr_t>(list);
    if (!mem::read(at_list + logic::LIST_ENTRANTS_BEGIN, begin) || !mem::read(at_list + logic::LIST_ENTRANTS_END, end))
        return refs;
    int64_t n = logic::entrant_count(begin, end);
    for (int64_t i = 0; i < n; ++i) {
        uint32_t ref = 0;
        if (mem::read(begin + size_t(i) * logic::ENTRANT_RECORD + logic::ENTRANT_VEHICLE, ref))
            refs.push_back(ref);
    }
    return refs;
}

bool box_list(const std::string &name)
{
    std::lock_guard<std::mutex> g(lock);
    for (const auto &box : boxes)
        if (box.entrant_list == name || box.time_trial == name)
            return true;
    return false;
}

uint8_t *resolved_esm(uintptr_t setup, uint8_t *esm)
{
    uint8_t id = 0;
    uintptr_t slots = 0, meta = 0, mask = 0;
    if (!esm || !mem::read(reinterpret_cast<uintptr_t>(esm), id) || !mem::read(setup + logic::MGR_OVERRIDE_MASK, mask) ||
        !mask || !mem::read(setup + logic::MGR_OVERRIDES, slots) || !mem::read(setup + logic::MGR_OVERRIDE_META, meta) ||
        !slots || meta < slots || (meta - slots) % logic::OVERRIDE_SLOT)
        return esm;
    size_t count = (meta - slots) / logic::OVERRIDE_SLOT;
    if (count > logic::MAX_OVERRIDE_SLOTS || !mem::readable(reinterpret_cast<void *>(slots), count * logic::OVERRIDE_SLOT) ||
        !mem::readable(reinterpret_cast<void *>(meta), count))
        return esm;
    auto o = logic::override_for(id, reinterpret_cast<const uint8_t *>(slots), reinterpret_cast<const uint8_t *>(meta), count);
    static uint32_t logged_null;
    if (o.found && !o.to && logged_null++ < LOG_FIRST)
        swofty::log("[era] the Time Trial's override row for entrant setup mode %u is empty: the game leaves that mode "
                    "out, so the budget does too",
                    unsigned(id));
    return o.found ? static_cast<uint8_t *>(const_cast<void *>(o.to)) : esm;
}

std::vector<uint32_t> pool_refs(uint32_t pools)
{
    std::vector<uint32_t> refs;
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_Manager") : 0, gm = 0, es = 0;
    if (!slot || !mem::read(slot, gm) || !gm || !mem::read(gm + runtime::manager_entrants_system(), es) || !es)
        return refs;
    for (auto [bit, off] : {std::pair<uint32_t, size_t>{logic::TAKES_POOL_A, logic::ES_POOL_A},
                            std::pair<uint32_t, size_t>{logic::TAKES_POOL_B, logic::ES_POOL_B}}) {
        uintptr_t pool = 0, begin = 0, end = 0;
        if (!(pools & bit) || !mem::read(es + off, pool) || !pool || !mem::read(pool + 8, begin) ||
            !mem::read(pool + 0x10, end))
            continue;
        auto got = logic::pool_refs_from(begin, end, [](uintptr_t a, auto &v) { return mem::read(a, v); });
        static uint32_t logged_cap;
        if (got.capped && logged_cap++ < LOG_FIRST)
            swofty::log("[era] entrants system pool +0x%zx holds more than %zu vehicle slots; the Time Trial budget counts "
                        "the first %zu",
                        off, logic::MAX_POOL, logic::MAX_POOL);
        refs.insert(refs.end(), got.refs.begin(), got.refs.end());
    }
    return refs;
}

void time_trial_budget(void *mode, uintptr_t setup)
{
    static uint32_t logged;
    uintptr_t selected = 0;
    uint8_t board = 0;
    if (!mem::read(setup + SELECTED_LIST, selected) || !selected || !leaderboard_of(reinterpret_cast<void *>(selected), board))
        return;
    auto *modes = setup_owner(mode);
    if (!modes)
        return;
    auto get = reinterpret_cast<uint8_t *(*)(void *)>((*reinterpret_cast<void ***>(modes))[SETUP_MODES_VFUNC / 8]);
    uint8_t *vec = get(modes);
    if (!vec)
        return;
    std::vector<LeaderboardList> found{{reinterpret_cast<void *>(selected), {}}};
    std::vector<bool> addon{false};
    uint32_t pools = 0;
    for (auto **p = at<uint8_t **>(vec, 8), **e = at<uint8_t **>(vec, 0x10); p != e; ++p) {
        const char *gsm = name_of(*p, SETUP_MODE_NAME);
        bool host = gsm && !strcmp(gsm, logic::HOST);
        int64_t n = at<int64_t>(*p, ENTRANT_SETUP_COUNT);
        auto **esms = reinterpret_cast<uint8_t **>(*p + ENTRANT_SETUPS);
        for (int64_t k = 0; k < n && k < ENTRANT_SETUP_SLOTS; ++k) {
            uint8_t *esm = resolved_esm(setup, esms[k]);
            int64_t count = 0;
            uint32_t player = 0, second = 0;
            if (!esm || !mem::read(reinterpret_cast<uintptr_t>(esm) + LIST_COUNT, count) || count <= 0 ||
                count > int64_t(LIST_SLOTS))
                continue;
            mem::read(reinterpret_cast<uintptr_t>(esm) + logic::ESM_VEHICLE_FLAGS, player);
            mem::read(reinterpret_cast<uintptr_t>(esm) + logic::ESM_VEHICLE_FLAGS + 4, second);
            uint32_t takes = logic::vehicle_flags(player, second);
            auto **lists = reinterpret_cast<void **>(esm + LISTS);
            for (int64_t i = 0; i < count; ++i) {
                uint8_t id = 0;
                if (!leaderboard_of(lists[i], id) || id != board)
                    continue;
                pools |= takes & (logic::TAKES_POOL_A | logic::TAKES_POOL_B);
                if (!(takes & logic::TAKES_LIST) && lists[i] != reinterpret_cast<void *>(selected))
                    continue;
                size_t at_found = found.size();
                for (size_t f = 0; f < found.size(); ++f)
                    if (found[f].list == lists[i])
                        at_found = f;
                if (at_found == found.size()) {
                    found.push_back({lists[i], {}});
                    addon.push_back(false);
                }
                found[at_found].slots.push_back({esm, size_t(i)});
                addon[at_found] = addon[at_found] || host;
            }
        }
    }
    std::vector<logic::TtList> lists;
    for (size_t f = 0; f < found.size(); ++f) {
        const char *name = name_of(found[f].list, LIST_NAME);
        logic::TtList l;
        l.name = name ? name : "?";
        l.selected = f == 0;
        l.addon = !l.selected && (addon[f] || box_list(l.name));
        l.refs = vehicle_refs(found[f].list);
        lists.push_back(std::move(l));
    }
    if (pools) {
        logic::TtList pool;
        pool.name = "entrants system pools";
        pool.fixed = true;
        pool.refs = pool_refs(pools);
        lists.push_back(std::move(pool));
    }
    auto budget = logic::time_trial_budget(lists);
    size_t unrecorded = 0;
    for (size_t d : budget.dropped)
        for (const auto &slot : found[d].slots)
            if (!logic::cut_list(narrowed, narrowed_count, std::size(narrowed), slot.setup, found[d].list))
                unrecorded++;
    if (unrecorded)
        swofty::log("[era] Time Trial cut skipped in %zu entrant setup mode(s): the core's restore stack of %zu is full, "
                    "so those lists stay offered",
                    unrecorded, std::size(narrowed));
    tt_budget = int64_t(budget.cars + logic::TT_OWN_ENTRIES);
    if (logged++ < LOG_FIRST * 3)
        swofty::log("[era] %s", logic::time_trial_budget_line(lists, budget, board).c_str());
}

bool skip_once(const std::string &key)
{
    std::lock_guard<std::mutex> g(lock);
    if (std::find(skipped.begin(), skipped.end(), key) != skipped.end())
        return false;
    skipped.push_back(key);
    return true;
}

bool is_ours(const char *list)
{
    return list && std::find(built.lists.begin(), built.lists.end(), list) != built.lists.end();
}

bool renarrow(void *mode)
{
    auto *modes = setup_owner(mode);
    if (!modes)
        return false;
    const char *selected = name_of(at<void *>(modes, SELECTED_LIST), LIST_NAME);
    std::string list = is_ours(selected) ? selected : selected ? std::string() : last_pick;
    if (list.empty() || !is_ours(list.c_str()))
        return false;
    narrow(mode, list.c_str());
    if (logged_renarrow++ < LOG_FIRST)
        swofty::log("[era] picker built with %s still selected (%s): %s narrowed to it again", list.c_str(),
                    selected ? "the setup's selected list" : "the last addon box pressed", logic::HOST);
    return true;
}

void *registry()
{
    return *reinterpret_cast<void **>(b.registry);
}

uint8_t *addon_list(uint8_t *vm)
{
    auto *root = at<uint8_t *>(vm, VM_ROOT);
    if (!root)
        return nullptr;
    uint64_t key = logic::ADDON_TILES;
    auto **h = reinterpret_cast<Vm_GetOrCreate_ListChild_t *>(b.list_child)(registry(), root + 8, reinterpret_cast<const uint32_t *>(&key));
    return h ? static_cast<uint8_t *>(*h) : nullptr;
}

uint8_t *addon_selection(uint8_t *vm)
{
    auto *root = at<uint8_t *>(vm, VM_ROOT);
    if (!root)
        return nullptr;
    Handle empty{};
    auto **h = reinterpret_cast<Vm_Set_NodeRef_t *>(b.node_ref)(registry(), root + 8, logic::SELECTED_ADDON_TILE, &empty, 1);
    return h ? static_cast<uint8_t *>(*h) : nullptr;
}

bool reset_set(uint8_t *vm, uint8_t *list)
{
    if ((set.list.node || set.selected.node) && (set.list.node != list || built.vm != vm) && logged_detached++ < LOG_FIRST)
        swofty::log("[era] detached the addon row from a previous view model (its boxes there cleared, its two nodes "
                    "unlinked and released)");
    logic::relink(set, list, [](Set &s) { reinterpret_cast<FeTile_Clear_EraTileSet_Thunk_t *>(b.clear)(&s); });
    uint8_t *selected = addon_selection(vm);
    if (!selected)
        return false;
    logic::link(set.selected, selected);
    return true;
}

void site_entry(SwoftyContext *c)
{
    auto *vm = reinterpret_cast<uint8_t *>(c->rsi);
    if (!vm || !live || off)
        return;
    restore_lists();
    void *mode = at<void *>(vm, VM_MODE);
    void *alloc = at<void *>(vm, VM_ALLOCATOR);
    auto *begin = at<uint8_t *>(vm, VM_TILE_BEGIN);
    auto *end = at<uint8_t *>(vm, VM_TILE_END);
    size_t stock = end > begin ? size_t(end - begin) / TILE_ENTRY : 0;
    uint8_t *list = addon_list(vm);
    if (!list || !reset_set(vm, list)) {
        put_back_drs();
        built = {};
        swofty::log("[era] the view model's addon_tiles or selected_addon_tile node could not be made; no addon row");
        return;
    }
    std::vector<logic::Box> order;
    {
        std::lock_guard<std::mutex> g(lock);
        order = logic::ordered(boxes);
    }
    auto available = reinterpret_cast<FeTile_Is_EraScenarioAvailable_Thunk_t *>(b.available);
    auto add = reinterpret_cast<FeTile_Add_EraTile_t *>(b.add);
    uint8_t *tiles_node = at<uint8_t *>(vm, VM_TILES_NODE);
    bool tt = time_trial(mode);
    built = {vm, stock, 0, {}};
    ref<uint8_t *>(vm, VM_TILES_NODE) = list;
    for (const auto &box : order) {
        if (built.boxes >= logic::MAX_BOXES)
            break;
        if (!logic::shown(box, tt))
            continue;
        if (!mode || !available(mode, logic::HOST, pressed_list(box.entrant_list.c_str()).c_str())) {
            if (skip_once(box.entrant_list))
                swofty::log("[era] box %s from %s skipped: this screen's mode does not offer the entrant list under %s "
                            "(it must exist and be in %s's entrant lists)",
                            box.entrant_list.c_str(), box.owner.c_str(), logic::HOST, logic::HOST);
            continue;
        }
        const std::string &list = logic::list_for(box, tt);
        if (tt && list != box.entrant_list && !available(mode, logic::HOST, pressed_list(list.c_str()).c_str())) {
            if (skip_once("time_trial " + box.entrant_list))
                swofty::log("[era] box %s from %s skipped in Time Trial: its Time Trial entrant list %s is not offered "
                            "under %s (it must exist and be in %s's entrant lists)",
                            box.entrant_list.c_str(), box.owner.c_str(), list.c_str(), logic::HOST, logic::HOST);
            continue;
        }
        if (tt) {
            int64_t cars = cars_of(mode, list);
            std::string why = logic::time_trial_refusal(box, cars);
            if (!why.empty()) {
                if (skip_once("time_trial " + box.entrant_list))
                    swofty::log("[era] box %s from %s skipped in Time Trial: %s", box.entrant_list.c_str(),
                                box.owner.c_str(), why.c_str());
                continue;
            }
            if (cars < 0 && skip_once("time_trial count " + list))
                swofty::log("[era] box %s from %s: the car count of its Time Trial list %s could not be read; not checked",
                            box.entrant_list.c_str(), box.owner.c_str(), list.c_str());
        }
        char node[0x20];
        snprintf(node, sizeof node, "select_tile_%zu", built.boxes);
        std::string image = logic::image_of(box);
        add(vm + VM_TILES, alloc, uint8_t(built.boxes), node, box.entrant_list.c_str(), 1, box.title.c_str(),
            list.c_str(), image.c_str(), 0);
        built.lists.push_back(list);
        built.boxes++;
    }
    ref<uint8_t *>(vm, VM_TILES_NODE) = tiles_node;
    if (!renarrow(mode)) {
        put_back_drs();
        if (tt)
            keep_time_trial_lists(mode);
    }
    if (auto *root = at<uint8_t *>(vm, VM_ROOT)) {
        uint8_t visible = built.boxes > 0;
        reinterpret_cast<Vm_Set_Bool_t *>(b.set_bool)(registry(), root + 8, logic::ADDON_TILES_VISIBLE, &visible, 1);
    }
    if (logged_builds++ < LOG_FIRST)
        swofty::log("[era] era select%s: %zu stock box(es), %zu addon box(es) of %zu registered",
                    tt ? " (Time Trial)" : "", stock, built.boxes, order.size());
}

char *press_entry(void *vm, void *command)
{
    auto *cmd = command ? *static_cast<const uint32_t **>(command) : nullptr;
    if (!cmd || !mem::readable(cmd, 4) || *cmd != logic::ADDON_PRESS)
        return press_orig(vm, command);
    auto *v = static_cast<uint8_t *>(vm);
    auto *begin = at<uint8_t *>(v, VM_TILE_BEGIN);
    auto *end = at<uint8_t *>(v, VM_TILE_END);
    size_t count = end > begin ? size_t(end - begin) / TILE_ENTRY : 0;
    std::string why = logic::press_refusal(v == built.vm, set.list.node && set.selected.node, count, built.stock,
                                           built.boxes);
    if (!why.empty()) {
        if (logged_refused++ < LOG_FIRST)
            swofty::log("[era] addon box press refused: %s", why.c_str());
        return nullptr;
    }
    uint8_t *tiles_node = at<uint8_t *>(v, VM_TILES_NODE), *selected_node = at<uint8_t *>(v, VM_SELECTED_NODE);
    ref<uint8_t *>(v, VM_TILES_NODE) = set.list.node;
    ref<uint8_t *>(v, VM_SELECTED_NODE) = set.selected.node;
    ref<uint8_t *>(v, VM_TILE_BEGIN) = begin + built.stock * TILE_ENTRY;
    uint32_t select = logic::SELECT_PRESS;
    const void *fake[4] = {&select, nullptr, nullptr, nullptr};
    char *r = press_orig(vm, fake);
    ref<uint8_t *>(v, VM_TILES_NODE) = tiles_node;
    ref<uint8_t *>(v, VM_SELECTED_NODE) = selected_node;
    ref<uint8_t *>(v, VM_TILE_BEGIN) = begin;
    return r;
}

void select_entry(void *container, const char *entrant_list)
{
    {
        std::lock_guard<std::mutex> g(held_lock);
        put_back();
    }
    void *mode = container ? at<void *>(container, CONTAINER_MODE) : nullptr;
    bool ours = entrant_list && std::find(built.lists.begin(), built.lists.end(), entrant_list) != built.lists.end();
    if (mode && ours)
        narrow(mode, entrant_list);
    else {
        restore();
        clear_fuel();
    }
    if (mode && !ours && time_trial(mode))
        keep_time_trial_lists(mode);
    last_press = entrant_list ? entrant_list : "";
    if (ours && !mode && logged_narrow++ < LOG_FIRST)
        swofty::log("[era] %s pressed but the container has no mode; %s is not narrowed", entrant_list, logic::HOST);
    last_pick = ours ? entrant_list : "";
    swofty_core::apply::moments::box_picked(entrant_list, mode);
    if (auto *modes = setup_owner(mode))
        ref<void *>(modes, SELECTED_LIST) = nullptr;
    std::string selected = ours ? pressed_list(entrant_list) : std::string(entrant_list ? entrant_list : "");
    select_orig(container, entrant_list ? selected.c_str() : entrant_list);
}

uintptr_t need(const char *id, std::string &missing)
{
    uintptr_t a = swofty::api->resolve(id);
    if (!a)
        missing += std::string(missing.empty() ? "" : ", ") + id;
    return a;
}

bool bind_all(std::string &why)
{
    std::string missing;
    b.site = need("site_FeTile_EraTilesBuilt", missing);
    b.press = need("FeTile_Press_EraTile", missing);
    b.select = need("Game_F1WorldActivityContainer_SelectEntrantList", missing);
    b.add = need("FeTile_Add_EraTile", missing);
    b.available = need("FeTile_Is_EraScenarioAvailable_Thunk", missing);
    b.list_child = need("Vm_GetOrCreate_ListChild", missing);
    b.node_ref = need("Vm_Set_NodeRef", missing);
    b.set_bool = need("Vm_Set_Bool", missing);
    b.clear = need("FeTile_Clear_EraTileSet_Thunk", missing);
    b.registry = need("g_Vm_Registry", missing);
    if (missing.empty())
        return true;
    why = "needs " + missing + ", which this build does not resolve";
    return false;
}

std::string caller(const void *from)
{
    std::string dll, id = settings_addon(from, dll);
    return id.empty() ? (dll.empty() ? std::string("unknown module") : dll) : id;
}

std::string text(const char *s)
{
    return s ? s : "";
}

// SwoftyMod's split_season_picker, read once settings.json has it: off turns the era picker off before any
// box registers. Without the option (an older settings.json) the picker stays split.
bool split_off()
{
    static std::atomic<int> split{-1};
    if (split < 0) {
        SwoftySettingValue v{};
        if (settings_get(SPLIT_OPTION, SWOFTY_SETTING_TOGGLE, &v))
            split = v.toggle ? 1 : 0;
    }
    if (split != 0 || off)
        return off;
    std::lock_guard<std::mutex> g(lock);
    if (!off) {
        off_why = "SwoftyMod's Split the season picker is off, so the picker keeps the game's single row";
        off = true;
        swofty::log("[era] addon row off: %s", off_why.c_str());
    }
    return true;
}

int api_available()
{
    split_off();
    return !off.load();
}

int api_add_box(const SwoftyEraBox *desc)
{
    std::string who = caller(__builtin_return_address(0));
    if (!desc || desc->size < BOX_TIME_TRIAL) {
        swofty::log("[era] box from %s refused: no descriptor, or its size %u is below %zu", who.c_str(),
                    desc ? desc->size : 0, BOX_TIME_TRIAL);
        return SWOFTY_ASSET_INVALID;
    }
    split_off();
    logic::Box box{text(desc->entrant_list), text(desc->title), text(desc->image), desc->order, who, 0, {}};
    if (desc->size >= BOX_TIME_TRIAL + sizeof(desc->time_trial_entrant_list))
        box.time_trial = text(desc->time_trial_entrant_list);
    if (desc->size >= BOX_ONLY + sizeof(desc->only))
        box.only = desc->only;
    std::lock_guard<std::mutex> g(lock);
    std::string why = logic::check(box, boxes);
    if (!why.empty()) {
        swofty::log("[era] box %s from %s refused: %s", box.entrant_list.c_str(), who.c_str(), why.c_str());
        return SWOFTY_ASSET_INVALID;
    }
    box.seq = boxes.size();
    boxes.push_back(box);
    std::string state = off ? "off: " + off_why : live ? "live from the next build of the picker" : "queued for the next mixin commit";
    std::string tt = box.time_trial.empty() ? std::string() : ", Time Trial list " + box.time_trial;
    if (box.only == logic::ONLY_GRAND_PRIX)
        tt += ", Grand Prix only";
    else if (box.only == logic::ONLY_TIME_TRIAL)
        tt += ", Time Trial only";
    swofty::log("[era] box %s from %s registered (title %s, image %s, order %d%s): %s", box.entrant_list.c_str(),
                who.c_str(), box.title.c_str(), logic::image_of(box).c_str(), box.order, tt.c_str(), state.c_str());
    return off ? SWOFTY_ASSET_UNAVAILABLE : SWOFTY_ASSET_OK;
}

int api_count()
{
    std::lock_guard<std::mutex> g(lock);
    return int(boxes.size());
}

const SwoftyEra table = {api_available, api_add_box, api_count};

std::vector<uint16_t> ids_of(uintptr_t begin, uintptr_t end, size_t field)
{
    std::vector<uint16_t> out;
    int64_t n = logic::entrant_count(begin, end);
    for (int64_t i = 0; i < n; ++i) {
        uint16_t id = logic::NO_TEAM;
        if (mem::read(begin + size_t(i) * logic::ENTRANT_RECORD + field, id))
            out.push_back(id);
    }
    return out;
}

std::vector<uint16_t> teams_of(uintptr_t begin, uintptr_t end)
{
    return ids_of(begin, end, logic::ENTRANT_TEAM);
}

logic::IdAliases id_aliases()
{
    static logic::IdAliases aliases;
    static bool read;
    if (read)
        return aliases;
    read = true;
    std::string text = runtime::read_file(runtime::swofty_root() + L"id_aliases.json", size_t(4) << 20);
    if (text.empty())
        return aliases;
    if (!logic::parse_aliases(text, aliases)) {
        swofty::log("[era] swofty\\id_aliases.json is unusable; saves made under earlier row ids are not mapped");
        return aliases;
    }
    swofty::log("[era] id aliases: %zu team id(s), %zu vehicle id(s), %zu driver id(s)", aliases.teams.size(),
                aliases.vehicles.size(), aliases.drivers.size());
    return aliases;
}

uint32_t remap_entrants(uintptr_t begin, uintptr_t end, const logic::IdAliases &aliases)
{
    uint32_t changed = 0;
    int64_t n = logic::entrant_count(begin, end);
    for (int64_t i = 0; i < n; ++i) {
        uintptr_t at = begin + size_t(i) * logic::ENTRANT_RECORD;
        const std::pair<size_t, const std::map<uint16_t, uint16_t> *> fields[] = {
            {logic::ENTRANT_TEAM, &aliases.teams},
            {logic::ENTRANT_VEHICLE_ID, &aliases.vehicles},
            {logic::ENTRANT_DRIVER, &aliases.drivers}};
        for (auto &[off, map] : fields) {
            uint16_t id = 0;
            if (!mem::read(at + off, id))
                continue;
            uint16_t now = logic::alias_of(*map, id);
            if (now == id)
                continue;
            ref<uint16_t>(reinterpret_cast<void *>(at), off) = now;
            changed++;
        }
    }
    return changed;
}

constexpr const char *CAREER_ALIAS_FN = "Ent_Deserialise_EntrantVector";
constexpr const char *CAREER_ALIAS_MIXIN = "swofty_core.dll (career id aliases)";
constexpr size_t VECTOR_BEGIN = 0x8;
constexpr size_t VECTOR_END = 0x10;
int (*entrant_vector_orig)(void *, void *);
std::atomic<bool> career_alias_offered, career_alias_settled, career_alias_bound;

int entrant_vector_entry(void *vector, void *bag)
{
    static uint32_t logged;
    int result = entrant_vector_orig(vector, bag);
    const logic::IdAliases &aliases = id_aliases();
    if (result != 0 || !vector || !aliases.any())
        return result;
    uintptr_t at = reinterpret_cast<uintptr_t>(vector), begin = 0, end = 0;
    if (!mem::read(at + VECTOR_BEGIN, begin) || !mem::read(at + VECTOR_END, end) || logic::entrant_count(begin, end) <= 0)
        return result;
    if (!logic::saved_predates(aliases, teams_of(begin, end), ids_of(begin, end, logic::ENTRANT_VEHICLE_ID),
                               ids_of(begin, end, logic::ENTRANT_DRIVER)))
        return result;
    uint32_t n = remap_entrants(begin, end, aliases);
    if (n && logged++ < LOG_FIRST)
        swofty::log("[era] career load: mapped %u saved entrant id(s) to the ids the rows have now", n);
    return result;
}

[[noreturn]] void stop_session_load(const std::string &why)
{
    swofty::log("[era] %s", why.c_str());
    swofty::log("[crash] the core closes F1 25 at StateLoadSession (exit code %u): %s", logic::CONTINUE_STOP_EXIT,
                why.c_str());
    TerminateProcess(GetCurrentProcess(), logic::CONTINUE_STOP_EXIT);
    ExitProcess(logic::CONTINUE_STOP_EXIT);
}

std::string mode_label(uintptr_t ctx)
{
    uintptr_t holder = 0, name = 0;
    std::string label;
    if (mem::read(ctx + MODE_SETUP, holder) && holder && mem::read(holder + MODE_NAME, name) && mem::c_string(name, label, 64))
        return label;
    return {};
}

void leave_front_end(uintptr_t ctx)
{
    static uint32_t logged;
    std::string label = mode_label(ctx);
    if (label.empty())
        return;
    uintptr_t holder = 0, setup = 0, list = 0;
    std::string selected;
    if (mem::read(ctx + MODE_SETUP, holder) && holder && mem::read(holder + SETUP_MODES, setup) && setup &&
        mem::read(setup + SELECTED_LIST, list) && list)
        if (const char *n = name_of(reinterpret_cast<void *>(list), LIST_NAME))
            selected = n;
    bool box = !selected.empty() && (box_list(selected) || is_ours(selected.c_str()));
    logic::FrontEndLeave leave = logic::leaving_front_end(label, box);
    if (leave.restore)
        restore();
    if (!box)
        clear_fuel();
    if (leave.clear_selected)
        ref<uintptr_t>(reinterpret_cast<void *>(setup), SELECTED_LIST) = 0;
    if (leave.forget_pick)
        last_pick.clear();
    if ((leave.restore || leave.clear_selected) && logged++ < LOG_FIRST)
        swofty::log("[era] %s session loads: %s's narrowed lists and DRS group are back%s", label.c_str(), logic::HOST,
                    leave.clear_selected ? ("; the box list " + selected + " no longer selected").c_str() : "");
}

void driver_career_load(uintptr_t ctx)
{
    static uint32_t logged;
    uintptr_t holder = 0, setup = 0, entrant_setup = 0, list = 0, gsm = 0, gsm_holder = 0, gsm_name = 0;
    if (!mem::read(ctx + MODE_SETUP, holder) || !holder || !mem::read(holder + SETUP_MODES, setup) || !setup ||
        !mem::read(setup + SELECTED_LIST, list) || list)
        return;
    std::string setup_mode;
    if (mem::read(setup + SETUP_GAME_SETUP_MODE, gsm) && gsm && mem::read(gsm + SETUP_MODE_NAME, gsm_holder) && gsm_holder &&
        mem::read(gsm_holder + 0x10, gsm_name))
        mem::c_string(gsm_name, setup_mode, 64);
    std::vector<void *> ptrs;
    std::vector<std::string> names;
    if (mem::read(setup + SETUP_ENTRANT_SETUP, entrant_setup) && entrant_setup)
        for (const auto &l : lists_of(reinterpret_cast<const uint8_t *>(entrant_setup), ptrs))
            names.push_back(l.name);
    auto career = swofty_core::apply::scheduler().context();
    logic::CareerListPick pick = logic::career_list(names, career.data_group, career.content, setup_mode);
    if (pick.index < 0)
        stop_session_load(logic::career_stop(pick));
    ref<void *>(reinterpret_cast<void *>(setup), SELECTED_LIST) = ptrs[size_t(pick.index)];
    if (logged++ < LOG_FIRST)
        swofty::log("[era] Driver Career load: %s", pick.line.c_str());
}

std::string grand_prix_load(uintptr_t ctx)
{
    static uint32_t logged;
    swofty_core::apply::moments::grand_prix_track_check();
    auto [mode, track] = swofty_core::apply::moments::session_mode_track();
    if (mode == "driver_career") {
        driver_career_load(ctx);
        return {};
    }
    if (mode != "grand_prix")
        return {};
    std::string track_why = logic::track_missing(swofty_core::apply::tracks::base(), track);
    uintptr_t holder = 0, setup = 0, entrant_setup = 0, list = 0, begin = 0, end = 0, gsm = 0, gsm_holder = 0, gsm_name = 0;
    if (!mem::read(ctx + MODE_SETUP, holder) || !holder || !mem::read(holder + SETUP_MODES, setup) || !setup ||
        !mem::read(setup + SELECTED_LIST, list)) {
        if (!track_why.empty())
            stop_session_load(logic::continue_stop(track_why, {}, false));
        return {};
    }
    logic::ContinuePick pick;
    std::string chosen;
    bool empty = !list && mem::read(setup + SETUP_ENTRANT_SETUP, entrant_setup) && entrant_setup;
    if (empty) {
        std::string setup_mode;
        if (mem::read(setup + SETUP_GAME_SETUP_MODE, gsm) && gsm && mem::read(gsm + SETUP_MODE_NAME, gsm_holder) &&
            gsm_holder && mem::read(gsm_holder + 0x10, gsm_name))
            mem::c_string(gsm_name, setup_mode, 64);
        std::vector<uint16_t> saved;
        if (mem::read(setup + logic::MGR_SESSION_BEGIN, begin) && mem::read(setup + logic::MGR_SESSION_END, end))
            saved = teams_of(begin, end);
        std::vector<void *> ptrs;
        auto cars = lists_of(reinterpret_cast<const uint8_t *>(entrant_setup), ptrs);
        std::vector<logic::ListTeams> lists;
        for (size_t i = 0; i < ptrs.size(); ++i) {
            logic::ListTeams l{cars[i].name, {}};
            uintptr_t at = reinterpret_cast<uintptr_t>(ptrs[i]), lb = 0, le = 0;
            if (at && mem::read(at + logic::LIST_ENTRANTS_BEGIN, lb) && mem::read(at + logic::LIST_ENTRANTS_END, le))
                l.teams = teams_of(lb, le);
            lists.push_back(std::move(l));
        }
        logic::AliasedPick aliased = logic::continue_list_aliased(lists, saved, last_press, setup_mode, id_aliases());
        pick = aliased.pick;
        if (aliased.remapped && track_why.empty()) {
            uint32_t n = remap_entrants(begin, end, id_aliases());
            swofty::log("[era] Grand Prix load: mapped %u saved entrant id(s) to the ids the rows have now", n);
        }
        if (pick.index >= 0 && track_why.empty()) {
            ref<void *>(reinterpret_cast<void *>(setup), SELECTED_LIST) = ptrs[size_t(pick.index)];
            chosen = cars[size_t(pick.index)].name;
            if (logged++ < LOG_FIRST)
                swofty::log("[era] Grand Prix load: %s", pick.line.c_str());
            std::string grid = continue_grid.raise(reinterpret_cast<uint8_t *>(entrant_setup), cars[size_t(pick.index)].cars);
            if (!grid.empty())
                swofty::log("[era] Grand Prix load: %s", grid.c_str());
        }
    }
    std::string why = logic::continue_stop(track_why, pick, empty);
    if (!why.empty()) {
        stop_session_load(why);
        return {};
    }
    if (chosen.empty() && list)
        if (const char *n = name_of(reinterpret_cast<void *>(list), LIST_NAME))
            chosen = n;
    return chosen;
}

}

extern "C" __declspec(dllexport) const SwoftyEra *SwoftyCoreEra(void)
{
    return &table;
}

extern "C" __declspec(dllexport) const uint32_t SwoftyCoreEraOnly = SWOFTY_API_ERA;

namespace runtime {

const SwoftyEra *era_table()
{
    return &table;
}

std::vector<era::Box> era_boxes()
{
    std::lock_guard<std::mutex> g(lock);
    return logic::ordered(boxes);
}

void era_press(const char *entrant_list)
{
    void *mode = nullptr, *entity = nullptr;
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_Manager") : 0, mgr = 0, ctx = 0;
    if (slot && mem::read(slot, mgr) && mgr)
        mem::read(mgr + offsetof(swofty::game::GameManager, mode_context), ctx);
    entity = reinterpret_cast<void *>(ctx);
    void *host = time_trial(entity) ? entity : nullptr;
    // The picker's own list of boxes only exists once the picker has been opened, so the name is taken as given: the narrowing finds
    // the list by name in the mode's setups and does nothing when it is not there.
    bool ours = entrant_list && *entrant_list;
    {
        std::lock_guard<std::mutex> g(held_lock);
        put_back();
    }
    if (host && ours)
        ::narrow(host, entrant_list);
    last_press = entrant_list ? entrant_list : "";
    last_pick = ours ? last_press : "";
    swofty::log("[era] box %s counts as pressed (a replay names it): %s", last_press.empty() ? "none" : last_press.c_str(),
                !ours ? "no box named" : host ? "the Time Trial mode is narrowed to it" : "the current mode is not Time Trial; nothing narrowed");
}

namespace {

// What a session load saw of a Time Trial: the list the session loads once the 24-car guard has chosen.
struct LoadSeen {
    bool time_trial = false;
    uintptr_t ctx = 0;
    std::string setup_mode, list, virtual_label, pressed;
};

std::mutex unlisted_lock;
std::string unlisted_list;

void set_unlisted(const std::string &list)
{
    std::lock_guard<std::mutex> g(unlisted_lock);
    unlisted_list = list;
}

// Makes the box of the loaded list the active content when it is not, in a Time Trial and in a Grand Prix
// (follow_grand_prix: a Continue presses no box): tt_loaded_logic.hpp. Runs without held_lock,
// like a press, before the session load's own appliers read the content.
void follow_grand_prix(const LoadSeen &seen)
{
    namespace tl = runtime::tt_loaded;
    if (seen.list.empty())
        return;
    std::vector<tl::BoxLists> lists;
    {
        std::lock_guard<std::mutex> g(lock);
        for (const auto &box : boxes)
            lists.push_back({box.entrant_list, box.time_trial});
    }
    tl::Loaded loaded = tl::loaded_box(lists, seen.list, {}, logic::HOST, VIRTUAL_HOST, seen.virtual_label);
    auto &scheduler = swofty_core::apply::scheduler();
    std::string package = loaded.kind == tl::Kind::box ? scheduler.find_package(loaded.box) : std::string();
    tl::Follow f = tl::follow_grand_prix(loaded, package, scheduler.active());
    if (f.action != tl::Action::follow)
        return;
    swofty::log("%s", f.line.c_str());
    swofty_core::apply::moments::box_picked(f.box.c_str(), reinterpret_cast<const void *>(seen.ctx));
}

void follow_loaded_list(const LoadSeen &seen)
{
    namespace tl = runtime::tt_loaded;
    if (!seen.time_trial) {
        set_unlisted({});
        follow_grand_prix(seen);
        return;
    }
    std::vector<tl::BoxLists> lists;
    {
        std::lock_guard<std::mutex> g(lock);
        for (const auto &box : boxes)
            lists.push_back({box.entrant_list, box.time_trial});
    }
    tl::Loaded loaded = tl::loaded_box(lists, seen.list, seen.setup_mode, logic::HOST, VIRTUAL_HOST, seen.virtual_label);
    auto &scheduler = swofty_core::apply::scheduler();
    std::string package = loaded.kind == tl::Kind::box || loaded.kind == tl::Kind::stock ? scheduler.find_package(loaded.box)
                                                                                         : std::string();
    tl::Follow f = tl::follow(loaded, package, scheduler.active(), seen.pressed);
    set_unlisted(f.action == tl::Action::no_board ? f.box : std::string());
    if (f.action == tl::Action::keep)
        return;
    swofty::log("%s", f.line.c_str());
    if (f.action == tl::Action::follow)
        swofty_core::apply::moments::box_picked(f.box.c_str(), reinterpret_cast<const void *>(seen.ctx));
}

void session_load_held(LoadSeen &seen)
{
    static uint32_t logged, swapped;
    static std::string last_seen;
    put_back();
    uintptr_t slot = swofty::api ? swofty::api->resolve("g_Game_Manager") : 0, mgr = 0, ctx = 0, setup = 0;
    if (!slot || !mem::read(slot, mgr) || !mgr || !mem::read(mgr + offsetof(swofty::game::GameManager, mode_context), ctx) ||
        !ctx)
        return;
    if (!time_trial(reinterpret_cast<void *>(ctx))) {
        leave_front_end(ctx);
        seen.ctx = ctx;
        seen.list = grand_prix_load(ctx);
        seen.virtual_label = filled.label;
        return;
    }
    seen.time_trial = true;
    seen.ctx = ctx;
    seen.pressed = last_press;
    seen.virtual_label = filled.label;
    uintptr_t holder = 0, gsm = 0, gsm_holder = 0, gsm_name = 0, entrant_setup = 0, list = 0;
    if (!mem::read(ctx + MODE_SETUP, holder) || !holder || !mem::read(holder + SETUP_MODES, setup) || !setup)
        return;
    std::string setup_mode = "?";
    if (mem::read(setup + SETUP_GAME_SETUP_MODE, gsm) && gsm && mem::read(gsm + SETUP_MODE_NAME, gsm_holder) && gsm_holder &&
        mem::read(gsm_holder + 0x10, gsm_name))
        mem::c_string(gsm_name, setup_mode, 64);
    if (!mem::read(setup + SETUP_ENTRANT_SETUP, entrant_setup) || !entrant_setup || !mem::read(setup + SELECTED_LIST, list))
        return;
    std::vector<void *> ptrs;
    auto lists = lists_of(reinterpret_cast<const uint8_t *>(entrant_setup), ptrs);
    logic::ListCars selected = list ? list_cars(reinterpret_cast<void *>(list)) : logic::ListCars{};
    logic::ListCars first = lists.empty() ? logic::ListCars{} : lists[0];
    std::string line = setup_mode + "/" + selected.name + "/" + std::to_string(selected.cars);
    if (line != last_seen && logged++ < LOG_FIRST * 2)
        swofty::log("[era] Time Trial session loads list %s (%lld cars) of setup mode %s (box pressed: %s)",
                    selected.name.empty() ? "?" : selected.name.c_str(), (long long)selected.cars, setup_mode.c_str(),
                    last_press.empty() ? "none" : last_press.c_str());
    last_seen = line;
    seen.setup_mode = setup_mode;
    seen.list = selected.name;
    if (!logic::oversized(selected.cars) && !logic::oversized(first.cars)) {
        time_trial_budget(reinterpret_cast<void *>(ctx), setup);
        return;
    }
    logic::ListCars bad = logic::oversized(selected.cars) ? selected : first;
    std::vector<std::string> prefer{last_press};
    {
        std::lock_guard<std::mutex> b(lock);
        for (const auto &box : boxes)
            if (box.entrant_list == last_press)
                prefer.push_back(logic::list_for(box, true));
    }
    int64_t i = logic::time_trial_loaded(selected, first, lists, prefer);
    auto *esm = reinterpret_cast<uint8_t *>(entrant_setup);
    held = {};
    held.esm = esm;
    if (i >= 0) {
        void *fit = ptrs[size_t(i)];
        ref<void *>(reinterpret_cast<void *>(setup), SELECTED_LIST) = fit;
        seen.list = lists[size_t(i)].name;
        if (logic::oversized(first.cars) && i > 0)
            logic::swap_into_first(held, esm, size_t(i));
    }
    int32_t limit = 0;
    if (mem::read(entrant_setup + ENTRANT_SETUP_MAX_ENTRANTS, limit) && logic::time_trial_limit(limit) != limit) {
        held.limit_was = limit;
        ref<int32_t>(esm, ENTRANT_SETUP_MAX_ENTRANTS) = logic::time_trial_limit(limit);
    }
    holding = held.first || held.limit_was;
    time_trial_budget(reinterpret_cast<void *>(ctx), setup);
    if (swapped++ < LOG_FIRST * 2)
        swofty::log("[era] %s", logic::time_trial_swap(bad.name, bad.cars, setup_mode, i >= 0 ? lists[size_t(i)].name : "",
                                                       held.limit_was)
                                    .c_str());
}

}

void era_session_load()
{
    LoadSeen seen;
    {
        std::lock_guard<std::mutex> g(held_lock);
        session_load_held(seen);
    }
    follow_loaded_list(seen);
}

bool era_session_unlisted(std::string &list)
{
    std::lock_guard<std::mutex> g(unlisted_lock);
    list = unlisted_list;
    return !list.empty();
}

constexpr const char *AVAILABILITY_MIXIN = "swofty_core.dll (setup mode guard)";
constexpr const char *AVAILABILITY_FN = "Sess_Apply_ContentAvailability";
using ContentAvailability_t = void(void *self, void *availability);
ContentAvailability_t *availability_orig;
std::atomic<bool> availability_bound, logged_dropped;

void drop_setup_pointers()
{
    logic::SetupPointers p;
    {
        std::lock_guard<std::mutex> g(held_lock);
        p.held = holding;
        holding = false;
        held = {};
        p.narrowed = narrowed_count;
        narrowed_count = 0;
    }
    p.drs = drs.forget();
    warmers.forget();
    tyre_change.forget();
    time_limits.forget();
    weekends.forget();
    points.forget();
    last_drs_line.clear();
    last_warmers_line.clear();
    last_points_line.clear();
    last_tyre_change_line.clear();
    last_weekend_line.clear();
    runtime::box_options_forget();
    last_box_options_line.clear();
    last_grid_line.clear();
    std::string line = logic::availability_drop_line(p);
    if (!line.empty() && !logged_dropped.exchange(true))
        swofty::log("[era] %s", line.c_str());
}

void availability_entry(void *self, void *availability)
{
    drop_setup_pointers();
    availability_orig(self, availability);
}

// An era tile press (0x1424b3640 -> 0x1424b2c90) collects the game mode's setup modes into a pointer vector
// with Sess_Collect_GameModeSetupModes, backed by a 256-byte arena on the caller's stack (0x1424b31af). The
// vector grows 1, 2, 3, 4, 6, 9, 13, so a game mode with a 10th matching setup mode asks the arena for 104 more
// bytes, gets null and the game memcpys to address 0. With the stack arena swapped for the game's default
// allocator the list grows on the heap; the vector keeps its allocator and the caller frees through it (vt+0x38).
constexpr const char *SETUP_LIST_MIXIN = "swofty_core.dll (era press setup mode list)";
constexpr const char *SETUP_LIST_FN = "Sess_Collect_GameModeSetupModes";
using CollectSetupModes_t = void *(void *out, void *allocator);
CollectSetupModes_t *setup_list_orig;
std::atomic<bool> setup_list_bound;
std::atomic<uint32_t> setup_list_swaps;

bool on_this_stack(const void *p)
{
    auto tib = reinterpret_cast<NT_TIB *>(NtCurrentTeb());
    auto a = reinterpret_cast<uintptr_t>(p);
    return a >= reinterpret_cast<uintptr_t>(tib->StackLimit) && a < reinterpret_cast<uintptr_t>(tib->StackBase);
}

void *setup_list_entry(void *out, void *allocator)
{
    uintptr_t slot = swofty::api->resolve("g_Ne_DefaultAllocator"), heap = 0;
    if (allocator && on_this_stack(allocator) && slot && mem::read(slot, heap) && heap) {
        if (setup_list_swaps++ < LOG_FIRST)
            swofty::log("[era] era press: the game mode's setup mode list grows on the heap, not in the 256-byte "
                        "stack arena that holds 9");
        return setup_list_orig(out, reinterpret_cast<void *>(heap));
    }
    return setup_list_orig(out, allocator);
}

std::vector<CoreMixins> tt_guard_mixins()
{
    std::vector<CoreMixins> out;
    if (vehicles_offered || !code_ready())
        return out;
    vehicles_offered = true;
    if (!swofty::api->resolve(VEHICLE_FN)) {
        swofty::log("[era] session vehicle guard off: needs %s, which this build does not resolve", VEHICLE_FN);
    } else {
        vehicles_bound = true;
        out.push_back(
            {VEHICLE_MIXIN, {{VEHICLE_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&vehicles_entry, (void **)&vehicles_orig}}});
    }
    if (!swofty::api->resolve(AVAILABILITY_FN)) {
        swofty::log("[era] setup mode guard off: needs %s, which this build does not resolve", AVAILABILITY_FN);
    } else {
        availability_bound = true;
        out.push_back({AVAILABILITY_MIXIN,
                       {{AVAILABILITY_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&availability_entry, (void **)&availability_orig}}});
    }
    if (!swofty::api->resolve(SETUP_LIST_FN) || !swofty::api->resolve("g_Ne_DefaultAllocator")) {
        swofty::log("[era] era press setup mode list off: needs %s and g_Ne_DefaultAllocator, which this build does "
                    "not resolve",
                    SETUP_LIST_FN);
    } else {
        setup_list_bound = true;
        out.push_back({SETUP_LIST_MIXIN,
                       {{SETUP_LIST_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&setup_list_entry, (void **)&setup_list_orig}}});
    }
    if (!swofty::api->resolve(PARSE_LIST_FN) || !swofty::api->resolve("g_Ne_DefaultAllocator")) {
        swofty::log("[era] virtual entrant lists off: needs %s and g_Ne_DefaultAllocator, which this build does not "
                    "resolve",
                    PARSE_LIST_FN);
    } else {
        parse_list_bound = true;
        out.push_back({PARSE_LIST_MIXIN,
                       {{PARSE_LIST_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&parse_list_entry, (void **)&parse_list_orig}}});
    }
    return out;
}

std::vector<CoreMixins> career_alias_mixins()
{
    std::vector<CoreMixins> out;
    if (career_alias_offered || !code_ready())
        return out;
    career_alias_offered = true;
    if (!swofty::api->resolve(CAREER_ALIAS_FN)) {
        swofty::log("[era] career id aliases off: needs %s, which this build does not resolve (a verify agree row on its "
                    "saved record layout is missing)",
                    CAREER_ALIAS_FN);
        return out;
    }
    career_alias_bound = true;
    out.push_back({CAREER_ALIAS_MIXIN,
                   {{CAREER_ALIAS_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&entrant_vector_entry,
                     (void **)&entrant_vector_orig}}});
    return out;
}

void career_alias_commit()
{
    if (!career_alias_offered || career_alias_settled)
        return;
    career_alias_settled = true;
    if (career_alias_bound)
        swofty::log("[era] career id aliases %s",
                    core_mixins_active(CAREER_ALIAS_MIXIN)
                        ? "armed: a career saved with earlier row ids has its entrants mapped to the current ids as it loads"
                        : "did not install (see the [core] lines)");
}

void tt_guard_commit()
{
    if (!vehicles_offered || vehicles_settled)
        return;
    vehicles_settled = true;
    if (vehicles_bound)
        swofty::log("[era] session vehicle guard %s",
                    core_mixins_active(VEHICLE_MIXIN)
                        ? "armed: a session with more vehicles than its table of 26 sets up the first 26 instead of writing past it"
                        : "did not install (see the [core] lines)");
    if (availability_bound)
        swofty::log("[era] setup mode guard %s",
                    core_mixins_active(AVAILABILITY_MIXIN)
                        ? "armed: when the game rebuilds its setup modes the core drops the pointers it keeps into them"
                        : "did not install (see the [core] lines)");
    if (setup_list_bound)
        swofty::log("[era] era press setup mode list %s",
                    core_mixins_active(SETUP_LIST_MIXIN)
                        ? "armed: a game mode with more than 9 setup modes no longer crashes an era tile press"
                        : "did not install (see the [core] lines)");
    if (parse_list_bound)
        swofty::log("[era] virtual entrant lists %s", core_mixins_active(PARSE_LIST_MIXIN)
                                                          ? "armed: a box whose list lies above the table's count "
                                                            "is read into swofty_virtual_list when it is pressed"
                                                          : "did not install (see the [core] lines)");
}

void era_session_trace(const char *state, int to)
{
    static constexpr const char *STEP[4] = {"None", "Entering", "Updating", "Exiting"};
    if (!state || !logic::trace_flow(state, to))
        return;
    session_trace(std::string(state) + " " + STEP[to & 3], -1);
}

void era_session_unloaded(const char *state, int to)
{
    static uint32_t logged;
    if (!state || !logic::time_trial_cut_off(state, to))
        return;
    tt_budget = -1;
    std::lock_guard<std::mutex> g(held_lock);
    logic::CutsPutBack r = put_back(true);
    if ((r.put || r.failed) && logged++ < LOG_FIRST)
        swofty::log("[era] StateUnloadSession Exiting: the lists the Time Trial guards left out are back in %zu entrant "
                    "setup mode(s)%s",
                    r.put, r.failed ? ("; " + std::to_string(r.failed) + " could not be put back").c_str() : "");
}

std::vector<CoreMixins> era_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || split_off() || off || !code_ready())
        return out;
    {
        std::lock_guard<std::mutex> g(lock);
        if (boxes.empty())
            return out;
    }
    offered = true;
    std::string why;
    if (!bind_all(why)) {
        off_why = why;
        off = true;
        swofty::log("[era] addon row off: %s", why.c_str());
        return out;
    }
    out.push_back({MIXIN,
                   {{"site_FeTile_EraTilesBuilt", SWOFTY_MIXIN_SITE, 100, (void *)&site_entry, nullptr},
                    {"FeTile_Press_EraTile", SWOFTY_MIXIN_WRAP, 100, (void *)&press_entry, (void **)&press_orig},
                    {"Game_F1WorldActivityContainer_SelectEntrantList", SWOFTY_MIXIN_WRAP, 100, (void *)&select_entry,
                     (void **)&select_orig}}});
    swofty::log("[era] %d box(es) registered; offering the era picker hooks", api_count());
    return out;
}

void era_commit()
{
    if (!offered || settled || off)
        return;
    settled = true;
    if (core_mixins_active(MIXIN)) {
        live = true;
        swofty::log("[era] era picker hooks installed; addon boxes show from the next build of the picker");
        return;
    }
    off_why = "the era picker hooks did not install (see the [core] lines for " + std::string(MIXIN) + ")";
    off = true;
    swofty::log("[era] addon row off: %s", off_why.c_str());
}

}
