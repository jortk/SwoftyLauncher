#include "runtime.hpp"
#include "teamhq_track_logic.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Team HQ advance track guard (re/db/globals.tsv site_Career_TeamHqAdvanceTrackLoad).
//
// Career_TeamHqAdvance_BuildTrackNode (0x1419b4390, virtual slot +0x18 of the Team HQ advance node builder) builds the
// track node of one calendar event for the Team HQ screens. It takes the event's track (event +0x18) from the career's
// TrackDataSystem (CareerEntity +0xf58) into rbx (the TrackDataRecord, 0 when the track has no record) and asks
// Game_TrackData_GetTrack for the TrackDataTrack in r14, which is 0 when no TrackDataSet matches the career's mode,
// season and format. Three loads run without a check: [rbx+0x2c0] at site_Career_TeamHqAdvanceRecordHandle and
// site_Career_TeamHqAdvanceRecordNode, and [r14+0x30] at site_Career_TeamHqAdvanceTrackLoad. A career saved on an older
// Progressive and loaded on a newer one faults reading 0x30 at F1_25.exe+0x19b47ab as the second Team HQ entry opens.
//
// Each load is followed by the game's own test of rax with the empty node handle (r13) as the fallback, so a null record
// or track resumes after the load with rax 0 and the event shows no track. Each track is logged once.
//
// Career_TeamHqAdvanceVM_Initialise (0x1422bc710) has a fourth load of the same kind at site_Career_TeamHqAdvanceVMRecordNode:
// it takes the record for the event's track (event +0xb0) straight from the TrackDataSystem item vector and reads
// [rax+0x2c0] with no check, so an event whose track has no record faults reading 0x2c0 at F1_25.exe+0x22bf18d as the
// second Team HQ entry opens. The code after the load is lea rcx,[rax+8]; test rax,rax; lea rdx,[empty node]; cmovne
// rdx,rcx, so a null record resumes after the load with rax 0 and the node is the game's own empty node. That site logs
// once per track as well.
//
// Career_TeamHqMyTeamVM_Initialise (0x1423278c0, the My Team variant of that view model) has a fifth load of the same
// kind at site_Career_TeamHqMyTeamVMRecordNode, byte-identical to the fourth except that the career entity is in rbx and
// the calendar event in rdi: an event whose track has no record faults reading 0x2c0 at F1_25.exe+0x23296cf. The code after
// the load is the same lea/test/lea/cmovne fallback, so it resumes after the load with rax 0 and logs once per track.
//
// The sites are found by byte scan (the KB rows are L1: subsystem career is owned by career-re).
//
// SWOFTY_TEAMHQ_TRACK_GUARD=0 turns it off.

