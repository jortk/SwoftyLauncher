#include "runtime.hpp"
#include "setup_vm_logic.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <mutex>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Car setup screen car guard (re/db/globals.tsv site_Game_CarSetupVm_*).
//
// The car setup screen's handlers (Game_CarSetupVm_Initialise 0x140c1b1a0, the function at 0x140c1c9f0 and the two in
// Car_Setup_OnValueChanged's neighbourhood, 0x140c1e690 and 0x140c1edd0) finish by asking whether the screen's car is
// handling version 100 before they update the differential ranges: rax = [vm+0x268], tested for 0, then rax = [rax+0x28]
// and cmp dword [rax+0x9f40], 0x64 without a test of the car. A setup screen that runs a refresh after its car is gone
// (the setup data's car at +0x28 is 0) faults reading 0x9f40 at F1_25.exe+0xc1b92c, 5 to 16 s into a session.
//
// Each cmp is followed by a jne to the game's own path for a car that is not version 100, so a null car resumes there.
// 0x140c22ad0, named as a sibling in the triage notes, is Game_GarageSetups_FindOwnerName and never reads [vm+0x268].
//
// The sites are found by byte scan (the KB rows are L1).
//
// SWOFTY_SETUP_VM_GUARD=0 turns it off.

namespace {

namespace mem = swofty::mem;

constexpr size_t CMP_LEN = 7;
constexpr size_t SITES = 4;

const runtime::Sig S_INIT{"site_Game_CarSetupVm_InitCarLoad",
                          "83 B8 40 9F 00 00 64 75 10 48 89 F1 E8 ?? ?? ?? ?? 48 89 F1 E8 ?? ?? ?? ?? C7 44 24 30 04 00 00 00",
                          0xc1b92c, runtime::Sig::addr, 0, 0, 0};
const runtime::Sig S_VALUE_SET{"site_Game_CarSetupVm_ValueSetCarLoad",
                               "83 B8 40 9F 00 00 64 75 10 48 89 F1 E8 ?? ?? ?? ?? 48 89 F1 E8 ?? ?? ?? ?? 4C 8D AE D8 76 00 00",
                               0xc1cbca, runtime::Sig::addr, 0, 0, 0};
const runtime::Sig S_CHANGED_A{"site_Game_CarSetupVm_OnValueChangedCarLoadA", "48 8B 40 28 83 B8 40 9F 00 00 64 0F 85", 0xc1ed9f,
                               runtime::Sig::addr, 0, 4, 0};
const runtime::Sig S_CHANGED_B{"site_Game_CarSetupVm_OnValueChangedCarLoadB",
                               "83 B8 40 9F 00 00 64 75 10 48 89 F1 E8 ?? ?? ?? ?? 48 89 F1 E8 ?? ?? ?? ?? 80 BE 6A 7E 00 00 00",
                               0xc1eeb7, runtime::Sig::addr, 0, 0, 0};

uintptr_t skip_to[SITES];
runtime::setup_vm::Once said;
std::mutex said_lock;
safetyhook::MidHook hooks[SITES];

template <size_t N> void car_load(safetyhook::Context &c)
{
    if (!runtime::setup_vm::skips(c.rax))
        return;
    c.rip = skip_to[N];
    bool first;
    {
        std::lock_guard<std::mutex> hold(said_lock);
        first = said.first();
    }
    if (first)
        swofty::log("[setups] the setup screen has no car yet; skipped");
}

bool arm(size_t n, const runtime::Sig &s, void (*fn)(safetyhook::Context &))
{
    uintptr_t at = runtime::find(s, "setups", "setups");
    if (!at)
        return false;
    uint8_t after[6] = {};
    for (size_t i = 0; i < sizeof after; i++)
        if (!mem::read(at + CMP_LEN + i, after[i]))
            return false;
    uintptr_t to = runtime::setup_vm::skip_target(at, CMP_LEN, after);
    if (!to) {
        swofty::log("[setups] car guard: %s: no jne after the compare", s.what);
        return false;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[setups] car guard: %s: the hook failed: %s", s.what, runtime::hook_error(h.error()).c_str());
        return false;
    }
    skip_to[n] = to;
    hooks[n] = std::move(*h);
    return true;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_SETUP_VM_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[setups] car guard off (SWOFTY_SETUP_VM_GUARD=%s)", v);
        return;
    }
    size_t armed = arm(0, S_INIT, car_load<0>) + arm(1, S_VALUE_SET, car_load<1>) + arm(2, S_CHANGED_A, car_load<2>) +
                   arm(3, S_CHANGED_B, car_load<3>);
    if (armed)
        swofty::log("[setups] car guard armed at %u of %u sites: a setup screen with no car skips its differential range "
                    "update instead of faulting",
                    unsigned(armed), unsigned(SITES));
    else
        swofty::log("[setups] car guard: not hooked");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("setup screen car guard", setup);
}
