#include "replay_grid_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Replay recorder holder guard (re/db/globals.tsv site_Ne_ReplayAfterRecord).
//
// Ne_Replay_WriteFrames walks one registry entry per recordable (stride 0x688). Each entry has a state holder, a heap
// block of a 0x80 byte header and a current and a previous buffer, whose class (capacity 0x20 to 0xffff) was picked at
// registration from the state a stock grid needs. Each frame the recordable serializes into a static scratch buffer
// through a stack stream, and the recorder copies that state into the holder's current buffer with the state's full
// size and no check against the holder. A recordable whose state grows with the session's vehicle count (a career
// addon that adds rows to the grid, 25 vehicles on a 22 car plan) outgrows its holder, and the copy writes past the
// block into the next heap block. The next walk calls the vtable of a holder that block overwrote: ACCESS_VIOLATION in
// Ne_Replay_WriteFrames+0x752, at the call of that holder's capacity method.
//
// At site_Ne_ReplayAfterRecord, after the recordable serialized and before either copy, the stream position is
// [rsp+0x98], its base [rsp+0xa0], the holder [rsp+0x118] and the holder's capacity [rsp+0x108]. A state larger than
// the capacity moves its entry to a holder of a larger class (the smallest with twice the state's size), allocated with
// the registry's allocator, header and both buffers copied, the registry slot and the recorder's cached pointers into
// the old holder moved to the new one; the old holder stays allocated. Only when no class fits (a state past 0xffff
// bytes, an allocation that fails, a build whose class vtables differ) is the state cut to nothing by moving the
// position back to the base. The recorder then writes an empty state and copies no bytes into the holder, and the
// recordable is remembered.
//
// An empty state read back would hand the recordable an empty stream: its readFrom reads a length from nothing and
// the heap error 0xc "Requested size:3747052065449325568" follows (Ai_CannedEvents_PerformAccident_2 under
// Ne_ReplayRecordable_ReadFrom, from flashback and instant replay). Ne_ReplayRecordable_ReadFrom (0x1445cd9d0) applies
// a stored state to its recordable in two places, the full state read (call [rax+0x38] at 0x1445cdccb) and the delta
// read (call [rax+0x28] / [rax+0x40] after 0x1445ceece). Both are found by byte scan; a remembered recordable whose
// stored state is empty skips the apply and keeps its current state.
//
// A session of 24 cars or fewer, the size of My Team's stock grid, keeps the game's code for a state past the holder's
// capacity that still fits its buffer: the recorder there writes CannedEventsSystem's state a few bytes past the
// capacity, inside the buffer, as it did before this guard. Only a state larger than the buffer (the distance from the
// current buffer [rsp+0xc0] to the previous one [rsp+0xe8]), whose copy would leave the holder's block, gets the larger
// holder there. Track data does that: DragReductionSystem's state is every car's RearWing record (13 bytes, plus 5 per
// detection point time the car holds, plus 2 per DRS zone) and 5 bytes of its own, and its 0x400 byte holder (0x480
// buffer) fits 22 cars on at most 5 zones or 24 on 4. Le Mans (mod.lemans) has 7, so late on a lap the previous-buffer
// copy writes up to 5 bytes into the next block, MarshalZoneMonitor's holder, whose vtable the walk then calls.
//
// The holder classes are looked up once, on a thread of their own when the guard installs, so the build scan (about
// 50 ms on a build whose class vtables moved) does not land in a race frame.
//
// SWOFTY_REPLAY_GRID_GUARD=0 turns it off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (replay recorder grid guard)";
constexpr const char *SITE = "site_Ne_ReplayAfterRecord";

bool offered = false, settled = false;
runtime::replay_grid::Tally tally, grown_tally, read_tally;
runtime::replay_grid::Emptied emptied;
std::once_flag classes_once;
bool classes_ok = false;
safetyhook::MidHook h_full, h_delta;

uint64_t &slot(SwoftyContext *c, uint32_t off)
{
    return *reinterpret_cast<uint64_t *>(c->rsp + off);
}

