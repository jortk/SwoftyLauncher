// Host-native unit test for the core's garage slot guard (native/src/core/garage_slots_logic.hpp).
// Built and run by native/test/run_garage_slots_test.sh.

#include "garage_slots_logic.hpp"

#include <cstdio>

namespace gs = runtime::garage_slots;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    uint64_t count = 0;
    expect(gs::count_of(0x20000, 0x20000 + 11 * 8, &count) && count == 11, "11 garages are counted from the vector");
    expect(gs::count_of(0x20000, 0x20000, &count) && count == 0, "an empty garage vector has no garages");
    expect(!gs::count_of(0, 0, &count), "a null vector is not counted");
    expect(!gs::count_of(0x20000, 0x10000, &count), "an end before the begin is not counted");
    expect(!gs::count_of(0x20000, 0x20005, &count), "a length that is not whole pointers is not counted");

    expect(gs::has_garage(0, 11), "team position 0 has a garage");
    expect(gs::has_garage(10, 11), "the last garage of 11 is assigned");
    expect(!gs::has_garage(11, 11), "position 11 of 11 garages is past the vector");
    expect(!gs::has_garage(12, 10), "the 13th team of a 10 garage world gets none");
    expect(!gs::has_garage(0, 0), "a world without garages assigns none");

    gs::Seen seen;
    expect(seen.first(10), "the first skip is logged");
    expect(!seen.first(10), "later skips in the same world are not");
    expect(seen.first(11), "a world with another garage count logs again");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
