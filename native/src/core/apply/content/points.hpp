#pragma once

#include "gvdb.hpp"
#include "lower.hpp"

#include <cstdint>
#include <cstdio>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

// session.points: each record becomes one game_setup.session_standings preset row at boot, a copy of the stock
// standard_race preset with its points, fastest lap bonus and the points of its three reduced_points_data levels
// (races stopped at 75, 50 and 25 % distance) set. The game parses every preset into the game setup registry
// (g_Game_Scenarios +0xe0) before the setup modes, and a mode's race session holds a pointer to its preset, read
// live by the standings recompute (re/db structs WeekendStructure, StandingsPreset). A box names its system in
// content.package rules.points; the era picker points swofty_f1's race sessions at the parsed preset with these
// points when the box is pressed (points_pick). Pure, for the host tests.

namespace swofty_core::apply::content {

inline constexpr const char *POINTS_TABLE = "game_setup.session_standings";
inline constexpr const char *POINTS_PRESETS = "game_setup.session_standings.session_standing_preset";
inline constexpr const char *POINTS_STOCK_PRESET = "standard_race";
inline constexpr const char *POINTS_REDUCED[3] = {"75_percent", "50_percent", "25_percent"};
inline constexpr std::size_t POINTS_PLACES = 26;
inline constexpr std::size_t POINTS_KEY_MAX = 40;
inline constexpr int64_t POINTS_MAX = 9999, POINTS_BONUS_MAX = 99;

// The points as the game holds them: 26 places each for the full race and the three reduced levels.
struct PointsSystem {
    std::string label;
    std::vector<int32_t> full;
    std::vector<std::vector<int32_t>> reduced;   // 3 levels, 75 / 50 / 25 %
    int32_t bonus = 0;                           // fastest_lap_bonus; 0 with rule None
    uint8_t rule = 0;                            // fastest_lap_rule: 0 None, 1 BestOfTop10
};

inline std::string points_label(std::string_view id)
{
    std::string key;
    for (char c : key_of(id)) {
        if (key.size() == POINTS_KEY_MAX)
            break;
        bool plain = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
        key += plain ? c : '_';
    }
    char hash[9];
    std::snprintf(hash, sizeof hash, "%08x", unsigned(gvdb::fnv64(id) & 0xffffffffu));
    return "swofty_pts_" + key + "_" + hash;
}

inline std::optional<std::vector<int32_t>> points_places(const Value &v, const char *what, std::string *why)
{
    if (!v.is_array() || !v.size() || v.size() > POINTS_PLACES) {
        if (why)
            *why = std::string(what) + " is a list of 1 to " + std::to_string(POINTS_PLACES) + " whole numbers";
        return std::nullopt;
    }
    std::vector<int32_t> out;
    for (std::size_t i = 0; i < v.size(); i++) {
        if (!v[i].is_int() || v[i].as_int() < 0 || v[i].as_int() > POINTS_MAX) {
            if (why)
                *why = std::string(what) + " place " + std::to_string(i + 1) + " is not a whole number from 0 to " +
                       std::to_string(POINTS_MAX);
            return std::nullopt;
        }
        out.push_back(int32_t(v[i].as_int()));
    }
    out.resize(POINTS_PLACES, 0);
    return out;
}

inline std::optional<PointsSystem> points_system(const std::string &id, const Value &rec, std::string *why)
{
    PointsSystem s;
    s.label = points_label(id);
    auto full = points_places(rec["points"], "points", why);
    if (!full)
        return std::nullopt;
    s.full = *full;
    const Value &fl = rec["fastest_lap"];
    if (!fl.is_null()) {
        if (!fl.is_int() || fl.as_int() < 0 || fl.as_int() > POINTS_BONUS_MAX) {
            if (why)
                *why = "fastest_lap is a bonus from 0 to " + std::to_string(POINTS_BONUS_MAX) + " or null";
            return std::nullopt;
        }
        s.bonus = int32_t(fl.as_int());
        s.rule = 1;
    }
    const Value &red = rec["reduced"];
    if (!red.is_null() && (!red.is_array() || red.size() > 3)) {
        if (why)
            *why = "reduced is a list of at most 3 point lists (75, 50 and 25 % distance)";
        return std::nullopt;
    }
    for (std::size_t i = 0; i < 3; i++) {
        if (red.is_array() && i < red.size()) {
            auto lv = points_places(red[i], "reduced", why);
            if (!lv)
                return std::nullopt;
            s.reduced.push_back(*lv);
        } else {
            s.reduced.push_back(s.full);
        }
    }
    return s;
}

inline std::string points_text(const std::vector<int32_t> &p)
{
    std::string out;
    for (std::size_t i = 0; i < p.size(); i++)
        out += (i ? "," : "") + std::to_string(p[i]);
    return out;
}

inline void lower_session_points(const std::string &id, const Value &rec, Plan &plan)
{
    plan.owner = addon_of(id);
    std::string why;
    auto s = points_system(id, rec, &why);
    if (!s) {
        plan.lines.push_back("session.points " + id + ": " + why + "; nothing applied");
        plan.refused++;
        return;
    }
    std::string rule = s->rule ? "BestOfTop10" : "None";
    std::string from = std::string(POINTS_PRESETS) + "[_metadata_=" + POINTS_STOCK_PRESET + "]";
    push_row(plan, POINTS_TABLE, from,
             Value::object({{"_metadata_", s->label},
                            {"points", points_text(s->full)},
                            {"fastest_lap_bonus", std::to_string(s->bonus)},
                            {"fastest_lap_rule", rule}}));
    std::string row = std::string(POINTS_PRESETS) + "[_metadata_=" + s->label + "]";
    for (std::size_t i = 0; i < 3; i++) {
        std::string level = row + ".reduced_points_data[_metadata_=" + POINTS_REDUCED[i] + "]";
        push_op(plan, set_op(level, "points", points_text(s->reduced[i])));
        push_op(plan, set_op(level, "fastest_lap_bonus", std::to_string(s->bonus)));
        push_op(plan, set_op(level, "fastest_lap_rule", rule));
    }
    std::string shown;
    for (int32_t p : s->full)
        if (p)
            shown += (shown.empty() ? "" : "-") + std::to_string(p);
    plan.lines.push_back("session.points " + id + ": preset " + s->label + " from " + POINTS_STOCK_PRESET + ", points " +
                         shown + (s->rule ? ", fastest lap +" + std::to_string(s->bonus) : std::string(", no fastest lap point")));
}

struct PointsPick {
    std::string package, rule;
    std::optional<PointsSystem> system;
    std::string line;
};

// The points system for the box whose entrant list was pressed: the last content.package (in the order given) with
// that entrant list, its rules.points, and that system if the boot apply added it.
inline PointsPick points_pick(const std::string &entrant_list,
                              const std::vector<std::pair<std::string, const Value *>> &packages,
                              const std::map<std::string, PointsSystem> &systems)
{
    PointsPick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; swofty_f1 keeps its own points";
        return p;
    }
    const Value &pts = (*found)["rules"]["points"];
    if (!pts.is_string()) {
        p.line = "box " + entrant_list + " (" + p.package + "): no points rule; swofty_f1 keeps its own points";
        return p;
    }
    p.rule = std::string(pts.as_string());
    auto it = systems.find(p.rule);
    if (it == systems.end()) {
        p.line = "box " + entrant_list + " (" + p.package + "): rules.points " + p.rule +
                 " was not added at boot; swofty_f1 keeps its own points";
        return p;
    }
    p.system = it->second;
    std::string shown;
    for (int32_t v : it->second.full)
        if (v)
            shown += (shown.empty() ? "" : "-") + std::to_string(v);
    p.line = "box " + entrant_list + " (" + p.package + "): swofty_f1 race points " + shown +
             (it->second.rule ? ", fastest lap +" + std::to_string(it->second.bonus) : std::string()) + " (rules.points " +
             p.rule + ", preset " + it->second.label + ")";
    return p;
}

}
