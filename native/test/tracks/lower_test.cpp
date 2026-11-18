#include "../../src/core/apply/tracks/boot.hpp"
#include "../../src/core/apply/content/gvdb.hpp"
#include "../../src/core/registry/declare_all.hpp"
#include "../../src/core/registry/table.hpp"
#include "../../src/core/registry/validate.hpp"

#include <cstdio>
#include <fstream>
#include <set>
#include <sstream>

using namespace swofty_core;
using swofty::json::Value;
namespace tk = apply::tracks;
namespace ids = apply::ids;

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

static std::string escape(const std::string &t)
{
    std::string o;
    for (char c : t)
        o += c == '\\' ? "\\\\" : c == '\t' ? "\\t" : c == '\n' ? "\\n" : c == '\r' ? "\\r" : std::string(1, c);
    return o;
}

static std::string unescape(const std::string &t)
{
    std::string o;
    for (std::size_t i = 0; i < t.size(); i++) {
        if (t[i] != '\\' || i + 1 == t.size()) {
            o += t[i];
            continue;
        }
        char n = t[++i];
        o += n == 't' ? '\t' : n == 'n' ? '\n' : n == 'r' ? '\r' : n;
    }
    return o;
}

using Fixture = std::map<uint64_t, tk::Rec>;

static Fixture read_fixture(const std::string &path)
{
    Fixture f;
    std::istringstream in(slurp(path));
    std::string line;
    while (std::getline(in, line)) {
        if (line.empty() || line[0] == '#')
            continue;
        auto a = line.find('\t'), b = line.find('\t', a + 1);
        if (a == std::string::npos || b == std::string::npos)
            continue;
        f[std::stoull(line.substr(0, a), nullptr, 16)] = {std::stoi(line.substr(a + 1, b - a - 1)), unescape(line.substr(b + 1))};
    }
    return f;
}

static tk::Read fixture_read(const Fixture &f)
{
    return [&f](uint64_t k) -> std::optional<tk::Rec> {
        auto it = f.find(k);
        if (it == f.end())
            return std::nullopt;
        return it->second;
    };
}

struct Case {
    const char *name, *code, *from, *slot;
    std::vector<std::string> ui_lists;
    std::vector<std::string> sprint;
};

static const Case CASES[] = {{"okayama", "okayama", "melbourne", "melbourne", {}},
                             {"bahrain24", "bahrain24", "bahrain", "", {}},
                             {"delhi", "delhi", "zandvoort", "", {}},
                             {"pictures", "pictures", "bahrain", "", {"fe_world.tracks_spline", "tracks.flags_circle"}},
                             {"delhi_sprint", "delhi", "zandvoort", "", {}, {"f1", "f1_2026"}}};

static int make_fixture(const std::string &gamedb, const std::string &out)
{
    apply::content::gvdb::File file;
    if (!file.load(slurp(gamedb))) {
        std::fprintf(stderr, "%s: not a game database\n", gamedb.c_str());
        return 2;
    }
    Fixture seen;
    tk::Read recording = [&](uint64_t k) -> std::optional<tk::Rec> {
        auto r = file.get(k);
        if (!r)
            return std::nullopt;
        tk::Rec rec{r->tag, r->text};
        seen[k] = rec;
        return rec;
    };
    {
        tk::Db db(recording);
        tk::scan(db, tk::builtin_tables());
    }
    for (auto &c : CASES) {
        tk::Db db(recording);
        tk::lower_track(db, tk::builtin_tables(), {c.code, c.code, c.from, c.slot, -1, true});
    }
    {
        tk::Db db(recording);
        tk::lower_track(db, tk::builtin_tables(), {"sepang", "sepang", "bahrain", "", -1, true});
    }
    std::ofstream o(out, std::ios::binary);
    o << "# Records of gamedatabases.2025.bin the track lowering reads for okayama (melbourne), bahrain24 (bahrain) and "
         "delhi (zandvoort), sepang (bahrain), and the track scan. Made by lower_test --make-fixture. key, tag, text.\n";
    for (auto &[k, r] : seen)
        o << tk::hex16(k) << '\t' << r.tag << '\t' << escape(r.text) << '\n';
    std::printf("%zu record(s) written to %s\n", seen.size(), out.c_str());
    return 0;
}

