#pragma once
#include <algorithm>
#include <cctype>
#include <cstdint>
#include <cstdio>
#include <set>
#include <string>
#include <string_view>
#include <vector>

#include <swofty/json.hpp>

namespace runtime::career_saves {

using swofty::json::Value;

constexpr int INDEX_VERSION = 1;
constexpr size_t MAX_DATA = size_t(8) << 20;
constexpr size_t MAX_CAREERS = 4096;

inline const char *const STATUS_PRESENT = "present";
inline const char *const STATUS_MISSING = "missing";
inline const char *const STATUS_DELETED = "deleted";
constexpr uint32_t NO_INDEX = 0xffffffffu;
constexpr uint8_t FLAG_MY_TEAM = 0x20;
constexpr uint8_t FLAG_HIDDEN = 0x41;

struct SlotRead {
    uint64_t id = 0, guid = 0;
    bool details = false;
    std::string name;
    uint8_t setup_mode = 0, flags = 0;
    uint32_t season = NO_INDEX, weekend = NO_INDEX;
    uint16_t team = 0, driver = 0, vehicle = 0;
    uint64_t date = 0;
};

inline bool career_slot(const SlotRead &s) { return s.details && !(s.flags & FLAG_HIDDEN) && s.date != 0; }

struct ListContext {
    std::string session, current;
    int64_t now_ms = 0;
    bool complete = true;
    bool at_load = false;
};

struct ListResult {
    std::vector<std::string> added, back, deleted, missing;
    bool changed = false;
};

inline std::string slot_key(uint64_t id, uint64_t guid)
{
    char b[24];
    if (guid)
        snprintf(b, sizeof b, "%016llx", (unsigned long long)guid);
    else
        snprintf(b, sizeof b, "%llu", (unsigned long long)id);
    return b;
}

inline std::string id_tail(std::string_view id)
{
    size_t cut = id.rfind(':');
    return std::string(cut == std::string_view::npos ? id : id.substr(cut + 1));
}

inline std::string_view str(const Value &v, const char *key) { return v[key].as_string(); }

inline bool provisional_tail(std::string_view tail) { return tail.compare(0, 3, "new") == 0; }

inline bool plain_id(std::string_view s, size_t limit = 160)
{
    if (s.empty() || s.size() > limit)
        return false;
    for (unsigned char c : s)
        if (c < 0x20 || c == '"' || c == '\\' || c == '/' || c == 0x7f)
            return false;
    return true;
}

inline bool addon_id(std::string_view s)
{
    if (s.empty() || s.size() > 96)
        return false;
    for (unsigned char c : s)
        if (!(std::isalnum(c) || c == '.' || c == '_' || c == '-'))
            return false;
    return s.front() != '.' && s.find("..") == std::string_view::npos;
}

inline std::string safe_name(std::string_view career)
{
    std::string out;
    for (unsigned char c : career) {
        if (std::isalnum(c) || c == '-' || c == '_' || c == '.')
            out += char(c);
        else {
            static const char hex[] = "0123456789abcdef";
            out += '%';
            out += hex[c >> 4];
            out += hex[c & 15];
        }
    }
    return out;
}

inline std::string data_dir(std::string_view career) { return "careers/data/" + safe_name(career); }

inline std::string data_file(std::string_view career, std::string_view addon)
{
    return data_dir(career) + "/" + std::string(addon) + ".json";
}

inline std::string wrap_data(uint32_t version, const Value &value)
{
    return swofty::json::dump(Value::object({{"version", Value(int64_t(version))}, {"data", value}}));
}

inline bool unwrap_data(const std::string &text, uint32_t &version, std::string &value)
{
    swofty::json::Error e;
    Value v = swofty::json::parse(text, &e);
    if (!e.ok() || !v.is_object() || !v.contains("version") || !v.contains("data") || !v["version"].is_int())
        return false;
    version = uint32_t(v["version"].as_int());
    value = swofty::json::dump(v["data"]);
    return true;
}

struct Need {
    std::string id, range, why;
};

class Index {
public:
    bool load(const std::string &text)
    {
        careers_.clear();
        if (text.empty())
            return true;
        swofty::json::Error e;
        Value v = swofty::json::parse(text, &e);
        if (!e.ok())
            return false;
        const swofty::json::Array *list = v.is_array() ? v.if_array() : v["careers"].if_array();
        if (!list)
            return false;
        for (const Value &c : *list)
            if (c.is_object() && plain_id(c["career"].as_string()) && careers_.size() < MAX_CAREERS)
                careers_.push_back(c);
        return true;
    }

    size_t size() const { return careers_.size(); }

