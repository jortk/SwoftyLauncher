// Host-native unit test for the core's direct start (native/src/core/direct_start_logic.hpp).
// Built and run by native/test/run_direct_start_test.sh.

#include "direct_start_logic.hpp"

#include <cstdio>
#include <vector>

namespace ds = runtime::direct_start;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

constexpr uint32_t FTUE_DONE = 0x0002b5d6;

int main()
{
    ds::Driver d;
    expect(d.on_update(ds::STATE_FTUE, FTUE_DONE, false) == FTUE_DONE, "an unarmed start leaves the menu path alone");
    expect(d.on_update(ds::STATE_FRONTEND_WORLD, ds::LINK_STAY, false) == ds::LINK_STAY,
           "an unarmed start forces nothing");
    expect(!d.engage_title(), "an unarmed start leaves the title alone");

    d.arm(ds::session_target());
    expect(d.engage_title(), "an armed start passes the title");
    d.setup_result(ds::Setup::done);
    expect(d.phase == ds::Phase::profile, "a setup result before the profile load is ignored");
    expect(d.on_update(ds::STATE_FTUE, FTUE_DONE, false) == ds::LINK_STAY,
           "the first-run state is held until the profile has loaded");
    expect(d.on_update(ds::STATE_FTUE, ds::LINK_STAY, false) == ds::LINK_STAY, "a first-run state still busy stays");

    d.profile_loaded();
    expect(d.phase == ds::Phase::setup, "the profile load starts the setup");
    expect(d.on_update(ds::STATE_FTUE, FTUE_DONE, false) == ds::LINK_STAY, "the first-run state is held during the setup");
    d.setup_result(ds::Setup::pending);
    expect(d.phase == ds::Phase::setup, "a pending setup keeps waiting");
    d.setup_result(ds::Setup::done);
    expect(d.phase == ds::Phase::leave, "a finished setup leaves the front end");

    expect(d.on_update(ds::STATE_FRONTEND_WORLD, ds::LINK_STAY, true) == ds::LINK_STAY,
           "the front end world is not forced while a child runs");
    expect(d.on_update(ds::STATE_FTUE, FTUE_DONE, false) == ds::LINK_EXIT,
           "the first-run state exits instead of opening the menu");
    expect(d.on_update(ds::STATE_FTUE, FTUE_DONE, false) == FTUE_DONE, "the exit is forced once");
    expect(d.on_update(ds::STATE_ONLINE_FRONTEND, ds::LINK_STAY, false) == ds::LINK_STAY, "another state keeps its link");
    expect(d.on_update(ds::STATE_FRONTEND_WORLD, 0x42b3b163, false) == ds::LINK_SESSION,
           "the front end world leaves with session whatever it picked");
    expect(d.phase == ds::Phase::left && !d.engage_title(), "the start is done after the forced session link");
    expect(d.on_update(ds::STATE_FRONTEND_WORLD, ds::LINK_STAY, false) == ds::LINK_STAY,
           "a later front end world is left alone");

    ds::Driver f;
    f.arm(ds::session_target());
    f.profile_loaded();
    f.setup_result(ds::Setup::failed, "no Grand Prix mode");
    expect(f.phase == ds::Phase::failed && f.why == "no Grand Prix mode", "a failed setup keeps its reason");
    expect(f.on_update(ds::STATE_FTUE, FTUE_DONE, false) == FTUE_DONE, "a failed start lets the menu open");
    expect(!f.engage_title(), "a failed start stops passing the title");

    ds::Driver r;
    r.arm(ds::room_target());
    expect(r.on_update(ds::STATE_FTUE, FTUE_DONE, false) == ds::LINK_STAY,
           "a room target holds the first-run state until the profile has loaded");
    r.profile_loaded();
    expect(r.on_update(ds::STATE_FTUE, FTUE_DONE, false) == ds::LINK_STAY,
           "a room target holds the first-run state while the room is set up");
    r.setup_result(ds::Setup::done);
    expect(r.phase == ds::Phase::left, "a room target is done once its setup is");
    expect(r.on_update(ds::STATE_FTUE, FTUE_DONE, false) == FTUE_DONE, "a set-up room lets the first-run state go home");
    expect(r.on_update(ds::STATE_FRONTEND_WORLD, ds::LINK_STAY, false) == ds::LINK_STAY,
           "a room target forces nothing on the front end world");
    expect(r.on_update(ds::STATE_ONLINE_FRONTEND, ds::LINK_STAY, false) == ds::LINK_STAY,
           "a room target lets home take the lobby link itself");

    expect(ds::link_needed(ds::Phase::profile, true, false, 0, 0, 60000, false) && ds::link_needed(ds::Phase::setup, false, false, 0, 0, 60000, false) &&
               ds::link_needed(ds::Phase::leave, false, false, 0, 0, 60000, false),
           "the link hook works while the start drives the flow");
    expect(!ds::link_needed(ds::Phase::failed, true, false, 0, 0, 60000, false) && !ds::link_needed(ds::Phase::off, true, false, 0, 0, 60000, false),
           "a failed or unarmed start needs no link work");
    expect(!ds::link_needed(ds::Phase::left, false, false, 0, 0, 60000, false), "a room or screen target is done once it left the front end");
    expect(ds::link_needed(ds::Phase::left, true, false, 0, 5000, 60000, false), "a session target waits for its session to start");
    expect(ds::link_needed(ds::Phase::left, true, false, 1000, 1000 + 59999, 60000, false), "and for its screen within the cover's wait");
    expect(!ds::link_needed(ds::Phase::left, true, true, 1000, 2000, 60000, false), "a session target is done once its screen is up");
    expect(!ds::link_needed(ds::Phase::left, true, false, 1000, 1000 + 60000, 60000, false), "or once the cover's wait ran out");
    expect(ds::link_needed(ds::Phase::left, false, true, 1000, 1000000, 60000, true), "the direct start diag keeps the link hook working");

    std::vector<ds::SetupModeOffer> modes = {{"f1_25", true}, {"f1_26", true}, {"swofty_f1", true}};
    expect(ds::pick_setup_mode(modes, "", "swofty_f1") == 2, "the host setup mode is taken when it offers the list");
    modes[2].offers = false;
    modes[0].offers = false;
    expect(ds::pick_setup_mode(modes, "", "swofty_f1") == 1,
           "the first mode offering the list is taken when the host mode does not (the 2026 list under f1_26)");
    modes[1].offers = false;
    expect(ds::pick_setup_mode(modes, "", "swofty_f1") == 2, "the host mode is kept when no mode offers the list");
    expect(ds::pick_setup_mode(modes, "f1_25", "swofty_f1") == 0, "a requested setup mode is taken as it is");
    expect(ds::pick_setup_mode(modes, "f1_99", "swofty_f1") == ds::NO_SETUP_MODE, "a requested mode the game lacks is none");
    expect(ds::pick_setup_mode({{"f1_26", false}}, "", "swofty_f1") == ds::NO_SETUP_MODE,
           "no host mode and no offer is none");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
