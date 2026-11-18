#include "replay_grid_logic.hpp"

#include <cstdio>
#include <thread>
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

int main()
{
    namespace rg = runtime::replay_grid;
    const uint64_t base = 0x146cc4ec0;
    CHECK(!rg::outgrew(base, base, 0x1c00), "an empty state is recorded");
    CHECK(!rg::outgrew(base + 0x1c00, base, 0x1c00), "a state exactly the holder's capacity is recorded");
    CHECK(rg::outgrew(base + 0x1c01, base, 0x1c00), "a state one byte over the holder's capacity is skipped");
    CHECK(rg::outgrew(base + 0x4000, base, 0x20), "a small holder with a large state is skipped");
    CHECK(!rg::outgrew(base + 0x4000, base, 0), "a capacity that read as zero leaves the game's code alone");
    CHECK(!rg::outgrew(base - 8, base, 0x20), "a position before the base leaves the game's code alone");
    CHECK(rg::capacity_of(0xdead00000020ull) == 0x20, "the capacity is the low 32 bits of the method's return");
    CHECK(!rg::guards(22) && !rg::guards(24), "a stock grid of up to 24 cars runs the game's code unchanged");
    CHECK(rg::guards(25) && rg::guards(30), "a grid past 24 cars is guarded");
    CHECK(rg::guards(-1), "an unknown car count is guarded");
    CHECK(!rg::outgrew(base + 0x20, base, rg::capacity_of(0xdead00000020ull)), "stale high bits do not shrink the capacity");

    const uint64_t holder = 0x341efdf00, cur = holder + rg::HEADER, prev = cur + 0x480;
    CHECK(rg::room_of(cur, prev, 0x400) == 0x480, "a holder's room is the distance from its current buffer to its previous one");
    CHECK(rg::room_of(cur, cur, 0x400) == 0 && rg::room_of(prev, cur, 0x400) == 0,
          "buffers that read out of order give no room, so the game's code runs unchanged");
    CHECK(rg::room_of(cur, cur + 0x100, 0x400) == 0, "a room smaller than the capacity is no room");
    const uint64_t canned_state = 1050, drs_lap_start = 22 * (13 + 2 * 7) + 5, drs_overrun = 22 * (13 + 5 * 6 + 2 * 7) + 5;
    CHECK(!rg::outgrew(base + canned_state, base, rg::room_of(cur, prev, 0x400)),
          "CannedEventsSystem a few bytes past a 1024 byte capacity stays in the game's code on a stock grid");
    CHECK(!rg::outgrew(base + drs_lap_start, base, rg::room_of(cur, prev, 0x400)),
          "DragReductionSystem with 7 DRS zones and 22 cars fits its holder at the start of a lap");
    CHECK(rg::outgrew(base + drs_overrun, base, rg::room_of(cur, prev, 0x400)),
          "DragReductionSystem after the sixth of 7 detection points with 22 cars outgrows its holder's room");
    CHECK(!rg::outgrew(base + 0x480, base, rg::room_of(cur, prev, 0x400)) && rg::outgrew(base + 0x481, base, rg::room_of(cur, prev, 0x400)),
          "a state of exactly the room is recorded and one byte more is not");
    CHECK(!rg::outgrew(base + 22 * (13 + 7 * 4) + 5, base, 0x400), "4 DRS zones with 22 cars fit the stock capacity");
    int drs = rg::grown_class(5, drs_overrun);
    CHECK(drs >= 0 && rg::CLASSES[drs].cap >= 2 * drs_overrun && rg::CLASSES[drs].cap >= 22 * (13 + 7 * 7) + 5,
          "the grown DragReductionSystem holder takes a full lap of 7 detection points");
    CHECK(rg::room_line(22, "DragReductionSystem", 1215, 1152, 3072) ==
              "[replay] session has 22 cars; DragReductionSystem needs 1215 bytes of replay state, more than the 1152 its "
              "holder has room for (track or session data larger than the game sized it for), so the holder grew to 3072",
          "the room line names the cars, the recordable, its state, the room and the grown capacity");
    CHECK(rg::room_line(-1, "", 1215, 1152, 0).find("session has ? cars; ? needs 1215 bytes") != std::string::npos &&
              rg::room_line(-1, "", 1215, 1152, 0).find("so it is not recorded (replay and flashback lose its state)") !=
                  std::string::npos,
          "the room line reads ? for an unknown count and name and says a cut state is not recorded");

    std::string l = rg::line(25, "VehicleSystem", 4096, 3072);
    CHECK(l == "[replay] session has 25 cars, the recorder holds 22; VehicleSystem needs 4096 bytes of replay state and its "
               "holder has 3072, so it is not recorded (replay and flashback lose its state; the session keeps all its cars)",
          "the log line names the cars, the recorder's capacity and the recordable");
    CHECK(rg::line(-1, "", 8, 4).find("session has ? cars") != std::string::npos, "an unknown car count reads as ?");
    CHECK(rg::line(-1, "", 8, 4).find("; ? needs 8 bytes") != std::string::npos, "an unnamed recordable reads as ?");

    CHECK(rg::class_of_rva(0x5660430) == 5 && rg::CLASSES[5].cap == 0x400, "the 1024 byte class (CannedEventsSystem) is found by vtable");
    CHECK(rg::class_of_rva(0x1234) == -1, "an unknown vtable is no class");
    int canned = rg::grown_class(5, 1032);
    CHECK(canned >= 0 && rg::CLASSES[canned].cap >= 2 * 1032, "a state 8 bytes over 1024 moves to a class with twice its size");
    CHECK(rg::CLASSES[canned].cap == 0xc00, "CannedEventsSystem at 1032 bytes takes the 3072 byte class");
    int canned_big = rg::grown_class(5, 1500);
    CHECK(canned_big >= 0 && rg::CLASSES[canned_big].cap >= 3000, "a state that kept growing to 1500 bytes still gets twice its size");
    const rg::HolderClass dyn[] = {{0x100, 0x400, 0x480}, {0x200, 0x2000, 0x2080}, {0x300, 0x800, 0x880}, {0x400, 0x1000, 0x1080}};
    CHECK(rg::class_of_rva(dyn, 4, 0x300) == 2 && rg::grown_class(dyn, 4, 0, 1032) == 3,
          "an unordered table found at run time picks the smallest class with twice the state");
    CHECK(rg::grown_class(dyn, 4, 0, 0x3000) == -1, "a table with no class that holds the state picks none");
    CHECK(rg::grown_class(5, 0x400) == -1, "a state that fits its class does not grow it");
    CHECK(rg::grown_class(5, 0x10000) == -1, "a state past the largest class has no class");
    int tight = rg::grown_class(5, 0xffff - 1);
    CHECK(tight == rg::CLASS_COUNT - 1, "a state that fits only the largest class takes it without headroom");
    for (int i = 0; i + 1 < rg::CLASS_COUNT; i++)
        if (rg::CLASSES[i].cap > rg::CLASSES[i + 1].cap || rg::CLASSES[i].buf > rg::CLASSES[i + 1].buf) {
            CHECK(false, "classes are ordered by capacity and buffer");
            break;
        }
    for (int i = 0; i < rg::CLASS_COUNT; i++) {
        int to = rg::grown_class(i, uint64_t(rg::CLASSES[i].cap) + 1);
        if (to >= 0 && (rg::CLASSES[to].buf <= rg::CLASSES[i].buf || rg::CLASSES[to].cap <= rg::CLASSES[i].cap)) {
            CHECK(false, "a grown holder is larger in capacity and buffer");
            break;
        }
    }
    CHECK(rg::grown_line(24, "CannedEventsSystem", 1032, 1024, 1980) ==
              "[replay] session has 24 cars, the recorder holds 22; CannedEventsSystem needs 1032 bytes of replay state and "
              "its holder had 1024, so the holder grew to 1980",
          "the grown line names the recordable and both capacities");

    rg::Emptied emptied;
    CHECK(!emptied.has(0x1000), "nothing is remembered at first");
    emptied.add(0x1000);
    emptied.add(0x1000);
    emptied.add(0x2000);
    CHECK(emptied.has(0x1000) && emptied.has(0x2000) && !emptied.has(0x3000), "recordables recorded as nothing are remembered");
    CHECK(rg::skips_read(0, emptied.has(0x1000)), "an empty stored state of a remembered recordable is not applied");
    CHECK(!rg::skips_read(0, emptied.has(0x3000)), "an empty stored state of another recordable is applied as the game does");
    CHECK(!rg::skips_read(12, emptied.has(0x1000)), "a stored state with bytes is applied");

    rg::Tally tally;
    CHECK(tally.first(), "the first skip is logged");
    CHECK(!tally.first() && !tally.first(), "later skips are not logged");
    CHECK(tally.skipped.load() == 3, "every skip is counted");

    rg::Tally shared;
    std::atomic<int> logged{0};
    std::vector<std::thread> workers;
    for (int i = 0; i < 8; i++)
        workers.emplace_back([&] {
            for (int k = 0; k < 1000; k++)
                if (shared.first())
                    logged++;
        });
    for (auto &w : workers)
        w.join();
    CHECK(logged == 1, "threads log one line between them");
    return failures ? 1 : 0;
}
