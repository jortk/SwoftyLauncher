#pragma once

#include "keys.hpp"

#include <swofty/json.hpp>

#include <algorithm>
#include <cstdint>
#include <cstdlib>
#include <functional>
#include <map>
#include <optional>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

// A track record lowered to the game database: the launcher's 1.5 track activation (packages/core/src/activation/
// db.rs activate) ported to the core, over the same table description (track_tables.json, embedded as
// track_keys.inc). The tracks.Track row is cloned from the slot row or the base row (or a row with the record's code
// is kept), the track resource is routed to the track's own asset groups, every per-track table gets a row, every
// data set holding the base gets a TrackData block, the sorted orders get the code and every available_tracks list
// holding the base gets it. Pure: the database is a Read, every write lands in a Db overlay, so host tests run it on a
// fixture and the core on the database the game loads.

namespace swofty_core::apply::tracks {

using Value = swofty::json::Value;

struct Rec {
    int tag = -1;
    std::string text;
};

using Read = std::function<std::optional<Rec>(uint64_t key)>;

struct Write {
    uint64_t key = 0;
    std::string path;
    int tag = -1;
    std::string text;
    bool fresh = false;
    std::string owner, record;
};

class Db {
public:
    explicit Db(Read base) : base_(std::move(base)) {}

    std::optional<Rec> get(uint64_t key) const
    {
        auto it = index_.find(key);
        if (it != index_.end())
            return Rec{out_[it->second].tag, out_[it->second].text};
        return base_ ? base_(key) : std::nullopt;
    }

    bool has(uint64_t key) const { return get(key).has_value(); }

    std::optional<std::string> text(uint64_t key) const
    {
        auto r = get(key);
        if (!r)
            return std::nullopt;
        return r->text;
    }

    std::optional<int64_t> int_of(uint64_t key) const
    {
        auto t = text(key);
        if (!t)
            return std::nullopt;
        std::string s = *t;
        while (!s.empty() && (s.front() == ' ' || s.front() == '\t' || s.front() == '\n' || s.front() == '\r'))
            s.erase(s.begin());
        while (!s.empty() && (s.back() == ' ' || s.back() == '\t' || s.back() == '\n' || s.back() == '\r'))
            s.pop_back();
        if (s.empty())
            return std::nullopt;
        std::size_t i = s[0] == '-' || s[0] == '+' ? 1 : 0;
        if (i == s.size())
            return std::nullopt;
        for (std::size_t k = i; k < s.size(); k++)
            if (s[k] < '0' || s[k] > '9')
                return std::nullopt;
        return std::strtoll(s.c_str(), nullptr, 10);
    }

    std::optional<std::size_t> count(uint64_t key) const
    {
        auto n = int_of(key);
        if (!n || *n < 0)
            return std::nullopt;
        return std::size_t(*n);
    }

    void attribute(std::string owner, std::string record)
    {
        owner_ = std::move(owner);
        record_ = std::move(record);
    }

    void put(const Key &k, int tag, std::string text)
    {
        auto it = index_.find(k.h);
        if (it != index_.end()) {
            Write &w = out_[it->second];
            if (w.tag == tag && w.text == text)
                return;
            undo_.push_back({k.h, w});
            w.tag = tag;
            w.text = std::move(text);
            w.owner = owner_;
            w.record = record_;
            changed++;
            return;
        }
        auto had = base_ ? base_(k.h) : std::nullopt;
        if (had && had->tag == tag && had->text == text)
            return;
        undo_.push_back({k.h, std::nullopt});
        index_[k.h] = out_.size();
        out_.push_back({k.h, k.path, tag, std::move(text), !had, owner_, record_});
        if (had)
            changed++;
        else
            added++;
    }

    bool copy(const Key &from, const Key &to)
    {
        auto r = get(from.h);
        if (!r)
            return false;
        put(to, r->tag, r->text);
        return true;
    }

    bool set_text(const Key &k, std::string text)
    {
        auto r = get(k.h);
        if (!r)
            return false;
        put(k, r->tag, std::move(text));
        return true;
    }

