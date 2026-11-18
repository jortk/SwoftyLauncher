#pragma once

#include "career_rows.hpp"
#include "drs.hpp"
#include "points.hpp"
#include "garage_lower.hpp"
#include "liveries.hpp"
#include "lower.hpp"
#include "rows.hpp"
#include "wear.hpp"
#include "teams.hpp"

#include "../ids.hpp"
#include "../../career_ops/start_teams.hpp"
#include "../../registry/store.hpp"

#include <map>
#include <memory>
#include <set>
#include <string>
#include <tuple>
#include <vector>

// The boot apply of the content registries, shared by the core (appliers.cpp) and the host tools: each registry's
// effective records, by owner in load order, lowered to operations and run through one row engine over the database
// the game loads. A record whose operations fail is taken back whole.

namespace swofty_core::apply::content {

inline constexpr const char *ROW_REGISTRIES[] = {"engine_sound",   "vehicle.package",     "vehicle",         "livery",
                                                 "team.identity",  "team.pit_times",   "team.garage",
                                                 "helmet",         "suit",                "gloves",          "driver.head",
                                                 "driver.look",    "driver.person",       "driver.identity", "driver.stats",
                                                 "wearable.season_set", "rules.drs",   "session.points", "content.package",
                                                 "career.rnd.tree", "career.market"};

inline Lookup store_lookup(registry::Store &st, std::map<std::string, Value> &keep)
{
    return [&st, &keep](const std::string &reg, const std::string &id) -> const Value * {
        std::string k = reg + "|" + id;
        auto it = keep.find(k);
        if (it != keep.end())
            return &it->second;
        auto rec = st.get(reg, id);
        if (rec.status != registry::Status::ok)
            return nullptr;
        return &(keep[k] = rec.value);
    };
}

// The addon records of a registry (stock "game/" records are the game's own rows), by owner in load order.
inline std::vector<std::string> in_load_order(registry::Store &st, const std::string &reg)
{
    std::vector<std::string> ids = st.list(reg, "");
    ids.erase(std::remove_if(ids.begin(), ids.end(), [](const std::string &id) { return id.rfind("game/", 0) == 0; }),
              ids.end());
    std::vector<std::string> order = st.order();
    auto rank = [&](const std::string &id) {
        std::string owner = addon_of(id);
        auto it = std::find(order.begin(), order.end(), owner);
        return it == order.end() ? order.size() : std::size_t(it - order.begin());
    };
    std::stable_sort(ids.begin(), ids.end(), [&](const std::string &a, const std::string &b) {
        auto ra = rank(a), rb = rank(b);
        return ra != rb ? ra < rb : a < b;
    });
    return ids;
}

inline WearCtx wear_context(registry::Store &st, const Lookup &lookup)
{
    WearCtx c;
    for (auto &id : in_load_order(st, "driver.identity")) {
        const Value *rec = lookup("driver.identity", id);
        if (!rec)
            continue;
        std::string label = label_of(lookup, "driver.identity", id);
        if ((*rec)["look"].is_string() && !c.look_driver.count(std::string((*rec)["look"].as_string())))
            c.look_driver[std::string((*rec)["look"].as_string())] = label;
        if ((*rec)["person"].is_string())
            c.person_driver[person_key((*rec)["person"].as_string())] = id;
    }
    for (auto &id : in_load_order(st, "driver.stats")) {
        const Value *rec = lookup("driver.stats", id);
        if (rec && (*rec)["driver"].is_string())
            c.stats_of.insert(std::string((*rec)["driver"].as_string()));
    }
    return c;
}

// A variant writes rows of its own and never its base's. Every value that names a row and that the variant only
// inherits from its base is made its own: an explicit stat_id counts as "auto"; a vehicle's label becomes the
// variant's key and its audio switch its label; an engine sound's bundle file is the base's, whose row it uses as
// the game's; a package's database block names the rows of the variant's own entrant list and no scenario id.
inline Value own_value(registry::Store &st, const std::string &reg, const std::string &id, const Value &rec)
{
    std::vector<std::string> chain = st.chain(reg, id);
    if (chain.size() < 2 || chain[0] != id)
        return rec;
    registry::Record found = st.get(reg, chain[1]);
    if (found.status != registry::Status::ok)
        return rec;
    const Value &base = found.value;
    Value v = rec;
    auto inherited = [&](std::string_view field) { return !rec[field].is_null() && rec[field] == base[field]; };
    if (rec["stat_id"].is_int() && inherited("stat_id"))
        v["stat_id"] = Value("auto");
    if (reg == "vehicle") {
        if (!rec["label"].is_string() || inherited("label"))
            v["label"] = Value(key_of(id));
        if (rec["audio_switch"].is_string() && inherited("audio_switch"))
            v["audio_switch"] = Value(std::string(static_cast<const Value &>(v)["label"].as_string()));
        std::string own(static_cast<const Value &>(v)["label"].as_string());
        std::string from = base["label"].is_string() ? std::string(base["label"].as_string()) : key_of(chain[1]);
        if (own != from)
            v["team_from"] = Value(from);
    } else if (reg == "engine_sound") {
        if (rec["bundle"].is_string() && inherited("bundle"))
            v["bundle"] = Value::object({{"stock", Value("audio/2025/bundles/" + bundle_name(rec) + ".bdl")}});
    } else if (is_wear_registry(reg)) {
        const char *base_field = reg == "driver.identity" ? "donor" : reg == "driver.head" ? "rig_from" : "from";
        if (reg != "driver.person" && reg != "driver.stats" && reg != "wearable.season_set")
            v[base_field] = Value(chain[1]);
        if (inherited("label"))
            v["label"] = Value(key_of(id));
    } else if (reg == "team.identity") {
        if (!rec["donor"].is_string())
            v["donor"] = Value(chain[1]);
    } else if (reg == "content.package") {
        Value db = rec["database"];
        if (db.is_object() && inherited("database") && !inherited("entrant_list")) {
            std::string list(rec["entrant_list"].as_string());
            db["entrant_list"] = Value(list);
            db["entrant_prefix"] = Value(list + "-");
            db["standings"] = Value(list + "_teams");
            db.erase("scenario_stat_id");
            v["database"] = std::move(db);
        }
    }
    return v;
}

// The store's effective records, each made its own (own_value).
inline Lookup own_lookup(registry::Store &st, std::map<std::string, Value> &keep)
{
    return [&st, &keep](const std::string &reg, const std::string &id) -> const Value * {
        std::string k = reg + "|" + id;
        auto it = keep.find(k);
        if (it != keep.end())
            return &it->second;
        auto rec = st.get(reg, id);
        if (rec.status != registry::Status::ok)
            return nullptr;
        return &(keep[k] = own_value(st, reg, id, rec.value));
    };
}

inline BaseImage box_base_image(registry::Store &st, const Lookup &lookup)
{
    return [&st, lookup](const std::string &package) -> std::string {
        std::vector<std::string> chain = st.chain("content.package", package);
        std::vector<std::string> boxes = in_load_order(st, "era.box");
        for (std::size_t i = 1; i < chain.size(); i++)
            for (auto &id : boxes) {
                const Value *rec = lookup("era.box", id);
                if (!rec || (*rec)["package"].as_string() != chain[i])
                    continue;
                if ((*rec)["image"].is_string() && !(*rec)["image"].as_string().empty())
                    return std::string((*rec)["image"].as_string());
                const Value *base = lookup("content.package", chain[i]);
                if (base && (*base)["entrant_list"].is_string())
                    return "#overrides.home_screen.tiles_solo_6x6[" + std::string((*base)["entrant_list"].as_string()) + "]";
            }
        return {};
    };
}

inline std::vector<PoolScan> scan_all_pools(const std::function<std::optional<std::string>(const std::string &)> &text)
{
    auto out = scan_pools(text);
    for (auto &s : scan_livery_pools(text))
        out.push_back(std::move(s));
    for (auto &s : scan_wear_pools(text))
        out.push_back(std::move(s));
    return out;
}

struct Pin {
    std::string registry, id;
    int64_t stat_id;
    std::string label;
};

// The label of the row a record's stat id goes on, as its lowering names it.
inline std::string row_label(const Lookup &lookup, std::string_view reg, const std::string &id, const Value &rec)
{
    if (reg == "content.package")
        return rec["entrant_list"].is_string() ? std::string(rec["entrant_list"].as_string()) : std::string();
    if (reg == "wearable.season_set")
        return rec["team"].is_string() ? std::to_string(rec["season"].as_int()) + "-" +
                                             label_of(lookup, "team.identity", rec["team"].as_string())
                                       : std::string();
    if (reg == "team.identity" || reg == "vehicle.package")
        return key_of(id);
    return record_label(lookup, std::string(reg).c_str(), id);
}

// Every content record that takes a stat id, with its row's label: its explicit stat id, or -1 for "auto", and the
// scenario ids of packages whose database block is their own.
inline std::vector<Pin> stat_records(registry::Store &st)
{
    std::vector<Pin> out;
    std::set<std::string> boards;
    std::map<std::string, Value> keep;
    Lookup lookup = own_lookup(st, keep);
    for (const char *reg : {"vehicle.package", "vehicle", "livery", "helmet", "suit", "gloves", "driver.head", "driver.look",
                            "driver.identity", "wearable.season_set", "content.package", "team.identity"})
        for (auto &id : in_load_order(st, reg)) {
            const Value *rec = lookup(reg, id);
            if (!rec)
                continue;
            std::string label = row_label(lookup, reg, id, *rec);
            out.push_back({reg, id, pin_of(*rec), label});
            if (std::string_view(reg) != "content.package")
                continue;
            const Value &db = (*rec)["database"];
            bool mine = !db["entrant_list"].is_string() || db["entrant_list"].as_string() == (*rec)["entrant_list"].as_string();
            if (mine && db["scenario_stat_id"].is_int())
                out.push_back({SCENARIO_IDS, id, db["scenario_stat_id"].as_int(), label});
            else if (!label.empty())
                out.push_back({SCENARIO_IDS, id, -1, label});
            if (auto board = board_of(id, *rec); board && !board->stock && !boards.count(board->label)) {
                boards.insert(board->label);
                out.push_back({LEADERBOARD_IDS, board->label, -1, board->label});
            }
        }
    return out;
}

// Every explicit stat id the content records ship: vehicles, vehicle packages, entrant lists and the scenario ids
// of packages whose database block is their own.
inline std::vector<Pin> pins(registry::Store &st)
{
    std::vector<Pin> out;
    for (auto &p : stat_records(st))
        if (p.stat_id >= 0)
            out.push_back(std::move(p));
    return out;
}

// The stat ids the rows of the database the game loads hold (the game's own and the older addons' database
// operations') are held in the store. A record pinned on one of them, or kept on one in ids.json, for a row with
// another label takes the next free id of its pool. The pins that land on nothing are placed first, so a moved
// record never takes another record's pin. Returns the lines to log.
inline std::vector<std::string> settle_ids(ids::Store &ids, registry::Store &st,
                                           const std::map<std::string, PoolScan> &scans, const std::string &day)
{
    for (auto &[reg, scan] : scans)
        ids.hold(reg, scan.used);
    std::vector<std::string> lines;
    std::vector<Pin> later;
    for (auto &p : stat_records(st)) {
        if (p.stat_id < 0 || ids.database_holds(p.registry, p.stat_id, p.label)) {
            later.push_back(std::move(p));
            continue;
        }
        ids::Result had = ids.get(p.registry, p.id);
        ids::Result r = ids.pin(p.registry, p.id, p.stat_id, day, p.label);
        if (had.id != p.stat_id || r.line.find("also holds") != std::string::npos)
            lines.push_back(r.line);
    }
    for (auto &p : later) {
        ids::Result r = p.stat_id >= 0 ? ids.pin(p.registry, p.id, p.stat_id, day, p.label)
                                       : ids.recheck(p.registry, p.id, p.label, day);
        if (!r.line.empty())
            lines.push_back(r.line);
    }
    return lines;
}

struct Expansion {
    explicit Expansion(rows::Base base) : engine(std::move(base)) {}

