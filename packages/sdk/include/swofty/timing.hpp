// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <optional>
#include <span>
#include <string>
#include <string_view>

#include <swofty/ease.hpp>
#include <swofty/hud_format.hpp>
#include <swofty/hud_leaves.hpp>
#include <swofty/live.hpp>

// The timing tower model: classification order in, tower rows out. Pure and deterministic over plain structs,
// so it runs on the host in tests and needs no game access. Header only.
//
// Car and Session below are the tower's own POD input, not the core's live structs: from_live() (end of this
// header) fills them from swofty::live (SwoftyLiveCar / SwoftyLiveSession, native memory reads), and
// update_from_live() does the whole read-and-update in one call. Field names follow the proposed live structs (position, lap,
// gap_ahead_ms, gap_leader_ms, tyre, time_penalty_s, drive_through, stop_go, result_status -> status, pit_state
// -> pit_state, safety_car/red_flag/yellow_sectors/chequered -> Session). Fill them by hand for your own reader, or
// from swofty::fake in tests.
//
// What each field means is the contract of the live layer, not of this model: the model never reads memory, so
// quirks of the readers (a stale tyre cache, a torn classification read, the yellow zones blinking) belong
// there. Hold<T> is available for what an addon still needs to hold itself.
//
// The tower rules and the leaf contract (hud_tt_*, rNN_*, cVV_row) are Project Halo's: its timing tower
// (tower_model.cpp, tower_leaves.cpp, docs/timing-tower/leaves.md v5). Uses work from Project Halo by
// nyannoying, https://github.com/nyannoying1337/f1-25-project-halo

namespace swofty::timing {

constexpr int max_cars = 24;
constexpr int max_rows = 24;
constexpr int default_rows = 22;

// result status numbering of the game: 2 running, 3 finished, 4 dnf, 5 disqualified, 6 not classified, 7 retired.
enum Status : uint8_t { unknown = 0, inactive = 1, running = 2, finished = 3, dnf = 4, dsq = 5, nc = 6, retired = 7 };

struct Car {
    uint32_t size = sizeof(Car), version = 1;
    uint8_t index = 0;
    uint8_t position = 0;
    uint8_t status = unknown;
    uint8_t pit_state = 0;
    uint8_t in_pit = 0;
    int32_t lap = -1;
    int64_t gap_ahead_ms = -1, gap_leader_ms = -1;
    uint8_t gap_ahead_valid = 0, gap_leader_valid = 0;
    char tyre = 0;
    float time_penalty_s = 0;
    uint8_t drive_through = 0, stop_go = 0;
    int32_t best_lap_ms = -1, last_lap_ms = -1;
    int32_t best_sector_ms[3] = {-1, -1, -1};
    uint8_t purple_lap = 0, purple_sector[3] = {}, green_sector[3] = {};
    char team_key[32] = {};
    char code[4] = {};
};

struct Session {
    uint32_t size = sizeof(Session), version = 1;
    uint32_t type = 0;
    int32_t laps_total = 0;
    enum Flag : uint8_t { none, safety_car, vsc, red, chequered, yellow };
    Flag header = none;
    uint16_t yellow_sectors = 0;
    bool yellow_sector_unknown = false;
    bool time_trial = false;
};

struct Config {
    enum class Mode : uint8_t { full, compact, top } mode = Mode::compact;
    int rows = 5;
    enum class Gap : uint8_t { interval, leader, autoswitch } gap = Gap::interval;
    double auto_period_s = 10;
    bool arrows = true, tyres = true, gaps = true, highlight = true;
    double arrow_seconds = 4;
    double tyre_hold_s = 1.0;
    int64_t max_gap_ms = 3ll * 3600 * 1000;
};

enum class GapKind : uint8_t { off, word, time, lapped, pit_stop, out_state, unavailable };

struct Row {
    bool on = false;
    int car = -1;
    int pos = 0;
    bool leader = false, up = false, down = false, me = false, pit = false, penalty = false, out = false;
    char gap[24] = {};
    bool gap_na = false;
    GapKind gap_kind = GapKind::off;
    char tyre = 0;
    bool tyre_na = false;
    bool purple_lap = false, purple_sector[3] = {}, green_sector[3] = {};
    char team_key[32] = {};
    char code[4] = {};
};

inline void window(int count, int n, int subject_index, Config::Mode mode, int &start, int &rows)
{
    start = 0;
    rows = 0;
    if (count <= 0)
        return;
    n = std::clamp(n, 1, max_rows);
    if (mode == Config::Mode::full) {
        rows = std::min(count, max_rows);
        return;
    }
    rows = std::min(n, count);
    if (mode == Config::Mode::compact && subject_index >= 0)
        start = std::clamp(subject_index - n / 2, 0, count - rows);
}

// The car one place ahead (P1 sees P2: the leader's rival is the car behind), by classification order.
inline std::optional<int> car_ahead(std::span<const Car> order, int subject)
{
    for (size_t i = 0; i < order.size(); i++)
        if (order[i].index == subject) {
            if (order.size() < 2)
                return std::nullopt;
            return order[i == 0 ? 1 : i - 1].index;
        }
    return std::nullopt;
}

inline std::optional<int> car_behind(std::span<const Car> order, int subject)
{
    for (size_t i = 0; i < order.size(); i++)
        if (order[i].index == subject) {
            if (i + 1 >= order.size())
                return std::nullopt;
            return order[i + 1].index;
        }
    return std::nullopt;
}

class Tower {
public:
    explicit Tower(Config c = {}) : cfg_(c) {}

