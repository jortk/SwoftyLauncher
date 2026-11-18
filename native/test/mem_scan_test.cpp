#include "mem_scan_logic.hpp"

#include <cstdio>

using namespace runtime::mem_scan;

static int failures = 0;
#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            failures++;                                                                                                \
        }                                                                                                              \
    } while (0)

int main()
{
    std::vector<uint8_t> buf(256, 0xaa);
    const char *name = "Swooftyy";
    std::memcpy(buf.data() + 100, name, 9);
    std::memcpy(buf.data() + 200, "Swooftyyy", 10);
    std::memcpy(buf.data() + 30, "Swooftyy", 8);
    std::vector<size_t> at;
    find_all(buf.data(), buf.size(), "Swooftyy", at, 8);
    CHECK(at.size() == 1 && at[0] == 100);

    std::vector<uint8_t> rec(32, 0);
    int16_t rc = 3;
    uint32_t w = 8;
    std::memcpy(rec.data() + 4, &rc, 2);
    std::memcpy(rec.data() + 6, &w, 4);
    std::memcpy(rec.data() + 9, name, 9);
    Record r = classify(rec.data(), 8);
    CHECK(r.is_record && r.refcount == 3 && r.length == 8);
    rc = 0;
    std::memcpy(rec.data() + 4, &rc, 2);
    CHECK(!classify(rec.data(), 8).is_record);

    std::vector<uint8_t> mem(64, 0);
    uint64_t target = 0x200000, other = 0x300000;
    std::memcpy(mem.data() + 16, &target, 8);
    std::memcpy(mem.data() + 40, &other, 8);
    std::memcpy(mem.data() + 49, &target, 8);
    std::vector<std::pair<uint64_t, uint64_t>> hits;
    find_pointers(mem.data(), mem.size(), 0x1000, {target}, hits, 8);
    CHECK(hits.size() == 1 && hits[0].first == 0x1010);
    if (failures)
        return 1;
    std::puts("mem scan: ok");
    return 0;
}
