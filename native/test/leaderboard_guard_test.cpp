// Host-native unit test for the core's Time Trial leaderboard guards (native/src/core/leaderboard_guard_logic.hpp):
// when a missing board, child list model, reference board or reference list leaderboard is skipped, how often the skip
// is logged, and how the env switches parse.
// Built and run by native/test/run_leaderboard_guard_test.sh.

#include "leaderboard_guard_logic.hpp"

#include <cstdio>

namespace lg = runtime::leaderboard_guard;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    expect(lg::board_missing(0), "a null board is skipped");
    expect(!lg::board_missing(0x32b702d80), "a board that was found runs the game's code");
    expect(lg::tab_child_missing(0), "a null child list model skips the tab activation");
    expect(!lg::tab_child_missing(0x2e9607f98), "a child list model that exists runs the game's code");
    expect(lg::tab_child_missing(0) && !lg::tab_child_missing(0x3575c3fa0),
           "the garage monitor's bound tab activation skips a null child list model the same way");
    expect(lg::reference_board_missing(0), "a reference with no board skips the game's log line");
    expect(!lg::reference_board_missing(0x3410bcc40), "a reference with a board logs it as the game does");
    expect(lg::reference_list_unboarded(0), "a selected list without a leaderboard leaves the reference unset");
    expect(!lg::reference_list_unboarded(1), "a selected list with a leaderboard runs the game's code");
    expect(lg::reference_list_line(0x3410f7260, "time_trial", "swofty_f1", "sw_f1_2019", 20) ==
               "Time Trial presenter 0x3410f7260: the selected entrant list sw_f1_2019 (20 cars) of setup mode swofty_f1 in "
               "game mode time_trial has no leaderboard; the reference is left unset instead of the game's bad optional "
               "access",
           "the skip names the list, its setup mode and the game mode");
    expect(lg::reference_list_line(0x10, "", "", "", -1).find("list ? (? cars) of setup mode ? in game mode ?") !=
               std::string::npos,
           "names that could not be read show as ?");

    expect(lg::say(0) == lg::Say::none, "occurrence 0 is not logged");
    expect(lg::say(1) == lg::Say::line, "the first skip is logged");
    expect(lg::say(lg::LOG_FIRST - 1) == lg::Say::line, "skips before the cap are logged plainly");
    expect(lg::say(lg::LOG_FIRST) == lg::Say::last, "the last logged skip says the rest are silent");
    expect(lg::say(lg::LOG_FIRST + 1) == lg::Say::none, "skips past the cap are silent");
    expect(lg::say(1000000) == lg::Say::none, "a long run stays silent");

    expect(lg::switch_off("0"), "SWOFTY_LEADERBOARD_TAB_GUARD=0 turns it off");
    expect(lg::switch_off("off"), "SWOFTY_LEADERBOARD_TAB_GUARD=off turns it off");
    expect(!lg::switch_off("1"), "1 keeps it on");
    expect(!lg::switch_off("00"), "00 keeps it on");
    expect(!lg::switch_off("offer"), "offer keeps it on");
    expect(!lg::switch_off(""), "an empty value keeps it on");
    expect(!lg::switch_off(nullptr), "an unset variable keeps it on");
    expect(lg::switch_off("off"), "SWOFTY_TT_REFERENCE_LOG_GUARD=off turns it off");
    expect(lg::switch_off("0"), "SWOFTY_TT_REFERENCE_LIST_GUARD=0 turns it off");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
