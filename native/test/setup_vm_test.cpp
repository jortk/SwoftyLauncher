#include "setup_vm_logic.hpp"

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
    using namespace runtime::setup_vm;
    CHECK(skips(0), "a setup screen with no car is skipped");
    CHECK(!skips(0x7f0000001000), "a setup screen with a car runs the game's code");

    const uint8_t rel8[] = {0x75, 0x10};
    CHECK(skip_target(0x140c1b92c, 7, rel8) == 0x140c1b945, "a short jne after the compare lands on the game's skip path");
    const uint8_t rel32[] = {0x0f, 0x85, 0x33, 0xfa, 0xff, 0xff};
    CHECK(skip_target(0x140c1ed9f, 7, rel32) == 0x140c1e7df, "a near jne after the compare lands on the game's skip path");
    const uint8_t other[] = {0x74, 0x10};
    CHECK(skip_target(0x1000, 7, other) == 0, "a compare not followed by a jne has no skip path");

    Once once;
    CHECK(once.first(), "the first skip is logged");
    CHECK(!once.first(), "the next skips are not logged");
    return failures ? 1 : 0;
}
