#include "box_grid_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include "apply/content/appliers.hpp"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <map>
#include <mutex>
#include <optional>
#include <string>
#include <utility>
#include <vector>

// Grid rules per box (content.package 1.5 rules.grid): qualifying scoring and the 107 % rule.
//
// The game ranks the race grid from the weekend's group-1 (Qualifying) classification: Weekend+0x488 is a
// std::map<u8 group, GroupResult>; a GroupResult holds 0x60-byte rows at +0x08..+0x10 (group position at row+0x50,
// 0x1c = unclassified) and the per-driver best lap of the group's latest session at +0x28..+0x30 ({u32 driver key,
// f32 time}). Sess_Record_GroupTimes (0x140af2880) writes those times after every qualifying-type session, driven or
// simulated; Sess_Build_GroupResult (0x140afa220) builds the rows when the group's last session ends or a save loads;
// the grid setup (Game_GridSetup_SetupStartingGrid 0x141210160 -> Game_GridSetup_FindQualiResult) reads the rows.
// This is the path the F1 1990 addon used for its best-of-two grid (addons/f1-1990 grid_best_of.cpp, 1.6 era).
//
// The era picker hands the pressed box's rules.grid here (box_grid_set). With a rule that re-ranks (qualifying
// best_of or aggregate, or rule_107), two wraps run:
// - after Sess_Record_GroupTimes, the group's times are kept per (group, session) of the weekend, so a driver's best
//   lap of every qualifying-type session stays known after the next session replaces it in the GroupResult;
// - after Sess_Build_GroupResult (group 1) and after each Sess_Record_GroupTimes, the group-1 row positions are
//   rewritten through the game's own Sess_Find_GroupRowPosition with runtime::box_grid::rank. Group 2 (sprint
//   shootout) times count only on a weekend without a sprint race (no race_2 session), the two-qualifying shape.
// The 107 % rule moves a car outside the limit behind every car inside it: the game has no non-qualifier, and a car
// removed from the start (retirement or disqualification at the lights) would change the race's entrant handling,
// so starting from the back is the safe form. A stock box or a box without a grid rule leaves the game's grid.
//
// rules.grid.rows moves the race grid's slots into the box's row pattern (3-2-3, 4-3-4). The game drops a car on its
// grid slot in Sess_VehiclePlacement_PlaceOnSlot (0x142f343d0, (placement, vehicle id, type, slot, tolerance), type 5 =
// grid): the slot's request is placement+0x38+type*0x100+slot*8 (request +0x18 slot, +0x1c type), the function
// refreshes it with Sess_VehiclePlacement_ResolveTransform (0x142f62580) and places the car at the request's
// transform (+0x20, position at +0x50/+0x54/+0x58, y up). With a row pattern two wraps run: the PlaceOnSlot wrap
// resolves the track's stock grid slots (the same call the game makes for each of them), computes the slot's place in
// the pattern (runtime::box_grid::row_position: pattern row r on stock row r's centre line, spread across it) and
// arms an override for that one request; the ResolveTransform wrap writes the position after the game's resolve, and
// the PlaceOnSlot wrap puts the request's own position back after the car is placed, so the track's slots never change.
// Any slot the maths can't place (missing stock slot, odd grid) keeps the game's own position.
//
// SWOFTY_BOX_GRID=0 turns it off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (grid rules per box)";
constexpr const char *RECORD_FN = "Sess_Record_GroupTimes";
constexpr const char *BUILD_FN = "Sess_Build_GroupResult";
constexpr const char *FIND_FN = "Sess_Find_GroupRowPosition";
constexpr const char *PLACE_FN = "Sess_VehiclePlacement_PlaceOnSlot";
constexpr const char *RESOLVE_FN = "Sess_VehiclePlacement_ResolveTransform";
constexpr uint8_t PLACE_GRID = 5;
constexpr uint32_t PLACE_SLOTS = 32;
constexpr size_t PLACE_REQUESTS = 0x38, PLACE_TYPE_STRIDE = 0x100, REQUEST_POS = 0x50;

