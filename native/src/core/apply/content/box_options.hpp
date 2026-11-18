#pragma once

#include <swofty/json.hpp>

#include <array>
#include <cctype>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

// Box rules that are game options (content.package 1.5): rules.safety_car, rules.formation_lap, rules.penalties and
// rules.corner_cutting. The game keeps each as a session option in the options store (g_Game_OptionsStore, found by
// name with Game_OptionsStore_FindOption, index read and set through the option's vtable +0x10 / +0x18, the way
// car_category.cpp sets the category option). Every addon box shares the swofty_f1 host mode, so the era picker sets
// the options when the box is pressed and puts the player's index back on the next press when an option still holds
// the box's index (OptionForce). Pure, for the host tests.
//
// Option and value names come from the stock database (options_system.options.game, difficulty_options folder of
// gamedatabases.2025.bin): safety_car_frequency [off, reduced, standard, increased], formation_lap [off, on],
// rules_and_penalties_mode [off, corner_cutting_only, eSports, on], corner_cutting_mode [regular, strict]. That the
// store holds them under these names in a Grand Prix weekend is not seen at runtime yet: the core tries the names in
// order (older guesses after the database name) and matches a value whose name is, or ends in, one of the setting's
// candidate names, case-insensitively. A miss sets nothing and logs the option's value names.

namespace swofty_core::apply::content {

using Value = swofty::json::Value;

// One box rule that maps onto one game option.
struct BoxOptionRule {
    const char *rule;                       // the key under content.package rules
    std::array<const char *, 4> options;    // game option names, tried in order (nullptr ends the list)
    bool boolean;                           // a bool rule: true -> "on", false -> "off"
    std::array<const char *, 4> settings;   // the string settings the schema allows (string rules)
};

inline constexpr std::array<BoxOptionRule, 4> BOX_OPTION_RULES = {{
    {"safety_car", {"safety_car_frequency", "safety_car", "safety_car_intensity", "safety_car_setting"}, false,
     {"off", "reduced", "standard", "increased"}},
    {"formation_lap", {"formation_lap", nullptr, nullptr, nullptr}, true, {nullptr, nullptr, nullptr, nullptr}},
    {"penalties", {"rules_and_penalties_mode", "rules_and_penalties", "rules_and_penalites", nullptr}, false,
     {"off", "corner_cutting_only", "esports", "on"}},
    {"corner_cutting", {"corner_cutting_mode", "corner_cutting", nullptr, nullptr}, false,
     {"regular", "strict", nullptr, nullptr}},
}};
inline constexpr size_t BOX_OPTION_COUNT = BOX_OPTION_RULES.size();

// Index of a rule in BOX_OPTION_RULES, or -1.
inline int box_option_index(std::string_view rule)
{
    for (size_t i = 0; i < BOX_OPTION_COUNT; ++i)
        if (rule == BOX_OPTION_RULES[i].rule)
            return int(i);
    return -1;
}

inline bool box_option_setting(const BoxOptionRule &r, std::string_view s)
{
    if (r.boolean)
        return s == "on" || s == "off";
    for (const char *ok : r.settings)
        if (ok && s == ok)
            return true;
    return false;
}

// The setting of one rule in a rules object: "on"/"off" for a bool rule, the string for a string rule; none when the
// value is missing, null or not one the schema allows.
inline std::optional<std::string> box_option_from(const BoxOptionRule &r, const Value &rules)
{
    const Value &v = rules[r.rule];
    if (r.boolean) {
        if (v.is_bool())
            return std::string(v.as_bool() ? "on" : "off");
        return std::nullopt;
    }
    if (v.is_string() && box_option_setting(r, v.as_string()))
        return std::string(v.as_string());
    return std::nullopt;
}

struct BoxOptionsPick {
    std::string package;
    std::array<std::optional<std::string>, BOX_OPTION_COUNT> settings;   // per BOX_OPTION_RULES entry
    std::string line;
};

// The option rules for the box whose entrant list was pressed: the last content.package (in the order given) with
// that entrant list, and each of its rules that holds an allowed setting.
inline BoxOptionsPick box_options_pick(const std::string &entrant_list,
                                       const std::vector<std::pair<std::string, const Value *>> &packages)
{
    BoxOptionsPick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; the options keep the player's settings";
        return p;
    }
    std::string set;
    for (size_t i = 0; i < BOX_OPTION_COUNT; ++i) {
        p.settings[i] = box_option_from(BOX_OPTION_RULES[i], (*found)["rules"]);
        if (p.settings[i])
            set += (set.empty() ? "" : ", ") + std::string(BOX_OPTION_RULES[i].rule) + " " + *p.settings[i];
    }
    p.line = "box " + entrant_list + " (" + p.package + "): " +
             (set.empty() ? std::string("no option rules; the options keep the player's settings") : set);
    return p;
}

