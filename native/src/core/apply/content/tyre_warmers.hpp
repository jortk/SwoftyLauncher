#pragma once

#include "lower.hpp"

#include <optional>
#include <string>
#include <utility>
#include <vector>

// rules.tyre_warmers (content.package 1.4): whether a box's races run with tyre blankets. The game reads it from the
// current game setup mode's has_tyre_warmers byte (GameSetupMode +0x1c8, set by the mode loader 0x142a14f20 from
// game_setup_modes.has_tyre_warmers). Every addon box shares the swofty_f1 host mode, so the era picker writes the
// pressed box's value into that byte, like its DRS group, and puts the game's value back when another box or a stock
// box is pressed (tyre_warmers_pick). Pure, for the host tests.

namespace swofty_core::apply::content {

struct WarmersPick {
    std::string package;
    std::optional<bool> warmers;
    std::string line;
};

// The tyre warmer rule for the box whose entrant list was pressed: the last content.package (in the order given) with
// that entrant list, and its rules.tyre_warmers when that is a bool.
inline WarmersPick tyre_warmers_pick(const std::string &entrant_list,
                                     const std::vector<std::pair<std::string, const Value *>> &packages)
{
    WarmersPick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; swofty_f1 keeps its own tyre warmer setting";
        return p;
    }
    const Value &w = (*found)["rules"]["tyre_warmers"];
    if (!w.is_bool()) {
        p.line = "box " + entrant_list + " (" + p.package + "): no tyre warmer rule; swofty_f1 keeps its own setting";
        return p;
    }
    p.warmers = w.as_bool();
    p.line = "box " + entrant_list + " (" + p.package + "): swofty_f1 races " +
             (*p.warmers ? "with" : "without") + " tyre warmers (rules.tyre_warmers)";
    return p;
}

}