    std::size_t mark() const { return undo_.size(); }

    void rollback(std::size_t to)
    {
        while (undo_.size() > to) {
            Undo u = std::move(undo_.back());
            undo_.pop_back();
            auto it = index_.find(u.key);
            if (it == index_.end())
                continue;
            if (u.before) {
                out_[it->second] = std::move(*u.before);
            } else {
                std::size_t at = it->second;
                index_.erase(it);
                out_.erase(out_.begin() + std::ptrdiff_t(at));
                for (auto &[k, i] : index_)
                    if (i > at)
                        i--;
            }
        }
    }

    void commit() { undo_.clear(); }

    const std::vector<Write> &writes() const { return out_; }

    std::size_t added = 0, changed = 0;

private:
    struct Undo {
        uint64_t key;
        std::optional<Write> before;
    };

    Read base_;
    std::string owner_, record_;
    std::unordered_map<uint64_t, std::size_t> index_;
    std::vector<Write> out_;
    std::vector<Undo> undo_;
};

struct Schema {
    std::vector<std::string> fields;
    std::map<std::string, Schema> tables;
};

struct TableSpec {
    std::string id;
    std::optional<std::string> path, base, count;
    std::vector<std::string> labels{"_metadata_"};
    std::vector<std::string> unique;
    Schema schema;
};

struct ListSpec {
    TableSpec table;
    std::string sub_table, field;
};

struct Tables {
    TableSpec track, datasets, track_data;
    std::vector<TableSpec> per_track, sorted_orders;
    std::vector<ListSpec> track_lists;
    std::optional<TableSpec> track_resources;
    bool ok = false;
};

inline Schema schema_of(const Value &v)
{
    Schema s;
    if (auto a = v["fields"].if_array())
        for (auto &f : *a)
            if (f.is_string())
                s.fields.emplace_back(f.as_string());
    if (auto o = v["tables"].if_object())
        for (auto &[name, sub] : *o)
            s.tables[name] = schema_of(sub);
    return s;
}

inline TableSpec spec_of(const Value &v)
{
    TableSpec t;
    t.id = std::string(v["id"].as_string());
    if (v["path"].is_string())
        t.path = std::string(v["path"].as_string());
    if (v["base"].is_string())
        t.base = std::string(v["base"].as_string());
    if (v["count"].is_string())
        t.count = std::string(v["count"].as_string());
    if (auto a = v["labels"].if_array()) {
        t.labels.clear();
        for (auto &l : *a)
            t.labels.emplace_back(l.as_string());
    }
    if (auto a = v["unique"].if_array())
        for (auto &u : *a)
            t.unique.emplace_back(u.as_string());
    t.schema = schema_of(v["schema"]);
    return t;
}

inline Tables tables_of(const Value &v)
{
    Tables t;
    if (!v.is_object())
        return t;
    t.track = spec_of(v["track"]);
    t.datasets = spec_of(v["datasets"]);
    t.track_data = spec_of(v["track_data"]);
    if (auto a = v["per_track"].if_array())
        for (auto &s : *a)
            t.per_track.push_back(spec_of(s));
    if (auto a = v["sorted_orders"].if_array())
        for (auto &s : *a)
            t.sorted_orders.push_back(spec_of(s));
    if (auto a = v["track_lists"].if_array())
        for (auto &s : *a)
            t.track_lists.push_back({spec_of(s["table"]), std::string(s["sub_table"].as_string()),
                                     std::string(s["field"].as_string())});
    if (v["track_resources"].is_object())
        t.track_resources = spec_of(v["track_resources"]);
    t.ok = t.track.path.has_value() && t.track_data.path.has_value();
    return t;
}

#include "track_keys.inc"

inline const Tables &builtin_tables()
{
    static const Tables t = [] {
        std::string all;
        for (const char *chunk : TRACK_TABLES)
            all += chunk;
        return tables_of(swofty::json::parse(all));
    }();
    return t;
}

inline std::optional<uint64_t> parse_hex(const std::string &s)
{
    std::string_view v = s;
    if (v.substr(0, 2) == "0x")
        v.remove_prefix(2);
    if (v.empty() || v.size() > 16)
        return std::nullopt;
    uint64_t h = 0;
    for (char c : v) {
        int d = c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1;
        if (d < 0)
            return std::nullopt;
        h = h << 4 | uint64_t(d);
    }
    return h;
}

struct Table {
    Key base;
    std::optional<Key> count;
    bool ok = true;

