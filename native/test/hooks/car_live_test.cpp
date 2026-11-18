// Host unit test for the core's live car operations (car_live.hpp) and the SDK side of cars::live
// (cars.hpp): the operations run on fake cars laid out as the KB's CarLive and its systems
// (Steering, GearBox, Differential, Turbo, FuelTank, F1WingDamage, F1SuspensionDamage) with fresh XOR keys
// on the obfuscated values. It checks that nothing is answered before the table is bound or without the
// globals, the car list and the slot/owner/handle checks, decoding and re-encoding (the key is kept, the
// plain half changes), range refusals (steering lock, differential, fuel over capacity, NaN), writes off
// the flow thread, a car without a system, cars::live::cars/get/set/scale/is_ai through the table, and the builds
// the operations run on (car_build_for).
// run_car_live_test.sh builds and runs it.
#include "../../src/core/hooks/car_live.hpp"
#include "../../src/core/hooks/player_facts.hpp"

#include <swofty/systems/cars.hpp>

#include <cmath>
#include <cstdio>
#include <cstring>
#include <string>

namespace cl = swofty_core::hooks::car_live;
namespace live = swofty::cars::live;
using swofty::Result;

static int fails;
static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

struct Tag {};
using Ops = cl::Ops<Tag>;

static constexpr int CARS = 3;
alignas(16) static uint8_t car_mem[CARS][0x600];
alignas(16) static uint8_t steering[CARS][0x100], gearbox[CARS][0x700], diff[CARS][0x100], turbo[CARS][0x310],
    tank[CARS][0x110], wing[CARS][0xc8], susp[CARS][0x140], engine[CARS][0x8c0];
static uintptr_t table[32];
static uint32_t count;
static bool flow = true;

template <class T> static T &at(uint8_t *base, size_t off) { return *reinterpret_cast<T *>(base + off); }

static uint32_t bits(float f)
{
    uint32_t b;
    memcpy(&b, &f, 4);
    return b;
}

static void put_obf(uint8_t *base, size_t off, float v, uint32_t key)
{
    at<uint32_t>(base, off + 8) = key;
    at<uint32_t>(base, off) = bits(v) ^ key;
}

static float plain(uint8_t *base, size_t off) { return at<float>(base, off); }

static void attach(uint8_t *car, uintptr_t slot, uint8_t *sys)
{
    at<void *>(car, slot) = sys;
    at<void *>(sys, cl::SYSTEM_CAR) = car;
}

static void build_world()
{
    memset(car_mem, 0, sizeof car_mem);
    memset(table, 0, sizeof table);
    for (int i = 0; i < CARS; i++) {
        uint8_t *c = car_mem[i];
        table[i] = reinterpret_cast<uintptr_t>(c);
        c[cl::CAR_VEHICLE_ID] = uint8_t(i);
        at<uint32_t>(c, cl::CAR_IS_AI) = i == 0 ? 0 : 1;
        attach(c, cl::STEERING, steering[i]);
        attach(c, cl::GEARBOX, gearbox[i]);
        attach(c, cl::DIFFERENTIAL, diff[i]);
        attach(c, cl::FUEL_TANK, tank[i]);
        attach(c, cl::WING_DAMAGE, wing[i]);
        attach(c, cl::SUSPENSION_DAMAGE, susp[i]);
        attach(c, cl::ENGINE, engine[i]);
        if (i != 2)
            attach(c, cl::TURBO, turbo[i]);
        put_obf(steering[i], 0x28, 0.34034f, 0xa5a50000u + i);
        at<float>(gearbox[i], 0xe8) = 1.5f;
        at<float>(diff[i], 0xf0) = 0.5f;
        at<float>(diff[i], 0xec) = 0.1f;
        at<float>(turbo[i], 0x2d8) = 1.0f;
        put_obf(tank[i], cl::FUEL_CAPACITY, 110.0f, 0x13572468u);
        at<float>(tank[i], 0x100) = 100.0f;
        at<float>(wing[i], 0x28) = 3.0f;
        at<float>(wing[i], 0x2c) = 0.0001f;
        at<float>(wing[i], 0x38) = 1.0f;
        put_obf(susp[i], 0x28, 7.0f, 0x0badf00du);
        put_obf(susp[i], 0x38, 0.001f, 0xdeadbeefu);
        put_obf(susp[i], 0x48, 0.0001f, 0x01020304u);
        put_obf(susp[i], 0x58, 1.0f, 0x55aa55aau);
        put_obf(susp[i], 0x68, 1.5f, 0xfeedfaceu);
        at<float>(susp[i], 0x78) = 50.0f;
        at<float>(susp[i], 0x7c) = 30.0f;
        put_obf(engine[i], 0x8b0, 1371.8f, 0x600dcafeu);
    }
    count = CARS;
}

static int writes = 0;

static cl::Env make_env(bool globals = true)
{
    cl::Env e;
    e.read = [](uintptr_t a, void *out, size_t n) {
        memcpy(out, reinterpret_cast<const void *>(a), n);
        return true;
    };
    e.write = [](uintptr_t a, const void *in, size_t n) {
        writes++;
        memcpy(reinterpret_cast<void *>(a), in, n);
        return true;
    };
    e.flow_thread = [] { return flow; };
    e.enabled = [] { return true; };
    if (globals) {
        e.table = reinterpret_cast<uintptr_t>(table);
        e.count = reinterpret_cast<uintptr_t>(&count);
    }
    return e;
}

