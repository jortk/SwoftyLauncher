#include "leaderboard_guard_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

// Time Trial leaderboard guard (re/db/functions.tsv Game_TimeTrialLeaderboard_SelectBoard).
//
// Pressing View Leaderboards in Time Trial runs Game_TimeTrialLeaderboard_SelectBoard through the
// leaderboard_container binding. It builds the board key 'LBV%02u%02d%s' from the track and the weather, then
// searches the leaderboard list (self+0x478 -> +0x10 -> +0x18..+0x20) for the board whose name (+0x68) equals the
// key, and reads the board's rank thresholds (+0x88, +0x98..+0xa0). The search leaves 0 in rcx when no board
// carries the key, and the read has no null check: with the online services off (the list is empty) or at a track
// without a board, the first read faults (ACCESS_VIOLATION reading 0x98 at F1_25.exe+0x2fce9e3).
//
// site_Game_TimeTrialLeaderboard_BoardRead is that read (rcx = the board). When the board is null the guard sets
// esi to 0, the function's return value, and continues at site_Game_TimeTrialLeaderboard_BoardsDone, the function's
// own path after the board work, which releases its locals and returns esi. The caller that tests the result
// takes its failure branch, and the other caller ignores it. A board that was found runs the game's code
// unchanged.
//
// Activating the leaderboard tab runs Sess_TimeTrialLeaderboard_ActivateTab (vtable slot 0x13 of the same model), which
// reads the child list model at self+0x9a0 and calls through it with no null check (F1_25.exe+0x2fcbb09 reading 0). The
// child is null when the screen opened for a track the view skipped above. site_Sess_TimeTrialLeaderboard_TabChild is
// that read (rcx = the child). When it is null the guard continues at site_Sess_TimeTrialLeaderboard_TabDone, the
// function's own epilogue, which skips the child call and the refresh that follows it. A child that exists runs the
// game's code unchanged. Sess_TimeTrialLeaderboard_ActivateTabBound, the same model's bound-only twin that the garage
// monitor panel calls directly for each leaderboard tab (from Vm_Dispatch_BindingMessage), makes the same read with no
// null check (F1_25.exe+0x2fcd166 reading 0, a Time Trial on an added track whose board is not listed).
// site_Sess_TimeTrialLeaderboard_BoundTabChild is that read, and for a null child the guard continues at
// site_Sess_TimeTrialLeaderboard_BoundTabDone, that function's own epilogue.
//
// Entering Time Trial (setCurrentGameMode none -> time_trial) runs Game_TimeTrial_SetReference, which stores the board
// for the reference's track and weather at presenter+0x20278 and then, when g_Ne_LogGate holds the GAME channel, logs
// '[TIME TRIALS] New reference : ... leaderboard(%s).' with the board's name read from [board+0x40]. The board is 0 when
// no board is listed for the track (offline play has no online board list), and the read faults (ACCESS_VIOLATION
// reading 0x40 at F1_25.exe+0x147bc9b). Requesting a rival ghost from the board logs the same name on GHOSTLAP-TRACE in
// Game_GhostlapTrace_OnRivalFromLeaderboardDownloadRequested. Stock builds keep both channels closed; swofty_gamelog
// opens them for a GAME or GHOSTLAP-TRACE channel rule. site_Game_TimeTrial_SetReferenceLogBoard and
// site_Game_GhostlapTrace_RivalLogBoard are those reads (rsi = the presenter). For a null board the guard continues at
// site_Game_TimeTrial_SetReferenceLogDone or site_Game_GhostlapTrace_RivalLogDone, the paths the game takes with the
// channel closed, so only the log line is skipped. A board that was found is logged as the game does.
//
// When the reference's track changed, Game_TimeTrial_SetReference reads the leaderboard descriptor of the current game
// mode's selected entrant list ([[[presenter+0x20]+0x88]+0x260]+0xe18, its stat id at +0xe0, engaged at +0xf0) and
// throws a bad optional access (a 0xc0000409 fast fail) when the list has none. A Time Trial entered from the home
// screen after a Grand Prix found swofty_f1 with such a list selected (1.9.18, Jellow07). site_Game_TimeTrial_SetReferenceListBoard
// is that check (rdx = the list, rsi = the presenter, ecx = the presenter's old track). For a list without a
// leaderboard the guard puts the old track back at presenter+0x3c, so the next call sees the change and sets the
// reference, and continues at site_Game_TimeTrial_SetReferenceNoList, the path the game takes when no list is selected.
// A list with a leaderboard runs the game's code unchanged.
//
// SWOFTY_LEADERBOARD_GUARD=0 turns the board guard off, SWOFTY_LEADERBOARD_TAB_GUARD=0 the tab guard,
// SWOFTY_TT_REFERENCE_LOG_GUARD=0 the reference log guard, SWOFTY_TT_REFERENCE_LIST_GUARD=0 the reference list guard.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (time trial leaderboard guard)";
namespace lg = runtime::leaderboard_guard;