static std::map<uint64_t, std::pair<int, std::string>> expected_of(const std::string &path, bool &found)
{
    std::map<uint64_t, std::pair<int, std::string>> out;
    std::string text = slurp(path);
    found = !text.empty();
    if (!found)
        return out;
    Value doc = swofty::json::parse(text);
    if (auto a = doc["records"].if_array())
        for (auto &r : *a)
            out[std::stoull(std::string(r["key"].as_string()), nullptr, 16)] = {int(r["tag"].as_int()),
                                                                                 std::string(r["text"].as_string())};
    return out;
}

static void parity_tests(const Fixture &f, const std::string &dir)
{
    for (auto &c : CASES) {
        tk::Db db(fixture_read(f));
        tk::Lowered l = tk::lower_track(db, tk::builtin_tables(), {c.code, c.code, c.from, c.slot, -1, true, c.ui_lists, c.sprint});
        check(l.error.empty(), std::string(c.name) + ": lowers over the fixture (" + l.error + ")");
        bool found = false;
        auto want = expected_of(dir + "/fixture/expected/" + c.name + ".json", found);
        if (!found) {
            check(false, std::string(c.name) + ": expected/" + c.name + ".json (cargo test track_lowering_parity) exists");
            continue;
        }
        std::map<uint64_t, std::pair<int, std::string>> got;
        for (auto &w : db.writes()) {
            auto had = f.find(w.key);
            if (had == f.end() || had->second.tag != w.tag || had->second.text != w.text)
                got[w.key] = {w.tag, w.text};
        }
        std::size_t missing = 0, extra = 0, differ = 0;
        std::string first;
        for (auto &[k, v] : want) {
            auto it = got.find(k);
            if (it == got.end()) {
                missing++;
                if (first.empty())
                    first = "missing " + tk::hex16(k) + " = " + v.second;
            } else if (it->second != v) {
                differ++;
                if (first.empty())
                    first = "differs " + tk::hex16(k) + ": " + it->second.second + " vs " + v.second;
            }
        }
        for (auto &[k, v] : got)
            if (!want.count(k)) {
                extra++;
                if (first.empty())
                    for (auto &w : db.writes())
                        if (w.key == k)
                            first = "extra " + w.path + " = " + v.second;
            }
        check(!missing && !extra && !differ,
              std::string(c.name) + ": the core's " + std::to_string(got.size()) + " record(s) equal the launcher's " +
                  std::to_string(want.size()) + " row for row (" + std::to_string(missing) + " missing, " +
                  std::to_string(extra) + " extra, " + std::to_string(differ) + " differ" + (first.empty() ? "" : "; " + first) + ")");
    }
}

static std::optional<std::string> written(const tk::Db &db, const std::string &suffix)
{
    for (auto &w : db.writes())
        if (w.path.size() >= suffix.size() && w.path.compare(w.path.size() - suffix.size(), suffix.size(), suffix) == 0)
            return w.text;
    return std::nullopt;
}

static bool holds(const std::vector<std::string> &v, const std::string &s)
{
    return std::find(v.begin(), v.end(), s) != v.end();
}

static void weekend_format_tests(const Fixture &f)
{
    const tk::Tables &t = tk::builtin_tables();
    {
        tk::Db db(fixture_read(f));
        tk::Lowered l = tk::lower_track(db, t, {"mod.x/delhi", "delhi", "zandvoort", "", -1, true});
        check(l.error.empty(), "delhi lowers on zandvoort (" + l.error + ")");
        std::size_t sprint = 0, normal = 0;
        for (auto &w : db.writes())
            if (w.path.find("TrackAuthenticWeekendData") != std::string::npos && w.path.ends_with(".weekend_format"))
                (w.text == "sprint" ? sprint : normal)++;
        check(sprint == 0 && normal > 0, "a track added on a sprint base runs normal weekends (" + std::to_string(sprint) +
                                             " sprint, " + std::to_string(normal) + " default)");
        check(!holds(l.warnings, "sprint f1_2026: delhi has no weekend format in that data set, so it takes its parent data set's"),
              "no sprint warning without a sprint list");
    }
    {
        tk::Db db(fixture_read(f));
        tk::Lowered l = tk::lower_track(db, t, {"mod.x/delhi", "delhi", "zandvoort", "", -1, true, {}, {"f1", "f1_2025"}});
        check(l.error.empty(), "delhi lowers with a sprint list (" + l.error + ")");
        check(holds(l.lines, "weekend format: delhi runs sprint weekends in f1"), "the sprint data sets are logged");
        check(holds(l.warnings, "sprint f1_2025: delhi has no weekend format in that data set, so it takes its parent data set's"),
              "a sprint data set without a weekend format block is a warning");
    }
}

