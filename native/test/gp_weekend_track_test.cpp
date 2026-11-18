// Host-native unit test for the weekend track guard (native/src/core/gp_weekend_track_logic.hpp): a weekend track the
// TrackDataSystem holds is left alone; a weekend track past the item vector or on a null item is moved to melbourne, or
// to the first record when melbourne has none, in every championship mode; the two layouts of the 2026-11-12 EA bundles
// (43 slots without Le Mans, 84 with it at 83) replay to a repair of track 47; and the Driver Career round of the
// 2026-11-18 bundles (the suzuka24 slot, stat id 36, its imported track uninstalled) replays to a repair of track 36.
// Built and run by native/test/run_gp_weekend_track_test.sh.

#include "gp_weekend_track_logic.hpp"

#include <cstdio>
#include <initializer_list>

namespace wt = runtime::gp_weekend_track;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

static bool has(const std::string &s, const char *part)
{
    return s.find(part) != std::string::npos;
}

static std::vector<wt::Item> stock_items(std::size_t size)
{
    static const std::initializer_list<std::pair<int, const char *>> live = {
        {0, "melbourne"},   {2, "shanghai"},  {3, "bahrain"},     {4, "catalunya"},  {5, "monaco"},   {6, "montreal"},
        {7, "silverstone"}, {9, "hungaroring"}, {10, "spa_francorchamps"}, {11, "monza"}, {12, "singapore"}, {13, "suzuka"},
        {14, "abu_dhabi"},  {15, "texas"},    {16, "brazil"},     {17, "austria"},   {19, "mexico"},  {20, "baku"},
        {26, "zandvoort"},  {27, "imola"},    {29, "jeddah"},     {30, "miami"},     {31, "las_vegas"}, {32, "losail"},
        {39, "silverstone_reverse"}, {40, "austria_reverse"}, {41, "zandvoort_reverse"}, {42, "madrid"},
    };
    std::vector<wt::Item> items(size);
    for (auto &[id, name] : live)
        if (std::size_t(id) < size)
            items[std::size_t(id)] = {true, name};
    return items;
}

int main()
{
    auto ea = stock_items(43);
    auto lemans = stock_items(84);
    lemans[83] = {true, "le_mans_2017"};

    wt::Decision keep = wt::decide(10, ea);
    expect(!keep.missing && !keep.repair, "a weekend track with a record is left alone");
    expect(!wt::decide(83, lemans).missing, "an added track with its record is left alone");

    wt::Decision past = wt::decide(47, ea);
    expect(past.missing && past.repair && past.to == 0, "bundle 1487/1489: track 47 past 43 slots moves to melbourne");
    wt::Decision hole = wt::decide(47, lemans);
    expect(hole.missing && hole.repair && hole.to == 0, "bundle 1484: track 47 on a null slot of 84 moves to melbourne");
    expect(wt::decide(25, ea).repair, "a stock slot without a record (25) is repaired");
    expect(wt::decide(0xff, ea).repair, "the 0xff placeholder is repaired");
    expect(wt::decide(-1, ea).repair, "a negative id is repaired");

    auto no_melbourne = ea;
    no_melbourne[0] = {};
    wt::Decision first = wt::decide(47, no_melbourne);
    expect(first.repair && first.to == 2, "without melbourne the first record (shanghai) is used");

    auto renamed = ea;
    renamed[0] = {true, "albert_park"};
    renamed[5] = {true, "melbourne"};
    expect(wt::decide(47, renamed).to == 5, "melbourne is found by its record name, not by stat id 0");

    wt::Decision empty = wt::decide(47, {});
    expect(empty.missing && !empty.repair && empty.to == -1, "an empty item vector cannot be repaired");
    std::vector<wt::Item> none(43);
    expect(!wt::decide(47, none).repair, "a vector of null items cannot be repaired");

    wt::Decision career = wt::decide(36, ea);
    expect(career.missing && career.repair && career.to == 0,
           "2026-11-18 bundles: a Driver Career round on the empty suzuka24 slot (36) moves to melbourne");
    auto fuji = ea;
    fuji[36] = {true, "suzuka24"};
    expect(!wt::decide(36, fuji).missing, "the suzuka24 slot with its imported track's record is left alone");

    std::string fixed = wt::line(past, 47, ea.size(), "grand_prix", wt::Site::tyre_allocation, "", "melbourne");
    expect(has(fixed, "grand_prix weekend races track stat id 47,") && has(fixed, "43 record slot(s)") &&
               has(fixed, "now melbourne (stat id 0) so the tyre allocation reads"),
           "the tyre allocation line names the mode, the track, the slots and the replacement");
    std::string session =
        wt::line(career, 36, ea.size(), "driver_career", wt::Site::session_settings, "suzuka24", "melbourne");
    expect(has(session, "driver_career weekend races track stat id 36 (suzuka24)") &&
               has(session, "so the session settings read a track the game has"),
           "the session settings line names the career mode, the slot's label and the reader");
    std::string stuck = wt::line(empty, 47, 0, "grand_prix", wt::Site::tyre_allocation, "", "");
    expect(has(stuck, "no track data record to put in its place") && has(stuck, "aborts in its tyre allocation"),
           "an unrepairable tyre allocation says so");
    std::string stuck_session = wt::line(empty, 36, 0, "my_team", wt::Site::session_settings, "", "");
    expect(has(stuck_session, "my_team weekend") && has(stuck_session, "faults in its session settings"),
           "an unrepairable session settings read says so");
    expect(std::string(wt::reader(wt::Site::session_settings)) == "the session settings", "the session site is named");

    expect(wt::switch_off("0") && wt::switch_off("off") && !wt::switch_off("1") && !wt::switch_off(""),
           "the switch turns off on 0/off only");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
