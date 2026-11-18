// Host-native unit test for the core's garage crew guard (native/src/core/garage_crew_logic.hpp): which entries of a
// garage's spawn vector are kept, against a fake session with team tables.
// Built and run by native/test/run_garage_crew_test.sh.

#include "garage_crew_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>
#include <vector>

namespace gc = runtime::garage_crew;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Memory {
    std::map<uint64_t, uint8_t> bytes;

    void put(uint64_t at, const void *p, size_t n)
    {
        for (size_t i = 0; i < n; i++)
            bytes[at + i] = static_cast<const uint8_t *>(p)[i];
    }
    template <class T> void set(uint64_t at, T v) { put(at, &v, sizeof v); }
};

static bool read(uint64_t address, void *out, size_t n, void *user)
{
    auto &m = *static_cast<Memory *>(user);
    for (size_t i = 0; i < n; i++) {
        auto it = m.bytes.find(address + i);
        if (it == m.bytes.end())
            return false;
        static_cast<uint8_t *>(out)[i] = it->second;
    }
    return true;
}

constexpr uint64_t RSP = 0x200000, MODULE = 0x300000, SESSION = 0x400000, TABLE = 0x500000, ROWS = 0x600000;
constexpr uint64_t TEMPLATE = 0x700000, RECORDS = 0x800000;

static uint64_t record(Memory &m, int n, uint8_t kind, uint16_t team)
{
    uint64_t r = RECORDS + n * 0x1000;
    m.set<uint8_t>(r + gc::REC_KIND, kind);
    m.set<uint16_t>(r + gc::REC_TEAM, team);
    return r;
}

struct Seen {
    int extra = 0, none = 0;
};

static void note(void *ctx, uint64_t, uint32_t, gc::Drop why)
{
    auto *s = static_cast<Seen *>(ctx);
    (why == gc::Drop::extra_car ? s->extra : s->none)++;
}

struct Vec {
    std::vector<uint8_t> raw;
    void add(uint64_t rec, uint32_t slot)
    {
        raw.resize(raw.size() + gc::ENTRY_SIZE);
        std::memcpy(&raw[raw.size() - gc::ENTRY_SIZE], &rec, sizeof rec);
        std::memcpy(&raw[raw.size() - gc::ENTRY_SIZE + gc::ENTRY_SLOT], &slot, sizeof slot);
    }
    size_t size() const { return raw.size() / gc::ENTRY_SIZE; }
    uint64_t record(size_t i) const
    {
        uint64_t r;
        std::memcpy(&r, &raw[i * gc::ENTRY_SIZE], sizeof r);
        return r;
    }
};

