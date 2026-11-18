#include "../src/core/live/live_bus.hpp"
#include "../src/core/live/live_rows.hpp"
#include "../src/core/live/live_state.hpp"
#include "../src/core/live/live_snapshot.hpp"
#include "../src/core/live/live_telemetry_fill.hpp"
#include "../src/core/live/live_turns.hpp"
#include "../src/core/telemetry_feed.hpp"
#include "../src/core/udp_telemetry_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>

using namespace swofty_core::live;
namespace bus = swofty_core::live::bus;
namespace L = swofty_core::live::layout;
namespace udp = runtime::udp_tel;

static int failures;
#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            failures++;                                                                                                \
        }                                                                                                              \
    } while (0)

constexpr uintptr_t BASE = 0x1000000;
constexpr uintptr_t VT = 0x1500, VT_CAR = 0x1510, VT_CTRL = 0x1520, VT_HUMAN = 0x1530;
constexpr uintptr_t KEY_BASE = 0x1800;

struct Arena {
    std::vector<uint8_t> mem = std::vector<uint8_t>(1 << 20, 0);
    size_t top = 0x100;
    uintptr_t alloc(size_t n)
    {
        n = (n + 15) & ~size_t(15);
        uintptr_t a = BASE + top;
        top += n;
        return a;
    }
    template <class T> void put(uintptr_t a, T v) { std::memcpy(&mem[a - BASE], &v, sizeof v); }
    template <class T> T get(uintptr_t a) { T v; std::memcpy(&v, &mem[a - BASE], sizeof v); return v; }
    bool read(uintptr_t a, void *dst, size_t n)
    {
        if (a < BASE || a + n > BASE + mem.size())
            return false;
        std::memcpy(dst, &mem[a - BASE], n);
        return true;
    }
};

static bool arena_read(void *ctx, uintptr_t a, void *dst, size_t n)
{
    return static_cast<Arena *>(ctx)->read(a, dst, n);
}

static uint32_t bits(float f)
{
    uint32_t u;
    std::memcpy(&u, &f, 4);
    return u;
}

struct World {
    Arena a;
    Sites s;
    uintptr_t cars[4] = {}, results = 0, results_entries[3] = {}, progress_entries[3] = {}, timing = 0, pit_sys = 0,
              pit_rec[4] = {}, sc = 0, red = 0, marshal = 0, mode_ctx = 0, spect = 0, ctrl_sets[4] = {}, ctrl_sys = 0,
              engines[4] = {}, bodies[4] = {}, states[4] = {}, chassis[4] = {}, steer[4] = {}, gearbox[4] = {}, ers[4] = {},
              tyre_sys = 0, penalty = 0, s1 = 0, mode_obj = 0, human[4] = {};
    int n_cars = 4;

    explicit World(uintptr_t progress_at = L::progress::from_manager)
    {
        s.progress_from_manager = progress_at;
        s.image_lo = 0x1000;
        s.image_hi = 0x2000;
        uintptr_t table = a.alloc(8 * 4), count = a.alloc(8), mgr_g = a.alloc(8), mgr = a.alloc(0x400);
        s.vehicle_table = table;
        s.vehicle_count = count;
        s.game_manager = mgr_g;
        s.vt_car_systems = VT_CAR;
        a.put<uintptr_t>(mgr_g, mgr);
        a.put<uint8_t>(count, 4);
        s.track_length = a.alloc(8);
        a.put<float>(s.track_length, 5000.f);
        s.session_type = a.alloc(8);
        a.put<uint32_t>(s.session_type, 15);

        uintptr_t reg = a.alloc(0x40);
        a.put<uintptr_t>(mgr + L::registry::from_manager, reg);
        const int N = 14;
        uintptr_t slots = a.alloc(16 * N), info = a.alloc(N);
        a.put<uintptr_t>(reg + L::registry::slots, slots);
        a.put<uintptr_t>(reg + L::registry::info, info);
        int next = 0;
        auto reg_add = [&](uintptr_t key, uintptr_t value) {
            a.put<uintptr_t>(slots + 16 * next, key);
            a.put<uintptr_t>(slots + 16 * next + 8, value);
            a.put<uint8_t>(info + next, 1);
            next++;
        };
        auto key = [&](int i) { return KEY_BASE + 16 * i; };
        s.key_results = key(0);
        s.key_spectator = key(1);
        s.key_timing = key(2);
        s.key_pit = key(3);
        s.key_safety_car = key(4);
        s.key_penalty_log = key(5);
        s.key_session_component = key(6);
        s.key_tyres = key(7);
        s.key_marshal = key(8);
        s.key_red_flag = key(9);
        s.key_game_mode = key(10);
        s.key_team_tables = key(11);

        for (int i = 0; i < 4; i++) {
            uintptr_t c = cars[i] = a.alloc(0x700);
            a.put<uintptr_t>(table + 8 * i, c);
            a.put<uintptr_t>(c + L::car::lock, c + L::car::lock_cs);
            a.put<uintptr_t>(c + L::car::systems_vt, VT_CAR);
            a.put<uint8_t>(c + L::car::vehicle_id, uint8_t(i));
            a.put<uint32_t>(c + L::car::is_ai, i == 0 ? 0 : 1);
            a.put<float>(c + L::car::throttle, 0.8f);
            a.put<float>(c + L::car::brake, 0.0f);
            a.put<float>(c + L::car::clutch, 0.0f);
            auto sys = [&](uintptr_t off, size_t size) {
                uintptr_t o = a.alloc(size);
                a.put<uintptr_t>(o, VT);
                a.put<uintptr_t>(o + 8, c);
                a.put<uintptr_t>(c + off, o);
                return o;
            };
            engines[i] = sys(L::car::engine, 0x2600);
            gearbox[i] = sys(L::car::gearbox, 0x800);
            chassis[i] = sys(L::car::chassis, 0x400);
            steer[i] = sys(L::car::steering, 0x200);
            if (i < 3)
                ers[i] = sys(L::car::ers, 0x1b00);
            a.put<float>(engines[i] + L::engine::rate, 1000.f);
            a.put<float>(engines[i] + L::engine::limiter, 1500.f);
            a.put<uint32_t>(gearbox[i] + L::gearbox::gear_index, 4);
            bodies[i] = a.alloc(0x40);
            states[i] = a.alloc(0x400);
            a.put<uintptr_t>(chassis[i] + L::chassis::body, bodies[i]);
            a.put<uintptr_t>(bodies[i] + L::chassis::body_state, states[i]);
            float vel[3] = {30.f, 0.f, 40.f}, pos[3] = {10.f, 2.f, 30.f};
            std::memcpy(&a.mem[states[i] + L::chassis::state_velocity - BASE], vel, sizeof vel);
            std::memcpy(&a.mem[states[i] + L::chassis::state_position - BASE], pos, sizeof pos);
            a.put<float>(chassis[i] + L::chassis::g_lat, 2.5f);
            a.put<float>(chassis[i] + L::chassis::g_long, -1.0f);
            const uint32_t K = 0x5a5a5a5a;
            a.put<uint32_t>(steer[i] + L::steering::angle, bits(0.2f) ^ K);
            a.put<uint32_t>(steer[i] + L::steering::angle + 8, K);
            a.put<uint32_t>(steer[i] + L::steering::lock, bits(0.5f) ^ K);
            a.put<uint32_t>(steer[i] + L::steering::lock + 8, K);
            if (i < 3) {
                a.put<float>(ers[i] + L::ers::energy, 2.0e6f + 1.0e5f * i);
                a.put<float>(ers[i] + L::ers::capacity, 4.0e6f);
                a.put<float>(ers[i] + L::ers::max_deploy, 350e3f);
                a.put<int32_t>(ers[i] + L::ers::mode, 2);
                a.put<float>(ers[i] + L::ers::harvest_k, 1.0e5f);
                a.put<float>(ers[i] + L::ers::deploy, 2.0e5f);
                a.put<float>(ers[i] + L::ers::lap_limit, 8.0e6f);
            }
        }
        a.put<uint32_t>(gearbox[2] + L::gearbox::gear_index, 0);
        a.put<float>(cars[2] + L::car::throttle, 0.1f);
        a.put<float>(cars[2] + L::car::brake, 0.9f);
        a.put<uint8_t>(cars[2] + L::car::input_flags, 0x08);

        uintptr_t ps = a.alloc(0x100);
        a.put<uintptr_t>(mgr + progress_at, ps);
        uintptr_t pvec = a.alloc(8 * 3);
        a.put<uintptr_t>(ps + L::progress::vec_begin, pvec);
        a.put<uintptr_t>(ps + L::progress::vec_end, pvec + 24);
        const float totals[3] = {3000.f, 5200.f, 2900.f};
        const int laps[3] = {0, 1, 0}, plaps[3] = {0, 1, 0};
        for (int i = 0; i < 3; i++) {
            uintptr_t e = progress_entries[i] = a.alloc(0xb0);
            a.put<uintptr_t>(pvec + 8 * i, e);
            a.put<uint16_t>(e + L::progress::lap_a, uint16_t(laps[i] ^ 0x1234));
            a.put<uint16_t>(e + L::progress::lap_b, 0x1234);
            a.put<uint16_t>(e + L::progress::plap_a, uint16_t(plaps[i] ^ 0x4321));
            a.put<uint16_t>(e + L::progress::plap_b, 0x4321);
            a.put<uint32_t>(e + L::progress::total_a, bits(totals[i]) ^ 0xdeadbeef);
            a.put<uint32_t>(e + L::progress::total_b, 0xdeadbeef);
        }

        results = a.alloc(0x200);
        reg_add(s.key_results, results);
        uintptr_t rvec = a.alloc(8 * 3);
        a.put<uintptr_t>(results + L::results::vec_begin, rvec);
        a.put<uintptr_t>(results + L::results::vec_end, rvec + 24);
        const int pos[3] = {2, 1, 3};
        for (int i = 0; i < 3; i++) {
            uintptr_t e = results_entries[i] = a.alloc(0x60);
            a.put<uintptr_t>(rvec + 8 * i, e);
            set_position(i, pos[i]);
            a.put<uint8_t>(e + L::results::vehicle, uint8_t(i));
            a.put<uint32_t>(e + L::results::status, 2);
        }
        a.put<uint64_t>(results_entries[2] + L::results::penalty_a, 0);

        timing = a.alloc(0x400);
        reg_add(s.key_timing, timing);
        uintptr_t tv = a.alloc(L::timing::stride * 3);
        a.put<uintptr_t>(timing + L::timing::vec_begin, tv);
        a.put<uintptr_t>(timing + L::timing::vec_begin + 8, tv + L::timing::stride * 3);
        const int64_t leader[3] = {1200, 0, 5600}, ahead[3] = {1200, 0, 4400};
        for (int i = 0; i < 3; i++) {
            uintptr_t e = tv + L::timing::stride * i;
            a.put<int64_t>(e + L::timing::leader_ms, leader[i]);
            a.put<uint8_t>(e + L::timing::leader_valid, i == 1 ? 0 : 1);
            a.put<int64_t>(e + L::timing::ahead_ms, ahead[i]);
            a.put<uint8_t>(e + L::timing::ahead_valid, i == 1 ? 0 : 1);
            a.put<uint8_t>(e + L::timing::vehicle, uint8_t(i));
        }

        pit_sys = a.alloc(0x200);
        reg_add(s.key_pit, pit_sys);
        uintptr_t lane = a.alloc(L::pit::lane_flags + 16 * 27 + 16);
        a.put<uintptr_t>(pit_sys + L::pit::lane_object, lane);
        for (int i = 0; i < 3; i++) {
            pit_rec[i] = a.alloc(0x40);
            a.put<uintptr_t>(pit_sys + L::pit::records + 16 * i, pit_rec[i]);
            a.put<uint32_t>(pit_rec[i] + L::pit::state, 0);
            uintptr_t flags = a.alloc(0x200);
            a.put<uintptr_t>(lane + L::pit::lane_flags + L::pit::lane_flag_stride * i, flags);
        }

        sc = a.alloc(0x200);
        reg_add(s.key_safety_car, sc);
        a.put<uint8_t>(sc + L::safety_car::state, 1);
        red = a.alloc(0x100);
        reg_add(s.key_red_flag, red);
        a.put<uint8_t>(red + L::red_flag::state, 0);

        s.sector_block = a.alloc(L::track::sector_block);
        a.put<float>(s.sector_block + 0, 1500.f);
        a.put<float>(s.sector_block + 4, 3200.f);
        a.put<float>(s.sector_block + 8, 5000.f);
        a.put<float>(s.sector_block + L::track::sector_length, 5000.f);
        a.put<uint32_t>(s.sector_block + L::track::sector_count, 3);
        s.track_length = s.sector_block + L::track::sector_length;
        s.sector_count = s.sector_block + L::track::sector_count;

        marshal = a.alloc(0x200);
        reg_add(s.key_marshal, marshal);
        uintptr_t zv = a.alloc(L::marshal::stride * 2);
        a.put<uintptr_t>(marshal + L::marshal::vec_begin, zv);
        a.put<uintptr_t>(marshal + L::marshal::vec_begin + 8, zv + L::marshal::stride * 2);
        for (int i = 0; i < 2; i++) {
            uintptr_t z = a.alloc(0x40), def = a.alloc(0x40);
            a.put<uintptr_t>(zv + L::marshal::stride * i + L::marshal::zone, z);
            a.put<uintptr_t>(z + L::marshal::zone_def, def);
            a.put<int32_t>(z + L::marshal::zone_flag, i == 1 ? 3 : 0);
            a.put<float>(def + L::marshal::def_start, 0.5f);
        }

        mode_ctx = a.alloc(0x200);
        reg_add(s.key_game_mode, mode_ctx);
        mode_obj = a.alloc(0x400);
        a.put<uintptr_t>(mode_ctx + L::mode::from_context, mode_obj);
        uintptr_t label = a.alloc(32);
        std::memcpy(&a.mem[label - BASE], "grand_prix_23", 14);
        a.put<uintptr_t>(mode_obj + L::mode::label, label);

        spect = a.alloc(0x100);
        reg_add(s.key_spectator, spect);

        s.controllers = a.alloc(8);
        ctrl_sys = a.alloc(0x400);
        a.put<uintptr_t>(s.controllers, ctrl_sys);
        a.put<uintptr_t>(ctrl_sys, VT_CTRL);
        s.vt_controllers = VT_CTRL;
        s.vt_human = VT_HUMAN;
        a.put<uint8_t>(ctrl_sys + L::controllers::built, 1);
        uintptr_t cv = a.alloc(0x40);
        a.put<uintptr_t>(ctrl_sys + L::controllers::count_vec, cv);
        a.put<uint32_t>(cv + L::controllers::count_lo, 0);
        a.put<uint32_t>(cv + L::controllers::count_hi, 4 * 8);
        uintptr_t sets = a.alloc(8 * 4);
        a.put<uintptr_t>(ctrl_sys + L::controllers::sets, sets);
        for (int i = 0; i < 4; i++) {
            ctrl_sets[i] = a.alloc(0x40);
            a.put<uintptr_t>(sets + 8 * i, ctrl_sets[i]);
            a.put<uint8_t>(ctrl_sets[i] + L::controllers::set_vehicle, uint8_t(i));
            if (i == 1) {
                human[i] = a.alloc(0x20);
                a.put<uintptr_t>(human[i], VT_HUMAN);
                a.put<uintptr_t>(ctrl_sets[i] + L::controllers::set_human, human[i]);
            }
        }
    }

