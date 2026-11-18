// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <map>
#include <string>
#include <string_view>
#include <vector>

#include <swofty/hud_format.hpp>
#include <swofty_api.h>
#if defined(_WIN32)
#include <swofty/hud.hpp>
#endif

// Typed HUD leaves over the core's registered leaf table (SwoftyHud version 2). Declare a leaf once with its
// type, set it every frame, flush once. The core remembers what it last wrote, writes only what changed,
// applies a whole group in one tick and re-sends everything itself when the game rebuilds its view model.
// On a core without version 2 the same code runs over hud's plain set: changes are detected here, writes are
// capped per flush and every leaf is refreshed every two seconds.
//
// Leaf layout and the Halo-style groups (one-hot, levels, rows) follow Project Halo's leaves.hpp and
// tower_leaves.cpp. Uses work from Project Halo by nyannoying, https://github.com/nyannoying1337/f1-25-project-halo

namespace swofty::hud {

namespace detail {

inline const SwoftyHud *&leaf_table_override()
{
    static const SwoftyHud *t = nullptr;
    return t;
}

inline const SwoftyHud *leaf_table()
{
    if (auto *o = leaf_table_override())
        return o;
#if defined(_WIN32)
    return table();
#else
    return nullptr;
#endif
}

inline std::string pad_index(int i, int width)
{
    char b[16];
    std::snprintf(b, sizeof b, "%0*d", width, i);
    return b;
}

}

template <class T> struct Leaf {
    uint32_t id = ~0u;
    explicit operator bool() const { return id != ~0u; }
};

using BoolLeaf = Leaf<bool>;
using IntLeaf = Leaf<int32_t>;
using TextLeaf = Leaf<std::string_view>;

struct OneHot {
    uint32_t first = 0, count = 0;
};
struct Level {
    uint32_t first = 0, count = 0;
};

struct LeafStats {
    uint32_t applied_last_tick = 0, queued = 0, dropped = 0, registered = 0, cap_per_tick = 0, failed = 0;
    uint32_t generation = 0, resyncs = 0;
};

class Leaves {
public:
    static constexpr uint32_t default_text_cap = 200;
    static constexpr int v1_writes_per_flush = 40;
    static constexpr double v1_refresh_s = 2.0;

    explicit Leaves(std::string_view prefix) : prefix_(prefix) {}

    BoolLeaf boolean(std::string_view key) { return {add(key, SWOFTY_HUD_BOOL, 0)}; }
    IntLeaf integer(std::string_view key) { return {add(key, SWOFTY_HUD_INT, 0)}; }
    TextLeaf text(std::string_view key, uint32_t cap = default_text_cap) { return {add(key, SWOFTY_HUD_TEXT, cap)}; }

    // stem + index: one_hot("g", 9) is g0..g8, one_hot("rv", 24, 2) is rv00..rv23. Exactly one is true.
    OneHot one_hot(std::string_view stem, int count, int width = 0, int start = 0)
    {
        return {group(stem, count, width, start), uint32_t(count)};
    }
    // Cumulative bars: level("th", 10) is th01..th10; set(level, 3) lights th01..th03.
    Level level(std::string_view stem, int count, int width = 2, int start = 1)
    {
        return {group(stem, count, width, start), uint32_t(count)};
    }

    void set(BoolLeaf l, bool v) { want_int(l.id, v); }
    void set(IntLeaf l, int32_t v) { want_int(l.id, v); }
    void set(TextLeaf l, std::string_view v) { want_text(l.id, v); }
    void set(OneHot g, int selected)
    {
        for (uint32_t i = 0; i < g.count; i++)
            want_int(g.first + i, int(i) == selected);
    }
    void set(Level g, int lit)
    {
        for (uint32_t i = 0; i < g.count; i++)
            want_int(g.first + i, int(i) < lit);
    }

    // Everything set while an Atomic lives is one group: the core applies it whole in one tick. A table row.
    class Atomic {
    public:
        explicit Atomic(Leaves &l) : l_(l), prev_(l.group_) { l.group_ = ++l.next_group_; }
        ~Atomic() { l_.group_ = prev_; }
        Atomic(const Atomic &) = delete;
        Atomic &operator=(const Atomic &) = delete;

