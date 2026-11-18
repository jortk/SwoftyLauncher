// Host unit test for career saves: the index model (career_saves_logic.hpp) and the SDK's career::saves calls
// over the in-memory fake table (fake_saves.hpp). run_events_test.sh builds and runs it.
#include "../src/core/career_saves_logic.hpp"

#include <swofty/fake_saves.hpp>

#include <cstdio>

namespace cs = runtime::career_saves;
using swofty::json::Value;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static void index_model()
{
    cs::Index ix;
    expect(ix.load(""), "an empty file loads as an empty index");
    ix.set_text("m:1", "mode", "driver_career");
    ix.set_text("m:1", "driver", "Max");
    ix.set_text("m:1", "driver", "");
    ix.set_int("m:1", "season", 2016);
    ix.need("m:1", {"a.b", "^1.0.0", "its calendar"});
    ix.need("m:1", {"a.b", "", "again"});
    ix.need("m:1", {"c.d", "", ""});
    ix.saved("m:1", 1700);
    std::string text = ix.to_json();
    cs::Index back;
    expect(back.load(text) && back.size() == 1, "the index reads back");
    const Value *c = back.find("m:1");
    expect(c && (*c)["driver"].as_string() == "Max" && (*c)["season"].as_int() == 2016, "fields survive; an empty value is not written");
    expect(c && (*c)["addons"].size() == 2 && (*c)["addons"][0]["need"].as_string() == "^1.0.0" &&
               (*c)["addons"][0]["why"].as_string() == "again",
           "a repeated need updates the addon's row and keeps the range");
    expect(c && (*c)["last_saved"].as_int() == 1700 && (*c)["status"].as_string() == "present", "saved stamps the time");
    expect(cs::Index().load("{\"careers\":[{\"career\":\"x\",\"profile\":\"p\",\"match\":{\"driver\":1}},{\"nope\":1}]}"),
           "the launcher's shape loads");
    cs::Index launcher;
    launcher.load("{\"careers\":[{\"career\":\"x\",\"profile\":\"p\",\"match\":{\"driver\":1}},{\"nope\":1}]}");
    launcher.set_text("x", "mode", "my_team");
    expect(launcher.size() == 1 && (*launcher.find("x"))["profile"].as_string() == "p" &&
               (*launcher.find("x"))["match"]["driver"].as_int() == 1,
           "fields the core does not own stay, and a record without an id is dropped");
    expect(!cs::Index().load("garbage"), "garbage is refused");
}

static void missing_and_back()
{
    cs::Index ix;
    ix.touch("a");
    ix.touch("b");
    ix.touch("c");
    auto gone = ix.reconcile({"a", "c"});
    expect(gone.size() == 1 && gone[0] == "b", "reconcile names the career the load did not find");
    expect(ix.reconcile({"a", "c"}).empty(), "a career already missing is not named again");
    expect(ix.missing_ids() == std::vector<std::string>{"b"}, "missing_ids lists it");
    ix.reconcile({"a", "b", "c"});
    expect(ix.missing_ids().empty(), "a career that comes back is present again");
    ix.reconcile({"a"});
    cs::Index fx;
    fx.touch("driver_career:00000000000000ab");
    fx.touch("driver_career_progressive:00000000000000cd");
    fx.touch("my_team:7");
    fx.touch("driver_career:new1");
    auto here = fx.found_in({"00000000000000ab", "7"});
    expect(here.size() == 3 && here.count("driver_career:00000000000000ab") && here.count("my_team:7") &&
               here.count("driver_career:new1") && !here.count("driver_career_progressive:00000000000000cd"),
           "found_in matches a career by its slot key and keeps a provisional one");
    Value only = swofty::json::parse(ix.only_json(ix.missing_ids()));
    expect(only["careers"].size() == 2, "only_json holds the missing records");
}

