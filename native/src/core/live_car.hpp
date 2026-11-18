#pragma once

#include <cstdint>

namespace live_car {

struct LiveCar {
    bool has_speed = false, has_pedals = false, has_steer = false, has_gear = false, has_rpm = false, has_g = false,
         has_surface = false, has_wetness = false, has_wheel_wetness = false;
    float wetness = 0, wheel_wetness = 0;
    float speed_kmh = 0, throttle = 0, brake = 0, steer = 0, rpm = 0, max_rpm = 0, g_lateral = 0, g_long = 0;
    int gear = 0;
    uint8_t surface[4] = {12, 12, 12, 12};
    int player = -1;
    bool ai = false;
};

bool read(LiveCar &out);

// True once the vehicle table resolves on this build: the sampler can find the player's car at all.
bool available();

// True on the KB build only: every KB offset the sampler and its callers know is this build's.
bool kb_fields();

uintptr_t player_car();

struct LiveDynamics {
    bool has_pos = false, has_vel = false, has_angvel = false, has_orient = false, has_spin = false, has_slip = false,
         has_drs = false;
    float pos[3] = {}, vel[3] = {}, angvel[3] = {}, quat[4] = {0, 0, 0, 1};
    float spin[4] = {}, slip_angle[4] = {}, slip_ratio[4] = {};
    float drs = 0;
    int player = -1;
    bool ai = false;
};

bool read_dynamics(LiveDynamics &out);

}
