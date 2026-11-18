#include "myteam_vehicle_heal_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

// My Team vehicle heal (re/db/functions.tsv Sess_Build_SessionVehicleClassLists, Ent_EntrantsSystem_ReplaceVehicle).
//
// A My Team car is a custom vehicle: kind 1 in the entrants system's vehicle tables ([entrants+0x328+kind*0x10], slots
// indexed by stat id), saved in the profile's entrants 'vehicles' list next to every other career's My Team cars. The
// player team's entrants carry its handle (EntrantRecord +0x5c, kind 1 and the car's stat id; the save slot's vehicle,
// 0x4000 | id). Launcher 1.9.21's My Team vehicle base guard dropped every saved car on load (it read row+0x200, which
// Gdb_Get_RowByIndex sets to -1 for every row it builds), and 1.9.22 still dropped More Engine Suppliers cars. The next
// save wrote the list without them, so the career's handle names nothing. Two places then read the missing car:
// loading the career's weekend spawns the cutscene driver actor (0x14023e540), which takes the car from
// Sess_ResolveEntityHandle([g_Game_Manager+0x290], actor+0x1b2) and hands it unchecked to
// Game_CutSceneScript_ApplyDriverPartOptions, which faults reading 0 at +0x8ee (F1_25.exe+0x24256e); and entering a
// session faults in 0x142458710 reading [vehicle+0xcea] (F1_25.exe+0x2458792 under Game_GameModule_OnSessionEnter).
//
// The heal rebuilds a missing car the way the game builds a My Team car (Career_MyTeamSetup_CreateTeamAndVehicle): a copy
// of a stock My Team car's vehicle data (myteam_mercedes_2026 and the other 2026 cars when the career's stock cars are
// 2026 ones, else myteam_mercedes and the other 2025 cars) with that stock car's handle as its base car (vehicle +0xcf2,
// the saved base_vehicle_stat_id), stored under the lost car's handle with Ent_EntrantsSystem_ReplaceVehicle. A My Team
// car whose base is 0x3fff0000 keeps Team HQ's load in its loading screen for good (launcher 1.9.25 rebuilt cars that
// way), so on career.loaded a car without a base, or with a base that names no loaded stock car, gets the stock car
// of its name as its base. It runs in two places:
// - career.loaded (career.cpp): every kind 1 handle the career's entrants (CareerEntity entrants_begin..entrants_end)
//   hold that the entrants system resolves to nothing is rebuilt, before the weekend's cutscene reads it.
// - site_Sess_SessionEnter_VehicleLookup, the zero check after the first lookup of each session entrant's vehicle in
//   Sess_Build_SessionVehicleClassLists, ahead of the unchecked one: a kind 1 handle that resolves to nothing is rebuilt
//   and the car is returned in rax.
// The next save keeps the rebuilt car: the career is healed with the stock car in place of the lost one. Every rebuilt
// car is logged once with the stock car it took. When every car resolves, the heal only reads. A career load that
// stops before the census (a championship entity or entrant read that fails, a null entrants system, a heal that is
// not set up, or no entrant on a My Team car) logs one line with the step and the values it read.
//
// SWOFTY_MYTEAM_VEHICLE_HEAL=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace mh = runtime::myteam_heal;

constexpr const char *MIXIN = "swofty_core.dll (my team vehicle heal)";
constexpr const char *SITE = "site_Sess_SessionEnter_VehicleLookup";
constexpr const char *REPLACE = "Ent_EntrantsSystem_ReplaceVehicle";

using Replace_t = int (*)(void *entrants, uint32_t handle, const void *vehicle);

uintptr_t replace_at = 0;
bool offered = false, settled = false;
const char *setup = nullptr;
std::mutex heal_lock;
mh::Ids healed;
mh::Ids failed;

struct Game {
    bool u64(uint64_t a, uint64_t &out)
    {
        return mem::read(static_cast<uintptr_t>(a), out);
    }

    bool u32(uint64_t a, uint32_t &out)
    {
        return mem::read(static_cast<uintptr_t>(a), out);
    }

    bool text(uint64_t a, std::string &out)
    {
        return mem::c_string(static_cast<uintptr_t>(a), out, mh::LABEL_CHARS);
    }
};

