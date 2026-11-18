#include "live_car.hpp"

#include "axes_logic.hpp"
#include "live_car_logic.hpp"
#include "postfx_logic.hpp"
#include "runtime.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cmath>
#include <cstdlib>

namespace live_car {
namespace {

namespace mem = swofty::mem;

constexpr size_t CAR_IS_AI = 0x124, CAR_ENGINE = 0x530, CAR_GEARBOX = 0x538, CAR_CHASSIS = 0x558, CAR_STEERING = 0x560,
                 CAR_SUSPENSION = 0x580, CAR_THROTTLE = 0x188, CAR_BRAKE = 0x190, CAR_FLAGS = 0x224;
constexpr size_t GEARBOX_INDEX = 0x680, ENGINE_RATE = 0x2538, ENGINE_LIMIT = 0x2528;
// The body axes (right, up, forward) are the three rows at chassis +0x190, +0x1a0, +0x1b0: the game's own trace records
// its Orientation channel from them (0x1437b3560) and its Speed channel from the forward row (0x1437c48c0).
constexpr size_t CHASSIS_G_LATERAL = 0x320, CHASSIS_G_LONG = 0x324, CHASSIS_BODY = 0x340, BODY_STATE = 0x18,
                 BODY_VELOCITY = 0x220;
constexpr size_t BODY_POSITION = 0x1c0, BODY_ANGULAR = 0x210, CHASSIS_BASIS = 0x190, BASIS_STRIDE = 0x10;
constexpr size_t CAR_AERO = 0x570, AERO_DRS = 0x19a8, WHEEL_SPIN = 0x188, WHEEL_SLIP_ANGLE = 0x3b0, WHEEL_SLIP_RATIO = 0x3b4;
constexpr size_t WHEEL_WETNESS = 0x280;
constexpr size_t STEER_ANGLE = 0xe8, STEER_LOCK = 0x28, SUSPENSION_WHEELS = 0x70, WHEEL_SURFACE = 0x200,
                 SURFACE_ID = 0x78;
// g_Game_WeatherSurfaceState: the track surface state; +0x3f0a38 is the track's wetness, 0 to 1, rewritten every frame
// by Game_TrackInformationSystem_Update. The pointer is null while no session world is loaded.
constexpr uintptr_t RVA_WEATHER_SURFACE_STATE = 0x6ce91d8;
constexpr size_t SURFACE_STATE_WETNESS = 0x3f0a38;
constexpr int MAX_CARS = 26;
constexpr float RAD_S_TO_RPM = 60.0f / 6.2831855f;

uintptr_t table = 0, count = 0;
bool resolved = false;
Fields fields;
int last_player = -1;

void resolve()
{
    resolved = true;
    fields = fields_of(runtime::same_build(), runtime::verified_build());
    if (!fields.table) {
        swofty::log("[shaders] live car signals are unavailable: this is not a build the car offsets are checked on");
        return;
    }
    if (swofty::api) {
        table = swofty::api->resolve("g_Sess_VehicleTable");
        count = swofty::api->resolve("g_Sess_VehicleCount");
    }
    if (!table || !count) {
        table = count = 0;
        swofty::log("[shaders] live car signals are unavailable: g_Sess_VehicleTable or g_Sess_VehicleCount does not "
                    "resolve");
        return;
    }
    if (!fields.wheels)
        swofty::log("[shaders] live car signals on this build read the proven car fields only: wheel surface, wheel "
                    "wetness, track wetness, orientation, wheel spin and slip are off until their offsets are checked on it");
}

uintptr_t car_at(int i)
{
    uintptr_t car = 0;
    if (!mem::read(table + 8 * uintptr_t(i), car) || !car || !mem::readable(car, CAR_IS_AI + 1))
        return 0;
    return car;
}

uintptr_t ptr(uintptr_t base, size_t off)
{
    uintptr_t p = 0;
    return mem::read(base + off, p) ? p : 0;
}

bool real(float v)
{
    return std::isfinite(v);
}

bool xor_float(uintptr_t at, float &out)
{
    uint32_t a, b;
    if (!mem::read(at, a) || !mem::read(at + 8, b))
        return false;
    a ^= b;
    __builtin_memcpy(&out, &a, 4);
    return real(out);
}

int find_player(uintptr_t &car, bool &ai)
{
    uint32_t n = 0;
    if (!mem::read(count, n))
        return -1;
    if (n > MAX_CARS)
        n = MAX_CARS;
    static const int forced = [] {
        char v[16] = {};
        return GetEnvironmentVariableA("SWOFTY_LIVE_CAR_SLOT", v, sizeof v) && v[0] ? std::atoi(v) : -1;
    }();
    if (forced >= 0 && uint32_t(forced) < n) {
        uintptr_t c = car_at(forced);
        uint8_t is_ai = 1;
        if (c) {
            mem::read(c + CAR_IS_AI, is_ai);
            car = c;
            ai = is_ai != 0;
            return forced;
        }
    }
    if (last_player >= 0 && uint32_t(last_player) < n) {
        uintptr_t c = car_at(last_player);
        uint8_t is_ai = 0;
        if (c && mem::read(c + CAR_IS_AI, is_ai) && !is_ai) {
            car = c;
            ai = false;
            return last_player;
        }
    }
    for (uint32_t i = 0; i < n; i++) {
        uintptr_t c = car_at(int(i));
        uint8_t is_ai = 1;
        if (c && mem::read(c + CAR_IS_AI, is_ai) && !is_ai) {
            last_player = int(i);
            car = c;
            ai = false;
            return last_player;
        }
    }
    if (last_player >= 0 && uint32_t(last_player) < n) {
        uintptr_t c = car_at(last_player);
        if (c) {
            car = c;
            ai = true;
            return last_player;
        }
    }
    return -1;
}

void read_surface(uintptr_t car, LiveCar &out)
{
    if (!fields.surface)
        return;
    uintptr_t susp = ptr(car, CAR_SUSPENSION);
    if (!susp)
        return;
    uint8_t game[4];
    float wettest = -1;
    for (int i = 0; i < 4; i++) {
        game[i] = 12;
        uintptr_t wheel = ptr(susp, SUSPENSION_WHEELS + 8 * size_t(i));
        if (!wheel)
            return;
        float w = 0;
        if (mem::read(wheel + WHEEL_WETNESS, w) && real(w) && w > wettest)
            wettest = w;
        uintptr_t rec = ptr(wheel, WHEEL_SURFACE);
        uint8_t id = 0;
        uint32_t wide = 0;
        if (rec && mem::read(rec + SURFACE_ID, wide)) {
            id = uint8_t(wide > 12 ? 12 : wide);
            game[i] = id;
        }
    }
    postfx::wheels_contract_order(game, out.surface);
    out.has_surface = true;
    if (wettest >= 0) {
        out.wheel_wetness = wettest > 1 ? 1 : wettest;
        out.has_wheel_wetness = true;
    }
}

bool vec3(uintptr_t at, float (&out)[3])
{
    return mem::read(at, out) && real(out[0]) && real(out[1]) && real(out[2]);
}

}

void read_wetness(LiveCar &out)
{
    if (!swofty::api || !fields.wetness)
        return;
    uintptr_t state = ptr(swofty::api->module_base() + RVA_WEATHER_SURFACE_STATE, 0);
    float w = 0;
    if (state && mem::read(state + SURFACE_STATE_WETNESS, w) && real(w)) {
        out.wetness = w < 0 ? 0 : w > 1 ? 1 : w;
        out.has_wetness = true;
    }
}

bool read(LiveCar &out)
{
    out = LiveCar{};
    if (!resolved)
        resolve();
    if (!table || !count)
        return false;
    read_wetness(out);
    uintptr_t car = 0;
    bool ai = false;
    int player = find_player(car, ai);
    if (player < 0)
        return false;
    out.player = player;
    out.ai = ai;

    uintptr_t gearbox = ptr(car, CAR_GEARBOX), engine = ptr(car, CAR_ENGINE), chassis = ptr(car, CAR_CHASSIS);
    uint32_t gear_index = 0;
    bool has_gear_index = gearbox && mem::read(gearbox + GEARBOX_INDEX, gear_index);
    if (has_gear_index && gear_index < 16) {
        out.gear = int(gear_index) - 1;
        out.has_gear = true;
    }

    if (chassis) {
        uintptr_t body = ptr(ptr(chassis, CHASSIS_BODY), BODY_STATE);
        float v[3];
        if (body && mem::read(body + BODY_VELOCITY, v) && real(v[0]) && real(v[1]) && real(v[2])) {
            out.speed_kmh = std::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) * 3.6f;
            out.has_speed = true;
        }
        float gl, gf;
        if (mem::read(chassis + CHASSIS_G_LATERAL, gl) && mem::read(chassis + CHASSIS_G_LONG, gf) && real(gl) && real(gf)) {
            out.g_lateral = gl;
            out.g_long = gf;
            out.has_g = true;
        }
    }

