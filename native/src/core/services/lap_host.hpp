#pragma once
#include <vector>

#include "../hud_drawlist.hpp"
#include "../presence_id_logic.hpp"

// The core's calls into the attached lap module (services/lap_host.cpp); each does nothing, or answers no, without one.
namespace runtime {

struct CoreMixins;

namespace lap_host {

void tick();
void session_enter();
void session_watch(bool on);
void session_end();
void lap_line(int car);
bool recording();
bool board_session();
bool player_id(presence_id::Id &out);
void screen(const char *name);
// The notice line: one line of text the module sets, painted at the top of the overlay frame.
bool notice_wanted();
bool notice_drawing();
void notice_paint(hud::Frame &frame, uint64_t now);

}

std::vector<CoreMixins> lap_host_mixins();
void lap_host_commit();

}
