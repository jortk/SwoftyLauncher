// Host unit test for the grid rules per box (box_grid_logic.hpp): qualifying scoring (best_lap, best_of, aggregate),
// the 107 % rule, the position base, drivers without a time, and the row pattern layout (design only).
// run_events_test.sh builds and runs it.
#include <cmath>
#include "../src/core/box_grid_logic.hpp"

#include <cstdio>
#include <string>
#include <vector>

namespace bg = runtime::box_grid;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static bg::Driver drv(uint32_t key, int32_t pos, std::vector<float> times)
{
    bg::Driver d;
    d.key = key;
    d.game_position = pos;
    d.session_best = std::move(times);
    return d;
}

static bg::Rule rule(bg::Scoring s, bool r107 = false)
{
    bg::Rule r;
    r.qualifying = s;
    r.rule_107 = r107;
    return r;
}

static void scoring_tests()
{
    // Two sessions (Friday, Saturday). The game ranked by Saturday: A, B, C, D (1-based positions).
    std::vector<bg::Driver> d = {
        drv(1, 1, {80.0f, 79.0f}),   // A: best 79.0, sum 159.0
        drv(2, 2, {78.5f, 79.5f}),   // B: best 78.5, sum 158.0
        drv(3, 3, {81.0f, 79.8f}),   // C: best 79.8, sum 160.8
        drv(4, 4, {77.9f, 0.0f}),    // D: Friday only, best 77.9
    };
    auto lap = bg::rank(d, rule(bg::Scoring::best_lap));
    expect(lap.positions == std::vector<int32_t>({1, 2, 3, 4}) && !lap.changed, "best_lap keeps the game's order");

    auto of = bg::rank(d, rule(bg::Scoring::best_of));
    expect(of.positions == std::vector<int32_t>({3, 2, 4, 1}) && of.changed,
           "best_of ranks the best lap of either session (D's Friday lap takes pole)");
    expect(of.pole == 77.9f, "best_of pole is the best lap of the weekend");

    auto agg = bg::rank(d, rule(bg::Scoring::aggregate));
    expect(agg.positions == std::vector<int32_t>({2, 1, 3, 4}),
           "aggregate ranks the sum of two sessions; a driver with one session follows the complete aggregates");
    expect(agg.classes[3] == bg::Class::partial && !agg.aggregate_one_session, "one-session driver is partial");

    std::vector<bg::Driver> one = {drv(1, 0, {80.0f}), drv(2, 1, {79.0f}), drv(3, 2, {81.0f})};
    auto agg1 = bg::rank(one, rule(bg::Scoring::aggregate));
    expect(agg1.aggregate_one_session && agg1.positions == std::vector<int32_t>({1, 0, 2}),
           "aggregate on a one-session weekend ranks by the best lap (0-based positions kept 0-based)");
}

static void rule_107_tests()
{
    // Pole 80.0 -> limit 85.6. E at 86.0 is outside, F has no time.
    std::vector<bg::Driver> d = {
        drv(10, 1, {80.0f}), drv(11, 2, {86.0f}), drv(12, 3, {85.5f}), drv(13, 4, {0.f}), drv(14, 5, {82.0f}),
    };
    auto r = bg::rank(d, rule(bg::Scoring::best_lap, true));
    expect(r.limit > 85.59f && r.limit < 85.61f, "107 % limit is 1.07 x pole");
    expect(r.classes[1] == bg::Class::outside_107 && r.classes[2] == bg::Class::ranked, "86.0 outside, 85.5 inside");
    expect(r.positions == std::vector<int32_t>({1, 4, 2, 5, 3}),
           "best_lap + 107 %: the slow car goes behind every car inside the limit, ahead of the car without a time");

    auto off = bg::rank(d, rule(bg::Scoring::best_lap, false));
    expect(!off.changed && off.limit == 0.f, "rule_107 off changes nothing under best_lap");

    auto of = bg::rank(d, rule(bg::Scoring::best_of, true));
    expect(of.positions == std::vector<int32_t>({1, 4, 3, 5, 2}), "best_of + 107 %: ranked by time, slow car behind");

    std::vector<bg::Driver> none = {drv(1, 1, {0.f}), drv(2, 2, {0.f})};
    auto n = bg::rank(none, rule(bg::Scoring::best_of, true));
    expect(!n.changed && n.pole == 0.f, "no times at all: the game's order stays");

    std::vector<bg::Driver> uncl = {drv(1, 1, {80.f}), drv(2, bg::UNCLASSIFIED, {0.f}), drv(3, bg::UNCLASSIFIED, {79.f})};
    auto u = bg::rank(uncl, rule(bg::Scoring::best_of));
    expect(u.positions == std::vector<int32_t>({2, bg::UNCLASSIFIED, 1}),
           "a driver with a time is classified; one with neither time nor position stays unclassified");
}