    private:
        Leaves &l_;
        uint32_t prev_;
    };

    // A repeated record: rows<Row>("r", 22, 2, make) builds Row make(Leaves &, int index) with every leaf
    // under "r00_", "r01_"... Built once; nothing allocates afterwards.
    template <class Row> class Rows {
    public:
        Row &operator[](int i) { return rows_[size_t(i)]; }
        const Row &operator[](int i) const { return rows_[size_t(i)]; }
        int size() const { return int(rows_.size()); }

    private:
        friend class Leaves;
        std::vector<Row> rows_;
    };

    template <class Row, class Make> Rows<Row> rows(std::string_view stem, int count, int width, Make make)
    {
        Rows<Row> out;
        std::string saved = scope_;
        for (int i = 0; i < count; i++) {
            scope_ = saved + std::string(stem) + detail::pad_index(i, width) + "_";
            out.rows_.push_back(make(*this, i));
        }
        scope_ = saved;
        return out;
    }

    // Hand every changed leaf to the core. Returns how many were handed over. A failure is logged once per
    // leaf by the core; a core without leaf support makes this a no-op that returns 0.
    int flush()
    {
        auto *t = detail::leaf_table();
        if (!t || t->version < 1)
            return 0;
        if (t->version >= 2 && t->leaves_register)
            return flush_v2(t);
        return flush_v1(t);
    }

    bool ready() const { return registered_; }
    size_t size() const { return entries_.size(); }
    std::vector<std::string> paths() const
    {
        std::vector<std::string> out;
        for (auto &e : entries_)
            out.push_back(e.path);
        return out;
    }
    uint32_t type_of(uint32_t index) const { return entries_[index].type; }

    // The core's rebuild counter; 0 on a core without version 2. Compare to a stored value to learn the game
    // rebuilt its view model.
    static uint32_t generation()
    {
        auto *t = detail::leaf_table();
        return t && t->version >= 2 && t->generation ? t->generation() : 0;
    }

    static bool stats(LeafStats &out)
    {
        auto *t = detail::leaf_table();
        if (!t || t->version < 2 || !t->stats)
            return false;
        SwoftyHudStats s{};
        s.size = sizeof s;
        if (t->stats(&s) != SWOFTY_ASSET_OK)
            return false;
        out = {s.applied_last_tick, s.queued, s.dropped, s.registered, s.cap_per_tick, s.failed, s.generation, s.resyncs};
        return true;
    }

private:
    struct Entry {
        std::string path;
        uint32_t type = SWOFTY_HUD_TEXT, cap = default_text_cap, core_id = ~0u, group = 0;
        std::string want_text, sent_text;
        int32_t want_num = 0, sent_num = 0;
        bool has_want = false, has_sent = false, dirty = false;
    };

    uint32_t add(std::string_view key, uint32_t type, uint32_t cap)
    {
        Entry e;
        e.path = prefix_ + scope_ + std::string(key);
        e.type = type;
        e.cap = cap ? std::min(cap, default_text_cap) : default_text_cap;
        entries_.push_back(std::move(e));
        registered_ = false;
        return uint32_t(entries_.size() - 1);
    }

    uint32_t group(std::string_view stem, int count, int width, int start)
    {
        uint32_t first = uint32_t(entries_.size());
        for (int i = 0; i < count; i++)
            add(std::string(stem) + detail::pad_index(start + i, width), SWOFTY_HUD_BOOL, 0);
        return first;
    }

    void want_int(uint32_t id, int32_t v)
    {
        if (id >= entries_.size())
            return;
        Entry &e = entries_[id];
        if (e.type == SWOFTY_HUD_BOOL)
            v = v != 0;
        if (e.has_want && e.want_num == v)
            return;
        e.want_num = v;
        mark(e);
    }

    void want_text(uint32_t id, std::string_view v)
    {
        if (id >= entries_.size())
            return;
        Entry &e = entries_[id];
        v = format::fit(v, e.cap);
        if (e.has_want && e.want_text == v)
            return;
        e.want_text.assign(v);
        mark(e);
    }

    void mark(Entry &e)
    {
        e.has_want = true;
        e.group = group_;
        bool differs = !e.has_sent || (e.type == SWOFTY_HUD_TEXT ? e.want_text != e.sent_text : e.want_num != e.sent_num);
        if (differs && !e.dirty) {
            e.dirty = true;
            dirty_n_++;
        } else if (!differs && e.dirty) {
            e.dirty = false;
            dirty_n_--;
        }
    }