    float thr, brk;
    uint8_t flags = 0;
    if (has_gear_index && mem::read(car + CAR_THROTTLE, thr) && mem::read(car + CAR_BRAKE, brk) &&
        mem::read(car + CAR_FLAGS, flags) && real(thr) && real(brk)) {
        if (gear_index == 0 && (flags & 0x88) == 8)
            std::swap(thr, brk);
        out.throttle = thr;
        out.brake = brk;
        out.has_pedals = true;
    }

    if (uintptr_t steering = ptr(car, CAR_STEERING)) {
        float angle, lock;
        if (xor_float(steering + STEER_ANGLE, angle) && xor_float(steering + STEER_LOCK, lock) && lock != 0) {
            out.steer = std::fmax(-1.0f, std::fmin(1.0f, -angle / lock));
            out.has_steer = true;
        }
    }

    if (engine) {
        float rate, limit;
        if (mem::read(engine + ENGINE_RATE, rate) && mem::read(engine + ENGINE_LIMIT, limit) && real(rate) && real(limit) &&
            limit > 0) {
            out.rpm = rate * RAD_S_TO_RPM;
            out.max_rpm = limit * RAD_S_TO_RPM;
            out.has_rpm = true;
        }
    }

    read_surface(car, out);
    return true;
}

bool available()
{
    if (!resolved)
        resolve();
    return table && count;
}

