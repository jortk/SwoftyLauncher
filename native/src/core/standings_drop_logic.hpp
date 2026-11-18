#pragma once

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <vector>

namespace runtime::standings_drop {

struct Record {
    uint32_t key;
    int32_t points;
};

struct PointsSession {
    uint32_t id = 0;
    std::vector<Record> records;
};

using PointsRound = std::vector<PointsSession>;

inline bool same_key(uint32_t a, uint32_t b)
{
    return (a & 0xffff00ffu) == (b & 0xffff00ffu);
}

inline int64_t best_sum(std::vector<int32_t> values, size_t best)
{
    std::sort(values.begin(), values.end(), std::greater<int32_t>());
    int64_t total = 0;
    for (size_t i = 0; i < values.size() && i < best; i++)
        total += values[i];
    return total;
}

inline int32_t counted_total(const std::vector<PointsRound> &rounds, size_t round, uint32_t session_id, uint32_t key,
                             size_t best)
{
    std::vector<int32_t> per_round;
    for (size_t j = 0; j <= round && j < rounds.size(); j++) {
        int64_t sum = 0;
        for (const PointsSession &s : rounds[j]) {
            for (const Record &r : s.records)
                if (same_key(r.key, key))
                    sum += r.points;
            if (j == round && s.id == session_id)
                break;
        }
        per_round.push_back(static_cast<int32_t>(sum));
    }
    return static_cast<int32_t>(best_sum(per_round, best));
}

}