uint64_t rebuild(Game &game, uint64_t entrants, uint32_t handle, uint64_t first, uint64_t end, const char *when)
{
    if (uint64_t v = mh::vehicle(game, entrants, handle))
        return v;
    uint16_t id = mh::id(handle);
    for (uint16_t f : failed.seen)
        if (f == id)
            return 0;
    mh::Source source;
    if (!mh::plan(game, entrants, first, end, source)) {
        if (failed.first(id)) {
            mh::Tables t = mh::tables(game, entrants);
            swofty::log("[myteam] heal: the player's My Team car (custom vehicle %u) is missing from the save and no stock My "
                        "Team car is loaded to rebuild it from (%s; entrants %p: stock table %p with %zu slots, My Team table %p "
                        "with %zu slots); the career fails as before",
                        (unsigned)id, when, reinterpret_cast<void *>(entrants), reinterpret_cast<void *>(t.stock),
                        t.stock_slots, reinterpret_cast<void *>(t.custom), t.custom_slots);
        }
        return 0;
    }
    static uint8_t data[mh::VEHICLE_DATA_SIZE];
    if (!mem::readable(static_cast<uintptr_t>(source.vehicle + mh::VEHICLE_DATA), sizeof data)) {
        if (failed.first(id))
            swofty::log("[myteam] heal: the stock My Team car '%s' to rebuild custom vehicle %u from is not readable (%s); the "
                        "career fails as before",
                        source.label.c_str(), (unsigned)id, when);
        return 0;
    }
    memcpy(data, reinterpret_cast<const void *>(source.vehicle + mh::VEHICLE_DATA), sizeof data);
    mh::set_base(data, source.handle);
    int r = reinterpret_cast<Replace_t>(replace_at)(reinterpret_cast<void *>(entrants), handle, data);
    uint64_t v = mh::replaced(r) ? mh::vehicle(game, entrants, handle) : 0;
    if (!v) {
        if (failed.first(id))
            swofty::log("[myteam] heal: rebuilding the player's My Team car (custom vehicle %u) from '%s' failed (%s returned "
                        "%d, %s); the career fails as before",
                        (unsigned)id, source.label.c_str(), REPLACE, r, when);
        return 0;
    }
    if (healed.first(id))
        swofty::log("[myteam] heal: the player's My Team car (custom vehicle %u) was missing from the save (launcher 1.9.21 "
                    "dropped every My Team car on load, 1.9.22 the More Engine Suppliers ones, and the next save wrote the "
                    "career without it); rebuilt it from the stock My Team car '%s' (%s season) %s so the career can be raced. "
                    "The next save keeps it",
                    (unsigned)id, source.label.c_str(), source.y2026 ? "2026" : "2025", when);
    return v;
}

std::string heal_state()
{
    if (setup)
        return setup;
    if (!offered)
        return std::string("never offered, the game code reads ") +
               (runtime::code_ready() ? "decrypted now" : "encrypted still");
    return runtime::core_mixins_active(MIXIN) ? "armed" : "armed for the career load only";
}

void session_vehicle(SwoftyContext *c)
{
    if (c->rax || !replace_at)
        return;
    Game game;
    uint32_t handle = 0;
    if (!game.u32(c->rsi + mh::ENTRANT_VEHICLE, handle) || !mh::healable(handle) || !c->rbx)
        return;
    uint64_t first = 0;
    game.u64(c->rdi + mh::FRAME_FIRST_ENTRANT, first);
    std::lock_guard<std::mutex> hold(heal_lock);
    if (uint64_t v = rebuild(game, c->rbx, handle, first, c->r12, "as its session started"))
        c->rax = v;
}

}

namespace runtime {

std::vector<CoreMixins> myteam_vehicle_heal_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_MYTEAM_VEHICLE_HEAL", v, sizeof v);
    if (n && n < sizeof v && mh::switch_off(v)) {
        setup = "off (SWOFTY_MYTEAM_VEHICLE_HEAL)";
        swofty::log("[myteam] vehicle heal off (SWOFTY_MYTEAM_VEHICLE_HEAL=%s)", v);
        return out;
    }
    replace_at = swofty::api->resolve(REPLACE);
    if (!replace_at) {
        setup = "off (Ent_EntrantsSystem_ReplaceVehicle is not resolved)";
        swofty::log("[myteam] vehicle heal off: needs %s, which this build does not resolve", REPLACE);
        return out;
    }
    if (!swofty::api->resolve(SITE)) {
        swofty::log("[myteam] vehicle heal: %s is not resolved on this build, so only the career load rebuilds a missing car",
                    SITE);
        return out;
    }
    out.push_back({MIXIN, {{SITE, SWOFTY_MIXIN_SITE, 100, (void *)&session_vehicle, nullptr}}});
    return out;
}

