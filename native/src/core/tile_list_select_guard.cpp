#include "runtime.hpp"

#include <windows.h>

#include "tile_list_select_logic.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <vector>

// Tile list select guard (re/db/functions.tsv Game_TileList_ApplyBoundSelection, F1_25.exe+0x3838fe8; EA +0x3826228).
//
// A list widget whose selection is bound to a view-model property (Choose Driver's tile_list, selection=@selected_driver)
// listens on that binding with Game_TileList_ApplyBoundSelection (0x143838d90). When the property changes, it walks the
// widget's children [widget + 0x130 .. + 0x138] for the one whose data-context component is bound to the new node:
// [child + 0x30] is the child's component array, scanned with no end test and no null test for the component of the
// data-context type. A child with no component array faults reading 0 at F1_25.exe+0x3838fe8 (fn + 0x258).
// Choose Driver hits it on the F2 drivers tab: the tile list respawns its tiles, raises children_triggered (0x20f2f60a),
// and the view model's slot-10 handler (0x141a5df70 + 0xa4, EA 0x141a58460) sets selected_driver (vm + 0x298) to
// drivers[vm + 0x430] through Ne_Property_SetS64IfChanged, which runs the walk while one of the tiles has no components.
// Eight reports, Steam and EA, with More Driver Models, Driver Manager, Progressive's cards or the Classic Seasons wizard.
//
// site_Game_TileListSelect_ChildStep is the head of the walk (mov rdi,[r15]; mov r14,[rdi+0x30]), r15 the cursor into
// the children and r12 their end. When the child under the cursor is not readable, has no component array or has an
// unreadable first component, the guard continues at site_Game_TileListSelect_ChildNext, the walk's own step to the
// next child (add r15,8 ... jne head; past the last child the game's no-match exit). The tile stays on screen without
// the selection: a child that cannot carry the data context is never the bound child. Children that read through run
// the game's code unchanged.
//
// The same widget writes the other way through Game_TileList_PushChildSelection_Thunk (0x14383b390, a jmp to the .data1
// body 0x15178c2a0; EA 0x1438285d0): rcx = the widget, rdx = a child, in every caller the focused child
// [widget + 0x108]. The body returns at once for a null child or a widget with no selection binding [widget + 0x60];
// otherwise it scans the widget's component array and then the child's (mov r13,[rsi+0x30] at body + 0xc2, again with
// no null test) for the data-context component and writes the child's node into the bound property. A focused child
// with no component array faults reading 0 at F1_25.exe+0x1178c378, 1 ms after the walk above skipped the same tiles
// (GDPika, 1.9.47). The thunk has five callers, one of them a .data1 body, so the guard wraps the thunk: when the
// widget or the child has no readable component array it returns without running the body, which is the body's own
// answer for a null child. The bound property keeps its value.
//
// SWOFTY_TILE_LIST_SELECT_GUARD=0 turns both off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (tile list select guard)";
constexpr const char *STEP = "site_Game_TileListSelect_ChildStep";
constexpr const char *NEXT = "site_Game_TileListSelect_ChildNext";
constexpr const char *PUSH_MIXIN = "swofty_core.dll (tile list push guard)";
constexpr const char *PUSH = "Game_TileList_PushChildSelection_Thunk";

namespace ts = runtime::tile_list_select;

using Push = void(void *widget, void *child);

uintptr_t next_child = 0;
Push *orig_push = nullptr;
bool offered = false, settled = false, push_offered = false;
std::atomic<int> skipped{0};
std::atomic<int> push_skipped{0};

bool read_memory(uint64_t address, void *out, size_t n, void *)
{
    if (!swofty::mem::readable(address, n))
        return false;
    memcpy(out, reinterpret_cast<const void *>(address), n);
    return true;
}

