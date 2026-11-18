#pragma once

#include <swofty/systems/cars.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/liveries.hpp>
#include <swofty/systems/power.hpp>
#include <swofty/systems/sessions.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/teams.hpp>
#include <swofty/systems/tracks.hpp>
#include <swofty/systems/wearables.hpp>

#include <cstddef>
#include <string>

namespace swofty_core::registry {

template <class Declare> std::size_t declare_system_registries(Declare &&declare)
{
    std::size_t count = 0;
    auto one = [&](const auto &r) {
        std::string schema = r.schema();
        if (!schema.empty() && declare(std::string(r.name()), schema))
            count++;
    };
    using namespace swofty;
    one(teams::constructors);
    one(teams::identities);
    one(teams::logos);
    one(teams::pit_times);
    one(teams::garages);
    one(wearables::helmets);
    one(wearables::suits);
    one(wearables::gloves);
    one(wearables::season_sets);
    one(drivers::people);
    one(drivers::identities);
    one(drivers::heads);
    one(drivers::looks);
    one(drivers::stats);
    one(liveries::liveries);
    one(cars::vehicles);
    one(cars::packages);
    one(cars::handling);
    one(cars::profiles);
    one(sounds::engine_sounds);
    one(content::packages);
    one(content::era_boxes);
    one(power::drs_rules);
    one(career::modes);
    one(career::seasons);
    one(career::calendars);
    one(career::ratings);
    one(career::rnd_trees);
    one(career::markets);
    one(tracks::tracks);
    one(tracks::venues);
    one(tracks::data);
    one(tracks::per_package);
    one(sessions::points);
    return count;
}

}
