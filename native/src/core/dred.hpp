#pragma once
#include <windows.h>

#include <string>
#include <vector>

#include "dred_logic.hpp"

namespace runtime::dred {

void arm(const std::wstring &swofty_dir, void (*log)(const char *, ...), HMODULE self);

// The "GPU device removal" section of a crash report: DRED breadcrumbs and page fault data when a device that
// the game created reports removal, and a note when only the registers show it. `registers_show_removal` is
// true when a register held a DXGI device-removed HRESULT. Empty when neither applies.
std::string section(bool registers_show_removal);

// The ray tracing build each unfinished command list stopped on, as the last section() read it from DRED.
std::vector<BuildStop> build_stops();

}
