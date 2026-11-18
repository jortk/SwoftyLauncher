#include "save_heap_logic.hpp"

#include <cstdio>
#include <cstring>

namespace sh = runtime::save_heap;

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
    CHECK(sh::parse_request("Not enough free memory to allocate 656560 byes (alignment 1)") == 656560);
    CHECK(sh::parse_request("something else") == 0);

    sh::Pool p;
    p.start = 0x10000000;
    p.size = 75 * sh::kMiB;
    p.cursor = p.start + p.size - 100000;
    CHECK(p.used() == p.size - 100000);
    CHECK(p.free_bytes() == 100000);
    std::string line = sh::pool_line(p, 656560);
    CHECK(line.find("size 75.0 MiB") != std::string::npos);
    CHECK(line.find("request 656560 bytes") != std::string::npos);
    CHECK(line.find("does not fit") != std::string::npos);
    p.cursor = p.start + 1000;
    CHECK(sh::pool_line(p, 656560).find("does not fit") == std::string::npos);
    p.cursor = 5;
    CHECK(p.used() == 0);

    std::string s = sh::session_line(200 * sh::kMiB, true, 2, 4420742);
    CHECK(s.find("200.0 MiB (raised)") != std::string::npos);
    CHECK(s.find("2 kept container(s), 4420742 bytes") != std::string::npos);
    CHECK(sh::session_line(75 * sh::kMiB, false, 1, 5).find("(stock)") != std::string::npos);

    if (g_fail) {
        std::printf("%d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("save_heap_test: ok\n");
    return 0;
}
