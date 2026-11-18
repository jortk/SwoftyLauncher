#pragma once

#include <cstdint>

namespace runtime::activity_resume {

inline bool skips(uint64_t field)
{
    return !field;
}

inline bool says(uint64_t field, bool &said)
{
    if (!skips(field) || said)
        return false;
    said = true;
    return true;
}

}