std::string name_of(uint64_t p)
{
    std::string out;
    if (!p)
        return out;
    for (size_t i = 0; i < 96; i++) {
        char ch = 0;
        if (!runtime::readable(reinterpret_cast<const void *>(p + i), 1))
            break;
        ch = *reinterpret_cast<const char *>(p + i);
        if (!ch || static_cast<unsigned char>(ch) < 0x20 || static_cast<unsigned char>(ch) > 0x7e)
            break;
        out += ch;
    }
    return out;
}

std::vector<runtime::replay_grid::HolderClass> classes;

bool fn_is(uintptr_t fn, const uint8_t *want, size_t n)
{
    return runtime::readable(reinterpret_cast<const void *>(fn), n) && !memcmp(reinterpret_cast<const void *>(fn), want, n);
}

bool decode_class(uintptr_t vt, runtime::replay_grid::HolderClass &out)
{
    uintptr_t slots[5];
    if (!swofty::mem::read(vt, slots))
        return false;
    const uint8_t *cap = reinterpret_cast<const uint8_t *>(slots[2]);
    if (!swofty::mem::in_image(slots[2]) || !swofty::mem::in_image(slots[3]) || !swofty::mem::in_image(slots[4]) ||
        !runtime::readable(cap, 6) || cap[0] != 0xb8 || cap[5] != 0xc3)
        return false;
    uint32_t capacity = 0;
    memcpy(&capacity, cap + 1, 4);
    static const uint8_t cur[8] = {0x48, 0x8d, 0x81, 0x80, 0, 0, 0, 0xc3};
    const uint8_t *prev = reinterpret_cast<const uint8_t *>(slots[4]);
    if (!capacity || !fn_is(slots[3], cur, 8) || !runtime::readable(prev, 8) || prev[0] != 0x48 || prev[1] != 0x8d ||
        prev[2] != 0x81 || prev[7] != 0xc3)
        return false;
    uint32_t disp = 0;
    memcpy(&disp, prev + 3, 4);
    if (disp <= runtime::replay_grid::HEADER || disp - runtime::replay_grid::HEADER < capacity)
        return false;
    out = {uint32_t(vt - swofty::api->module_base()), capacity, uint32_t(disp - runtime::replay_grid::HEADER)};
    return true;
}

void discover_classes()
{
    namespace rg = runtime::replay_grid;
    swofty::mem::detail::Image img = swofty::mem::detail::image();
    uintptr_t at = img.lo;
    while (at < img.hi) {
        MEMORY_BASIC_INFORMATION m{};
        if (!VirtualQuery(reinterpret_cast<void *>(at), &m, sizeof m))
            break;
        uintptr_t end = uintptr_t(m.BaseAddress) + m.RegionSize;
        bool ok = m.State == MEM_COMMIT && !(m.Protect & (PAGE_NOACCESS | PAGE_GUARD | PAGE_EXECUTE | PAGE_EXECUTE_READ |
                                                         PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY));
        if (ok)
            for (uintptr_t p = (at + 7) & ~uintptr_t(7); p + 40 <= end; p += 8) {
                rg::HolderClass k{};
                if (!decode_class(p, k))
                    continue;
                bool known = false;
                for (const auto &c : classes)
                    if (c.cap == k.cap && c.buf == k.buf)
                        known = true;
                if (!known)
                    classes.push_back(k);
            }
        at = end;
    }
}

void find_classes()
{
    namespace rg = runtime::replay_grid;
    bool all = true;
    for (int i = 0; i < rg::CLASS_COUNT && all; i++) {
        rg::HolderClass k{};
        all = decode_class(swofty::api->module_base() + rg::CLASSES[i].vtable_rva, k) && k.cap == rg::CLASSES[i].cap &&
              k.buf == rg::CLASSES[i].buf;
    }
    if (all)
        classes.assign(rg::CLASSES, rg::CLASSES + rg::CLASS_COUNT);
    else {
        discover_classes();
        swofty::log("[replay] holder classes of the Steam build do not match; %zu classes found by scanning this build",
                    classes.size());
    }
    classes_ok = classes.size() >= 5;
}

bool check_classes()
{
    std::call_once(classes_once, find_classes);
    return classes_ok;
}

