// Host-native test for the HUD runtime headers: hud_leaves (typed leaves, groups, v1 fallback, rebuild),
// hud_format, ease, hud_frame, log_csv's front end, settings_struct and timing (tower model and leaves),
// all against the fakes in swofty/fake_hud.hpp. Built and run by native/test/run_hud_osd_test.sh.

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>

#include <swofty/ease.hpp>
#include <swofty/fake_hud.hpp>
#include <swofty/hud_format.hpp>
#include <swofty/hud_frame.hpp>
#include <swofty/hud_leaves.hpp>
#include <swofty/log_csv.hpp>
#include <swofty/settings_struct.hpp>
#include <swofty/timing.hpp>

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

namespace hud = swofty::hud;
namespace ease = swofty::ease;
namespace fmt = swofty::hud::format;
namespace tm_ = swofty::timing;

static void test_leaves_v2()
{
    swofty::fake::Hud core;
    hud::Leaves L{"view_model_data.hud_x_"};
    auto kmh = L.text("kmh", 4);
    auto live = L.boolean("live");
    auto gear = L.one_hot("g", 9);
    auto th = L.level("th", 10);
    auto n = L.integer("n");
    CHECK(L.size() == 1 + 1 + 9 + 10 + 1);
    L.set(kmh, "21234");
    L.set(live, true);
    L.set(gear, 3);
    L.set(th, 4);
    L.set(n, 7);
    CHECK(L.flush() == int(L.size()));
    CHECK(core.text("view_model_data.hud_x_kmh") == "2123");
    CHECK(core.number("view_model_data.hud_x_g3") == 1 && core.number("view_model_data.hud_x_g4") == 0);
    CHECK(core.number("view_model_data.hud_x_th04") == 1 && core.number("view_model_data.hud_x_th05") == 0);
    CHECK(core.last("view_model_data.hud_x_th01").has_value() && !core.last("view_model_data.hud_x_th00").has_value());
    core.clear();
    L.set(gear, 3);
    L.set(th, 4);
    CHECK(L.flush() == 0 && core.writes().empty());
    L.set(gear, 4);
    CHECK(L.flush() == 2);
    CHECK(core.number("view_model_data.hud_x_g3") == 0 && core.number("view_model_data.hud_x_g4") == 1);
    L.set(gear, -1);
    L.flush();
    CHECK(core.number("view_model_data.hud_x_g4") == 0);

    uint32_t gen = hud::Leaves::generation();
    core.clear();
    core.rebuild();
    CHECK(hud::Leaves::generation() == gen + 1);
    CHECK(core.writes().size() == L.size());
    hud::LeafStats st;
    CHECK(hud::Leaves::stats(st) && st.registered == L.size());
}

static void test_groups()
{
    swofty::fake::Hud core;
    hud::Leaves L{"view_model_data.hud_g_"};
    struct Row {
        hud::TextLeaf pos;
        hud::BoolLeaf me;
    };
    auto rows = L.rows<Row>("r", 3, 2, [](hud::Leaves &l, int) { return Row{l.text("pos", 8), l.boolean("me")}; });
    CHECK(L.paths()[0] == "view_model_data.hud_g_r00_pos" && L.paths().back() == "view_model_data.hud_g_r02_me");
    for (int i = 0; i < 3; i++) {
        hud::Leaves::Atomic a(L);
        L.set(rows[i].pos, std::to_string(i + 1));
        L.set(rows[i].me, i == 1);
    }
    L.flush();
    auto w = core.writes();
    CHECK(w.size() == 6);
    uint32_t g0 = core.last("view_model_data.hud_g_r00_pos")->group, g1 = core.last("view_model_data.hud_g_r01_pos")->group;
    CHECK(g0 != 0 && g1 != 0 && g0 != g1);
    CHECK(core.last("view_model_data.hud_g_r00_me")->group == g0);
}

