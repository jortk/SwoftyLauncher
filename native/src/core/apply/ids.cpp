#include "ids.hpp"

#include <swofty/json.hpp>

#include <algorithm>

namespace swofty_core::apply::ids {

using swofty::json::Value;

namespace {
bool career_holds(const std::string &registry)
{
    return registry != "vehicle.package";
}
}

bool Store::load(const std::string &text, std::string *why)
{
    std::lock_guard g(mu_);
    ids_.clear();
    careers_.clear();
    deduped_ = 0;
    dirty_ = false;
    swofty::json::Error e;
    Value doc = swofty::json::parse(text, &e);
    if (!e.ok() || !doc.is_object()) {
        if (why)
            *why = e.ok() ? "not a JSON object" : e.message;
        return false;
    }
    if (auto regs = doc["ids"].if_object())
        for (auto &[reg, recs] : *regs)
            if (auto o = recs.if_object())
                for (auto &[rec, ent] : *o) {
                    Entry en;
                    if (ent.is_int())
                        en.id = ent.as_int();
                    else {
                        en.id = ent["id"].as_int(-1);
                        en.since = std::string(ent["since"].as_string());
                        en.pin = ent["pin"].as_int(-1);
                    }
                    if (en.id >= 0)
                        ids_[reg][rec] = en;
                }
    if (auto cs = doc["careers"].if_object())
        for (auto &[career, regs] : *cs)
            if (auto o = regs.if_object())
                for (auto &[reg, recs] : *o)
                    if (auto r = recs.if_object(); r && career_holds(reg))
                        for (auto &[rec, id] : *r)
                            if (id.is_int())
                                careers_[career][reg][rec] = id.as_int();
    for (auto &[reg, recs] : ids_) {
        std::map<int64_t, std::string> owner;
        std::vector<std::string> lose;
        for (auto &[rec, en] : recs) {
            auto [at, fresh] = owner.emplace(en.id, rec);
            if (fresh)
                continue;
            bool mine = false, theirs = false;
            for (auto &[career, map] : careers_)
                if (auto c = map.find(reg); c != map.end()) {
                    auto a = c->second.find(rec), b = c->second.find(at->second);
                    mine = mine || (a != c->second.end() && a->second == en.id);
                    theirs = theirs || (b != c->second.end() && b->second == en.id);
                }
            if (mine && !theirs) {
                lose.push_back(at->second);
                at->second = rec;
            } else
                lose.push_back(rec);
        }
        for (auto &rec : lose)
            recs.erase(rec);
        deduped_ += lose.size();
        dirty_ = dirty_ || !lose.empty();
    }
    return true;
}

std::string Store::dump() const
{
    std::lock_guard g(mu_);
    Value doc = Value::object();
    doc["version"] = Value(1);
    Value regs = Value::object();
    for (auto &[reg, recs] : ids_) {
        Value o = Value::object();
        for (auto &[rec, en] : recs) {
            Value e = Value::object({{"id", Value(en.id)}});
            if (!en.since.empty())
                e["since"] = Value(en.since);
            if (en.pin >= 0)
                e["pin"] = Value(en.pin);
            o[rec] = e;
        }
        regs[reg] = o;
    }
    doc["ids"] = regs;
    Value cs = Value::object();
    for (auto &[career, map] : careers_) {
        Value c = Value::object();
        for (auto &[reg, recs] : map) {
            Value o = Value::object();
            for (auto &[rec, id] : recs)
                o[rec] = Value(id);
            c[reg] = o;
        }
        cs[career] = c;
    }
    doc["careers"] = cs;
    return swofty::json::dump(doc, 1);
}

void Store::clear()
{
    std::lock_guard g(mu_);
    ids_.clear();
    careers_.clear();
    pools_.clear();
    live_.clear();
    db_.clear();
    dirty_ = false;
    lent_ = false;
}

bool Store::lend_json(const std::string &text, std::vector<std::string> *lines, std::string *why)
{
    swofty::json::Error e;
    Value doc = swofty::json::parse(text, &e);
    if (!e.ok() || !doc.is_object()) {
        if (why)
            *why = e.ok() ? "not a JSON object" : e.message;
        return false;
    }
    Lent lent;
    if (auto regs = doc["ids"].if_object())
        for (auto &[reg, recs] : *regs)
            if (auto o = recs.if_object())
                for (auto &[rec, id] : *o)
                    if (id.is_int() && id.as_int() >= 0)
                        lent[reg][rec] = id.as_int();
    std::vector<std::string> got = lend(lent);
    if (lines)
        *lines = std::move(got);
    return true;
}

std::vector<std::string> Store::lend(const Lent &lent)
{
    std::lock_guard g(mu_);
    std::vector<std::string> out;
    std::size_t applied = 0;
    for (auto &[reg, recs] : lent) {
        auto &mine = ids_[reg];
        std::map<int64_t, std::string> claimed;
        std::vector<std::string> displaced;
        for (auto &[rec, id] : recs) {
            auto [at, fresh] = claimed.emplace(id, rec);
            if (!fresh) {
                out.push_back("[registry] stat ids: " + reg + " " + rec + " not lent: the host gives " + at->second +
                              " the same stat id " + std::to_string(id));
                continue;
            }
            std::string who;
            for (auto &[career, map] : careers_) {
                auto it = map.find(reg);
                if (it == map.end())
                    continue;
                for (auto &[r, v] : it->second)
                    if (v == id && r != rec)
                        who = career + " uses " + std::to_string(id) + " for " + r;
            }
            if (!who.empty()) {
                out.push_back("[registry] stat ids: " + reg + " " + rec + " not lent (host's " + std::to_string(id) +
                              "): " + who + " here, so this record keeps its own id");
                claimed.erase(id);
                continue;
            }
            for (auto &[other, en] : mine)
                if (other != rec && en.id == id && !en.lent)
                    displaced.push_back(other);
            auto e = mine.find(rec);
            int64_t was = e == mine.end() ? -1 : e->second.id;
            mine[rec] = Entry{id, e == mine.end() ? std::string() : e->second.since, -1, true};
            applied++;
            out.push_back("[registry] stat ids: " + reg + " " + rec + " -> " + std::to_string(id) +
                          " (lent by the host for this session" +
                          (was >= 0 && was != id ? "; this install keeps " + std::to_string(was) : std::string()) +
                          ")");
        }
        for (auto &rec : displaced) {
            auto it = mine.find(rec);
            if (it == mine.end() || it->second.lent)
                continue;
            int64_t was = it->second.id;
            mine.erase(it);
            int64_t to = next_free_locked(reg, rec);
            if (to >= 0)
                mine[rec] = Entry{to, std::string(), -1, false};
            out.push_back("[registry] stat ids: " + reg + " " + rec + " moved off " + std::to_string(was) +
                          ", which the host's record needs, " +
                          (to >= 0 ? "to " + std::to_string(to) : std::string("and gets an id when it registers")));
        }
    }
    lent_ = lent_ || applied > 0;
    return out;
}

bool Store::lent() const
{
    std::lock_guard g(mu_);
    return lent_;
}

void Store::define_pool(const std::string &registry, Pool pool)
{
    std::lock_guard g(mu_);
    std::vector<std::pair<int64_t, int64_t>> within;
    for (auto &[lo, hi] : pool.ranges)
        if (lo <= std::min(hi, pool.cap))
            within.push_back({lo, std::min(hi, pool.cap)});
    pool.ranges = std::move(within);
    pools_[registry] = std::move(pool);
}

bool Store::has_pool(const std::string &registry) const
{
    std::lock_guard g(mu_);
    return pools_.count(registry) != 0;
}

void Store::set_live(const std::string &registry, const std::vector<std::string> &records)
{
    std::lock_guard g(mu_);
    live_[registry] = std::set<std::string>(records.begin(), records.end());
}

void Store::hold(const std::string &registry, std::map<int64_t, std::string> rows)
{
    std::lock_guard g(mu_);
    db_[registry] = std::move(rows);
}

bool Store::db_locked(const std::string &registry, int64_t id, const std::string &label, std::string *row) const
{
    auto it = db_.find(registry);
    if (it == db_.end())
        return false;
    auto r = it->second.find(id);
    if (r == it->second.end() || (!label.empty() && r->second == label))
        return false;
    if (row)
        *row = r->second;
    return true;
}

bool Store::database_holds(const std::string &registry, int64_t id, const std::string &label, std::string *row) const
{
    std::lock_guard g(mu_);
    return db_locked(registry, id, label, row);
}

bool Store::taken_locked(const std::string &registry, int64_t id, const std::string &record) const
{
    if (id < 0 || db_locked(registry, id, {}, nullptr))
        return true;
    std::string holder;
    if (held_locked(registry, id, &holder) && holder != record)
        return true;
    for (auto &[career, map] : careers_) {
        auto it = map.find(registry);
        if (it == map.end())
            continue;
        for (auto &[rec, v] : it->second)
            if (v == id && rec != record)
                return true;
    }
    return false;
}

int64_t Store::next_free_locked(const std::string &registry, const std::string &record) const
{
    auto it = pools_.find(registry);
    if (it == pools_.end())
        return -1;
    for (auto &[lo, hi] : it->second.ranges)
        for (int64_t id = lo; id <= hi; id++)
            if (!taken_locked(registry, id, record))
                return id;
    return -1;
}

std::string Store::moved_line(const std::string &registry, const std::string &record, const char *how, int64_t id,
                              const std::string &row, int64_t to) const
{
    return "[registry] ids: " + record + " " + how + " " + registry + " stat id " + std::to_string(id) + " is held by " +
           row + " from the database; " +
           (to >= 0 ? "uses " + std::to_string(to)
                    : "the " + pool_text_locked(registry) + " has no free id, so it keeps " + std::to_string(id));
}

bool Store::live_locked(const std::string &registry, const std::string &record) const
{
    auto it = live_.find(registry);
    return it == live_.end() || it->second.count(record);
}

bool Store::held_locked(const std::string &registry, int64_t id, std::string *holder) const
{
    auto it = ids_.find(registry);
    if (it == ids_.end())
        return false;
    for (auto &[rec, en] : it->second)
        if (en.id == id) {
            if (holder)
                *holder = rec;
            return true;
        }
    return false;
}

bool Store::career_uses_locked(const std::string &registry, int64_t id, const std::string &except) const
{
    for (auto &[career, map] : careers_) {
        if (career == except)
            continue;
        auto it = map.find(registry);
        if (it == map.end())
            continue;
        for (auto &[rec, v] : it->second)
            if (v == id)
                return true;
    }
    return false;
}

std::string Store::pool_text_locked(const std::string &registry) const
{
    auto it = pools_.find(registry);
    if (it == pools_.end())
        return "no pool";
    if (it->second.ranges.empty())
        return "pool (no id above the highest the game database holds)";
    std::string s = "pool ";
    bool first = true;
    for (auto &[lo, hi] : it->second.ranges) {
        s += first ? "" : ", ";
        first = false;
        s += lo == hi ? std::to_string(lo) : std::to_string(lo) + "-" + std::to_string(hi);
    }
    return s;
}

std::size_t Store::free_count_locked(const std::string &registry) const
{
    auto it = pools_.find(registry);
    if (it == pools_.end())
        return 0;
    std::size_t n = 0;
    for (auto &[lo, hi] : it->second.ranges)
        for (int64_t id = lo; id <= hi; id++)
            if (!held_locked(registry, id, nullptr) && !career_uses_locked(registry, id))
                n++;
    return n;
}

Result Store::get(const std::string &registry, const std::string &record) const
{
    std::lock_guard g(mu_);
    Result r;
    auto it = ids_.find(registry);
    if (it == ids_.end())
        return r;
    auto e = it->second.find(record);
    if (e == it->second.end())
        return r;
    auto pit = pools_.find(registry);
    if (pit != pools_.end() && e->second.id > pit->second.cap && !e->second.lent)
        return r;
    r.id = e->second.id;
    r.how = "kept";
    if (e->second.lent) {
        r.line = "[registry] stat ids: " + registry + " " + record + " -> " + std::to_string(r.id) +
                 " (lent by the host for this session)";
        return r;
    }
    r.line = "[registry] stat ids: " + registry + " " + record + " -> " + std::to_string(r.id) + " (kept from ids.json" +
             (e->second.since.empty() ? std::string(")") : ", since " + e->second.since + ")");
    return r;
}

Result Store::allocate(const std::string &registry, const std::string &record, const std::string &today)
{
    Result kept = get(registry, record);
    if (kept.id >= 0)
        return kept;
    std::lock_guard g(mu_);
    Result r;
    auto pit = pools_.find(registry);
    if (pit == pools_.end()) {
        r.how = "none";
        r.line = "[registry] stat ids: " + registry + " " + record + ": no stat-id pool for " + registry + " on this core";
        return r;
    }
    for (auto &[lo, hi] : pit->second.ranges)
        for (int64_t id = lo; id <= hi && r.id < 0; id++)
            if (!held_locked(registry, id, nullptr) && !career_uses_locked(registry, id) &&
                !db_locked(registry, id, {}, nullptr))
                r.id = id, r.how = "allocated";
    std::string reclaimed;
    if (r.id < 0)
        for (auto &[lo, hi] : pit->second.ranges)
            for (int64_t id = lo; id <= hi && r.id < 0; id++) {
                std::string holder;
                if (held_locked(registry, id, &holder) && !live_locked(registry, holder) &&
                    !career_uses_locked(registry, id) && !db_locked(registry, id, {}, nullptr)) {
                    ids_[registry].erase(holder);
                    reclaimed = holder;
                    r.id = id, r.how = "reclaimed";
                }
            }
    if (r.id < 0) {
        r.how = "full";
        r.line = "[registry] stat ids: " + registry + " " + record + " refused: the " + pool_text_locked(registry) +
                 (pit->second.ranges.empty() ? " leaves no id to give"
                                             : " is full (every id is held by a record or used by a career)");
        return r;
    }
    ids_[registry][record] = Entry{r.id, today};
    dirty_ = true;
    r.line = "[registry] stat ids: " + registry + " " + record + " -> " + std::to_string(r.id) + " (" +
             pool_text_locked(registry) + ", " + std::to_string(free_count_locked(registry)) + " free after this" +
             (reclaimed.empty() ? std::string() : "; reused from " + reclaimed + ", which no record or career uses") + ")";
    return r;
}

Result Store::pin(const std::string &registry, const std::string &record, int64_t id, const std::string &today,
                 const std::string &label)
{
    std::lock_guard g(mu_);
    Result r;
    r.id = id;
    r.how = "explicit";
    auto &recs = ids_[registry];
    auto e = recs.find(record);
    if (e != recs.end() && e->second.lent) {
        r.id = e->second.id;
        r.how = "kept";
        return r;
    }
    bool was_moved = e != recs.end() && e->second.pin == id && e->second.id != id;
    std::string row;
    if (label.empty() && was_moved) {
        r.id = e->second.id;
        r.how = "moved";
        return r;
    }
    if (!label.empty() && db_locked(registry, id, label, &row)) {
        int64_t to = was_moved && !taken_locked(registry, e->second.id, record) ? e->second.id
                                                                                 : next_free_locked(registry, record);
        r.line = moved_line(registry, record, "pinned", id, row, to);
        if (to >= 0) {
            if (!was_moved || e->second.id != to) {
                recs[record] = Entry{to, today, id};
                dirty_ = true;
            }
            r.id = to;
            r.how = "moved";
            return r;
        }
    }
    std::string holder;
    bool clash = false;
    for (auto &[rec, en] : recs)
        if (en.id == id && rec != record)
            holder = rec, clash = true;
    if (e == recs.end() || e->second.id != id || e->second.pin >= 0) {
        recs[record] = Entry{id, today};
        dirty_ = true;
    }
    if (r.line.empty())
        r.line = "[registry] stat ids: " + registry + " " + record + " -> " + std::to_string(id) + " (explicit" +
                 (clash ? "; " + holder + " also holds it" : std::string()) + ")";
    return r;
}

Result Store::recheck(const std::string &registry, const std::string &record, const std::string &label,
                      const std::string &today)
{
    std::lock_guard g(mu_);
    Result r;
    auto it = ids_.find(registry);
    if (it == ids_.end())
        return r;
    auto e = it->second.find(record);
    if (e == it->second.end() || e->second.pin >= 0)
        return r;
    r.id = e->second.id;
    r.how = "kept";
    if (e->second.lent)
        return r;
    std::string row;
    if (!db_locked(registry, r.id, label, &row))
        return r;
    int64_t to = next_free_locked(registry, record);
    r.line = moved_line(registry, record, "kept", r.id, row, to);
    if (to < 0)
        return r;
    e->second = Entry{to, today};
    dirty_ = true;
    r.id = to;
    r.how = "moved";
    return r;
}

CareerMap Store::map_for_career(const std::string &career)
{
    std::lock_guard g(mu_);
    CareerMap m;
    for (auto &[reg, recs] : ids_)
        for (auto &[rec, en] : recs)
            if (career_holds(reg) && live_locked(reg, rec))
                m[reg][rec] = en.id;
    if (!career.empty() && careers_[career] != m) {
        careers_[career] = m;
        dirty_ = true;
    }
    return m;
}

std::vector<Clash> Store::adopt(const std::string &career, const CareerMap &map, const std::string &today)
{
    std::lock_guard g(mu_);
    std::vector<Clash> out;
    for (auto &[reg, recs] : map)
        for (auto &[rec, id] : recs) {
            if (!career_holds(reg))
                continue;
            auto &mine = ids_[reg];
            auto e = mine.find(rec);
            if (e != mine.end() && e->second.id == id)
                continue;
            std::string holder, row;
            bool held = held_locked(reg, id, &holder) && holder != rec;
            bool in_db = !held && db_locked(reg, id, {}, &row);
            std::string who = held ? holder : in_db ? row + " from the database" : std::string();
            if (e != mine.end()) {
                Clash c{reg, rec, held ? holder : row, id, e->second.id, {}};
                c.line = career + " needs " + reg + " " + rec + " at stat id " + std::to_string(id) +
                         "; this install gives it " + std::to_string(e->second.id) +
                         (who.empty() ? std::string() : " and " + who + " holds " + std::to_string(id));
                out.push_back(c);
                continue;
            }
            if (held || in_db) {
                Clash c{reg, rec, held ? holder : row, id, -1, {}};
                c.line = career + " needs " + reg + " " + rec + " at stat id " + std::to_string(id) + "; " + who +
                         " holds it on this install";
                out.push_back(c);
                continue;
            }
            mine[rec] = Entry{id, today};
            dirty_ = true;
        }
    if (!career.empty() && out.empty()) {
        CareerMap held = map;
        held.erase("vehicle.package");
        careers_[career] = held;
        dirty_ = true;
    }
    return out;
}

void Store::forget_career(const std::string &career)
{
    std::lock_guard g(mu_);
    dirty_ = careers_.erase(career) || dirty_;
}

bool Store::dirty() const
{
    std::lock_guard g(mu_);
    return dirty_ && !lent_;
}

void Store::saved()
{
    std::lock_guard g(mu_);
    dirty_ = false;
}

std::size_t Store::deduped() const
{
    std::lock_guard g(mu_);
    return deduped_;
}

std::size_t Store::size() const
{
    std::lock_guard g(mu_);
    std::size_t n = 0;
    for (auto &[reg, recs] : ids_)
        n += recs.size();
    return n;
}

std::vector<int64_t> Store::ids_of(const std::string &registry) const
{
    std::lock_guard<std::mutex> g(mu_);
    std::vector<int64_t> out;
    auto it = ids_.find(registry);
    if (it != ids_.end())
        for (auto &[record, entry] : it->second)
            if (entry.id >= 0)
                out.push_back(entry.id);
    return out;
}

Store &store()
{
    static Store s;
    return s;
}

}