constexpr size_t WEEKEND_SESSIONS_BEGIN = 0x220, WEEKEND_SESSIONS_END = 0x228, WEEKEND_CURRENT = 0x238;
constexpr size_t WEEKEND_PARTICIPANTS_BEGIN = 0x288, WEEKEND_PARTICIPANTS_END = 0x290, WEEKEND_GROUPS = 0x488;
constexpr size_t SESSION_STRIDE = 0xa0, SESSION_ID = 0x08;
constexpr size_t NODE_PARENT = 0x08, NODE_LEFT = 0x00, NODE_RIGHT = 0x10, NODE_IS_NIL = 0x19, NODE_KEY = 0x20,
                 NODE_VALUE = 0x28;
constexpr size_t RESULT_ROWS_BEGIN = 0x08, RESULT_ROWS_END = 0x10, RESULT_TIMES_BEGIN = 0x28, RESULT_TIMES_END = 0x30;
constexpr size_t PARTICIPANT_FLAGS = 0x4b, PARTICIPANT_KEY = 0x50;
constexpr uint8_t PARTICIPANT_SKIP = 0x20;
constexpr uint8_t QUALIFYING = 1, SHOOTOUT = 2;
constexpr uint32_t RACE_2 = 16;
constexpr size_t MAX_SESSIONS = 32, MAX_TIMES = 64, MAX_PARTICIPANTS = 64;
constexpr uint32_t LIVE_SESSION = 0xffffffffu;
constexpr int LOG_APPLIES = 12;

namespace bg = runtime::box_grid;
namespace mem = swofty::mem;

using Record_t = void(void *weekend, uint8_t group, void *entries);
using Build_t = void(void *weekend, uint8_t group, void *result);
using Find_t = int32_t *(void *weekend, void *result, uint32_t driver_key);
Record_t *record_orig = nullptr;
Build_t *build_orig = nullptr;
Find_t *find_fn = nullptr;
using Place_t = void(void *placement, uint8_t vehicle, uint8_t type, uint32_t slot, float tolerance);
using Resolve_t = void(void *placement, void *request);
Place_t *place_orig = nullptr;
Resolve_t *resolve_orig = nullptr;
std::atomic<bool> rows_on{false};   // a row pattern is active (fast path for every other placement)
thread_local uintptr_t override_request = 0;
thread_local bg::Vec3 override_pos{};
int rows_logged = 0;

bool offered = false, settled = false, wanted = false;
std::mutex lock;
std::optional<bg::Rule> active;
std::string active_box;
uintptr_t weekend_seen = 0;
std::map<std::pair<uint8_t, uint32_t>, std::map<uint32_t, float>> snapshots;   // (group, session) -> key -> best lap
std::string last_signature;
int applies_logged = 0;

template <class T> T rd(uintptr_t at, T fallback = T{})
{
    T v{};
    return mem::read(at, v) ? v : fallback;
}

uintptr_t group_result(uintptr_t weekend, uint8_t group)
{
    uintptr_t head = rd<uintptr_t>(weekend + WEEKEND_GROUPS);
    if (!head)
        return 0;
    uintptr_t n = rd<uintptr_t>(head + NODE_PARENT);
    for (int guard = 0; n && guard < 64 && !rd<uint8_t>(n + NODE_IS_NIL, 1); ++guard) {
        uint8_t key = rd<uint8_t>(n + NODE_KEY, 0xff);
        if (key == group)
            return n + NODE_VALUE;
        n = rd<uintptr_t>(n + (group < key ? NODE_LEFT : NODE_RIGHT));
    }
    return 0;
}

std::map<uint32_t, float> times_of(uintptr_t result)
{
    std::map<uint32_t, float> out;
    uintptr_t b = rd<uintptr_t>(result + RESULT_TIMES_BEGIN), e = rd<uintptr_t>(result + RESULT_TIMES_END);
    if (!b || e < b || (e - b) / 8 > MAX_TIMES)
        return out;
    for (uintptr_t p = b; p < e; p += 8) {
        uint32_t key = 0;
        float t = 0.f;
        if (mem::read(p, key) && mem::read(p + 4, t) && bg::has_time(t))
            out[key] = t;
    }
    return out;
}

