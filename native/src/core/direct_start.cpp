#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <vector>
#include <cstdio>
#include <cctype>
#include <cstring>
#include <map>
#include <mutex>
#include <thread>

#include "apply/moments.hpp"
#include "direct_start_logic.hpp"
#include "direct_start_tt.hpp"
#include "json.hpp"
#include "room_cover.hpp"
#include "testagent/agent_logic.hpp"
#include "title_skip.hpp"

// The direct start: the game goes from boot to its target with no title, no legal screens' waits and no
// menu. SWOFTY_DIRECT_START in the game's environment, or swofty\direct_start.json written by the launcher for
// this launch, names the target: a session,
// or an external target (a plugin's setup, through the session host). The title engages by itself (title_skip.cpp), which loads the profile;
// the target sets itself up on the flow thread; then the flow leaves the front end from StateFTUE, whose
// finished link is held until then and replaced by its exit link, so StateFrontend (the menu) never starts.
// StateFTUE leaves with the game's interrupt exit (kind 0xe, written on the empty front of the interrupt queue
// at +0xbf8): its onStateExit (0x1436d8390) then despawns the front end's main scene and blocks until the
// scene's flags and component set are clear, so the front end's PhysicsTerrain is gone before the track's
// is created. The kind is cleared as that exit finishes, because the queue is shared with the parents.
// StateFrontendWorld, with no child left, then takes the target's link ('session' leads to StateLaunch's
// path and StateLoadSession).
//
// The flow machine (Vm_Update_FlowStateMachine 0x144423ce0) calls each state's onStateUpdate (vt+0x20,
// the shared wrapper that updates children first) at 0x144424c75 with rdi = the state and the returned
// link written to [rsp+0x40]; a mid hook on the next instruction (0x144424c78, where it compares that
// link with g_Game_FlowLinkStay) sees every state's link and can replace it. State nodes: +0xb0 the
// name's djb2 hash, +0xbd8 the active child.

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

