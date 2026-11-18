#pragma once
#include "track_record_logic.hpp"

#include <cstdint>

namespace runtime::teamhq_track {

enum class Miss { none, record, track };
enum class Load { record, track };

inline Miss classify(uint64_t record, uint64_t track)
{
    if (!record)
        return Miss::record;
    return track ? Miss::none : Miss::track;
}

inline bool skips(Load load, uint64_t record, uint64_t track)
{
    if (load == Load::record)
        return !record;
    return !track;
}

inline bool says(Load load, uint64_t record, uint64_t track, track_record::Seen &seen, uint8_t id)
{
    if (!skips(load, record, track))
        return false;
    if (load == Load::track && !record)
        return false;
    return seen.first(id);
}

}
