#pragma once

#include <cstdint>

namespace runtime::estimate_laptime {

constexpr int LOG_FIRST = 4;

inline bool tyre_ctx_missing(uint64_t ctx)
{
    return ctx == 0;
}

struct Skips {
    int count = 0;

    int note()
    {
        return ++count;
    }

    bool logged(int n) const
    {
        return n <= LOG_FIRST;
    }

    bool last(int n) const
    {
        return n == LOG_FIRST;
    }
};

}
