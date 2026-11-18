#include "../../src/core/apply/content/boot.hpp"
#include "../../src/core/apply/content/race_fuel.hpp"
#include "../../src/core/apply/content/box_options.hpp"
#include "../../src/core/apply/content/grid_rules.hpp"
#include "../../src/core/apply/content/quali_tyres.hpp"
#include "../../src/core/apply/content/tyre_change.hpp"
#include "../../src/core/apply/content/tyre_warmers.hpp"
#include "../../src/core/apply/content/weekend.hpp"
#include "../../src/core/apply/ids.hpp"
#include "../../src/core/apply/room.hpp"
#include "../../src/core/apply/scheduler.hpp"
#include "../../src/core/career_ops/teams_plan.hpp"
#include "../../src/core/career_ops/teams_store.hpp"
#include "../../src/core/team_slots_logic.hpp"
#include "../../src/core/registry/table.hpp"
#include "../../src/core/registry/validate.hpp"

#include <cstdio>
#include <fstream>
#include <set>
#include <sstream>

using namespace swofty_core;
using swofty::json::Value;
namespace content = apply::content;
namespace rows = apply::content::rows;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static std::string slurp(const std::string &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream s;
    s << f.rdbuf();
    return s.str();
}

struct Db {
    std::map<uint64_t, rows::Rec> recs;
    void put(const std::string &path, int tag, const std::string &text) { recs[content::gvdb::fnv64(path)] = {tag, text}; }
    rows::Base base()
    {
        return [this](uint64_t key) -> std::optional<rows::Rec> {
            auto it = recs.find(key);
            if (it == recs.end())
                return std::nullopt;
            return it->second;
        };
    }
};

static std::optional<std::string> written(const rows::Engine &e, const std::string &path)
{
    for (auto &w : e.writes())
        if (w.path == path)
            return w.text;
    return std::nullopt;
}

static Value op(const char *json) { return swofty::json::parse(json); }

static std::vector<std::string> holes(const std::vector<rows::Write> &applied, const rows::Base &base)
{
    std::unordered_map<uint64_t, std::string> have;
    for (auto &w : applied)
        have[w.key] = w.text;
    auto exists = [&](const std::string &path) {
        uint64_t k = content::gvdb::fnv64(path);
        return have.count(k) || base(k);
    };
    const std::string tail = "_instancecount_";
    std::vector<std::string> out;
    for (auto &w : applied) {
        std::size_t dot = w.path.rfind("._");
        if (dot == std::string::npos || w.path.size() < tail.size() ||
            w.path.compare(w.path.size() - tail.size(), tail.size(), tail) != 0)
            continue;
        std::string table = w.path.substr(0, dot);
        std::string row = w.path.substr(dot + 2, w.path.size() - dot - 2 - tail.size());
        auto stock = base(w.key);
        int64_t from = stock ? std::strtoll(stock->text.c_str(), nullptr, 10) : 0;
        for (int64_t i = from, n = std::strtoll(w.text.c_str(), nullptr, 10); i < n; i++) {
            std::string at = table + "." + row + "_" + std::to_string(i);
            if (!exists(at + "._metadata_") && !exists(at))
                out.push_back(at);
        }
    }
    return out;
}

static std::vector<rows::Write> owned_by(const rows::Engine &e, const std::set<std::string> &owners)
{
    std::vector<rows::Write> out;
    for (auto &w : e.writes())
        if (owners.count(w.owner))
            out.push_back(w);
    return out;
}

static void engine_tests()
{
    rows::Keys keys;
    keys.parse("@t.rows.row_N\n_metadata_\nstat_id\nspeed\n+part\n_part_instancecount_\n"
               "@t.rows.row_N.part_N\n_metadata_\nweight\n");
    Db db;
    db.put("t.rows._row_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++) {
        std::string r = "t.rows.row_" + std::to_string(i);
        db.put(r + "._metadata_", 4, i ? "beta" : "alpha");
        db.put(r + ".stat_id", 0, std::to_string(40 + i));
        db.put(r + ".speed", 1, i ? "2.5" : "1.5");
        db.put(r + "._part_instancecount_", 0, "1");
        db.put(r + ".part_0._metadata_", 4, "p");
        db.put(r + ".part_0.weight", 0, i ? "9" : "8");
    }
    db.put("t.list.items", 13, "a, b");
    rows::Engine e(db.base(), keys);
    e.attribute("mod.a", "vehicle");
    std::string why = e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=beta]",
                                     "set":{"_metadata_":"gamma"}})"));
    check(why.empty(), "add-row copies a row by its label (" + why + ")");
    check(written(e, "t.rows.row_2._metadata_") == "gamma", "at the table's next index, with the set value");
    check(written(e, "t.rows.row_2.speed") == "2.5" && written(e, "t.rows.row_2.part_0.weight") == "9",
          "every key of the source row, nested rows included");
    check(written(e, "t.rows.row_2._part_instancecount_") == "1", "with the nested table's count");
    check(written(e, "t.rows._row_instancecount_") == "3", "and the table's count goes up by one");
    check(written(e, "t.rows.row_2.stat_id") == "42", "a top-level table with unique ids gives the highest plus one");
    bool fresh = false, count_fresh = true;
    for (auto &w : e.writes()) {
        if (w.path == "t.rows.row_2.speed")
            fresh = w.fresh && w.owner == "mod.a" && w.tag == 1;
        if (w.path == "t.rows._row_instancecount_")
            count_fresh = w.fresh;
    }
    check(fresh && !count_fresh, "new keys are fresh, the count is not; the source's tag and the owner are kept");

    why = e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=alpha]",
                         "set":{"_metadata_":"delta","stat_id":"77"}})"));
    check(why.empty() && written(e, "t.rows.row_3.stat_id") == "77", "a set stat_id wins");
    check(e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=zeta]"})")).find("no row matches") !=
              std::string::npos,
          "a missing source row fails");
    check(e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=alpha]","set":{"colour":"1"}})"))
                  .find("unknown field") != std::string::npos,
          "a set field the row doesn't have fails");
    check(written(e, "t.rows.row_4._metadata_") == std::nullopt, "a failing operation writes nothing");

    std::size_t mark = e.mark();
    e.attribute("mod.b", "vehicle");
    check(e.apply(op(R"({"op":"set","select":"t.rows.row[_metadata_=gamma]","field":"speed","value":"7"})")).empty(),
          "set resolves a row the engine added");
    check(e.apply(op(R"({"op":"list-append","select":"t","field":"list.items","value":"b,c"})")).empty() &&
              written(e, "t.list.items") == "a,b,c",
          "list-append adds a missing value once");
    e.rollback(mark);
    check(written(e, "t.rows.row_2.speed") == "2.5" && written(e, "t.list.items") == std::nullopt,
          "rollback takes back every write since the mark");
    check(e.apply(op(R"({"op":"set","select":"t.rows.row_0","field":"nothing","value":"1"})")).find("no such key") !=
              std::string::npos,
          "set on a missing key fails");
    check(e.apply(op(R"({"op":"put-record","key":"t.extra","type":2,"value":"6869"})")).empty() &&
              written(e, "t.extra") == "hi",
          "put-record writes the hex value under the path");
    db.put("t.rows.row_9._metadata_", 4, "beta");
    rows::Engine twice(db.base(), keys);
    check(twice.apply(op(R"({"op":"set","select":"t.rows.row[_metadata_=beta]","field":"speed","value":"1"})")).empty(),
          "only rows under the count are searched");
    db.put("t.rows.row_1._metadata_", 4, "alpha");
    check(twice.apply(op(R"({"op":"set","select":"t.rows.row[_metadata_=alpha]","field":"speed","value":"1"})"))
                  .find("2 rows match") != std::string::npos,
          "two rows with the label fail");
    check(rows::shape_of("a.b_12.c.d_3") == "a.b_N.c.d_N" && rows::shape_of("audio.x_2025.y") == "audio.x_N.y",
          "row shapes");

    rows::Engine guard(db.base(), keys);
    guard.attribute("a.x", "vehicle", "a.x/one");
    guard.apply(op(R"({"op":"put-record","key":"audio.car.sw.bundle","type":2,"value":"61"})"));
    guard.attribute("b.y", "vehicle", "b.y/two");
    check(guard.apply(op(R"({"op":"put-record","key":"audio.car.sw.bundle","type":2,"value":"62"})")).find("a.x/one created it") !=
                  std::string::npos &&
              written(guard, "audio.car.sw.bundle") == "a",
          "a record never writes a key another record created");
    check(guard.apply(op(R"({"op":"put-record","key":"t.list.items","type":13,"value":"62"})")).empty(),
          "keys the game holds stay shared");
}

// A small database with the stock rows a package copies.
static void room_hole_tests()
{
    rows::Keys keys;
    keys.parse("@t.rows.row_N\n_metadata_\nspeed\n");
    Db db;
    db.put("t.rows._row_instancecount_", 0, "1");
    db.put("t.rows.row_0._metadata_", 4, "stock");
    db.put("t.rows.row_0.speed", 1, "1.5");
    std::vector<std::string> order = {"swofty.swofty-seasons-2015", "swofty.swofty-seasons-2024"};
    auto lay = [&](const std::set<std::string> &skip) {
        auto e = std::make_unique<rows::Engine>(db.base(), keys);
        auto add = [&](const std::string &owner, const std::string &label) {
            if (skip.count(owner))
                return;
            e->attribute(owner, "driver.identity");
            e->apply(op(("{\"op\":\"add-row\",\"table\":\"t.rows\",\"from\":\"t.rows.row[_metadata_=stock]\","
                         "\"set\":{\"_metadata_\":\"" + label + "\"}}").c_str()));
            e->commit();
        };
        add(order[0], "sw15_driver");
        add(order[1], "sw24_driver");
        add(order[0], "sw-sw15_maldonado");
        return e;
    };
    auto fresh_of = [](const rows::Engine &e) {
        std::map<std::string, std::size_t> out;
        for (auto &w : e.writes())
            if (w.fresh)
                out[w.owner]++;
        return out;
    };
    auto full = lay({});
    auto counted = fresh_of(*full);
    check(counted[order[0]] == 4 && counted[order[1]] == 2 && written(*full, "t.rows._row_instancecount_") == "4",
          "a seat entrant laid out after every registry sits past the later addon's row, and its addon owns the count");
    namespace rm = apply::room;
    std::vector<rm::AddonAsk> asks = {{order[0], counted[order[0]], 0}, {order[1], counted[order[1]], 0}};
    auto per_addon = rm::admit(4, asks);
    auto torn = holes(owned_by(*full, {order[0]}), db.base());
    check(per_addon.addons[0].admitted && !per_addon.addons[1].admitted && torn.size() == 1 && torn[0] == "t.rows.row_2",
          "refusing the later addon after layout leaves the count at 4 with row 2 never created (1121flatpunto45l's "
          "crash): " + (torn.empty() ? std::string("none") : torn[0]));
    auto whole = rm::admit_whole(4, asks);
    check(!whole.admitted_keys && !whole.addons[0].admitted && holes(owned_by(*full, {}), db.base()).empty(),
          "past the room after layout, nothing is applied and the table keeps its own rows");
    auto again = content::readmit(order, counted, {}, 4);
    check(again.skip == std::set<std::string>{order[1]}, "readmitted on its counted keys, the later addon is left out");
    auto relaid = lay(again.skip);
    check(holes(relaid->writes(), db.base()).empty() && written(*relaid, "t.rows._row_instancecount_") == "3" &&
              written(*relaid, "t.rows.row_2._metadata_") == "sw-sw15_maldonado" && fresh_of(*relaid)[order[0]] <= 4,
          "laid out again without it, the seat entrant takes row 2 and every counted row exists");
}

static void stock_package_rows(Db &db)
{
    auto row = [&](const std::string &prefix, std::initializer_list<std::pair<const char *, std::string>> fields) {
        for (auto &[f, v] : fields)
            db.put(prefix + "." + f, std::string_view(f) == "_metadata_" ? 4 : 2, v);
    };
    db.put("entrants.entrant_lists.entrants._entrant_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrants.entrant_0",
        {{"_metadata_", "max_verstappen"}, {"driver", "max_verstappen"}, {"driver_customisation", "x"}, {"car_number", "1"}});
    db.put("entrants.standings._standings_instancecount_", 0, "1");
    row("entrants.standings.standings_0", {{"_metadata_", "f1_teams_2024"}});
    db.put("entrants.standings.standings_0._team_reference_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++)
        row("entrants.standings.standings_0.team_reference_" + std::to_string(i),
            {{"_metadata_", "t" + std::to_string(i)}, {"team", "stock"}});
    db.put("entrants.entrant_lists._entrant_list_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrant_list_0", {{"_metadata_", "f1_2025"},
                                                  {"display_name", "x"},
                                                  {"previous_season_team_standings", "x"},
                                                  {"previous_season_driver_standings", "x"},
                                                  {"leaderboard", "f1"}});
    db.put("entrants.entrant_lists.entrant_list_0.stat_id", 0, "104");
    db.put("entrants.entrant_lists.entrant_list_0._entrant_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++)
        row("entrants.entrant_lists.entrant_list_0.entrant_" + std::to_string(i),
            {{"_metadata_", "e"},
             {"team", "t"},
             {"vehicle", "v"},
             {"entrant_reference", "r"},
             {"team_position", "0"},
             {"car_number", "1"},
             {"driver", "d"},
             {"driver_customisation", "c"}});
    db.put("game_setup.scenarios._scenario_instancecount_", 0, "1");
    row("game_setup.scenarios.scenario_0", {{"_metadata_", "f1"},
                                            {"display_name", "x"},
                                            {"entrant_list", "f1_2025"},
                                            {"game_setup_mode", "f1"},
                                            {"race_class", "f1"},
                                            {"use_custom_vehicle_spec", "False"}});
    db.put("game_setup.scenarios.scenario_0.statId", 0, "14");
    std::string opt = "options_system.options.multiplayer.options_folder_2";
    db.put(opt + "._option_instancecount_", 0, "1");
    row(opt + ".option_0", {{"_metadata_", "category"}});
    db.put(opt + ".option_0._value_instancecount_", 0, "1");
    row(opt + ".option_0.value_0", {{"_metadata_", "f1_2025"},
                                    {"confirmation_dialogue", ""},
                                    {"description_override", ""},
                                    {"game_value", "f1_2025"},
                                    {"is_default", "False"},
                                    {"loc_tag", "x"},
                                    {"loc_tag_alternate", ""},
                                    {"vr_is_default", "False"}});
    db.put("entrants.entrant_setup_modes._entrant_setup_mode_instancecount_", 0, "1");
    row("entrants.entrant_setup_modes.entrant_setup_mode_0", {{"_metadata_", "swofty_f1"}});
    db.put("entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists", 13, "f1_2025");
    db.put("entrants.entrant_setup_modes.entrant_setup_mode_0.selectable_entrant_lists", 13, "f1_2025");
    db.put("audio.audio_bundle_settings._audio_bundle_settings_instancecount_", 0, "1");
    row("audio.audio_bundle_settings.audio_bundle_settings_0",
        {{"_metadata_", "mercedes_2025"}, {"bundle_name", "mercedes_2025"}, {"era", "x"}, {"engine", "e"}, {"exhaust", "x"}, {"cabin", "c"}});
    for (auto &[f, v] : std::initializer_list<std::pair<const char *, const char *>>{
             {"_metadata_", "f1_m_mercedes"}, {"bundle_name", "mercedes_2025.bdl"}, {"engine_offset_x", "0.1"}})
        db.put(std::string("audio.audio_car_data.f1_m_mercedes.") + f, std::string_view(f) == "engine_offset_x" ? 1 : 2, v);
    db.put("entrants.vehicles._vehicle_instancecount_", 0, "1");
    row("entrants.vehicles.vehicle_0", {{"_metadata_", "mercedes_2025"}, {"performance_tier", "0"}, {"tyre_wear_scale", "1"},
                                          {"drs_capable", "True"}});
    db.put("entrants.vehicles.vehicle_0.stat_id", 0, "508");
    db.put("content_sets._content_set_instancecount_", 0, "1");
    row("content_sets.content_set_0", {{"_metadata_", "2025"}});
    db.put("content_sets.content_set_0.available_entrant_lists_0.entrant_lists", 13, "f1_2025");
    db.put("leaderboards._leaderboard_descriptor_instancecount_", 0, "5");
    const char *boards[5][3] = {{"f1", "0", "f1"}, {"classic", "1", "classic"}, {"f2", "3", "f2"}, {"f1_26", "2", "f1"}, {"f2_26", "5", "f2"}};
    for (int i = 0; i < 5; i++) {
        std::string at = "leaderboards.leaderboard_descriptor_" + std::to_string(i);
        row(at, {{"_metadata_", boards[i][0]}, {"default_ghost_folder", boards[i][2]}});
        db.put(at + ".stat_id", 0, boards[i][1]);
    }
}

static void leaderboard_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.lb"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + (std::string_view(name) == "content.package" ? "/1.5.json" : "/1.0.json"));
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    auto package = [](const char *list, const char *board) {
        std::string leaderboard = board ? std::string(R"("leaderboard": ")") + board + R"(", )" : std::string();
        return std::string(R"({ "title": "lng_x", "entrant_list": ")") + list + R"(", "season": 2017, "stat_id": "auto", )" +
               leaderboard + R"("teams": [ { "key": "mercedes", "vehicle": null,
                 "drivers": [ { "driver": "game/lewis_hamilton", "number": 44, "look": null } ] } ],
                 "database": { "entrant_list": ")" + list + R"(", "scenario_stat_id": null, "content_sets": ["2025"],
                 "looks": { "mercedes/lewis_hamilton": "x" } } })";
    };
    std::string json = "{ \"content.package\": {\n";
    const std::pair<const char *, const char *> pkgs[] = {{"lb_one", nullptr},     {"lb_two", "box"},       {"lb_three", "addon"},
                                                         {"lb_four", "addon"},    {"lb_five", "stock:classic"}, {"lb_six", "group:eight"},
                                                         {"lb_seven", "group:eight"}};
    for (std::size_t i = 0; i < std::size(pkgs); i++)
        json += std::string(i ? ",\n" : "") + "\"p" + std::to_string(i) + "\": " + package(pkgs[i].first, pkgs[i].second);
    json += "} }";
    check(registry::load_data_file("mod.lb", "registry/content.json", json) == SWOFTY_ASSET_OK, "the leaderboard packages register");
    st.end_register();

    struct Run {
        std::map<std::string, std::string> writes;
        std::vector<std::string> lines;
        std::string ids;
        int refused = 0;
    };
    auto run = [&](const std::string &kept) {
        Db db;
        stock_package_rows(db);
        Run out;
        apply::ids::Store ids;
        if (!kept.empty())
            ids.load(kept);
        auto text = [&](const std::string &path) -> std::optional<std::string> {
            auto r = db.base()(content::gvdb::fnv64(path));
            return r ? std::optional<std::string>(r->text) : std::nullopt;
        };
        std::map<std::string, content::PoolScan> scans;
        for (auto &scan : content::scan_all_pools(text)) {
            ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
            scans[scan.registry] = scan;
        }
        content::settle_ids(ids, st, scans, "test");
        content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
            return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
        };
        content::Expansion x(db.base());
        content::expand(st, x, stat);
        for (auto &[k, n] : x.refused)
            out.refused += n;
        for (auto &w : x.engine.writes())
            out.writes[w.path] = w.text;
        out.lines = x.lines["content.package"];
        out.ids = ids.dump();
        return out;
    };
    auto row_of = [](const Run &r, const std::string &table, const std::string &row, const std::string &label) {
        for (int i = 0; i < 40; i++) {
            std::string at = table + "." + row + "_" + std::to_string(i);
            auto it = r.writes.find(at + "._metadata_");
            if (it != r.writes.end() && it->second == label)
                return at;
        }
        return std::string();
    };
    auto board_of_list = [&](const Run &r, const char *list) {
        auto at = row_of(r, "entrants.entrant_lists", "entrant_list", list);
        auto it = r.writes.find(at + ".leaderboard");
        return it == r.writes.end() ? std::string("none") : it->second;
    };
    auto count_boards = [&](const Run &r) {
        int n = 0;
        for (int i = 5; i < 40; i++)
            n += r.writes.count("leaderboards.leaderboard_descriptor_" + std::to_string(i) + "._metadata_");
        return n;
    };
    Run first = run({});
    check(first.refused == 0, "every package applies");
    for (auto &l : first.lines)
        std::printf("      %s\n", l.c_str());
    std::string one = row_of(first, "leaderboards", "leaderboard_descriptor", "swofty_lb_one");
    check(!one.empty() && first.writes[one + ".default_ghost_folder"] == "classic",
          "a package gets a descriptor of its own, copied from classic with its ghost folder");
    check(board_of_list(first, "lb_one") == "swofty_lb_one" && board_of_list(first, "lb_two") == "swofty_lb_two",
          "its entrant list points at that label, by default and with \"box\"");
    std::string two = row_of(first, "leaderboards", "leaderboard_descriptor", "swofty_lb_two");
    check(!two.empty() && first.writes[one + ".stat_id"] != first.writes[two + ".stat_id"] &&
              std::stoi(first.writes[one + ".stat_id"]) > 5 && std::stoi(first.writes[two + ".stat_id"]) > 5,
          "each box's id is its own, above the stock rows");
    check(board_of_list(first, "lb_three") == "swofty_addon_mod_lb" && board_of_list(first, "lb_four") == "swofty_addon_mod_lb" &&
              !row_of(first, "leaderboards", "leaderboard_descriptor", "swofty_addon_mod_lb").empty(),
          "\"addon\" shares one descriptor across the addon's packages");
    check(board_of_list(first, "lb_six") == "swofty_group_mod_lb_eight" && board_of_list(first, "lb_seven") == "swofty_group_mod_lb_eight",
          "\"group:<name>\" shares one among the packages that name it");
    check(board_of_list(first, "lb_five") == "classic" && row_of(first, "leaderboards", "leaderboard_descriptor", "classic").empty(),
          "\"stock:classic\" keeps the game's descriptor and adds no row");
    check(count_boards(first) == 4 && first.writes["leaderboards._leaderboard_descriptor_instancecount_"] == "9",
          "one row per box, per addon group and per named group: 4 rows added, the count is 9");
    std::set<std::string> ids;
    for (int i = 5; i < 9; i++)
        ids.insert(first.writes["leaderboards.leaderboard_descriptor_" + std::to_string(i) + ".stat_id"]);
    check(ids.size() == 4 && ids.count("") == 0, "no two added descriptors share an id");

    Run second = run(first.ids);
    check(second.writes == first.writes && second.ids == first.ids,
          "a second build from the kept ids writes the same rows, labels and ids");

    Db full;
    stock_package_rows(full);
    full.put("leaderboards._leaderboard_descriptor_instancecount_", 0, "5");
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        auto r = full.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    bool capped = false;
    for (auto &scan : content::scan_all_pools(text))
        if (scan.registry == content::LEADERBOARD_IDS)
            capped = scan.lo == 6 && scan.hi == 255;
    check(capped, "descriptor ids run from the table's highest plus one up to 255, the byte the game reads");
}

