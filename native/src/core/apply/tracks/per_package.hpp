#pragma once

#include "base.hpp"

#include <swofty/json.hpp>

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <utility>
#include <vector>

// One box's track.per_package entries (TRACKS-CONTRACTS.md C1): ids <package id>/<key>, and the records of any
// addon whose package field names the package. An entry's target is its track when set, else its venue resolved
// through the base, else the base track whose code ends the id. A variant package uses its base package's entries;
// its own entry for the same target wins. Pure.

namespace swofty_core::apply::tracks {

struct Entry {
    std::string id, track;
    bool offered = true;
    int order = 0;
    std::optional<float> burn;
    std::optional<float> pit_speed;   // pit.speed_limit, km/h (TrackDataTrack +0x21c)
    std::optional<int32_t> laps;      // laps: the race's laps at full race length (TrackDataTrack +0xe0 number_of_laps)
};

// Race laps the core writes: a whole number from 1 to 999 (0 or anything else leaves the track's own).
inline std::optional<int32_t> laps_of(const swofty::json::Value &v)
{
    if (!v.is_int() || v.as_int() < 1 || v.as_int() > 999)
        return std::nullopt;
    return int32_t(v.as_int());
}

// A pit speed limit the core writes: above 0 and at most 400 km/h.
inline std::optional<float> pit_speed_of(const swofty::json::Value &pit)
{
    if (!pit.is_object() || !pit["speed_limit"].is_number())
        return std::nullopt;
    double v = pit["speed_limit"].as_double();
    return v > 0 && v <= 400 ? std::optional<float>(float(v)) : std::nullopt;
}

struct Entries {
    std::map<std::string, Entry> by_track;
    std::vector<std::string> lines;

    const Entry *of(const std::string &track_id) const
    {
        auto it = by_track.find(track_id);
        return it == by_track.end() ? nullptr : &it->second;
    }
};

inline std::string entry_key(const std::string &id)
{
    auto slash = id.rfind('/');
    return slash == std::string::npos ? id : id.substr(slash + 1);
}

// The record ids of one package's entries: every id under "<package>/" with no further slash after the key.
inline bool entry_of_package(const std::string &id, const std::string &package)
{
    return id.size() > package.size() + 1 && id.compare(0, package.size(), package) == 0 && id[package.size()] == '/' &&
           id.find('/', package.size() + 1) == std::string::npos;
}

// An entry of the package: its id is under "<package>/", or its package field names the package.
inline bool entry_for(const std::string &id, const swofty::json::Value &rec, const std::string &package)
{
    return entry_of_package(id, package) || (rec["package"].is_string() && rec["package"].as_string() == package);
}

// The records of a box's entries, from every track.per_package record in load order and the box's chain (the box
// first, then its bases): the bases' entries first and the box's last, so the box's own win.
inline std::vector<std::pair<std::string, swofty::json::Value>>
chain_records(const std::vector<std::pair<std::string, swofty::json::Value>> &all, const std::vector<std::string> &chain)
{
    std::vector<std::pair<std::string, swofty::json::Value>> out;
    for (auto it = chain.rbegin(); it != chain.rend(); ++it)
        for (auto &[id, rec] : all)
            if (entry_for(id, rec, *it))
                out.push_back({id, rec});
    return out;
}

inline const BaseTrack *entry_target(const Base &base, const std::string &id, const swofty::json::Value &rec, std::string &ref)
{
    if (rec["track"].is_string()) {
        ref = std::string(rec["track"].as_string());
        return base.resolve(ref);
    }
    if (rec["venue"].is_string()) {
        ref = std::string(rec["venue"].as_string());
        return base.resolve(ref);
    }
    ref = entry_key(id);
    for (const BaseTrack &t : base.tracks)
        if (t.code == ref)
            return &t;
    return nullptr;
}

// records: each package's entries as (id, effective value), the box's base packages first and the box last.
inline Entries resolve_entries(const Base &base, const std::vector<std::pair<std::string, swofty::json::Value>> &records)
{
    Entries out;
    for (auto &[id, rec] : records) {
        std::string ref;
        const BaseTrack *t = entry_target(base, id, rec, ref);
        if (!t) {
            bool optional = !ref.empty() && ref.back() == '?';
            out.lines.push_back("[tracks] " + id + ": no track for " + ref + (optional ? " (optional)" : "") +
                                "; nothing applied");
            continue;
        }
        Entry e;
        e.id = id;
        e.track = t->id;
        e.offered = rec["offered"].as_bool(true);
        e.order = rec["order"].is_int() ? int(rec["order"].as_int()) : 0;
        const swofty::json::Value &fuel = rec["fuel"];
        if (fuel.is_object() && fuel["burn_per_lap"].is_number())
            e.burn = float(fuel["burn_per_lap"].as_double());
        e.pit_speed = pit_speed_of(rec["pit"]);
        e.laps = laps_of(rec["laps"]);
        out.by_track[t->id] = std::move(e);
    }
    return out;
}

}