// A weekend whose sprint shootout group feeds a sprint race (race_2) keeps group 2 out of the race grid.
bool shootout_counts(uintptr_t weekend)
{
    uintptr_t s = rd<uintptr_t>(weekend + WEEKEND_SESSIONS_BEGIN), e = rd<uintptr_t>(weekend + WEEKEND_SESSIONS_END);
    if (!s || e < s || (e - s) / SESSION_STRIDE > MAX_SESSIONS)
        return false;
    for (; s < e; s += SESSION_STRIDE)
        if (rd<uint32_t>(s + SESSION_ID) == RACE_2)
            return false;
    return true;
}

void forget_weekend(uintptr_t weekend)
{
    if (weekend == weekend_seen)
        return;
    weekend_seen = weekend;
    snapshots.clear();
    last_signature.clear();
}

// Caller holds lock.
void apply(uintptr_t weekend, const char *why)
{
    if (!active || !active->reranks() || !find_fn)
        return;
    uintptr_t quali = group_result(weekend, QUALIFYING);
    if (!quali || rd<uintptr_t>(quali + RESULT_ROWS_BEGIN) == rd<uintptr_t>(quali + RESULT_ROWS_END))
        return;
    bool shootout = shootout_counts(weekend);
    // The sessions to rank by: every kept (group, session) snapshot, and the live group times of a group that has none.
    std::vector<const std::map<uint32_t, float> *> sessions;
    std::map<uint32_t, float> live[2];
    for (uint8_t g : {QUALIFYING, SHOOTOUT}) {
        if (g == SHOOTOUT && !shootout)
            continue;
        bool kept = false;
        for (auto &[k, times] : snapshots)
            if (k.first == g) {
                sessions.push_back(&times);
                kept = true;
            }
        if (!kept) {
            uintptr_t r = g == QUALIFYING ? quali : group_result(weekend, g);
            if (r) {
                live[g - 1] = times_of(r);
                if (!live[g - 1].empty())
                    sessions.push_back(&live[g - 1]);
            }
        }
    }
    uintptr_t pb = rd<uintptr_t>(weekend + WEEKEND_PARTICIPANTS_BEGIN), pe = rd<uintptr_t>(weekend + WEEKEND_PARTICIPANTS_END);
    if (!pb || pe < pb || (pe - pb) / 8 > MAX_PARTICIPANTS)
        return;
    std::vector<bg::Driver> drivers;
    std::vector<int32_t *> slots;
    for (uintptr_t p = pb; p < pe; p += 8) {
        uintptr_t part = rd<uintptr_t>(p);
        if (!part || (rd<uint8_t>(part + PARTICIPANT_FLAGS) & PARTICIPANT_SKIP))
            continue;
        uint32_t key = rd<uint32_t>(part + PARTICIPANT_KEY);
        int32_t *pos = find_fn(reinterpret_cast<void *>(weekend), reinterpret_cast<void *>(quali), key);
        if (!pos || std::find(slots.begin(), slots.end(), pos) != slots.end() || !mem::readable(uintptr_t(pos), 4))
            continue;
        bg::Driver d;
        d.key = key;
        d.game_position = *pos;
        for (auto *s : sessions) {
            auto it = s->find(key);
            d.session_best.push_back(it == s->end() ? 0.f : it->second);
        }
        slots.push_back(pos);
        drivers.push_back(std::move(d));
    }
    if (drivers.empty())
        return;
    bg::Ranked r = bg::rank(drivers, *active);
    std::string sig;
    for (size_t i = 0; i < drivers.size(); i++)
        sig += std::to_string(drivers[i].key) + ":" + std::to_string(r.positions[i]) + ";";
    if (r.changed)
        for (size_t i = 0; i < drivers.size(); i++)
            *slots[i] = r.positions[i];
    if (sig == last_signature || applies_logged >= LOG_APPLIES)
        return;
    last_signature = sig;
    applies_logged++;
    swofty::log("[grid] box %s (%s): %s over %zu session(s), %zu drivers%s%s; %s", active_box.c_str(), why,
                bg::describe(*active).c_str(), sessions.size(), drivers.size(),
                r.aggregate_one_session ? " (aggregate with one session: ranked by the best lap)" : "",
                r.limit > 0.f ? (", pole " + bg::time_text(r.pole) + " 107 % " + bg::time_text(r.limit)).c_str() : "",
                r.changed ? "group positions rewritten" : "the game's order already matches");
    for (size_t i = 0; i < drivers.size(); i++) {
        std::string times;
        for (float t : drivers[i].session_best)
            times += (times.empty() ? "" : " ") + bg::time_text(t);
        swofty::log("[grid]   driver %u/%u times [%s] score %s (%s): position %d -> %d", drivers[i].key & 0xff,
                    drivers[i].key >> 16, times.c_str(), bg::time_text(r.scores[i].time).c_str(),
                    bg::class_name(r.classes[i]), drivers[i].game_position, r.positions[i]);
    }
}

