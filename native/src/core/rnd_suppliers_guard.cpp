#include "rnd_suppliers_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <map>
#include <mutex>
#include <set>
#include <string>
#include <vector>

// R&D supplier budget (re/db/functions.tsv Career_Add_TeamUpgrades, Career_Load_UpgradeTreeNodes).
//
// A career builds one Upgrades model per team in Career_Add_TeamUpgrades (0x14123ca40, component, team key, vehicle
// key). Its builder (vtable 0x1458477d0 slot +0x10, Career_Build_UpgradeTreeModel 0x14353abb0) loads the team's own tree
// through Career_Load_UpgradeTreeNodes (0x14353b030) with engine byte 0xff, then walks every row of
// "<group>.engine_suppliers.trees" and loads each engine_supplier_upgrades row with its parent_engine's stat id: every
// supplier's rows go into every team's tree, whichever engine the team uses. Career_Instantiate_UpgradeTrees
// (0x14352cac0) then makes one instance per loaded row in each tree (two trees when the component's operating_mode at
// +0x3e0 is PerDriverUnique, as in the My Team groups myteam_2025 and myteam_om_xm) and appends a pointer to each at tree
// + 0x30 (count tree + 0xc30, room for 384, no check: 0x14352cf30). The R&D collectors (0x143536fe0, reached from
// Career_Check_DevelopmentTabHasAffordableUpgrade / HasPartsToStart / FittingTabHasUnfittedParts, and ten more with the
// same 0xc00-byte stack buffer) copy the matching instances of all trees into one 384-pointer buffer, unchecked. Stock
// myteam_2025 gives 86 team + 26 supplier rows = 112 per tree, 224 for both; an addon that adds engine suppliers
// overflows the buffer once team + supplier rows pass 192 per tree, and Team HQ crashes.
//
// The guard wraps both functions. Career_Add_TeamUpgrades records the model's operating mode and vehicle; the first
// supplier row Career_Load_UpgradeTreeNodes sees for that model plans the supplier table from the game database: while
// the team's rows and every supplier's rows fit 384 / trees, every row loads as before. Past it the rows of the supplier
// whose engines_linked_to (or parent_engine) names the vehicle's engine load first, then the other suppliers in table
// order while they fit; the rest are not loaded into this team's trees ("virtual"): they stay in the database and load
// into the trees of a team whose vehicle uses that engine.
//
// SWOFTY_RND_SUPPLIER_GUARD=0 turns it off.

namespace runtime {
std::string hook_error(const safetyhook::InlineHook::Error &e);
}

