#pragma once

#include "base.hpp"
#include "seed.hpp"

#include "../../registry/store.hpp"

#include <algorithm>
#include <functional>
#include <map>
#include <set>
#include <string>
#include <vector>

// The boot pass of the track registries, shared by the core (boot.cpp) and the host tests: the seeds go into the
// store, every addon track record in load order is lowered over the database (a record whose writes fail is taken
// back whole), its data blocks become variants of its base's, and the effective base (contract C4) is built from the
// rows, the older tracks, the records and the venues' resolution.

namespace swofty_core::apply::tracks {

inline std::string owner_of(std::string_view id)
{
    auto slash = id.find('/');
    return slash == std::string_view::npos ? std::string() : std::string(id.substr(0, slash));
}

inline std::string key_part(std::string_view id)
{
    auto slash = id.find('/');
    return slash == std::string_view::npos ? std::string(id) : std::string(id.substr(slash + 1));
}

inline void declare_venues(registry::Store &st)
{
    for (auto &b : venue_bases())
        st.add_base(b.registry, b.key, b.value);
}

inline bool has_track_records(registry::Store &st)
{
    static const char *const registries[] = {"track", "track.venue", "track.data", "track.per_package"};
    std::vector<std::string> addons = st.order();
    for (const char *reg : registries) {
        if (!st.list(reg, "").empty())
            return true;
        for (auto &a : addons)
            if (st.layer_count(a, reg))
                return true;
    }
    return false;
}

inline void declare_seeds(registry::Store &st, const Seeds &s)
{
    for (auto &b : s.base)
        st.add_base(b.registry, b.key, b.value);
    for (auto &[addon, seed] : s.older)
        st.put_older(addon, seed.registry, seed.key, &seed.value, "older-tracks.json");
    st.stock_loaded("track", true);
    st.stock_loaded("track.venue", true);
    st.stock_loaded("track.data", true);
}

struct StatId {
    int64_t id = -1;
    std::string how;
};

using StatIds = std::function<StatId(const std::string &id, int64_t pin)>;

struct Done {
    std::string id, code, base, slot, addon, name;
    int64_t stat_id = -1;
    std::vector<std::string> provides, sets;
    bool new_row = false;
};

struct Plan {
    std::vector<Done> done;
    std::vector<std::string> lines, stat_lines;
    std::map<std::string, std::string> filled;
    std::size_t fresh = 0;
    std::map<std::string, std::size_t> addon_fresh;
    int applied = 0, refused = 0;
};

// The addon track records in load order (stock game/ rows and the older tracks' records are not lowered).
inline std::vector<std::string> records_in_order(registry::Store &st, const std::vector<Older> &older)
{
    std::set<std::string> skip;
    for (auto &o : older)
        skip.insert(o.addon + "/" + o.code);
    std::vector<std::string> ids;
    for (auto &id : st.list("track", ""))
        if (id.rfind("game/", 0) != 0 && !skip.count(id))
            ids.push_back(id);
    std::vector<std::string> order = st.order();
    auto rank = [&](const std::string &id) {
        auto it = std::find(order.begin(), order.end(), owner_of(id));
        return it == order.end() ? order.size() : std::size_t(it - order.begin());
    };
    std::stable_sort(ids.begin(), ids.end(), [&](const std::string &a, const std::string &b) {
        auto ra = rank(a), rb = rank(b);
        return ra != rb ? ra < rb : a < b;
    });
    return ids;
}

inline std::vector<std::string> strings_of(const Value &v)
{
    std::vector<std::string> out;
    if (auto a = v.if_array())
        for (auto &x : *a)
            if (x.is_string())
                out.emplace_back(x.as_string());
    return out;
}

using UiLists = std::function<std::vector<std::string>(const std::string &code)>;

inline Plan lower_records(registry::Store &st, Db &db, const Tables &t, const Scan &scan, const std::vector<Older> &older,
                          const StatIds &stat, const UiLists &ui_lists = {})
{
    Plan p;
    std::map<std::string, std::string> code_of;
    std::set<std::string> codes;
    for (auto &r : scan.rows) {
        code_of["game/" + r.code] = r.code;
        codes.insert(r.code);
    }
    for (auto &o : older) {
        code_of[o.addon + "/" + o.code] = o.code;
        if (!o.slot.empty())
            p.filled[o.slot] = o.addon + "/" + o.code;
    }
    for (auto &id : records_in_order(st, older)) {
        registry::Record got = st.get("track", id);
        if (got.status != registry::Status::ok)
            continue;
        const Value &rec = got.value;
        std::string addon = owner_of(id);
        std::string code(rec["code"].as_string());
        std::string base_id(rec["base"].as_string()), slot_id(rec["slot"].as_string());
        auto refuse = [&](const std::string &why) {
            p.refused++;
            p.lines.push_back("track " + id + ": " + why + "; refused");
        };
        if (code.empty()) {
            refuse("no code");
            continue;
        }
        auto from = code_of.find(base_id);
        if (base_id.empty() || from == code_of.end()) {
            refuse(base_id.empty() ? std::string("no base") : "its base " + base_id + " is not a track in the base");
            continue;
        }
        std::string slot_code;
        if (!slot_id.empty()) {
            auto s = code_of.find(slot_id);
            if (s == code_of.end()) {
                refuse("its slot " + slot_id + " is not a track row");
                continue;
            }
            if (auto f = p.filled.find(slot_id); f != p.filled.end()) {
                p.refused++;
                p.lines.push_back("[registry] track " + id + ": slot " + slot_id + " is already filled by " + f->second +
                                  "; refused");
                continue;
            }
            slot_code = s->second;
        }
        bool fills = !slot_code.empty() && slot_code == code && is_dormant(code);
        if (codes.count(code) && !fills) {
            std::string holder;
            for (auto &[i, c] : code_of)
                if (c == code)
                    holder = i;
            refuse("code " + code + " is already " + (holder.empty() ? std::string("a track row") : holder + "'s"));
            continue;
        }
        Request req{id, code, from->second, slot_code, -1, true, ui_lists ? ui_lists(code) : std::vector<std::string>{},
                    strings_of(rec["sprint"])};
        StatId sid;
        if (!fills && stat) {
            int64_t pin = rec["stat_id"].is_int() ? rec["stat_id"].as_int() : -1;
            sid = stat(id, pin);
            if (sid.id < 0) {
                refuse("no stat id (" + (sid.how.empty() ? std::string("the track pool is full") : sid.how) + ")");
                continue;
            }
            req.stat_id = sid.id;
        }
        db.attribute(addon, id);
        std::size_t mark = db.mark();
        Lowered l = lower_track(db, t, req);
        if (!l.error.empty()) {
            db.rollback(mark);
            refuse("not applied: " + l.error);
            continue;
        }
        db.commit();
        p.applied++;
        code_of[id] = code;
        codes.insert(code);
        if (fills)
            p.filled[slot_id] = id;
        Done d{id, code, base_id, slot_id, addon, std::string(rec["name"].as_string()), l.stat_id, strings_of(rec["provides"]),
               l.sets, l.new_row};
        if (d.name.empty())
            d.name = code;
        for (auto &line : l.lines)
            p.lines.push_back("track " + id + ": " + line);
        for (auto &w : l.warnings)
            p.lines.push_back("track " + id + ": " + w);
        std::string how = fills ? "the dormant row " + slot_id + "'s" : sid.how.empty() ? std::string("first free from 254 down") : sid.how;
        p.stat_lines.push_back("[tracks] " + code + ": stat id " + std::to_string(l.stat_id) + " (" + how + ")");
        p.done.push_back(std::move(d));
    }
    for (auto &w : db.writes())
        if (w.fresh) {
            p.fresh++;
            p.addon_fresh[w.owner]++;
        }
    return p;
}

// A record track's data blocks: <addon>/<key>/<set>, a variant of <base id>/<set>, for every set holding the base.
inline std::size_t declare_data_variants(registry::Store &st, const Plan &p)
{
    std::size_t n = 0;
    for (auto &d : p.done)
        for (auto &set : d.sets) {
            registry::Change c;
            c.patch = Value::object({{"track", Value(d.id)}, {"data_set", Value(set)}});
            std::string out;
            if (st.variant(d.addon, "track.data", key_part(d.id) + "/" + set, d.base + "/" + set, c, &out) == SWOFTY_ASSET_OK)
                n++;
        }
    return n;
}

// The effective base: every row with assets in row order (a row an older track holds is its, a dormant row a record
// fills is the record's), then the record tracks with rows of their own, and each venue's resolved track.
inline Base make_base(const Scan &scan, const std::vector<Older> &older, const Plan &p, const std::vector<std::string> &venue_ids,
                      const std::function<std::string(const std::string &venue)> &resolved)
{
    Base b;
    std::map<std::string, const Older *> older_by;
    for (auto &o : older)
        older_by[o.code] = &o;
    std::map<std::string, const Done *> fill_by;
    for (auto &d : p.done)
        if (!d.new_row)
            fill_by[d.code] = &d;
    auto record = [](const Done &d) {
        BaseTrack t;
        t.id = d.id;
        t.code = d.code;
        t.stat_id = int(d.stat_id);
        t.base = d.base;
        t.slot = d.slot;
        t.provides = d.provides;
        t.time_trial = false;
        t.kind = Kind::record;
        t.addon = d.addon;
        t.name = d.name;
        return t;
    };
    for (auto &r : scan.rows) {
        if (auto o = older_by.find(r.code); o != older_by.end()) {
            BaseTrack t;
            t.id = o->second->addon + "/" + o->second->code;
            t.code = r.code;
            t.stat_id = int(o->second->stat_id >= 0 ? o->second->stat_id : r.stat_id);
            t.base = o->second->base;
            t.slot = o->second->slot;
            if (!o->second->venue.empty())
                t.provides.push_back(o->second->venue);
            t.kind = Kind::older;
            t.addon = o->second->addon;
            t.name = o->second->name.empty() ? r.code : o->second->name;
            b.tracks.push_back(std::move(t));
            if (is_dormant(r.code))
                b.dormant.push_back("game/" + r.code);
            continue;
        }
        if (is_dormant(r.code)) {
            b.dormant.push_back("game/" + r.code);
            if (auto f = fill_by.find(r.code); f != fill_by.end())
                b.tracks.push_back(record(*f->second));
            continue;
        }
        BaseTrack t;
        t.id = "game/" + r.code;
        t.code = r.code;
        t.stat_id = int(r.stat_id);
        if (const VenueSeed *v = stock_venue(r.code)) {
            t.provides.push_back(std::string("venue/") + v->key);
            t.name = v->name;
        } else {
            t.name = r.code;
        }
        t.time_trial = true;
        t.kind = Kind::stock;
        t.addon = "game";
        b.tracks.push_back(std::move(t));
    }
    for (auto &d : p.done)
        if (d.new_row)
            b.tracks.push_back(record(d));
    b.filled = p.filled;
    for (auto &id : venue_ids) {
        std::string track = resolved ? resolved(id) : std::string();
        if (!track.empty() && b.by_id(track))
            b.venues[id] = track;
    }
    return b;
}

inline std::string venue_track(registry::Store &st, const std::string &venue)
{
    registry::Record r = st.get("track.venue", venue);
    if (r.status != registry::Status::ok || !r.value["track"].is_string())
        return {};
    return std::string(r.value["track"].as_string());
}

void register_appliers();

}
