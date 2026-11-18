// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once

#include "garage_owner.hpp"

namespace runtime::garage_placement {

// Read after Game_PitBoxCarActor_Update. The actor's own cached world pose takes
// precedence. Uncached bones use model world, which differs from the actor
// transform used to place a mechanic's animated pelvis.
template<class Read>
bool parked_car_world(Read&& read, const Owner& owner, Matrix& out)
{
    const Address actor = owner.car_actor;
    Address model{};
    std::int32_t bone{};
    std::uint8_t cached{};
    if (!actor || !read(actor + 0x228, model) || !model || !read(actor + 0x678, bone) ||
        bone < 0 || bone >= 4096 || !read(actor + 0x660, cached))
        return false;

    Matrix world{};
    if (cached) {
        if (!read(actor + 0x620, world))
            return false;
    } else {
        Address skeleton{}, cursor{}, flags{}, buffer{};
        std::uint8_t current{};
        std::uint64_t ready{};
        if (!read(model + 0xd0, skeleton) || !skeleton || !read(skeleton + 0x58, cursor) || !cursor ||
            !read(cursor, current) || current > 2 || !read(skeleton + 0x30 + sizeof(Address) * current, flags) ||
            !flags || !read(flags + sizeof(ready) * (unsigned(bone) / 64), ready))
            return false;
        const bool evaluated = ((ready >> (unsigned(bone) % 64)) & 1u) != 0;
        const unsigned selected = evaluated ? current : (current + 2u) % 3u;
        std::array<float, 12> local{}, model_world{};
        if (!read(skeleton + 0x18 + sizeof(Address) * selected, buffer) || !buffer ||
            !read(buffer + sizeof(local) * unsigned(bone), local) || !read(model + 0x10, model_world))
            return false;
        for (unsigned row = 0; row < 3; ++row) {
            for (unsigned col = 0; col < 4; ++col) {
                float value = col == 3 ? model_world[4 * row + 3] : 0.f;
                for (unsigned k = 0; k < 3; ++k)
                    value += model_world[4 * row + k] * local[4 * k + col];
                world[4 * col + row] = value;
            }
        }
        world[15] = 1.f;
    }
    for (float v : world)
        if (!std::isfinite(v))
            return false;
    if (std::abs(world[3]) + std::abs(world[7]) + std::abs(world[11]) >= 0.001f ||
        std::abs(world[15] - 1.f) >= 0.001f)
        return false;
    out = world;
    return true;
}

} // namespace runtime::garage_placement