    void set_position(int i, int pos)
    {
        a.put<uint32_t>(results_entries[i] + L::results::pos_a, uint32_t(pos - 1) ^ 0x77);
        a.put<uint32_t>(results_entries[i] + L::results::pos_b, 0x77);
    }

    Mem mem() { return Mem{arena_read, &a}; }
};

static void test_order_gaps_cars()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    const SwoftyLiveSession &s = live.ses;
    CHECK(live.active());
    CHECK(s.cars == 4 && s.racers == 3);
    CHECK((s.have & SWOFTY_LIVE_SHAVE_ORDER) && s.classified == 3);
    CHECK(s.order[0] == 1 && s.order[1] == 0 && s.order[2] == 2);
    CHECK(s.on_track == 3 && s.road_order[0] == 1 && s.road_order[1] == 0 && s.road_order[2] == 2);
    CHECK(s.player == 1 && s.watched == -1 && s.subject == 1);
    CHECK(s.type == 15 && s.track_length_m == 5000.f);
    CHECK(s.game_mode == SWOFTY_LIVE_MODE_GRAND_PRIX);
    CHECK(s.sector_count == 3 && s.yellow_sectors == (1u << 1));
    CHECK(s.safety_car == 1 && s.red_flag == 0 && !s.chequered);

    const SwoftyLiveCar &c0 = live.cars[0], &c1 = live.cars[1], &c2 = live.cars[2], &c3 = live.cars[3];
    CHECK(c0.position == 2 && c1.position == 1 && c2.position == 3 && c3.position == 0);
    CHECK(!(c3.have & SWOFTY_LIVE_HAVE_CLASS) && !(c3.flags & SWOFTY_LIVE_CAR_RACER) && c3.ers_class == 0);
    CHECK(c0.flags & SWOFTY_LIVE_CAR_RACER);
    CHECK(c1.flags & SWOFTY_LIVE_CAR_PLAYER);
    CHECK(c0.gap_leader_ms == 1200 && (c0.have & SWOFTY_LIVE_HAVE_GAP_LEADER));
    CHECK(c1.gap_leader_ms == SWOFTY_LIVE_NO_GAP && !(c1.have & SWOFTY_LIVE_HAVE_GAP_LEADER));
    CHECK(c2.gap_ahead_ms == 4400 && c2.gap_leader_ms == 5600);
    CHECK(c0.lap == 0 && c0.lap_m == 3000.f && c1.lap == 1 && c1.lap_m == 200.f && c1.total_m == 5200.f);
    CHECK(c0.sector == 2 && c1.sector == 1);
    CHECK(std::fabs(c0.speed - 50.f) < 1e-3f && (c0.have & SWOFTY_LIVE_HAVE_SPEED));
    CHECK(c0.gear == 3 && std::fabs(c0.rpm - 1000.f * 9.5492966f) < 1.f && std::fabs(c0.rpm_limit - 1500.f * 9.5492966f) < 1.f);
    CHECK(std::fabs(c0.steer + 0.4f) < 1e-4f);
    CHECK(c0.g_lat == 2.5f && c0.g_long == -1.0f);
    CHECK(c0.throttle == 0.8f && c0.brake == 0.0f);
    CHECK(c2.gear == -1 && std::fabs(c2.throttle - 0.9f) < 1e-6f && std::fabs(c2.brake - 0.1f) < 1e-6f);
    CHECK(c0.ers_store == 2.0e6f && c0.ers_capacity == 4.0e6f && c0.ers_mode == 2 && c0.ers_max_deploy == 350e3f);
    CHECK(c3.ers_store == 0 && !(c3.have & SWOFTY_LIVE_HAVE_ERS_STORE));
    CHECK(c0.world_pos[0] == 10.f && c0.world_vel[2] == 40.f);
    CHECK(c0.pit_state == 0 && (c0.have & SWOFTY_LIVE_HAVE_PIT));
}