    Value *find(std::string_view id)
    {
        for (Value &c : careers_)
            if (c["career"].as_string() == id)
                return &c;
        return nullptr;
    }

    const Value *find(std::string_view id) const { return const_cast<Index *>(this)->find(id); }

    bool knows(std::string_view id) const { return find(id) != nullptr; }

    Value &touch(std::string_view id)
    {
        if (Value *c = find(id))
            return *c;
        Value rec = Value::object();
        rec["career"] = Value(std::string(id));
        rec["status"] = Value(STATUS_PRESENT);
        careers_.push_back(std::move(rec));
        return careers_.back();
    }

    void set_text(std::string_view id, const char *key, std::string_view value)
    {
        if (!value.empty())
            touch(id)[key] = Value(std::string(value));
    }

    void set_int(std::string_view id, const char *key, int64_t value)
    {
        touch(id)[key] = Value(value);
    }

    void saved(std::string_view id, int64_t now_ms)
    {
        Value &c = touch(id);
        c["last_saved"] = Value(now_ms);
        c["status"] = Value(STATUS_PRESENT);
    }

    void need(std::string_view id, const Need &n)
    {
        Value &c = touch(id);
        Value &list = c["addons"];
        if (!list.is_array())
            list = Value::array();
        for (Value &a : *list.if_array())
            if (a["id"].as_string() == n.id) {
                if (!n.range.empty())
                    a["need"] = Value(n.range);
                if (!n.why.empty())
                    a["why"] = Value(n.why);
                return;
            }
        Value a = Value::object({{"id", Value(n.id)}});
        if (!n.range.empty())
            a["need"] = Value(n.range);
        if (!n.why.empty())
            a["why"] = Value(n.why);
        list.push_back(std::move(a));
    }

    std::set<std::string> found_in(const std::set<std::string> &keys) const
    {
        std::set<std::string> out;
        for (const Value &c : careers_) {
            std::string id(c["career"].as_string());
            size_t cut = id.rfind(':');
            std::string tail = cut == std::string::npos ? id : id.substr(cut + 1);
            if (tail.compare(0, 3, "new") == 0 || keys.count(tail))
                out.insert(id);
        }
        return out;
    }

    std::vector<std::string> reconcile(const std::set<std::string> &present)
    {
        std::vector<std::string> gone;
        for (Value &c : careers_) {
            std::string id(c["career"].as_string());
            bool here = present.count(id) != 0;
            bool was_missing = c["status"].as_string() == STATUS_MISSING;
            if (here) {
                c["status"] = Value(STATUS_PRESENT);
            } else if (!was_missing) {
                c["status"] = Value(STATUS_MISSING);
                gone.push_back(id);
            }
        }
        return gone;
    }

    std::string career_for_slot(std::string_view key)
    {
        Value *c = find_by_tail(key);
        return c ? std::string(c->operator[]("career").as_string()) : std::string();
    }

    Value *find_by_tail(std::string_view key)
    {
        for (Value &c : careers_)
            if (id_tail(c["career"].as_string()) == key)
                return &c;
        return nullptr;
    }

    bool adopt(std::string_view id)
    {
        if (find(id))
            return false;
        std::string tail = id_tail(id);
        if (provisional_tail(tail))
            return false;
        Value *c = find_by_tail(tail);
        if (!c || str(*c, "source") != "slots")
            return false;
        (*c)["career"] = Value(std::string(id));
        c->erase("source");
        return true;
    }