    // New session or car set: arrows, holds and the gap-mode clock start afresh.
    void reset()
    {
        for (auto &p : prev_pos_)
            p = 0;
        for (auto &a : arrow_)
            a = {};
        for (auto &h : tyre_hold_)
            h.reset();
        rows_ = {};
        count_ = start_ = nrows_ = 0;
        car_row_.fill(-1);
        t_auto_ = -1;
    }

    // order is the classification, first place first. subject is the highlighted vehicle id (-1 none).
    void update(const Session &s, std::span<const Car> order, int subject, double now, const Config &cfg)
    {
        cfg_ = cfg;
        session_ = s;
        rows_ = {};
        car_row_.fill(-1);
        count_ = int(std::min<size_t>(order.size(), 64));
        on_ = !s.time_trial && count_ > 0;
        race_ = hud::format::is_race(s.type);
        timed_ = hud::format::is_timed(s.type);
        if (t_auto_ < 0)
            t_auto_ = now;
        lead_ = cfg.gap == Config::Gap::leader ||
                (cfg.gap == Config::Gap::autoswitch && int((now - t_auto_) / std::max(1.0, cfg.auto_period_s)) % 2 == 1);
        if (!on_) {
            start_ = nrows_ = 0;
            return;
        }
        int si = -1;
        for (int i = 0; i < count_; i++)
            if (order[size_t(i)].index == subject)
                si = i;
        window(count_, cfg.rows, si, cfg.mode, start_, nrows_);
        leader_ = &order[0];
        for (int r = 0; r < nrows_; r++) {
            int idx = start_ + r;
            fill(rows_[size_t(r)], order, idx, subject, now);
            if (rows_[size_t(r)].car >= 0 && rows_[size_t(r)].car < max_cars)
                car_row_[size_t(rows_[size_t(r)].car)] = r;
        }
        for (int i = 0; i < count_; i++) {
            int v = order[size_t(i)].index;
            if (v < max_cars)
                prev_pos_[size_t(v)] = order[size_t(i)].position ? order[size_t(i)].position : i + 1;
        }
    }