static void test_v1_fallback_and_old_core()
{
    {
        swofty::fake::Hud core{swofty::fake::Hud::v1};
        hud::Leaves L{"view_model_data.hud_o_"};
        auto a = L.integer("a");
        L.set(a, 5);
        CHECK(L.flush() == 1);
        CHECK(core.last("view_model_data.hud_o_a")->via_v1);
        CHECK(L.flush() == 0);
        L.set(a, 5);
        CHECK(L.flush() == 0);
        hud::LeafStats st;
        CHECK(!hud::Leaves::stats(st) && hud::Leaves::generation() == 0);
    }
    hud::Leaves none{"view_model_data.hud_n_"};
    none.set(none.boolean("x"), true);
    CHECK(none.flush() == 0);
}

static void test_type_check()
{
    swofty::fake::Hud core;
    hud::Leaves L{"view_model_data.hud_t_"};
    auto a = L.integer("a");
    L.set(a, 1);
    L.flush();
    hud::Leaves M{"view_model_data.hud_t_"};
    M.boolean("a");
    M.set(hud::BoolLeaf{0}, true);
    M.flush();
    CHECK(core.refused() >= 1);
    int32_t v = 1;
    auto *t = hud::detail::leaf_table();
    CHECK(t->set("view_model_data.hud_t_a", SWOFTY_HUD_TEXT, "x") == SWOFTY_ASSET_INVALID);
    CHECK(t->set("view_model_data.hud_t_a", SWOFTY_HUD_INT, &v) == SWOFTY_ASSET_OK);
    CHECK(t->set("view_model_data.nope", SWOFTY_HUD_INT, &v) == SWOFTY_ASSET_INVALID);
}

static void test_format()
{
    CHECK(fmt::speed(83.333f).kph == 300 && fmt::speed(83.333f).mph == 186 && fmt::speed(-3).kph == 0);
    char b[32];
    fmt::gap(5199, b, sizeof b);
    CHECK(!std::strcmp(b, "+5.1"));
    fmt::gap(5199, b, sizeof b, fmt::GapStyle::round_tenths);
    CHECK(!std::strcmp(b, "+5.2"));
    fmt::gap(65333, b, sizeof b);
    CHECK(!std::strcmp(b, "+1:05.3"));
    fmt::gap(65333, b, sizeof b, fmt::GapStyle::thousandths);
    CHECK(!std::strcmp(b, "+1:05.333"));
    fmt::lapped(1, b, sizeof b);
    CHECK(!std::strcmp(b, "+1 LAP"));
    fmt::lapped(3, b, sizeof b);
    CHECK(!std::strcmp(b, "+3 LAPS"));
    CHECK(fmt::lapped(0, b, sizeof b) == 0);
    fmt::lap_time(106123, b, sizeof b);
    CHECK(!std::strcmp(b, "1:46.123"));
    fmt::lap_time(59874, b, sizeof b);
    CHECK(!std::strcmp(b, "0:59.874"));
    fmt::sector_time(28412, b, sizeof b);
    CHECK(!std::strcmp(b, "28.412"));
    CHECK(std::string(fmt::session_label(6)) == "Q2" && std::string(fmt::session_label(15)) == "R" &&
          std::string(fmt::session_label(99)).empty());
    CHECK(fmt::fit("a\xc3\xa9z", 2) == "a");
    CHECK(fmt::fit("abc", 8) == "abc");
    char code[4];
    fmt::driver_code("", "Max Verstappen", code);
    CHECK(!std::strcmp(code, "VER"));
    fmt::driver_code("nor", "Lando Norris", code);
    CHECK(!std::strcmp(code, "NOR"));
}

static void test_ease()
{
    float x = 0;
    for (int i = 0; i < 100; i++)
        x = ease::toward(x, 1, 0.016f, 0.035f);
    CHECK(x > 0.99f);
    CHECK(ease::toward(0, 1, 5.f, 0.5f) < 0.2f);
    ease::Smoothed s(0.1f);
    CHECK(s.update(5, 0.016f) == 5);
    CHECK(s.update(0, 0.016f) > 4);
    ease::Median3 m;
    m.push(1);
    m.push(100);
    CHECK(m.push(2) == 2);
    ease::Fresh<float> f;
    CHECK(f.update(1.f) && !f.update(1.f) && f.update(2.f));
    ease::Hold<char> h;
    CHECK(!h.get(0, 1).has_value());
    h.set('S', 10);
    CHECK(h.get(10.5, 1).value() == 'S' && !h.get(12, 1).has_value());
    ease::Pulse p;
    p.trigger(0, 4);
    CHECK(p.on(3.9) && !p.on(4.1));
    p.trigger(3, 4);
    CHECK(p.on(6.9));
    ease::Window<8> w;
    for (int i = 0; i < 10; i++)
        w.push(i * 0.1, float(i) * 2.f);
    CHECK(std::fabs(*w.rate(0.9, 0.5) - 20.f) < 0.01f);
    CHECK(!ease::Window<8>{}.rate(0, 1).has_value());
    CHECK(ease::level(0.5f, 10) == 5 && ease::level(2.f, 10) == 10 && ease::level(-1.f, 16) == 0);
}

