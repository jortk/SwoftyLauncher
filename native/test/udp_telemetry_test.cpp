#include "../src/core/udp_telemetry_logic.hpp"

#include <cstdio>
#include <cstring>
#include <vector>

namespace tel = runtime::udp_tel;

static int failures;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                         \
            failures++;                                                                                                \
        }                                                                                                              \
    } while (0)

using Bytes = std::vector<uint8_t>;

static void put16(Bytes &b, size_t at, unsigned v)
{
    b[at] = v & 255;
    b[at + 1] = v >> 8 & 255;
}

static void put32(Bytes &b, size_t at, uint32_t v)
{
    for (int i = 0; i < 4; i++)
        b[at + i] = v >> (8 * i) & 255;
}

static void putf(Bytes &b, size_t at, float f)
{
    uint32_t v;
    memcpy(&v, &f, 4);
    put32(b, at, v);
}

static Bytes packet(int format, int id, size_t size, int player = 0)
{
    Bytes b(size, 0);
    put16(b, 0, format);
    b[2] = format == 2024 ? 24 : format == 2025 ? 25 : 26;
    b[6] = id;
    b[27] = player;
    b[28] = 255;
    return b;
}

static SwoftyTelemetryFeed *fresh()
{
    static SwoftyTelemetryFeed f;
    memset(&f, 0, sizeof f);
    return &f;
}

static void lap_data_2025()
{
    Bytes p = packet(2025, 2, 1285, 3);
    size_t e = 29 + 3 * 57;
    put32(p, e, 91234);
    put32(p, e + 4, 20500);
    put16(p, e + 8, 31000);
    p[e + 10] = 0;
    put16(p, e + 11, 55000);
    p[e + 13] = 1;
    put16(p, e + 14, 1500);
    p[e + 16] = 0;
    put16(p, e + 17, 3200);
    p[e + 19] = 1;
    putf(p, e + 20, 1234.5f);
    p[e + 32] = 4;
    p[e + 33] = 12;
    p[e + 34] = 1;
    p[e + 35] = 2;
    p[e + 36] = 1;
    p[e + 37] = 1;
    p[e + 38] = 10;
    p[e + 39] = 3;
    p[e + 40] = 2;
    p[e + 41] = 1;
    p[e + 42] = 1;
    p[e + 43] = 7;
    p[e + 44] = 3;
    p[e + 45] = 2;
    SwoftyTelemetryFeed *f = fresh();
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    const SwoftyCarState &c = f->cars[3];
    CHECK(c.last_lap_time == 91.234f);
    CHECK(c.current_lap_time == 20.5f);
    CHECK(c.sector_time[0] == 31.0f && c.sector_time[1] == 115.0f);
    CHECK(c.gap_front_ms == 1500 && c.gap_leader_ms == 63200);
    CHECK(c.lap_distance == 1234.5f);
    CHECK(c.position == 4 && c.lap == 12 && c.sector == 1);
    CHECK(c.pit_status == 1 && c.pit_stops == 2);
    CHECK(c.penalty_seconds == 10 && c.warnings == 3 && c.corner_cut_warnings == 2);
    CHECK(c.drive_through_pens == 1 && c.stop_go_pens == 1 && c.grid_position == 7);
    CHECK(c.driver_status == 3 && c.result_status == 2);
    CHECK(c.flags & SWOFTY_CAR_FLAG_LAP_DATA);
    CHECK(c.flags & SWOFTY_CAR_FLAG_IN_PIT_LANE);
    CHECK(c.flags & SWOFTY_CAR_FLAG_LAP_INVALID);
    CHECK(!(c.flags & SWOFTY_CAR_FLAG_RETIRED));
    CHECK(f->participants[3].position == 4 && f->participants[3].lap == 12);
    CHECK(f->session.player_index == 3 && f->session.udp_format == 2025 && (f->session.packets_seen & 4));
    CHECK(f->cars[2].position == 0);
}

