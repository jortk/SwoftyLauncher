// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <string>
#include <string_view>
#include <swofty/mixin.hpp>

// Typed read access to live car and session state, and an optional shared-memory feed for
// external tools. Reads reflect the most recent frame and return unavailable outside a running
// session. The shared feed is a named mapping with a fixed, versioned layout (SwoftyTelemetryFeed)
// guarded by a seqlock: read while seq is even and unchanged across the copy. Car, participant and
// most session fields are decoded from the game's own UDP telemetry packets, so they fill only while
// the game's UDP telemetry output is on; session.udp_format is non-zero once a packet has arrived.

namespace swofty::telemetry {

enum class Result { ok, late, unavailable, invalid };

using CarState = SwoftyCarState;
using SessionState = SwoftySessionState;
using Participant = SwoftyParticipant;
using Feed = SwoftyTelemetryFeed;

constexpr uint32_t version = SWOFTY_TELEMETRY_VERSION;

// Events fired on the flow thread (swofty::events::subscribe): a = overtaking car, b = overtaken car;
// a and b = the two cars; a = car, b = retirement reason.
namespace event {
inline constexpr const char *overtake = SWOFTY_EVENT_TELEMETRY_OVERTAKE;
inline constexpr const char *collision = SWOFTY_EVENT_TELEMETRY_COLLISION;
inline constexpr const char *retirement = SWOFTY_EVENT_TELEMETRY_RETIREMENT;
}

namespace detail {

inline const SwoftyTelemetry *table()
{
    return api && api->version >= 5 ? api->telemetry : nullptr;
}

inline Result result(int r)
{
    return r == SWOFTY_ASSET_OK ? Result::ok
           : r == SWOFTY_ASSET_LATE ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
                                           : Result::invalid;
}

}

// The telemetry library is usable (feature on, implemented, and a session is live).
inline bool available()
{
    auto t = detail::table();
    return t && t->available();
}

// Fill the session state for the current frame.
inline Result session(SessionState *out)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->session(out));
}

// Fill one car's state. index is the participant index; the player's index is in the session.
inline Result car(uint32_t index, CarState *out)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->car(index, out));
}

// Fill one participant's entry (name, team, position).
inline Result participant(uint32_t index, Participant *out)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->participant(index, out));
}

// Turn on the shared-memory feed and return the name of the mapping. The name stays valid for
// the lifetime of the process.
inline Result shared_memory(const char **name)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->shared_memory(name));
}

inline const char *name(Result r)
{
    return r == Result::ok ? "ok" : r == Result::late ? "late" : r == Result::unavailable ? "unavailable" : "invalid";
}

}
