---
title: Logging
description: Write to swofty/loader.log from a plugin with swofty::log, and configure the copy of the game's own log.
---

# Logging

`swofty::log` writes a `printf`-formatted line to `swofty/loader.log` in the game folder, the one log the loader, swofty_core, the game's own messages and every plugin share.

```cpp [native/src/my_plugin.cpp]
#include <windows.h>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

using swofty::game::FEHomeViewModel;
using swofty::game::NeStr;

namespace {

// Time your own work in swofty/perf.log (pattern from Career Mode Editor's native/src/plugin.cpp)
struct Profiler {
    uint32_t (*probe)(const char *, const char *) = nullptr;
    void (*enter)(uint32_t) = nullptr;
    void (*leave)() = nullptr;
};

const Profiler &profiler()
{
    static const Profiler p = [] {
        Profiler out;
        HMODULE core = GetModuleHandleW(L"swofty_core.dll");
        if (!core)
            return out;
        out.probe = reinterpret_cast<uint32_t (*)(const char *, const char *)>(
            reinterpret_cast<void *>(GetProcAddress(core, "SwoftyPerfProbe")));
        out.enter = reinterpret_cast<void (*)(uint32_t)>(reinterpret_cast<void *>(GetProcAddress(core, "SwoftyPerfEnter")));
        out.leave = reinterpret_cast<void (*)()>(reinterpret_cast<void *>(GetProcAddress(core, "SwoftyPerfLeave")));
        if (!out.probe || !out.enter || !out.leave)
            out = Profiler{}; // a core without them has no report
        return out;
    }();
    return p;
}

int g_logged = 0;

} // namespace

SWOFTY_WRAP(FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    static const uint32_t probe = profiler().probe ? profiler().probe(nullptr, "wrap FeHome_Handle_Command") : 0;
    if (probe)
        profiler().enter(probe);
    uint64_t r = orig(vm, cmd);
    if (probe)
        profiler().leave();
    if (g_logged++ < 8) // a hook can run every frame: cap what it logs
        swofty::log("[my-addon] home command -> %llu", (unsigned long long)r);
    return r;
}

SWOFTY_ON_LOAD
{
    // prefix every line with your addon's name: the log is shared
    swofty::log("[my-addon] loaded, game base 0x%llx", (unsigned long long)swofty::api->module_base());
}

SWOFTY_ADDON()
```

```json [swofty.json]
{
  "schema_version": 1,
  "id": "author.my-addon",
  "name": "My Addon",
  "version": "1.0.0",
  "author": "Your name",
  "kind": ["native"],
  "compat": { "game": "f1_25" },
  "requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
  "operations": { "native": [{ "dll": "native/my_plugin.dll" }] }
}
```

```text [swofty/loader.log]
2026-10-24 23:02:13.420 [  384] addon my_plugin.dll: loaded at 00006fffea2c0000, calling SwoftyAddonInit (API v8)
2026-10-24 23:02:13.421 [  384] [core] addon my_plugin.dll: registered 2 mixin(s)
2026-10-24 23:02:13.421 [  384] addon my_plugin.dll: SwoftyAddonInit returned 1 (ok)
2026-10-24 23:02:15.007 [  384] [my-addon] loaded, game base 0x140000000
2026-10-24 23:02:31.552 [  384] [my-addon] home command -> 1
```

Each line starts with the time and the thread id. Watch the log live in the launcher while the game runs, or run `swofty logs` for a summary of a past session's logs and crashes.

- **Header:** `swofty/mixin.hpp`
- **Table:** `SwoftyApi::log`, API version 1
- **Feature:** `log`
- **Thread:** any

## From a plugin

```cpp
template <class... A> void swofty::log(const char *fmt, A... a); // calls api->log(fmt, a...)
```

```cpp
swofty::log("[my-addon] %d car(s), best lap %.3f s", cars, best);
```

```c
api->log("[my-addon] loaded"); /* from C: the same call through the table */
```

`swofty::log` writes nothing until `SwoftyAddonInit` stores the API in `swofty::api`. Log from `SWOFTY_ON_LOAD` or later, not from a static constructor.