static void lap_data_2024_and_2026()
{
    Bytes p = packet(2024, 2, 1131);
    size_t e = 29 + 50;
    put16(p, e + 14, 700);
    put16(p, e + 16, 4100);
    putf(p, e + 18, 50.0f);
    p[e + 30] = 2;
    p[e + 31] = 5;
    p[e + 32] = 2;
    p[e + 33] = 1;
    p[e + 43] = 7;
    SwoftyTelemetryFeed *f = fresh();
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    const SwoftyCarState &c = f->cars[1];
    CHECK(c.gap_front_ms == 700 && c.gap_leader_ms == 4100 && c.lap_distance == 50.0f);
    CHECK(c.position == 2 && c.lap == 5 && c.pit_status == 2 && c.pit_stops == 1);
    CHECK(c.flags & SWOFTY_CAR_FLAG_RETIRED);

    Bytes q = packet(2026, 2, 1399);
    size_t g = 29 + 23 * 57;
    q[g + 32] = 20;
    q[g + 45] = 5;
    f = fresh();
    CHECK(tel::decode(q.data(), q.size(), *f, nullptr));
    CHECK(f->cars[23].position == 20);
    CHECK(f->cars[23].flags & SWOFTY_CAR_FLAG_DISQUALIFIED);
}

static void car_status_and_telemetry()
{
    Bytes p = packet(2025, 7, 1239);
    size_t e = 29 + 55 * 5;
    putf(p, e + 5, 42.5f);
    putf(p, e + 13, 18.25f);
    put16(p, e + 17, 13000);
    p[e + 22] = 1;
    p[e + 25] = 17;
    p[e + 26] = 16;
    p[e + 27] = 9;
    p[e + 28] = uint8_t(-1);
    putf(p, e + 37, 2500000.0f);
    SwoftyTelemetryFeed *f = fresh();
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    const SwoftyCarState &c = f->cars[5];
    CHECK(c.fuel == 42.5f && c.fuel_laps == 18.25f && c.max_rpm == 13000.0f);
    CHECK(c.tyre_compound == 17 && c.tyre_visual == 16 && c.tyre_age_laps == 9 && c.fia_flag == -1);
    CHECK(c.ers_store == 2500000.0f);
    CHECK(c.flags & SWOFTY_CAR_FLAG_DRS_ALLOWED && c.flags & SWOFTY_CAR_FLAG_STATUS);

    Bytes t = packet(2025, 6, 1352);
    size_t k = 29 + 60 * 5;
    put16(t, k, 360);
    putf(t, k + 2, 0.75f);
    putf(t, k + 6, -0.5f);
    putf(t, k + 10, 0.25f);
    t[k + 14] = 50;
    t[k + 15] = uint8_t(-1);
    put16(t, k + 16, 11500);
    t[k + 18] = 1;
    put16(t, k + 22 + 2, 640);
    t[k + 30 + 3] = 95;
    CHECK(tel::decode(t.data(), t.size(), *f, nullptr));
    CHECK(f->cars[5].speed == 100.0f && f->cars[5].throttle == 0.75f && f->cars[5].steer == -0.5f);
    CHECK(f->cars[5].brake == 0.25f && f->cars[5].clutch == 0.5f && f->cars[5].gear == -1);
    CHECK(f->cars[5].rpm == 11500.0f && f->cars[5].drs == 1);
    CHECK(f->cars[5].brake_temp[1] == 640.0f && f->cars[5].tyre_temp[3] == 95.0f);
    CHECK(f->cars[5].fuel == 42.5f);

    Bytes d = packet(2025, 10, 1041);
    putf(d, 29 + 46 * 5 + 8, 31.5f);
    CHECK(tel::decode(d.data(), d.size(), *f, nullptr));
    CHECK(f->cars[5].tyre_wear[2] == 31.5f);
}

