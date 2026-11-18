#include "boot.hpp"
#include "box.hpp"

#include "../applier.hpp"
#include "../database.hpp"
#include "../ids.hpp"
#include "../content/gvdb.hpp"

#include "../../runtime.hpp"

#include <swofty/mixin.hpp>

#include <ctime>
#include <memory>
#include <mutex>
#include <unordered_map>

// The core's appliers for the track registries at boot. Before any applier reads the store (prepare) the venues are
// declared and the tracks made for 1.5 are read from older-tracks.json and, when they or any addon's record or layer
// in the track registries exist, the database file under the older addons' edits is scanned once: the stock rows and
// data blocks are seeded, every record track is lowered without stat ids to count its new keys, and its data blocks
// become variants of its base's. With neither, the file is not read: only the stock tracks that provide the venues
// are seeded (career.calendar and livery x-ref venues), and the base is empty. At boot the record tracks are lowered
// again with stat ids from the track pool (254 down, pinned in ids.json), their records are
// staged for gamedb.cpp, and the effective base is built for the box lists and the guard.

namespace swofty_core::apply::tracks {

namespace {

std::recursive_mutex mu;

struct State {
    content::gvdb::File file;
    std::unordered_map<uint64_t, Rec> edits;
    bool loaded = false, ok = false, skipped = false, seeded = false, booted = false;
    Scan scan;
    OlderFile older;
    std::unique_ptr<Plan> plan;
    std::vector<std::string> lines;
};

State &state()
{
    static State s;
    return s;
}

std::string today()
{
    std::time_t t = std::time(nullptr);
    std::tm tm{};
    localtime_s(&tm, &t);
    char b[16];
    std::strftime(b, sizeof b, "%Y-%m-%d", &tm);
    return b;
}

Read reader(State &s)
{
    return [&s](uint64_t key) -> std::optional<Rec> {
        if (auto it = s.edits.find(key); it != s.edits.end())
            return it->second;
        if (auto r = s.file.get(key))
            return Rec{r->tag, std::move(r->text)};
        return std::nullopt;
    };
}

std::wstring data_registry()
{
    const std::wstring &st = runtime::state_root();
    return (st.empty() ? runtime::swofty_root() : st) + L"registry";
}

std::string build_profile()
{
    std::string text = runtime::read_file(runtime::under_swofty("registry/index.json"), size_t(16) << 20);
    if (text.empty())
        return {};
    swofty::json::Error err;
    Value doc = swofty::json::parse(text, &err);
    return err.message.empty() ? std::string(doc["profile"].as_string()) : std::string();
}

void load(State &s, registry::Store &st)
{
    if (s.loaded)
        return;
    s.loaded = true;
    declare_venues(st);
    std::string older = runtime::read_file(data_registry() + L"\\older-tracks.json", size_t(16) << 20);
    s.older = parse_older(older, build_profile());
    if (s.older.tracks.empty() && !has_track_records(st)) {
        s.skipped = true;
        return;
    }
    std::wstring path = runtime::game_root() + L"\\game_package\\gamedatabases\\gamedatabases.2025.bin";
    s.ok = s.file.load(runtime::read_file(path));
    if (!s.ok) {
        s.lines.push_back("the game database file is unreadable; no track records are lowered and the base is empty");
        return;
    }
    runtime::db_older_edits([&s](uint64_t key, int tag, const std::string &text) {
        Rec r{tag, text};
        if (r.tag < 0)
            if (auto f = s.file.get(key))
                r.tag = f->tag;
        s.edits[key] = std::move(r);
    });
    Db db(reader(s));
    s.scan = scan(db, builtin_tables());
}

void ensure(registry::Store &st)
{
    State &s = state();
    load(s, st);
    if (s.seeded)
        return;
    s.seeded = true;
    for (auto &l : s.older.lines)
        swofty::log("%s", l.c_str());
    if (!s.ok) {
        std::size_t providers = 0;
        for (auto &b : venue_providers().base) {
            st.add_base(b.registry, b.key, b.value);
            providers++;
        }
        st.stock_loaded("track.venue", true);
        swofty::log("[tracks] %s: %zu venue(s) seeded with %zu stock track(s) providing them, the track base is empty",
                    s.skipped ? "no track records and no tracks made for 1.5, so the game database is not read"
                              : "the game database file is unreadable",
                    std::size(VENUE_SEEDS), providers);
        return;
    }
    Seeds seeded = seeds(s.scan, s.older.tracks);
    declare_seeds(st, seeded);
    std::size_t dormant = 0;
    for (auto &r : s.scan.rows)
        dormant += is_dormant(r.code);
    swofty::log("[tracks] seeded %zu track row(s) (%zu dormant), %zu venue(s), %zu data block(s) in %zu data set(s), "
                "%zu track(s) made for 1.5",
                s.scan.rows.size(), dormant, std::size(VENUE_SEEDS), s.scan.blocks.size(), s.scan.sets.size(),
                s.older.tracks.size());
    for (auto &o : s.older.tracks) {
        std::string how = "the launcher's; made for 1.5 by " + o.addon + (o.venue.empty() ? ", no venue" : ", provides " + o.venue);
        swofty::log("[tracks] %s: stat id %lld (%s)", o.code.c_str(), static_cast<long long>(o.stat_id), how.c_str());
    }
}

const char *tag_name(int tag)
{
    switch (tag) {
    case 0:
        return "int";
    case 1:
        return "float";
    case 2:
        return "string";
    case 3:
        return "enum";
    case 4:
        return "id";
    case 5:
        return "bool";
    case 6:
        return "time";
    case 7:
        return "t7";
    case 8:
        return "t8";
    case 9:
        return "curve";
    case 11:
        return "colour";
    case 12:
        return "t12";
    case 13:
        return "list";
    case 14:
        return "vector";
    case 17:
        return "t17";
    case 18:
        return "t18";
    }
    return "";
}

std::vector<std::string> ui_lists_of(const std::string &code)
{
    const TrackFacts *f = facts().track(code);
    return f ? f->ui_lists : std::vector<std::string>{};
}

StatIds pool_ids(const State &s)
{
    std::string day = today();
    return [&s, day](const std::string &id, int64_t pin) -> StatId {
        if (pin >= 0) {
            if (auto u = s.scan.used.find(pin); u != s.scan.used.end())
                return {-1, "stat id " + std::to_string(pin) + " is the row " + u->second + "'s"};
            for (auto &o : s.older.tracks)
                if (o.stat_id == pin)
                    return {-1, "stat id " + std::to_string(pin) + " is " + o.addon + "/" + o.code + "'s"};
            ids::Result r = ids::store().pin("track", id, pin, day);
            return {r.id, "pinned"};
        }
        ids::Result r = ids::store().allocate("track", id, day);
        if (r.id < 0)
            return {-1, r.line};
        return {r.id, r.how == "allocated"   ? "first free from 254 down"
                      : r.how == "reclaimed" ? "reclaimed in ids.json"
                                             : "kept in ids.json"};
    };
}

class Tracks : public Applier {
public:
    const char *registry() const override { return "track"; }
    Applies applies() const override { return Applies::boot; }

