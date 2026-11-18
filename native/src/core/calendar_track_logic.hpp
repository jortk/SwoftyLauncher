#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

namespace runtime::calendar_track {

constexpr uintptr_t COMPONENT_CAREER = 0x278;
constexpr uintptr_t CALENDAR_BEGIN = 0x2b0;
constexpr uintptr_t CALENDAR_END = 0x2b8;
constexpr uintptr_t CAREER_TRACK_DATA = 0xf58;
constexpr uintptr_t RECORDS_BEGIN = 0x28;
constexpr uintptr_t RECORDS_END = 0x30;
constexpr size_t ENTRY = 0x28;
constexpr uintptr_t ENTRY_EVENT_ID = 0x8;
constexpr uintptr_t ENTRY_EVENT = 0x10;
constexpr uintptr_t EVENT_TRACK = 0xb0;
constexpr size_t ITEM = 0x10;
constexpr uintptr_t ITEM_RECORD = 0x8;
constexpr size_t ENTRY_DATA_WORDS = 4;
constexpr size_t MAX_ENTRIES = 256;
constexpr size_t MAX_RECORDS = 256;

struct Round {
    uint8_t event = 0, track = 0;
    bool data = false;
};

struct Calendar {
    bool read = false;
    uintptr_t begin = 0, end = 0;
    size_t records = 0;
    std::vector<Round> rounds;
};

template <class Read> Calendar scan(uintptr_t component, Read &&read)
{
    Calendar out;
    uint64_t begin = 0, end = 0, career = 0, system = 0, rb = 0, re = 0;
    if (!read(component + CALENDAR_BEGIN, begin) || !read(component + CALENDAR_END, end))
        return out;
    if (!begin || end < begin || (end - begin) % ENTRY || (end - begin) / ENTRY > MAX_ENTRIES)
        return out;
    if (!read(component + COMPONENT_CAREER, career) || !career || !read(career + CAREER_TRACK_DATA, system) || !system)
        return out;
    if (!read(system + RECORDS_BEGIN, rb) || !read(system + RECORDS_END, re))
        return out;
    if (!rb || re <= rb || (re - rb) % ITEM || (re - rb) / ITEM > MAX_RECORDS)
        return out;
    out.records = size_t((re - rb) / ITEM);
    for (uintptr_t entry = uintptr_t(begin); entry < end; entry += ENTRY) {
        Round r;
        uint64_t event = 0, record = 0;
        if (!read(entry + ENTRY_EVENT_ID, r.event) || !read(entry + ENTRY_EVENT, event) || !event ||
            !read(uintptr_t(event) + EVENT_TRACK, r.track))
            return Calendar{};
        r.data = r.track < out.records && read(uintptr_t(rb) + r.track * ITEM + ITEM_RECORD, record) && record;
        out.rounds.push_back(r);
    }
    out.begin = uintptr_t(begin);
    out.end = uintptr_t(end);
    out.read = true;
    return out;
}

inline std::vector<size_t> kept(const std::vector<Round> &rounds)
{
    std::vector<size_t> keep;
    for (size_t i = 0; i < rounds.size(); ++i)
        if (rounds[i].data)
            keep.push_back(i);
    if (keep.size() == rounds.size() || keep.empty())
        return {};
    return keep;
}

template <class Read, class Write>
uintptr_t compact(uintptr_t begin, const std::vector<size_t> &keep, Read &&read, Write &&write)
{
    std::vector<uint64_t> data;
    data.reserve(keep.size() * ENTRY_DATA_WORDS);
    for (size_t index : keep)
        for (size_t w = 0; w < ENTRY_DATA_WORDS; ++w) {
            uint64_t v = 0;
            if (!read(begin + index * ENTRY + 8 + w * 8, v))
                return 0;
            data.push_back(v);
        }
    for (size_t i = 0; i < data.size(); ++i)
        write(begin + (i / ENTRY_DATA_WORDS) * ENTRY + 8 + (i % ENTRY_DATA_WORDS) * 8, data[i]);
    return begin + keep.size() * ENTRY;
}

}
