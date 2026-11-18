#pragma once
#include <swofty_api.h>

#include <atomic>
#include <cstdint>
#include <cstring>

namespace swofty_core::live {

struct Snapshot {
    uint64_t seq = 0;
    uint64_t frame = 0;
    bool valid = false;
    int32_t official = -1;
    float turn_length = 0;
    SwoftyLiveSession ses;
    SwoftyLiveCar cars[SWOFTY_LIVE_MAX_CARS];
    bool present[SWOFTY_LIVE_MAX_CARS];
};

inline void snap_begin(Snapshot *s)
{
    std::atomic_ref<uint64_t> seq(s->seq);
    seq.store(seq.load(std::memory_order_relaxed) + 1, std::memory_order_relaxed);
    std::atomic_thread_fence(std::memory_order_release);
}

inline void snap_end(Snapshot *s)
{
    std::atomic_thread_fence(std::memory_order_release);
    std::atomic_ref<uint64_t> seq(s->seq);
    seq.store(seq.load(std::memory_order_relaxed) + 1, std::memory_order_release);
}

inline bool snap_read(const Snapshot *src, Snapshot *out, int tries = 128)
{
    std::atomic_ref<uint64_t> seq(*const_cast<uint64_t *>(&src->seq));
    for (int i = 0; i < tries; i++) {
        uint64_t before = seq.load(std::memory_order_acquire);
        if (before & 1)
            continue;
        std::memcpy(out, src, sizeof *out);
        std::atomic_thread_fence(std::memory_order_acquire);
        if (seq.load(std::memory_order_acquire) == before)
            return true;
    }
    return false;
}

}
