#pragma once
#include <swofty/game/base.hpp>
#include <swofty/game/vm.hpp>

namespace swofty::game {

struct Action {
    int32_t type; // L2 experimental
    uint8_t _pad_0x4[0x4];
    NeStr *str; // L2 experimental
};
static_assert(offsetof(Action, type) == 0x0);
static_assert(offsetof(Action, str) == 0x8);

struct HomeItem {
    ItemVtbl *vtable; // L3 stable
    uint8_t _pad_0x8[0x8];
    NeStr *id; // L2 experimental
    VmPropHolder button_node; // L3 stable
};
static_assert(offsetof(HomeItem, vtable) == 0x0);
static_assert(offsetof(HomeItem, id) == 0x10);
static_assert(offsetof(HomeItem, button_node) == 0x18);

struct ItemVtbl {
    uint8_t _pad_0x0[0x20];
    void *populate; // L3 stable
    void *get_action; // L3 stable
};
static_assert(offsetof(ItemVtbl, populate) == 0x20);
static_assert(offsetof(ItemVtbl, get_action) == 0x28);

struct Tile {
    uint8_t _pad_0x0[0xd0];
    HomeItem *item; // L3 stable
};
static_assert(offsetof(Tile, item) == 0xd0);

struct TileEntry {
    uint8_t _pad_0x0[0x8];
    Tile *tile; // L3 stable
};
static_assert(offsetof(TileEntry, tile) == 0x8);

struct TileList {
    uint8_t _pad_0x0[0x78];
    TileEntry *items_begin; // L3 stable
    TileEntry *items_end; // L3 stable
};
static_assert(offsetof(TileList, items_begin) == 0x78);
static_assert(offsetof(TileList, items_end) == 0x80);

using FeTile_Add_EraTile_t = void(void *tiles, void *alloc, uint8_t index, const char *node, const char *name, uint8_t enabled, const char *title, const char *command, const char *image, uint8_t is_dlc);
inline constexpr fn<"FeTile_Add_EraTile", FeTile_Add_EraTile_t> FeTile_Add_EraTile{}; // L2 experimental
using FeTile_Build_List_t = void(TileList *self, void *ctx, void *a3, void *a4, uint8_t menu_kind, uint8_t sub_kind, uint32_t a7);
inline constexpr fn<"FeTile_Build_List", FeTile_Build_List_t> FeTile_Build_List{}; // L3 stable
using FeTile_CareerDlcItem_Populate_t = void(HomeItem *item, void *node, uint32_t a7);
inline constexpr fn<"FeTile_CareerDlcItem_Populate", FeTile_CareerDlcItem_Populate_t> FeTile_CareerDlcItem_Populate{}; // L2 experimental
using FeTile_Check_EntrantListUsable_t = uint64_t(void *tile_ctx, const char *label);
inline constexpr fn<"FeTile_Check_EntrantListUsable", FeTile_Check_EntrantListUsable_t> FeTile_Check_EntrantListUsable{}; // L2 experimental
using FeTile_Clear_EraTileSet_Thunk_t = void(void *tiles);
inline constexpr fn<"FeTile_Clear_EraTileSet_Thunk", FeTile_Clear_EraTileSet_Thunk_t> FeTile_Clear_EraTileSet_Thunk{}; // L2 experimental
using FeTile_Construct_CareerItem_t = HomeItem *(HomeItem *item, NeAllocator *alloc, const char *id);
inline constexpr fn<"FeTile_Construct_CareerItem", FeTile_Construct_CareerItem_t> FeTile_Construct_CareerItem{}; // L2 experimental
using FeTile_Get_ListFirstEntrantId_t = void *(void *item, void *out);
inline constexpr fn<"FeTile_Get_ListFirstEntrantId", FeTile_Get_ListFirstEntrantId_t> FeTile_Get_ListFirstEntrantId{}; // L2 experimental
using FeTile_Is_EraScenarioAvailable_Thunk_t = uint8_t(void *mode, const char *setup_mode, const char *entrant_list);
inline constexpr fn<"FeTile_Is_EraScenarioAvailable_Thunk", FeTile_Is_EraScenarioAvailable_Thunk_t> FeTile_Is_EraScenarioAvailable_Thunk{}; // L2 experimental
using FeTile_ItemBase_Populate_t = void(HomeItem *item, void *node, uint32_t a7);
inline constexpr fn<"FeTile_ItemBase_Populate", FeTile_ItemBase_Populate_t> FeTile_ItemBase_Populate{}; // L3 stable
inline constexpr address<"FeTile_ItemBase_Slot78_Thunk"> FeTile_ItemBase_Slot78_Thunk{}; // L2 experimental
inline constexpr address<"FeTile_ItemGeneric_Populate"> FeTile_ItemGeneric_Populate{}; // L2 experimental
using FeTile_Press_EraTile_t = char *(void *view_model, void *command);
inline constexpr fn<"FeTile_Press_EraTile", FeTile_Press_EraTile_t> FeTile_Press_EraTile{}; // L2 experimental
using FeTile_Route_CareerItem_t = Action *(HomeItem *item, Action *out);
inline constexpr fn<"FeTile_Route_CareerItem", FeTile_Route_CareerItem_t> FeTile_Route_CareerItem{}; // L2 experimental
inline constexpr data<"g_FeTile_Ctx", void *> g_FeTile_Ctx{}; // L2 experimental
inline constexpr data<"g_FeTile_CustomisationSystem", void *> g_FeTile_CustomisationSystem{}; // L2 experimental
inline constexpr data<"g_FeTile_SelectedCareerContent", uint32_t> g_FeTile_SelectedCareerContent{}; // L2 experimental
inline constexpr site<"site_FeTile_CareerContentEnd"> site_FeTile_CareerContentEnd{}; // L2 experimental
inline constexpr site<"site_FeTile_EraTilesBuilt"> site_FeTile_EraTilesBuilt{}; // L2 experimental
inline constexpr site<"site_FeTile_ListRowEntry"> site_FeTile_ListRowEntry{}; // L2 experimental
inline constexpr site<"site_FeTile_ListRowFieldLoad"> site_FeTile_ListRowFieldLoad{}; // L2 experimental
inline constexpr site<"site_FeTile_ListRowNone"> site_FeTile_ListRowNone{}; // L2 experimental
inline constexpr site<"site_FeTile_ListScanNone"> site_FeTile_ListScanNone{}; // L2 experimental
inline constexpr site<"site_FeTile_ListScanStart"> site_FeTile_ListScanStart{}; // L2 experimental

}
