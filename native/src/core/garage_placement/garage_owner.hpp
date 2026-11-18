// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once

#include <array>
#include <cmath>
#include <cstdint>
#include <limits>

namespace runtime::garage_placement {

using Address = std::uintptr_t;
using Matrix = std::array<float, 16>; // row-major game matrix; translation in row 3

struct Owner {
    Address instance{};
    Address crew_scene{};
    Address car_actor{};
    Address pit_bank{};
    std::uint32_t garage{};     // zero-based active garage index
    std::uint32_t native_slot{};
    Address module{};
};

// Capture during site_Game_GarageCrew_SceneRead on verified Steam 26566308.
// Treat the result as session-owned. The caller checks its active track and the
// entire authored Pit bank and invalidates the capture on unload or respawn.
// read(address, value) must return false if the requested bytes are unavailable.
template<class Read>
bool owner_at_scene_read(Read&& read, Address instance, Address scene, Address stack, Owner& out)
{
    Owner captured{instance, scene};
    Address instance_scene{};
    if (!instance || !scene || !stack ||
        !read(instance + 0xc8, instance_scene) || instance_scene != scene ||
        !read(stack + 0xc0, captured.module) || !captured.module || !read(captured.module + 0x60, captured.pit_bank) ||
        !captured.pit_bank || !read(instance + 0x11c, captured.garage) || captured.garage >= 32 ||
        !read(stack + 0xa8, captured.native_slot) || captured.native_slot >= 2 ||
        !read(instance + 0x1a8, captured.car_actor) || !captured.car_actor)
        return false;
    out = captured;
    return true;
}

enum class PitState { unreadable, absent, present };

// A null record is a free slot. A failed read is distinct from an absent Pit
// so the caller can require an exact per-track match across all 32 entries.
template<class Read>
PitState pit_transform(Read&& read, const Owner& owner, unsigned index, Matrix& out)
{
    Address current_bank{};
    if (!owner.module || !owner.pit_bank || index >= 32 ||
        !read(owner.module + 0x60, current_bank) || current_bank != owner.pit_bank)
        return PitState::unreadable;
    Address record{};
    if (!read(owner.pit_bank + 0x838 + sizeof(Address) * index, record))
        return PitState::unreadable;
    if (!record)
        return PitState::absent;
    Matrix matrix{};
    if (!read(record + 0x20, matrix))
        return PitState::unreadable;
    for (float element : matrix) {
        if (!std::isfinite(element))
            return PitState::unreadable;
    }
    out = matrix;
    return PitState::present;
}

struct CrewIds {
    static constexpr std::uint32_t missing = std::numeric_limits<std::uint32_t>::max();
    std::uint32_t mb{missing};
    std::uint32_t eb1{missing};
    std::uint32_t mb_variant{missing}; // Low quality rigs and F2 use "MB:".
};

struct CentralCrew {
    Address mb{};
    Address eb1{};
    unsigned mb_index{};
    unsigned eb1_index{};
};

// Index only the actors in this owner's current scene. A role must resolve
// exactly once. Coordinates of the actor roots and the animated bones are
// separate observations on the returned actors.
template<class Read>
bool central_crew(Read&& read, const Owner& owner, const CrewIds& ids, CentralCrew& out)
{
    if (ids.eb1 == CrewIds::missing || (ids.mb == CrewIds::missing && ids.mb_variant == CrewIds::missing) ||
        ids.eb1 == ids.mb || ids.eb1 == ids.mb_variant)
        return false;
    Address scene{}, resource{}, list{};
    std::uint64_t count{};
    if (!owner.instance || !owner.crew_scene || !read(owner.instance + 0xc8, scene) ||
        scene != owner.crew_scene || !read(scene + 0xa0, resource) || !resource ||
        !read(resource + 0x100, count) || !count || count > 256 ||
        !read(scene + 0x78, list) || !list)
        return false;
    CentralCrew found{};
    for (std::uint64_t i = 0; i < count; ++i) {
        Address actor{};
        if (!read(list + sizeof(Address) * i, actor))
            return false;
        if (!actor)
            continue;
        std::uint32_t id{};
        if (!read(actor + 0x248, id))
            return false;
        if (id == CrewIds::missing)
            continue;
        if (id == ids.eb1) {
            if (found.eb1)
                return false;
            found.eb1 = actor;
            found.eb1_index = static_cast<unsigned>(i);
        } else if (id == ids.mb || id == ids.mb_variant) {
            if (found.mb)
                return false;
            found.mb = actor;
            found.mb_index = static_cast<unsigned>(i);
        }
    }
    if (!found.mb || !found.eb1)
        return false;
    out = found;
    return true;
}

} // namespace runtime::garage_placement
