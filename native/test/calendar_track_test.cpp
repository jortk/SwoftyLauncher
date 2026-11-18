#include "calendar_track_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>
#include <vector>

static int failures;

#define CHECK(cond, msg)                                                                                                \
    do {                                                                                                                \
        if (cond) {                                                                                                     \
            printf("ok  : %s\n", msg);                                                                                  \
        } else {                                                                                                        \
            printf("FAIL: %s\n", msg);                                                                                  \
            failures++;                                                                                                 \
        }                                                                                                               \
    } while (0)

using namespace runtime::calendar_track;

struct Memory {
    std::map<uintptr_t, uint8_t> bytes;

    void put(uintptr_t at, const void *src, size_t n)
    {
        for (size_t i = 0; i < n; ++i)
            bytes[at + i] = static_cast<const uint8_t *>(src)[i];
    }
    void q(uintptr_t at, uint64_t v) { put(at, &v, 8); }
    void b(uintptr_t at, uint8_t v) { put(at, &v, 1); }

    template <class T> bool read(uintptr_t at, T &out) const
    {
        uint8_t buf[sizeof(T)];
        for (size_t i = 0; i < sizeof(T); ++i) {
            auto it = bytes.find(at + i);
            if (it == bytes.end())
                return false;
            buf[i] = it->second;
        }
        memcpy(&out, buf, sizeof(T));
        return true;
    }
    uint64_t q_at(uintptr_t at) const
    {
        uint64_t v = 0;
        read(at, v);
        return v;
    }
};

constexpr uintptr_t COMPONENT = 0x10000, CAREER = 0x20000, SYSTEM = 0x30000, RECORDS = 0x40000, CALENDAR = 0x50000,
                    EVENTS = 0x60000, VTABLE = 0x140000000;

struct Game {
    Memory m;
    size_t rounds = 0;

    Game(const std::vector<uint8_t> &tracks, const std::vector<uint8_t> &loaded, size_t records = 43)
    {
        m.q(COMPONENT + COMPONENT_CAREER, CAREER);
        m.q(CAREER + CAREER_TRACK_DATA, SYSTEM);
        m.q(SYSTEM + RECORDS_BEGIN, RECORDS);
        m.q(SYSTEM + RECORDS_END, RECORDS + records * ITEM);
        for (size_t t = 0; t < records; ++t) {
            m.q(RECORDS + t * ITEM, 0xa110c);
            m.q(RECORDS + t * ITEM + ITEM_RECORD, 0);
        }
        for (uint8_t t : loaded)
            m.q(RECORDS + t * ITEM + ITEM_RECORD, 0x70000 + t * 0x1000);
        for (size_t i = 0; i < tracks.size(); ++i) {
            uintptr_t entry = CALENDAR + i * ENTRY, event = EVENTS + i * 0x100;
            m.q(entry, VTABLE);
            m.q(entry + ENTRY_EVENT_ID, 110 + i);
            m.q(entry + ENTRY_EVENT, event);
            m.q(entry + 0x18, 1000 + i);
            m.q(entry + 0x20, 0xd00 + i);
            m.b(event + EVENT_TRACK, tracks[i]);
        }
        rounds = tracks.size();
        m.q(COMPONENT + CALENDAR_BEGIN, CALENDAR);
        m.q(COMPONENT + CALENDAR_END, CALENDAR + rounds * ENTRY);
    }

    Calendar scan() const
    {
        return runtime::calendar_track::scan(COMPONENT, [this](uintptr_t at, auto &out) { return m.read(at, out); });
    }
};

static std::vector<uint8_t> loaded_2026()
{
    std::vector<uint8_t> out;
    for (uint8_t t : {0, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 19, 20, 26, 27, 29, 30, 31, 32, 39, 40, 41, 42})
        out.push_back(t);
    return out;
}

