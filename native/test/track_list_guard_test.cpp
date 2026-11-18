// Host-native unit test for the core's career calendar track list guard (native/src/core/track_list_guard_logic.hpp):
// the game's custom_track_list load loop is replayed with and without the guard over event rows that share ids, a
// track twice in one season, duplicate event rows, clean saves over several save and load cycles, a save the game
// already blew up, a season longer than the default event list, and the race weekends read from the career calendar.
// Built and run by native/test/run_track_list_guard_test.sh.

#include "track_list_guard_logic.hpp"

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace tl = runtime::track_list_guard;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

static uintptr_t addr(size_t row)
{
    return 0x10000 + row * tl::ENTRY;
}

static std::vector<size_t> load(const std::vector<uint8_t> &rows, const std::vector<uint8_t> &stored, tl::Load *guard, size_t season,
                                tl::Load *done = nullptr, uintptr_t component = 1, size_t weekends = 0)
{
    std::vector<size_t> calendar;
    std::vector<uintptr_t> matches;
    for (size_t e = 0; e < stored.size(); e++) {
        for (size_t r = 0; r < rows.size(); r++) {
            if (rows[r] != stored[e])
                continue;
            if (!guard) {
                calendar.push_back(r);
                continue;
            }
            if (tl::fresh(*guard, component, calendar.size()))
                tl::reset(*guard, component, stored.size(), season, weekends);
            if (!tl::enter(*guard, e, stored[e]))
                continue;
            matches.clear();
            for (size_t m = r; m < rows.size(); m++)
                if (rows[m] == stored[e])
                    matches.push_back(addr(m));
            r = (tl::choose(*guard, matches) - 0x10000) / tl::ENTRY;
            calendar.push_back(r);
        }
        if (guard && tl::last_entry(e, stored.size())) {
            tl::Load d = tl::finish(*guard, component);
            if (done)
                *done = d;
        }
    }
    return calendar;
}

static std::vector<uint8_t> save(const std::vector<uint8_t> &rows, const std::vector<size_t> &calendar)
{
    std::vector<uint8_t> out;
    for (size_t r : calendar)
        out.push_back(rows[r]);
    return out;
}

