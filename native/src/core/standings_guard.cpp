#include "runtime.hpp"
#include "track_record_logic.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <vector>

// Standings track guard (re/db/globals.tsv site_Career_StandingsWeekendTrackLoad).
//
// Career_Standings_BuildWeekendNode builds one weekend_<n> node per round of the season for the standings screens
// (Team HQ > Standings). For each round (rbp = round index, [rsp+0xa0] = the Weekend) it looks the round's track
// (Weekend +0x278) up in the career's TrackDataSystem (CareerEntity +0xf58) and keeps the TrackDataRecord in rbx, or
// 0 when the track has no record. One branch checks rbx; the other reads [rbx+0x2c0] without a check. A round at a
// community track that has since been turned off has no record: ACCESS_VIOLATION reading 0x2c0 at
// F1_25.exe+0x181e1a1 as the standings open.
//
// At site_Career_StandingsWeekendTrackLoad a null rbx continues at site_Career_StandingsWeekendTrackDone, the checked
// branch, which the game takes for a round without a record and which leaves that round's track fields empty. A
// round with a record runs the game's code unchanged.
//
// The same guard covers the weekend list builder at 0x14181b3a0 (Career_Weekends_BuildResultNodes), which Team HQ
// screens such as the R&D table build per round: weekend_<n> nodes with track, show_pos_result, pos_string and
// visited_track. For each round it takes the TrackDataRecord from TrackDataSystem (CareerEntity +0xf58) +0x28 by Weekend
// +0x278 into rax and reads [rax+0x2c0] without checking rax, so a round whose track has no record faults with
// ACCESS_VIOLATION reading 0x2c0 at F1_25.exe+0x181b859. The record is empty for a track the build or the addons leave
// out, such as Paul Ricard (stat id 1), which Progressive Career puts on the 2018, 2019, 2021 and 2022 calendars and
// which only a community track fills. At site_Career_WeekendsTrackRecordLoad a null rax resumes after the load with
// rax still 0, which the game already handles (the next instructions test rax and use an empty node), so the round
// shows no track. Each track is logged once.
//
// The same site also resumes with rax 0 when the record is not what the game expects: a track id (rdi) past the end of
// the TrackDataSystem track list (r14 is the builder, [[r14+0x40]+0xf58] the system, +0x28 and +0x30 the list, 16-byte
// items), a record whose +0x2c0 is unreadable, or a record whose +0x2c0 holds a node pointer whose first word
// (the load at F1_25.exe+0x181b87a, lea rcx,[rax+8] then mov rax,[rcx]) is unreadable. A season saved with a track the
// build no longer has reads a stale item past the list and then faults reading 0x1110000008 at F1_25.exe+0x181b87a. These
// cases are logged once per track as [guard] weekends. A round with a readable record and node runs the game's code
// unchanged.
//
// SWOFTY_STANDINGS_GUARD=0 turns both off.

