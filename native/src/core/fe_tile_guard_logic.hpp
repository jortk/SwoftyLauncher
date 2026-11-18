#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace runtime::fe_tile {

constexpr uint64_t SLOT_SIZE = 0x10;
constexpr uint64_t SLOT_RECORD = 8;
constexpr uint64_t RECORD_NAME = 0x28;
constexpr uint64_t NAME_CHARS = 0x10;
constexpr size_t MAX_SLOTS = 4096;
constexpr size_t LOG_LABELS = 32;

inline bool canonical(uint64_t p)
{
    uint64_t top = p >> 47;
    return top == 0 || top == 0x1ffff;
}

inline bool switch_off(const char *value)
{
    if (!value)
        return false;
    return value[0] == '0' && value[1] == 0 ? true : value[0] == 'o' && value[1] == 'f' && value[2] == 'f' && value[3] == 0;
}

template <class Mem> bool list_present(Mem &mem, uint64_t first_slot, uint64_t end, const std::string &label)
{
    if (!first_slot || first_slot >= end)
        return false;
    size_t seen = 0;
    for (uint64_t slot = first_slot; slot < end && seen < MAX_SLOTS; slot += SLOT_SIZE, seen++) {
        uint64_t record = 0;
        if (!mem.u64(slot + SLOT_RECORD, record))
            return false;
        if (!record)
            continue;
        uint64_t name_box = 0, chars = 0;
        std::string name;
        if (!mem.u64(record + RECORD_NAME, name_box) || !mem.u64(name_box + NAME_CHARS, chars) || !mem.text(chars, name))
            return false;
        if (name == label)
            return true;
    }
    return false;
}

struct Labels {
    std::vector<std::string> seen;

    bool first(const std::string &label)
    {
        for (const std::string &s : seen)
            if (s == label)
                return false;
        if (seen.size() >= LOG_LABELS)
            return false;
        seen.push_back(label);
        return true;
    }

    bool last() const
    {
        return seen.size() == LOG_LABELS;
    }
};

}