int main()
{
    std::vector<uint8_t> stock;
    for (uint8_t id = 0; id < 24; id++)
        stock.push_back(id);
    const std::vector<uint8_t> season24(stock);
    tl::Load guard;

    {
        std::vector<uint8_t> ids = season24;
        bool flat = true;
        for (int cycle = 0; cycle < 5; cycle++) {
            auto cal = load(stock, ids, &guard, 24);
            flat = flat && cal.size() == 24 && cal == load(stock, ids, nullptr, 24);
            ids = save(stock, cal);
        }
        expect(flat && ids == season24, "a clean 24-event save loads 24 events, as the game does, over 5 save and load cycles");
    }

    {
        std::vector<uint8_t> ids = {0, 5, 5, 9};
        auto game = load(stock, ids, nullptr, 4);
        tl::Load done;
        auto cal = load(stock, ids, &guard, 4, &done);
        expect(game.size() == 4 && cal == game, "a track twice in one season with one event row: 2 stored entries load 2 events, as the game does");
        expect(done.reused == 1 && !done.blown, "the second entry is logged as repeating an event already in the calendar");
        bool flat = true;
        for (int cycle = 0; cycle < 5; cycle++) {
            cal = load(stock, ids, &guard, 4);
            flat = flat && cal.size() == 4;
            ids = save(stock, cal);
        }
        expect(flat, "the double header keeps both races over 5 save and load cycles");
    }

    {
        std::vector<uint8_t> rows = stock;
        rows.insert(rows.end(), {0, 0, 0});
        auto game = load(rows, {0}, nullptr, 1);
        tl::Load done;
        auto cal = load(rows, {0}, &guard, 1, &done);
        expect(game.size() == 4, "3 duplicate rows of one id: the game alone loads 4 events for 1 stored entry");
        expect(cal.size() == 1 && cal[0] == 0, "3 duplicate rows of one id: 1 stored entry loads 1 event, the first row");
        expect(done.extra == 3, "the other matching rows are logged as skipped");
        std::vector<uint8_t> ids = save(rows, cal);
        for (int cycle = 0; cycle < 5; cycle++)
            ids = save(rows, load(rows, ids, &guard, 1));
        expect(ids.size() == 1, "3 duplicate rows of one id: no growth over 5 save and load cycles");
        auto twice = load(rows, {0, 0}, &guard, 2);
        expect(twice.size() == 2 && twice[0] != twice[1], "2 stored entries of an id with several rows load 2 different rows");
    }

    std::vector<uint8_t> seasons;
    for (int season = 0; season < 10; season++)
        seasons.insert(seasons.end(), stock.begin(), stock.end());

    {
        std::vector<uint8_t> ids = season24;
        for (int cycle = 0; cycle < 3; cycle++)
            ids = save(seasons, load(seasons, ids, nullptr, 24));
        expect(ids.size() == 24 * 1000, "10 seasons sharing ids: the game alone multiplies the calendar 10x per cycle (24 -> 24,000)");
        std::vector<uint8_t> blown;
        for (size_t i = 0; i < 24000; i++)
            blown.push_back(ids[i]);
        for (size_t i = 24000; i < 65535; i++)
            blown.push_back(23);
        tl::Load done;
        auto healed = load(seasons, blown, &guard, 24, &done);
        expect(healed.size() == 24 && save(seasons, healed) == season24, "a blown-up 65,535-entry save loads back to the season's 24 events, in order");
        expect(done.blown && done.limit == 24 && done.dropped == 65535 - 24, "the load is logged as blown up, with the entries dropped");
        expect(load(seasons, save(seasons, healed), &guard, 24) == healed, "the healed calendar saves and loads unchanged");
        auto grown = save(seasons, load(seasons, season24, nullptr, 24));
        expect(save(seasons, load(seasons, grown, &guard, 24)) == season24, "a save the game grew once (240 entries) heals to the 24 stored ids in order");
        auto no_season = load(seasons, blown, &guard, 0, &done);
        expect(no_season.size() == 24 && done.limit == tl::FALLBACK_LIMIT,
               "without a season event list length the bound is 64 and runs of one id still heal to 24 events");
        std::vector<uint8_t> scattered;
        for (size_t i = 0; i < 1000; i++)
            scattered.push_back(uint8_t(i % 24));
        expect(load(seasons, scattered, &guard, 24).size() == 24, "a blown-up list that is not in runs is cut at the season's length");
    }

    {
        std::vector<uint8_t> rows = stock;
        rows.insert(rows.end(), stock.begin(), stock.end());
        std::vector<uint8_t> merged;
        for (uint8_t id = 0; id < 24; id++) {
            merged.push_back(id);
            if (id % 3 == 0)
                merged.push_back(id);
        }
        tl::Load done;
        auto cal = load(rows, merged, &guard, 24, &done, 1, 32);
        expect(merged.size() == 32 && cal.size() == 32 && save(rows, cal) == merged,
               "a 32-round season (8 tracks twice) on a 24-event default list loads all 32 rounds in order when the career calendar races 32 weekends");
        expect(!done.blown && done.dropped == 0 && done.limit == 32, "the 32-round season is not read as blown up");
        bool flat = true;
        std::vector<uint8_t> ids = merged;
        for (int cycle = 0; cycle < 5; cycle++) {
            cal = load(rows, ids, &guard, 24, nullptr, 1, 32);
            flat = flat && cal.size() == 32;
            ids = save(rows, cal);
        }
        expect(flat && ids == merged, "the 32-round season keeps its 32 rounds over 5 save and load cycles");
        std::string line = tl::load_line(1, 32, 32, &done);
        expect(line.find("(season event list 24, 32 race weekends on the career calendar)") != std::string::npos &&
                   line.find("more entries than the season event list, all kept: the career calendar races 32 weekends") != std::string::npos &&
                   line.find("blown up") == std::string::npos,
               "the load line names the race weekends and says the longer list was kept");
        std::vector<uint8_t> scattered;
        for (uint8_t id = 0; id < 24; id++)
            scattered.push_back(id);
        for (uint8_t id = 0; id < 8; id++)
            scattered.push_back(uint8_t(id * 3));
        expect(load(rows, scattered, &guard, 24, nullptr, 1, 32).size() == 32,
               "a 32-round season whose repeated tracks are not side by side also keeps all 32 rounds");
        expect(load(rows, merged, &guard, 24, nullptr, 1, 0).size() == 24 && load(rows, merged, &guard, 24, nullptr, 1, 24).size() == 24,
               "without race weekends past 24 the same list is still bounded by the default event list");
        expect(load(rows, merged, &guard, 0, nullptr, 1, 32).size() == 32, "the race weekends bound the load when the event list length is not set");
    }

    {
        std::vector<uint8_t> rows = stock;
        rows.insert(rows.end(), stock.begin(), stock.end());
        std::vector<uint8_t> season22(stock.begin(), stock.begin() + 22);
        auto doubled = save(rows, load(rows, season22, nullptr, 24));
        expect(doubled.size() == 44, "two event rows per id: the game alone doubles a 22-round season to 44 entries in one load");
        tl::Load done;
        auto healed = load(rows, doubled, &guard, 24, &done, 1, 22);
        expect(healed.size() == 22 && save(rows, healed) == season22 && done.blown && done.dropped == 22,
               "the 44 = 22x2 doubled list loads back to its 22 rounds in order when the career calendar races 22 weekends");
        healed = load(rows, doubled, &guard, 24, &done, 1, 0);
        expect(healed.size() == 22 && save(rows, healed) == season22, "the doubled list also heals to 22 when the race weekends are unreadable");
        auto kept = load(rows, doubled, &guard, 24, &done, 1, 44);
        expect(kept.size() == 44 && !done.blown,
               "a career that already races the 44 weekends keeps all 44: a shorter calendar would strand the season's last rounds");
        auto blown = doubled;
        for (int cycle = 0; cycle < 3; cycle++)
            blown = save(rows, load(rows, blown, nullptr, 24));
        expect(blown.size() == 352 && load(rows, blown, &guard, 24, nullptr, 1, 22).size() == 22,
               "a list the game kept doubling (352 entries) heals to the season's 22 rounds");
    }

    {
        std::vector<uint8_t> mem(0x40000);
        const uintptr_t base = 0x100000;
        auto at = [&](uintptr_t a) { return mem.data() + (a - base); };
        auto put = [&](uintptr_t a, auto v) { std::memcpy(at(a), &v, sizeof v); };
        auto read = [&](uintptr_t a, auto &out) {
            if (a < base || a + sizeof out > base + mem.size())
                return false;
            std::memcpy(&out, at(a), sizeof out);
            return true;
        };
        const uintptr_t component = base, seasons = base + 0x1000, calendars = base + 0x2000, slots = base + 0x8000;
        const uintptr_t acts = base + 0x20000, race_def = base + 0x3f000, other_def = base + 0x3f100;
        put(race_def + tl::DEF_TYPE, uint8_t(tl::RACE_WEEKEND));
        put(other_def + tl::DEF_TYPE, uint8_t(0x0e));
        put(component + tl::SEASONS_BEGIN, uint64_t(seasons));
        put(component + tl::SEASONS_END, uint64_t(seasons + 3 * tl::SEASON_RECORD));
        put(component + tl::SEASON_INDEX, uint32_t(1));
        uintptr_t slot = slots, act = acts;
        auto calendar = [&](uintptr_t cal, size_t races, size_t others) {
            put(cal + tl::ACTIVITIES_BEGIN, uint64_t(slot));
            for (size_t i = 0; i < races + others; i++, slot += tl::ACTIVITY_SLOT, act += 0x20) {
                put(slot + tl::SLOT_ACTIVITY, uint64_t(act));
                put(act + tl::ACTIVITY_DEF, uint64_t(i < races ? race_def : other_def));
            }
            put(cal + tl::ACTIVITIES_END, uint64_t(slot));
        };
        uintptr_t current = seasons + tl::SEASON_RECORD;
        put(current + tl::DRIVER_CALENDARS_BEGIN, uint64_t(calendars));
        put(current + tl::DRIVER_CALENDARS_END, uint64_t(calendars + 3 * tl::DRIVER_CALENDAR));
        calendar(calendars, 30, 5);
        calendar(calendars + tl::DRIVER_CALENDAR, 32, 9);
        calendar(calendars + 2 * tl::DRIVER_CALENDAR, 0, 0);
        uintptr_t previous = seasons;
        put(previous + tl::DRIVER_CALENDARS_BEGIN, uint64_t(calendars + 3 * tl::DRIVER_CALENDAR));
        put(previous + tl::DRIVER_CALENDARS_END, uint64_t(calendars + 4 * tl::DRIVER_CALENDAR));
        calendar(calendars + 3 * tl::DRIVER_CALENDAR, 50, 0);
        expect(tl::race_weekends(component, read) == 32,
               "race weekends: the busiest driver calendar of the current season record, RaceWeekend activities only");
        put(component + tl::SEASON_INDEX, uint32_t(0));
        expect(tl::race_weekends(component, read) == 50, "race weekends follow the component's current season index");
        put(component + tl::SEASON_INDEX, uint32_t(2));
        expect(tl::race_weekends(component, read) == 0, "a season record without driver calendars reads as unknown");
        put(component + tl::SEASON_INDEX, uint32_t(3));
        expect(tl::race_weekends(component, read) == 0, "a season index past the records reads as unknown");
        put(component + tl::SEASON_INDEX, uint32_t(1));
        calendar(calendars + tl::DRIVER_CALENDAR, 65, 0);
        expect(tl::race_weekends(component, read) == 0, "more than 64 race weekends is not a real season and reads as unknown");
        put(component + tl::SEASONS_END, uint64_t(seasons + 0x81));
        expect(tl::race_weekends(component, read) == 0, "a season record vector that is not whole records reads as unknown");
        expect(tl::race_weekends(base + 0x3ff00, read) == 0, "an unreadable component reads as unknown");
    }

    {
        tl::Load l;
        expect(tl::fresh(l, 1, 0), "an empty calendar (a new load) starts over");
        tl::reset(l, 1, 3, 24);
        expect(tl::enter(l, 0, 7), "an entry's first match picks a row");
        expect(!tl::enter(l, 0, 7), "the entry's other matches continue the walk");
        expect(tl::enter(l, 1, 7), "the next entry picks again");
        expect(!tl::fresh(l, 1, 1) && tl::fresh(l, 1, 0) && tl::fresh(l, 2, 1), "a new load or another component starts over");
        tl::reset(l, 2, 3, 24);
        expect(l.component == 2 && l.kept == 0 && l.entry == UINT64_MAX, "a reset clears the load for the new component");
        tl::Load d = tl::finish(l, 3);
        expect(d.component == 0 && l.component == 0 && l.rows.empty(), "finishing another component reports nothing and clears the state");
    }

    expect(tl::season_events(24) == 24 && tl::season_events(0) == 0 && tl::season_events(0x10000) == 0,
           "the season event list length is read when it is 1..255");
    expect(tl::limit_for(24) == 24 && tl::limit_for(0) == 64, "the bound is the season's length, or 64");
    expect(tl::limit_for(24, 32) == 32 && tl::limit_for(24, 16) == 24 && tl::limit_for(0, 32) == 32,
           "the bound is the larger of the event list length and the race weekends");
    expect(tl::last_entry(4, 5) && !tl::last_entry(3, 5), "the last stored entry is index count - 1");
    expect(tl::entries(0x1000, 0x1000 + 3 * tl::ENTRY) == 3 && tl::entries(0x2000, 0x1000) == 0, "calendar size from begin and end");

    tl::SaveLog log;
    expect(log.changed(0xa, 24), "the first save of a component is logged");
    expect(!log.changed(0xa, 24), "a save with the same count is not logged again");
    expect(log.changed(0xa, 25), "a save with a new count is logged");
    expect(log.changed(0xb, 24), "another component's first save is logged");

    tl::Load shown;
    tl::reset(shown, 0x1234, 65535, 24);
    shown.dropped = 65511;
    shown.extra = 216;
    std::string line = tl::load_line(0x1234, 65535, 24, &shown);
    expect(line.find("65535 stored entries -> 24 calendar events (season event list 24)") != std::string::npos &&
               line.find("216 more matching events") != std::string::npos && line.find("blown up (more than 24 entries)") != std::string::npos &&
               line.find("65511 stored entries dropped") != std::string::npos,
           "the load line names the counts, the season bound, the skipped matches and the dropped entries");
    tl::Load clean;
    tl::reset(clean, 0x1234, 24, 24);
    expect(tl::load_line(0x1234, 24, 24, &clean).find("(season event list 24), no repeats") != std::string::npos, "a clean load says no repeats");
    expect(tl::load_line(0x1234, 5, 50, nullptr).find("5 stored entries -> 50 calendar events") != std::string::npos,
           "the unguarded line names the counts");
    expect(tl::save_line(0x1234, 24).find("24 calendar events") != std::string::npos, "the save line names the count");

    expect(tl::switch_off("0") && tl::switch_off("off"), "SWOFTY_TRACK_LIST_GUARD=0 or off turns it off");
    expect(!tl::switch_off("1") && !tl::switch_off("") && !tl::switch_off(nullptr), "anything else leaves it on");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