// 2026 format: 24 cars of 59 bytes; values as an F1 25 2026 Season Pack session sends them (1950 car: maxRPM 7815,
// idle 2864, fuel capacity 166.5 between fuel 5 and fuel laps 13, soft compound 16 at 25 / 26).
static void car_status_and_telemetry_2026()
{
    Bytes p = packet(2026, 7, 1445, 3);
    size_t e = 29 + 59 * 23;
    putf(p, e + 5, 44.92f);
    putf(p, e + 9, 166.5f);
    putf(p, e + 13, 3.13f);
    put16(p, e + 17, 7815);
    put16(p, e + 19, 2864);
    p[e + 21] = 6;
    p[e + 25] = 16;
    p[e + 26] = 16;
    p[e + 27] = 1;
    putf(p, e + 37, 1000000.0f);
    p[e + 58] = 0;
    SwoftyTelemetryFeed *f = fresh();
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    const SwoftyCarState &c = f->cars[23];
    CHECK(c.fuel == 44.92f && c.fuel_laps == 3.13f && c.max_rpm == 7815.0f);
    CHECK(c.tyre_compound == 16 && c.tyre_visual == 16 && c.tyre_age_laps == 1 && c.ers_store == 1000000.0f);
    CHECK(c.flags & SWOFTY_CAR_FLAG_STATUS && !(c.flags & SWOFTY_CAR_FLAG_DRS_ALLOWED));
    CHECK(f->session.udp_format == 2026 && f->session.player_index == 3);

    Bytes t = packet(2026, 6, 1448);
    size_t k = 29 + 59 * 23;
    put16(t, k, 206);
    putf(t, k + 2, 1.0f);
    t[k + 15] = 4;
    put16(t, k + 16, 6900);
    put16(t, k + 22 + 2, 410);
    t[k + 30 + 1] = 88;
    t[k + 38] = 101;
    CHECK(tel::decode(t.data(), t.size(), *f, nullptr));
    CHECK(f->cars[23].throttle == 1.0f && f->cars[23].gear == 4 && f->cars[23].rpm == 6900.0f);
    CHECK(f->cars[23].brake_temp[1] == 410.0f && f->cars[23].tyre_temp[1] == 88.0f);
    CHECK(f->cars[23].rpm / f->cars[23].max_rpm > 0.88f);
    CHECK(f->session.packets_seen & (1u << 6) && f->session.packets_seen & (1u << 7));
}

static void session_packet()
{
    Bytes p = packet(2025, 1, 753);
    p[29] = 2;
    p[30] = uint8_t(-3);
    p[31] = 21;
    p[32] = 57;
    put16(p, 33, 5303);
    p[35] = 15;
    p[36] = 10;
    put16(p, 38, 3000);
    p[42] = 80;
    p[47] = 3;
    p[48 + 4] = 1;
    p[48 + 5 + 4] = 2;
    p[48 + 10 + 4] = 3;
    p[153] = 1;
    SwoftyTelemetryFeed *f = fresh();
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    const SwoftySessionState &s = f->session;
    CHECK(s.weather == 2 && s.track_temp == -3.0f && s.air_temp == 21.0f && s.total_laps == 57);
    CHECK(s.track_length == 5303 && s.session_type == 15 && s.track == 10);
    CHECK(s.session_time_left == 3000.0f && s.pit_speed_limit == 80);
    CHECK(s.safety_car_status == 1 && s.flag == SWOFTY_FLAG_YELLOW);
    p[48 + 10 + 4] = 0;
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    CHECK(f->session.flag == SWOFTY_FLAG_BLUE);
}

static void participants_packet()
{
    Bytes p = packet(2025, 4, 1284);
    p[29] = 2;
    size_t e = 30 + 57;
    p[e] = 1;
    p[e + 1] = 14;
    p[e + 3] = 6;
    p[e + 4] = 1;
    p[e + 5] = 44;
    p[e + 6] = 9;
    memcpy(&p[e + 7], "Test Driver", 11);
    SwoftyTelemetryFeed *f = fresh();
    CHECK(tel::decode(p.data(), p.size(), *f, nullptr));
    const SwoftyParticipant &who = f->participants[1];
    CHECK(!strcmp(who.name, "Test Driver") && who.team == 6 && who.driver_id == 14);
    CHECK(who.race_number == 44 && who.nationality == 9 && who.my_team == 1 && who.ai_controlled == 1);
    CHECK(f->session.participant_count == 2);
    CHECK(f->cars[1].ai_controlled == 1);

    Bytes w = packet(2026, 4, 1470);
    w[29] = 1;
    put16(w, 30 + 1, 300);
    put16(w, 30 + 5, 481);
    CHECK(tel::decode(w.data(), w.size(), *f, nullptr));
    CHECK(f->participants[0].driver_id == 300 && f->participants[0].team == 481 && f->participants[0].name[0] == 0);
}