void myteam_vehicle_heal_entrants(const myteam_heal::Load &load)
{
    Game game;
    mh::Census census = mh::census(game, load.first, load.end);
    if (const char *stop = mh::heal_stop(load, replace_at != 0, census)) {
        std::string heal = heal_state();
        mh::Tables t = load.system ? mh::tables(game, load.system) : mh::Tables{};
        size_t cars = load.system ? mh::loaded(game, load.system, mh::KIND_CUSTOM) : 0;
        swofty::log("[myteam] heal: %s", mh::stopped(stop, load, census, t, cars, heal.c_str()).c_str());
        return;
    }
    uint64_t entrants = load.system, first = load.first, end = load.end;
    std::vector<uint32_t> lost = mh::missing(game, entrants, first, end);
    std::lock_guard<std::mutex> hold(heal_lock);
    size_t rebuilt = 0, based = 0;
    for (uint32_t handle : lost)
        rebuilt += rebuild(game, entrants, handle, first, end, "as the career loaded") != 0;
    for (uint64_t car : mh::held(game, entrants, first, end)) {
        if (!mh::unbased(game, entrants, car))
            continue;
        uint32_t base = mh::base_for(game, entrants, car);
        std::string name;
        mh::label(game, car, name);
        uint32_t handle = 0, was = 0;
        game.u32(car, handle);
        game.u32(car + mh::VEHICLE_BASE, was);
        if (base == mh::NO_VEHICLE || !mem::readable(static_cast<uintptr_t>(car + mh::VEHICLE_BASE), sizeof base)) {
            if (failed.first(mh::id(handle))) {
                mh::Tables t = mh::tables(game, entrants);
                swofty::log("[myteam] heal: the player's My Team car (custom vehicle %u, '%s') has no base car (base %08x) and "
                            "no stock car of that name is loaded to give it one (entrants %p: stock table %p with %zu slots)",
                            (unsigned)mh::id(handle), name.c_str(), was, reinterpret_cast<void *>(entrants),
                            reinterpret_cast<void *>(t.stock), t.stock_slots);
            }
            continue;
        }
        *reinterpret_cast<uint32_t *>(static_cast<uintptr_t>(car + mh::VEHICLE_BASE)) = base;
        based++;
        if (healed.first(mh::id(handle)))
            swofty::log("[myteam] heal: the player's My Team car (custom vehicle %u, '%s') had no base car or one that is "
                        "not loaded (base %08x; launcher 1.9.25 rebuilt it from a stock car without one, or the car was "
                        "made without one), which stops Team HQ loading; set its base to the stock '%s' (%08x) as the "
                        "career loaded. The next save keeps it",
                        (unsigned)mh::id(handle), name.c_str(), was, name.c_str(), base);
    }
    std::string ids;
    for (uint16_t i : census.ids)
        ids += (ids.empty() ? "" : ", ") + std::to_string(i);
    for (uint64_t car : mh::held(game, entrants, first, end)) {
        uint32_t h = 0, base = 0;
        uint8_t engine = 0;
        std::string name, base_name;
        game.u32(car, h);
        game.u32(car + mh::VEHICLE_BASE, base);
        mem::read(static_cast<uintptr_t>(car + mh::VEHICLE_ENGINE), engine);
        mh::label(game, car, name);
        bool named = mh::base_label(game, entrants, car, base_name);
        char b[64];
        snprintf(b, sizeof b, "' engine %u base car %08x", engine, base);
        ids += "; custom vehicle " + std::to_string(h >> 16) + " '" + name + b +
               (named ? " '" + base_name + "'" : std::string(" (no stock car)"));
    }
    swofty::log("[myteam] heal: the career's %zu entrants include %zu on My Team cars (custom vehicle %s) as it loaded: %zu "
                "missing and rebuilt, %zu given a base car, the rest present",
                census.entrants, census.on_my_team, ids.c_str(), rebuilt, based);
}

void myteam_vehicle_heal_commit()
{
    if (!offered || settled || !replace_at)
        return;
    settled = true;
    swofty::log("[myteam] vehicle heal %s",
                core_mixins_active(MIXIN) ? "armed: a My Team car missing from the save is rebuilt from a stock My Team car "
                                            "when its career loads or its session starts"
                                          : "armed for the career load only: the session lookup did not install (see the "
                                            "[core] lines)");
}

}
