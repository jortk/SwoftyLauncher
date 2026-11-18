#pragma once
#include <cstdint>
#include <cstring>
#include <functional>
#include <mutex>
#include <vector>

// Per-car AI racecraft fields (car_live.hpp, ai.<group>.<key>). The AI system S (the 0x1a8 object Sess_Build_AiSystem
// builds, captured at site_Ai_BuildSystem_RecordPushed) keeps one record R per vehicle, stride 0x508, in the vector
// [S+0xa8, S+0xb0). R+0x00 is the vehicle index and R+0x238 is the driver config D copied by value; every config field
// of D is an 8-byte slot holding a pointer P to the game's database cell: rec = *P, value = *(float *)(rec + 8) for a
// float (or int32) key, or a baked curve record (below) for a curve key. DRIVER lists the fields: key, D offset, type.
// The blocking evaluator (Ai_Blocking_Evaluate, 0x140624e60) re-reads the slot, P, rec and the float on every call,
// so a swapped slot is seen on the next tick.
//
// A curve record holds scale +0x10, offset +0x14, step +0x18, last value +0x1c, min +0x24, max +0x28 and a 64-sample
// byte LUT at +0x2c (evaluator inlined in 0x140624e60 and every other consumer, curve_at below). A layer on a curve
// field makes the own record flat: step 0 and an all-zero LUT make every sample min, and min, last value and max are
// the composed value, so every x evaluates to it.
//
// A car's field is changed by pointing its slot at a core-owned cell: cell[0] holds a pointer to an own record (a copy
// of the original record's first bytes) whose float at +8 is the composed value. The shared database cell is never
// written. The original slot pointer is kept and written back when the car's last layer on the field goes away.
// Owned cells are never freed: the game's AI threads may still be reading one when it is swapped out or the session
// ends, so a session leaks at most MAX_CARS * FIELD_COUNT blocks of 0x90 bytes, handed to `retired` on forget().
//
// Every method takes the lock. pushed() runs on the thread that builds the AI system; the rest run on the flow thread
// (apply, tick, forget) or any thread (get).