static void variant_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"swofty.classic-seasons-2017", "mod.esp"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + (std::string_view(name) == "vehicle" ? "/1.1.json" : "/1.0.json"));
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    int r1 = registry::load_data_file("swofty.classic-seasons-2017", "registry/content.json", R"j({
      "engine_sound": { "mercedes": { "bundle": "sounds/cs17_mercedes.bdl", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "mercedes": { "label": "cs17_mercedes", "donor": "game/mercedes_2025", "stat_id": 520,
                                 "engine_sound": "swofty.classic-seasons-2017/mercedes",
                                 "audio_switch": "f1_M_mercedes", "audio_from": "f1_m_mercedes" } },
      "content.package": { "f1_2017": {
        "title": "lng_cs_f1_2017", "entrant_list": "cs_f1_2017", "season": 2017, "stat_id": 106,
        "teams": [ { "key": "mercedes", "vehicle": null,
                     "drivers": [ { "driver": "game/lewis_hamilton", "number": 44, "look": null } ] },
                   { "key": "redbull", "vehicle": null,
                     "drivers": [ { "driver": "game/max_verstappen", "number": 33, "look": null } ] } ],
        "database": { "entrant_list": "cs_f1_2017", "scenario_stat_id": 16, "entrant_prefix": "cs2017-",
                      "standings": "cs_f1_teams_2017", "content_sets": ["2025"],
                      "looks": { "mercedes/lewis_hamilton": "2017-cs_mercedes_lewis_hamilton",
                                 "redbull/max_verstappen": "2017-cs_redbull_max_verstappen" } } } } })j");
    int r2 = registry::load_data_file("mod.esp", "registry/content.json", R"j({
      "engine_sound": { "v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/mercedes_2025" },
                        "mercedes_mix": { "extends": "swofty.classic-seasons-2017/mercedes", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "mercedes_v10": { "extends": "swofty.classic-seasons-2017/mercedes", "label": "esp_cs17_mercedes",
                                     "engine_sound": "mod.esp/v10_screamer", "drs_capable": false, "tyre_group": "F1_1990" },
                   "mercedes_copy": { "extends": "swofty.classic-seasons-2017/mercedes" } },
      "content.package": { "f1_2017_v10": {
        "extends": "swofty.classic-seasons-2017/f1_2017",
        "title": { "literal": "F1 2017 (V10)" }, "entrant_list": "esp_f1_2017_v10", "stat_id": "auto" } } })j");
    st.end_register();
    check(r1 == SWOFTY_ASSET_OK && r2 == SWOFTY_ASSET_OK, "the package and its variant register");
    auto v = st.get("content.package", "mod.esp/f1_2017_v10");
    check(v.status == registry::Status::ok && v.value["database"]["entrant_prefix"].as_string() == "cs2017-",
          "the variant inherits the base's private database block");

    Db db;
    stock_package_rows(db);
    auto pins = content::pins(st);
    check(pins.size() == 3 && pins[0].stat_id == 520 && pins[1].stat_id == 106 && pins[2].stat_id == 16 &&
              pins[2].registry == content::SCENARIO_IDS,
          "the bases pin their vehicle, entrant list and scenario ids; the variants inherit none");
    apply::ids::Store ids;
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    for (auto &scan : content::scan_all_pools(text))
        ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
    for (auto &p : pins)
        ids.pin(p.registry, p.id, p.stat_id, "test");
    content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
        return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
    };
    content::Expansion x(db.base());
    content::expand(st, x, stat);
    int refused = 0;
    for (auto &[k, n] : x.refused)
        refused += n;
    check(refused == 0 && x.records["content.package"] == 2, "both packages apply");
    for (auto &l : x.lines["content.package"])
        std::printf("      %s\n", l.c_str());
    auto has_label = [&](const std::string &table, const std::string &row, const std::string &label) {
        for (int i = 0; i < 8; i++)
            if (written(x.engine, table + "." + row + "_" + std::to_string(i) + "._metadata_") == label)
                return table + "." + row + "_" + std::to_string(i);
        return std::string();
    };
    std::string base_list = has_label("entrants.entrant_lists", "entrant_list", "cs_f1_2017");
    std::string var_list = has_label("entrants.entrant_lists", "entrant_list", "esp_f1_2017_v10");
    check(!base_list.empty() && !var_list.empty(), "each gets its own entrant list");
    check(written(x.engine, base_list + ".stat_id") == "106", "the base keeps its pinned entrant list id");
    check(written(x.engine, var_list + ".stat_id") == "105", "the variant's comes from the pool above the table's ids");
    check(!has_label("entrants.entrant_lists.entrants", "entrant", "cs2017-lewis_hamilton").empty() &&
              !has_label("entrants.entrant_lists.entrants", "entrant", "esp_f1_2017_v10-lewis_hamilton").empty(),
          "the variant's entrants are named for its own list, not the base's prefix");
    check(written(x.engine, var_list + ".entrant_0.entrant_reference") == "esp_f1_2017_v10-lewis_hamilton",
          "and its list points at them");
    std::string var_standings = has_label("entrants.standings", "standings", "esp_f1_2017_v10_teams");
    check(!has_label("entrants.standings", "standings", "cs_f1_teams_2017").empty() && !var_standings.empty(),
          "each gets its own standings row");
    check(written(x.engine, var_list + ".previous_season_team_standings") == "esp_f1_2017_v10_teams",
          "the variant's list reads its own standings");
    std::string base_sc = has_label("game_setup.scenarios", "scenario", "cs_f1_2017");
    std::string var_sc = has_label("game_setup.scenarios", "scenario", "esp_f1_2017_v10");
    check(written(x.engine, base_sc + ".statId") == "16" && written(x.engine, var_sc + ".statId") == "15",
          "the base keeps its scenario id, the variant gets the first free one above the stock scenarios");
    check(written(x.engine, var_list + ".display_name") == "F1 2017 (V10)", "a literal title is the list's name");
    std::string car = has_label("entrants.vehicles", "vehicle", "cs17_mercedes");
    std::string v10 = has_label("entrants.vehicles", "vehicle", "esp_cs17_mercedes");
    check(!car.empty() && written(x.engine, car + ".stat_id") == "520", "the base car keeps its pinned stat id");
    check(!v10.empty() && written(x.engine, v10 + ".stat_id") == "510" &&
              written(x.engine, has_label("entrants.vehicles", "vehicle", "mercedes_copy") + ".stat_id") == "509",
          "its variant gets one of its own from the pool, not the base's");
    std::string base_sw = "audio.audio_car_data.f1_M_mercedes.", v10_sw = "audio.audio_car_data.esp_cs17_mercedes.";
    check(written(x.engine, base_sw + "_metadata_") == "f1_M_mercedes" &&
              written(x.engine, base_sw + "bundle_name") == "cs17_mercedes.bdl",
          "the base car's sound rows keep the base's sound");
    check(written(x.engine, v10 + ".drs_capable") == "False" &&
              written(x.engine, car + ".drs_capable") == "True",
          "a vehicle's drs_capable reaches its row; a row that names none keeps the donor's");
    check(written(x.engine, v10_sw + "_metadata_") == "esp_cs17_mercedes" &&
              written(x.engine, v10_sw + "bundle_name") == "v10_screamer.bdl" &&
              written(x.engine, v10_sw + "engine_offset_x") == "0.1",
          "its variant's sound rows are under an audio switch of its own, named by its label");
    std::string copy = has_label("entrants.vehicles", "vehicle", "mercedes_copy");
    check(!copy.empty() && has_label("entrants.vehicles", "vehicle", "cs17_mercedes") == car &&
              written(x.engine, "audio.audio_car_data.mercedes_copy.bundle_name") == "cs17_mercedes.bdl",
          "a variant that inherits the label gets its key as its label and audio switch");
    int bundles = 0;
    for (int i = 0; i < 8; i++)
        bundles += written(x.engine, "audio.audio_bundle_settings.audio_bundle_settings_" + std::to_string(i) +
                                         "._metadata_") == "cs17_mercedes";
    check(bundles == 1 && !has_label("audio.audio_bundle_settings", "audio_bundle_settings", "v10_screamer").empty(),
          "an engine sound variant with its base's bundle adds no second row for it");
    int refused_rows = 0;
    for (auto &[k, n] : x.refused)
        refused_rows += n;
    check(refused_rows == 0 && x.records["vehicle"] == 3 && x.records["engine_sound"] == 3,
          "every base and variant applies");
    check(written(x.engine, "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists") ==
              "f1_2025,cs_f1_2017,esp_f1_2017_v10",
          "both lists join the setup mode, in load order");
    std::size_t base_keys = x.addon_fresh["swofty.classic-seasons-2017"], var_keys = x.addon_fresh["mod.esp"];
    check(base_keys > 0 && var_keys > 0 && x.fresh["vehicle"] > 0, "each addon's new keys are counted to it");

    auto adm = content::admit(st.order(), x.addon_fresh, base_keys + var_keys - 1);
    check(adm.skip.size() == 1 && adm.skip.count("mod.esp"), "past the room, the later addon is left out whole");
    content::Expansion y(db.base());
    content::expand(st, y, stat, {"swofty.classic-seasons-2017"});
    check(y.records["content.package"] == 1 && written(y.engine, "entrants.entrant_lists.entrant_list_1._metadata_") ==
                                                   "esp_f1_2017_v10",
          "with the first addon left out, the next one's rows start at the table's next index");

    content::Expansion p(db.base());
    content::expand(st, p, content::planning_ids());
    check(p.addon_fresh == x.addon_fresh && p.fresh == x.fresh, "the plan counts the new keys the boot writes, per addon");
    std::vector<std::string> order = st.order();
    check(holes(x.engine.writes(), db.base()).empty(), "a boot applied whole leaves no table claiming a missing row");
    namespace rm = apply::room;
    std::vector<rm::AddonAsk> asks;
    for (auto &a : order)
        asks.push_back({a, x.addon_fresh[a], 0});
    uint64_t room = std::min(base_keys, var_keys) == var_keys ? var_keys : base_keys + var_keys - 1;
    auto per_addon = rm::admit(room, asks);
    std::set<std::string> kept;
    for (auto &v : per_addon.addons)
        if (v.admitted)
            kept.insert(v.addon);
    auto torn = holes(owned_by(x.engine, kept), db.base());
    std::printf("      per-addon refusal at room %llu keeps %zu addon(s), %zu missing row(s)%s\n", (unsigned long long)room,
                kept.size(), torn.size(), torn.empty() ? "" : (": first " + torn.front()).c_str());
    if (var_keys < base_keys)
        check(!torn.empty() && kept.count("mod.esp") && !kept.count("swofty.classic-seasons-2017"),
              "refusing the first addon of a laid-out boot keeps the second's rows past the first's: the table claims "
              "rows that were never created (1121flatpunto45l's crash)");
    auto whole = rm::admit_whole(room, asks);
    check(!whole.admitted_keys && holes(owned_by(x.engine, {}), db.base()).empty(),
          "past the room, nothing of the laid-out boot is applied, so no table claims a missing row");
    auto again = content::readmit(order, x.addon_fresh, {}, room);
    check(again.skip.size() == 1, "the counted keys leave one addon out whole");
    content::Expansion z(db.base());
    content::expand(st, z, stat, again.skip);
    std::size_t z_keys = 0;
    for (auto &[a, n] : z.addon_fresh)
        z_keys += n;
    check(z_keys <= room && holes(z.engine.writes(), db.base()).empty(),
          "laid out again without it, the boot fits the room and no table claims a missing row");
    auto settled = content::readmit(order, z.addon_fresh, again, room);
    check(settled.skip == again.skip, "the second layout fits, so the readmission settles");
    auto none = content::readmit(order, x.addon_fresh, {}, base_keys + var_keys);
    check(none.skip.empty(), "a boot that fits the room is laid out once");
    std::map<std::string, std::size_t> core_first = x.addon_fresh;
    core_first["swofty.core"] = 5;
    check(content::readmit(order, core_first, {}, base_keys + var_keys).skip.size() == 1,
          "keys of owners outside the load order are charged before any addon");
}

// The stock wide rows next to the ones a 24-car package copies (f1_2025 and f1_teams_2024 from stock_package_rows):
// f1_2026 holds 22 entrants and f1_teams_2026 11 teams, as in the game; a creator's own 12-team standings row (made by
// a database operation) sits next to them.
static void stock_wide_rows(Db &db)
{
    stock_package_rows(db);
    auto row = [&](const std::string &prefix, std::initializer_list<std::pair<const char *, std::string>> fields) {
        for (auto &[f, v] : fields)
            db.put(prefix + "." + f, std::string_view(f) == "_metadata_" ? 4 : 2, v);
    };
    db.put("entrants.entrant_lists._entrant_list_instancecount_", 0, "2");
    row("entrants.entrant_lists.entrant_list_1", {{"_metadata_", "f1_2026"},
                                                  {"display_name", "x"},
                                                  {"previous_season_team_standings", "f1_teams_2025"},
                                                  {"previous_season_driver_standings", "x"},
                                                  {"leaderboard", "f1"}});
    db.put("entrants.entrant_lists.entrant_list_1.stat_id", 0, "111");
    db.put("entrants.entrant_lists.entrant_list_1._entrant_instancecount_", 0, "22");
    for (int i = 0; i < 22; i++)
        row("entrants.entrant_lists.entrant_list_1.entrant_" + std::to_string(i),
            {{"_metadata_", "e" + std::to_string(i)},
             {"team", "t"},
             {"vehicle", "v"},
             {"entrant_reference", "r"},
             {"team_position", std::to_string(i % 2)},
             {"car_number", "1"},
             {"driver", "d"},
             {"driver_customisation", "c"}});
    db.put("entrants.standings._standings_instancecount_", 0, "3");
    row("entrants.standings.standings_1", {{"_metadata_", "f1_teams_2026"}});
    db.put("entrants.standings.standings_1._team_reference_instancecount_", 0, "11");
    for (int i = 0; i < 11; i++)
        row("entrants.standings.standings_1.team_reference_" + std::to_string(i),
            {{"_metadata_", "s" + std::to_string(i)}, {"team", "stock_26"}});
    row("entrants.standings.standings_2", {{"_metadata_", "bdx_s5_teams"}});
    db.put("entrants.standings.standings_2._team_reference_instancecount_", 0, "12");
    for (int i = 0; i < 12; i++)
        row("entrants.standings.standings_2.team_reference_" + std::to_string(i),
            {{"_metadata_", "b" + std::to_string(i)}, {"team", "bdx"}});
    db.put("entrants.entrant_setup_modes.entrant_setup_mode_0.max_entrants", 0, "20");
}

static std::string twelve_team_package(const char *extra)
{
    static const char *keys[] = {"redbull", "mclaren", "ferrari", "mercedes", "lotus", "forceindia",
                                 "sauber", "tororosso", "williams", "caterham", "hrt", "marussia"};
    std::string teams;
    for (int t = 0; t < 12; t++) {
        std::string k = keys[t];
        teams += std::string(t ? "," : "") + R"({ "key": ")" + k + R"(", "vehicle": "game/)" + k + R"(_2012", "drivers": [ )" +
                 R"({ "driver": "game/)" + k + R"(_a", "number": )" + std::to_string(2 * t + 1) + R"(, "look": null }, )" +
                 R"({ "driver": "game/)" + k + R"(_b", "number": )" + std::to_string(2 * t + 2) + R"(, "look": null } ] })";
    }
    return R"({ "content.package": { "f1_2012": { "title": "lng_f2012", "entrant_list": "f2012", "season": 2012,
        "teams": [ )" + teams + R"( ], "database": { "entrant_list": "f2012", "entrant_prefix": "f12-", "standings": "f2012_teams",
        "content_sets": ["2025"] )" + extra + R"( } } } })";
}

static void twelve_team_tests(const std::string &regs)
{
    for (int pass = 0; pass < 2; pass++) {
        bool own_row = pass == 1;
        registry::reset_store();
        auto &st = registry::store();
        st.set_order({"bydanix.f1-2012-season"});
        st.set_validator(&registry::schema_validator());
        const SwoftyRegistry *t = registry::table();
        for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
            std::string schema = slurp(regs + "/" + name + "/1.0.json");
            t->declare(name, SwoftyText{schema.data(), schema.size()});
        }
        std::string data = twelve_team_package(own_row ? R"(, "standings_from": "bdx_s5_teams")" : "");
        int r = registry::load_data_file("bydanix.f1-2012-season", "registry/content.json", data);
        st.end_register();
        check(r == SWOFTY_ASSET_OK, std::string("a 12-team, 24-car package registers") + (own_row ? " (own standings_from)" : ""));
        Db db;
        stock_wide_rows(db);
        content::Expansion x(db.base());
        content::expand(st, x, {});
        int refused = 0;
        for (auto &[k, n] : x.refused)
            refused += n;
        for (auto &l : x.lines["content.package"])
            std::printf("      %s\n", l.c_str());
        std::string standings, list;
        for (int i = 0; i < 8; i++) {
            std::string sp = "entrants.standings.standings_" + std::to_string(i);
            std::string lp = "entrants.entrant_lists.entrant_list_" + std::to_string(i);
            if (written(x.engine, sp + "._metadata_") == "f2012_teams")
                standings = sp;
            if (written(x.engine, lp + "._metadata_") == "f2012")
                list = lp;
        }
        auto text = [&](const std::string &path) -> std::optional<std::string> {
            if (auto w = written(x.engine, path))
                return w;
            auto rec = db.base()(content::gvdb::fnv64(path));
            return rec ? std::optional<std::string>(rec->text) : std::nullopt;
        };
        bool added_line = false;
        for (auto &l : x.lines["content.package"])
            added_line |= l.find("standings f2012_teams has 12 teams; f1_teams_2024 holds 2, the rest are added") != std::string::npos;
        if (!own_row) {
            check(refused == 0 && x.records["content.package"] == 1,
                  "with no standings_from, a 12-team package applies on the stock f1_teams_2024 row");
            check(!standings.empty() && text(standings + "._team_reference_instancecount_") == "12",
                  "its standings row is grown to 12 teams");
            check(text(standings + ".team_reference_10.team") == "hrt_2012" &&
                      text(standings + ".team_reference_11.team") == "marussia_2012" &&
                      text(standings + ".team_reference_11._metadata_") == "marussia" &&
                      text(standings + ".team_reference_0.team") == "redbull_2012",
                  "in package order: the 12th is the package's 12th team");
            check(text("entrants.standings.standings_1._team_reference_instancecount_") == "11" &&
                      !text("entrants.standings.standings_1.team_reference_11._metadata_"),
                  "the stock f1_teams_2026 row keeps its 11 teams");
            check(added_line, "the boot log says the standings row was grown");
            check(!list.empty() && text(list + "._entrant_instancecount_") == "24" &&
                      text(list + ".entrant_23.entrant_reference") == "f12-marussia_b" &&
                      text(list + ".entrant_22.team_position") == "0" && text(list + ".entrant_23.team") == "marussia_2012",
                  "its entrant list is grown to 24 entrants");
            check(text("entrants.entrant_lists.entrant_list_1._entrant_instancecount_") == "22",
                  "the stock f1_2026 list keeps its 22 entrants");
            check(text("entrants.entrant_setup_modes.entrant_setup_mode_0.max_entrants") == "24",
                  "the setup mode takes 24 entrants");
        } else {
            check(refused == 0 && x.records["content.package"] == 1, "with standings_from naming a 12-team row it applies");
            check(!standings.empty() && text(standings + "._team_reference_instancecount_") == "12" &&
                      text(standings + ".team_reference_11.team") == "marussia_2012" &&
                      text(standings + ".team_reference_11._metadata_") == "b11",
                  "the creator's row is copied as it is and filled, nothing added");
            check(!added_line, "and no grown row is logged");
        }
    }
}

static void short_list_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"swofty.swofty-seasons-2012"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + "/1.0.json");
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    static const char *keys[] = {"redbull", "mclaren", "ferrari", "mercedes", "lotus", "forceindia",
                                 "sauber", "tororosso", "williams", "caterham", "hrt", "marussia"};
    std::string teams;
    for (int i = 0; i < 12; i++) {
        std::string k = keys[i];
        teams += std::string(i ? "," : "") + R"({ "key": ")" + k + R"(", "vehicle": "game/)" + k + R"(_2012", "drivers": [ )" +
                 R"({ "driver": "game/)" + k + R"(_a", "number": )" + std::to_string(i + 1) + R"(, "look": null } ] })";
    }
    std::string tt = R"({ "content.package": { "f1_2012_tt": { "title": "lng_f2012", "entrant_list": "f2012_tt", "season": 2012,
        "teams": [ )" + teams + R"( ], "database": { "entrant_list": "f2012_tt", "entrant_prefix": "f12-", "standings": "f2012_tt_teams",
        "entrant_list_from": "f1_2026", "content_sets": ["2025"] } },
        "f1_2012_one": { "title": "lng_f2012", "entrant_list": "f2012_one", "season": 2012,
        "teams": [ { "key": "redbull", "vehicle": "game/redbull_2012", "drivers": [ { "driver": "game/redbull_a", "number": 1, "look": null } ] } ],
        "database": { "entrant_list": "f2012_one", "entrant_prefix": "f12o-", "standings": "f2012_one_teams", "content_sets": ["2025"] } } } })";
    int r = registry::load_data_file("swofty.swofty-seasons-2012", "registry/content.json", tt);
    st.end_register();
    check(r == SWOFTY_ASSET_OK, "a one-car-per-team package registers");
    Db db;
    stock_wide_rows(db);
    content::Expansion x(db.base());
    content::expand(st, x, {});
    int refused = 0;
    for (auto &[k, n] : x.refused)
        refused += n;
    for (auto &l : x.lines["content.package"])
        std::printf("      %s\n", l.c_str());
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        if (auto w = written(x.engine, path))
            return w;
        auto rec = db.base()(content::gvdb::fnv64(path));
        return rec ? std::optional<std::string>(rec->text) : std::nullopt;
    };
    auto row_of = [&](const char *table, const char *row, const std::string &label) {
        for (int i = 0; i < 8; i++) {
            std::string p = std::string(table) + "." + row + "_" + std::to_string(i);
            if (written(x.engine, p + "._metadata_") == label)
                return p;
        }
        return std::string();
    };
    std::string list = row_of("entrants.entrant_lists", "entrant_list", "f2012_tt");
    std::string one = row_of("entrants.entrant_lists", "entrant_list", "f2012_one");
    std::string one_teams = row_of("entrants.standings", "standings", "f2012_one_teams");
    bool line = false;
    for (auto &l : x.lines["content.package"])
        line |= l.find("entrant list f2012_tt has 12 entrants; f1_2026 holds 22, the rest are left out") != std::string::npos;
    check(refused == 0 && x.records["content.package"] == 2, "both short packages apply");
    check(!list.empty() && text(list + "._entrant_instancecount_") == "12" &&
              text(list + ".entrant_11.team") == "marussia_2012",
          "a 12-car list copied from a 22-car one holds 12 entrants, not the source's last 10");
    check(text("entrants.entrant_lists.entrant_list_1._entrant_instancecount_") == "22",
          "the stock f1_2026 list keeps its 22 entrants");
    check(line, "the boot log says the rest of the source list is left out");
    check(!one.empty() && text(one + "._entrant_instancecount_") == "1", "a one-car list holds one entrant");
    check(!one_teams.empty() && text(one_teams + "._team_reference_instancecount_") == "1" &&
              text(one_teams + ".team_reference_0.team") == "redbull_2012",
          "a one-team package's standings hold one team");
    check(text("entrants.standings.standings_0._team_reference_instancecount_") == "2",
          "the stock f1_teams_2024 row keeps its 2 teams");
}