static void core_ops()
{
    build_world();
    float v = 0;
    uint32_t n = 0;
    expect(!Ops::op_available() && Ops::op_count(&n) == SWOFTY_ASSET_UNAVAILABLE, "nothing before the table is bound");
    Ops::bind(make_env(false));
    expect(!Ops::op_available() && Ops::op_get(0, nullptr, SWOFTY_CAR_STEERING_LOCK, &v) == SWOFTY_ASSET_UNAVAILABLE,
           "unavailable when the vehicle table does not resolve");
    Ops::bind(make_env());
    expect(Ops::op_available() == 1, "available once bound");
    expect(Ops::op_count(&n) == SWOFTY_ASSET_OK && n == 3, "count reads g_Sess_VehicleCount");

    void *h = nullptr;
    uint32_t ai = 9;
    expect(Ops::op_car(0, &h, &ai) == SWOFTY_ASSET_OK && h == car_mem[0] && ai == 0, "car 0: handle and is_ai 0");
    expect(Ops::op_car(1, &h, &ai) == SWOFTY_ASSET_OK && h == car_mem[1] && ai == 1, "car 1: is_ai 1");
    expect(Ops::op_car(3, &h, nullptr) == SWOFTY_ASSET_LATE, "an index past the count is late");
    expect(Ops::op_car(40, &h, nullptr) == SWOFTY_ASSET_INVALID, "an index past 31 is invalid");

    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_STEERING_LOCK, &v) == SWOFTY_ASSET_OK && std::fabs(v - 0.34034f) < 1e-6f,
           "steering lock decodes the XOR pair");
    expect(Ops::op_get(1, car_mem[1], SWOFTY_CAR_SUSP_FRONT_DAMAGE_SCALE, &v) == SWOFTY_ASSET_OK &&
               std::fabs(v - 0.001f) < 1e-9f,
           "suspension front damage scale decodes with the handle checked");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_CLUTCH_TORQUE, &v) == SWOFTY_ASSET_OK && v == 1.5f, "clutch torque plain");
    expect(Ops::op_get(0, nullptr, 200, &v) == SWOFTY_ASSET_INVALID, "an unknown field is invalid");
    expect(Ops::op_get(0, car_mem[1], SWOFTY_CAR_CLUTCH_TORQUE, &v) == SWOFTY_ASSET_LATE,
           "a handle that isn't the slot's car is late");
    expect(Ops::op_get(2, nullptr, SWOFTY_CAR_TURBO_MAX_BOOST, &v) == SWOFTY_ASSET_UNAVAILABLE,
           "a car without the system answers unavailable");

    uint32_t key = at<uint32_t>(steering[0], 0x30);
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_STEERING_LOCK, 0.4f) == SWOFTY_ASSET_OK, "set steering lock");
    expect(at<uint32_t>(steering[0], 0x30) == key && (at<uint32_t>(steering[0], 0x28) ^ key) == bits(0.4f),
           "the obfuscated value is re-encoded under its key");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_STEERING_LOCK, &v) == SWOFTY_ASSET_OK && v == 0.4f, "and reads back");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_STEERING_LOCK, 2.0f) == SWOFTY_ASSET_INVALID &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_STEERING_LOCK, 0.0f) == SWOFTY_ASSET_INVALID,
           "steering lock outside 0.01 .. pi/2 is refused");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_DIFF_ON_THROTTLE, 1.2f) == SWOFTY_ASSET_INVALID &&
               plain(diff[0], 0xf0) == 0.5f,
           "differential above 1 is refused and untouched");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_DIFF_OFF_THROTTLE, 0.3f) == SWOFTY_ASSET_OK && plain(diff[0], 0xec) == 0.3f,
           "differential off throttle written plain");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_WING_DAMAGE_SCALE, NAN) == SWOFTY_ASSET_INVALID, "NaN is refused");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_FUEL_MASS, 120.0f) == SWOFTY_ASSET_INVALID &&
               plain(tank[0], 0x100) == 100.0f,
           "fuel over the tank's capacity is refused");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_FUEL_MASS, 50.0f) == SWOFTY_ASSET_OK && plain(tank[0], 0x100) == 50.0f,
           "fuel within capacity is written");
    flow = false;
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_CLUTCH_TORQUE, 2.0f) == SWOFTY_ASSET_LATE && plain(gearbox[0], 0xe8) == 1.5f,
           "a write off the flow thread is late and changes nothing");
    flow = true;

    at<void *>(susp[1], cl::SYSTEM_CAR) = car_mem[0];
    expect(Ops::op_set(1, nullptr, SWOFTY_CAR_SUSP_SPEED_THRESHOLD, 9.0f) == SWOFTY_ASSET_LATE,
           "a system whose owner is another car is late");
    at<void *>(susp[1], cl::SYSTEM_CAR) = car_mem[1];
    car_mem[1][cl::CAR_VEHICLE_ID] = 2;
    expect(Ops::op_car(1, &h, nullptr) == SWOFTY_ASSET_LATE, "a slot whose car has another vehicle id is late");
    car_mem[1][cl::CAR_VEHICLE_ID] = 1;
    table[1] = 0;
    expect(Ops::op_car(1, &h, nullptr) == SWOFTY_ASSET_LATE, "an empty slot is late");
    table[1] = reinterpret_cast<uintptr_t>(car_mem[1]);
}

static void sdk_side()
{
    build_world();
    Ops::bind(make_env());
    live::Car none(0);
    expect(!live::available() && live::cars().empty() && !none.get(live::Field::steering_lock) &&
               none.set(live::Field::steering_lock, 0.3f) == Result::unavailable && none.is_ai(),
           "the SDK answers nothing without the table");
    swofty::detail::set_car_ops_table(&Ops::table);
    expect(live::available(), "available through the table");
    auto all = live::cars();
    expect(all.size() == 3 && all[1].index() == 1 && all[1].handle() == car_mem[1], "cars() lists index and handle");
    expect(!all[0].is_ai() && all[1].is_ai(), "is_ai reads the car");
    auto lock = all[0].get(live::Field::steering_lock);
    expect(lock && std::fabs(*lock - 0.34034f) < 1e-6f, "get decodes");
    expect(all[0].scale(live::Field::steering_lock, 1.1f) == Result::ok && all[0].get(live::Field::steering_lock) &&
               std::fabs(*all[0].get(live::Field::steering_lock) - 0.374374f) < 1e-5f,
           "scale reads, multiplies and writes");
    expect(all[1].set(live::Field::susp_wheel_detach_scale, 0.8f) == Result::ok &&
               all[1].get(live::Field::susp_wheel_detach_scale) == 0.8f,
           "set re-encodes a suspension value");
    expect(all[1].set(live::Field::diff_on_throttle, -0.1f) == Result::invalid, "set reports a refused value");
    expect(all[2].scale(live::Field::turbo_max_boost, 2.0f) == Result::unavailable, "scale passes unavailable on");
    table[1] = reinterpret_cast<uintptr_t>(car_mem[2]);
    expect(!all[1].get(live::Field::clutch_torque) && all[1].is_ai(), "a stale handle reads nothing");
    swofty::detail::set_car_ops_table(nullptr);
}

static void pending_queue()
{
    build_world();
    Ops::bind(make_env());
    auto addr = [](int i) { return reinterpret_cast<uintptr_t>(car_mem[i]); };
    Ops::built(addr(1));
    Ops::reset(addr(0) + cl::CAR_LOADER);
    Ops::built(addr(2));
    car_mem[0][cl::CAR_VEHICLE_ID] = 40;
    Ops::built(addr(0));
    car_mem[0][cl::CAR_VEHICLE_ID] = 0;
    Ops::reset(0);
    table[2] = addr(1);
    auto got = Ops::take();
    expect(got.size() == 2 && got[0].index == 1 && got[0].car == addr(1) && got[0].what == Ops::What::built,
           "car.built keeps a built car still in its slot, drops a bad id and a car gone from its slot");
    expect(got.size() == 2 && got[1].index == 0 && got[1].car == addr(0) && got[1].what == Ops::What::reset,
           "car.reset finds the car from the reset thunk's loader (CarLive+0x4a0); a null loader is dropped");
    expect(Ops::take().empty(), "the queue is taken once");
    alignas(16) static uint8_t front_wing[0x100];
    *reinterpret_cast<uintptr_t *>(front_wing + cl::SYSTEM_CAR) = addr(1);
    Ops::damaged(reinterpret_cast<uintptr_t>(front_wing), 1, 0.25f);
    Ops::damaged(reinterpret_cast<uintptr_t>(front_wing), 0, 0.0f);
    auto hit = Ops::take();
    expect(hit.size() == 1 && hit[0].index == 1 && hit[0].what == Ops::What::damage && hit[0].part == 1 &&
               hit[0].progress == 0.25f,
           "car.damage queues a front wing hit with its side and amount and drops a hit that added none");
    for (int i = 0; i < 300; i++)
        Ops::built(addr(1));
    expect(Ops::take().size() == 8 * cl::MAX_CARS, "the queue is capped when no frame takes it");
}