namespace swofty_core::hooks::ai_racecraft {

constexpr uintptr_t RECORDS_BEGIN = 0xa8, RECORDS_END = 0xb0, STRIDE = 0x508;
constexpr uintptr_t R_VEHICLE_INDEX = 0x00, R_DRIVER = 0x238;
constexpr uintptr_t VALUE_AT = 8, USER_BASE = 0x10000, USER_END = 0x800000000000ull;
constexpr uint32_t MAX_RECORDS = 64, MAX_CARS = 32;
constexpr uint32_t SETTLE_FRAMES = 30;
constexpr size_t RECORD_COPY = 0x80, RECORD_MID = 0x40, RECORD_MIN = 0x10;

constexpr uintptr_t CURVE_SCALE = 0x10, CURVE_OFFSET = 0x14, CURVE_STEP = 0x18, CURVE_LAST = 0x1c, CURVE_MIN = 0x24,
                    CURVE_MAX = 0x28, CURVE_LUT = 0x2c;
constexpr uint32_t CURVE_SAMPLES = 64;
constexpr size_t CURVE_BYTES = CURVE_LUT + CURVE_SAMPLES + 1 + 3;
constexpr int32_t INT_MAX_STOCK = 0x10000;

enum class Type : uint8_t { flt, i32, curve };

struct Config {
    const char *key;
    uintptr_t offset;
    Type type;
};

inline constexpr Config DRIVER[] = {
    {"blocking.fraction_of_follow_speed", 0x90, Type::flt},
    {"blocking.fraction_of_follow_speed_at_race_start", 0x98, Type::flt},
    {"blocking.max_blocking_distance", 0xa0, Type::flt},
    {"blocking.max_blocking_distance_at_race_start", 0xa8, Type::flt},
    {"alongside.speed_difference_for_limited_zone_yielding", 0x20, Type::flt},
    {"alongside.yield_overtake_rate", 0x28, Type::flt},
    {"alongside.min_presence_within_buffer", 0x30, Type::curve},
    {"divebomb_defense.max_presence_alongside", 0xf0, Type::curve},
    {"divebomb_defense.max_time_to_ignore", 0xf8, Type::flt},
    {"divebomb_defense.max_track_offset", 0x100, Type::flt},
    {"divebomb_defense.max_track_radius", 0x108, Type::flt},
    {"offset_variation.max_offset", 0x120, Type::flt},
    {"offset_variation.offset_limit_at_corner_start", 0x128, Type::flt},
    {"offset_variation.offset_limit_blend_distance", 0x130, Type::flt},
    {"offset_variation.half_width_scaling_graph", 0x148, Type::curve},
    {"offset_variation.radius_scalar_graph", 0x150, Type::curve},
    {"overtake.attacker_close_enough_car_lengths", 0x158, Type::flt},
    {"overtake.braking_zone_abort_overtake_time", 0x160, Type::flt},
    {"overtake.close_range_car_lengths_to_continue_overtake", 0x168, Type::flt},
    {"overtake.close_range_car_lengths_to_start_overtake", 0x170, Type::flt},
    {"overtake.oppertunist_overtake_time", 0x178, Type::flt},
    {"overtake.oppertunist_overtake_track_speed_difference", 0x180, Type::flt},
    {"overtake.danger_overtake_continue_time_adjustment", 0x188, Type::flt},
    {"overtake.danger_overtake_full_heat_time_adjustment", 0x190, Type::flt},
    {"overtake.danger_overtake_initiate_time_adjustment", 0x198, Type::flt},
    {"overtake.danger_overtake_pull_out_time", 0x1a0, Type::curve},
    {"overtake.regular_overtake_continue_time_adjustment", 0x1b0, Type::flt},
    {"overtake.regular_overtake_full_heat_time_adjustment", 0x1b8, Type::flt},
    {"overtake.regular_overtake_initiate_time_adjustment", 0x1c0, Type::flt},
    {"overtake.regular_overtake_pull_out_time", 0x1c8, Type::curve},
    {"overtake.overlap_percentage", 0x1d0, Type::curve},
    {"overtake.concede_speed_diff", 0x1d8, Type::curve},
    {"overtake.concede_slowdown_speed_diff", 0x1e0, Type::flt},
    {"overtake.concede_time_vs_behind", 0x1e8, Type::curve},
    {"space_seeking.added_buffer", 0x238, Type::flt},
    {"space_seeking.speed_below_follow", 0x240, Type::flt},
    {"space_seeking.first_speed_below_follow", 0x248, Type::flt},
    {"space_seeking.car_in_front_needed", 0x250, Type::i32},
    {"space_seeking.remember_crowed", 0x258, Type::flt},
    {"space_seeking.concede_overtake_blocked_by_crowed_time", 0x260, Type::flt},
    {"space_seeking.total_queue_needed", 0x268, Type::i32},
    {"space_seeking.distance_braking_point", 0x270, Type::flt},
    {"space_seeking.time_to_switch_sides", 0x278, Type::flt},
    {"space_seeking.track_distance_needed", 0x280, Type::flt},
    {"space_seeking.track_abort_distance", 0x288, Type::flt},
    {"space_seeking.cars_moving_away_speed_to_ignore", 0x290, Type::flt},
    {"space_seeking.can_start_track_radius", 0x298, Type::flt},
    {"blocking.start_blocking_intersection_time", 0x40, Type::flt},
    {"blocking.start_blocking_maximum_time_to_corner", 0x48, Type::flt},
    {"blocking.start_blocking_minimum_time_to_corner", 0x50, Type::curve},
    {"blocking.time_to_corner_for_end_merge_rate", 0x58, Type::flt},
    {"blocking.time_to_corner_for_start_merge_rate", 0x60, Type::flt},
    {"blocking.end_blocking_time_to_corner_vs_offset", 0x68, Type::curve},
    {"blocking.blocking_offset_from_car_behind", 0x70, Type::curve},
    {"blocking.blocking_speed_differential", 0x78, Type::flt},
    {"blocking.blocking_threat_time_modifier", 0x80, Type::flt},
    {"blocking.blocking_variable_offset_fraction", 0x88, Type::flt},
    {"blocking.avoidance_decrease_rate", 0xb0, Type::flt},
    {"blocking.merge_back_to_racing_line", 0xb8, Type::curve},
};

constexpr uint32_t FIELD_COUNT = sizeof DRIVER / sizeof DRIVER[0];

constexpr bool same_key(const char *a, const char *b)
{
    while (*a && *a == *b) {
        a++;
        b++;
    }
    return *a == *b;
}

constexpr uint32_t slot(const char *key)
{
    for (uint32_t i = 0; i < FIELD_COUNT; i++)
        if (same_key(DRIVER[i].key, key))
            return i;
    return FIELD_COUNT;
}

inline float float_at(const unsigned char *p)
{
    float v;
    std::memcpy(&v, p, 4);
    return v;
}

inline void put_float(unsigned char *p, float v) { std::memcpy(p, &v, 4); }

inline bool finite(float v) { return v - v == 0; }

// The evaluator the game inlines at every curve read: t = max(0, (|x| - offset) * scale), t > 1 -> 1; t == 1 gives the
// last value, anything else the LUT sample at t * 64 interpolated to the next one. NaN when t is NaN.
inline float curve_at(const unsigned char *rec, float x)
{
    float scale = float_at(rec + CURVE_SCALE), offset = float_at(rec + CURVE_OFFSET), step = float_at(rec + CURVE_STEP);
    float t = ((x < 0 ? -x : x) - offset) * scale;
    if (t != t)
        return t;
    float u = 0 > t ? 0.0f : t;
    if (1.0f < t)
        u = 1.0f;
    if (u == 1.0f)
        return float_at(rec + CURVE_LAST);
    uint32_t i = uint32_t(u * 64.0f);
    unsigned lo = rec[CURVE_LUT + i], hi = rec[CURVE_LUT + i + 1];
    return float(lo) * step + float_at(rec + CURVE_MIN) + step * (u * 64.0f - float(i)) * float(int(hi) - int(lo));
}

inline bool curve_shape(const unsigned char *rec)
{
    return finite(float_at(rec + CURVE_SCALE)) && finite(float_at(rec + CURVE_OFFSET)) &&
           finite(float_at(rec + CURVE_STEP)) && finite(float_at(rec + CURVE_LAST)) &&
           finite(float_at(rec + CURVE_MIN)) && finite(float_at(rec + CURVE_MAX));
}

inline void flatten(unsigned char *rec, float v)
{
    put_float(rec + CURVE_STEP, 0);
    std::memset(rec + CURVE_LUT, 0, CURVE_SAMPLES + 1);
    put_float(rec + CURVE_MIN, v);
    put_float(rec + CURVE_LAST, v);
    put_float(rec + CURVE_MAX, v);
}

inline void store_value(unsigned char *rec, Type type, float v)
{
    if (type == Type::curve)
        flatten(rec, v);
    else if (type == Type::i32) {
        int32_t n = int32_t(v < 0 ? v - 0.5f : v + 0.5f);
        std::memcpy(rec + VALUE_AT, &n, 4);
    } else
        put_float(rec + VALUE_AT, v);
}

// A float cell holding a denormal or a non-finite is an int (or bool) cell read through the wrong type, and an int
// cell holding anything outside a small count is a float: both are refused.
inline bool stock_value(Type type, const unsigned char *rec, float &out)
{
    if (type == Type::curve) {
        if (!curve_shape(rec))
            return false;
        out = curve_at(rec, 0.0f);
        return finite(out);
    }
    if (type == Type::i32) {
        int32_t n;
        std::memcpy(&n, rec + VALUE_AT, 4);
        if (n < 0 || n > INT_MAX_STOCK)
            return false;
        out = float(n);
        return true;
    }
    uint32_t bits;
    std::memcpy(&bits, rec + VALUE_AT, 4);
    float v = float_at(rec + VALUE_AT);
    if (!finite(v) || ((bits & 0x7f800000u) == 0 && (bits & 0x007fffffu) != 0))
        return false;
    out = v;
    return true;
}

enum class Result { ok, unavailable, late };

using Read = std::function<bool(uintptr_t addr, void *out, size_t n)>;
using Write = std::function<bool(uintptr_t addr, const void *in, size_t n)>;

struct Mem {
    Read read;
    Write write;
};

inline bool user_pointer(uint64_t p) { return p >= USER_BASE && p < USER_END && !(p & 7); }

struct Cell {
    alignas(16) unsigned char bytes[0x10 + RECORD_COPY] = {};
    uint64_t *pointer() { return reinterpret_cast<uint64_t *>(bytes); }
    unsigned char *record() { return bytes + 0x10; }
    float *value() { return reinterpret_cast<float *>(bytes + 0x10 + VALUE_AT); }
};

struct Swap {
    bool active = false;
    uintptr_t slot = 0, original = 0;
    Cell *cell = nullptr;
};

class Fields {
public:
    void pushed(const Mem &m, uintptr_t system)
    {
        std::lock_guard<std::mutex> g(lock);
        uint32_t n = 0;
        if (!records(m, system, n))
            return;
        if (system != system_ || n <= count_)
            forget_locked();
        system_ = system;
        count_ = n;
        quiet_ = 0;
    }