static void event_packets()
{
    SwoftyTelemetryFeed *f = fresh();
    std::vector<tel::Event> ev;
    Bytes p = packet(2025, 3, 45);
    memcpy(&p[29], "OVTK", 4);
    p[33] = 7;
    p[34] = 3;
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(ev.size() == 1 && !strcmp(ev[0].name, SWOFTY_EVENT_TELEMETRY_OVERTAKE) && ev[0].a == 7 && ev[0].b == 3);
    memcpy(&p[29], "COLL", 4);
    p[33] = 1;
    p[34] = 2;
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(ev.size() == 2 && !strcmp(ev[1].name, SWOFTY_EVENT_TELEMETRY_COLLISION) && ev[1].a == 1 && ev[1].b == 2);
    memcpy(&p[29], "RTMT", 4);
    p[33] = 9;
    p[34] = 255;
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(ev.size() == 3 && !strcmp(ev[2].name, SWOFTY_EVENT_TELEMETRY_RETIREMENT) && ev[2].a == 9 && ev[2].b == 255);
    memcpy(&p[29], "OVTK", 4);
    p[33] = 30;
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(ev.size() == 3);
    memcpy(&p[29], "SPTP", 4);
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(ev.size() == 3);

    memcpy(&p[29], "RDFL", 4);
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(f->session.red_flag == 1 && f->session.flag == SWOFTY_FLAG_RED);
    Bytes s = packet(2025, 1, 753);
    CHECK(tel::decode(s.data(), s.size(), *f, &ev));
    CHECK(f->session.flag == SWOFTY_FLAG_RED);
    memcpy(&p[29], "GLGT", 4);
    CHECK(tel::decode(p.data(), p.size(), *f, &ev));
    CHECK(f->session.red_flag == 0 && f->session.flag == SWOFTY_FLAG_NONE);
}

static void rejects()
{
    SwoftyTelemetryFeed *f = fresh();
    Bytes p = packet(2025, 2, 1284);
    CHECK(!tel::decode(p.data(), p.size(), *f, nullptr));
    Bytes q = packet(2023, 2, 1285);
    CHECK(!tel::decode(q.data(), q.size(), *f, nullptr));
    Bytes r = packet(2026, 7, 1444);
    CHECK(!tel::decode(r.data(), r.size(), *f, nullptr));
    CHECK(!tel::decode(nullptr, 0, *f, nullptr));
    CHECK(f->session.udp_format == 0);
}

static void layout()
{
    CHECK(SWOFTY_TELEMETRY_VERSION == 2);
    CHECK(sizeof(SwoftyCarState) == 192);
    CHECK(sizeof(SwoftySessionState) == 80);
    CHECK(sizeof(SwoftyParticipant) == 92);
    CHECK(offsetof(SwoftyCarState, tyre_compound) == 160);
    CHECK(offsetof(SwoftyCarState, gap_front_ms) == 176);
    CHECK(offsetof(SwoftySessionState, safety_car_status) == 48);
}

int main()
{
    layout();
    lap_data_2025();
    lap_data_2024_and_2026();
    car_status_and_telemetry();
    car_status_and_telemetry_2026();
    session_packet();
    participants_packet();
    event_packets();
    rejects();
    if (failures)
        printf("%d failure(s)\n", failures);
    else
        printf("udp_telemetry_test: all ok\n");
    return failures ? 1 : 0;
}