// The team and car rows a package's teams are checked against: three team rows inside the table's count and one
// past it, two cars.
static void package_team_row_tests()
{
    std::map<std::string, std::string> db = {
        {"entrants.teams._team_instancecount_", "3"},
        {"entrants.teams.team_0._metadata_", "cs16_mercedes"},
        {"entrants.teams.team_1._metadata_", "f1_world"},
        {"entrants.teams.team_2._metadata_", "f1_generic"},
        {"entrants.teams.team_3._metadata_", "beyond_count"},
        {"entrants.vehicles._vehicle_instancecount_", "2"},
        {"entrants.vehicles.vehicle_0._metadata_", "cs16_mercedes"},
        {"entrants.vehicles.vehicle_1._metadata_", "world_car"},
    };
    content::Read read = [&](const std::string &p) -> std::optional<std::string> {
        auto it = db.find(p);
        if (it != db.end())
            return it->second;
        auto at = p.find("[_metadata_=");
        if (at == std::string::npos)
            return std::nullopt;
        std::string table = p.substr(0, at), want = p.substr(at + 12, p.find(']', at) - at - 12);
        std::string container = table.substr(0, table.rfind('.')), row = table.substr(table.rfind('.') + 1);
        auto count = db.find(container + "._" + row + "_instancecount_");
        int n = count == db.end() ? 0 : std::atoi(count->second.c_str());
        for (int i = 0; i < n; i++) {
            auto m = db.find(table + "_" + std::to_string(i) + "._metadata_");
            if (m != db.end() && m->second == want)
                return want;
        }
        return std::nullopt;
    };
    std::map<std::string, Value> vehicles = {{"mod.a/mercedes", Value::object({{"label", "cs16_mercedes"}})},
                                                  {"mod.a/haas", Value::object({{"label", "cs16_haas"}})}};
    content::Lookup lookup = [&](const std::string &reg, const std::string &id) -> const Value * {
        auto it = vehicles.find(id);
        return reg == "vehicle" && it != vehicles.end() ? &it->second : nullptr;
    };
    auto team = [](const char *key, Value identity, const char *vehicle) {
        return Value::object({{"key", key}, {"team", std::move(identity)}, {"vehicle", vehicle},
                              {"drivers", Value::array({Value::object({{"driver", "game/d"}, {"number", 9}})})}});
    };
    content::ConstructorTeams constructors = {{"world", "mod.b/f1_world"}};
    auto lower = [&](Value teams, const content::Read &with) {
        content::Plan plan;
        Value rec = Value::object({{"entrant_list", "cs_f1_2016"}, {"title", "x"}, {"teams", std::move(teams)}});
        content::lower_package("mod.a/f1_2016", rec, lookup, plan, {}, nullptr, with, &constructors);
        return plan;
    };
    auto field = [](const content::Plan &p, const std::string &at, const char *name) -> std::string {
        for (std::size_t i = 0; i < p.ops.size(); i++) {
            const Value &op = p.ops[i];
            if (op["op"].as_string() == "set" && op["select"].as_string() == at && op["field"].as_string() == name)
                return std::string(op["value"].as_string());
        }
        return "<none>";
    };
    std::string list = std::string(content::LISTS) + "[_metadata_=cs_f1_2016]";

    content::Plan ok = lower(Value::array({team("mercedes", Value(), "mod.a/mercedes"),
                                           team("f1_world", Value("mod.b/f1_world"), "game/world_car")}),
                             read);
    check(!ok.refused && ok.ops.size() > 0, "a package whose team and car rows all exist applies");
    check(field(ok, list + ".entrant_0", "team") == "cs16_mercedes" && field(ok, list + ".entrant_0", "vehicle") == "cs16_mercedes",
          "a team with no team record names the row labelled like its car");
    check(field(ok, list + ".entrant_1", "team") == "f1_world" && field(ok, list + ".entrant_1", "vehicle") == "world_car" &&
              field(ok, list + ".entrant_1", "_metadata_") == "world_car_driver_01",
          "a team with a team record names that record's row, and its car stays its own");

    content::Plan no_team = lower(Value::array({team("mercedes", Value(), "mod.a/mercedes"),
                                                team("f1_world", Value(), "game/world_car")}),
                                  read);
    bool said = false;
    for (auto &l : no_team.lines)
        said |= l.find("team f1_world names entrants.teams row world_car, which the game database lacks") != std::string::npos;
    check(no_team.refused == 1 && no_team.ops.size() == 0 && said,
          "a team whose row the game lacks refuses the whole package and says which row");
    check(lower(Value::array({team("x", Value("mod.b/beyond_count"), "game/world_car")}), read).refused == 1,
          "a team row past the table's instance count counts as missing");
    content::Plan season = lower(Value::array({team("mercedes", Value(), "mod.a/mercedes"), team("haas", Value(), "mod.a/haas")}), read);
    bool expected = false;
    for (auto &l : season.lines)
        expected |= l.find("team haas names entrants.teams row cs16_haas, which the game database does not hold yet") != std::string::npos;
    check(!season.refused && season.ops.size() > 0 && expected &&
              field(season, list + ".entrant_1", "team") == "cs16_haas" && field(season, list + ".entrant_1", "vehicle") == "cs16_haas",
          "a Classic Seasons style package applies when its own car's team row is one its addon's database operations add");
    check(lower(Value::array({team("x", Value("mod.b/no_such_team"), "mod.a/haas")}), read).refused == 1,
          "a team record that names no row the game sees, with no row for its car either, refuses the package");
    content::Plan no_car = lower(Value::array({team("x", Value("game/f1_generic"), "game/no_such_car")}), read);
    bool warned = false;
    for (auto &l : no_car.lines)
        warned |= l.find("team x names entrants.vehicles row no_such_car, which the game database lacks") != std::string::npos;
    check(!no_car.refused && warned, "a car the game database lacks is reported, and the package still applies");
    content::Plan by_constructor = lower(Value::array({team("w", Value("constructor/world"), "game/world_car")}), read);
    check(!by_constructor.refused && field(by_constructor, list + ".entrant_0", "team") == "f1_world",
          "a constructor/ team names the row of the team that provides it");
    content::Plan fell = lower(Value::array({team("mercedes", Value("mod.b/no_row"), "mod.a/mercedes")}), read);
    bool noted = false;
    for (auto &l : fell.lines)
        noted |= l.find("its team mod.b/no_row gives no entrants.teams row (no_row) the game sees; it uses its car's row cs16_mercedes") !=
                 std::string::npos;
    check(!fell.refused && field(fell, list + ".entrant_0", "team") == "cs16_mercedes" && noted,
          "a team record without a row falls back to its car's row, and says so");
    content::Read empty = [](const std::string &) -> std::optional<std::string> { return std::nullopt; };
    check(!lower(Value::array({team("x", Value(), "game/no_such_car")}), empty).refused,
          "with no teams or vehicles table in the database nothing is checked");
}