static void row_tests()
{
    auto l = bg::row_layout({3, 2}, 8);
    bool ok = l.size() == 8 && l[0].row == 0 && l[0].cars == 3 && l[2].index == 2 && l[3].row == 1 && l[3].cars == 2 &&
              l[5].row == 2 && l[5].cars == 3 && l[7].row == 2 && l[7].index == 2;
    expect(ok, "3-2-3 layout: rows of 3, 2, 3");
    auto t = bg::row_layout({4, 3}, 9);
    expect(t.size() == 9 && t[8].row == 2 && t[8].cars == 2 && t[8].index == 1, "4-3-4: the last row takes what is left");
    expect(bg::row_layout({3, 0}, 6).empty() && bg::row_layout({}, 6).empty(), "a zero or empty pattern lays out nothing");
    expect(bg::row_offset({0, 0, 2}) == -0.5f && bg::row_offset({0, 1, 2}) == 0.5f, "2-wide rows keep the stock offsets");
    expect(bg::row_offset({0, 0, 3}) == -0.8f && bg::row_offset({0, 1, 3}) == 0.f && bg::row_offset({0, 2, 3}) == 0.8f,
           "3-wide rows span 0.8 spacing each side");
    {
        // A straight stock grid along -z: row k at z = -16k, slots at x = -3 (pole side, 4 m ahead) and +3.
        std::vector<bg::Vec3> stock;
        for (int k = 0; k < 11; k++) {
            stock.push_back({-3.f, 1.f, -16.f * k});
            stock.push_back({3.f, 1.f, -16.f * k - 8.f});
        }
        auto p0 = bg::row_position({3, 2}, stock, 0), p2 = bg::row_position({3, 2}, stock, 2);
        auto p3 = bg::row_position({3, 2}, stock, 3), p5 = bg::row_position({3, 2}, stock, 5);
        auto near = [](float a, float b) { return std::fabs(a - b) < 1e-3f; };
        expect(p0 && near(p0->x, -4.8f) && near(p0->z, -4.f) && near(p0->y, 1.f), "3-2-3: pole on the pole side of row 0");
        expect(p2 && near(p2->x, 4.8f) && near(p2->z, -4.f), "3-2-3: third car of row 0 on the far side");
        expect(p3 && near(p3->x, -3.f) && near(p3->z, -20.f), "3-2-3: row 1 holds 2 cars on stock row 1's line");
        expect(p5 && near(p5->x, -4.8f) && near(p5->z, -36.f), "3-2-3: row 2 is 3 wide again");
        expect(!bg::row_position({}, stock, 0) && !bg::row_position({3, 2}, stock, 40),
               "no pattern or a slot past 31 keeps the track's slot");
        std::vector<bg::Vec3> short_grid(stock.begin(), stock.begin() + 4);
        expect(!bg::row_position({3, 2}, short_grid, 5), "a row the stock grid lacks keeps the track's slot");
        std::vector<bg::Vec3> flat(4, bg::Vec3{0.f, 0.f, 0.f});
        expect(!bg::row_position({3, 2}, flat, 0), "a degenerate stock grid keeps the track's slot");
    }
    bg::Rule r;
    r.rows = {3, 2};
    expect(!r.reranks() && bg::describe(r) == "qualifying best_lap, 107 % rule off, rows 3-2",
           "rows alone do not re-rank; describe names them");
    bg::Scoring s;
    expect(bg::parse_scoring("aggregate", s) && s == bg::Scoring::aggregate && !bg::parse_scoring("best", s),
           "parse_scoring takes the three schema values only");
}

int main()
{
    scoring_tests();
    rule_107_tests();
    row_tests();
    printf("%s\n", fails ? "box_grid_test: FAILED" : "box_grid_test: all passed");
    return fails ? 1 : 0;
}
