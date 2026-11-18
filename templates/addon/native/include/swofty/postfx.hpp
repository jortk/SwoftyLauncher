// SPDX-License-Identifier: MIT
#pragma once
#include <windows.h>

#include <cstdint>

// Bytes from an addon's code to its post-process passes. set_data gives one of the addon's passes up to
// 64 KB, which the pass reads every frame from the next one on (data_size() and data_uint() in
// swofty/postfx.hlsli) until the code gives it others: per-frame values a manifest option can't carry, such
// as a camera, positions or lines to draw. The core copies the bytes, so the caller's buffer can go at once.
// A core without the call (older than the launcher this SDK came with) reads as unavailable.

namespace swofty::postfx {

// ok: the pass is running and reads the bytes. no_pass: the bytes are kept, but the pass isn't running (switched off,
// over its GPU budget, or not yet: from an addon's on-load the core may not have read the profile's passes, and the
// bytes wait for them), so code that would otherwise draw the same thing itself can go on doing so. denied: the call
// didn't come from that addon's own DLL. unknown_pass: this profile has no pass of that name for the addon (not in its
// manifest, or the build left it out, e.g. its shader didn't compile). Nothing is kept for denied, unknown_pass,
// invalid or too_big. Anything but ok means the pass draws nothing from these bytes this frame.
enum class Result { ok, no_pass, unavailable, invalid, too_big, denied, unknown_pass };

constexpr uint32_t data_max = 64 * 1024;

namespace detail {

using SetDataFn = int (*)(const char *addon_id, const char *pass, const void *data, uint32_t size);

inline SetDataFn set_data_fn()
{
    static SetDataFn fn = [] {
        HMODULE core = GetModuleHandleW(L"swofty_core.dll");
        return core ? reinterpret_cast<SetDataFn>(GetProcAddress(core, "SwoftyCorePostfxSetData")) : nullptr;
    }();
    return fn;
}

}

// The running core takes data for passes.
inline bool available()
{
    return detail::set_data_fn() != nullptr;
}

// Gives the pass `pass` (its "name" in the manifest) of the addon `addon_id` these bytes from the next frame on;
// size 0 takes them away. Call it from the addon's own DLL (swofty\addons\<addon_id>\...), any thread.
inline Result set_data(const char *addon_id, const char *pass, const void *data, uint32_t size)
{
    auto fn = detail::set_data_fn();
    if (!fn)
        return Result::unavailable;
    int r = fn(addon_id, pass, data, size);
    return r == 0    ? Result::ok
           : r == 1  ? Result::no_pass
           : r == -2 ? Result::too_big
           : r == -3 ? Result::denied
           : r == -4 ? Result::unknown_pass
                     : Result::invalid;
}

template <class T> Result set_data(const char *addon_id, const char *pass, const T &value)
{
    return set_data(addon_id, pass, &value, uint32_t(sizeof(T)));
}

inline Result clear_data(const char *addon_id, const char *pass)
{
    return set_data(addon_id, pass, nullptr, 0);
}

}