static void garage_tests()
{
    std::map<std::string, std::string> db = {
        {std::string(content::VEHICLES) + "[_metadata_=cs20_lotus].vehicle_package", "cs_2020_lotus"},
        {std::string(content::VEHICLES) + "[_metadata_=cs20_mercedes].vehicle_package", "cs_2020_mercedes"},
        {std::string(content::VEHICLES) + "[_metadata_=renault_2025].vehicle_package", "f1_2025_renault"},
        {std::string(content::PACKAGES) + "[_metadata_=cs_2020_lotus].resource_path", "cs_2020_vehicle_package/teams/lotus/wep/lotus"},
        {std::string(content::PACKAGES) + "[_metadata_=cs_2020_mercedes].resource_path", "cs_2020_vehicle_package/teams/mercedes/wep/mercedes"},
        {std::string(content::PACKAGES) + "[_metadata_=f1_2025_renault].resource_path", "f1_2025_vehicle_package/teams/renault/wep/renault"},
    };
    content::Read read = [&](const std::string &p) -> std::optional<std::string> {
        auto it = db.find(p);
        return it == db.end() ? std::nullopt : std::optional<std::string>(it->second);
    };
    auto team = [](const char *key, const char *vehicle, const char *identity) {
        return Value::object({{"key", key}, {"team", identity}, {"vehicle", vehicle}});
    };
    std::map<std::string, Value> recs = {
        {"vehicle|mod.a/lotus", Value::object({{"label", "cs20_lotus"}})},
        {"vehicle|mod.a/mercedes", Value::object({{"label", "cs20_mercedes"}})},
        {"vehicle|mod.b/renault", Value::object({{"label", "renault_2025"}})},
        {"content.package|mod.a/f1_2020",
         Value::object({{"teams", Value::array({team("lotus", "mod.a/lotus", ""), team("mercedes", "mod.a/mercedes", "mod.a/merc")})}})},
        {"content.package|mod.b/f1_2025", Value::object({{"teams", Value::array({team("renault", "mod.b/renault", "game/renault")})}})}};
    content::Lookup lookup = [&](const std::string &reg, const std::string &id) -> const Value * {
        auto it = recs.find(reg + "|" + id);
        return it == recs.end() ? nullptr : &it->second;
    };
    auto lower = [&](const char *json) {
        content::Plan plan;
        Value rec = swofty::json::parse(json);
        content::lower_garage("mod.a/renault_2020", rec, lookup, read, plan);
        return plan;
    };

    content::Plan by_entry = lower(R"j({"package":"mod.a/f1_2020","entry":"lotus","textures":"garage/renault_2020"})j");
    check(by_entry.garages.size() == 1 && by_entry.garages[0].groups.size() == 1 &&
              by_entry.garages[0].groups[0] == "asset_groups/cs_2020_vehicle_package/teams/lotus/wep/lotus.assetgroup",
          "a garage by team entry key goes to that team's car group in the box");
    check(by_entry.garages[0].folder == "garage/renault_2020/" && by_entry.garages[0].srgb &&
              by_entry.garages[0].root == "eaid://environment_package/core_assets/objects/textures/",
          "the folder gains a slash and root and srgb take their defaults");
    check(by_entry.garages[0].game_path == "2025_asset_groups/swofty_garage/mod_a_renault_2020.erp",
          "the archive has a game path of its own per record");
    content::Plan by_team = lower(R"j({"package":"mod.a/f1_2020","team":"mod.a/merc","textures":"g","root":"eaid://x/","srgb":false})j");
    check(by_team.garages.size() == 1 && by_team.garages[0].groups[0] == "asset_groups/cs_2020_vehicle_package/teams/mercedes/wep/mercedes.assetgroup" &&
              by_team.garages[0].root == "eaid://x/" && !by_team.garages[0].srgb,
          "a garage by team identity matches the package entry that names it");
    check(lower(R"j({"package":"mod.b/f1_2025","entry":"renault","textures":"g"})j").garages[0].groups[0] ==
              "asset_groups/f1_2025_vehicle_package/teams/renault/wep/renault.assetgroup",
          "another box's garage lands on that box's own car group, never the 2020 ones");
    content::Plan other = lower(R"j({"package":"mod.a/f1_2020","entry":"lotus","textures":"g"})j");
    bool leaks = false;
    for (auto &g : other.garages[0].groups)
        leaks = leaks || g.find("f1_2025") != std::string::npos || g.find("mercedes") != std::string::npos;
    check(!leaks, "a box's garage names only the groups of its team's cars");
    for (const char *bad : {R"j({"package":"mod.a/f1_2020","entry":"ferrari","textures":"g"})j", R"j({"package":"mod.a/nope","entry":"lotus","textures":"g"})j",
                            R"j({"package":"mod.a/f1_2020","textures":"g"})j", R"j({"package":"mod.a/f1_2020","entry":"lotus"})j"}) {
        content::Plan p = lower(bad);
        check(p.garages.empty() && p.refused == 1, std::string("refused: ") + bad);
    }
    db.erase(std::string(content::VEHICLES) + "[_metadata_=cs20_lotus].vehicle_package");
    check(lower(R"j({"package":"mod.a/f1_2020","entry":"lotus","textures":"g"})j").garages.empty(),
          "a car without a vehicle row in the game database leaves nothing to attach to");
    check(!content::garage_art_enabled(nullptr) && !content::garage_art_enabled("") && !content::garage_art_enabled("0") &&
              !content::garage_art_enabled("on") && !content::garage_art_enabled("11") && content::garage_art_enabled("1"),
          "garage art stays fenced off unless SWOFTY_GARAGE_ART is exactly 1");
}

static void livery_tests()
{
    std::map<std::string, std::string> db = {
        {std::string(content::VEHICLES) + "[_metadata_=cs17_mercedes].vehicle_package", "cs_2017_mercedes"},
        {std::string(content::VEHICLES) + "[_metadata_=mercedes_2025].vehicle_package", "f1_2025_mercedes"},
        {std::string(content::PACKAGES) + "[_metadata_=cs_2017_mercedes].resource_path", "cs_2017_vehicle_package/teams/mercedes/wep/mercedes"},
        {std::string(content::PACKAGES) + "[_metadata_=f1_2025_mercedes].resource_path", "f1_2025_vehicle_package/teams/mercedes/wep/mercedes"},
        {std::string(content::SE_LIVERIES) + "._se_livery_instancecount_", "1"},
        {std::string(content::SE_LIVERIES) + ".se_livery_0.id", "9"},
        {std::string(content::SE_LIVERIES) + ".se_livery_0._metadata_", "mercedes_2025_bw"},
        {std::string(content::SE_LIVERIES) + ".se_livery_0.compatible_vehicle", "cs17_mercedes"},
        {"unlockable_content.unlockable_items.unlockable_item_list_0.folder_0._unlockable_se_livery_instancecount_", "1"},
        {"unlockable_content.unlockable_items.unlockable_item_list_0.folder_0.unlockable_se_livery_0._metadata_", "bw"},
        {"unlockable_content.unlockable_items.unlockable_item_list_0.folder_0.unlockable_se_livery_0.statId", "700"},
    };
    content::Read read = [&](const std::string &p) -> std::optional<std::string> {
        auto it = db.find(p);
        return it == db.end() ? std::nullopt : std::optional<std::string>(it->second);
    };
    std::map<std::string, Value> recs = {
        {"mod.a/car", Value::object({{"label", "cs17_mercedes"}, {"donor", "game/mercedes_2025"}})}};
    content::Lookup lookup = [&](const std::string &reg, const std::string &id) -> const Value * {
        auto it = recs.find(id);
        return reg == "vehicle" && it != recs.end() ? &it->second : nullptr;
    };
    content::StatIds stat = [](const std::string &, const std::string &, int64_t pin) -> int64_t { return pin >= 0 ? pin : 10; };

    content::Plan plan;
    Value look = Value::object({{"vehicle", "mod.a/car"}, {"archive", "cars/mercedes.erp"}, {"textures", "tex/"}});
    content::lower_livery("mod.a/look", look, {"mod.a/car"}, lookup, read, plan, stat);
    check(plan.groups.size() == 1 && plan.groups[0].erp == "2025_asset_groups/cs_2017_vehicle_package/teams/mercedes/wep/mercedes.erp",
          "a livery's archive is routed over the car's own wep archive");
    check(plan.groups[0].tmpl == "asset_groups/f1_2025_vehicle_package/teams/mercedes/wep/mercedes.assetgroup" &&
              plan.groups[0].common.size() == 2 &&
              plan.groups[0].common[0] == "2025_asset_groups/cs_2017_vehicle_package/teams/common.erp",
          "its asset group copies the donor's and lists the package's common archive first");
    check(plan.folders.size() == 1 && plan.folders[0].game_dir == "cs_2017_vehicle_package/teams/mercedes/textures/",
          "its textures folder is routed over the car's textures folder");
    check(plan.ops.size() == 0 && !plan.refused, "a plain livery writes no database rows");

    content::Plan none;
    content::lower_livery("mod.a/look", look, {}, lookup, read, none, stat);
    check(none.groups.empty() && none.folders.empty(), "a livery no car wears applies nothing");

    content::Plan special;
    Value sp = Value::object({{"vehicle", "mod.a/car"}, {"archive", "cars/mercedes.erp"}, {"special", true},
                              {"suffix", "auto"}, {"picker_name", "lng_x"}, {"unlock", "owned"}});
    int slot = 0;
    content::lower_livery("mod.a/sp", sp, {}, lookup, read, special, stat, &slot);
    check(special.ops.size() == 2 && special.owned.size() == 1 && special.owned[0].label == "cs17_mercedes_sp",
          "a special livery adds its se_livery row and unlock row and is reported owned");
    check(special.groups.size() == 1 && special.groups[0].erp == "2025_asset_groups/cs_2017_vehicle_package/teams/mercedes/wep/mercedes_lx01.erp",
          "its archive takes the next wep slot");
    std::string ops = swofty::json::dump(special.ops);
    check(ops.find("\"wep_override\"") != std::string::npos && ops.find("_lx01") != std::string::npos &&
              ops.find("mercedes_2025_bw") != std::string::npos,
          "the se_livery row is copied from the car's stock special livery with the slot as its wep override");

    check(ops.find("team_customisation") == std::string::npos,
          "a special livery cloned from the car's own stock row keeps that row's team customisation");

    db[std::string(content::VEHICLES) + "[_metadata_=cs23_redbull].vehicle_package"] = "cs_2017_mercedes";
    db[std::string(content::SE_LIVERIES) + ".se_livery_0.compatible_vehicle"] = "mercedes_2025";
    db[std::string(content::SE_LIVERIES) + ".se_livery_0.team_customisation"] = "2025-mclarensilverstone";
    recs["mod.a/car"] = Value::object({{"label", "cs23_redbull"}});
    content::Plan foreign;
    content::lower_livery("mod.a/sp", sp, {}, lookup, read, foreign, stat, &slot);
    std::string fops = swofty::json::dump(foreign.ops);
    check(foreign.ops.size() == 2 && fops.find("2025-mclarensilverstone") == std::string::npos &&
              fops.find("\"team_customisation\":\"\"") != std::string::npos,
          "a special on a car with no stock se_livery row takes no other team's team customisation");

    recs["mod.a/car"] = Value::object({{"label", "cs17_mercedes"}, {"donor", "game/mercedes_2025"}});
    db[std::string(content::SE_LIVERIES) + ".se_livery_0.compatible_vehicle"] = "cs17_mercedes";
    db.erase(std::string(content::SE_LIVERIES) + ".se_livery_0.team_customisation");
    content::Plan booted, unplanned, planned;
    int s1 = 0, s2 = 0, s3 = 0;
    content::lower_livery("mod.a/sp", sp, {}, lookup, read, booted, stat, &s1);
    content::lower_livery("mod.a/sp", sp, {}, lookup, read, unplanned, {}, &s2);
    content::lower_livery("mod.a/sp", sp, {}, lookup, read, planned, content::planning_ids(), &s3);
    check(booted.ops.size() == 2 && unplanned.ops.size() == 0 && unplanned.refused == 1,
          "without stat ids a special livery lowers to nothing, so the old plan missed its rows (315 keys of SWOFTY "
          "2021-2024)");
    check(planned.ops.size() == booted.ops.size() && !planned.refused,
          "the plan's own stat ids lower it to the rows the boot writes");

    content::Plan missing;
    Value bad = Value::object({{"vehicle", "mod.a/car"}, {"archive", "a.erp"}});
    recs["mod.a/car"] = Value::object({{"label", "nope"}});
    content::lower_livery("mod.a/look", bad, {"mod.a/car"}, lookup, read, missing, stat);
    check(missing.groups.empty() && missing.refused == 1, "a car without a vehicle row in the game database refuses the livery");
}

static void stock_team_rows(Db &db)
{
    auto row = [&](const std::string &path, std::initializer_list<std::pair<const char *, const char *>> fields) {
        for (auto &[f, v] : fields)
            db.put(path + "." + f, 2, v);
    };
    db.put("entrants.teams._team_instancecount_", 0, "2");
    const char *names[] = {"mercedes", "tororosso"};
    for (int i = 0; i < 2; i++) {
        std::string p = "entrants.teams.team_" + std::to_string(i);
        row(p, {{"_metadata_", names[i]}, {"display_name", "lng_stock"}, {"display_name_code", "STK"},
                {"display_name_short", "lng_stock_s"}, {"display_name_super_short", "lng_stock_ss"},
                {"team_base", "lng_base"}, {"livery_name", names[i]}, {"timing_tower_texture_path", names[i]},
                {"animation_set", "f1_2025"}, {"nationality", "britain"}, {"team_boss", "boss"},
                {"expression_label", names[i]}});
        db.put(p + ".stat_id", 0, i ? "3" : "0");
        db.put(p + ".available", 5, "False");
        db.put(p + ".audio_id", 0, "4");
        db.put(p + ".prestige", 0, "-1");
        for (const char *c : {"primary_colour", "secondary_colour", "tertiary_colour", "team_wear_primary_colour",
                              "team_wear_secondary_colour", "team_wear_tertiary_colour", "ui_team_colour"})
            db.put(p + "." + c, 11, "-1000");
        db.put(p + "._story_info_instancecount_", 0, "1");
        row(p + ".story_info_0", {{"_metadata_", "s"}, {"description", i ? "lng_desc_tr" : "lng_desc_mer"}, {"hashtag", "#x"}});
    }
    db.put("pit_lane.pit_times._team_pit_times_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++) {
        std::string p = "pit_lane.pit_times.team_pit_times_" + std::to_string(i);
        row(p, {{"_metadata_", names[i]}, {"team", names[i]}, {"min_pit_time", i ? "2.9" : "2.4"}, {"max_pit_time", "3.5"},
                {"min_pit_time_fully_upgraded", "2"}, {"max_pit_time_fully_upgraded", "3"}});
    }
    db.put("bank.initial_team_cash._InitialTeamCash_instancecount_", 0, "1");
    row("bank.initial_team_cash.InitialTeamCash_0", {{"_metadata_", "tororosso"}, {"team", "tororosso"}, {"cash", "5000"}});
    db.put("facilities.facilities._Facilities_instancecount_", 0, "2");
    for (int g = 0; g < 2; g++) {
        std::string gp = "facilities.facilities.Facilities_" + std::to_string(g);
        row(gp, {{"_metadata_", g ? "2021" : "base"}});
        db.put(gp + ".team_developments._TeamDevelopment_instancecount_", 0, g ? "0" : "1");
    }
    std::string td = "facilities.facilities.Facilities_0.team_developments.TeamDevelopment_0";
    row(td, {{"_metadata_", "tr"}, {"team", "tororosso"}, {"installed", "spec_1,spec_2"}, {"purchased", ""}, {"shutdown", ""}});
    db.put(td + "._date_instancecount_", 0, "1");
    row(td + ".date_0", {{"_metadata_", "d"}, {"day", "1"}, {"month", "2"}, {"year", "2012"}});
    db.put("driver_market.driver_market._DataGroup_instancecount_", 0, "1");
    row("driver_market.driver_market.DataGroup_0", {{"_metadata_", "2025"}});
    db.put("driver_market.driver_market.DataGroup_0.team_driver_rating_range_requirements._TeamDriverRatingRangeRequirement_instancecount_", 0, "1");
    row("driver_market.driver_market.DataGroup_0.team_driver_rating_range_requirements.TeamDriverRatingRangeRequirement_0",
        {{"_metadata_", "tororosso"}, {"team", "tororosso"}, {"rating_range", "60,80"}});
    db.put("showroom.teams_history._team_history_instancecount_", 0, "1");
    row("showroom.teams_history.team_history_0", {{"_metadata_", "tororosso"}, {"team", "tororosso"}, {"team_name", "n"}});
}

static void team_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.a", "mod.b"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"team.identity", "team.constructor", "team.pit_times", "team.logo"}) {
        std::string schema = slurp(regs + "/" + name + (std::string(name) == "team.identity" ? "/1.1.json" : "/1.0.json"));
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    int r1 = registry::load_data_file("mod.a", "registry/teams.json", R"j({
      "team.logo": { "toro_rosso_2017": { "images": { "hud": "logos/hud.png", "driver_select": "logos/ds.png",
                                                       "hub": { "stock": "tororosso" } } },
                     "plain": { "images": { "team_select": "logos/ts.png" } } },
      "team.identity": {
        "toro_rosso_2017": { "donor": "game/tororosso", "constructor": "constructor/toro_rosso",
          "provides": [ "constructor/toro_rosso" ],
          "names": { "title": "lng_cs_toro", "short_name": { "literal": "Toro Rosso" }, "super_short": { "literal": "STR" }, "code": "STR" },
          "colours": { "menu": "#0A1E3C", "primary": "#0A1E3C", "secondary": "#C8102E", "tertiary": "#80FFFFFF" },
          "wear": null, "livery_name": "cs17_toro_rosso", "logo": "mod.a/toro_rosso_2017", "prestige": 2,
          "nationality": "italy", "stat_id": 600, "team_boss": "franz_tost", "team_base": { "literal": "Faenza, Italy" } },
        "lost": { "donor": "game/nobody", "names": { "code": "LST" }, "livery_name": "x" },
        "mercedes": { "donor": "game/mercedes", "names": { "code": "MER" }, "livery_name": "x" },
        "plain_team": { "donor": "game/mercedes", "names": { "code": "PLN" }, "livery_name": "cs_plain",
                        "logo": "mod.a/plain", "stat_id": "auto" } },
      "team.pit_times": { "toro_rosso": { "team": "mod.a/toro_rosso_2017", "from": "game/mercedes", "min_pit_time": 2.3 } } })j");
    int r2 = registry::load_data_file("mod.b", "registry/teams.json", R"j({
      "team.identity": { "toro_rosso_b": { "extends": "mod.a/toro_rosso_2017", "livery_name": "b_toro_rosso",
                                           "names": { "title": { "literal": "B Toro" } }, "stat_id": "auto" } } })j");
    st.end_register();
    for (auto &r : st.refusals())
        std::printf("      refused: %s %s %s %s: %s\n", r.addon.c_str(), r.registry.c_str(), r.id.c_str(), r.reason.c_str(), r.message.c_str());
    check(r1 == SWOFTY_ASSET_OK && r2 == SWOFTY_ASSET_OK, "the team records register");
    Db db;
    stock_team_rows(db);
    content::Read stock_read = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    for (auto &c : content::CONSTRUCTOR_SEEDS)
        st.add_base("team.constructor", c.key,
                    Value::object({{"name", Value(std::string(c.name))}, {"country", Value(std::string(c.country))}, {"team", Value()}}));
    auto stock = content::stock_teams(stock_read);
    for (auto &t : stock)
        st.add_base("team.identity", t.label, t.record);
    st.stock_loaded("team.identity", true);
    check(stock.size() == 2 && stock[0].label == "mercedes" && stock[1].record["stat_id"].as_int() == 3, "the stock team rows become team.identity bases with their stat ids");
    auto stock_mer = st.get("team.identity", "game/mercedes");
    check(stock_mer.status == registry::Status::ok && stock_mer.value["names"]["title"]["literal"].as_string() == "lng_stock" &&
              stock_mer.value["colours"]["primary"].as_string() == "#FFFC18" && stock_mer.value["provides"][0].as_string() == "constructor/mercedes",
          "a stock base takes its names and colours from the row and provides its real team's constructor id");
    auto cm = st.get("team.constructor", "constructor/mercedes");
    check(cm.status == registry::Status::ok && cm.value["team"].as_string() == "game/mercedes", "constructor/mercedes resolves to the stock row");
    auto pins = content::pins(st);
    check(pins.size() == 1 && pins[0].registry == "team.identity" && pins[0].stat_id == 600, "the pinned team stat id");
    apply::ids::Store ids;
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    bool pool = false;
    for (auto &scan : content::scan_all_pools(text)) {
        ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
        pool |= scan.registry == "team.identity" && scan.lo == 4 && scan.hi == 16382;
    }
    check(pool, "the team stat id pool starts above the table's ids");
    for (auto &p : pins)
        ids.pin(p.registry, p.id, p.stat_id, "test");
    content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
        return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
    };
    content::Expansion x(db.base());
    content::expand(st, x, stat);
    for (auto &l : x.lines["team.identity"])
        std::printf("      %s\n", l.c_str());
    for (auto &l : x.lines["team.pit_times"])
        std::printf("      %s\n", l.c_str());
    check(x.records["team.identity"] == 3 && x.refused["team.identity"] == 1 && st.get("team.identity", "mod.a/lost").status != registry::Status::ok,
          "three teams apply; the one with a taken label does not, and with the stock rows declared the one whose donor is no stock team never registers");
    check(written(x.engine, "entrants.teams._team_instancecount_") == "5", "the team row count grows by three");
    auto field = [&](const std::string &row, const char *f) { return written(x.engine, "entrants.teams." + row + "." + f); };
    auto team_row = [&](const char *label) {
        for (int i = 2; i < 5; i++)
            if (written(x.engine, "entrants.teams.team_" + std::to_string(i) + "._metadata_") == label)
                return "team_" + std::to_string(i);
        return std::string("none");
    };
    auto side_row = [&](const std::string &table, const std::string &row, const char *label) {
        for (int i = 0; i < 8; i++)
            if (written(x.engine, table + "." + row + "_" + std::to_string(i) + ".team") == label)
                return table + "." + row + "_" + std::to_string(i);
        return std::string("none");
    };
    std::string tr = team_row("toro_rosso_2017"), pl = team_row("plain_team"), vrow = team_row("toro_rosso_b");
    check(tr != "none" && pl != "none" && vrow != "none" && field(tr, "stat_id") == "600", "each applied team has a row at the next index; the pinned id is kept");
    check(field(tr, "display_name") == "lng_cs_toro" && field(tr, "display_name_short") == "Toro Rosso" &&
              field(tr, "display_name_super_short") == "STR" && field(tr, "display_name_code") == "STR",
          "names: a lang key stays one, a literal is the text");
    check(field(tr, "livery_name") == "cs17_toro_rosso" && field(tr, "nationality") == "italy" && field(tr, "prestige") == "2",
          "livery name, nationality and prestige");
    auto signed_argb = [](uint32_t v) { return std::to_string(int32_t(v)); };
    check(field(tr, "primary_colour") == signed_argb(0xFF0A1E3C) && field(tr, "ui_team_colour") == signed_argb(0xFF0A1E3C) &&
              field(tr, "tertiary_colour") == signed_argb(0x80FFFFFF),
          "colours are signed ARGB, #RRGGBB opaque");
    check(field(tr, "team_wear_primary_colour") == signed_argb(0xFF0A1E3C) && field(tr, "team_wear_secondary_colour") == signed_argb(0xFFC8102E),
          "no wear colours: the kit follows the primary triple");
    check(field(tr, "timing_tower_texture_path") == "toro_rosso_2017", "a hud image gives the team its own timing tower key");
    check(field(tr, "story_info_0.description") == "lng_desc_tr" && field(tr, "_story_info_instancecount_") == "1" &&
              field(pl, "team_base") == "lng_base" && field(pl, "team_boss") == "boss" && field(tr, "animation_set") == "f1_2025",
          "every other key, story_info included, is the donor's");
    check(field(tr, "team_boss") == "franz_tost" && field(tr, "team_base") == "Faenza, Italy",
          "team_boss and team_base: the principal's character label and the base text go on the row");
    check(field(tr, "available") == "False", "available stays false until an entrant list names the team");
    check(field(pl, "timing_tower_texture_path") == "plain_team", "a team with an image but none for hud or broadcast gets its own timing tower key");
    check(written(x.engine, "pit_lane.pit_times._team_pit_times_instancecount_") == "5", "three pit time rows are added");
    std::string pit = side_row("pit_lane.pit_times", "team_pit_times", "toro_rosso_2017");
    check(pit != "none" && written(x.engine, pit + ".min_pit_time") == "2.3" && written(x.engine, pit + ".max_pit_time") == "3.5" &&
              x.engine.text(pit + ".min_pit_time") == "2.3" && written(x.engine, pit + "._metadata_") == "toro_rosso_2017",
          "a team.pit_times record copies the named team's row and sets its values (the identity did not copy its own)");
    std::string pit_pl = side_row("pit_lane.pit_times", "team_pit_times", "plain_team");
    check(pit_pl != "none" && written(x.engine, pit_pl + ".min_pit_time") == "2.4", "without a record the pit times are the donor's");
    std::string pit_b = side_row("pit_lane.pit_times", "team_pit_times", "toro_rosso_b");
    check(pit_b != "none" && written(x.engine, pit_b + ".min_pit_time") == "2.9", "a variant's pit times are its donor's: it inherits the base's donor");
    std::string bank = side_row("bank.initial_team_cash", "InitialTeamCash", "toro_rosso_2017");
    check(bank != "none" && written(x.engine, bank + ".cash") == "5000" &&
              written(x.engine, "bank.initial_team_cash._InitialTeamCash_instancecount_") == "3",
          "initial cash is the donor's");
    std::string fac = side_row("facilities.facilities.Facilities_0.team_developments", "TeamDevelopment", "toro_rosso_2017");
    check(fac != "none" && written(x.engine, fac + ".installed") == "spec_1,spec_2" && written(x.engine, fac + ".date_0.year") == "2012" &&
              written(x.engine, fac + "._date_instancecount_") == "1" &&
              side_row("facilities.facilities.Facilities_1.team_developments", "TeamDevelopment", "toro_rosso_2017") == "none",
          "facilities: the data group that holds the donor's row gets one, with its nested dates; the others none");
    std::string dm = side_row("driver_market.driver_market.DataGroup_0.team_driver_rating_range_requirements", "TeamDriverRatingRangeRequirement", "toro_rosso_2017");
    check(dm != "none" && written(x.engine, dm + ".rating_range") == "60,80" &&
              side_row("showroom.teams_history", "team_history", "toro_rosso_2017") != "none",
          "driver market and showroom history rows");
    check(vrow != "none" && field(vrow, "livery_name") == "b_toro_rosso" && field(vrow, "display_name") == "B Toro" &&
              field(vrow, "stat_id") != "600" && field(vrow, "timing_tower_texture_path") == "toro_rosso_b",
          "a variant copies its base's row, takes its own stat id and its own names");
    check(field(vrow, "story_info_0.description") == "lng_desc_tr", "and the base team's copied keys");
    bool hud = false, ds = false, hub_alias = false, ts = false;
    for (auto &l : x.logos) {
        hud |= l.array == "osd.timing_tower.team_badges_hud" && l.key == "toro_rosso_2017" && l.file == "logos/hud.png";
        ds |= l.array == "teams.driver_select_team_logos" && l.key == "toro_rosso_2017" && l.context == "driver_select";
        hub_alias |= l.context == "hub" && !l.stock.empty();
        ts |= l.array == "team_logos.driver_career_2024_team_select" && l.key == "plain_team" && l.file == "logos/ts.png";
    }
    check(hud && ds && ts && !hub_alias, "logo images: the hud array by the timing tower key, the label arrays by the label, driver select wins over hub");
    auto fell = [&](const char *team, const char *array) {
        for (auto &l : x.logos)
            if (l.team == team && l.array == array && l.fallback)
                return l.stock + "|" + l.key + "|" + l.context;
        return std::string();
    };
    auto image = [&](const char *team, const char *array) {
        for (auto &l : x.logos)
            if (l.team == team && l.array == array && !l.fallback && l.stock.empty())
                return l.file + "|" + l.key + "|" + l.context;
        return std::string();
    };
    bool any_fell = false;
    for (auto &l : x.logos)
        any_fell |= (l.team == "plain_team" || l.team == "toro_rosso_2017") && l.fallback;
    check(!any_fell, "a team with an image of its own never takes its donor's slice");
    check(image("plain_team", "teams.driver_select_team_logos") == "logos/ts.png|plain_team|driver_select" &&
              image("plain_team", "teams.badges_broadcast") == "logos/ts.png|plain_team|broadcast" &&
              image("plain_team", "team_logos.driver_career_2024_team_select") == "logos/ts.png|plain_team|team_select",
          "a context with no image takes the team's own image of another context");
    check(image("plain_team", "osd.timing_tower.team_badges_hud") == "logos/ts.png|plain_team|hud" &&
              image("plain_team", "osd.timing_tower.team_badges_broadcast") == "logos/ts.png|plain_team|broadcast",
          "the timing tower arrays take it too, under the team's own badge key");
    check(image("toro_rosso_2017", "team_logos.driver_career_2024_team_select") == "logos/ds.png|toro_rosso_2017|team_select" &&
              image("toro_rosso_2017", "teams.badges_broadcast") == "logos/ds.png|toro_rosso_2017|broadcast" &&
              image("toro_rosso_2017", "osd.timing_tower.team_badges_broadcast") == "logos/hud.png|toro_rosso_2017|broadcast" &&
              image("toro_rosso_2017", "teams.driver_select_team_logos") == "logos/ds.png|toro_rosso_2017|driver_select",
          "a menu array borrows the driver select image first, a timing tower array the other tower context's");
    int said = 0, donor_said = 0;
    for (auto &l : x.lines["team.identity"]) {
        said += l == "team.logo: plain_team uses its team_select image for driver_select";
        said += l == "team.logo: toro_rosso_2017 uses its hud image for broadcast";
        said += l == "team.logo: toro_rosso_2017 uses its driver_select image for broadcast";
        donor_said += l.find("mercedes's") != std::string::npos || l.find("tororosso's") != std::string::npos;
    }
    check(said == 3 && donor_said == 0, "the borrowed image is logged once per team, image and context; no donor line");
    {
        content::Plan bare, ig;
        content::lower_logo_images("bare", "williams", "mod.c/bare", nullptr, "", bare);
        bool donor_only = !bare.logos.empty();
        for (auto &l : bare.logos)
            donor_only &= l.fallback && l.stock == "williams" && l.file.empty() && l.array.rfind("osd.timing_tower.", 0) != 0;
        check(donor_only && bare.lines.size() == 3 && bare.lines[0] == "team.logo: bare uses williams's driver_select logo",
              "only a team with no image at all falls back to the donor's slice, outside the timing tower");
        Value pack = Value::object({{"images", Value::object({{"driver_select", Value("logos/ig_racing_driver_select.png")},
                                                               {"team_select", Value("logos/ig_racing_team_select.png")},
                                                               {"hud", Value("logos/ig_racing_hud.png")}})}});
        content::lower_logo_images("ig_racing", "williams", "ig.teampack/ig_racing", &pack, "ig_racing", ig);
        std::map<std::string, std::string> by_array;
        bool fallback = false;
        for (auto &l : ig.logos) {
            by_array[l.array] = l.file;
            fallback |= l.fallback;
        }
        check(!fallback && by_array["teams.badges_broadcast"] == "logos/ig_racing_driver_select.png" &&
                  by_array["broadcast_ui_generic.grid_team_logos"] == "logos/ig_racing_driver_select.png" &&
                  by_array["osd.timing_tower.team_badges_broadcast"] == "logos/ig_racing_hud.png" &&
                  by_array["osd.timing_tower.team_badges_hud"] == "logos/ig_racing_hud.png" &&
                  by_array["teams.driver_select_team_logos"] == "logos/ig_racing_driver_select.png" && by_array.size() == 7,
              "ig.teampack's three images: the broadcast badge (Choose Driver's card) is its driver select image, not williams's");
        std::vector<std::string> want = {"team.logo: ig_racing uses its hud image for broadcast",
                                         "team.logo: ig_racing uses its driver_select image for broadcast"};
        check(ig.lines == want, "and the core logs where each borrowed image came from");
    }
    std::map<std::string, Value> keep;
    auto lookup = content::own_lookup(st, keep);
    auto refs = content::refs_of(st, lookup);
    check(refs.constructor_team["toro_rosso"] == "mod.b/toro_rosso_b", "constructor/toro_rosso is the last enabled team in load order that is that constructor");
    check(content::team_label(Value("constructor/toro_rosso"), refs) == "toro_rosso_b", "and a constructor/ reference resolves to its label");
    check(refs.lines.size() == 1 && refs.lines[0].find("provided by mod.b/toro_rosso_b") != std::string::npos, "the constructor report line");

    namespace ct = career_ops::teams;
    auto season = [](int year, const char *team) {
        ct::SeasonIn s;
        s.year = year;
        ct::SlotIn slot;
        slot.slot = 1;
        slot.team = team;
        slot.seats = {"a", "b"};
        s.slots.push_back(slot);
        return s;
    };
    ct::Table table = ct::build_table({season(2012, "mod.a/toro_rosso_2017"), season(2013, "mod.b/toro_rosso_b")}, false);
    ct::Plan plan = ct::plan_season(table, season(2013, "mod.b/toro_rosso_b"));
    check(table.ok() && plan.ok() && plan.count(ct::Kind::dressed) == 1, "apply_teams plans slot 1 as dressed in 2013");
    std::vector<uint8_t> record(0x768, 0);
    std::vector<std::string> bufs(5, std::string(0x80, 'x'));
    for (std::size_t i = 0; i < 5; i++) {
        char *p = bufs[i].data();
        std::size_t at[] = {content::team_record::DISPLAY_NAME, content::team_record::SHORT, content::team_record::SUPER_SHORT,
                            content::team_record::CODE, content::team_record::TEAM_BASE};
        std::memcpy(record.data() + at[i], &p, sizeof p);
    }
    record[content::team_record::TEAM_BOSS] = 9;
    std::vector<std::string> log;
    auto &live = content::live();
    live = content::Live();
    live.log = [&](const std::string &l) { log.push_back(l); };
    live.identity = [&](const std::string &id) -> const Value * {
        auto r = st.get("team.identity", id);
        return r.status == registry::Status::ok ? &(keep["live|" + id] = r.value) : nullptr;
    };
    live.stat_of = [&](const std::string &id) -> int64_t { return ids.get("team.identity", id).id; };
    live.character_stat = [](const std::string &label) -> int64_t { return label == "franz_tost" ? 41 : -1; };
    ct::dresser() = &content::dress_slot;
    auto run_dresser = [&]() {
        int dressed = 0, pending = 0;
        for (auto &a : plan.actions)
            if (a.kind == ct::Kind::dressed) {
                if (ct::dresser() && ct::dresser()(table.rows.empty() ? "" : "mod.a.2012", a.slot, a.row.c_str(), a.team.c_str(), -1))
                    dressed++;
                else
                    pending++;
            }
        return std::pair<int, int>(dressed, pending);
    };
    auto none = run_dresser();
    check(none.first == 0 && none.second == 1 && live.pending.size() == 1, "without a way to the loaded team record the dressing is queued and reported pending");
    live.record = [&](uint16_t stat) -> uint8_t * { return stat == 600 ? record.data() : nullptr; };
    live.pending.clear();
    auto done = run_dresser();
    int32_t primary;
    std::memcpy(&primary, record.data() + content::team_record::PRIMARY, sizeof primary);
    check(done.first == 1 && done.second == 0, "apply_teams dresses the slot row through the dresser");
    check(std::string(bufs[0].c_str()) == "B Toro" && std::string(bufs[3].c_str()) == "STR" && primary == int32_t(0xFF0A1E3C),
          "the loaded record takes the season team's names and colours");
    check(std::string(bufs[4].c_str()) == "Faenza, Italy" && record[content::team_record::TEAM_BOSS] == 41,
          "and its base text and principal: team_boss is the stat id of the character row its label names");
    std::vector<uint8_t> reloaded(0x768, 0);
    std::vector<std::string> bufs2(4, std::string(0x80, 'y'));
    for (std::size_t i = 0; i < 4; i++) {
        char *p = bufs2[i].data();
        std::size_t at[] = {content::team_record::DISPLAY_NAME, content::team_record::SHORT, content::team_record::SUPER_SHORT, content::team_record::CODE};
        std::memcpy(reloaded.data() + at[i], &p, sizeof p);
    }
    check(content::redress(600, reloaded.data()) && std::string(bufs2[0].c_str()) == "B Toro",
          "a record the loader stores again is dressed again from the slot's remembered team");
    check(!content::redress(601, reloaded.data()), "and a stat id nobody dressed is left alone");

    live.stat_of = [&](const std::string &id) -> int64_t {
        int64_t got = ids.get("team.identity", id).id;
        if (got >= 0)
            return got;
        auto rec = st.get("team.identity", id);
        return rec.status == registry::Status::ok && rec.value["stat_id"].is_int() ? rec.value["stat_id"].as_int() : -1;
    };
    std::vector<uint8_t> stock_record(0x768, 0);
    std::vector<std::string> bufs3(5, std::string(0x80, 'z'));
    bufs3[4] = std::string("lng_base") + std::string(0x80 - 8, '\0');
    for (std::size_t i = 0; i < 5; i++) {
        char *p = bufs3[i].data();
        std::size_t at[] = {content::team_record::DISPLAY_NAME, content::team_record::SHORT, content::team_record::SUPER_SHORT,
                            content::team_record::CODE, content::team_record::TEAM_BASE};
        std::memcpy(stock_record.data() + at[i], &p, sizeof p);
    }
    stock_record[content::team_record::TEAM_BOSS] = 7;
    live.record = [&](uint16_t stat) -> uint8_t * { return stat == 3 ? stock_record.data() : nullptr; };
    check(ct::dresser()("g", 2, "game/tororosso", "mod.b/toro_rosso_b", -1) && std::string(bufs3[0].c_str()) == "B Toro" &&
              std::string(bufs3[4].c_str()) == "Faenza, Italy" && stock_record[content::team_record::TEAM_BOSS] == 41,
          "a stock slot row dresses through the stat id of its stock record");
    check(ct::dresser()("g", 2, "game/tororosso", "game/tororosso", -1) && std::string(bufs3[0].c_str()) == "lng_stock" && !live.wears.count(3),
          "a slot that returns to its own team gets the stock names back and is forgotten");
    check(std::string(bufs3[4].c_str()) == "lng_base" && stock_record[content::team_record::TEAM_BOSS] == 7 && !live.kept.count(3),
          "and its own base and principal, as the record held them before the dress");
    live.character_stat = nullptr;
    check(ct::dresser()("g", 2, "game/tororosso", "mod.b/toro_rosso_b", -1) && std::string(bufs3[4].c_str()) == "Faenza, Italy" &&
              stock_record[content::team_record::TEAM_BOSS] == 7 && log.back().find("its team_boss franz_tost is not an entrants.characters row") != std::string::npos,
          "a principal whose character row can't be found leaves the record's principal and says so");
    check(content::undress_all("the career unloaded") == 1 && std::string(bufs3[0].c_str()) == "lng_stock" &&
              std::string(bufs3[4].c_str()) == "lng_base" && stock_record[content::team_record::TEAM_BOSS] == 7 && live.wears.empty() &&
              live.kept.empty() && log.back() == "undress stat id 3: mod.b/toro_rosso_b taken off, game/tororosso back in its own names and colours (the career unloaded)",
          "a career unload puts every dressed record back in its own identity, base and principal");
    ct::dresser() = nullptr;
}

