#pragma once
#include <cstddef>
#include <cstdint>

namespace runtime::setup_vm {

inline bool skips(uint64_t car)
{
    return car == 0;
}

inline uint64_t skip_target(uint64_t at, size_t cmp_len, const uint8_t *after)
{
    if (after[0] == 0x75)
        return at + cmp_len + 2 + uint64_t(int64_t(int8_t(after[1])));
    if (after[0] == 0x0f && after[1] == 0x85) {
        int32_t rel = int32_t(uint32_t(after[2]) | uint32_t(after[3]) << 8 | uint32_t(after[4]) << 16 | uint32_t(after[5]) << 24);
        return at + cmp_len + 6 + uint64_t(int64_t(rel));
    }
    return 0;
}

struct Once {
    bool said = false;
    bool first()
    {
        if (said)
            return false;
        said = true;
        return true;
    }
};

}
