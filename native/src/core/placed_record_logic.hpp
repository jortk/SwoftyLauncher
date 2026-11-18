#pragma once

#include <atomic>
#include <cstdint>
#include <deque>
#include <mutex>
#include <utility>
#include <vector>

namespace runtime::placed {

constexpr int CAPACITY = 128;
constexpr int HEAPS = 3;
constexpr int NOTE_FROM = 64;
constexpr uint64_t PARK_MS = 5000;

struct Stats {
    std::atomic<int> high[HEAPS + 1]{};
    std::atomic<uint64_t> overflows{0};
    std::atomic<uint64_t> parked{0};
    std::atomic<bool> guard{true};
};

inline Stats &stats()
{
    static Stats s;
    return s;
}

struct Seen {
    int reached = 0;
    bool new_high = false;
    bool log_high = false;
    bool overflow = false;
    bool first_overflow = false;
};

inline int slot(uint32_t heap_id)
{
    return heap_id < HEAPS ? int(heap_id) : HEAPS;
}

inline Seen observe(Stats &s, uint32_t heap_id, int32_t index)
{
    Seen r;
    r.reached = index + 1;
    std::atomic<int> &h = s.high[slot(heap_id)];
    int prev = h.load(std::memory_order_relaxed);
    while (prev < r.reached && !h.compare_exchange_weak(prev, r.reached, std::memory_order_relaxed))
        ;
    r.new_high = prev < r.reached;
    r.log_high = r.new_high && r.reached > NOTE_FROM;
    r.overflow = index >= CAPACITY;
    if (r.overflow)
        r.first_overflow = s.overflows.fetch_add(1, std::memory_order_relaxed) == 0;
    return r;
}

inline bool skip_record(const Stats &s, int32_t index)
{
    return s.guard.load(std::memory_order_relaxed) && index >= CAPACITY;
}

template <class T> class Parked {
public:
    void push(T item, uint64_t now)
    {
        std::lock_guard<std::mutex> g(lock);
        items.emplace_back(now, std::move(item));
    }

    std::vector<T> take_due(uint64_t now, uint64_t delay)
    {
        std::vector<T> out;
        std::lock_guard<std::mutex> g(lock);
        while (!items.empty() && now - items.front().first >= delay) {
            out.push_back(std::move(items.front().second));
            items.pop_front();
        }
        return out;
    }

    size_t size()
    {
        std::lock_guard<std::mutex> g(lock);
        return items.size();
    }

private:
    std::mutex lock;
    std::deque<std::pair<uint64_t, T>> items;
};

}
