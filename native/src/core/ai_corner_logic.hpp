#pragma once
#include <atomic>
#include <cstdint>

namespace runtime::ai_corner {

inline bool missing(uint64_t corner_line)
{
    return corner_line == 0;
}

struct Tally {
    std::atomic<uint64_t> skipped{0};

    bool first()
    {
        return skipped.fetch_add(1, std::memory_order_relaxed) == 0;
    }
};

}
