#pragma once

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

// Read-only check of the fitted tyre models (tyre_models.cpp). A tyre component keeps 5 sets of 3 models; the game
// places a spawned fitted model on its wheel (render instance +0x2c real translation, model +0x278/+0x27c the wheel
// dimensions, render instance +0x128 = 2). A model that was never placed keeps the spawn defaults below, so the car
// drives without visible tyres.

namespace runtime::tyre_models {

constexpr float SPAWN_TRANSLATION = -1000.0f;
constexpr float SPAWN_DIMENSION_A = 60.0f;
constexpr float SPAWN_DIMENSION_B = 30.0f;
constexpr uint32_t PLACED_STATE = 2;
constexpr size_t MAX_CARS = 64;

struct Model {
    bool spawned = false;
    float translation = 0.f, dimension_a = 0.f, dimension_b = 0.f;
    uint32_t state = 0;
};

enum class Verdict { no_models, placed, partial, unplaced };

struct Car {
    uint8_t vehicle = 0xff;
    uint16_t team = 0xffff;
    bool human = false;
    bool readable = false;
    unsigned spawned = 0, unplaced = 0;
    Verdict verdict = Verdict::no_models;
};

inline bool is_unplaced(const Model &m)
{
    return m.spawned && m.translation == SPAWN_TRANSLATION && m.dimension_a == SPAWN_DIMENSION_A &&
           m.dimension_b == SPAWN_DIMENSION_B && m.state != PLACED_STATE;
}

inline Verdict judge(const Model *models, size_t n, unsigned *spawned, unsigned *unplaced)
{
    *spawned = *unplaced = 0;
    for (size_t i = 0; i < n; i++) {
        *spawned += models[i].spawned;
        *unplaced += is_unplaced(models[i]);
    }
    if (!*spawned)
        return Verdict::no_models;
    if (*unplaced == *spawned)
        return Verdict::unplaced;
    return *unplaced ? Verdict::partial : Verdict::placed;
}

inline std::string car_line(const Car &c, uint32_t session, const char *when)
{
    char buf[480];
    std::snprintf(buf, sizeof buf,
                  "[tyres] car %u (team %u, %s): fitted tyres were never placed on the wheels (%u of %u fitted tyre "
                  "model(s) still at the spawn position %.0f and size %.0f/%.0f, session %u, %s); the car has no "
                  "visible tyres",
                  unsigned(c.vehicle), unsigned(c.team), c.human ? "human" : "AI", c.unplaced, c.spawned,
                  double(SPAWN_TRANSLATION), double(SPAWN_DIMENSION_A), double(SPAWN_DIMENSION_B), session, when);
    return buf;
}

inline std::string summary_line(const std::vector<Car> &cars, uint32_t session, const char *when)
{
    unsigned checked = 0, bad = 0, partial = 0;
    std::string list;
    for (const Car &c : cars) {
        if (!c.readable || c.verdict == Verdict::no_models)
            continue;
        checked++;
        partial += c.verdict == Verdict::partial;
        if (c.verdict != Verdict::unplaced)
            continue;
        list += (bad ? ", " : "") + std::to_string(unsigned(c.vehicle));
        bad++;
    }
    char buf[640];
    if (bad)
        std::snprintf(buf, sizeof buf, "[tyres] fitted tyre check (%s), session %u: %u of %u car(s) have unplaced tyres (cars %s)",
                      when, session, bad, checked, list.c_str());
    else
        std::snprintf(buf, sizeof buf,
                      "[tyres] fitted tyre check (%s), session %u: all %u car(s) have their fitted tyres placed%s", when,
                      session, checked, partial ? " (some cars have a spare model not placed)" : "");
    return buf;
}

}