static void engine_limiter()
{
    build_world();
    Ops::bind(make_env());
    float v = 0;
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_ENGINE_LIMITER, &v) == SWOFTY_ASSET_OK && v == 1371.8f,
           "engine limiter decodes LimiterRotationRate");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_ENGINE_LIMITER, 1300.0f) == SWOFTY_ASSET_UNAVAILABLE &&
               (at<uint32_t>(engine[0], 0x8b0) ^ 0x600dcafeu) == bits(1371.8f),
           "without the derived-values function the limiter is unavailable and nothing is written");
    static uintptr_t derived_on = 0;
    cl::Env e = make_env();
    e.engine_derived = [](uintptr_t eng) { derived_on = eng; };
    Ops::bind(std::move(e));
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_ENGINE_LIMITER, 1300.0f) == SWOFTY_ASSET_OK &&
               (at<uint32_t>(engine[0], 0x8b0) ^ 0x600dcafeu) == bits(1300.0f) &&
               derived_on == reinterpret_cast<uintptr_t>(engine[0]),
           "set writes the limiter under its key and re-runs the engine's derived values on that engine");
    derived_on = 0;
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_ENGINE_LIMITER, 3000.0f) == SWOFTY_ASSET_INVALID && !derived_on,
           "a limiter outside 100..2500 rad/s is invalid and derives nothing");
}

static void gear_ratios()
{
    build_world();
    put_obf(gearbox[0], 0x28, 4.9f, 0x0f0f0f0fu);
    put_obf(gearbox[0], 0x78, 2.1f, 0x12121212u);
    Ops::bind(make_env());
    float v = 0;
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_GEAR_RATIO_1 + 2, &v) == SWOFTY_ASSET_OK && v == 2.1f &&
               Ops::op_get(0, nullptr, SWOFTY_CAR_FINAL_DRIVE, &v) == SWOFTY_ASSET_OK && v == 4.9f,
           "3rd gear (+0x78) and the final drive (+0x28) decode");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_GEAR_RATIO_1 + 2, 2.3f) == SWOFTY_ASSET_UNAVAILABLE,
           "without the gearbox reset the ratios are unavailable");
    static uintptr_t reset_on = 0;
    static uint32_t reset_flags = 0;
    cl::Env e = make_env();
    e.gearbox_reset = [](uintptr_t gb, uint32_t flags) {
        reset_on = gb;
        reset_flags = flags;
    };
    Ops::bind(std::move(e));
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_GEAR_RATIO_1 + 2, 2.3f) == SWOFTY_ASSET_OK &&
               (at<uint32_t>(gearbox[0], 0x78) ^ 0x12121212u) == bits(2.3f) &&
               reset_on == reinterpret_cast<uintptr_t>(gearbox[0]) && reset_flags == 0x2020,
           "set writes the ratio under its key and rebuilds the table with 0x20|0x2000 (no neutral)");
    reset_on = 0;
    gearbox[0][cl::GEARBOX_USE_SETUP_RATIOS] = 1;
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_FINAL_DRIVE, 5.0f) == SWOFTY_ASSET_UNAVAILABLE && !reset_on &&
               (at<uint32_t>(gearbox[0], 0x28) ^ 0x0f0f0f0fu) == bits(4.9f),
           "with setup ratios on the handling ratios would be ignored: unavailable, nothing written");
    gearbox[0][cl::GEARBOX_USE_SETUP_RATIOS] = 0;
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_GEAR_RATIO_1, 0.0f) == SWOFTY_ASSET_INVALID,
           "a zero ratio is invalid");
}

alignas(16) static uint8_t aero[CARS][0x1a10], brake[CARS][0x2210], chassis[CARS][0x200], suspension[CARS][0xd8],
    tyre_front[CARS][0x80], tyre_rear[CARS][0x80], axle_a[CARS][0x1670], axle_b[CARS][0x1670];

static void build_v2()
{
    build_world();
    for (int i = 0; i < CARS; i++) {
        uint8_t *c = car_mem[i];
        attach(c, cl::F1_AERO, aero[i]);
        attach(c, cl::BRAKE_ECU, brake[i]);
        attach(c, cl::CHASSIS, chassis[i]);
        attach(c, cl::SUSPENSION, suspension[i]);
        at<float>(aero[i], 0x19a4) = 1.0f;
        at<float>(aero[i], 0x19a0) = 1.0f;
        put_obf(aero[i], 0x118, 0.65f, 0x2468ace0u);
        at<float>(brake[i], 0x21d8) = 0.4f;
        at<float>(brake[i], 0x21dc) = 1.0f;
        put_obf(chassis[i], 0x28, 800.0f, 0x31415926u);
        put_obf(chassis[i], 0x98, std::sqrt(800.0f), 0x27182818u);
        memset(tyre_front[i], 0, sizeof tyre_front[i]);
        memset(tyre_rear[i], 0, sizeof tyre_rear[i]);
        strcpy(reinterpret_cast<char *>(tyre_rear[i]), "Rear");
        strcpy(reinterpret_cast<char *>(tyre_front[i]), "Front");
        at<float>(tyre_front[i], 0x2c) = 1.0f;
        at<float>(tyre_rear[i], 0x2c) = 1.1f;
        at<void *>(suspension[i], cl::SUSP_TYRE_TYPES) = tyre_rear[i];
        at<void *>(suspension[i], cl::SUSP_TYRE_TYPES + 8) = tyre_front[i];
        at<void *>(suspension[i], cl::SUSP_AXLE0) = axle_a[i];
        at<void *>(suspension[i], cl::SUSP_AXLE1) = axle_b[i];
        at<float>(axle_a[i], cl::AXLE_LOCAL_REST_Z) = -1.7f;
        at<float>(axle_b[i], cl::AXLE_LOCAL_REST_Z) = 1.6f;
        at<float>(axle_a[i], 0x1664) = 50000.0f;
        at<float>(axle_b[i], 0x1664) = 30000.0f;
    }
}

static uint32_t token_of(Ops &, uint32_t t) { return t; }