static void test_inert_and_gates()
{
    World w;
    Sites none;
    Live off(w.mem(), none, Gates::all());
    off.update(0.0);
    CHECK(!off.active() && off.ses.have == 0 && !off.present[0]);

    Sites no_results = w.s;
    no_results.key_results = 0;
    Live part(w.mem(), no_results, Gates::all());
    part.update(0.0);
    CHECK(part.active() && !(part.ses.have & SWOFTY_LIVE_SHAVE_ORDER) && part.cars[0].position == 0 &&
          !(part.cars[0].have & SWOFTY_LIVE_HAVE_CLASS));
    CHECK(part.cars[0].have & SWOFTY_LIVE_HAVE_SPEED);

    Gates none_on;
    Live gated(w.mem(), w.s, none_on);
    gated.update(0.0);
    CHECK(gated.active() && gated.cars[0].have == SWOFTY_LIVE_HAVE_IDENT && !(gated.ses.have & SWOFTY_LIVE_SHAVE_ORDER));

    w.a.put<uint32_t>(w.cars[1] + L::car::vehicle_id, 7);
    Live bad(w.mem(), w.s, Gates::all());
    bad.update(0.0);
    CHECK(!bad.present[1] && bad.present[0]);

    World torn;
    torn.set_position(0, 1);
    Live dup(torn.mem(), torn.s, Gates::all());
    dup.update(0.0);
    CHECK(!(dup.ses.have & SWOFTY_LIVE_SHAVE_ORDER) && dup.cars[0].position == 0);
}

static void test_player_watched()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    uint32_t gen = live.ses.generation, sub = live.ses.subject_generation;
    CHECK(live.ses.player == 1);
    w.a.put<uint8_t>(w.spect + L::spectate::ui, 1);
    w.a.put<uint8_t>(w.spect + L::spectate::active, 1);
    w.a.put<uint8_t>(w.spect + L::spectate::vehicle, 2);
    live.update(0.05);
    CHECK(live.ses.watched == 2 && live.ses.subject == 2 && live.ses.player == 1);
    CHECK(live.ses.generation == gen && live.ses.subject_generation == sub + 1);
    CHECK(live.cars[2].flags & SWOFTY_LIVE_CAR_WATCHED);
    w.a.put<uint8_t>(w.spect + L::spectate::ui, 0);
    live.update(0.1);
    CHECK(live.ses.watched == -1 && live.ses.subject == 1);

    w.a.put<uintptr_t>(w.ctrl_sets[1] + L::controllers::set_human, 0);
    live.update(0.5);
    CHECK(live.ses.player == 1);

    World all_ai;
    for (int i = 0; i < 4; i++)
        all_ai.a.put<uint32_t>(all_ai.cars[i] + L::car::is_ai, 1);
    Live by_controller(all_ai.mem(), all_ai.s, Gates::all());
    by_controller.update(0.0);
    CHECK(by_controller.ses.player == 1 && !(by_controller.cars[1].flags & SWOFTY_LIVE_CAR_AI_DRIVING));
    Gates no_control = Gates::all();
    no_control.on[G_CONTROL] = false;
    Live blind(all_ai.mem(), all_ai.s, no_control);
    blind.update(0.0);
    CHECK(blind.ses.player == -1);
}

static void test_events()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    CHECK(live.take_events().empty());
    w.set_position(0, 1);
    w.set_position(1, 2);
    live.update(0.2);
    auto ev = live.take_events();
    CHECK(ev.size() == 1 && ev[0].kind == Event::overtake && ev[0].a == 0 && ev[0].b == 1);

    w.a.put<uint32_t>(w.results_entries[2] + L::results::status, 4);
    live.update(0.4);
    ev = live.take_events();
    CHECK(ev.size() == 1 && ev[0].kind == Event::retirement && ev[0].a == 2 && ev[0].b == 0);
    live.update(0.6);
    CHECK(live.take_events().empty());

    World pits;
    Live pit(pits.mem(), pits.s, Gates::all());
    pit.update(0.0);
    pits.a.put<uint32_t>(pits.pit_rec[1] + L::pit::state, 5);
    pits.set_position(0, 1);
    pits.set_position(1, 2);
    pit.update(0.2);
    CHECK(pit.take_events().empty());

    World quali;
    quali.a.put<uint32_t>(quali.s.session_type, 6);
    Live q(quali.mem(), quali.s, Gates::all());
    q.update(0.0);
    quali.set_position(0, 1);
    quali.set_position(1, 2);
    q.update(0.2);
    CHECK(q.take_events().empty());
}

static void test_derive_pure()
{
    Observed prev[MAX_CARS], now[MAX_CARS];
    for (int i = 0; i < 3; i++) {
        prev[i] = now[i] = {true, uint8_t(i + 1), 2, 0, 0.f};
    }
    std::swap(now[1].position, now[2].position);
    std::vector<Event> out;
    derive_events(prev, now, 16, 0, out);
    CHECK(out.size() == 1 && out[0].kind == Event::overtake && out[0].a == 2 && out[0].b == 1);
    out.clear();
    derive_events(prev, now, 16, 3, out);
    CHECK(out.empty());
    now[2].penalty = 5.f;
    derive_events(prev, now, 16, 0, out);
    CHECK(out.empty());
    prev[0].status = 2;
    now[0].status = 7;
    derive_events(prev, now, 16, 0, out);
    CHECK(out.size() == 1 && out[0].kind == Event::retirement && out[0].a == 0);
}

static void test_turns()
{
    int i = turns::official_for("Silverstone_Grand_Prix", 5890.f);
    CHECK(i >= 0 && turns::kTracks[i].track == 7);
    CHECK(turns::official_for("silverstone_reverse", 5890.f) == -1);
    CHECK(turns::official_for("Silverstone", 9000.f) == -1);
    CHECK(turns::official_for("abu_dhabi", 5281.f) >= 0);
    CHECK(turns::official_for("", 3337.f) >= 0 && turns::kTracks[turns::official_for("", 3337.f)].track == 5);
    CHECK(turns::official_for("", 5793.f) == -1);
    CHECK(turns::official_for("unknown_modded", 12345.f) == -1);

    turns::Table t;
    t.reset("silverstone", 5891.f);
    CHECK(t.count() == 18 && t.track == 7);
    const turns::Corner &c0 = turns::kCorners7[0];
    SwoftyLiveTurn out;
    float apex = c0.frac * 5891.f;
    CHECK(t.at(apex - 100.f, out) && out.number == c0.number && out.right == (c0.left ? 0 : 1));
    CHECK(std::fabs(out.distance_ahead_m - 100.f) < 0.01f && out.source == SWOFTY_LIVE_TURN_OFFICIAL);
    CHECK(t.at(apex + 10.f, out) && out.number == c0.number && out.distance_ahead_m < 0);
    CHECK(!t.at(apex + 400.f, out) || out.number != c0.number);
    CHECK(!t.at(-5.f + 5891.f + 0.f, out) || out.number > 0);
    turns::Table none;
    none.reset("nowhere", 1234.f);
    CHECK(none.count() == 0 && !none.at(100.f, out));
    SwoftyLiveTurn all[32];
    for (int k = 0; k < t.count(); k++)
        t.fill(k, all[k], 0.f);
    CHECK(all[17].number == turns::kCorners7[17].number);
    char label[8];
    std::snprintf(label, sizeof label, "%s", all[0].label);
    CHECK(std::strlen(label) >= 1);
}

static void test_helpers()
{
    float ends[3] = {1500.f, 3200.f, 5000.f};
    CHECK(sector_of(ends, 3, 0.f) == 1 && sector_of(ends, 3, 1500.f) == 1 && sector_of(ends, 3, 1500.1f) == 2 &&
          sector_of(ends, 3, 5000.f) == 3 && sector_of(ends, 3, 5001.f) == 0 && sector_of(ends, 0, 1.f) == 0);
    CHECK(checked_tyre_letter(16, 3) == 'S' && checked_tyre_letter(17, 4) == 'M' && checked_tyre_letter(18, 5) == 'H' &&
          checked_tyre_letter(7, 7) == 'I' && checked_tyre_letter(8, 8) == 'W');
    CHECK(checked_tyre_letter(16, 4) == 0 && checked_tyre_letter(99, 3) == 0);
}

static void test_snapshot()
{
    Snapshot s, out;
    snap_begin(&s);
    s.frame = 7;
    s.valid = true;
    snap_end(&s);
    CHECK(snap_read(&s, &out) && out.frame == 7 && out.valid);
    snap_begin(&s);
    CHECK(!snap_read(&s, &out, 4));
    snap_end(&s);
}

using Bytes = std::vector<uint8_t>;

static void put32(Bytes &b, size_t at, uint32_t v)
{
    for (int i = 0; i < 4; i++)
        b[at + i] = v >> (8 * i) & 255;
}

static void putf(Bytes &b, size_t at, float f)
{
    uint32_t v;
    std::memcpy(&v, &f, 4);
    put32(b, at, v);
}

