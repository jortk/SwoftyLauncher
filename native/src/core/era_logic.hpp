#pragma once
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "apply/facts.hpp"
#include "apply/tracks/base.hpp"
#include "apply/tracks/seed.hpp"
#include "json.hpp"

// Pure logic of the era picker addon row (era_picker.cpp): the box check, the box order, the image
// default, the Time Trial list and its car limit, the press guard, the links of the row's two node handles
// and the DRS group swap of the host setup mode. No game calls, so the unit test runs on the host.
//
// Time Trial: a session holds the cars of the entrant list it was picked with plus three cars of its own,
// and the session system that walks them (0x142f45960, called from Game_GameModule_OnSessionEnter) keeps
// them in a table of 26 at +0xb0 with its count right behind it at +0x180. A 22-car list gives 25; a
// 24-car list gives 27 and overwrites the count. A list object (Sess_Load_AvailableEntrantLists, copied
// by 0x1424df240) holds its entrants as 0x88-byte records from +0x10 to +0x18.
//
// The guard holds for any list, whichever box or addon made the setup select it. On a Time Trial picker,
// a press on a stock box leaves swofty_f1 offering only the lists of at most 23 cars, and when a Time Trial
// session loads (StateLoadSession) the core reads the setup's resolved list (setup +0xe18, the setup being
// *(*(mode context+0x88)+0x260), its entrant setup mode +0xe10, its game setup mode +0x2b0) and, when that
// list or the entrant setup mode's first list holds more than 23 cars, selects a list of at most 23 cars
// from the same entrant setup mode instead (the box pressed, then its Time Trial list, then the first that
// fits) and holds the entrant setup mode's max_entrants (+0x268, the selector's limit, research grid24-cap
// §2 step 6) at 23 until the session ends or a box is pressed. As the last net, a wrap of 0x142f45960
// (Sess_Setup_VehicleComponents) that sees more vehicles than the table's 26 hands the game a vector of the
// first 26 for that call and logs the list, so the table is never written past its end.

namespace runtime::era {

constexpr const char *HOST = "swofty_f1";
constexpr const char *SELECTED_ADDON_TILE = "selected_addon_tile";
constexpr const char *ADDON_TILES_VISIBLE = "addon_tiles_visible";
constexpr const char *IMAGE_ARRAY = "#overrides.home_screen.tiles_solo_6x6";
// FeTile_Add_EraTile takes the tile index as a byte (the count of tiles added so far; the tiles themselves go into
// a growing vector and VM list), so the addon row holds up to 255 boxes, index 0..254.
constexpr size_t MAX_BOXES = 255;
constexpr uint32_t ONLY_ANY = 0, ONLY_GRAND_PRIX = 1, ONLY_TIME_TRIAL = 2;
constexpr size_t LIST_NAME_MAX = 63;
constexpr size_t TEXT_MAX = 255;
constexpr size_t NODE_REFS = 0x24;
constexpr size_t NODE_HOLDERS = 0x28;
constexpr size_t HOLDER_PREV = 0x10;
constexpr size_t TT_TABLE = 26;
// A raised grid stops at the stock 22 cars: with the safety car, 25 vehicles overran the replay recorder
// (Ne_Replay_WriteFrames) and 27 overrun the 26-slot physics body registry (Car_Chassis_Build).
constexpr size_t GRID_RAISE_MAX = 22;
constexpr size_t TT_EXTRA_CARS = 3;
constexpr size_t TT_MAX_CARS = TT_TABLE - TT_EXTRA_CARS;
constexpr size_t LIST_ENTRANTS_BEGIN = 0x10;
constexpr size_t LIST_ENTRANTS_END = 0x18;
constexpr size_t ENTRANT_RECORD = 0x88;
constexpr size_t MAX_LIST_ENTRANTS = 64;

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    while (*s)
        h = h * 33 + uint8_t(*s++);
    return h;
}

constexpr uint32_t ADDON_TILES = djb2("addon_tiles");
constexpr uint32_t ADDON_PRESS = djb2("addon_select_pressed");
constexpr uint32_t SELECT_PRESS = djb2("select_pressed");
static_assert(ADDON_TILES == 0xf2614e2b && ADDON_PRESS == 0x8fb68dff && SELECT_PRESS == 0x2b0357a);

struct Box {
    std::string entrant_list, title, image;
    int32_t order = 0;
    std::string owner;
    size_t seq = 0;
    std::string time_trial;
    uint32_t only = ONLY_ANY;
};

inline bool shown(const Box &box, bool time_trial)
{
    return box.only == ONLY_ANY || box.only == (time_trial ? ONLY_TIME_TRIAL : ONLY_GRAND_PRIX);
}

inline bool plain_name(const std::string &s)
{
    if (s.empty() || s.size() > LIST_NAME_MAX)
        return false;
    return std::all_of(s.begin(), s.end(), [](char c) {
        return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
    });
}

inline std::string check(Box &box, const std::vector<Box> &have)
{
    if (!plain_name(box.entrant_list))
        return "entrant_list must be 1 to 63 letters, digits or underscores";
    if (box.only > ONLY_TIME_TRIAL)
        return "only must be 0 (both pickers), 1 (Grand Prix) or 2 (Time Trial)";
    if (box.time_trial == box.entrant_list)
        box.time_trial.clear();
    if (!box.time_trial.empty() && !plain_name(box.time_trial))
        return "time_trial_entrant_list must be empty or 1 to 63 letters, digits or underscores";
    if (box.title.empty())
        box.title = "lng_" + box.entrant_list;
    if (box.title.size() > TEXT_MAX || box.image.size() > TEXT_MAX)
        return "title and image must be at most 255 characters";
    if (box.title.find('\0') != std::string::npos || box.image.find('\0') != std::string::npos)
        return "title and image must not hold NUL bytes";
    for (const auto &h : have)
        if (h.entrant_list == box.entrant_list)
            return "a box for this entrant list is already registered by " + h.owner;
    if (have.size() >= MAX_BOXES)
        return "the core holds at most 255 boxes";
    return {};
}

inline std::string image_of(const Box &box)
{
    return box.image.empty() ? std::string(IMAGE_ARRAY) + "[" + box.entrant_list + "]" : box.image;
}

inline bool time_trial_mode(std::string_view label)
{
    return swofty_core::apply::facts::mode_of(label) == "time_trial";
}

struct FrontEndLeave {
    bool restore = false, clear_selected = false, forget_pick = false;
};

inline FrontEndLeave leaving_front_end(std::string_view mode_label, bool selected_is_box)
{
    std::string mode = swofty_core::apply::facts::mode_of(mode_label);
    if (mode != "driver_career" && mode != "my_team" && mode != "story" && mode != "f1_world")
        return {};
    return {true, selected_is_box, true};
}

inline const std::string &list_for(const Box &box, bool time_trial)
{
    return time_trial && !box.time_trial.empty() ? box.time_trial : box.entrant_list;
}

inline int64_t entrant_count(uintptr_t begin, uintptr_t end)
{
    if (!begin || end < begin || (end - begin) % ENTRANT_RECORD || (end - begin) / ENTRANT_RECORD > MAX_LIST_ENTRANTS)
        return -1;
    return int64_t((end - begin) / ENTRANT_RECORD);
}

inline std::string time_trial_refusal(const Box &box, int64_t cars)
{
    if (cars < 0 || size_t(cars) <= TT_MAX_CARS)
        return {};
    std::string list = list_for(box, true);
    std::string why = "its Time Trial list " + list + " holds " + std::to_string(cars) + " cars; Time Trial holds at most " +
                      std::to_string(TT_MAX_CARS) + " (the game adds " + std::to_string(TT_EXTRA_CARS) +
                      " cars of its own to a table of " + std::to_string(TT_TABLE) + ")";
    if (box.time_trial.empty())
        why += ". Give the box a Time Trial entrant list of at most " + std::to_string(TT_MAX_CARS) + " cars";
    return why;
}

// The Time Trial guard. A list whose car count was read and is past TT_MAX_CARS is oversized; a count that
// could not be read (-1) is kept, so the guard never drops what it cannot see.
struct ListCars {
    std::string name;
    int64_t cars = -1;
};

inline bool oversized(int64_t cars)
{
    return cars >= 0 && size_t(cars) > TT_MAX_CARS;
}

inline std::vector<size_t> time_trial_keep(const std::vector<ListCars> &lists)
{
    std::vector<size_t> keep;
    for (size_t i = 0; i < lists.size(); ++i)
        if (!oversized(lists[i].cars))
            keep.push_back(i);
    return keep;
}

