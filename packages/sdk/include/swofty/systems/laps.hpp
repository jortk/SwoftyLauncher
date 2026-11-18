// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <string_view>
#include <swofty/events.hpp>

// Laps: the lap recorder (a lap module, swofty_api.h SwoftyLapHost) writes one .swlap file per completed Time Trial
// lap while the launcher has Time Trial boards on. laps.sealing runs just before the file is renamed into place and lets a handler append a
// section; laps.recorded fires once the file is in place.

namespace swofty::laps::events {

// A lap file is written (notify). path is the file's full path, time_ms the lap time and valid 1, 0 or -1 when the
// lap's validity is not known.
struct Recorded {
    static constexpr const char *point = "laps.recorded";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    const char *path_str;
    int32_t time_ms;
    int32_t valid;

    std::string_view path() const { return swofty::detail::payload_str(path_str); }
};

// A lap file is about to be renamed into place (modify). body holds the file as built so far, which is what a
// handler seals; add_section appends one section after it. A tag that is the format's own (SMPL, GHST, END) is
// refused.
struct Sealing {
    static constexpr const char *point = "laps.sealing";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    const char *path_str;
    int32_t time_ms;
    const uint8_t *body;
    uint32_t body_len;
    int32_t (*add_section_fn)(void *user, const char *tag, const void *bytes, uint32_t len);
    void *user;

    std::string_view path() const { return swofty::detail::payload_str(path_str); }
    Result add_section(const char (&tag)[5], const void *bytes, uint32_t len)
    {
        if (!add_section_fn)
            return Result::unavailable;
        return swofty::detail::result(add_section_fn(user, tag, bytes, len));
    }
};

}
