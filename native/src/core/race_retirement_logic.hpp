#pragma once

#include "driver_ref_logic.hpp"

#include <cstddef>
#include <cstdint>
#include <cstring>

namespace runtime::race_retirement {

using driver_ref::driver_count;
using driver_ref::Miss;
using driver_ref::miss;
using driver_ref::miss_text;

constexpr size_t NAME_SIZE = 0x98;
constexpr uint64_t NAME_CAPACITY = 0x80;
constexpr size_t NAME_CHARS = 0x18;
constexpr uint8_t NO_CAR = 0xff;
constexpr size_t MAX_SAID = 512;

inline bool skips(uint64_t driver_plus_8)
{
    return !driver_plus_8;
}

inline void empty_name(uint8_t *name, uint64_t vtable)
{
    uint64_t chars = reinterpret_cast<uintptr_t>(name + NAME_CHARS);
    std::memcpy(name, &vtable, 8);
    std::memcpy(name + 8, &NAME_CAPACITY, 8);
    std::memcpy(name + 0x10, &chars, 8);
    name[NAME_CHARS] = 0;
}

enum class Entrant { no_car, no_list, not_found, found };

inline Entrant entrant(uint8_t car, uint64_t begin, uint64_t end, bool found)
{
    if (car == NO_CAR)
        return Entrant::no_car;
    if (!begin || end <= begin)
        return Entrant::no_list;
    return found ? Entrant::found : Entrant::not_found;
}

inline uint64_t key(uint8_t car, Entrant e, uint8_t domain, uint16_t id)
{
    if (e != Entrant::found)
        return uint64_t(1) << 40 | uint64_t(car) << 24 | uint64_t(e) << 16;
    return uint64_t(car) << 24 | uint64_t(domain) << 16 | id;
}

struct Said {
    uint64_t keys[MAX_SAID] = {};
    size_t count = 0;

    bool first(uint64_t k)
    {
        for (size_t i = 0; i < count; i++)
            if (keys[i] == k)
                return false;
        if (count == MAX_SAID)
            return false;
        keys[count++] = k;
        return true;
    }
};

inline const char *entrant_text(Entrant e)
{
    switch (e) {
    case Entrant::no_car:
        return "the retired car has no vehicle index";
    case Entrant::no_list:
        return "the game module has no entrant list";
    case Entrant::not_found:
        return "no entrant of the game module's list has this vehicle index";
    case Entrant::found:
        break;
    }
    return "its entrant has a driver";
}

}
