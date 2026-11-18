#pragma once

#include "pacing_logic.hpp"

#include <string>
#include <vector>

// The machine around the game for the profiler (perf.cpp): the displays and the game's window and swapchain, each GPU's
// WDDM version, scheduling, clocks, sensors and 3D engine use, the CPU's load and clocks, the Windows settings that change
// frame pacing, and the other processes and overlay modules. Nothing here needs admin rights or a library the game does
// not ship with Windows; every call but learn_chain runs on the profiler's writer thread.

namespace perf::sys {

struct Minute {
    pace::GpuMinute gpu;
    unsigned gpu_index = 0;
    pace::SysCpuMinute cpu;
    double refresh = 0;
    std::string display;
    double other_cores = -1;
    std::vector<pace::Hog> hogs;
    std::vector<std::string> tools, modules;
};

void boot();
void learn_chain(void *swapchain);
void sample();
Minute minute(double secs);
bool session(pace::GpuMinute &gpu, unsigned &gpu_index, pace::SysCpuMinute &cpu, std::string &display);

}
