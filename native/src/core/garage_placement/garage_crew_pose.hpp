// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <limits>

namespace runtime::garage_crew_pose {

using Read = bool (*)(std::uint64_t address, void *out, std::size_t bytes, void *user);
using FindNode = std::int32_t (*)(const void *node_table, const std::uint32_t *name_id);
struct Position { float x, y, z; };

namespace detail {
template <typename T>
inline bool field(Read read, void *user, std::uint64_t base, std::uint64_t offset, T &value)
{
    return base && offset <= std::numeric_limits<std::uint64_t>::max() - sizeof(T) &&
           base <= std::numeric_limits<std::uint64_t>::max() - offset - sizeof(T) &&
           read(base + offset, &value, sizeof(T), user);
}

struct Transform { float row[3][4]; };
static_assert(sizeof(Transform) == 48);

inline bool valid_transform(const Transform &matrix)
{
    for (const auto &row : matrix.row)
        for (float component : row)
            if (!std::isfinite(component))
                return false;
    const auto &m = matrix.row;
    const double det = double(m[0][0]) * (double(m[1][1])*m[2][2] - double(m[1][2])*m[2][1])
                     - double(m[0][1]) * (double(m[1][0])*m[2][2] - double(m[1][2])*m[2][0])
                     + double(m[0][2]) * (double(m[1][0])*m[2][1] - double(m[1][1])*m[2][0]);
    return det != 0.0;
}
} // namespace detail

// Call on the game evaluation thread with a live actor already identified as MB or EB1
// in the requested rig. Resolve the literal "Bip01" through Game_Get_ModelNodeId.
// FindNode is Ne_NodeTable_FindIndexByHash supplied by Swofty for this build.
// The POI consumer uses the actor-to-world transform; the parked car has its own getter.
inline bool read_pelvis(Read read, void *user, std::uint64_t actor, std::uint32_t bip01_id,
                        FindNode find, Position &out)
{
    using detail::field;
    std::uint64_t model = 0, skeleton = 0, table = 0;
    std::uint16_t count = 0;
    if (!read || !find || bip01_id > std::uint32_t(std::numeric_limits<std::int32_t>::max()) ||
        !field(read, user, actor, 0x228, model) || !field(read, user, model, 0xd0, skeleton) ||
        !field(read, user, skeleton, 0x48, table) || !field(read, user, table, 0, count) || !count)
        return false;
    const std::int32_t index = find(reinterpret_cast<const void *>(std::uintptr_t(table)), &bip01_id);
    // The camera refuses a zero index before entering the sampled-bone path.
    if (index <= 0 || index >= count)
        return false;

    std::uint64_t current_ptr = 0, flags = 0, bits = 0;
    std::uint8_t current = 0;
    if (!field(read, user, skeleton, 0x58, current_ptr) || !field(read, user, current_ptr, 0, current) ||
        current >= 3 || !field(read, user, skeleton, 0x30 + 8u * current, flags) ||
        !field(read, user, flags, 8u * (std::uint32_t(index) >> 6), bits))
        return false;
    const bool ready = (bits & (std::uint64_t{1} << (index & 63))) != 0;
    const unsigned previous = current == 0 ? 2u : unsigned(current - 1);
    const unsigned selected = ready ? current : previous;
    std::uint64_t buffer = 0;
    detail::Transform bone{}, world{};
    if (!field(read, user, skeleton, 0x18 + 8u * selected, buffer) ||
        !field(read, user, buffer, sizeof(bone) * std::uint64_t(index), bone) ||
        !field(read, user, actor, 0x400, world) || !detail::valid_transform(bone) ||
        !detail::valid_transform(world))
        return false;

    const auto &m = world.row;
    const float x = bone.row[0][3], y = bone.row[1][3], z = bone.row[2][3];
    const auto axis = [&](unsigned i) { return m[i][0]*x + m[i][1]*y + m[i][2]*z + m[i][3]; };
    const Position result{axis(0), axis(1), axis(2)};
    if (!std::isfinite(result.x) || !std::isfinite(result.y) || !std::isfinite(result.z))
        return false;
    out = result;
    return true;
}
} // namespace runtime::garage_crew_pose