static void v2_fields()
{
    build_v2();
    Ops::bind(make_env());
    uint32_t id = 0;
    expect(Ops::op_field("aero.lift_scale", &id) == SWOFTY_ASSET_OK && id == SWOFTY_CAR_AERO_LIFT_SCALE &&
               Ops::op_field("steering.max_steering_lock", &id) == SWOFTY_ASSET_OK && id == SWOFTY_CAR_STEERING_LOCK &&
               Ops::op_field("nope.field", &id) == SWOFTY_ASSET_INVALID,
           "field(path) resolves a Handling path and refuses an unknown one");
    for (size_t i = 0; i < cl::FIELD_COUNT; i++)
        for (size_t j = i + 1; j < cl::FIELD_COUNT; j++)
            if (cl::FIELDS[i].id == cl::FIELDS[j].id || !std::strcmp(cl::FIELDS[i].name, cl::FIELDS[j].name))
                expect(false, std::string("duplicate field ") + cl::FIELDS[i].name);
    float v = 0;
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_AERO_LIFT_SCALE, &v) == SWOFTY_ASSET_OK && v == 1.0f &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_AERO_DRAG_SCALE, 0.96f) == SWOFTY_ASSET_OK &&
               plain(aero[0], 0x19a0) == 0.96f,
           "the aero scales are plain floats at +0x19a4 and +0x19a0 of F1Aero");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_AERO_SLIPSTREAM_DRAG, &v) == SWOFTY_ASSET_OK && std::fabs(v - 0.65f) < 1e-6f &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_AERO_SLIPSTREAM_DRAG, 0.6f) == SWOFTY_ASSET_OK &&
               Ops::op_get(0, nullptr, SWOFTY_CAR_AERO_SLIPSTREAM_DRAG, &v) == SWOFTY_ASSET_OK && std::fabs(v - 0.6f) < 1e-6f &&
               Ops::op_get(1, nullptr, SWOFTY_CAR_AERO_SLIPSTREAM_DRAG, &v) == SWOFTY_ASSET_OK && std::fabs(v - 0.65f) < 1e-6f &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_AERO_SLIPSTREAM_DRAG, 1.5f) == SWOFTY_ASSET_INVALID,
           "the slipstream drag scale is an obfuscated float at +0x118 of F1Aero, per car, refused above 1");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_BRAKES_BIAS, &v) == SWOFTY_ASSET_OK && v == 0.4f &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_BRAKES_PRESSURE, 0.9f) == SWOFTY_ASSET_OK && plain(brake[0], 0x21dc) == 0.9f,
           "brake bias and pressure are plain floats at +0x21d8 and +0x21dc of the BrakeECU");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_TYRES_FRONT_GRIP_SCALE, &v) == SWOFTY_ASSET_OK && v == 1.0f &&
               Ops::op_get(0, nullptr, SWOFTY_CAR_TYRES_REAR_GRIP_SCALE, &v) == SWOFTY_ASSET_OK && v == 1.1f,
           "the tyre records are found by name, whatever their order");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_TYRES_REAR_GRIP_SCALE, 0.9f) == SWOFTY_ASSET_OK &&
               plain(tyre_rear[0], 0x2c) == 0.9f && plain(tyre_front[0], 0x2c) == 1.0f,
           "grip scale writes only its axle's record");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_SUSP_REAR_ANTI_ROLL, &v) == SWOFTY_ASSET_OK && v == 50000.0f &&
               Ops::op_get(0, nullptr, SWOFTY_CAR_SUSP_FRONT_ANTI_ROLL, &v) == SWOFTY_ASSET_OK && v == 30000.0f,
           "the rear axle is the one with the lower local rest position");
    at<float>(axle_b[1], cl::AXLE_LOCAL_REST_Z) = -1.0f;
    expect(Ops::op_get(1, nullptr, SWOFTY_CAR_SUSP_FRONT_ANTI_ROLL, &v) == SWOFTY_ASSET_UNAVAILABLE,
           "two axles on the same side are unavailable");
    at<float>(axle_b[1], cl::AXLE_LOCAL_REST_Z) = 1.6f;

    static uintptr_t chassis_reset_on = 0;
    static uint32_t chassis_flags = 0;
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_CHASSIS_BALLAST_KG, 20.0f) == SWOFTY_ASSET_UNAVAILABLE,
           "ballast is unavailable without the chassis reset");
    cl::Env e = make_env();
    e.chassis_reset = [](uintptr_t c, uint32_t f) {
        chassis_reset_on = c;
        chassis_flags = f;
    };
    Ops::bind(std::move(e));
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_CHASSIS_BALLAST_KG, 20.0f) == SWOFTY_ASSET_OK &&
               (at<uint32_t>(chassis[0], 0x28) ^ 0x31415926u) == bits(820.0f) &&
               std::fabs(std::sqrt(820.0f) - [] { uint32_t b = at<uint32_t>(chassis[0], 0x98) ^ 0x27182818u; float f; memcpy(&f, &b, 4); return f; }()) < 1e-4f &&
               chassis_reset_on == reinterpret_cast<uintptr_t>(chassis[0]) && chassis_flags == 0x208,
           "ballast adds to the VTF mass, writes its square root and runs the chassis reset with 0x208");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_CHASSIS_BALLAST_KG, &v) == SWOFTY_ASSET_OK && std::fabs(v - 20.0f) < 1e-3f,
           "ballast reads back as the kilograms added");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_CHASSIS_BALLAST_KG, 30.0f) == SWOFTY_ASSET_OK &&
               (at<uint32_t>(chassis[0], 0x28) ^ 0x31415926u) == bits(830.0f),
           "setting ballast twice does not compound");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_CHASSIS_BALLAST_KG, 900.0f) == SWOFTY_ASSET_INVALID,
           "ballast outside -300 .. 600 is refused");
}

