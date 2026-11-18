#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "main_menu.hpp"
#include "main_menu_logic.hpp"
#include "perf.hpp"
#include "runtime.hpp"

using namespace swofty::game;

namespace badge_images {
void trace(const char *message) { swofty::log("[badges] %s", message); }
}

bool settings_command(uint32_t hash);

#if defined(SWOFTY_NO_MOD_SETTINGS)
#define SWOFTY_MOD_SETTINGS_MODE "compiled out (diagnostic build: no site hook, no tile)"
#elif defined(SWOFTY_MOD_SETTINGS_NOOP)
#define SWOFTY_MOD_SETTINGS_MODE "site hook installed, handler is a no-op (diagnostic build: no tile)"
#else
#define SWOFTY_MOD_SETTINGS_MODE "active"
#define SWOFTY_MOD_SETTINGS_ACTIVE 1
#endif

namespace {

static_assert(sizeof(TileEntry) == 0x10);

unsigned long long rva(uintptr_t a)
{
    return a - swofty::api->module_base();
}

#ifdef SWOFTY_MOD_SETTINGS_ACTIVE

constexpr const char *ROUTE = "mod_menu";
constexpr const char *TITLE = "ADDON SETTINGS";
constexpr const char *DESCRIPTION = "Configure the addons installed by Swofty Launcher";

unsigned hook_calls, populate_calls, action_calls;
void **mod_vt;

void set_string(void *reg, void *parent, const char *name, const char *text)
{
    NeStr *s = nullptr;
    Ne_Make_StrFromCStr(&s, text, 0, 0);
    Vm_Set_String(reg, parent, name, &s, 1);
    Ne_Make_StrFromCStr(&s, nullptr, 0, 0);
}

void mod_populate(HomeItem *item, void *node, uint32_t a7)
{
    unsigned call = ++populate_calls;
    FeTile_ItemBase_Populate(item, node, a7);
    auto vm_node = static_cast<uint8_t *>(item->button_node.node);
    if (!vm_node) {
        swofty::log("[core] populate #%u item=%p: no VM node after base populate, label left as is", call, item);
        return;
    }
    void *reg = *g_Vm_Registry, *parent = vm_node + 8;
    uint8_t no = 0;
    set_string(reg, parent, "title", TITLE);
    Vm_Set_Bool(reg, parent, "title_localised", &no, 1);
    set_string(reg, parent, "description", DESCRIPTION);
    Vm_Set_Bool(reg, parent, "description_localised", &no, 1);
    swofty::log("[core] populate #%u item=%p node=%p a7=%u vm=%p: title='%s' description set, localised=false", call,
                item, node, a7, vm_node, TITLE);
    runtime::main_menu::style_settings(item);
}

Action *mod_get_action(HomeItem *item, Action *out)
{
    out->type = 1;
    out->str = nullptr;
    Ne_Make_StrFromCStr(&out->str, ROUTE, 0, 0);
    swofty::log("[core] getAction #%u item=%p -> {type 1, route '%s'}", ++action_calls, item, ROUTE);
    return out;
}

HomeItem *last_item(TileList *list, size_t *count)
{
    auto begin = reinterpret_cast<uintptr_t>(list->items_begin), end = reinterpret_cast<uintptr_t>(list->items_end);
    if (!begin || end <= begin || (end - begin) % sizeof(TileEntry))
        return nullptr;
    *count = list->items_end - list->items_begin;
    Tile *tile = list->items_end[-1].tile;
    return tile ? tile->item : nullptr;
}

#endif

bool check()
{
    uintptr_t site = site_FeHome_AfterCareerCall.address(), builder = FeTile_Build_List.address();
    swofty::log("[core] swofty_core init, game base 0x%llx", (unsigned long long)swofty::api->module_base());
    swofty::log("[core] ADDON SETTINGS mixin: %s", SWOFTY_MOD_SETTINGS_MODE);
    if (site < builder || site - builder > 0x8000) {
        swofty::log("[core] hook site 0x%llx is not inside the builder at 0x%llx; installing nothing",
                    (unsigned long long)site, (unsigned long long)builder);
        return false;
    }
    swofty::log("[core] builder base+0x%llx, hook site base+0x%llx", rva(builder), rva(site));
    swofty::log("[core] career_adder base+0x%llx, base_populate base+0x%llx, nestr_from base+0x%llx",
                rva(FeHome_Add_CareerTile.address()), rva(FeTile_ItemBase_Populate.address()),
                rva(Ne_Make_StrFromCStr.address()));
    swofty::log("[core] registry global base+0x%llx (= %p now), vm_set_string base+0x%llx, vm_set_bool base+0x%llx",
                rva(g_Vm_Registry.address()), *g_Vm_Registry, rva(Vm_Set_String.address()), rva(Vm_Set_Bool.address()));
    return true;
}

}

