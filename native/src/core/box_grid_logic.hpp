#pragma once

#include <algorithm>
#include <cmath>
#include <optional>
#include <cstdint>
#include <cstdio>
#include <numeric>
#include <string>
#include <vector>

// Grid rules per box (content.package 1.5 rules.grid), the pure part: how a weekend's qualifying classification is
// re-ranked. The game ranks the race grid from the group-1 (Qualifying) GroupResult rows of the weekend (Weekend+0x488,
// row position at +0x50, 0x1c = unclassified); box_grid.cpp collects each driver's best lap of every qualifying-type
// session (Sess_Record_GroupTimes) and rewrites those row positions after Sess_Build_GroupResult, through the game's
// own Sess_Find_GroupRowPosition. Everything here works on plain values for the host tests.

namespace runtime::box_grid {

enum class Scoring { best_lap, best_of, aggregate };

constexpr int32_t UNCLASSIFIED = 0x1c;
constexpr float LIMIT_107 = 1.07f;

inline const char *scoring_name(Scoring s)
{
    return s == Scoring::best_of ? "best_of" : s == Scoring::aggregate ? "aggregate" : "best_lap";
}

inline bool parse_scoring(const std::string &s, Scoring &out)
{
    if (s == "best_lap")
        out = Scoring::best_lap;
    else if (s == "best_of")
        out = Scoring::best_of;
    else if (s == "aggregate")
        out = Scoring::aggregate;
    else
        return false;
    return true;
}

struct Rule {
    Scoring qualifying = Scoring::best_lap;
    std::vector<int> rows;   // cars per row, repeating; empty = the track's 2-wide grid
    bool rule_107 = false;
    // Whether the rule re-ranks the qualifying classification (rows don't: they move the grid slots).
    bool reranks() const { return qualifying != Scoring::best_lap || rule_107; }
    // Whether the rule moves the grid slots into a row pattern.
    bool moves_slots() const { return !rows.empty(); }
};

inline std::string describe(const Rule &r)
{
    std::string s = std::string("qualifying ") + scoring_name(r.qualifying) + ", 107 % rule " + (r.rule_107 ? "on" : "off");
    if (!r.rows.empty()) {
        s += ", rows ";
        for (size_t i = 0; i < r.rows.size(); i++)
            s += (i ? "-" : "") + std::to_string(r.rows[i]);
    }
    return s;
}

inline bool has_time(float t)
{
    return std::isfinite(t) && t > 0.f;
}

// One driver of the weekend: the group key (EntrantRecord +0x50), the position the game gave it in the group-1
// result, and its best lap of each qualifying-type session seen (0 or negative = no time in that session).
struct Driver {
    uint32_t key = 0;
    int32_t game_position = UNCLASSIFIED;
    std::vector<float> session_best;
};

inline std::vector<float> times_of(const Driver &d)
{
    std::vector<float> t;
    for (float v : d.session_best)
        if (has_time(v))
            t.push_back(v);
    std::sort(t.begin(), t.end());
    return t;
}

// The time a driver is ranked by. best_lap and best_of: the best lap over every session seen (under best_lap the order
// stays the game's and the time only feeds the 107 % rule). aggregate: the sum of the two best session laps; complete
// is false when the driver has fewer than two.
struct Score {
    float time = 0.f;
    bool complete = false;
};

inline Score score_of(const Driver &d, Scoring s)
{
    std::vector<float> t = times_of(d);
    Score r;
    if (t.empty())
        return r;
    if (s != Scoring::aggregate) {
        r.time = t[0];
        r.complete = true;
        return r;
    }
    if (t.size() >= 2) {
        r.time = t[0] + t[1];
        r.complete = true;
    } else {
        r.time = t[0];
    }
    return r;
}

// Ranking classes, front to back: a full score inside the 107 % limit, an aggregate with one session only, a time
// outside the 107 % limit, no time at all (in the game's own order).
enum class Class { ranked = 0, partial = 1, outside_107 = 2, no_time = 3 };

struct Ranked {
    std::vector<int32_t> positions;   // new group position per driver, in input order
    std::vector<Score> scores;
    std::vector<Class> classes;
    float pole = 0.f;                 // the best ranked time (0 = none)
    float limit = 0.f;                // 107 % of pole when the rule is on (0 = off)
    bool aggregate_one_session = false;   // aggregate asked, but no driver has two sessions: ranked as best_of
    bool changed = false;
};

inline int32_t base_position(const std::vector<Driver> &drivers)
{
    int32_t base = UNCLASSIFIED;
    for (const Driver &d : drivers)
        if (d.game_position >= 0 && d.game_position < UNCLASSIFIED)
            base = std::min(base, d.game_position);
    return base == UNCLASSIFIED ? 0 : std::min(base, 1);
}

// The new group positions. best_lap keeps the game's order (only the 107 % rule moves cars: then drivers outside the
// limit and drivers without a time go to the back); best_of and aggregate
// rank by the score, ties in the game's order. Drivers outside 107 % of pole follow every driver inside it, in the same
// ranking; drivers without a time follow them in the game's order. Positions are handed out from the lowest classified
// position the game used (0- or 1-based), one per driver with a time or a game classification; a driver with neither
// stays unclassified.
inline Ranked rank(const std::vector<Driver> &drivers, const Rule &rule)
{
    Ranked out;
    size_t n = drivers.size();
    Scoring s = rule.qualifying;
    if (s == Scoring::aggregate) {
        bool any_two = false;
        for (const Driver &d : drivers)
            any_two = any_two || times_of(d).size() >= 2;
        if (!any_two) {
            s = Scoring::best_of;
            out.aggregate_one_session = true;
        }
    }
    out.scores.resize(n);
    out.classes.resize(n, Class::no_time);
    for (size_t i = 0; i < n; i++) {
        out.scores[i] = score_of(drivers[i], s);
        if (has_time(out.scores[i].time))
            out.classes[i] = out.scores[i].complete ? Class::ranked : Class::partial;
    }
    for (size_t i = 0; i < n; i++)
        if (out.classes[i] == Class::ranked && (!has_time(out.pole) || out.scores[i].time < out.pole))
            out.pole = out.scores[i].time;
    if (rule.rule_107 && has_time(out.pole)) {
        out.limit = out.pole * LIMIT_107;
        for (size_t i = 0; i < n; i++)
            if ((out.classes[i] == Class::ranked || out.classes[i] == Class::partial) && out.scores[i].time > out.limit)
                out.classes[i] = Class::outside_107;
    }
    std::vector<size_t> order(n);
    std::iota(order.begin(), order.end(), size_t{0});
    bool by_time = s != Scoring::best_lap;
    std::stable_sort(order.begin(), order.end(), [&](size_t a, size_t b) {
        if (out.classes[a] != out.classes[b])
            return out.classes[a] < out.classes[b];
        if (by_time && out.classes[a] != Class::no_time && out.scores[a].time != out.scores[b].time)
            return out.scores[a].time < out.scores[b].time;
        return drivers[a].game_position < drivers[b].game_position;
    });
    out.positions.assign(n, UNCLASSIFIED);
    if (s == Scoring::best_lap && !rule.rule_107) {
        for (size_t i = 0; i < n; i++)
            out.positions[i] = drivers[i].game_position;
        return out;
    }
    int32_t next = base_position(drivers);
    for (size_t i : order) {
        const Driver &d = drivers[i];
        if (out.classes[i] != Class::no_time || (d.game_position >= 0 && d.game_position < UNCLASSIFIED))
            out.positions[i] = next++;
        else
            out.positions[i] = d.game_position;
        out.changed = out.changed || out.positions[i] != d.game_position;
    }
    return out;
}

inline const char *class_name(Class c)
{
    switch (c) {
    case Class::ranked:
        return "ranked";
    case Class::partial:
        return "one session only";
    case Class::outside_107:
        return "outside 107 %";
    default:
        return "no time";
    }
}

inline std::string time_text(float t)
{
    if (!has_time(t))
        return "-";
    char b[32];
    std::snprintf(b, sizeof b, "%.3f", double(t));
    return b;
}

// Row pattern (rules.grid.rows): which row a grid slot falls in, its place in the row and the row's car count, for a
// pattern that repeats from the front ([3,2] -> 3-2-3-2...). box_grid.cpp moves each grid slot with it: see
// row_position below and docs/std/systems/sessions.md (grid rows).
struct RowSlot {
    int row = 0, index = 0, cars = 0;
};

inline std::vector<RowSlot> row_layout(const std::vector<int> &pattern, int cars)
{
    std::vector<RowSlot> out;
    if (pattern.empty() || cars <= 0)
        return out;
    for (int v : pattern)
        if (v <= 0)
            return out;
    int row = 0;
    size_t p = 0;
    while (int(out.size()) < cars) {
        int in_row = std::min(pattern[p], cars - int(out.size()));
        for (int i = 0; i < in_row; i++)
            out.push_back({row, i, in_row});
        row++;
        p = (p + 1) % pattern.size();
    }
    return out;
}

// Lateral offset of a slot from the row's centre line, in units of the track's 2-wide slot spacing (the lateral
// distance between the two cars of a stock row): a 2-car row keeps the stock -0.5/+0.5, wider rows are packed into
// 1.6 spacings (-0.8..+0.8), so they stay within the stock grid's width plus 0.3 spacing on each side. Index 0 is the
// pole side (the side of the stock row's first slot).
inline float row_offset(const RowSlot &s)
{
    if (s.cars <= 1)
        return 0.f;
    float span = s.cars == 2 ? 1.f : 1.6f;
    return -span / 2.f + span * float(s.index) / float(s.cars - 1);
}


struct Vec3 {
    float x = 0.f, y = 0.f, z = 0.f;
};

// The cars a row pattern lays out the slots for: every grid slot of a track (0..31), full rows, so a slot's place never
// depends on the field size.
constexpr int ROW_SLOTS = 32;

// Where grid slot `slot` stands in the row pattern, from the track's stock grid (stock[k] = slot k's position, y up; the
// stock grid is 2-wide, slots 2r and 2r+1 form stock row r). Pattern row r is put on stock row r's centre line (so it
// follows the track's own grid, bends and slope included), facing like stock row r, its cars spread across with
// row_offset. Nothing (the game's own slot) when the pattern is empty, the slot is outside 0..31, the stock grid lacks
// row r, or the stock rows are degenerate (two slots on the same spot, rows on the same spot).
inline std::optional<Vec3> row_position(const std::vector<int> &pattern, const std::vector<Vec3> &stock, int slot)
{
    auto rows = row_layout(pattern, ROW_SLOTS);
    if (slot < 0 || slot >= int(rows.size()))
        return std::nullopt;
    const RowSlot &rs = rows[size_t(slot)];
    int stock_rows = int(stock.size()) / 2, r = rs.row;
    if (r >= stock_rows || stock_rows < 2)
        return std::nullopt;
    auto centre = [&](int k) {
        const Vec3 &a = stock[size_t(2 * k)], &b = stock[size_t(2 * k + 1)];
        return Vec3{(a.x + b.x) / 2.f, (a.y + b.y) / 2.f, (a.z + b.z) / 2.f};
    };
    Vec3 c = centre(r);
    Vec3 f0 = r + 1 < stock_rows ? c : centre(r - 1), f1 = r + 1 < stock_rows ? centre(r + 1) : c;
    float fx = f1.x - f0.x, fz = f1.z - f0.z, fl = std::sqrt(fx * fx + fz * fz);
    if (!(fl > 0.5f))
        return std::nullopt;
    fx /= fl;
    fz /= fl;
    const Vec3 &a = stock[size_t(2 * r)], &b = stock[size_t(2 * r + 1)];
    float dx = b.x - a.x, dz = b.z - a.z, along = dx * fx + dz * fz;
    float lx = dx - along * fx, lz = dz - along * fz, w = std::sqrt(lx * lx + lz * lz);
    if (!(w > 0.5f))
        return std::nullopt;
    float o = row_offset(rs) * w;
    return Vec3{c.x + lx / w * o, c.y, c.z + lz / w * o};
}

}
