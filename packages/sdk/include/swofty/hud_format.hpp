// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <string>
#include <string_view>

// Text for HUD leaves: speed, gaps, lap times, session labels, driver codes. Header only and pure.
// The gap and lap formats follow Project Halo's tower (tower_model.cpp). Uses work from Project Halo by
// nyannoying, https://github.com/nyannoying1337/f1-25-project-halo

namespace swofty::hud::format {

struct Speed {
    int kph, mph;
};

// One source for both units, rounded. Negative speeds (reversing) read 0.
inline Speed speed(float mps)
{
    float v = std::max(0.f, mps);
    return {int(std::lround(v * 3.6f)), int(std::lround(v * 2.2369363f))};
}

inline std::string speed_kph(float mps) { return std::to_string(speed(mps).kph); }
inline std::string speed_mph(float mps) { return std::to_string(speed(mps).mph); }

enum class GapStyle { cut_tenths, round_tenths, thousandths };

// "+5.1", "+1:05.3". cut_tenths drops the hundredths like a timing screen; round_tenths rounds;
// thousandths prints "+5.123". Returns the length written (snprintf semantics, cap includes the NUL).
inline size_t gap(int64_t ms, char *out, size_t cap, GapStyle style = GapStyle::cut_tenths)
{
    if (ms < 0)
        ms = 0;
    int n;
    if (style == GapStyle::thousandths) {
        long long s = ms / 1000;
        if (s < 60)
            n = std::snprintf(out, cap, "+%lld.%03lld", s, (long long)(ms % 1000));
        else
            n = std::snprintf(out, cap, "+%lld:%02lld.%03lld", s / 60, s % 60, (long long)(ms % 1000));
    } else {
        long long tenths = style == GapStyle::cut_tenths ? ms / 100 : (ms + 50) / 100;
        if (tenths < 600)
            n = std::snprintf(out, cap, "+%lld.%lld", tenths / 10, tenths % 10);
        else
            n = std::snprintf(out, cap, "+%lld:%02lld.%lld", tenths / 600, (tenths % 600) / 10, tenths % 10);
    }
    return n < 0 ? 0 : size_t(n);
}

// "+1 LAP", "+3 LAPS". Nothing (and 0) for fewer than one lap.
inline size_t lapped(int laps, char *out, size_t cap)
{
    if (laps < 1) {
        if (cap)
            out[0] = 0;
        return 0;
    }
    int n = laps == 1 ? std::snprintf(out, cap, "+1 LAP") : std::snprintf(out, cap, "+%d LAPS", laps);
    return n < 0 ? 0 : size_t(n);
}

// "1:46.123". A lap time always shows the minute: "0:59.874".
inline size_t lap_time(int64_t ms, char *out, size_t cap)
{
    if (ms < 0) {
        if (cap)
            out[0] = 0;
        return 0;
    }
    int n = std::snprintf(out, cap, "%lld:%02lld.%03lld", (long long)(ms / 60000), (long long)((ms / 1000) % 60),
                          (long long)(ms % 1000));
    return n < 0 ? 0 : size_t(n);
}

// "28.412" (a sector over a minute prints as a lap time).
inline size_t sector_time(int64_t ms, char *out, size_t cap)
{
    if (ms < 0) {
        if (cap)
            out[0] = 0;
        return 0;
    }
    if (ms >= 60000)
        return lap_time(ms, out, cap);
    int n = std::snprintf(out, cap, "%lld.%03lld", (long long)(ms / 1000), (long long)(ms % 1000));
    return n < 0 ? 0 : size_t(n);
}

// The game's session type numbers: 1..4 practice, 5..9 qualifying, 10..14 sprint qualifying, 15..17 race, 18
// time trial.
inline const char *session_label(uint32_t type)
{
    static const char *const label[] = {"",    "P1",  "P2", "P3",  "P",   "Q1", "Q2", "Q3", "Q",  "OSQ",
                                        "SQ1", "SQ2", "SQ3", "SQ", "OSQ", "R",  "R2", "R3", "TT"};
    return type < sizeof label / sizeof label[0] ? label[type] : "";
}

inline bool is_practice(uint32_t t) { return t >= 1 && t <= 4; }
inline bool is_qualifying(uint32_t t) { return t >= 5 && t <= 14; }
inline bool is_race(uint32_t t) { return t >= 15 && t <= 17; }
inline bool is_timed(uint32_t t) { return t >= 1 && t <= 14; }
inline bool is_time_trial(uint32_t t) { return t == 18; }

// The longest prefix of s that fits cap bytes and ends on a UTF-8 character boundary.
inline std::string_view fit(std::string_view s, size_t cap)
{
    if (s.size() <= cap)
        return s;
    size_t n = cap;
    while (n > 0 && (static_cast<unsigned char>(s[n]) & 0xC0) == 0x80)
        n--;
    return s.substr(0, n);
}

// A three-letter driver code. Uses code when the game gave one, else the first three letters of the family
// name (the last word of name), upper-cased ASCII. out is NUL-terminated, 4 bytes.
inline void driver_code(std::string_view code, std::string_view name, char out[4])
{
    auto upper = [](std::string_view s, char *o) {
        size_t n = 0;
        for (char c : s) {
            if (n == 3)
                break;
            if (std::isalpha(static_cast<unsigned char>(c)))
                o[n++] = char(std::toupper(static_cast<unsigned char>(c)));
        }
        o[n] = 0;
        return n;
    };
    out[0] = 0;
    if (upper(code, out) == 3)
        return;
    size_t sp = name.find_last_of(' ');
    upper(sp == std::string_view::npos ? name : name.substr(sp + 1), out);
}

}
