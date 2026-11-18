#include "runtime.hpp"

#include <windows.h>

#include "theatre_list_logic.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <vector>

// Theatre list guard (re/db/functions.tsv Game_TheatreList_FindItemSaveSlot, F1_25.exe+0x2bfc6a7; EA +0x2bf0c37).
//
// Opening Theatre builds its two lists of saved replays and highlights (Game_TheatreRows_BuildLists, FUN_141f0eac0):
// for each saved-item record it calls the row builder FUN_141f0bbd0, which asks the list object at [vm + 0x970] for the
// record's save slot (slot +8 of the function table it is handed: Game_TheatreList_FindItemSaveSlot for the records at
// [list + 0x2a8], Game_TheatreList_FindItemSaveSlot_Alt for the records at [list + 0x2c8]) and returns false without a
// row when there is none.
// The record (0x1a0 bytes) names its owner by id at +0x8, and the lookup walks the flat hash set at
// [[list + 0x40] + 0x58] (control words at +0x60, one control byte per 8 bits of the tzcnt) for the entry whose
// [[[entry + 0x38] + 0x260] + 0x2a8] equals that id, then takes that owner's save slot manager and returns the slot
// record for the item, or 0 when it has none. The walk is the same one the garage setup guard covers
// (garage_setup.cpp): it has no end test and assumes the id is in the set. When no entry carries the id, because the
// item was recorded with a car or team the current list does not have, the walk reaches the set's end marker, takes
// the slot past the end array as an entry and faults reading [entry + 0x38] with a non-canonical entry
// (r9 = 0x2a00002f33000020 in every report, "reading 0xffffffffffffffff", 0.4 s after screen theatre_container).
//
// site_Game_TheatreList_EntryStep (and site_Game_TheatreList_EntryStepAlt in the clone) is the lea that follows the
// entry load in the walk's loop. When the entry in r9 is not a readable chain down to the id, the guard sets rax to 0
// and continues at site_Game_TheatreList_NoSlot (NoSlotAlt), the function's epilogue, which is how the function itself
// answers when the item has no slot record. An entry that reads through runs the game's code unchanged.
//
// The no-slot answer leaves a gap in the rows. The list builder passes the record index as the row index too, and the
// row getter Game_TheatreRows_GetRow (FUN_141f0b160) appends one row when the index is not below the row count, then
// reads the row at the index. After a record without a row, the next record's index is one past the new last row, so
// the getter reads its VM property handles from past the row array (F1_25.exe+0x1f0ba29, "reading 0x41200026", 1 ms
// after the no-slot line). site_Game_TheatreRows_RowOffset is the imul that turns the index into the row offset,
// after the getter has appended and stored the index as the row's slot_id. When the index is not below the row
// count, the guard points it at the last row, the one just appended. The row keeps the record index as its slot_id,
// which is what Theatre reads back to act on the selected item, so the rows stay in order with no gap and each tile
// still names its own record.
//
// SWOFTY_THEATRE_LIST_GUARD=0 turns all three off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (theatre list guard)";
constexpr const char *MIXIN_ALT = "swofty_core.dll (theatre saved list guard)";
constexpr const char *MIXIN_ROWS = "swofty_core.dll (theatre rows guard)";
constexpr const char *STEP = "site_Game_TheatreList_EntryStep";
constexpr const char *NO_SLOT = "site_Game_TheatreList_NoSlot";
constexpr const char *STEP_ALT = "site_Game_TheatreList_EntryStepAlt";
constexpr const char *NO_SLOT_ALT = "site_Game_TheatreList_NoSlotAlt";
constexpr const char *ROW_OFFSET = "site_Game_TheatreRows_RowOffset";

namespace tl = runtime::theatre_list;

uintptr_t no_slot = 0, no_slot_alt = 0;
bool offered = false, settled = false, rows_offered = false;
std::atomic<int> skipped{0}, moved{0};

