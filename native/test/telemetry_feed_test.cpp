// Host unit test for the telemetry feed layout and seqlock. Builds and runs on the CI host
// (no Wine, no game); the core publishes the live feed with the same telemetry_feed.hpp.

#include "../src/core/telemetry_feed.hpp"

#include <atomic>
#include <cstdio>
#include <thread>

using namespace swofty::tel;

static int failures;

static void check(bool ok, const char *what)
{
    if (!ok) {
        std::printf("FAIL: %s\n", what);
        failures++;
    } else {
        std::printf("ok  : %s\n", what);
    }
}

static void test_layout()
{
    check(sizeof(SwoftyTelemetryFeed) == 6920, "feed is 6920 bytes");
    check(SWOFTY_TELEMETRY_VERSION == 2, "telemetry version is 2");
    SwoftyTelemetryFeed f;
    feed_init(&f);
    check(f.version == 2 && f.size == 6920, "feed_init sets version and size");
    check(f.session.version == 2, "feed_init tags the session");
    check(f.cars[0].version == 2 && f.cars[23].version == 2, "feed_init tags every car");
    check(f.participants[0].version == 2 && f.participants[23].version == 2, "feed_init tags every participant");
    check((f.seq & 1) == 0, "feed starts on an even seq");
}

static void test_copy()
{
    SwoftyTelemetryFeed src, dst;
    feed_init(&src);
    feed_init(&dst);
    src.frame = 42;
    src.session.total_laps = 58;
    src.cars[7].rpm = 12000.0f;
    src.participants[3].position = 4;
    feed_copy(&dst, &src);
    check(dst.frame == 42, "feed_copy carries the frame");
    check(dst.session.total_laps == 58, "feed_copy carries the session");
    check(dst.cars[7].rpm == 12000.0f, "feed_copy carries a car field");
    check(dst.participants[3].position == 4, "feed_copy carries a participant field");
    check(dst.version == 2 && dst.size == 6920, "feed_copy keeps the header");
    check((dst.seq & 1) == 0, "feed_copy leaves an even seq");
    SwoftyTelemetryFeed out;
    check(feed_read(&dst, &out) && out.frame == 42 && out.cars[7].rpm == 12000.0f, "feed_read sees the copy");
}

static void test_seqlock()
{
    static SwoftyTelemetryFeed shared;
    feed_init(&shared);
    std::atomic<bool> stop{false};
    std::atomic<uint64_t> completed{0};
    const uint64_t rounds = 2000000;

    // Past `rounds` the writer keeps writing, yielding between frames, until the reader has completed a read
    // while it writes; a reader thread scheduled only after the writer's burst would otherwise read nothing.
    std::thread writer([&] {
        for (uint64_t n = 1; n <= rounds || (completed.load(std::memory_order_relaxed) == 0 && n <= 4 * rounds);
             n++) {
            if (n > rounds)
                std::this_thread::yield();
            feed_begin(&shared);
            shared.frame = n;
            shared.session.total_laps = uint32_t(n);
            shared.cars[0].lap = uint32_t(n);
            shared.cars[23].lap = uint32_t(n);
            shared.participants[23].lap = uint32_t(n);
            feed_end(&shared);
        }
        stop.store(true, std::memory_order_release);
    });

    uint64_t reads = 0, torn = 0, retries = 0;
    SwoftyTelemetryFeed out;
    while (!stop.load(std::memory_order_acquire)) {
        if (!feed_read(&shared, &out)) {
            retries++;
            continue;
        }
        reads++;
        completed.store(reads, std::memory_order_relaxed);
        uint32_t tag = uint32_t(out.frame);
        if (out.session.total_laps != tag || out.cars[0].lap != tag || out.cars[23].lap != tag ||
            out.participants[23].lap != tag)
            torn++;
    }
    writer.join();
    std::printf("     seqlock: %llu reads, %llu retries, %llu torn\n", (unsigned long long)reads,
                (unsigned long long)retries, (unsigned long long)torn);
    check(reads > 0, "seqlock reader completed reads");
    check(torn == 0, "seqlock reader never saw a torn frame");
}

int main()
{
    test_layout();
    test_copy();
    test_seqlock();
    std::printf(failures ? "\n%d check(s) failed\n" : "\nall checks passed\n", failures);
    return failures ? 1 : 0;
}
