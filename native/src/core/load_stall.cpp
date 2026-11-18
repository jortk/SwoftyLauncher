#include "load_stall_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mixin.hpp>

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>

// Load-stall watchdog. A load that stops moving (1.9.9: a Time Trial at Suzuka sat on loading_transitions for three
// minutes after "State launch exited", rendering at 242 fps, with no StateLoadSession) leaves nothing in the log, so
// this thread checks every second whether a load has stopped (load_stall_logic.hpp) and asks the crash code for a
// report: every thread's stack, unwound with the modules' exception data, to loader.log and to swofty\stall-<time>.txt
// beside the crash reports (crash.cpp, runtime::stall_report). The watch's lock is never held while the report runs.
//
// The game's own hang signals wake the thread at once (load_stall_signal): the game log's deadlock WARN, seen by
// crash.cpp's log tap on the thread that logs it, and the boot watchdog grace holding the game's kill, on the game's
// watchdog thread. Those threads only store the time in an atomic and set an event; the report runs here.
//
// SWOFTY_LOAD_STALL=0 turns it off; SWOFTY_LOAD_STALL_MS sets the loading-screen threshold (tests). The test-only
// SWOFTY_TEST_LOAD_STALL=<seconds> holds the first game thread that moves a flow to Exiting on a loading screen other
// than the boot's splash_transition outside a session load, which is where the 1.9.9 load stopped, for that long;
// with SWOFTY_TEST_LOAD_STALL_AT=<flow> it holds the first thread that moves that flow to Entering instead
// (StateSession: where the 1.9.x first-session loads hang until the game logs its deadlock WARN).
// SWOFTY_TEST_LOAD_STALL_REPORT=1 writes one report at once, before setup returns (the Wine test).

namespace runtime {
bool stall_report(int index, int max, const char *why, const char *state);
}

namespace {

namespace ls = runtime::load_stall;

std::mutex lock;
ls::Watch watch;
std::atomic<bool> armed{false};
uint64_t test_hold_ms = 0;
char test_hold_at[ls::kName];
std::atomic<bool> test_held{false};
std::atomic<DWORD> test_thread{0};
HANDLE wake;
std::atomic<uint64_t> signal_at[ls::kTriggers];
std::atomic<uint64_t> signal_ms[ls::kTriggers];

uint64_t env_number(const char *name, uint64_t fallback)
{
    char v[32] = {};
    DWORD n = GetEnvironmentVariableA(name, v, sizeof v);
    if (!n || n >= sizeof v)
        return fallback;
    char *end = nullptr;
    unsigned long long x = std::strtoull(v, &end, 10);
    return end && end != v && !*end ? uint64_t(x) : fallback;
}

__attribute__((noinline)) void load_stall_test_hold(uint64_t ms)
{
    HANDLE never = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    ULONGLONG until = GetTickCount64() + ms;
    for (ULONGLONG now = GetTickCount64(); now < until; now = GetTickCount64()) {
        DWORD step = DWORD(std::min<ULONGLONG>(until - now, 1000));
        if (never)
            WaitForSingleObject(never, step);
        else
            Sleep(step);
    }
    if (never)
        CloseHandle(never);
}

HANDLE test_report_done;

DWORD WINAPI loop(void *)
{
    static char state[8192];
    static char why[512];
    if (test_report_done) {
        {
            std::lock_guard<std::mutex> g(lock);
            watch.describe(state, sizeof state, GetTickCount64());
        }
        runtime::stall_report(1, 1, "test: SWOFTY_TEST_LOAD_STALL_REPORT asked for this report", state);
        SetEvent(test_report_done);
    }
    for (;;) {
        if (wake)
            WaitForSingleObject(wake, 1000);
        else
            Sleep(1000);
        ls::Due d;
        {
            std::lock_guard<std::mutex> g(lock);
            for (uint32_t k = 0; k < ls::kTriggers; k++)
                if (uint64_t at = signal_at[k].exchange(0))
                    watch.game(k, at, signal_ms[k].load());
            uint64_t now = GetTickCount64();
            d = watch.poll(now);
            if (d.report) {
                size_t n = watch.describe(state, sizeof state, now);
                if (DWORD t = test_thread.load())
                    std::snprintf(state + n, sizeof state - n,
                                  "test: SWOFTY_TEST_LOAD_STALL is holding thread %lu in load_stall_test_hold\n",
                                  (unsigned long)t);
                ls::headline(d, why, sizeof why);
            }
        }
        if (d.report)
            runtime::stall_report(d.index, ls::kReportsPerStall, why, state);
    }
}

}