    void sent(Entry &e)
    {
        e.has_sent = true;
        e.sent_text = e.want_text;
        e.sent_num = e.want_num;
        if (e.dirty) {
            e.dirty = false;
            dirty_n_--;
        }
    }

    const void *value_of(const Entry &e) const
    {
        return e.type == SWOFTY_HUD_TEXT ? static_cast<const void *>(e.want_text.c_str())
                                         : static_cast<const void *>(&e.want_num);
    }

    bool register_all(const SwoftyHud *t)
    {
        if (registered_ && core_table_ == t)
            return true;
        std::vector<SwoftyHudLeaf> specs;
        std::vector<std::string> paths;
        specs.reserve(entries_.size());
        for (auto &e : entries_)
            specs.push_back({sizeof(SwoftyHudLeaf), e.path.c_str(), e.type, e.cap});
        std::vector<uint32_t> ids(entries_.size(), ~0u);
        int rc = specs.empty() ? SWOFTY_ASSET_OK : t->leaves_register(specs.data(), uint32_t(specs.size()), ids.data());
        for (size_t i = 0; i < entries_.size(); i++)
            entries_[i].core_id = ids[i];
        if (rc == SWOFTY_ASSET_UNAVAILABLE || rc == SWOFTY_ASSET_LATE)
            return false;
        registered_ = true;
        core_table_ = t;
        for (auto &e : entries_)
            if (e.has_want && !e.dirty) {
                e.has_sent = false;
                e.dirty = true;
                dirty_n_++;
            }
        return true;
    }

    int flush_v2(const SwoftyHud *t)
    {
        if (!register_all(t))
            return 0;
        if (!dirty_n_)
            return 0;
        std::map<uint32_t, std::vector<uint32_t>> by_group;
        for (uint32_t i = 0; i < entries_.size(); i++)
            if (entries_[i].dirty && entries_[i].core_id != ~0u)
                by_group[entries_[i].group].push_back(i);
        int n = 0;
        for (auto &[g, idx] : by_group) {
            ids_.clear();
            vals_.clear();
            for (uint32_t i : idx) {
                ids_.push_back(entries_[i].core_id);
                vals_.push_back(value_of(entries_[i]));
            }
            if (g && t->leaves_begin)
                t->leaves_begin();
            t->leaves_set(ids_.data(), vals_.data(), uint32_t(ids_.size()));
            if (g && t->leaves_end)
                t->leaves_end();
            for (uint32_t i : idx)
                sent(entries_[i]);
            n += int(idx.size());
        }
        return n;
    }

    int flush_v1(const SwoftyHud *t)
    {
        if (!t->set)
            return 0;
        double now = v1_clock_();
        if (now - v1_last_refresh_ >= v1_refresh_s) {
            v1_last_refresh_ = now;
            for (auto &e : entries_)
                if (e.has_want && !e.dirty) {
                    e.dirty = true;
                    dirty_n_++;
                }
        }
        int n = 0;
        for (size_t k = 0; k < entries_.size() && n < v1_writes_per_flush; k++) {
            Entry &e = entries_[(v1_cursor_ + k) % entries_.size()];
            if (!e.dirty)
                continue;
            if (t->set(e.path.c_str(), e.type, value_of(e)) == SWOFTY_ASSET_OK)
                sent(e);
            n++;
            v1_next_ = (v1_cursor_ + k + 1) % entries_.size();
        }
        v1_cursor_ = v1_next_;
        return n;
    }

    static double v1_clock_();

    std::string prefix_, scope_;
    std::vector<Entry> entries_;
    std::vector<uint32_t> ids_;
    std::vector<const void *> vals_;
    uint32_t group_ = 0, next_group_ = 0;
    uint32_t dirty_n_ = 0;
    size_t v1_cursor_ = 0, v1_next_ = 0;
    double v1_last_refresh_ = -1e9;
    bool registered_ = false;
    const SwoftyHud *core_table_ = nullptr;
};

inline double Leaves::v1_clock_()
{
    return std::chrono::duration<double>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

}
