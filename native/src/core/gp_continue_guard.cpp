#include "gp_continue_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <mutex>
#include <string>
#include <vector>

// Grand Prix continue F1 World car guard (re/db/globals.tsv site_Game_GrandPrixContinueVm_F1wCarCheck and
// site_Game_GrandPrixContinueVm_F1wWarningCheck, both in Game_GrandPrixContinueVm_PressEntry).
//
// Continue on a Grand Prix save runs two offline checks before the save loads. Each compares the vehicle
// category of a game setup mode (its first qword) with the F1 World player component's +0x580, the category of
// the player's F1 World car. When they are equal and the game is offline, the first shows f1w_car_offline ("You
// must be online to resume this saved Grand Prix") and the second continue_solo_gp_f1w_offline_warning. The
// component's category is the generic one the f1_world setup mode uses, but on profiles where it holds the real F1
// category every F1 season save matches, and under the launcher, which always runs offline, the save cannot be
// resumed.
//
// The first site is the cmp [r8+0x580],rcx of the f1w_car_offline check (r8 = the component, rcx = the category of
// the entrants manager's current setup mode [rax+0x2b0], rbx = the Grand Prix mode entity); the second is the cmp
// [rax+0x580],rcx of the warning (rax = the component, r14 = the save's setup mode, rcx = its category, [rsp+0x48] =
// the mode entity). When the categories match, the guard looks up the setup mode labelled f1_world in the mode
// entity's setup mode list (its vtable slot +0x1d0, the same list the function walks for the save's setup mode).
// Unless the component's category is that setup mode's category, the guard sets rcx to 0, so the game's own
// compare takes its not-equal path: no popup, and the save continues as it does online. A save on the F1 World
// car, or a build where the f1_world setup mode is not found, keeps the game's check. Each press logs one line.
//
// SWOFTY_GP_F1W_CHECK_GUARD=0 turns it off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (grand prix continue F1 World car guard)";
constexpr const char *CAR_SITE = "site_Game_GrandPrixContinueVm_F1wCarCheck";
constexpr const char *WARNING_SITE = "site_Game_GrandPrixContinueVm_F1wWarningCheck";

constexpr uintptr_t COMPONENT_CATEGORY = 0x580;
constexpr uintptr_t MANAGER_SETUP_MODE = 0x2b0;
constexpr uintptr_t FRAME_ENTITY = 0x48;
constexpr uintptr_t ENTITY_SETUP_MODES = 0x1d0;
constexpr uintptr_t LIST_BEGIN = 0x8;
constexpr uintptr_t LIST_END = 0x10;
constexpr uintptr_t SETUP_MODE_NAME = 0xd8;
constexpr uintptr_t HOLDER_TEXT = 0x10;
constexpr uintptr_t MAX_SETUP_MODES = 256;

namespace gc = runtime::gp_continue;
namespace mem = swofty::mem;

bool offered = false, settled = false, car_bound = false, warning_bound = false;
std::mutex log_lock;
gc::Press last_press;
int lines = 0;

std::string label_of(uintptr_t mode)
{
    uintptr_t holder = 0, text = 0;
    std::string out;
    if (!mode || !mem::read(mode + SETUP_MODE_NAME, holder) || !holder || !mem::read(holder + HOLDER_TEXT, text) || !text ||
        !mem::c_string(text, out, 64))
        return {};
    return out;
}

uint64_t f1_world_category(uintptr_t entity)
{
    uintptr_t vt = 0, fn = 0;
    if (!mem::object_like(entity) || !mem::read(entity, vt) || !mem::read(vt + ENTITY_SETUP_MODES, fn) || !mem::in_image(fn))
        return 0;
    uintptr_t list = reinterpret_cast<uintptr_t (*)(uintptr_t)>(fn)(entity), begin = 0, end = 0;
    if (!list || !mem::read(list + LIST_BEGIN, begin) || !mem::read(list + LIST_END, end) || end < begin ||
        (end - begin) / 8 > MAX_SETUP_MODES)
        return 0;
    for (uintptr_t p = begin; p < end; p += 8) {
        uintptr_t mode = 0;
        uint64_t category = 0;
        if (mem::read(p, mode) && mode && label_of(mode) == gc::F1_WORLD && mem::read(mode, category))
            return category;
    }
    return 0;
}