// A Time Trial session's cars come from more than its own list: the Time Trial mode's descriptor fill
// (0x142936760, vtable 0x1457b13d0 slot 0x80) walks every game setup mode of the mode (EntrantsManager slot
// 0x1d0), each one's entrant setup modes (slot 0x1d8) and their selectable lists (+0x130, count +0x230), takes
// every list whose leaderboard (the row's leaderboard descriptor stat id, optional byte +0xe0, engaged +0xf0)
// matches the selected list's, and pushes one entrant per distinct vehicle ref (EntrantRecord +0x5c) of them,
// plus 4 entries of its own. A stock F1 2025 Time Trial draws 11 cars (f1_custom_team, leaderboard f1) and
// spawns 15 vehicles. A list an addon copies from f1_2025 keeps leaderboard f1, so F1 2012 Season's
// bdx_s5_2012 (12 vehicles) made it 27, past the 26-slot vehicle table. A package's list has a descriptor of its
// own since 1.7.20 (content.package leaderboard), so only the lists that share one count together. A stock press in Time Trial narrows swofty_f1 whenever one of
// its lists is oversized, to no list at all when none fits; at each Time Trial load the core also counts the distinct cars of every list of the
// selected list's leaderboard and drops whole lists, addon lists first, until they fit 22 (26 less the 4).
inline bool time_trial_narrows(const std::vector<ListCars> &lists, const std::vector<size_t> &keep)
{
    return !lists.empty() && keep.size() != lists.size();
}

constexpr size_t LIST_LEADERBOARD = 0xe0;
constexpr size_t LIST_LEADERBOARD_SET = 0xf0;
constexpr size_t ENTRANT_VEHICLE = 0x5c;
constexpr size_t TT_OWN_ENTRIES = 4;
constexpr size_t TT_ROOM = TT_TABLE - TT_OWN_ENTRIES;

inline bool vehicle_ref_set(uint32_t ref)
{
    return (ref >> 16) != 0x3fff;
}

struct TtList {
    std::string name;
    bool selected = false, addon = false;
    std::vector<uint32_t> refs;
    bool fixed = false;
};

// The fill dedupes vehicle refs by their byte +0 and u16 +2 (0x1424a1c20), so byte +1 is not part of the key.
inline uint32_t ref_key(uint32_t ref)
{
    return ref & 0xffff00ffu;
}

// Entrant setup mode overrides (the game mode's EntrantSetupModeOverride rows): the fill gets each game setup mode's
// entrant setup modes through EntrantsManager slot 0x1d8 (body 0x1485b5280), which copies gsm +0x10 (count +0x70)
// and swaps every mode whose stat id byte +0 has an entry in the flat map at manager +0xfd8: slots of 0x10 bytes
// from +0xfe0 to the metadata bytes at +0xfe8 (key byte +0, override at +8), mask +0xff8 (0 = empty map). A slot is
// in use when its metadata byte is not 0; keys are unique, so a scan finds what the game's probe finds.
constexpr size_t MGR_OVERRIDES = 0xfe0;
constexpr size_t MGR_OVERRIDE_META = 0xfe8;
constexpr size_t MGR_OVERRIDE_MASK = 0xff8;
constexpr size_t OVERRIDE_SLOT = 0x10;
constexpr size_t MAX_OVERRIDE_SLOTS = 1024;

struct Override {
    bool found = false;
    const void *to = nullptr;
};

// The game installs a row's value even when it is null (0x1485b5280 writes it unchecked), so a found row with a
// null value leaves that entrant setup mode slot empty.
inline Override override_for(uint8_t id, const uint8_t *slots, const uint8_t *meta, size_t count)
{
    if (!slots || !meta || count > MAX_OVERRIDE_SLOTS)
        return {};
    for (size_t i = 0; i < count; ++i)
        if (meta[i] && slots[i * OVERRIDE_SLOT] == id)
            return {true, *reinterpret_cast<void *const *>(slots + i * OVERRIDE_SLOT + 8)};
    return {};
}

// 0x1424a1c20 takes a list's own records when the mode's flag word +0x258 + idx * 4 has bit 0 (idx 0 or 1 for the
// fill's human record), every ref of the entrants system pool +0x338 with bit 1 and of +0x348 with bit 2.
constexpr size_t ESM_VEHICLE_FLAGS = 0x258;
constexpr uint32_t TAKES_LIST = 1, TAKES_POOL_A = 2, TAKES_POOL_B = 4;
constexpr size_t ES_POOL_A = 0x338;
constexpr size_t ES_POOL_B = 0x348;
constexpr size_t POOL_SLOT = 0x10;
constexpr size_t MAX_POOL = 8192;

// An entrants system pool is indexed by vehicle stat id, so its length is the highest id + 1: 0x10-byte slots from
// begin to end, a pointer to the vehicle ref at +8 or 0 when the id is free. read(address, value) is a guarded read.
struct PoolRefs {
    std::vector<uint32_t> refs;
    bool capped = false;
};

template <class Read> PoolRefs pool_refs_from(uintptr_t begin, uintptr_t end, Read read)
{
    PoolRefs out;
    if (!begin || end < begin || (end - begin) % POOL_SLOT)
        return out;
    size_t slots = (end - begin) / POOL_SLOT;
    if (slots > MAX_POOL) {
        out.capped = true;
        slots = MAX_POOL;
    }
    for (size_t i = 0; i < slots; ++i) {
        uintptr_t at_ref = 0;
        uint32_t ref = 0;
        if (read(begin + i * POOL_SLOT + 8, at_ref) && at_ref && read(at_ref, ref))
            out.refs.push_back(ref);
    }
    return out;
}

inline uint32_t vehicle_flags(uint32_t player, uint32_t second_player)
{
    return player | second_player;
}

// A cut is made only where the restore stack can record it, or where an entry for that mode already does.
inline bool cut_recordable(size_t used, size_t capacity, bool known)
{
    return known || used < capacity;
}

struct TtBudget {
    std::vector<size_t> dropped;
    size_t cars = 0;
};

// The selected list always stays; the other lists of its leaderboard join in the order stock lists, then addon
// lists, each whole while the distinct vehicle refs of everything kept stay within room.
inline TtBudget time_trial_budget(const std::vector<TtList> &lists, size_t room = TT_ROOM)
{
    std::vector<size_t> order(lists.size());
    for (size_t i = 0; i < order.size(); ++i)
        order[i] = i;
    auto rank = [&](size_t i) { return lists[i].selected || lists[i].fixed ? 0 : lists[i].addon ? 2 : 1; };
    std::stable_sort(order.begin(), order.end(), [&](size_t a, size_t b) { return rank(a) < rank(b); });
    std::vector<uint32_t> have;
    TtBudget out;
    for (size_t i : order) {
        std::vector<uint32_t> next = have;
        for (uint32_t r : lists[i].refs)
            if (vehicle_ref_set(r) && std::find(next.begin(), next.end(), ref_key(r)) == next.end())
                next.push_back(ref_key(r));
        if (lists[i].selected || lists[i].fixed || next.size() <= room)
            have = std::move(next);
        else
            out.dropped.push_back(i);
    }
    std::sort(out.dropped.begin(), out.dropped.end());
    out.cars = have.size();
    return out;
}

inline size_t distinct_cars(const std::vector<uint32_t> &refs)
{
    std::vector<uint32_t> d;
    for (uint32_t r : refs)
        if (vehicle_ref_set(r) && std::find(d.begin(), d.end(), ref_key(r)) == d.end())
            d.push_back(ref_key(r));
    return d.size();
}

// Leaderboard descriptor stat ids of the stock leaderboards.leaderboard_descriptor rows, for the log only; an id past
// the stock rows is a descriptor an addon's package or database operations added.
inline std::string leaderboard_text(unsigned id)
{
    static constexpr const char *STOCK[] = {"f1", "classic", "f1_26", "f2", nullptr, "f2_26"};
    std::string out = std::to_string(id);
    if (id < std::size(STOCK) && STOCK[id])
        out += std::string(", ") + STOCK[id];
    else if (id >= std::size(STOCK))
        out += ", added";
    return out;
}

inline std::string time_trial_budget_line(const std::vector<TtList> &lists, const TtBudget &b, unsigned leaderboard)
{
    std::string selected = "?";
    size_t kept = 0, pool = 0;
    for (size_t i = 0; i < lists.size(); ++i) {
        if (lists[i].selected)
            selected = lists[i].name;
        if (lists[i].fixed)
            pool += distinct_cars(lists[i].refs);
        else if (std::find(b.dropped.begin(), b.dropped.end(), i) == b.dropped.end())
            kept++;
    }
    std::string line = "the Time Trial session's list " + selected + " (leaderboard " + leaderboard_text(leaderboard) +
                       ") draws " + std::to_string(b.cars) + " cars from " + std::to_string(kept) +
                       " list(s) of its leaderboard" +
                       (pool ? " and the entrants system pools (" + std::to_string(pool) + ")" : std::string()) +
                       "; with the game's " + std::to_string(TT_OWN_ENTRIES) + " entries " +
                       std::to_string(b.cars + TT_OWN_ENTRIES) + " of the " + std::to_string(TT_TABLE) + "-slot vehicle table";
    if (!b.dropped.empty()) {
        line += "; left out until the session unloads:";
        for (size_t k = 0; k < b.dropped.size(); ++k) {
            const auto &l = lists[b.dropped[k]];
            line += std::string(k ? "," : "") + " " + l.name + " (" + std::to_string(distinct_cars(l.refs)) + " cars" +
                    (l.addon ? "" : ", stock") + ")";
        }
    }
    return line;
}

