#pragma once

#include <cstdint>

namespace runtime::ai_curves {

inline bool skips(uint64_t vehicle)
{
    return !vehicle;
}

inline bool says(uint64_t vehicle, bool &said)
{
    if (!skips(vehicle) || said)
        return false;
    said = true;
    return true;
}

}