static void lowering_tests(const Fixture &f)
{
    const tk::Tables &t = tk::builtin_tables();
    check(t.ok && t.per_track.size() >= 20 && t.track_resources.has_value(), "the embedded table description parses");
    {
        tk::Db db(fixture_read(f));
        tk::Lowered l = tk::lower_track(db, t, {"mod.x/okayama", "okayama", "melbourne", "melbourne", 250, true});
        check(l.error.empty() && l.new_row && l.stat_id == 250, "a new row takes the stat id it is given (" + l.error + ")");
        check(written(db, ".instanceName") == "okayama" && written(db, ".fileName") == "okayama", "the track row is relabelled");
        check(written(db, "tracks._Track_instancecount_") == "44", "tracks.Track grows by one row");
        check(l.sets.size() >= 4, "TrackData blocks in " + std::to_string(l.sets.size()) + " data set(s)");
        check(written(db, ".iblKey") == "key_2019.assetgroup" && written(db, ".track_resource") == "swofty_okayama",
              "the track resource routes to the track's own asset groups");
        bool listed = false;
        for (auto &w : db.writes())
            if (w.path.find("available_tracks") != std::string::npos && w.text.find(",okayama") != std::string::npos)
                listed = true;
        check(listed, "available_tracks lists holding melbourne get okayama");
        std::size_t fresh = 0;
        for (auto &w : db.writes())
            fresh += w.fresh;
        check(fresh > 1000, std::to_string(fresh) + " new database key(s) for one track");
        std::size_t mark = 0;
        tk::Db again(fixture_read(f));
        mark = again.mark();
        tk::lower_track(again, t, {"mod.x/okayama", "okayama", "melbourne", "melbourne", 250, true});
        again.rollback(mark);
        check(again.writes().empty(), "a rollback takes every write back");
    }
    {
        tk::Db db(fixture_read(f));
        tk::Lowered l = tk::lower_track(db, t, {"mod.x/bahrain24", "bahrain24", "bahrain", "bahrain24", 200, true});
        check(l.error.empty() && !l.new_row && l.stat_id == 35, "filling the dormant bahrain24 row keeps its row and stat id 35");
    }
    {
        tk::Db db(fixture_read(f));
        tk::Lowered l = tk::lower_track(db, t, {"mod.x/nowhere", "nowhere", "atlantis", "", 200, true});
        check(!l.error.empty() && db.writes().empty(), "a base that is no track is refused before any write (" + l.error + ")");
    }
}

static void declare_tracks(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"mod.a", "mod.older", "mod.b"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"track", "track.venue", "track.data", "track.per_package"}) {
        std::string schema = slurp(regs + "/" + name + "/1.0.json");
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
}

