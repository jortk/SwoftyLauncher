// Host-native unit test for the core's CSV recorder ring (native/src/core/log_csv_logic.hpp): row building,
// formatting and quoting, drop counting on a full ring, and that rows come out in order.
// Built and run by native/test/run_log_csv_test.sh.

#include "log_csv_logic.hpp"

#include <cstdio>

namespace lc = runtime::log_csv;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

int main()
{
    lc::Recorder r({"t", "pos", "gap", "on"}, {lc::F32, lc::I32, lc::TEXT, lc::BOOL});
    CHECK(r.header() == "t,pos,gap,on\n");
    r.set_f32(0, 1.5f);
    r.set_i32(1, 3);
    r.set_text(2, "+1.2", 4);
    r.set_i32(3, 1);
    CHECK(!r.set_i32(9, 1));
    CHECK(r.commit());
    r.set_text(2, "a,\"b\"", 5);
    CHECK(r.commit());
    std::string out;
    CHECK(r.drain([&](const std::string &s) { out += s; }) == 2);
    CHECK(out == "1.5,3,+1.2,1\n0,0,\"a,\"\"b\"\"\",0\n");
    CHECK(r.drain([&](const std::string &) { CHECK(false); }) == 0);

    lc::Recorder q({"a"}, {lc::I32});
    for (size_t i = 0; i < lc::RING_ROWS + 10; i++) {
        q.set_i32(0, int32_t(i));
        q.commit();
    }
    CHECK(q.dropped() == 10);
    std::string all;
    CHECK(q.drain([&](const std::string &s) { all = s; }) == lc::RING_ROWS);
    CHECK(all.rfind("0\n1\n2\n", 0) == 0);
    q.set_i32(0, 7);
    CHECK(q.commit());

    char longtext[100];
    std::memset(longtext, 'x', sizeof longtext);
    lc::Recorder t({"s"}, {lc::TEXT});
    t.set_text(0, longtext, sizeof longtext);
    t.commit();
    std::string o;
    t.drain([&](const std::string &s) { o = s; });
    CHECK(o.size() == lc::TEXT_BYTES - 1 + 1);

    if (g_fail)
        return 1;
    std::printf("log csv test: ok\n");
    return 0;
}