// The value names a setting may carry in the game's option, best first.
inline std::vector<std::string> box_option_candidates(std::string_view rule, std::string_view setting)
{
    if (rule == "safety_car") {
        if (setting == "off")
            return {"off", "none", "disabled"};
        if (setting == "reduced")
            return {"reduced", "low"};
        if (setting == "standard")
            return {"standard", "normal", "default", "on"};
        if (setting == "increased")
            return {"increased", "high"};
        return {};
    }
    if (rule == "formation_lap") {
        if (setting == "on")
            return {"on", "enabled", "true"};
        if (setting == "off")
            return {"off", "disabled", "false"};
        return {};
    }
    if (rule == "penalties") {
        if (setting == "off")
            return {"off", "disabled"};
        if (setting == "corner_cutting_only")
            return {"corner_cutting_only", "corner_cutting"};
        if (setting == "esports")
            return {"esports"};
        if (setting == "on")
            return {"on", "standard", "full"};
        return {};
    }
    if (rule == "corner_cutting") {
        if (setting == "regular")
            return {"regular", "standard", "normal"};
        if (setting == "strict")
            return {"strict"};
        return {};
    }
    return {};
}

inline std::string lower_text(std::string_view s)
{
    std::string out(s);
    for (auto &c : out)
        c = char(std::tolower(static_cast<unsigned char>(c)));
    return out;
}

// The index of the value that names the setting: an exact name first, then a name ending in "_<candidate>"
// (safety_car_off), candidates in order, case-insensitive. -1 when none does.
inline int box_option_value(const std::vector<std::string> &names, std::string_view rule, std::string_view setting)
{
    for (const auto &want : box_option_candidates(rule, setting)) {
        for (size_t i = 0; i < names.size(); ++i)
            if (lower_text(names[i]) == want)
                return int(i);
        for (size_t i = 0; i < names.size(); ++i) {
            std::string n = lower_text(names[i]);
            std::string tail = "_" + want;
            if (n.size() > tail.size() && n.compare(n.size() - tail.size(), tail.size(), tail) == 0)
                return int(i);
        }
    }
    return -1;
}

// One forced option index: what the player had and what the box wrote. put_back restores the player's index only
// while the option still holds the box's, so a value the player picks after the press stays.
struct OptionForce {
    const void *option = nullptr;
    int prior = -1, ours = -1;

    bool active() const { return option != nullptr; }

    // get(option) -> current index; set(option, index). Returns true when the index was written.
    template <class Get, class Set> bool force(const void *opt, int index, Get get, Set set)
    {
        if (!opt || index < 0)
            return false;
        int now = get(opt);
        if (option != opt) {
            option = opt;
            prior = now;
        } else if (now != ours) {
            prior = now;   // the player changed it since the last press: that is the value to give back
        }
        ours = index;
        if (now != index)
            set(opt, index);
        return true;
    }

    // Returns 1 when the player's index was put back, 0 when there was nothing to do or the option changed since.
    template <class Get, class Set> int put_back(const void *current, Get get, Set set)
    {
        if (!option)
            return 0;
        int n = 0;
        if (current == option && get(current) == ours && prior >= 0 && prior != ours) {
            set(current, prior);
            n = 1;
        }
        forget();
        return n;
    }

    void forget()
    {
        option = nullptr;
        prior = ours = -1;
    }
};

}
