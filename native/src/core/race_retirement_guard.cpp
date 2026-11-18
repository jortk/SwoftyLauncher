#include "race_retirement_logic.hpp"
#include "runtime.hpp"

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

// Race retirement driver guard (re/db/globals.tsv site_Sess_InGameRaceRetirementDriver).
//
// When a car reaches retired status, Sess_InGameRace_Update (0x142d61d70) posts a car_retirement HUD message that
// names its driver. It finds the car's entrant in the game module's entrant list (gm = [[state+0xca0]+0x88]+0x260,
// 0x88-byte entrants at [gm+0x2e0..+0x2e8) whose byte +0x82 is the vehicle index r15b), looks its packed driver ref
// (domain byte +0x50, u16 stat id +0x52) up with Ent_Get_Driver_Thunk (0x1423dbcf0, rcx = [gm+0xf78] = the entrants
// system) and passes driver + 0x20 to the driver name fill (0x14249ac60 -> 0x14cb8a5d0), which writes the name into a
// 0x80-character inline string at rsp+0x50. The block joins at add rcx,0x18 (0x142d624e0) with rcx = driver + 8, or 0
// when the lookup returned 0, no entrant has the vehicle index, the list is empty or the vehicle index is 0xff. The
// name fill reads [rcx+0x18+0x1d8] with no test, so every one of those paths faults reading 0x1f0 at
// F1_25.exe+0xcb8a5e1 < Sess_InGameRace_Update+0x77e as the car retires (a career entrant whose driver the entrants
// system does not have).
//
// At the join a null rcx writes the name fill's own empty-name fallback into the result (0x14cb8a684: vtable
// g_Ne_InlineStr128Vtable, capacity 0x80, the inline character pointer, a NUL) and resumes at the call's return
// (site_Sess_InGameRaceRetirementDriverJoin), which reads only the result's character pointer. The retirement message
// is posted with an empty driver name and the race goes on. The car, and for a found entrant its driver ref and why
// the entrants system has no driver for it, are logged once per car and ref.
//
// The three bindings are exported by the bindings (L2 with an agree verify). SWOFTY_RACE_RETIREMENT_GUARD=0 turns
// it off.