    void forget()
    {
        std::lock_guard<std::mutex> g(lock);
        forget_locked();
    }

    bool tick(const Mem &m)
    {
        std::lock_guard<std::mutex> g(lock);
        if (!system_)
            return false;
        uint32_t n = 0;
        if (!records(m, system_, n) || n != count_) {
            forget_locked();
            return false;
        }
        if (quiet_ < SETTLE_FRAMES)
            quiet_++;
        bool now = quiet_ == SETTLE_FRAMES && waiting_;
        if (now)
            waiting_ = false;
        return now;
    }

    bool settled() const
    {
        std::lock_guard<std::mutex> g(lock);
        return system_ && quiet_ >= SETTLE_FRAMES;
    }

    bool read(const Mem &m, uint32_t car, uint32_t field, float &out)
    {
        std::lock_guard<std::mutex> g(lock);
        uintptr_t r = 0;
        if (field >= FIELD_COUNT || !record(m, car, r))
            return false;
        uint64_t p = 0;
        float v = 0;
        if (!m.read(r + R_DRIVER + DRIVER[field].offset, &p, 8) || !original_value(m, field, p, v))
            return false;
        out = v;
        return true;
    }

    Result apply(const Mem &m, uint32_t car, uint32_t field, bool want, const std::function<float(float base)> &compose)
    {
        std::lock_guard<std::mutex> g(lock);
        if (field >= FIELD_COUNT || car >= MAX_CARS)
            return Result::unavailable;
        Swap &w = swaps_[car][field];
        uintptr_t r = 0;
        if (!record(m, car, r)) {
            if (want)
                waiting_ = true;
            return Result::unavailable;
        }
        uintptr_t at = r + R_DRIVER + DRIVER[field].offset;
        uint64_t current = 0;
        if (!m.read(at, &current, 8))
            return Result::late;
        if (w.active && current != reinterpret_cast<uintptr_t>(w.cell->bytes)) {
            w.active = false;
            if (!want)
                return Result::ok;
        }
        if (w.active)
            w.slot = at;
        if (!want) {
            if (w.active) {
                if (!m.write(at, &w.original, 8))
                    return Result::late;
                w.active = false;
            }
            return Result::ok;
        }
        uint64_t original = w.active ? w.original : current;
        float base = 0;
        if (!original_value(m, field, original, base))
            return Result::unavailable;
        float v = compose(base);
        const Type type = DRIVER[field].type;
        if (!w.active) {
            if (!w.cell)
                w.cell = new Cell();
            uint64_t rec = 0;
            m.read(original, &rec, 8);
            bool copied = type == Type::curve
                              ? m.read(rec, w.cell->record(), CURVE_BYTES)
                              : m.read(rec, w.cell->record(), RECORD_COPY) || m.read(rec, w.cell->record(), RECORD_MID) ||
                                    m.read(rec, w.cell->record(), RECORD_MIN);
            if (!copied)
                return Result::unavailable;
            *w.cell->pointer() = reinterpret_cast<uintptr_t>(w.cell->record());
        }
        store_value(w.cell->record(), type, v);
        if (!w.active) {
            uint64_t mine = reinterpret_cast<uintptr_t>(w.cell->bytes);
            if (!m.write(at, &mine, 8))
                return Result::late;
            w.active = true;
            w.slot = at;
            w.original = original;
        }
        return Result::ok;
    }

