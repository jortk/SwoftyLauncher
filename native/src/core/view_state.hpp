#pragma once
#include <cstdint>

namespace view {

enum Kind : uint32_t { UNKNOWN = 0, COCKPIT, TCAM, TCAM_OFFSET, CHASE_NEAR, CHASE_FAR, OTHER };
enum Flag : uint32_t { PAUSED = 1, REPLAY = 2, PHOTO = 4, MENU = 8, CUTSCENE = 16, SPECTATING = 32 };

struct State {
    uint32_t kind = UNKNOWN, flags = 0;
    bool valid = false;
    uint32_t serial = 0;
};

State current();
uint32_t pause_reasons();
void setup();
void flow(uint32_t hash, uint32_t to);

}