static void twelve_slot_tests(const std::string &regs)
{
    namespace ct = career_ops::teams;
    registry::reset_store();
    auto &st = registry::store();
    const std::string by = "bydanix.f1-2012-season", group = "bydanix_2012";
    st.set_order({by});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (auto [name, file] : {std::pair{"team.identity", "1.1.json"}, {"team.logo", "1.0.json"}, {"career.season", "1.0.json"}}) {
        std::string schema = slurp(regs + "/" + name + "/" + file);
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    const char *teams[] = {"red_bull", "mclaren", "ferrari", "mercedes", "lotus", "force_india",
                           "sauber", "toro_rosso", "williams", "caterham", "marussia", "hrt"};
    std::string logos, identities, slots, stock_slots;
    for (int i = 0; i < 12; i++) {
        std::string n = teams[i], sep = i ? "," : "";
        logos += sep + "\"" + n + "\": { \"images\": { \"team_select\": \"logos/" + n + "_ts.png\", \"hud\": \"logos/" + n + "_hud.png\" } }";
        identities += sep + "\"" + n + "\": { \"donor\": \"game/williams_26\", \"names\": { \"title\": { \"literal\": \"" + n +
                      " 2012\" }, \"code\": \"T" + std::to_string(i) + "\" }, \"colours\": { \"menu\": \"#10203" + std::to_string(i % 10) +
                      "\", \"primary\": \"#10203" + std::to_string(i % 10) + "\" }, \"livery_name\": \"c12_" + n + "\", \"logo\": \"" + by +
                      "/" + n + "\", \"team_boss\": \"boss_" + n + "\", \"team_base\": { \"literal\": \"Base " + n + "\" } }";
        slots += sep + "{ \"slot\": " + std::to_string(i + 1) + ", \"team\": \"" + by + "/" + n + "\" }";
    }
    for (int i = 0; i < 11; i++)
        stock_slots += std::string(i ? "," : "") + "{ \"slot\": " + std::to_string(i + 1) + ", \"team\": \"game/team26_" + std::to_string(i) + "\" }";
    int r = registry::load_data_file(by, "registry/teams.json",
                                     "{ \"team.logo\": {" + logos + "}, \"team.identity\": {" + identities + "}, \"career.season\": { \"2012\": { \"data_group\": \"" +
                                         group + "\", \"year\": 2012, \"teams\": [" + slots + "] }, \"stock_2026\": { \"data_group\": \"f1_career_2026\", \"year\": 2026, \"teams\": [" +
                                         stock_slots + "] } } }");
    st.end_register();
    Value zero("#00000000");
    Value stock = Value::object({{"names", Value::object({{"title", Value::object({{"literal", Value("lng_stock")}})}})},
                                 {"colours", Value::object({{"menu", zero}, {"primary", zero}, {"secondary", zero}, {"tertiary", zero}})},
                                 {"livery_name", Value("x")}});
    st.add_base("team.identity", "williams_26", stock);
    for (int i = 0; i < 12; i++)
        st.add_base("team.identity", "c12_slot_" + std::to_string(i), stock);
    for (int i = 0; i < 11; i++)
        st.add_base("team.identity", "team26_" + std::to_string(i), stock);
    st.stock_loaded("team.identity", true);
    for (auto &f : st.refusals())
        std::printf("      refused: %s %s %s: %s\n", f.addon.c_str(), f.registry.c_str(), f.id.c_str(), f.message.c_str());
    check(r == SWOFTY_ASSET_OK, "12 team.identity records with logos, principals and bases and a 12-slot season register");

    std::vector<ct::SeasonIn> seasons = ct::seasons_of(st, group);
    check(seasons.size() == 1 && seasons[0].year == 2012 && seasons[0].slots.size() == 12,
          "the data group's seasons are its own 2012 record, not the f1_career_2026 one");
    check(ct::build_table(seasons, true).refusals.size() == 1, "12 slots with My Team stays refused (13 teams)");
    ct::Table table = ct::build_table(seasons, false);
    std::vector<std::string> physical;
    for (int i = 0; i < 12; i++)
        physical.push_back("c12_slot_" + std::to_string(i));
    ct::anchor_rows(table, physical);
    ct::Plan plan = ct::plan_season(table, seasons[0]);
    check(table.ok() && plan.ok() && plan.count(ct::Kind::dressed) == 12, "apply_teams plans all 12 slots dressed: " + ct::summary(plan));

    struct Rec {
        std::vector<uint8_t> bytes = std::vector<uint8_t>(0x768, 0);
        std::vector<std::string> bufs = std::vector<std::string>(5, std::string(0x80, '\0'));
        std::vector<uint8_t> before;
    };
    std::map<uint16_t, Rec> loaded;
    auto load = [&](uint16_t stat, const std::string &label) {
        Rec &rec = loaded[stat];
        std::size_t at[] = {content::team_record::DISPLAY_NAME, content::team_record::SHORT, content::team_record::SUPER_SHORT,
                            content::team_record::CODE, content::team_record::TEAM_BASE};
        for (std::size_t i = 0; i < 5; i++) {
            std::memcpy(rec.bufs[i].data(), label.data(), std::min<std::size_t>(label.size(), 0x7f));
            char *p = rec.bufs[i].data();
            std::memcpy(rec.bytes.data() + at[i], &p, sizeof p);
        }
        rec.bytes[content::team_record::TEAM_BOSS] = 3;
        rec.before = rec.bytes;
    };
    for (int i = 0; i < 12; i++)
        load(uint16_t(700 + i), "c12_slot_" + std::to_string(i));
    for (int i = 0; i < 11; i++)
        load(uint16_t(i), "team26_" + std::to_string(i));
    std::map<std::string, Value> keep;
    std::vector<std::string> log;
    auto &live = content::live();
    live = content::Live();
    live.log = [&](const std::string &l) { log.push_back(l); };
    live.identity = [&](const std::string &id) -> const Value * {
        auto rec = st.get("team.identity", id);
        return rec.status == registry::Status::ok ? &(keep[id] = rec.value) : nullptr;
    };
    live.stat_of = [](const std::string &) -> int64_t { return -1; };
    live.record = [&](uint16_t stat) -> uint8_t * {
        auto it = loaded.find(stat);
        return it == loaded.end() ? nullptr : it->second.bytes.data();
    };
    live.character_stat = [&](const std::string &label) -> int64_t {
        for (int i = 0; i < 12; i++)
            if (label == std::string("boss_") + teams[i])
                return 50 + i;
        return -1;
    };
    ct::dresser() = &content::dress_slot;
    int dressed = 0;
    for (auto &a : plan.actions)
        dressed += a.kind == ct::Kind::dressed &&
                   ct::dresser()(group.c_str(), a.slot, ct::identity_id(a.row).c_str(), ct::identity_id(a.team).c_str(), 700 + a.slot - 1);
    bool all = dressed == 12;
    for (int i = 0; i < 12; i++) {
        Rec &rec = loaded[uint16_t(700 + i)];
        all &= std::string(rec.bufs[0].c_str()) == std::string(teams[i]) + " 2012" &&
               std::string(rec.bufs[3].c_str()) == "T" + std::to_string(i) &&
               std::string(rec.bufs[4].c_str()) == std::string("Base ") + teams[i] &&
               rec.bytes[content::team_record::TEAM_BOSS] == 50 + i;
    }
    check(all, "the 12 slot rows' loaded records wear the 12 teams' names, codes, bases and principals (" + std::to_string(dressed) + " dressed)");
    bool untouched = true;
    for (int i = 0; i < 11; i++)
        untouched &= loaded[uint16_t(i)].bytes == loaded[uint16_t(i)].before && std::string(loaded[uint16_t(i)].bufs[0].c_str()) == "team26_" + std::to_string(i);
    check(untouched, "the f1_career_2026 rows' loaded records are untouched");
    content::Plan logo_plan;
    for (int i = 0; i < 12; i++) {
        auto logo = st.get("team.logo", by + "/" + teams[i]);
        Value v = logo.value;
        content::lower_logo_images(teams[i], "williams_26", by + "/" + teams[i], logo.status == registry::Status::ok ? &v : nullptr,
                                   teams[i], logo_plan);
    }
    int own_slices = 0;
    for (auto &l : logo_plan.logos)
        own_slices += l.array == std::string("team_logos.driver_career_2024_team_select") && !l.fallback && l.key == l.team;
    check(own_slices == 12, "each team's logo is built as slices under its own label, not the slot row's (" + std::to_string(own_slices) + ")");
    check(content::undress_all("the career unloaded") == 12 && loaded[700].bytes == loaded[700].before &&
              loaded[711].bytes == loaded[711].before && std::string(loaded[705].bufs[4].c_str()) == "c12_slot_5",
          "after the career unloads the 12 slot rows are their own rows again, base and principal included");
    ct::dresser() = nullptr;
}

static void slot_tests()
{
    std::map<uintptr_t, uintptr_t> mem;
    uintptr_t vector = 0x1000, entries = 0x2000;
    mem[vector + 8] = entries;
    mem[vector + 0x10] = entries + 4 * 0x10;
    mem[entries + 3 * 0x10 + 8] = 0xabc000;
    auto read = [&](uintptr_t at, uintptr_t &out) {
        auto it = mem.find(at);
        if (it == mem.end())
            return false;
        out = it->second;
        return true;
    };
    check(team_slots::record_in(vector, 3, read) == 0xabc000, "a stat id's record is read through the slot vector");
    check(team_slots::record_in(vector, 4, read) == 0 && team_slots::record_in(0, 3, read) == 0, "a stat id past the vector's end has none");
    mem[vector + 0x10] = entries + 4 * 0x10 + 3;
    check(team_slots::record_in(vector, 3, read) == 0, "a vector whose size is not a whole number of entries is not read");
    check(team_slots::stat_of(600u << 16) == 600 && team_slots::stat_of(3u << 16) == 3, "the key carries the stat id in its high word");
    check(team_slots::database_key(600u << 16) && team_slots::database_key(0), "a loader key has no set byte: the low word is zero");
    check(!team_slots::database_key((600u << 16) | 1u), "a key with a set or flag byte set is not a database loader key");
}

static void seat_entrant_tests()
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"ig.teampack", "swofty.progressive"});
    for (const char *name : {"career.season", "driver.identity", "team.identity"})
        st.declare(name, swofty::json::parse(R"({"type":"object"})"), false, "");
    st.add("ig.teampack", "team.identity", "ig_racing", swofty::json::parse(R"({"donor":"game/williams"})"), nullptr);
    st.add("ig.teampack", "driver.identity", "ig_driver",
           swofty::json::parse(R"({"donor":"game/jarno_opmeer","look":"ig.teampack/ig_look","number":77})"), nullptr);
    st.add("ig.teampack", "driver.identity", "stub", swofty::json::parse(R"({"names":{"code":"STB"}})"), nullptr);
    st.add("swofty.progressive", "career.season", "2016",
           swofty::json::parse(R"({"data_group":"f1_career_progressive","year":2016,"teams":[
               {"slot":1,"team":"game/cs16_mercedes","seats":["game/lewis_hamilton","game/nico_rosberg"]},
               {"slot":11,"team":"ig.teampack/ig_racing","seats":["ig.teampack/ig_driver","ig.teampack/stub"]}]})"),
           nullptr);
    st.add("swofty.progressive", "career.season", "2017",
           swofty::json::parse(R"({"data_group":"f1_career_progressive","year":2017,"teams":[
               {"slot":11,"team":"ig.teampack/ig_racing","seats":["ig.teampack/ig_driver",""]}]})"),
           nullptr);
    st.end_register();
    auto seats = content::added_seats(st);
    check(seats.size() == 2 && seats["ig.teampack/ig_driver"].size() == 2 && !seats.count("game/lewis_hamilton"),
          "an added team's seats are found in every season that names it; a game/ team's seats are not");

    Db db;
    auto row = [&](const std::string &prefix, std::initializer_list<std::pair<const char *, std::string>> fields) {
        for (auto &[f, v] : fields)
            db.put(prefix + "." + f, std::string_view(f) == "_metadata_" ? 4 : 2, v);
    };
    db.put("career.career_setup._CareerSetupDataGroup_instancecount_", 0, "1");
    row("career.career_setup.CareerSetupDataGroup_0", {{"_metadata_", "f1_career_progressive"}});
    db.put("career.career_setup.CareerSetupDataGroup_0.ComCareerSetup_0.entrant_list", 4, "f1_career_progressive");
    db.put("entrants.drivers._driver_instancecount_", 0, "2");
    row("entrants.drivers.driver_0", {{"_metadata_", "jarno_opmeer"}});
    row("entrants.drivers.driver_1", {{"_metadata_", "ig_driver"}});
    db.put("entrants.entrant_lists.entrants._entrant_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrants.entrant_0", {{"_metadata_", "pg-daniel_ricciardo"},
                                                      {"driver", "daniel_ricciardo"},
                                                      {"driver_customisation", "x"},
                                                      {"car_number", "3"},
                                                      {"team", ""},
                                                      {"team_position", "-1"},
                                                      {"vehicle", ""},
                                                      {"vehicle_customisation", ""},
                                                      {"badge_customisation", ""},
                                                      {"entrant_reference", ""}});
    db.put("entrants.entrant_lists._entrant_list_instancecount_", 0, "3");
    row("entrants.entrant_lists.entrant_list_0", {{"_metadata_", "f1_career_progressive"},
                                                  {"entrant_list_references", "f1_2026,f1_career_progressive_driver_pool"}});
    db.put("entrants.entrant_lists.entrant_list_0._entrant_instancecount_", 0, "0");
    row("entrants.entrant_lists.entrant_list_1", {{"_metadata_", "f1_2026"}});
    db.put("entrants.entrant_lists.entrant_list_1._entrant_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrant_list_1.entrant_0", {{"_metadata_", "e"}, {"team", "mercedes_26"}, {"team_position", "0"},
                                                            {"entrant_reference", "pg-daniel_ricciardo"}});
    row("entrants.entrant_lists.entrant_list_2", {{"_metadata_", "f1_career_progressive_driver_pool"}});
    db.put("entrants.entrant_lists.entrant_list_2._entrant_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrant_list_2.entrant_0", {{"_metadata_", "pg_pool_daniel_ricciardo"},
                                                            {"entrant_reference", "pg-daniel_ricciardo"},
                                                            {"team", ""},
                                                            {"vehicle", ""},
                                                            {"team_position", "-1"},
                                                            {"car_number", "255"},
                                                            {"badge_customisation", ""},
                                                            {"vehicle_customisation", ""},
                                                            {"driver", ""},
                                                            {"driver_customisation", ""}});
    content::Expansion x(db.base());
    std::map<std::string, Value> keep;
    content::Lookup lookup = content::own_lookup(st, keep);
    content::Read read = [&x](const std::string &path) -> std::optional<std::string> {
        auto p = x.engine.resolve(path);
        return p ? x.engine.text(*p) : std::nullopt;
    };
    content::lower_seat_entrants(st, x, lookup, read, {});
    for (auto &l : x.lines["driver.identity"])
        std::printf("      %s\n", l.c_str());
    check(written(x.engine, "entrants.entrant_lists.entrants.entrant_1._metadata_") == "sw-ig_driver" &&
              written(x.engine, "entrants.entrant_lists.entrants.entrant_1.driver") == "ig_driver" &&
              written(x.engine, "entrants.entrant_lists.entrants.entrant_1.team_position") == "-1" &&
              written(x.engine, "entrants.entrant_lists.entrants.entrant_1.car_number") == "77",
          "the added team's new driver gets one teamless entrant with his number");
    check(written(x.engine, "entrants.entrant_lists.entrant_list_2.entrant_1._metadata_") == "sw_pool_ig_driver" &&
              written(x.engine, "entrants.entrant_lists.entrant_list_2.entrant_1.entrant_reference") == "sw-ig_driver" &&
              written(x.engine, "entrants.entrant_lists.entrant_list_2._entrant_instancecount_") == "2",
          "and an entry next to the pool's own in the teamless list the career list references");
    check(!written(x.engine, "entrants.entrant_lists.entrant_list_1._entrant_instancecount_") &&
              !written(x.engine, "entrants.entrant_lists.entrant_list_0._entrant_instancecount_"),
          "the seated list and the 0-entry career list are left alone");
    check(x.lines["driver.identity"].size() == 1, "a driver with no donor (a row something else made) gets nothing");
    content::Expansion y(db.base());
    content::Read ready = [&y](const std::string &path) -> std::optional<std::string> {
        auto p = y.engine.resolve(path);
        return p ? y.engine.text(*p) : std::nullopt;
    };
    content::lower_seat_entrants(st, y, lookup, ready, {"ig.teampack"});
    check(y.engine.writes().empty(), "an addon left out of the boot apply adds no entrant");
}

static Value package_of(int teams, int seats)
{
    Value list = Value::array();
    for (int t = 0; t < teams; t++) {
        Value drivers = Value::array();
        for (int d = 0; d < seats; d++)
            drivers.push_back(Value::object({{"driver", "game/d" + std::to_string(t * seats + d)}, {"number", t * seats + d + 1}}));
        list.push_back(Value::object({{"key", "t" + std::to_string(t)}, {"drivers", drivers}}));
    }
    return Value::object({{"entrant_list", "ex_f1_2012"}, {"teams", list}});
}

static void box_time_trial_tests()
{
    Value grid = package_of(12, 2), small = package_of(11, 2);
    content::Lookup lookup = [&](const std::string &reg, const std::string &id) -> const Value * {
        if (reg != "content.package")
            return nullptr;
        return id == "mod.example/f1_2012" ? &grid : id == "mod.example/f1_2025ish" ? &small : nullptr;
    };
    auto lower = [&](const char *json) {
        content::Plan plan;
        content::lower_box("mod.example/box", swofty::json::parse(json), lookup, plan);
        return plan;
    };
    auto warned = [](const content::Plan &p) {
        for (auto &l : p.lines)
            if (l.find("more than a Time Trial session holds (23)") != std::string::npos)
                return true;
        return false;
    };
    content::Plan p = lower(R"({"package":"mod.example/f1_2012","time_trial":{"entrant_list":"ex_f1_2012_tt"}})");
    check(p.boxes.size() == 1 && p.boxes[0].entrant_list == "ex_f1_2012" && p.boxes[0].time_trial == "ex_f1_2012_tt" &&
              !warned(p),
          "an era.box's time_trial entrant_list reaches the box, and a 24-car package with one is not warned about");
    p = lower(R"({"package":"mod.example/f1_2012"})");
    check(p.boxes.size() == 1 && p.boxes[0].time_trial.empty() && warned(p),
          "a 24-car package's box without a Time Trial list is kept and warned about");
    p = lower(R"({"package":"mod.example/f1_2012","time_trial":{"entrant_list":"ex_f1_2012"}})");
    check(p.boxes.size() == 1 && p.boxes[0].time_trial.empty() && warned(p),
          "a Time Trial list equal to the package's list is the package's list");
    p = lower(R"({"package":"mod.example/f1_2025ish"})");
    check(p.boxes.size() == 1 && p.boxes[0].time_trial.empty() && !warned(p), "a 22-car package's box needs no Time Trial list");
    p = lower(R"({"package":"mod.example/f1_2025ish"})");
    check(p.boxes.size() == 1 && p.boxes[0].only == 0, "an era.box without only shows on both pickers");
    p = lower(R"({"package":"mod.example/f1_2025ish","only":"time_trial"})");
    check(p.boxes.size() == 1 && p.boxes[0].only == 2, "an era.box's only time_trial reaches the box");
    p = lower(R"({"package":"mod.example/f1_2012","only":"grand_prix"})");
    check(p.boxes.size() == 1 && p.boxes[0].only == 1 && !warned(p),
          "a Grand Prix only box is not warned about its Time Trial car count");
    p = lower(R"({"package":"mod.example/f1_2025ish","only":null})");
    check(p.boxes.size() == 1 && p.boxes[0].only == 0, "a null only is both pickers");
}

