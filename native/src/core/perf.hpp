#pragma once
#include <atomic>
#include <cstdint>
#include <string>

// Always-on profiler for swofty code on the game's threads. A probe is one hook, callback or API
// entry point; Scope times it with rdtsc into per-thread counters, so the hot path takes no lock
// and no atomic read-modify-write. Pause marks game code running inside a probe (the original of a
// wrapped function) so it is not counted as swofty time. Frames come from the frame index the game
// passes to Streamline (or, while it is not updated, from gaps between flow-state ticks), which also
// paces the on_frame callbacks through frame_claim; the report goes to swofty/perf.log every 15 s and
// at exit, and summary lines go to loader.log every minute: frames and swofty time, VirtualQuery calls
// from swofty modules, the process's CPU use, frame pacing, presents (presented, from the core's Present
// hook), the GPU, the CPU, the other processes and a verdict (pacing_logic.hpp, perf_system.cpp).

namespace perf {

extern std::atomic<bool> g_live;

inline bool live()
{
    return g_live.load(std::memory_order_relaxed);
}

uint32_t probe(const std::string &owner, const std::string &what);
std::string owner_of(const void *code);
void enter(uint32_t id);
void leave();

class Scope {
public:
    explicit Scope(uint32_t id) : on_(id && live())
    {
        if (on_)
            enter(id);
    }
    ~Scope()
    {
        if (on_)
            leave();
    }
    Scope(const Scope &) = delete;
    Scope &operator=(const Scope &) = delete;

private:
    bool on_;
};

class Pause {
public:
    Pause() : on_(live())
    {
        if (on_)
            enter(0);
    }
    ~Pause()
    {
        if (on_)
            leave();
    }
    Pause(const Pause &) = delete;
    Pause &operator=(const Pause &) = delete;

private:
    bool on_;
};

inline uint64_t stamp()
{
    return __builtin_ia32_rdtsc();
}

void presented(void *swapchain, unsigned sync, unsigned flags, long hr, uint64_t entry, uint64_t before, uint64_t after);
void flow_tick();
bool frame_claim();
void frame_release();
void background();
void mark(const char *kind, const std::string &what);
void attach();
void start();
void flush(const char *why);
int open_probes(uint32_t *out, int max);
size_t crash_text(char *out, size_t cap, const uint32_t *open, int depth);

}