static void data_files()
{
    expect(cs::safe_name("f1:7/../x") == "f1%3a7%2f..%2fx", "a career id becomes one safe folder name");
    expect(cs::data_file("m:1", "a.b") == "careers/data/m%3a1/a.b.json", "data file path");
    expect(cs::addon_id("swofty.progressive") && !cs::addon_id("../x") && !cs::addon_id("a/b") && !cs::addon_id(""), "addon ids");
    expect(!cs::plain_id("a/b") && cs::plain_id("driver_career:7731"), "career ids");
    std::string wrapped = cs::wrap_data(3, swofty::json::parse("{\"k\":[1,2]}"));
    uint32_t v = 0;
    std::string value;
    expect(cs::unwrap_data(wrapped, v, value) && v == 3 && value == "{\"k\":[1,2]}", "data round-trips with its version");
    expect(!cs::unwrap_data("{\"data\":1}", v, value) && !cs::unwrap_data("nope", v, value), "a file without a version is refused");
    expect(cs::label_of("g", "Max", "RB", 2016) == "g: Max, RB, 2016", "label");
}

static void sdk_over_fake()
{
    using namespace swofty;
    expect(!career::saves::available() && career::saves::index().empty() && career::saves::current().empty(),
           "without a table nothing is available");
    expect(career::saves::need("a.b") == career::Result::unavailable, "need is unavailable without a table");
    {
        fake::Saves saves;
        saves.install();
        expect(career::saves::available(), "the fake table is available");
        expect(career::saves::current().empty(), "no career loaded");
        saves.load("m:1", "driver_career", "g1");
        expect(career::saves::current() == "m:1", "current is the loaded career");
        expect(career::saves::need("a.b", "^1.2.0", "its calendar") == career::Result::ok, "need");
        career::saves::Description d;
        d.driver = "Max";
        d.season = 2016;
        d.round = 4;
        expect(career::saves::describe(d) == career::Result::ok, "describe");
        auto e = career::saves::find("m:1");
        expect(e && e->driver == "Max" && e->season == 2016 && e->round == 4 && e->content == "g1" && e->mode == "driver_career",
               "the index entry has what was described");
        expect(e && !e->addons.empty() && e->addons[0].id == "a.b" && e->addons[0].range == "^1.2.0" &&
                   e->addons[0].why == "its calendar",
               "the entry lists the need");
        auto mine = career::saves::data("a.b");
        expect(!mine.get("m:1"), "no data yet");
        expect(mine.set_current(json::parse("{\"x\":1}"), 2) == career::Result::ok, "set_current");
        auto got = mine.get_current();
        expect(got && got->version == 2 && got->value["x"].as_int() == 1, "get returns the version and the value");
        expect(career::saves::data("c.d").get("m:1") == std::nullopt, "another addon's data is separate");
        expect(mine.set("zzz", json::parse("1")) == career::Result::refused, "data for a career the index does not know is refused");
        expect(mine.set("m:1", json::parse("{}")) == career::Result::ok && mine.get("m:1")->version == 1, "the default version is 1");
        saves.save();
        expect(career::saves::find("m:1")->last_saved == saves.clock(), "save stamps last_saved");
        expect(career::saves::missing().empty(), "nothing missing yet");
        saves.drop("m:1");
        auto m = career::saves::missing();
        expect(m.size() == 1 && m[0].career == "m:1" && m[0].status == "missing", "a dropped career is listed missing");
        expect(mine.remove("m:1") == career::Result::ok && !mine.get("m:1"), "remove");
        std::string big(5000, 'x');
        saves.load("m:2");
        expect(career::saves::describe({.label = big}) == career::Result::ok && career::saves::find("m:2")->label == big,
               "a long index grows the buffer");
    }
    expect(!career::saves::available(), "the table goes with the fake");
}

static cs::SlotRead slot(uint64_t id, uint64_t guid, uint16_t driver = 0, uint8_t flags = 0, bool details = true,
                         uint64_t date = 1761600000)
{
    cs::SlotRead r;
    r.id = id;
    r.guid = guid;
    r.details = details;
    r.driver = driver;
    r.team = 7;
    r.flags = flags;
    r.season = 2;
    r.weekend = 4;
    r.date = date;
    r.name = "Slot";
    return r;
}

static cs::ListContext ctx(const char *session, bool load, bool complete = true)
{
    cs::ListContext c;
    c.session = session;
    c.at_load = load;
    c.complete = complete;
    c.now_ms = 5;
    return c;
}

