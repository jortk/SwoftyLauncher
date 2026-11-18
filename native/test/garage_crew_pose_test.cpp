// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#include "garage_crew_pose.hpp"

#include <cassert>
#include <cstdio>
#include <cstring>
#include <map>
#include <vector>

namespace pose = runtime::garage_crew_pose;
namespace {
using Address = std::uint64_t;
using Memory = std::map<Address, std::vector<unsigned char>>;
constexpr Address actor = 0x10000, model = 0x20000, skeleton = 0x30000, table = 0x40000;
constexpr Address active = 0x50000, flags = 0x60000, current = 0x70000, previous = 0x80000;
constexpr std::uint32_t bip01 = 743;
constexpr std::int32_t pelvis_index = 66;
constexpr std::uint64_t ready = std::uint64_t{1} << 2;

template <typename T> void put(Memory &memory, Address address, const T &value)
{
    auto &bytes = memory[address];
    bytes.resize(sizeof(value));
    std::memcpy(bytes.data(), &value, sizeof(value));
}

bool read(Address address, void *out, std::size_t size, void *user)
{
    auto &memory = *static_cast<Memory *>(user);
    const auto it = memory.find(address);
    if (it == memory.end() || it->second.size() != size)
        return false;
    std::memcpy(out, it->second.data(), size);
    return true;
}

std::int32_t find(const void *node_table, const std::uint32_t *name_id)
{
    assert(reinterpret_cast<std::uintptr_t>(node_table) == table);
    if (*name_id == 0)
        return 0;
    return *name_id == bip01 ? pelvis_index : -1;
}

struct Matrix { float rows[3][4]; };
Matrix translation(float x, float y, float z)
{
    return Matrix{{{1, 0, 0, x}, {0, 1, 0, y}, {0, 0, 1, z}}};
}

void failed(Memory &memory, std::uint32_t name = bip01)
{
    pose::Position result{9, 8, 7};
    assert(!pose::read_pelvis(read, &memory, actor, name, find, result));
    assert(result.x == 9 && result.y == 8 && result.z == 7);
}
} // namespace

int main()
{
    Memory memory;
    put(memory, actor + 0x228, model);
    put(memory, model + 0xd0, skeleton);
    put(memory, skeleton + 0x48, table);
    put(memory, table, std::uint16_t{75});
    put(memory, skeleton + 0x58, active);
    put(memory, active, std::uint8_t{0});
    put(memory, skeleton + 0x30, flags);
    put(memory, flags + 8, ready);
    put(memory, skeleton + 0x18, current);
    put(memory, skeleton + 0x28, previous);
    put(memory, current + 48 * pelvis_index, translation(2, 3, 5));
    put(memory, previous + 48 * pelvis_index, translation(7, 11, 13));
    put(memory, model + 0x10, translation(1000, 2000, 3000));
    const Matrix world{{{0, 0, 1, 10}, {0, 1, 0, 20}, {-1, 0, 0, 30}}};
    put(memory, actor + 0x400, world);

    pose::Position result{};
    assert(pose::read_pelvis(read, &memory, actor, bip01, find, result));
    assert(result.x == 15 && result.y == 23 && result.z == 28);
    put(memory, flags + 8, std::uint64_t{1});
    assert(pose::read_pelvis(read, &memory, actor, bip01, find, result));
    assert(result.x == 23 && result.y == 31 && result.z == 23);
    memory.erase(skeleton + 0x28);
    failed(memory);
    // No ready bit for slot 1. The previous slot is 0, irrespective of slot 2.
    put(memory, active, std::uint8_t{1});
    put(memory, skeleton + 0x38, flags);
    assert(pose::read_pelvis(read, &memory, actor, bip01, find, result));
    assert(result.x == 15 && result.y == 23 && result.z == 28);
    failed(memory, 802);
    failed(memory, 0);
    put(memory, table, std::uint16_t{66});
    failed(memory);
    put(memory, table, std::uint16_t{75});
    put(memory, active, std::uint8_t{3});
    failed(memory);
    put(memory, active, std::uint8_t{0});
    put(memory, flags + 8, ready);
    put(memory, actor + 0x400, Matrix{});
    failed(memory);
    put(memory, actor + 0x400, world);
    put(memory, current + 48 * pelvis_index, translation(std::numeric_limits<float>::infinity(), 0, 0));
    failed(memory);
    std::puts("garage crew animated pelvis: passed");
}
