#pragma once
#include <swofty_api.h>

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstring>

// The shared telemetry feed and its seqlock, with no OS dependency so the host unit test
// (native/test/telemetry_feed_test.cpp) exercises the same code the core publishes with.
// A single writer mutates a feed between feed_begin and feed_end; seq is odd while a frame is
// in flight. A reader copies the body while seq is even and unchanged.

namespace swofty::tel {

static_assert(SWOFTY_TELEMETRY_VERSION == 2);
static_assert(sizeof(SwoftySessionState) == 80);
static_assert(sizeof(SwoftyCarState) == 192);
static_assert(sizeof(SwoftyParticipant) == 92);
static_assert(sizeof(SwoftyTelemetryFeed) == 6920);
static_assert(offsetof(SwoftyTelemetryFeed, seq) == 8);
static_assert(offsetof(SwoftyTelemetryFeed, frame) == 16);
static_assert(offsetof(SwoftyTelemetryFeed, session) == 24);
static_assert(offsetof(SwoftyTelemetryFeed, cars) == 104);
static_assert(offsetof(SwoftyTelemetryFeed, participants) == 4712);

constexpr uint32_t FEED_CARS = 24;
constexpr uint32_t FEED_PARTICIPANTS = 24;

inline void feed_init(SwoftyTelemetryFeed *f)
{
    std::memset(f, 0, sizeof *f);
    f->version = SWOFTY_TELEMETRY_VERSION;
    f->size = uint32_t(sizeof(SwoftyTelemetryFeed));
    f->session.version = SWOFTY_TELEMETRY_VERSION;
    for (auto &c : f->cars)
        c.version = SWOFTY_TELEMETRY_VERSION;
    for (auto &p : f->participants)
        p.version = SWOFTY_TELEMETRY_VERSION;
}

inline void feed_begin(SwoftyTelemetryFeed *f)
{
    std::atomic_ref<uint64_t> seq(f->seq);
    seq.store(seq.load(std::memory_order_relaxed) + 1, std::memory_order_relaxed);
    std::atomic_thread_fence(std::memory_order_release);
}

inline void feed_end(SwoftyTelemetryFeed *f)
{
    std::atomic_thread_fence(std::memory_order_release);
    std::atomic_ref<uint64_t> seq(f->seq);
    seq.store(seq.load(std::memory_order_relaxed) + 1, std::memory_order_release);
}

inline void feed_copy(SwoftyTelemetryFeed *dst, const SwoftyTelemetryFeed *src)
{
    feed_begin(dst);
    dst->frame = src->frame;
    dst->session = src->session;
    std::memcpy(dst->cars, src->cars, sizeof dst->cars);
    std::memcpy(dst->participants, src->participants, sizeof dst->participants);
    feed_end(dst);
}

inline bool feed_read(const SwoftyTelemetryFeed *src, SwoftyTelemetryFeed *out, int tries = 128)
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