    static Table of(const TableSpec &spec)
    {
        Table t;
        if (spec.base) {
            auto h = parse_hex(*spec.base);
            t.ok = h.has_value();
            t.base = hashed(h.value_or(0));
        } else if (spec.path) {
            t.base = key(*spec.path + "_");
        } else {
            t.ok = false;
        }
        if (spec.count) {
            auto h = parse_hex(*spec.count);
            if (h)
                t.count = hashed(*h);
            else
                t.ok = false;
        } else if (spec.path) {
            auto dot = spec.path->rfind('.');
            if (dot != std::string::npos)
                t.count = key(spec.path->substr(0, dot) + "._" + spec.path->substr(dot + 1) + "_instancecount_");
        }
        return t;
    }

    static Table nested(const Key &parent_row, const std::string &name)
    {
        return {sub_table(parent_row, name), sub_count(parent_row, name)};
    }

    std::size_t len(const Db &db) const
    {
        if (count)
            return db.count(count->h).value_or(0);
        std::size_t n = 0;
        while (db.has(field(row(base, n), "_metadata_").h))
            n++;
        return n;
    }

    std::optional<Key> row_of(const Db &db, const std::string &label, const std::string &label_field) const
    {
        std::size_t n = len(db);
        for (std::size_t i = 0; i < n; i++) {
            Key r = row(base, i);
            if (db.text(field(r, label_field).h) == label)
                return r;
        }
        return std::nullopt;
    }