uint64_t grow_holder(SwoftyContext *c, uint64_t state)
{
    namespace rg = runtime::replay_grid;
    uintptr_t old = slot(c, rg::HOLDER);
    if (!swofty::mem::object_like(old) || !check_classes())
        return 0;
    uintptr_t vt = *reinterpret_cast<uintptr_t *>(old), base_addr = swofty::api->module_base();
    const int n = int(classes.size());
    int from = rg::class_of_rva(classes.data(), n, vt - base_addr), to = rg::grown_class(classes.data(), n, from, state);
    if (from < 0 || to < 0)
        return 0;
    uintptr_t entry_slot = slot(c, rg::ENTRY_BASE) + slot(c, rg::ENTRY_INDEX) + 0x20, alloc = 0;
    if (!runtime::readable(reinterpret_cast<void *>(entry_slot - 8), 16) || *reinterpret_cast<uintptr_t *>(entry_slot) != old ||
        !swofty::mem::read(entry_slot - 8, alloc) || !swofty::mem::object_like(alloc))
        return 0;
    const rg::HolderClass a = classes[from], b = classes[to];
    using Alloc = void *(*)(uintptr_t, uint64_t, uint64_t);
    auto alloc_fn = reinterpret_cast<Alloc>((*reinterpret_cast<uintptr_t **>(alloc))[3]);
    auto *fresh = static_cast<uint8_t *>(alloc_fn(alloc, rg::HEADER + 2 * uint64_t(b.buf), 0x10));
    if (!fresh)
        return 0;
    auto *src = reinterpret_cast<const uint8_t *>(old);
    memset(fresh, 0, rg::HEADER + 2 * size_t(b.buf));
    memcpy(fresh, src, rg::HEADER);
    *reinterpret_cast<uintptr_t *>(fresh) = base_addr + b.vtable_rva;
    memcpy(fresh + rg::HEADER, src + rg::HEADER, a.buf);
    memcpy(fresh + rg::HEADER + b.buf, src + rg::HEADER + a.buf, a.buf);
    uintptr_t now = reinterpret_cast<uintptr_t>(fresh);
    *reinterpret_cast<uintptr_t *>(entry_slot) = now;
    slot(c, rg::HOLDER) = now;
    for (uint32_t off : rg::HEADER_SLOTS)
        if (slot(c, off) >= old && slot(c, off) < old + rg::HEADER)
            slot(c, off) += now - old;
    slot(c, rg::CUR_SLOT) = now + rg::HEADER;
    slot(c, rg::PREV_SLOT) = now + rg::HEADER + b.buf;
    for (uint32_t off : rg::CAP_SLOTS)
        if (rg::capacity_of(slot(c, off)) == a.cap)
            slot(c, off) = b.cap;
    c->r12 = now;
    return b.cap;
}

uintptr_t recordable_of_entry(SwoftyContext *c)
{
    namespace rg = runtime::replay_grid;
    uintptr_t entry = slot(c, rg::ENTRY_BASE) + slot(c, rg::ENTRY_INDEX), rec = 0;
    if (!swofty::mem::read(entry + 0x10, rec))
        return 0;
    return rec;
}

void after_record(SwoftyContext *c)
{
    namespace rg = runtime::replay_grid;
    const int64_t cars = rg::session_vehicles.load();
    const bool raised = rg::guards(cars);
    uint64_t pos = slot(c, rg::STREAM_POS), base = slot(c, rg::STREAM_BASE);
    uint64_t cap = rg::capacity_of(slot(c, rg::HOLDER_CAPACITY));
    uint64_t limit = raised ? cap : rg::room_of(slot(c, rg::CUR_SLOT), slot(c, rg::PREV_SLOT), cap);
    if (!rg::outgrew(pos, base, limit))
        return;
    const uint64_t state = pos - base;
    if (uint64_t grown = grow_holder(c, state)) {
        if (grown_tally.skipped.fetch_add(1, std::memory_order_relaxed) < 64) {
            std::string name = name_of(c->r13);
            swofty::log("%s", (raised ? rg::grown_line(cars, name, state, cap, grown)
                                      : rg::room_line(cars, name, state, limit, grown))
                                  .c_str());
        }
        return;
    }
    slot(c, rg::STREAM_POS) = base;
    emptied.add(recordable_of_entry(c));
    if (tally.first()) {
        std::string name = name_of(c->r13);
        swofty::log("%s",
                    (raised ? rg::line(cars, name, state, cap) : rg::room_line(cars, name, state, limit, 0)).c_str());
    }
}

