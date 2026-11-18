#pragma once

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <string>
#include <unordered_set>
#include <vector>

namespace runtime::track_list_guard {

constexpr uintptr_t ENTRY = 0x28;
constexpr uintptr_t CALENDAR_BEGIN = 0x2b0;
constexpr uintptr_t CALENDAR_END = 0x2b8;
constexpr uintptr_t SEASON_EVENTS = 0x2c8;
constexpr size_t FALLBACK_LIMIT = 64;
constexpr uint32_t SEASON_MAX = 255;

constexpr uintptr_t SEASONS_BEGIN = 0x410;
constexpr uintptr_t SEASONS_END = 0x418;
constexpr uintptr_t SEASON_INDEX = 0x428;
constexpr size_t SEASON_RECORD = 0x80;
constexpr uintptr_t DRIVER_CALENDARS_BEGIN = 0x40;
constexpr uintptr_t DRIVER_CALENDARS_END = 0x48;
constexpr size_t DRIVER_CALENDAR = 0xc8;
constexpr uintptr_t ACTIVITIES_BEGIN = 0x50;
constexpr uintptr_t ACTIVITIES_END = 0x58;
constexpr size_t ACTIVITY_SLOT = 0x10;
constexpr uintptr_t SLOT_ACTIVITY = 0x8;
constexpr uintptr_t ACTIVITY_DEF = 0x10;
constexpr uintptr_t DEF_TYPE = 0x10;
constexpr uint8_t RACE_WEEKEND = 0x0a;
constexpr size_t MAX_SEASONS = 64;
constexpr size_t MAX_CALENDARS = 256;
constexpr size_t MAX_ACTIVITIES = 4096;

struct Load {
    uintptr_t component = 0;
    uint64_t entry = UINT64_MAX;
    int prev_id = -1;
    size_t stored = 0, season = 0, weekends = 0, limit = 0;
    bool blown = false;
    size_t kept = 0, dropped = 0, reused = 0, extra = 0;
    std::unordered_set<uintptr_t> rows;
};

inline size_t entries(uintptr_t begin, uintptr_t end)
{
    return end > begin ? (end - begin) / ENTRY : 0;
}

inline size_t season_events(uint32_t raw)
{
    return raw >= 1 && raw <= SEASON_MAX ? raw : 0;
}

template <class Read> size_t race_weekends(uintptr_t component, Read &&read)
{
    uint64_t sb = 0, se = 0;
    uint32_t index = 0;
    if (!read(component + SEASONS_BEGIN, sb) || !read(component + SEASONS_END, se) || !read(component + SEASON_INDEX, index))
        return 0;
    if (!sb || se <= sb || (se - sb) % SEASON_RECORD || (se - sb) / SEASON_RECORD > MAX_SEASONS || index >= (se - sb) / SEASON_RECORD)
        return 0;
    uintptr_t season = uintptr_t(sb) + index * SEASON_RECORD;
    uint64_t cb = 0, ce = 0;
    if (!read(season + DRIVER_CALENDARS_BEGIN, cb) || !read(season + DRIVER_CALENDARS_END, ce))
        return 0;
    if (!cb || ce <= cb || (ce - cb) % DRIVER_CALENDAR || (ce - cb) / DRIVER_CALENDAR > MAX_CALENDARS)
        return 0;
    size_t best = 0;
    for (uintptr_t calendar = uintptr_t(cb); calendar < ce; calendar += DRIVER_CALENDAR) {
        uint64_t ab = 0, ae = 0;
        if (!read(calendar + ACTIVITIES_BEGIN, ab) || !read(calendar + ACTIVITIES_END, ae))
            continue;
        if (!ab || ae <= ab || (ae - ab) % ACTIVITY_SLOT || (ae - ab) / ACTIVITY_SLOT > MAX_ACTIVITIES)
            continue;
        size_t count = 0;
        for (uintptr_t slot = uintptr_t(ab); slot < ae; slot += ACTIVITY_SLOT) {
            uint64_t activity = 0, def = 0;
            uint8_t type = 0;
            if (read(slot + SLOT_ACTIVITY, activity) && activity && read(uintptr_t(activity) + ACTIVITY_DEF, def) && def &&
                read(uintptr_t(def) + DEF_TYPE, type) && type == RACE_WEEKEND)
                ++count;
        }
        best = std::max(best, count);
    }
    return best <= FALLBACK_LIMIT ? best : 0;
}

inline size_t limit_for(size_t season, size_t weekends = 0)
{
    size_t limit = std::max(season, weekends);
    return limit ? limit : FALLBACK_LIMIT;
}

inline void reset(Load &load, uintptr_t component, size_t stored, size_t season, size_t weekends = 0)
{
    load = Load{};
    load.component = component;
    load.stored = stored;
    load.season = season;
    load.weekends = weekends;
    load.limit = limit_for(season, weekends);
    load.blown = stored > load.limit;
}

inline bool fresh(const Load &load, uintptr_t component, size_t calendar)
{
    return calendar == 0 || component != load.component;
}

inline bool enter(Load &load, uint64_t entry, uint8_t id)
{
    if (entry == load.entry)
        return false;
    load.entry = entry;
    bool run = load.prev_id == id;
    load.prev_id = id;
    if (load.blown && (run || load.kept >= load.limit)) {
        ++load.dropped;
        return false;
    }
    return true;
}

inline uintptr_t choose(Load &load, const std::vector<uintptr_t> &matches)
{
    if (matches.empty())
        return 0;
    uintptr_t pick = 0;
    for (uintptr_t row : matches)
        if (!load.rows.count(row)) {
            pick = row;
            break;
        }
    if (!pick) {
        pick = matches.front();
        ++load.reused;
    }
    load.rows.insert(pick);
    load.extra += matches.size() - 1;
    ++load.kept;
    return pick;
}

inline bool last_entry(uint64_t index, uint64_t stored)
{
    return index + 1 >= stored;
}

inline Load finish(Load &load, uintptr_t component)
{
    Load done = load.component == component ? load : Load{};
    load = Load{};
    return done;
}

inline std::string load_line(uintptr_t component, size_t stored, size_t loaded, const Load *guard)
{
    char buf[400];
    snprintf(buf, sizeof buf, "[saves] custom_track_list loaded (schedule component 0x%llx): %zu stored entr%s -> %zu calendar event%s",
             (unsigned long long)component, stored, stored == 1 ? "y" : "ies", loaded, loaded == 1 ? "" : "s");
    std::string s = buf;
    if (!guard)
        return s;
    if (guard->season && guard->weekends)
        snprintf(buf, sizeof buf, " (season event list %zu, %zu race weekend%s on the career calendar)", guard->season, guard->weekends,
                 guard->weekends == 1 ? "" : "s");
    else if (guard->season)
        snprintf(buf, sizeof buf, " (season event list %zu)", guard->season);
    else if (guard->weekends)
        snprintf(buf, sizeof buf, " (season event list unknown, %zu race weekend%s on the career calendar)", guard->weekends,
                 guard->weekends == 1 ? "" : "s");
    else
        snprintf(buf, sizeof buf, " (season event list unknown, limit %zu)", guard->limit);
    s += buf;
    if (!guard->blown && guard->season && guard->stored > guard->season) {
        snprintf(buf, sizeof buf, "; more entries than the season event list, all kept: the career calendar races %zu weekend%s",
                 guard->weekends, guard->weekends == 1 ? "" : "s");
        s += buf;
    }
    if (guard->extra) {
        snprintf(buf, sizeof buf, "; %zu more matching event%s the game would also have loaded skipped", guard->extra,
                 guard->extra == 1 ? "" : "s");
        s += buf;
    }
    if (guard->reused) {
        snprintf(buf, sizeof buf, "; %zu entr%s repeat an event already in the calendar", guard->reused, guard->reused == 1 ? "y" : "ies");
        s += buf;
    }
    if (guard->blown) {
        snprintf(buf, sizeof buf, "; blown up (more than %zu entries): a run of one id reads as one entry, %zu stored entr%s dropped",
                 guard->limit, guard->dropped, guard->dropped == 1 ? "y" : "ies");
        s += buf;
    }
    if (!guard->extra && !guard->reused && !guard->blown)
        s += ", no repeats";
    return s;
}

inline std::string save_line(uintptr_t component, size_t count)
{
    char buf[200];
    snprintf(buf, sizeof buf, "[saves] custom_track_list saved (schedule component 0x%llx): %zu calendar event%s",
             (unsigned long long)component, count, count == 1 ? "" : "s");
    return buf;
}

struct SaveLog {
    uintptr_t component[8] = {};
    size_t count[8] = {};
    size_t next = 0;

    bool changed(uintptr_t comp, size_t n)
    {
        for (size_t i = 0; i < 8; i++)
            if (component[i] == comp) {
                if (count[i] == n)
                    return false;
                count[i] = n;
                return true;
            }
        component[next] = comp;
        count[next] = n;
        next = (next + 1) % 8;
        return true;
    }
};

inline bool switch_off(const char *value)
{
    if (!value)
        return false;
    return (value[0] == '0' && value[1] == 0) || (value[0] == 'o' && value[1] == 'f' && value[2] == 'f' && value[3] == 0);
}

}
