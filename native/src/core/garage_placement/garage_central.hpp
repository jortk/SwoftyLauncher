// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once

#include "garage_rig.hpp"
#include <algorithm>

namespace runtime::garage_placement {

using CrewDelta = std::array<float, 3>;
enum class CrewRole : unsigned { mb, eb1 };
struct CrewMemberPlacement {
    Address actor{};
    Matrix native{}, target{};
};
struct CrewPlacement {
    Owner owner{};
    CrewIds ids{};
    std::array<CrewMemberPlacement, 2> members{}; // MB, EB1
};
struct CrewReadback {
    std::array<Matrix, 2> worlds{};
    std::array<double, 2> native_error_m{}, target_error_m{};
};
enum class CrewResult { unavailable, applied, restored, rolled_back, rollback_failed };

namespace crew_detail {
inline double xyz_error(const Matrix& a, const Matrix& b)
{
    return std::hypot(double(a[12]) - b[12], double(a[13]) - b[13], double(a[14]) - b[14]);
}

template<class Read>
bool model_world(Read&& read, Address actor, Matrix& world)
{
    Address model{};
    std::array<float, 12> packed{};
    if (!read(actor + 0x228, model) || !model || !read(model + 0x10, packed))
        return false;
    Matrix result{};
    for (unsigned row = 0; row < 3; ++row)
        for (unsigned col = 0; col < 4; ++col)
            result[4 * col + row] = packed[4 * row + col];
    result[15] = 1;
    if (!rig_detail::affine(result))
        return false;
    world = result;
    return true;
}

template<class Read>
bool roles_still_owned(Read&& read, const Owner& owner, const CrewPlacement& saved)
{
    Address current_bank{}, car{};
    std::uint32_t garage{};
    const Owner& first = saved.owner;
    if (!owner.module || !owner.instance || owner.module != first.module || owner.pit_bank != first.pit_bank ||
        owner.instance != first.instance || owner.crew_scene != first.crew_scene ||
        owner.car_actor != first.car_actor || owner.garage != first.garage || owner.native_slot != first.native_slot ||
        !read(owner.module + 0x60, current_bank) || !current_bank || current_bank != owner.pit_bank ||
        !read(owner.instance + 0x11c, garage) || garage != owner.garage ||
        !read(owner.instance + 0x1a8, car) || !car || car != owner.car_actor)
        return false;
    CentralCrew actors{};
    return central_crew(read, owner, saved.ids, actors) &&
           actors.mb == saved.members[0].actor && actors.eb1 == saved.members[1].actor;
}
// Private actor-scene setter. Both callers first match a unique actor on the scoped crew scene.
template<class Read, class IsGameTable, class IsGameCode>
bool set_actor_world(Address actor, const Matrix &value, Read &&read, IsGameTable &&game_table, IsGameCode &&game_code)
{
    Address table{}, setter{};
    if (!read(actor, table) || !table || !game_table(table) || !read(table + 0x70, setter) ||
        !setter || !game_code(setter))
        return false;
    alignas(16) float packed[12];
    for (unsigned row = 0; row < 3; ++row)
        for (unsigned col = 0; col < 4; ++col)
            packed[4 * row + col] = value[4 * col + row];
    using Setter = void (*)(void*, const float*, std::uint32_t);
    reinterpret_cast<Setter>(setter)(reinterpret_cast<void*>(actor), packed, 1);
    return true;
}
} // namespace crew_detail

// Called after the independent parked-car pose has been checked. Uses the same
// definition × scene composition as native scene refresh for unmodified poses.
// Repeating preparation therefore cannot add a delta to an already-shifted root.
template<class Read, class Scope>
bool prepare_central_crew(const Owner& owner, const CrewIds& ids, const Matrix& parked_car,
                          const std::array<CrewDelta, 2>& deltas, Read&& read, Scope&& in_scope,
                          CrewPlacement& out)
{
    if (!in_scope(owner) || !rig_detail::affine(parked_car))
        return false;
    CentralCrew roles{};
    Address resource{}, definitions{};
    Matrix scene_world{};
    if (!central_crew(read, owner, ids, roles) ||
        !read(owner.crew_scene + 0xa0, resource) || !resource ||
        !read(resource + 0x50, definitions) || !definitions ||
        !read(owner.crew_scene + 0x20, scene_world) || !rig_detail::affine(scene_world))
        return false;
    CrewPlacement capture{};
    capture.owner = owner;
    capture.ids = ids;
    for (unsigned k = 0; k < 2; ++k) {
        const auto& delta = deltas[k];
        if (!std::isfinite(delta[0]) || !std::isfinite(delta[1]) || !std::isfinite(delta[2]) ||
            std::hypot(double(delta[0]), double(delta[2])) > 3. || std::abs(delta[1]) > .05f)
            return false;
        auto& member = capture.members[k];
        member.actor = k == 0 ? roles.mb : roles.eb1;
        const unsigned index = k == 0 ? roles.mb_index : roles.eb1_index;
        Address definition{};
        Matrix local{}, observed{};
        if (!read(member.actor + 0x538, definition) ||
            (!definition && !read(definitions + sizeof(Address) * index, definition)) ||
            !definition || !read(definition + 0x40, local) || !rig_detail::affine(local) ||
            !crew_detail::model_world(read, member.actor, observed))
            return false;
        member.native = rig_detail::world_from_definition(local, scene_world);
        if (!rig_detail::affine(member.native) || crew_detail::xyz_error(member.native, parked_car) > 12.)
            return false;
        member.target = member.native;
        for (unsigned i = 0; i < 3; ++i)
            member.target[12 + i] += delta[i];
        if (!rig_detail::affine(member.target))
            return false;
    }
    if (!crew_detail::roles_still_owned(read, owner, capture))
        return false;
    out = capture;
    return true;
}

template<class Read>
bool observe_central_crew(const Owner& owner, const CrewPlacement& saved, Read&& read, CrewReadback& out)
{
    if (!crew_detail::roles_still_owned(read, owner, saved))
        return false;
    CrewReadback observed{};
    for (unsigned i = 0; i < 2; ++i) {
        const auto& member = saved.members[i];
        if (!crew_detail::model_world(read, member.actor, observed.worlds[i]))
            return false;
        observed.native_error_m[i] = crew_detail::xyz_error(observed.worlds[i], member.native);
        observed.target_error_m[i] = crew_detail::xyz_error(observed.worlds[i], member.target);
    }
    out = observed;
    return true;
}

// Game-thread callback used by the bound core implementation. The dispatcher
// accepts only a member of the current capture and checks the game's read-only
// vtable and executable function ranges before following the scene refresh
// method. The engine takes three aligned float4 rows and the enabled flag.
template<class Read, class IsGameTable, class IsGameCode>
bool push_central_world(const Owner& owner, const CrewPlacement& saved, CrewRole role, const Matrix& value,
                        Read&& read, IsGameTable&& game_table, IsGameCode&& game_code)
{
    const unsigned index = static_cast<unsigned>(role);
    if (index >= 2 || (value != saved.members[index].native && value != saved.members[index].target) ||
        !rig_detail::affine(value) || !crew_detail::roles_still_owned(read, owner, saved))
        return false;
    const Address actor = saved.members[index].actor;
    return crew_detail::set_actor_world(actor,value,read,game_table,game_code);
}

// Restore also works during a departure sequence; its scope check should accept
// this owner's departure while still checking the active world and exact Pit bank.
// push accepts a CrewRole and Matrix and is normally bound to push_central_world.
template<class Read, class Scope, class Push>
CrewResult set_central_crew(const Owner& owner, const CrewPlacement& saved, bool restore,
                            Read&& read, Scope&& in_scope, Push&& push)
{
    if (!in_scope(owner) || !crew_detail::roles_still_owned(read, owner, saved))
        return CrewResult::unavailable;
    auto restore_pair = [&]() {
        bool ok = true;
        for (unsigned k = 0; k < 2; ++k) {
            if (!push(static_cast<CrewRole>(k), saved.members[k].native))
                ok = false;
        }
        CrewReadback actual{};
        return observe_central_crew(owner, saved, read, actual) && ok &&
               *std::max_element(actual.native_error_m.begin(), actual.native_error_m.end()) <= .001;
    };
    if (restore)
        return restore_pair() ? CrewResult::restored : CrewResult::rollback_failed;
    for (unsigned k = 0; k < 2; ++k) {
        if (!push(static_cast<CrewRole>(k), saved.members[k].target))
            return restore_pair() ? CrewResult::rolled_back : CrewResult::rollback_failed;
    }
    CrewReadback actual{};
    if (!observe_central_crew(owner, saved, read, actual) ||
        *std::max_element(actual.target_error_m.begin(), actual.target_error_m.end()) > .001)
        return restore_pair() ? CrewResult::rolled_back : CrewResult::rollback_failed;
    return CrewResult::applied;
}
} // namespace runtime::garage_placement