bool read_memory(uint64_t address, void *out, size_t n, void *)
{
    if (!swofty::mem::readable(address, n))
        return false;
    memcpy(out, reinterpret_cast<const void *>(address), n);
    return true;
}

void skip(SwoftyContext *c, uintptr_t exit_at)
{
    tl::Exit exit;
    bool usable;
    {
        swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
        usable = tl::entry_step(c->r9, exit_at, read_memory, nullptr, &exit);
    }
    if (usable)
        return;
    c->rax = exit.rax;
    c->rip = exit.rip;
    int n = ++skipped;
    if (n > tl::LOG_FIRST)
        return;
    swofty::log("[theatre] a saved item names an owner that is not in the current list, so it has no save slot%s",
                n == tl::LOG_FIRST ? " (further skips are not logged)" : "");
}

void entry_step(SwoftyContext *c)
{
    skip(c, no_slot);
}

void entry_step_alt(SwoftyContext *c)
{
    skip(c, no_slot_alt);
}

void row_offset(SwoftyContext *c)
{
    uint64_t end = 0;
    {
        swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
        if (!read_memory(c->r13 + tl::ROWS_END_AT, &end, sizeof end, nullptr))
            return;
    }
    uint64_t index = tl::row_index(c->rdi, c->rbx, end);
    if (index == c->rdi)
        return;
    int n = ++moved;
    if (n <= tl::LOG_FIRST)
        swofty::log("[theatre] saved item %llu goes in row %llu, after an item that has no row%s",
                    (unsigned long long)c->rdi, (unsigned long long)index,
                    n == tl::LOG_FIRST ? " (further moves are not logged)" : "");
    c->rdi = index;
}

bool guard_off()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_THEATRE_LIST_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[theatre] list guard off (SWOFTY_THEATRE_LIST_GUARD=%s)", v);
        return true;
    }
    return false;
}

uintptr_t walk_site(const char *step, const char *exit_at)
{
    uintptr_t to = swofty::api->resolve(exit_at);
    if (to && swofty::api->resolve(step))
        return to;
    swofty::log("[theatre] list guard part off: needs %s and %s, which this build does not resolve", step, exit_at);
    return 0;
}

const char *state(const char *mixin, bool offered_it)
{
    if (!offered_it)
        return "not offered";
    return runtime::core_mixins_active(mixin) ? "armed" : "did not install (see the [core] lines)";
}

}

namespace runtime {

std::vector<CoreMixins> theatre_list_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    if (guard_off())
        return out;
    no_slot = walk_site(STEP, NO_SLOT);
    if (no_slot)
        out.push_back({MIXIN, {{STEP, SWOFTY_MIXIN_SITE, 100, (void *)&entry_step, nullptr}}});
    no_slot_alt = walk_site(STEP_ALT, NO_SLOT_ALT);
    if (no_slot_alt)
        out.push_back({MIXIN_ALT, {{STEP_ALT, SWOFTY_MIXIN_SITE, 100, (void *)&entry_step_alt, nullptr}}});
    rows_offered = swofty::api->resolve(ROW_OFFSET) != 0;
    if (rows_offered)
        out.push_back({MIXIN_ROWS, {{ROW_OFFSET, SWOFTY_MIXIN_SITE, 100, (void *)&row_offset, nullptr}}});
    else
        swofty::log("[theatre] rows guard off: needs %s, which this build does not resolve", ROW_OFFSET);
    return out;
}

void theatre_list_commit()
{
    if (!offered || settled || (!no_slot && !no_slot_alt && !rows_offered))
        return;
    settled = true;
    swofty::log("[theatre] list guard: recent list %s, saved list %s, rows %s; a saved item whose owner is gone has no "
                "save slot and the next item takes its row",
                state(MIXIN, no_slot), state(MIXIN_ALT, no_slot_alt), state(MIXIN_ROWS, rows_offered));
}

}
