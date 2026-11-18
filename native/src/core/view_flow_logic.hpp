#pragma once
#include <cstddef>
#include <cstdint>

#include "career_state.hpp"

// The game hashes a flow node's own name, not its class name: Game_SessionStateViewInstantReplay is the node
// "InstantReplay" (0x24941c93), the driving node is "InGame" (0xba16bc16). Each entry below is the djb2 of a node
// name that a real run logged, either as a "[career] flow" line or in the "[direct] flow node" graph dump.
namespace view_flow {

enum Group : uint32_t { G_DRIVING = 1, G_PAUSE = 2, G_REPLAY = 4, G_CUTSCENE = 8, G_SPECTATE = 16, G_SESSION = 32 };

struct Tracked {
    uint32_t hash;
    uint32_t group;
};

constexpr uint32_t djb2(const char *s)
{
    return runtime::career::djb2(s);
}

constexpr Tracked TRACKED[] = {
    {djb2("StateSession"), G_SESSION},
    {djb2("InGame"), G_DRIVING},
    {djb2("InstantReplay"), G_REPLAY},
    {djb2("ViewReplayHighlights"), G_REPLAY},
    {djb2("PostRaceReplay"), G_REPLAY},
    {djb2("SnapshotReplay"), G_REPLAY},
    {djb2("LoadSnapshot"), G_REPLAY},
    {djb2("QuitReplayHighlights"), G_REPLAY},
    {djb2("SessionStateLoadReplaySession"), G_REPLAY},
    {djb2("SessionStateReplaySessionLoaded"), G_REPLAY},
    {djb2("Cutscene"), G_CUTSCENE},
    {djb2("CutScene"), G_CUTSCENE},
    {djb2("IntroCameras"), G_CUTSCENE},
    {djb2("IntroVideo"), G_CUTSCENE},
    {djb2("DoughnutSequence"), G_CUTSCENE},
    {djb2("WinnerSequence"), G_CUTSCENE},
    {djb2("PostRaceCutscene"), G_CUTSCENE},
    {djb2("ZoomToMonitorCutscene"), G_CUTSCENE},
    {djb2("CinFormationLap"), G_CUTSCENE},
    {djb2("RewardSequence"), G_CUTSCENE},
    {djb2("Spectate"), G_SPECTATE},
};

constexpr size_t TRACKED_N = sizeof TRACKED / sizeof TRACKED[0];
static_assert(TRACKED_N <= 64, "the active set is one word");

constexpr uint32_t GROUP_MASK_ALL = G_DRIVING | G_PAUSE | G_REPLAY | G_CUTSCENE | G_SPECTATE | G_SESSION;

constexpr uint32_t group_of(uint32_t hash)
{
    for (size_t i = 0; i < TRACKED_N; i++)
        if (TRACKED[i].hash == hash)
            return TRACKED[i].group;
    return 0;
}

static_assert(djb2("StateSession") == 0x1505f42au, "state name hashing drifted");
static_assert(djb2("InGame") == 0xba16bc16u, "the driving node moved");
static_assert(djb2("InstantReplay") == 0x24941c93u, "the instant replay node moved");

}