constexpr const char *TAB_MIXIN = "swofty_core.dll (time trial leaderboard tab guard)";
constexpr const char *REFERENCE_MIXIN = "swofty_core.dll (time trial reference log guard)";
constexpr const char *LIST_MIXIN = "swofty_core.dll (time trial reference list guard)";
constexpr uintptr_t PRESENTER_BOARD = 0x20278;
constexpr uintptr_t PRESENTER_CONTEXT = 0x20, PRESENTER_TRACK = 0x3c, CONTEXT_MODE = 0x88, MODE_ENTITY = 0x260,
                    MODE_NAME = 0x278, ENTITY_SETUP_MODE = 0x2b0, SETUP_MODE_NAME = 0xd8, LIST_NAME = 0x28, HOLDER_TEXT = 0x10,
                    LIST_BEGIN = 0x10, LIST_END = 0x18, ENTRANT_RECORD = 0x88;

uintptr_t boards_done = 0, tab_done = 0, bound_tab_done = 0, reference_done = 0, rival_done = 0, list_done = 0;
bool offered = false, settled = false;
std::atomic<int> skipped{0}, tab_skipped{0}, reference_skipped{0}, list_skipped{0};

bool env_off(const char *name, const char *label)
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA(name, v, sizeof v);
    if (!n || n >= sizeof v || !lg::switch_off(v))
        return false;
    swofty::log("[leaderboard] %s off (%s=%s)", label, name, v);
    return true;
}

void board_read(SwoftyContext *c)
{
    if (!lg::board_missing(c->rcx))
        return;
    c->rsi = 0;
    c->rip = boards_done;
    lg::Say say = lg::say(++skipped);
    if (say == lg::Say::none)
        return;
    swofty::log("[leaderboard] Time Trial leaderboard view 0x%llx has no board for this track (online services off or no "
                "board listed); view skipped%s",
                (unsigned long long)c->rdi, say == lg::Say::last ? " (further skips are not logged)" : "");
}

void tab_child_skipped(SwoftyContext *c, uintptr_t done, const char *how)
{
    c->rip = done;
    lg::Say say = lg::say(++tab_skipped);
    if (say == lg::Say::none)
        return;
    swofty::log("[leaderboard] Time Trial leaderboard model 0x%llx activated its tab%s with no child list model; tab "
                "activation skipped%s",
                (unsigned long long)(c->rsi - 0x660), how, say == lg::Say::last ? " (further skips are not logged)" : "");
}

void tab_child_read(SwoftyContext *c)
{
    if (lg::tab_child_missing(c->rcx))
        tab_child_skipped(c, tab_done, "");
}

void bound_tab_child_read(SwoftyContext *c)
{
    if (lg::tab_child_missing(c->rcx))
        tab_child_skipped(c, bound_tab_done, " from the garage monitor");
}

bool presenter_board_missing(uintptr_t presenter)
{
    uint64_t board = 0;
    return !swofty::mem::read(presenter + PRESENTER_BOARD, board) || lg::reference_board_missing(board);
}

void reference_log_skipped(uintptr_t presenter, const char *line)
{
    lg::Say say = lg::say(++reference_skipped);
    if (say == lg::Say::none)
        return;
    swofty::log("[leaderboard] Time Trial presenter 0x%llx has no board for this reference (online services off or no "
                "board listed); the game's %s log line skipped%s",
                (unsigned long long)presenter, line, say == lg::Say::last ? " (further skips are not logged)" : "");
}

void reference_log_board(SwoftyContext *c)
{
    if (!presenter_board_missing(c->rsi))
        return;
    c->rip = reference_done;
    reference_log_skipped(c->rsi, "GAME 'New reference'");
}