    rows::Engine engine;
    std::map<std::string, std::size_t> fresh, addon_fresh;
    std::map<std::string, std::vector<std::string>> lines;
    std::map<std::string, int> records, refused;
    std::map<std::string, std::vector<std::string>> applied;
    std::vector<Redirect> redirects;
    std::vector<Group> groups;
    std::vector<Folder> folders;
    std::vector<Owned> owned;
    std::vector<LogoImage> logos;
    std::vector<GarageArt> garages;
    std::map<std::string, std::vector<std::pair<std::string, std::string>>> texts;
    std::set<std::string> owners;
    std::map<std::string, std::map<std::string, int>> left_out;
    std::vector<std::string> virtual_lists;
};

// The runtime twin of the build's virtual-list step (packages/core/src/virtual_lists.rs) for content.package lists
// with "virtual_list": true. After every record is lowered: adds the host list swofty_virtual_list (a copy of the
// first virtual list, offered under swofty_f1) when no row has that label, then moves each virtual list's rows to
// entrant_list_10000 and up, puts the table's last row in its place and lowers the count. Only rows this engine
// wrote move; a list whose row or the table's last row is the game's own stays and is logged.
inline void settle_virtual_lists(Expansion &x, const StatIds &stat)
{
    if (x.virtual_lists.empty())
        return;
    auto &e = x.engine;
    auto &lines = x.lines["content.package"];
    const std::string row = std::string(LISTS) + "_";
    const std::string count_path = "entrants.entrant_lists._entrant_list_instancecount_";
    auto count = [&]() -> int64_t {
        auto t = e.text(count_path);
        return t ? std::strtoll(t->c_str(), nullptr, 10) : -1;
    };
    auto label_at = [&](int64_t i) { return e.text(row + std::to_string(i) + "._metadata_"); };
    int64_t n = count();
    if (n < 0)
        return;
    bool host = false;
    for (int64_t i = 0; i < n && !host; i++)
        host = label_at(i) == std::string(VIRTUAL_HOST_LIST);
    if (!host) {
        e.attribute("swofty.core", "content.package", VIRTUAL_HOST_LIST);
        Value set = Value::object({{"_metadata_", VIRTUAL_HOST_LIST}});
        if (auto id = stat_of(stat, "content.package", std::string("swofty.core/") + VIRTUAL_HOST_LIST, -1))
            set["stat_id"] = std::to_string(*id);
        std::size_t mark = e.mark();
        std::string why = e.apply(add_row("entrants.entrant_lists",
                                          std::string(LISTS) + "[_metadata_=" + x.virtual_lists.front() + "]", set));
        for (const char *field : {"entrant_lists", "selectable_entrant_lists"})
            if (why.empty())
                why = e.apply(append_op(std::string(SETUP_ROW) + "[_metadata_=swofty_f1]", field, VIRTUAL_HOST_LIST));
        for (const char *cs : {"2025", "2026"}) {
            std::string sel = std::string("content_sets.content_set[_metadata_=") + cs + "].available_entrant_lists_0";
            if (why.empty() && e.resolve(sel))
                why = e.apply(append_op(sel, "entrant_lists", VIRTUAL_HOST_LIST));
        }
        if (!why.empty()) {
            e.rollback(mark);
            lines.push_back(std::string("virtual lists: the host ") + VIRTUAL_HOST_LIST + " could not be added (" + why +
                            "); virtual boxes stay hidden");
        } else {
            e.commit();
            lines.push_back(std::string("virtual lists: host ") + VIRTUAL_HOST_LIST + " added");
        }
    }
    int64_t next = 10000;
    while (label_at(next))
        next++;
    std::size_t moved = 0;
    for (auto &label : x.virtual_lists) {
        n = count();
        int64_t i = -1;
        for (int64_t k = 0; k < n && i < 0; k++)
            if (label_at(k) == label)
                i = k;
        if (i < 0) {
            lines.push_back("virtual list " + label + ": no counted row has this label; nothing moved");
            continue;
        }
        std::string from = row + std::to_string(i), last = row + std::to_string(n - 1), to = row + std::to_string(next);
        if (!e.wrote_fresh(from + "._metadata_") || (i != n - 1 && !e.wrote_fresh(last + "._metadata_"))) {
            lines.push_back("virtual list " + label + ": its row or the table's last row is the game's own; nothing moved");
            continue;
        }
        std::size_t keys = e.move_row(from, to);
        if (i != n - 1)
            e.move_row(last, from);
        auto tag = e.get(gvdb::fnv64(count_path));
        e.put_text(count_path, tag ? tag->tag : 0, std::to_string(n - 1));
        next++;
        moved++;
        lines.push_back("virtual list " + label + ": " + std::to_string(keys) + " key(s) moved from " + from + " to " + to);
    }
    lines.push_back("virtual lists: " + std::to_string(moved) + " moved above the count (" + std::to_string(count()) +
                    " lists left for the game)");
}

inline TeamRefs refs_of(registry::Store &st, const Lookup &lookup)
{
    std::vector<std::pair<std::string, const Value *>> idents, ctors, pits;
    for (auto &id : in_load_order(st, "team.identity"))
        if (const Value *r = lookup("team.identity", id))
            idents.push_back({id, r});
    for (auto &id : in_load_order(st, "team.constructor"))
        if (const Value *r = lookup("team.constructor", id))
            ctors.push_back({id, r});
    for (auto &id : in_load_order(st, "team.pit_times"))
        if (const Value *r = lookup("team.pit_times", id))
            pits.push_back({id, r});
    return team_refs(idents, ctors, pits);
}

// The vehicles that wear each livery: every vehicle record, stock or an addon's, whose effective livery names it.
inline std::map<std::string, std::vector<std::string>> wearers(registry::Store &st)
{
    std::map<std::string, std::vector<std::string>> out;
    for (auto &vid : st.list("vehicle", "")) {
        registry::Record r = st.get("vehicle", vid);
        if (r.status == registry::Status::ok && r.value["livery"].is_string())
            out[std::string(r.value["livery"].as_string())].push_back(vid);
    }
    return out;
}

// The drivers of the added teams (career_ops/start_teams.hpp added_team_id) in each data group's effective
// career.season records, by driver identity: where each one races.
struct AddedSeat {
    std::string group, team;
    int year = 0, slot = 0;
};

inline std::map<std::string, std::vector<AddedSeat>> added_seats(registry::Store &st)
{
    std::map<std::string, std::vector<AddedSeat>> out;
    for (auto &id : st.list("career.season", "")) {
        registry::Record r = st.get("career.season", id);
        if (r.status != registry::Status::ok)
            continue;
        std::string group(r.value["data_group"].as_string());
        const auto *teams = r.value["teams"].if_array();
        if (group.empty() || !teams)
            continue;
        for (auto &t : *teams) {
            std::string team(t["team"].as_string());
            if (!career_ops::teams::added_team_id(team))
                continue;
            for (std::size_t k = 0; k < 2; k++) {
                std::string driver(t["seats"][k].as_string());
                if (!driver.empty())
                    out[driver].push_back({group, team, int(r.value["year"].as_int()), int(t["slot"].as_int())});
            }
        }
    }
    return out;
}

// A driver row the core makes (a driver.identity with a donor) has no entrant row, so a career has no line-up row
// for him and apply_teams can't seat him. Each such driver of an added team gets one teamless entrant (team "",
// team_position -1, his look and number) and an entry for it in the first list the data group's career list
// references whose entries are teamless (a driver pool), next to the pool's own entries: the career copies it at
// creation like every pool driver. The career list is the one the data group's career setup names
// (ComCareerSetup_0.entrant_list), else the list named as the data group.
inline void lower_seat_entrants(registry::Store &st, Expansion &x, const Lookup &lookup, const Read &read,
                                const std::set<std::string> &skip)
{
    auto &lines = x.lines["driver.identity"];
    for (auto &[driver_id, where] : added_seats(st)) {
        const Value *rec = lookup("driver.identity", driver_id);
        if (!rec || !(*rec)["donor"].is_string() || skip.count(addon_of(driver_id)))
            continue;
        std::string label = label_of(lookup, "driver.identity", driver_id);
        if (!read(row_at(DRIVERS, "driver", label) + "._metadata_"))
            continue;
        std::map<std::string, const AddedSeat *> groups;
        for (auto &w : where)
            groups.emplace(w.group, &w);
        for (auto &[group, seat] : groups) {
            std::string why_seat = "a seat of the added team " + seat->team + " in " + std::to_string(seat->year) + " slot " +
                                   std::to_string(seat->slot) + " of " + group;
            std::string career = read("career.career_setup.CareerSetupDataGroup[_metadata_=" + group + "].ComCareerSetup_0.entrant_list")
                                     .value_or(std::string());
            if (career.empty() && read(std::string(LISTS) + "[_metadata_=" + group + "]._metadata_"))
                career = group;
            std::string pool, from_entrant;
            std::string refs = career.empty() ? std::string()
                                              : read(std::string(LISTS) + "[_metadata_=" + career + "].entrant_list_references").value_or("");
            std::size_t at = 0;
            while (pool.empty() && at <= refs.size()) {
                std::size_t comma = refs.find(',', at);
                std::string ref = refs.substr(at, comma == std::string::npos ? std::string::npos : comma - at);
                at = comma == std::string::npos ? refs.size() + 1 : comma + 1;
                std::string first = std::string(LISTS) + "[_metadata_=" + ref + "].entrant_0";
                auto team = read(first + ".team");
                auto pos = read(first + ".team_position");
                auto entrant = read(first + ".entrant_reference");
                if (!ref.empty() && team && team->empty() && pos && *pos == "-1" && entrant && !entrant->empty() &&
                    read(std::string(ENTRANTS) + "[_metadata_=" + *entrant + "]._metadata_")) {
                    pool = ref;
                    from_entrant = *entrant;
                }
            }
            if (pool.empty()) {
                lines.push_back("driver.identity " + driver_id + ": no entrant row (" + why_seat + "): " +
                                (career.empty() ? "the data group has no career list"
                                                : "no list " + career + " references holds teamless entries to sit next to") +
                                "; apply_teams can't seat him");
                continue;
            }
            std::string entrant = "sw-" + label, entry = "sw_pool_" + label;
            std::string list = std::string(LISTS) + "[_metadata_=" + pool + "]";
            if (read(list + ".entrant[_metadata_=" + entry + "]._metadata_"))
                continue;
            Plan plan;
            plan.owner = addon_of(driver_id);
            std::string look = (*rec)["look"].is_string() ? label_of(lookup, "driver.look", (*rec)["look"].as_string()) : std::string();
            std::string number = (*rec)["number"].is_int() && (*rec)["number"].as_int() > 0 ? std::to_string((*rec)["number"].as_int()) : "255";
            if (!read(std::string(ENTRANTS) + "[_metadata_=" + entrant + "]._metadata_"))
                push_row(plan, "entrants.entrant_lists.entrants", std::string(ENTRANTS) + "[_metadata_=" + from_entrant + "]",
                         Value::object({{"_metadata_", entrant},
                                        {"driver", label},
                                        {"driver_customisation", look},
                                        {"car_number", number},
                                        {"team", ""},
                                        {"team_position", "-1"},
                                        {"vehicle", ""},
                                        {"vehicle_customisation", ""},
                                        {"badge_customisation", ""},
                                        {"entrant_reference", ""}}));
            push_row(plan, list, list + ".entrant_0",
                     Value::object({{"_metadata_", entry},
                                    {"entrant_reference", entrant},
                                    {"team", ""},
                                    {"vehicle", ""},
                                    {"team_position", "-1"},
                                    {"car_number", "255"},
                                    {"badge_customisation", ""},
                                    {"vehicle_customisation", ""},
                                    {"driver", ""},
                                    {"driver_customisation", ""}}));
            x.engine.add_fields(plan.fields);
            x.engine.attribute(plan.owner, "driver.identity", driver_id);
            std::size_t mark = x.engine.mark();
            std::string why;
            for (std::size_t i = 0; i < plan.ops.size() && why.empty(); i++)
                why = x.engine.apply(plan.ops[i]);
            if (!why.empty()) {
                x.engine.rollback(mark);
                lines.push_back("driver.identity " + driver_id + ": no entrant row (" + why_seat + "): " + why);
                continue;
            }
            x.engine.commit();
            lines.push_back("driver.identity " + driver_id + ": entrant " + entrant + " in " + pool + " (teamless, look " +
                            (look.empty() ? std::string("none") : look) + ", referenced by " + career + "): " + why_seat);
        }
    }
}

// Classic Seasons and the SWOFTY seasons both pick a car's sound switch as a case variant of its slot's stock label,
// numbered by the season, so two packs of one year can name the same switch. The game keeps one audio_car_data row
// per switch name and the car's archive names the switch, so the later car shares the earlier car's sound rows: it
// drops its own instead of being left out, which would leave every entrant list that names it (Progressive's season
// lists, built by database operations) pointing at a car row the game can't find, and the game crashes at boot.
inline void share_taken_switch(const std::string &id, const Value &rec, Plan &plan, const rows::Engine &engine)
{
    if (!rec["audio_switch"].is_string())
        return;
    std::string sw(rec["audio_switch"].as_string());
    std::string prefix = std::string(AUDIO_CARS) + "." + sw + ".";
    auto ours = [&](const Value &op) {
        return op["op"].as_string() == "put-record" && op["key"].as_string().starts_with(prefix);
    };
    std::string taker;
    for (std::size_t i = 0; i < plan.ops.size() && taker.empty(); i++)
        if (ours(plan.ops[i])) {
            std::string by = engine.creator(std::string(plan.ops[i]["key"].as_string()));
            if (by != id)
                taker = by;
        }
    if (taker.empty())
        return;
    Value kept = Value::array();
    std::vector<std::string> owners;
    std::size_t dropped = 0;
    for (std::size_t i = 0; i < plan.ops.size(); i++) {
        if (ours(plan.ops[i])) {
            dropped++;
            continue;
        }
        kept.push_back(plan.ops[i]);
        owners.push_back(i < plan.owners.size() ? plan.owners[i] : plan.owner);
    }
    plan.ops = std::move(kept);
    plan.owners = std::move(owners);
    plan.keys -= std::min(plan.keys, dropped);
    plan.lines.push_back("vehicle " + id + ": sound switch " + sw + " is " + taker +
                         "'s, so the car keeps its row and plays that car's sound (" + std::to_string(dropped) +
                         " audio records not written)");
}

inline void expand(registry::Store &st, Expansion &x, const StatIds &stat, const std::set<std::string> &skip = {})
{
    std::map<std::string, Value> keep;
    Lookup lookup = own_lookup(st, keep);
    Source source = [&x](const std::string &key) -> std::optional<Stock> {
        auto r = x.engine.get(gvdb::fnv64(key));
        if (!r)
            return std::nullopt;
        return Stock{r->tag, r->text};
    };
    Read read = [&x](const std::string &path) -> std::optional<std::string> {
        auto p = x.engine.resolve(path);
        return p ? x.engine.text(*p) : std::nullopt;
    };
    auto worn = wearers(st);
    WearCtx wear = wear_context(st, lookup);
    TeamRefs refs = refs_of(st, lookup);
    std::map<std::string, std::pair<std::string, std::string>> vehicle_tower;
    for (auto &pid : in_load_order(st, "content.package")) {
        const Value *pkg = lookup("content.package", pid);
        if (!pkg)
            continue;
        const Value &entries = (*pkg)["teams"];
        for (std::size_t t = 0; t < entries.size(); t++) {
            if (!entries[t]["vehicle"].is_string() || !entries[t]["team"].is_string())
                continue;
            std::string team(entries[t]["team"].as_string());
            const Value *ident = lookup("team.identity", team);
            if (!ident)
                continue;
            const Value *logo = (*ident)["logo"].is_string() ? lookup("team.logo", std::string((*ident)["logo"].as_string())) : nullptr;
            TowerBadge badge = tower_badge(logo, label_of(lookup, "team.identity", team));
            if (!badge.key.empty())
                vehicle_tower.emplace(std::string(entries[t]["vehicle"].as_string()), std::make_pair(team, badge.key));
        }
    }
    int slot = 0;
    Dropped dropped;
    for (const char *reg : ROW_REGISTRIES) {
        auto &lines = x.lines[reg];
        for (auto &id : in_load_order(st, reg)) {
            const Value *rec = lookup(reg, id);
            if (!rec)
                continue;
            std::string owner = addon_of(id);
            if (skip.count(owner))
                continue;
            Plan plan;
            std::string r = reg;
            if (r == "engine_sound")
                lower_engine_sound(id, *rec, plan);
            else if (r == "vehicle.package")
                lower_vehicle_package(id, *rec, plan, stat);
            else if (r == "vehicle") {
                lower_vehicle(id, *rec, lookup, source, plan, stat, &dropped);
                share_taken_switch(id, *rec, plan, x.engine);
                auto linked = vehicle_tower.find(id);
                if (plan.ops.size() == 0 && plan.refused) {
                } else if (linked == vehicle_tower.end())
                    lower_vehicle_team(*rec, read, plan);
                else
                    lower_vehicle_team(*rec, read, plan, linked->second.first, linked->second.second);
            }
            else if (r == "livery")
                lower_livery(id, *rec, worn[id], lookup, read, plan, stat, &slot);
            else if (r == "helmet" || r == "suit" || r == "gloves")
                lower_item(item_kind(r), id, *rec, lookup, read, plan, stat, wear);
            else if (r == "driver.head")
                lower_head(id, *rec, lookup, read, plan, stat);
            else if (r == "driver.look")
                lower_look(id, *rec, lookup, read, plan, stat, wear);
            else if (r == "driver.person")
                lower_person(id, *rec, lookup, wear, plan);
            else if (r == "driver.identity")
                lower_identity(id, *rec, lookup, read, plan, stat, wear);
            else if (r == "driver.stats")
                lower_stats(id, *rec, lookup, read, plan);
            else if (r == "wearable.season_set")
                lower_season_set(id, *rec, lookup, read, plan, stat);
            else if (r == "team.identity")
                lower_team(id, *rec, lookup, read, refs, plan, stat);
            else if (r == "team.pit_times")
                lower_pit_times(id, *rec, lookup, read, refs, plan);
            else if (r == "team.garage")
                lower_garage(id, *rec, lookup, read, plan);
            else if (r == "rules.drs")
                lower_drs_rule(id, *rec, plan);
            else if (r == "session.points")
                lower_session_points(id, *rec, plan);
            else if (r == "career.rnd.tree")
                lower_rnd_tree(id, *rec, read, plan);
            else if (r == "career.market")
                lower_market(id, *rec, lookup, read, refs, plan);
            else
                lower_package(id, *rec, lookup, plan, stat, &wear.person_driver, read, &refs.constructor_team, &dropped);
            x.engine.add_fields(plan.fields);
            x.engine.attribute(owner, reg, id);
            std::size_t mark = x.engine.mark();
            std::string why;
            for (std::size_t i = 0; i < plan.ops.size() && why.empty(); i++)
                why = x.engine.apply(plan.ops[i]);
            bool nothing = plan.ops.size() == 0 && plan.groups.empty() && plan.folders.empty();
            if (!why.empty() || (plan.refused && nothing)) {
                x.engine.rollback(mark);
                if (r == "vehicle.package")
                    dropped.insert(id);
                if (r == "vehicle" || r == "team.identity" ||
                    (r == "content.package" && !dropped_ref(*rec, &dropped).empty()))
                    x.left_out[owner][r]++;
                if (r == "vehicle" || r == "team.identity")
                    dropped.insert(drop_key(reg, id));
                x.refused[reg]++;
                lines.push_back(std::string(reg) + " " + id + ": not applied: " +
                                (why.empty() ? (plan.lines.empty() ? std::string("it lowers to nothing")
                                                                   : plan.lines.back())
                                             : why));
                continue;
            }
            x.engine.commit();
            x.virtual_lists.insert(x.virtual_lists.end(), plan.virtual_lists.begin(), plan.virtual_lists.end());
            x.records[reg]++;
            x.applied[reg].push_back(id);
            x.owners.insert(owner);
            for (auto &l : plan.lines)
                lines.push_back(l);
            for (auto &rd : plan.redirects)
                x.redirects.push_back(rd);
            x.groups.insert(x.groups.end(), plan.groups.begin(), plan.groups.end());
            x.folders.insert(x.folders.end(), plan.folders.begin(), plan.folders.end());
            x.owned.insert(x.owned.end(), plan.owned.begin(), plan.owned.end());
            x.logos.insert(x.logos.end(), plan.logos.begin(), plan.logos.end());
            x.garages.insert(x.garages.end(), plan.garages.begin(), plan.garages.end());
            x.texts[reg].insert(x.texts[reg].end(), plan.texts.begin(), plan.texts.end());
            if (plan.refused && (r == "livery" || is_wear_registry(r)))
                x.refused[reg] += int(plan.refused);
        }
    }
    lower_seat_entrants(st, x, lookup, read, skip);
    settle_virtual_lists(x, stat);
    for (auto &l : x.engine.lines)
        x.lines["content.package"].push_back(l);
    for (auto &w : x.engine.writes())
        if (w.fresh) {
            x.fresh[w.registry]++;
            x.addon_fresh[w.owner]++;
        }
}

// Whole addons in load order while their new keys fit the room; the rest are left out before anything is
// expanded, so the rows of the addons after a refused one stay contiguous.
struct Admission {
    std::set<std::string> skip;
    std::vector<std::pair<std::string, std::size_t>> refused;
};

inline Admission admit(const std::vector<std::string> &order, const std::map<std::string, std::size_t> &fresh,
                       uint64_t room)
{
    Admission a;
    uint64_t left = room;
    for (auto &addon : order) {
        auto it = fresh.find(addon);
        if (it == fresh.end() || !it->second)
            continue;
        if (it->second <= left) {
            left -= it->second;
            continue;
        }
        a.skip.insert(addon);
        a.refused.push_back({addon, it->second});
    }
    return a;
}

// The expansion counts each addon's new keys once its rows are laid out. Every addon still in is charged what it
// counted; one past the room is left out whole as well, so the rows are laid out again without it and stay
// contiguous. An addon left out before stays out, and keys of owners outside the load order are charged first.
inline Admission readmit(const std::vector<std::string> &order, const std::map<std::string, std::size_t> &counted,
                         const Admission &before, uint64_t room)
{
    Admission a = before;
    uint64_t left = room;
    for (auto &[owner, keys] : counted)
        if (std::find(order.begin(), order.end(), owner) == order.end())
            left = keys < left ? left - keys : 0;
    for (auto &addon : order) {
        auto it = counted.find(addon);
        if (a.skip.count(addon) || it == counted.end() || !it->second)
            continue;
        if (it->second <= left) {
            left -= it->second;
            continue;
        }
        a.skip.insert(addon);
        a.refused.push_back({addon, it->second});
    }
    return a;
}

// The plan's stat ids: a record's own pin, else a distinct id per registry, so the plan lowers every record the
// boot can give an id and counts its rows.
inline StatIds planning_ids()
{
    auto next = std::make_shared<std::map<std::string, int64_t>>();
    return [next](const std::string &registry, const std::string &, int64_t pin) -> int64_t {
        return pin >= 0 ? pin : (*next)[registry]++;
    };
}

inline const char *tag_name(int tag)
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

}
