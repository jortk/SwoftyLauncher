#include "activity_resume_logic.hpp"

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
    using namespace runtime::activity_resume;
    CHECK(skips(0), "a field read with no group is skipped");
    CHECK(!skips(0x1000), "a field read with a group runs the game's code");
    bool said = false;
    CHECK(says(0, said), "the first miss is logged");
    CHECK(!says(0, said), "a second miss is not logged");
    bool other = false;
    CHECK(!says(0x1000, other) && !other, "a field that exists is not logged");
    return failures ? 1 : 0;
}