constexpr uint32_t STORED_SIZE = 0xa8;

const runtime::Sig S_FULL{"site_Ne_ReplayReadFullApply",
                          "48 89 F9 48 89 F2 E8 ?? ?? ?? ?? 49 8B 4C 24 10 48 8B 01 48 89 FA FF 50 38 E9", 0x45cdcc0,
                          runtime::Sig::addr, 0, 11, 0};
constexpr uintptr_t FULL_SKIP = 14;
const runtime::Sig S_DELTA{
    "site_Ne_ReplayReadDeltaApply",
    "41 8B 45 04 83 F8 02 74 19 83 F8 01 75 30 48 8B 4F 10 48 8B 01 48 8D 94 24 90 00 00 00 FF 50 28 EB 1C C4 C1 7A 10 45 "
    "10 C5 FA 5A D0 48 8B 4F 10 48 8B 01 48 8D 94 24 90 00 00 00 FF 50 40 48 8B 06 48 89 F1",
    0x45ceece};
constexpr uintptr_t DELTA_SKIP = 0x3e;
uintptr_t full_at = 0, delta_at = 0;

void read_apply(safetyhook::Context &c, uintptr_t recordable, uintptr_t resume)
{
    uint32_t size = 0;
    if (!swofty::mem::read(c.rsp + STORED_SIZE, size))
        return;
    if (!runtime::replay_grid::skips_read(size, emptied.has(recordable)))
        return;
    c.rip = resume;
    if (read_tally.first())
        swofty::log("[replay] a recordable the recorder could not hold has no stored state; it keeps its current state "
                    "instead of reading an empty one");
}

void read_full(safetyhook::Context &c)
{
    uintptr_t rec = 0;
    swofty::mem::read(c.r12 + 0x10, rec);
    read_apply(c, rec, full_at + FULL_SKIP);
}

void read_delta(safetyhook::Context &c)
{
    uintptr_t rec = 0;
    swofty::mem::read(c.rdi + 0x10, rec);
    read_apply(c, rec, delta_at + DELTA_SKIP);
}

bool arm_read(safetyhook::MidHook &slot_hook, const runtime::Sig &s, uintptr_t at, void (*fn)(safetyhook::Context &))
{
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[replay] read guard: %s: the hook failed: %s", s.what, runtime::hook_error(h.error()).c_str());
        return false;
    }
    slot_hook = std::move(*h);
    return true;
}

void setup_read()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_REPLAY_GRID_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off")))
        return;
    full_at = runtime::find(S_FULL, "replay", "replay");
    delta_at = runtime::find(S_DELTA, "replay", "replay");
    if (!full_at || !delta_at) {
        swofty::log("[replay] read guard not hooked: %s%s", full_at ? "" : "full state read ", delta_at ? "" : "delta read");
        return;
    }
    if (arm_read(h_full, S_FULL, full_at, read_full) && arm_read(h_delta, S_DELTA, delta_at, read_delta))
        swofty::log("[replay] read guard armed: a recordable with no stored state keeps its current state when a replay is read");
}

}

namespace runtime {

std::vector<CoreMixins> replay_grid_guard_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_REPLAY_GRID_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[replay] recorder grid guard off (SWOFTY_REPLAY_GRID_GUARD=%s)", v);
        return out;
    }
    if (!swofty::api->resolve(SITE)) {
        swofty::log("[replay] recorder grid guard off: needs %s, which this build does not resolve", SITE);
        return out;
    }
    out.push_back({MIXIN, {{SITE, SWOFTY_MIXIN_SITE, 100, (void *)&after_record, nullptr}}});
    return out;
}

void replay_grid_guard_commit()
{
    if (!offered || settled)
        return;
    settled = true;
    const bool armed = core_mixins_active(MIXIN);
    swofty::log("[replay] recorder grid guard %s",
                armed ? "armed: a recordable whose state outgrows its holder gets a larger holder"
                      : "did not install (see the [core] lines)");
    if (armed)
        std::thread([] { check_classes(); }).detach();
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("replay read guard", setup_read);
}
