#pragma once

#include <cstddef>
#include <cstdint>

namespace runtime::ai_line_set {

constexpr uint64_t RECORDS_BEGIN_AT = 0xa8;
constexpr uint64_t RECORDS_END_AT = 0xb0;
constexpr uint64_t RECORD_SIZE = 0x508;
constexpr size_t SLOT_COUNT = 6;
constexpr uint64_t SLOTS[SLOT_COUNT] = {0x58, 0x60, 0x68, 0x70, 0x78, 0x80};
constexpr uint64_t LINE_CACHE_SLOT = 0x58;
constexpr uint64_t OPTIONAL_SLOT = 0x78;
constexpr uint64_t BRAKE_SETTINGS_ID_AT = 0x8;
constexpr uint64_t USER_BASE = 0x10000;
constexpr uint64_t USER_END = 0x800000000000ull;
constexpr uint64_t MAX_RECORDS = 64;
constexpr int LOG_FIRST = 4;

using Read = bool (*)(uint64_t address, void *out, size_t n, void *user);
using Write = bool (*)(uint64_t address, const void *in, size_t n, void *user);

struct Repair {
    bool ran = false;
    uint32_t index = 0;
    uint32_t filled = 0;
    uint32_t missing = 0;
    uint32_t backfilled = 0;
    bool brake_settings = false;
};

inline bool user_pointer(uint64_t p)
{
    return p >= USER_BASE && p < USER_END;
}

inline bool word(Read read, void *user, uint64_t address, uint64_t *out)
{
    return read(address, out, sizeof *out, user);
}

inline bool borrow_brake_settings(Read read, Write write, void *user, uint64_t begin, uint64_t last, uint64_t target)
{
    for (uint64_t at = begin; at <= last; at += RECORD_SIZE) {
        uint64_t line_cache = 0, id = 0;
        if (at == target || !word(read, user, at + LINE_CACHE_SLOT, &line_cache) || !user_pointer(line_cache))
            continue;
        return word(read, user, at + BRAKE_SETTINGS_ID_AT, &id) && id != 0 &&
               write(target + BRAKE_SETTINGS_ID_AT, &id, sizeof id, user);
    }
    return false;
}

inline void fill_record(Read read, Write write, void *user, uint64_t begin, uint64_t last, uint64_t target,
                        uint32_t *filled, uint32_t *missing, bool *brake_settings)
{
    uint64_t line_cache = 0;
    bool unlisted = word(read, user, target + LINE_CACHE_SLOT, &line_cache) && line_cache == 0;
    if (unlisted)
        *brake_settings = borrow_brake_settings(read, write, user, begin, last, target);
    for (uint64_t slot : SLOTS) {
        uint64_t value = 0;
        if (!word(read, user, target + slot, &value) || value != 0)
            continue;
        if (slot == OPTIONAL_SLOT && !unlisted)
            continue;
        uint64_t donor = 0;
        for (uint64_t at = begin; at <= last && donor == 0; at += RECORD_SIZE) {
            uint64_t candidate = 0;
            if (at != target && word(read, user, at + slot, &candidate) && user_pointer(candidate))
                donor = candidate;
        }
        if (donor && write(target + slot, &donor, sizeof donor, user))
            (*filled)++;
        else if (target == last)
            (*missing)++;
    }
}

inline Repair repair(Read read, Write write, void *user, uint64_t builder)
{
    Repair r;
    uint64_t begin = 0, end = 0;
    if (!user_pointer(builder) || !word(read, user, builder + RECORDS_BEGIN_AT, &begin) ||
        !word(read, user, builder + RECORDS_END_AT, &end))
        return r;
    if (!user_pointer(begin) || end < begin + RECORD_SIZE || (end - begin) % RECORD_SIZE != 0 ||
        (end - begin) / RECORD_SIZE > MAX_RECORDS)
        return r;
    uint64_t last = end - RECORD_SIZE;
    r.ran = true;
    r.index = static_cast<uint32_t>((last - begin) / RECORD_SIZE);
    for (uint64_t at = begin; at <= last; at += RECORD_SIZE) {
        uint32_t filled = 0, missing = 0;
        bool brake_settings = false;
        fill_record(read, write, user, begin, last, at, &filled, &missing, &brake_settings);
        if (at == last) {
            r.filled = filled;
            r.missing = missing;
            r.brake_settings = brake_settings;
        } else if (filled) {
            r.backfilled++;
        }
    }
    return r;
}

}