    int count() const { return count_; }
    int start() const { return start_; }
    int rows() const { return nrows_; }
    const Row &row(int i) const { return rows_[size_t(i)]; }
    int row_of_car(int car) const { return car >= 0 && car < max_cars ? car_row_[size_t(car)] : -1; }
    bool on() const { return on_; }
    bool lead_gaps() const { return lead_; }
    bool race() const { return race_; }
    bool timed() const { return timed_; }
    const Session &session() const { return session_; }
    int leader_lap() const { return leader_ ? leader_->lap : -1; }

private:
    struct Arrow {
        int dir = 0;
        ease::Pulse pulse;
    };

    void fill(Row &r, std::span<const Car> order, int idx, int subject, double now)
    {
        const Car &c = order[size_t(idx)];
        r.on = true;
        r.car = c.index;
        r.pos = c.position ? c.position : idx + 1;
        r.leader = r.pos == 1;
        r.me = cfg_.highlight && c.index == subject;
        r.pit = c.in_pit != 0;
        r.penalty = c.time_penalty_s > 0 || c.drive_through || c.stop_go;
        r.out = c.status == dnf || c.status == dsq || c.status == nc || c.status == retired;
        std::snprintf(r.team_key, sizeof r.team_key, "%s", c.team_key);
        std::snprintf(r.code, sizeof r.code, "%s", c.code);
        r.purple_lap = c.purple_lap;
        for (int k = 0; k < 3; k++) {
            r.purple_sector[k] = c.purple_sector[k];
            r.green_sector[k] = c.green_sector[k];
        }
        if (c.index < max_cars) {
            Arrow &a = arrow_[size_t(c.index)];
            int was = prev_pos_[size_t(c.index)];
            if (cfg_.arrows && was && was != r.pos) {
                a.dir = r.pos < was ? 1 : -1;
                a.pulse.trigger(now, cfg_.arrow_seconds);
            }
            if (cfg_.arrows && a.pulse.on(now)) {
                r.up = a.dir > 0;
                r.down = a.dir < 0;
            }
            if (cfg_.tyres) {
                if (c.tyre)
                    tyre_hold_[size_t(c.index)].set(c.tyre, now);
                auto h = tyre_hold_[size_t(c.index)].get(now, cfg_.tyre_hold_s);
                if (h)
                    r.tyre = *h;
                else if (!r.out)
                    r.tyre_na = true;
            }
        }
        fill_gap(r, c, order, idx);
    }

    void fill_gap(Row &r, const Car &c, std::span<const Car> order, int idx)
    {
        if (!cfg_.gaps) {
            r.gap_kind = GapKind::off;
            return;
        }
        if (r.out) {
            std::snprintf(r.gap, sizeof r.gap, "%s", c.status == dsq ? "DSQ" : c.status == nc ? "NC" : "Out");
            r.gap_kind = GapKind::out_state;
            return;
        }
        r.gap_kind = GapKind::unavailable;
        r.gap_na = true;
        if (!race_) {
            if (timed_ && c.best_lap_ms > 0 && leader_->best_lap_ms > 0) {
                if (r.leader)
                    hud::format::lap_time(c.best_lap_ms, r.gap, sizeof r.gap);
                else
                    hud::format::gap(c.best_lap_ms - leader_->best_lap_ms, r.gap, sizeof r.gap);
                r.gap_kind = r.leader ? GapKind::word : GapKind::time;
                r.gap_na = false;
            }
            return;
        }
        if (r.leader) {
            std::snprintf(r.gap, sizeof r.gap, "%s", lead_ ? "Leader" : "Interval");
            r.gap_kind = GapKind::word;
            r.gap_na = false;
            return;
        }
        bool valid = lead_ ? c.gap_leader_valid : c.gap_ahead_valid;
        int64_t ms = lead_ ? c.gap_leader_ms : c.gap_ahead_ms;
        if (valid && (c.status == running || c.status == finished) && ms >= 0 && ms <= cfg_.max_gap_ms) {
            hud::format::gap(ms, r.gap, sizeof r.gap);
            r.gap_kind = GapKind::time;
            r.gap_na = false;
            return;
        }
        if (valid)
            return;
        if (c.in_pit) {
            std::snprintf(r.gap, sizeof r.gap, "PIT");
            r.gap_kind = GapKind::pit_stop;
            r.gap_na = false;
            return;
        }
        const Car *ref = lead_ ? leader_ : (idx > 0 ? &order[size_t(idx - 1)] : nullptr);
        if (c.status == running && ref && ref->lap >= 0 && c.lap >= 0) {
            int n = ref->lap - c.lap;
            if (n >= 1 && n <= 99) {
                hud::format::lapped(n, r.gap, sizeof r.gap);
                r.gap_kind = GapKind::lapped;
                r.gap_na = false;
            }
        }
    }

