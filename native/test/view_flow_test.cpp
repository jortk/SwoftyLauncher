// Host unit test for the view tracker's flow table (view_flow_logic.hpp). The hashes are the ones a real run
// logged: session 20261105-215752 entered Instant Replay as flow node 24941c93 and logged no
// djb2("SessionStateViewInstantReplay") (203f3b13).
#include "../src/core/view_flow_logic.hpp"

#include <cstdio>
#include <string>

using namespace view_flow;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

int main()
{
    expect(group_of(0x24941c93) == G_REPLAY, "instant replay node 24941c93 is a replay state");
    expect(group_of(djb2("SessionStateViewInstantReplay")) == 0, "the class name hash 203f3b13 is not a flow node");
    expect(group_of(0x89b82f18) == G_REPLAY, "highlights replay node 89b82f18 is a replay state");
    expect(group_of(0x5764b973) == G_REPLAY, "post race replay node 5764b973 is a replay state");
    expect(group_of(0xba16bc16) == G_DRIVING, "InGame node ba16bc16 is the driving state");
    expect(group_of(0x1505f42a) == G_SESSION, "StateSession 1505f42a is the session state");
    expect(group_of(0x963b35bf) == G_CUTSCENE, "Cutscene node 963b35bf is a cutscene state");
    expect(group_of(0xb55facbe) == G_SPECTATE, "Spectate node b55facbe is a spectating state");
    expect(group_of(0x835af69b) == G_REPLAY, "LoadReplaySession 835af69b is a replay state");
    for (size_t i = 0; i < TRACKED_N; i++)
        for (size_t j = i + 1; j < TRACKED_N; j++)
            if (TRACKED[i].hash == TRACKED[j].hash)
                expect(false, "duplicate hash " + std::to_string(i) + "/" + std::to_string(j));
    printf("%s\n", fails ? "view flow test: FAILED" : "view flow test: ok");
    return fails ? 1 : 0;
}
