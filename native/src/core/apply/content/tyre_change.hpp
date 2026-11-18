#pragma once

#include "lower.hpp"

#include <optional>
#include <string>
#include <utility>
#include <vector>

// rules.mandatory_tyre_change (content.package 1.6): whether a box's races require two dry compounds. The game keeps
// the rule as three 13-byte requirement arrays in the setup mode (GameSetupMode +0x8b race, +0x98 race_2, +0xa5
// race_3, indexed by the race length) and reads the current mode's array when a race session starts
// (Sess_RaceState_ResetTimingAndTowState 0x142f29820 -> rule query 0x14358d1e0 case 0x13, the TyreRegulations
// rule). Every addon box shares the swofty_f1 host mode, so the era picker writes the pressed box's rule into those
// bytes, like its tyre warmers, and puts the game's bytes back when another box or a stock box is pressed
// (tyre_change_pick). Pure, for the host tests.

namespace swofty_core::apply::content {

struct TyreChangePick {
    std::string package;
    std::optional<bool> required;
    std::string line;
};

// The mandatory tyre change rule for the box whose entrant list was pressed: the last content.package (in the order
// given) with that entrant list, and its rules.mandatory_tyre_change when that is a bool.
inline TyreChangePick tyre_change_pick(const std::string &entrant_list,
                                       const std::vector<std::pair<std::string, const Value *>> &packages)
{
    TyreChangePick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; swofty_f1 keeps its own tyre regulations";
        return p;
    }
    const Value &m = (*found)["rules"]["mandatory_tyre_change"];
    if (!m.is_bool()) {
        p.line = "box " + entrant_list + " (" + p.package +
                 "): no mandatory tyre change rule; swofty_f1 keeps its own tyre regulations";
        return p;
    }
    p.required = m.as_bool();
    p.line = "box " + entrant_list + " (" + p.package + "): swofty_f1 races " + (*p.required ? "with" : "without") +
             " a mandatory tyre change (rules.mandatory_tyre_change)";
    return p;
}

}
