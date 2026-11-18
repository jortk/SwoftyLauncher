#pragma once
#include <cstdint>
#include <cstdio>
#include <string>

#include "career_saves_logic.hpp"

namespace runtime::save_diag {

inline std::string utc(int64_t secs)
{
    if (secs <= 0)
        return "unknown";
    int64_t days = secs / 86400, rem = secs % 86400;
    days += 719468;
    int64_t era = (days >= 0 ? days : days - 146096) / 146097;
    int64_t doe = days - era * 146097;
    int64_t yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    int64_t y = yoe + era * 400;
    int64_t doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    int64_t mp = (5 * doy + 2) / 153;
    int64_t d = doy - (153 * mp + 2) / 5 + 1;
    int64_t m = mp < 10 ? mp + 3 : mp - 9;
    if (m <= 2)
        ++y;
    char b[48];
    snprintf(b, sizeof b, "%04lld-%02lld-%02lld %02lld:%02lld:%02lld UTC", (long long)y, (long long)m, (long long)d,
             (long long)(rem / 3600), (long long)(rem % 3600 / 60), (long long)(rem % 60));
    return b;
}

inline std::string off_line(bool present, const std::string &profile, const std::string &why)
{
    std::string head = present && !profile.empty() ? "launcher saves are off for profile " + profile
                       : present                  ? std::string("launcher saves are off")
                                                  : std::string("launcher saves are off (settings.json has no launcher_saves)");
    std::string out = head + ": the game reads Steam's saves natively";
    if (!why.empty())
        out += " (" + why + ")";
    return out;
}

inline std::string file_line(const std::string &name, int64_t bytes, int64_t mtime)
{
    return "save file " + name + ": " + std::to_string(bytes) + " bytes, modified " + utc(mtime);
}

inline const char *mode_of(const career_saves::SlotRead &s)
{
    return s.flags & career_saves::FLAG_MY_TEAM ? "gm_my_team" : "gm_driver_career";
}

inline std::string slot_line(unsigned load, size_t manager, size_t index, const career_saves::SlotRead &s,
                             const std::string &career)
{
    char b[512];
    std::string key = career_saves::slot_key(s.id, s.guid);
    snprintf(b, sizeof b, "profile load %u: manager %zu slot %zu: %s%s id=%llu key=%s career=%s flags=0x%02x", load, manager,
             index, s.details ? mode_of(s) : "details-unreadable", career_saves::career_slot(s) ? "" : " (not a career slot)",
             (unsigned long long)s.id, key.c_str(), career.empty() ? "?" : career.c_str(), s.flags);
    std::string out = b;
    if (s.details) {
        snprintf(b, sizeof b, " setup=%u season=%d weekend=%d date=%llu", s.setup_mode,
                 s.season == career_saves::NO_INDEX ? -1 : int(s.season) + 1,
                 s.weekend == career_saves::NO_INDEX ? -1 : int(s.weekend), (unsigned long long)s.date);
        out += b;
    }
    if (!s.name.empty())
        out += " name=\"" + s.name + "\"";
    return out;
}

inline std::string manager_line(unsigned load, size_t manager, size_t slots, uintptr_t vtable_rva)
{
    char b[160];
    snprintf(b, sizeof b, "profile load %u: manager %zu (vtable +0x%llx): %zu slot(s)", load, manager,
             (unsigned long long)vtable_rva, slots);
    return b;
}

inline std::string kept_line(unsigned load, const std::string &name, size_t bytes)
{
    return "profile load " + std::to_string(load) + ": container " + name + ": " + std::to_string(bytes) +
           " bytes (no loaded game mode claims it)";
}

}
