#include "runtime.hpp"
#include "standings_drop_logic.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <map>
#include <mutex>
#include <string>
#include <vector>

// Dropped scores (re/db/functions.tsv Career_Recompute_SeasonStandings, Career_Compare_StandingsEntries).
//
// The drivers' standings of a season (F1SeasonStandings, embedded in Season at +0x2a8) hold the points each session
// scored (+0x20: rounds of 0x20 bytes {allocator, sessions begin, end, capacity}, sessions of 0x28 bytes {allocator,
// records begin, end, capacity, session id at +0x20}, records {u32 key, i32 points}) and the cumulative table
// (+0x40, the same shape): the session's table is a copy of the previous one plus the session's points, entries
// {u32 key, i32 total}. Career_Recompute_SeasonStandings (0x140b2fa60, a jmp to the .data1 body 0x149618ee0) refreshes
// the points, adds every new session to the table and sorts every table session with Career_Compare_StandingsEntries
// (total, then the countback of finishing positions, then the key). The constructors' standings sum the drivers'
// points, not this table, so they keep counting every result.
//
// After the game's recompute, a data group with a rule has every table entry set to the sum of the best N
// per-round totals of the driver up to that session, taken from the points records, which stay gross, and each
// table session is sorted again with the game's own predicate. The totals are absolute, so a later recompute that
// adds a session on top of a counted table is corrected the same way.
//
// The rule is set by set_dropped_scores(data group, N) and applies while the loaded career is filed under that data
// group, to the seasons of the current championship. The hook is installed by the first set_dropped_scores call with a best above 0, so a launch with no
// such addon never runs through it. SWOFTY_STANDINGS_DROP=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace sd = runtime::standings_drop;

const runtime::Sig S_RECOMPUTE{"Career_Recompute_SeasonStandings", "E9 ?? ?? ?? ?? 84 F4 0F 83 ?? ?? ?? ?? 8C AB 0F",
                               0x140b2fa60, runtime::Sig::addr, 0, 0, 0};
const runtime::Sig S_COMPARE{"Career_Compare_StandingsEntries", "41 56 56 57 53 48 81 EC 08 01 00 00 41 8B 40 04",
                             0x140b317a0, runtime::Sig::addr, 0, 0, 0};

constexpr size_t SEASON_STANDINGS = 0x2a8;
constexpr size_t POINTS_ROUNDS = 0x20, TABLE_ROUNDS = 0x40;
constexpr size_t ROUND_SIZE = 0x20, SESSION_SIZE = 0x28, RECORD_SIZE = 8;
constexpr size_t SESSION_ID = 0x20;
constexpr size_t CHAMPIONSHIP = 0x1058, SEASONS_BEGIN = 0x548, SEASONS_END = 0x550, SEASON_ENTRY = 0x10;
constexpr size_t MAX_ROUNDS = 64, MAX_SESSIONS = 64, MAX_RECORDS = 128;

using Compare_t = bool(void *ctx, const void *a, const void *b);
using Recompute_t = void(void *standings, char mode);

struct Entry {
    uint32_t key;
    int32_t total;
};

std::mutex rules_lock;
std::map<std::string, uint32_t> rules;
safetyhook::InlineHook h_recompute;
Compare_t *compare = nullptr;
bool logged_apply = false;

bool rd(uintptr_t at, uintptr_t &v)
{
    return mem::read(at, v);
}

struct RoundView {
    uintptr_t sessions_begin = 0, sessions_end = 0;
};

bool round_at(uintptr_t array_field, size_t index, RoundView &out)
{
    uintptr_t b = 0, e = 0;
    if (!rd(array_field, b) || !rd(array_field + 8, e) || e < b || (e - b) % ROUND_SIZE || index >= (e - b) / ROUND_SIZE)
        return false;
    uintptr_t r = b + index * ROUND_SIZE;
    return rd(r + 8, out.sessions_begin) && rd(r + 16, out.sessions_end) && out.sessions_end >= out.sessions_begin &&
           (out.sessions_end - out.sessions_begin) % SESSION_SIZE == 0 &&
           (out.sessions_end - out.sessions_begin) / SESSION_SIZE <= MAX_SESSIONS;
}

size_t round_count(uintptr_t array_field)
{
    uintptr_t b = 0, e = 0;
    if (!rd(array_field, b) || !rd(array_field + 8, e) || e < b || (e - b) % ROUND_SIZE)
        return 0;
    return std::min<size_t>((e - b) / ROUND_SIZE, MAX_ROUNDS);
}

bool read_points(uintptr_t standings, std::vector<sd::PointsRound> &out)
{
    size_t n = round_count(standings + POINTS_ROUNDS);
    for (size_t r = 0; r < n; r++) {
        RoundView v;
        if (!round_at(standings + POINTS_ROUNDS, r, v))
            return false;
        sd::PointsRound round;
        for (uintptr_t s = v.sessions_begin; s < v.sessions_end; s += SESSION_SIZE) {
            sd::PointsSession ps;
            uintptr_t rb = 0, re = 0;
            uint32_t id = 0;
            if (!rd(s + 8, rb) || !rd(s + 16, re) || re < rb || (re - rb) % RECORD_SIZE ||
                (re - rb) / RECORD_SIZE > MAX_RECORDS || !mem::read(s + SESSION_ID, id))
                return false;
            ps.id = id;
            for (uintptr_t p = rb; p < re; p += RECORD_SIZE) {
                sd::Record rec{};
                if (!mem::read(p, rec))
                    return false;
                ps.records.push_back(rec);
            }
            round.push_back(std::move(ps));
        }
        out.push_back(std::move(round));
    }
    return true;
}

