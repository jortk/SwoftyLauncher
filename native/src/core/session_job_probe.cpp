#include "blackbox.hpp"
#include "runtime.hpp"
#include "session_job_probe_logic.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Entry probe on Game_Vfx_UpdateWheelTyreInstance (session_job_probe_logic.hpp has the hang it is for). A trace-only
// MidHook at the function's first instruction reads, with guarded reads only, the path its prologue takes to the
// nested loop and the two lists the loop runs over:
//   vehicle id dl (0xff returns), wheel r8d, fifth argument [rsp+0x28] (null returns),
//   holder = [r9 + r8d*8 + 0x1e8], state = [holder+0x48], flag = byte [[[state+0x38]+0x18]+0x31],
//   lists = [holder+0x40], pending list [state+0x20]..[state+0x28] (the loop runs only while it is empty),
//   outer list [[lists+0x1e8]+0x48]..[[lists+0x1e8]+0x50] (0x20-byte items, count tested unsigned),
//   inner list [lists+0x178]..[lists+0x180] (entry pointers, each with a u32 list at +0x170..+0x178).
// It changes nothing, and it reads only inside the armed window (session_job_probe_logic.hpp: a session's entry, from
// the flow transitions career.cpp passes in); outside it a call is one relaxed atomic load. A value it has not logged in
// the window (gate and both sizes) gets one [session-job] line with the pointers and the entries' u32 list lengths, a
// [session-job] [warning] line when a size is negative, huge or ragged, and a black-box note for the stall and crash
// reports; the kPerSession-th value disarms the window.
// SWOFTY_SESSION_JOB_PROBE=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace sj = runtime::session_job_probe;

constexpr const char *FUNC = "Game_Vfx_UpdateWheelTyreInstance";
constexpr uintptr_t OWNER_ARG = 0x28;
constexpr uintptr_t HOLDERS = 0x1e8;
constexpr uintptr_t HOLDER_STATE = 0x48;
constexpr uintptr_t HOLDER_LISTS = 0x40;
constexpr uintptr_t STATE_FLAG_A = 0x38;
constexpr uintptr_t STATE_FLAG_B = 0x18;
constexpr uintptr_t STATE_FLAG = 0x31;
constexpr uintptr_t STATE_PENDING = 0x20;
constexpr uintptr_t LISTS_OUTER = 0x1e8;
constexpr uintptr_t OUTER_BEGIN = 0x48;
constexpr uintptr_t INNER_BEGIN = 0x178;
constexpr uintptr_t ENTRY_LIST = 0x170;

safetyhook::MidHook hook;
sj::Window window;
std::atomic<bool> hooked{false};

bool span(uintptr_t at, sj::Span &s)
{
    uint64_t v[2] = {};
    s.read = at && mem::read(at, v);
    s.begin = v[0];
    s.end = v[1];
    return s.read;
}

void entry_lists(sj::Entry &e)
{
    sj::Verdict v = sj::verdict(e.inner, sj::kInnerItem);
    if (v != sj::Ok && v != sj::Ragged)
        return;
    int64_t n = std::min<int64_t>(sj::items(e.inner, sj::kInnerItem), sj::kListsRead);
    for (int64_t i = 0; i < n; i++) {
        uint64_t p = 0;
        sj::Span l;
        if (!mem::read(uintptr_t(e.inner.begin + i * sj::kInnerItem), p) || !span(uintptr_t(p) + ENTRY_LIST, l))
            continue;
        int64_t b = l.bytes();
        int64_t c = b / int64_t(sj::kListItem);
        if (b < 0 || b % int64_t(sj::kListItem))
            e.lists_bad++;
        if (!e.lists_read || c < e.list_min)
            e.list_min = c;
        if (!e.lists_read || c > e.list_max)
            e.list_max = c;
        e.lists_read++;
    }
}

