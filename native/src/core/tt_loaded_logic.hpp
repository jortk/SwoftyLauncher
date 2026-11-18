#pragma once

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

// What a Time Trial session really loaded, as pure decisions the host test covers (tt_loaded_test.cpp).
//
// The board a lap is filed under has two parts of ours: the content (the box) and the car's team label. Both
// used to come from the front end: the content from the last box pressed in the picker, the team from the
// player's entrant selection. The game can start a Time Trial without either being written again: from a
// session's results screen its own change of car and track goes StateUnloadSession -> StateLoadSession with
// another setup mode and entrant list, no front end and no picker. A press of SWOFTY 2015 followed by that
// path into the game's own cars filed F1 25 McLaren laps under the 2015 board with the 2015 Mercedes label.
//
// So at each Time Trial StateLoadSession the list the session loads decides the box (loaded_box), and the box
// becomes the active content when it is not already (follow): the same call a picker press makes. A list no
// box can be named for gets no board. At each lap the team the game's own lap stats name decides the label
// when it is not the team the entrant selection holds (team_pick).
//
// A Grand Prix > Continue presses no box either: the saved weekend's list is the one the session loads, and a
// registered box's list makes its package the active content when it is not (follow_grand_prix). A stock, unlisted
// or unread list leaves the active content as it is.

namespace runtime::tt_loaded {

constexpr const char *STOCK_BOX = "stock box ", *CONTENT = "content ", *CAREER = "career ";

struct BoxLists {
    std::string entrant_list, time_trial;
};

enum class Kind : uint8_t {
    unread,   // the session's list could not be named: nothing is concluded from it
    box,      // a registered box's Grand Prix or Time Trial list
    stock,    // a list of one of the game's own setup modes
    unlisted, // a list of the host setup mode that no registered box names
};

struct Loaded {
    Kind kind = Kind::unread;
    // What a picker press of that box hands the scheduler: the box's list as the picker shows it, or the game's
    // own box name.
    std::string box;
};

// The game's own boxes are pressed by the name of their Grand Prix list, while a Time Trial loads the setup
// mode's custom team list (f1_2025 pressed, f1_custom_team of setup mode f1 loaded).
inline std::string stock_box(std::string_view setup_mode, std::string_view list)
{
    if (setup_mode == "f1")
        return "f1_2025";
    if (setup_mode == "f1_26")
        return "f1_2026";
    return std::string(list);
}

// `virtual_host` is the list a virtual box's entrants are read into and `virtual_label` the box label it holds now.
inline Loaded loaded_box(const std::vector<BoxLists> &boxes, std::string_view list, std::string_view setup_mode,
                         std::string_view host_setup_mode, std::string_view virtual_host, std::string_view virtual_label)
{
    if (!virtual_host.empty() && list == virtual_host)
        list = virtual_label;
    if (list.empty())
        return {};
    for (const auto &b : boxes)
        if (b.entrant_list == list || (!b.time_trial.empty() && b.time_trial == list))
            return {Kind::box, std::string(list)};
    if (setup_mode.empty() || setup_mode == "?")
        return {};
    if (setup_mode != host_setup_mode)
        return {Kind::stock, stock_box(setup_mode, list)};
    return {Kind::unlisted, std::string(list)};
}

// The scheduler's label of an active box: the package a box's list belongs to, else the box as a stock box.
inline std::string active_label(std::string_view box, std::string_view package)
{
    return package.empty() ? STOCK_BOX + std::string(box) : CONTENT + std::string(package);
}

inline std::string board_content(std::string_view package)
{
    return package.empty() ? std::string("game/f1_2025") : std::string(package);
}

enum class Action : uint8_t { keep, follow, no_board };

struct Follow {
    Action action = Action::keep;
    std::string box, line;
};

// `package` is the content package of loaded.box ("" for none) and `active` the scheduler's active label.
inline Follow follow(const Loaded &loaded, std::string_view package, std::string_view active, std::string_view pressed)
{
    std::string was = pressed.empty() ? std::string("none") : std::string(pressed);
    if (loaded.kind == Kind::unread || active.rfind(CAREER, 0) == 0)
        return {};
    if (loaded.kind == Kind::unlisted)
        return {Action::no_board, loaded.box,
                "[boards] no board for this Time Trial session: it loads the list " + loaded.box +
                    " that no box names (box pressed was " + was + "); its laps are written without a board"};
    if (active == active_label(loaded.box, package))
        return {};
    return {Action::follow, loaded.box,
            "[boards] content follows the loaded list: " + board_content(package) + " (box " + loaded.box +
                "; box pressed was " + was + ", active was " + (active.empty() ? std::string("nothing") : std::string(active)) + ")"};
}

// `package` is the content package of loaded.box ("" for none) and `active` the scheduler's active label.
inline Follow follow_grand_prix(const Loaded &loaded, std::string_view package, std::string_view active)
{
    if (loaded.kind != Kind::box || package.empty() || active.rfind(CAREER, 0) == 0 ||
        active == active_label(loaded.box, package))
        return {};
    return {Action::follow, loaded.box,
            "[apply] Grand Prix content follows the loaded list: " + std::string(package) + " (box " + loaded.box +
                "; active was " + (active.empty() ? std::string("nothing") : std::string(active)) + ")"};
}

// A team reference is a stat id dword: the id in the high word, its kind in the low byte.
constexpr bool same_team(uint32_t a, uint32_t b)
{
    return (a >> 16) == (b >> 16) && (a & 0xff) == (b & 0xff);
}

// Whether the entrant selection's team is one of the loaded list's teams. A list that could not be read (no
// teams) vouches for nothing and refuses nothing.
inline bool selection_in_list(const std::vector<uint32_t> &list_teams, uint32_t selected)
{
    if (list_teams.empty())
        return true;
    for (uint32_t t : list_teams)
        if (same_team(t, selected))
            return true;
    return false;
}

enum class TeamFrom : uint8_t { none, selection, stats };

struct TeamPick {
    std::string label;
    TeamFrom from = TeamFrom::none;
    bool stale = false;
};

// The lap's team label. The entrant selection's label stands while the game's lap stats name the same team or
// name none; when they name another team the selection is stale, and the label is the stats team's (none when
// that team has no label).
inline TeamPick team_pick(const std::string &selection, bool have_selection_ref, uint32_t selection_ref, bool stats_ok,
                          uint32_t stats_ref, const std::string &stats_label)
{
    bool same = have_selection_ref && stats_ok && same_team(selection_ref, stats_ref);
    if (!stats_ok || (same && !selection.empty()))
        return {selection, selection.empty() ? TeamFrom::none : TeamFrom::selection, false};
    return {stats_label, stats_label.empty() ? TeamFrom::none : TeamFrom::stats, !selection.empty() && !same};
}

}
