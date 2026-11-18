// Host-native test for the live to timing adapter: drives swofty::fake::Live through a few laps and checks the
// tower rows, gaps, arrows and order. Built and run by native/test/run_timing_live_test.sh.

#include <cstdio>
#include <cstring>

#include <swofty/fake_live.hpp>
#include <swofty/timing.hpp>

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

namespace timing = swofty::timing;
namespace fake = swofty::fake;

static void car_state(fake::Live &f, uint32_t v, int lap, int32_t ahead, int32_t leader, uint8_t status = 2)
{
    SwoftyLiveCar &c = f.car(v);
    c.have |= SWOFTY_LIVE_HAVE_PROGRESS | SWOFTY_LIVE_HAVE_PIT | SWOFTY_LIVE_HAVE_TYRE | SWOFTY_LIVE_HAVE_PENALTIES |
              SWOFTY_LIVE_HAVE_GAP_AHEAD | SWOFTY_LIVE_HAVE_GAP_LEADER;
    c.lap = lap;
    c.result_status = status;
    c.tyre_letter = 'M';
    c.gap_ahead_ms = ahead;
    c.gap_leader_ms = leader;
}

int main()
{
    fake::Live f;
    f.session().type = 15;
    f.session().have |= SWOFTY_LIVE_SHAVE_GAME_MODE;
    f.session().game_mode = SWOFTY_LIVE_MODE_GRAND_PRIX;
    f.add_car(3);
    f.add_car(5);
    f.add_car(7);
    f.add_car(9);
    f.set_order({5, 3, 7, 9});
    f.set_player(7);
    f.set_badge(5, "ferrari");
    car_state(f, 5, 0, SWOFTY_LIVE_NO_GAP, 0);
    car_state(f, 3, 0, 1200, 1200);
    car_state(f, 7, 0, 3500, 4700);
    car_state(f, 9, 0, SWOFTY_LIVE_NO_GAP, SWOFTY_LIVE_NO_GAP);
    f.car(9).have &= ~(SWOFTY_LIVE_HAVE_GAP_AHEAD | SWOFTY_LIVE_HAVE_GAP_LEADER);
    f.install();

    timing::Config cfg;
    cfg.mode = timing::Config::Mode::full;
    cfg.gap = timing::Config::Gap::interval;
    timing::Tower tower;

    CHECK(timing::update_from_live(tower, 0.0, cfg));
    CHECK(tower.on() && tower.race());
    CHECK(tower.rows() == 4);
    CHECK(tower.row(0).car == 5 && tower.row(0).leader && tower.row(0).pos == 1);
    CHECK(std::strcmp(tower.row(0).team_key, "ferrari") == 0);
    CHECK(tower.row(1).car == 3 && tower.row(2).car == 7 && tower.row(3).car == 9);
    CHECK(std::strcmp(tower.row(0).gap, "Interval") == 0);
    CHECK(tower.row(1).gap_kind == timing::GapKind::time && std::strcmp(tower.row(1).gap, "+1.2") == 0);
    CHECK(tower.row(2).gap_kind == timing::GapKind::time && std::strcmp(tower.row(2).gap, "+3.5") == 0);
    CHECK(tower.row(3).gap_na);
    CHECK(tower.row(2).me);
    CHECK(tower.row(1).tyre == 'M');
    CHECK(tower.row_of_car(9) == 3 && tower.row_of_car(1) == -1);

    f.car(7).have &= ~SWOFTY_LIVE_HAVE_TYRE;
    f.car(7).tyre_letter = 0;
    f.advance();
    CHECK(timing::update_from_live(tower, 0.5, cfg));
    CHECK(tower.row(2).tyre == 'M');

    cfg.gap = timing::Config::Gap::leader;
    for (uint32_t v : {5u, 3u, 7u, 9u})
        f.car(v).lap = 1;
    f.car(9).lap = 0;
    f.car(9).gap_leader_ms = f.car(9).gap_ahead_ms = 0;
    f.advance();
    CHECK(timing::update_from_live(tower, 60.0, cfg));
    CHECK(tower.lead_gaps());
    CHECK(std::strcmp(tower.row(0).gap, "Leader") == 0);
    CHECK(std::strcmp(tower.row(2).gap, "+4.7") == 0);
    CHECK(tower.row(3).gap_kind == timing::GapKind::lapped && std::strcmp(tower.row(3).gap, "+1 LAP") == 0);

    f.set_order({3, 5, 7, 9});
    f.advance();
    CHECK(timing::update_from_live(tower, 61.0, cfg));
    CHECK(tower.row(0).car == 3 && tower.row(1).car == 5);
    CHECK(tower.row(0).pos == 1 && tower.row(1).pos == 2);
    CHECK(tower.row(0).up && tower.row(1).down);
    CHECK(!tower.row(2).up && !tower.row(2).down);

    f.car(5).result_status = 4;
    f.car(7).time_penalty_s = 5.f;
    f.session().have |= SWOFTY_LIVE_SHAVE_SAFETY_CAR;
    f.session().safety_car = 1;
    f.advance();
    CHECK(timing::update_from_live(tower, 62.0, cfg));
    CHECK(tower.row(1).out && std::strcmp(tower.row(1).gap, "Out") == 0);
    CHECK(tower.row(2).penalty);
    CHECK(tower.session().header == timing::Session::safety_car);

    f.session().safety_car = 0;
    f.session().have |= SWOFTY_LIVE_SHAVE_YELLOW;
    f.session().yellow_sectors = 0b101;
    f.session().chequered = 0;
    timing::Session s;
    timing::from_live(f.session(), s);
    CHECK(s.header == timing::Session::yellow && s.yellow_sectors == 5 && !s.yellow_sector_unknown);
    f.session().chequered = 1;
    timing::from_live(f.session(), s);
    CHECK(s.header == timing::Session::chequered);

    cfg.rows = 2;
    cfg.mode = timing::Config::Mode::compact;
    f.advance();
    CHECK(timing::update_from_live(tower, 63.0, cfg));
    CHECK(tower.rows() == 2 && tower.start() == 1);

    f.session().game_mode = SWOFTY_LIVE_MODE_TIME_TRIAL;
    CHECK(timing::update_from_live(tower, 64.0, cfg));
    CHECK(!tower.on() && tower.session().time_trial);

    timing::Car c;
    timing::from_live(f.car(5), c);
    CHECK(c.index == 5 && c.status == 4 && c.best_lap_ms == -1 && c.code[0] == 0);

    f.set_available(false);
    CHECK(!timing::update_from_live(tower, 65.0, cfg));
    CHECK(!tower.on());

    std::printf(g_fail ? "FAILED %d\n" : "timing live ok\n", g_fail);
    return g_fail ? 1 : 0;
}