sj::Entry read_entry(const safetyhook::Context &c)
{
    sj::Entry e;
    e.vehicle = uint8_t(c.rdx);
    e.wheel = uint32_t(c.r8);
    uint64_t owner = 0, state = 0, a = 0, b = 0, outer = 0;
    uint8_t flag = 0;
    uint64_t pending[2] = {};
    if (!mem::read(c.rsp + OWNER_ARG, owner) || !mem::read(c.r9 + HOLDERS + uint64_t(e.wheel) * 8, e.holder))
        return e;
    bool state_read = e.holder && mem::read(uintptr_t(e.holder) + HOLDER_STATE, state);
    bool flag_read = state && mem::read(uintptr_t(state) + STATE_FLAG_A, a) && a &&
                     mem::read(uintptr_t(a) + STATE_FLAG_B, b) && b && mem::read(uintptr_t(b) + STATE_FLAG, flag);
    bool lists_read = e.holder && mem::read(uintptr_t(e.holder) + HOLDER_LISTS, e.lists);
    bool pending_read = state && mem::read(uintptr_t(state) + STATE_PENDING, pending);
    if (e.lists && mem::read(uintptr_t(e.lists) + LISTS_OUTER, outer) && outer)
        span(uintptr_t(outer) + OUTER_BEGIN, e.outer);
    if (e.lists)
        span(uintptr_t(e.lists) + INNER_BEGIN, e.inner);
    if (e.vehicle == 0xff)
        e.gate = sj::NoVehicle;
    else if (!owner)
        e.gate = sj::NoOwner;
    else if (!e.holder)
        e.gate = sj::NoHolder;
    else if (!state_read)
        e.gate = sj::Unread;
    else if (!state)
        e.gate = sj::NoState;
    else if (!flag_read)
        e.gate = sj::Unread;
    else if (!flag)
        e.gate = sj::FlagOff;
    else if (!lists_read)
        e.gate = sj::Unread;
    else if (!e.lists)
        e.gate = sj::NoLists;
    else if (!pending_read)
        e.gate = sj::Unread;
    else if (pending[0] != pending[1])
        e.gate = sj::Pending;
    else if (!e.outer.read)
        e.gate = sj::Unread;
    else if (!e.outer.bytes())
        e.gate = sj::OuterEmpty;
    else
        e.gate = sj::Runs;
    return e;
}

void on_entry(safetyhook::Context &c)
{
    uint32_t g = window.armed();
    if (!g || !window.live(g, GetTickCount64()))
        return;
    sj::Entry e = read_entry(c);
    sj::Admit a = window.admit(g, sj::key(e));
    if (a == sj::Repeat)
        return;
    uint64_t serial = runtime::session_serial();
    static thread_local char text[1024];
    entry_lists(e);
    sj::line(e, serial, text, sizeof text);
    swofty::log("%s", text);
    char note[blackbox::TEXT];
    std::snprintf(note, sizeof note, "0x1445acb00 v%u w%u gate %u outer %lld B at %llx inner %lld B at %llx lists %lld",
                  e.vehicle, e.wheel, e.gate, (long long)e.outer.bytes(), (unsigned long long)e.outer.begin,
                  (long long)e.inner.bytes(), (unsigned long long)e.inner.begin, (long long)e.list_max);
    blackbox::note("session-job", note);
    if (sj::bad(e)) {
        sj::warning(e, serial, text, sizeof text);
        swofty::log("%s", text);
    }
    if (a == sj::FirstLast) {
        sj::last_line(serial, text, sizeof text);
        swofty::log("%s", text);
    }
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_SESSION_JOB_PROBE", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[session-job] 0x1445acb00 entry probe off (SWOFTY_SESSION_JOB_PROBE=%s)", v);
        return;
    }
    uintptr_t at = swofty::api->resolve(FUNC);
    if (!at) {
        swofty::log("[session-job] 0x1445acb00 entry probe off: needs %s, which this build does not resolve", FUNC);
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), on_entry);
    if (!h) {
        swofty::log("[session-job] 0x1445acb00 entry probe: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    hook = std::move(*h);
    hooked.store(true);
    swofty::log("[session-job] 0x1445acb00 entry probe hooked at 0x%llx (%s): while a session is entered (StateSession or "
                "StateLoadSession Entering, for %llu s, cut to %llu s after StateSession updates) it logs the wheel tyre "
                "instance list sizes once per value, at most %d per session, and warns on a negative or huge size",
                (unsigned long long)at, FUNC, (unsigned long long)(sj::kArmMs / 1000),
                (unsigned long long)(sj::kSettleMs / 1000), sj::kPerSession);
}

}

namespace runtime {

void session_job_probe_flow(uint32_t to, const char *name)
{
    if (hooked.load(std::memory_order_relaxed))
        window.flow(to, name, GetTickCount64());
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("session job probe", setup);
}
