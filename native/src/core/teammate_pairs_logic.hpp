#pragma once
#include "garage_crew_logic.hpp"

#include <cstdint>

namespace runtime::teammate_pairs {

constexpr uint64_t CAR_BYTES = 8;
constexpr uint64_t PAIR_BYTES = garage_crew::CREW_SLOTS * CAR_BYTES;
constexpr uint64_t MAX_BYTES = 0x800;
constexpr uint64_t TEAM_ID = 0x28;
constexpr size_t SEEN_TEAMS = 64;

inline bool over(uint64_t bytes)
{
    return bytes > PAIR_BYTES && bytes <= MAX_BYTES && bytes % CAR_BYTES == 0;
}

inline uint64_t cars(uint64_t bytes)
{
    return bytes / CAR_BYTES;
}

struct Seen {
    uint64_t session = 0;
    uint32_t teams[SEEN_TEAMS] = {};
    size_t used = 0;

    bool first(uint64_t now, uint32_t team)
    {
        if (now != session) {
            session = now;
            used = 0;
        }
        for (size_t i = 0; i < used; i++)
            if (teams[i] == team)
                return false;
        if (used < SEEN_TEAMS)
            teams[used++] = team;
        return true;
    }
};

}
