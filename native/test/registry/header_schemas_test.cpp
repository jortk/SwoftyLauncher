#include "../../src/core/registry/declare_all.hpp"
#include "../../src/core/registry/schema.hpp"
#include "../../src/core/registry/table.hpp"
#include "../../src/core/registry/validate.hpp"

#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <map>
#include <memory>
#include <sstream>
#include <string>

using namespace swofty_core::registry;
namespace fs = std::filesystem;
namespace cars = swofty::cars;

static int failures = 0;
#define CHECK(c, ...)                                  \
    do {                                               \
        if (!(c)) {                                    \
            std::printf("FAIL: " __VA_ARGS__);         \
            std::printf("\n");                         \
            failures++;                                \
        }                                              \
    } while (0)

static std::string slurp(const fs::path &p)
{
    std::ifstream f(p, std::ios::binary);
    std::stringstream ss;
    ss << f.rdbuf();
    return ss.str();
}

static void write(const fs::path &p, const std::string &text)
{
    fs::create_directories(p.parent_path());
    std::ofstream(p, std::ios::binary) << text;
}

static Schema::Result check(const Schema &s, const swofty::json::Value &v, Schema::Mode mode = Schema::Mode::records)
{
    static const Schema::RefResolver any = [](std::string_view, std::string_view) { return true; };
    Schema::Options o;
    o.mode = mode;
    o.keyed_paths = true;
    o.resolve = &any;
    return s.validate(v, o);
}

static std::string first_error(const Schema::Result &r)
{
    return r.ok() ? std::string("none") : r.errors.front().pointer + " " + r.errors.front().message;
}