    Config cfg_;
    Session session_;
    std::array<Row, max_rows> rows_{};
    std::array<int, max_cars> car_row_{};
    std::array<int, max_cars> prev_pos_{};
    std::array<Arrow, max_cars> arrow_{};
    std::array<ease::Hold<char>, max_cars> tyre_hold_{};
    const Car *leader_ = nullptr;
    int count_ = 0, start_ = 0, nrows_ = 0;
    bool on_ = false, lead_ = false, race_ = false, timed_ = false;
    double t_auto_ = -1;
};

// The standard tower leaves (Halo's contract, v5): globals under prefix, 24 car-to-row ints (cVV_row) and
// rows (default_rows, at most max_rows) rows of rNN_{on,vid,pos,p1,up,dn,gap,gna,tyre,tS..tW,tna,pit,pen,out,me,code,badge,bdg,cVV}. Declared
// once; publish() sets them from a Tower and each row goes out as one atomic group.
class TowerLeaves {
public:
    struct RowLeaves {
        hud::BoolLeaf on, p1, up, dn, gna, tS, tM, tH, tI, tW, tna, pit, pen, out, me, bdg;
        hud::IntLeaf vid;
        hud::TextLeaf pos, gap, tyre, code, badge;
        hud::OneHot car;
    };

    explicit TowerLeaves(hud::Leaves &leaves, int rows = default_rows, int cars = max_cars)
        : L(leaves), nrows_(std::min(rows, max_rows)), ncars_(std::min(cars, max_cars))
    {
        on = L.boolean("on");
        trial = L.boolean("trial");
        show = L.boolean("show");
        rows_n = L.integer("rows");
        lead = L.boolean("lead");
        race = L.boolean("race");
        timed = L.boolean("timed");
        sess = L.text("sess", 8);
        lap = L.text("lap", 8);
        lapna = L.boolean("lapna");
        hs = L.integer("hs");
        sc = L.boolean("sc");
        vsc = L.boolean("vsc");
        chq = L.boolean("chq");
        red = L.boolean("red");
        yel = L.boolean("yel");
        ysec = L.text("ysec", 31);
        ysna = L.boolean("ysna");
        for (int v = 0; v < ncars_; v++) {
            char key[24];
            std::snprintf(key, sizeof key, "c%02d_row", v);
            car_row_[size_t(v)] = L.integer(key);
        }
        for (int r = 0; r < nrows_; r++) {
            char stem[16];
            std::snprintf(stem, sizeof stem, "r%02d_", r);
            RowLeaves &w = row_[size_t(r)];
            auto k = [&](const char *name) { return std::string(stem) + name; };
            w.on = L.boolean(k("on"));
            w.vid = L.integer(k("vid"));
            w.pos = L.text(k("pos"), 8);
            w.p1 = L.boolean(k("p1"));
            w.up = L.boolean(k("up"));
            w.dn = L.boolean(k("dn"));
            w.gap = L.text(k("gap"), 15);
            w.gna = L.boolean(k("gna"));
            w.tyre = L.text(k("tyre"), 2);
            w.tS = L.boolean(k("tS"));
            w.tM = L.boolean(k("tM"));
            w.tH = L.boolean(k("tH"));
            w.tI = L.boolean(k("tI"));
            w.tW = L.boolean(k("tW"));
            w.tna = L.boolean(k("tna"));
            w.pit = L.boolean(k("pit"));
            w.pen = L.boolean(k("pen"));
            w.out = L.boolean(k("out"));
            w.me = L.boolean(k("me"));
            w.code = L.text(k("code"), 4);
            w.badge = L.text(k("badge"), 31);
            w.bdg = L.boolean(k("bdg"));
            w.car = L.one_hot(std::string(stem) + "c", ncars_, 2, 0);
        }
    }

