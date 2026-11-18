// SPDX-License-Identifier: MIT
#pragma once
#include <windows.h>

#include <atomic>
#include <cstdint>
#include <string>
#include <string_view>
#include <swofty/mixin.hpp>

// Addon boxes on the era picker: the box row shown after Grand Prix or Time Trial (and the split-screen
// game mode select) gets a second row, Addons, under the game's own Built In row. Register a box from
// the init check of your addon (the function you pass to SWOFTY_ADDON), before the core's mixin commit;
// the core then builds the box in the Addons row every time the picker is built, presses it like a stock
// box and narrows the host setup mode to its entrant list when it is pressed, so Time Trial loads only
// that list's cars. A box can name a second, smaller list for Time Trial: a Time Trial session holds at
// most 23 cars of the list it was picked with, and the core leaves a box off the Time Trial picker, with
// a log line, when its list holds more.
//
// The data side comes from your database operations. The entrant list must exist (its
// entrants.entrant_lists row, its teams and drivers) and must be in the entrant lists of the core's host
// setup mode swofty_f1: list-append it onto the swofty_f1 entrant setup mode's entrant_lists and
// selectable_entrant_lists, and add the game_setup.scenarios row the game checks it against. The core
// checks every box each time it builds the picker and skips, with one log line, a box whose entrant list
// the screen's mode does not offer under swofty_f1, so a box for a list that another addon or a missing
// database operation leaves out never reaches the game.
//
// Feature detection: the table is read only when the running swofty_core.dll exports SwoftyCoreEra, so
// an addon built against this header also loads on an older core and sees available() false there.

namespace swofty::era {

enum class Result { ok, late, unavailable, invalid };

enum class Only : uint32_t { any = SWOFTY_ERA_ONLY_ANY, grand_prix = SWOFTY_ERA_ONLY_GRAND_PRIX, time_trial = SWOFTY_ERA_ONLY_TIME_TRIAL };

// One box. entrant_list is the entrant list the box selects (its press raises entrant_list_selected with
// it). title is the lng_* key the box shows; empty means lng_<entrant_list>. image is the texture path
// the box shows, e.g. "#overrides.home_screen.tiles_solo_6x6[cs_f1_2016]"; empty means
// #overrides.home_screen.tiles_solo_6x6[<entrant_list>], an overrides entry named after the list. Boxes
// show in ascending order, then in registration order. time_trial is the entrant list the box selects on
// the Time Trial picker, set up like entrant_list (in swofty_f1's entrant lists); empty means
// entrant_list; a core before launcher 1.6.2 ignores it. only keeps the box off one picker: Only::time_trial
// leaves it off the Grand Prix picker, Only::grand_prix off the Time Trial picker, Only::any (the default)
// shows it on both. A core without it would show the box on both, so add_box returns Result::unavailable
// for an only other than Only::any there (see only_supported()). Strings are copied.
struct Box {
    std::string_view entrant_list;
    std::string_view title = {};
    std::string_view image = {};
    int32_t order = 0;
    std::string_view time_trial = {};
    Only only = Only::any;
};

namespace detail {

inline const SwoftyEra *table()
{
    if (!api || api->version < 8)
        return nullptr;
    static std::atomic<const SwoftyEra *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    if (!core || !GetProcAddress(core, "SwoftyCoreEra") || !api->era)
        return nullptr;
    found = api->era;
    return api->era;
}

inline Result result(int r)
{
    switch (r) {
    case SWOFTY_ASSET_OK:
        return Result::ok;
    case SWOFTY_ASSET_LATE:
        return Result::late;
    case SWOFTY_ASSET_UNAVAILABLE:
        return Result::unavailable;
    default:
        return Result::invalid;
    }
}

}

// The core builds addon boxes and its era picker hooks are not off.
inline bool available()
{
    auto t = detail::table();
    return t && t->available();
}

// The running core honours Box::only.
inline bool only_supported()
{
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    return detail::table() && core && GetProcAddress(core, "SwoftyCoreEraOnly");
}

// Register a box. ok: queued for the next mixin commit (or live from the next build of the picker when
// the hooks are already installed); invalid: a bad entrant list name, an over-long string or a second box
// for the same entrant list (the log says which); unavailable: an older core, or the era picker hooks
// could not be installed on this build.
inline Result add_box(const Box &box)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    if (box.only != Only::any && !only_supported())
        return Result::unavailable;
    std::string list(box.entrant_list), title(box.title), image(box.image), tt(box.time_trial);
    SwoftyEraBox d{sizeof d, list.c_str(), title.c_str(), image.c_str(), box.order, tt.c_str(), uint32_t(box.only)};
    return detail::result(t->add_box(&d));
}

// Boxes registered so far by every addon.
inline int count()
{
    auto t = detail::table();
    return t ? t->count() : 0;
}

}