void rival_log_board(SwoftyContext *c)
{
    if (!presenter_board_missing(c->rsi))
        return;
    c->rip = rival_done;
    reference_log_skipped(c->rsi, "GHOSTLAP-TRACE 'Ghost requested'");
}

std::string text_at(uintptr_t text)
{
    std::string out;
    if (text)
        swofty::mem::c_string(text, out, 64);
    return out;
}

std::string held_text(uintptr_t object, uintptr_t off)
{
    uintptr_t holder = 0, text = 0;
    if (!object || !swofty::mem::read(object + off, holder) || !holder || !swofty::mem::read(holder + HOLDER_TEXT, text))
        return {};
    return text_at(text);
}

void reference_list_skipped(uintptr_t presenter, uintptr_t list)
{
    lg::Say say = lg::say(++list_skipped);
    if (say == lg::Say::none)
        return;
    uintptr_t context = 0, mode = 0, entity = 0, setup_mode = 0, name = 0, begin = 0, end = 0;
    std::string mode_name, setup_name;
    if (swofty::mem::read(presenter + PRESENTER_CONTEXT, context) && context && swofty::mem::read(context + CONTEXT_MODE, mode) &&
        mode) {
        if (swofty::mem::read(mode + MODE_NAME, name))
            mode_name = text_at(name);
        if (swofty::mem::read(mode + MODE_ENTITY, entity) && entity && swofty::mem::read(entity + ENTITY_SETUP_MODE, setup_mode))
            setup_name = held_text(setup_mode, SETUP_MODE_NAME);
    }
    int64_t cars = -1;
    if (swofty::mem::read(list + LIST_BEGIN, begin) && swofty::mem::read(list + LIST_END, end) && end >= begin)
        cars = int64_t((end - begin) / ENTRANT_RECORD);
    swofty::log("[leaderboard] %s%s",
                lg::reference_list_line(presenter, mode_name, setup_name, held_text(list, LIST_NAME), cars).c_str(),
                say == lg::Say::last ? " (further skips are not logged)" : "");
}

void reference_list_board(SwoftyContext *c)
{
    uint8_t engaged = 1;
    if (!swofty::mem::read(c->rdx + lg::LIST_BOARD_SET, engaged) || !lg::reference_list_unboarded(engaged))
        return;
    uint32_t old_track = uint32_t(c->rcx);
    if (swofty::mem::readable(c->rsi + PRESENTER_TRACK, sizeof old_track))
        *reinterpret_cast<uint32_t *>(c->rsi + PRESENTER_TRACK) = old_track;
    c->rip = list_done;
    reference_list_skipped(c->rsi, c->rdx);
}

}

