#pragma once
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <string>

namespace runtime::replay_grid {

constexpr uint32_t STREAM_POS = 0x98;
constexpr uint32_t STREAM_BASE = 0xa0;
constexpr uint32_t HOLDER_CAPACITY = 0x108;
constexpr size_t STOCK_CARS = 22;
constexpr int64_t STOCK_GRID_MAX = 24;

constexpr uintptr_t HEADER = 0x80;
constexpr uint32_t HOLDER = 0x118;
constexpr uint32_t ENTRY_BASE = 0x198;
constexpr uint32_t ENTRY_INDEX = 0x190;
constexpr uint32_t CUR_SLOT = 0xc0;
constexpr uint32_t PREV_SLOT = 0xe8;
constexpr uint32_t CAP_SLOTS[] = {0xc8, 0x108};
constexpr uint32_t HEADER_SLOTS[] = {0xb8, 0xd0, 0xd8, 0xe0, 0xf0, 0xf8, 0x100};

struct HolderClass {
    uint32_t vtable_rva;
    uint32_t cap;
    uint32_t buf;
};

constexpr HolderClass CLASSES[] = {
    {0x5660400, 0x20, 0x80},     {0x56cfa10, 0x39, 0xc0},     {0x56588e0, 0x40, 0xc0},     {0x5660460, 0x80, 0x100},
    {0x5658970, 0x100, 0x180},   {0x5660430, 0x400, 0x480},   {0x568a270, 0x7bc, 0x840},   {0x565b180, 0x800, 0x880},
    {0x565b1b0, 0xc00, 0xc80},   {0x5660490, 0x1000, 0x1080}, {0x5659790, 0x1800, 0x1880}, {0x5635390, 0x1c00, 0x1c80},
    {0x5659c30, 0x2000, 0x2080}, {0x568b3f0, 0x2008, 0x2080}, {0x57b6ec0, 0x2400, 0x2480}, {0x56589d0, 0x2800, 0x2880},
    {0x56589a0, 0x4000, 0x4080}, {0x56360a0, 0x8000, 0x8080}, {0x5658940, 0xa800, 0xa880}, {0x5658910, 0xffff, 0x10080},
};
constexpr int CLASS_COUNT = sizeof CLASSES / sizeof CLASSES[0];

inline int class_of_rva(const HolderClass *t, int n, uint64_t rva)
{
    for (int i = 0; i < n; i++)
        if (t[i].vtable_rva == rva)
            return i;
    return -1;
}

inline int class_of_rva(uint64_t rva)
{
    return class_of_rva(CLASSES, CLASS_COUNT, rva);
}

inline int grown_class(const HolderClass *t, int n, int from, uint64_t state)
{
    if (from < 0 || from >= n || state <= t[from].cap)
        return -1;
    int fit = -1, roomy = -1;
    for (int i = 0; i < n; i++) {
        if (t[i].buf <= t[from].buf || t[i].cap <= t[from].cap || t[i].cap < state)
            continue;
        if (fit < 0 || t[i].cap < t[fit].cap)
            fit = i;
        if (t[i].cap >= state * 2 && (roomy < 0 || t[i].cap < t[roomy].cap))
            roomy = i;
    }
    return roomy >= 0 ? roomy : fit;
}

inline int grown_class(int from, uint64_t state)
{
    return grown_class(CLASSES, CLASS_COUNT, from, state);
}

inline bool skips_read(uint32_t stored_size, bool recorded_as_nothing)
{
    return stored_size == 0 && recorded_as_nothing;
}

class Emptied {
    static constexpr size_t N = 512;
    std::atomic<uintptr_t> slots[N] = {};

public:
    void add(uintptr_t p)
    {
        if (!p)
            return;
        size_t h = (p >> 4) % N;
        for (size_t k = 0; k < N; k++) {
            auto &s = slots[(h + k) % N];
            uintptr_t expected = 0;
            if (s.compare_exchange_strong(expected, p) || expected == p)
                return;
        }
    }

    bool has(uintptr_t p) const
    {
        if (!p)
            return false;
        size_t h = (p >> 4) % N;
        for (size_t k = 0; k < N; k++) {
            uintptr_t v = slots[(h + k) % N].load(std::memory_order_relaxed);
            if (v == p)
                return true;
            if (!v)
                return false;
        }
        return false;
    }
};

inline std::atomic<int64_t> session_vehicles{-1};

inline uint64_t capacity_of(uint64_t raw)
{
    return raw & 0xffffffffull;
}

inline bool guards(int64_t cars)
{
    return cars < 0 || cars > STOCK_GRID_MAX;
}

inline bool outgrew(uint64_t pos, uint64_t base, uint64_t capacity)
{
    return capacity != 0 && pos > base && pos - base > capacity;
}

inline uint64_t room_of(uint64_t cur, uint64_t prev, uint64_t capacity)
{
    return prev > cur && prev - cur >= capacity ? prev - cur : 0;
}

struct Tally {
    std::atomic<uint64_t> skipped{0};

    bool first()
    {
        return skipped.fetch_add(1, std::memory_order_relaxed) == 0;
    }
};

inline std::string grown_line(int64_t cars, const std::string &recordable, uint64_t state, uint64_t from, uint64_t to)
{
    char buf[400];
    std::string name = recordable.empty() ? "?" : recordable;
    char count[24] = "?";
    if (cars >= 0)
        snprintf(count, sizeof count, "%lld", static_cast<long long>(cars));
    snprintf(buf, sizeof buf,
             "[replay] session has %s cars, the recorder holds %zu; %s needs %llu bytes of replay state and its holder had "
             "%llu, so the holder grew to %llu",
             count, STOCK_CARS, name.c_str(), static_cast<unsigned long long>(state), static_cast<unsigned long long>(from),
             static_cast<unsigned long long>(to));
    return buf;
}

inline std::string line(int64_t cars, const std::string &recordable, uint64_t state, uint64_t capacity)
{
    char buf[400];
    std::string name = recordable.empty() ? "?" : recordable;
    char count[24] = "?";
    if (cars >= 0)
        snprintf(count, sizeof count, "%lld", static_cast<long long>(cars));
    snprintf(buf, sizeof buf,
             "[replay] session has %s cars, the recorder holds %zu; %s needs %llu bytes of replay state and its holder has "
             "%llu, so it is not recorded (replay and flashback lose its state; the session keeps all its cars)",
             count, STOCK_CARS, name.c_str(), static_cast<unsigned long long>(state),
             static_cast<unsigned long long>(capacity));
    return buf;
}

inline std::string room_line(int64_t cars, const std::string &recordable, uint64_t state, uint64_t room, uint64_t grown)
{
    char buf[400];
    std::string name = recordable.empty() ? "?" : recordable;
    char count[24] = "?";
    if (cars >= 0)
        snprintf(count, sizeof count, "%lld", static_cast<long long>(cars));
    char outcome[96];
    if (grown)
        snprintf(outcome, sizeof outcome, "so the holder grew to %llu", static_cast<unsigned long long>(grown));
    else
        snprintf(outcome, sizeof outcome, "so it is not recorded (replay and flashback lose its state)");
    snprintf(buf, sizeof buf,
             "[replay] session has %s cars; %s needs %llu bytes of replay state, more than the %llu its holder has room "
             "for (track or session data larger than the game sized it for), %s",
             count, name.c_str(), static_cast<unsigned long long>(state), static_cast<unsigned long long>(room), outcome);
    return buf;
}

}
