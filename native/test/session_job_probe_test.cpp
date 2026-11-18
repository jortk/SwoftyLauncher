// Host-native unit test for the Game_Vfx_UpdateWheelTyreInstance entry probe (native/src/core/session_job_probe_logic.hpp):
// the size verdicts the game's unsigned loop test turns into a hang, the armed window (on at a session's entry, off at
// its deadline, after StateSession updates and at the value cap), the lock-free dedupe under 8 threads, that the logic
// has no mutex, and the log and [warning] lines. Built and run by native/test/run_session_job_probe_test.sh.

#include "session_job_probe_logic.hpp"

#if defined(_GLIBCXX_MUTEX) || defined(_LIBCPP_MUTEX)
#error "the probe's logic must not use a mutex: its hot path runs on the game's job threads"
#endif

#include <cstdio>
#include <cstring>
#include <thread>
#include <vector>

namespace sj = runtime::session_job_probe;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

static sj::Span span(uint64_t begin, int64_t bytes)
{
    sj::Span s;
    s.begin = begin;
    s.end = begin + uint64_t(bytes);
    s.read = true;
    return s;
}

static sj::Entry normal()
{
    sj::Entry e;
    e.vehicle = 7;
    e.wheel = 2;
    e.gate = sj::Runs;
    e.holder = 0x3a0b00000;
    e.lists = 0x3a0c00000;
    e.outer = span(0x3b0000000, 21 * 0x20);
    e.inner = span(0x3b1000000, 22 * 8);
    e.lists_read = 22;
    e.list_min = 1;
    e.list_max = 21;
    return e;
}

static void verdicts()
{
    CHECK(sj::verdict(span(0x1000, 0), sj::kOuterItem) == sj::Ok);
    CHECK(sj::verdict(span(0x1000, 0x20 * 21), sj::kOuterItem) == sj::Ok);
    CHECK(sj::verdict(span(0x1000, -0x60), sj::kOuterItem) == sj::Negative);
    CHECK(sj::verdict(span(0x1000, (sj::kHuge + 1) * 0x20), sj::kOuterItem) == sj::Huge);
    CHECK(sj::verdict(span(0x1000, sj::kHuge * 0x20), sj::kOuterItem) == sj::Ok);
    CHECK(sj::verdict(span(0x1000, 0x30), sj::kOuterItem) == sj::Ragged);
    CHECK(sj::verdict(sj::Span{}, sj::kOuterItem) == sj::NotRead);
    CHECK(sj::items(span(0x1000, -0x60), sj::kOuterItem) == -3);

    CHECK(sj::loop_count(span(0x1000, 0x20 * 21)) == 21);
    CHECK(sj::loop_count(span(0x1000, -0x60)) == ~uint64_t(0) - 2);
    CHECK(sj::loop_count(span(0x1000, -0x60)) > 0xffffffffull);

    CHECK(!sj::bad(normal()));
    sj::Entry e = normal();
    e.outer = span(0x3b0000000, -0x60);
    CHECK(sj::bad(e));
    e = normal();
    e.inner = span(0x3b1000000, -8);
    CHECK(sj::bad(e));
    e = normal();
    e.lists_bad = 1;
    CHECK(sj::bad(e));
    e = normal();
    e.list_max = sj::kHuge + 1;
    CHECK(sj::bad(e));
    e = normal();
    e.outer = sj::Span{};
    e.inner = sj::Span{};
    e.gate = sj::NoHolder;
    CHECK(!sj::bad(e));
}

static uint64_t value(int64_t outer_bytes)
{
    sj::Entry e = normal();
    e.outer = span(0x3b0000000, outer_bytes);
    return sj::key(e);
}

static void window()
{
    sj::Window w;
    CHECK(!w.armed());
    w.flow(2, "StateSession", 500);
    CHECK(!w.armed());
    w.flow(1, "SessionStateCareerHubUpdate", 600);
    CHECK(!w.armed());
    w.flow(1, "StateSessionX", 650);
    CHECK(!w.armed());
    w.flow(1, nullptr, 660);
    CHECK(!w.armed());

    w.flow(1, "StateSession", 1000);
    uint32_t g = w.armed();
    CHECK(g);
    CHECK(w.live(g, 1000));
    CHECK(w.live(g, 1000 + sj::kArmMs - 1));
    CHECK(!w.live(g, 1000 + sj::kArmMs));
    CHECK(!w.armed());
    CHECK(!w.live(0, 1000));

    w.flow(1, "StateSession", 50000);
    g = w.armed();
    w.flow(2, "StateSession", 53000);
    CHECK(w.live(g, 53000 + sj::kSettleMs - 1));
    CHECK(!w.live(g, 53000 + sj::kSettleMs));
    CHECK(!w.armed());

    w.flow(1, "StateLoadSession", 90000);
    g = w.armed();
    CHECK(g);
    w.flow(2, "StateSession", 90000 + 15000);
    CHECK(w.live(g, 90000 + sj::kArmMs - 1));
    CHECK(!w.live(g, 90000 + sj::kArmMs));

    w.flow(2, "StateSession", 200000);
    CHECK(!w.armed());

    w.flow(1, "StateSession", 300000);
    uint32_t old = w.armed();
    w.flow(1, "StateSession", 300100);
    uint32_t now = w.armed();
    CHECK(now && now != old);
    CHECK(!w.live(old, 300100 + sj::kArmMs));
    CHECK(w.armed() == now);
}