static Bytes udp_packet(int id, size_t size)
{
    Bytes b(size, 0);
    b[0] = 2025 & 255;
    b[1] = 2025 >> 8;
    b[2] = 25;
    b[6] = uint8_t(id);
    b[28] = 255;
    return b;
}

static void test_telemetry_fill()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    SwoftyTelemetryFeed mem;
    swofty::tel::feed_init(&mem);
    uint32_t groups = fill_feed(mem, live.ses, live.cars, live.present);
    CHECK((groups & PKT_LAP_DATA) && (groups & PKT_CAR_TELEMETRY) && (groups & PKT_CAR_STATUS) && (groups & PKT_SESSION));
    const SwoftyCarState &c0 = mem.cars[0], &c1 = mem.cars[1], &c2 = mem.cars[2];
    CHECK(c0.position == 2 && c1.position == 1 && c0.lap == 1 && c1.lap == 2);
    CHECK(c0.sector == 1 && c1.sector == 0);
    CHECK(c0.gap_leader_ms == 1200 && c1.gap_leader_ms == 0 && c2.gap_front_ms == 4400);
    CHECK(c0.flags & SWOFTY_CAR_FLAG_LAP_DATA && c0.flags & SWOFTY_CAR_FLAG_STATUS && !(c0.flags & SWOFTY_CAR_FLAG_RETIRED));
    CHECK(std::fabs(c0.speed - 50.f) < 1e-3f && c0.gear == 3 && c2.gear == -1);
    CHECK(c0.ers_store == 2.0e6f);
    CHECK(mem.session.session_type == 15 && mem.session.track_length == 5000 && mem.session.safety_car_status == 1);
    CHECK(mem.session.player_index == 1 && mem.session.participant_count == 3);
    CHECK(mem.session.udp_format == 2026 && (mem.session.packets_seen & PKT_LAP_DATA));
    CHECK(mem.session.reserved[0] == uint32_t(-1));
    CHECK(mem.session.flag == SWOFTY_FLAG_YELLOW);
    CHECK(mem.cars[3].flags == 0 && mem.cars[4].flags == 0);

    Bytes lap = udp_packet(2, 1285);
    const size_t first = 29, stride = 57, at = 32;
    const int pos[3] = {2, 1, 3}, lapn[3] = {1, 2, 1};
    const float dist[3] = {3000.f, 200.f, 2900.f};
    for (int i = 0; i < 3; i++) {
        size_t e = first + i * stride;
        putf(lap, e + 20, dist[i]);
        lap[e + at] = uint8_t(pos[i]);
        lap[e + at + 1] = uint8_t(lapn[i]);
        lap[e + at + 13] = 2;
    }
    SwoftyTelemetryFeed udp_feed;
    swofty::tel::feed_init(&udp_feed);
    auto *sp = udp::decode(lap.data(), lap.size(), udp_feed, nullptr);
    CHECK(sp && sp->id == 2);
    std::vector<std::string> bad;
    cross_check(mem, udp_feed, 1u << 2, bad);
    CHECK(bad.empty());
    lap[first + at] = 3;
    swofty::tel::feed_init(&udp_feed);
    udp::decode(lap.data(), lap.size(), udp_feed, nullptr);
    cross_check(mem, udp_feed, 1u << 2, bad);
    CHECK(bad.size() == 1 && bad[0].find("position") != std::string::npos);

    Bytes tel = udp_packet(6, 1352);
    for (int i = 0; i < 3; i++) {
        size_t e = 29 + i * 60;
        tel[e] = 180;
        tel[e + 1] = 0;
        tel[e + 15] = uint8_t(i == 2 ? int8_t(-1) : int8_t(3));
        unsigned rpm = unsigned(1000.f * 9.5492966f);
        tel[e + 16] = rpm & 255;
        tel[e + 17] = rpm >> 8;
    }
    swofty::tel::feed_init(&udp_feed);
    udp::decode(tel.data(), tel.size(), udp_feed, nullptr);
    bad.clear();
    cross_check(mem, udp_feed, 1u << 6, bad);
    CHECK(bad.empty());
    tel[29 + 15] = 5;
    swofty::tel::feed_init(&udp_feed);
    udp::decode(tel.data(), tel.size(), udp_feed, nullptr);
    cross_check(mem, udp_feed, 1u << 6, bad);
    CHECK(bad.size() == 1 && bad[0].find("gear") != std::string::npos);

    SwoftyTelemetryFeed merged;
    swofty::tel::feed_init(&merged);
    udp::decode(lap.data(), lap.size(), merged, nullptr);
    fill_feed(merged, live.ses, live.cars, live.present);
    CHECK(merged.cars[0].position == 2);
}

