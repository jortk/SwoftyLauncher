#include "placed_record_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <string>

// Per-frame placed-buffer record array of the upload heaps (Render_Gfx2Heap_BumpAllocPlacedResource,
// re/db/globals.tsv site_Render_Gfx2Heap_RecordIndex).
//
// Each of the three upload heap objects keeps 128 records of 0x28 bytes at heap+0x90 and a count at heap+0x88.
// BumpAllocPlacedResource takes the next index with lock xadd for every placed buffer (flags & 0x8000048) and
// writes resource, name, buffer and the per-frame GPU address at heap+0x90+index*0x28 without a bound. The frame
// start releases records 0..count-1 and zeroes the count. Index 128 lands at heap+0x1490, on the command list
// counters and the first command list pointers (heap+0x1490/0x1494/0x1498, lists from heap+0x14a0).
//
// The hook sits right after the xadd (rdi = heap, r9d = the index taken, rdx = resource, rsi = buffer). It keeps
// a high-water mark of the count per heap, logs each new maximum past 64, and logs once loudly when a record
// would be index 128 or more. With the guard on, an index past 127 skips the record write and the count is put
// back to 128; the resource is released by this hook five seconds later instead of by the frame start.
//
// SWOFTY_PLACED_RECORD_GUARD=0 turns the guard off; the high-water marks stay on.

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

namespace {

const runtime::Sig S_RECORD{"Render_Gfx2Heap_BumpAllocPlacedResource record index",
                            "F0 44 0F C1 8F 88 00 00 00 4D 63 C9 4F 8D 0C 89 4E 8D 14 CF 4A 89 94 CF 90 00 00 00", 0x39b9c79,
                            runtime::Sig::addr, 0, 9, 0};

constexpr uintptr_t EPILOGUE = 0x2b;
constexpr uint64_t SUMMARY_MS = 15000;
std::atomic<uint64_t> last_summary{0};

safetyhook::MidHook hook;
runtime::placed::Parked<void *> parked;

void release_due(uint64_t now)
{
    for (void *r : parked.take_due(now, runtime::placed::PARK_MS))
        static_cast<IUnknown *>(r)->Release();
}

void on_record(safetyhook::Context &c)
{
    namespace pl = runtime::placed;
    auto &s = pl::stats();
    auto *heap = reinterpret_cast<uint8_t *>(c.rdi);
    int32_t index = int32_t(c.r9);
    uint32_t id = *reinterpret_cast<uint32_t *>(heap);
    pl::Seen seen = pl::observe(s, id, index);
    if (seen.log_high) {
        auto *buf = reinterpret_cast<const uint8_t *>(c.rsi);
        auto *name = reinterpret_cast<const char *>(c.rcx);
        char label[48] = "?";
        if (runtime::readable(name, 48)) {
            strncpy(label, name, sizeof label - 1);
            label[sizeof label - 1] = 0;
        }
        uint64_t flags = 0, bytes = 0;
        if (runtime::readable(buf, 0x38)) {
            flags = *reinterpret_cast<const uint64_t *>(buf + 0x20);
            bytes = (*reinterpret_cast<const uint64_t *>(buf + 0x28)) * (*reinterpret_cast<const uint64_t *>(buf + 0x30)) & 0xffffffffull;
        }
        swofty::log("[gfx] placed records: heap %u reached %d of %d this frame (buffer '%s', flags 0x%llx, %llu bytes)", id,
                    seen.reached, pl::CAPACITY, label, (unsigned long long)flags, (unsigned long long)bytes);
    }
    uint64_t now = GetTickCount64();
    uint64_t last = last_summary.load(std::memory_order_relaxed);
    if (now - last >= SUMMARY_MS && last_summary.compare_exchange_strong(last, now)) {
        char t[256];
        runtime::placed_record_crash_text(t, sizeof t);
        swofty::log("[gfx] %s", t);
    }
    if (!seen.overflow)
        return;
    if (seen.first_overflow)
        swofty::log("[gfx] PLACED RECORD OVERFLOW: heap %u took record %d of %d in one frame; the game writes it over "
                    "the command list arrays (%s)",
                    id, index, pl::CAPACITY, s.guard ? "guard on: record skipped" : "guard off");
    if (!pl::skip_record(s, index))
        return;
    *reinterpret_cast<volatile int32_t *>(heap + 0x88) = pl::CAPACITY;
    parked.push(reinterpret_cast<void *>(c.rdx), GetTickCount64());
    s.parked.fetch_add(1, std::memory_order_relaxed);
    release_due(GetTickCount64());
    c.rip = c.rip + EPILOGUE;
}

}

namespace runtime {

void placed_record_setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_PLACED_RECORD_GUARD", v, sizeof v);
    bool off = n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"));
    placed::stats().guard = !off;
    uintptr_t at = find(S_RECORD, "gfx", "gfx");
    if (!at) {
        swofty::log("[gfx] placed record counter: signature not found; high-water marks and guard off");
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), on_record);
    if (!h) {
        swofty::log("[gfx] placed record counter: hook failed: %s", hook_error(h.error()).c_str());
        return;
    }
    hook = std::move(*h);
    swofty::log("[gfx] placed record counter on (high-water marks per upload heap); guard %s",
                off ? "off (SWOFTY_PLACED_RECORD_GUARD=0)" : "armed: records past 127 are skipped");
}

void placed_record_crash_text(char *out, size_t cap)
{
    auto &s = placed::stats();
    snprintf(out, cap, "placed records high-water (of %d): heap0 %d, heap1 %d, heap2 %d; overflows %llu, parked %llu, guard %s",
             placed::CAPACITY, s.high[0].load(), s.high[1].load(), s.high[2].load(),
             (unsigned long long)s.overflows.load(), (unsigned long long)s.parked.load(), s.guard ? "on" : "off");
}

}