static void v2_layers()
{
    build_v2();
    Ops::bind(make_env());
    float v = 0;
    uint32_t a = 0, b = 0, c = 0;
    expect(Ops::op_layer("one", SWOFTY_CAR_ALL, SWOFTY_CAR_AERO_LIFT_SCALE, SWOFTY_CAR_OP_SCALE, 0.5f, 0, &a) == SWOFTY_ASSET_OK &&
               plain(aero[0], 0x19a4) == 0.5f && plain(aero[1], 0x19a4) == 0.5f && plain(aero[2], 0x19a4) == 0.5f,
           "a layer for all cars applies at once on the flow thread");
    Ops::flush();
    Ops::apply_all();
    expect(plain(aero[0], 0x19a4) == 0.5f, "applying the same layers again does not compound");
    expect(Ops::op_layer("two", 1, SWOFTY_CAR_AERO_LIFT_SCALE, SWOFTY_CAR_OP_ADD, 0.1f, 0, &b) == SWOFTY_ASSET_OK &&
               std::fabs(plain(aero[1], 0x19a4) - 0.6f) < 1e-6f && plain(aero[0], 0x19a4) == 0.5f,
           "a layer for one car composes in load order and leaves the others");
    expect(Ops::op_layer("three", 1, SWOFTY_CAR_AERO_LIFT_SCALE, SWOFTY_CAR_OP_SET, 2.0f, -5, &c) == SWOFTY_ASSET_OK &&
               std::fabs(plain(aero[1], 0x19a4) - 1.1f) < 1e-6f,
           "a lower priority applies first: the set at priority -5 is the base the scale and add then act on");
    expect(Ops::op_layer("four", 2, SWOFTY_CAR_AERO_LIFT_SCALE, SWOFTY_CAR_OP_SET, 3.0f, 10, nullptr) == SWOFTY_ASSET_OK &&
               plain(aero[2], 0x19a4) == 3.0f,
           "a higher priority set wins");
    at<float>(aero[0], 0x19a4) = 1.0f;
    expect(plain(aero[0], 0x19a4) == 1.0f, "(the game resets a value)");
    Ops::reset(reinterpret_cast<uintptr_t>(car_mem[0]) + cl::CAR_LOADER);
    for (auto &p : Ops::take())
        Ops::apply_car(p.index);
    expect(plain(aero[0], 0x19a4) == 0.5f, "after car.reset the layers are applied again to the game's value");
    expect(Ops::op_unlayer(a) == SWOFTY_ASSET_OK && plain(aero[0], 0x19a4) == 1.0f && std::fabs(plain(aero[1], 0x19a4) - 2.1f) < 1e-6f,
           "unlayer restores a car with no layer left and recomposes the others");
    expect(Ops::op_unlayer(a) == SWOFTY_ASSET_INVALID, "an unknown token is invalid");
    Ops::drop_owner("one", SWOFTY_CAR_ALL);
    Ops::drop_owner("two", 1);
    Ops::drop_owner("three", 1);
    Ops::flush();
    expect(plain(aero[1], 0x19a4) == 1.0f, "dropping an owner's layers restores the car");
    expect(Ops::op_layer("x", 40, SWOFTY_CAR_AERO_LIFT_SCALE, SWOFTY_CAR_OP_SET, 1.0f, 0, nullptr) == SWOFTY_ASSET_INVALID &&
               Ops::op_layer("x", 0, SWOFTY_CAR_AERO_LIFT_SCALE, 7, 1.0f, 0, nullptr) == SWOFTY_ASSET_INVALID &&
               Ops::op_layer("x", 0, 999, SWOFTY_CAR_OP_SET, 1.0f, 0, nullptr) == SWOFTY_ASSET_INVALID,
           "an index past the cars, an unknown op and an unknown field are invalid");
    Ops::op_layer("clamp", 0, SWOFTY_CAR_AERO_DRAG_SCALE, SWOFTY_CAR_OP_SCALE, 100.0f, 0, nullptr);
    expect(plain(aero[0], 0x19a0) == 10.0f, "a composed value is clamped to the field's range");
    (void)token_of;
    (void)v;
}

static void v2_virtual()
{
    build_v2();
    Ops::bind(make_env());
    float v = 0;
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_ENGINE_POWER_SCALE, &v) == SWOFTY_ASSET_OK && v == 1.0f &&
               Ops::op_get(0, nullptr, SWOFTY_CAR_ENGINE_LIMITER_OFFSET_RPM, &v) == SWOFTY_ASSET_OK && v == 0.0f &&
               Ops::op_get(0, nullptr, SWOFTY_CAR_AI_GRIP_SCALE, &v) == SWOFTY_ASSET_OK && v == 1.0f,
           "virtual fields start at their neutral value");
    uintptr_t eng = reinterpret_cast<uintptr_t>(engine[1]);
    expect(Ops::drive_torque(eng, 100.0f, 1000.0f, 1371.8f) == 100.0f && Ops::ai_grip(1, 0, 2.0f) == 2.0f,
           "no layer: the handlers change nothing");
    int limiter_reads = 0;
    auto limiter = [&] {
        ++limiter_reads;
        return 1371.8f;
    };
    expect(Ops::drive_torque_with(eng, 100.0f, 1000.0f, limiter) == 100.0f && limiter_reads == 0,
           "no layer: the engine site reads no limiter");
    Ops::op_layer("profile", SWOFTY_CAR_ALL, SWOFTY_CAR_ENGINE_POWER_SCALE, SWOFTY_CAR_OP_SCALE, 0.9725f, 0, nullptr);
    expect(std::fabs(Ops::drive_torque_with(eng, 100.0f, 1000.0f, limiter) - 97.25f) < 1e-3f && limiter_reads == 1,
           "with a power layer the engine site reads the limiter once");
    Ops::op_layer("profile", SWOFTY_CAR_ALL, SWOFTY_CAR_ENGINE_LIMITER_OFFSET_RPM, SWOFTY_CAR_OP_ADD, -300.0f, 0, nullptr);
    Ops::op_layer("profile", SWOFTY_CAR_ALL, SWOFTY_CAR_AI_GRIP_SCALE, SWOFTY_CAR_OP_SCALE, 0.87f, 0, nullptr);
    float t = Ops::drive_torque(eng, 100.0f, 1000.0f, 1371.8f);
    expect(std::fabs(t - 97.25f) < 1e-3f, "below the cut the torque is scaled by the power factor only");
    float cut = 1371.8f - 300.0f * cl::RPM_TO_RAD_S;
    expect(Ops::drive_torque(eng, 100.0f, cut, 1371.8f) == 0.0f && Ops::drive_torque(eng, 100.0f, cut + 5.0f, 1371.8f) == 0.0f,
           "at the lowered limiter the torque is cut to zero");
    float mid = Ops::drive_torque(eng, 100.0f, cut - 3.0f, 1371.8f);
    expect(mid > 0.0f && mid < 97.25f, "inside the ramp under the lowered limiter it falls linearly");
    expect(std::fabs(Ops::ai_grip(1, 0, 2.0f) - 1.74f) < 1e-5f && Ops::ai_grip(1, 1, 2.0f) == 2.0f,
           "AI grip is scaled in the dry only");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_ENGINE_POWER_SCALE, &v) == SWOFTY_ASSET_OK && v == 0.9725f,
           "a virtual field reads the composed value");
    Ops::op_layer("one-car", 1, SWOFTY_CAR_ENGINE_POWER_SCALE, SWOFTY_CAR_OP_SET, 1.2f, 5, nullptr);
    expect(std::fabs(Ops::drive_torque(eng, 100.0f, 1000.0f, 1371.8f) - 120.0f) < 1e-3f &&
               std::fabs(Ops::drive_torque(reinterpret_cast<uintptr_t>(engine[0]), 100.0f, 1000.0f, 1371.8f) - 97.25f) < 1e-3f,
           "a layer for one car does not reach the others, and the higher priority set wins");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_ENGINE_POWER_SCALE, 9.0f) == SWOFTY_ASSET_INVALID, "power scale above 4 is refused");
    expect(std::fabs(cl::shaped_torque(100.0f, 1300.0f, 1371.8f, 1.0f, 300.0f) - 100.0f) < 1e-4f,
           "a positive limiter offset changes nothing");
    Ops::drop_owner("profile", SWOFTY_CAR_ALL);
    Ops::drop_owner("one-car", SWOFTY_CAR_ALL);
    expect(Ops::drive_torque(eng, 100.0f, 1000.0f, 1371.8f) == 100.0f && Ops::ai_grip(1, 0, 2.0f) == 2.0f,
           "dropping the layers returns the handlers to doing nothing");
}