#if defined(SWOFTY_MOD_SETTINGS_NOOP)

SWOFTY_SITE(site_FeHome_AfterCareerCall, swofty::Context &c)
{
    static unsigned calls;
    swofty::log("[core] hook #%u (no-op): list(r14)=0x%llx r12=0x%llx r15=0x%llx; nothing added", ++calls,
                (unsigned long long)c.r14, (unsigned long long)c.r12, (unsigned long long)c.r15);
}

#elif defined(SWOFTY_MOD_SETTINGS_ACTIVE)

SWOFTY_SITE(site_FeHome_AfterCareerCall, swofty::Context &c)
{
    unsigned call = ++hook_calls;
    auto list = reinterpret_cast<TileList *>(c.r14);
    swofty::log("[core] hook #%u: list(r14)=0x%llx r12=0x%llx r15=0x%llx rsi=0x%llx rip=0x%llx", call,
                (unsigned long long)c.r14, (unsigned long long)c.r12, (unsigned long long)c.r15,
                (unsigned long long)c.rsi, (unsigned long long)c.rip);
    size_t before = 0, after = 0;
    HomeItem *career = last_item(list, &before);
    if (!career) {
        swofty::log("[core] hook #%u: items vector does not end with a tile, nothing added", call);
        return;
    }
    auto career_vt = reinterpret_cast<void **>(career->vtable);
    uint8_t last = runtime::main_menu::add_tiles(list, uint8_t(c.r12), reinterpret_cast<const uint8_t *>(c.rsi));
    last_item(list, &before);
    uint8_t index = uint8_t(last + 1);
    FeHome_Add_CareerTile(list, index);
    HomeItem *item = last_item(list, &after);
    c.r12 = index;
    c.r15 = index;
    if (!item || after != before + 1 || reinterpret_cast<void **>(item->vtable) != career_vt) {
        swofty::log("[core] hook #%u: adder result unexpected (items %zu -> %zu, item %p, vt %p vs career %p); tile "
                    "left as a plain Career copy",
                    call, before, after, item, item ? static_cast<void *>(item->vtable) : nullptr, career_vt);
        return;
    }
    void **vt = swofty::subclass(item, {{4, reinterpret_cast<void *>(mod_populate)}, {5, reinterpret_cast<void *>(mod_get_action)}});
    if (vt != mod_vt) {
        mod_vt = vt;
        swofty::log("[core] hook #%u: built mod vtable %p from career vtable %p (slot 4 populate %p -> %p, slot 5 "
                    "getAction %p -> %p)",
                    call, vt, career_vt, career_vt[4], vt[4], career_vt[5], vt[5]);
    }
    NeStr *old_id = item->id;
    Ne_Make_StrFromCStr(&item->id, ROUTE, 0, 0);
    runtime::main_menu::style_settings(item);
    swofty::log("[core] hook #%u: added button_%u (items %zu -> %zu) item=%p, id %p -> %p '%s' (old id released by "
                "the game's string setter), r12 -> 0x%llx, r15 -> 0x%llx",
                call, (unsigned)index, before, after, item, old_id, item->id, ROUTE, (unsigned long long)c.r12,
                (unsigned long long)c.r15);
}

#endif

SWOFTY_WRAP(FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    static const uint32_t probe = perf::probe("core", "home command (addon settings)");
    if (vm && cmd && *cmd)
        runtime::main_menu::command(vm, *reinterpret_cast<const uint32_t *>(*cmd),
                                    *reinterpret_cast<const uint32_t *>(reinterpret_cast<const uint8_t *>(vm) + 0x3f0));
    {
        perf::Scope p(probe);
        settings_command(cmd && *cmd ? *reinterpret_cast<const uint32_t *>(*cmd) : 0x1505);
    }
    runtime::session_host_home(vm);
    runtime::testagent_home_vm(vm);
    return orig(vm, cmd);
}

