#include <stdio.h>
#include "supported_builds.h"
#include "build_gate.h"

static int failures;

static void check(int cond, const char *what)
{
    if (!cond) {
        printf("FAIL: %s\n", what);
        failures++;
    }
}

int main(void)
{
    const unsigned long long steam = 408063872ULL, ea_old = 342936448ULL, steam_new = 391613312ULL;
    const unsigned long long unlisted = 370000000ULL, ea_pre_update = 358465536ULL;

    check(!swofty_build_blocked(steam, 1789118616UL, NULL), "the first Steam build loads");
    check(!swofty_build_blocked(ea_old, 1789128892UL, NULL), "the first EA build loads");
    check(!swofty_build_blocked(steam_new, 1794312636UL, NULL), "the 1.0.145 Steam build loads");
    check(!swofty_build_blocked(steam_new, 1794312636UL, ""), "an empty override leaves a supported build loaded");

    check(swofty_build_blocked(unlisted, 1794000000UL, NULL), "an exe the table does not list is blocked");
    check(swofty_build_blocked(ea_pre_update, 1789128892UL, NULL), "a size that pairs with another stamp is blocked");
    check(swofty_build_blocked(ea_old, 1794000000UL, NULL), "a stamp that pairs with another size is blocked");
    check(swofty_build_blocked(unlisted, 1794000000UL, ""), "an empty override does not unblock");
    check(swofty_build_blocked(unlisted, 1794000000UL, "0"), "an override of 0 does not unblock");
    check(swofty_build_blocked(unlisted, 1794000000UL, " 0 "), "an override of 0 with spaces does not unblock");
    check(!swofty_build_blocked(unlisted, 1794000000UL, "1"), "an override of 1 unblocks");
    check(!swofty_build_blocked(unlisted, 1794000000UL, "yes"), "any other value unblocks");

    check(!swofty_build_blocked(1 << 20, 1UL, NULL), "a small stand-in exe is never gated");
    check(!swofty_build_blocked(SWOFTY_GATE_MIN_SIZE - 1, 1UL, NULL), "just under the size floor is never gated");
    check(swofty_build_blocked(SWOFTY_GATE_MIN_SIZE, 1UL, NULL), "the size floor itself is gated");

    printf(failures ? "build gate: %d failure(s)\n" : "build gate: all checks passed\n", failures);
    return failures != 0;
}
