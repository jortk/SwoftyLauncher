// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once
#include "garage_central.hpp"
#include "garage_car_pose.hpp"
#include <swofty/garage_placement.hpp>
#include <string_view>

namespace runtime::garage_placement::steam_26566308 {
namespace sdk = swofty::garage_placement;
inline bool supported(std::string_view distribution, std::string_view sha)
{
    return distribution == "steam" &&
           sha == "46009bd31f49d93a92d73cbd0b66131d2b37964a92da7293168d3c3a872d9389";
}

struct Bank {
    std::array<Matrix, 32> matrices{};
    uint32_t mask{};
};
inline bool authored(const sdk::Pit *pits, uint32_t count, uint32_t garage, Bank &out)
{
    if (!pits || !count || count > 32 || garage >= 32)
        return false;
    Bank bank{};
    for (uint32_t i = 0; i < count; ++i) {
        const auto index = pits[i].garage;
        if (index >= 32 || (bank.mask & (uint32_t{1} << index)))
            return false;
        bank.mask |= uint32_t{1} << index;
        std::copy_n(pits[i].world.values, 16, bank.matrices[index].begin());
        if (!rig_detail::affine(bank.matrices[index]))
            return false;
    }
    if (!(bank.mask & (uint32_t{1} << garage)))
        return false;
    out = bank;
    return true;
}

template<class Read>
bool matches(Read &&read, const Owner &owner, const Bank &bank)
{
    for (unsigned i = 0; i < 32; ++i) {
        Matrix live{};
        const bool expected = (bank.mask & (uint32_t{1} << i)) != 0;
        auto state = pit_transform(read, owner, i, live);
        if (state != (expected ? PitState::present : PitState::absent))
            return false;
        if (expected)
            for (unsigned k = 0; k < 16; ++k)
                if (std::abs(double(live[k]) - bank.matrices[i][k]) > (k >= 12 ? .0005 : .00001))
                    return false;
    }
    return true;
}

template<class Read>
bool owns(Read &&read, const Owner &o)
{
    Address crew{}, car{}, bank{};
    uint32_t garage{};
    return o.instance && o.module && o.crew_scene && o.car_actor && o.pit_bank && o.native_slot < 2 &&
           o.garage < 32 && read(o.module + 0x60, bank) && bank == o.pit_bank &&
           read(o.instance + 0x11c, garage) && garage == o.garage &&
           read(o.instance + 0xc8, crew) && crew == o.crew_scene &&
           read(o.instance + 0x1a8, car) && car == o.car_actor;
}
} // namespace runtime::garage_placement::steam_26566308