static void v2_gating()
{
    build_v2();
    static std::string asked;
    cl::Env e = make_env();
    e.row_ready = [](const char *row) {
        asked = row;
        return std::strcmp(row, "CarF1Aero.lift_scale") != 0 && std::strcmp(row, "site_Ai_TableGripInput") != 0;
    };
    Ops::bind(std::move(e));
    float v = 0;
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_AERO_LIFT_SCALE, &v) == SWOFTY_ASSET_UNAVAILABLE &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_AERO_LIFT_SCALE, 0.5f) == SWOFTY_ASSET_UNAVAILABLE &&
               plain(aero[0], 0x19a4) == 1.0f,
           "a field whose row has no verify row is unavailable and writes nothing");
    expect(Ops::op_layer("x", SWOFTY_CAR_ALL, SWOFTY_CAR_AERO_LIFT_SCALE, SWOFTY_CAR_OP_SET, 0.5f, 0, nullptr) ==
                   SWOFTY_ASSET_UNAVAILABLE &&
               Ops::op_layer("x", SWOFTY_CAR_ALL, SWOFTY_CAR_AI_GRIP_SCALE, SWOFTY_CAR_OP_SET, 0.5f, 0, nullptr) ==
                   SWOFTY_ASSET_UNAVAILABLE,
           "a layer on an off field is unavailable, and AI grip stays off until its site row is verified");
    expect(Ops::op_layer("x", SWOFTY_CAR_ALL, SWOFTY_CAR_AERO_DRAG_SCALE, SWOFTY_CAR_OP_SET, 0.9f, 0, nullptr) == SWOFTY_ASSET_OK &&
               plain(aero[0], 0x19a0) == 0.9f,
           "the other fields keep working");
    expect(Ops::op_refit_tyres(0, "F1_1990") == SWOFTY_ASSET_UNAVAILABLE && Ops::op_refit_tyres(0, "bad name") == SWOFTY_ASSET_INVALID &&
               Ops::op_refit_tyres(0, nullptr) == SWOFTY_ASSET_INVALID,
           "refit_tyres is unavailable without its rows and refuses a bad group name");
}

static void v2_refit()
{
    build_v2();
    static char buffer[2][0x28];
    static uintptr_t refitted = 0;
    memset(buffer, 0, sizeof buffer);
    for (int i = 0; i < CARS; i++) {
        at<size_t>(chassis[i], cl::CHASSIS_GROUP + cl::STRING_CAPACITY) = 0x20;
        at<void *>(chassis[i], cl::CHASSIS_GROUP + cl::STRING_BUFFER) = buffer[i % 2];
    }
    cl::Env e = make_env();
    e.tyre_component = [](uint32_t index, uintptr_t) { return index == 2 ? uintptr_t(0) : uintptr_t(0x1000 + index); };
    e.tyre_refit = [](uintptr_t tc) { refitted = tc; };
    Ops::bind(std::move(e));
    expect(Ops::op_refit_tyres(1, "F1_1990P") == SWOFTY_ASSET_OK && !std::strcmp(buffer[1], "F1_1990P") && refitted == 0x1001,
           "refit writes the group into the chassis string and refits that car's tyre component");
    refitted = 0;
    expect(Ops::op_refit_tyres(2, "F1_1990") == SWOFTY_ASSET_UNAVAILABLE && !refitted,
           "a car without a tyre component is unavailable and nothing is written");
    expect(Ops::op_refit_tyres(0, "ThisGroupNameIsFarTooLongForTheStringXX") == SWOFTY_ASSET_INVALID, "a name past 31 characters is invalid");
}

alignas(16) static uint8_t ecu[CARS][0x170];
alignas(16) static uint8_t avail_mem[0x400];