namespace {

namespace mem = swofty::mem;

constexpr const char *MIXIN = "swofty_core.dll (standings track guard)";
constexpr size_t FRAME_WEEKEND = 0xa0;
constexpr size_t WEEKEND_TRACK = 0x278;
constexpr size_t LOG_ROUNDS = 64;

constexpr size_t RECORD_LOAD_LEN = 7;
constexpr size_t BUILDER_MODE = 0x40;
constexpr size_t MODE_TRACK_DATA = 0xf58;
constexpr size_t SYSTEM_RECORDS_BEGIN = 0x28;
constexpr size_t SYSTEM_RECORDS_END = 0x30;
constexpr size_t RECORD_ITEM = 0x10;
constexpr size_t RECORD_NODE = 0x2c0;
constexpr size_t NODE_FIRST_WORD = 0x8;

uintptr_t track_done = 0, record_next = 0;
runtime::track_record::Seen record_seen;
bool offered = false, settled = false;
std::mutex logged_lock;
std::vector<uint32_t> logged;

void track_load(SwoftyContext *c)
{
    if (c->rbx)
        return;
    c->rip = track_done;
    uintptr_t weekend = 0;
    uint8_t track = 0;
    bool known = mem::read(c->rsp + FRAME_WEEKEND, weekend) && weekend && mem::read(weekend + WEEKEND_TRACK, track);
    uint32_t round = static_cast<uint32_t>(c->rbp) + 1;
    uint32_t key = (round << 9) | (known ? track : 0x100u);
    bool last = false;
    {
        std::lock_guard<std::mutex> hold(logged_lock);
        for (uint32_t k : logged)
            if (k == key)
                return;
        if (logged.size() >= LOG_ROUNDS)
            return;
        logged.push_back(key);
        last = logged.size() == LOG_ROUNDS;
    }
    const char *more = last ? " (further rounds are not logged)" : "";
    if (known)
        swofty::log("[career] standings: round %u has no track data (track %u is off); skipped%s", round, unsigned(track),
                    more);
    else
        swofty::log("[career] standings: round %u has no track data; skipped%s", round, more);
}

bool table_count(uintptr_t builder, uint64_t &count)
{
    uintptr_t mode = 0, system = 0, begin = 0, end = 0;
    if (!mem::read(builder + BUILDER_MODE, mode) || !mode || !mem::read(mode + MODE_TRACK_DATA, system) || !system ||
        !mem::read(system + SYSTEM_RECORDS_BEGIN, begin) || !mem::read(system + SYSTEM_RECORDS_END, end) || end < begin)
        return false;
    count = (end - begin) / RECORD_ITEM;
    return true;
}

void record_load(SwoftyContext *c)
{
    runtime::track_record::Round r;
    r.track = static_cast<uint8_t>(c->rdi);
    r.table_known = table_count(c->r14, r.table_count);
    r.record = c->rax;
    r.record_readable = !r.record || mem::readable(r.record + RECORD_NODE, sizeof(uintptr_t));
    r.node_readable = true;
    if (r.record && r.record_readable && mem::read(r.record + RECORD_NODE, r.node))
        r.node_readable = mem::readable(r.node + NODE_FIRST_WORD, sizeof(uintptr_t));
    auto verdict = runtime::track_record::judge(r);
    if (verdict == runtime::track_record::Verdict::game)
        return;
    c->rax = 0;
    c->rip = record_next;
    uint8_t track = static_cast<uint8_t>(r.track);
    bool first;
    {
        std::lock_guard<std::mutex> hold(logged_lock);
        first = record_seen.first(track);
    }
    if (!first)
        return;
    if (verdict == runtime::track_record::Verdict::no_record)
        swofty::log("[career] weekends: track %u has no track data; the round shows no track", unsigned(track));
    else
        swofty::log("[guard] weekends: track id %u has no record (%s, record 0x%llx, node 0x%llx, %llu tracks); the round shows no track",
                    unsigned(track), runtime::track_record::why(verdict), (unsigned long long)r.record,
                    (unsigned long long)r.node, (unsigned long long)r.table_count);
}

}

namespace runtime {

std::vector<CoreMixins> standings_guard_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_STANDINGS_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] standings guard off (SWOFTY_STANDINGS_GUARD=%s)", v);
        return out;
    }
    track_done = swofty::api->resolve("site_Career_StandingsWeekendTrackDone");
    if (!track_done || !swofty::api->resolve("site_Career_StandingsWeekendTrackLoad")) {
        track_done = 0;
        swofty::log("[career] standings guard off: needs site_Career_StandingsWeekendTrackLoad and "
                    "site_Career_StandingsWeekendTrackDone, which this build does not resolve");
        return out;
    }
    std::vector<SwoftyMixin> sites = {
        {"site_Career_StandingsWeekendTrackLoad", SWOFTY_MIXIN_SITE, 100, (void *)&track_load, nullptr}};
    uintptr_t record_load_site = swofty::api->resolve("site_Career_WeekendsTrackRecordLoad");
    if (record_load_site) {
        record_next = record_load_site + RECORD_LOAD_LEN;
        sites.push_back({"site_Career_WeekendsTrackRecordLoad", SWOFTY_MIXIN_SITE, 100, (void *)&record_load, nullptr});
    } else
        swofty::log("[career] weekends track guard off: site_Career_WeekendsTrackRecordLoad is not resolved in this build");
    out.push_back({MIXIN, sites});
    return out;
}

void standings_guard_commit()
{
    if (!offered || settled || !track_done)
        return;
    settled = true;
    swofty::log("[career] standings guard %s",
                core_mixins_active(MIXIN)
                    ? "armed: a standings round whose track has no track data is skipped instead of faulting"
                    : "did not install (see the [core] lines)");
}

}