static void seed_tests(const Fixture &f, const std::string &regs)
{
    tk::Db db(fixture_read(f));
    tk::Scan s = tk::scan(db, tk::builtin_tables());
    check(s.rows.size() == 43, std::to_string(s.rows.size()) + " tracks.Track rows scanned");
    check(s.used.count(0) && s.used.at(0) == "melbourne" && s.used.count(42), "the loaded rows' stat ids are known");
    check(s.sets.size() == 20, std::to_string(s.sets.size()) + " data sets scanned");
    const tk::Block *melb = nullptr;
    std::size_t with_laps = 0;
    for (auto &b : s.blocks) {
        with_laps += b.value["laps"].is_int();
        if (!melb && b.code == "melbourne" && b.value["laps"].is_int())
            melb = &b;
    }
    check(melb && melb->value["laps"].as_int() > 0 && melb->value["fuel"]["burn_per_lap"].is_number(),
          "a melbourne data block carries laps and fuel (" + (melb ? melb->set + ": " + std::to_string(melb->value["laps"].as_int()) : std::string("none")) +
              " laps; " + std::to_string(with_laps) + " of " + std::to_string(s.blocks.size()) + " blocks hold laps)");

    std::string older_json = R"({"version":1,"profile":"Classic Seasons","tracks":[
        {"addon":"mod.older","code":"okayama","name":"Okayama International Circuit","base":"game/suzuka","slot":null,"stat_id":44,"venue":"venue/okayama"},
        {"addon":"mod.older","code":"sochi","name":"Sochi Autodrom","base":"game/baku","slot":"game/sochi","stat_id":18,"venue":"venue/sochi"}]})";
    tk::OlderFile other = tk::parse_older(older_json, "Tails what-if");
    check(!other.read && other.tracks.empty() && !other.lines.empty() &&
              other.lines[0].find("is for profile Classic Seasons; this build is Tails what-if") != std::string::npos,
          "older-tracks.json for another profile is ignored");
    tk::OlderFile older = tk::parse_older(older_json, "Classic Seasons");
    check(older.read && older.tracks.size() == 2 && older.tracks[1].slot == "game/sochi", "older-tracks.json is read");

    tk::Seeds seeded = tk::seeds(s, older.tracks);
    auto seed = [&](const std::string &reg, const std::string &key) -> const Value * {
        for (auto &b : seeded.base)
            if (b.registry == reg && b.key == key)
                return &b.value;
        return nullptr;
    };
    const Value *m = seed("track", "melbourne");
    check(m && (*m)["time_trial"].as_bool() && (*m)["provides"][0].as_string() == "venue/albert_park" && (*m)["stat_id"].as_int() == 0,
          "game/melbourne: time_trial, provides venue/albert_park, stat id 0");
    const Value *rev = seed("track", "silverstone_reverse");
    check(rev && rev->size() && (*rev)["provides"].size() == 0 && (*rev)["time_trial"].as_bool(),
          "a reverse layout is a track that provides nothing");
    const Value *dormant = seed("track", "sochi");
    check(dormant && !(*dormant)["time_trial"].as_bool() && (*dormant)["provides"].size() == 0,
          "a dormant row is a record that provides nothing and is not in Time Trial");
    const tk::Seed *sepang = nullptr;
    std::vector<tk::Seed> venues_seeded = tk::venue_bases();
    for (auto &v : venues_seeded)
        if (v.key == "sepang")
            sepang = &v;
    check(sepang && sepang->value["track"].is_null() && !seed("track.venue", "sepang"),
          "venue/sepang is seeded with no track, apart from the database's seeds");
    check(seed("track.data", "melbourne/f1_2025") != nullptr, "game/melbourne/f1_2025 is seeded");
    check(seeded.older.size() == 2 && seeded.older[0].second.value["provides"][0].as_string() == "venue/okayama" &&
              !seeded.older[0].second.value["time_trial"].as_bool(),
          "an older track is <addon>/<code>, not in Time Trial, providing its matched venue");

    ids::Pool pool = tk::track_pool(s.used, older.tracks);
    check(!pool.ranges.empty() && pool.ranges.front().first == 254 && pool.ranges.back().first == 43,
          "the track pool runs from 254 down to the first id above the loaded rows");
    bool clash = false;
    for (auto &[lo, hi] : pool.ranges)
        clash = clash || s.used.count(lo) || lo == 44;
    check(!clash, "the pool skips every loaded row's and older track's id");

    declare_tracks(regs);
    auto &st = registry::store();
    int r1 = registry::load_data_file("mod.a", "registry/track.json", R"j({ "track": {
        "sepang": { "code": "sepang", "name": "Sepang International Circuit", "base": "game/bahrain", "provides": ["venue/sepang"], "stat_id": "auto" },
        "albert": { "code": "albert", "base": "game/melbourne", "provides": ["venue/albert_park"] },
        "fill_sochi": { "code": "sochi", "base": "game/baku", "slot": "game/sochi" } } })j");
    int r2 = registry::load_data_file("mod.b", "registry/track.json", R"j({ "track": {
        "a_bahrain24": { "code": "bahrain24", "base": "game/bahrain", "slot": "game/bahrain24" },
        "b_bahrain24": { "code": "bahrain24", "base": "game/bahrain", "slot": "game/bahrain24" },
        "copy_melb": { "code": "melbourne", "base": "game/melbourne" },
        "pinned": { "code": "pinned", "base": "game/zandvoort", "stat_id": 3 } } })j");
    st.end_register();
    for (auto &r : st.refusals())
        std::printf("      refused: %s %s %s %s: %s\n", r.addon.c_str(), r.registry.c_str(), r.id.c_str(), r.reason.c_str(), r.message.c_str());
    check(r1 == SWOFTY_ASSET_OK && r2 == SWOFTY_ASSET_OK, "the track records register");
    tk::declare_venues(st);
    tk::declare_seeds(st, seeded);
    check(st.get("track", "game/melbourne").status == registry::Status::ok, "game/melbourne is a record");
    check(st.get("track", "mod.older/okayama").status == registry::Status::ok, "the older track is a record");

    ids::store().clear();
    ids::store().define_pool("track", pool);
    std::map<int64_t, std::string> used = s.used;
    tk::StatIds stat = [&](const std::string &id, int64_t pin) -> tk::StatId {
        if (pin >= 0) {
            if (used.count(pin))
                return {-1, "stat id " + std::to_string(pin) + " is the row " + used[pin] + "'s"};
            return {ids::store().pin("track", id, pin, "2026-10-30").id, "pinned"};
        }
        ids::Result r = ids::store().allocate("track", id, "2026-10-30");
        return {r.id, r.how};
    };
    tk::Db boot(fixture_read(f));
    tk::Plan p = tk::lower_records(st, boot, tk::builtin_tables(), s, older.tracks, stat);
    for (auto &l : p.lines)
        std::printf("      %s\n", l.c_str());
    auto done = [&](const std::string &id) -> const tk::Done * {
        for (auto &d : p.done)
            if (d.id == id)
                return &d;
        return nullptr;
    };
    auto said = [&](const std::string &part) {
        for (auto &l : p.lines)
            if (l.find(part) != std::string::npos)
                return true;
        return false;
    };
    check(done("mod.a/albert") && done("mod.a/albert")->stat_id == 254, "the first record track in load order takes stat id 254");
    check(done("mod.a/sepang") && done("mod.a/sepang")->stat_id == 253, "the next takes 253");
    check(!done("mod.a/fill_sochi") && said("[registry] track mod.a/fill_sochi: slot game/sochi is already filled by mod.older/sochi; refused"),
          "a slot an older track fills is refused");
    check(done("mod.b/a_bahrain24") && done("mod.b/a_bahrain24")->stat_id == 35 && !done("mod.b/a_bahrain24")->new_row,
          "a record filling a dormant row keeps the row's stat id");
    check(!done("mod.b/b_bahrain24") &&
              said("[registry] track mod.b/b_bahrain24: slot game/bahrain24 is already filled by mod.b/a_bahrain24; refused"),
          "two records naming one slot: the later is refused");
    check(!done("mod.b/copy_melb") && said("code melbourne is already game/melbourne's"), "a code a row already has is refused");
    check(!done("mod.b/pinned") && said("stat id 3 is the row bahrain's"), "a pinned stat id a loaded row holds is refused");
    check(p.fresh > 1000 && p.addon_fresh["mod.a"] > 0, std::to_string(p.fresh) + " new database key(s) counted for sizing");
    check(ids::store().get("track", "mod.a/sepang").id == 253, "the allocated id is pinned in ids.json");

    std::size_t variants = tk::declare_data_variants(st, p);
    check(variants >= 4, std::to_string(variants) + " data block variant(s) declared");
    std::string lap_set;
    int64_t laps = 0;
    for (auto &bl : s.blocks)
        if (bl.code == "bahrain" && bl.value["laps"].is_int() && lap_set.empty()) {
            lap_set = bl.set;
            laps = bl.value["laps"].as_int();
        }
    registry::Record block = st.get("track.data", "mod.a/sepang/" + lap_set);
    check(!lap_set.empty() && block.status == registry::Status::ok && block.value["track"].as_string() == "mod.a/sepang" &&
              block.value["data_set"].as_string() == lap_set && block.value["laps"].as_int() == laps,
          "mod.a/sepang/" + lap_set + " inherits game/bahrain/" + lap_set + "'s " + std::to_string(laps) + " laps (" +
              std::to_string(block.value["laps"].as_int()) + ")");

    std::vector<std::string> venues = st.list("track.venue", "");
    tk::Base b = tk::make_base(s, older.tracks, p, venues, [&st](const std::string &v) { return tk::venue_track(st, v); });
    check(b.by_id("mod.a/sepang") && b.by_id("mod.a/sepang")->kind == tk::Kind::record && b.by_stat_id(253) == b.by_id("mod.a/sepang"),
          "the base holds the record track by id and stat id");
    check(b.by_id("mod.older/sochi") && b.by_id("mod.older/sochi")->kind == tk::Kind::older && b.by_stat_id(18) == b.by_id("mod.older/sochi"),
          "the base holds the older track in the dormant row it fills");
    check(!b.by_code("okayama"), "an older track whose row the database lacks is not in the base");
    check(b.by_id("game/melbourne") && b.by_id("game/melbourne")->time_trial && !b.by_id("game/sochi"),
          "stock rows with assets are in the base, dormant rows are not");
    check(b.filled["game/sochi"] == "mod.older/sochi" && b.filled["game/bahrain24"] == "mod.b/a_bahrain24",
          "the filled dormant rows name their tracks");
    check(b.venue("venue/sepang") == b.by_id("mod.a/sepang"), "venue/sepang resolves to the record track");
    check(b.venue("venue/albert_park") == b.by_id("mod.a/albert"), "venue/albert_park: the later provider (a record) wins over the stock row");
    check(b.venue("venue/okayama") == nullptr && b.venue("venue/sochi") == b.by_id("mod.older/sochi"),
          "an older track provides its matched venue (" + std::string(b.venue("venue/sochi") ? b.venue("venue/sochi")->id : "none") + ")");
    check(b.venue("venue/red_bull_ring") == b.by_id("game/austria") && b.resolve("venue/hockenheimring?") == nullptr,
          "a stock venue resolves to its stock row; a venue with no provider resolves to nothing");
}