static void drs_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.a", "mod.b"});
    st.set_validator(&registry::schema_validator());
    std::string schema = slurp(regs + "/rules.drs/1.0.json");
    registry::table()->declare("rules.drs", SwoftyText{schema.data(), schema.size()});
    int ra = registry::load_data_file("mod.a", "registry/drs.json", R"j({
      "rules.drs": { "never": { "race_start": 200, "red_flag_restart": 200, "safety_car_end": 200 },
                     "stock": {},
                     "late": { "extends": "mod.a/stock", "race_start": 3 } } })j");
    registry::load_data_file("mod.b", "registry/drs.json", R"j({ "rules.drs": { "bad": { "race_start": -1 } } })j");
    st.end_register();
    check(ra == SWOFTY_ASSET_OK && st.get("rules.drs", "mod.a/late").status == registry::Status::ok &&
              st.get("rules.drs", "mod.b/bad").status != registry::Status::ok,
          "rules.drs records register; a negative lap is refused by the schema");

    Db db;
    auto group = [&](int i, const char *label, const char *start) {
        std::string g = std::string(content::DRS_GROUPS) + "_" + std::to_string(i);
        db.put(g + "._metadata_", 2, label);
        db.put(g + "._drs_allowed_lap_instancecount_", 0, "3");
        const char *laps[] = {"RaceStart", "RedFlagRestart", "FullSafetyCarEnd"};
        for (int j = 0; j < 3; j++) {
            db.put(g + ".drs_allowed_lap_" + std::to_string(j) + "._metadata_", 2, laps[j]);
            db.put(g + ".drs_allowed_lap_" + std::to_string(j) + ".lap_number", 0, j ? "1" : start);
        }
    };
    db.put("drs.drs_allowed_lap._drs_allowed_lap_group_instancecount_", 0, "3");
    group(0, "f1_2024", "1");
    group(1, "f2_2023", "1");
    group(2, "f1_elimination", "0");
    content::StatIds stat = [](const std::string &, const std::string &, int64_t pin) -> int64_t { return pin >= 0 ? pin : 10; };
    content::Expansion x(db.base());
    content::expand(st, x, stat);
    for (auto &l : x.lines["rules.drs"])
        std::printf("      %s\n", l.c_str());
    check(x.records["rules.drs"] == 3 && x.refused["rules.drs"] == 0 && x.applied["rules.drs"].size() == 3,
          "three rules.drs records apply");
    check(written(x.engine, "drs.drs_allowed_lap._drs_allowed_lap_group_instancecount_") == "6", "the group count grows by three");
    std::map<std::string, std::string> rows;
    for (int i = 3; i < 6; i++)
        if (auto l = written(x.engine, std::string(content::DRS_GROUPS) + "_" + std::to_string(i) + "._metadata_"))
            rows[*l] = std::string(content::DRS_GROUPS) + "_" + std::to_string(i);
    std::string never = content::drs_label("mod.a/never"), late = content::drs_label("mod.a/late");
    check(rows.count(never) && rows.count(late) && rows.count(content::drs_label("mod.a/stock")),
          "each record is a group row labelled " + never);
    auto lap = [&](const std::string &label, int j) {
        return written(x.engine, rows[label] + ".drs_allowed_lap_" + std::to_string(j) + ".lap_number").value_or("-");
    };
    auto label_of = [&](const std::string &label, int j) {
        return written(x.engine, rows[label] + ".drs_allowed_lap_" + std::to_string(j) + "._metadata_").value_or("-");
    };
    check(lap(never, 0) == "200" && lap(never, 1) == "200" && lap(never, 2) == "200" && label_of(never, 0) == "RaceStart" &&
              label_of(never, 2) == "FullSafetyCarEnd",
          "the never group's RaceStart, RedFlagRestart and FullSafetyCarEnd rows hold 200");
    check(lap(late, 0) == "3" && lap(late, 1) == "1" && lap(late, 2) == "1", "a variant sets its race start and keeps the base's 1s");
    check(written(x.engine, std::string(content::DRS_GROUPS) + "_0.drs_allowed_lap_0.lap_number") == std::nullopt,
          "the stock f1_2024 group is not written");

    check(never.rfind("swofty_drs_never_", 0) == 0 && never.size() == 25 && never != content::drs_label("mod.b/never"),
          "a group label is swofty_drs_<key>_<8 hex of the id>, unique per id");
    std::string long_label = content::drs_label("mod.a/" + std::string(80, 'k') + "-x");
    check(long_label.size() <= content::DRS_LABEL_MAX && long_label.find('-') == std::string::npos,
          "a long or unplain key is cut and made plain within 63 characters");
    std::string why;
    check(!content::drs_laps(swofty::json::parse(R"({"race_start": 10000})"), &why) && !why.empty() &&
              content::drs_laps(swofty::json::parse("{}"), nullptr)->race_start == 1,
          "a lap past 9999 is refused; a missing lap is the stock 1");

    std::map<std::string, content::DrsGroup> groups = {{"mod.a/never", {never, {200, 200, 200}}}};
    Value p2010 = swofty::json::parse(R"({"entrant_list":"cs_2010","rules":{"drs":"mod.a/never"}})");
    Value p2011 = swofty::json::parse(R"({"entrant_list":"cs_2011","rules":{"drs":null}})");
    Value p2012 = swofty::json::parse(R"({"entrant_list":"cs_2012","rules":{"drs":"mod.b/bad"}})");
    std::vector<std::pair<std::string, const Value *>> packages = {
        {"mod.a/f1_2010", &p2010}, {"mod.a/f1_2011", &p2011}, {"mod.a/f1_2012", &p2012}};
    auto pick = content::drs_pick("cs_2010", packages, groups);
    std::printf("      %s\n", pick.line.c_str());
    check(pick.group && pick.group->label == never && pick.package == "mod.a/f1_2010" && pick.rule == "mod.a/never",
          "a box whose package names an applied rule runs its group");
    check(!content::drs_pick("cs_2011", packages, groups).group && !content::drs_pick("cs_2012", packages, groups).group &&
              !content::drs_pick("f1_2025", packages, groups).group,
          "no rule, a rule not added at boot, or a stock box: no group");
    Value p2010b = swofty::json::parse(R"({"entrant_list":"cs_2010"})");
    packages.push_back({"mod.b/f1_2010", &p2010b});
    check(!content::drs_pick("cs_2010", packages, groups).group, "the last package with the box's entrant list decides");

    Value w1959 = swofty::json::parse(R"({"entrant_list":"f1_1959","rules":{"tyre_warmers":false}})");
    Value w2009 = swofty::json::parse(R"({"entrant_list":"f1_2009","rules":{"tyre_warmers":null}})");
    Value w2010 = swofty::json::parse(R"({"entrant_list":"f1_2010","rules":{"tyre_warmers":"off"}})");
    std::vector<std::pair<std::string, const Value *>> boxes = {{"mod.a/f1_1959", &w1959}, {"mod.a/f1_2009", &w2009}, {"mod.a/f1_2010", &w2010}};
    auto warm = content::tyre_warmers_pick("f1_1959", boxes);
    std::printf("      %s\n", warm.line.c_str());
    check(warm.warmers == false && warm.package == "mod.a/f1_1959", "a box whose package sets rules.tyre_warmers runs that setting");
    check(!content::tyre_warmers_pick("f1_2009", boxes).warmers && !content::tyre_warmers_pick("f1_2010", boxes).warmers &&
              !content::tyre_warmers_pick("f1_2025", boxes).warmers,
          "null, a non-bool or a stock box: the mode keeps its own tyre warmer setting");
    Value w1959b = swofty::json::parse(R"({"entrant_list":"f1_1959","rules":{"tyre_warmers":true}})");
    boxes.push_back({"mod.b/f1_1959", &w1959b});
    check(content::tyre_warmers_pick("f1_1959", boxes).warmers == true, "the last package with the box's entrant list decides");
    Value c1950 = swofty::json::parse(R"({"entrant_list":"f1_1950","rules":{"mandatory_tyre_change":false}})");
    Value c2007 = swofty::json::parse(R"({"entrant_list":"f1_2007","rules":{"mandatory_tyre_change":true}})");
    Value c2008 = swofty::json::parse(R"({"entrant_list":"f1_2008","rules":{"mandatory_tyre_change":null}})");
    std::vector<std::pair<std::string, const Value *>> change = {{"mod.a/f1_1950", &c1950}, {"mod.a/f1_2007", &c2007}, {"mod.a/f1_2008", &c2008}};
    auto chg = content::tyre_change_pick("f1_1950", change);
    std::printf("      %s\n", chg.line.c_str());
    check(chg.required == false && content::tyre_change_pick("f1_2007", change).required == true,
          "a box whose package sets rules.mandatory_tyre_change runs that rule");
    check(!content::tyre_change_pick("f1_2008", change).required && !content::tyre_change_pick("f1_2025", change).required,
          "null or a stock box: the mode keeps its own tyre regulations");

    Value k1959 = swofty::json::parse(R"({"entrant_list":"f1_1959","rules":{"race_time_limit_s":10800,"weekend_mode":"swofty_f1_short_quali"}})");
    Value k2004 = swofty::json::parse(R"({"entrant_list":"f1_2004","rules":{"weekend_mode":"swofty_f1_one_shot"}})");
    Value k2009 = swofty::json::parse(R"({"entrant_list":"f1_2009","rules":{"race_time_limit_s":30,"weekend_mode":"Bad Name"}})");
    Value k2010 = swofty::json::parse(R"({"entrant_list":"f1_2010","rules":{"race_time_limit_s":"3h","weekend_mode":"swofty_f1"}})");
    std::vector<std::pair<std::string, const Value *>> weekends = {
        {"mod.a/f1_1959", &k1959}, {"mod.a/f1_2004", &k2004}, {"mod.a/f1_2009", &k2009}, {"mod.a/f1_2010", &k2010}};
    auto wk = content::weekend_pick("f1_1959", weekends);
    std::printf("      %s\n", wk.line.c_str());
    check(wk.time_limit_s == 10800u && wk.weekend_mode == "swofty_f1_short_quali" && wk.package == "mod.a/f1_1959",
          "a box whose package sets rules.race_time_limit_s and rules.weekend_mode runs both");
    auto wk04 = content::weekend_pick("f1_2004", weekends);
    check(!wk04.time_limit_s && wk04.weekend_mode == "swofty_f1_one_shot", "the weekend rule alone keeps the mode's time limit");
    auto wk09 = content::weekend_pick("f1_2009", weekends);
    auto wk10 = content::weekend_pick("f1_2010", weekends);
    check(!wk09.time_limit_s && wk09.weekend_mode.empty() && !wk10.time_limit_s && wk10.weekend_mode.empty(),
          "a limit under 60 s or not a whole number, a malformed mode name or the host itself: the mode keeps its own");
    auto wk25 = content::weekend_pick("f1_2025", weekends);
    check(!wk25.time_limit_s && wk25.weekend_mode.empty(), "a stock box: the mode keeps its own time limit and weekend");
    Value k1959b = swofty::json::parse(R"({"entrant_list":"f1_1959","rules":{"race_time_limit_s":7200}})");
    weekends.push_back({"mod.b/f1_1959", &k1959b});
    auto wkb = content::weekend_pick("f1_1959", weekends);
    check(wkb.time_limit_s == 7200u && wkb.weekend_mode.empty(), "the last package with the box's entrant list decides");

    // The helper row is made available at boot (the game parses only available rows into its setup mode store).
    std::map<std::string, std::string> avail = {
        {"game_setup.game_setup_modes.game_setup_mode[_metadata_=swofty_f1_short_quali].available", "False"},
        {"game_setup.game_setup_modes.game_setup_mode[_metadata_=swofty_f1_one_shot].available", "True"}};
    content::Read aread = [&](const std::string &p) -> std::optional<std::string> {
        auto it = avail.find(p);
        return it == avail.end() ? std::nullopt : std::optional<std::string>(it->second);
    };
    content::Plan wp;
    content::lower_weekend_mode("mod.a/f1_1959", k1959, "swofty_f1", aread, wp);
    check(wp.ops.size() == 1 && wp.ops[0]["op"].as_string() == "set" && wp.ops[0]["field"].as_string() == "available" &&
              wp.ops[0]["value"].as_string() == "True" &&
              wp.ops[0]["select"].as_string() == "game_setup.game_setup_modes.game_setup_mode[_metadata_=swofty_f1_short_quali]",
          "a box's weekend mode row that ships available=False is set available");
    content::Plan wp04, wp09, wp10, wpx;
    content::lower_weekend_mode("mod.a/f1_2004", k2004, "swofty_f1", aread, wp04);
    content::lower_weekend_mode("mod.a/f1_2009", k2009, "swofty_f1", aread, wp09);
    content::lower_weekend_mode("mod.a/f1_2010", k2010, "swofty_f1", aread, wp10);
    check(wp04.ops.size() == 0 && wp09.ops.size() == 0 && wp10.ops.size() == 0,
          "an already available row, a malformed name or the host itself: nothing written");
    Value kx = swofty::json::parse(R"({"entrant_list":"f1_1951","rules":{"weekend_mode":"f51_wk"}})");
    content::lower_weekend_mode("mod.a/f1_1951", kx, "swofty_f1", aread, wpx);
    check(wpx.ops.size() == 0 && wpx.lines.size() == 1 && wpx.lines[0].find("f51_wk") != std::string::npos,
          "a missing row writes nothing (the package still applies) and says so");

    Value s1959 = swofty::json::parse(
        R"({"entrant_list":"f1_1959","rules":{"safety_car":"off","formation_lap":false,"penalties":"corner_cutting_only","corner_cutting":"regular"}})");
    Value s1993 = swofty::json::parse(R"({"entrant_list":"f1_1993","rules":{"safety_car":"standard","formation_lap":true,"penalties":"on"}})");
    Value s2000 = swofty::json::parse(R"({"entrant_list":"f1_2000","rules":{"safety_car":null,"formation_lap":null}})");
    Value s2001 = swofty::json::parse(
        R"({"entrant_list":"f1_2001","rules":{"safety_car":"none","formation_lap":"off","penalties":"reduced","corner_cutting":"lax"}})");
    std::vector<std::pair<std::string, const Value *>> sc = {
        {"mod.a/f1_1959", &s1959}, {"mod.a/f1_1993", &s1993}, {"mod.a/f1_2000", &s2000}, {"mod.a/f1_2001", &s2001}};
    const size_t SC = size_t(content::box_option_index("safety_car")), FL = size_t(content::box_option_index("formation_lap")),
                 PE = size_t(content::box_option_index("penalties")), CC = size_t(content::box_option_index("corner_cutting"));
    check(content::box_option_index("refuelling") == -1 && SC != FL && PE != CC, "the four option rules have their own slots");
    auto sc59 = content::box_options_pick("f1_1959", sc);
    std::printf("      %s\n", sc59.line.c_str());
    check(sc59.settings[SC] == std::optional<std::string>("off") && sc59.package == "mod.a/f1_1959" &&
              content::box_options_pick("f1_1993", sc).settings[SC] == std::optional<std::string>("standard"),
          "a box whose package sets rules.safety_car runs that setting");
    check(sc59.settings[FL] == std::optional<std::string>("off") &&
              content::box_options_pick("f1_1993", sc).settings[FL] == std::optional<std::string>("on"),
          "rules.formation_lap false / true picks the option's off / on");
    check(sc59.settings[PE] == std::optional<std::string>("corner_cutting_only") && sc59.settings[CC] == std::optional<std::string>("regular") &&
              content::box_options_pick("f1_1993", sc).settings[PE] == std::optional<std::string>("on") &&
              !content::box_options_pick("f1_1993", sc).settings[CC],
          "rules.penalties and rules.corner_cutting pick their settings; a missing rule picks none");
    auto sc01 = content::box_options_pick("f1_2001", sc);
    check(!content::box_options_pick("f1_2000", sc).settings[SC] && !content::box_options_pick("f1_2000", sc).settings[FL] &&
              !sc01.settings[SC] && !sc01.settings[FL] && !sc01.settings[PE] && !sc01.settings[CC] &&
              !content::box_options_pick("f1_2025", sc).settings[SC],
          "null, an unknown setting, a wrong type or a stock box: the player's settings stay");
    std::vector<std::string> names = {"safety_car_standard", "Off", "reduced", "increased"};
    check(content::box_option_value(names, "safety_car", "off") == 1 && content::box_option_value(names, "safety_car", "standard") == 0 &&
              content::box_option_value({"low", "normal", "high", "none"}, "safety_car", "off") == 3 &&
              content::box_option_value({"low", "normal"}, "safety_car", "off") == -1,
          "a setting matches a value named after it, one ending in _<name>, or a known alias; none: -1");
    // the stock value lists (options_system.options.game, gamedatabases.2025.bin)
    check(content::box_option_value({"off", "reduced", "standard", "increased"}, "safety_car", "increased") == 3 &&
              content::box_option_value({"off", "on"}, "formation_lap", "off") == 0 &&
              content::box_option_value({"off", "on"}, "formation_lap", "on") == 1 &&
              content::box_option_value({"off", "corner_cutting_only", "eSports", "on"}, "penalties", "esports") == 2 &&
              content::box_option_value({"off", "corner_cutting_only", "eSports", "on"}, "penalties", "corner_cutting_only") == 1 &&
              content::box_option_value({"off", "corner_cutting_only", "eSports", "on"}, "penalties", "on") == 3 &&
              content::box_option_value({"regular", "strict"}, "corner_cutting", "strict") == 1,
          "every setting finds its value in the stock option's value list");
    check(std::string(content::BOX_OPTION_RULES[SC].options[0]) == "safety_car_frequency" &&
              std::string(content::BOX_OPTION_RULES[FL].options[0]) == "formation_lap" &&
              std::string(content::BOX_OPTION_RULES[PE].options[0]) == "rules_and_penalties_mode" &&
              std::string(content::BOX_OPTION_RULES[CC].options[0]) == "corner_cutting_mode",
          "each rule tries the stock database's option name first");
    std::vector<int> opt = {2};
    auto get = [&](const void *) { return opt[0]; };
    auto set = [&](const void *, int i) { opt[0] = i; };
    content::OptionForce f;
    int o1 = 0, o2 = 0;
    check(f.force(&o1, 0, get, set) && opt[0] == 0 && f.prior == 2, "a press sets the box's index and keeps the player's");
    check(f.force(&o1, 0, get, set) && f.prior == 2, "a second press of the same box keeps the player's index");
    check(f.put_back(&o1, get, set) == 1 && opt[0] == 2 && !f.active(), "another box gives the player's index back");
    f.force(&o1, 0, get, set);
    opt[0] = 3;   // the player picks another setting after the press
    check(f.put_back(&o1, get, set) == 0 && opt[0] == 3, "a setting the player picks after the press stays");
    f.force(&o1, 0, get, set);
    check(f.put_back(&o2, get, set) == 0 && opt[0] == 0 && !f.active(), "a changed option object is left as it is");

    Value g1990 = swofty::json::parse(R"({"entrant_list":"f1_1990","rules":{"grid":{"qualifying":"best_of","rows":null}}})");
    Value g2000 = swofty::json::parse(R"({"entrant_list":"f1_2000","rules":{"grid":{"rule_107":true}}})");
    Value g2005 = swofty::json::parse(R"({"entrant_list":"f1_2005","rules":{"grid":{"qualifying":"aggregate"}}})");
    Value g1955 = swofty::json::parse(R"({"entrant_list":"f1_1955","rules":{"grid":{"qualifying":"best_of","rows":[3,2]}}})");
    Value g2008 = swofty::json::parse(R"({"entrant_list":"f1_2008","rules":{"grid":null}})");
    std::vector<std::pair<std::string, const Value *>> grids = {
        {"mod.a/f1_1990", &g1990}, {"mod.a/f1_2000", &g2000}, {"mod.a/f1_2005", &g2005}, {"mod.a/f1_1955", &g1955}, {"mod.a/f1_2008", &g2008}};
    auto gp = content::grid_pick("f1_1955", grids);
    std::printf("      %s\n", gp.line.c_str());
    check(gp.rule && gp.rule->qualifying == runtime::box_grid::Scoring::best_of && gp.rule->rows == std::vector<int>({3, 2}) &&
              !gp.rule->rule_107,
          "rules.grid best_of with a 3-2 row pattern");
    auto g00 = content::grid_pick("f1_2000", grids);
    check(g00.rule && g00.rule->qualifying == runtime::box_grid::Scoring::best_lap && g00.rule->rule_107 && g00.rule->reranks(),
          "rule_107 alone keeps best_lap scoring and re-ranks");
    check(content::grid_pick("f1_2005", grids).rule->qualifying == runtime::box_grid::Scoring::aggregate &&
              content::grid_pick("f1_1990", grids).rule->rows.empty(),
          "aggregate scoring; rows null is the track's own grid");
    check(!content::grid_pick("f1_2008", grids).rule && !content::grid_pick("f1_2025", grids).rule,
          "grid null or a stock box: the game's own grid");

    Value q1985 = swofty::json::parse(R"({"entrant_list":"f1_1985","rules":{"qualifying_tyres":["softer"]}})");
    Value q1990 = swofty::json::parse(R"({"entrant_list":"f1_1990","rules":{"qualifying_tyres":["medium","softer","softer","wet"]}})");
    Value q1991 = swofty::json::parse(R"({"entrant_list":"f1_1991","rules":{"qualifying_tyres":null}})");
    Value q1992 = swofty::json::parse(R"({"entrant_list":"f1_1992","rules":{"qualifying_tyres":"softer"}})");
    std::vector<std::pair<std::string, const Value *>> qboxes = {
        {"mod.a/f1_1985", &q1985}, {"mod.a/f1_1990", &q1990}, {"mod.a/f1_1991", &q1991}, {"mod.a/f1_1992", &q1992}};
    auto q = content::quali_tyres_pick("f1_1985", qboxes);
    std::printf("      %s\n", q.line.c_str());
    check(q.mask == 1 && q.package == "mod.a/f1_1985", "rules.qualifying_tyres [softer] masks class option 0");
    check(content::quali_tyres_pick("f1_1990", qboxes).mask == 3, "names map to options 0..2, duplicates and wet are left out");
    check(!content::quali_tyres_pick("f1_1991", qboxes).mask && !content::quali_tyres_pick("f1_1992", qboxes).mask &&
              !content::quali_tyres_pick("f1_2025", qboxes).mask,
          "null, a non-array or a stock box: no qualifying tyre rule");
    Value q1985b = swofty::json::parse(R"({"entrant_list":"f1_1985","rules":{}})");
    qboxes.push_back({"mod.b/f1_1985", &q1985b});
    check(!content::quali_tyres_pick("f1_1985", qboxes).mask, "the last package with the box's entrant list decides (qualifying tyres)");
    check(content::quali_refused(15, 0, 1) && content::quali_refused(16, 0, 1) && content::quali_refused(17, 0, 1),
          "a masked option is refused in Race, Race2 and Race3");
    check(!content::quali_refused(5, 0, 1) && !content::quali_refused(9, 0, 1) && !content::quali_refused(1, 0, 1) &&
              !content::quali_refused(12, 0, 1) && !content::quali_refused(18, 0, 1),
          "a masked option stays usable in practice, qualifying and the sprint shootout");
    check(!content::quali_refused(15, 1, 1) && !content::quali_refused(15, 3, 7) && !content::quali_refused(15, 4, 0xff) &&
              !content::quali_refused(15, 0, 0) && !content::quali_refused(15, 0xffffffffu, 0xff),
          "other options, inter / wet, no mask and an unknown option are never refused");
    uint32_t pref[5] = {0, 1, 2, 3, 4}, out[5] = {};
    check(content::quali_filter_preference(15, 1, pref, out) == 1 && out[0] == content::QUALI_NO_SLOT && out[1] == 1 &&
              out[2] == 2 && out[3] == 3 && out[4] == 4,
          "in a race the set choice skips the qualifying option and keeps the order of the rest");
    uint32_t pref2[5] = {2, 0, 1, 3, 4};
    check(content::quali_filter_preference(5, 1, pref2, out) == 0 && !memcmp(out, pref2, sizeof out),
          "in qualifying the preference list is unchanged");
}