## The log file

The game writes `swofty/loader.log` in its own folder. When the game can't create that file, the loader writes to `%LOCALAPPDATA%\Swofty\state\loader-fallback.log` instead:

- The second line names the file it couldn't open and the Windows error.
- The third line gives the rights the game runs with: the user, the integrity level, whether it's elevated, UAC virtualization and `__COMPAT_LAYER`.
- The session copies it to its folder as `loader.log`.

The loader writes lines from a background thread in batches. Crash lines go out at once:

```text
log: asynchronous writer (batched, crash lines flush at once; SWOFTY_LOG_SYNC=1 writes each line synchronously)
```

## What the log holds

To trace what happened to your addon, look for these lines.

Plugins as they load, with the API version each was built against (the plugin's `SwoftyAddonApi`), and the result of init:

```text
addon progressive_back.dll: loaded at 00006fffea2c0000, calling SwoftyAddonInit (API v8)
addon progressive_back.dll: SwoftyAddonInit returned 1 (ok)
addon career_editor.dll: SwoftyAddonInit returned 0 (FAILED)
addon swofty_trace.dll: loaded at 00006fffea440000, API v1 plugin, SwoftyModInit runs after the mixin commit
```

Signature scans (`api->scan`) and the hit count for each. A scan writes its result only for exactly one hit:

```text
scan .data1 [0F B6 53 20 8D 42 F5 3C 06 77 08 48 89 F1 E8] -> 1 hit at 0x14fa4e018 (base+0xfa4e018)
```

Mixin chains as they install, including every conflict and dropped addon (see [Chains](./mixin#chains)):

```text
[core] wrap FeTile_Route_CareerItem at 0x141725620 (base+0x1725620) installed: swofty_core.dll (career tile route)(100) > progressive_suffix.dll(0)
[core] addon t_conflict_x.dll dropped: it conflicts with another addon (see the conflict line above); none of its mixins install
```

Settings declared in code but missing from `swofty.json` (see [Settings](./settings)):

```text
addon settings: greeting_count is not a slider option of this addon in swofty.json; the code default is used
```

Addon screens that fail to build, with the engine's result code. With `SWOFTY_UI_DIAG=1`, each node the engine rejects, with a reason:

```text
[ui] per-node screen build diagnostics off (set SWOFTY_UI_DIAG=1 or "diagnostics": true in the "ui" block to turn them on); the screen result hook stays on
[ui] screen swofty_addon_0 (flow_options): root 'persistent' built with result 0x0 (ok, the engine inserts it into the screen table next)
```

Crashes, with the exception, registers and a stack named from `symbols.tsv`. The report is named `swofty\crash-<time>.txt`, or by its full path in the data folder's `state` folder when the game folder refuses the write. Calls to `ExitProcess` and `TerminateProcess` include the caller's stack:

```text
[crash] crash report swofty\crash-20261023-092347.txt: unhandled ACCESS_VIOLATION (0xc0000005) reading 0x978 at F1_25.exe+0x15436fda .data1 fn base+0x15436e70+0x16a; stack F1_25.exe+0x15436fda .data1 fn base+0x15436e70+0x16a < F1_25.exe+0x46b1829 .xtls fn base+0x46b1460+0x3c9 < F1_25.exe+0x4629740 .xtls fn base+0x4628190+0x15b0 < F1_25.exe+0x4637a08 .xtls Game_Check_MovingWingNodes+0x18; minidump crash-20261023-092347.dmp, 2.2 MB, thread stacks and registers, written in 2.5 s
[crash] ExitProcess(1) on thread 340, 11 s after load; caller stack:
```

A fast-fail ends the game with exit code `0xc0000409` and skips every exception handler, so the core hooks the paths that end in one before they run: `__report_gsfailure` (a stack cookie check failed), the CRT's invalid-parameter terminate path, the CRT's `abort()` path and `RaiseFailFastException`. The report is written first and the fast-fail then goes ahead. Its reason names the path, the fast-fail code and the exit code, and its `Exception` section carries the code as parameter 0:

```text
[crash] fast-fail FAST_FAIL_STACK_COOKIE_CHECK_FAILURE (2) from __report_gsfailure (a stack cookie check failed); the process would end with exit code 0xc0000409; at F1_25.exe+0x4c1a2f3 .xtls fn base+0x4c1a000+0x2f3
[crash] crash report swofty\crash-20261101-231504.txt: fast-fail FAST_FAIL_STACK_COOKIE_CHECK_FAILURE (2) from __report_gsfailure (a stack cookie check failed); the process would end with exit code 0xc0000409; at ...; minidump crash-20261101-231504.dmp, 2.2 MB, thread stacks and registers, written in 2.4 s
```

```text
== Exception
  STACK_BUFFER_OVERRUN (0xc0000409), flags 0x1, at F1_25.exe+0x4d9245c .xtls Ne_Crt_ReportGsFailure+0x0
  parameter 0: 0x2
  fast-fail code 2: FAST_FAIL_STACK_COOKIE_CHECK_FAILURE
```

Codes the report names: `2` stack cookie check, `5` invalid argument, `7` fatal app exit (`abort()`). `RaiseFailFastException` ends with the code of the exception record it was given, or `0xc0000602` without one. A fast-fail the game's own code does inline, without these paths, still ends with no report; the launcher then names the exit code in the Send Logs summary.

## The game's own log

`swofty_gamelog.dll` copies the engine's own messages into `loader.log`: warnings and errors from every channel by default. To configure that copy, edit `swofty/session.json`:

```jsonc [swofty/session.json]
"game_log": {
  "enabled": true,          // false turns the copy off
  "level": "info",          // off, error, warn (the default) or info
  "channels": "default",    // comma-separated channel patterns, or a list of strings; see below
  "rate": 100,              // lines per second; lines beyond it are dropped and counted
  "repeat": 100,            // times one format string is logged before further repeats are suppressed
  "max_lines": 20000        // lines per session
}
```

`channels` takes patterns, each with an optional `:level`:

- `*` matches any text.
- `default` adds the default rules.
- `verbose` adds info from the UI, flow and ERP loader channels. `"default,verbose"` copies those at info as well.
- `all` matches every channel.

To find the engine's channel names, look in `swofty/addons/core/log_channels.tsv`. The copy logs its settings when it starts, and its limits as they hit:

```text
[gamelog] game log to loader.log: on (level info, from environment), channels 'default,GAME:info', 100 line(s)/s, 100 repeat(s) per format, cap 20000 line(s)
[game] (102 message(s) dropped by the rate limit of 100 per second)
[game] (line cap of 20000 reached; no further game messages are logged)
```

Environment variables override those fields. If you set both, the environment variable takes precedence:

| Variable | Field |
|---|---|
| `SWOFTY_GAME_LOG` | `level` |
| `SWOFTY_GAME_LOG_CHANNELS` | `channels` |
| `SWOFTY_GAME_LOG_RATE` | `rate` |
| `SWOFTY_GAME_LOG_REPEAT` | `repeat` |
| `SWOFTY_GAME_LOG_MAX` | `max_lines` |

## Diagnostic switches

These environment variables change what swofty_core and the loader log. Set them for the game under `"env"` in `"launch"` in `$DATA/config.json`, as in [Advanced launch settings](/guide/linux#advanced-launch-settings):

```jsonc [$DATA/config.json]
"launch": {
  "env": { "SWOFTY_UI_DIAG": "1", "SWOFTY_PERF": "1" }
}
```

- `SWOFTY_UI_DIAG=1`: logs each node of an addon screen as it builds, and every node the engine rejects with a reason. `"diagnostics": true` in the `"ui"` block of `swofty/settings.json` does the same.
- `SWOFTY_LOG_SYNC=1`: writes each `loader.log` line as it's logged. Without it, a background thread writes lines in batches and crash lines go out at once.
- `SWOFTY_DRED=1`: switches on D3D12 Device Removed Extended Data (DRED) before the game creates its GPU device. DRED is on by default, so the first GPU hang of a session already has breadcrumbs: auto-breadcrumbs and page fault reporting cost a small marker write per command list, and without them a hang says only `0x887a0006`. `SWOFTY_DRED=0` turns it off. `loader.log` also gets `[system]` lines at boot (RAM and commit limit, and each GPU's name, vendor and device id, driver version and dedicated and shared memory) and a `[perf] last 60 s: memory:` line each minute with RAM and commit used, the game's working set and private bytes, and each GPU's video memory used against its budget. When DRED was on and the GPU was removed, the crash report has a `GPU device removal (D3D12 DRED)` section: the removal reason, the command lists that did not finish with the last 64 operations around the last one that completed, and for a page fault the faulting GPU address and the existing and recently freed allocations with their names and types. It reads `DRED is on` in `loader.log` as `[dred] on (...)`. Wine and vkd3d may lack the interface; `loader.log` says so and the game goes on.
- `SWOFTY_D3D_DEBUG=1`: switches on the D3D12 debug layer before the game creates its GPU device and logs the first 50 `ERROR` and `CORRUPTION` messages with their message IDs to `loader.log` as `[d3d-debug] #1 ERROR EXECUTION id=...: ...`. The same messages go into a `D3D12 debug layer` section of the crash report. It names the call a driver would otherwise report only as `0x887a0001` (`DXGI_ERROR_INVALID_CALL`). F1 25 ships `d3d12SDKLayers.dll` in its `D3D12` folder, so Windows Graphics Tools is not needed. When no debug layer can be loaded, `loader.log` says so once and the game goes on. The debug layer slows the game. Leave it off unless you are chasing a removed GPU device; it can be combined with `SWOFTY_DRED=1`.
- `SWOFTY_STORAGE_LOG=all`: logs every storage call. Without it, swofty_core logs the first three successful calls of each kind from each addon, then only failures:

  ```text
  [storage] api: swofty.career-editor (career_editor.dll) append journal.jsonl (293 byte(s)) -> ok; further successful calls like this are not logged (failures still are; SWOFTY_STORAGE_LOG=all logs every call)
  ```

- `SWOFTY_RESOLVE=scan`: finds every binding by scanning for its pattern and leaves the scan cache off. Without it, swofty_core checks each pattern at its address on the analysed build first, and keeps the addresses it scanned for in `swofty/cache/scan-cache.tsv`, or in `%LOCALAPPDATA%\Swofty\cache\game\` when the game can't create files in its folder:

  ```text
  [core] binding resolution: 284 of 284 pattern(s) carry their KB address; each is checked there first and scanned only when the bytes differ
  [core] scan cache: 16 address(es) saved to swofty\cache\scan-cache.tsv
  ```

- `SWOFTY_PERF=1`: adds `[perf]` lines for every screen open, each career flow from Entering to its next state, the game log's volume every 10 seconds, each Career Editor section pass, and every hitch in the performance report. Screen opens of 50 ms or more are logged without it:

  ```text
  [perf] screen open 'audio_themes': 0.02 ms (addon screen registration 0.00 ms, engine open 0.02 ms)
  [perf] flow "#04fc8ffa" Entering -> Updating took 3.0 ms
  ```

- `SWOFTY_PROFILE=0`: turns the [performance report](#performance-report) off.
- `SWOFTY_PERF_HITCH_MS=<ms>`: the frame time the performance report counts as a hitch. Default 50.
- `SWOFTY_PERF_SLOW_CALL_MS=<ms>`: the time a single hook or callback takes before the performance report lists it as a slow call. Default 8.
- `SWOFTY_FRAME_CAPTURE=<folder>`: writes the presented frame, overlay included, to `<folder>\frame-NNNN.png` every `SWOFTY_FRAME_CAPTURE_EVERY` seconds (default 10). Under Proton, give a Windows path such as `Z:\mnt\work\shots`. It arms the overlay's present hook even when no addon turns the overlay on, and captures only swapchains the overlay would draw on:

  ```text
  [capture] wrote Z:\mnt\work\.f1-pfx\AA\frames\frame-0001.png (1908x1028, format 28, 39 KB, 24 ms off the render thread)
  ```

- `SWOFTY_OVERLAY_KILL=<list>`: turns overlay parts off: `all`, `paint`, `capture` or `hdr`, comma-separated. `"overlay_kill"` in `swofty/assets.json` does the same.
- `SWOFTY_OVERLAY_FG`: draws the overlay into the game's back buffer before frame generation, so generated frames carry it. On by default when frame generation is on in the game's settings at start. `SWOFTY_OVERLAY_FG=0` or `"overlay_fg": false` in `swofty/assets.json` turns it off, and keeps the device hooks off for a swapchain that names no present queue; `=1` or `true` turns it on whatever the game's setting.
- `SWOFTY_OVERLAY_RIVAL=1`: arms the overlay's `Present` hook while RivaTuner Statistics Server is running. Without it the hook stays off and nothing draws over the game. `"overlay_rival": true` in `swofty/assets.json` does the same.
- `SWOFTY_OVERLAY_NITS=<nits>`: the overlay's paper white on HDR10 and scRGB output, 80 to 1000. Default 350, the level of the game's own HUD white.
- `SWOFTY_KEEP_FOCUS=1`: keeps the game running when its window loses focus. swofty_core drops `WM_KILLFOCUS`, `WM_ACTIVATE(WA_INACTIVE)` and `WM_ACTIVATEAPP(FALSE)` before the game's window procedure sees them, and hands a window that starts without focus one `WM_SETFOCUS`. A race in a parked or hidden window then runs instead of stopping at the pause menu. For test runs only.
- `SWOFTY_POSTFX_CAPTURE=<folder>`: for developing [post-process passes](/reference/addons/postfx). Writes the picture just before and just after the passes to `<folder>\postfx-NN-before.ppm` and `postfx-NN-after.ppm`, at most six pairs, ten seconds apart, the first thirty seconds after the passes start. The pictures have no HUD. Under Proton, give a Windows path:

  ```text
  [postfx] 2 pass(es) wait for the game's tonemap; captures go to Z:\mnt\work\.f1-pfx\A\cap
  ```

- `SWOFTY_POSTFX=0`: turns every post-process pass off for the session.

## Performance report

swofty_core times every hook, site handler, event callback and storage call it runs, and writes the result to `swofty/perf.log` in the game folder every 15 seconds and when the game exits. Each timed call costs about 12 ns. After the game exits, `swofty run` keeps a copy in the session folder, and [Send Logs to Developers](/using/send-logs) sends it with the other logs. Every minute, three lines such as these go to `loader.log`:

```text
[perf] last 60 s: 10029 frame(s) (178 fps, p99 11.9 ms, 0 over 50 ms); swofty 1.417 ms/s on game threads (core flow tick (events, career poll) 1.012 ms/s 60.0 calls/s, core game log (Ne_Log_Thunk) 0.393 ms/s 841.4 calls/s, swofty.progressive api storage read 0.020 ms/s 1.0 calls/s); worst call 0.32 ms (swofty.progressive api storage read)
[perf] last 60 s: VirtualQuery from swofty 18,402 calls 912.4 ms, worst 12.35 ms (game threads 12,000 calls 640.1 ms): core 15,000 calls 800.2 ms, progressive.dll 3,402 calls 112.2 ms; top sites core+0x1a2b3c 9,100 calls 500.3 ms, core+0x2200 4,000 calls 100.0 ms; game exe 120,334 calls 30.1 ms; memory checks: 52,301 page(s) from the working set, 3 from VirtualQuery
[perf] last 60 s: process CPU user 1.42 kernel 0.61 cores; swofty threads 0.081 cores (user 0.061 kernel 0.020) in 7 thread(s): core+0x9a20 (tid 900, via _beginthreadex) 0.050, core+0x6000 (tid 4) 0.020, core+0x5150 (tid 812) 0.012
```

Swofty time is the time spent in swofty_core and addon code, without the game code that a hook calls. The busiest probes show their time and calls per second on the game's threads.

The second line counts the `VirtualQuery` calls that swofty_core, the loader and every DLL under `swofty\` make, and the time they take, kernel time included. Game threads are the threads that swofty code did not start. A site is a return address as `module+0xRVA`. `game exe` counts the game's own calls, when its import table has one. `memory checks` counts the pages that the core's page table looked up in the past minute, for the core's checks of game memory and for the addons' checks through `SwoftyCoreMem`: from the process working set, which costs the same in any heap, or from `VirtualQuery` for pages that are not resident. Pages checked again within a quarter of a second are not counted. `SWOFTY_PERF_VQ=0` turns the counting off.

The third line has the whole process's CPU use and the CPU use of the threads swofty code started, as cores (CPU seconds per second). A thread is named by its start address, or by the code that started it for threads made through `_beginthreadex`, `std::thread` among them.

To name a core RVA, build the same commit with `native/build.sh` after removing `-s` from `STATIC`, then run `llvm-symbolizer --obj=swofty_core.dll 0x<0x180000000 + RVA>`. The code layout does not change without `-s`. Frames are counted from the frame index the game passes to NVIDIA Streamline, and from the flow-state ticks while the game does not update it, as on the pause screen.

`swofty/perf.log` has seven sections:

```text
== Summary
# frame times and slow frames, swofty time in total and per second on game threads and on addon threads,
# the most swofty time in one frame, the slowest single call, the hitch count and the game's CPU use
frame times: p50 31.12 ms, p95 1050.00 ms, p99 1050.00 ms, max 1008 ms
swofty time: 86.7 ms in total, 0.718 ms/s (0.072% of one core); game threads 0.718 ms/s, addon threads 0.000 ms/s
swofty per frame: max 6.12 ms; 20 frame(s) over 1 ms, 1 over 4 ms
worst call: 3.19 ms, core postfx SetName
game process cpu: 1.29 cores on average since the core attached (user 1.19, kernel 0.10)
# the frame pacing, present, GPU, CPU and display lines of the session so far (see Frame pacing below)
pacing: 81204 frame(s) 116 fps; frame times p50 8.62 ms, p99 14.12, p99.9 16.88, max 61.0 ms; ...
presents: 81190 (115.3/s, 1.00 per game frame); intervals p50 8.62 ms, ...
gpu 0: core 2380-2490 MHz (max 2599), ... (since the core attached)
cpu: system load 30-41% (average 36%), ... (since the core attached)
display: the game's window is on \\.\DISPLAY1 (2560x1440 at 144 Hz), ...
verdicts: 14 minute(s): 12 smooth in frame times, 2 stutter, 0 judder; 14 GPU bound, 0 CPU bound; 0 display mismatch; 0 background load
newest verdict: smooth in frame times; GPU bound (3D engine 97% busy)

== Pacing (last 2 minute(s), one row a minute, local time at its end)
# the last 30 minutes, one row each: frame and present times, the time inside Present, the GPU and the busiest core
time       fps  p50 ms  p99 ms p99.9 ms  max ms  uneven spikes pres p99  in pres  gpu MHz  gpu C 3D busy cpu max  verdict
21:14:00   117    8.38   10.12    14.88    21.4    0.8%      0    10.38     0.21     2440     66     97%     74%  smooth, GPU bound
21:15:00   116    8.62   14.12    16.88    61.0    2.1%      2    14.12     0.23     2435     66     97%     76%  stutter, GPU bound

== Addons
# calls, own time, time per second and slowest call for swofty_core and each addon
owner                                   calls      self ms       ms/s   worst ms  busiest
swofty.core                                99          1.3      0.010       0.38  site site_FeFlow_SetStateStatus

== Hooks
# every hook, callback and storage call: calls, total time, average, slowest call and calls per second
owner                        probe                                               calls     total ms     avg us   worst ms    calls/s
core                         game log (Ne_Log_Thunk)                            101612          9.0       0.09       0.75      841.4

== Hitches (last 64 of 125, frames over 50 ms)
# the latest frames over the hitch time, with the screen or flow state and the swofty calls in that frame
00:00:57      1001 ms  swofty    0.55 ms  during "screen loading"; core postfx SetName 0.46 ms, core postfx ResourceBarrier 0.07 ms

== Slow calls (last 0, single calls over 8 ms)
# the latest single calls over the slow call time, with the thread and the screen or flow state

== Timeline (last 20; each row runs until the next one)
# screens, flow states, sessions and career events in order, each with its duration, frames, slow frames,
# slowest frame and swofty time
at          took ms   frames   slow   worst ms  swofty ms  what
00:00:06        280       27      1         96       2.84  screen persistent; screen audio_themes; screen tutorials_masked  [core postfx SetName 1.22 ms, core ui screen loader handler 0.66 ms, core postfx ResourceBarrier 0.48 ms]
```

### Frame pacing

After the three lines above, the core logs six more every minute:

```text
[perf] last 60 s: pacing: 6900 frame(s) 116 fps; frame times p50 8.62 ms, p99 14.12, p99.9 16.88, max 61.0 ms; 2.1% uneven (over 20% and 1 ms off the previous one), change p99 7.12 ms; 2 over 2x the median, 1 over 50 ms, 0 over 250 ms; 2 spike(s) over 2x the running average: 21:14:07 61.0 ms (average 8.5) during "screen garage", 21:14:25 38.0 ms (average 8.6)
[perf] last 60 s: presents: 6900 (115.0/s, 1.00 per game frame); intervals p50 8.62 ms, p99 14.12, p99.9 16.88, max 61.0 ms, 2.1% uneven (over 20% and 1 ms off the previous one), change p99 7.12 ms, 2 over 2x the median; inside Present average 0.23 ms, max 0.28 ms; sync interval 0 (VSync off) in 100%, tearing flag in 100%
[perf] last 60 s: gpu 0: core 2380-2490 MHz (max 2599), memory 2249 MHz, 64-67 C, power 88-92%; 3D engine busy 96-98% (average 97%), the game 96%, other processes 1% (most dwm.exe)
[perf] last 60 s: cpu: system load 30-41% (average 36%), busiest core 70-81% (average 76%); reported clocks 4700 MHz of 4700, limit 4700 MHz; busiest game thread 0.71 cores (tid 8812)
[perf] last 60 s: other processes: 0.62 cores in total; busiest chrome.exe 0.21, discord.exe 0.08; overlay, capture and monitoring tools running: discord.exe, radeonsoftware.exe; overlay modules in the game: discordhook64.dll
[perf] last 60 s: verdict: stutter (1 frame(s) over 50 ms, 2 spike(s) over 2x the running average); GPU bound (3D engine 97% busy); display mismatch (VSync off at 116 presents/s on a 144 Hz display: tearing and uneven motion unless FreeSync or G-Sync is on)
```

`pacing` has the game's frame times from the frame index. A frame is uneven when it is over 20% and over 1 ms longer or shorter than the frame before it. `change p99` is the 99th percentile of that difference. A spike is a frame over twice the running average of the frames before it and over 4 ms above it; the line shows the five longest with their local time and the screen or flow state they fell in. Frames over 250 ms, such as loading screens and the pause screen, are left out of the uneven share, the change, the spikes and the running average, and count under `over 250 ms` instead of `over 50 ms`.

`presents` counts the `Present` and `Present1` calls on the game's swapchain, timed in the core's Present hook: their intervals, with the same measures as `pacing`, the time the call took (with VSync on, the wait for a free back buffer shows here), the sync interval the game passed and the share of presents with the tearing flag. Presents to other swapchains, occluded presents and failed presents, with the last error code, are counted at the end. `per game frame` above 1 means the game presents more often than its frame index advances.

`gpu N` is the GPU the game's swapchain runs on, sampled every 5 seconds. The clocks, temperature, power and fan come from the graphics driver; a driver that gives none logs one `[perf] gpu N: the driver gives no ...` line and the minute line says `clocks and sensors unavailable`. The 3D engine use comes from the Windows GPU Engine performance counters, split into the game and the other processes, with the busiest other process. `[perf] gpu engine use:` says once how it is sampled; a counter read over 250 ms moves sampling to every 30 seconds, and one over 2 seconds turns it off.

`cpu` has the load of the whole system and of its busiest core, sampled every 5 seconds, the clocks Windows reports with their limit (a limit under the maximum is throttling), and the busiest thread that swofty code did not start.

`other processes` has the CPU use of the other processes by executable name, the overlay, capture, monitoring and RGB tools in a known list that are running, and the overlay DLLs in the game process, such as RivaTuner, Discord, Steam, OBS, NVIDIA and ReShade hooks. `[perf] overlay module in the game: <name>` is logged once per module.

`verdict` sums up the minute:

| Verdict | When |
|---|---|
| `stutter` | a frame between 50 and 250 ms, or 6 or more spikes a minute |
| `judder` | 10% or more of the frames, or of the present intervals, uneven |
| `smooth in frame times` | neither of the above |
| `GPU bound` | the 3D engine 95% or more busy |
| `CPU bound` | not GPU bound, and the busiest game thread at 0.9 cores or more |
| `display mismatch` | presents under 95% of the refresh rate divided by the sync interval (the refresh rate itself with VSync off) |
| `background load` | other processes at 1 core or more, or 10% or more of the 3D engine |
| `too few frames to judge` | under 30 frames in the minute |

A minute can be smooth in frame times and still look choppy: `display mismatch` and the `presents` line cover what happens after the game's frame. The core cannot see whether FreeSync or G-Sync is on, so `display mismatch` with VSync off is only tearing and uneven motion without them.

When the game's window moves to another display, changes mode, or its swapchain changes, one line describes it:

```text
[perf] display: the game's window is on \\.\DISPLAY1 (2560x1440 at 144 Hz), covering it without a border (borderless or exclusive fullscreen); swapchain 2560x1440 format 24, 3 buffers, flip discard, windowed, tearing allowed, frame latency waitable
```

At start, after the `[system] gpu` and `[system] ram` lines, the core logs each display, each GPU's driver model and scheduling, the Windows settings that change frame pacing, and the display settings in the game's `hardware_settings_config.xml`:

```text
[system] display 0: \\.\DISPLAY1 (Generic PnP Monitor) 2560x1440 at 144 Hz, primary
[system] gpu 0: WDDM 3.1, hardware-accelerated GPU scheduling on (supported), tearing (variable refresh) presents supported
[system] windows: power plan Balanced, power mode Best performance; game mode on; Game Bar recording off, background capture off; hardware-accelerated GPU scheduling setting on; DirectX graphics settings SwapEffectUpgradeEnable=1; game exe compatibility flags none; process priority normal, affinity 16 of 16 logical processors; timer resolution 1.00 ms (15.62 to 0.50)
[system] game display settings: resolution(width=2560 height=1440 refreshRate=144 fullscreen=false) vsync(enabled=false) framelimit(value=0) nvidiareflex(mode=1)
```

Process names are executable names only; no line has a path or a user name.

### Timing your own work

A plugin times its own work in the report through three functions that `swofty_core.dll` exports. Find them with `GetProcAddress`, as in the example at the top; a core without them has no report.

```cpp
uint32_t SwoftyPerfProbe(const char *owner, const char *what); // the id of a probe; owner nullptr is the calling addon's id
void SwoftyPerfEnter(uint32_t id);                             // starts timing the probe on this thread; id 0 starts game code, which is not counted
void SwoftyPerfLeave(void);                                    // ends the latest SwoftyPerfEnter on this thread
```

```cpp
static const uint32_t probe = profiler().probe("swofty.career-editor", "wrap Sess_GameWeatherSystem_SetupSession");
profiler().enter(probe);
// ... your work ...
profiler().leave();
```

The probe shows in `== Hooks` and in the `[perf]` line under its owner and name.