static void venue_seed_tests(const std::string &regs)
{
    const char *calendar = R"j({ "career.calendar": { "f1_2016": { "rounds": [
        { "round": 1, "venue": "venue/albert_park" }, { "round": 2, "venue": "venue/sochi?" },
        { "round": 3, "venue": "venue/red_bull_ring" } ] } } })j";
    for (int with_providers = 0; with_providers < 2; ++with_providers) {
        declare_tracks(regs);
        auto &st = registry::store();
        std::string schema = slurp(regs + "/career.calendar/1.0.json");
        registry::table()->declare("career.calendar", SwoftyText{schema.data(), schema.size()});
        int r = registry::load_data_file("mod.a", "registry/content.json", calendar);
        st.end_register();
        tk::Seeds seeded = with_providers ? tk::venue_providers() : tk::Seeds{};
        tk::declare_venues(st);
        for (auto &b : seeded.base)
            st.add_base(b.registry, b.key, b.value);
        st.stock_loaded("track.venue", true);
        registry::Record cal = st.get("career.calendar", "mod.a/f1_2016");
        if (!with_providers) {
            check(r == SWOFTY_ASSET_OK && cal.status == registry::Status::invalid,
                  "without the database and with only the venues seeded, a calendar's venue does not resolve (" + cal.why + ")");
            continue;
        }
        check(cal.status == registry::Status::ok, "with the stock tracks that provide the venues seeded, a Classic Seasons "
                                                  "calendar is valid without the database (" + cal.why + ")");
        registry::Record v = st.get("track.venue", "venue/albert_park");
        check(v.status == registry::Status::ok && v.value["track"].as_string() == "game/melbourne",
              "venue/albert_park resolves to game/melbourne");
        registry::Record t = st.get("track", "game/melbourne");
        check(t.status == registry::Status::ok && !t.value["stat_id"].is_int() && t.value["time_trial"].as_bool(),
              "the seeded provider has no stat id");
        std::size_t providers = 0;
        for (auto &b : seeded.base)
            providers += b.registry == "track";
        check(providers > 20 && providers < std::size(tk::VENUE_SEEDS), std::to_string(providers) +
                                                                         " stock track(s) provide a venue; venues without one get none");
        check(st.get("track.venue", "venue/sochi").status != registry::Status::ok || !st.get("track.venue", "venue/sochi").value["track"].is_string(),
              "venue/sochi has no provider");
    }
}