namespace runtime {

std::vector<CoreMixins> leaderboard_guard_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    if (!env_off("SWOFTY_LEADERBOARD_GUARD", "time trial leaderboard guard")) {
        boards_done = swofty::api->resolve("site_Game_TimeTrialLeaderboard_BoardsDone");
        if (!boards_done || !swofty::api->resolve("site_Game_TimeTrialLeaderboard_BoardRead")) {
            boards_done = 0;
            swofty::log("[leaderboard] time trial leaderboard guard off: needs site_Game_TimeTrialLeaderboard_BoardRead "
                        "and site_Game_TimeTrialLeaderboard_BoardsDone, which this build does not resolve");
        } else {
            out.push_back({MIXIN,
                           {{"site_Game_TimeTrialLeaderboard_BoardRead", SWOFTY_MIXIN_SITE, 100, (void *)&board_read, nullptr}}});
        }
    }
    if (!env_off("SWOFTY_LEADERBOARD_TAB_GUARD", "time trial leaderboard tab guard")) {
        tab_done = swofty::api->resolve("site_Sess_TimeTrialLeaderboard_TabDone");
        bound_tab_done = swofty::api->resolve("site_Sess_TimeTrialLeaderboard_BoundTabDone");
        if (!tab_done || !swofty::api->resolve("site_Sess_TimeTrialLeaderboard_TabChild"))
            tab_done = 0;
        if (!bound_tab_done || !swofty::api->resolve("site_Sess_TimeTrialLeaderboard_BoundTabChild"))
            bound_tab_done = 0;
        std::vector<SwoftyMixin> sites;
        if (tab_done)
            sites.push_back({"site_Sess_TimeTrialLeaderboard_TabChild", SWOFTY_MIXIN_SITE, 100, (void *)&tab_child_read, nullptr});
        if (bound_tab_done)
            sites.push_back({"site_Sess_TimeTrialLeaderboard_BoundTabChild", SWOFTY_MIXIN_SITE, 100, (void *)&bound_tab_child_read,
                             nullptr});
        if (sites.empty())
            swofty::log("[leaderboard] time trial leaderboard tab guard off: needs site_Sess_TimeTrialLeaderboard_TabChild "
                        "and site_Sess_TimeTrialLeaderboard_TabDone, which this build does not resolve");
        else
            out.push_back({TAB_MIXIN, std::move(sites)});
    }
    if (!env_off("SWOFTY_TT_REFERENCE_LOG_GUARD", "time trial reference log guard")) {
        reference_done = swofty::api->resolve("site_Game_TimeTrial_SetReferenceLogDone");
        rival_done = swofty::api->resolve("site_Game_GhostlapTrace_RivalLogDone");
        if (!reference_done || !swofty::api->resolve("site_Game_TimeTrial_SetReferenceLogBoard"))
            reference_done = 0;
        if (!rival_done || !swofty::api->resolve("site_Game_GhostlapTrace_RivalLogBoard"))
            rival_done = 0;
        std::vector<SwoftyMixin> sites;
        if (reference_done)
            sites.push_back({"site_Game_TimeTrial_SetReferenceLogBoard", SWOFTY_MIXIN_SITE, 100, (void *)&reference_log_board, nullptr});
        if (rival_done)
            sites.push_back({"site_Game_GhostlapTrace_RivalLogBoard", SWOFTY_MIXIN_SITE, 100, (void *)&rival_log_board, nullptr});
        if (sites.empty())
            swofty::log("[leaderboard] time trial reference log guard off: needs site_Game_TimeTrial_SetReferenceLogBoard "
                        "and site_Game_TimeTrial_SetReferenceLogDone, which this build does not resolve");
        else
            out.push_back({REFERENCE_MIXIN, std::move(sites)});
    }
    if (!env_off("SWOFTY_TT_REFERENCE_LIST_GUARD", "time trial reference list guard")) {
        list_done = swofty::api->resolve("site_Game_TimeTrial_SetReferenceNoList");
        if (!list_done || !swofty::api->resolve("site_Game_TimeTrial_SetReferenceListBoard")) {
            list_done = 0;
            swofty::log("[leaderboard] time trial reference list guard off: needs site_Game_TimeTrial_SetReferenceListBoard "
                        "and site_Game_TimeTrial_SetReferenceNoList, which this build does not resolve");
        } else {
            out.push_back({LIST_MIXIN,
                           {{"site_Game_TimeTrial_SetReferenceListBoard", SWOFTY_MIXIN_SITE, 100, (void *)&reference_list_board,
                             nullptr}}});
        }
    }
    return out;
}

void leaderboard_guard_commit()
{
    if (!offered || settled)
        return;
    settled = true;
    if (boards_done)
        swofty::log("[leaderboard] time trial leaderboard guard %s",
                    core_mixins_active(MIXIN)
                        ? "armed: View Leaderboards at a track without a board returns instead of faulting"
                        : "did not install (see the [core] lines)");
    if (tab_done || bound_tab_done)
        swofty::log("[leaderboard] time trial leaderboard tab guard %s",
                    core_mixins_active(TAB_MIXIN)
                        ? (tab_done && bound_tab_done
                               ? "armed: activating the leaderboard tab without a child list model returns instead of faulting"
                               : "armed on one of its two tab activations (see the [core] lines)")
                        : "did not install (see the [core] lines)");
    if (reference_done || rival_done)
        swofty::log("[leaderboard] time trial reference log guard %s",
                    core_mixins_active(REFERENCE_MIXIN)
                        ? (rival_done && reference_done
                               ? "armed: the game's GAME and GHOSTLAP-TRACE lines for a reference with no board are "
                                 "skipped instead of faulting"
                               : "armed on one of its two log lines (see the [core] lines)")
                        : "did not install (see the [core] lines)");
    if (list_done)
        swofty::log("[leaderboard] time trial reference list guard %s",
                    core_mixins_active(LIST_MIXIN)
                        ? "armed: a Time Trial reference for a selected list without a leaderboard is left unset instead of "
                          "throwing"
                        : "did not install (see the [core] lines)");
}

}
