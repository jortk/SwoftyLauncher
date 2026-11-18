#pragma once
#include <swofty_api.h>

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

// Fills the telemetry feed from the game's own UDP packets (formats 2024, 2025 and 2026), decoded from the
// buffer the game hands to sendto before the stock id rewrite touches it. Offsets from the packet start; the
// header is 29 bytes (u16 format at 0, u8 packet id at 6, u8 player car index at 27). Layouts are the
// published F1 24 and F1 25 UDP specification:
//   Session (1), 753 bytes: weather 29, trackTemp i8 30, airTemp i8 31, totalLaps 32, trackLength u16 33,
//     sessionType 35, trackId i8 36, sessionTimeLeft u16 38, pitSpeedLimit 42, numMarshalZones 47, zones from 48
//     (5 bytes: f32 start, i8 flag at +4), safetyCarStatus 153.
//   Lap Data (2): entries from 29, then two u8 time trial indices. 2024: 22 x 50 bytes (deltas are u16 ms); 2025:
//     22 x 57 bytes (deltas are u16 ms + u8 minutes); 2026: 24 x 57 bytes. Entry: lastLapMs u32 0, currentLapMs
//     u32 4, sector1 ms u16 8 and minutes u8 10, sector2 ms u16 11 and minutes u8 13, delta to the car in front,
//     delta to the leader, then lapDistance f32, totalDistance f32, safetyCarDelta f32, carPosition, currentLapNum,
//     pitStatus, numPitStops, sector, currentLapInvalid, penalties, totalWarnings, cornerCuttingWarnings,
//     numUnservedDriveThroughPens, numUnservedStopGoPens, gridPosition, driverStatus, resultStatus.
//   Event (3), 45 bytes: code at 29, details at 33. OVTK, COLL and RTMT carry two u8 at 33 and 34.
//   Participants (4): numActiveCars 29, entries from 30: aiControlled 0, driverId 1, teamId 3, raceNumber 5,
//     nationality 6, name 7 (2024: 60 bytes entries, 48-byte name; 2025: 57 bytes, 32-byte name). 2026: 24 x 60
//     bytes, driverId u16 at 1 and teamId u16 at 5; the name offset is not catalogued, so the name stays empty.
//   Car Telemetry (6), 1352 bytes: 22 x 60 bytes: speed km/h u16 0, throttle f32 2, steer f32 6, brake f32 10,
//     clutch u8 14, gear i8 15, rpm u16 16, drs u8 18, brakesTemp u16[4] 22, tyresSurfaceTemp u8[4] 30.
//   Car Status (7), 1239 bytes: 22 x 55 bytes: fuelInTank f32 5, fuelRemainingLaps f32 13, maxRPM u16 17,
//     drsAllowed 22, actualTyreCompound 25, visualTyreCompound 26, tyresAgeLaps 27, vehicleFiaFlags i8 28,
//     ersStoreEnergy f32 37.
//   Car Damage (10), 1041 bytes: 22 x 46 bytes: tyresWear f32[4] at 0.
//   2026 Car Telemetry (6), 1448 bytes: 24 x 59 bytes, then three u8; engineTemperature is a u8 (u16 before), so
//     the fields read above keep their offsets. Never replayed on a recording; it rests on the published
//     f1-packets 2026.1.0 spec.
//   2026 Car Status (7), 1445 bytes: 24 x 59 bytes; the fields read above sit at offsets 5..37 of the entry and
//     the extra bytes are not catalogued.
// The 2026 Session and Car Damage packets have other sizes whose field offsets are not catalogued, so they are
// left alone.

