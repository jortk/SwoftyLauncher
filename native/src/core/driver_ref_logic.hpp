#pragma once

#include <cstdint>

// Why Ent_Get_Driver_Thunk (0x1423dbcf0) returns no driver for a packed driver ref (domain byte, u16 stat id at +2):
// the domain is above 2, the entrants system has no driver table for it (entrants + 0x268 + domain * 0x10), the stat
// id is not below the table's count of 16-byte slots (items begin at table + 8, end at + 0x10), or its slot is empty.
// Shared by the guards on the game's unchecked callers of the lookup.

namespace runtime::driver_ref {

enum class Miss { unread, domain, no_table, past_end, empty_slot };

inline uint64_t driver_count(uint64_t begin, uint64_t end)
{
    return end > begin ? (end - begin) >> 4 : 0;
}

inline Miss miss(uint8_t domain, uint64_t table, uint64_t count, uint16_t id)
{
    if (domain > 2)
        return Miss::domain;
    if (!table)
        return Miss::no_table;
    if (id >= count)
        return Miss::past_end;
    return Miss::empty_slot;
}

inline const char *miss_text(Miss m)
{
    switch (m) {
    case Miss::domain:
        return "its domain is above 2";
    case Miss::no_table:
        return "the entrants system has no driver table for its domain";
    case Miss::past_end:
        return "its stat id is past the end of its domain's driver table";
    case Miss::empty_slot:
        return "its slot in its domain's driver table is empty";
    case Miss::unread:
        break;
    }
    return "its driver reference could not be read";
}

}
