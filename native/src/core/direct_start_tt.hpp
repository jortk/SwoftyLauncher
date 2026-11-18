#pragma once

#include <cstdint>
#include <string>

#include "direct_start_logic.hpp"
#include "json.hpp"

namespace runtime::direct_start::tt {

// The Time Trial for a session request with "mode": "time_trial" ({box, track, team, setup_mode}), polled on
// the flow thread while StateFTUE is held, until it answers done or failed.
Setup setup(const json::Value &request, std::string &why);

// Starts the setup over for a saved replay's session: the same steps, without the direct start's scene changes.
void reset_for_replay();

std::string current_track();

std::string session_track();
// The stat id byte of the Time Trial's track record (what a save slot stores as its track), or -1.
int track_stat();

// The entrant list the Time Trial mode has selected (the request's "box").
// The Time Trial game mode object and its entity, or nulls.
void mode_objects(void *&mode, void *&entity);
std::string player_list();

// [g_Game_Root]'s address and Entity_SetSetupMode's, as the Time Trial setup finds them; 0 on a build where they
// don't resolve.
uintptr_t game_root();
uintptr_t set_setup_mode();

// The team of the player's entrant selection: its label, and with `ref` its team reference. Empty or false when
// nothing is selected or the selected team is not in the list the Time Trial mode holds now.
std::string player_team();
bool player_team(uint32_t &ref, std::string &name);
// The label of a team reference (a participant's team stat id) in the Time Trial mode's teams.
std::string team_of(uint32_t ref);

}