void say(gc::Check check, gc::Verdict verdict, const std::string &mode, uint64_t frame)
{
    std::lock_guard<std::mutex> g(log_lock);
    uint64_t now = GetTickCount64();
    if (gc::same_press(last_press, frame, now, verdict, mode)) {
        last_press.at = now;
        return;
    }
    last_press = {frame, now, verdict, mode, true};
    if (lines >= gc::LOG_CAP)
        return;
    ++lines;
    swofty::log("[gp] %s%s", gc::line(check, verdict, mode).c_str(),
                lines == gc::LOG_CAP ? " (further presses are not logged)" : "");
}

void check(SwoftyContext *c, gc::Check which, uintptr_t component, uintptr_t mode, uintptr_t entity)
{
    uint64_t car = 0, setup = c->rcx, setup_first = 0;
    if (!mem::read(component + COMPONENT_CATEGORY, car))
        return;
    std::string label = mode && mem::read(mode, setup_first) && setup_first == setup ? label_of(mode) : std::string();
    bool matched = setup && car == setup;
    gc::Verdict verdict = gc::decide(car, setup, matched ? f1_world_category(entity) : 0);
    if (verdict == gc::Verdict::skip)
        c->rcx = 0;
    say(which, verdict, label, c->rsp);
}

void car_check(SwoftyContext *c)
{
    uintptr_t mode = 0;
    mem::read(c->rax + MANAGER_SETUP_MODE, mode);
    check(c, gc::Check::car_offline, c->r8, mode, c->rbx);
}

void warning_check(SwoftyContext *c)
{
    uintptr_t entity = 0;
    mem::read(c->rsp + FRAME_ENTITY, entity);
    check(c, gc::Check::offline_warning, c->rax, c->r14, entity);
}

}

namespace runtime {

std::vector<CoreMixins> gp_continue_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_GP_F1W_CHECK_GUARD", v, sizeof v);
    if (n && n < sizeof v && gc::switch_off(v)) {
        swofty::log("[gp] continue F1 World car guard off (SWOFTY_GP_F1W_CHECK_GUARD=%s)", v);
        return out;
    }
    car_bound = swofty::api->resolve(CAR_SITE) != 0;
    warning_bound = swofty::api->resolve(WARNING_SITE) != 0;
    std::vector<SwoftyMixin> sites;
    if (car_bound)
        sites.push_back({CAR_SITE, SWOFTY_MIXIN_SITE, 100, (void *)&car_check, nullptr});
    if (warning_bound)
        sites.push_back({WARNING_SITE, SWOFTY_MIXIN_SITE, 100, (void *)&warning_check, nullptr});
    if (sites.empty()) {
        swofty::log("[gp] continue F1 World car guard off: needs %s and %s, which this build does not resolve", CAR_SITE,
                    WARNING_SITE);
        return out;
    }
    out.push_back({MIXIN, std::move(sites)});
    return out;
}

void gp_continue_commit()
{
    if (!offered || settled || !(car_bound || warning_bound))
        return;
    settled = true;
    if (!core_mixins_active(MIXIN))
        swofty::log("[gp] continue F1 World car guard did not install (see the [core] lines)");
    else if (car_bound && warning_bound)
        swofty::log("[gp] continue F1 World car guard armed: Grand Prix Continue asks to go online only for a save on the "
                    "F1 World car");
    else
        swofty::log("[gp] continue F1 World car guard armed on %s only (see the [core] lines)",
                    car_bound ? "the f1w_car_offline check" : "the offline warning");
}

}
