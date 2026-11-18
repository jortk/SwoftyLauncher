#pragma once

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

// Pure helpers of the read-only copy scan (player_name.cpp): where a NUL-terminated name sits in a buffer, whether the bytes
// around it are a refcounted VM string record (i16 refcount at +4, 20-bit length in the u32 at +6, characters from +9), and
// which aligned qwords of a buffer point at one of a set of addresses.
namespace runtime::mem_scan {

inline void find_all(const uint8_t *buf, size_t n, const std::string &needle, std::vector<size_t> &out, size_t cap)
{
    std::string pat = needle;
    pat.push_back('\0');
    if (pat.size() > n)
        return;
    for (size_t i = 0; i + pat.size() <= n && out.size() < cap; i++) {
        const void *p = std::memchr(buf + i, pat[0], n - i - pat.size() + 1);
        if (!p)
            break;
        i = size_t(static_cast<const uint8_t *>(p) - buf);
        if (std::memcmp(buf + i, pat.data(), pat.size()) == 0)
            out.push_back(i);
    }
}

struct Record {
    bool is_record = false;
    int refcount = 0;
    uint32_t length = 0;
};

inline Record classify(const uint8_t *head9, size_t text_len)
{
    Record r;
    int16_t rc;
    uint32_t w;
    std::memcpy(&rc, head9 + 4, 2);
    std::memcpy(&w, head9 + 6, 4);
    r.refcount = rc;
    r.length = w & 0xfffff;
    r.is_record = rc > 0 && r.length == text_len;
    return r;
}

inline void find_pointers(const uint8_t *buf, size_t n, uint64_t base, const std::vector<uint64_t> &targets, std::vector<std::pair<uint64_t, uint64_t>> &out,
                          size_t cap)
{
    for (size_t i = 0; i + 8 <= n && out.size() < cap; i += 8) {
        uint64_t v;
        std::memcpy(&v, buf + i, 8);
        if (v < 0x10000)
            continue;
        for (uint64_t t : targets)
            if (v == t) {
                out.emplace_back(base + i, v);
                break;
            }
    }
}

}
