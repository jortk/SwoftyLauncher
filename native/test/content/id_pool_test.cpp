#include "../../src/core/apply/content/boot.hpp"
#include "../../src/core/apply/ids.hpp"

#include <cstdio>
#include <set>

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

static const char *DAY = "2026-11-08";

static void pool(ids::Store &s, const char *registry, int64_t lo, int64_t hi, int64_t cap)
{
    ids::Pool p;
    p.ranges.push_back({lo, hi});
    p.cap = cap;
    s.define_pool(registry, p);
}

static std::string name(int i)
{
    return "season-" + std::to_string(i) + "/team";
}

static content::StatIds allocator(ids::Store &s, const char *registry)
{
    return [&s, registry](const std::string &, const std::string &id, int64_t) -> int64_t {
        return s.allocate(registry, id, DAY).id;
    };
}

int main()
{
    {
        ids::Store s;
        pool(s, "team.identity", 500, 509, 1023);
        for (int i = 0; i < 10; i++)
            check(s.allocate("team.identity", name(i), DAY).id == 500 + i, "fills " + name(i));
        ids::Result full = s.allocate("team.identity", "new/team", DAY);
        check(full.id < 0 && full.how == "full", "a full pool with every holder live refuses");
        s.set_live("team.identity", {"new/team", name(0), name(1)});
        ids::Result a = s.allocate("team.identity", "new/team", DAY);
        check(a.id == 502 && a.how == "reclaimed", "a full pool reclaims the lowest id of a holder that is not installed");
        check(s.get("team.identity", name(2)).id < 0, "and the holder it took from no longer holds one");
        check(s.get("team.identity", name(0)).id == 500 && s.get("team.identity", name(1)).id == 501,
              "installed holders keep their ids");
    }
    {
        ids::Store s;
        std::string text = R"({"version":1,"ids":{"team.identity":{
            "gone/a":{"id":500},"gone/b":{"id":501},"gone/c":{"id":502}}},
            "careers":{"driver_career:1":{"team.identity":{"gone/b":501}}}})";
        check(s.load(text), "an ids.json with a career loads");
        pool(s, "team.identity", 500, 502, 1023);
        s.set_live("team.identity", {"new/x", "new/y"});
        ids::Result a = s.allocate("team.identity", "new/x", DAY);
        ids::Result b = s.allocate("team.identity", "new/y", DAY);
        check(a.id == 500 && b.id == 502, "reclaims the ids no career holds");
        ids::Result c = s.allocate("team.identity", "new/z", DAY);
        check(c.id < 0 && c.how == "full", "and never the id a saved career holds (501)");
        check(s.get("team.identity", "gone/b").id == 501, "the career's holder keeps it");
    }
    {
        ids::Store s;
        pool(s, "vehicle", 1020, 1030, 1023);
        std::set<int64_t> got;
        for (int i = 0; i < 20; i++) {
            int64_t id = s.allocate("vehicle", "a/v" + std::to_string(i), DAY).id;
            if (id >= 0)
                got.insert(id);
        }
        check(got.size() == 4 && *got.rbegin() == 1023, "no id passes the ceiling and the pool gives 1020 to 1023");
        std::string text = R"({"version":1,"ids":{"vehicle":{"old/car":{"id":1334}}}})";
        ids::Store t;
        t.load(text);
        pool(t, "vehicle", 500, 1030, 1023);
        ids::Result r = t.get("vehicle", "old/car");
        check(r.id < 0, "an id kept above the ceiling is not handed out again");
        ids::Result n = t.allocate("vehicle", "old/car", DAY);
        check(n.id == 500, "the record takes one inside the ceiling");
    }
    {
        ids::Store s;
        pool(s, "vehicle", 500, 501, 1023);
        s.allocate("vehicle", "a/one", DAY);
        s.allocate("vehicle", "a/two", DAY);
        Value car = Value::object({{"label", Value("three")}, {"donor", Value("game/ferrari")}});
        content::Lookup lookup = [](const std::string &, const std::string &) -> const Value * { return nullptr; };
        content::Source source = [](const std::string &) -> std::optional<content::Stock> { return std::nullopt; };
        content::Plan p;
        content::lower_vehicle("a/three", car, lookup, source, p, allocator(s, "vehicle"));
        check(p.ops.size() == 0 && p.refused == 1 && p.lines.back().find("left out") != std::string::npos,
              "a vehicle that can't get an id lowers to no row and is refused");
        content::Plan big;
        content::lower_vehicle("a/big", car, lookup, source, big, [](const std::string &, const std::string &, int64_t) -> int64_t { return 16383; });
        check(big.ops.size() == 0 && big.refused == 1, "a vehicle given an id above 16382 is left out");
        content::Plan wide;
        content::lower_vehicle("a/wide", car, lookup, source, wide, [](const std::string &, const std::string &, int64_t) -> int64_t { return 1334; });
        check(wide.refused == 0 && wide.ops.size() > 0, "a vehicle given an id above 1023 gets its row");

        content::Dropped dropped = {content::drop_key("vehicle", "a/three")};
        Value list = Value::object({{"entrant_list", Value("f_1986")},
                                    {"teams", Value::array({Value::object({{"key", Value("a")}, {"vehicle", Value("a/one")}}),
                                                            Value::object({{"key", Value("b")}, {"vehicle", Value("a/three")}})})}});
        check(content::dropped_ref(list, &dropped) == "vehicle a/three", "a list naming the left-out vehicle is found");
        content::Plan lp;
        content::lower_package("a/list", list, lookup, lp, {}, nullptr, {}, nullptr, &dropped);
        check(lp.ops.size() == 0 && lp.refused == 1 && lp.lines.back().find("a/three") != std::string::npos,
              "the entrant list is left out and nothing of it references the missing row");
        content::Dropped team_drop = {content::drop_key("team.identity", "a/t")};
        Value tl = Value::object({{"entrant_list", Value("g")},
                                  {"teams", Value::array({Value::object({{"key", Value("a")}, {"team", Value("a/t")}})})}});
        check(content::dropped_ref(tl, &team_drop) == "team a/t", "a list naming a left-out team is found");
        check(content::dropped_ref(list, &team_drop).empty(), "a list naming neither is kept");
    }
    {
        ids::Store s;
        pool(s, "team.identity", 500, 1023, 1023);
        std::set<int64_t> seen;
        bool dup = false;
        for (int i = 0; i < 80; i++) {
            int64_t id = s.allocate("team.identity", "season-" + std::to_string(i % 8) + "/t" + std::to_string(i), DAY).id;
            dup = dup || !seen.insert(id).second;
        }
        check(!dup, "two addons never get the same id");
        std::string text = R"({"version":1,"ids":{"team.identity":{
            "season-1972/mclaren":{"id":663},"season-1986/mclaren":{"id":663},"season-2000/x":{"id":664}}},
            "careers":{"c":{"team.identity":{"season-1986/mclaren":663}}}})";
        ids::Store d;
        d.load(text);
        check(d.deduped() == 1 && d.get("team.identity", "season-1986/mclaren").id == 663 &&
                  d.get("team.identity", "season-1972/mclaren").id < 0,
              "an ids.json that gives two addons one id keeps it for the one a career holds");
    }
    {
        std::string own = R"({"version":1,"ids":{
            "content.package":{"swofty.s23/sw_2023":{"id":115},"swofty.other/pack":{"id":116}},
            "content.package.scenario":{"swofty.s23/sw_2023":{"id":25}}}})";
        std::string host = R"({"version":1,"ids":{
            "content.package":{"swofty.s23/sw_2023":116},
            "content.package.scenario":{"swofty.s23/sw_2023":26}}})";
        ids::Store s;
        s.load(own);
        pool(s, "content.package", 15, 254, 254);
        pool(s, "content.package.scenario", 15, 254, 254);
        std::vector<std::string> lines;
        std::string why;
        check(s.lend_json(host, &lines, &why) && why.empty(), "the host's ids are read");
        check(s.get("content.package", "swofty.s23/sw_2023").id == 116 &&
                  s.get("content.package.scenario", "swofty.s23/sw_2023").id == 26,
              "a lent package and scenario id replace the install's own");
        check(s.allocate("content.package", "swofty.s23/sw_2023", DAY).id == 116, "allocating a lent record gives the lent id");
        int64_t moved = s.get("content.package", "swofty.other/pack").id;
        check(moved >= 15 && moved != 116 && moved != 115, "an unrelated record on the lent id moves to a free one");
        check(s.allocate("content.package", "new/pack", DAY).id != 116 && s.allocate("content.package", "new/pack", DAY).id != moved,
              "a new record never takes a lent id");
        check(s.pin("content.package", "swofty.s23/sw_2023", 40, DAY).id == 116, "an explicit pin does not undo a lent id");
        check(!s.dirty() && s.lent(), "a lent store is never written back");
        std::string dumped = s.dump();
        check(dumped.find("\"id\": 116") != std::string::npos, "the lent ids are in the dump a caller would see");
    }
    {
        ids::Store s;
        s.load(R"({"version":1,"ids":{"content.package":{"a/p":{"id":20}}},
            "careers":{"c":{"content.package":{"a/p":20,"b/q":30}}}})");
        pool(s, "content.package", 15, 254, 254);
        s.set_live("content.package", {"a/p", "b/q"});
        std::vector<std::string> lines;
        s.lend_json(R"({"ids":{"content.package":{"c/r":30}}})", &lines);
        bool refused = false;
        for (auto &l : lines)
            refused = refused || l.find("not lent") != std::string::npos;
        check(refused && s.get("content.package", "c/r").id < 0, "an id a career uses for another record is refused and logged");
        std::vector<std::string> both;
        s.lend_json(R"({"ids":{"content.package":{"x/p":50,"y/p":50}}})", &both);
        check(s.get("content.package", "x/p").id == 50 && s.get("content.package", "y/p").id < 0,
              "two lent records on one id lend only the first");
        ids::Store clean;
        std::string why;
        check(!clean.lend_json("nope", nullptr, &why) && !why.empty() && !clean.lent(), "unreadable lent ids lend nothing");
    }
    return failures ? 1 : 0;
}
