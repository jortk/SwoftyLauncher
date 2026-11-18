// SPDX-License-Identifier: MIT
#pragma once
#include <windows.h>

#include <cstdint>

// Whether the game is paused. pause_reasons() is the game's pause reason mask, one bit per reason that
// holds the pause (the pause menu, a focus loss and so on); paused() is true while any bit is set. The
// core sets the mask from the game's own pause calls, and fires the `pause` event when it goes from 0 to
// non-zero and `resume` when it returns to 0 (SWOFTY_EVENT_PAUSE and SWOFTY_EVENT_RESUME).
//
// Feature detection: the query is read only when the running swofty_core.dll exports
// SwoftyCorePauseReasons, so an addon built against this header also loads on an older core and sees
// available() false and a mask of 0 there. Any thread.

namespace swofty::view {

namespace detail {

using PauseFn = uint32_t (*)();

inline PauseFn pause_fn()
{
    static PauseFn fn = [] {
        HMODULE core = GetModuleHandleW(L"swofty_core.dll");
        return core ? reinterpret_cast<PauseFn>(GetProcAddress(core, "SwoftyCorePauseReasons")) : nullptr;
    }();
    return fn;
}

}

// The running core reports the pause reasons.
inline bool available()
{
    return detail::pause_fn() != nullptr;
}

// The pause reason mask: 0 while the game runs.
inline uint32_t pause_reasons()
{
    auto fn = detail::pause_fn();
    return fn ? fn() : 0;
}

inline bool paused()
{
    return pause_reasons() != 0;
}

}
