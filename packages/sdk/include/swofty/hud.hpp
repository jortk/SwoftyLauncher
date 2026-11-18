// SPDX-License-Identifier: MIT
#pragma once
#include <windows.h>

#include <atomic>
#include <string>
#include <string_view>
#include <swofty/mixin.hpp>

// The race HUD's own notifications. toast() shows your text in the central race notification, the one
// the game uses for DRS enabled or a fastest lap, drawn by the game's UI in the game's style. It shows
// during a session on track (race, qualifying, practice, time trial); a toast posted in the menus is not
// drawn. toast() queues from any thread; the core posts it on the game's flow thread.
//
// Feature detection: the table is read only when the running swofty_core.dll exports SwoftyCoreHud, so
// an addon built against this header also loads on an older core and sees available() false there. The
// table is versioned (SWOFTY_HUD_VERSION); version 1 holds available, toast and set, version 2 adds the
// registered leaf table that swofty/hud_leaves.hpp wraps.

namespace swofty::hud {

enum class Result { ok, late, unavailable, invalid };

namespace detail {

inline const SwoftyHud *table()
{
    static std::atomic<const SwoftyHud *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<const SwoftyHud *(*)()>(GetProcAddress(core, "SwoftyCoreHud")) : nullptr;
    const SwoftyHud *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
}

inline Result result(int r)
{
    return r == SWOFTY_ASSET_OK ? Result::ok
           : r == SWOFTY_ASSET_LATE ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
                                           : Result::invalid;
}

}

// The core found the game's message system on this build.
inline bool available()
{
    auto t = detail::table();
    return t && t->available();
}

// Queue a toast. text is the main line, detail an optional second line. seconds is rounded to 2, 3, 4,
// 5, 6, 8, 10, 12, 15, 20 or 30. icon is empty for the default or a race flag icon: generic, diagonal,
// black, checkpoint, chequered, fast_lap, formation_lap, pp_icon, red, safety_car, virtual_safety_car,
// virtual_safety_car_green, flag_rival_won. ok once queued; invalid for empty text or an unknown icon;
// unavailable on an older core or a build where the message system did not resolve. UTF-8, each string
// capped at 200 bytes.
inline Result toast(std::string_view text, std::string_view detail = {}, float seconds = 4.0f,
                    std::string_view icon = {})
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(
        t->toast(std::string(text).c_str(), std::string(detail).c_str(), seconds, std::string(icon).c_str()));
}

// Write one HUD data leaf, e.g. "ui_menu_data.hud_lap_description_string". Only leaves under
// ui_menu_data.hud_* and view_model_data.hud* are accepted (invalid otherwise). The write is queued and
// applied on the flow thread; a later write to the same path replaces a queued one.
inline Result set(std::string_view path, std::string_view text)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    std::string v(text);
    return detail::result(t->set(std::string(path).c_str(), SWOFTY_HUD_TEXT, v.c_str()));
}

inline Result set(std::string_view path, bool value)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    int32_t v = value;
    return detail::result(t->set(std::string(path).c_str(), SWOFTY_HUD_BOOL, &v));
}

inline Result set(std::string_view path, int32_t value)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->set(std::string(path).c_str(), SWOFTY_HUD_INT, &value));
}

inline Result set(std::string_view path, const char *text)
{
    return set(path, std::string_view(text ? text : ""));
}

inline const char *name(Result r)
{
    return r == Result::ok ? "ok" : r == Result::late ? "late" : r == Result::unavailable ? "unavailable" : "invalid";
}

}
