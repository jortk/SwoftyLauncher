// Host-native unit test for the core's Grand Prix continue F1 World car guard (native/src/core/gp_continue_logic.hpp).
// Built and run by native/test/run_gp_continue_test.sh.

#include "gp_continue_logic.hpp"

#include <cstdio>
#include <string>

namespace gc = runtime::gp_continue;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    const uint64_t real_f1 = 0x502f8e3e0, f2 = 0x502f8e3e8, generic = 0x502f8e408;

    expect(gc::decide(generic, real_f1, generic) == gc::Verdict::no_match,
           "the usual state: an F1 season save does not match the F1 World car, no popup");
    expect(gc::decide(real_f1, real_f1, generic) == gc::Verdict::skip,
           "the reported state: an F1 World car on the real F1 category matches an F1 season save, the check is skipped");
    expect(gc::decide(f2, f2, generic) == gc::Verdict::skip, "an F2 save on a stray F2 category is skipped too");
    expect(gc::decide(generic, generic, generic) == gc::Verdict::keep,
           "a save on the F1 World car's category keeps the game's popup");
    expect(gc::decide(real_f1, real_f1, 0) == gc::Verdict::keep_unknown,
           "without an f1_world setup mode to compare with, the game's check runs unchanged");
    expect(gc::decide(real_f1, 0, generic) == gc::Verdict::no_match, "a setup mode without a category never matches");
    expect(gc::decide(0, 0, generic) == gc::Verdict::no_match, "an empty component and an empty category do not match");
    expect(gc::decide(generic, real_f1, 0) == gc::Verdict::no_match, "a mismatch needs no lookup and stays a mismatch");

    expect(gc::line(gc::Check::car_offline, gc::Verdict::skip, "f1_26") ==
               "continue: F1 World car check skipped (setup mode f1_26 is not the F1 World car)",
           "the skip line names the check and the setup mode");
    expect(gc::line(gc::Check::car_offline, gc::Verdict::keep, "f1_world").find("is the F1 World car") != std::string::npos,
           "the keep line says the save is on the F1 World car");
    expect(gc::line(gc::Check::offline_warning, gc::Verdict::skip, "swofty_f1").rfind("continue: F1 World offline warning skipped", 0) == 0,
           "the warning check has its own name");
    expect(gc::line(gc::Check::car_offline, gc::Verdict::skip, "").find("setup mode ?") != std::string::npos,
           "an unreadable label shows as ?");
    expect(gc::line(gc::Check::car_offline, gc::Verdict::no_match, "f1_26").rfind("continue: no F1 World car check", 0) == 0,
           "a mismatch logs that no check applies");

    gc::Press last;
    expect(!gc::same_press(last, 0x1000, 50, gc::Verdict::skip, "f1_26"), "the first press is always logged");
    last = {0x1000, 50, gc::Verdict::skip, "f1_26", true};
    expect(gc::same_press(last, 0x1000, 60, gc::Verdict::skip, "f1_26"),
           "the second check of the same press with the same outcome folds into the first line");
    expect(!gc::same_press(last, 0x1000, 60 + gc::PRESS_MS, gc::Verdict::skip, "f1_26"), "a later press logs again");
    expect(!gc::same_press(last, 0x2000, 60, gc::Verdict::skip, "f1_26"), "another frame is another press");
    expect(!gc::same_press(last, 0x1000, 60, gc::Verdict::no_match, "f1_26"), "a different outcome is logged");
    expect(!gc::same_press(last, 0x1000, 60, gc::Verdict::skip, "f1"), "a different setup mode is logged");
    expect(!gc::same_press(last, 0x1000, 40, gc::Verdict::skip, "f1_26"), "a clock that went back counts as a new press");

    expect(gc::switch_off("0") && gc::switch_off("off") && gc::switch_off("false"), "0, off and false turn the guard off");
    expect(!gc::switch_off("1") && !gc::switch_off("") && !gc::switch_off(nullptr), "anything else keeps it on");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
