// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/forward.hpp>

// Tracks: the operations on the game's track data and the session.fuel.load hook point (1.6.0), and the four
// tracks registries (1.6.5, experimental): tracks (track; stock rows are game/<code>), venues (track.venue;
// core-owned shared ids venue/<key>, resolved to the last enabled track in load order that provides one), data
// (track.data; id <track id>/<data set>; an added track's blocks are variants of its base's) and per_package
// (track.per_package; id <package id>/<key>; one box's data for one track or venue). track.layer, track.slots
// and track.ai_line are not in this release: PackageTrack::world is stored and not applied.
// set_track_fuel and copy_track_fuel call the core's career operations (SwoftyCoreCareerOps version 4) and
// return unavailable on a core without them. The core keeps the bytes it found and puts them back when the
// career unloads, or on restore_track_fuel. Data sets: f1_2025, sprint_f1_2025, f1_2026, sprint_f1_2026.

namespace swofty::tracks {

using Result = swofty::career::Result;

struct Fuel {
    float burn_per_lap = 0;
};
SWOFTY_RECORD(Fuel, burn_per_lap)

struct Venue {
    static constexpr Version version{1, 0};
    std::string name;
    std::string country;
    Ref<Track> track;
    json::Value extra;
};
SWOFTY_RECORD(Venue, name, country, track)

struct TrackNames {
    Text title;
    Text gp;
    std::string acronym;
    Text city;
};
SWOFTY_RECORD(TrackNames, title, gp, acronym, city)

struct Tyres {
    std::string softer, medium, harder;
};
SWOFTY_RECORD(Tyres, softer, medium, harder)

struct Pit {
    int speed_limit = 0;
    float stop_time_dry = 0, stop_time_wet = 0;
};
SWOFTY_RECORD(Pit, speed_limit, stop_time_dry, stop_time_wet)

struct Date {
    int day = 0, month = 0;
};
SWOFTY_RECORD(Date, day, month)

struct Track {
    static constexpr Version version{1, 1};
    std::string code;
    std::string name;
    Ref<Track> base;
    Ref<Track> slot;
    std::vector<Ref<Venue>> provides;
    TrackNames names;
    Path files;
    float length_metres = 0;
    int corners = 0;
    float latitude = 0, longitude = 0;
    bool time_trial = false;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(Track, code, name, base, slot, provides, names, files, length_metres, corners, latitude, longitude,
              time_trial, stat_id)

struct TrackData {
    static constexpr Version version{1, 0};
    Ref<Track> track;
    std::string data_set;
    int laps = 0;
    Fuel fuel;
    Tyres tyres;
    Ref<tyres::Class> tyre_class;
    Pit pit;
    Ref<cars::Vehicle> safety_car;
    TrackNames names;
    Date date;
    json::Value extra;
};
SWOFTY_RECORD(TrackData, track, data_set, laps, fuel, tyres, tyre_class, pit, safety_car, names, date)
SWOFTY_RECORD_REGISTRY(TrackData, "track.data")

struct PackageTrack {
    static constexpr Version version{1, 0};
    Ref<content::Package> package;
    Ref<Track> track;
    Ref<Venue> venue;
    bool offered = true;
    int order = 0;
    Ref<Layer> world;
    std::optional<int> laps;
    std::optional<Fuel> fuel;
    Ref<tyres::Class> tyre_class;
    std::optional<Pit> pit;
    std::optional<TrackNames> names;
    std::optional<Date> date;
    json::Value extra;
};
SWOFTY_RECORD(PackageTrack, package, track, venue, offered, order, world, laps, fuel, tyre_class, pit, names, date)
SWOFTY_RECORD_REGISTRY(PackageTrack, "track.per_package")

// What a box offers in one mode, as the core's catalog.json and [tracks] log lines report it: the season's real
// calendar in round order, then every other track that fits ("More tracks"), and the tracks left out with why.
struct LeftOut {
    Ref<Track> track;
    std::string reason;
};
SWOFTY_RECORD(LeftOut, track, reason)

struct TrackList {
    std::vector<Ref<Track>> calendar;
    std::vector<Ref<Track>> more;
    std::vector<LeftOut> left_out;
};
SWOFTY_RECORD(TrackList, calendar, more, left_out)

inline Registry<Track> tracks{"track", {.applies = Applies::boot}};
inline Registry<Venue> venues{"track.venue",
                              {.applies = Applies::boot,
                               .shared = {.provider_registry = "track", .provides = "/provides", .resolved = "/track"}}};
inline Registry<TrackData> data{"track.data", {.applies = Applies::session}};
inline Registry<PackageTrack> per_package{"track.per_package", {.applies = Applies::content}};

namespace detail {

inline std::string code_of(const Ref<Track> &track)
{
    std::string id(track.id());
    size_t slash = id.find('/');
    return slash == std::string::npos ? id : id.substr(slash + 1);
}

inline std::string text(std::string_view s)
{
    std::string out = "\"";
    for (char c : s)
        if (c != '"' && c != '\\' && static_cast<unsigned char>(c) >= 0x20)
            out += c;
    return out + "\"";
}

inline Result fuel_request(const std::string &request)
{
    auto t = swofty::career::detail::live_ops();
    if (!t || t->version < 4 || !t->set_track_fuel)
        return Result::unavailable;
    char out[2048];
    size_t len = 0;
    int r = t->set_track_fuel({request.data(), request.size()}, out, sizeof out, &len);
    return r == SWOFTY_TOO_SMALL ? Result::ok : swofty::career::detail::ops_result(r);
}

}

// The track's burn per lap in one data set (game/melbourne: the track code after the slash).
inline Result set_track_fuel(Ref<Track> track, const Fuel &fuel, std::string_view data_set = "f1_2026")
{
    return detail::fuel_request("{\"track\":" + detail::text(detail::code_of(track)) + ",\"set\":" + detail::text(data_set) +
                                ",\"burn_per_lap\":" + std::to_string(fuel.burn_per_lap) + "}");
}

// The track's fuel in data_set (burn per lap, race fuel rows, qualifying fuel) from the same track in from;
// track "*" copies every track.
inline Result copy_track_fuel(std::string_view track, std::string_view data_set, std::string_view from, std::string_view why = {})
{
    return detail::fuel_request("{\"track\":" + detail::text(track) + ",\"set\":" + detail::text(data_set) +
                                ",\"from\":" + detail::text(from) + ",\"why\":" + detail::text(why) + "}");
}

inline Result restore_track_fuel(std::string_view why = {})
{
    return detail::fuel_request("{\"restore\":true,\"why\":" + detail::text(why) + "}");
}

}

namespace swofty::tracks::events {

struct FuelLoad {
    static constexpr const char *point = "session.fuel.load";
    static constexpr uint32_t version = 1;
    enum Kind : uint8_t { practice = 0, qualifying = 1, race = 2, other = 3 };
    uint32_t size, payload_version;
    const char *track_id;
    uint32_t session_type;
    float kg;
    uint8_t kind;

    Ref<Track> track() const { return swofty::detail::payload_ref<Track>(track_id); }
};

}
