#pragma once
#include <cstdint>
#include <optional>
#include <vector>

#include <swofty_api.h>

#include "context.hpp"

// The player and participants of the Facts, read from the live car table. A car whose is_ai flag is clear and
// whose controller is the human's is the player's: the flag alone is clear on every car while the game builds the
// table, so a car the controller set does not call human (or cannot yet answer for) is not taken. The table is
// scanned when the car count or the facts epoch changes or the remembered slot stops being the player's, and on
// every frame while a controller answer is still missing; every other frame costs the count read and that one slot.

namespace swofty_core::hooks::player_facts {

using Control = int (*)(uint32_t car, uint32_t *mode);

constexpr uint32_t CONTROL_HUMAN = 0;

struct Cache {
    bool scanned = false;
    uint32_t count = 0;
    uint64_t epoch = 0;
    int player = -1;
    std::vector<Entrant> cars;
};

inline int human_of(Control control, uint32_t car)
{
    if (!control)
        return 1;
    uint32_t mode = 0;
    int r = control(car, &mode);
    if (r == SWOFTY_ASSET_OK)
        return mode == CONTROL_HUMAN ? 1 : 0;
    return r == SWOFTY_ASSET_LATE ? -1 : 1;
}

template <class Ops> bool stale(const Cache &c, uint32_t n, uint64_t epoch, Control control)
{
    if (!c.scanned || c.count != n || c.epoch != epoch)
        return true;
    if (c.player < 0)
        return false;
    uint32_t ai = 1;
    return Ops::op_car(uint32_t(c.player), nullptr, &ai) != SWOFTY_ASSET_OK || ai != 0 ||
           human_of(control, uint32_t(c.player)) != 1;
}

template <class Ops> void scan(Cache &c, uint32_t n, uint64_t epoch, Control control)
{
    c.scanned = true;
    c.count = n;
    c.epoch = epoch;
    c.player = -1;
    c.cars.clear();
    for (uint32_t i = 0; i < n; i++) {
        uint32_t ai = 1;
        if (Ops::op_car(i, nullptr, &ai) != SWOFTY_ASSET_OK)
            continue;
        Entrant e;
        e.car = i;
        e.human = ai == 0;
        if (e.human) {
            int h = human_of(control, i);
            if (h < 0)
                c.scanned = false;
            e.human = h == 1;
        }
        if (e.human && c.player < 0)
            c.player = int(i);
        c.cars.push_back(std::move(e));
    }
}

struct Change {
    std::optional<Entrant> player;
    std::vector<Entrant> participants;
};

template <class Ops> std::optional<Change> sync(Cache &c, const Facts &facts, Control control = nullptr)
{
    uint32_t n = 0;
    bool live = facts.session && Ops::op_count(&n) == SWOFTY_ASSET_OK && n > 0;
    if (!live) {
        c = Cache{};
        if (facts.player || !facts.participants.empty())
            return Change{};
        return std::nullopt;
    }
    if (stale<Ops>(c, n, facts.epoch, control))
        scan<Ops>(c, n, facts.epoch, control);
    bool same = facts.participants.size() == c.cars.size() && bool(facts.player) == (c.player >= 0) &&
                (!facts.player || facts.player->car == uint32_t(c.player));
    for (size_t i = 0; same && i < c.cars.size(); i++)
        same = facts.participants[i].car == c.cars[i].car && facts.participants[i].human == c.cars[i].human;
    if (same)
        return std::nullopt;
    Change out;
    out.participants = c.cars;
    if (c.player >= 0)
        for (auto &e : c.cars)
            if (e.car == uint32_t(c.player))
                out.player = e;
    return out;
}

}