static void race_fuel_tests()
{
    using content::FuelCar;
    using content::FuelPlan;
    using content::FuelRules;
    Value p2008 = swofty::json::parse(R"({"entrant_list":"f1_2008","rules":{"refuelling":true,"race_fuel_kg":90,"refuel_stint_kg":60,"quali_fuel_kg":45,"race_start_fuel_from_quali":true}})");
    Value p1994 = swofty::json::parse(R"({"entrant_list":"f1_1994","rules":{"refuelling":true,"race_fuel_kg":80}})");
    Value p2017 = swofty::json::parse(R"({"entrant_list":"f1_2017","rules":{"refuelling":false,"race_fuel_kg":105}})");
    Value pbad = swofty::json::parse(R"({"entrant_list":"f1_bad","rules":{"refuelling":"yes","quali_fuel_kg":-3,"race_start_fuel_from_quali":1}})");
    std::vector<std::pair<std::string, const Value *>> boxes = {
        {"mod.a/f1_2008", &p2008}, {"mod.a/f1_1994", &p1994}, {"mod.a/f1_2017", &p2017}, {"mod.a/f1_bad", &pbad}};
    FuelRules r08 = content::fuel_rules_pick("f1_2008", boxes);
    std::printf("      %s\n", r08.line.c_str());
    check(r08.refuelling && r08.race_fuel_kg == 90.f && r08.refuel_stint_kg == 60.f && r08.quali_fuel_kg == 45.f && r08.start_from_quali,
          "a box's content.package gives its refuelling, race, stint and qualifying fuel and parc ferme rule");
    FuelRules r94 = content::fuel_rules_pick("f1_1994", boxes);
    std::printf("      %s\n", r94.line.c_str());
    check(r94.any() && !r94.start_from_quali && !r94.quali_fuel_kg, "refuelling without the 1.5 fields");
    check(!content::fuel_rules_pick("f1_2017", boxes).any() && !content::fuel_rules_pick("f1_bad", boxes).any() &&
              !content::fuel_rules_pick("f1_2025", boxes).any(),
          "refuelling off, malformed values or a stock box: no fuel rule (the game's own fuel)");

    auto car = [](float fuel, int lap, uint8_t stops) {
        FuelCar c;
        c.present = true;
        c.pit_stops_ok = true;
        c.pit_stops = stops;
        c.lap = lap;
        c.fuel = fuel;
        c.capacity = 110;
        return c;
    };
    // 1994: race start fuel, then a refill at each counted stop.
    FuelPlan plan;
    FuelCar cars[2] = {car(30, 1, 0), car(120, 1, 0)};
    cars[1].capacity = 70;
    auto w = plan.tick(r94, 15, 1, cars, 2);
    check(w.size() == 2 && w[0].car == 0 && w[0].kg == 80.f && w[1].kg == 70.f, "race start: race_fuel_kg, capped at the tank");
    cars[0] = car(80, 1, 0);
    check(plan.tick(r94, 15, 1, cars, 1).empty(), "the start fuel held: nothing written again");
    cars[0] = car(100, 1, 0);
    w = plan.tick(r94, 15, 1, cars, 1);
    check(w.size() == 1 && w[0].kg == 80.f, "the start sequence loaded its own fuel: the start fuel is written again");
    cars[0] = car(79, 1, 0);
    check(plan.tick(r94, 15, 1, cars, 1).empty(), "the car burns: the start fuel is done");
    cars[0] = car(100, 1, 0);
    check(plan.tick(r94, 15, 1, cars, 1).empty(), "after the start, a higher fuel is left alone");
    cars[0] = car(12, 20, 1);
    w = plan.tick(r94, 15, 1, cars, 1);
    check(w.size() == 1 && w[0].kg == 80.f && std::string(w[0].why) == "pit stop refuel", "a counted pit stop refuels to the stint (race_fuel_kg)");
    check(plan.tick(r94, 15, 1, cars, 1).empty(), "one refuel per stop");
    plan.failed(0);
    w = plan.tick(r94, 15, 1, cars, 1);
    check(w.size() == 1 && w[0].kg == 80.f, "a refuel the core could not write is tried again");
    cars[0] = car(95, 30, 2);
    check(plan.tick(r94, 15, 1, cars, 1).empty(), "a stop never drains a car that holds more than the stint");
    cars[0] = car(5, 1, 0);
    cars[0].pit_stops_ok = false;
    w = plan.tick(r94, 15, 2, cars, 1);
    check(w.size() == 1 && w[0].kg == 80.f, "a restarted race (new generation) writes the start fuel again");
    check(plan.tick(r94, 1, 3, cars, 1).empty(), "practice: nothing");

    // 2008: qualifying fuel, parc ferme race start, stint refuels.
    FuelPlan pf;
    FuelCar q[2] = {car(100, 0, 0), car(100, 0, 0)};
    w = pf.tick(r08, 8, 1, q, 2);
    check(w.size() == 2 && w[0].kg == 45.f && w[1].kg == 45.f, "qualifying starts on quali_fuel_kg");
    q[0] = car(45, 0, 0);
    q[1] = car(45, 0, 0);
    pf.tick(r08, 8, 1, q, 2);
    q[0] = car(31.5f, 3, 0);
    q[1].present = false;
    pf.tick(r08, 8, 1, q, 2);
    float left = 0;
    check(pf.quali_known(0, left) && left == 31.5f, "the fuel left at the end of qualifying is kept per car");
    FuelCar race[3] = {car(100, 1, 0), car(100, 1, 0), car(100, 1, 0)};
    w = pf.tick(r08, 15, 2, race, 3);
    check(w.size() == 3 && w[0].kg == 31.5f && w[1].kg == 45.f && w[2].kg == 45.f,
          "parc ferme: the race starts on the fuel left after qualifying, quali_fuel_kg for a car without it");
    std::printf("      car 0: %s; car 2: %s\n", w[0].why, w[2].why);
    race[0] = car(10, 25, 1);
    w = pf.tick(r08, 15, 2, race, 1);
    check(w.size() == 1 && w[0].kg == 60.f, "a stop refuels to refuel_stint_kg");
    pf.reset();
    check(!pf.quali_known(0, left), "a box change forgets the qualifying fuel");

    FuelRules none;
    check(FuelPlan{}.tick(none, 15, 1, race, 3).empty(), "no rule: nothing written");

    // 2005: one tyre set for qualifying and race (race_tyre_change false).
    Value p2005 = swofty::json::parse(R"({"entrant_list":"f1_2005","rules":{"refuelling":true,"race_fuel_kg":75,"race_tyre_change":false}})");
    Value pbadt = swofty::json::parse(R"({"entrant_list":"f1_badt","rules":{"race_tyre_change":"no"}})");
    std::vector<std::pair<std::string, const Value *>> tb = {{"mod.a/f1_2005", &p2005}, {"mod.a/f1_badt", &pbadt}, {"mod.a/f1_1994", &p1994}};
    FuelRules r05 = content::fuel_rules_pick("f1_2005", tb);
    std::printf("      %s\n", r05.line.c_str());
    check(r05.tyres_kept() && r05.any(), "race_tyre_change false: the box keeps its tyres at race stops (and refuels)");
    check(!content::fuel_rules_pick("f1_badt", tb).tyres_kept() && !content::fuel_rules_pick("f1_1994", tb).tyres_kept(),
          "race_tyre_change missing or malformed: the game's tyre change");
    Value ponly = swofty::json::parse(R"({"entrant_list":"f1_t","rules":{"race_tyre_change":false}})");
    std::vector<std::pair<std::string, const Value *>> to = {{"mod.a/f1_t", &ponly}};
    FuelRules rt = content::fuel_rules_pick("f1_t", to);
    check(rt.tyres_kept() && !rt.any() && rt.line.find("keep the tyres") != std::string::npos,
          "a tyre rule alone is logged and handed over without a fuel rule");
    using content::TyreChange;
    check(content::race_tyre_change_pick(true, 15, true, 0) == TyreChange::kept, "race stop, no damage flag: tyres kept");
    check(content::race_tyre_change_pick(true, 16, true, 0x4) == TyreChange::damage, "race stop with a damage flag: the game's change");
    check(content::race_tyre_change_pick(true, 15, false, 0) == TyreChange::game, "the garage actor's call (flag 0): the game's");
    check(content::race_tyre_change_pick(true, 8, true, 0) == TyreChange::game, "qualifying: the game's change");
    check(content::race_tyre_change_pick(false, 15, true, 0) == TyreChange::game, "no rule: the game's change");
    check(content::race_tyre_change_pick(true, 15, true, 0x20) == TyreChange::kept, "flags outside the damage mask: kept");
}

static void points_tests()
{
    std::printf("== session.points\n");
    Value rec = swofty::json::parse(R"({"points":[8,6,4,3,2],"fastest_lap":1})");
    content::Plan plan;
    content::lower_session_points("mod.a/f1_1950", rec, plan);
    for (auto &l : plan.lines)
        std::printf("      %s\n", l.c_str());
    std::string label = content::points_label("mod.a/f1_1950");
    std::string row = std::string(content::POINTS_PRESETS) + "[_metadata_=" + label + "]";
    bool full = false, levels = true;
    size_t sets = 0;
    for (std::size_t i = 0; i < plan.ops.size(); i++) {
        const Value &op = plan.ops[i];
        if (op["op"].as_string() == "add-row") {
            full = op["table"].as_string() == std::string(content::POINTS_TABLE) &&
                   op["set"]["points"].as_string() == "8,6,4,3,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0" &&
                   op["set"]["fastest_lap_bonus"].as_string() == "1" && op["set"]["fastest_lap_rule"].as_string() == "BestOfTop10";
        } else if (op["op"].as_string() == "set") {
            sets++;
            levels = levels && op["select"].as_string().rfind(row + ".reduced_points_data[_metadata_=", 0) == 0;
        }
    }
    check(plan.refused == 0 && full && sets == 9 && levels,
          "a points system becomes one standing preset row (26 places, BestOfTop10) and its three reduced levels");

    Value none = swofty::json::parse(R"({"points":[9,6,4,3,2,1],"fastest_lap":null,"reduced":[[4,3,2,1,1]]})");
    auto s = content::points_system("mod.a/f1_1990", none, nullptr);
    check(s && s->rule == 0 && s->bonus == 0 && s->reduced.size() == 3 && s->reduced[0][0] == 4 && s->reduced[1][0] == 9 &&
              s->full.size() == content::POINTS_PLACES,
          "no fastest lap: rule None; a missing reduced level gives the full points");
    for (const char *bad : {R"({"points":"25,18"})", R"({"points":[]})", R"({"points":[1,-1]})",
                            R"({"points":[1],"fastest_lap":"1"})", R"({"points":[1],"reduced":[[1],[1],[1],[1]]})"}) {
        content::Plan p;
        content::lower_session_points("mod.a/bad", swofty::json::parse(bad), p);
        check(p.refused == 1 && p.ops.size() == 0, std::string("refused, nothing applied: ") + bad);
    }

    std::map<std::string, content::PointsSystem> systems = {{"mod.a/f1_1950", *content::points_system("mod.a/f1_1950", rec, nullptr)}};
    Value b1950 = swofty::json::parse(R"({"entrant_list":"f1_1950","rules":{"points":"mod.a/f1_1950"}})");
    Value b1951 = swofty::json::parse(R"({"entrant_list":"f1_1951","rules":{"points":"mod.a/missing"}})");
    Value b1952 = swofty::json::parse(R"({"entrant_list":"f1_1952","rules":{}})");
    std::vector<std::pair<std::string, const Value *>> boxes = {{"mod.a/f1_1950", &b1950}, {"mod.a/f1_1951", &b1951}, {"mod.a/f1_1952", &b1952}};
    auto pick = content::points_pick("f1_1950", boxes, systems);
    std::printf("      %s\n", pick.line.c_str());
    check(pick.system && pick.system->label == label && pick.package == "mod.a/f1_1950",
          "a box whose package names an applied points system runs it");
    check(!content::points_pick("f1_1951", boxes, systems).system && !content::points_pick("f1_1952", boxes, systems).system &&
              !content::points_pick("f1_2025", boxes, systems).system,
          "a system not added at boot, no rule, or a stock box: the mode keeps its own points");
}

static void stock_career_rows(Db &db)
{
    auto node = [&](const std::string &c, int i, const char *label, const char *type, const char *dept, const char *links) {
        std::string r = c + ".upgrade_" + std::to_string(i);
        db.put(r + "._metadata_", 2, label);
        db.put(r + ".upgrade_name", 2, std::string("n_") + label);
        db.put(r + ".upgrade_type", 3, type);
        db.put(r + ".upgrade_size", 3, "Minor");
        db.put(r + ".upgrade_department", 3, dept);
        db.put(r + ".cost", 0, "500");
        db.put(r + ".cost_to_develop", 0, "1000");
        db.put(r + ".weighting", 1, "1.0");
        db.put(r + ".unlock_at_facility_level", 0, "0");
        db.put(r + ".supplier_delivery_date_index", 0, "-1");
        db.put(r + ".links", 13, links);
        db.put(r + ".fitting_times.num_days_to_research", 0, "20");
        db.put(r + ".fitting_times.num_days_to_develop", 0, "25");
        db.put(r + ".grid_coordinates.x", 1, "0");
        db.put(r + ".grid_coordinates.y", 1, std::to_string(-5 + i));
    };
    for (const char *group : {"2026", "myteam_2025"}) {
        std::string t = std::string("upgrades.") + group + ".trees.mercedes.";
        db.put(t + "aerodynamics._upgrade_instancecount_", 0, "2");
        node(t + "aerodynamics", 0, "drag_minor_1", "Drag", "Aerodynamics", "drag_major_1");
        node(t + "aerodynamics", 1, "drag_major_1", "Drag", "Aerodynamics", "");
        db.put(t + "chassis._upgrade_instancecount_", 0, "1");
        node(t + "chassis", 0, "brakes_minor_1", "Brakes", "Chassis", "");
        db.put(t + "fake._upgrade_instancecount_", 0, "1");
        node(t + "fake", 0, "default", "Drag", "Fake", "drag_minor_1,brakes_minor_1");
    }
    db.put("driver_market.driver_market._DataGroup_instancecount_", 0, "2");
    for (int gi = 0; gi < 2; gi++) {
    std::string g = "driver_market.driver_market.DataGroup_" + std::to_string(gi);
    db.put(g + "._metadata_", 2, gi ? "2025" : "2026");
    db.put(g + ".driver_retirements._DriverRetirementSchedule_instancecount_", 0, "2");
    const char *retiring[] = {"fernando_alonso", "lewis_hamilton"};
    for (int s = 0; s < 2; s++) {
        std::string sc = g + ".driver_retirements.DriverRetirementSchedule_" + std::to_string(s);
        db.put(sc + "._metadata_", 2, "DriverRetirementSchedule_" + std::to_string(s));
        db.put(sc + ".statId", 0, std::to_string(s));
        db.put(sc + "._DriverRetirement_instancecount_", 0, "2");
        for (int i = 0; i < 2; i++) {
            std::string r = sc + ".DriverRetirement_" + std::to_string(i);
            db.put(r + "._metadata_", 2, retiring[i]);
            db.put(r + ".driver", 4, retiring[i]);
            db.put(r + "._date_time_instancecount_", 0, "2");
            const char *labels[] = {"Retirement", "Announcement"};
            for (int d = 0; d < 2; d++) {
                std::string dt = r + ".date_time_" + std::to_string(d);
                db.put(dt + "._metadata_", 2, labels[d]);
                db.put(dt + ".year", 0, "2026");
                db.put(dt + ".month", 3, d ? "September" : "December");
                db.put(dt + ".day", 0, "14");
                db.put(dt + ".hour", 0, "0");
                db.put(dt + ".minute", 0, "0");
                db.put(dt + ".second", 0, "0");
                db.put(dt + ".daylight_saving", 5, "False");
            }
        }
    }
    db.put(g + ".driver_firings._DriverFiredSchedule_instancecount_", 0, "1");
    std::string fs = g + ".driver_firings.DriverFiredSchedule_0";
    db.put(fs + "._metadata_", 2, "DriverFiredSchedule");
    db.put(fs + ".statId", 0, "0");
    db.put(fs + "._DriverFired_instancecount_", 0, "3");
    for (int i = 0; i < 3; i++) {
        std::string r = fs + ".DriverFired_" + std::to_string(i);
        db.put(r + "._metadata_", 2, "Season_1_DriverFired_" + std::to_string(i));
        db.put(r + ".driver", 4, "lance_stroll");
        db.put(r + ".season_index", 0, std::to_string(i));
    }
    std::string tr = g + ".team_driver_rating_range_requirements";
    db.put(tr + "._TeamDriverRatingRangeRequirement_instancecount_", 0, "1");
    db.put(tr + ".TeamDriverRatingRangeRequirement_0._metadata_", 2, "mclaren");
    db.put(tr + ".TeamDriverRatingRangeRequirement_0.team", 4, "mclaren_26");
    db.put(tr + ".TeamDriverRatingRangeRequirement_0.rating_range", 0, "99");
    }
    db.put("entrants.drivers._driver_instancecount_", 0, "4");
    const char *drivers[] = {"fernando_alonso", "lewis_hamilton", "lance_stroll", "max_verstappen"};
    for (int i = 0; i < 4; i++)
        db.put("entrants.drivers.driver_" + std::to_string(i) + "._metadata_", 2, drivers[i]);
    db.put("entrants.teams._team_instancecount_", 0, "2");
    db.put("entrants.teams.team_0._metadata_", 2, "mclaren_26");
    db.put("entrants.teams.team_1._metadata_", 2, "ferrari_26");
}

