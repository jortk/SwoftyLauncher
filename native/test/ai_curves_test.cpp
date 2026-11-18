#include "ai_curves_logic.hpp"

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
    using namespace runtime::ai_curves;
    CHECK(skips(0), "a player with no vehicle in the table is skipped");
    CHECK(!skips(0x344176570), "a player whose vehicle resolved runs the game's code");
    bool said = false;
    CHECK(says(0, said), "the first miss is logged");
    CHECK(!says(0, said), "a second miss is not logged");
    bool other = false;
    CHECK(!says(0x1000, other) && !other, "a vehicle that exists is not logged");
    return failures ? 1 : 0;
}