void record_entry(void *weekend, uint8_t group, void *entries)
{
    record_orig(weekend, group, entries);
    if (group != QUALIFYING && group != SHOOTOUT)
        return;
    std::lock_guard<std::mutex> g(lock);
    if (!active || !active->reranks())
        return;
    uintptr_t w = uintptr_t(weekend);
    forget_weekend(w);
    uintptr_t r = group_result(w, group);
    if (r) {
        auto times = times_of(r);
        if (!times.empty() && snapshots.size() < MAX_SESSIONS)
            snapshots[{group, rd<uint32_t>(w + WEEKEND_CURRENT, LIVE_SESSION)}] = std::move(times);
    }
    apply(w, group == QUALIFYING ? "qualifying times" : "shootout times");
}

void build_entry(void *weekend, uint8_t group, void *result)
{
    build_orig(weekend, group, result);
    if (group != QUALIFYING)
        return;
    std::lock_guard<std::mutex> g(lock);
    if (!active || !active->reranks())
        return;
    forget_weekend(uintptr_t(weekend));
    apply(uintptr_t(weekend), "qualifying built");
}


uintptr_t grid_request(uintptr_t placement, uint32_t slot)
{
    return rd<uintptr_t>(placement + PLACE_REQUESTS + PLACE_GRID * PLACE_TYPE_STRIDE + slot * 8);
}

void resolve_entry(void *placement, void *request)
{
    resolve_orig(placement, request);
    if (override_request && uintptr_t(request) == override_request) {
        float p[3] = {override_pos.x, override_pos.y, override_pos.z};
        if (mem::readable(override_request + REQUEST_POS, sizeof p))
            std::memcpy(reinterpret_cast<void *>(override_request + REQUEST_POS), p, sizeof p);
    }
}

