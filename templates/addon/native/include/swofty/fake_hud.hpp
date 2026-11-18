// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <cstring>
#include <optional>
#include <string>
#include <vector>

#include <swofty/hud_frame.hpp>
#include <swofty/hud_leaves.hpp>
#include <swofty_api.h>

// Host test fakes for HUD code.
//
// fake::Hud installs a SwoftyHud table that records every write instead of drawing, with the real core's rules:
// the path prefix, one type per leaf, change detection, groups, and a type mismatch refused. Leaves, TowerLeaves
// and anything over swofty::hud::Leaves run unchanged against it.
//
//   swofty::fake::Hud hud;                       // version 2 core
//   swofty::fake::Hud old{swofty::fake::Hud::v1}; // a core without the leaf table: the v1 fallback runs
//   leaves.set(speed, "212"); leaves.flush();
//   assert(hud.text("view_model_data.hud_x_kmh") == "212");
//   hud.rebuild();                               // the game rebuilt its view model: everything is re-sent
//
// fake::Clock replaces the clock Frame reads, so cadences and cost run without sleeping.
//
//   swofty::fake::Clock clock;  clock.advance(0.5);
//
// fake::Live (fake_live.hpp) is the fake core live state for swofty::live.

namespace swofty::fake {

class Clock {
public:
    Clock() : prev_(clock::source)
    {
        current() = this;
        clock::source = [] { return current()->t_; };
    }
    ~Clock()
    {
        clock::source = prev_;
        current() = nullptr;
    }
    Clock(const Clock &) = delete;
    Clock &operator=(const Clock &) = delete;
    void advance(double s) { t_ += s; }
    void set(double s) { t_ = s; }
    double now() const { return t_; }

private:
    static Clock *&current()
    {
        static Clock *c = nullptr;
        return c;
    }
    double (*prev_)();
    double t_ = 0;
};

class Hud {
public:
    enum Version { v1 = 1, v2 = 2 };

    struct Write {
        std::string path;
        uint32_t type = 0;
        std::string text;
        int32_t number = 0;
        uint32_t group = 0;
        bool via_v1 = false;
    };

    explicit Hud(Version v = v2) : prev_(detail_override())
    {
        current() = this;
        table_ = {};
        table_.version = v;
        table_.available = [] { return 1; };
        table_.set = &Hud::c_set;
        if (v >= 2) {
            table_.leaves_register = &Hud::c_register;
            table_.leaves_set = &Hud::c_set_leaves;
            table_.leaves_begin = &Hud::c_begin;
            table_.leaves_end = &Hud::c_end;
            table_.generation = &Hud::c_generation;
            table_.stats = &Hud::c_stats;
        }
        detail_override() = &table_;
    }
    ~Hud()
    {
        detail_override() = prev_;
        current() = nullptr;
    }
    Hud(const Hud &) = delete;
    Hud &operator=(const Hud &) = delete;

    const std::vector<Write> &writes() const { return writes_; }
    void clear() { writes_.clear(); }
    size_t registered() const { return leaves_.size(); }
    uint32_t refused() const { return refused_; }

    std::optional<Write> last(const std::string &path) const
    {
        for (auto it = writes_.rbegin(); it != writes_.rend(); ++it)
            if (it->path == path)
                return *it;
        return std::nullopt;
    }
    std::string text(const std::string &path) const
    {
        auto w = last(path);
        return w ? w->text : std::string();
    }
    int32_t number(const std::string &path) const
    {
        auto w = last(path);
        return w ? w->number : 0;
    }
    int writes_to(const std::string &path) const
    {
        int n = 0;
        for (auto &w : writes_)
            n += w.path == path;
        return n;
    }

    // The game rebuilt its view model: the generation moves and every registered leaf that has a value is
    // written again, as the core does.
    void rebuild()
    {
        generation_++;
        for (auto &l : leaves_)
            if (l.has_value)
                record(l, 0, false);
    }
    uint32_t generation() const { return generation_; }

private:
    struct Leaf {
        std::string path;
        uint32_t type = 0, cap = 200;
        std::string text;
        int32_t number = 0;
        bool has_value = false;
    };

    static const SwoftyHud *&detail_override() { return hud::detail::leaf_table_override(); }
    static Hud *&current()
    {
        static Hud *h = nullptr;
        return h;
    }

    static bool path_ok(const std::string &p)
    {
        static const char *const prefixes[] = {"ui_menu_data.hud_", "view_model_data.hud.", "view_model_data.hud_"};
        bool listed = false;
        for (auto *x : prefixes)
            listed |= p.rfind(x, 0) == 0 && p.size() > std::strlen(x);
        return listed && p.find_first_of(" []") == std::string::npos && p.back() != '.';
    }

