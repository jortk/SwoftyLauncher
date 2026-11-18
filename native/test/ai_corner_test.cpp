#include "ai_corner_logic.hpp"

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
    namespace ac = runtime::ai_corner;
    CHECK(ac::missing(0), "a zero corner line is skipped");
    CHECK(!ac::missing(0x7ff612340000ull), "a corner line that is there runs the game's code");
    CHECK(!ac::missing(1), "any nonzero corner line runs the game's code");

    ac::Tally tally;
    CHECK(tally.first(), "the first skip is logged");
    CHECK(!tally.first() && !tally.first(), "later skips are not logged");
    CHECK(tally.skipped.load() == 3, "every skip is counted");

    ac::Tally shared;
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
    CHECK(logged == 1, "cars on parallel workers log one line between them");
    return failures ? 1 : 0;
}