static void test_frame()
{
    swofty::fake::Clock clock;
    hud::Frame f(hud::FrameBudget{1e9, 2});
    hud::Gate g;
    g.session_serial = 1;
    CHECK(!f.begin(g, 0.016));
    g.live = true;
    CHECK(f.begin(g, 0.016) && f.changed_live() && !f.changed_subject());
    hud::Cadence a(1.0), b(1.0), c(1.0);
    CHECK(f.due(a) && f.due(b) && !f.due(c));
    f.end();
    CHECK(f.begin(g, 0.016) && f.due(c) && !f.due(a));
    g.subject_serial = 2;
    f.begin(g, 0.5);
    CHECK(f.changed_subject());
    for (int i = 0; i < 80; i++) {
        f.begin(g, 0.2);
        clock.advance(0.0001);
        if (f.end())
            CHECK(f.cost().frames > 0);
    }
    CHECK(f.watch("tower", 1) && !f.watch("tower", 1) && f.watch("tower", 0));
    CHECK(std::strstr(f.note(), "1 -> 0") != nullptr);
}

static void test_csv_frontend()
{
    swofty::diag::Csv csv("t", {{"a", swofty::diag::Csv::i32}}, 10);
    CHECK(!csv.available());
    CHECK(!csv.row());
    CHECK(csv.dropped() == 0);
}

#define TEST_OPTS(B, I, F, C)                                                                                         \
    B(header, true)                                                                                                    \
    I(rows, 5, 3, 22)                                                                                                  \
    F(opacity, 1.0f, 0.0f, 1.0f)                                                                                       \
    C(mode, Mode, Mode::compact, "full", "compact", "top")
enum class Mode { full, compact, top };
SWOFTY_OPTIONS(TestOptions, TEST_OPTS);

static void test_settings()
{
    auto o = TestOptions::read();
    CHECK(o.header && o.rows == 5 && o.opacity == 1.0f && o.mode == Mode::compact);
    uint32_t seen = 0;
    CHECK(!TestOptions::changed(seen));
    TestOptions::changes()++;
    CHECK(TestOptions::changed(seen) && !TestOptions::changed(seen));
}

static tm_::Car car(int idx, int pos, int lap, int64_t ahead, int64_t leader)
{
    tm_::Car c;
    c.index = uint8_t(idx);
    c.position = uint8_t(pos);
    c.status = tm_::running;
    c.lap = lap;
    c.gap_ahead_ms = ahead;
    c.gap_leader_ms = leader;
    c.gap_ahead_valid = ahead >= 0;
    c.gap_leader_valid = leader >= 0;
    c.tyre = 'M';
    return c;
}