    void publish(const Tower &t)
    {
        const Session &s = t.session();
        bool live = t.on();
        L.set(on, live);
        L.set(trial, s.time_trial);
        L.set(show, live || s.time_trial);
        L.set(rows_n, live ? t.rows() : 0);
        L.set(lead, t.lead_gaps());
        L.set(race, live && t.race());
        L.set(timed, live && t.timed());
        L.set(sess, live ? hud::format::session_label(s.type) : "");
        L.set(lap, live && t.race() && t.leader_lap() >= 0 ? std::to_string(t.leader_lap() + 1) : std::string());
        L.set(lapna, live && t.race() && t.leader_lap() < 0);
        int h = !live ? 0 : s.header == Session::chequered ? 4 : s.header == Session::red ? 3 : s.header == Session::safety_car ? 1
                : s.header == Session::vsc ? 2 : s.header == Session::yellow ? 5 : 0;
        L.set(hs, h);
        L.set(sc, live && s.header == Session::safety_car);
        L.set(vsc, live && s.header == Session::vsc);
        L.set(chq, live && s.header == Session::chequered);
        L.set(red, live && s.header == Session::red);
        L.set(yel, live && s.header == Session::yellow);
        L.set(ysec, live && s.header == Session::yellow ? yellow_text(s.yellow_sectors) : std::string());
        L.set(ysna, live && s.header == Session::yellow && s.yellow_sector_unknown);
        for (int v = 0; v < ncars_; v++)
            L.set(car_row_[size_t(v)], live ? t.row_of_car(v) : -1);
        for (int r = 0; r < nrows_; r++) {
            hud::Leaves::Atomic group(L);
            RowLeaves &w = row_[size_t(r)];
            const Row *row = live && r < t.rows() ? &t.row(r) : nullptr;
            L.set(w.on, row != nullptr);
            L.set(w.vid, row ? row->car : -1);
            char pos[8] = {};
            if (row)
                std::snprintf(pos, sizeof pos, "%d", row->pos);
            L.set(w.pos, pos);
            L.set(w.p1, row && row->leader);
            L.set(w.up, row && row->up);
            L.set(w.dn, row && row->down);
            L.set(w.gap, row ? row->gap : "");
            L.set(w.gna, row && row->gap_na && row->gap_kind == GapKind::unavailable);
            char tyre[2] = {row ? row->tyre : char(0), 0};
            L.set(w.tyre, tyre);
            L.set(w.tS, tyre[0] == 'S');
            L.set(w.tM, tyre[0] == 'M');
            L.set(w.tH, tyre[0] == 'H');
            L.set(w.tI, tyre[0] == 'I');
            L.set(w.tW, tyre[0] == 'W');
            L.set(w.tna, row && row->tyre_na);
            L.set(w.pit, row && row->pit);
            L.set(w.pen, row && row->penalty);
            L.set(w.out, row && row->out);
            L.set(w.me, row && row->me);
            L.set(w.code, row ? row->code : "");
            L.set(w.badge, row ? row->team_key : "");
            L.set(w.bdg, row && row->team_key[0]);
            L.set(w.car, row ? row->car : -1);
        }
    }

