#pragma once
#include <cstdint>

#include "hud_drawlist.hpp"

namespace runtime::main_menu {

bool enabled();
bool wants_overlay();
bool drawing();
bool arm_patch();
uint8_t add_tiles(void *list, uint8_t career_index, const uint8_t *flags);
void screen(const char *name);
void session_enter();
void tick();
void style_settings(void *item);
void command(void *home_vm, uint32_t command_hash, uint32_t focused_hash);
void paint(hud::Frame &frame, uint64_t now);

}
