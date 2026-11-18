// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once

#include "garage_owner.hpp"

namespace runtime::garage_placement {

enum class RootResult { unavailable, invalid_transform, applied, rolled_back, rollback_failed };

namespace rig_detail {
constexpr std::array<unsigned, 5> scenes{0xc8, 0xd0, 0xd8, 0xe0, 0xf0};

inline bool affine(const Matrix& m)
{
    for (float v : m) {
        if (!std::isfinite(v))
            return false;
    }
    const double det = double(m[0]) * (double(m[5]) * m[10] - double(m[6]) * m[9])
                     - double(m[1]) * (double(m[4]) * m[10] - double(m[6]) * m[8])
                     + double(m[2]) * (double(m[4]) * m[9] - double(m[5]) * m[8]);
    return std::abs(m[3]) < 0.0001f && std::abs(m[7]) < 0.0001f &&
           std::abs(m[11]) < 0.0001f && std::abs(m[15] - 1.f) < 0.0001f &&
           std::abs(det) > 1.e-8;
}

inline Matrix world_from_definition(const Matrix& definition, const Matrix& root)
{
    Matrix result{};
    for (unsigned row = 0; row < 4; ++row) {
        for (unsigned col = 0; col < 4; ++col) {
            for (unsigned k = 0; k < 4; ++k)
                result[4 * row + col] += definition[4 * row + k] * root[4 * k + col];
        }
    }
    return result;
}

struct SceneChange {
    Address scene{};
    Matrix before{}, after{};
};
} // namespace rig_detail

// Game-thread operation for a parked garage rig captured on the active session.
// The owner must be removed on despawn/unload. in_scope verifies active track,
// idle state, and the complete Pit bank. read/write are the core's checked local
// field accesses. refresh calls the verified game scene refresh for the supplied
// ActorScene; no game function addresses are stored or discovered here.
// applied acknowledges the rig update. The next evaluated model frame is the
// independent check of car pose and crew placement.
template<class Read, class Write, class Scope, class Refresh>
RootResult set_garage_root(const Owner& owner, const Matrix& root, Read&& read, Write&& write,
                           Scope&& in_scope, Refresh&& refresh)
{
    using namespace rig_detail;
    if (!affine(root))
        return RootResult::invalid_transform;
    std::uint32_t garage{};
    Address crew{}, car{}, current_bank{};
    Matrix old_root{};
    if (!owner.instance || !owner.pit_bank || !owner.module || !owner.crew_scene || !owner.car_actor ||
        owner.native_slot > 1 || owner.garage >= 32 ||
        !read(owner.module + 0x60, current_bank) || current_bank != owner.pit_bank || !in_scope(owner) ||
        !read(owner.instance + 0x11c, garage) || garage != owner.garage ||
        !read(owner.instance + 0xc8, crew) || crew != owner.crew_scene ||
        !read(owner.instance + 0x1a8, car) || car != owner.car_actor ||
        !read(owner.instance + 0x1c0, old_root) || !affine(old_root))
        return RootResult::unavailable;

    std::array<SceneChange, scenes.size()> changes{};
    unsigned count = 0;
    for (unsigned field : scenes) {
        Address scene{}, resource{};
        if (!read(owner.instance + field, scene))
            return RootResult::unavailable;
        if (!scene)
            continue;
        SceneChange change{};
        change.scene = scene;
        Matrix definition{};
        if (!read(scene + 0xa0, resource) || !resource ||
            !read(resource + 0x140, definition) || !affine(definition) ||
            !read(scene + 0x20, change.before) || !affine(change.before))
            return RootResult::unavailable;
        change.after = world_from_definition(definition, root);
        if (!affine(change.after))
            return RootResult::invalid_transform;
        changes[count++] = change;
    }
    Address renderer_actor{}, renderer{};
    Matrix old_renderer_root{};
    std::uint8_t old_dirty{};
    if (!count || !read(owner.instance + 0xe8, renderer_actor) ||
        (renderer_actor && !read(renderer_actor + 0x38, renderer)) ||
        (renderer && (!read(renderer + 0x1a80, old_renderer_root) || !affine(old_renderer_root) ||
                      !read(renderer + 0x1ac1, old_dirty))))
        return RootResult::unavailable;

    // Include failed write attempts in rollback since an access can be partial.
    unsigned scenes_touched = 0;
    bool renderer_touched = false, dirty_touched = false, refresh_started = false;
    auto rollback = [&]() {
        bool restored = write(owner.instance + 0x1c0, old_root);
        for (unsigned i = 0; i < scenes_touched; ++i) {
            if (!write(changes[i].scene + 0x20, changes[i].before))
                restored = false;
        }
        if (renderer_touched && !write(renderer + 0x1a80, old_renderer_root))
            restored = false;
        if (dirty_touched && !write(renderer + 0x1ac1, old_dirty))
            restored = false;
        if (refresh_started) {
            for (unsigned i = 0; i < count; ++i) {
                if (!refresh(changes[i].scene))
                    restored = false;
            }
        }
        return restored ? RootResult::rolled_back : RootResult::rollback_failed;
    };

    if (!write(owner.instance + 0x1c0, root))
        return rollback();
    for (unsigned i = 0; i < count; ++i) {
        ++scenes_touched;
        if (!write(changes[i].scene + 0x20, changes[i].after))
            return rollback();
    }
    if (renderer) {
        renderer_touched = true;
        if (!write(renderer + 0x1a80, root))
            return rollback();
        dirty_touched = true;
        if (!write(renderer + 0x1ac1, std::uint8_t{1}))
            return rollback();
    }
    refresh_started = true;
    for (unsigned i = 0; i < count; ++i) {
        if (!refresh(changes[i].scene))
            return rollback();
    }
    return RootResult::applied;
}

} // namespace runtime::garage_placement