namespace {

namespace ds = runtime::direct_start;

const runtime::Sig SIG_FLOW_LINK{"vm.flow_state_update_link",
                                 "48 8B 07 48 8D 54 24 40 48 89 F9 C5 F9 28 D6 FF 50 20 8B 44 24 40 39 05", 0x4424c78,
                                 runtime::Sig::addr, 0, 0x12};
const runtime::Sig SIG_GAME_MANAGER{"game.manager", "48 8B B8 98 01 00 00 48 85 FF 0F 84 ?? ?? ?? ?? 48 8D 4C 24 70 E8 ?? ?? ?? ?? 48 8B 05", 0x604c2c8, runtime::Sig::riprel, 26, 29, 7};
const runtime::Sig SIG_SET_MODE{"game.game_set_current_game_mode", "41 56 56 57 55 53 48 81 EC 80 00 00 00 48 8B 81 88 00 00 00", 0x7d07a0};
const runtime::Sig SIG_MODE_BY_PREDICATE{"game.game_modes_find_by_predicate",
                                         "48 8D 54 24 28 E8 ?? ?? ?? ?? 48 85 C0 74 22 48 89 C7 48 8B 8E 08 03 00 00",
                                         0x7d2280, runtime::Sig::rel32, 5, 6, 5};
const runtime::Sig SIG_GP_PREDICATE{"game.grand_prix_mode_predicate",
                                    "48 8D 05 ?? ?? ?? ?? 48 89 44 24 28 48 8D 54 24 28 E8 ?? ?? ?? ?? 48 85 C0 74 22",
                                    0x77cff0, runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_FTUE_EXIT{"game.state_ftue_on_state_exit",
                                 "56 48 83 EC 30 48 89 CE 48 8B 81 F8 0B 00 00 48 8B 48 18 48 8D 50 30 48 39 48 20 48 0F "
                                 "45 D1 83 7A 10 0E 0F 85 0D 02 00 00 48 8B 8E 88 0C 00 00 48 8B 41 40",
                                 0x36d8390};
const runtime::Sig SIG_BUILD_SETTINGS{"game.championship_game_mode_build_session_settings",
                                      "41 57 41 56 41 55 41 54 56 57 55 53 B8 68 68 00", 0x7b7b90};
// The front end's scene and the Grand Prix weekend's game addresses, each matching once on the KB, EA and Steam
// 26566308 builds at the same code (re/tools/sigverify.py). Thunked functions are found from a call to them.
const runtime::Sig SIG_DESPAWN_MAIN_SCENE{"game.frontend_scene_manager_despawn_main_scene_1428d9a90",
                                          "56 57 53 48 83 EC 20 48 89 CE 80 3D ?? ?? ?? ?? 01 75 66 B8 BD 00 00 00", 0x28d9a90};
const runtime::Sig SIG_LIST_STAT_ID{"game.entrant_list_stat_id_thunk_1424286e0",
                                    "E8 ?? ?? ?? ?? 41 89 C6 48 8D 4C 24 50 48 8D 15", 0x24286e0, runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_ROUNDS_CLEAR{"game.championship_rounds_clear_thunk_14420cd60",
                                    "E8 ?? ?? ?? ?? 48 8B 8E 70 02 00 00 48 8B 01 FF 90 88 00 00 00 48 85 C0 0F 84 ?? ?? ?? ?? "
                                    "48 89 C3",
                                    0x420cd60, runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_ADD_TRACK{"game.championship_rounds_add_track_thunk_140b99450",
                                 "E8 ?? ?? ?? ?? 48 83 C6 08 48 39 DE 75 EC 48 8B BC 24 70 01 00 00", 0xb99450,
                                 runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_BUILD_SEASON{"game.championship_build_season_141207290", "56 48 83 EC 20 48 89 CE BA 98 02 00 00",
                                    0x1207290};
const runtime::Sig SIG_FILL_ENTRANTS{"sess.build_candidate_pool_142460230",
                                     "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC B8 00 00 00 4D 89 CE 4C 89 C7", 0x2460230};
const runtime::Sig SIG_WEEKEND_STRUCTURE{"sess.build_dev_weekend_structure_140b025f0",
                                         "41 57 41 56 41 55 41 54 56 57 55 53 48 83 EC 48 44 89 CD 45 89 C6", 0xb025f0};
const runtime::Sig SIG_WEEKEND_SERVICE{"game.weekend_service_component_14605ada0",
                                       "48 8D 15 ?? ?? ?? ?? 41 B8 01 00 00 00 FF 50 10 48 85 C0 74 4B", 0x605ada0,
                                       runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_ROUNDS_COMPONENT{"game.championship_rounds_component_14605ad40",
                                        "48 8D 15 ?? ?? ?? ?? FF 50 10 48 89 83 50 05 00 00", 0x605ad40, runtime::Sig::riprel,
                                        0, 3, 7};
const runtime::Sig SIG_MESSAGE_BUS{"ne.message_bus_14618b3b8", "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 30 48 8B 48 08", 0x618b3b8,
                                   runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_POST_BEGIN{"ne.post_entrants_document_opened_thunk_1424e2b90",
                                  "E8 ?? ?? ?? ?? C6 86 F8 02 00 00 01 4C 89 F1", 0x24e2b90, runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_POST_END{"ne.post_entrants_document_closed_thunk_1424e2f00", "E8 ?? ?? ?? ?? C6 86 F8 02 00 00 00 85 FF",
                                0x24e2f00, runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_VT_MSG_BEGIN{"game.vt_entrants_document_opened_14578d4e8",
                                    "4C 8D 25 ?? ?? ?? ?? 4C 89 64 24 50 48 8B 48 08", 0x578d4e8, runtime::Sig::riprel, 0, 3, 7};
// sigverify.py's slot comparison reads two slots past this vtable; its type getter (slot 1) names EntrantsDocumentClosed
// on all three builds.
const runtime::Sig SIG_VT_MSG_END{"game.vt_entrants_document_closed_14578d510",
                                  "48 8D 0D ?? ?? ?? ?? 48 89 4C 24 38 4C 89 74 24 48", 0x578d510, runtime::Sig::riprel, 0, 3, 7};

constexpr uintptr_t MANAGER_MODES = 0xc0, ENTITY_REGISTRY = 0x1030, ENTITY_MODE = 0xf08;

using ModeByPredicate_t = uint8_t *(void *modes, void *const *predicate);
using SetMode_t = void(void *modes, void *mode);

// A new Grand Prix weekend, as the dev session's Apply (0x141125e80) builds one, on the Grand Prix mode entity E
// (KB build: re/db/subsystems/session.md, re/new-gp): the game setup mode from E's list (vt+0x1d0) by name, the
// setter 0x14079f300, the entrant list by name (EntrantsSystem [game manager+0x290] +0x560), one round at a
// track found by name in E's track system (+0xf58), the season built by 0x141207290 on a stub, E's components
// attached (vt+0xe8) and the entrants filled from the list (0x142460230).
// A session request without a box or track gets the stock 2025 grid at Albert Park; it never continues a saved Grand
// Prix, whose weekend may be one the session load can't take.
constexpr const char *DEFAULT_BOX = "f1_2025", *DEFAULT_TRACK = "melbourne", *HOST_SETUP_MODE = "swofty_f1";
constexpr uintptr_t ES_LISTS = 0x560, ENTITY_TRACKS = 0xf58,
                    ENTITY_LIST = 0xe18, ENTITY_ENTRANTS = 0x2e0, ENTRANT_SIZE = 0x88;

struct Game {
    uintptr_t manager = 0, set_mode = 0, by_predicate = 0, gp_predicate = 0, root = 0, despawn_main_scene = 0;
    uintptr_t set_setup_mode = 0, list_stat_id = 0, rounds_clear = 0, add_track = 0, build_season = 0, fill_entrants = 0,
              weekend_structure = 0, weekend_service = 0, rounds_component = 0, message_bus = 0, post_begin = 0,
              post_end = 0, vt_msg_begin = 0, vt_msg_end = 0;
    bool grand_prix() const
    {
        return manager && set_mode && by_predicate && gp_predicate && set_setup_mode && list_stat_id && rounds_clear &&
               add_track && build_season && fill_entrants && weekend_structure && weekend_service && rounds_component &&
               message_bus && post_begin && post_end && vt_msg_begin && vt_msg_end;
    }
} game;

// The game modes context SetCurrentGameMode takes, as the game passes it (it sets the mode once at boot).
safetyhook::InlineHook h_set_mode;
std::atomic<void *> modes_seen{nullptr};

void set_mode_detour(void *modes, void *mode)
{
    if (modes && !modes_seen.exchange(modes))
        swofty::log("[direct] the game modes context is %p (SetCurrentGameMode, mode %p)", modes, mode);
    h_set_mode.call<void>(modes, mode);
}

constexpr size_t STATE_HASH = 0xb0, STATE_CHILD = 0xbd8, LINK_SLOT = 0x40;

std::mutex lock;
ds::Driver driver;
json::Value request;
safetyhook::MidHook h_link;
std::atomic<bool> armed{false};
std::atomic<int> session_frames{0};
// Read by the cover from the present thread, which holds the cover's lock; the flow thread calls the cover's step()
// under this file's lock, so the cover side never takes it.
std::atomic<bool> start_failed{false};
std::atomic<uint64_t> session_at{0};
std::atomic<bool> session_screen{false};
// The broadcast intro runs under the cover; it comes off at the garage (or the grid), or this long after StateSession
// started updating when neither screen opens.
constexpr uint64_t SESSION_SCREEN_WAIT_MS = 60000;
ds::Phase logged = ds::Phase::off;

void note_phase()
{
    if (driver.phase == logged)
        return;
    logged = driver.phase;
    if (driver.phase == ds::Phase::failed)
        start_failed = true;
    if (driver.phase == ds::Phase::failed)
        swofty::log("[direct] %s start failed: %s; the menu opens as usual", driver.target.name.c_str(),
                    driver.why.c_str());
    else
        swofty::log("[direct] %s start: %s", driver.target.name.c_str(), ds::phase_name(driver.phase));
}

constexpr size_t STATE_STATUS = 0xb8, STATE_CHILDREN = 0xc0, STATE_LINKS = 0xaf0, STATE_LINK_COUNT = 0xb30,
                 STATE_PARENT = 0xc10;
constexpr uint32_t STATE_APP = 0xe11edb87, STATE_SESSION = 0x1505f42a, STATE_LOAD_SESSION = 0x17e3cd0a;
constexpr size_t LOAD_SYNC = 0xcd0;
constexpr uint64_t LOAD_EXIT_WAIT_MS = 500;

void choose_session(void *modes, const char *when);
template <class T> bool rd(const void *p, size_t off, T &out);

// The loading screen's activation (Game_LoadingScreenVM_OnScreenActivated 0x141ca43e0) releases the fade reason
// FrontEnd (bit 1) on the fade owner [[g_Game_Root]+0xa0]: mask +0x7b8, and with no reason left the fade in (state
// +0x7bc = 3, +0x7cc = 0, curve +0x7c4). A direct start never shows that screen, so the session would stay black
// under its HUD: once StateSession updates, the core releases FrontEnd itself.
constexpr uint32_t FADE_FRONTEND = 1u << 1;
bool fade_released = false;
constexpr uint32_t FADE_EXIT_REQUEST = 1u << 0;
std::atomic<bool> ftue_interrupt_raised{false};

void release_frontend_fade()
{
    auto *root = game.root ? *reinterpret_cast<uint8_t **>(game.root) : nullptr;
    uint8_t *owner = nullptr;
    if (!root || !rd(root, 0xa0, owner) || !owner || !runtime::readable(owner + 0x7b8, 0x18)) {
        swofty::log("[direct] no fade owner to release FrontEnd on (game root %p)", root);
        return;
    }
    auto &mask = *reinterpret_cast<uint32_t *>(owner + 0x7b8);
    uint32_t was = mask, state = *reinterpret_cast<uint32_t *>(owner + 0x7bc);
    fade_released = true;
    uint32_t release = FADE_FRONTEND | (ftue_interrupt_raised.load() ? FADE_EXIT_REQUEST : 0);
    if (!(was & release)) {
        swofty::log("[direct] fade owner %p: mask %08x, state %u; FrontEnd is not held", owner, was, state);
        return;
    }
    mask = was & ~release;
    if (mask == 0) {
        *reinterpret_cast<uint32_t *>(owner + 0x7bc) = 3;
        owner[0x7cc] = 0;
    }
    *reinterpret_cast<uint32_t *>(owner + 0x7c4) = 0xd47eb226;
    swofty::log("[direct] fade owner %p: mask %08x -> %08x (FrontEnd released), state %u -> %u", owner, was, mask, state,
                *reinterpret_cast<uint32_t *>(owner + 0x7bc));
}

// The fade owner's reasons and state as the session's intro runs, logged when they change. A race opens no screen
// of its own: its broadcast starts with the first fade in after the intro video (BroadcastFull released, state 3),
// and the cover comes off there. The grid follows the broadcast's grid rundown, two minutes or more later.
bool race_session = false;

void note_fade()
{
    static uint32_t last_mask = ~0u, last_state = ~0u;
    static int told = 0;
    auto *root = game.root ? *reinterpret_cast<uint8_t **>(game.root) : nullptr;
    uint8_t *owner = nullptr;
    if (told >= 40 || !root || !rd(root, 0xa0, owner) || !owner || !runtime::readable(owner + 0x7b8, 8))
        return;
    uint32_t mask = *reinterpret_cast<uint32_t *>(owner + 0x7b8), state = *reinterpret_cast<uint32_t *>(owner + 0x7bc);
    if (mask == last_mask && state == last_state)
        return;
    bool faded_in = last_mask != ~0u && last_mask != 0 && mask == 0 && state == 3;
    last_mask = mask;
    last_state = state;
    told++;
    uint64_t ms = GetTickCount64() - session_at.load();
    swofty::log("[direct] fade owner: mask %08x, state %u, %llu ms into the session", mask, state, (unsigned long long)ms);
    if (race_session && faded_in && !session_screen.exchange(true))
        swofty::log("[direct] the race's broadcast is up after %llu ms; the cover comes off", (unsigned long long)ms);
}

uint8_t *flow_root = nullptr;
uint64_t load_exiting_at = 0;

// The front end's main scene (the menus' dark backdrop with red light streaks) is despawned by StateFrontend's
// exit (Game_StateFrontend_OnStateExit 0x1436d3b00: Game_FrontendSceneManager_DespawnMainScene 0x1428d9a90 on
// [state+0xd20] when [[state+0xd20]+0x40]+0x1048 or +0x1049 is set). A direct start never enters StateFrontend, so
// the backdrop stays under the session: a Time Trial's garage shows it instead of the 3D view, and a Grand Prix's
// broadcast intro cards sit on it. The node exists in the flow graph all the same (FrontendWorld's child
// 0x957880c6, its +0xd20 set by its init), so the core despawns the scene through its manager, once.
constexpr uint32_t STATE_FRONTEND = 0x957880c6;
constexpr uintptr_t FRONTEND_SCENES = 0xd20, SCENES_MAIN = 0x40, MAIN_SPAWNED = 0x1048;
std::atomic<bool> frontend_scene_despawned{false};

uint8_t *flow_node(uint32_t want)
{
    std::vector<uint8_t *> todo{flow_root};
    for (size_t i = 0; i < todo.size() && i < 1024; ++i) {
        uint8_t *n = todo[i];
        if (!n || !runtime::readable(n, 0xc8))
            continue;
        if (*reinterpret_cast<uint32_t *>(n + 0xb0) == want)
            return n;
        auto *cv = *reinterpret_cast<uint8_t **>(n + 0xc0);
        if (!cv || !runtime::readable(cv, 0x18))
            continue;
        for (auto **p = *reinterpret_cast<uint8_t ***>(cv + 8), **e = *reinterpret_cast<uint8_t ***>(cv + 0x10);
             p && p < e && runtime::readable(p, 8); ++p)
            todo.push_back(*p);
    }
    return nullptr;
}

bool frontend_main_scene(uint8_t *&mgr)
{
    uint8_t *node = flow_node(STATE_FRONTEND);
    mgr = node && runtime::readable(node, FRONTEND_SCENES + 8) ? *reinterpret_cast<uint8_t **>(node + FRONTEND_SCENES)
                                                                : nullptr;
    auto *scenes = mgr && runtime::readable(mgr, SCENES_MAIN + 8) ? *reinterpret_cast<uint8_t **>(mgr + SCENES_MAIN)
                                                                  : nullptr;
    return scenes && runtime::readable(scenes, MAIN_SPAWNED + 2) && (scenes[MAIN_SPAWNED] || scenes[MAIN_SPAWNED + 1]);
}

void despawn_frontend_scene(const char *when)
{
    if (frontend_scene_despawned.load())
        return;
    uint8_t *mgr = nullptr;
    bool spawned = frontend_main_scene(mgr);
    if (!mgr) {
        swofty::log("[direct] no StateFrontend node or front end scene manager %s", when);
        return;
    }
    if (!spawned) {
        swofty::log("[direct] the front end's main scene is not spawned %s (manager %p)", when, mgr);
        return;
    }
    if (!game.despawn_main_scene) {
        swofty::log("[direct] the front end's main scene stays %s: DespawnMainScene did not resolve", when);
        return;
    }
    frontend_scene_despawned = true;
    reinterpret_cast<void (*)(void *)>(game.despawn_main_scene)(mgr);
    swofty::log("[direct] despawned the front end's main scene %s (manager %p)", when, mgr);
}

bool env_flag(const char *name)
{
    char v[8];
    DWORD n = GetEnvironmentVariableA(name, v, sizeof v);
    return n > 0 && n < sizeof v && v[0] != '0';
}

constexpr uintptr_t RVA_TERRAIN_COUNT = 0x6cb5d50, FTUE_EXIT_GATE = 0xc18, FTUE_INTERRUPT_SEEN = 0xc90,
                    INTERRUPT_QUEUE = 0xbf8, INTERRUPT_EMPTY_FRONT = 0x30;
constexpr uint32_t INTERRUPT_EXIT = 0xe;
const bool diag_on = env_flag("SWOFTY_DIRECT_DIAG");
bool ftue_interrupt_on = true;
std::atomic<bool> diag_started{false}, diag_load_told{false}, ftue_interrupt_cleared{false};
uint8_t *ftue_interrupt_queue = nullptr;

uint32_t terrain_count()
{
    uint32_t v = 0xffffffffu;
    if (runtime::same_build())
        rd(reinterpret_cast<const void *>(swofty::api->module_base() + RVA_TERRAIN_COUNT), 0, v);
    return v;
}

struct SceneProbe {
    uint8_t *mgr = nullptr, *scenes = nullptr;
    uint8_t f48 = 0xff, f49 = 0xff, f4b = 0xff;
    unsigned occupied = 0, flagged = 0;
    uint64_t count = 0;
    size_t cap = 0;
    bool clear() const { return scenes && f48 == 0 && f49 == 0 && flagged == 0; }
};

SceneProbe probe_scene()
{
    SceneProbe p;
    uint8_t *node = flow_node(STATE_FRONTEND);
    if (node)
        rd(node, FRONTEND_SCENES, p.mgr);
    if (p.mgr)
        rd(p.mgr, SCENES_MAIN, p.scenes);
    if (!p.scenes)
        return p;
    rd(p.scenes, MAIN_SPAWNED, p.f48);
    rd(p.scenes, MAIN_SPAWNED + 1, p.f49);
    rd(p.scenes, 0x104b, p.f4b);
    rd(p.scenes, 0x100, p.count);
    uint8_t *data = nullptr, *ctrl = nullptr;
    if (!rd(p.scenes, 0xf0, data) || !rd(p.scenes, 0xf8, ctrl) || !data || ctrl <= data || (ctrl - data) / 0x18 > 8192)
        return p;
    p.cap = size_t(ctrl - data) / 0x18;
    for (size_t i = 0; i < p.cap; i++) {
        uint8_t c = 0;
        if (!rd(ctrl, i, c) || !(c & 1))
            continue;
        p.occupied++;
        uint8_t *v = nullptr;
        uint8_t a = 0, b = 0;
        if (rd(data, i * 0x18 + 0x10, v) && v && rd(v, 0xa1, a) && rd(v, 0xa2, b) && (a || b))
            p.flagged++;
    }
    return p;
}

void diag_snapshot(const char *tag)
{
    SceneProbe p = probe_scene();
    int32_t trans = -1, o790 = -1;
    if (p.mgr) {
        rd(p.mgr, 0x20, trans);
        uint8_t *m58 = nullptr, *obj = nullptr;
        if (rd(p.mgr, 0x58, m58) && m58 && rd(m58, 0x790, obj) && obj)
            rd(obj, 0x50, o790);
    }
    uint8_t *ftue = flow_node(ds::STATE_FTUE);
    int32_t gate = -1;
    uint8_t seen = 0xff;
    if (ftue) {
        rd(ftue, FTUE_EXIT_GATE, gate);
        rd(ftue, FTUE_INTERRUPT_SEEN, seen);
    }
    swofty::log("[direct] diag %s: scene %p flags 1048=%u 1049=%u 104b=%u set count=%llu cap=%zu occupied=%u flagged=%u "
                "transition=%d obj790_50=%d ftue_gate=%d ftue_seen=%u terrain=%u",
                tag, p.scenes, p.f48, p.f49, p.f4b, (unsigned long long)p.count, p.cap, p.occupied, p.flagged, trans,
                o790, gate, seen, terrain_count());
}

void diag_start()
{
    if (!diag_on || diag_started.exchange(true))
        return;
    std::thread([] {
        uint32_t last = ~0u;
        uint64_t t0 = GetTickCount64();
        for (int i = 0; i < 960; i++) {
            uint64_t ms = GetTickCount64() - t0;
            char tag[48];
            snprintf(tag, sizeof tag, "+%llums", (unsigned long long)ms);
            uint32_t now = terrain_count();
            if (i < 40 || now != last || i % 8 == 0)
                diag_snapshot(tag);
            last = now;
            Sleep(250);
        }
    }).detach();
}

bool raise_ftue_interrupt(uint8_t *ftue)
{
    uint8_t *queue = nullptr;
    uint64_t begin = 0, end = 0, cap = 0;
    if (!rd(ftue, INTERRUPT_QUEUE, queue) || !queue || !rd(queue, 0x18, begin) || !rd(queue, 0x20, end) ||
        !rd(queue, 0x28, cap) || !runtime::readable(queue + INTERRUPT_EMPTY_FRONT, 0x20)) {
        swofty::log("[direct] FTUE interrupt: the interrupt queue is not readable");
        return false;
    }
    auto *type = reinterpret_cast<uint32_t *>(queue + INTERRUPT_EMPTY_FRONT + 0x10);
    swofty::log("[direct] FTUE interrupt: queue %p begin %llx end %llx +28 %llx front type %u", queue,
                (unsigned long long)begin, (unsigned long long)end, (unsigned long long)cap, *type);
    if (begin != end || *type != 0) {
        swofty::log("[direct] FTUE interrupt: the queue is not empty; leaving by the link instead");
        return false;
    }
    *type = INTERRUPT_EXIT;
    ftue_interrupt_queue = queue;
    swofty::log("[direct] FTUE interrupt: raised eInterruptExit (kind %u) on the empty front of %p", INTERRUPT_EXIT, queue);
    return true;
}
safetyhook::InlineHook h_ftue_exit;

uint64_t ftue_exit_detour(void *state)
{
    uint64_t r = h_ftue_exit.call<uint64_t>(state);
    if ((r & 0xffffffffu) == 1 && ftue_interrupt_raised.load() && !ftue_interrupt_cleared.exchange(true)) {
        auto *type = reinterpret_cast<uint32_t *>(ftue_interrupt_queue + INTERRUPT_EMPTY_FRONT + 0x10);
        if (*type == INTERRUPT_EXIT)
            *type = 0;
        swofty::log("[direct] FTUE interrupt: StateFTUE::onStateExit finished (the main scene is unloaded); eInterruptExit "
                    "cleared");
    }
    return r;
}

bool load_sync_done = false;
int load_sync_tries = 0;

// StateLoadSession's exit (0x1436e0e40) waits for its sync point (+0xcd0) once the load is done: the loading
// screen's UI sets it as it hands over to the session. A direct start never opened that screen, so once the
// state has been exiting for LOAD_EXIT_WAIT_MS the core sets both nodes itself.
void load_exit(uint8_t *state)
{
    if (*reinterpret_cast<uint8_t *>(state + STATE_STATUS) != 3) {
        load_exiting_at = 0;
        return;
    }
    uint64_t now = GetTickCount64();
    if (!load_exiting_at)
        load_exiting_at = now;
    if (load_sync_done || load_sync_tries >= 10 || now - load_exiting_at < LOAD_EXIT_WAIT_MS * (load_sync_tries + 1))
        return;
    load_sync_tries++;
    bool done = runtime::title::complete_sync(state + LOAD_SYNC, "StateLoadSession's sync point (+0xcd0)");
    load_sync_done = done;
    if (done)
        runtime::room_cover::step("Starting the session", 0.85f, 0.9f);
    swofty::log("[direct] StateLoadSession exiting for %llu ms: its sync point is %s",
                (unsigned long long)(now - load_exiting_at), done ? "complete" : "still not complete");
}
uint32_t seen_states[64], seen_links[64];
size_t seen_count = 0;
uint64_t chain_at = 0;
std::string chain_last;

std::string rva(const void *p)
{
    return runtime::code_where(reinterpret_cast<uintptr_t>(p));
}

void describe_state(uint8_t *state, uint32_t hash, uint32_t link)
{
    for (size_t i = 0; i < seen_count; i++)
        if (seen_states[i] == hash) {
            if (seen_links[i] != link) {
                swofty::log("[direct] state %08x returns link %08x (was %08x)", hash, link, seen_links[i]);
                seen_links[i] = link;
            }
            return;
        }
    if (seen_count < 64) {
        seen_states[seen_count] = hash;
        seen_links[seen_count++] = link;
    }
    auto **vt = *reinterpret_cast<void ***>(state);
    std::string links;
    uint32_t n = *reinterpret_cast<uint32_t *>(state + STATE_LINK_COUNT);
    for (uint32_t i = 0; i < n && i < 16; i++) {
        char b[24];
        snprintf(b, sizeof b, " %08x>%08x", reinterpret_cast<uint32_t *>(state + STATE_LINKS)[i],
                 *reinterpret_cast<uint32_t *>(state + 0xf0 + size_t(i) * 0xa0 + 0x98));
        links += b;
    }
    std::string kids;
    auto *cv = *reinterpret_cast<uint8_t **>(state + STATE_CHILDREN);
    if (cv && runtime::readable(cv, 0x18))
        for (auto **p = *reinterpret_cast<uint8_t ***>(cv + 8), **e = *reinterpret_cast<uint8_t ***>(cv + 0x10);
             p && p < e && kids.size() < 400; ++p)
            if (*p && runtime::readable(*p, STATE_HASH + 4)) {
                char b[12];
                snprintf(b, sizeof b, " %08x", *reinterpret_cast<uint32_t *>(*p + STATE_HASH));
                kids += b;
            }
    auto *parent = *reinterpret_cast<uint8_t **>(state + STATE_PARENT);
    uint32_t ph = parent && runtime::readable(parent, STATE_HASH + 4) ? *reinterpret_cast<uint32_t *>(parent + STATE_HASH) : 0;
    std::string type = "?";
    auto *col = runtime::readable(vt - 1, 8) ? static_cast<uint8_t *>(vt[-1]) : nullptr;
    if (col && runtime::readable(col, 0x18)) {
        auto *td = reinterpret_cast<const char *>(swofty::api->module_base() + *reinterpret_cast<uint32_t *>(col + 0xc));
        if (runtime::readable(td + 0x10, 64))
            type.assign(td + 0x10, strnlen(td + 0x10, 63));
    }
    swofty::log("[direct] flow node %08x (%p, parent %08x, type %s): vt %s enter %s update %s exit %s; link now %08x; "
                "links%s; children%s",
                hash, state, ph, type.c_str(), rva(vt).c_str(), rva(vt[8]).c_str(), rva(vt[9]).c_str(),
                rva(vt[10]).c_str(), link, links.c_str(), kids.c_str());
}

void log_chain()
{
    if (!flow_root)
        return;
    std::string chain;
    uint8_t *s = flow_root;
    for (int depth = 0; s && depth < 24 && runtime::readable(s, STATE_PARENT + 8); depth++) {
        char b[32];
        snprintf(b, sizeof b, "%s%08x:%u", depth ? " > " : "", *reinterpret_cast<uint32_t *>(s + STATE_HASH),
                 unsigned(*reinterpret_cast<uint8_t *>(s + STATE_STATUS)));
        chain += b;
        s = *reinterpret_cast<uint8_t **>(s + STATE_CHILD);
    }
    if (chain == chain_last)
        return;
    chain_last = chain;
    swofty::log("[direct] flow chain: %s", chain.c_str());
}

std::atomic<runtime::FlowLinkListener> link_listener{nullptr};
// Set when the start arms, cleared once ds::link_needed says the start has nothing left to do at the flow link.
std::atomic<bool> link_live{false};

// The listener (a Theatre pick's viewer) is handed the node unread: it checks its own stage before it reads anything.
void link_mid(safetyhook::Context &c)
{
    auto listener = link_listener.load(std::memory_order_relaxed);
    bool live = link_live.load(std::memory_order_relaxed);
    if (!listener && !live)
        return;
    auto *state = reinterpret_cast<uint8_t *>(c.rdi);
    if (!state)
        return;
    auto *slot = reinterpret_cast<uint32_t *>(c.rsp + LINK_SLOT);
    if (listener)
        listener(state, slot);
    if (!live)
        return;
    swofty::mem::Trust trust(runtime::HEAP_TRUST_MS);
    if (!runtime::readable(state, STATE_PARENT + 8))
        return;
    uint32_t hash = *reinterpret_cast<uint32_t *>(state + STATE_HASH);
    bool child = *reinterpret_cast<void **>(state + STATE_CHILD) != nullptr;
    std::lock_guard<std::mutex> g(lock);
    if (hash == STATE_APP)
        flow_root = state;
    if (hash == STATE_SESSION && *reinterpret_cast<uint8_t *>(state + STATE_STATUS) == 2 &&
        session_frames.fetch_add(1) == 0 && driver.target.name == "session" && !fade_released) {
        session_at = GetTickCount64();
        release_frontend_fade();
        if (request["mode"].str() != "time_trial")
            despawn_frontend_scene("as the Grand Prix session starts");
        int id = testagent::session_id_of(request["session"].str());
        bool race = id >= 15 && id <= 17;
        race_session = race && request["mode"].str() != "time_trial";
        runtime::room_cover::step(race ? "Lining up on the grid" : "Opening the garage", 0.9f, 0.97f);
    }
    if (session_at.load())
        note_fade();
    uint32_t link = *slot, out;
    if (ftue_interrupt_on && hash == ds::STATE_FTUE && driver.phase == ds::Phase::leave && link != ds::LINK_STAY &&
        !driver.exited && !ftue_interrupt_raised.load() && raise_ftue_interrupt(state)) {
        ftue_interrupt_raised = true;
        out = ds::LINK_STAY;
        if (diag_on) {
            diag_snapshot("interrupt raised");
            diag_start();
        }
    } else
        out = driver.on_update(hash, link, child);
    if (diag_on && hash == STATE_LOAD_SESSION && !diag_load_told.exchange(true))
        diag_snapshot("StateLoadSession enters");
    if (diag_on && driver.phase == ds::Phase::left)
        diag_start();
    if (out != link) {
        *slot = out;
        if (out != ds::LINK_STAY)
            swofty::log("[direct] state %08x: link %08x replaced by %08x", hash, link, out);
    }
    if (driver.phase == ds::Phase::left) {
        describe_state(state, hash, out);
        if (flow_root && driver.target.name == "session")
            for (uint8_t *n = flow_root; n && runtime::readable(n, STATE_PARENT + 8);
                 n = *reinterpret_cast<uint8_t **>(n + STATE_CHILD))
                if (*reinterpret_cast<uint32_t *>(n + STATE_HASH) == STATE_LOAD_SESSION) {
                    static bool told = false;
                    if (!told && driver.target.name == "session") {
                        told = true;
                        std::string track = request["track"].str();
                        static std::string what;
                        what = track.empty() ? "Loading the session" : "Loading " + track;
                        runtime::room_cover::step(what.c_str(), 0.58f, 0.82f);
                        if (request["mode"].str() != "time_trial")
                            choose_session(modes_seen.load(), "load");
                    }
                    load_exit(n);
                    break;
                }
        uint64_t now = GetTickCount64();
        if (now - chain_at >= 1000) {
            chain_at = now;
            log_chain();
        }
    }
    note_phase();
    if (!ds::link_needed(driver.phase, driver.target.name == "session", session_screen.load(), session_at.load(), GetTickCount64(),
                         SESSION_SCREEN_WAIT_MS, diag_on) &&
        link_live.exchange(false))
        swofty::log("[direct] %s start: the flow link has nothing left to do (%s); it returns at once from now on", driver.target.name.c_str(),
                    ds::phase_name(driver.phase));
}

void *registry_call(uint8_t *entity, size_t slot, const void *a2, int a3)
{
    auto *reg = entity + ENTITY_REGISTRY;
    auto **vt = *reinterpret_cast<void ***>(reg);
    return reinterpret_cast<void *(*)(void *, const void *, int)>(vt[slot / 8])(reg, a2, a3);
}

runtime::DirectStartSetup room_setup = nullptr, time_trial_setup = nullptr;
void (*time_trial_screen)(const char *name) = nullptr;

// The race's laps: Game_ChampionshipGameMode_BuildSessionSettings (0x1407b7b90, the championship modes' slot) fills
// the session's SessionSettings (sessionId +4) and stores the laps at +0xdf0 XOR +0xdf8; for a race session the core
// writes the requested laps over it after the game's own value.
safetyhook::InlineHook h_settings;

uint64_t settings_detour(void *mode, uint8_t *settings)
{
    uint64_t r = h_settings.call<uint64_t>(mode, settings);
    if (!armed.load() || !settings)
        return r;
    long long laps = request["laps"].num(0);
    uint32_t id = *reinterpret_cast<uint32_t *>(settings + 4);
    if (laps > 0 && laps <= 100 && id >= 15 && id <= 17 && request["mode"].str() != "time_trial") {
        auto &stored = *reinterpret_cast<uint32_t *>(settings + 0xdf0);
        uint32_t key = *reinterpret_cast<uint32_t *>(settings + 0xdf8), was = stored ^ key;
        stored = uint32_t(laps) ^ key;
        swofty::log("[direct] race laps %u -> %lld (session %s)", was, laps, testagent::SESSION_NAMES[id]);
    }
    return r;
}
std::string request_text;
std::atomic<bool> setup_ran{false};
runtime::DirectStartContext context;

void fill_context()
{
    if (!game.manager)
        return;
    void *mgr = *reinterpret_cast<void **>(game.manager);
    context.game_manager = mgr;
    void *modes = modes_seen.load();
    if (!modes && mgr && runtime::readable(mgr, MANAGER_MODES + 8))
        modes = *reinterpret_cast<void **>(static_cast<uint8_t *>(mgr) + MANAGER_MODES);
    context.game_modes = modes;
}

template <class F> F *fn(uintptr_t at)
{
    return reinterpret_cast<F *>(at);
}

uint8_t *grand_prix_entity(void *modes)
{
    void *pred = reinterpret_cast<void *>(game.gp_predicate);
    uint8_t *gp = reinterpret_cast<ModeByPredicate_t *>(game.by_predicate)(modes, &pred);
    return gp && runtime::readable(gp, ENTITY_TRACKS + 8) ? gp : nullptr;
}

// The weekend the session load reads: the current mode's championship holder ([modes+0x88]) > its entity (+0x260)
// > the current championship (+0x1058) > seasons[season_index] (+0x548, +0x560, 16-byte entries, season at +8) >
// weekends[current_weekend] (+0x288, +0x2a0) (apply/moments.cpp reads the same chain for the session facts).
constexpr size_t WK_SESSIONS = 0x220, WK_CURRENT = 0x238, WK_TRACK = 0x278, SESSION_STRIDE = 0xa0, SESSION_ID = 0x8;

template <class T> bool rd(const void *p, size_t off, T &out)
{
    auto *a = static_cast<const uint8_t *>(p) + off;
    if (!p || !runtime::readable(a, sizeof(T)))
        return false;
    out = *reinterpret_cast<const T *>(a);
    return true;
}

uint8_t *current_weekend(void *modes)
{
    uint8_t *holder = nullptr, *entity = nullptr, *champ = nullptr, *sb = nullptr, *se = nullptr, *season = nullptr,
            **wb = nullptr, **we = nullptr, *wk = nullptr;
    uint32_t si = 0, wi = 0;
    if (!rd(modes, 0x88, holder) || !rd(holder, 0x260, entity) || !rd(entity, 0x1058, champ) ||
        !rd(champ, 0x548, sb) || !rd(champ, 0x550, se) || !rd(champ, 0x560, si) || se <= sb ||
        si >= size_t(se - sb) / 16 || !rd(sb, size_t(si) * 16 + 8, season) || !rd(season, 0x288, wb) ||
        !rd(season, 0x290, we) || !rd(season, 0x2a0, wi) || we <= wb || wi >= size_t(we - wb) ||
        !rd(wb, size_t(wi) * 8, wk) || !runtime::readable(wk, 0x280))
        return nullptr;
    return wk;
}

std::string weekend_text(uint8_t *wk)
{
    auto *b = *reinterpret_cast<uint8_t **>(wk + WK_SESSIONS), *e = *reinterpret_cast<uint8_t **>(wk + WK_SESSIONS + 8);
    uint32_t cur = *reinterpret_cast<uint32_t *>(wk + WK_CURRENT);
    std::string ids;
    for (auto *p = b; p && p < e && runtime::readable(p, SESSION_STRIDE); p += SESSION_STRIDE) {
        uint32_t id = *reinterpret_cast<uint32_t *>(p + SESSION_ID);
        ids += (ids.empty() ? "" : " ") + std::string(id < 19 ? testagent::SESSION_NAMES[id] : "?");
    }
    char head[96];
    snprintf(head, sizeof head, "weekend %p, track stat id %u, current session %u of: ", wk,
             unsigned(wk[WK_TRACK]), cur);
    return head + ids;
}

bool set_weekend_track(void *modes, uint8_t id, const char *track_name, const char *when)
{
    uint8_t *wk = current_weekend(modes);
    if (!wk)
        return false;
    if (wk[WK_TRACK] != id) {
        swofty::log("[direct] Grand Prix: the weekend's track stat id %u -> %u (%s) %s", unsigned(wk[WK_TRACK]),
                    unsigned(id), track_name, when);
        wk[WK_TRACK] = id;
    }
    return true;
}

// The requested session (the CLI's session=, a game session name or practice1..3, qualifying, race) made the
// weekend's current one. A Grand Prix the profile already holds keeps its season (the season build only runs for a
// mode with no season), so the weekend is the saved one: its first session becomes the requested one and the
// weekend starts there. Moving the current index past a session instead leaves the race without the results it
// builds the grid from (the AI's team performance build faulted on a race behind an unplayed qualifying).
void choose_session(void *modes, const char *when)
{
    uint8_t *wk = current_weekend(modes);
    if (!wk) {
        swofty::log("[direct] Grand Prix (%s): no current weekend to read", when);
        return;
    }
    swofty::log("[direct] Grand Prix (%s): %s", when, weekend_text(wk).c_str());
    std::string name = request["session"].str();
    if (name.empty())
        return;
    int want = testagent::session_id_of(name);
    if (want < 0 || want == testagent::SESSION_TIME_TRIAL) {
        swofty::log("[direct] Grand Prix: session '%s' is not a Grand Prix session; the weekend's own is kept", name.c_str());
        return;
    }
    auto *b = *reinterpret_cast<uint8_t **>(wk + WK_SESSIONS), *e = *reinterpret_cast<uint8_t **>(wk + WK_SESSIONS + 8);
    auto &cur = *reinterpret_cast<uint32_t *>(wk + WK_CURRENT);
    uint32_t n = b && e > b ? uint32_t(size_t(e - b) / SESSION_STRIDE) : 0;
    if (!n)
        return;
    if (cur != 0) {
        swofty::log("[direct] Grand Prix: current session %u -> 0", cur);
        cur = 0;
    }
    auto &id = *reinterpret_cast<uint32_t *>(b + SESSION_ID);
    bool family = (name == "qualifying" && id >= 5 && id <= 9) || (name == "sprint_shootout" && id >= 10 && id <= 14);
    if (family) {
        swofty::log("[direct] Grand Prix: session 0 %s is the weekend's own %s; it is kept", testagent::SESSION_NAMES[id],
                    name.c_str());
        return;
    }
    if (id != uint32_t(want)) {
        swofty::log("[direct] Grand Prix: session 0 %s -> %s", id < 19 ? testagent::SESSION_NAMES[id] : "?",
                    testagent::SESSION_NAMES[want]);
        id = uint32_t(want);
    }
}

// The weekend's sessions as the dev session's Apply (0x141125e80) builds them: Sess_Build_DevWeekendStructure
// (0x140b025f0) on a 0x38-byte list {+8, +0x10 the allocator, +0x18..+0x20 16-byte entries} with the game setup mode
// ([E+0x2b0]) and a (practice, qualifying, race) triple from the session name, handed to the weekend service
// (registry component 0x14605ada0) slot 0x268; then the entries are released and the list freed.

struct Triple {
    uint64_t practice, qualifying, race;
};

bool session_triple(int id, Triple &t)
{
    if (id == 4)
        t = {2, 0, 3};
    else if (id >= 1 && id <= 3)
        t = {6, 0, 3};
    else if (id == 5 || id == 6 || id == 7)
        t = {0, 4, 3};
    else if (id == 8)
        t = {0, 3, 3};
    else if (id == 9)
        t = {0, 1, 3};
    else if (id >= 10 && id <= 14)
        t = {0, 3, 3};
    else if (id >= 15 && id <= 17)
        t = {0, 0, 7};
    else
        return false;
    return true;
}

void weekend_structure(uint8_t *e, uint8_t *component)
{
    std::string name = request["session"].str();
    if (name.empty())
        return;
    int id = testagent::session_id_of(name);
    if (name == "practice")
        id = 4;
    Triple t{};
    if (!session_triple(id, t)) {
        swofty::log("[direct] Grand Prix: no weekend structure for session '%s'", name.c_str());
        return;
    }
    void *alloc = *reinterpret_cast<void **>(component + 0x210), *gsm = *reinterpret_cast<void **>(e + 0x2b0);
    void *svc = registry_call(e, 0x10, reinterpret_cast<void *>(game.weekend_service), 1);
    if (!alloc || !gsm || !svc) {
        swofty::log("[direct] Grand Prix: the weekend structure is not set (allocator %p, setup mode %p, service %p)",
                    alloc, gsm, svc);
        return;
    }
    alignas(16) uint8_t list[0x38] = {};
    *reinterpret_cast<void **>(list + 8) = alloc;
    *reinterpret_cast<void **>(list + 0x10) = alloc;
    fn<void(void *, void *, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t)>(
        game.weekend_structure)(list, gsm, t.practice, t.qualifying, t.race, 0, 1, 0, 0);
    auto **svt = *reinterpret_cast<void ***>(svc);
    reinterpret_cast<void (*)(void *, void *, int)>(svt[0x268 / 8])(svc, list, 1);
    auto *b = *reinterpret_cast<uint8_t **>(list + 0x18), *end = *reinterpret_cast<uint8_t **>(list + 0x20);
    size_t n = 0;
    for (auto *it = b; it && it < end; it += 0x10, n++) {
        auto *o = *reinterpret_cast<uint8_t **>(it + 8);
        if (!o)
            continue;
        auto **ovt = *reinterpret_cast<void ***>(o);
        if (InterlockedDecrement(reinterpret_cast<volatile LONG *>(o + 8)) == 0) {
            reinterpret_cast<void (*)(void *)>(ovt[0])(o);
            if (InterlockedDecrement(reinterpret_cast<volatile LONG *>(o + 0xc)) == 0)
                reinterpret_cast<void (*)(void *)>(ovt[1])(o);
        }
    }
    if (b)
        reinterpret_cast<void (*)(void *, void *)>((*reinterpret_cast<void ***>(alloc))[0x38 / 8])(alloc, b);
    swofty::log("[direct] Grand Prix: weekend structure for %s: practice %llu, qualifying %llu, race %llu (%zu action(s))",
                name.c_str(), (unsigned long long)t.practice, (unsigned long long)t.qualifying,
                (unsigned long long)t.race, n);
}

// The player's seat, as Apply's drivers config (0x141129850) sets it: the first entrant of the requested team (and
// driver) that is not human gets the player's identity block (the profile [[[game manager+0x140]+0x1c0]+0x1c0]
// slot 0x38, 0x40 bytes to +8) and the human flag (+0x48 = 0x10000000), between the entrants-changing messages.

std::map<uint32_t, std::string> labels_of(const char *table, const char *stem)
{
    std::map<uint32_t, std::string> out;
    const SwoftyDb *db = runtime::db_table();
    if (!db || !db->available || !db->available())
        return out;
    int64_t count = 0;
    std::string t = table;
    if (db->int_at((t + "._" + stem + "_instancecount_").c_str(), &count) != SWOFTY_ASSET_OK || count <= 0 || count > 8192)
        return out;
    for (int64_t i = 0; i < count; i++) {
        std::string row = t + "." + stem + "_" + std::to_string(i);
        int64_t id = -1;
        const char *label = nullptr;
        if (db->int_at((row + ".stat_id").c_str(), &id) == SWOFTY_ASSET_OK && id >= 0 && id < 0x3fff &&
            db->string_at((row + "._metadata_").c_str(), &label) == SWOFTY_ASSET_OK && label)
            out[uint32_t(id)] = std::string(label, strnlen(label, 128));
    }
    return out;
}

bool label_matches(const std::string &label, const std::string &want)
{
    if (want.empty())
        return true;
    if (label == want)
        return true;
    std::string l = label, w = want;
    for (auto &c : l)
        c = char(tolower(uint8_t(c)));
    for (auto &c : w)
        c = char(tolower(uint8_t(c)));
    return l.find(w) != std::string::npos;
}

void seat_player(uint8_t *e, void *mgr)
{
    std::string team = request["team"].str(), driver = request["driver"].str();
    auto **begin = reinterpret_cast<uint8_t **>(e + ENTITY_ENTRANTS);
    size_t n = size_t(begin[1] - begin[0]) / ENTRANT_SIZE;
    if (!n)
        return;
    auto teams = labels_of("entrants.teams", "team"), drivers = labels_of("entrants.drivers", "driver");
    auto team_of = [&](uint8_t *r) -> std::string {
        if (r[0x58])
            return "?";
        auto it = teams.find(*reinterpret_cast<uint16_t *>(r + 0x5a));
        return it == teams.end() ? "#" + std::to_string(*reinterpret_cast<uint16_t *>(r + 0x5a)) : it->second;
    };
    auto driver_of = [&](uint8_t *r) -> std::string {
        if (r[0x50])
            return "?";
        auto it = drivers.find(*reinterpret_cast<uint16_t *>(r + 0x52));
        return it == drivers.end() ? "#" + std::to_string(*reinterpret_cast<uint16_t *>(r + 0x52)) : it->second;
    };
    uint8_t *seat = nullptr;
    size_t humans = 0;
    std::string grid;
    for (size_t i = 0; i < n; i++) {
        uint8_t *r = begin[0] + i * ENTRANT_SIZE;
        bool human = (r[0x4b] & 0x10) != 0;
        humans += human;
        if (grid.size() < 1500)
            grid += (grid.empty() ? "" : ", ") + team_of(r) + "/" + driver_of(r) + (human ? " (human)" : "");
        if (!seat && !human && label_matches(team_of(r), team) && label_matches(driver_of(r), driver))
            seat = r;
    }
    swofty::log("[direct] Grand Prix: %zu entrant(s), %zu human: %s", n, humans, grid.c_str());
    if (!seat) {
        if (!team.empty() || !driver.empty())
            swofty::log("[direct] Grand Prix: no entrant of team '%s' driver '%s'; the player takes the first seat",
                        team.c_str(), driver.c_str());
        seat = begin[0];
        if (seat[0x4b] & 0x10) {
            swofty::log("[direct] Grand Prix: the first seat is already the player's (%s/%s)", team_of(seat).c_str(),
                        driver_of(seat).c_str());
            return;
        }
    }
    alignas(16) uint8_t id[0x40] = {};
    void *users = mgr ? *reinterpret_cast<void **>(static_cast<uint8_t *>(mgr) + 0x140) : nullptr;
    void *user = users && runtime::readable(users, 0x1c8) ? *reinterpret_cast<void **>(static_cast<uint8_t *>(users) + 0x1c0) : nullptr;
    void *prof = user && runtime::readable(user, 0x1c8) ? *reinterpret_cast<void **>(static_cast<uint8_t *>(user) + 0x1c0) : nullptr;
    if (!prof) {
        swofty::log("[direct] Grand Prix: no player profile for the identity block; the seat is not set");
        return;
    }
    reinterpret_cast<void (*)(void *, void *)>((*reinterpret_cast<void ***>(prof))[0x38 / 8])(prof, id);
    void *bus = *reinterpret_cast<void **>(game.message_bus);
    void *mb = bus && runtime::readable(bus, 0x10) ? *reinterpret_cast<void **>(static_cast<uint8_t *>(bus) + 8) : nullptr;
    alignas(16) void *msg[3] = {reinterpret_cast<void *>(game.vt_msg_begin), nullptr, nullptr};
    if (mb)
        fn<void(void *, void *)>(game.post_begin)(mb, msg);
    e[0x2f8] = 1;
    for (size_t i = 0; i < n; i++) {
        uint8_t *r = begin[0] + i * ENTRANT_SIZE;
        if (r != seat && (r[0x4b] & 0x10)) {
            *reinterpret_cast<uint32_t *>(r + 0x48) &= ~0x10000000u;
            swofty::log("[direct] Grand Prix: %s/%s is no longer the player's", team_of(r).c_str(), driver_of(r).c_str());
        }
    }
    std::memcpy(seat + 8, id, 0x40);
    *reinterpret_cast<uint32_t *>(seat + 0x48) = 0x10000000;
    msg[0] = reinterpret_cast<void *>(game.vt_msg_end);
    msg[1] = nullptr;
    msg[2] = e + 0x2d8;
    if (mb)
        fn<void(void *, void *)>(game.post_end)(mb, msg);
    e[0x2f8] = 0;
    swofty::log("[direct] Grand Prix: the player drives for %s as %s (seat %zu)", team_of(seat).c_str(),
                driver_of(seat).c_str(), size_t(seat - begin[0]) / ENTRANT_SIZE);
}

ds::Setup new_grand_prix(std::string &why)
{
    if (!game.grand_prix()) {
        why = "one of the Grand Prix weekend's game addresses did not resolve on this build";
        return ds::Setup::failed;
    }
    void *mgr = *reinterpret_cast<void **>(game.manager);
    void *modes = modes_seen.load();
    if (!mgr || !modes)
        return ds::Setup::pending;
    uint8_t *e = grand_prix_entity(modes);
    if (!e)
        return ds::Setup::pending;
    // The mode attaches its components over the frames after it is made current, so the weekend is built on a
    // later tick, once the rounds component is there (looked up without creating it: 0x1407c1200, mode 0).
    static uint64_t mode_set_at = 0;
    if (!mode_set_at) {
        reinterpret_cast<SetMode_t *>(game.set_mode)(modes, *reinterpret_cast<void **>(e + ENTITY_MODE));
        mode_set_at = GetTickCount64();
        swofty::log("[direct] Grand Prix: the Grand Prix mode is current (entity %p); waiting for its components", e);
        return ds::Setup::pending;
    }
    void *rounds_seen = registry_call(e, 0x10, reinterpret_cast<void *>(game.rounds_component), 0);
    if (!rounds_seen) {
        if (GetTickCount64() - mode_set_at < 15000)
            return ds::Setup::pending;
        why = "the Grand Prix mode's rounds component did not appear within 15 s of making the mode current";
        return ds::Setup::failed;
    }
    swofty::log("[direct] Grand Prix: rounds component %p after %llu ms", rounds_seen,
                (unsigned long long)(GetTickCount64() - mode_set_at));
    std::string requested_mode = request["setup_mode"].str(), list_name = request["box"].str(DEFAULT_BOX),
                track_name = request["track"].str(DEFAULT_TRACK);
    // 1. The entrant list by name: EntrantsSystem +0x560, 16-byte entries, the list at +8.
    uint8_t *es = *reinterpret_cast<uint8_t **>(static_cast<uint8_t *>(mgr) + runtime::manager_entrants_system());
    uint8_t id = fn<uint8_t(const char *)>(game.list_stat_id)(list_name.c_str());
    auto *lists = *reinterpret_cast<uint8_t **>(es + ES_LISTS), *lists_end = *reinterpret_cast<uint8_t **>(es + ES_LISTS + 8);
    if (!lists || size_t(lists_end - lists) / 16 <= id) {
        why = "no entrant list '" + list_name + "' (stat id " + std::to_string(id) + ")";
        return ds::Setup::failed;
    }
    void *list = *reinterpret_cast<void **>(lists + size_t(id) * 16 + 8);
    if (!list) {
        std::string held;
        size_t count = size_t(lists_end - lists) / 16;
        for (size_t i = 0; i < count && held.size() < 1200; i++)
            if (*reinterpret_cast<void **>(lists + i * 16 + 8))
                held += " " + std::to_string(i);
        why = "the entrant list '" + list_name + "' (stat id " + std::to_string(id) + ") is not loaded";
        swofty::log("[direct] Grand Prix: %s; the entrants system holds %zu list slot(s), loaded:%s", why.c_str(), count,
                    held.c_str());
        return ds::Setup::failed;
    }
    // 2. The game setup mode from E's setup modes (vt+0x1d0: a vector at +8..+0x10): the requested one, else the
    // one that offers the list (entrant setup modes at +0x10, count +0x70; their lists at +0x130, count +0x230).
    auto **evt = *reinterpret_cast<void ***>(e);
    auto *modes_vec = reinterpret_cast<uint8_t *(*)(void *)>(evt[0x1d0 / 8])(e);
    std::vector<uint8_t *> setup_modes;
    std::vector<ds::SetupModeOffer> offers;
    for (auto **p = *reinterpret_cast<uint8_t ***>(modes_vec + 8), **end = *reinterpret_cast<uint8_t ***>(modes_vec + 0x10);
         p && p != end; ++p) {
        uint8_t *m = *p;
        void *holder = m && runtime::readable(m, 0xe0) ? *reinterpret_cast<void **>(m + 0xd8) : nullptr;
        const char *name = holder ? *reinterpret_cast<const char **>(static_cast<uint8_t *>(holder) + 0x10) : nullptr;
        if (!name)
            continue;
        bool offered = false;
        int64_t setups = *reinterpret_cast<int64_t *>(m + 0x70);
        for (int64_t i = 0; i < setups && i < 12 && !offered; i++) {
            uint8_t *esm = *reinterpret_cast<uint8_t **>(m + 0x10 + size_t(i) * 8);
            if (!esm || !runtime::readable(esm, 0x238))
                continue;
            int64_t n = *reinterpret_cast<int64_t *>(esm + 0x230);
            for (int64_t k = 0; k < n && k < 32 && !offered; k++)
                offered = *reinterpret_cast<void **>(esm + 0x130 + size_t(k) * 8) == list;
        }
        setup_modes.push_back(m);
        offers.push_back({name, offered});
    }
    size_t pick = ds::pick_setup_mode(offers, requested_mode, HOST_SETUP_MODE);
    if (pick == ds::NO_SETUP_MODE) {
        why = "the Grand Prix mode has no game setup mode '" +
              (requested_mode.empty() ? std::string(HOST_SETUP_MODE) : requested_mode) + "'";
        return ds::Setup::failed;
    }
    uint8_t *gsm = setup_modes[pick];
    const std::string &setup_name = offers[pick].name;
    std::string offering;
    for (auto &o : offers)
        if (o.offers)
            offering += (offering.empty() ? "" : ", ") + o.name;
    swofty::log("[direct] Grand Prix: setup mode %s for entrant list %s (%s; setup modes offering the list: %s)",
                setup_name.c_str(), list_name.c_str(),
                !requested_mode.empty() ? "requested" : offers[pick].offers ? "it offers the list" : "none offers the list",
                offering.empty() ? "none" : offering.c_str());
    fn<void(void *, void *, int)>(game.set_setup_mode)(e, gsm, 0);
    swofty::log("[direct] Grand Prix: setup mode %s (%p) set on %p", setup_name.c_str(), gsm, e);
    *reinterpret_cast<void **>(e + ENTITY_LIST) = list;
    swofty::log("[direct] Grand Prix: entrant list %s (stat id %u) is %p", list_name.c_str(), unsigned(id), list);
    // 3. The track by name in E's track system: 16-byte entries at +0x28..+0x30, the record at +8, its name at +0x78.
    uint8_t *tracks = *reinterpret_cast<uint8_t **>(e + ENTITY_TRACKS);
    uint8_t *track = nullptr;
    if (tracks && runtime::readable(tracks, 0x38))
        for (auto *q = *reinterpret_cast<uint8_t **>(tracks + 0x28), *qend = *reinterpret_cast<uint8_t **>(tracks + 0x30);
             q && q < qend; q += 16) {
            auto *rec = *reinterpret_cast<uint8_t **>(q + 8);
            const char *name = rec && runtime::readable(rec, 0x80) ? *reinterpret_cast<const char **>(rec + 0x78) : nullptr;
            if (name && track_name == name) {
                track = rec;
                break;
            }
        }
    if (!track) {
        why = "the Grand Prix mode's track system has no track '" + track_name + "'";
        return ds::Setup::failed;
    }
    // 4. One round at that track, then the season built from the rounds (0x141207290 on a stub that holds the
    // registry at +0x270, the rounds at +0x290 and the track at [+0x298]+0x18).
    void *rounds = registry_call(e, 0x10, reinterpret_cast<void *>(game.rounds_component), 0);
    if (!rounds) {
        why = "the Grand Prix mode has no rounds component";
        return ds::Setup::failed;
    }
    // The clear (0x14420cd60, a jump to runtime-decrypted code) reads its list holder from the second argument:
    // it starts mov rcx,[rdx], and the track select calls it with rdx = rounds+0x298.
    fn<void(void *, void *)>(game.rounds_clear)(nullptr, static_cast<uint8_t *>(rounds) + 0x298);
    fn<void(void *, void *)>(game.add_track)(rounds, track);
    alignas(16) static uint8_t stub[0x400], holder[0x40];
    std::memset(stub, 0, sizeof stub);
    std::memset(holder, 0, sizeof holder);
    *reinterpret_cast<void **>(stub + 0x270) = e + ENTITY_REGISTRY;
    *reinterpret_cast<void **>(stub + 0x290) = rounds;
    *reinterpret_cast<void **>(holder + 0x18) = track;
    *reinterpret_cast<void **>(stub + 0x298) = holder;
    fn<void(void *)>(game.build_season)(stub);
    set_weekend_track(modes, track[1], track_name.c_str(), "after the season build");
    // 5. E's components attached again, as the track select confirm does after its rounds, then the sessions.
    reinterpret_cast<void (*)(void *)>(evt[0xe8 / 8])(e);
    weekend_structure(e, static_cast<uint8_t *>(rounds));
    // 6. The entrants from the list. The fill clears the entrants a saved Grand Prix left first.
    auto **begin = reinterpret_cast<uint8_t **>(e + ENTITY_ENTRANTS);
    size_t before = size_t(begin[1] - begin[0]) / ENTRANT_SIZE;
    {
        alignas(16) static uint8_t blk[0x190];
        std::memset(blk, 0, sizeof blk);
        *reinterpret_cast<uint64_t *>(blk + 0x180) = 1;
        fn<void(void *, void *, void *, int)>(game.fill_entrants)(es, e, blk, 0);
    }
    size_t entrants = size_t(begin[1] - begin[0]) / ENTRANT_SIZE;
    swofty::log("[direct] Grand Prix: entrants %zu before the fill, %zu after", before, entrants);
    seat_player(e, mgr);
    swofty::log("[direct] Grand Prix: new weekend: setup mode %s, list %s (%p), track %s (%p), rounds %p, %zu entrant(s)",
                setup_name.c_str(), list_name.c_str(), list, track_name.c_str(), track, rounds, entrants);
    set_weekend_track(modes, track[1], track_name.c_str(), "after the entrants");
    choose_session(modes, "built");
    return ds::Setup::done;
}

ds::Setup set_up_target(std::string &why)
{
    if (driver.target.name == "session" && request["mode"].str() == "time_trial") {
        if (!time_trial_setup) {
            why = "no Time Trial setup is registered (direct_start_tt.cpp)";
            return ds::Setup::failed;
        }
        return time_trial_setup(why);
    }
    if (driver.target.name == "session")
        return new_grand_prix(why);
    if (driver.target.name == "room") {
        if (!room_setup) {
            why = "no setup is registered for this target (the session plugin is not loaded)";
            return ds::Setup::failed;
        }
        return room_setup(why);
    }
    return ds::Setup::done;
}

}

namespace runtime {

void direct_start_setup()
{
    struct Ran {
        ~Ran() { setup_ran = true; }
    } ran;
    char env[1024] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_DIRECT_START", env, sizeof env);
    std::string target, text;
    if (n && n < sizeof env && env[0] == '@')
        text = runtime::read_file(runtime::wide(env + 1), size_t(1) << 16);
    else if (n && n < sizeof env)
        target = env;
    else
        text = runtime::read_file(runtime::swofty_root() + L"direct_start.json", size_t(1) << 16);
    if (target.empty()) {
        if (text.empty())
            return;
        request_text = text;
        if (!json::parse(text, request)) {
            swofty::log("[direct] the direct start request is unusable; the game starts as usual");
            return;
        }
        target = request["target"].str();
    }
    ds::Target t;
    if (target == "session")
        t = ds::session_target();
    else if (target == "room")
        t = ds::room_target();
    else {
        swofty::log("[direct] swofty\\direct_start.json names no known target ('%s'); the game starts as usual",
                    target.c_str());
        return;
    }
    game.manager = runtime::find(SIG_GAME_MANAGER, "game manager", "direct");
    game.set_mode = runtime::find(SIG_SET_MODE, "SetCurrentGameMode", "direct");
    game.by_predicate = runtime::find(SIG_MODE_BY_PREDICATE, "game mode lookup by predicate", "direct");
    game.gp_predicate = runtime::find(SIG_GP_PREDICATE, "Grand Prix mode predicate", "direct");
    game.root = runtime::direct_start::tt::game_root();
    game.despawn_main_scene = runtime::find(SIG_DESPAWN_MAIN_SCENE, "front end scene", "direct");
    if (target == "session") {
        game.set_setup_mode = runtime::direct_start::tt::set_setup_mode();
        game.list_stat_id = runtime::find(SIG_LIST_STAT_ID, "Grand Prix weekend", "direct");
        game.rounds_clear = runtime::find(SIG_ROUNDS_CLEAR, "Grand Prix weekend", "direct");
        game.add_track = runtime::find(SIG_ADD_TRACK, "Grand Prix weekend", "direct");
        game.build_season = runtime::find(SIG_BUILD_SEASON, "Grand Prix weekend", "direct");
        game.fill_entrants = runtime::find(SIG_FILL_ENTRANTS, "Grand Prix weekend", "direct");
        game.weekend_structure = runtime::find(SIG_WEEKEND_STRUCTURE, "Grand Prix weekend", "direct");
        game.weekend_service = runtime::find(SIG_WEEKEND_SERVICE, "Grand Prix weekend", "direct");
        game.rounds_component = runtime::find(SIG_ROUNDS_COMPONENT, "Grand Prix weekend", "direct");
        game.message_bus = runtime::find(SIG_MESSAGE_BUS, "Grand Prix weekend", "direct");
        game.post_begin = runtime::find(SIG_POST_BEGIN, "Grand Prix weekend", "direct");
        game.post_end = runtime::find(SIG_POST_END, "Grand Prix weekend", "direct");
        game.vt_msg_begin = runtime::find(SIG_VT_MSG_BEGIN, "Grand Prix weekend", "direct");
        game.vt_msg_end = runtime::find(SIG_VT_MSG_END, "Grand Prix weekend", "direct");
    }
    if (game.set_mode) {
        auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(game.set_mode),
                                                reinterpret_cast<void *>(set_mode_detour));
        if (h)
            h_set_mode = std::move(*h);
    }
    if (target == "session" && request["laps"].num(0) > 0) {
        uintptr_t at = runtime::verified_build() ? runtime::find(SIG_BUILD_SETTINGS, "session settings", "direct") : 0;
        bool hooked = false;
        if (at) {
            auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(settings_detour));
            if (h) {
                h_settings = std::move(*h);
                hooked = true;
            }
        }
        if (!hooked)
            swofty::log("[direct] the race laps are not set: Game_ChampionshipGameMode_BuildSessionSettings did not hook");
    }
    runtime::title::setup();
    if (!runtime::title::why().empty()) {
        swofty::log("[direct] off: the title can't pass by itself (%s)", runtime::title::why().c_str());
        return;
    }
    if (ftue_interrupt_on) {
        uintptr_t at = runtime::verified_build() ? runtime::find(SIG_FTUE_EXIT, "StateFTUE exit", "direct") : 0;
        bool hooked = false;
        if (at) {
            auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(at), reinterpret_cast<void *>(ftue_exit_detour));
            if (h) {
                h_ftue_exit = std::move(*h);
                hooked = true;
            }
        }
        if (!hooked) {
            ftue_interrupt_on = false;
            swofty::log("[direct] FTUE interrupt off: StateFTUE::onStateExit did not hook; leaving by the link");
        }
    }
    if (t.exit_state || t.from_state) {
        uintptr_t at = runtime::verified_build() ? runtime::find(SIG_FLOW_LINK, "flow link", "direct") : 0;
        if (!at) {
            swofty::log("[direct] off: the flow link site is not verified or did not resolve on this build");
            return;
        }
        auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), link_mid);
        if (!h) {
            swofty::log("[direct] off: the hook on the flow link site failed: %s", runtime::hook_error(h.error()).c_str());
            return;
        }
        h_link = std::move(*h);
    }
    {
        std::lock_guard<std::mutex> g(lock);
        driver.arm(t);
        note_phase();
    }
    runtime::title::want(true);
    armed = true;
    link_live = true;
}

void log_stack(const char *what)
{
    void *frames[24] = {};
    USHORT n = RtlCaptureStackBackTrace(1, 24, frames, nullptr);
    swofty::log("[direct] %s, thread %lu, from:", what, GetCurrentThreadId());
    for (USHORT i = 0; i < n; i++)
        swofty::log("[direct]   #%u %s", unsigned(i), runtime::code_where(reinterpret_cast<uintptr_t>(frames[i])).c_str());
}

uintptr_t direct_start_set_mode()
{
    if (game.set_mode)
        return game.set_mode;
    static std::atomic<uintptr_t> found{0};
    static std::atomic<bool> tried{false};
    if (!tried.exchange(true)) {
        found = runtime::verified_build() ? runtime::find(SIG_SET_MODE, "SetCurrentGameMode", "direct") : 0;
        swofty::log("[direct] SetCurrentGameMode for a session without a direct start: %s", found.load() ? "resolved" : "not resolved");
    }
    return found.load();
}

DirectStartContext direct_start_context()
{
    if (game.manager)
        return context;
    static std::atomic<uintptr_t> manager{0};
    static std::atomic<bool> tried{false};
    if (!manager.load() && !tried.exchange(true)) {
        manager = runtime::verified_build() ? runtime::find(SIG_GAME_MANAGER, "game manager", "direct") : 0;
        swofty::log("[direct] game manager for a session without a direct start: %s", manager.load() ? "resolved" : "not resolved");
    }
    DirectStartContext c;
    if (!manager.load() || !runtime::readable(reinterpret_cast<void *>(manager.load()), 8))
        return c;
    void *mgr = *reinterpret_cast<void **>(manager.load());
    void *modes = modes_seen.load();
    if (!modes && mgr && runtime::readable(mgr, MANAGER_MODES + 8))
        modes = *reinterpret_cast<void **>(static_cast<uint8_t *>(mgr) + MANAGER_MODES);
    c.game_manager = mgr;
    c.game_modes = modes;
    return c;
}

void direct_start_register_room(DirectStartSetup setup)
{
    room_setup = setup;
}

void direct_start_register_time_trial(DirectStartSetup setup)
{
    time_trial_setup = setup;
}

std::string direct_start_request_text()
{
    return request_text;
}

int direct_start_cover_signal()
{
    if (!setup_ran.load())
        return 0;
    if (!armed.load())
        return -1;
    if (start_failed.load())
        return -1;
    if (session_screen.load())
        return 1;
    uint64_t at = session_at.load();
    return at && GetTickCount64() - at >= SESSION_SCREEN_WAIT_MS ? 1 : 0;
}

bool flow_link_listen(FlowLinkListener listener)
{
    link_listener = listener;
    if (h_link)
        return true;
    uintptr_t at = runtime::verified_build() ? runtime::find(SIG_FLOW_LINK, "flow link", "direct") : 0;
    if (!at)
        return false;
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), link_mid);
    if (!h) {
        swofty::log("[direct] the hook on the flow link site failed: %s", runtime::hook_error(h.error()).c_str());
        return false;
    }
    h_link = std::move(*h);
    return true;
}

void *direct_start_flow_root()
{
    return flow_root;
}

void direct_start_despawn_frontend_scene(const char *when)
{
    despawn_frontend_scene(when);
}

void direct_start_register_time_trial_screen(void (*screen)(const char *name))
{
    time_trial_screen = screen;
}

void direct_start_screen(const char *name)
{
    if (armed.load() && name && time_trial_screen)
        time_trial_screen(name);
    if (!armed.load() || !name || !session_at.load() || session_screen.load())
        return;
    static std::atomic<int> told{0};
    if (told.fetch_add(1) < 40)
        swofty::log("[direct] screen %s opened %llu ms into the session", name,
                    (unsigned long long)(GetTickCount64() - session_at.load()));
    if (!std::strcmp(name, "garage_cockpit") || !std::strcmp(name, "race_hud") || !std::strcmp(name, "grid")) {
        session_screen = true;
        swofty::log("[direct] the session's screen %s is up after %llu ms; the cover comes off", name,
                    (unsigned long long)(GetTickCount64() - session_at.load()));
    }
}

void direct_start_profile_read()
{
    static std::atomic<bool> once{false};
    if (armed.load() && !once.exchange(true))
        log_stack("the first profile file read");
}

void direct_start_profile_loaded(bool ok)
{
    if (!armed.load())
        return;
    static std::atomic<bool> once{false};
    if (!once.exchange(true))
        log_stack("the profile root load");
    std::lock_guard<std::mutex> g(lock);
    if (!ok) {
        swofty::log("[direct] the profile load failed; the start waits for the next one");
        return;
    }
    runtime::room_cover::step("Profile loaded", 0.4f, 0.54f);
    driver.profile_loaded();
    note_phase();
}

void direct_start_tick()
{
    if (!armed.load())
        return;
    std::lock_guard<std::mutex> g(lock);
    if (driver.phase == ds::Phase::setup) {
        fill_context();
        std::string why;
        ds::Setup r = set_up_target(why);
        driver.setup_result(r, why);
        if (r == ds::Setup::done && driver.target.name == "session")
            runtime::room_cover::step("Weekend ready", 0.5f, 0.56f);
    }
    // A session keeps the virtual pad plugged in, idle: the game bound the player to it at the title, and letting
    // go of the claim would stop the session on CONTROLLER DISCONNECTED.
    if (!driver.engage_title() && runtime::title::wanted() && !runtime::test_run() && driver.target.name != "session")
        runtime::title::want(false);
    note_phase();
}

}