inline int64_t time_trial_fallback(const std::vector<ListCars> &lists, const std::vector<std::string> &prefer)
{
    auto fits = [&](size_t i) { return lists[i].cars >= 0 && !oversized(lists[i].cars); };
    for (const auto &want : prefer)
        for (size_t i = 0; i < lists.size(); ++i)
            if (!want.empty() && lists[i].name == want && fits(i))
                return int64_t(i);
    for (size_t i = 0; i < lists.size(); ++i)
        if (fits(i))
            return int64_t(i);
    return -1;
}

// The index in lists of the list a Time Trial session loads after the 24-car guard, -1 while the session's own
// selected list stays. The leaderboard budget is computed for this list, not for the one selected before the swap.
inline int64_t time_trial_loaded(const ListCars &selected, const ListCars &first, const std::vector<ListCars> &lists,
                                 const std::vector<std::string> &prefer)
{
    if (!oversized(selected.cars) && !oversized(first.cars))
        return -1;
    return time_trial_fallback(lists, prefer);
}

constexpr size_t ENTRANT_SETUP_MAX_ENTRANTS = 0x268;

struct GridLimit {
    uint8_t *esm = nullptr;
    int32_t was = 0, set = 0;

    std::string raise(uint8_t *setup_mode, int64_t cars)
    {
        restore();
        if (!setup_mode || cars <= 0)
            return "";
        int32_t limit = *reinterpret_cast<int32_t *>(setup_mode + ENTRANT_SETUP_MAX_ENTRANTS);
        if (limit <= 0 || cars <= limit)
            return "";
        bool clamped = size_t(cars) > GRID_RAISE_MAX;
        int32_t target = clamped ? int32_t(GRID_RAISE_MAX) : int32_t(cars);
        if (target <= limit)
            return "";
        esm = setup_mode;
        was = limit;
        set = target;
        *reinterpret_cast<int32_t *>(esm + ENTRANT_SETUP_MAX_ENTRANTS) = set;
        std::string line = "max_entrants " + std::to_string(was) + " -> " + std::to_string(set) + " for a list of " +
                           std::to_string(cars) + " cars";
        if (clamped)
            line += "; clamped to " + std::to_string(GRID_RAISE_MAX) + " cars, the stock grid";
        return line;
    }

    void restore()
    {
        if (set && esm && *reinterpret_cast<int32_t *>(esm + ENTRANT_SETUP_MAX_ENTRANTS) == set)
            *reinterpret_cast<int32_t *>(esm + ENTRANT_SETUP_MAX_ENTRANTS) = was;
        esm = nullptr;
        was = set = 0;
    }
};

inline int32_t time_trial_limit(int32_t max_entrants)
{
    return max_entrants < 1 || size_t(max_entrants) > TT_MAX_CARS ? int32_t(TT_MAX_CARS) : max_entrants;
}

inline std::string time_trial_swap(const std::string &list, int64_t cars, const std::string &setup_mode,
                                   const std::string &instead, int32_t limit_was)
{
    std::string why = "the Time Trial session's list " + list + " (setup mode " + setup_mode + ") holds " +
                      std::to_string(cars) + " cars; Time Trial holds at most " + std::to_string(TT_MAX_CARS) +
                      " (the game adds " + std::to_string(TT_EXTRA_CARS) + " cars of its own to a table of " +
                      std::to_string(TT_TABLE) + ")";
    if (!instead.empty())
        why += "; loading " + instead + " instead";
    else
        why += "; its entrant setup mode offers no list of at most " + std::to_string(TT_MAX_CARS) + " cars";
    if (limit_was)
        why += "; the session draws at most " + std::to_string(TT_MAX_CARS) + " cars (max_entrants " +
               std::to_string(limit_was) + " -> " + std::to_string(TT_MAX_CARS) + " until the session ends)";
    return why;
}

// The core's restore stack of entrant setup mode list arrays (the lists at +0x130, their count at +0x230). An addon
// box press narrows swofty_f1's modes to one list, a stock box press on a Time Trial picker keeps the lists past 23 cars
// out, and a Time Trial load leaves whole lists of its leaderboard out (time_trial set on the last two). Each entry
// holds the array before (lists, count) and after (kept_lists, kept); it is put back only while the mode still holds
// what the core left, so a list array changed since is not overwritten.
constexpr size_t ESM_LISTS = 0x130;
constexpr size_t ESM_LIST_COUNT = 0x230;
constexpr size_t ESM_LIST_SLOTS = 32;

inline void **esm_lists(uint8_t *esm)
{
    return reinterpret_cast<void **>(esm + ESM_LISTS);
}

inline int64_t &esm_list_count(uint8_t *esm)
{
    return *reinterpret_cast<int64_t *>(esm + ESM_LIST_COUNT);
}

struct ListCut {
    uint8_t *setup = nullptr;
    int64_t count = 0, kept = 0;
    void *lists[ESM_LIST_SLOTS] = {};
    void *kept_lists[ESM_LIST_SLOTS] = {};
    bool time_trial = false;
    GridLimit grid;
};

inline bool put_back_cut(ListCut &n)
{
    n.grid.restore();
    if (!n.setup || n.kept < 0 || n.count < 0 || size_t(n.kept) > ESM_LIST_SLOTS || size_t(n.count) > ESM_LIST_SLOTS ||
        esm_list_count(n.setup) != n.kept || memcmp(esm_lists(n.setup), n.kept_lists, size_t(n.kept) * sizeof(void *)))
        return false;
    memcpy(esm_lists(n.setup), n.lists, size_t(n.count) * sizeof(void *));
    esm_list_count(n.setup) = n.count;
    return true;
}

// The lists an entry left out of its mode, in the mode's order before the cut.
inline std::vector<void *> left_out(const ListCut &n)
{
    std::vector<void *> out;
    int64_t kept = std::clamp<int64_t>(n.kept, 0, int64_t(ESM_LIST_SLOTS));
    for (int64_t i = 0; i < n.count && size_t(i) < ESM_LIST_SLOTS; ++i)
        if (std::find(n.kept_lists, n.kept_lists + kept, n.lists[i]) == n.kept_lists + kept)
            out.push_back(n.lists[i]);
    return out;
}

struct CutsPutBack {
    size_t put = 0, failed = 0;
};

// Puts entries back from the top of the stack (only the Time Trial ones when time_trial_only) and drops them. An entry
// whose mode no longer holds what it left is handed to failed and is not counted as put back.
template <class Failed> CutsPutBack put_back_cuts(ListCut *stack, size_t &used, bool time_trial_only, Failed failed)
{
    CutsPutBack r;
    for (size_t i = used; i-- > 0;) {
        if (time_trial_only && !stack[i].time_trial)
            continue;
        if (put_back_cut(stack[i])) {
            r.put++;
        } else {
            r.failed++;
            failed(stack[i]);
        }
    }
    size_t keep = 0;
    if (time_trial_only)
        for (size_t i = 0; i < used; ++i)
            if (!stack[i].time_trial)
                stack[keep++] = stack[i];
    used = keep;
    return r;
}

// A Time Trial load's leaderboard cut of `list` out of `esm`. The first cut of a mode pushes an entry when the stack has
// room; a later one extends the Time Trial entry that already holds the mode. False when the stack is full: the list stays.
inline bool cut_list(ListCut *stack, size_t &used, size_t capacity, uint8_t *esm, void *list)
{
    int64_t count = esm_list_count(esm);
    if (count < 0 || size_t(count) > ESM_LIST_SLOTS)
        return false;
    bool known = false;
    for (size_t i = 0; i < used; ++i)
        known = known || (stack[i].time_trial && stack[i].setup == esm && stack[i].kept == count);
    if (!cut_recordable(used, capacity, known))
        return false;
    void **arr = esm_lists(esm);
    if (!known) {
        ListCut &n = stack[used++];
        n = {};
        n.setup = esm;
        n.count = count;
        memcpy(n.lists, arr, size_t(count) * sizeof(void *));
        n.time_trial = true;
        n.kept = count;
        memcpy(n.kept_lists, n.lists, sizeof(n.lists));
    }
    int64_t w = 0;
    for (int64_t i = 0; i < count; ++i)
        if (arr[i] != list)
            arr[w++] = arr[i];
    esm_list_count(esm) = w;
    for (size_t i = used; i-- > 0;)
        if (stack[i].time_trial && stack[i].setup == esm) {
            stack[i].kept = w;
            memcpy(stack[i].kept_lists, arr, size_t(w) * sizeof(void *));
            break;
        }
    return true;
}

