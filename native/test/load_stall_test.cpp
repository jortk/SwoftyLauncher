// Host-native unit test for the core's load-stall watchdog trigger (native/src/core/load_stall_logic.hpp): when a
// load that has stopped moving gets its reports, replayed from the flow and screen sequences of the 1.9.9 Suzuka
// Time Trial stall (session 20261112-013117), of the 1.9.x first-session hangs the game ends itself with its
// deadlock WARN (cptovius, session 20261112-044919), of the 1.9.15 boot an addon held on splash_transition (ArS3nic,
// session 20261112-121623) and of working loads. Built and run by
// native/test/run_load_stall_test.sh.

#include "load_stall_logic.hpp"

#include <cstdio>
#include <cstring>

namespace ls = runtime::load_stall;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

static void menus(ls::Watch &w, uint64_t t)
{
    w.flow(0x5f9ae18b, ls::Entering, "#5f9ae18b", t);
    w.flow(0x5f9ae18b, ls::Updating, "#5f9ae18b", t + 10);
    w.screen("fe_home", t + 1000);
    w.screen("solo_container", t + 2000);
    w.screen("track_select", t + 3000);
}

static uint64_t first_session(ls::Watch &w, uint64_t t)
{
    w.flow(0x10, ls::Entering, "StateLoadSession", t);
    w.flow(0x10, ls::Updating, "StateLoadSession", t + 367);
    w.flow(0x10, ls::Exiting, "StateLoadSession", t + 11999);
    w.flow(0x10, ls::None, "StateLoadSession", t + 12470);
    w.flow(0x20, ls::Entering, "StateSession", t + 12568);
    return t + 12568;
}

static void boot_to_splash_transition(ls::Watch &w)
{
    w.screen("persistent", 25000);
    w.screen("audio_themes", 25000);
    w.screen("tutorials_masked", 25000);
    w.screen("video_1", 25250);
    w.flow(0xe4ef00ba, ls::Entering, "#e4ef00ba", 25712);
    w.flow(0x290b11ae, ls::Entering, "#290b11ae", 25712);
    w.screen("legal", 25972);
    w.screen("legal_2", 26288);
    w.screen("seizure_warning", 26653);
    w.flow(0xa4d3a4b2, ls::Entering, "#a4d3a4b2", 34619);
    w.screen("splash", 34807);
    w.screen("render_badge", 36558);
    w.flow(0xa4d3a4b2, ls::Exiting, "#a4d3a4b2", 37668);
    w.flow(0xfd9f4f95, ls::Entering, "#fd9f4f95", 37668);
    w.flow(0xfd9f4f95, ls::Exiting, "#fd9f4f95", 37668);
    w.flow(0x04fc8ffa, ls::Entering, "#04fc8ffa", 37668);
    w.screen("splash_transition", 39345);
}

static const char kWarn[] =
    "[game] WARN (warn): The signal timed out, this is unexpected and may indicate a deadlock, see trace above for "
    "Extra Info";

static void stall_start(ls::Watch &w, uint64_t t)
{
    w.flow(0x5f9ae18b, ls::Exiting, "#5f9ae18b", t);
    w.flow(0x455cf081, ls::Entering, "#455cf081", t + 1);
    w.screen("loading_transitions", t + 20);
    w.flow(0x455cf081, ls::Exiting, "#455cf081", t + 30);
}