static void test_tower()
{
    tm_::Session s;
    s.type = 15;
    tm_::Config cfg;
    cfg.mode = tm_::Config::Mode::compact;
    cfg.rows = 5;
    std::vector<tm_::Car> order;
    for (int i = 0; i < 10; i++)
        order.push_back(car(i + 10, i + 1, 3, i * 1500, i * 1500 + 300));
    order[4].gap_ahead_valid = 0;
    order[4].in_pit = 1;
    order[5].gap_ahead_valid = 0;
    order[5].lap = 2;
    order[6].status = tm_::dsq;
    tm_::Tower t;
    t.update(s, order, 15, 0.0, cfg);
    CHECK(t.on() && t.count() == 10 && t.rows() == 5 && t.start() == 3);
    CHECK(t.row(0).car == 13 && t.row(2).me && t.row(2).car == 15);
    CHECK(t.row_of_car(15) == 2 && t.row_of_car(10) == -1);
    CHECK(std::string(t.row(1).gap) == "PIT");
    CHECK(std::string(t.row(2).gap) == "+1 LAP");
    CHECK(std::string(t.row(3).gap) == "DSQ" && t.row(3).out);
    CHECK(std::string(t.row(4).gap) == "+10.5" && t.row(4).tyre == 'M');
    t.update(s, order, 10, 0.1, cfg);
    CHECK(t.start() == 0 && t.row(0).leader && std::string(t.row(0).gap) == "Interval");
    cfg.gap = tm_::Config::Gap::leader;
    t.update(s, order, 10, 0.2, cfg);
    CHECK(t.lead_gaps() && std::string(t.row(0).gap) == "Leader" && std::string(t.row(1).gap) == "+1.8");

    std::swap(order[1], order[2]);
    order[1].position = 2;
    order[2].position = 3;
    t.update(s, order, 10, 1.0, cfg);
    const tm_::Row *r1 = &t.row(1), *r2 = &t.row(2);
    CHECK(r1->up && !r1->down && r2->down);
    t.update(s, order, 10, 6.0, cfg);
    CHECK(!t.row(1).up && !t.row(2).down);

    auto ahead = tm_::car_ahead(order, 10);
    CHECK(ahead && *ahead == 12);
    auto behind = tm_::car_behind(order, 19);
    CHECK(!behind);
    CHECK(*tm_::car_ahead(order, 12) == 10);

    s.type = 6;
    order[0].best_lap_ms = 90000;
    order[2].best_lap_ms = 90500;
    t.update(s, order, 10, 7.0, cfg);
    CHECK(t.timed() && std::string(t.row(0).gap) == "1:30.000" && std::string(t.row(1).gap).empty() && t.row(1).gap_na);
    CHECK(std::string(t.row(2).gap) == "+0.5" || std::string(t.row(1).gap) == "+0.5");

    s.time_trial = true;
    t.update(s, order, 10, 8.0, cfg);
    CHECK(!t.on() && t.rows() == 0);
}

static void test_tower_leaves()
{
    swofty::fake::Hud core;
    hud::Leaves L{"view_model_data.hud_tt_"};
    tm_::TowerLeaves leaves(L);
    CHECK(L.size() == 18 + 24 + 22 * (22 + 24));
    tm_::Session s;
    s.type = 15;
    s.header = tm_::Session::safety_car;
    tm_::Config cfg;
    std::vector<tm_::Car> order;
    for (int i = 0; i < 6; i++)
        order.push_back(car(i, i + 1, 3, i * 1000, i * 1000));
    tm_::Tower t;
    t.update(s, order, 2, 0, cfg);
    leaves.publish(t);
    CHECK(L.flush() == int(L.size()));
    CHECK(core.number("view_model_data.hud_tt_on") == 1 && core.number("view_model_data.hud_tt_hs") == 1);
    CHECK(core.number("view_model_data.hud_tt_sc") == 1 && core.text("view_model_data.hud_tt_sess") == "R");
    CHECK(core.text("view_model_data.hud_tt_lap") == "4");
    CHECK(core.number("view_model_data.hud_tt_r02_me") == 1 && core.number("view_model_data.hud_tt_r00_vid") == 0);
    CHECK(core.number("view_model_data.hud_tt_r00_c00") == 1 && core.number("view_model_data.hud_tt_r00_c01") == 0);
    CHECK(core.number("view_model_data.hud_tt_c03_row") == 3);
    CHECK(core.text("view_model_data.hud_tt_r01_tyre") == "M" && core.number("view_model_data.hud_tt_r01_tM") == 1);
    core.clear();
    leaves.publish(t);
    CHECK(L.flush() == 0);
    s.header = tm_::Session::red;
    t.update(s, order, 2, 0.1, cfg);
    leaves.publish(t);
    CHECK(L.flush() == 3);
}

int main()
{
    test_leaves_v2();
    test_groups();
    test_v1_fallback_and_old_core();
    test_type_check();
    test_format();
    test_ease();
    test_frame();
    test_csv_frontend();
    test_settings();
    test_tower();
    test_tower_leaves();
    if (g_fail)
        return 1;
    std::printf("hud osd test: ok\n");
    return 0;
}
