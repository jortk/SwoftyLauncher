// Host-native unit test for the core's stock game message log (native/src/core/game_message_log_logic.hpp): which ids
// count as stock messages, that each id is logged once, and where the id cap stops logging. Built and run by
// native/test/run_game_message_log_test.sh.

#include "game_message_log_logic.hpp"

#include <cstdio>
#include <string>

namespace gm = runtime::game_message_log;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    expect(gm::stock("continue_solo_gp_f1w_offline_warning", false), "a game id raised by the game is stock");
    expect(gm::stock("f1w_car_offline", false), "f1w_car_offline is stock");
    expect(!gm::stock("beta_motd", true), "the core's beta_motd fallback is not stock");
    expect(gm::stock("beta_motd", false), "beta_motd raised by the game is stock");
    expect(!gm::stock("swofty_message", false), "swofty_message is the core's own");
    expect(!gm::stock("swofty_toast_5", false), "swofty_toast_* is the core's own");
    expect(!gm::stock(nullptr, false), "an unreadable id is not logged");
    expect(!gm::stock("", false), "an empty id is not logged");
    expect(!gm::stock("bad\x01id", false), "an id with control bytes is not logged");
    std::string longest(gm::MAX_ID_CHARS, 'a'), too_long(gm::MAX_ID_CHARS + 1, 'a');
    expect(gm::stock(longest.c_str(), false), "an id at the length cap is logged");
    expect(!gm::stock(too_long.c_str(), false), "an id past the length cap is not logged");

    gm::Seen seen;
    expect(seen.first("f1w_car_offline"), "the first sight of an id logs");
    expect(!seen.first("f1w_car_offline"), "a repeat of the same id does not log");
    expect(seen.first("continue_solo_gp_f1w_offline_warning"), "another id logs");
    expect(!seen.last(), "two ids are below the cap");
    for (size_t i = seen.ids.size(); i < gm::MAX_IDS - 1; i++)
        seen.first(("msg_" + std::to_string(i)).c_str());
    expect(!seen.last(), "one below the cap is not the last");
    expect(seen.first("the_last_one"), "the id that reaches the cap logs");
    expect(seen.last(), "the id that reaches the cap is marked last");
    expect(!seen.first("past_the_cap"), "ids past the cap do not log");
    expect(!seen.first("f1w_car_offline"), "an id seen before the cap still does not repeat");

    std::printf("%s\n", failed ? "game message log test FAILED" : "game message log test passed");
    return failed ? 1 : 0;
}