    std::optional<Key> push(Db &db) const
    {
        std::size_t n = len(db);
        if (!count || !db.has(count->h))
            return std::nullopt;
        db.set_text(*count, std::to_string(n + 1));
        return row(base, n);
    }
};

inline std::size_t clone_row(Db &db, const Schema &schema, const Key &src, const Key &dst)
{
    std::size_t n = 0;
    for (auto &f : schema.fields)
        if (db.copy(field(src, f), field(dst, f)))
            n++;
    for (auto &[name, sub] : schema.tables) {
        Key from = sub_count(src, name), to = sub_count(dst, name);
        auto count = db.count(from.h);
        if (!count)
            continue;
        db.copy(from, to);
        n++;
        for (std::size_t i = 0; i < *count; i++)
            n += clone_row(db, sub, row(sub_table(src, name), i), row(sub_table(dst, name), i));
    }
    return n;
}

inline void relabel(Db &db, const Key &row_key, const std::vector<std::string> &labels, const std::string &code)
{
    for (auto &l : labels) {
        Key k = field(row_key, l);
        if (db.has(k.h))
            db.set_text(k, code);
    }
}

inline void fresh_unique(Db &db, const Table &table, const Key &dst, const std::vector<std::string> &unique)
{
    for (auto &u : unique) {
        int64_t max = -1;
        std::size_t n = table.len(db);
        for (std::size_t i = 0; i < n; i++) {
            Key r = row(table.base, i);
            if (r.h == dst.h)
                continue;
            if (auto v = db.int_of(field(r, u).h))
                max = std::max(max, *v);
        }
        Key k = field(dst, u);
        if (db.has(k.h))
            db.set_text(k, std::to_string(max + 1));
    }
}

inline std::optional<Key> add_row_from(Db &db, const TableSpec &spec, const Table &table, const Key &src,
                                       const std::string &code)
{
    auto dst = table.push(db);
    if (!dst)
        return std::nullopt;
    clone_row(db, spec.schema, src, *dst);
    relabel(db, *dst, spec.labels, code);
    fresh_unique(db, table, *dst, spec.unique);
    return dst;
}

inline constexpr const char *RESOURCE_GROUP_FIELDS[] = {"worldAssetGroup",     "aoBakeAssetGroup",
                                                        "radianceBakeAssetGroup", "lightListAssetGroup",
                                                        "ddgiAssetGroup",      "trackMapAssetGroup"};

inline std::string natural_group(const std::string &code, std::string_view f)
{
    std::string rel = f == "worldAssetGroup"          ? "wep/" + code + "_common"
                      : f == "aoBakeAssetGroup"       ? "bake/" + code + "_ao.bake"
                      : f == "radianceBakeAssetGroup" ? "bake/" + code + "_radiance.bake"
                      : f == "lightListAssetGroup"    ? "bake/" + code + "_light_list.bake"
                      : f == "ddgiAssetGroup"         ? std::string("probes/config.ddgi")
                                                      : code;
    return "asset_groups/environment_package/tracks/" + code + "/" + rel + ".assetgroup";
}

struct Request {
    std::string id, code, from, slot;
    int64_t stat_id = -1;
    bool route = true;
    std::vector<std::string> ui_lists;
    std::vector<std::string> sprint;
};

struct Lowered {
    std::string error;
    std::vector<std::string> lines, warnings, sets;
    int64_t stat_id = -1;
    bool new_row = false;
};

inline std::string route_resource(Db &db, const Tables &t, const Key &track_row, const std::string &code,
                                  std::vector<std::string> &lines)
{
    if (!t.track_resources)
        return "the table description has no track resource table";
    const TableSpec &spec = *t.track_resources;
    Table table = Table::of(spec);
    auto current = db.text(field(track_row, "track_resource").h);
    if (!current)
        return code + ": track row has no track_resource";
    std::string label = "swofty_" + code;
    auto src = table.row_of(db, *current, "_metadata_");
    if (!src)
        src = table.row_of(db, "2019", "_metadata_");
    if (!src)
        return "track resource " + *current + " not found";
    auto dst = table.row_of(db, label, "_metadata_");
    if (!dst)
        dst = table.push(db);
    if (!dst)
        return "track resource table has no row count";
    if (dst->h != src->h)
        clone_row(db, spec.schema, *src, *dst);
    if (!db.set_text(field(*dst, "_metadata_"), label))
        return "_metadata_: no such key";
    for (const char *f : RESOURCE_GROUP_FIELDS)
        if (!db.set_text(field(*dst, f), natural_group(code, f)))
            return std::string(f) + ": no such key";
    if (!db.set_text(field(*dst, "iblAssetGroupsRoot"), "asset_groups/environment_package/tracks/" + code + "/probes/"))
        return "iblAssetGroupsRoot: no such key";
    if (!db.set_text(field(*dst, "iblKey"), "key_2019.assetgroup"))
        return "iblKey: no such key";
    db.set_text(field(track_row, "track_resource"), label);
    lines.push_back("track resource " + label + " (from " + *current + ") loads " + code + " from " +
                    natural_group(code, ""));
    return {};
}

// The launcher builds an added track's menu pictures into its UI lists as overrides.<list>.<code>; a TrackTextures
// value naming overrides.<list>.<other> in one of those lists is pointed at the track's own (activation::db).
inline std::size_t point_textures(Db &db, const std::vector<Key> &keys, const std::string &code,
                                  const std::vector<std::string> &lists)
{
    std::size_t n = 0;
    for (auto &k : keys) {
        auto v = db.text(k.h);
        if (!v || v->rfind("overrides.", 0) != 0)
            continue;
        std::string rest = v->substr(10);
        auto dot = rest.rfind('.');
        if (dot == std::string::npos)
            continue;
        std::string list = rest.substr(0, dot);
        if (std::find(lists.begin(), lists.end(), list) == lists.end())
            continue;
        std::string to = "overrides." + list + "." + code;
        if (to != *v && db.set_text(k, to))
            n++;
    }
    return n;
}

inline std::vector<Key> texture_keys(const Db &db, const Tables &t, const Key &track_row, const std::vector<Key> &data_rows)
{
    std::vector<Key> keys;
    auto tex = t.track_data.schema.tables.find("TrackTextures");
    if (tex != t.track_data.schema.tables.end())
        for (auto &r : data_rows) {
            std::size_t m = db.count(sub_count(r, "TrackTextures").h).value_or(0);
            for (std::size_t i = 0; i < m; i++)
                for (auto &f : tex->second.fields)
                    keys.push_back(field(row(sub_table(r, "TrackTextures"), i), f));
        }
    std::size_t m = db.count(sub_count(track_row, "TrackTextures").h).value_or(0);
    for (std::size_t i = 0; i < m; i++) {
        Key tr = row(sub_table(track_row, "TrackTextures"), i);
        std::size_t k = db.count(sub_count(tr, "TrackTexture").h).value_or(0);
        for (std::size_t j = 0; j < k; j++)
            keys.push_back(field(row(sub_table(tr, "TrackTexture"), j), "value"));
    }
    return keys;
}

inline constexpr const char *WEEKEND_TABLE = "TrackAuthenticWeekendData";

inline std::optional<std::string> weekend_format(const Db &db, const Key &data_row)
{
    return db.text(field(row(sub_table(data_row, WEEKEND_TABLE), 0), "weekend_format").h);
}

inline bool set_weekend_format(Db &db, const Key &data_row, const std::string &format)
{
    std::size_t n = db.count(sub_count(data_row, WEEKEND_TABLE).h).value_or(0);
    bool set = false;
    for (std::size_t i = 0; i < n; i++)
        set = db.set_text(field(row(sub_table(data_row, WEEKEND_TABLE), i), "weekend_format"), format) || set;
    return set;
}

inline std::vector<std::string> split_items(const std::string &v)
{
    std::vector<std::string> out;
    std::size_t at = 0;
    while (at <= v.size()) {
        std::size_t c = v.find(',', at);
        if (c == std::string::npos)
            c = v.size();
        if (c > at)
            out.push_back(v.substr(at, c - at));
        at = c + 1;
    }
    return out;
}

// Writes into db; on an error the caller rolls back to its mark.
inline Lowered lower_track(Db &db, const Tables &t, const Request &req)
{
    Lowered out;
    const std::string &code = req.code, &from = req.from;
    std::string slot = req.slot.empty() ? from : req.slot;
    if (!t.ok) {
        out.error = "the track table description is not readable";
        return out;
    }
    Table tracks = Table::of(t.track);
    std::string label = t.track.labels.empty() ? std::string("_metadata_") : t.track.labels.front();
    if (!tracks.row_of(db, from, label)) {
        out.error = from + " is not a track in this game";
        return out;
    }
    std::optional<Key> track_row = tracks.row_of(db, code, label);
    if (track_row) {
        out.lines.push_back(code + ": uses its existing track slot");
    } else {
        auto slot_row = tracks.row_of(db, slot, label);
        if (!slot_row) {
            out.error = "slot " + slot + " is not a track in this game";
            return out;
        }
        track_row = add_row_from(db, t.track, tracks, *slot_row, code);
        if (!track_row) {
            out.error = "tracks.Track has no row count";
            return out;
        }
        out.new_row = true;
        if (req.stat_id >= 0)
            db.set_text(field(*track_row, "statId"), std::to_string(req.stat_id));
        out.lines.push_back(code + ": new track row cloned from " + slot);
    }
    if (auto s = db.int_of(field(*track_row, "statId").h))
        out.stat_id = *s;
    if (req.route) {
        std::string why = route_resource(db, t, *track_row, code, out.lines);
        if (!why.empty()) {
            out.error = why;
            return out;
        }
    }
    for (auto &spec : t.per_track) {
        Table table = Table::of(spec);
        if (!table.ok || table.len(db) == 0) {
            out.warnings.push_back(spec.id + ": table not found in this database");
            continue;
        }
        std::string l = spec.labels.empty() ? std::string("_metadata_") : spec.labels.front();
        if (table.row_of(db, code, l))
            continue;
        auto src = table.row_of(db, slot, l);
        if (!src)
            src = table.row_of(db, from, l);
        if (!src)
            continue;
        if (!add_row_from(db, spec, table, *src, code)) {
            out.error = spec.id + ": table has no row count";
            return out;
        }
        out.lines.push_back(spec.id + ": added " + code);
    }
    Table sets = Table::of(t.datasets);
    std::vector<Key> data_rows;
    std::vector<std::string> sprint_sets;
    std::size_t n = sets.ok ? sets.len(db) : 0;
    if (!n) {
        out.error = "the dataset table is missing or empty";
        return out;
    }
    for (std::size_t i = 0; i < n; i++) {
        Key set = row(sets.base, i);
        std::string set_label = db.text(field(set, "_metadata_").h).value_or("");
        Table data = Table::nested(set, *t.track_data.path);
        auto src = data.row_of(db, from, "track");
        if (!src)
            continue;
        auto dst = data.row_of(db, code, "track");
        std::optional<std::string> own;
        if (dst)
            own = weekend_format(db, *dst);
        if (!dst)
            dst = data.push(db);
        if (!dst) {
            out.error = set_label + ": TrackData has no row count";
            return out;
        }
        clone_row(db, t.track_data.schema, *src, *dst);
        relabel(db, *dst, t.track_data.labels, code);
        if (dst->h != src->h) {
            bool sprint = std::find(req.sprint.begin(), req.sprint.end(), set_label) != req.sprint.end();
            if (set_weekend_format(db, *dst, sprint ? std::string("sprint") : own.value_or("default")) && sprint)
                sprint_sets.push_back(set_label);
        }
        data_rows.push_back(*dst);
        out.sets.push_back(set_label);
    }
    std::string joined;
    for (auto &s : out.sets)
        joined += (joined.empty() ? "" : ", ") + s;
    out.lines.push_back("track data: " + code + " cloned from " + from + " in " + std::to_string(out.sets.size()) +
                        " dataset(s): " + joined);
    if (!sprint_sets.empty()) {
        std::string listed;
        for (auto &s : sprint_sets)
            listed += (listed.empty() ? "" : ", ") + s;
        out.lines.push_back("weekend format: " + code + " runs sprint weekends in " + listed);
    }
    for (auto &s : req.sprint)
        if (std::find(sprint_sets.begin(), sprint_sets.end(), s) == sprint_sets.end())
            out.warnings.push_back("sprint " + s + ": " + code +
                                   " has no weekend format in that data set, so it takes its parent data set's");
    for (auto &spec : t.sorted_orders) {
        Table table = Table::of(spec);
        std::string l = spec.labels.empty() ? std::string("_metadata_") : spec.labels.front();
        if (table.row_of(db, code, l))
            continue;
        if (auto src = table.row_of(db, from, l)) {
            if (!add_row_from(db, spec, table, *src, code)) {
                out.error = spec.id + ": table has no row count";
                return out;
            }
            out.lines.push_back(spec.id + ": appended " + code);
        }
    }
    for (auto &list : t.track_lists) {
        Table table = Table::of(list.table);
        std::size_t rows = table.len(db);
        for (std::size_t i = 0; i < rows; i++) {
            Key set = row(table.base, i);
            Table inner = Table::nested(set, list.sub_table);
            std::size_t m = inner.len(db);
            for (std::size_t j = 0; j < m; j++) {
                Key k = field(row(inner.base, j), list.field);
                auto v = db.text(k.h);
                if (!v)
                    continue;
                auto items = split_items(*v);
                if (std::find(items.begin(), items.end(), from) == items.end() ||
                    std::find(items.begin(), items.end(), code) != items.end())
                    continue;
                items.push_back(code);
                std::string text;
                for (auto &it : items)
                    text += (text.empty() ? "" : ",") + it;
                db.set_text(k, text);
                out.lines.push_back(list.sub_table + " " + db.text(field(set, "_metadata_").h).value_or("") + ": added " + code);
            }
        }
    }
    if (!req.ui_lists.empty()) {
        std::size_t pointed = point_textures(db, texture_keys(db, t, *track_row, data_rows), code, req.ui_lists);
        out.lines.push_back(code + ": " + std::to_string(pointed) + " texture path(s) point at its own pictures");
    }
    return out;
}

}