void child_step(SwoftyContext *c)
{
    ts::Child child;
    ts::Position at;
    {
        swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
        child = ts::check_child(c->r15, read_memory, nullptr);
        if (child == ts::Child::Usable)
            return;
        uint64_t widget = 0, begin = 0;
        if (read_memory(c->rsp + ts::WIDGET_AT_RSP, &widget, sizeof widget, nullptr) && ts::user_pointer(widget) &&
            read_memory(widget + ts::CHILDREN_BEGIN_AT, &begin, sizeof begin, nullptr))
            at = ts::position(c->r15, c->r12, begin);
    }
    c->rip = next_child;
    int n = ++skipped;
    if (n > ts::LOG_FIRST)
        return;
    if (at.known)
        swofty::log("[tile-list] tile %llu of %llu %s, so the list's selection skips it%s", (unsigned long long)at.index,
                    (unsigned long long)at.count, ts::why(child),
                    n == ts::LOG_FIRST ? " (further skips are not logged)" : "");
    else
        swofty::log("[tile-list] a tile %s, so the list's selection skips it%s", ts::why(child),
                    n == ts::LOG_FIRST ? " (further skips are not logged)" : "");
}

void push_child_selection(void *widget, void *child)
{
    if (!child) {
        orig_push(widget, child);
        return;
    }
    uint64_t w = reinterpret_cast<uint64_t>(widget), t = reinterpret_cast<uint64_t>(child);
    ts::Child list, tile;
    ts::Position at;
    uint64_t vtable = 0;
    {
        swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
        list = ts::check_tile(w, read_memory, nullptr);
        tile = list == ts::Child::Usable ? ts::check_tile(t, read_memory, nullptr) : ts::Child::Usable;
        if (list != ts::Child::Usable || tile != ts::Child::Usable) {
            at = ts::find_child(w, t, read_memory, nullptr);
            if (ts::user_pointer(t))
                read_memory(t, &vtable, sizeof vtable, nullptr);
        }
    }
    if (list == ts::Child::Usable && tile == ts::Child::Usable) {
        orig_push(widget, child);
        return;
    }
    int n = ++push_skipped;
    if (n > ts::LOG_FIRST)
        return;
    const char *last = n == ts::LOG_FIRST ? " (further skips are not logged)" : "";
    if (list != ts::Child::Usable)
        swofty::log("[tile-list] the list %s, so it does not write its focused tile to its selection%s", ts::why(list), last);
    else if (at.known)
        swofty::log("[tile-list] focused tile %llu of %llu (node 0x%llx, class 0x%llx) %s, so the list does not write it "
                    "to its selection%s",
                    (unsigned long long)at.index, (unsigned long long)at.count, (unsigned long long)t,
                    (unsigned long long)vtable, ts::why(tile), last);
    else
        swofty::log("[tile-list] the focused tile (node 0x%llx, class 0x%llx) %s and is not among the list's tiles, so the "
                    "list does not write it to its selection%s",
                    (unsigned long long)t, (unsigned long long)vtable, ts::why(tile), last);
}

bool guard_off()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_TILE_LIST_SELECT_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[tile-list] select guard off (SWOFTY_TILE_LIST_SELECT_GUARD=%s)", v);
        return true;
    }
    return false;
}

}

namespace runtime {

std::vector<CoreMixins> tile_list_select_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    if (guard_off())
        return out;
    next_child = swofty::api->resolve(NEXT);
    if (!next_child || !swofty::api->resolve(STEP)) {
        next_child = 0;
        swofty::log("[tile-list] select guard off: needs %s and %s, which this build does not resolve", STEP, NEXT);
    } else {
        out.push_back({MIXIN, {{STEP, SWOFTY_MIXIN_SITE, 100, (void *)&child_step, nullptr}}});
    }
    if (!swofty::api->resolve(PUSH)) {
        swofty::log("[tile-list] push guard off: needs %s, which this build does not resolve", PUSH);
    } else {
        push_offered = true;
        out.push_back({PUSH_MIXIN, {{PUSH, SWOFTY_MIXIN_WRAP, 100, (void *)&push_child_selection, (void **)&orig_push}}});
    }
    return out;
}

void tile_list_select_commit()
{
    if (!offered || settled)
        return;
    settled = true;
    if (next_child)
        swofty::log("[tile-list] select guard %s; a tile with no components is skipped when a list moves its selection",
                    core_mixins_active(MIXIN) ? "armed" : "did not install (see the [core] lines)");
    if (push_offered)
        swofty::log("[tile-list] push guard %s; a focused tile with no components is not written to a list's selection",
                    core_mixins_active(PUSH_MIXIN) ? "armed" : "did not install (see the [core] lines)");
}

}
