// Host-native unit test for the direct start's Time Trial (native/src/core/direct_start_tt_logic.hpp).
// Built and run by native/test/run_direct_start_tt_test.sh.

#include "direct_start_tt_logic.hpp"

#include <cstdio>

namespace tt = runtime::direct_start::tt;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    std::vector<tt::BoxLists> boxes{{"cs_f1_2017", "cs_f1_2017_tt"}, {"f1_1990", "f1_1990_tt"}, {"bdx_s5_2012", ""}};
    expect(tt::list_for(boxes, "cs_f1_2017") == "cs_f1_2017_tt", "a box's Grand Prix list maps to its Time Trial list");
    expect(tt::list_for(boxes, "f1_1990_tt") == "f1_1990_tt", "a box's Time Trial list stays itself");
    expect(tt::list_for(boxes, "bdx_s5_2012") == "bdx_s5_2012", "a box without a Time Trial list keeps its own list");
    expect(tt::list_for(boxes, "f1_2025") == "f1_2025", "a list no box names is used as it is");

    std::vector<std::string> teams{"ferrari_1990", "McLaren_1990", "mclaren_mercedes_2017", "williams", "team_mcl"};
    expect(tt::pick_team(teams, "mclaren") == 1, "a team word picks the first label holding it as a word");
    expect(tt::pick_team(teams, "MCLAREN_MERCEDES_2017") == 2, "an exact label wins, whatever its case");
    expect(tt::pick_team(teams, "williams") == 3, "an exact label wins over a word");
    expect(tt::pick_team(teams, "mcl") == 4, "a word match wins over a substring");
    expect(tt::pick_team(teams, "ferr") == 0, "a substring is the last resort");
    expect(tt::pick_team(teams, "lotus") == -1, "no match picks nothing");
    expect(tt::pick_team(teams, "") == -1, "an empty name picks nothing");

    expect(tt::word_of("red_bull_racing", "bull"), "a middle word is a word");
    expect(!tt::word_of("redbull", "bull"), "a word's tail is not a word");

    expect(tt::ref_kind(0x00120001) == 1 && tt::ref_index(0x00120001) == 0x12, "a packed ref splits into kind and index");
    expect(tt::ref_index(0x3fff0000) == tt::NO_REF, "the empty ref's index is the sentinel");
    expect(tt::joined({}) == "none" && tt::joined({"a", "b"}) == "a, b", "names join for the log");

    expect(std::string(tt::weather_value("clear")) == "dry" && std::string(tt::weather_value("Rain")) == "wet" &&
               std::string(tt::weather_value("wet")) == "wet",
           "session weather maps to the Time Trial's dry or wet");
    expect(!tt::weather_value("") && !tt::weather_value("fog"), "no or unknown weather leaves the Time Trial's own");

    std::vector<std::string> modes{"f1", "swofty_f1"};
    std::vector<std::vector<std::string>> offered{{"f1_2025", "f1_2025_tt"}, {"f1_1990_tt", "ms_f1_2015"}};
    expect(tt::pick_setup_mode(modes, offered, "swofty_f1", false, "f1_1990_tt") == 1,
           "the default setup mode is kept when it offers the list");
    expect(tt::pick_setup_mode(modes, offered, "swofty_f1", false, "f1_2025") == 0,
           "a stock list falls back to the setup mode that offers it");
    expect(tt::pick_setup_mode(modes, offered, "swofty_f1", true, "f1_2025") == 1,
           "a setup mode the request names is kept, offered or not");
    expect(tt::pick_setup_mode(modes, offered, "swofty_f1", false, "cs_f1_2017_tt") == -1,
           "a list no setup mode offers picks none");
    expect(tt::pick_setup_mode(modes, offered, "custom", true, "f1_2025") == -1, "a named setup mode that is missing picks none");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
