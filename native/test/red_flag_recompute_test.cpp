// Host-native unit test for the core's red flag recompute guard (native/src/core/red_flag_recompute_logic.hpp).
// Built and run by native/test/run_red_flag_recompute_test.sh.

#include "red_flag_recompute_logic.hpp"

#include <cstdio>

namespace rf = runtime::red_flag_recompute;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    expect(rf::record_missing(0), "a null race session record is skipped");
    expect(!rf::record_missing(0x1f4a3c8000), "a vehicle with a race session record runs the game's code");
    expect(!rf::record_missing(1), "any non-null race session record is kept");

    rf::Skips s;
    int lines = 0, last = 0;
    for (int i = 0; i < 500; i++) {
        int n = s.note();
        if (s.logged(n))
            lines++;
        if (s.last(n))
            last++;
    }
    expect(lines == rf::LOG_FIRST, "five hundred skips produce exactly the capped number of lines");
    expect(last == 1, "the capped line is marked once");
    expect(s.count == 500, "every skip is counted");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
