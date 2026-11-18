#pragma once

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

namespace runtime::save_heap {

constexpr uint64_t kMiB = 1048576;

constexpr uintptr_t kPoolVtable = 0x14568df40;
constexpr uintptr_t kImageBase = 0x140000000;
constexpr size_t kPoolStart = 0x98;
constexpr size_t kPoolSize = 0xa0;
constexpr size_t kPoolCursor = 0xa8;
constexpr size_t kPoolFlags = 0x80;

struct Pool {
    uint64_t start = 0, size = 0, cursor = 0;
    uint32_t flags = 0;
    uint64_t used() const { return cursor >= start && cursor - start <= size ? cursor - start : 0; }
    uint64_t free_bytes() const { return size - used(); }
};

inline uint64_t parse_request(const char *msg)
{
    const char *p = std::strstr(msg, "allocate ");
    return p ? std::strtoull(p + 9, nullptr, 10) : 0;
}

inline std::string mib(uint64_t bytes)
{
    char b[48];
    std::snprintf(b, sizeof b, "%.1f MiB", double(bytes) / double(kMiB));
    return b;
}

inline std::string pool_line(const Pool &p, uint64_t request)
{
    std::string s = "[saves] temp pool SAVE_PROPERTY_BAG_TEMP: size " + mib(p.size) + ", used " + mib(p.used()) +
                    ", free " + mib(p.free_bytes()) + ", request " + std::to_string(request) + " bytes";
    if (request > p.free_bytes())
        s += "; the request does not fit, the pool is " + mib(p.size) + " (the largest free block of SAVE_TEMP_ALLOC "
             "when the save began)";
    return s;
}

inline std::string session_line(uint64_t heap_bytes, bool raised, uint64_t kept_count, uint64_t kept_bytes)
{
    return "[saves] temp pool: SAVE_TEMP_ALLOC is " + mib(heap_bytes) + (raised ? " (raised)" : " (stock)") +
           "; the profile save pool is its largest free block; this save writes back " + std::to_string(kept_count) +
           " kept container(s), " + std::to_string(kept_bytes) +
           " bytes, straight into the save stream (not the pool)";
}

}