static void dedupe()
{
    sj::Window w;
    w.flow(1, "StateSession", 0);
    uint32_t g = w.armed();
    sj::Entry e = normal();
    CHECK(w.admit(g, sj::key(e)) == sj::First);
    CHECK(w.admit(g, sj::key(e)) == sj::Repeat);
    sj::Entry moved = e;
    moved.outer.begin += 0x1000;
    moved.outer.end += 0x1000;
    moved.wheel = 3;
    CHECK(sj::key(moved) == sj::key(e));
    CHECK(w.admit(g, sj::key(moved)) == sj::Repeat);

    sj::Entry neg = e;
    neg.outer = span(0x3b0000000, -0x60);
    CHECK(sj::key(neg) != sj::key(e));
    CHECK(w.admit(g, sj::key(neg)) == sj::First);
    sj::Entry gate = e;
    gate.gate = sj::FlagOff;
    CHECK(sj::key(gate) != sj::key(e));

    w.flow(1, "StateSession", 1000);
    g = w.armed();
    CHECK(w.admit(g, sj::key(e)) == sj::First);

    sj::Window cap;
    cap.flow(1, "StateSession", 0);
    g = cap.armed();
    for (int i = 0; i < sj::kPerSession - 1; i++) {
        CHECK(cap.admit(g, value((i + 1) * 0x20)) == sj::First);
        CHECK(cap.armed() == g);
    }
    CHECK(cap.admit(g, value(0x20 * 100)) == sj::FirstLast);
    CHECK(!cap.armed());
    CHECK(cap.admit(g, value(0x20 * 101)) == sj::Repeat);
    cap.flow(1, "StateSession", 5000);
    g = cap.armed();
    CHECK(g);
    CHECK(cap.admit(g, value(0x20 * 101)) == sj::First);

    sj::Window shared;
    shared.flow(1, "StateSession", 0);
    g = shared.armed();
    std::vector<std::thread> threads;
    std::vector<int> firsts(8, 0), lasts(8, 0);
    for (int t = 0; t < 8; t++)
        threads.emplace_back([&, t] {
            for (int i = 0; i < 4000; i++) {
                sj::Admit a = shared.admit(g, i % 2 ? sj::key(normal()) : value(((i / 2 + t) % 20 + 1) * 0x20));
                firsts[t] += a == sj::First;
                lasts[t] += a == sj::FirstLast;
            }
        });
    for (auto &t : threads)
        t.join();
    int f = 0, l = 0;
    for (int t = 0; t < 8; t++) {
        f += firsts[t];
        l += lasts[t];
    }
    CHECK(f == sj::kPerSession - 1);
    CHECK(l == 1);
    CHECK(!shared.armed());
}

static void lines()
{
    char b[1024];
    sj::Entry e = normal();
    size_t n = sj::line(e, 3, b, sizeof b);
    CHECK(n == std::strlen(b));
    std::printf("%s\n", b);
    CHECK(std::strstr(b, "[session-job] 0x1445acb00 entry, session 3: vehicle 7, wheel 2; the nested loop runs; "));
    CHECK(std::strstr(b, "outer list 0x3b0000000..0x3b00002a0 (672 bytes, 21 items, ok, loop count 21)"));
    CHECK(std::strstr(b, "inner list 0x3b1000000..0x3b10000b0 (176 bytes, 22 entries, ok)"));
    CHECK(std::strstr(b, "22 entry lists read, 1..21 u32s, 0 bad"));

    e.outer = span(0x3b0000000, -0x60);
    sj::line(e, 3, b, sizeof b);
    std::printf("%s\n", b);
    CHECK(std::strstr(b, "(-96 bytes, -3 items, NEGATIVE, loop count 18446744073709551613)"));
    sj::warning(e, 3, b, sizeof b);
    std::printf("%s\n", b);
    CHECK(std::strstr(b, "[session-job] [warning] 0x1445acb00 entry, session 3, vehicle 7, wheel 2: the outer list's end is "
                         "below its begin, so its count is negative and the loop's unsigned exit test never ends it"));
    CHECK(std::strstr(b, "(outer -96 bytes at 0x3b0000000, inner 176 bytes at 0x3b1000000, longest entry list 21 u32s)"));

    e = normal();
    e.outer = span(0x3b0000000, (sj::kHuge + 5) * 0x20);
    sj::warning(e, 3, b, sizeof b);
    CHECK(std::strstr(b, "the outer list's count is huge, so the loop runs for hours"));
    e = normal();
    e.inner = span(0x3b1000000, -16);
    sj::warning(e, 3, b, sizeof b);
    CHECK(std::strstr(b, "the inner list's end is below its begin"));
    e = normal();
    e.lists_bad = 2;
    sj::warning(e, 3, b, sizeof b);
    CHECK(std::strstr(b, "an entry's u32 list has its end below its begin"));

    e = sj::Entry{};
    e.vehicle = 4;
    e.gate = sj::NoHolder;
    sj::line(e, 2, b, sizeof b);
    std::printf("%s\n", b);
    CHECK(std::strstr(b, "vehicle 4, wheel 0; no loop: the wheel's slot holds no object (object 0x0, lists 0x0)"));
    CHECK(!std::strstr(b, "outer list"));

    sj::last_line(2, b, sizeof b);
    CHECK(std::strstr(b, "[session-job] 0x1445acb00: 8 distinct values logged in session 2; the probe is off until the "
                         "next session load"));

    char small[16];
    n = sj::line(normal(), 3, small, sizeof small);
    CHECK(n == sizeof small - 1 && std::strlen(small) == sizeof small - 1);
}

int main()
{
    verdicts();
    window();
    dedupe();
    lines();
    if (g_fail) {
        std::printf("session job probe test: %d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("session job probe test: ok\n");
    return 0;
}