static void slot_list()
{
    cs::Index ix;
    auto r = ix.apply_list({slot(1, 0xa1), slot(2, 0xb2), slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", true));
    expect(r.added.size() == 3 && ix.size() == 3, "a first list adds every save slot");
    expect(ix.knows("driver_career:00000000000000a1") && ix.knows("my_team:00000000000000c3"),
           "a new slot is named by its kind and its slot key");
    const Value *a = ix.find("driver_career:00000000000000a1");
    expect(a && (*a)["slot_season"].as_int() == 3 && (*a)["slot_weekend"].as_int() == 4 &&
               (*a)["last_played"].as_int() == 1761600000 && (*a)["team_raw"].as_int() == 7 &&
               (*a)["status"].as_string() == "present" && (*a)["mode"].as_string() == "driver_career",
           "the details are recorded; the season shows as the game's number");
    r = ix.apply_list({slot(1, 0xa1), slot(2, 0xb2), slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", false));
    expect(!r.changed && r.added.empty() && r.deleted.empty() && r.missing.empty(), "an unchanged list changes nothing");
    r = ix.apply_list({slot(1, 0xa1), slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", false));
    expect(r.deleted.size() == 1 && r.deleted[0] == "driver_career:00000000000000b2" && r.missing.empty(),
           "a slot gone since a list of this session is deleted-in-game");
    expect((*ix.find("driver_career:00000000000000b2"))["status"].as_string() == "deleted", "and carries the deleted state");
    r = ix.apply_list({slot(1, 0xa1), slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", true));
    expect(!r.changed, "a deleted career is not reported again");
    r = ix.apply_list({slot(1, 0xa1), slot(2, 0xb2), slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", true));
    expect(r.back.size() == 1 && (*ix.find("driver_career:00000000000000b2"))["status"].as_string() == "present",
           "a slot that comes back is present again");

    cs::Index old;
    old.load(ix.to_json());
    r = old.apply_list({slot(1, 0xa1)}, ctx("s2", true));
    expect(r.missing.size() == 2 && r.deleted.empty() && (*old.find("my_team:00000000000000c3"))["status"].as_string() == "missing",
           "a slot gone since a list of an earlier session is missing, not deleted");
    r = old.apply_list({slot(1, 0xa1)}, ctx("s2", false));
    expect(r.deleted.empty() && r.missing.empty(), "a career already missing stays missing");

    cs::Index part;
    part.apply_list({slot(1, 0xa1), slot(2, 0xb2)}, ctx("s1", true));
    r = part.apply_list({slot(1, 0xa1)}, ctx("s1", true, false));
    expect(!r.changed && (*part.find("driver_career:00000000000000b2"))["status"].as_string() == "present",
           "an incomplete walk marks nothing gone");

    cs::Index sw;
    sw.apply_list({slot(1, 0xa1), slot(2, 0xb2)}, ctx("s1", true));
    r = sw.apply_list({slot(8, 0xd8)}, ctx("s1", true));
    expect(r.missing.size() == 2 && r.deleted.empty(), "a load that shares no career with this session's list is another profile: missing");
    cs::Index none;
    none.apply_list({slot(1, 0xa1)}, ctx("s1", true));
    r = none.apply_list({}, ctx("s1", false));
    expect(r.deleted.size() == 1, "deleting the only career is deleted-in-game");

    cs::Index prov;
    prov.touch("driver_career:new1");
    r = prov.apply_list({slot(1, 0xa1)}, ctx("s1", true));
    expect(r.missing.empty() && (*prov.find("driver_career:new1"))["status"].as_string() == "present",
           "a provisional career is kept");

    cs::Index nm;
    nm.apply_list({slot(1, 0xa1, 5)}, ctx("s1", true));
    nm.set_text("driver_career:00000000000000a1", "driver", "Oscar");
    nm.apply_list({slot(1, 0xa1, 5)}, ctx("s1", false));
    expect((*nm.find("driver_career:00000000000000a1"))["driver"].as_string() == "Oscar", "a known driver name stays while the driver id is the same");
    nm.apply_list({slot(1, 0xa1, 6)}, ctx("s1", false));
    expect(!nm.find("driver_career:00000000000000a1")->contains("driver"), "a name is dropped when the driver id changed");

    cs::Index ad;
    ad.apply_list({slot(1, 0xa1)}, ctx("s1", true));
    expect(ad.adopt("progressive.driver_career:00000000000000a1") && ad.size() == 1 &&
               ad.knows("progressive.driver_career:00000000000000a1") && !ad.adopt("driver_career:new1"),
           "the loaded career's id replaces the slot list's name for it");
    r = ad.apply_list({slot(1, 0xa1)}, ctx("s1", false));
    expect(!r.changed && !r.deleted.size(), "and the next list matches it by its slot key");

    cs::Index tk;
    tk.apply_list({slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", true));
    for (const char *id : {"mt-00000001-00d00000", "mt-00000001-00bc0000"}) {
        tk.set_text(id, "mode", "my_team");
        tk.saved(id, 5);
    }
    r = tk.apply_list({slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", false));
    expect(r.missing.empty() && r.deleted.empty() && tk.missing_ids().empty(),
           "tracker-key ids of a my team career whose slot is listed are not missing");
    tk.find("mt-00000001-00d00000")->operator[]("status") = Value("missing");
    r = tk.apply_list({slot(3, 0xc3, 9, cs::FLAG_MY_TEAM)}, ctx("s1", false));
    expect(r.changed && tk.missing_ids().empty(), "and one already marked missing is present again");
    r = tk.apply_list({slot(1, 0xa1)}, ctx("s2", true));
    expect(r.missing.size() == 3, "with no my team slot listed they are missing");

    cs::Index nd;
    r = nd.apply_list({slot(1, 0xa1, 0, 0x41), slot(2, 0xb2, 0, 0, false), slot(3, 0xc3, 0, 0x40), slot(4, 0xd4, 0, 0x01),
                       slot(5, 0xe5, 0, 0, true, 0)},
                      ctx("s1", true));
    expect(nd.size() == 0 && r.added.empty() && !r.changed,
           "hidden slots, slots without readable details and slots with no date are not careers");

    cs::Index fx;
    std::vector<cs::SlotRead> all;
    for (int i = 0; i < 24; i++) {
        uint8_t flags = i % 4 == 0 ? 0x40 : i % 4 == 1 ? 0x01 : 0;
        uint64_t date = i % 4 == 2 ? 0 : 1761600000;
        all.push_back(slot(100 + i, 0x1000 + i, 0, flags, i % 4 != 3, date));
    }
    all[5] = slot(105, 0x2005, 11, 0, true, 1761600000);
    all[12] = slot(112, 0x2012, 12, 0, true, 1761000000);
    all[20] = slot(120, 0x2020, 13, cs::FLAG_MY_TEAM, true, 1760000000);
    r = fx.apply_list(all, ctx("s1", true));
    expect(fx.size() == 3 && r.added.size() == 3 && fx.knows("driver_career:0000000000002005") &&
               fx.knows("driver_career:0000000000002012") && fx.knows("my_team:0000000000002020"),
           "24 slots of which 3 are real careers list exactly those 3, each named by its own flags");
    r = fx.apply_list(all, ctx("s1", false));
    expect(!r.changed && r.deleted.empty() && r.missing.empty(), "and the same list again changes nothing");

    cs::Index junk;
    junk.touch("driver_career:0000000000001007")["source"] = Value("slots");
    junk.touch("driver_career:0000000000002005")["source"] = Value("slots");
    r = junk.apply_list(all, ctx("s1", true));
    expect(!junk.knows("driver_career:0000000000001007") && junk.knows("driver_career:0000000000002005") && r.changed,
           "a slot-listed entry that is not a career is dropped from an older index");

    cs::Index real;
    real.set_text("driver_career:0000000000001007", "label", "Kept");
    r = real.apply_list(all, ctx("s1", true));
    expect(real.knows("driver_career:0000000000001007") && r.deleted.empty() && r.missing.empty() &&
               (*real.find("driver_career:0000000000001007"))["status"].as_string() == "present",
           "a snapshotted career whose slot turns hidden or empty is not reported gone");
}

int main()
{
    index_model();
    missing_and_back();
    slot_list();
    data_files();
    sdk_over_fake();
    printf("career saves: %d failed\n", fails);
    return fails ? 1 : 0;
}
