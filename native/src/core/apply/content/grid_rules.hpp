#pragma once

#include "lower.hpp"
#include "../../box_grid_logic.hpp"

#include <optional>
#include <string>
#include <utility>
#include <vector>

// rules.grid (content.package 1.5): how a box's race grid forms from qualifying (qualifying scoring, the 107 % rule,
// a row pattern). The era picker hands the pressed box's rule to box_grid.cpp, which re-ranks the weekend's
// qualifying classification (runtime::box_grid::rank). Pure, for the host tests.

namespace swofty_core::apply::content {

struct GridPick {
    std::string package;
    std::optional<runtime::box_grid::Rule> rule;
    std::string line;
};

// The grid rule for the box whose entrant list was pressed: the last content.package (in the order given) with that
// entrant list, and its rules.grid when that is an object. Unknown values (the schema refuses them) count as missing.
inline GridPick grid_pick(const std::string &entrant_list, const std::vector<std::pair<std::string, const Value *>> &packages)
{
    namespace bg = runtime::box_grid;
    GridPick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; the game's own grid";
        return p;
    }
    const Value &g = (*found)["rules"]["grid"];
    if (!g.is_object()) {
        p.line = "box " + entrant_list + " (" + p.package + "): no grid rule; the game's own grid";
        return p;
    }
    bg::Rule r;
    const Value &q = g["qualifying"];
    if (q.is_string() && !bg::parse_scoring(std::string(q.as_string()), r.qualifying))
        r.qualifying = bg::Scoring::best_lap;
    const Value &rows = g["rows"];
    if (rows.is_array())
        for (size_t i = 0; i < rows.size(); i++) {
            int64_t v = rows[i].as_int(0);
            if (v < 1 || v > 5) {
                r.rows.clear();
                break;
            }
            r.rows.push_back(int(v));
        }
    r.rule_107 = g["rule_107"].is_bool() && g["rule_107"].as_bool();
    p.rule = r;
    p.line = "box " + entrant_list + " (" + p.package + "): grid rule " + bg::describe(r) +
             (r.reranks() ? "" : " (the game's own order)") +
             (r.rows.empty() ? "" : "; rows move the race grid's slots");
    return p;
}

}