static void career_registry_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.a"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"career.rnd.tree", "career.market"}) {
        std::string schema = slurp(regs + "/" + name + "/1.0.json");
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    int r1 = registry::load_data_file("mod.a", "registry/career.json", R"j({
      "career.rnd.tree": {
        "mercedes": { "token": "mercedes", "nodes": [
          { "label": "sw_wing_1", "department": "aerodynamics", "type": "Drag", "size": "major", "cost": 650, "weighting": 2.5,
            "x": 1, "y": -5, "after": [ "default", "drag_minor_1" ] },
          { "label": "drag_major_1", "department": "aerodynamics", "cost": 900, "links": [ "sw_wing_1" ] },
          { "label": "brakes_minor_1", "department": "aerodynamics", "cost": 1 },
          { "label": "sw_hover_1", "department": "chassis", "type": "Hover" } ] },
        "ferrari": { "token": "ferrari", "nodes": [ { "label": "sw_x_1" } ] },
        "mercedes_26": { "data_group": "2026", "token": "mercedes", "nodes": [ { "label": "drag_minor_1", "cost": 333 } ] } },
      "career.market": {
        "history": { "data_group": "2026",
          "retirements": [ { "driver": "game/fernando_alonso", "retire": "2027-11-30", "announce": "2027-07-04" },
                           { "driver": "game/lewis_hamilton", "retire": "2028-12-14", "announce": "2028-09-23" },
                           { "driver": "game/max_verstappen", "retire": "2030-12-01", "announce": "2030-08-01" } ],
          "firings": [ { "driver": "game/lance_stroll", "season_index": 1 } ],
          "team_ratings": [ { "team": "game/mclaren_26", "rating": 97 }, { "team": "game/ferrari_26", "rating": 90 },
                            { "team": "game/nobody", "rating": 80 } ] },
        "lost": { "data_group": "1990", "team_ratings": [ { "team": "game/mclaren_26", "rating": 50 } ] },
        "everywhere": { "team_ratings": [ { "team": "game/mclaren_26", "rating": 97 } ] } } })j");
    st.end_register();
    for (auto &r : st.refusals())
        std::printf("      refused: %s %s %s %s: %s\n", r.addon.c_str(), r.registry.c_str(), r.id.c_str(), r.reason.c_str(), r.message.c_str());
    check(r1 == SWOFTY_ASSET_OK && st.list("career.rnd.tree", "").size() == 3 && st.list("career.market", "").size() == 3,
          "the career.rnd.tree and career.market records register");
    Db db;
    stock_career_rows(db);
    content::Expansion x(db.base());
    content::StatIds stat = [](const std::string &, const std::string &, int64_t pin) -> int64_t { return pin; };
    content::expand(st, x, stat);
    for (const char *reg : {"career.rnd.tree", "career.market"})
        for (auto &l : x.lines[reg])
            std::printf("      %s\n", l.c_str());
    std::string a = "upgrades.2026.trees.mercedes.aerodynamics.";
    check(x.records["career.rnd.tree"] == 2 && x.refused["career.rnd.tree"] == 1,
          "the mercedes trees apply (two of their nodes refused); the ferrari record, with no such tree in any group, does not");
    std::string m = "upgrades.myteam_2025.trees.mercedes.aerodynamics.";
    check(written(x.engine, m + "_upgrade_instancecount_") == "3" && written(x.engine, m + "upgrade_2._metadata_") == "sw_wing_1" &&
              written(x.engine, m + "upgrade_1.cost") == "900" &&
              written(x.engine, "upgrades.myteam_2025.trees.mercedes.fake.upgrade_0.links") == "drag_minor_1,brakes_minor_1,sw_wing_1",
          "a record without a data_group edits the tree in every upgrades group that has it");
    check(written(x.engine, a + "upgrade_0.cost") == "333" && !written(x.engine, m + "upgrade_0.cost"),
          "a record that names its data_group edits only that group's tree");
    check(written(x.engine, a + "_upgrade_instancecount_") == "3" && written(x.engine, a + "upgrade_2._metadata_") == "sw_wing_1",
          "a new node is a row added to its department");
    check(written(x.engine, a + "upgrade_2.upgrade_size") == "Major" && written(x.engine, a + "upgrade_2.upgrade_type") == "Drag" &&
              written(x.engine, a + "upgrade_2.cost") == "650" && written(x.engine, a + "upgrade_2.weighting") == "2.5" &&
              written(x.engine, a + "upgrade_2.grid_coordinates.x") == "1" && written(x.engine, a + "upgrade_2.links") == "" &&
              written(x.engine, a + "upgrade_2.upgrade_department") == "Aerodynamics" &&
              written(x.engine, a + "upgrade_2.fitting_times.num_days_to_research") == "20",
          "the new node takes the given fields over its source row's, no links of its own, and its department");
    check(written(x.engine, "upgrades.2026.trees.mercedes.fake.upgrade_0.links") == "drag_minor_1,brakes_minor_1,sw_wing_1" &&
              written(x.engine, a + "upgrade_0.links") == "drag_major_1,sw_wing_1",
          "the after nodes, the hub included, link to the new node");
    check(written(x.engine, a + "upgrade_1.cost") == "900" && written(x.engine, a + "upgrade_1.links") == "sw_wing_1" &&
              !written(x.engine, a + "upgrade_1.upgrade_name"),
          "a stock label edits that node's given fields only");
    check(!written(x.engine, "upgrades.2026.trees.mercedes.chassis.upgrade_0.cost") &&
              !written(x.engine, "upgrades.2026.trees.mercedes.chassis._upgrade_instancecount_"),
          "a node named under the wrong department and a type the trees don't use are refused");
    std::string g = "driver_market.driver_market.DataGroup_0.";
    std::string s0 = g + "driver_retirements.DriverRetirementSchedule_0.", s1 = g + "driver_retirements.DriverRetirementSchedule_1.";
    check(written(x.engine, s0 + "_DriverRetirement_instancecount_") == "3" && written(x.engine, s1 + "_DriverRetirement_instancecount_") == "3" &&
              written(x.engine, s0 + "DriverRetirement_2.driver") == "max_verstappen" &&
              written(x.engine, s1 + "DriverRetirement_2.date_time_0.year") == "2030" &&
              written(x.engine, s1 + "DriverRetirement_2.date_time_1.month") == "August",
          "every retirement schedule takes the list, a third driver's row copied with its dates");
    check(written(x.engine, s0 + "DriverRetirement_0.date_time_0.month") == "November" &&
              written(x.engine, s0 + "DriverRetirement_0.date_time_0.day") == "30" &&
              written(x.engine, s0 + "DriverRetirement_0.date_time_1.year") == "2027",
          "a stock retirement row is rewritten in place, Retirement and Announcement by their labels");
    std::string f0 = g + "driver_firings.DriverFiredSchedule_0.";
    check(written(x.engine, f0 + "_DriverFired_instancecount_") == "1" && written(x.engine, f0 + "DriverFired_0.season_index") == "1",
          "a shorter firing list cuts the schedule by its instance count");
    std::string tr = g + "team_driver_rating_range_requirements.";
    check(written(x.engine, tr + "TeamDriverRatingRangeRequirement_0.rating_range") == "97" &&
              written(x.engine, tr + "_TeamDriverRatingRangeRequirement_instancecount_") == "2" &&
              written(x.engine, tr + "TeamDriverRatingRangeRequirement_1.team") == "ferrari_26" &&
              written(x.engine, tr + "TeamDriverRatingRangeRequirement_1.rating_range") == "90",
          "a team's rating is set on its row, or a row is added for a team without one; a team with no entrants.teams row is left out");
    check(x.records["career.market"] == 2 && x.refused["career.market"] == 1, "a market group the database doesn't have is refused");
    std::string g25 = "driver_market.driver_market.DataGroup_1.";
    check(written(x.engine, g25 + "team_driver_rating_range_requirements.TeamDriverRatingRangeRequirement_0.rating_range") == "97" &&
              !written(x.engine, g25 + "driver_retirements.DriverRetirementSchedule_0._DriverRetirement_instancecount_") &&
              !written(x.engine, g25 + "driver_firings.DriverFiredSchedule_0._DriverFired_instancecount_"),
          "a record without a data_group writes every market group; one that names 2026 leaves 2025 alone");
}

static void older_rows(Db &db)
{
    auto row = [&](const std::string &prefix, std::initializer_list<std::pair<const char *, std::string>> fields) {
        for (auto &[f, v] : fields)
            db.put(prefix + "." + f, std::string_view(f) == "_metadata_" ? 4 : 2, v);
    };
    db.put("entrants.vehicles._vehicle_instancecount_", 0, "2");
    row("entrants.vehicles.vehicle_1", {{"_metadata_", "bdx_s5_car_mclaren_3"}, {"performance_tier", "0"}, {"tyre_wear_scale", "1"}});
    db.put("entrants.vehicles.vehicle_1.stat_id", 0, "520");
    db.put("entrants.entrant_lists._entrant_list_instancecount_", 0, "2");
    row("entrants.entrant_lists.entrant_list_1", {{"_metadata_", "bdx_s5_2012"}, {"display_name", "x"}, {"leaderboard", "f1"}});
    db.put("entrants.entrant_lists.entrant_list_1.stat_id", 0, "106");
    db.put("game_setup.scenarios._scenario_instancecount_", 0, "2");
    row("game_setup.scenarios.scenario_1", {{"_metadata_", "bdx_s5_2012"}, {"entrant_list", "bdx_s5_2012"}, {"game_setup_mode", "f1"}});
    db.put("game_setup.scenarios.scenario_1.statId", 0, "16");
}

static void shared_switch_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"swofty.classic-seasons-2017", "swofty.swofty-seasons-2017"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + (std::string_view(name) == "vehicle" ? "/1.1.json" : "/1.0.json"));
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    int r1 = registry::load_data_file("swofty.classic-seasons-2017", "registry/content.json", R"j({
      "engine_sound": { "honda": { "bundle": "sounds/cs17_honda.bdl", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "toro_rosso": { "label": "cs17_toro_rosso", "donor": "game/mercedes_2025", "stat_id": 526,
                                   "engine_sound": "swofty.classic-seasons-2017/honda",
                                   "audio_switch": "f1_M_mercedes", "audio_from": "f1_m_mercedes" } } })j");
    int r2 = registry::load_data_file("swofty.swofty-seasons-2017", "registry/content.json", R"j({
      "engine_sound": { "renault": { "bundle": "sounds/sw17_renault.bdl", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "lotus": { "label": "sw17_lotus", "donor": "game/mercedes_2025", "stat_id": 919,
                              "engine_sound": "swofty.swofty-seasons-2017/renault",
                              "audio_switch": "f1_M_mercedes", "audio_from": "f1_m_mercedes" } },
      "content.package": { "sw_2017": {
        "title": { "literal": "SWOFTY 2017" }, "entrant_list": "sw_f1_2017", "season": 2017,
        "teams": [ { "key": "lotus", "vehicle": "swofty.swofty-seasons-2017/lotus",
                     "drivers": [ { "driver": "game/max_verstappen", "number": 33, "look": null } ] } ],
        "database": { "entrant_list_from": "f1_2025", "standings_from": "f1_teams_2024", "content_sets": ["2025"] } } } })j");
    st.end_register();
    check(r1 == SWOFTY_ASSET_OK && r2 == SWOFTY_ASSET_OK, "two packs whose cars name one sound switch register");

    Db db;
    stock_package_rows(db);
    apply::ids::Store ids;
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    for (auto &scan : content::scan_all_pools(text))
        ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
    for (auto &p : content::pins(st))
        ids.pin(p.registry, p.id, p.stat_id, "test");
    content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
        return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
    };
    content::Expansion x(db.base());
    content::expand(st, x, stat);
    for (auto &l : x.lines["vehicle"])
        std::printf("      %s\n", l.c_str());
    auto has_label = [&](const std::string &table, const std::string &row, const std::string &label) {
        for (int i = 0; i < 8; i++)
            if (written(x.engine, table + "." + row + "_" + std::to_string(i) + "._metadata_") == label)
                return table + "." + row + "_" + std::to_string(i);
        return std::string();
    };
    check(x.records["vehicle"] == 2 && x.refused["vehicle"] == 0 && x.left_out.empty(),
          "both cars apply and nothing is left out");
    std::string first = has_label("entrants.vehicles", "vehicle", "cs17_toro_rosso");
    std::string second = has_label("entrants.vehicles", "vehicle", "sw17_lotus");
    check(!first.empty() && !second.empty() && written(x.engine, second + ".stat_id") == "919",
          "the later car keeps its row, so every entrant list that names sw17_lotus finds it");
    check(written(x.engine, "audio.audio_car_data.f1_M_mercedes.bundle_name") == "cs17_honda.bdl",
          "the switch's sound rows stay the earlier car's");
    bool said = false;
    for (auto &l : x.lines["vehicle"])
        said |= l.find("vehicle swofty.swofty-seasons-2017/lotus: sound switch f1_M_mercedes is "
                       "swofty.classic-seasons-2017/toro_rosso's") == 0;
    check(said, "the log names the car that has the switch");
    check(x.records["content.package"] == 1 && !has_label("entrants.entrant_lists", "entrant_list", "sw_f1_2017").empty(),
          "the later car's own season list is not left out");
}

static void id_clash_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"swofty.classic-seasons-2017"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + "/1.0.json");
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    int r = registry::load_data_file("swofty.classic-seasons-2017", "registry/content.json", R"j({
      "engine_sound": { "mercedes": { "bundle": "sounds/cs17_mercedes.bdl", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "mercedes": { "label": "cs17_mercedes", "donor": "game/mercedes_2025", "stat_id": 520,
                                 "engine_sound": "swofty.classic-seasons-2017/mercedes",
                                 "audio_switch": "f1_M_mercedes", "audio_from": "f1_m_mercedes" },
                   "redbull": { "label": "cs17_redbull", "donor": "game/mercedes_2025", "stat_id": 521 } },
      "content.package": { "f1_2017": {
        "title": "lng_cs_f1_2017", "entrant_list": "cs_f1_2017", "season": 2017, "stat_id": 106,
        "teams": [ { "key": "mercedes", "vehicle": null,
                     "drivers": [ { "driver": "game/lewis_hamilton", "number": 44, "look": null } ] } ],
        "database": { "entrant_list": "cs_f1_2017", "scenario_stat_id": 16, "entrant_prefix": "cs2017-",
                      "standings": "cs_f1_teams_2017", "content_sets": ["2025"],
                      "looks": { "mercedes/lewis_hamilton": "2017-cs_mercedes_lewis_hamilton" } } } } })j");
    st.end_register();
    check(r == SWOFTY_ASSET_OK, "the CS-style records register");

    struct Run {
        std::vector<std::string> lines;
        std::vector<std::pair<std::string, std::string>> writes;
    };
    auto run = [&](Db &db, apply::ids::Store &ids, bool settle) {
        Run out;
        auto text = [&](const std::string &path) -> std::optional<std::string> {
            auto r = db.base()(content::gvdb::fnv64(path));
            return r ? std::optional<std::string>(r->text) : std::nullopt;
        };
        std::map<std::string, content::PoolScan> scans;
        for (auto &scan : content::scan_all_pools(text)) {
            ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
            scans[scan.registry] = scan;
        }
        if (settle)
            out.lines = content::settle_ids(ids, st, scans, "test");
        else
            for (auto &p : content::pins(st))
                ids.pin(p.registry, p.id, p.stat_id, "test");
        content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
            return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
        };
        content::Expansion x(db.base());
        content::expand(st, x, stat);
        for (auto &w : x.engine.writes())
            out.writes.push_back({w.path, w.text});
        return out;
    };
    auto stat_of = [](const Run &r, const std::string &table, const std::string &row, const std::string &label,
                      const char *field) {
        std::string at;
        for (auto &[path, text] : r.writes)
            if (text == label && path.rfind(table + "." + row + "_", 0) == 0 && path.size() > 11 &&
                path.substr(path.size() - 11) == "._metadata_")
                at = path.substr(0, path.size() - 10);
        for (auto &[path, text] : r.writes)
            if (!at.empty() && path == at + field)
                return text;
        return std::string("none");
    };

    Db stock;
    stock_package_rows(stock);
    apply::ids::Store old_ids, new_ids;
    Run before = run(stock, old_ids, false), after = run(stock, new_ids, true);
    check(after.lines.size() == 4 && after.lines[0].find("[registry] stat ids:") == 0,
          "no clash: only the usual pin lines (" + std::to_string(after.lines.size()) + ")");
    check(before.writes == after.writes && !after.writes.empty(), "no clash: the plan is byte-identical to before");
    check(old_ids.dump() == new_ids.dump(), "no clash: ids.json is identical");

    Db older;
    stock_package_rows(older);
    older_rows(older);
    apply::ids::Store ids;
    Run moved = run(older, ids, true);
    for (auto &l : moved.lines)
        std::printf("      %s\n", l.c_str());
    auto has = [&](const std::string &line) {
        return std::find(moved.lines.begin(), moved.lines.end(), line) != moved.lines.end();
    };
    check(has("[registry] ids: swofty.classic-seasons-2017/mercedes pinned vehicle stat id 520 is held by "
              "bdx_s5_car_mclaren_3 from the database; uses 522"),
          "a pin on a 1.5 row's vehicle id moves past the other pins, with the log line");
    check(has("[registry] ids: swofty.classic-seasons-2017/f1_2017 pinned content.package stat id 106 is held by "
              "bdx_s5_2012 from the database; uses 107") &&
              has("[registry] ids: swofty.classic-seasons-2017/f1_2017 pinned content.package.scenario stat id 16 is "
                  "held by bdx_s5_2012 from the database; uses 17"),
          "so do the entrant list and scenario ids");
    check(stat_of(moved, "entrants.vehicles", "vehicle", "cs17_mercedes", "stat_id") == "522" &&
              stat_of(moved, "entrants.vehicles", "vehicle", "cs17_redbull", "stat_id") == "521",
          "the moved car's row carries its new id; the car with no clash keeps its pin");
    check(stat_of(moved, "entrants.entrant_lists", "entrant_list", "cs_f1_2017", "stat_id") == "107" &&
              stat_of(moved, "game_setup.scenarios", "scenario", "cs_f1_2017", "statId") == "17",
          "and so do the list and scenario rows");
    bool untouched = true;
    for (auto &[path, text] : moved.writes)
        untouched &= path.rfind("entrants.vehicles.vehicle_1.", 0) != 0 && path.rfind("entrants.vehicles.vehicle_0.", 0) != 0;
    check(untouched, "the stock and 1.5 rows are not written");
    Run again = run(older, ids, true);
    check(again.writes == moved.writes, "the next boot moves it to the same id");
}

static void stock_only_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.a"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"team.identity", "team.constructor", "team.pit_times", "team.logo"}) {
        std::string schema = slurp(regs + "/" + name + (std::string(name) == "team.identity" ? "/1.1.json" : "/1.0.json"));
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    st.end_register();
    Db db;
    stock_team_rows(db);
    content::Read stock_read = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    check(content::in_load_order(st, "team.identity").empty() && content::in_load_order(st, "team.pit_times").empty(),
          "a profile with no team records has none in load order");
    for (auto &c : content::CONSTRUCTOR_SEEDS)
        st.add_base("team.constructor", c.key,
                    Value::object({{"name", Value(std::string(c.name))}, {"country", Value(std::string(c.country))}, {"team", Value()}}));
    for (auto &team : content::stock_teams(stock_read))
        st.add_base("team.identity", team.label, team.record);
    st.stock_loaded("team.identity", true);
    check(st.get("team.identity", "game/mercedes").status == registry::Status::ok &&
              st.get("team.identity", "game/tororosso").status == registry::Status::ok &&
              st.get("team.constructor", "constructor/mercedes").status == registry::Status::ok,
          "the stock team and constructor records exist with no addon team records");
    check(st.list("team.identity", "").size() == 2, "and the stock teams are the only team.identity records");
    check(content::in_load_order(st, "team.identity").empty() && content::in_load_order(st, "team.pit_times").empty(),
          "seeding adds no team.identity or team.pit_times record the content appliers walk in load order");
    std::map<std::string, Value> keep;
    auto lookup = content::own_lookup(st, keep);
    auto refs = content::refs_of(st, lookup);
    bool only_stock = true;
    for (auto &[token, team] : refs.constructor_team)
        only_stock = only_stock && team.rfind("game/", 0) == 0;
    check(refs.constructors == 0 && refs.constructors_refused == 0 && refs.lines.empty() && only_stock && refs.pit_record.empty(),
          "the constructor applier's report is unchanged: nothing applied, refused or logged, every constructor id resolves to a stock row");
    apply::ids::Store ids;
    ids.define_pool("team.identity", {{{4, 1023}}});
    int64_t out = -1;
    std::string line;
    check(apply::stat_id(st, ids, "team.identity", "game/tororosso", &out, "test", &line) == SWOFTY_ASSET_OK && out == 3,
          "stat_id on a stock record returns the row's own stat id");
    check(apply::stat_id(st, ids, "team.identity", "game/mercedes", &out, "test", &line) == SWOFTY_ASSET_OK && out == 0,
          "including stat id 0");
    ids.allocate("team.identity", "mod.a/new", "test");
    int64_t fresh = ids.allocate("team.identity", "mod.a/other", "test").id;
    check(fresh == 5, "a stock stat_id call allocates nothing from the pool: the next free id is still the first above the table");
}

static void virtual_list_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.vl"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "livery", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + (std::string_view(name) == "content.package" ? "/1.5.json" : "/1.0.json"));
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    auto package = [](const char *list, bool virt) {
        return std::string(R"({ "title": "lng_x", "entrant_list": ")") + list + R"(", "season": 1960, "stat_id": "auto", )" +
               (virt ? R"("virtual_list": true, )" : "") + R"("teams": [ { "key": "mercedes", "vehicle": null,
                 "drivers": [ { "driver": "game/lewis_hamilton", "number": 44, "look": null } ] } ],
                 "database": { "entrant_list": ")" + list + R"(", "scenario_stat_id": null, "content_sets": ["2025"],
                 "looks": { "mercedes/lewis_hamilton": "x" } } })";
    };
    std::string json = std::string("{ \"content.package\": { \"real\": ") + package("vl_real", false) +
                       ", \"v1\": " + package("vl_1960", true) + ", \"v2\": " + package("vl_1961", true) + " } }";
    check(registry::load_data_file("mod.vl", "registry/content.json", json) == SWOFTY_ASSET_OK, "the virtual list packages register");
    st.end_register();
    Db db;
    stock_package_rows(db);
    apply::ids::Store ids;
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    std::map<std::string, content::PoolScan> scans;
    for (auto &scan : content::scan_all_pools(text)) {
        ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
        scans[scan.registry] = scan;
    }
    content::settle_ids(ids, st, scans, "test");
    content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
        return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
    };
    content::Expansion x(db.base());
    content::expand(st, x, stat);
    auto &e = x.engine;
    const std::string lists = "entrants.entrant_lists.entrant_list_";
    auto label = [&](int i) { return e.text(lists + std::to_string(i) + "._metadata_").value_or(""); };
    std::string all;
    for (auto &l : x.lines["content.package"])
        all += l + "\n";
    check(e.text("entrants.entrant_lists._entrant_list_instancecount_").value_or("") == "3",
          "two virtual lists leave the stock list, the real one and the host counted\n" + all);
    std::set<std::string> counted = {label(0), label(1), label(2)};
    check(counted == std::set<std::string>{"f1_2025", "vl_real", "swofty_virtual_list"}, "the counted rows are f1_2025, vl_real and the host");
    check(label(10000) == "vl_1960" && label(10001) == "vl_1961", "the virtual lists sit at entrant_list_10000 and 10001");
    check(e.text(lists + "10000.entrant_0.entrant_reference").has_value(), "a virtual list keeps its entrant rows");
    check(!e.text(lists + "10000.stat_id").has_value() || e.text(lists + "10000.stat_id") != e.text(lists + "1.stat_id"),
          "a virtual list does not share the real list's stat id");
    check(all.find("virtual lists: host swofty_virtual_list added") != std::string::npos, "the host is added and logged");
    check(all.find("virtual lists: 2 moved above the count") != std::string::npos, "the move is logged");
}

int main(int argc, char **argv)
{
    box_time_trial_tests();
    seat_entrant_tests();
    slot_tests();
    engine_tests();
    room_hole_tests();
    livery_tests();
    garage_tests();
    package_team_row_tests();
    points_tests();
    if (argc > 1)
        variant_tests(argv[1]);
    if (argc > 1)
        leaderboard_tests(argv[1]);
    if (argc > 1)
        team_tests(argv[1]);
    if (argc > 1)
        twelve_team_tests(argv[1]);
    if (argc > 1)
        short_list_tests(argv[1]);
    if (argc > 1)
        stock_only_tests(argv[1]);
    if (argc > 1)
        twelve_slot_tests(argv[1]);
    if (argc > 1)
        drs_tests(argv[1]);
    race_fuel_tests();
    if (argc > 1)
        career_registry_tests(argv[1]);
    if (argc > 1)
        shared_switch_tests(argv[1]);
    if (argc > 1)
        id_clash_tests(argv[1]);
    if (argc > 1)
        virtual_list_tests(argv[1]);
    std::printf("%s\n", failures ? "FAILED" : "all passed");
    return failures ? 1 : 0;
}
