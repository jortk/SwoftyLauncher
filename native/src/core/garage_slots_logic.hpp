#pragma once

#include <cstdint>

namespace runtime::garage_slots {

constexpr uint64_t USER_BASE = 0x10000;
constexpr uint64_t USER_END = 0x800000000000ull;
constexpr uint64_t VEC_BEGIN = 0x8;
constexpr uint64_t VEC_END = 0x10;
constexpr uint64_t ELEM = 8;
constexpr uint64_t FRAME_INDEX = 0x58;

inline bool user_pointer(uint64_t p)
{
    return p >= USER_BASE && p < USER_END;
}

inline bool count_of(uint64_t begin, uint64_t end, uint64_t *count)
{
    if (!user_pointer(begin) || end < begin || (end - begin) % ELEM)
        return false;
    *count = (end - begin) / ELEM;
    return true;
}

inline bool has_garage(uint64_t index, uint64_t count)
{
    return index < count;
}

struct Seen {
    uint64_t last_count = ~0ull;
    bool logged = false;

    bool first(uint64_t count)
    {
        if (logged && last_count == count)
            return false;
        logged = true;
        last_count = count;
        return true;
    }
};

}