// The 24-car guard's hold on a Time Trial session's entrant setup mode: the list of at most 23 cars it moved into slot 0
// (fit, from `slot`, where the first list went) and the max_entrants it lowered (limit_was, 0 when untouched).
struct TtHold {
    uint8_t *esm = nullptr;
    int32_t limit_was = 0;
    size_t slot = 0;
    void *first = nullptr;
    void *fit = nullptr;
};

inline void swap_into_first(TtHold &h, uint8_t *esm, size_t slot)
{
    void **arr = esm_lists(esm);
    h.esm = esm;
    h.slot = slot;
    h.first = arr[0];
    h.fit = arr[slot];
    arr[slot] = h.first;
    arr[0] = h.fit;
}

inline void put_back_hold(const TtHold &h)
{
    if (!h.esm)
        return;
    auto &limit = *reinterpret_cast<int32_t *>(h.esm + ENTRANT_SETUP_MAX_ENTRANTS);
    if (h.limit_was && limit == time_trial_limit(h.limit_was))
        limit = h.limit_was;
    void **arr = esm_lists(h.esm);
    if (h.first && h.slot < ESM_LIST_SLOTS && arr[0] == h.fit && arr[h.slot] == h.first) {
        arr[h.slot] = arr[0];
        arr[0] = h.first;
    }
}

// Ends the hold. The leaderboard cut of the same load was made on the swapped array and is recorded against it, so the
// Time Trial entries are put back first: undoing the swap first changes the array, and the cut lists would stay out.
// With `cuts` (the session unload) they are put back whether or not a swap is held; otherwise only while one is.
template <class Failed> CutsPutBack end_hold(ListCut *stack, size_t &used, TtHold &held, bool &holding, bool cuts, Failed failed)
{
    CutsPutBack r;
    if (cuts || (holding && held.first))
        r = put_back_cuts(stack, used, true, failed);
    if (holding) {
        holding = false;
        put_back_hold(held);
    }
    return r;
}

inline std::string put_back_failed_line(const std::vector<std::string> &names, int64_t now, int64_t kept, bool time_trial)
{
    std::string list;
    for (const auto &name : names)
        list += (list.empty() ? "" : ", ") + name;
    return std::string("the lists the ") + (time_trial ? "Time Trial guard" : "box narrowing") +
           " left out of an entrant setup mode could not be put back: its lists changed since (" + std::to_string(now) +
           " now, " + std::to_string(kept) + " left), so " + (list.empty() ? "none" : list) + " stay out of it";
}

constexpr size_t VEHICLE_SLOT = 8;

inline size_t session_vehicles(uintptr_t begin, uintptr_t end)
{
    return end > begin ? size_t(((end - begin) / VEHICLE_SLOT) & 0xff) : 0;
}

inline uintptr_t vehicles_end_within_table(uintptr_t begin, uintptr_t end)
{
    return session_vehicles(begin, end) > TT_TABLE ? begin + TT_TABLE * VEHICLE_SLOT : end;
}

// Session trace (diagnostic, read only). A Time Trial after a Grand Prix on a 24-car box spawns 27 vehicles
// while the entrants manager's session entrants (EntrantsManager +0x2e0..+0x2e8, 0x88-byte records, team stat
// id u16 at +0x5a) hold at most 23 at StateLoadSession Entering. The Time Trial mode's descriptor fill
// (0x142936760, vtable 0x1457b13d0 slot 0x80) clears those entrants, pushes its own and builds one 0x6780-byte
// session participant per entrant with a vehicle into the descriptor (+0xad0 vector, begin +0xad8, end +0xae0).
// The trace logs, at each StateLoadSession step, at StateSession Entering and at the session vehicle setup,
// the manager's entrant count and team ids, its list and setup mode, the participant counts of the session
// descriptors the game manager (+0x1a8) and game module (game manager +0x140, its +0x1a8 and +0x198) hold, and
// the session module's vehicle count, so one run shows which of them holds the 27 and when it fills.
constexpr size_t MGR_SESSION_BEGIN = 0x2e0;
constexpr size_t MGR_SESSION_END = 0x2e8;
constexpr size_t ENTRANT_DRIVER = 0x52;
constexpr size_t ENTRANT_TEAM = 0x5a;
constexpr size_t ENTRANT_VEHICLE_ID = 0x5e;
constexpr size_t PARTICIPANT = 0x6780;
constexpr size_t DESC_PARTICIPANTS_BEGIN = 0xad8;
constexpr size_t DESC_PARTICIPANTS_END = 0xae0;
constexpr size_t GM_SESSION = 0x1a8;
constexpr size_t GM_MODULE = 0x140;
constexpr size_t MODULE_SESSION = 0x1a8;
constexpr size_t MODULE_SESSION_NEXT = 0x198;
constexpr size_t MAX_PARTICIPANTS = 64;
constexpr uint32_t TRACE_LINES = 120;

inline bool trace_flow(std::string_view state, int to)
{
    return ((state == "StateLoadSession" || state == "StateUnloadSession") && to >= 1 && to <= 3) ||
           (state == "StateSession" && (to == 1 || to == 3));
}

// The Time Trial cut stays through StateSession onStateExit and StateUnloadSession onStateEnter, whose loader keep
// checks (0x1436f8340, 0x143709970 -> 0x142d48040) fill a temporary descriptor and compare it with the live one; it
// comes off at StateUnloadSession Exiting.
inline bool time_trial_cut_off(std::string_view state, int to)
{
    return state == "StateUnloadSession" && to == 3;
}

inline int64_t participant_count(uintptr_t begin, uintptr_t end)
{
    if (!begin || end < begin || (end - begin) % PARTICIPANT || (end - begin) / PARTICIPANT > MAX_PARTICIPANTS)
        return -1;
    return int64_t((end - begin) / PARTICIPANT);
}

struct SessionTrace {
    std::string where, mode, list, setup_mode;
    int64_t entrants = -1, list_cars = -1;
    std::vector<uint16_t> teams;
    int64_t gm_session = -1, module_session = -1, module_next = -1;
    int64_t vehicles = -1;
    int64_t budget = -1;
    int64_t previous = -1;
};

// The Time Trial budget's expected vehicles (its cars plus the game's 4 entries) against what the fill made: the
// participants in the game manager's descriptor, or the vehicles at the session's vehicle setup.
// The fill runs between StateLoadSession Updating and Exiting, so at Entering and Updating the descriptor may still
// hold the previous session's participants: there a count is compared only when it changed since the previous
// traced step (previous), which a fill before Entering would show.
inline std::string budget_mismatch(const SessionTrace &t)
{
    int64_t seen = t.vehicles >= 0 ? t.vehicles : t.gm_session;
    if (t.budget < 0 || seen <= 0 || seen == t.budget)
        return {};
    bool before_fill = t.where == "StateLoadSession Entering" || t.where == "StateLoadSession Updating";
    if (before_fill && t.vehicles < 0 && t.gm_session == t.previous)
        return {};
    return "Time Trial fill at " + t.where + " made " + std::to_string(seen) + " vehicles where the budget expected " +
           std::to_string(t.budget);
}

inline std::string count_text(int64_t n)
{
    return n < 0 ? std::string("-") : std::to_string(n);
}

inline std::string session_trace_line(const SessionTrace &t)
{
    std::string teams;
    for (size_t i = 0; i < t.teams.size(); ++i)
        teams += (i ? "," : "") + std::to_string(t.teams[i]);
    return "session trace " + t.where + " (" + (t.mode.empty() ? "?" : t.mode) + "): entrants " + count_text(t.entrants) +
           (teams.empty() ? "" : " (teams " + teams + ")") + ", list " + (t.list.empty() ? "?" : t.list) + " (" +
           count_text(t.list_cars) + " cars) of setup mode " + (t.setup_mode.empty() ? "?" : t.setup_mode) +
           "; participants: game manager " + count_text(t.gm_session) + ", game module " + count_text(t.module_session) +
           " (next " + count_text(t.module_next) + "); session vehicles " + count_text(t.vehicles) +
           (t.budget >= 0 ? "; Time Trial budget " + std::to_string(t.budget) : std::string());
}

// Grand Prix load guard. Sess_Setup_ChampionshipEntrants (0x140ba3ad0) sets the entrants manager's list (+0xe18)
// only when the weekend's entrant setup mode offers exactly one list; swofty_f1 hosts every season box, so a Grand
// Prix > Continue lands at StateLoadSession with +0xe18 null and Sess_Assign_GarageSlotsByClass (0x1424a8cf0) reads
// [0+0x48]. At StateLoadSession Entering the core picks the setup mode's list that holds every team of the saved
// entrants (manager +0x2e0..+0x2e8, team stat id u16 at +0x5a of each 0x88-byte record, the same records a list
// holds from +0x10 to +0x18), else the list of the last box pressed. A saved weekend whose track is no track of the
// base (a dormant row whose addon is off: no track archive, no facts entry) would crash in
// Sess_Loader_SetupTrackAssets. With no list or no track the load is stopped: the core logs why and closes F1 25
// with CONTINUE_STOP_EXIT before the game touches either, and the launcher names the reason.
constexpr uint16_t NO_TEAM = 0x3fff;
constexpr unsigned CONTINUE_STOP_EXIT = 92;

