#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct EntrantRecord {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x40];
    uint32_t ordinal_flags; // L2 experimental
    uint8_t _pad_0x4c[0x4];
    uint8_t id_lo; // L2 experimental
    uint8_t _pad_0x51[0x1];
    uint16_t id_hi; // L2 experimental
    uint32_t driver_customisation; // L2 experimental
    uint32_t team_stat_id; // L2 experimental
    uint32_t vehicle_ref; // L2 experimental
    uint32_t driver_reg_value; // L2 experimental
    uint32_t vehicle_customisation; // L2 experimental
    uint32_t badge_stat_id; // L2 experimental
    uint8_t _pad_0x6c[0x4];
    void **subobj_vtable; // L2 experimental
    uint32_t const_ffff0012; // L2 experimental
    uint8_t _pad_0x7c[0x4];
    uint16_t team_position; // L2 experimental
    uint8_t _pad_0x82[0x1];
    uint8_t car_number; // L2 experimental
    uint8_t _pad_0x84[0x4];
};
static_assert(offsetof(EntrantRecord, vtable) == 0x0);
static_assert(offsetof(EntrantRecord, ordinal_flags) == 0x48);
static_assert(offsetof(EntrantRecord, id_lo) == 0x50);
static_assert(offsetof(EntrantRecord, id_hi) == 0x52);
static_assert(offsetof(EntrantRecord, driver_customisation) == 0x54);
static_assert(offsetof(EntrantRecord, team_stat_id) == 0x58);
static_assert(offsetof(EntrantRecord, vehicle_ref) == 0x5c);
static_assert(offsetof(EntrantRecord, driver_reg_value) == 0x60);
static_assert(offsetof(EntrantRecord, vehicle_customisation) == 0x64);
static_assert(offsetof(EntrantRecord, badge_stat_id) == 0x68);
static_assert(offsetof(EntrantRecord, subobj_vtable) == 0x70);
static_assert(offsetof(EntrantRecord, const_ffff0012) == 0x78);
static_assert(offsetof(EntrantRecord, team_position) == 0x80);
static_assert(offsetof(EntrantRecord, car_number) == 0x83);
static_assert(sizeof(EntrantRecord) == 0x88);

struct EntrantWriteData {
    uint32_t driver; // L2 experimental
    uint32_t driver_customisation; // L2 experimental
    uint32_t team_stat_id; // L2 experimental
    uint32_t vehicle_ref; // L2 experimental
    uint32_t driver_reg_value; // L2 experimental
    uint32_t vehicle_customisation; // L2 experimental
    uint32_t badge_stat_id; // L2 experimental
    uint8_t _pad_0x1c[0xc];
    uint32_t seat_value; // L2 experimental
};
static_assert(offsetof(EntrantWriteData, driver) == 0x0);
static_assert(offsetof(EntrantWriteData, driver_customisation) == 0x4);
static_assert(offsetof(EntrantWriteData, team_stat_id) == 0x8);
static_assert(offsetof(EntrantWriteData, vehicle_ref) == 0xc);
static_assert(offsetof(EntrantWriteData, driver_reg_value) == 0x10);
static_assert(offsetof(EntrantWriteData, vehicle_customisation) == 0x14);
static_assert(offsetof(EntrantWriteData, badge_stat_id) == 0x18);
static_assert(offsetof(EntrantWriteData, seat_value) == 0x28);
static_assert(sizeof(EntrantWriteData) == 0x2c);

using Ent_Deserialise_EntrantVector_t = int(void *vector_object, void *bag);
inline constexpr fn<"Ent_Deserialise_EntrantVector", Ent_Deserialise_EntrantVector_t> Ent_Deserialise_EntrantVector{}; // L2 experimental
using Ent_EntrantsSystem_ReplaceVehicle_t = int(void *entrants, uint32_t handle, const void *vehicle);
inline constexpr fn<"Ent_EntrantsSystem_ReplaceVehicle", Ent_EntrantsSystem_ReplaceVehicle_t> Ent_EntrantsSystem_ReplaceVehicle{}; // L2 experimental

}