namespace runtime::udp_tel {

constexpr size_t HEADER = 29;
constexpr size_t MAX_CARS = 24;

struct Spec {
    int format, id;
    size_t size, stride, cars;
    size_t first;
};

inline const Spec *specs(size_t &n)
{
    static const Spec table[] = {
        {2024, 1, 753, 0, 0, 0},      {2025, 1, 753, 0, 0, 0},      {2024, 2, 1131, 50, 22, HEADER},
        {2025, 2, 1285, 57, 22, HEADER}, {2026, 2, 1399, 57, 24, HEADER}, {2024, 3, 45, 0, 0, 0},
        {2025, 3, 45, 0, 0, 0},       {2026, 3, 45, 0, 0, 0},       {2024, 4, 1350, 60, 22, HEADER + 1},
        {2025, 4, 1284, 57, 22, HEADER + 1}, {2026, 4, 1470, 60, 24, HEADER + 1}, {2024, 6, 1352, 60, 22, HEADER},
        {2025, 6, 1352, 60, 22, HEADER}, {2026, 6, 1448, 59, 24, HEADER}, {2024, 7, 1239, 55, 22, HEADER},
        {2025, 7, 1239, 55, 22, HEADER}, {2026, 7, 1445, 59, 24, HEADER},
        {2024, 10, 1041, 46, 22, HEADER}, {2025, 10, 1041, 46, 22, HEADER},
    };
    n = sizeof table / sizeof *table;
    return table;
}

inline const Spec *identify(const uint8_t *p, size_t len)
{
    if (!p || len < HEADER + 1)
        return nullptr;
    int format = p[0] | p[1] << 8;
    size_t n;
    const Spec *t = specs(n);
    for (size_t i = 0; i < n; i++)
        if (t[i].format == format && t[i].id == p[6] && t[i].size == len)
            return &t[i];
    return nullptr;
}

struct Event {
    const char *name;
    int64_t a, b;
};

inline uint16_t u16(const uint8_t *p)
{
    return uint16_t(p[0] | p[1] << 8);
}

inline uint32_t u32(const uint8_t *p)
{
    return uint32_t(p[0]) | uint32_t(p[1]) << 8 | uint32_t(p[2]) << 16 | uint32_t(p[3]) << 24;
}

inline float f32(const uint8_t *p)
{
    uint32_t v = u32(p);
    float f;
    std::memcpy(&f, &v, sizeof f);
    return f;
}

inline float ms_to_s(uint32_t ms)
{
    return float(ms) / 1000.0f;
}

inline uint32_t delta_ms(const uint8_t *e, size_t at, bool minutes)
{
    return minutes ? uint32_t(u16(e + at)) + uint32_t(e[at + 2]) * 60000u : u16(e + at);
}

inline void session(const uint8_t *p, SwoftyTelemetryFeed &f)
{
    SwoftySessionState &s = f.session;
    s.weather = p[29];
    s.track_temp = float(int8_t(p[30]));
    s.air_temp = float(int8_t(p[31]));
    s.total_laps = p[32];
    s.track_length = u16(p + 33);
    s.session_type = p[35];
    s.track = uint32_t(int32_t(int8_t(p[36])));
    s.session_time_left = float(u16(p + 38));
    s.pit_speed_limit = p[42];
    s.safety_car_status = p[153];
    unsigned zones = p[47] > 21 ? 21 : p[47];
    auto rank = [](uint32_t v) { return v == SWOFTY_FLAG_YELLOW ? 3 : v == SWOFTY_FLAG_BLUE ? 2 : v == SWOFTY_FLAG_GREEN ? 1 : 0; };
    uint32_t flag = SWOFTY_FLAG_NONE;
    for (unsigned i = 0; i < zones; i++) {
        int z = int8_t(p[48 + i * 5 + 4]);
        uint32_t seen = z == 3 ? SWOFTY_FLAG_YELLOW : z == 2 ? SWOFTY_FLAG_BLUE : z == 1 ? SWOFTY_FLAG_GREEN : SWOFTY_FLAG_NONE;
        if (rank(seen) > rank(flag))
            flag = seen;
    }
    s.flag = s.red_flag ? SWOFTY_FLAG_RED : flag;
}

inline void lap_data(const uint8_t *p, const Spec &sp, SwoftyTelemetryFeed &f)
{
    bool wide = sp.stride == 57;
    for (size_t i = 0; i < sp.cars; i++) {
        const uint8_t *e = p + sp.first + i * sp.stride;
        SwoftyCarState &c = f.cars[i];
        SwoftyParticipant &who = f.participants[i];
        size_t t = wide ? 20 : 18;
        size_t at = wide ? 32 : 30;
        c.last_lap_time = ms_to_s(u32(e));
        c.current_lap_time = ms_to_s(u32(e + 4));
        c.sector_time[0] = ms_to_s(u16(e + 8) + uint32_t(e[10]) * 60000u);
        c.sector_time[1] = ms_to_s(u16(e + 11) + uint32_t(e[13]) * 60000u);
        c.gap_front_ms = delta_ms(e, 14, wide);
        c.gap_leader_ms = wide ? delta_ms(e, 17, true) : u16(e + 16);
        c.lap_distance = f32(e + t);
        c.position = e[at];
        c.lap = e[at + 1];
        c.pit_status = e[at + 2];
        c.pit_stops = e[at + 3];
        c.sector = e[at + 4];
        uint8_t invalid = e[at + 5];
        c.penalty_seconds = e[at + 6];
        c.warnings = e[at + 7];
        c.corner_cut_warnings = e[at + 8];
        c.drive_through_pens = e[at + 9];
        c.stop_go_pens = e[at + 10];
        c.grid_position = e[at + 11];
        c.driver_status = e[at + 12];
        c.result_status = e[at + 13];
        uint32_t flags = c.flags & (SWOFTY_CAR_FLAG_STATUS | SWOFTY_CAR_FLAG_DRS_ALLOWED);
        flags |= SWOFTY_CAR_FLAG_LAP_DATA;
        if (c.pit_status == 1 || c.pit_status == 2)
            flags |= SWOFTY_CAR_FLAG_IN_PIT_LANE;
        if (c.result_status == 4 || c.result_status == 7)
            flags |= SWOFTY_CAR_FLAG_RETIRED;
        if (c.result_status == 5)
            flags |= SWOFTY_CAR_FLAG_DISQUALIFIED;
        if (c.result_status == 3)
            flags |= SWOFTY_CAR_FLAG_FINISHED;
        if (c.driver_status == 0)
            flags |= SWOFTY_CAR_FLAG_IN_GARAGE;
        if (invalid)
            flags |= SWOFTY_CAR_FLAG_LAP_INVALID;
        c.flags = flags;
        who.position = c.position;
        who.lap = c.lap;
        who.lap_distance = c.lap_distance;
    }
}

inline void car_telemetry(const uint8_t *p, const Spec &sp, SwoftyTelemetryFeed &f)
{
    for (size_t i = 0; i < sp.cars; i++) {
        const uint8_t *e = p + sp.first + i * sp.stride;
        SwoftyCarState &c = f.cars[i];
        c.speed = float(u16(e)) / 3.6f;
        c.throttle = f32(e + 2);
        c.steer = f32(e + 6);
        c.brake = f32(e + 10);
        c.clutch = float(e[14]) / 100.0f;
        c.gear = int8_t(e[15]);
        c.rpm = float(u16(e + 16));
        c.drs = e[18];
        for (int w = 0; w < 4; w++) {
            c.brake_temp[w] = float(u16(e + 22 + w * 2));
            c.tyre_temp[w] = float(e[30 + w]);
        }
    }
}

inline void car_status(const uint8_t *p, const Spec &sp, SwoftyTelemetryFeed &f)
{
    for (size_t i = 0; i < sp.cars; i++) {
        const uint8_t *e = p + sp.first + i * sp.stride;
        SwoftyCarState &c = f.cars[i];
        c.fuel = f32(e + 5);
        c.fuel_laps = f32(e + 13);
        c.max_rpm = float(u16(e + 17));
        c.tyre_compound = e[25];
        c.tyre_visual = e[26];
        c.tyre_age_laps = e[27];
        c.fia_flag = int8_t(e[28]);
        c.ers_store = f32(e + 37);
        c.flags &= ~SWOFTY_CAR_FLAG_DRS_ALLOWED;
        c.flags |= SWOFTY_CAR_FLAG_STATUS | (e[22] == 1 ? SWOFTY_CAR_FLAG_DRS_ALLOWED : 0u);
    }
}

inline void car_damage(const uint8_t *p, const Spec &sp, SwoftyTelemetryFeed &f)
{
    for (size_t i = 0; i < sp.cars; i++) {
        const uint8_t *e = p + sp.first + i * sp.stride;
        for (int w = 0; w < 4; w++)
            f.cars[i].tyre_wear[w] = f32(e + w * 4);
    }
}

inline void participants(const uint8_t *p, const Spec &sp, SwoftyTelemetryFeed &f)
{
    size_t count = p[HEADER];
    if (count > sp.cars)
        count = sp.cars;
    f.session.participant_count = uint32_t(count);
    bool wide = sp.format == 2026;
    size_t name_len = sp.format == 2024 ? 48 : 32;
    for (size_t i = 0; i < count; i++) {
        const uint8_t *e = p + sp.first + i * sp.stride;
        SwoftyParticipant &who = f.participants[i];
        who.ai_controlled = e[0];
        who.driver_id = wide ? u16(e + 1) : e[1];
        who.team = wide ? u16(e + 5) : e[3];
        f.cars[i].ai_controlled = e[0];
        if (wide)
            continue;
        who.my_team = e[4];
        who.race_number = e[5];
        who.nationality = e[6];
        size_t n = 0;
        while (n < name_len && n < sizeof who.name - 1 && e[7 + n])
            n++;
        std::memcpy(who.name, e + 7, n);
        std::memset(who.name + n, 0, sizeof who.name - n);
    }
}

inline void event(const uint8_t *p, SwoftyTelemetryFeed &f, std::vector<Event> *out)
{
    const char *code = reinterpret_cast<const char *>(p + 29);
    auto is = [&](const char *c) { return !std::memcmp(code, c, 4); };
    if (is("RDFL")) {
        f.session.red_flag = 1;
        f.session.flag = SWOFTY_FLAG_RED;
        return;
    }
    if (is("GLGT")) {
        f.session.red_flag = 0;
        if (f.session.flag == SWOFTY_FLAG_RED)
            f.session.flag = SWOFTY_FLAG_NONE;
        return;
    }
    bool retire = is("RTMT");
    const char *name = is("OVTK")   ? SWOFTY_EVENT_TELEMETRY_OVERTAKE
                       : is("COLL") ? SWOFTY_EVENT_TELEMETRY_COLLISION
                       : is("RTMT") ? SWOFTY_EVENT_TELEMETRY_RETIREMENT
                                    : nullptr;
    if (!name || !out)
        return;
    if (p[33] >= MAX_CARS || (!retire && p[34] >= MAX_CARS))
        return;
    out->push_back({name, p[33], p[34]});
}

inline const Spec *decode(const uint8_t *p, size_t len, SwoftyTelemetryFeed &f, std::vector<Event> *events)
{
    const Spec *sp = identify(p, len);
    if (!sp)
        return nullptr;
    f.session.udp_format = uint32_t(sp->format);
    f.session.player_index = p[27] < MAX_CARS ? p[27] : 0;
    if (sp->id < 32)
        f.session.packets_seen |= 1u << sp->id;
    switch (sp->id) {
    case 1:
        session(p, f);
        break;
    case 2:
        lap_data(p, *sp, f);
        break;
    case 3:
        event(p, f, events);
        break;
    case 4:
        participants(p, *sp, f);
        break;
    case 6:
        car_telemetry(p, *sp, f);
        break;
    case 7:
        car_status(p, *sp, f);
        break;
    case 10:
        car_damage(p, *sp, f);
        break;
    }
    return sp;
}

}