int main()
{
    {
        ls::Watch w;
        CHECK(!w.poll(0).report);
        CHECK(!w.poll(10'000'000).report);
        CHECK(w.threshold() == 0);
    }
    {
        ls::Watch w;
        menus(w, 1000);
        CHECK(w.threshold() == 0);
        CHECK(!w.poll(3'600'000).report);
    }
    {
        ls::Watch w;
        menus(w, 1000);
        stall_start(w, 10000);
        uint64_t last = 10030;
        CHECK(w.threshold() == ls::kLoadingMs);
        CHECK(!w.poll(last + ls::kLoadingMs - 1).report);
        ls::Due d = w.poll(last + ls::kLoadingMs);
        CHECK(d.report && d.index == 1 && d.stalled_ms == ls::kLoadingMs && d.threshold_ms == ls::kLoadingMs);
        CHECK(!w.poll(last + ls::kLoadingMs + 1).report);
        CHECK(!w.poll(last + 3 * ls::kLoadingMs - 1).report);
        d = w.poll(last + 3 * ls::kLoadingMs);
        CHECK(d.report && d.index == 2);
        CHECK(!w.poll(last + 100 * ls::kLoadingMs).report);
        CHECK(w.reports() == 2);
        char text[2048];
        size_t n = w.describe(text, sizeof text, last + 170000);
        CHECK(n > 0 && n < sizeof text);
        CHECK(std::strstr(text, "newest screen \"loading_transitions\", opened 170.0 s ago (a loading screen)"));
        CHECK(std::strstr(text, "last flow transition: \"#455cf081\" -> Exiting, 170.0 s ago"));
        CHECK(std::strstr(text, "\"#455cf081\" in Exiting for 170.0 s"));
        CHECK(std::strstr(text, "\"#5f9ae18b\" in Exiting"));
        CHECK(std::strstr(text, "session load flow: none"));
    }
    {
        ls::Watch w;
        menus(w, 1000);
        stall_start(w, 10000);
        w.screen("loading", 10500);
        w.flow(0xe2dcf787, ls::Entering, "#e2dcf787", 10550);
        w.flow(0xe2dcf787, ls::Exiting, "#e2dcf787", 10551);
        w.flow(0xe2dcf787, ls::None, "#e2dcf787", 10552);
        w.flow(0x1, ls::Entering, "StateLoadSession", 10560);
        CHECK(w.threshold() == ls::kSessionLoadMs);
        w.flow(0x1, ls::Updating, "StateLoadSession", 10561);
        CHECK(!w.poll(10561 + 169000).report);
        w.screen("render_badge", 10561 + 169000);
        w.flow(0x1, ls::Exiting, "StateLoadSession", 10561 + 170000);
        w.flow(0x1, ls::None, "StateLoadSession", 10561 + 170001);
        w.flow(0x2, ls::Entering, "StateSession", 10561 + 170010);
        w.flow(0x2, ls::Updating, "StateSession", 10561 + 170020);
        w.screen("hud", 10561 + 171000);
        CHECK(w.threshold() == 0);
        CHECK(!w.poll(10561 + 3'600'000).report);
        CHECK(w.reports() == 0);
    }
    {
        ls::Watch w;
        w.flow(0x1, ls::Entering, "StateLoadSession", 0);
        w.flow(0x1, ls::Updating, "StateLoadSession", 5);
        ls::Due d = w.poll(5 + ls::kSessionLoadMs);
        CHECK(d.report && d.index == 1 && d.threshold_ms == ls::kSessionLoadMs);
        char text[2048];
        w.describe(text, sizeof text, 5 + ls::kSessionLoadMs);
        CHECK(std::strstr(text, "session load flow: \"StateLoadSession\" in Updating for 180.0 s"));
    }
    {
        ls::Watch w;
        stall_start(w, 0);
        CHECK(w.poll(30 + ls::kLoadingMs).report);
        w.flow(0x77, ls::Entering, "#00000077", 30 + ls::kLoadingMs + 10);
        CHECK(!w.poll(30 + ls::kLoadingMs + 11).report);
        ls::Due d = w.poll(30 + 2 * ls::kLoadingMs + 10);
        CHECK(d.report && d.index == 1);
        CHECK(w.poll(30 + 4 * ls::kLoadingMs + 10).report);
        CHECK(w.reports() == 3);
        w.screen("loading", 30 + 5 * ls::kLoadingMs);
        CHECK(w.poll(30 + 6 * ls::kLoadingMs).report);
        CHECK(w.reports() == ls::kMaxReports);
        w.screen("loading", 30 + 7 * ls::kLoadingMs);
        CHECK(!w.poll(30 + 20 * ls::kLoadingMs).report);
    }
    {
        ls::Watch w;
        w.loading_ms = 15000;
        stall_start(w, 0);
        CHECK(!w.poll(30 + 14999).report);
        CHECK(w.poll(30 + 15000).report);
        CHECK(w.poll(30 + 45000).report);
    }
    {
        ls::Watch w;
        w.screen("loading_transitions", 0);
        CHECK(!w.poll(0).report);
        CHECK(w.in_loading_screen());
        w.screen("pause", 1);
        CHECK(!w.in_loading_screen() && w.threshold() == 0);
    }
    {
        ls::Watch w;
        boot_to_splash_transition(w);
        CHECK(w.in_loading_screen() && w.in_boot_loading_screen() && w.threshold() == ls::kLoadingMs);
        CHECK(!w.poll(39345 + ls::kLoadingMs - 1).report);
        ls::Due d = w.poll(39345 + ls::kLoadingMs);
        CHECK(d.report && d.index == 1 && d.trigger == ls::Threshold && d.threshold_ms == ls::kLoadingMs);
        CHECK(!w.poll(39345 + ls::kRepeatFactor * ls::kLoadingMs - 1).report);
        d = w.poll(39345 + ls::kRepeatFactor * ls::kLoadingMs);
        CHECK(d.report && d.index == 2);
        CHECK(!w.poll(166730).report && w.reports() == 2);
        char text[1024];
        w.describe(text, sizeof text, 166730);
        CHECK(std::strstr(text, "newest screen \"splash_transition\", opened 127.4 s ago (a loading screen)"));
    }
    {
        ls::Watch w;
        boot_to_splash_transition(w);
        w.flow(0x04fc8ffa, ls::Exiting, "#04fc8ffa", 39345 + 18000);
        w.flow(0x957880c6, ls::Entering, "#957880c6", 39345 + 18000);
        w.screen("playercard", 39345 + 21000);
        CHECK(!w.in_loading_screen() && w.threshold() == 0);
        CHECK(!w.poll(39345 + 21000 + 10 * ls::kLoadingMs).report && w.reports() == 0);
    }
    {
        ls::Watch w;
        boot_to_splash_transition(w);
        w.screen("splash", 40000);
        CHECK(!w.poll(40000 + 10 * ls::kLoadingMs).report);
    }
    {
        ls::Watch w;
        for (uint32_t i = 0; i < 100; i++)
            w.flow(0x1000 + i, ls::Updating, "#menu", i);
        w.flow(0x1, ls::Entering, "StateUnloadSession", 200);
        for (uint32_t i = 0; i < 100; i++)
            w.flow(0x2000 + i, ls::Entering, "#later", 300 + i);
        CHECK(w.session_flow() && !std::strcmp(w.session_flow()->name, "StateUnloadSession"));
        w.flow(0x1, ls::None, "StateUnloadSession", 500);
        CHECK(!w.session_flow());
    }
    {
        ls::Watch w;
        stall_start(w, 0);
        char small[24];
        std::memset(small, 'x', sizeof small);
        size_t n = w.describe(small, sizeof small, 1000);
        CHECK(n < sizeof small && small[n] == 0 && std::strlen(small) == n);
    }
    {
        ls::Watch w;
        menus(w, 1000);
        uint64_t hang = first_session(w, 1'000'000);
        CHECK(w.threshold() == 0);
        CHECK(!w.poll(hang + 15302).report);
        w.game(ls::GameDeadlock, hang + 15302, 0);
        ls::Due d = w.poll(hang + 15302);
        CHECK(d.report && d.index == 1 && d.trigger == ls::GameDeadlock && d.cause == ls::GameDeadlock);
        CHECK(d.stalled_ms == 15302 && d.threshold_ms == 0 && d.signal_ms == 0 && d.moved);
        char line[512];
        size_t n = ls::headline(d, line, sizeof line);
        CHECK(n == std::strlen(line));
        CHECK(!std::strcmp(line, "the game logged \"The signal timed out, this is unexpected and may indicate a "
                                 "deadlock\" 0.0 s ago; the last flow transition or screen open was 15.3 s ago"));
        char text[2048];
        w.describe(text, sizeof text, hang + 15302);
        CHECK(std::strstr(text, "\"StateSession\" in Entering for 15.3 s"));
        CHECK(std::strstr(text, "game hang signal: the game's deadlock warning, 0.0 s ago"));
        CHECK(!w.poll(hang + 15303).report);
        w.game(ls::GameWatchdog, hang + 30169, 30169);
        CHECK(!w.poll(hang + 15302 + ls::kFollowUpMs - 1).report);
        d = w.poll(hang + 15302 + ls::kFollowUpMs);
        CHECK(d.report && d.index == 2 && d.trigger == ls::FollowUp && d.cause == ls::GameDeadlock);
        CHECK(d.previous_ms == ls::kFollowUpMs && d.stalled_ms == 15302 + ls::kFollowUpMs);
        ls::headline(d, line, sizeof line);
        CHECK(!std::strcmp(line, "15.0 s after report 1 (asked for by the game's deadlock warning), nothing has moved; "
                                 "the last flow transition or screen open was 30.3 s ago"));
        CHECK(!w.poll(hang + ls::kSessionLoadMs * 10).report);
        CHECK(w.reports() == 2);
    }
    {
        ls::Watch w;
        uint64_t hang = first_session(w, 1000);
        w.game(ls::GameDeadlock, hang + 15300, 0);
        CHECK(w.poll(hang + 15300 + 400).report);
        w.flow(0x20, ls::Updating, "StateSession", hang + 39280);
        CHECK(!w.poll(hang + 15300 + ls::kFollowUpMs * 3).report);
        w.flow(0x30, ls::Entering, "StateSession", hang + 600000);
        w.game(ls::GameDeadlock, hang + 600000 - 1, 0);
        CHECK(!w.poll(hang + 600000 + 10).report);
        w.game(ls::GameDeadlock, hang + 615300, 0);
        ls::Due d = w.poll(hang + 615300);
        CHECK(d.report && d.index == 1 && d.trigger == ls::GameDeadlock && w.reports() == 2);
        CHECK(w.poll(hang + 615300 + ls::kFollowUpMs).report && w.reports() == 3);
        w.flow(0x20, ls::Updating, "StateSession", hang + 700000);
        w.game(ls::GameWatchdog, hang + 730000, 30000);
        CHECK(w.poll(hang + 730000).report && w.reports() == ls::kMaxReports);
        CHECK(!w.poll(hang + 730000 + ls::kFollowUpMs).report);
        w.flow(0x20, ls::None, "StateSession", hang + 800000);
        w.game(ls::GameDeadlock, hang + 815300, 0);
        CHECK(!w.poll(hang + 815300).report);
    }
    {
        ls::Watch w;
        uint64_t hang = first_session(w, 5000);
        w.game(ls::GameWatchdog, hang + 30169, 30169);
        ls::Due d = w.poll(hang + 30500);
        CHECK(d.report && d.index == 1 && d.trigger == ls::GameWatchdog && d.detail_ms == 30169 && d.signal_ms == 331);
        char line[512];
        ls::headline(d, line, sizeof line);
        CHECK(!std::strcmp(line, "the game's watchdog found the frame tick silent for 30.2 s and the boot watchdog grace "
                                 "is holding its kill; the last flow transition or screen open was 30.5 s ago"));
        d = w.poll(hang + 30500 + ls::kFollowUpMs);
        CHECK(d.report && d.index == 2 && d.trigger == ls::FollowUp && d.cause == ls::GameWatchdog);
        ls::headline(d, line, sizeof line);
        CHECK(std::strstr(line, "(asked for by the game's watchdog)"));
    }
    {
        ls::Watch w;
        stall_start(w, 0);
        ls::Due d = w.poll(30 + ls::kLoadingMs);
        CHECK(d.report && d.index == 1 && d.trigger == ls::Threshold);
        char line[512];
        ls::headline(d, line, sizeof line);
        CHECK(!std::strcmp(line, "the load has not moved for 45.0 s (no flow transition or screen open; threshold 45 s)"));
        w.game(ls::GameDeadlock, 30 + ls::kLoadingMs, 0);
        CHECK(!w.poll(30 + ls::kLoadingMs + 1).report);
        CHECK(w.poll(30 + ls::kLoadingMs + ls::kFollowUpMs).report && w.reports() == 2);
    }
    {
        ls::Watch w;
        stall_start(w, 0);
        CHECK(w.poll(30 + ls::kLoadingMs).report);
        w.game(ls::GameDeadlock, 30 + ls::kLoadingMs + 5000, 0);
        ls::Due d = w.poll(30 + ls::kLoadingMs + 5000);
        CHECK(d.report && d.index == 2 && d.trigger == ls::GameDeadlock);
        CHECK(!w.poll(30 + 10 * ls::kLoadingMs).report && w.reports() == 2);
    }
    {
        ls::Watch w;
        w.game(ls::GameDeadlock, 8000, 0);
        ls::Due d = w.poll(9000);
        CHECK(d.report && d.index == 1 && !d.moved && d.signal_ms == 1000);
        char line[512];
        ls::headline(d, line, sizeof line);
        CHECK(std::strstr(line, "1.0 s ago; no flow transition or screen open seen yet"));
        char small[16];
        size_t n = ls::headline(d, small, sizeof small);
        CHECK(n == sizeof small - 1 && std::strlen(small) == n);
        CHECK(ls::headline(d, nullptr, 0) == 0);
    }
    {
        ls::Watch w;
        stall_start(w, 0);
        w.game(ls::Threshold, 100, 0);
        w.game(ls::FollowUp, 100, 0);
        w.game(7, 100, 0);
        CHECK(!w.poll(200).report);
    }
    CHECK(ls::game_deadlock_line(kWarn));
    CHECK(ls::game_deadlock_line("[game] TEST_WARN (warn): The signal timed out, this is unexpected and may indicate a "
                                 "deadlock, see trace above for Extra Info"));
    CHECK(!ls::game_deadlock_line("[game] WARN (warn): Extra Info queueName=\"<unknown/queue>\", AJCF=0x80000001, JCS=22"));
    CHECK(!ls::game_deadlock_line("[stall]   -    0.001 s [ 1234] game      WARN (warn): The signal timed out, this is "
                                  "unexpected and may indicate a deadlock"));
    CHECK(!ls::game_deadlock_line("[game] WARN (warn): The signal timed out"));
    CHECK(!ls::game_deadlock_line(nullptr));
    CHECK(ls::loading_screen("loading_transitions") && ls::loading_screen("loading") && !ls::loading_screen("hud"));
    CHECK(!ls::loading_screen(nullptr));
    CHECK(ls::loading_screen("splash_transition") && ls::boot_loading_screen("splash_transition"));
    CHECK(!ls::boot_loading_screen("loading_transitions") && !ls::boot_loading_screen(nullptr));
    CHECK(!ls::loading_screen("splash") && !ls::loading_screen("render_badge") && !ls::loading_screen("video_1"));
    CHECK(!ls::loading_screen("seizure_warning") && !ls::loading_screen("frontend_transition"));
    CHECK(ls::session_load_flow("StateLoadSession") && ls::session_load_flow("StateUnloadSession"));
    CHECK(!ls::session_load_flow("StateSession") && !ls::session_load_flow(nullptr));
    CHECK(!std::strcmp(ls::status_name(3), "Exiting") && !std::strcmp(ls::status_name(9), "?"));
    if (g_fail) {
        std::printf("load stall test: %d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("load stall test OK\n");
    return 0;
}
