#include "placed_record_logic.hpp"

#include <cstdio>
#include <thread>
#include <vector>

static int failures;

#define CHECK(cond, msg)                                                                                                \
    do {                                                                                                                \
        if (cond) {                                                                                                     \
            printf("ok  : %s\n", msg);                                                                                  \
        } else {                                                                                                        \
            printf("FAIL: %s\n", msg);                                                                                  \
            failures++;                                                                                                 \
        }                                                                                                               \
    } while (0)

int main()
{
    namespace pl = runtime::placed;
    {
        pl::Stats s;
        auto a = pl::observe(s, 1, 0);
        CHECK(a.reached == 1 && a.new_high && !a.log_high && !a.overflow, "the first record is a quiet new maximum");
        CHECK(!pl::observe(s, 1, 0).new_high, "the same count again is not a new maximum");
        CHECK(!pl::observe(s, 1, 63).log_high, "64 records is not logged");
        auto b = pl::observe(s, 1, 64);
        CHECK(b.reached == 65 && b.log_high, "65 records is logged");
        CHECK(!pl::observe(s, 1, 64).log_high, "65 again is not logged twice");
        CHECK(pl::observe(s, 1, 96).log_high, "a higher maximum is logged");
        CHECK(!pl::observe(s, 1, 80).log_high, "a lower count is not logged");
        CHECK(s.high[1].load() == 97, "the high-water mark is the highest count");
        CHECK(s.high[0].load() == 0 && s.high[2].load() == 0, "other heaps keep their own mark");
        CHECK(pl::observe(s, 7, 100).reached == 101 && s.high[pl::HEAPS].load() == 101, "an unknown heap id has its own slot");
    }
    {
        pl::Stats s;
        auto ok = pl::observe(s, 0, 127);
        CHECK(ok.reached == 128 && !ok.overflow && !pl::skip_record(s, 127), "record 127 is the last one the array holds");
        auto o = pl::observe(s, 0, 128);
        CHECK(o.overflow && o.first_overflow && pl::skip_record(s, 128), "record 128 overflows and is skipped");
        auto o2 = pl::observe(s, 0, 129);
        CHECK(o2.overflow && !o2.first_overflow, "only the first overflow is loud");
        CHECK(s.overflows.load() == 2, "every overflow is counted");
        s.guard = false;
        CHECK(!pl::skip_record(s, 128) && !pl::skip_record(s, 500), "with the guard off nothing is skipped");
    }
    {
        pl::Parked<int> p;
        p.push(1, 1000);
        p.push(2, 2000);
        p.push(3, 7000);
        CHECK(p.take_due(6000, pl::PARK_MS).size() == 1, "only entries parked long enough are released");
        auto rest = p.take_due(20000, pl::PARK_MS);
        CHECK(rest.size() == 2 && rest[0] == 2 && rest[1] == 3, "the rest follow in order");
        CHECK(p.size() == 0 && p.take_due(99999, pl::PARK_MS).empty(), "nothing is released twice");
    }
    {
        pl::Stats s;
        std::vector<std::thread> workers;
        for (int t = 0; t < 8; t++)
            workers.emplace_back([&s, t] {
                for (int i = 0; i < 200; i++)
                    pl::observe(s, 2, (i * 8 + t) % 190);
            });
        for (auto &w : workers)
            w.join();
        CHECK(s.high[2].load() == 190, "parallel render threads agree on the maximum");
    }
    return failures ? 1 : 0;
}
