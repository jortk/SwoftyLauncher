// SPDX-License-Identifier: MIT
#pragma once
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/forward.hpp>
#include <swofty/systems/sessions.hpp>

#include <atomic>
#include <optional>
#include <string>
#include <string_view>

#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

// Grand Prix and Time Trial content: a package is one season build (an entrant list, its teams, cars
// and drivers, and the box's rules); an era box is the tile in the era picker that opens it.

namespace swofty::content {

using Result = swofty::Result;

struct Seat {
    Ref<drivers::Identity> driver;
    int number = 0;
    Ref<drivers::Look> look;
};
SWOFTY_RECORD(Seat, driver, number, look)

struct Team {
    std::string key;
    Ref<teams::Identity> team;
    Ref<cars::Vehicle> vehicle;
    Ref<sounds::EngineSound> engine_sound;
    Ref<liveries::Livery> livery;
    std::vector<Seat> drivers;
};
SWOFTY_RECORD(Team, key, team, vehicle, engine_sound, livery, drivers)

struct Package {
    static constexpr Version version{1, 3};
    Text title;
    std::string entrant_list;
    int season = 0;
    Ref<career::Calendar> calendar;
    Keyed<Team, &Team::key> teams;
    sessions::Rules rules;
    std::string setup_mode = "swofty_f1";
    std::string leaderboard = "box";
    bool virtual_list = false;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(Package, title, entrant_list, season, calendar, teams, rules, setup_mode, leaderboard, virtual_list,
              stat_id)

struct TimeTrial {
    std::string entrant_list;
};
SWOFTY_RECORD(TimeTrial, entrant_list)

enum class EraOnly { grand_prix, time_trial };
SWOFTY_ENUM(EraOnly, grand_prix, time_trial)

struct EraBox {
    static constexpr Version version{1, 2};
    Ref<Package> package;
    Text title;
    std::string image;
    int order = 0;
    TimeTrial time_trial;
    std::optional<EraOnly> only;
    json::Value extra;
};
SWOFTY_RECORD(EraBox, package, title, image, order, time_trial, only)

inline Registry<Package> packages{"content.package", {.applies = Applies::content}};
inline Registry<EraBox> era_boxes{"era.box", {.applies = Applies::read}};

}

namespace swofty::detail {

inline std::atomic<const SwoftyContentOps *> &content_ops_override()
{
    static std::atomic<const SwoftyContentOps *> t{nullptr};
    return t;
}

inline void set_content_ops_table(const SwoftyContentOps *t) { content_ops_override() = t; }

inline const SwoftyContentOps *content_ops_table()
{
    if (auto t = content_ops_override().load())
        return t->version >= 1 ? t : nullptr;
#if defined(_WIN32)
    static std::atomic<const SwoftyContentOps *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreContentOpsFn>(GetProcAddress(core, "SwoftyCoreContentOps")) : nullptr;
    const SwoftyContentOps *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
#else
    return nullptr;
#endif
}

}

namespace swofty::content {

enum class Ownership { unknown, owned, not_owned };

// Whether the player owns the content behind an entitlement name (standard library 1.8, experimental). "f1_2026"
// is the F1 2026 season content. unknown until the game has the answer, for any other name, and on a core or
// build without the game binding. A cache read: safe from any thread.
inline Ownership owned(std::string_view entitlement)
{
    auto t = swofty::detail::content_ops_table();
    if (!t || !t->owned)
        return Ownership::unknown;
    int r = t->owned(std::string(entitlement).c_str());
    return r == SWOFTY_OWNERSHIP_OWNED ? Ownership::owned : r == SWOFTY_OWNERSHIP_NOT_OWNED ? Ownership::not_owned : Ownership::unknown;
}

}