namespace {

namespace mem = swofty::mem;
namespace rs = runtime::rnd_suppliers;

const runtime::Sig S_ADD{"Career_Add_TeamUpgrades",
                         "41 57 41 56 41 55 41 54 56 57 55 53 B8 78 04 04 00 E8 ?? ?? ?? ?? 48 29 C4 44 89 44",
                         0x123ca40};
const runtime::Sig S_LOAD{"Career_Load_UpgradeTreeNodes",
                          "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC D8 06 00 00 C5 78 29 84 24",
                          0x353b030};

constexpr uintptr_t COMPONENT_MODE = 0x3e0; // UpgradesComponent: operating_mode, copied to model + 8 (0x14123cb06)
constexpr uintptr_t KEY_PATH = 0x10;        // the loader's row key: char * of the joined path (0x14353b063)
constexpr size_t PATH_MAX_CHARS = 256;
constexpr size_t LOGGED_MAX = 256;

using Add_t = void *(void *component, uint32_t team, uint32_t vehicle);
using Load_t = void(void *container, void *key, uint8_t engine);

safetyhook::InlineHook h_add, h_load;

// One model being built (Career_Add_TeamUpgrades runs on the thread that loads the career).
struct Build {
    bool active = false;
    uint8_t mode = 0;
    uint32_t vehicle = 0;
    std::string team_path;
    std::string table;
    std::set<std::string> skip;
};
thread_local Build building;

std::mutex lock;
std::map<uint32_t, std::string> engines; // vehicle stat id -> entrants.vehicles engine label
std::set<std::string> logged;

int64_t count_at(const SwoftyDb *db, const std::string &table, const char *row)
{
    int64_t n = 0;
    std::string key = table + "._" + row + "_instancecount_";
    if (db->int_at(key.c_str(), &n) != SWOFTY_ASSET_OK || n < 0 || n > 8192)
        return 0;
    return n;
}

std::string text_at(const SwoftyDb *db, const std::string &key)
{
    const char *s = nullptr;
    if (db->string_at(key.c_str(), &s) != SWOFTY_ASSET_OK || !s)
        return {};
    return s;
}

// Rows a tree gets from one row path: one per upgrade row of each department (the loader's department table
// 0x145735d60, codes 0..4).
int rows_of(const SwoftyDb *db, const std::string &path)
{
    int64_t n = 0;
    for (const char *d : rs::DEPARTMENTS)
        n += count_at(db, path + "." + d, "upgrade");
    return int(n);
}

std::string engine_of(const SwoftyDb *db, uint32_t stat)
{
    std::lock_guard<std::mutex> g(lock);
    auto it = engines.find(stat);
    if (it != engines.end())
        return it->second;
    std::string out;
    int64_t n = count_at(db, "entrants.vehicles", "vehicle");
    for (int64_t i = 0; i < n; i++) {
        std::string row = "entrants.vehicles.vehicle_" + std::to_string(i);
        int64_t id = -1;
        if (db->int_at((row + ".stat_id").c_str(), &id) == SWOFTY_ASSET_OK && id == int64_t(stat)) {
            out = text_at(db, row + ".engine");
            break;
        }
    }
    engines.emplace(stat, out);
    return out;
}

void plan_for(Build &b, const std::string &table)
{
    b.table = table;
    const SwoftyDb *db = runtime::db_table();
    if (!db || !db->available || !db->available())
        return;
    uint32_t stat = rs::vehicle_stat(b.vehicle);
    std::string engine = engine_of(db, stat);
    int team = b.team_path.empty() ? 0 : rows_of(db, b.team_path);
    std::vector<rs::Supplier> suppliers;
    int64_t n = count_at(db, table, rs::SUPPLIER_ROW);
    for (int64_t i = 0; i < n; i++) {
        rs::Supplier s;
        s.row = table + "." + rs::SUPPLIER_ROW + "_" + std::to_string(i);
        s.rows = rows_of(db, s.row);
        s.chosen = rs::names(text_at(db, s.row + ".engines_linked_to"), engine) ||
                   (!engine.empty() && text_at(db, s.row + ".parent_engine") == engine);
        suppliers.push_back(std::move(s));
    }
    int limit = rs::budget(b.mode);
    rs::Plan p = rs::plan(team, limit, suppliers);
    std::string kept, left;
    int supplier_rows = 0;
    for (size_t i = 0; i < suppliers.size(); i++) {
        supplier_rows += suppliers[i].rows;
        std::string label = text_at(db, suppliers[i].row + "._metadata_");
        if (label.empty())
            label = suppliers[i].row;
        std::string &to = p.load[i] ? kept : left;
        to += to.empty() ? label : "," + label;
        if (!p.load[i])
            b.skip.insert(suppliers[i].row);
    }
    if (!p.over)
        return;
    std::string once = table + "|" + std::to_string(stat) + "|" + std::to_string(b.mode);
    {
        std::lock_guard<std::mutex> g(lock);
        if (logged.size() >= LOGGED_MAX || !logged.insert(once).second)
            return;
    }
    swofty::log("[rnd] %s, vehicle %u (engine %s): %d team + %d supplier rows pass %d per tree (%d tree(s) of 384); "
                "each tree keeps %d rows with suppliers %s; left out of this team's trees: %s",
                table.c_str(), unsigned(stat), engine.empty() ? "unknown" : engine.c_str(), team, supplier_rows, limit,
                rs::trees(b.mode), p.used, kept.empty() ? "no supplier" : kept.c_str(), left.c_str());
    if (p.short_of_chosen)
        swofty::log("[rnd] %s: the team's own supplier rows do not all fit %d per tree; a supplier of engine %s stays out",
                    table.c_str(), limit, engine.c_str());
    if (team > limit)
        swofty::log("[rnd] %s: the team tree %s alone has %d rows, more than %d per tree; the game's R&D lists can still "
                    "overflow",
                    table.c_str(), b.team_path.c_str(), team, limit);
}

void *add_hook(void *component, uint32_t team, uint32_t vehicle)
{
    Build saved = std::move(building);
    building = Build{};
    building.active = true;
    building.vehicle = vehicle;
    uint8_t mode = 0;
    if (component && mem::read(reinterpret_cast<uintptr_t>(component) + COMPONENT_MODE, mode))
        building.mode = mode;
    void *out = h_add.call<void *>(component, team, vehicle);
    building = std::move(saved);
    return out;
}

void load_hook(void *container, void *key, uint8_t engine)
{
    Build &b = building;
    std::string path;
    uintptr_t at = 0;
    if (b.active && key && mem::read(reinterpret_cast<uintptr_t>(key) + KEY_PATH, at) && at)
        mem::c_string(at, path, PATH_MAX_CHARS);
    if (b.active && !path.empty()) {
        if (engine == rs::OWN) {
            b.team_path = path;
        } else {
            std::string table = rs::table_of(path);
            if (!table.empty() && table != b.table)
                plan_for(b, table);
            if (b.skip.count(path))
                return;
        }
    }
    h_load.call<void>(container, key, engine);
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_RND_SUPPLIER_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[rnd] supplier budget off (SWOFTY_RND_SUPPLIER_GUARD=%s)", v);
        return;
    }
    uintptr_t add = runtime::find(S_ADD, "rnd supplier budget", "rnd");
    uintptr_t load = runtime::find(S_LOAD, "rnd supplier budget", "rnd");
    if (!add || !load) {
        swofty::log("[rnd] supplier budget off: Career_Add_TeamUpgrades or Career_Load_UpgradeTreeNodes did not resolve");
        return;
    }
    auto l = safetyhook::InlineHook::create(reinterpret_cast<void *>(load), reinterpret_cast<void *>(&load_hook));
    if (!l) {
        swofty::log("[rnd] supplier budget: the loader hook failed: %s", runtime::hook_error(l.error()).c_str());
        return;
    }
    h_load = std::move(*l);
    auto a = safetyhook::InlineHook::create(reinterpret_cast<void *>(add), reinterpret_cast<void *>(&add_hook));
    if (!a) {
        h_load = {};
        swofty::log("[rnd] supplier budget: the model hook failed: %s", runtime::hook_error(a.error()).c_str());
        return;
    }
    h_add = std::move(*a);
    swofty::log("[rnd] supplier budget armed: a team's R&D trees hold at most 384 / trees upgrade rows; past that only "
                "the team's own engine supplier's rows (then others while they fit) are loaded");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("rnd supplier budget", setup);
}