// F1 THE MOVIE home tile. The home case of FeTile_Build_List adds it (FeHome_Add_MovieTile at index
// r15b+2, then r12 = r15) only while feature flag bit 42 ([rsi+5] & 4) is set; with the flag clear the
// je skips to the F1 World block, which numbers itself r12+1 from the tile before. The anchor is the
// movie block's first instruction, the je's fall-through right after the 6 bytes the
// site_FeHome_AfterCareerCall hook relocates; a short jmp there to the F1 World block builds the
// flag-clear list, so the button numbering stays contiguous. No branch lands in the 2 replaced bytes.
namespace {

const runtime::Sig S_MOVIE_TILE{"FeHome_MovieTileBlock",
                                "41 80 C7 02 4C 89 F1 44 89 FA E8 ?? ?? ?? ?? 45 89 FC 45 8D 7C 24 01 4C 89 F1 44 89 FA E8",
                                0x1742fdc, runtime::Sig::addr, 0, 0, 0};

void hide_movie_tile()
{
    static const uint8_t skip_movie[] = {0xEB, 0x10};
    auto at = reinterpret_cast<void *>(runtime::find(S_MOVIE_TILE, "home", "core"));
    DWORD old;
    bool hidden = at && VirtualProtect(at, sizeof skip_movie, PAGE_EXECUTE_READWRITE, &old);
    if (hidden) {
        memcpy(at, skip_movie, sizeof skip_movie);
        VirtualProtect(at, sizeof skip_movie, old, &old);
        FlushInstructionCache(GetCurrentProcess(), at, sizeof skip_movie);
    }
    swofty::log("[core] F1 THE MOVIE home tile %s", hidden ? "hidden" : "left in place (signature did not resolve)");
}

// Local Multiplayer list. FeTile_Build_List (base+0x173df80) builds menu_kind 2 as Splitscreen
// (base+0x1747f90, button_1) and then an inline LAN tile (id "lan", item vtable base+0x5704740,
// button_2) that the shared tail pushes into the item vector. The anchor is the instruction after the
// Splitscreen call; a jmp there to the tail the adder-only menus jump to (base+0x1745016, after the
// temporaries' cleanup, at the item init/populate loop) leaves Splitscreen as the only entry. No
// branch lands in the 9 replaced bytes.
const runtime::Sig S_LAN_TILE{"FeTile_LocalMp_LanTile",
                              "4C 8D 05 ?? ?? ?? ?? 4C 89 F1 B2 01 E8 ?? ?? ?? ?? 48 C7 44 24 38 00 00 00 00",
                              0x173e7a7, runtime::Sig::addr, 0, 0x11, 0};
const runtime::Sig S_TILE_TAIL{"FeTile_Build_List_ItemsTail", "4C 89 F1 E8 ?? ?? ?? ?? E9 ?? ?? ?? ?? E8 ?? ?? ?? ?? B2 01",
                               0x1745016, runtime::Sig::rel32, 8, 9, 5};

void hide_lan_tile()
{
    uintptr_t lan = runtime::find(S_LAN_TILE, "core", "core"), tail = runtime::find(S_TILE_TAIL, "core", "core");
    bool hidden = false;
    if (lan && tail && tail > lan && tail - lan < 0x10000) {
        uint8_t skip_lan[9] = {0xE9, 0, 0, 0, 0, 0x90, 0x90, 0x90, 0x90};
        int32_t rel = int32_t(tail - (lan + 5));
        memcpy(skip_lan + 1, &rel, sizeof rel);
        auto at = reinterpret_cast<void *>(lan);
        DWORD old;
        hidden = VirtualProtect(at, sizeof skip_lan, PAGE_EXECUTE_READWRITE, &old);
        if (hidden) {
            memcpy(at, skip_lan, sizeof skip_lan);
            VirtualProtect(at, sizeof skip_lan, old, &old);
            FlushInstructionCache(GetCurrentProcess(), at, sizeof skip_lan);
        }
    }
    swofty::log("[core] LAN Game entry in Local Multiplayer %s",
                hidden ? "hidden (Splitscreen only)" : "left in place (signature did not resolve)");
}

}

namespace runtime {

void local_tiles_setup()
{
    hide_lan_tile();
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("core: F1 THE MOVIE home tile", hide_movie_tile);
}

SWOFTY_MOD(check)