struct ListTeams {
    std::string name;
    std::vector<uint16_t> teams;
};

struct ContinuePick {
    int64_t index = -1;
    std::string line;
    bool holds = false;
};

struct IdAliases {
    std::map<uint16_t, uint16_t> teams, vehicles, drivers;

    bool any() const
    {
        return !teams.empty() || !vehicles.empty() || !drivers.empty();
    }
};

inline bool parse_aliases(const std::string &text, IdAliases &out)
{
    json::Value v;
    if (!json::parse(text, v) || v.type != json::Value::Obj || v["version"].num() != 1)
        return false;
    IdAliases a;
    auto table = [&](const char *name, std::map<uint16_t, uint16_t> &into) {
        for (auto &[k, to] : v["tables"][name].o) {
            long long from = strtoll(k.c_str(), nullptr, 10), now = to.num(-1);
            if (from > 0 && from < NO_TEAM && now > 0 && now < NO_TEAM && from != now)
                into[uint16_t(from)] = uint16_t(now);
        }
    };
    table("teams", a.teams);
    table("vehicles", a.vehicles);
    table("drivers", a.drivers);
    out = std::move(a);
    return true;
}

inline uint16_t alias_of(const std::map<uint16_t, uint16_t> &map, uint16_t id)
{
    auto it = map.find(id);
    return it == map.end() ? id : it->second;
}

inline std::vector<uint16_t> remap_teams(const IdAliases &aliases, std::vector<uint16_t> teams)
{
    for (uint16_t &t : teams)
        t = alias_of(aliases.teams, t);
    return teams;
}

inline bool saved_predates(const IdAliases &aliases, const std::vector<uint16_t> &teams)
{
    for (uint16_t t : teams)
        if (t && t != NO_TEAM && aliases.teams.count(t))
            return true;
    return false;
}

inline bool saved_predates(const IdAliases &aliases, const std::vector<uint16_t> &teams,
                           const std::vector<uint16_t> &vehicles, const std::vector<uint16_t> &drivers)
{
    if (saved_predates(aliases, teams))
        return true;
    for (uint16_t v : vehicles)
        if (v && v != NO_TEAM && aliases.vehicles.count(v))
            return true;
    for (uint16_t d : drivers)
        if (d && d != NO_TEAM && aliases.drivers.count(d))
            return true;
    return false;
}

inline std::vector<uint16_t> distinct_teams(const std::vector<uint16_t> &teams)
{
    std::vector<uint16_t> out;
    for (uint16_t t : teams)
        if (t && t != NO_TEAM && std::find(out.begin(), out.end(), t) == out.end())
            out.push_back(t);
    std::sort(out.begin(), out.end());
    return out;
}

inline std::string team_text(const std::vector<uint16_t> &teams)
{
    std::string s;
    for (size_t i = 0; i < teams.size(); ++i)
        s += (i ? "," : "") + std::to_string(teams[i]);
    return s.empty() ? std::string("none") : s;
}

inline ContinuePick continue_list(const std::vector<ListTeams> &lists, const std::vector<uint16_t> &entrants,
                                  const std::string &pressed, const std::string &setup_mode)
{
    ContinuePick p;
    std::vector<uint16_t> want = distinct_teams(entrants);
    std::string head = "the game left the entrants manager's list empty (setup mode " +
                       (setup_mode.empty() ? std::string("?") : setup_mode) + " offers " + std::to_string(lists.size()) +
                       " lists); ";
    auto holds_all = [&](const ListTeams &l) {
        if (want.empty())
            return false;
        std::vector<uint16_t> have = distinct_teams(l.teams);
        return std::includes(have.begin(), have.end(), want.begin(), want.end());
    };
    int64_t first = -1, press = -1;
    for (size_t i = 0; i < lists.size(); ++i) {
        if (first < 0 && holds_all(lists[i]))
            first = int64_t(i);
        if (press < 0 && !pressed.empty() && lists[i].name == pressed)
            press = int64_t(i);
    }
    if (press >= 0 && holds_all(lists[size_t(press)]))
        first = press;
    if (first >= 0) {
        p.index = first;
        p.holds = true;
        p.line = head + "loading " + lists[size_t(first)].name + ", which holds all " + std::to_string(want.size()) +
                 " teams of the saved entrants";
        return p;
    }
    if (press >= 0) {
        p.index = press;
        p.line = head + "no list holds the saved entrants' teams (" + team_text(want) + "); loading " + pressed +
                 ", the box pressed last";
        return p;
    }
    p.line = head + "no list holds the saved entrants' teams (" + team_text(want) + ") and no box was pressed";
    return p;
}

struct AliasedPick {
    ContinuePick pick;
    std::vector<uint16_t> teams;
    bool remapped = false;
};

inline AliasedPick continue_list_aliased(const std::vector<ListTeams> &lists, const std::vector<uint16_t> &entrants,
                                         const std::string &pressed, const std::string &setup_mode,
                                         const IdAliases &aliases)
{
    AliasedPick out;
    out.pick = continue_list(lists, entrants, pressed, setup_mode);
    out.teams = entrants;
    if (out.pick.holds || !aliases.any())
        return out;
    std::vector<uint16_t> mapped = remap_teams(aliases, entrants);
    if (mapped == entrants)
        return out;
    ContinuePick again = continue_list(lists, mapped, pressed, setup_mode);
    if (!again.holds)
        return out;
    out.pick = again;
    out.pick.line += " after mapping the saved team ids " + team_text(distinct_teams(entrants)) + " to " +
                     team_text(distinct_teams(mapped)) + " (the ids the rows had before this build)";
    out.teams = mapped;
    out.remapped = true;
    return out;
}

inline std::string track_code_of(const std::string &track)
{
    size_t slash = track.rfind('/');
    return slash == std::string::npos ? track : track.substr(slash + 1);
}

inline std::string track_missing(const swofty_core::apply::tracks::Base &base, const std::string &track)
{
    if (!base.ready || track.empty() || base.by_id(track))
        return {};
    std::string code = track_code_of(track);
    if (base.by_code(code))
        return {};
    // A stock track is in every build, also when the base holds no tracks because no track addon is on.
    if (track.starts_with("game/") && swofty_core::apply::tracks::stock_venue(code))
        return {};
    std::string why = "the saved weekend's track " + track + " is not a track of this build";
    if (swofty_core::apply::tracks::is_dormant(code))
        why += " (" + code + " is a dormant row with no track archive; the addon that fills it is off)";
    return why;
}

struct CareerListPick {
    int64_t index = -1;
    std::string line;
};

inline CareerListPick career_list(const std::vector<std::string> &lists, const std::string &data_group,
                                  const std::vector<std::string> &content, const std::string &setup_mode)
{
    CareerListPick p;
    std::string head = "the game left the entrants manager's list empty in a Driver Career (setup mode " +
                       (setup_mode.empty() ? std::string("?") : setup_mode) + " offers " + std::to_string(lists.size()) +
                       " lists); ";
    auto find = [&](const std::string &name) -> int64_t {
        if (name.empty())
            return -1;
        for (size_t i = 0; i < lists.size(); ++i)
            if (lists[i] == name)
                return int64_t(i);
        return -1;
    };
    p.index = find(data_group);
    if (p.index >= 0) {
        p.line = head + "loading " + lists[size_t(p.index)] + ", the list named like the career's data group";
        return p;
    }
    for (const std::string &c : content) {
        p.index = find(c);
        if (p.index >= 0) {
            p.line = head + "loading " + lists[size_t(p.index)] + ", the list of the content the career runs";
            return p;
        }
    }
    p.line = head + "no list is named like the career's data group (" + (data_group.empty() ? std::string("none") : data_group) +
             ") or its content";
    return p;
}

inline std::string career_stop(const CareerListPick &pick)
{
    return "Driver Career load stopped: " + pick.line +
           ". F1 25 closes now instead of crashing on it; start a new Driver Career, or turn the addons back on that the "
           "career was made with";
}

inline std::string continue_stop(const std::string &track_why, const ContinuePick &pick, bool list_missing)
{
    std::vector<std::string> why;
    if (!track_why.empty())
        why.push_back(track_why);
    if (list_missing && pick.index < 0)
        why.push_back(pick.line);
    if (why.empty())
        return {};
    std::string s = "Grand Prix load stopped: ";
    for (size_t i = 0; i < why.size(); ++i)
        s += (i ? "; " : "") + why[i];
    return s + ". F1 25 closes now instead of crashing on it; turn the addon that made the save back on, or start a "
               "new Grand Prix in place of Continue";
}

inline std::vector<Box> ordered(std::vector<Box> boxes)
{
    std::stable_sort(boxes.begin(), boxes.end(), [](const Box &a, const Box &b) {
        return a.order != b.order ? a.order < b.order : a.seq < b.seq;
    });
    return boxes;
}