namespace {

namespace mem = swofty::mem;

const runtime::Sig S_RECORD_HANDLE{"site_Career_TeamHqAdvanceRecordHandle", "45 31 F6 48 8B 83 C0 02 00 00 48 8D 50 08",
                                   0x19b44e8, runtime::Sig::addr, 0, 3, 0};
const runtime::Sig S_RECORD_NODE{"site_Career_TeamHqAdvanceRecordNode", "E8 ?? ?? ?? ?? 48 8B 83 C0 02 00 00",
                                 0x19b478d, runtime::Sig::addr, 0, 5, 0};
const runtime::Sig S_TRACK_LOAD{"site_Career_TeamHqAdvanceTrackLoad", "E8 ?? ?? ?? ?? 49 8B 46 30 48 8D 48 08", 0x19b47ab,
                                runtime::Sig::addr, 0, 5, 0};

const runtime::Sig S_VM_RECORD_NODE{"site_Career_TeamHqAdvanceVMRecordNode",
                                    "48 8B 8F 58 0F 00 00 48 8B 49 28 48 C1 E0 04 48 8B 44 01 08 48 8B 80 C0 02 00 00",
                                    0x22bf18d, runtime::Sig::addr, 0, 0x14, 0};

const runtime::Sig S_MYTEAM_VM_RECORD_NODE{"site_Career_TeamHqMyTeamVMRecordNode",
                                           "48 8B 8B 58 0F 00 00 48 8B 49 28 48 C1 E0 04 48 8B 44 01 08 48 8B 80 C0 02 00 00",
                                           0x23296cf, runtime::Sig::addr, 0, 0x14, 0};

constexpr size_t VM_EVENT_TRACK = 0xb0;
constexpr size_t EVENT_TRACK = 0x18;
constexpr size_t RECORD_INDEX = 0x1;
constexpr size_t RECORD_LOAD_LEN = 7;
constexpr size_t TRACK_LOAD_LEN = 0xf;

uintptr_t handle_next = 0, node_next = 0, track_next = 0, vm_next = 0, myteam_next = 0;
runtime::track_record::Seen seen, vm_seen, myteam_seen;
std::mutex seen_lock;
safetyhook::MidHook h_handle, h_node, h_track, h_vm, h_myteam;

using runtime::teamhq_track::Load;

void say(Load load, uint64_t record, uint64_t track_ref, uint8_t id)
{
    bool first;
    {
        std::lock_guard<std::mutex> hold(seen_lock);
        first = runtime::teamhq_track::says(load, record, track_ref, seen, id);
    }
    if (first)
        swofty::log("[career] teamhq: track %u has no track data; the event shows no track", unsigned(id));
}

void say_vm(uint64_t record, uint8_t id)
{
    bool first;
    {
        std::lock_guard<std::mutex> hold(seen_lock);
        first = runtime::teamhq_track::says(Load::record, record, 0, vm_seen, id);
    }
    if (first)
        swofty::log("[career] teamhq: advance view model track %u has no track data; the event shows no track", unsigned(id));
}

void say_myteam(uint64_t record, uint8_t id)
{
    bool first;
    {
        std::lock_guard<std::mutex> hold(seen_lock);
        first = runtime::teamhq_track::says(Load::record, record, 0, myteam_seen, id);
    }
    if (first)
        swofty::log("[career] teamhq: my team view model track %u has no track data; the event shows no track", unsigned(id));
}

void myteam_record_node(safetyhook::Context &c)
{
    if (!runtime::teamhq_track::skips(Load::record, c.rax, 0))
        return;
    c.rip = myteam_next;
    uint8_t id = 0;
    if (mem::read(c.rdi + VM_EVENT_TRACK, id))
        say_myteam(c.rax, id);
}

void vm_record_node(safetyhook::Context &c)
{
    if (!runtime::teamhq_track::skips(Load::record, c.rax, 0))
        return;
    c.rip = vm_next;
    uint8_t id = 0;
    if (mem::read(c.r15 + VM_EVENT_TRACK, id))
        say_vm(c.rax, id);
}

void record_handle(safetyhook::Context &c)
{
    if (!runtime::teamhq_track::skips(Load::record, c.rbx, 0))
        return;
    c.rax = 0;
    c.rip = handle_next;
    uint8_t id = 0;
    if (mem::read(c.rdi + EVENT_TRACK, id))
        say(Load::record, c.rbx, 0, id);
}

void record_node(safetyhook::Context &c)
{
    if (!runtime::teamhq_track::skips(Load::record, c.rbx, 0))
        return;
    c.rax = 0;
    c.rip = node_next;
}

void track_load(safetyhook::Context &c)
{
    if (!runtime::teamhq_track::skips(Load::track, c.rbx, c.r14))
        return;
    c.rax = 0;
    c.rcx = c.r13;
    c.rip = track_next;
    uint8_t id = 0;
    if (c.rbx && mem::read(c.rbx + RECORD_INDEX, id))
        say(Load::track, c.rbx, c.r14, id);
}

uintptr_t arm(safetyhook::MidHook &slot, const runtime::Sig &s, void (*fn)(safetyhook::Context &), uintptr_t &next,
              size_t len)
{
    uintptr_t at = runtime::find(s, "career", "career");
    if (!at)
        return 0;
    next = at + len;
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[career] teamhq track guard: %s: the hook failed: %s", s.what, runtime::hook_error(h.error()).c_str());
        return 0;
    }
    slot = std::move(*h);
    return at;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_TEAMHQ_TRACK_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] teamhq track guard off (SWOFTY_TEAMHQ_TRACK_GUARD=%s)", v);
        return;
    }
    uintptr_t track = arm(h_track, S_TRACK_LOAD, track_load, track_next, TRACK_LOAD_LEN),
              handle = arm(h_handle, S_RECORD_HANDLE, record_handle, handle_next, RECORD_LOAD_LEN),
              node = arm(h_node, S_RECORD_NODE, record_node, node_next, RECORD_LOAD_LEN),
              vm = arm(h_vm, S_VM_RECORD_NODE, vm_record_node, vm_next, RECORD_LOAD_LEN),
              myteam = arm(h_myteam, S_MYTEAM_VM_RECORD_NODE, myteam_record_node, myteam_next, RECORD_LOAD_LEN);
    if (track && handle && node && vm && myteam)
        swofty::log("[career] teamhq track guard armed: a Team HQ event whose track has no track data shows no track "
                    "instead of faulting");
    else
        swofty::log("[career] teamhq track guard: not hooked: %s%s%s%s", track ? "" : "track load ", handle ? "" : "record handle ",
                    node ? "" : "record node ", vm ? "" : "view model record node");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("teamhq track guard", setup);
}
