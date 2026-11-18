#include "race_retirement_logic.hpp"

#include <cstdio>
#include <cstring>
#include <initializer_list>

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

static uint64_t at(const uint8_t *p, size_t off)
{
    uint64_t v;
    std::memcpy(&v, p + off, 8);
    return v;
}

int main()
{
    using namespace runtime::race_retirement;

    CHECK(skips(0), "a car with no driver skips the name fill");
    CHECK(!skips(0x3462d4f48 + 8), "a car with a driver runs the game's name fill");

    alignas(8) uint8_t name[NAME_SIZE];
    std::memset(name, 0xcc, sizeof name);
    empty_name(name, 0x145613670);
    CHECK(at(name, 0) == 0x145613670, "the empty name carries the inline string vtable");
    CHECK(at(name, 8) == 0x80, "the empty name has the 0x80 capacity the name fill writes");
    CHECK(at(name, 0x10) == reinterpret_cast<uintptr_t>(name + 0x18), "the empty name points at its own characters");
    CHECK(name[0x18] == 0, "the empty name's characters are an empty string");
    CHECK(name[0x19] == 0xcc && name[NAME_SIZE - 1] == 0xcc, "the empty name writes nothing past its NUL");

    CHECK(entrant(NO_CAR, 0x1000, 0x1000 + 22 * 0x88, false) == Entrant::no_car, "vehicle index 0xff has no car");
    CHECK(entrant(NO_CAR, 0x1000, 0x1000 + 22 * 0x88, true) == Entrant::no_car, "the car test comes first");
    CHECK(entrant(18, 0, 0, false) == Entrant::no_list, "no game module list has no entrant");
    CHECK(entrant(18, 0x1000, 0x1000, false) == Entrant::no_list, "an empty list has no entrant");
    CHECK(entrant(18, 0x2000, 0x1000, false) == Entrant::no_list, "a list whose end is before its begin is empty");
    CHECK(entrant(18, 0x1000, 0x1000 + 22 * 0x88, false) == Entrant::not_found, "no entrant with the car's index");
    CHECK(entrant(18, 0x1000, 0x1000 + 22 * 0x88, true) == Entrant::found, "the car's entrant is found");

    CHECK(key(18, Entrant::found, 0, 312) != key(18, Entrant::found, 0, 313), "two refs of a car log apart");
    CHECK(key(18, Entrant::found, 0, 312) != key(17, Entrant::found, 0, 312), "two cars with one ref log apart");
    CHECK(key(18, Entrant::found, 1, 312) != key(18, Entrant::found, 2, 312), "two domains log apart");
    CHECK(key(18, Entrant::not_found, 0, 0) != key(18, Entrant::found, 0, 0), "a missing entrant is not ref 0");
    CHECK(key(18, Entrant::not_found, 3, 9) == key(18, Entrant::not_found, 0, 0), "a missing entrant has no ref");
    CHECK(key(0xff, Entrant::found, 0xff, 0xffff) != key(0xff, Entrant::no_car, 0, 0), "the largest ref is not no car");

    static Said said;
    CHECK(said.first(key(18, Entrant::found, 0, 312)), "a car's missing driver is logged");
    CHECK(!said.first(key(18, Entrant::found, 0, 312)), "the same car and ref is logged once");
    CHECK(said.first(key(4, Entrant::found, 0, 312)), "another car is logged");
    static Said full;
    for (size_t i = 0; i < MAX_SAID; i++)
        full.first(i);
    CHECK(!full.first(MAX_SAID + 7), "the log stops after its cap");

    CHECK(miss(0, 0x1000, 25, 25) == Miss::past_end, "the shared miss rules apply");
    CHECK(driver_count(0x1000, 0x1000 + 25 * 0x10) == 25, "the shared driver count applies");
    bool texts = true;
    for (Entrant e : {Entrant::no_car, Entrant::no_list, Entrant::not_found, Entrant::found})
        texts = texts && entrant_text(e) && *entrant_text(e);
    CHECK(texts, "every entrant case has a log text");
    return failures ? 1 : 0;
}