static void v3_fuel()
{
    build_world();
    for (int i = 0; i < CARS; i++) {
        attach(car_mem[i], cl::FUEL_ECU, ecu[i]);
        for (int k = 0; k < 16; k++)
            put_obf(ecu[i], 0x28 + 0x10 * k, 1.0f, 0x7000u + k + i);
        at<int32_t>(ecu[i], cl::FUEL_ECU_MODE) = 1;
    }
    static std::vector<std::pair<uintptr_t, int>> calls;
    calls.clear();
    cl::Env e = make_env();
    e.fuel_set_mode = [](uintptr_t ecu_at, int mode) { calls.emplace_back(ecu_at, mode); };
    e.mode_avail = [] { return reinterpret_cast<uintptr_t>(avail_mem); };
    Ops::bind(std::move(e));

    float v = 0;
    uint32_t id = 0;
    expect(Ops::op_field("fuel_ecu.fuel_power_scale.rich", &id) == SWOFTY_ASSET_OK && id == SWOFTY_CAR_FUEL_POWER_LEAN + 2,
           "fuel_ecu.fuel_power_scale.rich is field 54");
    expect(Ops::op_field("fuel_ecu.temperature_scale.max", &id) == SWOFTY_ASSET_OK && id == SWOFTY_CAR_FUEL_TEMPERATURE_LEAN + 3,
           "fuel_ecu.temperature_scale.max is field 63");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_FUEL_POWER_LEAN + 2, 1.05f) == SWOFTY_ASSET_OK &&
               (at<uint32_t>(ecu[0], 0x28 + 0x40 + 0x20) ^ (0x7000u + 6)) == bits(1.05f) &&
               (at<uint32_t>(ecu[0], 0x28 + 0x40 + 0x10) ^ (0x7000u + 5)) == bits(1.0f),
           "a mode table entry is written under its own key and only that entry");
    expect(calls.size() == 2 && calls[0] == std::make_pair(reinterpret_cast<uintptr_t>(ecu[0]), 0) &&
               calls[1] == std::make_pair(reinterpret_cast<uintptr_t>(ecu[0]), 1),
           "the active mode is re-run: another mode and back");
    calls.clear();
    at<int32_t>(ecu[0], cl::FUEL_ECU_MODE) = 0;
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_FUEL_BURN_LEAN, 0.9f) == SWOFTY_ASSET_OK && calls.size() == 2 &&
               calls[0].second == 1 && calls[1].second == 0,
           "from lean the toggle goes to standard and back");
    expect(Ops::op_set(0, nullptr, SWOFTY_CAR_FUEL_REVS_LEAN, 0.1f) == SWOFTY_ASSET_INVALID &&
               Ops::op_set(0, nullptr, SWOFTY_CAR_FUEL_BURN_LEAN, 11.0f) == SWOFTY_ASSET_INVALID,
           "values outside the range are invalid");
    expect(Ops::op_get(0, nullptr, SWOFTY_CAR_FUEL_BURN_LEAN, &v) == SWOFTY_ASSET_OK && std::fabs(v - 0.9f) < 1e-6f,
           "a table entry reads back");

    uint32_t mode = 9;
    expect(Ops::op_fuel_mix(0, &mode) == SWOFTY_ASSET_OK && mode == 0, "fuel_mix reads the car's mode");

    memset(avail_mem, 0xff, sizeof avail_mem);
    expect(Ops::op_unlock_fuel_mixes("", 4) == SWOFTY_ASSET_INVALID && Ops::op_unlock_fuel_mixes("a", 16) == SWOFTY_ASSET_INVALID,
           "an empty owner and an unknown mix bit are invalid");
    expect(Ops::op_unlock_fuel_mixes("a", SWOFTY_CAR_FUEL_MIX_RICH) == SWOFTY_ASSET_OK, "an owner unlocks rich");
    Ops::unlock_tick();
    bool ok = true;
    for (uint32_t c = 0; c < CARS; c++)
        for (uint32_t m = 0; m < 4; m++) {
            uint32_t got = at<uint32_t>(avail_mem, cl::MODE_AVAIL_FUEL + c * cl::MODE_AVAIL_STRIDE + m * 4);
            ok = ok && (got == (m == 2 ? 0u : 0xffffffffu));
        }
    expect(ok, "only the rich entry of every car is cleared");
    memset(avail_mem, 0xff, sizeof avail_mem);
    count = 32;
    Ops::unlock_tick();
    count = CARS;
    bool tail = true;
    for (size_t i = cl::MODE_AVAIL_FUEL + cl::MODE_AVAIL_CARS * cl::MODE_AVAIL_STRIDE; i < sizeof avail_mem; i++)
        tail = tail && avail_mem[i] == 0xff;
    expect(tail, "the unlock never writes past the 26 car entries of the table");
    memset(avail_mem, 0xff, sizeof avail_mem);
    Ops::unlock_tick();
    writes = 0;
    Ops::unlock_tick();
    expect(writes == 0, "an entry already cleared is not written again");
    uint32_t one = 1;
    memcpy(avail_mem + cl::MODE_AVAIL_FUEL + 1 * cl::MODE_AVAIL_STRIDE + 8, &one, 4);
    Ops::unlock_tick();
    expect(writes == 1 && at<uint32_t>(avail_mem, cl::MODE_AVAIL_FUEL + 1 * cl::MODE_AVAIL_STRIDE + 8) == 0,
           "only the entry the game relocked is written");
    expect(Ops::op_unlock_fuel_mixes("a", 0) == SWOFTY_ASSET_OK && Ops::unlocked_mask() == 0, "withdrawing the mask stops the writes");

    expect(SWOFTY_CAR_OPS_VERSION >= 4 && Ops::table.version >= 4 && Ops::table.set_fuel_mix == Ops::op_set_fuel_mix,
           "the op table offers set_fuel_mix from version 4");
    calls.clear();
    memset(avail_mem, 0xff, sizeof avail_mem);
    for (uint32_t m : { 0u, 1u })
        memset(avail_mem + cl::MODE_AVAIL_FUEL + m * 4, 0, 4);
    at<int32_t>(ecu[0], cl::FUEL_ECU_MODE) = 0;
    expect(Ops::op_set_fuel_mix(0, 4) == SWOFTY_ASSET_INVALID && Ops::op_set_fuel_mix(0, 0xffffffffu) == SWOFTY_ASSET_INVALID,
           "a mode above 3 is invalid");
    expect(Ops::op_set_fuel_mix(CARS, 1) != SWOFTY_ASSET_OK, "an unknown car is refused");
    expect(Ops::op_set_fuel_mix(0, 2) == SWOFTY_ASSET_INVALID && calls.empty(), "a mix the MFD does not offer is refused untouched");
    expect(Ops::op_set_fuel_mix(0, 1) == SWOFTY_ASSET_OK && calls.size() == 1 &&
               calls[0] == std::make_pair(reinterpret_cast<uintptr_t>(ecu[0]), 1),
           "an offered mix goes through the game's mode switch");
    calls.clear();
    at<int32_t>(ecu[0], cl::FUEL_ECU_MODE) = 1;
    expect(Ops::op_set_fuel_mix(0, 1) == SWOFTY_ASSET_OK && calls.empty(), "the active mix is not switched again");
    flow = false;
    expect(Ops::op_set_fuel_mix(0, 0) == SWOFTY_ASSET_LATE && calls.empty(), "off the flow thread the switch is late");
    flow = true;
    cl::Env none = make_env();
    none.mode_avail = [] { return reinterpret_cast<uintptr_t>(avail_mem); };
    Ops::bind(std::move(none));
    expect(Ops::op_set_fuel_mix(0, 1) == SWOFTY_ASSET_UNAVAILABLE, "without the game's mode switch the op is unavailable");
}

static void replay_frames()
{
    build_world();
    for (int i = 0; i < CARS; i++) {
        attach(car_mem[i], cl::FUEL_ECU, ecu[i]);
        for (int k = 0; k < 16; k++)
            put_obf(ecu[i], 0x28 + 0x10 * k, 1.0f, 0x7000u + k + i);
        at<int32_t>(ecu[i], cl::FUEL_ECU_MODE) = 1;
    }
    static int calls;
    calls = 0;
    cl::Env e = make_env();
    e.fuel_set_mode = [](uintptr_t, int) { calls++; };
    Ops::bind(std::move(e));
    for (uint32_t k = 0; k < 16; k++)
        Ops::op_layer("rfm", SWOFTY_CAR_ALL, SWOFTY_CAR_FUEL_BASE + k, SWOFTY_CAR_OP_SCALE, 1.05f, 0, nullptr);
    Ops::flush();
    Ops::apply_all();
    calls = 0;
    for (int frame = 0; frame < 3; frame++) {
        for (int c = 0; c < CARS; c++)
            Ops::replayed(reinterpret_cast<uintptr_t>(car_mem[c]) + cl::CAR_LOADER, 0.5f);
        for (int c = 0; c < CARS; c++)
            Ops::replayed(reinterpret_cast<uintptr_t>(car_mem[c]) + cl::CAR_LOADER, 0.75f);
        auto batch = Ops::take_frame();
        expect(batch.size() == size_t(CARS) && batch[0].progress == 0.75f, "a replay frame queues each car once, with its last progress");
        for (auto &p : batch)
            Ops::apply_car(p.index);
    }
    expect(calls == 0, "unchanged fuel layers are not re-run on every replay frame");
    writes = 0;
    for (int c = 0; c < CARS; c++)
        Ops::apply_car(c);
    expect(writes == 0, "a steady frame writes nothing: the game's values are the targets");
    Ops::flush();
    expect(writes == 0, "and a flush with no layer change does nothing");
    at<uint32_t>(ecu[0], 0x28) ^= 0x1;
    writes = 0;
    Ops::apply_car(0);
    expect(writes >= 1, "a game reset of one value is written back");
    int first = writes;
    Ops::apply_car(0);
    expect(writes == first, "and only once: the next frame is steady again");
    expect(calls == 2, "a game reset of a fuel entry re-runs the mix once for the car, not once per entry");
}

