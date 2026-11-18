// Host unit test for what a Time Trial session really loaded (tt_loaded_logic.hpp): the box of the loaded list, whether
// the active content follows it, and the lap's team label. run_tt_loaded_test.sh builds and runs it.
#include "../src/core/tt_loaded_logic.hpp"

#include <cstdio>
#include <string>
#include <vector>

namespace tl = runtime::tt_loaded;

static int failed;

static void expect(bool ok, const char *what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    failed += !ok;
}

static constexpr const char *HOST = "swofty_f1", *VHOST = "swofty_virtual_list";
static constexpr const char *SW15 = "swofty.swofty-seasons-2015/sw_2015";

static tl::Loaded load(const std::vector<tl::BoxLists> &boxes, const char *list, const char *mode, const char *vlabel = "")
{
    return tl::loaded_box(boxes, list, mode, HOST, VHOST, vlabel);
}

static constexpr uint32_t team(uint16_t stat, uint8_t kind = 0)
{
    return uint32_t(stat) << 16 | kind;
}

int main()
{
    std::vector<tl::BoxLists> boxes{{"sw_f1_2015", ""}, {"bdx_s5_2012", "bdx_s5_2012_tt"}, {"ms_f1_2015", ""}, {"cs_f1_2021", ""}};

    // The loaded list is the pressed box's: nothing changes.
    tl::Loaded same = load(boxes, "sw_f1_2015", HOST);
    expect(same.kind == tl::Kind::box && same.box == "sw_f1_2015", "a box's list is that box");
    tl::Follow f = tl::follow(same, SW15, std::string(tl::CONTENT) + SW15, "sw_f1_2015");
    expect(f.action == tl::Action::keep && f.line.empty(), "the pressed box's own list keeps the content and logs nothing");
    tl::Loaded own = load(boxes, "f1_custom_team", "f1");
    expect(own.kind == tl::Kind::stock && own.box == "f1_2025", "the game's F1 25 Time Trial list is the box f1_2025");
    expect(tl::follow(own, "", "stock box f1_2025", "f1_2025").action == tl::Action::keep, "the game's box pressed and its list loaded keeps the content");
    tl::Loaded own26 = load(boxes, "f1_custom_team_2026", "f1_26");
    expect(own26.kind == tl::Kind::stock && own26.box == "f1_2026", "the F1 26 Time Trial list is the box f1_2026");
    expect(tl::follow(own26, "", "stock box f1_2026", "f1_2026").action == tl::Action::keep, "the F1 26 box pressed and its list loaded keeps the content");
    expect(tl::follow(load(boxes, "ms_f1_2015", HOST), "", "stock box ms_f1_2015", "ms_f1_2015").action == tl::Action::keep,
           "a box without a package keeps its stock box label");
    tl::Loaded tt = load(boxes, "bdx_s5_2012_tt", HOST);
    expect(tt.kind == tl::Kind::box && tt.box == "bdx_s5_2012_tt", "a box's Time Trial list is named as the picker presses it");
    expect(tl::follow(tt, "", "stock box bdx_s5_2012_tt", "bdx_s5_2012_tt").action == tl::Action::keep, "a pressed Time Trial list keeps the content");

    // SWOFTY 2015 pressed, then the game loads its own cars from the results screen.
    f = tl::follow(own, "", std::string(tl::CONTENT) + SW15, "sw_f1_2015");
    expect(f.action == tl::Action::follow && f.box == "f1_2025", "a SWOFTY box pressed and the game's own list loaded follows to f1_2025");
    expect(f.line == "[boards] content follows the loaded list: game/f1_2025 (box f1_2025; box pressed was sw_f1_2015, active was content "
                     "swofty.swofty-seasons-2015/sw_2015)",
           "the follow line names the board content, the box, the press and what was active");
    expect(tl::board_content("") == "game/f1_2025" && tl::board_content(SW15) == SW15, "the board content is the package, else the game's board");
    f = tl::follow(own26, "", "stock box sw_f1_2020", "sw_f1_2020");
    expect(f.action == tl::Action::follow && f.box == "f1_2026", "an addon box pressed and the F1 26 list loaded follows to f1_2026");

    // The game's box pressed, an addon season's list loaded.
    f = tl::follow(same, SW15, "stock box f1_2025", "f1_2025");
    expect(f.action == tl::Action::follow && f.box == "sw_f1_2015", "the game's box pressed and a season's list loaded follows to the season");
    expect(f.line.find("content follows the loaded list: swofty.swofty-seasons-2015/sw_2015 (box sw_f1_2015; box pressed was f1_2025") != std::string::npos,
           "the follow line names the season's content");
    f = tl::follow(same, SW15, "", "");
    expect(f.action == tl::Action::follow && f.line.find("box pressed was none, active was nothing") != std::string::npos,
           "no press at all follows to the loaded season");
    f = tl::follow(load(boxes, "cs_f1_2021", HOST), "a.b/cs_2021", std::string(tl::CONTENT) + SW15, "sw_f1_2015");
    expect(f.action == tl::Action::follow && f.box == "cs_f1_2021", "one season pressed and another loaded follows to the loaded one");

    // A list no box names.
    tl::Loaded unknown = load(boxes, "some_list", HOST);
    expect(unknown.kind == tl::Kind::unlisted, "a host setup mode list no box names is unlisted");
    f = tl::follow(unknown, "", std::string(tl::CONTENT) + SW15, "sw_f1_2015");
    expect(f.action == tl::Action::no_board && f.box == "some_list" && f.line.find("no board for this Time Trial session") != std::string::npos,
           "an unlisted list gets no board and says why");
    tl::Loaded f2 = load(boxes, "f2_2025", "f2");
    expect(f2.kind == tl::Kind::stock && f2.box == "f2_2025", "a list of another setup mode of the game is a stock box under its own name");

    // What cannot be read concludes nothing.
    expect(load(boxes, "", "f1").kind == tl::Kind::unread, "no list name is unread");
    expect(load(boxes, "some_list", "?").kind == tl::Kind::unread, "an unread setup mode with an unknown list is unread");
    expect(load(boxes, "sw_f1_2015", "?").kind == tl::Kind::box, "an unread setup mode with a box's list is still that box");
    expect(tl::follow(tl::Loaded{}, "", std::string(tl::CONTENT) + SW15, "sw_f1_2015").action == tl::Action::keep, "an unread list keeps the content");
    expect(tl::follow(own, "", "career abc", "").action == tl::Action::keep, "a career's activation is never replaced");

    // Virtual boxes share one host list; the label it holds now decides.
    tl::Loaded v = load(boxes, VHOST, HOST, "cs_f1_2021");
    expect(v.kind == tl::Kind::box && v.box == "cs_f1_2021", "the virtual host list is the box it holds");
    expect(load(boxes, VHOST, HOST, "").kind == tl::Kind::unread, "the virtual host list holding nothing is unread");
    expect(load(boxes, VHOST, HOST, "not_a_box").kind == tl::Kind::unlisted, "the virtual host list holding an unknown label is unlisted");

    // The entrant selection against the loaded list.
    std::vector<uint32_t> stock{team(8), team(1), team(2), team(0), team(104, 1)};
    expect(!tl::selection_in_list(stock, team(596)), "the 2015 Mercedes is not in the game's own list");
    expect(tl::selection_in_list(stock, team(8)) && tl::selection_in_list(stock, team(104, 1)), "a team of the list is in it");
    expect(!tl::selection_in_list(stock, team(104, 0)), "the same id of another kind is another team");
    expect(tl::selection_in_list({}, team(596)), "an unread list refuses nothing");
    expect(tl::same_team(team(8), team(8) | 0x0100), "bits outside the id and the kind do not make another team");

    // A Grand Prix > Continue presses no box: the saved weekend's list names the content.
    tl::Loaded gp = tl::loaded_box(boxes, "sw_f1_2015", "", HOST, VHOST, "");
    f = tl::follow_grand_prix(gp, SW15, "");
    expect(f.action == tl::Action::follow && f.box == "sw_f1_2015" &&
               f.line == "[apply] Grand Prix content follows the loaded list: " + std::string(SW15) +
                             " (box sw_f1_2015; active was nothing)",
           "a continued Grand Prix with nothing active follows its box's list");
    f = tl::follow_grand_prix(load(boxes, "cs_f1_2021", HOST), "a.b/cs_2021", std::string(tl::CONTENT) + SW15);
    expect(f.action == tl::Action::follow && f.box == "cs_f1_2021", "a continued Grand Prix of another box replaces the active content");
    expect(tl::follow_grand_prix(gp, SW15, std::string(tl::CONTENT) + SW15).action == tl::Action::keep,
           "a Grand Prix whose box is already active keeps it, as after a New Grand Prix press");
    expect(tl::follow_grand_prix(gp, SW15, "career abc").action == tl::Action::keep, "a career's activation is never replaced");
    expect(tl::follow_grand_prix(load(boxes, "ms_f1_2015", HOST), "", std::string(tl::CONTENT) + SW15).action == tl::Action::keep,
           "a box without a package leaves the active content");
    expect(tl::follow_grand_prix(tl::loaded_box(boxes, "f1_2025", "", HOST, VHOST, ""), "", std::string(tl::CONTENT) + SW15).action ==
               tl::Action::keep,
           "a list no box names leaves the active content");
    expect(tl::follow_grand_prix(tl::loaded_box(boxes, VHOST, "", HOST, VHOST, ""), "", std::string(tl::CONTENT) + SW15).action ==
               tl::Action::keep,
           "the virtual host list without a label leaves the active content");
    expect(tl::follow_grand_prix(tl::loaded_box(boxes, VHOST, "", HOST, VHOST, "cs_f1_2021"), "a.b/cs_2021", "").action ==
               tl::Action::follow,
           "the virtual host list follows the box it holds");

    // The lap's team label.
    tl::TeamPick p = tl::team_pick("sw15_mercedes", true, team(596), true, team(596), "");
    expect(p.label == "sw15_mercedes" && p.from == tl::TeamFrom::selection && !p.stale, "the selection stands when the lap stats name the same team");
    p = tl::team_pick("sw15_mercedes", true, team(596), false, 0, "");
    expect(p.label == "sw15_mercedes" && p.from == tl::TeamFrom::selection, "the selection stands when the lap has no stats");
    p = tl::team_pick("sw15_mercedes", true, team(596), true, team(8), "mclaren");
    expect(p.label == "mclaren" && p.from == tl::TeamFrom::stats && p.stale, "the label comes from the loaded team when the selection names another");
    p = tl::team_pick("", false, 0, true, team(8), "mclaren");
    expect(p.label == "mclaren" && p.from == tl::TeamFrom::stats && !p.stale, "no selection takes the loaded team's label");
    p = tl::team_pick("sw15_mercedes", true, team(596), true, team(8), "");
    expect(p.label.empty() && p.from == tl::TeamFrom::none && p.stale, "a stale selection and an unlabelled team give no label");
    p = tl::team_pick("", false, 0, false, 0, "");
    expect(p.label.empty() && p.from == tl::TeamFrom::none && !p.stale, "nothing known gives no label");

    printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