int main()
{
    Memory m;
    m.set<uint64_t>(RSP + gc::FRAME_MODULE, MODULE);
    m.set<uint64_t>(MODULE + gc::MODULE_SESSION, SESSION);
    m.set<uint64_t>(SESSION + gc::SESSION_TABLES, TABLE);
    m.set<uint64_t>(TABLE + gc::TABLE_BEGIN, ROWS);
    m.set<uint64_t>(TABLE + gc::TABLE_END, ROWS + 4 * gc::ROW_SIZE);
    for (int i = 0; i < 4; i++)
        m.set<uint64_t>(ROWS + i * gc::ROW_SIZE + gc::ROW_TEMPLATE, i == 2 ? 0 : TEMPLATE + i * 0x1000);

    uint64_t session = 0;
    expect(gc::session_of(RSP, read, &m, &session) && session == SESSION, "the session is read through the frame");
    expect(!gc::session_of(0x123, read, &m, &session), "an unreadable frame gives no session");

    uint64_t ok0 = record(m, 0, 0, 0), ok1 = record(m, 1, 0, 3), nullrow = record(m, 2, 0, 2);
    uint64_t past = record(m, 3, 0, 479), badkind = record(m, 4, 3, 0), nokind_table = record(m, 5, 1, 0);
    expect(gc::has_template(SESSION, ok0, read, &m), "a team with a row and a template resolves");
    expect(gc::has_template(SESSION, ok1, read, &m), "the last row resolves");
    expect(!gc::has_template(SESSION, nullrow, read, &m), "a row whose template is null does not resolve");
    expect(!gc::has_template(SESSION, past, read, &m), "a team past the table does not resolve");
    expect(!gc::has_template(SESSION, badkind, read, &m), "a kind past the three tables does not resolve");
    expect(!gc::has_template(SESSION, nokind_table, read, &m), "a kind whose table pointer is null does not resolve");
    expect(!gc::has_template(SESSION, 0, read, &m), "a null record does not resolve");

    {
        Vec v;
        v.add(ok0, 0);
        v.add(ok1, 1);
        Seen s;
        size_t kept = gc::compact(v.raw.data(), v.size(), SESSION, true, read, &m, note, &s);
        expect(kept == 2 && s.extra == 0 && s.none == 0, "two resolved cars in slots 0 and 1 are kept");
    }
    {
        Vec v;
        v.add(ok0, 0);
        v.add(ok1, 1);
        v.add(ok1, 2);
        Seen s;
        size_t kept = gc::compact(v.raw.data(), v.size(), SESSION, true, read, &m, note, &s);
        expect(kept == 2 && s.extra == 1, "a third car in slot 2 is dropped");
    }
    {
        Vec v;
        v.add(past, 0);
        v.add(past, 1);
        Seen s;
        size_t kept = gc::compact(v.raw.data(), v.size(), SESSION, true, read, &m, note, &s);
        expect(kept == 0 && s.none == 2, "a garage whose cars have no template ends up empty");
    }
    {
        Vec v;
        v.add(past, 0);
        v.add(ok0, 1);
        v.add(ok1, 0);
        Seen s;
        size_t kept = gc::compact(v.raw.data(), v.size(), SESSION, true, read, &m, note, &s);
        expect(kept == 2 && v.record(0) == ok0 && v.record(1) == ok1 && s.none == 1,
               "a car without a template is skipped and the next two resolved cars take its place in order");
    }
    {
        Vec v;
        v.add(ok0, 0);
        v.add(past, 1);
        v.add(ok1, 2);
        Seen s;
        size_t kept = gc::compact(v.raw.data(), v.size(), SESSION, true, read, &m, note, &s);
        expect(kept == 1 && v.record(0) == ok0 && s.none == 1 && s.extra == 1,
               "both reasons count apart in one vector");
    }
    {
        Vec v;
        v.add(past, 0);
        v.add(ok0, 1);
        Seen s;
        size_t kept = gc::compact(v.raw.data(), v.size(), 0, false, read, &m, note, &s);
        expect(kept == 2 && s.none == 0, "without a readable session only the slot rule applies");
    }
    {
        Vec v;
        v.add(ok0, 0);
        v.add(ok1, 1);
        v.add(ok0, 5);
        v.add(ok1, 7);
        size_t kept = gc::compact(v.raw.data(), v.size(), SESSION, true, read, &m, nullptr, nullptr);
        expect(kept == 2, "a missing drop callback is allowed");
    }
    {
        expect(gc::setup_template_missing(0), "a null pit stops crew template is missing");
        expect(gc::setup_template_missing(0x560), "a template below the user range is missing");
        expect(!gc::setup_template_missing(TEMPLATE), "a template in the user range is kept");
        expect(gc::SETUP_VECTOR_BEGIN == 0xa8, "the garage entry vector begin sits at rsp + 0xa8");
    }
    {
        expect(gc::scene_missing(0), "a null garage crew scene is missing");
        expect(gc::scene_missing(0xe0), "a scene below the user range is missing");
        expect(!gc::scene_missing(TEMPLATE), "a scene in the user range is kept");
    }
    {
        gc::SetupSkips skips;
        expect(skips.first(3), "the first skip of a garage is logged");
        expect(!skips.first(3), "a garage is logged once");
        expect(skips.first(4), "another garage is logged on its own");
        expect(!skips.first(gc::GARAGES), "a garage past the table is not logged");
        expect(skips.first(gc::GARAGES - 1), "the last garage is logged");
    }

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
