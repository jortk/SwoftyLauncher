// SPDX-License-Identifier: MIT
#pragma once

#include <bit>
#include <cstdint>
#include <array>
#include <optional>
#include <string_view>
#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

namespace swofty::garage_placement {

// Game coordinates, row-major, translation at [12..14]. Garage and native_slot are zero-based.
struct WorldMatrix { float values[16]{}; };
struct Pit { uint32_t garage{}; WorldMatrix world{}; };
struct Offsets { float mb[3]{}, eb1[3]{}; };
struct ActorRoots { WorldMatrix mb{}, eb1{}; };
struct Position { float x{}, y{}, z{}; };
struct CrewPositions { Position mb{}, eb1{}; };
enum class Result : uint32_t { unavailable, ok, invalid, rolled_back, rollback_failed, missing, ambiguous, conflict, pending };
enum class Mechanic : uint32_t { mb, eb1, fj1, fj2, fl, fr, rj, rl, rr, eng, eb2, count };
inline constexpr std::array<std::string_view, unsigned(Mechanic::count)> mechanic_names{
    "MB", "EB1", "FJ1", "FJ2", "FL", "FR", "RJ", "RL", "RR", "ENG", "EB2"
};
inline constexpr std::string_view mechanic_name(Mechanic role)
{
    return unsigned(role) < mechanic_names.size() ? mechanic_names[unsigned(role)] : std::string_view{};
}
inline constexpr std::optional<Mechanic> mechanic_from_name(std::string_view name)
{
    for (unsigned i = 0; i < mechanic_names.size(); ++i)
        if (mechanic_names[i] == name) return Mechanic(i);
    return {};
}
struct Mechanics {
    uint32_t present{}, ambiguous{};
    bool contains(Mechanic role) const { return unsigned(role) < mechanic_names.size() && (present & (1u << unsigned(role))); }
};
struct Bay;
using Visitor = void (*)(const Bay &, void *user);

namespace detail {
struct Ops {
    uint32_t version;
    Result (*visit)(const char *, const Pit *, uint32_t, uint32_t, uint32_t, Visitor, void *);
    Result (*root)(uint64_t, WorldMatrix *);
    Result (*car)(uint64_t, WorldMatrix *);
    Result (*set_root)(uint64_t, const WorldMatrix *);
    Result (*crew)(uint64_t, ActorRoots *);
    Result (*set_crew)(uint64_t, const Offsets *);
    Result (*restore_crew)(uint64_t);
    Result (*crew_pelvis)(uint64_t, CrewPositions *);
};
struct OpsV2 {
    uint32_t version;
    Result (*mechanics)(uint64_t, Mechanics *);
    Result (*read)(uint64_t, Mechanic, WorldMatrix *);
    Result (*pelvis)(uint64_t, Mechanic, Position *);
    Result (*set)(uint64_t, Mechanic, const WorldMatrix *);
    Result (*offset)(uint64_t, Mechanic, const Position *);
    Result (*restore)(uint64_t, Mechanic);
    Result (*set_car)(uint64_t, const WorldMatrix *);
    Result (*restore_car)(uint64_t);
    Result (*car_result)(uint64_t);
};
inline const OpsV2 *ops_v2()
{
#ifdef _WIN32
    const auto module = GetModuleHandleW(L"swofty_core.dll");
    using Export = const OpsV2 *(*)();
    auto get = module ? std::bit_cast<Export>(GetProcAddress(module, "SwoftyCoreGaragePlacementV2")) : nullptr;
    const OpsV2 *v = get ? get() : nullptr;
    return v && v->version == 2 ? v : nullptr;
#else
    return nullptr;
#endif
}
inline const Ops *ops()
{
#ifdef _WIN32
    const auto module = GetModuleHandleW(L"swofty_core.dll");
    using Export = const Ops *(*)();
    auto get = module ? std::bit_cast<Export>(GetProcAddress(module, "SwoftyCoreGaragePlacement")) : nullptr;
    const Ops *v = get ? get() : nullptr;
    return v && v->version == 1 ? v : nullptr;
#else
    return nullptr;
#endif
}
}

struct Bay {
    uint64_t ticket; // Valid only while the current visitor executes on the game thread.
    uint64_t generation; // Changes whenever this bay is respawned.
    uint32_t garage, native_slot;
    bool idle;
    Result read_root(WorldMatrix &out) const { return call(&detail::Ops::root, &out); }
    Result read_parked_car(WorldMatrix &out) const { return call(&detail::Ops::car, &out); }
    Result set_root(const WorldMatrix &world) const { return call(&detail::Ops::set_root, &world); }
    Result read_crew(ActorRoots &out) const { return call(&detail::Ops::crew, &out); }
    Result set_crew(const Offsets &offsets) const { return call(&detail::Ops::set_crew, &offsets); }
    Result restore_crew() const { return call(&detail::Ops::restore_crew); }
    Result read_crew_pelvis(CrewPositions &out) const { return call(&detail::Ops::crew_pelvis, &out); }
    Result mechanics(Mechanics &out) const { return call_v2(&detail::OpsV2::mechanics, &out); }
    Result read_mechanic(Mechanic role, WorldMatrix &out) const { return call_v2(&detail::OpsV2::read, role, &out); }
    Result read_mechanic_pelvis(Mechanic role, Position &out) const { return call_v2(&detail::OpsV2::pelvis, role, &out); }
    Result set_mechanic(Mechanic role, const WorldMatrix &world) const { return call_v2(&detail::OpsV2::set, role, &world); }
    Result set_mechanic_offset(Mechanic role, Position delta) const { return call_v2(&detail::OpsV2::offset, role, &delta); }
    Result restore_mechanic(Mechanic role) const { return call_v2(&detail::OpsV2::restore, role); }
    Result set_parked_car(const WorldMatrix &world) const { return call_v2(&detail::OpsV2::set_car, &world); }
    Result restore_parked_car() const { return call_v2(&detail::OpsV2::restore_car); }
    Result parked_car_result() const { return call_v2(&detail::OpsV2::car_result); }
private:
    template<class Member, class... Args> Result call_v2(Member member, Args... args) const
    {
        const auto *v = detail::ops_v2();
        return v ? (v->*member)(ticket, args...) : Result::unavailable;
    }
    template<class Member, class... Args> Result call(Member member, Args... args) const
    {
        const auto *v = detail::ops();
        return v ? (v->*member)(ticket, args...) : Result::unavailable;
    }
};

// Pass the complete authored Pit bank and the registered track ID. Called by the
// addon during SWOFTY_EVENT_GARAGE_PARKED_CAR (a = garage, b = native_slot).
inline Result visit(const char *track, const Pit *pits, uint32_t count, uint32_t garage,
                    uint32_t native_slot, Visitor f, void *user = nullptr)
{
    const auto *v = detail::ops();
    return v ? v->visit(track, pits, count, garage, native_slot, f, user) : Result::unavailable;
}
} // namespace swofty::garage_placement
