#include "../../src/core/apply/content/boot.hpp"
#include "../../src/core/apply/ids.hpp"

#include <cstdio>
#include <map>

using namespace swofty_core;
using swofty::json::Value;
namespace content = apply::content;
namespace ids = apply::ids;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static const char *TABLE = "entrants.vehicles.vehicle_packages";

static std::map<std::string, std::string> table_with(const std::vector<int64_t> &stat_ids)
{
    std::map<std::string, std::string> db;
    db[std::string(TABLE) + "._vehicle_package_instancecount_"] = std::to_string(stat_ids.size());
    for (std::size_t i = 0; i < stat_ids.size(); i++) {
        std::string at = std::string(TABLE) + ".vehicle_package_" + std::to_string(i);
        db[at + ".stat_id"] = std::to_string(stat_ids[i]);
        db[at + "._metadata_"] = "row" + std::to_string(i);
    }
    return db;
}

static content::PoolScan scan_of(const std::map<std::string, std::string> &db)
{
    auto text = [&db](const std::string &path) -> std::optional<std::string> {
        auto it = db.find(path);
        return it == db.end() ? std::nullopt : std::optional<std::string>(it->second);
    };
    for (auto &s : content::scan_pools(text))
        if (s.registry == "vehicle.package")
            return s;
    return {};
}

static void load_pool(const content::PoolScan &scan)
{
    ids::store().clear();
    ids::Pool pool;
    if (scan.lo <= scan.hi)
        pool.ranges.push_back({scan.lo, scan.hi});
    ids::store().define_pool(scan.registry, pool);
    ids::store().hold(scan.registry, scan.used);
}

static content::StatIds allocator()
{
    return [](const std::string &registry, const std::string &id, int64_t pin) -> int64_t {
        if (pin >= 0)
            return ids::store().pin(registry, id, pin, "2026-11-06").id;
        return ids::store().allocate(registry, id, "2026-11-06").id;
    };
}

static Value package(int64_t pin = -1)
{
    Value rec = Value::object({{"donor", Value("game/f1_2025")}, {"resource_path", Value("sw_2022_vehicle_package/teams/[FILENAME]/wep/[FILENAME]")}});
    if (pin >= 0)
        rec["stat_id"] = Value(pin);
    return rec;
}

static bool wrote_stat_id(const content::Plan &p, int64_t want)
{
    for (std::size_t i = 0; i < p.ops.size(); i++) {
        const Value &op = p.ops[i];
        if (op["op"].as_string() == std::string("add-row") && op["set"]["stat_id"].is_string() &&
            std::string(op["set"]["stat_id"].as_string()) == std::to_string(want))
            return true;
    }
    return false;
}

int main()
{
    const std::string id = "swofty.swofty-seasons-2022/ferrari";
    {
        std::vector<int64_t> full;
        for (int64_t i = 0; i <= 254; i++)
            full.push_back(i);
        content::PoolScan scan = scan_of(table_with(full));
        check(scan.lo == 255 && scan.hi == 254, "a table that holds id 254 leaves an empty pool");
        load_pool(scan);
        ids::Result r = ids::store().allocate("vehicle.package", id, "2026-11-06");
        check(r.id < 0 && r.how == "full", "an unpinned package finds no id in the empty pool");
        check(r.line.find("the pool  is") == std::string::npos && r.line.find("no id above the highest") != std::string::npos,
              "and the refusal names the pool instead of printing an empty name");
        content::Plan p;
        content::lower_vehicle_package(id, package(), p, allocator());
        check(p.ops.size() == 0 && p.refused == 1, "the package lowers to no rows and counts as refused");
        check(!p.lines.empty() && p.lines.back().find(id) != std::string::npos &&
                  p.lines.back().find("left out") != std::string::npos,
              "and the line names the record");
    }
    {
        content::PoolScan scan = scan_of(table_with({0, 1, 2, 70, 138}));
        check(scan.lo == 139 && scan.hi == 254, "a table with room leaves the ids above its highest");
        load_pool(scan);
        content::Plan p;
        content::lower_vehicle_package(id, package(), p, allocator());
        check(p.refused == 0 && p.ops.size() == 2 && wrote_stat_id(p, 139), "an unpinned package takes the first free id, 139");
    }
    {
        std::map<std::string, std::string> db = table_with({0, 1, 2, 150});
        content::PoolScan scan = scan_of(db);
        check(scan.used.count(150) && scan.used[150] == "row3", "an id another addon's add-row gave is in the scan");
        ids::store().clear();
        ids::Pool pool;
        pool.ranges.push_back({148, 152});
        ids::store().define_pool("vehicle.package", pool);
        ids::store().hold("vehicle.package", {{148, "a"}, {149, "b"}, {150, "c"}, {151, "d"}});
        ids::Result r = ids::store().allocate("vehicle.package", id, "2026-11-06");
        check(r.id == 152, "ids the database holds are skipped by the allocator");
        ids::Result more = ids::store().allocate("vehicle.package", id + "2", "2026-11-06");
        check(more.id < 0, "and when they are all held the allocator refuses");
    }
    {
        load_pool(scan_of(table_with({0, 1, 2})));
        content::Plan p;
        content::lower_vehicle_package(id, package(300), p, allocator());
        check(p.ops.size() == 0 && p.refused == 1, "a pin above 254 is left out");
        content::Plan q;
        content::lower_vehicle_package(id, package(200), q, allocator());
        check(q.refused == 0 && wrote_stat_id(q, 200), "a pin inside the byte is written as it is");
    }
    {
        content::Plan p;
        content::lower_vehicle_package(id, package(), p);
        check(p.refused == 0 && p.ops.size() == 2, "the planning pass, with no allocator, still counts the package's rows");
    }
    {
        Value car = Value::object({{"label", Value("sw22_ferrari")}, {"donor", Value("game/ferrari")}, {"package", Value(id)}});
        content::Lookup lookup = [](const std::string &, const std::string &) -> const Value * { return nullptr; };
        content::Source source = [](const std::string &) -> std::optional<content::Stock> { return std::nullopt; };
        auto package_of = [&](const content::Dropped *dropped) {
            content::Plan p;
            content::lower_vehicle("swofty.swofty-seasons-2022/ferrari_car", car, lookup, source, p, {}, dropped);
            for (std::size_t i = 0; i < p.ops.size(); i++)
                if (p.ops[i]["op"].as_string() == std::string("add-row"))
                    return std::make_pair(p.ops[i]["set"]["vehicle_package"].is_string(), p.lines.back());
            return std::make_pair(false, std::string());
        };
        auto kept = package_of(nullptr);
        check(kept.first, "a vehicle names its package");
        content::Dropped dropped = {id};
        auto fell = package_of(&dropped);
        check(!fell.first && fell.second.find("left out") != std::string::npos,
              "a vehicle whose package was left out keeps its donor's package");
    }
    return failures ? 1 : 0;
}