    void prepare(Store &st) override
    {
        std::lock_guard g(mu);
        ensure(st);
    }

    std::size_t database_keys(Store &st) override
    {
        std::lock_guard g(mu);
        State &s = state();
        ensure(st);
        if (!s.ok || s.plan)
            return s.plan ? s.plan->fresh : 0;
        Db db(reader(s));
        s.plan = std::make_unique<Plan>(lower_records(st, db, builtin_tables(), s.scan, s.older.tracks, {}, ui_lists_of));
        std::size_t variants = declare_data_variants(st, *s.plan);
        if (variants)
            swofty::log("[tracks] %zu data block(s) of record tracks declared as variants of their bases' blocks", variants);
        return s.plan->fresh;
    }

    void boot(Store &st, Report &report) override
    {
        std::lock_guard g(mu);
        State &s = state();
        if (s.booted)
            return;
        s.booted = true;
        ensure(st);
        for (auto &l : s.lines)
            report.lines.push_back(l);
        Plan p;
        std::vector<std::string> records = records_in_order(st, s.older.tracks);
        if (s.ok && !records.empty() && !db::staging().available()) {
            report.refused += int(records.size());
            report.lines.push_back("the game database is not reachable at this boot; no track rows applied");
        } else if (s.ok && !records.empty()) {
            ids::store().define_pool("track", track_pool(s.scan.used, s.older.tracks));
            ids::store().set_live("track", records);
            Db db(reader(s));
            p = lower_records(st, db, builtin_tables(), s.scan, s.older.tracks, pool_ids(s), ui_lists_of);
            std::size_t staged = 0;
            for (auto &w : db.writes()) {
                const char *type = tag_name(w.tag);
                if (!*type) {
                    report.lines.push_back(w.path + ": tag " + std::to_string(w.tag) + " stores nothing; not staged");
                    continue;
                }
                staged += db::staging().set(w.owner, "track", w.path, type, w.text);
            }
            report.applied = p.applied;
            report.refused += p.refused;
            for (auto &l : p.lines)
                report.lines.push_back(l);
            report.lines.push_back(std::to_string(staged) + " database record(s) staged");
            for (auto &l : p.stat_lines)
                swofty::log("%s", l.c_str());
            std::string names;
            for (auto &d : p.done) {
                names += (names.empty() ? "" : ", ") + d.code + " " + std::to_string(d.stat_id);
                runtime::udp_track(d.stat_id, d.addon, d.code);
            }
            if (!names.empty())
                report.summary = names;
        }
        std::vector<std::string> venues = st.list("track.venue", "");
        Base b = make_base(s.scan, s.older.tracks, p, venues, [&st](const std::string &v) { return venue_track(st, v); });
        std::size_t older = 0, record = 0;
        for (auto &t : b.tracks) {
            older += t.kind == Kind::older;
            record += t.kind == Kind::record;
        }
        report.lines.push_back("base: " + std::to_string(b.tracks.size()) + " track(s) (" + std::to_string(record) + " record, " +
                               std::to_string(older) + " made for 1.5), " + std::to_string(b.dormant.size()) + " dormant row(s), " +
                               std::to_string(b.venues.size()) + " of " + std::to_string(venues.size()) + " venue(s) provided");
        Base &live = base();
        live.tracks = std::move(b.tracks);
        live.dormant = std::move(b.dormant);
        live.filled = std::move(b.filled);
        live.venues = std::move(b.venues);
        live.ready = true;
        s.plan.reset();
        s.edits.clear();
        s.file.clear();
    }
};

class Venues : public Applier {
public:
    const char *registry() const override { return "track.venue"; }
    Applies applies() const override { return Applies::boot; }

    void boot(Store &st, Report &report) override
    {
        std::lock_guard g(mu);
        ensure(st);
        for (auto &id : st.list("track.venue", "")) {
            std::string track = venue_track(st, id);
            bool core = id.rfind("venue/", 0) == 0;
            if (track.empty() || (core && track.rfind("game/", 0) == 0))
                continue;
            report.applied++;
            if (core)
                report.lines.push_back(id + ": provided by " + track);
        }
    }
};

}

void register_appliers()
{
    static std::once_flag once;
    std::call_once(once, [] {
        make<Tracks>();
        make<Venues>();
    });
}

}