void place_entry(void *placement, uint8_t vehicle, uint8_t type, uint32_t slot, float tolerance)
{
    if (type != PLACE_GRID || slot >= PLACE_SLOTS || !rows_on.load(std::memory_order_relaxed)) {
        place_orig(placement, vehicle, type, slot, tolerance);
        return;
    }
    std::vector<int> pattern;
    std::string box;
    {
        std::lock_guard<std::mutex> g(lock);
        if (active && active->moves_slots()) {
            pattern = active->rows;
            box = active_box;
        }
    }
    uintptr_t pl = uintptr_t(placement), req = pattern.empty() ? 0 : grid_request(pl, slot);
    std::optional<bg::Vec3> at;
    if (req) {
        // The track's stock grid: every slot's request, resolved the way the game resolves it before placing a car.
        std::vector<bg::Vec3> stock;
        for (uint32_t k = 0; k < PLACE_SLOTS; k++) {
            uintptr_t r = grid_request(pl, k);
            float p[3] = {};
            if (!r || !mem::readable(r + REQUEST_POS, sizeof p))
                break;
            resolve_orig(placement, reinterpret_cast<void *>(r));
            std::memcpy(p, reinterpret_cast<const void *>(r + REQUEST_POS), sizeof p);
            if (!std::isfinite(p[0]) || !std::isfinite(p[1]) || !std::isfinite(p[2]))
                break;
            stock.push_back({p[0], p[1], p[2]});
        }
        at = bg::row_position(pattern, stock, int(slot));
        if (rows_logged < LOG_APPLIES) {
            rows_logged++;
            if (at)
                swofty::log("[grid] box %s: grid slot %u -> row pattern place (%.1f, %.1f, %.1f), %zu stock slots", box.c_str(),
                            slot, at->x, at->y, at->z, stock.size());
            else
                swofty::log("[grid] box %s: grid slot %u keeps the track's slot (%zu stock slots)", box.c_str(), slot,
                            stock.size());
        }
    }
    float saved[3] = {};
    bool armed = at && mem::readable(req + REQUEST_POS, sizeof saved);
    if (armed)
        std::memcpy(saved, reinterpret_cast<const void *>(req + REQUEST_POS), sizeof saved);
    if (armed) {
        override_request = req;
        override_pos = *at;
    }
    place_orig(placement, vehicle, type, slot, tolerance);
    if (armed) {
        override_request = 0;
        std::memcpy(reinterpret_cast<void *>(req + REQUEST_POS), saved, sizeof saved);
    }
}

}

namespace runtime {

// The pressed box's grid rule (none: a stock box, a box without a rule, or no box). Called by the era picker.
void box_grid_set(const std::optional<box_grid::Rule> &rule, const std::string &box)
{
    std::lock_guard<std::mutex> g(lock);
    bool same = box == active_box && rule.has_value() == active.has_value() &&
                (!rule || (rule->qualifying == active->qualifying && rule->rule_107 == active->rule_107 &&
                           rule->rows == active->rows));
    if (same)
        return;
    active = rule;
    active_box = box;
    rows_on.store(rule && rule->moves_slots(), std::memory_order_relaxed);
    rows_logged = 0;
    snapshots.clear();
    weekend_seen = 0;
    last_signature.clear();
}

std::vector<CoreMixins> box_grid_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    if (!swofty_core::apply::content::any_grid_rule())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_BOX_GRID", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[grid] grid rules per box off (SWOFTY_BOX_GRID=%s)", v);
        return out;
    }
    uintptr_t find = swofty::api->resolve(FIND_FN);
    if (!find || !swofty::api->resolve(RECORD_FN) || !swofty::api->resolve(BUILD_FN)) {
        swofty::log("[grid] grid rules per box off: needs %s, %s and %s, which this build does not all resolve", RECORD_FN,
                    BUILD_FN, FIND_FN);
        return out;
    }
    find_fn = reinterpret_cast<Find_t *>(find);
    wanted = true;
    std::vector<SwoftyMixin> specs = {{RECORD_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&record_entry, (void **)&record_orig},
                                            {BUILD_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&build_entry, (void **)&build_orig}};
    // Row patterns need the placement pair; without it the boxes keep the track's 2-wide slots (scoring still works).
    if (swofty::api->resolve(PLACE_FN) && swofty::api->resolve(RESOLVE_FN)) {
        specs.push_back({PLACE_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&place_entry, (void **)&place_orig});
        specs.push_back({RESOLVE_FN, SWOFTY_MIXIN_WRAP, 100, (void *)&resolve_entry, (void **)&resolve_orig});
    } else
        swofty::log("[grid] row patterns off: this build does not resolve %s and %s", PLACE_FN, RESOLVE_FN);
    out.push_back({MIXIN, specs});
    return out;
}

void box_grid_commit()
{
    if (!offered || settled || !wanted)
        return;
    settled = true;
    swofty::log("[grid] grid rules per box %s",
                core_mixins_active(MIXIN) ? "armed: a box's rules.grid re-ranks the weekend's qualifying classification"
                                          : "did not install (see the [core] lines); every box keeps the game's grid");
}

}