static std::string handling(const std::string &op)
{
    return R"({"vtf": {"erp": "cars/teams/cater/wep/cater.erp", "resources": ["eaid://sw_2013_vehicle_package/teams/cater/handling/cater.vtf"]},
  "fuel_tank": {"fuel_optimal_revs": 1780.235837034216, "fuel_optimal_torque": 314.15776065474785},
  "rad_upgrades": [{"name": "radUpgradeEngine", "changes": [{"section": "fuel_tank", "key": "FuelOptimalTorque", "op": )" +
           op + R"(, "value": 314.15776065474785}]}]})";
}

static void published_examples(const std::map<std::string, std::string> &declared)
{
    fs::path root = "../../packages/sdk/std/registries";
    std::size_t checked = 0;
    for (auto &[name, text] : declared) {
        Schema::Error e;
        auto s = Schema::compile(std::string_view(text), &e);
        CHECK(s.has_value(), "%s: the header schema compiles (%s %s)", name.c_str(), e.pointer.c_str(), e.message.c_str());
        if (!s)
            continue;
        Schema::Version have = s->version();
        fs::path dir = root / name / "examples";
        if (!fs::is_directory(dir))
            continue;
        for (auto &f : fs::directory_iterator(dir)) {
            if (!f.is_regular_file() || f.path().extension() != ".json")
                continue;
            std::string stem = f.path().stem().string();
            if (auto at = stem.find('@'); at != std::string::npos) {
                int major = 0, minor = 0;
                if (std::sscanf(stem.c_str() + at + 1, "%d.%d", &major, &minor) == 2 &&
                    (major != have.major || minor > have.minor))
                    continue;
            }
            swofty::json::Value doc = swofty::json::parse(slurp(f.path()));
            const swofty::json::Value &records = doc[name];
            if (!records.is_object())
                continue;
            for (auto &[key, value] : *records.if_object()) {
                if (key == "$select")
                    continue;
                bool layer = key.find('/') != std::string::npos;
                auto r = check(*s, value, layer ? Schema::Mode::patch : Schema::Mode::records);
                checked++;
                CHECK(r.ok(), "%s %s %s: the schema the core declares from the headers refuses the published example: %s",
                      f.path().filename().string().c_str(), name.c_str(), key.c_str(), first_error(r).c_str());
            }
        }
    }
    std::printf("published examples checked against the header schemas: %zu\n", checked);
    CHECK(checked >= 40, "the published examples are found at %s (%zu)", root.string().c_str(), checked);
}

static void upgrade_ops(const std::string &text)
{
    auto s = Schema::compile(std::string_view(text));
    CHECK(s.has_value(), "vehicle.handling: the header schema compiles");
    if (!s)
        return;
    for (const char *op : {"\"=\"", "\"+\"", "\"-\"", "\"*\""}) {
        auto r = check(*s, swofty::json::parse(handling(op)));
        CHECK(r.ok(), "vehicle.handling: op %s is accepted: %s", op, first_error(r).c_str());
    }
    for (const char *op : {"\"/\"", "\"==\"", "\"\"", "61", "null"}) {
        auto r = check(*s, swofty::json::parse(handling(op)));
        CHECK(!r.ok() && r.errors.front().pointer == "/rad_upgrades/0/changes/0/op",
              "vehicle.handling: op %s is refused at /rad_upgrades/0/changes/0/op (%s)", op, first_error(r).c_str());
    }
    swofty::json::Value bad =
        swofty::json::parse(slurp("../../packages/sdk/std/registries/vehicle.handling/examples/must-fail/bad-op.json"));
    auto r = check(*s, bad["vehicle.handling"]["bad_op"]);
    CHECK(!r.ok(), "vehicle.handling: the published must-fail bad-op.json is refused");

    cars::Handling h;
    h.upgrade("radUpgradeEngine").set("fuel_tank", "FuelOptimalTorque", '=', 314.16f).set("engine", "WearICERate", '*', 0.5f);
    std::string ht = swofty::to_json(h);
    auto back = check(*s, swofty::json::parse(ht));
    CHECK(back.ok(), "vehicle.handling: a record the SDK writes validates: %s", first_error(back).c_str());
}

static void store_records()
{
    const char *tmp = std::getenv("TMPDIR");
    fs::path dir = fs::path(tmp ? tmp : "/tmp") / "swofty-header-schemas-test";
    fs::remove_all(dir);
    write(dir / "registry/index.json",
          R"({"version":1,"profile":"default","order":["x.base","x.real"],"data":{"x.base":["registry/x.base/content.json"],"x.real":["registry/x.real/handling.json"]},"registry":{},"addons":{}})");
    const std::pair<const char *, const char *> base[] = {{"v8_set", "\"=\""}, {"v8_add", "\"+\""}, {"v8_sub", "\"-\""},
                                                          {"v8_mul", "\"*\""}, {"v8_int", "61"},     {"v8_div", "\"/\""}};
    std::string content;
    for (auto &[key, op] : base)
        content += (content.empty() ? "{\"vehicle.handling\": {\"" : ", \"") + std::string(key) + "\": " + handling(op);
    write(dir / "registry/x.base/content.json", content + "}}");
    write(dir / "registry/x.real/handling.json", R"({"vehicle.handling": {
  "v8_real": {"extends": "x.base/v8_set", "vtf": {"erp": "cars/cater.erp", "resources": ["eaid://sw_2013_vehicle_package/teams/cater/handling/cater.vtf"]}},
  "v8_real_int": {"extends": "x.base/v8_int", "vtf": {"erp": "cars/cater.erp", "resources": ["eaid://sw_2013_vehicle_package/teams/cater/handling/cater.vtf"]}}
}})");

    store().set_validator(&schema_validator());
    declare_system_registries([&](const std::string &name, const std::string &schema) {
        return store().declare(name, swofty::json::parse(schema), false, "") == 0;
    });
    IndexResult ix = load_index(dir.string());
    CHECK(ix.found && ix.files == 2 && ix.failed == 0, "the handling fixture loads (%zu file(s), %zu failed)", ix.files, ix.failed);
    store().end_register();

    const std::pair<const char *, char> ok[] = {
        {"x.base/v8_set", '='}, {"x.base/v8_add", '+'}, {"x.base/v8_sub", '-'}, {"x.base/v8_mul", '*'}, {"x.real/v8_real", '='}};
    for (auto &[id, op] : ok) {
        Record r = store().get("vehicle.handling", id);
        CHECK(r.status == Status::ok, "vehicle.handling %s is valid (%s: %s)", id, status_name(r.status), r.why.c_str());
        cars::Handling h;
        bool read = swofty::from_json(swofty::json::dump(r.value), h);
        CHECK(read && h.rad_upgrades.size() == 1 && h.rad_upgrades[0].changes.size() == 1 &&
                  h.rad_upgrades[0].changes[0].op == op && h.rad_upgrades[0].changes[0].key == "FuelOptimalTorque",
              "vehicle.handling %s reads into cars::Handling with op '%c'", id, op);
        CHECK(swofty::to_json(h).find(std::string("\"op\":\"") + op + "\"") != std::string::npos,
              "vehicle.handling %s writes op '%c' back as a one-character string", id, op);
    }
    Record vtf = store().get("vehicle.handling", "x.real/v8_real");
    CHECK(vtf.value["vtf"]["erp"].as_string() == "cars/cater.erp", "the variant's own vtf wins over its base's");

    const std::pair<const char *, const char *> bad[] = {
        {"x.base/v8_int", "/rad_upgrades/0/changes/0/op"},
        {"x.base/v8_div", "/rad_upgrades/0/changes/0/op"},
        {"x.real/v8_real_int", "its base x.base/v8_int is invalid"}};
    for (auto &[id, why] : bad) {
        Record r = store().get("vehicle.handling", id);
        CHECK(r.status == Status::invalid && r.why.find(why) != std::string::npos, "vehicle.handling %s is invalid: %s (%s: %s)",
              id, why, status_name(r.status), r.why.c_str());
    }
    fs::remove_all(dir);
}

int main()
{
    std::map<std::string, std::string> declared;
    std::size_t n = declare_system_registries([&](const std::string &name, const std::string &schema) {
        declared[name] = schema;
        return true;
    });
    CHECK(n >= 24 && declared.count("vehicle.handling"), "the headers declare the system registries (%zu)", n);
    published_examples(declared);
    upgrade_ops(declared["vehicle.handling"]);
    store_records();
    if (failures) {
        std::printf("%d failure(s)\n", failures);
        return 1;
    }
    std::printf("ok: the schemas the core declares from the headers accept the published examples and string upgrade ops\n");
    return 0;
}
