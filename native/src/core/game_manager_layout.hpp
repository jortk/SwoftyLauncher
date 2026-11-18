#pragma once

#include <cstdint>
#include <string_view>

// The game manager singleton (*g_Game_Manager) has a shorter tail on the EA App build: its qword fields from Steam's
// +0x278 on sit 0x30 lower (the byte flag at Steam +0x263 is EA +0x237; +0xc0, +0x140 and +0x208 are the same on both).
// The entrants system is [g_Game_Manager]+0x290 on Steam and +0x260 on EA: the same Sess_ResolveEntityHandle caller
// reads it as `mov rcx,[rax+0x290]` at Steam 0x14022de3f and `mov rcx,[rax+0x260]` at EA 0x14022e5cf, and the 146 Steam
// loads of +0x290 match 146 EA loads of +0x260 (the live progress system, Steam +0x2d0, is EA +0x2a0 the same way).
// Reading +0x290 on EA reads the system at Steam +0x2c0, so the entrants tables behind it read as empty.

namespace runtime::game_manager {

constexpr uintptr_t ENTRANTS_SYSTEM = 0x290;
constexpr uintptr_t EA_TAIL = 0x278;
constexpr uintptr_t EA_SHIFT = 0x30;

constexpr uintptr_t tail_field(uintptr_t steam_offset, std::string_view distribution)
{
    return distribution == "ea" && steam_offset >= EA_TAIL ? steam_offset - EA_SHIFT : steam_offset;
}

constexpr uintptr_t entrants_system(std::string_view distribution)
{
    return tail_field(ENTRANTS_SYSTEM, distribution);
}

static_assert(entrants_system("ea") == 0x260 && tail_field(0x2d0, "ea") == 0x2a0);

}