    ListResult apply_list(const std::vector<SlotRead> &now, const ListContext &ctx)
    {
        ListResult out;
        std::set<std::string> seen;
        std::set<std::string> kept_this_session;
        std::set<std::string> slot_modes;
        for (const Value &c : careers_)
            if (c["listed_session"].as_string() == ctx.session)
                kept_this_session.insert(std::string(c["career"].as_string()));
        bool any_known = false;
        for (const SlotRead &s : now) {
            std::string key = slot_key(s.id, s.guid);
            Value *rec = find_by_tail(key);
            if (!career_slot(s)) {
                if (!rec)
                    continue;
                if (str(*rec, "source") == "slots") {
                    careers_.erase(careers_.begin() + (rec - careers_.data()));
                    out.changed = true;
                } else
                    seen.insert(std::string(str(*rec, "career")));
                continue;
            }
            if (!rec) {
                if (careers_.size() >= MAX_CAREERS)
                    continue;
                std::string kind = s.flags & FLAG_MY_TEAM ? "my_team" : "driver_career";
                rec = &touch(kind + ":" + key);
                (*rec)["source"] = Value("slots");
                out.added.push_back(std::string(str(*rec, "career")));
                out.changed = true;
            } else {
                any_known = any_known || kept_this_session.count(std::string(str(*rec, "career"))) != 0;
                std::string st(str(*rec, "status"));
                if (st == STATUS_MISSING || st == STATUS_DELETED) {
                    out.back.push_back(std::string(str(*rec, "career")));
                    out.changed = true;
                }
            }
            std::string id(str(*rec, "career"));
            seen.insert(id);
            slot_modes.insert(s.flags & FLAG_MY_TEAM ? "my_team" : "driver_career");
            (*rec)["status"] = Value(STATUS_PRESENT);
            (*rec)["listed_session"] = Value(ctx.session);
            (*rec)["listed_at"] = Value(ctx.now_ms);
            (*rec)["slot"] = Value(key);
            if (str(*rec, "mode").empty())
                (*rec)["mode"] = Value(std::string(s.flags & FLAG_MY_TEAM ? "my_team" : "driver_career"));
            bool same = id == ctx.current;
            if (!same && rec->contains("driver_raw") && (*rec)["driver_raw"].as_int() != s.driver)
                rec->erase("driver");
            if (!same && rec->contains("team_raw") && (*rec)["team_raw"].as_int() != s.team)
                rec->erase("team");
            (*rec)["driver_raw"] = Value(int64_t(s.driver));
            (*rec)["team_raw"] = Value(int64_t(s.team));
            (*rec)["vehicle_raw"] = Value(int64_t(s.vehicle));
            if (!s.name.empty())
                (*rec)["slot_name"] = Value(s.name);
            if (s.season != NO_INDEX)
                (*rec)["slot_season"] = Value(int64_t(s.season) + 1);
            if (s.weekend != NO_INDEX)
                (*rec)["slot_weekend"] = Value(int64_t(s.weekend));
            if (s.date)
                (*rec)["last_played"] = Value(int64_t(s.date));
            if (s.flags & FLAG_HIDDEN)
                (*rec)["hidden"] = Value(true);
            else
                rec->erase("hidden");
        }
        if (!ctx.complete)
            return out;
        bool switched = ctx.at_load && !seen.empty() && !kept_this_session.empty() && !any_known;
        for (Value &c : careers_) {
            std::string id(str(c, "career"));
            if (seen.count(id) || provisional_tail(id_tail(id)))
                continue;
            std::string st(str(c, "status"));
            if (st == STATUS_DELETED)
                continue;
            if (id.find(':') == std::string::npos && slot_modes.count(std::string(str(c, "mode")))) {
                if (st == STATUS_MISSING) {
                    c["status"] = Value(STATUS_PRESENT);
                    out.changed = true;
                }
                continue;
            }
            bool here = !switched && str(c, "listed_session") == ctx.session;
            if (here) {
                c["status"] = Value(STATUS_DELETED);
                out.deleted.push_back(id);
                out.changed = true;
            } else if (st != STATUS_MISSING) {
                c["status"] = Value(STATUS_MISSING);
                out.missing.push_back(id);
                out.changed = true;
            }
        }
        return out;
    }

    std::vector<std::string> missing_ids() const
    {
        std::vector<std::string> out;
        for (const Value &c : careers_)
            if (c["status"].as_string() == STATUS_MISSING)
                out.emplace_back(c["career"].as_string());
        return out;
    }

    std::string to_json() const
    {
        Value list = Value::array();
        for (const Value &c : careers_)
            list.push_back(c);
        return swofty::json::dump(
            Value::object({{"version", Value(int64_t(INDEX_VERSION))}, {"careers", std::move(list)}}));
    }

    std::string only_json(const std::vector<std::string> &ids) const
    {
        Value list = Value::array();
        for (const Value &c : careers_)
            if (std::find(ids.begin(), ids.end(), std::string(c["career"].as_string())) != ids.end())
                list.push_back(c);
        return swofty::json::dump(
            Value::object({{"version", Value(int64_t(INDEX_VERSION))}, {"careers", std::move(list)}}));
    }

private:
    std::vector<Value> careers_;
};

inline std::string label_of(std::string_view content, std::string_view driver, std::string_view team, int season)
{
    std::string out = std::string(content);
    auto add = [&](std::string_view part) {
        if (!part.empty())
            out += (out.empty() ? "" : ": ") + std::string(part);
    };
    add(driver);
    if (!team.empty())
        out += (out.empty() ? "" : ", ") + std::string(team);
    if (season > 0)
        out += (out.empty() ? "" : ", ") + std::to_string(season);
    return out;
}

}
