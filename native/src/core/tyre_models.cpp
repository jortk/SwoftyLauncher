#include "runtime.hpp"

#include <windows.h>

#include "tyre_models_logic.hpp"

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

// Fitted tyre model check (read-only diagnostic; every offset is L1, read live on F1 1990 2026-11-03).
//
// Sess_TyreAllocation_OnSessionStart's component is kept (+0x270 session provider, +0x278 career with the entrants,
// +0x2d8 car list). TYRE_FIRST_MS after the session starts, and again at TYRE_SECOND_MS, each car's tyre component is
// read: the slot table (+0x9bd0, count +0x9c48, class +0x9bc0) names the entry of the fitted option, and the three
// models of that entry (component +0x30 + entry * 0xde0 + 0 / 0x4a0 / 0x940) are checked for the spawn defaults.
// A car whose spawned fitted models all still have them was never placed (Game_CompoundModelSet_PlaceFittedTyre,
// driven per vehicle by Game_TyreSystem_UpdateVehicle) and drives without visible tyres; known_problems explains it.
// Nothing is written and no placement is attempted. SWOFTY_TYRE_MODELS_CHECK=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace tmod = runtime::tyre_models;

constexpr const char *MIXIN = "swofty_core.dll (tyre model check)";
constexpr const char *SITE = "Sess_TyreAllocation_OnSessionStart";
constexpr int64_t TYRE_FIRST_MS = 10000, TYRE_SECOND_MS = 60000;
constexpr size_t COMP_PROVIDER = 0x270, COMP_CAREER = 0x278, COMP_CAR_LIST = 0x2d8;
constexpr size_t PROVIDER_SESSION_SLOT = 0xa8 / 8, SESSION_ID = 0x08;
constexpr size_t CAREER_ENTRANTS = 0x2e0, ENTRANT_SIZE = 0x88, ENTRANT_MAX = 256;
constexpr size_t ENTRANT_FLAGS = 0x4b, ENTRANT_TEAM = 0x5a, ENTRANT_VEHICLE = 0x82;
constexpr size_t CARS_BEGIN = 0x28, CARS_END = 0x30, CAR_ENTITY = 0x10;
constexpr size_t TC_PROBE = 0x9c68, TC_CLASS = 0x9bc0, TC_SLOTS = 0x9bd0, TC_COUNT = 0x9c48, TC_FITTED = 0x9c98;
constexpr size_t TC_MODEL_COUNT = 0x98d0, TC_MODELS = 0x30, MODEL_ENTRY_STRIDE = 0xde0;
constexpr size_t SLOT_STRIDE = 0x18, SLOT_ROW = 0x10, CLASS_VISUAL_MAP = 0x1d8;
constexpr ptrdiff_t FITTED_OPTION = -0x6c;
constexpr size_t MODEL_SUB[] = {0, 0x4a0, 0x940};
constexpr size_t MODEL_RENDER = 0x78, RENDER_INSTANCE = 0x80, INSTANCE_TRANSLATION = 0x2c, INSTANCE_STATE = 0x128;
constexpr size_t MODEL_DIMENSION_A = 0x278, MODEL_DIMENSION_B = 0x27c;
constexpr uint64_t MAX_ENTRIES = 11;

void (*start_orig)(void *) = nullptr;
bool offered = false, wanted = false, settled = false;
std::mutex lock;
uintptr_t component = 0;
uint32_t session = 0;
int64_t started_ms = 0;
std::atomic<int> stage{2};

int64_t now_ms()
{
    return std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now().time_since_epoch())
        .count();
}

bool read_ptr(uintptr_t at, uintptr_t &out)
{
    return mem::read(at, out) && out;
}

uintptr_t tyre_component_of(uintptr_t car_list, uint8_t v)
{
    uintptr_t begin = 0, end = 0, slot = 0, obj = 0;
    if (!car_list || !mem::read(car_list + CARS_BEGIN, begin) || !mem::read(car_list + CARS_END, end) || !begin ||
        end <= begin || (end - begin) / sizeof(uintptr_t) <= v || !read_ptr(begin + v * sizeof(uintptr_t), slot) ||
        !read_ptr(slot + CAR_ENTITY, obj))
        return 0;
    auto type = reinterpret_cast<const uint8_t *>(swofty::api->resolve("k_Sess_TyreComponentType"));
    auto base = reinterpret_cast<const uint8_t *>(swofty::api->resolve("k_Ne_TypeRoot"));
    auto chain_end = reinterpret_cast<const uint8_t *>(swofty::api->resolve("k_Ne_TypeChainEnd"));
    auto find = reinterpret_cast<void *(*)(void *, const void *)>(swofty::api->resolve("Ne_Entity_FindComponent"));
    if (!type || !base || !chain_end || !find)
        return 0;
    bool is_component = type == base || type == chain_end;
    for (const uint8_t *n = type + 8; !is_component && n && mem::readable(n + 0x10, 8);) {
        if (!mem::read(n + 0x10, n))
            break;
        is_component = n == base + 8;
        if (n == chain_end + 8)
            break;
    }
    if (is_component)
        obj = uintptr_t(find(reinterpret_cast<void *>(obj), type));
    uint64_t probe = 0;
    return obj && mem::read(obj + TC_PROBE, probe) ? obj : 0;
}

