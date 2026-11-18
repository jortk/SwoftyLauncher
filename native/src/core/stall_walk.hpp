#pragma once
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include "stall_order_logic.hpp"
#include <cstddef>
#include <cstdint>

// The load-stall report's unwinder (crash.cpp, runtime::stall_report). It runs while the thread it walks is
// suspended, so it takes no lock and allocates nothing: function entries come from the caller's lookup over a
// module list read before the suspension, and RtlVirtualUnwind only reads the image's unwind data and the stack.
// A fault while reading ends the walk instead of the process (SEH; stall_walk.cpp is built with -fms-extensions).
// With a Kept, the walk also copies the non-volatile registers as the unwind restored them at the first frame whose
// pc is in [lo, hi): for a thread spinning under a call out of the game, those are the game function's own values.

namespace stall_walk {

struct Kept {
    uintptr_t lo = 0, hi = 0;
    runtime::stall_order::Kept at;
};

using Lookup = PRUNTIME_FUNCTION (*)(DWORD64 pc, DWORD64 *base);
using Readable = bool (*)(uintptr_t a, size_t n);

int walk(CONTEXT *ctx, uintptr_t lo, uintptr_t hi, int max, Lookup lookup, Readable readable, uint64_t *rip,
         uint64_t *rsp, bool *faulted, Kept *kept = nullptr);

}
