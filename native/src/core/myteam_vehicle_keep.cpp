#include "myteam_vehicle_keep_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

// My Team vehicle keep (re/db/functions.tsv Game_Load_MyTeamVehicleBases, Game_Save_MyTeamVehicleBases).
//
// A saved My Team car whose vehicle row is gone (an engine supplier addon's car, myteam_mes_*, with the addon off) can't
// be loaded as saved: Sess_Load_VehicleAiConfig faults on the 'Invalid' row. The vehicle base guard used to skip it, which
// left its slot empty: the next save wrote the profile's 'vehicles' list without it (Game_Save_MyTeamVehicleBases writes
// only the cars in the table), and a new My Team career took its id (Ent_EntrantsSystem_AddVehicle uses the first free
// slot), so the old career later raced the new career's car.
//
// With the keep armed, the guard hands such a car to myteam_keep_substitute instead: the loader's row buffer gets the
// row of a stock My Team car (myteam_mercedes_2026 and the other 2026 cars for a *_2026 car, else myteam_mercedes and the
// other 2025 ones; built like Gdb_Get_RowByIndex on the loader's entrants.vehicles/vehicle cursor: 'entrants.vehicles',
// 'vehicle_<index>', +0x200 -1, +0x204 1), so the
// game's own load builds the AI config from it and stores the car with its saved display names, engine id and base car
// (its file_name comes from the AI config's row, so it is the stock car's). At site_Game_MyTeamVehicleBases_NextBase
// (rax = the car Sess_VehicleTable_EmplaceAt stored) the core copies that car into a shadow with the saved file_name put
// back, and makes the live car the stock one (engine at +0xce9 and base car at +0xcf2 from the stock car), so the career
// loads and races and the id stays taken. When the profile saves,
// site_Game_MyTeamVehicleBases_SaveRecord (r14 = the car being written) points the writer at the shadow while the live car
// is still that placeholder (same address, handle, stock file_name and stock base), so the save keeps the original entry.
// With the addon back on, the row exists, the guard leaves the car alone and the real car loads.
//
// The two sites are one core mixin set: if either does not install, nothing is substituted and the guard skips as before.
// SWOFTY_MYTEAM_VEHICLE_KEEP=0 turns it off. SWOFTY_TEST_MYTEAM_KEEP_LABEL=<file_name> makes the guard treat that car as
// an addon car whose row is gone (tests); SWOFTY_TEST_MYTEAM_KEEP_STOCK=<label>:<handle>:<engine>:<row index> names the
// stock car where neither the entrants system nor the game database can be read (the Wine fake).

namespace {

namespace mem = swofty::mem;
namespace mk = runtime::myteam_keep;
namespace mh = runtime::myteam_heal;

constexpr const char *MIXIN = "swofty_core.dll (my team vehicle keep)";
constexpr const char *LOAD = "site_Game_MyTeamVehicleBases_AiConfigLoad";
constexpr const char *NEXT = "site_Game_MyTeamVehicleBases_NextBase";
constexpr const char *SAVE = "site_Game_MyTeamVehicleBases_SaveRecord";

bool offered = false, settled = false, live = false;
uintptr_t manager_slot = 0;
std::mutex keep_lock;
mk::Pending pending;
mk::Shadows shadows;
mh::Ids said_kept, said_saved, said_failed;
char test_label[mh::LABEL_CHARS] = {};
bool test_stock_set = false;
mk::Stock test_stock;

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

bool row_index(const std::string &label, uint64_t &out)
{
    if (const SwoftyDb *db = runtime::db_table()) {
        SwoftyDbCursor c{};
        if (db->row("entrants.vehicles", label.c_str(), &c) == SWOFTY_ASSET_OK) {
            out = c.index;
            return true;
        }
    }
    return false;
}

bool stock_car(const std::string &label, mk::Stock &out)
{
    Game game;
    uintptr_t mgr = 0, entrants = 0;
    mh::Source source;
    if (manager_slot && mem::read(manager_slot, mgr) && mgr && mem::read(mgr + runtime::manager_entrants_system(), entrants) && entrants &&
        mk::pick(mh::stock(game, entrants), label, source)) {
        uint8_t engine = 0;
        out.label = source.label;
        if (!game.u32(source.vehicle, out.handle) || mh::kind(out.handle) != mh::KIND_STOCK ||
            !mem::read(static_cast<uintptr_t>(source.vehicle + mk::VEHICLE_ENGINE), engine))
            return false;
        out.engine = engine;
        return row_index(out.label, out.row_index);
    }
    if (!test_stock_set || test_stock.label == label)
        return false;
    out = test_stock;
    return true;
}

void failed(uint32_t stat, const std::string &label, const char *why)
{
    if (said_failed.first(uint16_t(stat)))
        swofty::log("[myteam] keep: My Team vehicle base (stat id %u) on '%s' can't be kept as a stock placeholder (%s); it is "
                    "skipped as before",
                    stat, label.c_str(), why);
}

void next_base(SwoftyContext *c)
{
    std::lock_guard<std::mutex> hold(keep_lock);
    if (!pending.live)
        return;
    mk::Pending p = pending;
    pending = mk::Pending{};
    uintptr_t car = c->rax;
    uint32_t h = 0;
    if (!car || !mem::readable(car, mk::SAVED_BYTES) || !mem::read(car, h) || h != p.handle) {
        swofty::log("[myteam] keep: the stock placeholder for '%s' was not stored where expected (car %p, handle %08x); "
                    "the next save may drop it",
                    p.original.c_str(), reinterpret_cast<void *>(car), h);
        return;
    }
    mk::Shadow s = mk::shadow_of(car, reinterpret_cast<const uint8_t *>(car), p);
    uint8_t engine = s.saved_engine;
    uint32_t base = s.saved_base;
    if (!shadows.put(std::move(s)))
        return;
    mk::patch(reinterpret_cast<uint8_t *>(car), p.stock);
    if (said_kept.first(uint16_t(p.handle >> 16)))
        swofty::log("[myteam] keep: My Team car (custom vehicle %u) on '%s' (engine %u, base car %08x) has no vehicle row (its "
                    "addon is off or gone); it loads as the stock '%s' (engine %u, base car %08x) so the career loads and its "
                    "id stays taken, and saves keep the original '%s' entry",
                    p.handle >> 16, p.original.c_str(), engine, base, p.stock.label.c_str(), p.stock.engine, p.stock.handle,
                    p.original.c_str());
}

void save_record(SwoftyContext *c)
{
    uintptr_t car = c->r14;
    std::lock_guard<std::mutex> hold(keep_lock);
    if (shadows.list.empty() || !car || !mem::readable(car, mk::SAVED_BYTES))
        return;
    const mk::Shadow *s = shadows.find(car, reinterpret_cast<const uint8_t *>(car));
    if (!s)
        return;
    c->r14 = reinterpret_cast<uintptr_t>(s->bytes.data());
    if (said_saved.first(uint16_t(s->handle >> 16)))
        swofty::log("[myteam] keep: the save writes My Team car (custom vehicle %u) as its original '%s' (engine %u, base car "
                    "%08x), not the stock placeholder '%s'",
                    s->handle >> 16, s->original.c_str(), s->saved_engine, s->saved_base, s->stock.c_str());
}

}

