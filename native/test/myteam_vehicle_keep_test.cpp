// Host-native unit test for the core's My Team vehicle keep (native/src/core/myteam_vehicle_keep_logic.hpp).
// Built and run by native/test/run_myteam_vehicle_keep_test.sh.

#include "myteam_vehicle_keep_logic.hpp"

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace mk = runtime::myteam_keep;
namespace mh = runtime::myteam_heal;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

static std::vector<uint8_t> car(uint32_t handle, const char *label, uint8_t engine, uint32_t base)
{
    std::vector<uint8_t> r(0xd10, 0);
    std::memcpy(r.data(), &handle, 4);
    std::strcpy(reinterpret_cast<char *>(r.data() + mk::VEHICLE_FILE_NAME), label);
    std::strcpy(reinterpret_cast<char *>(r.data() + 0x2a0), "My Team MES");
    r[mk::VEHICLE_ENGINE] = engine;
    std::memcpy(r.data() + mk::VEHICLE_BASE, &base, 4);
    return r;
}

int main()
{
    expect(mk::handle(0x4008) == (8u << 16 | 1) && mk::handle(0) == 1, "a saved stat id becomes the custom vehicle handle");

    uint8_t row[mk::ROW_SIZE];
    std::memset(row, 0xcc, sizeof row);
    expect(mk::build_row(row, mk::VEHICLES_TABLE, mk::VEHICLES_ROW, 412), "a row is built for entrants.vehicles");
    int32_t count = 0;
    std::memcpy(&count, row + mk::ROW_COUNT, 4);
    expect(mk::text_at(row, 0x100) == "entrants.vehicles" && mk::text_at(row + mk::ROW_NAME, 0x100) == "vehicle_412" &&
               count == -1 && row[mk::ROW_FOUND] == 1 && row[0x40] == 0,
           "it reads like Gdb_Get_RowByIndex's row: container, vehicle_<index>, +0x200 -1, +0x204 1");
    expect(!mk::build_row(row, "", "vehicle", 1) && !mk::build_row(row, "entrants.vehicles", "", 1), "an empty name builds no row");

    mh::Stock table;
    table.myteam = {{0x1000, "myteam_ferrari"}, {0x2000, "myteam_mercedes"}, {0x3000, "myteam_honda_2026"},
                    {0x4000, "myteam_mercedes_2026"}};
    mh::Source src;
    expect(mk::pick(table, "myteam_mes_porsche_2026", src) && src.label == "myteam_mercedes_2026",
           "a 2026 addon car is kept as myteam_mercedes_2026");
    expect(mk::pick(table, "myteam_mes_hyundai", src) && src.label == "myteam_mercedes", "a 2025 addon car as myteam_mercedes");
    expect(mk::pick(table, "myteam_mercedes_2026", src) && src.label != "myteam_mercedes_2026",
           "the test car never stands in for itself");

    std::vector<uint8_t> live = car(mk::handle(0x4008), "myteam_mercedes", 21, 0x00a40000u);
    mk::Pending p;
    p.live = true;
    p.handle = mk::handle(0x4008);
    p.original = "myteam_mes_hyundai";
    p.stock.label = "myteam_mercedes";
    p.stock.handle = 0x02160000u;
    p.stock.engine = 2;
    uint64_t at = reinterpret_cast<uint64_t>(live.data());
    mk::Shadow s = mk::shadow_of(at, live.data(), p);
    mk::patch(live.data(), p.stock);
    expect(s.bytes.size() == mk::SAVED_BYTES && mk::text_at(s.bytes.data() + mk::VEHICLE_FILE_NAME, 0x100) == "myteam_mes_hyundai" &&
               s.bytes[mk::VEHICLE_ENGINE] == 21 && mk::base_of(s.bytes.data()) == 0x00a40000u && s.saved_engine == 21,
           "the shadow holds the saved file_name (the game stored the stock row's), engine and base car");
    expect(mk::text_at(live.data() + mk::VEHICLE_FILE_NAME, 0x100) == "myteam_mercedes" && live[mk::VEHICLE_ENGINE] == 2 &&
               mk::base_of(live.data()) == 0x02160000u,
           "the live car becomes the stock car: file_name, engine and base car");
    expect(mk::text_at(live.data() + 0x2a0, 0x100) == "My Team MES", "its display name is left as saved");

    mk::Shadows shadows;
    expect(shadows.put(s), "the shadow is kept");
    const mk::Shadow *f = shadows.find(at, live.data());
    expect(f && mk::text_at(f->bytes.data() + mk::VEHICLE_FILE_NAME, 0x100) == "myteam_mes_hyundai",
           "the save finds the placeholder's shadow and writes the original car");

    std::vector<uint8_t> other = live;
    expect(!shadows.find(reinterpret_cast<uint64_t>(other.data()), other.data()), "another address never takes a shadow");
    std::vector<uint8_t> moved = live;
    std::strcpy(reinterpret_cast<char *>(live.data() + mk::VEHICLE_FILE_NAME), "myteam_renault");
    expect(!shadows.find(at, live.data()), "a car whose file_name changed (replaced in place) is written as it is");
    live = moved;
    uint32_t base = 0x3fff0000u;
    std::memcpy(live.data() + mk::VEHICLE_BASE, &base, 4);
    expect(!shadows.find(at, live.data()), "a car whose base car changed is written as it is");
    live = moved;
    uint32_t h = mk::handle(0x4009);
    std::memcpy(live.data(), &h, 4);
    expect(!shadows.find(at, live.data()), "a car with another handle at the same address is written as it is");
    live = moved;
    expect(shadows.find(at, live.data()) != nullptr, "the untouched placeholder still matches");

    mk::Shadows full;
    for (size_t i = 0; i < mk::MAX_SHADOWS; i++) {
        mk::Shadow x = s;
        x.live = 0x1000 + i;
        full.put(x);
    }
    mk::Shadow extra = s;
    extra.live = 0x9999;
    expect(!full.room(0x9999) && !full.put(extra), "past 64 placeholders a new one is refused (the car is skipped as before)");
    mk::Shadow again = s;
    again.live = 0x1000;
    expect(full.room(0x1000) && full.put(again), "a placeholder reloaded at the same address replaces its shadow");

    mk::Stock ts;
    expect(mk::parse_stock("myteam_mercedes_2026:0x02160000:2:412", ts) && ts.label == "myteam_mercedes_2026" &&
               ts.handle == 0x02160000u && ts.engine == 2 && ts.row_index == 412,
           "SWOFTY_TEST_MYTEAM_KEEP_STOCK reads label:handle:engine:row");
    expect(!mk::parse_stock("myteam_mercedes_2026:0x02160000:2", ts) && !mk::parse_stock(":1:2:3", ts) &&
               !mk::parse_stock("a:x:2:3", ts) && !mk::parse_stock("a:1:999:3", ts) && !mk::parse_stock(nullptr, ts),
           "a malformed test stock car is refused");
    expect(mk::forced("myteam_honda_2026", "myteam_honda_2026") && !mk::forced("", "myteam_honda_2026") &&
               !mk::forced(nullptr, "x") && !mk::forced("myteam_honda_2026", "myteam_honda"),
           "only the test car named by SWOFTY_TEST_MYTEAM_KEEP_LABEL is forced");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
