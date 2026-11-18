#pragma once
#include <swofty_api.h>

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

namespace swofty_core::live {

constexpr uint32_t PKT_SESSION = 1u << 1, PKT_LAP_DATA = 1u << 2, PKT_PARTICIPANTS = 1u << 4, PKT_CAR_TELEMETRY = 1u << 6,
                   PKT_CAR_STATUS = 1u << 7;
constexpr unsigned FEED_CARS = 24;

inline bool has(const SwoftyLiveCar &c, uint64_t bits)
{
    return (c.have & bits) != 0;
}

inline uint32_t fill_feed(SwoftyTelemetryFeed &f, const SwoftyLiveSession &s, const SwoftyLiveCar *cars,
                          const bool *present)
{
    uint32_t groups = 0;
    constexpr uint64_t LAP = SWOFTY_LIVE_HAVE_PROGRESS | SWOFTY_LIVE_HAVE_CLASS | SWOFTY_LIVE_HAVE_PIT |
                             SWOFTY_LIVE_HAVE_PIT_LANE |
                             SWOFTY_LIVE_HAVE_PENALTIES | SWOFTY_LIVE_HAVE_GAP_LEADER | SWOFTY_LIVE_HAVE_GAP_AHEAD;
    constexpr uint64_t TEL = SWOFTY_LIVE_HAVE_SPEED | SWOFTY_LIVE_HAVE_GEAR | SWOFTY_LIVE_HAVE_RPM | SWOFTY_LIVE_HAVE_PEDALS |
                             SWOFTY_LIVE_HAVE_STEER | SWOFTY_LIVE_HAVE_CLUTCH | SWOFTY_LIVE_HAVE_DRS | SWOFTY_LIVE_HAVE_WORLD;
    constexpr uint64_t STATUS = SWOFTY_LIVE_HAVE_ERS_STORE | SWOFTY_LIVE_HAVE_TYRE;
    for (unsigned i = 0; i < FEED_CARS; i++) {
        if (!present[i])
            continue;
        const SwoftyLiveCar &m = cars[i];
        SwoftyCarState &c = f.cars[i];
        SwoftyParticipant &who = f.participants[i];
        c.ai_controlled = (m.flags & SWOFTY_LIVE_CAR_AI_DRIVING) ? 1 : 0;
        who.ai_controlled = c.ai_controlled;
        if (has(m, SWOFTY_LIVE_HAVE_SPEED)) {
            c.speed = m.speed;
        }
        if (has(m, SWOFTY_LIVE_HAVE_WORLD)) {
            for (int k = 0; k < 3; k++) {
                c.world_pos[k] = m.world_pos[k];
                c.world_vel[k] = m.world_vel[k];
            }
        }
        if (has(m, SWOFTY_LIVE_HAVE_RPM)) {
            c.rpm = m.rpm;
            c.max_rpm = m.rpm_limit;
        }
        if (has(m, SWOFTY_LIVE_HAVE_GEAR))
            c.gear = m.gear;
        if (has(m, SWOFTY_LIVE_HAVE_PEDALS)) {
            c.throttle = m.throttle;
            c.brake = m.brake;
        }
        if (has(m, SWOFTY_LIVE_HAVE_CLUTCH))
            c.clutch = m.clutch;
        if (has(m, SWOFTY_LIVE_HAVE_STEER))
            c.steer = m.steer;
        if (has(m, SWOFTY_LIVE_HAVE_DRS))
            c.drs = m.drs;
        if (has(m, SWOFTY_LIVE_HAVE_ERS_STORE))
            c.ers_store = m.ers_store;
        if (has(m, SWOFTY_LIVE_HAVE_TYRE)) {
            c.tyre_compound = m.tyre_actual;
            c.tyre_visual = m.tyre_visual;
        }
        uint32_t flags = c.flags;
        if (has(m, TEL))
            groups |= PKT_CAR_TELEMETRY;
        if (has(m, STATUS)) {
            groups |= PKT_CAR_STATUS;
            flags |= SWOFTY_CAR_FLAG_STATUS;
        }
        if (has(m, SWOFTY_LIVE_HAVE_PROGRESS)) {
            c.lap = uint32_t(m.lap + 1);
            c.lap_distance = m.lap_m;
            who.lap = c.lap;
            who.lap_distance = c.lap_distance;
        }
        if (has(m, SWOFTY_LIVE_HAVE_SECTOR) && m.sector >= 1)
            c.sector = uint32_t(m.sector - 1);
        if (has(m, SWOFTY_LIVE_HAVE_CLASS)) {
            c.position = int32_t(m.position);
            who.position = c.position;
            c.result_status = m.result_status;
            c.penalty_seconds = uint8_t(std::lround(std::fmin(255.f, m.time_penalty_s)));
            flags &= ~(SWOFTY_CAR_FLAG_RETIRED | SWOFTY_CAR_FLAG_DISQUALIFIED | SWOFTY_CAR_FLAG_FINISHED);
            if (m.result_status == 4 || m.result_status == 7)
                flags |= SWOFTY_CAR_FLAG_RETIRED;
            if (m.result_status == 5)
                flags |= SWOFTY_CAR_FLAG_DISQUALIFIED;
            if (m.result_status == 3)
                flags |= SWOFTY_CAR_FLAG_FINISHED;
            c.gap_leader_ms = has(m, SWOFTY_LIVE_HAVE_GAP_LEADER) ? uint32_t(m.gap_leader_ms) : 0;
            c.gap_front_ms = has(m, SWOFTY_LIVE_HAVE_GAP_AHEAD) ? uint32_t(m.gap_ahead_ms) : 0;
        }
        if (has(m, SWOFTY_LIVE_HAVE_PIT_LANE)) {
            c.pit_status = m.pit_status;
            c.pit_stops = m.pit_stops;
            if (m.pit_status == 1 || m.pit_status == 2)
                flags |= SWOFTY_CAR_FLAG_IN_PIT_LANE;
            else
                flags &= ~SWOFTY_CAR_FLAG_IN_PIT_LANE;
        } else if (has(m, SWOFTY_LIVE_HAVE_PIT)) {
            if (m.pit_state >= 2 && m.pit_state <= 15)
                flags |= SWOFTY_CAR_FLAG_IN_PIT_LANE;
            else
                flags &= ~SWOFTY_CAR_FLAG_IN_PIT_LANE;
        }
        if (has(m, SWOFTY_LIVE_HAVE_PENALTIES)) {
            c.warnings = m.warnings;
            c.corner_cut_warnings = m.track_limit_warnings;
            c.drive_through_pens = m.drive_through;
            c.stop_go_pens = m.stop_go;
        }
        if (has(m, LAP)) {
            groups |= PKT_LAP_DATA;
            flags |= SWOFTY_CAR_FLAG_LAP_DATA;
        }
        c.flags = flags;
    }
    SwoftySessionState &o = f.session;
    if (s.have & SWOFTY_LIVE_SHAVE_TYPE) {
        o.session_type = s.type;
        groups |= PKT_SESSION;
    }
    if (s.have & SWOFTY_LIVE_SHAVE_TRACK_LENGTH) {
        o.track_length = uint32_t(std::lround(s.track_length_m));
        groups |= PKT_SESSION;
    }
    if ((s.have & SWOFTY_LIVE_SHAVE_SAFETY_CAR) && s.safety_car != 0xff) {
        o.safety_car_status = s.safety_car;
        groups |= PKT_SESSION;
    }
    if (s.have & SWOFTY_LIVE_SHAVE_RED_FLAG) {
        o.red_flag = s.red_flag ? 1 : 0;
        groups |= PKT_SESSION;
    }
    if (s.have & (SWOFTY_LIVE_SHAVE_RED_FLAG | SWOFTY_LIVE_SHAVE_YELLOW))
        o.flag = o.red_flag ? SWOFTY_FLAG_RED : s.yellow_sectors ? SWOFTY_FLAG_YELLOW : SWOFTY_FLAG_NONE;
    if (s.have & SWOFTY_LIVE_SHAVE_CARS) {
        o.participant_count = s.racers > FEED_CARS ? FEED_CARS : s.racers;
        groups |= PKT_PARTICIPANTS;
    }
    if (s.player >= 0 && unsigned(s.player) < FEED_CARS)
        o.player_index = unsigned(s.player);
    o.reserved[0] = uint32_t(s.watched);
    o.reserved[1] = s.subject_generation;
    if (groups && !o.udp_format)
        o.udp_format = 2026;
    o.packets_seen |= groups;
    return groups;
}

inline void mismatch(std::vector<std::string> &out, unsigned car, const char *what, double mem, double udp)
{
    char b[96];
    std::snprintf(b, sizeof b, "car %u %s memory %.3f udp %.3f", car, what, mem, udp);
    out.emplace_back(b);
}

inline void cross_check(const SwoftyTelemetryFeed &mem, const SwoftyTelemetryFeed &udp, uint32_t packet,
                        std::vector<std::string> &out)
{
    for (unsigned i = 0; i < FEED_CARS; i++) {
        const SwoftyCarState &m = mem.cars[i], &u = udp.cars[i];
        if (packet == PKT_LAP_DATA && (m.flags & SWOFTY_CAR_FLAG_LAP_DATA) && (u.flags & SWOFTY_CAR_FLAG_LAP_DATA)) {
            if (m.position != u.position)
                mismatch(out, i, "position", m.position, u.position);
            if (m.lap != u.lap)
                mismatch(out, i, "lap", m.lap, u.lap);
            if (std::fabs(m.lap_distance - u.lap_distance) > 1.0f)
                mismatch(out, i, "lap_distance", m.lap_distance, u.lap_distance);
            if (m.result_status != u.result_status)
                mismatch(out, i, "result_status", m.result_status, u.result_status);
            if (m.warnings != u.warnings)
                mismatch(out, i, "warnings", m.warnings, u.warnings);
            if (m.drive_through_pens != u.drive_through_pens)
                mismatch(out, i, "drive_through_pens", m.drive_through_pens, u.drive_through_pens);
            if (m.stop_go_pens != u.stop_go_pens)
                mismatch(out, i, "stop_go_pens", m.stop_go_pens, u.stop_go_pens);
            if (m.pit_status != u.pit_status)
                mismatch(out, i, "pit_status", m.pit_status, u.pit_status);
            if (m.pit_stops != u.pit_stops)
                mismatch(out, i, "pit_stops", m.pit_stops, u.pit_stops);
        }
        if (packet == PKT_CAR_TELEMETRY && (u.speed != 0 || u.rpm != 0)) {
            if (std::fabs(m.speed - u.speed) > 0.5f)
                mismatch(out, i, "speed", m.speed, u.speed);
            if (std::fabs(m.rpm - u.rpm) > 5.f)
                mismatch(out, i, "rpm", m.rpm, u.rpm);
            if (m.gear != u.gear)
                mismatch(out, i, "gear", m.gear, u.gear);
        }
        if (packet == PKT_CAR_STATUS && (m.flags & SWOFTY_CAR_FLAG_STATUS) && (u.flags & SWOFTY_CAR_FLAG_STATUS)) {
            if (std::fabs(m.ers_store - u.ers_store) > 2000.f)
                mismatch(out, i, "ers_store", m.ers_store, u.ers_store);
            if (m.tyre_visual != u.tyre_visual)
                mismatch(out, i, "tyre_visual", m.tyre_visual, u.tyre_visual);
        }
    }
    if (packet == PKT_SESSION) {
        if (mem.session.safety_car_status != udp.session.safety_car_status)
            mismatch(out, 99, "safety_car_status", mem.session.safety_car_status, udp.session.safety_car_status);
        if (mem.session.session_type != udp.session.session_type)
            mismatch(out, 99, "session_type", mem.session.session_type, udp.session.session_type);
    }
}

}