struct Handle {
    uint8_t *node;
    void *next, *prev;
};
static_assert(offsetof(Handle, prev) == HOLDER_PREV);

inline void link(Handle &h, uint8_t *node)
{
    h = {node, *reinterpret_cast<void **>(node + NODE_HOLDERS), nullptr};
    if (h.next)
        *reinterpret_cast<void **>(static_cast<uint8_t *>(h.next) + HOLDER_PREV) = &h;
    *reinterpret_cast<void **>(node + NODE_HOLDERS) = &h;
    __atomic_fetch_add(reinterpret_cast<int16_t *>(node + NODE_REFS), int16_t(1), __ATOMIC_SEQ_CST);
}

// clear (FeTile_Clear_EraTileSet_Thunk in game) unlinks and releases both handles from the nodes they still
// hold, whichever view model those belong to, before the list handle is linked to this build's node.
template <class Set, class Clear> void relink(Set &set, uint8_t *list, Clear clear)
{
    if (set.list.node || set.selected.node)
        clear(set);
    set = {};
    link(set.list, list);
}

inline std::string press_refusal(bool same_view_model, bool nodes, size_t vector_tiles, size_t stock, size_t boxes)
{
    if (!same_view_model)
        return "the picker that raised it is not the one the addon row was built for";
    if (!nodes || !boxes)
        return "the addon row has no boxes";
    if (vector_tiles < stock + boxes)
        return "the tile vector holds " + std::to_string(vector_tiles) + " tiles, fewer than the " +
               std::to_string(stock) + " stock and " + std::to_string(boxes) + " addon tiles of the last build";
    return {};
}

// A box's DRS rule (content.package rules.drs) applies through the host setup mode's DRS group. A game setup mode
// object keeps each string field as a holder whose +0x10 is the char * label (re/db structs/GameSetupMode.h); its
// +0x1e0 holder names the drs.drs_allowed_lap group the DRS system looks up at session load, at a full safety car
// end and at a red flag restart. The swap points that field at a holder of the rule's group label and remembers the
// game's holder; put_back restores it wherever the field still holds the swap's holder.
constexpr size_t MODE_NAME = 0xd8;
constexpr size_t MODE_DRS_GROUP = 0x1e0;
constexpr size_t HOLDER_TEXT = 0x10;
constexpr size_t DRS_LABEL_MAX = 63;

struct DrsHolder {
    void *unused[2] = {};
    const char *text = nullptr;
    uint8_t pad[0x40] = {};
};
static_assert(offsetof(DrsHolder, text) == HOLDER_TEXT);

class DrsSwaps {
public:
    // A holder per label, at a stable address for the life of the process.
    DrsHolder *holder(const std::string &label)
    {
        if (label.empty() || label.size() > DRS_LABEL_MAX || label.find('\0') != std::string::npos)
            return nullptr;
        auto &h = holders_[label];
        if (!h) {
            h = std::make_unique<Entry>();
            h->label = label;
            h->holder.text = h->label.c_str();
        }
        return &h->holder;
    }

    void swap(uint8_t *mode, DrsHolder *holder)
    {
        if (!mode || !holder)
            return;
        void *&field = *reinterpret_cast<void **>(mode + MODE_DRS_GROUP);
        if (field == holder)
            return;
        bool known = false;
        for (auto &s : swaps_)
            known = known || s.mode == mode;
        if (!known)
            swaps_.push_back({mode, field});
        field = holder;
    }

    size_t put_back()
    {
        size_t n = 0;
        for (auto &s : swaps_) {
            void *&field = *reinterpret_cast<void **>(s.mode + MODE_DRS_GROUP);
            if (!owns(field))
                continue;
            field = s.original;
            n++;
        }
        swaps_.clear();
        return n;
    }

    // Drops the swaps without reading or writing the modes, which the game may have freed.
    size_t forget()
    {
        size_t n = swaps_.size();
        swaps_.clear();
        return n;
    }

    size_t active() const { return swaps_.size(); }

    bool owns(const void *holder) const
    {
        for (auto &[label, e] : holders_)
            if (&e->holder == holder)
                return true;
        return false;
    }

private:
    struct Entry {
        std::string label;
        DrsHolder holder;
    };
    struct Swap {
        uint8_t *mode;
        void *original;
    };
    std::map<std::string, std::unique_ptr<Entry>> holders_;
    std::vector<Swap> swaps_;
};

// A box's tyre warmer rule (content.package rules.tyre_warmers) applies through the host setup mode's
// has_tyre_warmers byte (+0x1c8, re/db structs/GameSetupMode.h, written by the mode loader 0x142a14f20). The swap
// writes the box's value and remembers the game's; put_back restores it wherever the byte still holds the swap's value.
constexpr size_t MODE_TYRE_WARMERS = 0x1c8;

class WarmerSwaps {
public:
    void swap(uint8_t *mode, bool warmers)
    {
        if (!mode)
            return;
        uint8_t &field = mode[MODE_TYRE_WARMERS];
        uint8_t want = warmers ? 1 : 0;
        Swap *s = find(mode);
        if (!s) {
            swaps_.push_back({mode, field, want});
            s = &swaps_.back();
        }
        s->written = want;
        field = want;
    }

    size_t put_back()
    {
        size_t n = 0;
        for (auto &s : swaps_) {
            uint8_t &field = s.mode[MODE_TYRE_WARMERS];
            if (field != s.written)
                continue;
            field = s.original;
            n++;
        }
        swaps_.clear();
        return n;
    }

    // Drops the swaps without reading or writing the modes, which the game may have freed.
    size_t forget()
    {
        size_t n = swaps_.size();
        swaps_.clear();
        return n;
    }

    size_t active() const { return swaps_.size(); }

private:
    struct Swap {
        uint8_t *mode;
        uint8_t original, written;
    };
    Swap *find(const uint8_t *mode)
    {
        for (auto &s : swaps_)
            if (s.mode == mode)
                return &s;
        return nullptr;
    }
    std::vector<Swap> swaps_;
};

// A box's race time limit (content.package rules.race_time_limit_s) applies through the host setup mode's
// race_time_limit_in_seconds (+0xb4, three u32, re/db structs/GameSetupMode.h, copied by the mode loader 0x142a14f20
// from the comma list; classic 7200,7200,7200). The swap writes the box's seconds into all three and remembers the
// game's; put_back restores them wherever the three still hold the swap's value.
constexpr size_t MODE_RACE_TIME_LIMIT = 0xb4;
constexpr size_t RACE_TIME_LIMITS = 3;

class TimeLimitSwaps {
public:
    void swap(uint8_t *mode, uint32_t seconds)
    {
        if (!mode)
            return;
        uint32_t *field = limits(mode);
        Swap *s = find(mode);
        if (!s) {
            swaps_.push_back({mode, {field[0], field[1], field[2]}, seconds});
            s = &swaps_.back();
        }
        s->written = seconds;
        for (size_t i = 0; i < RACE_TIME_LIMITS; ++i)
            field[i] = seconds;
    }

    size_t put_back()
    {
        size_t n = 0;
        for (auto &s : swaps_) {
            uint32_t *field = limits(s.mode);
            bool ours = true;
            for (size_t i = 0; i < RACE_TIME_LIMITS; ++i)
                ours = ours && field[i] == s.written;
            if (!ours)
                continue;
            for (size_t i = 0; i < RACE_TIME_LIMITS; ++i)
                field[i] = s.original[i];
            n++;
        }
        swaps_.clear();
        return n;
    }

    // Drops the swaps without reading or writing the modes, which the game may have freed.
    size_t forget()
    {
        size_t n = swaps_.size();
        swaps_.clear();
        return n;
    }

    size_t active() const { return swaps_.size(); }

private:
    struct Swap {
        uint8_t *mode;
        uint32_t original[RACE_TIME_LIMITS];
        uint32_t written;
    };
    static uint32_t *limits(uint8_t *mode) { return reinterpret_cast<uint32_t *>(mode + MODE_RACE_TIME_LIMIT); }
    Swap *find(const uint8_t *mode)
    {
        for (auto &s : swaps_)
            if (s.mode == mode)
                return &s;
        return nullptr;
    }
    std::vector<Swap> swaps_;
};

