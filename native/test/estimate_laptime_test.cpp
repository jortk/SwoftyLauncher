// Host-native unit test for the core's estimate lap time guard (native/src/core/estimate_laptime_logic.hpp).
// Built and run by native/test/run_estimate_laptime_test.sh.

#include "estimate_laptime_logic.hpp"

#include <cstdio>

namespace el = runtime::estimate_laptime;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    expect(el::tyre_ctx_missing(0), "a null tyre object is skipped");
    expect(!el::tyre_ctx_missing(0x1f4a3c8000), "a found tyre object runs the game's code");
    expect(!el::tyre_ctx_missing(1), "any non-null tyre object is kept");

    el::Skips s;
    int lines = 0, last = 0;
    for (int i = 0; i < 500; i++) {
        int n = s.note();
        if (s.logged(n))
            lines++;
        if (s.last(n))
            last++;
    }
    expect(lines == el::LOG_FIRST, "five hundred skips produce exactly the capped number of lines");
    expect(last == 1, "the capped line is marked once");
    expect(s.count == 500, "every skip is counted");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
