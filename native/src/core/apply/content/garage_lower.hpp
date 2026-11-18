#pragma once

#include "liveries.hpp"
#include "lower.hpp"

// Lowers a team.garage record: the box is a content.package, and the art goes to the asset groups of the cars the
// package gives the team, so a session of any other box never loads it. A car's group is the one its vehicle package
// names (car_paths). The archive the core builds is reached through a game path of its own.

namespace swofty_core::apply::content {

inline constexpr const char *GARAGE_FOLDER = "2025_asset_groups/swofty_garage/";

inline bool garage_art_enabled(const char *env)
{
    return env && env[0] == '1' && !env[1];
}

inline constexpr const char *GARAGE_FENCE_LINE =
    "team.garage: left off; a stock streamed copy of a texture the archive holds crashed the texture stream in 1.7.8 to 1.7.10 "
    "(Ne_Crt_Memcpy in the ERP stream copy). SWOFTY_GARAGE_ART=1 turns it on";

inline std::string garage_game_path(const std::string &id)
{
    std::string s;
    for (unsigned char c : id)
        s += std::isalnum(c) ? char(std::tolower(c)) : '_';
    return std::string(GARAGE_FOLDER) + s + ".erp";
}

inline void lower_garage(const std::string &id, const Value &rec, const Lookup &lookup, const Read &read, Plan &plan)
{
    plan.owner = addon_of(id);
    auto refuse = [&](const std::string &why) {
        plan.lines.push_back("team.garage " + id + ": " + why + "; nothing applied");
        plan.refused++;
    };
    bool by_team = rec["team"].is_string() && !rec["team"].as_string().empty();
    bool by_entry = rec["entry"].is_string() && !rec["entry"].as_string().empty();
    if (!rec["package"].is_string() || !(by_team || by_entry) || !rec["textures"].is_string() ||
        rec["textures"].as_string().empty())
        return refuse("it needs a package, a team or entry, and a textures folder");
    std::string package(rec["package"].as_string()),
        team(by_team ? std::string(rec["team"].as_string()) : std::string(rec["entry"].as_string()));
    const Value *pkg = lookup("content.package", package);
    if (!pkg)
        return refuse("package " + package + " is not a record");
    GarageArt g;
    g.id = id;
    g.addon = addon_of(id);
    g.package = package;
    g.team = team;
    g.folder = std::string(rec["textures"].as_string());
    if (g.folder.back() != '/')
        g.folder += '/';
    g.root = rec["root"].is_string() ? std::string(rec["root"].as_string())
                                     : std::string("eaid://environment_package/core_assets/objects/textures/");
    g.srgb = !rec["srgb"].is_bool() || rec["srgb"].as_bool();
    g.game_path = garage_game_path(id);
    const Value &teams = (*pkg)["teams"];
    for (std::size_t t = 0; t < teams.size(); t++)
        if (teams[t]["vehicle"].is_string() &&
            (by_team ? teams[t]["team"].is_string() && teams[t]["team"].as_string() == team
                     : teams[t]["key"].is_string() && teams[t]["key"].as_string() == team))
            g.vehicles.push_back(std::string(teams[t]["vehicle"].as_string()));
    if (g.vehicles.empty())
        return refuse("package " + package + " has no team entry for " + team);
    for (auto &vid : g.vehicles) {
        CarPaths car = car_paths(record_label(lookup, "vehicle", vid), read);
        if (!car.ok()) {
            plan.lines.push_back("team.garage " + id + " on " + vid + ": no asset group: " + car.why);
            continue;
        }
        if (std::find(g.groups.begin(), g.groups.end(), car.group) == g.groups.end())
            g.groups.push_back(car.group);
    }
    if (g.groups.empty())
        return refuse("none of the cars of " + team + " in " + package + " has an asset group");
    std::string list;
    for (auto &grp : g.groups)
        list += (list.empty() ? "" : ", ") + grp;
    plan.lines.push_back("team.garage " + id + ": art of " + team + " in " + package + " for " + std::to_string(g.groups.size()) +
                         " car group(s): " + list);
    plan.garages.push_back(std::move(g));
}

}
