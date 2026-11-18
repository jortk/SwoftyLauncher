#include "teammate_pairs_logic.hpp"

#include <cstdio>

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
    using namespace runtime::teammate_pairs;
    CHECK(!over(0) && !over(8) && !over(16), "a team with no car, one car or two cars is left alone");
    CHECK(over(24), "a team with three cars is clamped");
    CHECK(over(32), "a team with four cars is clamped");
    CHECK(!over(20), "a size that is not a whole number of pointers is left alone");
    CHECK(!over(MAX_BYTES + 8), "an implausibly large vector is left alone");
    CHECK(PAIR_BYTES == 16 && cars(24) == 3, "a pair is two pointers and three cars are 24 bytes");

    Seen seen;
    CHECK(seen.first(0x1000, 6), "the first sighting of team 6 is logged");
    CHECK(!seen.first(0x1000, 6), "team 6 again in the same session is not logged");
    CHECK(seen.first(0x1000, 3), "team 3 is logged");
    CHECK(seen.first(0x2000, 6), "team 6 in a new session is logged again");
    CHECK(!seen.first(0x2000, 6), "and only once");
    for (uint32_t t = 100; t < 100 + SEEN_TEAMS + 4; t++)
        seen.first(0x3000, t);
    CHECK(!seen.first(0x3000, 100), "a team seen before the table filled stays quiet");
    return failures ? 1 : 0;
}