static void replay_hold()
{
    using W = Ops::What;
    expect(!Ops::skip_in_replay(W::replayed, false) && !Ops::resume_due(false), "live frames apply and have nothing to resume");
    expect(!Ops::skip_in_replay(W::built, true) && !Ops::resume_due(true), "a built car applies even in a replay");
    expect(Ops::skip_in_replay(W::replayed, true), "a replayed car is held while the replay shows");
    expect(Ops::skip_in_replay(W::reset, true) && Ops::skip_in_replay(W::setup, true), "a reset or setup is held too");
    expect(!Ops::resume_due(true), "nothing resumes while the replay still shows");
    expect(Ops::resume_due(false), "the first live frame re-applies every car");
    expect(!Ops::resume_due(false), "and only once");
}

static uint32_t control_modes[8];
static int control_answer = SWOFTY_ASSET_OK;

static int fake_control(uint32_t car, uint32_t *mode)
{
    if (control_answer != SWOFTY_ASSET_OK)
        return control_answer;
    if (car >= 8)
        return SWOFTY_ASSET_INVALID;
    *mode = control_modes[car];
    return SWOFTY_ASSET_OK;
}

static void player_facts_control()
{
    namespace hk = swofty_core::hooks;
    build_world();
    Ops::bind(make_env());
    hk::Facts f;
    hk::player_facts::Cache cache;
    f.session.emplace();
    f.epoch = 1;
    for (int i = 0; i < 3; i++) {
        at<uint32_t>(car_mem[i], cl::CAR_IS_AI) = 0;
        control_modes[i] = 1;
    }
    control_modes[2] = 0;

    auto built = hk::player_facts::sync<Ops>(cache, f, fake_control);
    expect(built && built->player && built->player->car == 2, "a table with every is_ai flag clear takes the car the controller calls human");
    expect(built && built->participants[0].human == false && built->participants[2].human, "a flagged car under an AI controller is not human");

    hk::player_facts::Cache fresh;
    control_answer = SWOFTY_ASSET_LATE;
    auto early = hk::player_facts::sync<Ops>(fresh, f, fake_control);
    expect(!early || !early->player, "no controller answer yet leaves the player empty");
    expect(!fresh.scanned, "and the table is scanned again on the next frame");
    control_answer = SWOFTY_ASSET_OK;
    auto later = hk::player_facts::sync<Ops>(fresh, f, fake_control);
    expect(later && later->player && later->player->car == 2, "the player appears once the controller answers");

    control_modes[2] = 1;
    control_modes[1] = 0;
    f.player = later->player;
    f.participants = later->participants;
    auto moved = hk::player_facts::sync<Ops>(fresh, f, fake_control);
    expect(moved && moved->player && moved->player->car == 1, "the player follows the controller when the human control moves");
}

static void player_facts_sync()
{
    namespace hk = swofty_core::hooks;
    build_world();
    Ops::bind(make_env());
    hk::Facts f;
    hk::player_facts::Cache cache;
    f.session.emplace();
    f.epoch = 1;
    auto apply = [&](const std::optional<hk::player_facts::Change> &c) {
        if (!c)
            return;
        f.player = c->player;
        f.participants = c->participants;
        f.epoch++;
    };

    hk::Facts menu;
    expect(!hk::player_facts::sync<Ops>(cache, menu), "no session, no cars: nothing to change");
    auto first = hk::player_facts::sync<Ops>(cache, f);
    expect(first && first->player && first->player->car == 0 && first->player->human, "the human car is the player");
    expect(first && first->participants.size() == 3 && !first->participants[1].human, "every car is a participant");
    apply(first);
    expect(!hk::player_facts::sync<Ops>(cache, f), "an unchanged table changes nothing");
    expect(!hk::player_facts::sync<Ops>(cache, f), "and stays quiet across frames");

    at<uint32_t>(car_mem[0], cl::CAR_IS_AI) = 1;
    at<uint32_t>(car_mem[2], cl::CAR_IS_AI) = 0;
    auto moved = hk::player_facts::sync<Ops>(cache, f);
    expect(moved && moved->player && moved->player->car == 2, "the player follows a changed human car");
    apply(moved);

    at<uint32_t>(car_mem[2], cl::CAR_IS_AI) = 1;
    auto spectating = hk::player_facts::sync<Ops>(cache, f);
    expect(spectating && !spectating->player && spectating->participants.size() == 3, "no human car leaves the player empty");
    apply(spectating);

    at<uint32_t>(car_mem[1], cl::CAR_IS_AI) = 0;
    f.epoch++;
    auto back = hk::player_facts::sync<Ops>(cache, f);
    expect(back && back->player && back->player->car == 1, "a human car after spectating is found on the next epoch");
    apply(back);

    f.session.reset();
    auto ended = hk::player_facts::sync<Ops>(cache, f);
    expect(ended && !ended->player && ended->participants.empty(), "the session ending clears player and participants");
    apply(ended);
    expect(!hk::player_facts::sync<Ops>(cache, f), "and stays clear");

    f.session.emplace();
    count = 0;
    expect(!hk::player_facts::sync<Ops>(cache, f), "no cars yet leaves the player empty");
    count = CARS;
}

static void car_builds()
{
    const cl::CarBuild *kb = cl::car_build_for(1789118616u, 0x193e6000u);
    expect(kb && kb->mode_avail_rva == 0x606ec28, "the KB build runs with g_Game_EngineModeAvail at 0x606ec28");
    expect(!cl::car_build_for(1789118616u, 0x5000000u), "the KB stamp on a packed image is off");
    const cl::CarBuild *steam = cl::car_build_for(1794312636u, 0x18434000u);
    expect(steam && steam->mode_avail_rva == 0x6070c28, "Steam 26566308 runs with g_Game_EngineModeAvail at 0x6070c28");
    const cl::CarBuild *ea = cl::car_build_for(1794315435u, 0x1615e000u);
    expect(ea && ea->mode_avail_rva == 0x61a7968, "EA 1.0.145 runs with g_Game_EngineModeAvail at 0x61a7968");
    expect(!cl::car_build_for(1794312636u, 0x1615e000u), "a known stamp with another image size is off");
    expect(!cl::car_build_for(1789128892u, 0x155dc000u), "the unchecked EA KB-version build is off");
    expect(!cl::car_build_for(0, 0), "an unknown build is off");
}

int main()
{
    car_builds();
    core_ops();
    sdk_side();
    pending_queue();
    engine_limiter();
    gear_ratios();
    v2_fields();
    v2_layers();
    v2_virtual();
    v2_gating();
    v2_refit();
    v3_fuel();
    replay_frames();
    replay_hold();
    player_facts_sync();
    player_facts_control();
    if (fails) {
        printf("car_live_test: %d failure(s)\n", fails);
        return 1;
    }
    printf("car_live_test: ok\n");
    return 0;
}