static void boot_order_tests(const Fixture &f, const std::string &regs)
{
    (void)regs;
    for (int with_database = 0; with_database < 2; ++with_database) {
        registry::reset_store();
        auto &st = registry::store();
        st.set_order({"mod.a"});
        st.set_validator(&registry::schema_validator());
        const SwoftyRegistry *t = registry::table();
        registry::declare_system_registries([t](const std::string &name, const std::string &schema) {
            return name.rfind("track", 0) == 0 && t->declare(name.c_str(), SwoftyText{schema.data(), schema.size()}) == SWOFTY_ASSET_OK;
        });
        std::vector<std::string> notes;
        st.set_log([&notes](const std::string &line) { notes.push_back(line); });
        int r = registry::load_data_file("mod.a", "registry/track.json", R"j({ "track": {
            "hockenheim": { "code": "hockenheim", "name": "Hockenheimring", "base": "game/austria",
                            "provides": ["venue/hockenheimring"], "files": "track/" },
            "nowhere": { "code": "nowhere", "base": "game/austria", "provides": ["venue/nowhere"] } } })j");
        st.end_register();
        tk::declare_venues(st);
        bool content = tk::has_track_records(st);
        if (with_database) {
            tk::Db db(fixture_read(f));
            tk::declare_seeds(st, tk::seeds(tk::scan(db, tk::builtin_tables()), {}));
        } else {
            for (auto &b : tk::venue_providers().base)
                st.add_base(b.registry, b.key, b.value);
            st.stock_loaded("track.venue", true);
        }
        st.list("track", "");
        st.list("track.venue", "");
        auto said = [&](const std::string &part) {
            std::size_t n = 0;
            for (auto &l : notes)
                n += l.find(part) != std::string::npos;
            return n;
        };
        std::string with = with_database ? "with the database: " : "without the database: ";
        check(r == SWOFTY_ASSET_OK && content, with + "the track records count as track content");
        check(said("venue/hockenheimring") == 0 && st.get("track", "mod.a/hockenheim").status == registry::Status::ok,
              with + "a record providing a seeded venue logs nothing at boot and is valid");
        check(st.get("track.venue", "venue/hockenheimring").value["track"].as_string() == "mod.a/hockenheim",
              with + "venue/hockenheimring resolves to the record");
        check(said("[registry] track mod.a/nowhere is invalid: /provides/0 venue/nowhere does not resolve in track.venue") == 1 &&
                  st.get("track", "mod.a/nowhere").status == registry::Status::invalid,
              with + "a record providing an unknown venue is invalid and logged once");
        check(said("is invalid") == 1, with + std::to_string(said("is invalid")) + " invalid line(s) at boot");
        st.set_log(nullptr);
    }
}

int main(int argc, char **argv)
{
    if (argc >= 4 && std::string(argv[1]) == "--make-fixture")
        return make_fixture(argv[2], argv[3]);
    if (argc < 3) {
        std::fprintf(stderr, "usage: lower_test <native/test/tracks> <packages/sdk/std/registries>\n"
                             "       lower_test --make-fixture <gamedatabases.2025.bin> <fixture.tsv>\n");
        return 2;
    }
    std::string dir = argv[1], regs = argv[2];
    Fixture f = read_fixture(dir + "/fixture/tracks.tsv");
    check(f.size() > 1000, std::to_string(f.size()) + " fixture record(s)");
    lowering_tests(f);
    parity_tests(f, dir);
    weekend_format_tests(f);
    seed_tests(f, regs);
    venue_seed_tests(regs);
    boot_order_tests(f, regs);
    std::printf("%s\n", failures ? "FAILED" : "all passed");
    return failures ? 1 : 0;
}