namespace runtime {

void load_stall_signal(uint32_t trigger, uint64_t detail_ms)
{
    if (trigger >= ls::kTriggers || !armed.load(std::memory_order_relaxed))
        return;
    signal_ms[trigger].store(detail_ms);
    uint64_t none = 0;
    if (signal_at[trigger].compare_exchange_strong(none, GetTickCount64()) && wake)
        SetEvent(wake);
}

void load_stall_game_line(const char *line)
{
    if (ls::game_deadlock_line(line))
        load_stall_signal(ls::GameDeadlock, 0);
}

void load_stall_flow(uint32_t hash, uint32_t to, const char *name)
{
    if (!armed.load(std::memory_order_relaxed))
        return;
    bool hold = false;
    {
        std::lock_guard<std::mutex> g(lock);
        watch.flow(hash, to, name, GetTickCount64());
        bool at = test_hold_at[0] ? to == ls::Entering && name && !std::strcmp(name, test_hold_at)
                                  : to == ls::Exiting && watch.in_loading_screen() && !watch.in_boot_loading_screen() &&
                                        !watch.session_flow();
        hold = test_hold_ms && at && !test_held.exchange(true);
    }
    if (!hold)
        return;
    test_thread.store(GetCurrentThreadId());
    swofty::log("[stall] test: SWOFTY_TEST_LOAD_STALL holds thread %lu in flow \"%s\" %s for %llu s",
                (unsigned long)GetCurrentThreadId(), name, ls::status_name(to),
                (unsigned long long)(test_hold_ms / 1000));
    load_stall_test_hold(test_hold_ms);
    test_thread.store(0);
    swofty::log("[stall] test: thread %lu released", (unsigned long)GetCurrentThreadId());
}

void load_stall_screen(const char *name)
{
    if (!armed.load(std::memory_order_relaxed))
        return;
    std::lock_guard<std::mutex> g(lock);
    watch.screen(name, GetTickCount64());
}

void load_stall_setup()
{
    static bool done;
    if (done)
        return;
    done = true;
    if (!env_number("SWOFTY_LOAD_STALL", 1)) {
        swofty::log("[stall] load-stall watchdog off (SWOFTY_LOAD_STALL=0)");
        return;
    }
    {
        std::lock_guard<std::mutex> g(lock);
        watch.loading_ms = std::max<uint64_t>(1000, env_number("SWOFTY_LOAD_STALL_MS", ls::kLoadingMs));
    }
    test_hold_ms = env_number("SWOFTY_TEST_LOAD_STALL", 0) * 1000;
    DWORD at_n = GetEnvironmentVariableA("SWOFTY_TEST_LOAD_STALL_AT", test_hold_at, sizeof test_hold_at);
    if (!at_n || at_n >= sizeof test_hold_at)
        test_hold_at[0] = 0;
    wake = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (env_number("SWOFTY_TEST_LOAD_STALL_REPORT", 0))
        test_report_done = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    HANDLE t = CreateThread(nullptr, 256 << 10, loop, nullptr, STACK_SIZE_PARAM_IS_A_RESERVATION | CREATE_SUSPENDED,
                            nullptr);
    if (!t) {
        swofty::log("[stall] load-stall watchdog not started: CreateThread failed (error %lu)", GetLastError());
        return;
    }
    using Describe = HRESULT(WINAPI *)(HANDLE, PCWSTR);
    HMODULE kb = GetModuleHandleW(L"kernelbase.dll");
    if (auto describe = reinterpret_cast<Describe>(
            reinterpret_cast<void *>(GetProcAddress(kb ? kb : GetModuleHandleW(L"kernel32.dll"), "SetThreadDescription"))))
        describe(t, L"swofty load-stall watchdog");
    ResumeThread(t);
    CloseHandle(t);
    armed.store(true);
    if (test_report_done && WaitForSingleObject(test_report_done, 20000) != WAIT_OBJECT_0)
        swofty::log("[stall] test: the SWOFTY_TEST_LOAD_STALL_REPORT report did not finish in 20 s");
    swofty::log("[stall] load-stall watchdog armed: a report of every thread's stack when a load sits still %llu s on a "
                "loading screen or %llu s in a session load or unload, again at %llux, and at once when the game logs "
                "its deadlock warning or its watchdog kill is held, again %llu s later; at most %d per process%s%s%s",
                (unsigned long long)(watch.loading_ms / 1000), (unsigned long long)(ls::kSessionLoadMs / 1000),
                (unsigned long long)ls::kRepeatFactor, (unsigned long long)(ls::kFollowUpMs / 1000), ls::kMaxReports,
                test_hold_ms ? "; SWOFTY_TEST_LOAD_STALL is set" : "", test_hold_ms && test_hold_at[0] ? " at " : "",
                test_hold_ms && test_hold_at[0] ? test_hold_at : "");
}

}