bool is_current_season(uintptr_t standings)
{
    const SwoftyCareer *c = runtime::career_table();
    auto e = c && c->entity ? reinterpret_cast<uintptr_t>(c->entity()) : 0;
    uintptr_t champ = 0, sb = 0, se = 0;
    if (!e || !mem::object_like(e) || !rd(e + CHAMPIONSHIP, champ) || !champ || !rd(champ + SEASONS_BEGIN, sb) ||
        !rd(champ + SEASONS_END, se) || se < sb || (se - sb) % SEASON_ENTRY)
        return false;
    for (uintptr_t p = sb; p < se; p += SEASON_ENTRY) {
        uintptr_t season = 0;
        if (rd(p + 8, season) && season && season + SEASON_STANDINGS == standings)
            return true;
    }
    return false;
}

uint32_t rule_for_loaded_career()
{
    {
        std::lock_guard<std::mutex> hold(rules_lock);
        if (rules.empty())
            return 0;
    }
    const SwoftyCareerContent *t = runtime::career_content_table();
    char group[128] = {};
    if (!t || !t->data_group || t->data_group(nullptr, group, sizeof group) != SWOFTY_ASSET_OK)
        return 0;
    std::lock_guard<std::mutex> hold(rules_lock);
    auto it = rules.find(group);
    return it == rules.end() ? 0 : it->second;
}

void recount(uintptr_t standings, uint32_t best)
{
    std::vector<sd::PointsRound> points;
    if (!read_points(standings, points) || points.empty())
        return;
    size_t rounds = round_count(standings + TABLE_ROUNDS);
    uint64_t packed = 0;
    bool have_packed = false;
    for (size_t r = rounds; r-- > 0 && !have_packed;) {
        RoundView v;
        if (!round_at(standings + TABLE_ROUNDS, r, v) || v.sessions_end == v.sessions_begin)
            continue;
        uint32_t id = 0;
        if (mem::read(v.sessions_end - SESSION_SIZE + SESSION_ID, id)) {
            packed = (uint64_t(id) << 32) | uint32_t(r);
            have_packed = true;
        }
    }
    struct Ctx {
        uintptr_t standings;
        uint64_t *packed;
    } ctx{standings, &packed};
    size_t changed = 0, sessions = 0;
    for (size_t r = 0; r < rounds; r++) {
        RoundView v;
        if (!round_at(standings + TABLE_ROUNDS, r, v))
            continue;
        for (uintptr_t s = v.sessions_begin; s < v.sessions_end; s += SESSION_SIZE) {
            uintptr_t eb = 0, ee = 0;
            uint32_t id = 0;
            if (!rd(s + 8, eb) || !rd(s + 16, ee) || ee < eb || (ee - eb) % sizeof(Entry) ||
                (ee - eb) / sizeof(Entry) > MAX_RECORDS || !mem::read(s + SESSION_ID, id) || !mem::readable(eb, ee - eb))
                continue;
            auto *entries = reinterpret_cast<Entry *>(eb);
            size_t count = (ee - eb) / sizeof(Entry);
            for (size_t i = 0; i < count; i++) {
                int32_t counted = sd::counted_total(points, r, id, entries[i].key, best);
                changed += counted != entries[i].total;
                entries[i].total = counted;
            }
            if (compare && have_packed)
                std::sort(entries, entries + count,
                          [&](const Entry &a, const Entry &b) { return compare(&ctx, &a, &b); });
            sessions++;
        }
    }
    if (!logged_apply || changed) {
        swofty::log("[career] dropped scores: best %u results count; %zu table session(s) recounted, %zu total(s) changed",
                    best, sessions, changed);
        logged_apply = true;
    }
}

void recompute_hook(void *standings, char mode)
{
    h_recompute.call<void>(standings, mode);
    uint32_t best = rule_for_loaded_career();
    if (!best || !standings || !is_current_season(reinterpret_cast<uintptr_t>(standings)))
        return;
    recount(reinterpret_cast<uintptr_t>(standings), best);
}

bool armed = false, arm_failed = false;

bool arm()
{
    if (armed)
        return true;
    if (arm_failed)
        return false;
    arm_failed = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_STANDINGS_DROP", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] dropped scores off (SWOFTY_STANDINGS_DROP=%s)", v);
        return false;
    }
    uintptr_t recompute = runtime::find(S_RECOMPUTE, "career", "career"), cmp = runtime::find(S_COMPARE, "career", "career");
    if (!recompute || !cmp) {
        swofty::log("[career] dropped scores: not hooked: %s%s", recompute ? "" : "recompute ", cmp ? "" : "compare");
        return false;
    }
    compare = reinterpret_cast<Compare_t *>(cmp);
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(recompute), reinterpret_cast<void *>(&recompute_hook));
    if (!h) {
        swofty::log("[career] dropped scores: the hook failed");
        return false;
    }
    h_recompute = std::move(*h);
    armed = true;
    arm_failed = false;
    swofty::log("[career] dropped scores armed (hook at base+0x%llx), first rule set", (unsigned long long)recompute);
    return true;
}

}

namespace runtime {

int standings_drop_set(const std::string &data_group, uint32_t best)
{
    if (best && !arm())
        return SWOFTY_ASSET_LATE;
    std::lock_guard<std::mutex> hold(rules_lock);
    if (best)
        rules[data_group] = best;
    else
        rules.erase(data_group);
    swofty::log("[career] dropped scores: data group %s %s", data_group.c_str(),
                best ? ("counts the best " + std::to_string(best) + " results").c_str() : "counts every result");
    return SWOFTY_ASSET_OK;
}

}