bool kb_fields()
{
    if (!resolved)
        resolve();
    return fields.wheels;
}

uintptr_t player_car()
{
    if (!resolved)
        resolve();
    if (!table || !count)
        return 0;
    uintptr_t car = 0;
    bool ai = false;
    return find_player(car, ai) >= 0 && !ai ? car : 0;
}

bool read_dynamics(LiveDynamics &out)
{
    out = LiveDynamics{};
    if (!resolved)
        resolve();
    if (!table || !count)
        return false;
    uintptr_t car = 0;
    bool ai = false;
    int player = find_player(car, ai);
    if (player < 0)
        return false;
    out.player = player;
    out.ai = ai;

    if (uintptr_t chassis = ptr(car, CAR_CHASSIS)) {
        if (uintptr_t body = ptr(ptr(chassis, CHASSIS_BODY), BODY_STATE)) {
            out.has_pos = vec3(body + BODY_POSITION, out.pos);
            out.has_vel = vec3(body + BODY_VELOCITY, out.vel);
            out.has_angvel = fields.angvel && vec3(body + BODY_ANGULAR, out.angvel);
        }
        float axes[3][3];
        bool read = fields.orient;
        for (int c = 0; c < 3 && read; c++)
            read = vec3(chassis + CHASSIS_BASIS + BASIS_STRIDE * size_t(c), axes[c]);
        if (read && runtime::axes::unit_axes(axes)) {
            runtime::axes::quaternion_of(axes, out.quat);
            out.has_orient = true;
        }
    }

    if (uintptr_t susp = fields.wheels ? ptr(car, CAR_SUSPENSION) : 0) {
        bool spin = true, slip = true;
        for (int i = 0; i < 4; i++) {
            uintptr_t wheel = ptr(susp, SUSPENSION_WHEELS + 8 * size_t(i));
            if (!wheel) {
                spin = slip = false;
                break;
            }
            spin = spin && mem::read(wheel + WHEEL_SPIN, out.spin[i]) && real(out.spin[i]);
            slip = slip && mem::read(wheel + WHEEL_SLIP_ANGLE, out.slip_angle[i]) && real(out.slip_angle[i]) &&
                   mem::read(wheel + WHEEL_SLIP_RATIO, out.slip_ratio[i]) && real(out.slip_ratio[i]);
        }
        out.has_spin = spin;
        out.has_slip = slip;
    }

    if (uintptr_t aero = fields.drs ? ptr(car, CAR_AERO) : 0) {
        float drs;
        if (mem::read(aero + AERO_DRS, drs) && real(drs)) {
            out.drs = drs < 0 ? 0 : drs > 1 ? 1 : drs;
            out.has_drs = true;
        }
    }
    return true;
}

}