// A box's weekend (content.package rules.weekend_mode) applies by exchanging the weekend options container of the host
// setup mode with the one of a helper setup mode (+0x190..+0x1a8: four qwords the mode loader 0x142a14f20 fills from
// weekend_options_0: the practice, qualifying and race groups and the length options they offer). It is an exchange,
// not a copy: each container stays owned by exactly one mode object, so a game rebuild that frees every mode while a
// swap is in place still frees each container once (forget). The swap is held for the whole weekend and undone by the
// next press of another box, a stock box or a picker build without an addon list. Only one host mode takes the
// helper's container: a second host mode keeps its own (one container cannot sit in two modes).
// The options tag (+0x00: the options_system game_setup_mode tag Sess_Parse_GameSetupMode resolves from the row's
// options_tag through 0x142a1a740 and stores at 0x142a187e4) decides which values the weekend menu offers
// (options_system.context tag restrictions); it is exchanged with the weekend options so the menu offers the helper's.
constexpr size_t MODE_OPTIONS_TAG = 0x00;
constexpr size_t MODE_WEEKEND_OPTIONS = 0x190;
constexpr size_t WEEKEND_OPTION_QWORDS = 5;
inline constexpr size_t WEEKEND_SWAP_OFFSETS[WEEKEND_OPTION_QWORDS] = {MODE_OPTIONS_TAG, MODE_WEEKEND_OPTIONS, MODE_WEEKEND_OPTIONS + 8,
                                                                       MODE_WEEKEND_OPTIONS + 16, MODE_WEEKEND_OPTIONS + 24};

class WeekendSwaps {
public:
    // Exchanges host's and helper's weekend options; a different held pair is put back first. False when host or
    // helper is missing or the two are the same object.
    bool swap(uint8_t *host, uint8_t *helper)
    {
        if (!host || !helper || host == helper)
            return false;
        if (held_ && held_->host == host && held_->helper == helper)
            return true;
        put_back();
        exchange(host, helper);
        Held h{host, helper, {}, {}};
        for (size_t i = 0; i < WEEKEND_OPTION_QWORDS; ++i) {
            h.host_now[i] = slots(host)[i];
            h.helper_now[i] = slots(helper)[i];
        }
        held_ = h;
        return true;
    }

    // Exchanges back when both modes still hold what the swap left in them; otherwise drops the pair without writing.
    size_t put_back()
    {
        if (!held_)
            return 0;
        Held h = *held_;
        held_.reset();
        for (size_t i = 0; i < WEEKEND_OPTION_QWORDS; ++i)
            if (slots(h.host)[i] != h.host_now[i] || slots(h.helper)[i] != h.helper_now[i])
                return 0;
        exchange(h.host, h.helper);
        return 1;
    }

    // Drops the pair without reading or writing the modes, which the game may have freed.
    size_t forget()
    {
        size_t n = held_ ? 1 : 0;
        held_.reset();
        return n;
    }

    size_t active() const { return held_ ? 1 : 0; }
    const uint8_t *helper() const { return held_ ? held_->helper : nullptr; }

private:
    struct Held {
        uint8_t *host, *helper;
        uint64_t host_now[WEEKEND_OPTION_QWORDS], helper_now[WEEKEND_OPTION_QWORDS];
    };
    struct Slots {
        uint8_t *mode;
        uint64_t &operator[](size_t i) const { return *reinterpret_cast<uint64_t *>(mode + WEEKEND_SWAP_OFFSETS[i]); }
    };
    static Slots slots(uint8_t *mode) { return {mode}; }
    static void exchange(uint8_t *a, uint8_t *b)
    {
        for (size_t i = 0; i < WEEKEND_OPTION_QWORDS; ++i)
            std::swap(slots(a)[i], slots(b)[i]);
    }
    std::optional<Held> held_;
};

// Points per box (content.package rules.points). A setup mode keeps its weekend structures in a vector at +0x190
// (begin +0x198, end +0x1a0, 0x88 each, the sprint flag at +0x80); a structure keeps its sessions at +0x00 (begin
// +0x08, end +0x10, 16 bytes each: +0x00 the parsed standings preset or 0, +0x08 the session id, 15-17 = race,
// race_2, race_3). The game resolves a session's standings label to a pointer into the game setup registry's preset
// vector (g_Game_Scenarios +0xe0: begin +0xe8, end +0xf0, 0x20 each) when it parses the mode, and the standings
// recompute reads the pointer live. A preset holds its points levels at +0x08 / +0x10 (0x18 each: +0x00 the
// fastest lap rule byte, +0x04 the bonus, +0x08 the points column, whose i32 values run from +0x08 to +0x10);
// level 0 is the full race, then the reduced_points_data rows. Presets carry no label, so the era picker finds the
// one the boot apply added for the box by its points (find_points_preset) and points the race sessions of every
// swofty_f1 mode's non-sprint structures at it (PointsSwaps).
constexpr size_t MODE_WEEKENDS_BEGIN = 0x198, MODE_WEEKENDS_END = 0x1a0, WEEKEND_SIZE = 0x88, WEEKEND_SPRINT = 0x80;
constexpr size_t WEEKEND_SESSIONS_BEGIN = 0x08, WEEKEND_SESSIONS_END = 0x10, SESSION_SIZE = 0x10, SESSION_ID = 0x08;
constexpr size_t SCENARIOS_PRESETS_BEGIN = 0xe8, SCENARIOS_PRESETS_END = 0xf0, PRESET_SIZE = 0x20;
constexpr size_t PRESET_LEVELS_BEGIN = 0x08, PRESET_LEVELS_END = 0x10, LEVEL_SIZE = 0x18;
constexpr size_t LEVEL_RULE = 0x00, LEVEL_BONUS = 0x04, LEVEL_POINTS = 0x08, COLUMN_BEGIN = 0x08, COLUMN_END = 0x10;
constexpr uint32_t SESSION_RACE = 15, SESSION_RACE_3 = 17;
constexpr size_t WEEKENDS_MAX = 16, SESSIONS_MAX = 64, PRESETS_MAX = 4096, LEVELS_MAX = 8, PLACES_MAX = 64;

using Readable = std::function<bool(const void *, size_t)>;

struct PointsWant {
    std::vector<int32_t> full;
    std::vector<std::vector<int32_t>> reduced;
    int32_t bonus = 0;
    uint8_t rule = 0;
};

template <class T> inline bool peek(const Readable &ok, const void *base, size_t off, T &out)
{
    if (!base)
        return false;
    auto *p = static_cast<const uint8_t *>(base) + off;
    if (!ok(p, sizeof(T)))
        return false;
    std::memcpy(&out, p, sizeof(T));
    return true;
}

inline bool level_is(const Readable &ok, const uint8_t *level, const std::vector<int32_t> &points, int32_t bonus,
                     uint8_t rule)
{
    uint8_t r = 0;
    int32_t b = 0;
    const uint8_t *column = nullptr, *begin = nullptr, *end = nullptr;
    if (!peek(ok, level, LEVEL_RULE, r) || !peek(ok, level, LEVEL_BONUS, b) || !peek(ok, level, LEVEL_POINTS, column) ||
        !peek(ok, column, COLUMN_BEGIN, begin) || !peek(ok, column, COLUMN_END, end))
        return false;
    if (r != rule || (rule && b != bonus) || end < begin || size_t(end - begin) != points.size() * 4 ||
        points.size() > PLACES_MAX || !ok(begin, size_t(end - begin)))
        return false;
    return !std::memcmp(begin, points.data(), points.size() * 4);
}

// The parsed preset whose full and reduced points, fastest lap rule and bonus are the box's, or nullptr.
inline const uint8_t *find_points_preset(const Readable &ok, const uint8_t *begin, const uint8_t *end,
                                         const PointsWant &want)
{
    if (!begin || end <= begin || size_t(end - begin) % PRESET_SIZE || size_t(end - begin) / PRESET_SIZE > PRESETS_MAX)
        return nullptr;
    for (const uint8_t *p = begin; p < end; p += PRESET_SIZE) {
        const uint8_t *lb = nullptr, *le = nullptr;
        if (!peek(ok, p, PRESET_LEVELS_BEGIN, lb) || !peek(ok, p, PRESET_LEVELS_END, le) || le < lb ||
            size_t(le - lb) % LEVEL_SIZE)
            continue;
        size_t levels = size_t(le - lb) / LEVEL_SIZE;
        if (!levels || levels > LEVELS_MAX || !level_is(ok, lb, want.full, want.bonus, want.rule))
            continue;
        bool same = true;
        for (size_t i = 1; i < levels && i <= want.reduced.size() && same; i++)
            same = level_is(ok, lb + i * LEVEL_SIZE, want.reduced[i - 1], want.bonus, want.rule);
        if (same)
            return p;
    }
    return nullptr;
}

// The race sessions (race, race_2, race_3) of a setup mode's non-sprint weekend structures that hold a preset.
inline std::vector<uint64_t *> race_sessions(const Readable &ok, uint8_t *mode)
{
    std::vector<uint64_t *> out;
    uint8_t *wb = nullptr, *we = nullptr;
    if (!peek(ok, mode, MODE_WEEKENDS_BEGIN, wb) || !peek(ok, mode, MODE_WEEKENDS_END, we) || !wb || we < wb ||
        size_t(we - wb) % WEEKEND_SIZE || size_t(we - wb) / WEEKEND_SIZE > WEEKENDS_MAX)
        return out;
    for (uint8_t *w = wb; w < we; w += WEEKEND_SIZE) {
        uint8_t sprint = 1;
        uint8_t *sb = nullptr, *se = nullptr;
        if (!peek(ok, w, WEEKEND_SPRINT, sprint) || sprint || !peek(ok, w, WEEKEND_SESSIONS_BEGIN, sb) ||
            !peek(ok, w, WEEKEND_SESSIONS_END, se) || !sb || se < sb || size_t(se - sb) % SESSION_SIZE ||
            size_t(se - sb) / SESSION_SIZE > SESSIONS_MAX)
            continue;
        for (uint8_t *s = sb; s < se; s += SESSION_SIZE) {
            uint32_t id = 0;
            uint64_t preset = 0;
            if (peek(ok, s, SESSION_ID, id) && id >= SESSION_RACE && id <= SESSION_RACE_3 && peek(ok, s, 0, preset) &&
                preset)
                out.push_back(reinterpret_cast<uint64_t *>(s));
        }
    }
    return out;
}