    bool swapped(uint32_t car, uint32_t field) const
    {
        std::lock_guard<std::mutex> g(lock);
        return car < MAX_CARS && field < FIELD_COUNT && swaps_[car][field].active;
    }

    size_t retired_cells() const
    {
        std::lock_guard<std::mutex> g(lock);
        return retired.size();
    }

    uintptr_t system() const
    {
        std::lock_guard<std::mutex> g(lock);
        return system_;
    }

private:
    mutable std::mutex lock;
    uintptr_t system_ = 0;
    uint32_t count_ = 0, quiet_ = 0;
    bool waiting_ = false;
    Swap swaps_[MAX_CARS][FIELD_COUNT];
    std::vector<Cell *> retired;

    static bool records(const Mem &m, uintptr_t system, uint32_t &n)
    {
        uint64_t begin = 0, end = 0;
        if (!user_pointer(system) || !m.read(system + RECORDS_BEGIN, &begin, 8) ||
            !m.read(system + RECORDS_END, &end, 8) || !user_pointer(begin) || end < begin + STRIDE ||
            (end - begin) % STRIDE != 0 || (end - begin) / STRIDE > MAX_RECORDS)
            return false;
        n = uint32_t((end - begin) / STRIDE);
        return true;
    }

    bool record(const Mem &m, uint32_t car, uintptr_t &r)
    {
        if (!system_)
            return false;
        uint32_t n = 0;
        if (!records(m, system_, n) || n != count_) {
            forget_locked();
            return false;
        }
        if (quiet_ < SETTLE_FRAMES || car >= count_)
            return false;
        uint64_t begin = 0;
        uint8_t index = 0;
        if (!m.read(system_ + RECORDS_BEGIN, &begin, 8))
            return false;
        r = begin + uintptr_t(car) * STRIDE;
        return m.read(r + R_VEHICLE_INDEX, &index, 1) && index == car;
    }

    static bool original_value(const Mem &m, uint32_t field, uint64_t p, float &v)
    {
        uint64_t rec = 0;
        if (!user_pointer(p) || !m.read(p, &rec, 8) || !user_pointer(rec))
            return false;
        unsigned char bytes[CURVE_BYTES] = {};
        size_t n = DRIVER[field].type == Type::curve ? CURVE_BYTES : VALUE_AT + 4;
        return m.read(rec, bytes, n) && stock_value(DRIVER[field].type, bytes, v);
    }

    void forget_locked()
    {
        for (auto &row : swaps_)
            for (auto &w : row) {
                if (w.cell)
                    retired.push_back(w.cell);
                w = Swap{};
            }
        system_ = 0;
        count_ = quiet_ = 0;
    }
};

}