namespace {

namespace mem = swofty::mem;
namespace rr = runtime::race_retirement;

constexpr const char *SITE = "site_Sess_InGameRaceRetirementDriver";
constexpr const char *SITE_JOIN = "site_Sess_InGameRaceRetirementDriverJoin";
constexpr const char *NAME_VTABLE = "g_Ne_InlineStr128Vtable";
constexpr uintptr_t NAME = 0x50;
constexpr uintptr_t JOIN_AFTER = 0xe;
constexpr uintptr_t STATE_MODULES = 0xca0;
constexpr uintptr_t MODULES_GAME = 0x88;
constexpr uintptr_t GAME_MODULE = 0x260;
constexpr uintptr_t ENTRANTS_BEGIN = 0x2e0;
constexpr uintptr_t ENTRANT_SIZE = 0x88;
constexpr uintptr_t ENTRANT_VEHICLE = 0x82;
constexpr uintptr_t ENTRANT_DRIVER_REF = 0x50;
constexpr uintptr_t ENTRANTS_SYSTEM = 0xf78;
constexpr uintptr_t DRIVER_TABLES = 0x268;
constexpr uint64_t MAX_ENTRANTS = 256;

uintptr_t join_at = 0;
uintptr_t name_vtable = 0;
rr::Said said;
std::mutex said_lock;
safetyhook::MidHook h_driver;

uintptr_t find_entrant(uintptr_t begin, uintptr_t end, uint8_t car)
{
    uint64_t n = (end - begin) / ENTRANT_SIZE;
    for (uint64_t i = 0; i < n && i < MAX_ENTRANTS; i++) {
        uintptr_t e = begin + i * ENTRANT_SIZE;
        uint8_t vehicle = 0;
        if (mem::read(e + ENTRANT_VEHICLE, vehicle) && vehicle == car)
            return e;
    }
    return 0;
}

void log_car(uintptr_t state, uint8_t car)
{
    uintptr_t modules = 0, game = 0, gm = 0;
    if (mem::read(state + STATE_MODULES, modules) && modules && mem::read(modules + MODULES_GAME, game) && game)
        gm = game + GAME_MODULE;
    uintptr_t list[2] = {};
    if (gm)
        mem::read(gm + ENTRANTS_BEGIN, list);
    uintptr_t found = car != rr::NO_CAR && list[0] && list[1] > list[0] ? find_entrant(list[0], list[1], car) : 0;
    rr::Entrant e = rr::entrant(car, list[0], list[1], found != 0);

    uint8_t domain = 0;
    uint16_t id = 0;
    bool ref = e == rr::Entrant::found && mem::read(found + ENTRANT_DRIVER_REF, domain) &&
               mem::read(found + ENTRANT_DRIVER_REF + 2, id);
    {
        std::lock_guard<std::mutex> hold(said_lock);
        if (!said.first(rr::key(car, e, domain, id)))
            return;
    }
    if (e != rr::Entrant::found) {
        swofty::log("[race] retirement: car %u retired, but %s; its retirement message names no driver", car,
                    rr::entrant_text(e));
        return;
    }
    if (!ref) {
        swofty::log("[race] retirement: car %u retired, but its entrant's driver is not in the entrants system (%s); "
                    "its retirement message names no driver",
                    car, rr::miss_text(rr::Miss::unread));
        return;
    }
    uintptr_t entrants = 0, table = 0;
    uintptr_t items[2] = {};
    mem::read(gm + ENTRANTS_SYSTEM, entrants);
    if (entrants && domain <= 2 && mem::read(entrants + DRIVER_TABLES + domain * 0x10, table) && table)
        mem::read(table + 8, items);
    uint64_t count = rr::driver_count(items[0], items[1]);
    swofty::log("[race] retirement: car %u retired, but its entrant's driver (domain %u, stat id %u) is not in the "
                "entrants system: %s (%llu drivers); its retirement message names no driver",
                car, domain, id, rr::miss_text(rr::miss(domain, table, count, id)), (unsigned long long)count);
}

void retirement_driver(safetyhook::Context &c)
{
    if (!rr::skips(c.rcx))
        return;
    rr::empty_name(reinterpret_cast<uint8_t *>(c.rsp + NAME), name_vtable);
    c.rip = join_at;
    log_car(c.rdi, uint8_t(c.r15));
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_RACE_RETIREMENT_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[race] retirement driver guard off (SWOFTY_RACE_RETIREMENT_GUARD=%s)", v);
        return;
    }
    uintptr_t at = swofty::api->resolve(SITE);
    uintptr_t join = swofty::api->resolve(SITE_JOIN);
    uintptr_t vtable = swofty::api->resolve(NAME_VTABLE);
    if (!at || !join || !vtable) {
        swofty::log("[race] retirement driver guard off: needs %s, %s and %s, which this build does not resolve", SITE,
                    SITE_JOIN, NAME_VTABLE);
        return;
    }
    if (join != at + JOIN_AFTER) {
        swofty::log("[race] retirement driver guard off: %s resolves 0x%llx past %s, not 0x%llx", SITE_JOIN,
                    (unsigned long long)(join - at), SITE, (unsigned long long)JOIN_AFTER);
        return;
    }
    join_at = join;
    name_vtable = vtable;
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), retirement_driver);
    if (!h) {
        swofty::log("[race] retirement driver guard: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_driver = std::move(*h);
    swofty::log("[race] retirement driver guard armed: a car that retires with no driver in the entrants system gets "
                "a retirement message with no driver name instead of faulting");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("race retirement driver guard", setup);
}