namespace runtime {

bool myteam_keep_forced(const std::string &label)
{
    return live && mk::forced(test_label, label);
}

bool myteam_keep_substitute(SwoftyContext *c, uint32_t stat, const std::string &label)
{
    if (!live || !c || !c->r12)
        return false;
    mk::Stock stock;
    if (!stock_car(label, stock)) {
        failed(stat, label, "no stock My Team car with a game database row is loaded");
        return false;
    }
    uint8_t row[mk::ROW_SIZE];
    if (!mem::readable(c->r12, mk::ROW_SIZE) || !mk::build_row(row, mk::VEHICLES_TABLE, mk::VEHICLES_ROW, stock.row_index)) {
        failed(stat, label, "the loader's row can't be rebuilt");
        return false;
    }
    std::lock_guard<std::mutex> hold(keep_lock);
    if (!shadows.room(0)) {
        failed(stat, label, "too many placeholders this session");
        return false;
    }
    std::memcpy(reinterpret_cast<void *>(c->r12), row, mk::ROW_SIZE);
    pending.live = true;
    pending.handle = mk::handle(stat);
    pending.original = label;
    pending.stock = stock;
    return true;
}

std::vector<CoreMixins> myteam_vehicle_keep_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_MYTEAM_VEHICLE_KEEP", v, sizeof v);
    if (n && n < sizeof v && mh::switch_off(v)) {
        swofty::log("[myteam] vehicle keep off (SWOFTY_MYTEAM_VEHICLE_KEEP=%s): a car whose row is gone is skipped", v);
        return out;
    }
    n = GetEnvironmentVariableA("SWOFTY_TEST_MYTEAM_KEEP_LABEL", test_label, sizeof test_label);
    if (!n || n >= sizeof test_label)
        test_label[0] = 0;
    char t[160] = {};
    n = GetEnvironmentVariableA("SWOFTY_TEST_MYTEAM_KEEP_STOCK", t, sizeof t);
    if (n && n < sizeof t)
        test_stock_set = mk::parse_stock(t, test_stock);
    manager_slot = swofty::api->resolve("g_Game_Manager");
    if ((!manager_slot && !test_stock_set) || !swofty::api->resolve(LOAD) || !swofty::api->resolve(NEXT) || !swofty::api->resolve(SAVE)) {
        swofty::log("[myteam] vehicle keep off: needs %s, %s, %s and g_Game_Manager, which this build does not all resolve; "
                    "a car whose row is gone is skipped",
                    LOAD, NEXT, SAVE);
        return out;
    }
    out.push_back({MIXIN,
                   {{NEXT, SWOFTY_MIXIN_SITE, 100, (void *)&next_base, nullptr},
                    {SAVE, SWOFTY_MIXIN_SITE, 100, (void *)&save_record, nullptr}}});
    return out;
}

void myteam_vehicle_keep_commit()
{
    if (!offered || settled || (!manager_slot && !test_stock_set))
        return;
    settled = true;
    live = core_mixins_active(MIXIN);
    swofty::log("[myteam] vehicle keep %s%s%s",
                live ? "armed: a My Team car whose row is gone loads as a stock placeholder that keeps its id, and saves keep "
                       "the original car"
                     : "did not install (see the [core] lines); a car whose row is gone is skipped",
                live && test_label[0] ? "; test car " : "", live && test_label[0] ? test_label : "");
}

}