    void record(const Leaf &l, uint32_t group, bool v1)
    {
        Write w;
        w.path = l.path;
        w.type = l.type;
        w.text = l.text;
        w.number = l.number;
        w.group = group;
        w.via_v1 = v1;
        writes_.push_back(std::move(w));
    }

    Leaf *find(const std::string &path)
    {
        for (auto &l : leaves_)
            if (l.path == path)
                return &l;
        return nullptr;
    }

    static int c_set(const char *path, uint32_t type, const void *value)
    {
        Hud &h = *current();
        std::string p = path ? path : "";
        if (!path || !value || !path_ok(p) || type > SWOFTY_HUD_INT) {
            h.refused_++;
            return SWOFTY_ASSET_INVALID;
        }
        Leaf *reg = h.find(p);
        if (reg && reg->type != type) {
            h.refused_++;
            return SWOFTY_ASSET_INVALID;
        }
        Leaf l;
        l.path = p;
        l.type = type;
        if (type == SWOFTY_HUD_TEXT)
            l.text = static_cast<const char *>(value);
        else
            l.number = *static_cast<const int32_t *>(value) != 0 || type == SWOFTY_HUD_INT ? *static_cast<const int32_t *>(value) : 0;
        h.record(l, 0, true);
        return SWOFTY_ASSET_OK;
    }

    static int c_register(const SwoftyHudLeaf *leaves, uint32_t n, uint32_t *ids)
    {
        Hud &h = *current();
        int rc = SWOFTY_ASSET_OK;
        for (uint32_t i = 0; i < n; i++) {
            std::string p = leaves[i].path ? leaves[i].path : "";
            ids[i] = ~0u;
            if (!path_ok(p) || leaves[i].type > SWOFTY_HUD_INT) {
                rc = SWOFTY_ASSET_INVALID;
                h.refused_++;
                continue;
            }
            if (Leaf *e = h.find(p)) {
                if (e->type != leaves[i].type) {
                    rc = SWOFTY_ASSET_INVALID;
                    h.refused_++;
                    continue;
                }
                ids[i] = uint32_t(e - h.leaves_.data());
                continue;
            }
            Leaf l;
            l.path = p;
            l.type = leaves[i].type;
            l.cap = leaves[i].text_cap ? leaves[i].text_cap : 200;
            ids[i] = uint32_t(h.leaves_.size());
            h.leaves_.push_back(std::move(l));
        }
        return rc;
    }

    static int c_set_leaves(const uint32_t *ids, const void *const *values, uint32_t n)
    {
        Hud &h = *current();
        int rc = SWOFTY_ASSET_OK;
        for (uint32_t i = 0; i < n; i++) {
            if (ids[i] >= h.leaves_.size() || !values[i]) {
                rc = SWOFTY_ASSET_INVALID;
                continue;
            }
            Leaf &l = h.leaves_[ids[i]];
            bool changed;
            if (l.type == SWOFTY_HUD_TEXT) {
                std::string t = static_cast<const char *>(values[i]);
                if (t.size() > l.cap)
                    t.resize(l.cap);
                changed = !l.has_value || t != l.text;
                l.text = t;
            } else {
                int32_t v = *static_cast<const int32_t *>(values[i]);
                if (l.type == SWOFTY_HUD_BOOL)
                    v = v != 0;
                changed = !l.has_value || v != l.number;
                l.number = v;
            }
            l.has_value = true;
            if (changed)
                h.record(l, h.depth_ ? h.group_ : 0, false);
        }
        return rc;
    }

    static int c_begin()
    {
        Hud &h = *current();
        if (h.depth_++ == 0)
            h.group_ = ++h.groups_;
        return SWOFTY_ASSET_OK;
    }
    static int c_end()
    {
        Hud &h = *current();
        if (h.depth_ > 0)
            h.depth_--;
        return SWOFTY_ASSET_OK;
    }
    static uint32_t c_generation() { return current()->generation_; }
    static int c_stats(SwoftyHudStats *out)
    {
        Hud &h = *current();
        out->applied_last_tick = uint32_t(h.writes_.size());
        out->registered = uint32_t(h.leaves_.size());
        out->generation = h.generation_;
        out->cap_per_tick = 0xffffffffu;
        return SWOFTY_ASSET_OK;
    }

    const SwoftyHud *prev_;
    SwoftyHud table_{};
    std::vector<Leaf> leaves_;
    std::vector<Write> writes_;
    uint32_t generation_ = 1, groups_ = 0, group_ = 0, refused_ = 0;
    int depth_ = 0;
};

}