int main()
{
    {
        Game g({3, 12, 29, 31, 32, 25, 0, 14}, loaded_2026());
        Calendar cal = g.scan();
        CHECK(cal.read && cal.rounds.size() == 8 && cal.records == 43, "the 8-round calendar is read");
        CHECK(!cal.rounds[5].data && cal.rounds[5].track == 25 && cal.rounds[5].event == 115,
              "round 6 on track 25 (styria) has no track data, as in the BackUpRise crash");
        size_t with = 0;
        for (const Round &r : cal.rounds)
            with += r.data;
        CHECK(with == 7, "the other 7 rounds have track data");
        std::vector<size_t> keep = kept(cal.rounds);
        CHECK((keep == std::vector<size_t>{0, 1, 2, 3, 4, 6, 7}), "every round but round 6 is kept, in order");
        uintptr_t end = compact(
            cal.begin, keep, [&](uintptr_t at, auto &out) { return g.m.read(at, out); },
            [&](uintptr_t at, uint64_t v) { g.m.q(at, v); });
        CHECK(end == CALENDAR + 7 * ENTRY, "the calendar ends after 7 entries");
        CHECK(g.m.q_at(CALENDAR + 5 * ENTRY + ENTRY_EVENT_ID) == 116 && g.m.q_at(CALENDAR + 5 * ENTRY + 0x18) == 1006 &&
                  g.m.q_at(CALENDAR + 5 * ENTRY + 0x20) == 0xd06,
              "the round after the dropped one moves into its slot with its event id, day and last word");
        CHECK(g.m.q_at(CALENDAR + 6 * ENTRY + ENTRY_EVENT) == EVENTS + 7 * 0x100, "the last round moves up with its event");
        CHECK(g.m.q_at(CALENDAR + 4 * ENTRY + ENTRY_EVENT) == EVENTS + 4 * 0x100, "rounds before the dropped one stay");
        CHECK(g.m.q_at(CALENDAR + 5 * ENTRY) == VTABLE, "each slot keeps its vtable");
        g.m.q(COMPONENT + CALENDAR_END, end);
        Calendar after = g.scan();
        bool all = after.read && after.rounds.size() == 7;
        for (const Round &r : after.rounds)
            all = all && r.data;
        CHECK(all, "after the drop every round of the calendar has track data");
        CHECK(kept(after.rounds).empty(), "a calendar with track data for every round is left as it is");
    }
    {
        Game g({89, 33, 36, 2, 10, 11, 16, 29, 31, 27}, [] {
            auto l = loaded_2026();
            l.push_back(89);
            return l;
        }(), 90);
        Calendar cal = g.scan();
        std::vector<size_t> keep = kept(cal.rounds);
        CHECK(cal.read && !cal.rounds[1].data && !cal.rounds[2].data && cal.rounds[0].data,
              "imola_prototype (33) and suzuka24 (36) have no data; the added track 89 has");
        CHECK(keep.size() == 8 && keep[0] == 0 && keep[1] == 3, "both are left out and the added track stays first");
    }
    {
        Game g({34, 25, 33}, loaded_2026());
        Calendar cal = g.scan();
        CHECK(cal.read && kept(cal.rounds).empty(), "a calendar with no round that has track data is left as it is");
    }
    {
        Game g({0, 200, 3}, loaded_2026());
        Calendar cal = g.scan();
        CHECK(cal.read && !cal.rounds[1].data, "a track past the record vector has no track data");
    }
    {
        Game g({0, 3}, loaded_2026());
        g.m.q(CALENDAR + ENTRY + ENTRY_EVENT, 0);
        CHECK(!g.scan().read, "an entry without an event leaves the calendar unread");
    }
    {
        Game g({0, 3}, loaded_2026());
        g.m.q(COMPONENT + CALENDAR_END, CALENDAR + ENTRY + 8);
        CHECK(!g.scan().read, "a calendar whose length is not a whole number of entries is not read");
        g.m.q(COMPONENT + CALENDAR_END, CALENDAR + (MAX_ENTRIES + 1) * ENTRY);
        CHECK(!g.scan().read, "a calendar past the entry bound is not read");
    }
    {
        Game g({0, 3}, loaded_2026());
        g.m.q(CAREER + CAREER_TRACK_DATA, 0);
        CHECK(!g.scan().read, "a career without a track data system is not read");
    }
    {
        Game g({0, 3}, loaded_2026());
        g.m.q(SYSTEM + RECORDS_END, RECORDS);
        CHECK(!g.scan().read, "an empty record vector is not read");
    }
    {
        Game g({0, 25, 3}, loaded_2026());
        Calendar cal = g.scan();
        std::vector<size_t> keep = kept(cal.rounds);
        g.m.bytes.erase(CALENDAR + 2 * ENTRY + 0x20);
        uintptr_t end = compact(
            cal.begin, keep, [&](uintptr_t at, auto &out) { return g.m.read(at, out); },
            [&](uintptr_t at, uint64_t v) { g.m.q(at, v); });
        CHECK(end == 0 && g.m.q_at(CALENDAR + ENTRY + ENTRY_EVENT) == EVENTS + 0x100,
              "a kept round that can't be read writes nothing");
    }
    printf("%s (%d failure(s))\n", failures ? "FAILED" : "passed", failures);
    return failures ? 1 : 0;
}
