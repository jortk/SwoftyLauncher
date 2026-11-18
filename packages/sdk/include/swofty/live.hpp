// SPDX-License-Identifier: MIT
#pragma once
#include <swofty/live_images.h>
#include <atomic>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>
#include <swofty_api.h>
#if defined(_WIN32)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

// Live game state read from the game's memory: every car by vehicle id (the watched car included), the session
// header, the classification and road order, gaps, pit, tyre and penalty state, and the official turn at a car's
// lap distance. The core keeps one snapshot per frame and every call copies from it, so the calls are cheap and
// consistent from any thread. A value the game does not give for a car has its bit in Car::have clear and is zero;
// check has() before drawing it. A vehicle id is the slot of the game's vehicle table, the same index as
// cars::live::Car::index() and not the UDP participant index. Every call answers empty on a core without the
// live state, while no session is live, or while the group is not yet verified for the running build.

namespace swofty::live {

using Car = SwoftyLiveCar;
using Session = SwoftyLiveSession;
using Turn = SwoftyLiveTurn;

constexpr uint32_t version = SWOFTY_LIVE_VERSION;
constexpr uint32_t max_cars = SWOFTY_LIVE_MAX_CARS;
constexpr int32_t no_gap = SWOFTY_LIVE_NO_GAP;

enum class GameMode : uint8_t { unknown = 0, grand_prix = 1, time_trial = 2 };
enum class SafetyCar : uint8_t { none = 0, safety_car = 1, virtual_safety_car = 2, formation = 3, unknown = 0xff };

namespace detail {

inline std::atomic<const SwoftyLive *> &override_table()
{
    static std::atomic<const SwoftyLive *> t{nullptr};
    return t;
}

inline void set_table(const SwoftyLive *t) { override_table() = t; }

inline const SwoftyLive *table()
{
    if (auto t = override_table().load())
        return t->version >= 1 ? t : nullptr;
#if defined(_WIN32)
    static std::atomic<const SwoftyLive *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreLiveFn>(GetProcAddress(core, "SwoftyCoreLive")) : nullptr;
    const SwoftyLive *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
#else
    return nullptr;
#endif
}

}

inline bool available()
{
    auto t = detail::table();
    return t && t->available && t->available();
}

inline std::optional<Session> session()
{
    auto t = detail::table();
    Session s{};
    if (!t || !t->session || t->session(&s) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return s;
}

inline std::optional<Car> car(uint32_t vehicle)
{
    auto t = detail::table();
    Car c{};
    if (!t || !t->car || t->car(vehicle, &c) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return c;
}

inline std::optional<Car> subject()
{
    auto t = detail::table();
    Car c{};
    if (!t || !t->subject || t->subject(&c) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return c;
}

inline std::optional<Car> player()
{
    auto s = session();
    return s && s->player >= 0 ? car(static_cast<uint32_t>(s->player)) : std::nullopt;
}

inline std::optional<Car> watched()
{
    auto s = session();
    return s && s->watched >= 0 ? car(static_cast<uint32_t>(s->watched)) : std::nullopt;
}

inline std::optional<Turn> turn(const Car &c)
{
    auto t = detail::table();
    Turn out{};
    if (!t || !t->turn || t->turn(c.vehicle, &out) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return out;
}

inline std::vector<Turn> track_turns()
{
    auto t = detail::table();
    uint32_t count = 0;
    if (!t || !t->track_turns || t->track_turns(&count, nullptr, 0) != SWOFTY_ASSET_OK || !count)
        return {};
    std::vector<Turn> out(count);
    if (t->track_turns(&count, out.data(), static_cast<uint32_t>(out.size())) != SWOFTY_ASSET_OK)
        return {};
    out.resize(count < out.size() ? count : out.size());
    return out;
}

inline std::optional<std::string> team_badge(uint32_t vehicle)
{
    auto t = detail::table();
    char key[48] = {};
    if (!t || !t->team_badge || t->team_badge(vehicle, key, sizeof key) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return std::string(key);
}

// The driver name the game shows for the car (the player's name for a human), UTF-8. Empty on a core whose live table
// is older than version 2 or while the driver names group is off.
inline std::optional<std::string> driver_name(uint32_t vehicle)
{
    auto t = detail::table();
    char name[128] = {};
    if (!t || t->version < 2 || !t->driver_name || t->driver_name(vehicle, name, sizeof name) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return std::string(name);
}

inline std::optional<std::string> team_badge_texture(uint32_t vehicle)
{
    auto t = detail::table();
    char path[96] = {};
    if (!t || t->version < 3 || !t->team_badge_texture ||
        t->team_badge_texture(vehicle, path, sizeof path) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return std::string(path);
}

// Caller-owned image bytes; no disk files or table presentation policy.
inline const SwoftyLiveImages *images()
{
    const auto *t = detail::table();
    return t && t->version >= 4 && t->images ? t->images() : nullptr;
}

inline std::optional<uint64_t> frame()
{
    auto t = detail::table();
    uint64_t f = 0;
    if (!t || !t->seq || t->seq(&f) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return f;
}

inline bool has(const Car &c, uint64_t bit) { return (c.have & bit) != 0; }
inline bool has(const Session &s, uint64_t bit) { return (s.have & bit) != 0; }

inline bool is_player(const Car &c) { return (c.flags & SWOFTY_LIVE_CAR_PLAYER) != 0; }
inline bool is_watched(const Car &c) { return (c.flags & SWOFTY_LIVE_CAR_WATCHED) != 0; }
inline bool is_racer(const Car &c) { return (c.flags & SWOFTY_LIVE_CAR_RACER) != 0; }
inline bool ai_driving(const Car &c) { return (c.flags & SWOFTY_LIVE_CAR_AI_DRIVING) != 0; }

inline std::optional<float> speed_kmh(const Car &c)
{
    return has(c, SWOFTY_LIVE_HAVE_SPEED) ? std::optional<float>(c.speed * 3.6f) : std::nullopt;
}

inline std::optional<float> ers_fraction(const Car &c)
{
    if (!has(c, SWOFTY_LIVE_HAVE_ERS_STORE) || c.ers_capacity <= 0)
        return std::nullopt;
    float f = c.ers_store / c.ers_capacity;
    return f < 0 ? 0.f : f > 1 ? 1.f : f;
}

inline std::optional<int32_t> gap_ahead_ms(const Car &c)
{
    return has(c, SWOFTY_LIVE_HAVE_GAP_AHEAD) && c.gap_ahead_ms != no_gap ? std::optional<int32_t>(c.gap_ahead_ms) : std::nullopt;
}

inline std::optional<int32_t> gap_leader_ms(const Car &c)
{
    return has(c, SWOFTY_LIVE_HAVE_GAP_LEADER) && c.gap_leader_ms != no_gap ? std::optional<int32_t>(c.gap_leader_ms) : std::nullopt;
}

inline bool in_pit(const Car &c)
{
    return has(c, SWOFTY_LIVE_HAVE_PIT) && c.pit_state >= 2 && c.pit_state <= 15;
}

inline std::optional<uint8_t> pit_status(const Car &c)
{
    return has(c, SWOFTY_LIVE_HAVE_PIT_LANE) ? std::optional<uint8_t>(c.pit_status) : std::nullopt;
}

inline std::optional<uint8_t> pit_stops(const Car &c)
{
    return has(c, SWOFTY_LIVE_HAVE_PIT_LANE) ? std::optional<uint8_t>(c.pit_stops) : std::nullopt;
}

inline GameMode game_mode(const Session &s)
{
    return has(s, SWOFTY_LIVE_SHAVE_GAME_MODE) ? static_cast<GameMode>(s.game_mode) : GameMode::unknown;
}

inline SafetyCar safety_car(const Session &s)
{
    return has(s, SWOFTY_LIVE_SHAVE_SAFETY_CAR) ? static_cast<SafetyCar>(s.safety_car) : SafetyCar::unknown;
}

struct Order {
    std::vector<uint32_t> ids;
    int player_rank = -1;
    int subject_rank = -1;

    int rank_of(uint32_t vehicle) const
    {
        for (size_t i = 0; i < ids.size(); i++)
            if (ids[i] == vehicle)
                return static_cast<int>(i);
        return -1;
    }

    std::optional<Car> ahead(uint32_t vehicle) const
    {
        int r = rank_of(vehicle);
        if (r < 0 || ids.size() < 2)
            return std::nullopt;
        return car(ids[r == 0 ? 1 : static_cast<size_t>(r) - 1]);
    }

    std::optional<Car> behind(uint32_t vehicle) const
    {
        int r = rank_of(vehicle);
        if (r < 0 || static_cast<size_t>(r) + 1 >= ids.size())
            return std::nullopt;
        return car(ids[static_cast<size_t>(r) + 1]);
    }
};

inline Order order()
{
    Order o;
    auto s = session();
    if (!s || !has(*s, SWOFTY_LIVE_SHAVE_ORDER))
        return o;
    o.ids.assign(s->order, s->order + (s->classified < max_cars ? s->classified : max_cars));
    if (s->player >= 0)
        o.player_rank = o.rank_of(static_cast<uint32_t>(s->player));
    if (s->subject >= 0)
        o.subject_rank = o.rank_of(static_cast<uint32_t>(s->subject));
    return o;
}

inline std::vector<uint32_t> road_order()
{
    auto s = session();
    if (!s || !has(*s, SWOFTY_LIVE_SHAVE_ROAD_ORDER))
        return {};
    return std::vector<uint32_t>(s->road_order, s->road_order + (s->on_track < max_cars ? s->on_track : max_cars));
}

}
