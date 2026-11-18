// SPDX-License-Identifier: MIT
#include <string>

#include "hockenheim.hpp"

using namespace hockenheim_test;

static void load(const std::string &addon, const std::string &path, const std::string &rel)
{
    std::string text = slurp(path);
    check(!text.empty() && reg::load_data_file(addon, rel, text) == SWOFTY_ASSET_OK, addon + " " + rel + " loads");
}

static void classic_seasons_2016()
{
    load("swofty.classic-seasons", "test/fixtures/classic-seasons.drivers.json", "registry/drivers.json");
    load("swofty.classic-seasons-2016", "test/fixtures/classic-seasons-2016.content.json", "registry/content.json");
}

static int round_of(const Value &rounds, int round)
{
    for (std::size_t i = 0; i < rounds.size(); i++)
        if (rounds[i]["round"].as_int() == round)
            return static_cast<int>(i);
    return -1;
}

static void check_calendar(reg::Store &st, bool hockenheim)
{
    std::string with = hockenheim ? "with Hockenheim: " : "without Hockenheim: ";
    auto cal = st.get("career.calendar", "swofty.classic-seasons-2016/f1_2016");
    check(cal.status == reg::Status::ok, with + "Classic Seasons 2016's calendar is valid");
    const Value &rounds = cal.value["rounds"];
    check(rounds.size() == (hockenheim ? 19u : 18u),
          with + std::to_string(rounds.size()) + " of 21 rounds (Sochi and Sepang dropped" +
              (hockenheim ? ")" : ", and Hockenheim)"));
    check(round_of(rounds, 4) < 0 && round_of(rounds, 16) < 0, with + "rounds 4 (venue/sochi) and 16 (venue/sepang) dropped");
    int r1 = round_of(rounds, 1);
    check(r1 >= 0 && rounds[r1]["venue"].as_string() == "venue/albert_park", with + "round 1 races at venue/albert_park");
    int r12 = round_of(rounds, 12);
    if (hockenheim)
        check(r12 >= 0 && rounds[r12]["venue"].as_string() == "venue/hockenheimring",
              with + "round 12 races at venue/hockenheimring, no modifier");
    else
        check(r12 < 0, with + "round 12 (venue/hockenheimring) dropped");
    auto pkg = st.get("content.package", "swofty.classic-seasons-2016/f1_2016");
    check(pkg.status == reg::Status::ok, with + "the 2016 package is valid");
    check(pkg.value["calendar"].as_string() == "swofty.classic-seasons-2016/f1_2016", with + "and names that calendar");
}

int main()
{
    {
        auto &st = fresh_store({"swofty.classic-seasons", "swofty.classic-seasons-2016", "mod.hockenheim"});
        core_blocks(st);
        load("mod.hockenheim", "registry/track.json", "registry/track.json");
        load("mod.hockenheim", "registry/layers.json", "registry/layers.json");
        classic_seasons_2016();
        st.end_register();
        for (auto &r : st.refusals())
            check(false, "refused: " + r.registry + " " + r.id + ": " + r.message);
        check_hockenheim(st, "data files");
        check_calendar(st, true);
    }
    {
        auto &st = fresh_store({"swofty.classic-seasons", "swofty.classic-seasons-2016"});
        classic_seasons_2016();
        st.end_register();
        check_calendar(st, false);
    }
    return failures ? 1 : 0;
}
