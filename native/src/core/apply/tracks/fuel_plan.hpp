#pragma once

#include "base.hpp"
#include "per_package.hpp"
#include "../../career_ops/fuel_logic.hpp"

#include <cstdint>
#include <cstdio>
#include <map>
#include <string>
#include <string_view>
#include <vector>

// The fuel writes of a box's track.per_package entries and of track.data records, planned over the track data sets
// the fuel writer reads (career_ops/fuel_logic.hpp): the box's burn per lap on its tracks in every set its setup
// mode can read and the sets the session's query picks, a track.data record's burn per lap in its named set. A track is written only when its fuel reads as
// fuel (one to three race rows) and differs from the planned value; each write keeps the bytes it found, so the core
// puts them back in reverse order. Pure.

namespace swofty_core::apply::tracks {

namespace fuel = ::career_ops::fuel;

struct FuelWrite {
    uintptr_t object = 0;
    std::string track, entry;
    fuel::Fuel before, after;
};

struct Query {
    uint32_t mode = 0, season = 0;
    bool season_used = false;
};

// The sets the session's query reads: for each weekend format the game knows (default, sprint), the first set in
// row order whose masks hold the mode, the season when it is used, and the format.
inline std::vector<const fuel::Set *> query_sets(const std::vector<fuel::Set> &sets, const Query &q)
{
    auto has = [](uint32_t mask, uint32_t bit) { return bit < 32 && ((mask >> bit) & 1u); };
    std::vector<const fuel::Set *> out;
    for (uint32_t format : {fuel::kFormatDefault, fuel::kFormatSprint})
        for (auto &s : sets)
            if (has(s.modes, q.mode) && (!q.season_used || has(s.seasons, q.season)) && has(s.formats, format)) {
                bool seen = false;
                for (auto *o : out)
                    seen = seen || o == &s;
                if (!seen)
                    out.push_back(&s);
                break;
            }
    return out;
}

// The sets a box's fuel goes into: every set whose mode mask holds the box's setup mode stat id, whatever season
// and weekend format it carries (a session in that mode reads one of them, and the season the query carries can
// change between the box press and the race), then the sets the session's own query picks when it is known.
inline std::vector<const fuel::Set *> box_sets(const std::vector<fuel::Set> &sets, uint32_t mode, const Query *q)
{
    std::vector<const fuel::Set *> out;
    auto add = [&out](const fuel::Set *s) {
        for (auto *o : out)
            if (o == s)
                return;
        out.push_back(s);
    };
    for (auto &s : sets)
        if (mode < 32 && ((s.modes >> mode) & 1u))
            add(&s);
    if (q)
        for (auto *s : query_sets(sets, *q))
            add(s);
    return out;
}

inline std::string set_masks(const std::vector<const fuel::Set *> &sets)
{
    std::string out;
    char b[64];
    for (auto *s : sets) {
        std::snprintf(b, sizeof b, "%smodes %#x seasons %#x formats %#x", out.empty() ? "" : "; ", s->modes, s->seasons,
                      s->formats);
        out += b;
    }
    return out;
}

// Whether an addon set a track.data record's fuel.burn_per_lap: the record is an addon's own (its owner isn't the
// game), or an addon's layer or variant left an origin on the field or on a field above it. A seeded record no addon
// changed is the game's and has none, and must not overwrite the live value.
inline bool burn_from_addon(const std::map<std::string, std::string> &origins, const std::string &owner = "game")
{
    if (!owner.empty() && owner != "game")
        return true;
    for (auto &[p, addon] : origins)
        if (!addon.empty() && (p.empty() || p == "/fuel" || p == "/fuel/burn_per_lap"))
            return true;
    return false;
}

inline void plan_burn(std::vector<FuelWrite> &out, const fuel::Set &set, const std::string &code, float burn,
                      const std::string &entry)
{
    const fuel::Track *t = fuel::find(set, code);
    if (!t || t->fuel.count < 1 || t->fuel.count > fuel::kMaxRows)
        return;
    fuel::Fuel after = fuel::with_burn(t->fuel, burn);
    if (fuel::same(after, t->fuel))
        return;
    out.push_back({t->object, code, entry, t->fuel, after});
}

// A box's pit speed limit (track.per_package pit.speed_limit, km/h): TrackDataTrack +0x21c, filled by the track data
// reader (0x142e47817) and read by Game_PitLane_GenerateSplines (0x1434ca550: [[mgr+0xb30]+0x1d0]+0x21c, km/h to m/s
// into mgr+0xba4) when the session builds its pit lane. Written at the box press into the box's sets like the fuel and
// at the session load into the sets the query picks, put back when the box is left. read(address, value) gives the
// current float.
constexpr uintptr_t kPitSpeed = 0x21c;

struct PitWrite {
    uintptr_t object = 0;
    std::string track, entry;
    float before = 0, after = 0;
};

template <class Read>
inline std::vector<PitWrite> plan_box_pit(const Base &base, const Entries &entries, const std::vector<const fuel::Set *> &sets,
                                          std::string_view session_code, Read read)
{
    std::vector<PitWrite> out;
    for (auto &[id, e] : entries.by_track) {
        const BaseTrack *t = base.by_id(id);
        if (!e.pit_speed || !t || (!session_code.empty() && t->code != session_code))
            continue;
        for (auto *s : sets) {
            const fuel::Track *ft = fuel::find(*s, t->code);
            float now = 0;
            if (!ft || !ft->object || !read(ft->object + kPitSpeed, now) || !(now > 0 && now <= 400))
                continue;   // not a pit speed limit as the game's tracks hold it (60..80): left alone
            bool seen = false;
            for (auto &w : out)
                seen = seen || w.object == ft->object;
            if (seen || now == *e.pit_speed)
                continue;
            out.push_back({ft->object, t->code, e.id, now, *e.pit_speed});
        }
    }
    return out;
}

// A box's race laps (track.per_package laps, at full race length). TrackDataTrack +0xe0 holds the Gdb value slot of
// the TrackData row's number_of_laps (writer 0x142e46e8c), not the value: every reader takes
// laps = ((int32 *)[[slot]+0x10])[race] at use (race 0, 1 for session type 0x10, 2 for 0x11; 0x141782564,
// 0x1436966xx) and scales it by the chosen race length option (0x140af68b0), so the box's laps are written into the
// value record's int list (begin +0x10, end +0x18, filled by the list parser 0x1445a4c80) and the race length option
// scales from them. Only sets of the default weekend format are written (a sprint set's list holds the sprint's laps),
// only the list's entries that hold the race's laps (entry 0's value: stock lists are "57,57,57"), each address once
// (a set without its own number_of_laps shares its parent set's record). Put back when the box is left.
constexpr uintptr_t kLapsSlot = 0xe0;
constexpr uintptr_t kListBegin = 0x10, kListEnd = 0x18;
constexpr size_t kLapsMax = 8;

struct LapsWrite {
    uintptr_t at = 0;   // the int32 in the number_of_laps list
    std::string track, entry;
    int32_t before = 0, after = 0;
};

// read_ptr(address, value) and read_int(address, value) give the current pointer / int32, false when unreadable.
template <class ReadPtr, class ReadInt>
inline std::vector<LapsWrite> plan_box_laps(const Base &base, const Entries &entries, const std::vector<const fuel::Set *> &sets,
                                            std::string_view session_code, ReadPtr read_ptr, ReadInt read_int)
{
    std::vector<LapsWrite> out;
    auto seen = [&out](uintptr_t at) {
        for (auto &w : out)
            if (w.at == at)
                return true;
        return false;
    };
    for (auto &[id, e] : entries.by_track) {
        const BaseTrack *t = base.by_id(id);
        if (!e.laps || !t || (!session_code.empty() && t->code != session_code))
            continue;
        for (auto *s : sets) {
            if (!((s->formats >> fuel::kFormatDefault) & 1u) || ((s->formats >> fuel::kFormatSprint) & 1u))
                continue;
            const fuel::Track *ft = fuel::find(*s, t->code);
            uintptr_t slot = 0, rec = 0, begin = 0, end = 0;
            if (!ft || !ft->object || !read_ptr(ft->object + kLapsSlot, slot) || !slot || !read_ptr(slot, rec) || !rec ||
                !read_ptr(rec + kListBegin, begin) || !read_ptr(rec + kListEnd, end) || !begin || end <= begin ||
                (end - begin) % 4 || (end - begin) / 4 > kLapsMax)
                continue;
            int32_t race = 0;
            if (!read_int(begin, race) || race < 1 || race > 999)
                continue;   // not a lap count as the game's tracks hold it: left alone
            for (uintptr_t at = begin; at < end; at += 4) {
                int32_t now = 0;
                if (!read_int(at, now) || now != race || now == *e.laps || seen(at))
                    continue;
                out.push_back({at, t->code, e.id, now, *e.laps});
            }
        }
    }
    return out;
}

inline std::vector<FuelWrite> plan_box_fuel(const Base &base, const Entries &entries, const std::vector<const fuel::Set *> &sets,
                                            std::string_view session_code)
{
    std::vector<FuelWrite> out;
    for (auto &[id, e] : entries.by_track) {
        const BaseTrack *t = base.by_id(id);
        if (!e.burn || !t || (!session_code.empty() && t->code != session_code))
            continue;
        if (!(*e.burn > 0.0f && *e.burn < 20.0f))
            continue;
        for (auto *s : sets)
            plan_burn(out, *s, t->code, *e.burn, e.id);
    }
    return out;
}

}