// The race sessions the era picker pointed at a box's preset, with the pointer each held before, so a box without a
// points rule, a stock box or a career load puts the game's presets back (only where the pointer is still the one
// written).
class PointsSwaps {
public:
    void swap(uint64_t *session, uint64_t preset)
    {
        if (!session)
            return;
        Swap *s = find(session);
        if (!s) {
            swaps_.push_back({session, *session, preset});
            s = &swaps_.back();
        }
        s->written = preset;
        *session = preset;
    }

    size_t put_back()
    {
        size_t n = 0;
        for (auto &s : swaps_) {
            if (*s.session != s.written)
                continue;
            *s.session = s.original;
            n++;
        }
        swaps_.clear();
        return n;
    }

    // Drops the swaps without reading or writing the sessions, which the game may have freed.
    size_t forget()
    {
        size_t n = swaps_.size();
        swaps_.clear();
        return n;
    }

    size_t active() const { return swaps_.size(); }

private:
    struct Swap {
        uint64_t *session;
        uint64_t original, written;
    };
    Swap *find(const uint64_t *session)
    {
        for (auto &s : swaps_)
            if (s.session == session)
                return &s;
        return nullptr;
    }
    std::vector<Swap> swaps_;
};

// The mandatory tyre change per box (content.package rules.mandatory_tyre_change): GameSetupMode +0x8b, +0x98 and
// +0xa5 hold the race, race_2 and race_3 requirement flags, 13 each, indexed by the race length; a race session's
// TyreRegulations rule is on when its length's flag is set (read at race session start). The era picker writes all
// 39 flags of every swofty_f1 mode and puts the game's back (only where the bytes are still the ones written).
constexpr size_t MODE_TYRE_REQUIREMENTS = 0x8b, TYRE_REQUIREMENT_BYTES = 39;

class TyreChangeSwaps {
public:
    void swap(uint8_t *mode, bool required)
    {
        if (!mode)
            return;
        uint8_t *field = mode + MODE_TYRE_REQUIREMENTS;
        Swap *s = find(mode);
        if (!s) {
            swaps_.push_back({mode, {}, {}});
            s = &swaps_.back();
            std::memcpy(s->original, field, TYRE_REQUIREMENT_BYTES);
        }
        std::memset(s->written, required ? 1 : 0, TYRE_REQUIREMENT_BYTES);
        std::memcpy(field, s->written, TYRE_REQUIREMENT_BYTES);
    }

    size_t put_back()
    {
        size_t n = 0;
        for (auto &s : swaps_) {
            uint8_t *field = s.mode + MODE_TYRE_REQUIREMENTS;
            if (std::memcmp(field, s.written, TYRE_REQUIREMENT_BYTES))
                continue;
            std::memcpy(field, s.original, TYRE_REQUIREMENT_BYTES);
            n++;
        }
        swaps_.clear();
        return n;
    }

    // Drops the swaps without reading or writing the modes, which the game may have freed.
    size_t forget()
    {
        size_t n = swaps_.size();
        swaps_.clear();
        return n;
    }

    size_t active() const { return swaps_.size(); }

private:
    struct Swap {
        uint8_t *mode;
        uint8_t original[TYRE_REQUIREMENT_BYTES], written[TYRE_REQUIREMENT_BYTES];
    };
    Swap *find(const uint8_t *mode)
    {
        for (auto &s : swaps_)
            if (s.mode == mode)
                return &s;
        return nullptr;
    }
    std::vector<Swap> swaps_;
};

inline const char *holder_text(const void *holder)
{
    return holder ? *reinterpret_cast<const char *const *>(static_cast<const uint8_t *>(holder) + HOLDER_TEXT) : nullptr;
}

// Every parsed game setup mode: Sess_Apply_ContentAvailability calls the thunk 0x142a223f0 (-> 0x14d83fa30) with
// g_Game_Scenarios, which passes the registry's +0x40 to Sess_Load_GameSetupModes (0x142a148f0). That loader parses each
// game_setup.game_setup_modes row with available=true and stores it by its stat id (the parsed byte +0x80) in a vector
// {allocator +0x40, begin +0x48, end +0x50, capacity +0x58} of 16-byte entries {allocator, GameSetupMode *}; ids
// without a row hold a null mode. A game mode's own setup mode list (the vfunc +0x1d0 list the era picker walks) holds
// only the modes its game_setup_modes field names, so a weekend helper mode that no game mode lists (it must not: a
// listed mode is offered by the picker) is only found here. Seen in game 2026-11-15: f50_wk (stat id 21) was not in
// the Grand Prix mode's list.
constexpr size_t STORE_SETUP_MODES_BEGIN = 0x48;
constexpr size_t STORE_SETUP_MODES_END = 0x50;
constexpr size_t STORE_SETUP_MODE_ENTRY = 0x10;
constexpr size_t STORE_SETUP_MODE_PTR = 8;
constexpr size_t STORE_SETUP_MODES_MAX = 256; // the stat id is a byte

// Finds the stored setup mode labelled want. readable(p, n) guards every read (the game passes mem::readable). Counts
// the stored modes in *seen and collects their labels (at most 63 characters each, "?" when unreadable) in *labels
// when given, for the log line of a helper that is not there.
template <class Readable>
uint8_t *find_stored_mode(const uint8_t *registry, const std::string &want, Readable readable, size_t *seen = nullptr,
                          std::vector<std::string> *labels = nullptr)
{
    if (seen)
        *seen = 0;
    if (labels)
        labels->clear();
    if (!registry || want.empty() || !readable(registry + STORE_SETUP_MODES_BEGIN, 16))
        return nullptr;
    auto *begin = *reinterpret_cast<const uint8_t *const *>(registry + STORE_SETUP_MODES_BEGIN);
    auto *end = *reinterpret_cast<const uint8_t *const *>(registry + STORE_SETUP_MODES_END);
    if (!begin || end < begin || size_t(end - begin) % STORE_SETUP_MODE_ENTRY ||
        size_t(end - begin) / STORE_SETUP_MODE_ENTRY > STORE_SETUP_MODES_MAX || !readable(begin, size_t(end - begin)))
        return nullptr;
    for (auto *e = begin; e != end; e += STORE_SETUP_MODE_ENTRY) {
        auto *mode = *reinterpret_cast<uint8_t *const *>(e + STORE_SETUP_MODE_PTR);
        if (!mode || !readable(mode + MODE_NAME, 8))
            continue;
        if (seen)
            ++*seen;
        auto *holder = *reinterpret_cast<const uint8_t *const *>(mode + MODE_NAME);
        const char *text = holder && readable(holder + HOLDER_TEXT, 8) ? holder_text(holder) : nullptr;
        if (labels) {
            std::string l;
            for (size_t i = 0; text && i < 63 && readable(text + i, 1) && text[i]; i++)
                l += text[i];
            labels->push_back(l.empty() ? std::string("?") : l);
        }
        if (!text || !readable(text, want.size() + 1))
            continue;
        if (!strncmp(text, want.c_str(), want.size() + 1))
            return mode;
    }
    return nullptr;
}

// Sess_Apply_ContentAvailability calls Sess_Load_GameSetupModes (through the thunk 0x142a223f0), which frees every
// GameSetupMode object before it parses the rows again. The pointers the picker keeps into the old objects (its
// narrowed entrant setups, its DRS group swaps, the Time Trial hold) are dropped at that function's entry and never
// written back; the next press or picker build applies the box to the new objects.
struct SetupPointers {
    size_t narrowed = 0, drs = 0;
    bool held = false;
};

inline std::string availability_drop_line(const SetupPointers &p)
{
    std::string what;
    auto add = [&](const std::string &part) { what += (what.empty() ? "" : ", ") + part; };
    if (p.narrowed)
        add(std::to_string(p.narrowed) + " narrowed entrant setup mode(s)");
    if (p.drs)
        add(std::to_string(p.drs) + " DRS group swap(s)");
    if (p.held)
        add("the Time Trial hold");
    if (what.empty())
        return {};
    return "the game rebuilds its setup modes (Sess_Apply_ContentAvailability): dropped " + what +
           " without writing them back; the next box press or picker build applies the box again";
}

}