static void test_badge()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    Arena &a = w.a;
    uintptr_t mode = a.get<uintptr_t>(w.mode_ctx + L::badge::mode), game = a.alloc(0x1200), ents = a.alloc(L::badge::entrant_stride * 2),
              tt = a.alloc(0x400), table = a.alloc(0x40), vec = a.alloc(32), rec = a.alloc(0x600), key = a.alloc(32);
    a.put<uintptr_t>(mode + L::badge::game, game);
    a.put<uintptr_t>(game + L::badge::entrants, ents);
    a.put<uintptr_t>(game + L::badge::entrants + 8, ents + L::badge::entrant_stride * 2);
    a.put<uint8_t>(ents + L::badge::entrant_vid, 9);
    a.put<uint8_t>(ents + L::badge::entrant_stride + L::badge::entrant_vid, 1);
    a.put<uint32_t>(ents + L::badge::entrant_stride + L::badge::entrant_team, (3u << 16) | 0);
    uintptr_t reg = a.get<uintptr_t>(a.get<uintptr_t>(w.s.game_manager) + L::registry::from_manager);
    uintptr_t slots = a.get<uintptr_t>(reg + L::registry::slots), info = a.get<uintptr_t>(reg + L::registry::info);
    a.put<uintptr_t>(slots + 16 * 12, w.s.key_team_tables);
    a.put<uintptr_t>(slots + 16 * 12 + 8, tt);
    a.put<uint8_t>(info + 12, 1);
    a.put<uintptr_t>(tt + L::badge::tables, table);
    a.put<uintptr_t>(table + L::badge::table_vec, vec);
    a.put<uintptr_t>(table + L::badge::table_vec + 8, vec + 16 * 4);
    a.put<uintptr_t>(vec + 3 * 16 + 8, rec);
    a.put<uintptr_t>(rec + L::badge::record_key, key);
    std::memcpy(&a.mem[key - BASE], "ferrari_2026", 13);
    char out[40];
    live.update(2.0);
    CHECK(live.read_team_badge(1, out, sizeof out) && !std::strcmp(out, "ferrari_2026"));
    CHECK(!live.read_team_badge(2, out, sizeof out));
    char path[96];
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "#overrides.osd.timing_tower.team_badges_hud.ferrari_2026"));
    CHECK(!live.read_team_badge_texture(1, path, 40) && !path[0]);
    CHECK(!live.read_team_badge_texture(2, path, sizeof path));
    a.put<uint8_t>(rec + L::badge::record_render, 1);
    CHECK(!live.read_team_badge(1, out, sizeof out));
    uintptr_t badge = ents + L::badge::entrant_stride + L::badge::entrant_badge;
    a.put<uint32_t>(badge, (7u << 16) | 0);
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_ePreLoaded 7"));
    a.put<uint32_t>(badge, (12u << 16) | 0x5500 | 2);
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eNetworkReplicated 12"));
    a.put<uint32_t>(badge, (L::badge::badge_none << 16) | 0);
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_default.tga"));
    a.put<uint32_t>(badge, (7u << 16) | 3);
    CHECK(!live.read_team_badge_texture(1, path, sizeof path));
    a.put<uint32_t>(badge, (7u << 16) | 0);
    Gates off = Gates::all();
    off.on[G_BADGE_RENDER] = false;
    Live plain(w.mem(), w.s, off);
    plain.update(2.0);
    CHECK(!plain.read_team_badge_texture(1, path, sizeof path));
    a.put<uint8_t>(rec + L::badge::record_render, 0);
    CHECK(plain.read_team_badge_texture(1, path, sizeof path));
    a.put<uint8_t>(rec + L::badge::record_render, 1);
    a.put<uint32_t>(ents + L::badge::entrant_stride + L::badge::entrant_team, (3u << 16) | L::badge::my_team_kind);
    a.put<uint32_t>(badge, (4u << 16) | 1);
    CHECK(!live.read_team_badge(1, out, sizeof out));
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eUserDefined 4"));
    a.put<uint32_t>(badge, 1);
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eUserDefined 0"));
    a.put<uint32_t>(ents + L::badge::entrant_team, L::badge::my_team_kind);
    a.put<uint32_t>(ents + L::badge::entrant_badge, (6u << 16) | 1);
    CHECK(live.read_team_badge_texture(9, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eUserDefined 6"));
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eUserDefined 0"));
    CHECK(!plain.read_team_badge_texture(1, path, sizeof path) && !path[0]);
    // The packed badge's own table/index are independent of the My Team's
    // table/index. A My Team can use a preloaded or replicated badge too.
    a.put<uint32_t>(badge, (12u << 16) | 0x5500 | 2);
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eNetworkReplicated 12"));
    a.put<uint32_t>(badge, (7u << 16));
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_ePreLoaded 7"));
    a.put<uint32_t>(badge, (L::badge::badge_none << 16) | 1);
    CHECK(live.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_default.tga"));
    a.put<uint32_t>(badge, (4u << 16) | 3);
    CHECK(!live.read_team_badge_texture(1, path, sizeof path) && !path[0]);
    a.put<uint32_t>(badge, (4u << 16) | 1);
    CHECK(!live.read_team_badge_texture(1, path, 12) && !path[0]);
    CHECK(!live.read_team_badge_texture(1, nullptr, 0));
    // Regression from the real career: fallback badge 0x1, but mode 79 /
    // loaded slot 11 has saved badge 0x10001. Both My Team cars use index 1.
    uintptr_t custom = a.alloc(0x300), descriptor = a.alloc(0x20), type_name = a.alloc(64);
    std::memcpy(&a.mem[type_name - BASE], "playerCustomisationInfoSystem", 29);
    a.put<uintptr_t>(descriptor + 8, type_name);
    a.put<uintptr_t>(slots + 16 * 13, descriptor);
    a.put<uintptr_t>(slots + 16 * 13 + 8, custom);
    a.put<uint8_t>(info + 13, 1);
    uintptr_t head = a.alloc(0x40), node = a.alloc(0x40), tree = a.alloc(0x40), leaf = a.alloc(0x40);
    a.put<uintptr_t>(custom + 0x120, head); a.put<uint64_t>(custom + 0x128, 1);
    a.put<uintptr_t>(head, node); a.put<uintptr_t>(node, head); a.put<uint32_t>(node + 0x10, 79);
    a.put<uintptr_t>(node + 0x20, tree); a.put<uint64_t>(node + 0x28, 1);
    a.put<uintptr_t>(tree + 8, leaf); a.put<uintptr_t>(leaf, tree); a.put<uintptr_t>(leaf + 0x10, tree);
    a.put<uint64_t>(leaf + 0x20, 11); a.put<uint32_t>(leaf + 0x28, 0x10001);
    uintptr_t save = a.alloc(0x300), children = a.alloc(8), component = a.alloc(0x300);
    uintptr_t vt = a.alloc(0x40), fn = a.alloc(16), save_type = a.alloc(0x20), save_name = a.alloc(64), slot = a.alloc(16);
    a.put<uintptr_t>(game + 0xf08, save);
    a.put<uintptr_t>(save + 0x238, children); a.put<uintptr_t>(save + 0x240, children + 8);
    a.put<uintptr_t>(children, component); a.put<uintptr_t>(component + 0x18, vt);
    a.put<uintptr_t>(vt + 0x28, fn);
    const uint8_t type_code[] = {0x48, 0x8d, 0x05, 0, 0, 0, 0, 0xc3};
    std::memcpy(&a.mem[fn - BASE], type_code, sizeof type_code);
    a.put<int32_t>(fn + 3, int32_t(save_type - fn - 7));
    std::memcpy(&a.mem[save_name - BASE], "SaveSlotGameModeComponent", 26);
    a.put<uintptr_t>(save_type + 8, save_name); a.put<uintptr_t>(component + 0x280, slot); a.put<uint64_t>(slot, 11);
    uintptr_t career_name = a.alloc(64);
    std::memcpy(&a.mem[career_name - BASE], "my_team_career_26", 18);
    a.put<uintptr_t>(mode + L::mode::label, career_name); a.put<uint32_t>(game + 0x2a8, 79);
    a.put<uint32_t>(badge, 1);
    Sites career_sites = w.s; career_sites.my_team_saved_badge = true;
    career_sites.image_hi = BASE + a.mem.size(); // Fixture type-getter code and metadata live in its arena.
    Live career_badges(w.mem(), career_sites, Gates::all()); career_badges.update(2.0);
    CHECK(career_badges.read_team_badge_texture(1, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eUserDefined 1"));
    CHECK(career_badges.read_team_badge_texture(9, path, sizeof path) &&
          !std::strcmp(path, "eaid://ui_package/art/textures/render_badge_eUserDefined 1"));
    a.put<uint64_t>(slot, 12);
    CHECK(!career_badges.read_team_badge_texture(1, path, sizeof path) && !path[0]);
    a.put<uint64_t>(slot, 11); a.put<uint32_t>(node + 0x10, 27);
    CHECK(!career_badges.read_team_badge_texture(1, path, sizeof path) && !path[0]);
    a.put<uint32_t>(node + 0x10, 79); a.put<uintptr_t>(node, node);
    CHECK(!career_badges.read_team_badge_texture(1, path, sizeof path) && !path[0]);
    a.put<uintptr_t>(node, head); a.put<uint8_t>(fn, 0xe9);
    CHECK(!career_badges.read_team_badge_texture(1, path, sizeof path) && !path[0]);
    a.put<uint8_t>(fn, 0x48);
    CHECK(!live.read_team_badge_texture(1, path, sizeof path) && !path[0]); // Old unproved build stays unavailable.
    a.put<uint32_t>(ents + L::badge::entrant_stride + L::badge::entrant_team, (3u << 16) | 0);
    a.put<uint8_t>(rec + L::badge::record_render, 0);
    CHECK(live.read_team_badge(1, out, sizeof out));
    a.put<uint32_t>(w.s.vehicle_count, 0);
    live.update(3.0);
    CHECK(!live.active() && !live.read_team_badge(1, out, sizeof out) && !live.read_team_badge_texture(1, path, sizeof path));
}

static void test_utf8_prefix()
{
    const char *ok = "S\xc3\xa9" "bastien Bourdais"; // 2-byte e acute
    size_t n = std::strlen(ok);
    CHECK(utf8_prefix(ok, n, 64) == n);
    CHECK(utf8_prefix(ok, n, 2) == 1);
    CHECK(utf8_prefix(ok, n, 3) == 3);
    CHECK(utf8_prefix("\xe2\x82\xac", 3, 2) == 0 && utf8_prefix("\xe2\x82\xac", 3, 3) == 3);
    CHECK(utf8_prefix("\xf0\x9f\x8f\x81", 4, 4) == 4);
    CHECK(utf8_prefix("ab\x01", 3, 8) == ~size_t(0));
    CHECK(utf8_prefix("a\xc3", 2, 8) == ~size_t(0));
    CHECK(utf8_prefix("\xc0\xaf", 2, 8) == ~size_t(0));
    CHECK(utf8_prefix("\xed\xa0\x80", 3, 8) == ~size_t(0));
    CHECK(utf8_prefix("\x80", 1, 8) == ~size_t(0));
}

// Builds the chains Sess_Get_CarDriverName walks: VehicleHelperSystem participants -> CarPhysicsRecord -> CarLive
// vehicle id, the human flag and player name, and the session entrant's driver ref into the EntrantsSystem driver tables.
static void test_driver_name()
{
    namespace D = L::driver;
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    Arena &a = w.a;
    auto str = [&](const char *s) {
        uintptr_t p = a.alloc(std::strlen(s) + 1);
        std::memcpy(&a.mem[p - BASE], s, std::strlen(s) + 1);
        return p;
    };
    uintptr_t reg = a.get<uintptr_t>(a.get<uintptr_t>(w.s.game_manager) + L::registry::from_manager);
    uintptr_t slots = a.get<uintptr_t>(reg + L::registry::slots), info = a.get<uintptr_t>(reg + L::registry::info);
    auto reg_at = [&](int i, uintptr_t key, uintptr_t value) {
        a.put<uintptr_t>(slots + 16 * i, key);
        a.put<uintptr_t>(slots + 16 * i + 8, value);
        a.put<uint8_t>(info + i, 1);
    };

    // VehicleHelperSystem: participants for vehicles 0..3 in reverse order after one null entry; vehicle 0 is a human.
    uintptr_t vh = a.alloc(0x200), parts = a.alloc(8 * 5);
    reg_at(13, w.s.key_session_component, vh);
    a.put<uintptr_t>(vh + D::player_name, str("Marcel"));
    a.put<uintptr_t>(vh + D::participants, parts);
    a.put<uintptr_t>(vh + D::participants + 8, parts + 8 * 5);
    for (int i = 0; i < 4; i++) {
        uintptr_t p = a.alloc(0x40), phys = a.alloc(0x80);
        a.put<uintptr_t>(p + D::participant_physics, phys);
        a.put<uintptr_t>(phys + D::physics_car, w.cars[i]);
        a.put<uint8_t>(p + D::participant_flags, i == 0 ? 0x80 : 0x40);
        a.put<uintptr_t>(parts + 8 * (4 - i), p);
    }

    // Session entrants: vehicle 2 -> domain 0 id 3, vehicle 1 -> domain 2 id 1, vehicle 3 -> domain 3 (invalid).
    uintptr_t mode = a.get<uintptr_t>(w.mode_ctx + L::badge::mode), game = a.alloc(0x400),
              ents = a.alloc(L::badge::entrant_stride * 3);
    a.put<uintptr_t>(mode + L::badge::game, game);
    a.put<uintptr_t>(game + L::badge::entrants, ents);
    a.put<uintptr_t>(game + L::badge::entrants + 8, ents + L::badge::entrant_stride * 3);
    a.put<uint8_t>(ents + L::badge::entrant_vid, 2);
    a.put<uint32_t>(ents + D::entrant_ref, (3u << 16) | 0);
    a.put<uint8_t>(ents + L::badge::entrant_stride + L::badge::entrant_vid, 1);
    a.put<uint32_t>(ents + L::badge::entrant_stride + D::entrant_ref, (1u << 16) | 2);
    a.put<uint8_t>(ents + 2 * L::badge::entrant_stride + L::badge::entrant_vid, 3);
    a.put<uint32_t>(ents + 2 * L::badge::entrant_stride + D::entrant_ref, (0u << 16) | 3);

    // EntrantsSystem driver tables 0 and 2.
    uintptr_t es = a.alloc(0x400);
    reg_at(12, w.s.key_team_tables, es);
    auto table = [&](unsigned domain, unsigned n) {
        uintptr_t t = a.alloc(0x20), vec = a.alloc(16 * n);
        a.put<uintptr_t>(es + D::sets + domain * D::set_stride, t);
        a.put<uintptr_t>(t + D::table_vec, vec);
        a.put<uintptr_t>(t + D::table_vec + 8, vec + 16 * n);
        return vec;
    };
    auto record = [&](uintptr_t vec, unsigned id, const char *name) {
        uintptr_t r = a.alloc(0x100);
        a.put<uintptr_t>(r + D::record_name, str(name));
        a.put<uintptr_t>(vec + 16 * id + D::slot_record, r);
    };
    record(table(0, 4), 3, "Nico H\xc3\xbclkenberg");
    record(table(2, 2), 1, "Sergio P\xc3\xa9rez");

    live.update(2.0);
    char out[64];
    CHECK(live.read_driver_name(0, out, sizeof out) && !std::strcmp(out, "Marcel"));
    CHECK(live.read_driver_name(2, out, sizeof out) && !std::strcmp(out, "Nico H\xc3\xbclkenberg"));
    CHECK(live.read_driver_name(1, out, sizeof out) && !std::strcmp(out, "Sergio P\xc3\xa9rez"));
    CHECK(!live.read_driver_name(3, out, sizeof out)); // domain above 2
    CHECK(!live.read_driver_name(9, out, sizeof out)); // no participant
    CHECK(!live.read_driver_name(40, out, sizeof out));
    CHECK(live.read_driver_name(1, out, 9) && !std::strcmp(out, "Sergio P")); // cut before the 2-byte character
    CHECK(live.read_driver_name(1, out, 11) && !std::strcmp(out, "Sergio P\xc3\xa9"));
    CHECK(live.read_driver_name(2, out, 2) && !std::strcmp(out, "N"));
    CHECK(!live.read_driver_name(2, out, 1));

    // Bit 7 of the participant flags on vehicle 2 takes the player name; bit 6 alone does not.
    uintptr_t p2 = a.get<uintptr_t>(parts + 8 * 2);
    a.put<uint8_t>(p2 + D::participant_flags, 0xc0);
    CHECK(live.read_driver_name(2, out, sizeof out) && !std::strcmp(out, "Marcel"));
    a.put<uint8_t>(p2 + D::participant_flags, 0x40);

    // Past the end of the driver table, an empty slot, a control character: no name.
    a.put<uint32_t>(ents + D::entrant_ref, (4u << 16) | 0);
    CHECK(!live.read_driver_name(2, out, sizeof out));
    a.put<uint32_t>(ents + D::entrant_ref, (2u << 16) | 0);
    CHECK(!live.read_driver_name(2, out, sizeof out));
    a.put<uint32_t>(ents + D::entrant_ref, (3u << 16) | 0);
    CHECK(live.read_driver_name(2, out, sizeof out));
    a.put<uintptr_t>(vh + D::player_name, str("bad\tname"));
    CHECK(!live.read_driver_name(0, out, sizeof out));

    // As in the game, a vehicle id at or past the participant vector's length has no name.
    a.put<uintptr_t>(vh + D::participants + 8, parts + 8 * 2);
    CHECK(!live.read_driver_name(2, out, sizeof out));

    Gates off = Gates::all();
    off.on[G_DRIVER_NAMES] = false;
    Live gated(w.mem(), w.s, off);
    gated.update(0.0);
    CHECK(!gated.read_driver_name(1, out, sizeof out));
    a.put<uintptr_t>(vh + D::participants + 8, parts + 8 * 5);
    CHECK(live.read_driver_name(1, out, sizeof out));
    a.put<uint32_t>(w.s.vehicle_count, 0);
    live.update(4.0);
    CHECK(!live.active() && !live.read_driver_name(1, out, sizeof out));
}

static void test_pit_lane()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    live.update(1.0);
    auto lane_flag = [&](int i) {
        uintptr_t lane = w.a.get<uintptr_t>(w.pit_sys + L::pit::lane_object);
        return w.a.get<uintptr_t>(lane + L::pit::lane_flags + L::pit::lane_flag_stride * i) + L::pit::in_lane;
    };
    CHECK(live.cars[0].have & SWOFTY_LIVE_HAVE_PIT_LANE);
    CHECK(live.cars[0].pit_status == 0 && live.cars[0].pit_stops == 0);
    w.a.put<uint8_t>(lane_flag(0), 1);
    w.a.put<uint32_t>(w.pit_rec[0] + L::pit::state, 3);
    w.a.put<uint32_t>(w.pit_rec[0] + L::pit::stops, 0x1202);
    w.a.put<uint8_t>(lane_flag(1), 1);
    w.a.put<uint32_t>(w.pit_rec[1] + L::pit::state, 4);
    live.update(2.0);
    CHECK(live.cars[0].pit_status == 1 && live.cars[0].pit_stops == 2);
    CHECK(live.cars[1].pit_status == 2);
    CHECK(live.cars[2].pit_status == 0);
    w.a.put<uint8_t>(lane_flag(0), 0);
    live.update(3.0);
    CHECK(live.cars[0].pit_status == 0 && live.cars[0].pit_stops == 2);

    SwoftyTelemetryFeed f;
    swofty::tel::feed_init(&f);
    w.a.put<uint8_t>(lane_flag(1), 1);
    live.update(4.0);
    uint32_t groups = fill_feed(f, live.ses, live.cars, live.present);
    CHECK(groups & PKT_LAP_DATA);
    CHECK(f.cars[1].pit_status == 2 && f.cars[0].pit_stops == 2);
    CHECK(f.cars[1].flags & SWOFTY_CAR_FLAG_IN_PIT_LANE);
    CHECK(!(f.cars[0].flags & SWOFTY_CAR_FLAG_IN_PIT_LANE));

    Gates no_lane = Gates::all();
    no_lane.on[G_PIT_LANE] = false;
    Live plain(w.mem(), w.s, no_lane);
    plain.update(0.0);
    plain.update(1.0);
    CHECK(!(plain.cars[1].have & SWOFTY_LIVE_HAVE_PIT_LANE) && plain.cars[1].pit_state == 4);
}

static void test_sector_sites()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.update(0.0);
    live.update(1.0);
    CHECK(live.ses.sector_count == 3);

    World apart;
    apart.s.sector_count += 0x10;
    Live a(apart.mem(), apart.s, Gates::all());
    a.update(0.0);
    a.update(1.0);
    CHECK(a.ses.sector_count == 0);
}

static void test_bus_logic()
{
    uint8_t m[0x20] = {};
    auto collide = [&](uint8_t a, uint8_t b) {
        m[bus::msg::collision_a] = a;
        m[bus::msg::collision_b] = b;
    };
    bus::Collisions c;
    Event e{};
    collide(3, 5);
    CHECK(c.take(m, 1000.0, e) && e.kind == Event::collision && e.a == 3 && e.b == 5);
    collide(7, 5);
    CHECK(!c.take(m, 4900.0, e));
    collide(5, 7);
    CHECK(c.take(m, 4900.0, e) && e.a == 5 && e.b == 7);
    collide(3, 8);
    CHECK(c.take(m, 5100.0, e));
    collide(7, 3);
    CHECK(!c.take(m, 5200.0, e));
    collide(7, 3);
    CHECK(c.take(m, 9200.0, e));
    collide(24, 1);
    CHECK(!c.take(m, 20000.0, e));
    collide(1, 24);
    CHECK(!c.take(m, 20000.0, e));

    uint8_t st[0x20] = {};
    st[bus::msg::status_vehicle] = 6;
    st[bus::msg::status_reason] = 3;
    st[bus::msg::status_code] = 5;
    CHECK(bus::retirement_from(st, e) && e.kind == Event::retirement && e.a == 6 && e.b == 3);
    st[bus::msg::status_code] = 7;
    CHECK(bus::retirement_from(st, e));
    st[bus::msg::status_code] = 4;
    CHECK(!bus::retirement_from(st, e));
    st[bus::msg::status_code] = 2;
    CHECK(!bus::retirement_from(st, e));
    st[bus::msg::status_code] = 5;
    st[bus::msg::status_vehicle] = 30;
    CHECK(!bus::retirement_from(st, e));

    bus::Queue q;
    collide(1, 2);
    q.push_collision(m, 100.0);
    q.push_collision(m, 200.0);
    st[bus::msg::status_vehicle] = 2;
    q.push_retirement(st);
    auto got = q.drain();
    CHECK(got.size() == 2 && got[0].kind == Event::collision && got[1].kind == Event::retirement && got[1].b == 3);
    CHECK(q.drain().empty());
}

namespace fake_bus {
Arena *arena;
uintptr_t slot, holder, bus, allocator, vtable;
int subscribed;
std::vector<std::pair<std::string, uintptr_t>> subs;
uintptr_t next_delegate;

uintptr_t alloc(uintptr_t a, uintptr_t fn, uintptr_t ctx)
{
    if (a != allocator)
        return 0;
    uintptr_t d = arena->alloc(0x10);
    arena->put<uintptr_t>(d, fn);
    arena->put<uintptr_t>(d + 8, ctx);
    return d;
}

void subscribe(uintptr_t b, const char *name, uintptr_t delegate)
{
    CHECK(b == bus);
    subs.emplace_back(name, delegate);
}
}

static void test_bus_subscribe()
{
    Arena a;
    fake_bus::arena = &a;
    fake_bus::slot = a.alloc(8);
    fake_bus::holder = a.alloc(0x20);
    fake_bus::bus = a.alloc(0x100);
    fake_bus::allocator = a.alloc(0x20);
    fake_bus::subs.clear();
    a.put<uintptr_t>(fake_bus::slot, fake_bus::holder);
    a.put<uintptr_t>(fake_bus::holder + bus::bus_from_holder, fake_bus::bus);
    a.put<uintptr_t>(fake_bus::bus + bus::bus_allocator, fake_bus::allocator);

    bus::Ops o;
    o.mem = Mem{arena_read, &a};
    o.bus_slot = fake_bus::slot;
    o.alloc = fake_bus::alloc;
    o.subscribe = fake_bus::subscribe;
    o.collision_fn = 0xc0de;
    o.status_fn = 0x57a7;
    o.ctx = 0;

    bus::Subscriber s;
    CHECK(!s.arm(o) && fake_bus::subs.empty());
    a.put<uint8_t>(fake_bus::bus + bus::bus_active, 1);
    CHECK(s.arm(o) && s.armed() && fake_bus::subs.size() == 2);
    CHECK(fake_bus::subs[0].first == "r_car_to_car_col" && fake_bus::subs[1].first == "vehicle_status_changed");
    CHECK(a.get<uintptr_t>(fake_bus::subs[0].second) == 0xc0de && a.get<uintptr_t>(fake_bus::subs[1].second) == 0x57a7);
    CHECK(s.arm(o) && fake_bus::subs.size() == 2);

    uintptr_t fresh = a.alloc(0x100);
    a.put<uintptr_t>(fake_bus::holder + bus::bus_from_holder, fresh);
    a.put<uint8_t>(fresh + bus::bus_active, 1);
    a.put<uintptr_t>(fresh + bus::bus_allocator, fake_bus::allocator);
    fake_bus::bus = fresh;
    CHECK(s.arm(o) && fake_bus::subs.size() == 4 && s.bus() == fresh);

    bus::Subscriber none;
    bus::Ops bare = o;
    bare.alloc = nullptr;
    CHECK(!none.arm(bare));
}

static void test_bus_events_in_live()
{
    World w;
    Live live(w.mem(), w.s, Gates::all());
    live.set_retirements_from_bus(true);
    live.update(0.0);
    live.take_events();
    w.a.put<uint32_t>(w.results_entries[2] + L::results::status, 4);
    live.update(0.4);
    CHECK(live.take_events().empty());

    live.add_event({Event::retirement, 2, 5});
    live.add_event({Event::collision, 0, 1});
    auto ev = live.take_events();
    CHECK(ev.size() == 2 && ev[0].kind == Event::retirement && ev[0].b == 5 && ev[1].kind == Event::collision);

    World w2;
    Live edge(w2.mem(), w2.s, Gates::all());
    edge.update(0.0);
    w2.a.put<uint32_t>(w2.results_entries[2] + L::results::status, 4);
    edge.update(0.4);
    ev = edge.take_events();
    CHECK(ev.size() == 1 && ev[0].kind == Event::retirement && ev[0].b == 0);
}

static uint32_t on_mask(const Decision &d)
{
    uint32_t m = 0;
    for (unsigned g = 0; g < G_COUNT; g++)
        m |= d.gates.on[g] ? bit(Group(g)) : 0;
    return m;
}

static Sites resolved_everywhere(Sites s)
{
    for (const Needs &n : NEEDS)
        for (auto slot : n.sites)
            if (slot && !(s.*slot))
                s.*slot = 0x1000;
    return s;
}

static void test_build_gate()
{
    Row backed[G_COUNT];
    for (unsigned g = 0; g < G_COUNT; g++)
        backed[g] = {Group(g), true, ROWS[g].mapped, ROWS[g].needs};
    backed[G_TYRES].mapped = false;
    backed[G_MODE].fields = false;
    uint32_t mapped = 0;
    for (unsigned g = 0; g < G_COUNT; g++)
        mapped |= backed[g].mapped && backed[g].fields ? bit(Group(g)) : 0;
    World kb;
    kb.s = resolved_everywhere(kb.s);
    Decision dk = decide(kb.s, KB_BUILD, false, backed);
    CHECK(on_mask(dk) == mapped);
    CHECK(dk.verdict[G_TYRES].why == Why::row && dk.verdict[G_PROGRESS].why == Why::on);
    Decision undeclared = decide(kb.s, KB_BUILD, false);
    for (unsigned g = 0; g < G_COUNT; g++)
        CHECK(undeclared.gates.on[g] == (ROWS[g].fields && ROWS[g].mapped));

    CHECK(proof_for(1789118616u) == &KB_BUILD && proof_for(1789128892u) == &EA_BUILD && !proof_for(1234u) && !proof_for(0));
    CHECK(EA_BUILD.progress_from_manager == 0x2a0 && KB_BUILD.progress_from_manager == L::progress::from_manager);
    CHECK(EA_BUILD.groups == (ALL_GROUPS & ~(bit(G_DRIVER_NAMES) | bit(G_BADGE_RENDER))) && KB_BUILD.groups == ALL_GROUPS);

    World ea(EA_BUILD.progress_from_manager);
    Sites ea_sites = ea.s;
    Decision de = decide(resolved_everywhere(ea.s), EA_BUILD, false, backed);
    CHECK(on_mask(de) == (mapped & EA_BUILD.groups) && (on_mask(de) & bit(G_PROGRESS)) && (on_mask(de) & bit(G_BUS_EVENTS)));
    CHECK(de.verdict[G_TYRES].why == Why::row && de.verdict[G_MODE].why == Why::row && de.verdict[G_BADGES].why == Why::on);

    Live live(ea.mem(), ea_sites, Gates::all());
    live.update(0.0);
    CHECK(live.active() && (live.cars[0].have & SWOFTY_LIVE_HAVE_PROGRESS) && live.cars[0].lap_m == 3000.f);
    CHECK(live.cars[1].lap == 1 && live.ses.order[0] == 1);
    Sites misplaced_sites = ea_sites;
    misplaced_sites.progress_from_manager = KB_BUILD.progress_from_manager;
    Live misplaced(ea.mem(), misplaced_sites, Gates::all());
    misplaced.update(0.0);
    CHECK(!(misplaced.cars[0].have & SWOFTY_LIVE_HAVE_PROGRESS));

    BuildProof fake = {42u, "Fake", bit(G_CLASS) | bit(G_TIMING), 0};
    Decision df = decide(kb.s, fake, false, backed);
    CHECK(on_mask(df) == (bit(G_CLASS) | bit(G_TIMING)));
    CHECK(df.verdict[G_PROGRESS].why == Why::build && df.verdict[G_CLASS].why == Why::on);
    Decision ff = decide(kb.s, fake, true, backed);
    CHECK((on_mask(ff) & bit(G_PROGRESS)) && (on_mask(ff) & bit(G_TYRES)));
    BuildProof nothing = {43u, "Nothing", 0, 0};
    CHECK(on_mask(decide(kb.s, nothing, false, backed)) == 0);

    Sites missing = resolved_everywhere(ea.s);
    missing.key_timing = 0;
    Decision dm = decide(missing, EA_BUILD, true, backed);
    CHECK(!dm.gates[G_TIMING] && dm.verdict[G_TIMING].why == Why::site && dm.verdict[G_TIMING].missing == &Sites::key_timing);
    CHECK(dm.gates[G_CLASS]);

    Sites none;
    CHECK(on_mask(decide(none, EA_BUILD, false, backed)) == 0);
}

static void test_steam_26566308()
{
    const BuildProof *build = proof_for(1794312636u);
    CHECK(build == &STEAM_26566308_BUILD);
    CHECK(build->groups == ALL_GROUPS);
    CHECK(build->progress_from_manager == 0x2d0);
    CHECK(!KB_BUILD.field_proofs && !EA_BUILD.field_proofs);
    World w;
    Arena &a = w.a;
    uintptr_t reg = a.get<uintptr_t>(a.get<uintptr_t>(w.s.game_manager) + L::registry::from_manager);
    uintptr_t slots = a.get<uintptr_t>(reg + L::registry::slots), info = a.get<uintptr_t>(reg + L::registry::info);
    uintptr_t tyres = a.alloc(0x120), vehicles = a.alloc(3 * 8), components[3];
    a.put<uintptr_t>(slots + 8 * 16, w.s.key_tyres);
    a.put<uintptr_t>(slots + 8 * 16 + 8, tyres);
    a.put<uint8_t>(info + 8, 1);
    a.put<uintptr_t>(tyres + L::tyres::per_vehicle, vehicles);
    a.put<uintptr_t>(tyres + L::tyres::per_vehicle + 8, vehicles + 3 * 8);
    for (unsigned i = 0; i < 3; i++) {
        uintptr_t c = components[i] = a.alloc(L::tyres::hi), set = a.alloc(L::tyres::set_stride);
        uintptr_t physical = a.alloc(8), visual = a.alloc(0x58);
        a.put<uintptr_t>(vehicles + 8 * i, c);
        a.put<uint64_t>(c + L::tyres::version, 1);
        a.put<uint64_t>(c + L::tyres::built_for, 1);
        a.put<uint64_t>(c + L::tyres::built, 1);
        a.put<uint64_t>(c + L::tyres::count, 1);
        a.put<uintptr_t>(c + L::tyres::set_begin, set);
        a.put<uintptr_t>(c + L::tyres::set_end, set + L::tyres::set_stride);
        a.put<uint32_t>(set + 0xc, 1);
        a.put<uint32_t>(c + L::tyres::list, 1);
        a.put<uintptr_t>(c + L::tyres::list + L::tyres::desc_physical, physical);
        a.put<uintptr_t>(c + L::tyres::list + L::tyres::desc_visual, visual);
        a.put<uint8_t>(physical, uint8_t(i == 2 ? 7 : 18));
        a.put<uint8_t>(visual, uint8_t(i == 2 ? 7 : 16));
        a.put<int32_t>(visual + L::tyres::visual_compound, i == 2 ? 7 : 3);
    }
    Sites sites = resolved_everywhere(w.s);
    sites.progress_from_manager = build->progress_from_manager;
    sites.ers_mode = build->ers_mode;
    for (unsigned i = 0; i < 3; i++) a.put<int32_t>(w.ers[i] + sites.ers_mode, 2);
    Decision d = decide(sites, *build, false);
    CHECK(on_mask(d) == STEAM_26566308_GROUPS);
    BuildProof missing_fields = *build; missing_fields.field_proofs = 0;
    CHECK(!decide(sites, missing_fields, false).gates[G_DRIVER_NAMES]);
    Row unmapped[G_COUNT]; std::memcpy(unmapped, ROWS, sizeof unmapped);
    unmapped[G_DRIVER_NAMES].mapped = false;
    CHECK(!decide(sites, *build, false, unmapped).gates[G_DRIVER_NAMES]);
    for (const auto &need : NEEDS) for (auto slot : need.sites) if (slot) {
        Sites missing = sites; missing.*slot = 0;
        CHECK(!decide(missing, *build, false).gates[need.group]);
    }
    for (unsigned i = 0; i < G_COUNT; i++)
        CHECK((build->groups & bit(Group(i))) ? d.verdict[i].why == Why::on : !d.gates[Group(i)]);

    Live live(w.mem(), sites, d.gates);
    live.update(0.0);
    live.update(1.0);
    CHECK(live.active());
    CHECK(live.ses.have & SWOFTY_LIVE_SHAVE_PLAYER);
    CHECK(live.ses.have & SWOFTY_LIVE_SHAVE_ORDER);
    constexpr uint64_t expected = SWOFTY_LIVE_HAVE_IDENT | SWOFTY_LIVE_HAVE_CLASS | SWOFTY_LIVE_HAVE_PIT |
        SWOFTY_LIVE_HAVE_PIT_LANE | SWOFTY_LIVE_HAVE_TYRE | SWOFTY_LIVE_HAVE_SPEED | SWOFTY_LIVE_HAVE_RPM |
        SWOFTY_LIVE_HAVE_GEAR | SWOFTY_LIVE_HAVE_PEDALS | SWOFTY_LIVE_HAVE_CLUTCH | SWOFTY_LIVE_HAVE_STEER |
        SWOFTY_LIVE_HAVE_G | SWOFTY_LIVE_HAVE_WORLD | SWOFTY_LIVE_HAVE_ERS_STORE | SWOFTY_LIVE_HAVE_ERS_MODE |
        SWOFTY_LIVE_HAVE_ERS_POWER | SWOFTY_LIVE_HAVE_ERS_LAP | SWOFTY_LIVE_HAVE_PROGRESS |
        SWOFTY_LIVE_HAVE_GAP_LEADER | SWOFTY_LIVE_HAVE_GAP_AHEAD | SWOFTY_LIVE_HAVE_SECTOR;
    CHECK((live.cars[0].have & expected) == expected);
    CHECK(live.cars[0].ers_mode == 2 && live.cars[0].gear == 3 && live.cars[0].speed == 50.f);
    CHECK(live.cars[0].tyre_actual == 18 && live.cars[0].tyre_visual == 16 && live.cars[0].tyre_letter == 'S');
    CHECK(live.cars[2].tyre_actual == 7 && live.cars[2].tyre_letter == 'I');
    constexpr uint64_t session_bits = SWOFTY_LIVE_SHAVE_TYPE | SWOFTY_LIVE_SHAVE_TRACK_LENGTH |
        SWOFTY_LIVE_SHAVE_GAME_MODE | SWOFTY_LIVE_SHAVE_ROAD_ORDER;
    CHECK((live.ses.have & session_bits) == session_bits);
    char name[128];
    CHECK(!live.read_driver_name(0, name, sizeof name));
    a.put<uint64_t>(components[0] + L::tyres::built_for, 2);
    live.update(2.0);
    CHECK(!(live.cars[0].have & SWOFTY_LIVE_HAVE_TYRE));
    CHECK(live.cars[1].have & SWOFTY_LIVE_HAVE_TYRE);
    a.put<uint64_t>(components[0] + L::tyres::built_for, 1);
    live.update(3.0);
    CHECK(live.cars[0].have & SWOFTY_LIVE_HAVE_TYRE);

    sites.key_pit = 0;
    d = decide(sites, *build, false);
    CHECK(!d.gates[G_PIT] && !d.gates[G_PIT_LANE]);
    CHECK(d.verdict[G_PIT].why == Why::site && d.verdict[G_PIT_LANE].why == Why::site);
    CHECK(d.gates[G_CLASS] && d.gates[G_TYRES] && d.gates[G_CONTROL]);
    sites.key_tyres = 0;
    CHECK(!decide(sites, *build, false).gates[G_TYRES]);
    sites.controllers = 0;
    CHECK(!decide(sites, *build, false).gates[G_CONTROL]);
    sites.key_results = 0;
    CHECK(!decide(sites, *build, false).gates[G_CLASS]);
    CHECK(on_mask(decide(Sites{}, *build, false)) == 0);
    CHECK(!proof_for(1794312635u) && !proof_for(1794312637u));
}

static void test_steam_26566308_ers_mode()
{
    CHECK(KB_BUILD.ers_mode == 0x1a68 && EA_BUILD.ers_mode == 0x1a68);
    CHECK(STEAM_26566308_BUILD.ers_mode == 0x1a6c);
    World original;
    Live kb(original.mem(), original.s, Gates::all());
    kb.update(0.0);
    CHECK((kb.cars[0].have & SWOFTY_LIVE_HAVE_ERS_MODE) && kb.cars[0].ers_mode == 2);
    World current;
    current.s.ers_mode = STEAM_26566308_BUILD.ers_mode;
    current.a.put<int32_t>(current.ers[0] + L::ers::mode, 0x7fffffff);
    current.a.put<int32_t>(current.ers[0] + current.s.ers_mode, 1);
    Live live(current.mem(), current.s, Gates::all());
    live.update(0.0);
    CHECK((live.cars[0].have & SWOFTY_LIVE_HAVE_ERS_MODE) && live.cars[0].ers_mode == 1);
    current.a.put<int32_t>(current.ers[0] + current.s.ers_mode, 3);
    live.update(1.0);
    CHECK((live.cars[0].have & SWOFTY_LIVE_HAVE_ERS_MODE) && live.cars[0].ers_mode == 3);
    current.a.put<int32_t>(current.ers[0] + current.s.ers_mode, 5);
    live.update(2.0);
    CHECK(!(live.cars[0].have & SWOFTY_LIVE_HAVE_ERS_MODE));
    CHECK(live.cars[0].have & SWOFTY_LIVE_HAVE_ERS_STORE);
}

static void test_readiness_rows()
{
    for (unsigned g = 0; g < G_COUNT; g++) {
        CHECK(ROWS[g].group == Group(g));
        CHECK(ROWS[g].needs && ROWS[g].needs[0]);
        CHECK(std::strlen(group_name(Group(g))) > 0);
    }
    CHECK(ROWS[G_TRACK].fields && ROWS[G_TRACK].mapped);
    CHECK(ROWS[G_TYRES].mapped && ROWS[G_MODE].mapped && ROWS[G_BADGES].mapped && ROWS[G_PIT_LANE].mapped);
    CHECK(ROWS[G_TRACK_NAME].mapped && ROWS[G_SPECTATE_GAP].mapped);
    CHECK(ROWS[G_AERO].fields && ROWS[G_AERO].mapped);
    CHECK(L::pit::lane_object == 0x48 && L::pit::lane_flags == 0x938 && L::pit::in_lane == 0x103);
    CHECK(L::pit::stops == 0x14 && L::pit::lane_flag_stride == 0x10);
}

int main()
{
    test_order_gaps_cars();
    test_inert_and_gates();
    test_player_watched();
    test_events();
    test_derive_pure();
    test_turns();
    test_helpers();
    test_snapshot();
    test_telemetry_fill();
    test_badge();
    test_utf8_prefix();
    test_driver_name();
    test_pit_lane();
    test_sector_sites();
    test_bus_logic();
    test_bus_subscribe();
    test_bus_events_in_live();
    test_readiness_rows();
    test_build_gate();
    test_steam_26566308();
    test_steam_26566308_ers_mode();
    if (failures) {
        std::printf("%d failures\n", failures);
        return 1;
    }
    std::printf("live ok\n");
    return 0;
}
