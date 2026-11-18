#pragma once
#include <cstdint>

namespace runtime::track_record {

struct Seen {
    bool bits[256] = {};

    bool first(uint8_t track)
    {
        if (bits[track])
            return false;
        bits[track] = true;
        return true;
    }
};

enum class Verdict { game, no_record, past_table, bad_record, bad_node };

struct Round {
    uint64_t track = 0;
    bool table_known = false;
    uint64_t table_count = 0;
    uint64_t record = 0;
    bool record_readable = false;
    uint64_t node = 0;
    bool node_readable = false;
};

inline Verdict judge(const Round &r)
{
    if (r.table_known && r.track >= r.table_count)
        return Verdict::past_table;
    if (!r.record)
        return Verdict::no_record;
    if (!r.record_readable)
        return Verdict::bad_record;
    if (r.node && !r.node_readable)
        return Verdict::bad_node;
    return Verdict::game;
}

inline const char *why(Verdict v)
{
    switch (v) {
    case Verdict::past_table:
        return "track id past the track table";
    case Verdict::bad_record:
        return "unreadable track record";
    case Verdict::bad_node:
        return "unreadable track node";
    default:
        return "no track record";
    }
}

}