    static std::string yellow_text(uint16_t sectors)
    {
        std::string out;
        for (int i = 0; i < 16; i++)
            if (sectors >> i & 1) {
                if (!out.empty())
                    out += ' ';
                out += std::to_string(i + 1);
            }
        return out;
    }

private:
    hud::Leaves &L;
    int nrows_, ncars_;
    hud::BoolLeaf on, trial, show, lead, race, timed, lapna, sc, vsc, chq, red, yel, ysna;
    hud::IntLeaf rows_n, hs;
    hud::TextLeaf sess, lap, ysec;
    std::array<hud::IntLeaf, max_cars> car_row_{};
    std::array<RowLeaves, max_rows> row_{};
};

// Live adapter. A field the live layer does not give (its have bit clear, or no source yet) is left empty:
// names (code), lap total, lap and sector times, best laps and the purple and green flags stay at their
// defaults; the tower shows the gap of those as unavailable.

inline Session::Flag header_flag(const live::Session &s)
{
    if (s.chequered)
        return Session::chequered;
    if (live::has(s, SWOFTY_LIVE_SHAVE_RED_FLAG) && s.red_flag)
        return Session::red;
    switch (live::safety_car(s)) {
    case live::SafetyCar::safety_car:
        return Session::safety_car;
    case live::SafetyCar::virtual_safety_car:
        return Session::vsc;
    default:
        break;
    }
    if (live::has(s, SWOFTY_LIVE_SHAVE_YELLOW) && s.yellow_sectors)
        return Session::yellow;
    return Session::none;
}

inline void from_live(const live::Session &s, Session &out)
{
    out = Session{};
    out.type = live::has(s, SWOFTY_LIVE_SHAVE_TYPE) ? s.type : 0;
    out.header = header_flag(s);
    out.yellow_sectors = live::has(s, SWOFTY_LIVE_SHAVE_YELLOW) ? s.yellow_sectors : 0;
    out.yellow_sector_unknown = !live::has(s, SWOFTY_LIVE_SHAVE_YELLOW);
    out.time_trial = live::game_mode(s) == live::GameMode::time_trial;
}

inline void from_live(const live::Car &c, Car &out, std::string_view team_key = {})
{
    out = Car{};
    out.index = uint8_t(c.vehicle);
    if (live::has(c, SWOFTY_LIVE_HAVE_CLASS)) {
        out.position = uint8_t(c.position > 0 && c.position < 256 ? c.position : 0);
        out.status = c.result_status;
    }
    if (live::has(c, SWOFTY_LIVE_HAVE_PROGRESS))
        out.lap = c.lap;
    if (live::has(c, SWOFTY_LIVE_HAVE_PIT)) {
        out.pit_state = c.pit_state;
        out.in_pit = live::in_pit(c);
    }
    if (auto g = live::gap_ahead_ms(c)) {
        out.gap_ahead_ms = *g;
        out.gap_ahead_valid = 1;
    }
    if (auto g = live::gap_leader_ms(c)) {
        out.gap_leader_ms = *g;
        out.gap_leader_valid = 1;
    }
    if (live::has(c, SWOFTY_LIVE_HAVE_TYRE))
        out.tyre = c.tyre_letter;
    if (live::has(c, SWOFTY_LIVE_HAVE_PENALTIES)) {
        out.time_penalty_s = c.time_penalty_s;
        out.drive_through = c.drive_through;
        out.stop_go = c.stop_go;
    }
    std::snprintf(out.team_key, sizeof out.team_key, "%.*s", int(team_key.size()), team_key.data());
}

// Reads the live session and classification and updates the tower; false (and the tower cleared to off) while the
// live state is unavailable. The subject is the watched car when spectating, else the player's.
inline bool update_from_live(Tower &tower, double now, const Config &cfg)
{
    auto s = live::session();
    if (!s) {
        tower.update(Session{}, {}, -1, now, cfg);
        return false;
    }
    Session session;
    from_live(*s, session);
    std::array<Car, live::max_cars> cars{};
    size_t n = 0;
    for (uint32_t id : live::order().ids) {
        auto c = live::car(id);
        if (!c)
            continue;
        auto badge = live::team_badge(id);
        from_live(*c, cars[n++], badge ? std::string_view(*badge) : std::string_view());
    }
    tower.update(session, std::span<const Car>(cars.data(), n), s->subject, now, cfg);
    return true;
}

}
