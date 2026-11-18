// Host-native unit test for the core's fitted tyre model check (native/src/core/tyre_models_logic.hpp): which
// models count as never placed, and the log lines built from them. Built and run by native/test/run_tyre_models_test.sh.

#include "tyre_models_logic.hpp"

#include <cstdio>
#include <cstring>
#include <initializer_list>
#include <string>
#include <vector>

static int failures;

#define CHECK(cond, msg)                                                                                                \
    do {                                                                                                                \
        if (cond) {                                                                                                     \
            printf("ok  : %s\n", msg);                                                                                  \
        } else {                                                                                                        \
            printf("FAIL: %s\n", msg);                                                                                  \
            failures++;                                                                                                 \
        }                                                                                                               \
    } while (0)

namespace tmod = runtime::tyre_models;

static tmod::Model placed()
{
    return {true, -2.9f, 0.354f, 0.248f, 2};
}

static tmod::Model unplaced()
{
    return {true, -1000.0f, 60.0f, 30.0f, 0};
}

static tmod::Verdict judge(std::initializer_list<tmod::Model> ms, unsigned *spawned = nullptr, unsigned *bad = nullptr)
{
    tmod::Model a[3] = {};
    size_t n = 0;
    for (auto &m : ms)
        a[n++] = m;
    unsigned s, b;
    auto v = tmod::judge(a, n, &s, &b);
    if (spawned)
        *spawned = s;
    if (bad)
        *bad = b;
    return v;
}

int main()
{
    CHECK(tmod::is_unplaced(unplaced()), "a spawned model at the spawn defaults is unplaced");
    CHECK(!tmod::is_unplaced(placed()), "a placed model is not unplaced");
    tmod::Model never = unplaced();
    never.spawned = false;
    CHECK(!tmod::is_unplaced(never), "a model that was never spawned is not counted");
    tmod::Model moved = unplaced();
    moved.translation = -2.9f;
    CHECK(!tmod::is_unplaced(moved), "a model with a real translation is placed even with default dimensions");
    tmod::Model sized = unplaced();
    sized.dimension_a = 0.354f;
    CHECK(!tmod::is_unplaced(sized), "a model with real dimensions is placed even at the spawn translation");
    tmod::Model marked = unplaced();
    marked.state = tmod::PLACED_STATE;
    CHECK(!tmod::is_unplaced(marked), "a model whose render instance state is 2 is placed");

    unsigned s = 0, b = 0;
    CHECK(judge({unplaced(), unplaced(), unplaced()}, &s, &b) == tmod::Verdict::unplaced && s == 3 && b == 3,
          "three unplaced fitted models make the car unplaced");
    CHECK(judge({placed(), placed(), placed()}) == tmod::Verdict::placed, "three placed models are fine");
    CHECK(judge({placed(), unplaced(), placed()}, &s, &b) == tmod::Verdict::partial && s == 3 && b == 1,
          "a spare unplaced model next to placed ones is only partial");
    CHECK(judge({never, never, never}) == tmod::Verdict::no_models, "no spawned models means nothing to judge");
    CHECK(judge({placed(), never, never}) == tmod::Verdict::placed, "unspawned siblings are ignored");

    tmod::Car car;
    car.vehicle = 24;
    car.team = 7;
    car.human = false;
    car.readable = true;
    car.spawned = 3;
    car.unplaced = 3;
    car.verdict = tmod::Verdict::unplaced;
    std::string line = tmod::car_line(car, 12, "10 s in");
    CHECK(line.rfind("[tyres] car 24 (team 7, AI): fitted tyres were never placed on the wheels", 0) == 0,
          "the car line starts with the car, team and kind");
    CHECK(line.find("3 of 3") != std::string::npos && line.find("-1000") != std::string::npos &&
              line.find("size 60/30") != std::string::npos,
          "the car line names the counts and the spawn defaults");
    car.human = true;
    CHECK(tmod::car_line(car, 12, "10 s in").find("(team 7, human)") != std::string::npos, "a human car is labelled");

    tmod::Car ok = car;
    ok.verdict = tmod::Verdict::placed;
    ok.unplaced = 0;
    std::vector<tmod::Car> cars{ok, car};
    cars[0].vehicle = 3;
    std::string sum = tmod::summary_line(cars, 12, "10 s in");
    CHECK(sum == "[tyres] fitted tyre check (10 s in), session 12: 1 of 2 car(s) have unplaced tyres (cars 24)",
          "the summary counts the unplaced cars and lists their numbers");
    cars.pop_back();
    CHECK(tmod::summary_line(cars, 12, "60 s in").find("all 1 car(s) have their fitted tyres placed") != std::string::npos,
          "a healthy grid is summarised as placed");
    tmod::Car blind;
    blind.readable = false;
    cars.push_back(blind);
    CHECK(tmod::summary_line(cars, 12, "60 s in").find("all 1 car(s)") != std::string::npos,
          "cars that could not be read are left out of the count");

    return failures ? 1 : 0;
}