uint32_t fitted_entry(uintptr_t tc)
{
    uint64_t count = 0;
    uintptr_t fitted = 0, cls = 0;
    uint32_t option = 0;
    if (!mem::read(tc + TC_COUNT, count) || !read_ptr(tc + TC_FITTED, fitted) || !read_ptr(tc + TC_CLASS, cls) ||
        !mem::read(fitted + FITTED_OPTION, option))
        return 0xff;
    for (uint64_t i = 0; i < count && i < 8; i++) {
        uintptr_t e = tc + TC_SLOTS + i * SLOT_STRIDE, row = 0, map = 0;
        uint32_t o = 0, visual = 0;
        uint8_t slot = 0xff;
        if (!mem::read(e, o) || o != option)
            continue;
        if (!read_ptr(e + SLOT_ROW, row) || !mem::read(row, visual) || !read_ptr(cls + CLASS_VISUAL_MAP, map) ||
            !mem::read(map + (visual & 0xff), slot))
            return 0xff;
        return slot;
    }
    return 0xff;
}

tmod::Model read_model(uintptr_t m)
{
    tmod::Model out;
    uintptr_t render = 0, instance = 0;
    if (!read_ptr(m + MODEL_RENDER, render) || !read_ptr(render + RENDER_INSTANCE, instance))
        return out;
    out.spawned = mem::read(instance + INSTANCE_TRANSLATION, out.translation) &&
                  mem::read(instance + INSTANCE_STATE, out.state) && mem::read(m + MODEL_DIMENSION_A, out.dimension_a) &&
                  mem::read(m + MODEL_DIMENSION_B, out.dimension_b);
    return out;
}

tmod::Car check_car(uintptr_t car_list, uintptr_t row)
{
    tmod::Car car;
    uint8_t flags = 0;
    mem::read(row + ENTRANT_VEHICLE, car.vehicle);
    mem::read(row + ENTRANT_FLAGS, flags);
    mem::read(row + ENTRANT_TEAM, car.team);
    car.human = flags & 0x10;
    uintptr_t tc = tyre_component_of(car_list, car.vehicle);
    uint64_t entries = 0;
    if (!tc || !mem::read(tc + TC_MODEL_COUNT, entries))
        return car;
    uint32_t entry = fitted_entry(tc);
    if (entry >= entries || entry >= MAX_ENTRIES)
        return car;
    tmod::Model models[3];
    for (size_t i = 0; i < 3; i++)
        models[i] = read_model(tc + TC_MODELS + entry * MODEL_ENTRY_STRIDE + MODEL_SUB[i]);
    car.readable = true;
    car.verdict = tmod::judge(models, 3, &car.spawned, &car.unplaced);
    return car;
}

void check(const char *when)
{
    uintptr_t career = 0, car_list = 0, begin = 0, end = 0;
    if (!read_ptr(component + COMP_CAREER, career) || !read_ptr(component + COMP_CAR_LIST, car_list) ||
        !mem::read(career + CAREER_ENTRANTS, begin) || !mem::read(career + CAREER_ENTRANTS + 8, end) || !begin ||
        end <= begin || (end - begin) % ENTRANT_SIZE || (end - begin) / ENTRANT_SIZE > ENTRANT_MAX ||
        !mem::readable(begin, end - begin)) {
        swofty::log("[tyres] fitted tyre check (%s), session %u: the entrants could not be read", when, session);
        return;
    }
    std::vector<tmod::Car> cars;
    for (uintptr_t row = begin; row < end && cars.size() < tmod::MAX_CARS; row += ENTRANT_SIZE) {
        uint8_t v = 0xff;
        if (!mem::read(row + ENTRANT_VEHICLE, v) || v == 0xff)
            continue;
        cars.push_back(check_car(car_list, row));
        if (cars.back().verdict == tmod::Verdict::unplaced)
            swofty::log("%s", tmod::car_line(cars.back(), session, when).c_str());
    }
    swofty::log("%s", tmod::summary_line(cars, session, when).c_str());
}

void on_start(void *c)
{
    start_orig(c);
    if (!c)
        return;
    uintptr_t provider = 0, vt = 0, getter = 0, s = 0;
    uint32_t id = 0;
    if (read_ptr(uintptr_t(c) + COMP_PROVIDER, provider) && read_ptr(provider, vt) &&
        read_ptr(vt + PROVIDER_SESSION_SLOT * sizeof(uintptr_t), getter)) {
        s = uintptr_t(reinterpret_cast<void *(*)(void *)>(getter)(reinterpret_cast<void *>(provider)));
        if (s)
            mem::read(s + SESSION_ID, id);
    }
    std::lock_guard<std::mutex> g(lock);
    component = uintptr_t(c);
    session = id;
    started_ms = now_ms();
    stage = 0;
}

}

namespace runtime {

std::vector<CoreMixins> tyre_models_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_TYRE_MODELS_CHECK", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[tyres] model check off (SWOFTY_TYRE_MODELS_CHECK=%s)", v);
        return out;
    }
    if (!swofty::api->resolve(SITE)) {
        swofty::log("[tyres] model check off: needs %s, which this build does not resolve", SITE);
        return out;
    }
    wanted = true;
    out.push_back({MIXIN, {{SITE, SWOFTY_MIXIN_WRAP, 0, (void *)&on_start, (void **)&start_orig}}});
    return out;
}

void tyre_models_commit()
{
    if (!wanted || settled)
        return;
    settled = true;
    swofty::log("[tyres] model check %s",
                core_mixins_active(MIXIN) ? "armed: fitted tyre models are read 10 s and 60 s into each session"
                                          : "did not install (see the [core] lines)");
}

void tyre_models_tick()
{
    if (stage >= 2)
        return;
    std::lock_guard<std::mutex> g(lock);
    if (stage >= 2 || !component)
        return;
    int64_t due = stage == 0 ? TYRE_FIRST_MS : TYRE_SECOND_MS;
    if (now_ms() - started_ms < due)
        return;
    const char *when = stage == 0 ? "10 s in" : "60 s in";
    stage++;
    check(when);
}

void tyre_models_drop()
{
    std::lock_guard<std::mutex> g(lock);
    component = 0;
    stage = 2;
}

}
