#pragma once

#include "garage_setup_logic.hpp"

#include <cstdint>

namespace runtime::theatre_list {

using garage_setup::Read;
using garage_setup::Skips;
constexpr int LOG_FIRST = garage_setup::LOG_FIRST;

constexpr uint64_t ROWS_END_AT = 0x118;
constexpr uint64_t ROW_SIZE = 0x4c8;

struct Exit {
    uint64_t rax = 0;
    uint64_t rip = 0;
};

inline bool entry_step(uint64_t entry, uint64_t no_slot, Read read, void *user, Exit *out)
{
    if (garage_setup::entry_usable(entry, read, user))
        return true;
    out->rax = 0;
    out->rip = no_slot;
    return false;
}

inline uint64_t row_index(uint64_t index, uint64_t begin, uint64_t end)
{
    if (end <= begin)
        return index;
    uint64_t count = (end - begin) / ROW_SIZE;
    if (!count || index < count)
        return index;
    return count - 1;
}

}
