#pragma once

#include "lower.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

// rules.race_time_limit_s and rules.weekend_mode (content.package 1.5): a box's race time limit and the weekend it runs
// (practice, qualifying and race groups, and the length options they offer). Every addon box shares the swofty_f1 host
// mode, so the era picker writes the pressed box's time limit into swofty_f1's race_time_limit_in_seconds
// (GameSetupMode +0xb4, three u32) and swaps the weekend options of the named helper mode (GameSetupMode +0x190..+0x1a8)
// into swofty_f1, like its DRS group, and puts the game's values back when another box or a stock box is pressed
// (weekend_pick). Pure, for the host tests.

namespace swofty_core::apply::content {

struct WeekendPick {
    std::string package;
    std::optional<uint32_t> time_limit_s;
    std::string weekend_mode;
    std::string line;
};

// weekend_mode_name_ok: lower.hpp (the boot lowering marks the helper row available).

// The weekend rules for the box whose entrant list was pressed: the last content.package (in the order given) with that
// entrant list, its rules.race_time_limit_s when that is a whole number of seconds in 60..86400 and its
// rules.weekend_mode when that names a setup mode other than the host.
inline WeekendPick weekend_pick(const std::string &entrant_list,
                                const std::vector<std::pair<std::string, const Value *>> &packages,
                                const char *host = "swofty_f1")
{
    WeekendPick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; " + host + " keeps its own race time limit and weekend";
        return p;
    }
    const Value &rules = (*found)["rules"];
    const Value &t = rules["race_time_limit_s"];
    if (t.is_int() && t.as_int() >= 60 && t.as_int() <= 86400)
        p.time_limit_s = uint32_t(t.as_int());
    const Value &w = rules["weekend_mode"];
    if (w.is_string() && weekend_mode_name_ok(w.as_string()) && w.as_string() != host)
        p.weekend_mode = std::string(w.as_string());
    std::string what;
    if (p.time_limit_s)
        what = "race time limit " + std::to_string(*p.time_limit_s) + " s (rules.race_time_limit_s)";
    if (!p.weekend_mode.empty())
        what += (what.empty() ? "" : ", ") + std::string("the weekend of setup mode ") + p.weekend_mode + " (rules.weekend_mode)";
    p.line = "box " + entrant_list + " (" + p.package + "): " + host + " runs " +
             (what.empty() ? std::string("its own race time limit and weekend") : what);
    return p;
}

}
