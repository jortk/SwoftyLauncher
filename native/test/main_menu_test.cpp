// Host unit test for the main menu logic (main_menu_logic.hpp): the row layout, the removed entries, the status
// strip text, the home menu state file and the Time Trial warning rule. run_main_menu_test.sh builds and runs it.
#include "../src/core/main_menu_logic.hpp"

#include <cmath>
#include <cstdio>
#include <string>
#include <vector>

namespace mm = runtime::main_menu;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static std::vector<std::string> retail()
{
    return {"story", "career", "mod_menu", "f1_world", "divider", "local_multiplayer", "pro_leagues", "player_hub",
            "theatre"};
}

static std::string at(const mm::Plan &p, int row, int col)
{
    for (auto &c : p.cards)
        if (c.row == row && c.column == col)
            return c.id;
    return "";
}

int main()
{
    auto p = mm::layout(retail(), true, true);
    expect(at(p, 0, 0) == "story" && at(p, 0, 1) == "career", "Braking Point and Career lead the first row");
    expect(at(p, 1, 0) == mm::ID_GRAND_PRIX && at(p, 1, 1) == mm::ID_TIME_TRIAL, "Grand Prix and Time Trial share row 2");
    expect(at(p, 1, 2) == "local_multiplayer", "the Split Screen card (the stock Local Multiplayer item) ends row 2");
    expect(at(p, 2, 0) == "theatre" && p.cards[5].wide, "Theatre is a full-width row under them");
    expect(at(p, 3, 0) == mm::ID_SHOWROOM, "Showroom follows Theatre");
    expect(at(p, 4, 0) == mm::ID_CUSTOMISATION && p.cards[7].wide, "Customisation follows Showroom, full width");
    expect(at(p, 5, 0) == mm::ID_DIVIDER, "a divider sits between the cards and Addon Settings");
    expect(at(p, 6, 0) == "mod_menu" && p.cards.back().id == "mod_menu", "Addon Settings is last");
    for (auto &c : p.cards)
        expect(c.id != "f1_world" && c.id != "pro_leagues" && c.id != "player_hub", "removed entry gone: " + c.id);
    expect(!mm::removed("local_multiplayer") && mm::removed("f1_world") && mm::removed("pro_leagues") && mm::removed("player_hub"),
           "Local Multiplayer is kept (it is the only way to Split Screen); F1 World, League Racing and Player Hub go");
    expect(p.dropped.size() == 4, "the three removed entries and the divider are reported dropped");
    auto bare = mm::layout({"career", "f1_world", "theatre", "mod_menu"}, true, true);
    expect(at(bare, 1, 2) == "", "no Split Screen card when the game builds no Local Multiplayer tile");

    auto q = mm::layout({"career", "home_navigate_to_challenge_event", "theatre", "mod_menu", "f1_world"}, false, false);
    expect(at(q, 0, 1) == "home_navigate_to_challenge_event", "the challenge tile joins the first row");
    expect(at(q, 4, 0) == "mod_menu", "no Showroom or Customisation card when their routes are unavailable");
    auto c = mm::layout({"career", "theatre", "mod_menu", "f1_world"}, false, true);
    expect(at(c, 3, 0) == mm::ID_CUSTOMISATION && at(c, 5, 0) == "mod_menu", "a Customisation card without the Showroom takes the Showroom's row");
    auto no_c = mm::layout(retail(), true, false);
    for (auto &card : no_c.cards)
        expect(card.id != mm::ID_CUSTOMISATION, "no Customisation card when its routes are not in the build");
    auto r = mm::layout({"career", "future_tile", "theatre", "mod_menu", "f1_world"}, true, true);
    expect(at(r, 0, 1) == "future_tile", "an unknown stock entry stays in the first row");
    auto none = mm::layout({"career", "theatre", "mod_menu"}, true, true);
    expect(at(none, 1, 0) == "" && at(none, 1, 1) == "", "no Grand Prix or Time Trial card without the F1 World tile");

    auto s = mm::parse_state(
        R"({"schema":1,"launcher_version":"1.8.37","discord":{"signed_in":true,"name":"swofty","avatar":"C:/a.png"},)"
        R"("video":{"title":"T","url":"https://youtu.be/x","thumbnail":"C:/t.jpg"}})");
    expect(s.loaded && s.version == "1.8.37", "the state file loads");
    expect(mm::version_line(s) == "Swofty Launcher 1.8.37", "version line");
    expect(mm::discord_line(s) == "Signed in to Discord as swofty", "signed in line");
    expect(mm::has_avatar(s) && mm::has_video(s), "avatar and video present");
    expect(mm::watch_line("RB") == "Press RB to watch", "watch line");
    expect(std::string(mm::template_for("story")) == mm::TEMPLATE_TOP_LEFT && std::string(mm::template_for("career")) == mm::TEMPLATE_TOP_RIGHT &&
               std::string(mm::template_for("swofty_grand_prix")) == mm::TEMPLATE_MODES_LEFT && std::string(mm::template_for("swofty_time_trial")) == mm::TEMPLATE_MODES_RIGHT &&
               std::string(mm::template_for("local_multiplayer")) == mm::TEMPLATE_MODES_THIRD &&
               std::string(mm::template_for("theatre")) == mm::TEMPLATE_WIDE_A && std::string(mm::template_for("swofty_showroom")) == mm::TEMPLATE_WIDE_B &&
               std::string(mm::template_for("mod_menu")) == mm::TEMPLATE_WIDE_C && std::string(mm::template_for("f1_world")).empty(),
           "every card has its row template and unknown ids have none");
    expect(std::string(mm::template_for("theatre", false)) == mm::TEMPLATE_DEFAULT && std::string(mm::template_for("swofty_showroom", false)) == mm::TEMPLATE_DEFAULT &&
               std::string(mm::template_for("local_multiplayer", false)) == mm::TEMPLATE_DEFAULT &&
               std::string(mm::template_for("swofty_customisation", false)) == mm::TEMPLATE_DEFAULT &&
               std::string(mm::template_for("career", false)).empty() && std::string(mm::template_for("swofty_divider")) == mm::TEMPLATE_DIVIDER,
           "without rows Theatre, Showroom, Customisation and Split Screen take the default large tile; with rows the divider has its slot");
    expect(std::string(mm::CUSTOMISATION_TITLE) == "CUSTOMISATION" && std::string(mm::CUSTOMISATION_ROUTE) == mm::ID_CUSTOMISATION &&
               std::string(mm::CUSTOMISATION_DESCRIPTION).find("helmet") != std::string::npos,
           "the Customisation card is titled for the game's screen and fires its own launcher route");
    expect(std::string(mm::SPLIT_SCREEN_TITLE) == "SPLIT SCREEN" && !std::string(mm::SPLIT_SCREEN_DESCRIPTION).empty(),
           "the Local Multiplayer item is titled Split Screen");
    expect(mm::with_separators(1234) == "1,234" && mm::with_separators(999) == "999" && mm::with_separators(1000000) == "1,000,000" &&
               mm::with_separators(12) == "12" && mm::with_separators(100000) == "100,000",
           "thousands separators");
    expect(mm::online_line(1234) == "1,234 currently playing" && mm::online_line(1) == "1 currently playing" &&
               mm::online_line(0).empty() && mm::online_line(-5).empty(),
           "the online line: separators, singular count, hidden for 0 or less");
    expect(mm::offline_text(0) == "Launched with Swofty Launcher" && mm::offline_text(2500) == "Launched with Swofty Launcher\n2,500 currently playing",
           "the Launched with panel text gains the online line");
    expect(mm::parse_state(R"({"schema":1,"online":321,"discord":{}})").online == 321 && mm::parse_state(R"({"schema":1,"discord":{}})").online == 0,
           "the state file's online field");
    {
        auto list = mm::scene_to_px(-8.6, 3.4, 1914, 1079);
        auto note = mm::scene_to_px(-8.6, -1.77, 1914, 1079);
        auto logo = mm::scene_to_px(-8.65, 3.7, 1914, 1079);
        expect(list.x > 92 && list.x < 104, "the list's left edge (x -8.6) lands at the screenshot's 98 px");
        expect(note.y > 700 && note.y < 722, "the description (y -1.77) lands at the screenshot's 713 px");
        expect(logo.y > 160 && logo.y < 176, "the logo's bottom edge (y 3.7) lands at the screenshot's 168 px");
        auto small = mm::scene_to_px(-8.6, 3.4, 1366, 768);
        expect(small.x / 1366.0f > 0.0481f && small.x / 1366.0f < 0.0661f, "the left edge keeps its place relative to the frame at 1366x768");
    }
    {
        auto small = mm::plan_card(1.0f, 100);
        expect(small.width == 480 && small.height == 150 + 24, "a short title keeps the minimum width and the thumbnail sits in an even 12 px inset");
        auto big = mm::plan_card(1.0f, 300);
        expect(big.width == 12 + 300 + 28 + big.thumb_w + 12 && big.width > 480, "a longer title widens the card");
        auto capped = mm::plan_card(1.0f, 900);
        expect(capped.text_max == 420 && capped.width == 12 + 420 + 28 + capped.thumb_w + 12, "the text block is capped at 420 px");
        auto scaled = mm::plan_card(2.0f, 100);
        expect(scaled.width == 960 && scaled.thumb_h == 300, "the card scales with the screen height");
        expect(std::abs(big.thumb_w / big.thumb_h - 16.0 / 9.0) < 1e-4, "the thumbnail is 16:9");
        auto br = mm::scene_to_px(mm::CARD_RIGHT_X, mm::CARD_BOTTOM_Y, 1908, 1028);
        expect(br.x > 1795 && br.x < 1805 && br.y > 950 && br.y < 962, "the card's right and bottom anchors land at (1800, 956) at 1908x1028");
        auto deck = mm::scene_to_px(mm::CARD_RIGHT_X, mm::CARD_BOTTOM_Y, 1280, 800);
        auto wide = mm::scene_to_px(mm::CARD_RIGHT_X, mm::CARD_BOTTOM_Y, 1280, 720);
        expect(std::abs(deck.x - wide.x) < 0.01f && std::abs(deck.y - (wide.y + 40)) < 0.01f,
               "at 16:10 the card keeps its 16:9 place, moved down by the 40 px band above it (1280x800)");
        expect(deck.x < 1280 && deck.y < 760, "at 16:10 the card ends inside the frame and above the band's bottom edge");
        expect(std::abs(mm::panel_scale(1280, 800) - mm::panel_scale(1280, 720)) < 1e-6f &&
                   std::abs(mm::panel_scale(1908, 1028) - 1.0f) < 1e-6f,
               "the panel's size follows the band, not the frame height");
        auto ultra = mm::scene_to_px(mm::CARD_RIGHT_X, mm::CARD_BOTTOM_Y, 3440, 1440);
        auto ultra_band = mm::ui_band(3440, 1440);
        expect(ultra_band.top == 0 && ultra_band.left > 0 && ultra.x < ultra_band.left + ultra_band.width,
               "on a 21:9 frame the band is the full height and the card stays inside it");
    }
    expect(mm::panel_due(true, true, true, false, true), "the panel is due on the open home screen once prepared");
    expect(!mm::panel_due(false, true, true, false, true) && !mm::panel_due(true, false, true, false, true) &&
               !mm::panel_due(true, true, false, false, true) && !mm::panel_due(true, true, true, true, true) &&
               !mm::panel_due(true, true, true, false, false),
           "the panel is not due before the tile patch, off the home screen, without a state file, after it disabled itself, or before it is prepared");
    expect(!mm::panel_too_slow(1.0) && !mm::panel_too_slow(4.0) && mm::panel_too_slow(4.1), "the draw budget is a few ms");
    expect(std::string(mm::TT_BODY).find("will not be saved") != std::string::npos && std::string(mm::TT_BODY).find("OK to continue") != std::string::npos,
           "the warning says laps are not saved and that OK continues");
    auto out = mm::parse_state(R"({"schema":1,"launcher_version":"1.8.37","discord":{"signed_in":false,"name":""}})");
    expect(mm::discord_line(out) == "Not signed in to Discord", "signed out line");
    expect(!mm::has_avatar(out) && !mm::has_video(out), "signed out has no avatar, no video card without a video");
    expect(!mm::parse_state("not json").loaded && !mm::parse_state(R"({"schema":2})").loaded, "bad state is not loaded");

    expect(mm::time_trial_warning_due(out, true, false), "signed out Time Trial warns");
    expect(!mm::time_trial_warning_due(out, true, true), "the warning shows once per session");
    expect(!mm::time_trial_warning_due(out, false, false), "Grand Prix does not warn");
    expect(!mm::time_trial_warning_due(s, true, false), "signed in does not warn");
    expect(!mm::time_trial_warning_due(mm::State{}, true, false), "no state file, no warning");
    expect(mm::djb2("") == 5381 && mm::djb2("a") == 5381 * 33 + 'a', "djb2 matches the game's string hash");
    expect(mm::open_request_json("https://youtu.be/a") == R"({"url":"https://youtu.be/a"})", "open request json");
    expect(mm::open_request_json("a\"b") == R"({"url":"a\"b"})", "open request json escapes quotes");
    expect(std::string(mm::press_route(mm::Mode::time_trial)) == "swofty_time_trial" &&
               std::string(mm::press_route(mm::Mode::grand_prix)) == "home_to_activity" &&
               std::string(mm::press_route(mm::Mode::none)).empty(),
           "the Time Trial card has its launcher route; the Grand Prix card fires the game's home_to_activity");
    expect(std::string(mm::return_command(mm::Mode::time_trial)) == "time_trial" &&
               std::string(mm::return_command(mm::Mode::grand_prix)) == "solo_gp" && std::string(mm::return_command(mm::Mode::none)).empty(),
           "the session-end hand-over raises the command of the card that started the session");
    expect(mm::driven_mode(mm::Mode::grand_prix, "time_trial") == mm::Mode::time_trial &&
               std::string(mm::return_command(mm::driven_mode(mm::Mode::grand_prix, "time_trial"))) == "time_trial",
           "a Grand Prix card press that ends in a Time Trial (the container's Time Trial tab) hands over to the Time Trial picker");
    expect(mm::driven_mode(mm::Mode::time_trial, "grand_prix") == mm::Mode::grand_prix &&
               std::string(mm::return_command(mm::driven_mode(mm::Mode::time_trial, "grand_prix"))) == "solo_gp",
           "a Time Trial card press that ends in a Grand Prix hands over to the Grand Prix");
    expect(mm::driven_mode(mm::Mode::grand_prix, "grand_prix") == mm::Mode::grand_prix &&
               mm::driven_mode(mm::Mode::time_trial, "time_trial") == mm::Mode::time_trial,
           "a session of the pressed card's own mode keeps the card");
    expect(mm::driven_mode(mm::Mode::grand_prix, "") == mm::Mode::grand_prix && mm::driven_mode(mm::Mode::grand_prix, "unknown") == mm::Mode::grand_prix &&
               mm::driven_mode(mm::Mode::time_trial, "unknown") == mm::Mode::time_trial,
           "an unread or unknown game mode keeps the card that was pressed");
    expect(mm::driven_mode(mm::Mode::grand_prix, "split_screen") == mm::Mode::none && mm::driven_mode(mm::Mode::time_trial, "f1_world") == mm::Mode::none,
           "a session of another mode from the container's tabs raises no card command");
    expect(mm::driven_mode(mm::Mode::none, "time_trial") == mm::Mode::none && mm::driven_mode(mm::Mode::none, "grand_prix") == mm::Mode::none,
           "a session no card started never arms a hand-over");
    {
        auto route = mm::Back::route;
        expect(std::string(mm::back_replacement(route, mm::djb2("f1_world_activity_container"), "f1_world")) == "swofty_back_home" &&
                   std::string(mm::back_replacement(route, mm::djb2("f1_world_activity_container"), "home")) == "swofty_back_home",
               "the container's Back (to the hub or home) becomes the launcher's route home");
        expect(std::string(mm::back_replacement(route, mm::djb2("fe_showroom"), "back_1")) == "swofty_home",
               "the Showroom's Back becomes the launcher's route home");
        expect(mm::back_replacement(route, mm::djb2("fe_f1_world"), "back_1") == nullptr &&
                   mm::back_replacement(route, mm::djb2("f1_world_activity_container"), "tt_track_select") == nullptr &&
                   mm::back_replacement(route, mm::djb2("fe_showroom"), "showroom_f1") == nullptr,
               "other routes stay the game's");
        expect(std::string(mm::back_replacement(mm::Back::home, mm::djb2("f1_world_activity_container"), "f1_world")) == "home" &&
                   mm::back_replacement(mm::Back::home, mm::djb2("fe_showroom"), "back_1") == nullptr,
               "the home variant fires the container's stock home route only");
        expect(mm::back_replacement(mm::Back::stock, mm::djb2("f1_world_activity_container"), "f1_world") == nullptr &&
                   mm::back_replacement(mm::Back::twostep, mm::djb2("f1_world_activity_container"), "f1_world") == nullptr,
               "the stock and two-step variants leave the route alone");
        expect(mm::back_replacement(route, mm::djb2("grand_prix_continue"), "back") == nullptr &&
                   mm::back_replacement(route, mm::djb2("grand_prix_continue"), "back_1") == nullptr,
               "the saves list's Back stays the game's (a front-end route home from it left a black screen)");
        expect(mm::saves_back_leaves_home(route, mm::djb2("grand_prix_continue"), "back", mm::Mode::grand_prix) &&
                   !mm::saves_back_leaves_home(route, mm::djb2("grand_prix_continue"), "back", mm::Mode::none) &&
                   !mm::saves_back_leaves_home(route, mm::djb2("grand_prix_continue"), "back", mm::Mode::time_trial) &&
                   !mm::saves_back_leaves_home(mm::Back::stock, mm::djb2("grand_prix_continue"), "back", mm::Mode::grand_prix) &&
                   !mm::saves_back_leaves_home(route, mm::djb2("grand_prix_continue"), "accept", mm::Mode::grand_prix),
               "after a Grand Prix card press the saves list's Back leads home through the container");
        uint32_t entry = mm::djb2("customisation_f1_life_entry");
        expect(std::string(mm::customisation_back(route, true, entry, "back")) == "swofty_back_home" &&
                   std::string(mm::customisation_back(route, true, entry, "back_1")) == "swofty_back_home" &&
                   std::string(mm::customisation_back(route, true, entry, "back_2")) == "swofty_back_home" &&
                   std::string(mm::customisation_back(mm::Back::home, true, entry, "back_1")) == "swofty_back_home",
               "after a Customisation card press the Customisation screen's Back (back from the main menu, to the hub or "
               "the news) becomes the launcher's route home");
        expect(mm::customisation_back(route, false, entry, "back_1") == nullptr &&
                   mm::customisation_back(route, false, entry, "back") == nullptr,
               "a Customisation screen the card did not open keeps the game's Back");
        expect(mm::customisation_back(route, true, entry, "to_customisation_car_category_select") == nullptr &&
                   mm::customisation_back(route, true, mm::djb2("customisation_driver_suit"), "back_1") == nullptr &&
                   mm::customisation_back(route, true, mm::djb2("fe_f1_world"), "back_1") == nullptr,
               "the Customisation screen's other routes and its sub-screens' Backs stay the game's");
        expect(mm::customisation_back(mm::Back::stock, true, entry, "back_1") == nullptr &&
                   mm::customisation_back(mm::Back::twostep, true, entry, "back_1") == nullptr,
               "the stock and two-step variants leave the Customisation screen's Back alone");
        expect(mm::back_replacement(route, entry, "back_1") == nullptr, "the Customisation swap is armed by the card only");
        expect(!mm::customisation_disarms("customisation_flow") && !mm::customisation_disarms("") &&
                   !mm::customisation_disarms("team_hq_flow") && mm::customisation_disarms("frontend_flow"),
               "a Customisation card press stays armed through the customisation flow and its help page and ends when the front end is entered");
        expect(mm::back_variant_of("", true) == mm::Back::route && mm::back_variant_of("", false) == mm::Back::stock &&
                   mm::back_variant_of("home", false) == mm::Back::home && mm::back_variant_of("twostep", true) == mm::Back::twostep &&
                   mm::back_variant_of("stock", true) == mm::Back::stock,
               "the default Back is the launcher's route when its data is in the build, else the game's");
    }
    expect(mm::hides_panel("home_to_activity") && mm::hides_panel("swofty_time_trial") && mm::hides_panel("swofty_grand_prix_saves") &&
               mm::hides_panel("swofty_customisation") && !mm::hides_panel("swofty_showroom"),
           "the card routes and the Customisation route hide the panel, the Showroom's route does not");
    expect(mm::recovery_for(mm::Expect::none, "frontend_flow", false) == mm::Recovery::none &&
               mm::recovery_for(mm::Expect::home, "frontend_flow", true) == mm::Recovery::none,
           "nothing to recover when nothing was expected or the main menu is up");
    expect(mm::recovery_for(mm::Expect::home, "f1_world_flow", false) == mm::Recovery::back_home_route &&
               mm::recovery_for(mm::Expect::container, "f1_world_flow", false) == mm::Recovery::back_home_route &&
               mm::recovery_for(mm::Expect::saves, "f1_world_flow", false) == mm::Recovery::back_home_route,
           "a screen that did not open while the f1_world flow is current takes the migration back to the main menu");
    expect(mm::recovery_for(mm::Expect::home, "customisation_flow", false) == mm::Recovery::back_home_route,
           "a main menu that did not open after the Customisation screen's Back takes the migration back too");
    expect(mm::recovery_for(mm::Expect::container, "frontend_flow", false) == mm::Recovery::open_home &&
               mm::recovery_for(mm::Expect::saves, "frontend_flow", false) == mm::Recovery::open_home &&
               mm::recovery_for(mm::Expect::home, "frontend_flow", false) == mm::Recovery::open_home,
           "a screen that did not open in the front-end flow reopens the main menu screen");
    expect(mm::recovery_for(mm::Expect::container, "frontend_flow", true) == mm::Recovery::none,
           "a main menu that is up is never replaced");
    expect(mm::migrate_failed("Screen fe_home failed to migrate because of 5") && mm::migrate_failed("No screen called x exists") && !mm::migrate_failed(""),
           "a failed migration is told from its error text");
    expect(mm::return_due(true, false, mm::Mode::time_trial, true) && !mm::return_due(true, false, mm::Mode::none, true) &&
               !mm::return_due(false, false, mm::Mode::time_trial, true) && !mm::return_due(true, true, mm::Mode::time_trial, true) &&
               !mm::return_due(true, false, mm::Mode::time_trial, false),
           "the hand-over is armed when a session that a card started has ended");
    expect(mm::migrate_logged(true, "", "frontend") && mm::migrate_logged(false, "f1_world", "frontend"), "a failed or flow-changing migrate is logged");
    expect(!mm::migrate_logged(false, "frontend", "frontend") && !mm::migrate_logged(false, "", "frontend"), "a migrate within the same flow is not logged");
    expect(mm::region_writable(true, 0x04, 0x1000, 0x1000, 0x1ffc, 4), "read-write memory inside its region is writable");
    expect(mm::region_writable(true, 0x08, 0x1000, 0x1000, 0x1000, 4) && mm::region_writable(true, 0x40, 0x1000, 0x1000, 0x1000, 4),
           "write-copy and execute-read-write are writable");
    expect(!mm::region_writable(true, 0x02, 0x1000, 0x1000, 0x1000, 4) && !mm::region_writable(true, 0x20, 0x1000, 0x1000, 0x1000, 4),
           "read-only and execute-read are not writable");
    expect(!mm::region_writable(false, 0x04, 0x1000, 0x1000, 0x1000, 4), "reserved memory is not writable");
    expect(!mm::region_writable(true, 0x104, 0x1000, 0x1000, 0x1000, 4), "a guard page is not writable");
    expect(!mm::region_writable(true, 0x04, 0x1000, 0x1000, 0x1ffe, 4), "bytes past the region are not writable");
    expect(!mm::region_writable(true, 0x04, 0x1000, 0x1000, 0, 4), "null is not writable");
    expect(mm::entry_point_write(0x146066c74, true) == mm::EntryPointWrite::write,
           "the F1 World entry point is cleared through the address the binding set resolved");
    expect(mm::entry_point_write(0, true) == mm::EntryPointWrite::unresolved && mm::entry_point_write(0, false) == mm::EntryPointWrite::unresolved,
           "a build whose set did not resolve g_Game_F1WorldEntryPoint never writes it (1.9.0 wrote the Steam offset on the EA build)");
    expect(mm::entry_point_write(0x146066c74, false) == mm::EntryPointWrite::unwritable,
           "a resolved address that is not writable memory is not written");
    printf("%s\n", fails ? "FAILED" : "all passed");
    return fails ? 1 : 0;
}
