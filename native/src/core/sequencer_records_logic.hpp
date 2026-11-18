#pragma once

#include "driver_ref_logic.hpp"

#include <cstdint>

namespace runtime::sequencer_records {

inline bool skips(uint64_t first_match)
{
    return !first_match;
}

inline bool says(uint64_t first_match, bool &said)
{
    if (!skips(first_match) || said)
        return false;
    said = true;
    return true;
}

using driver_ref::driver_count;
using driver_ref::Miss;
using driver_ref::miss;
using driver_ref::miss_text;

enum class Driver { team_mate, player };

inline const char *who(Driver d)
{
    return d == Driver::player ? "the player's own driver" : "the team mate's driver";
}

}
