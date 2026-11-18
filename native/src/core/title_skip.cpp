#include "title_skip.hpp"

#include "runtime.hpp"
#include "virtual_pad.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdio>
#include <cstdlib>
#include <mutex>

namespace {

// StateIntro's onStateUpdate (FeFlow_StateIntro_OnUpdate 0x1436da4d0, slot 9 of vt_Sess_Intro 0x14585bff0,
// L1) waits until the title's sync point (state+0xcd0, sync_points.boot; Vm_SyncPoint_IsComplete 0x1442626c0, a
// protector thunk, found through the update's own call) has both bool nodes true: the request node +0x80 and the
// UI's node +0xa0, which the title's UI sets on a button. Then it sends its own request (kind 0x2e), which loads the
// profile and whose callback (FeFlow_StateIntro_OnEngaged) sets +0xef4, and the next update returns the
// front-end link. The core sets the two nodes with Vm_NodeSet_Bool (0x140973e20, (node, &value, 0, 0) as the
// sync point's own writes do) once the title's view exists ([[state+0xc88]+0x140]+0x198), so the game's own
// path runs from there.
const runtime::Sig SIG_INTRO_UPDATE{"fe_flow.state_intro_on_update", "41 57 41 56 56 57 53 48 83 EC 70 48 89 D6 48 89 CF 48 8B 89 F8 0B 00 00", 0x36da4d0};
const runtime::Sig SIG_NODE_SET_BOOL{"vm.node_set_bool", "41 56 56 57 55 53 48 81 EC 20 03 00 00 F6 41 26 01 0F 85 ?? ?? ?? ?? 44 89 CB 44 89 C7 49 89 D6 48 89 CE 48 8B 49 58 48 8B 01 FF 50 38 84 DB 75 0E 0F B6 08", 0x973e20};
const runtime::Sig SIG_SYNC_COMPLETE{"vm.sync_point_is_complete", "48 8D 8F D0 0C 00 00 E8 ?? ?? ?? ?? 84 C0", 0x42626c0,
                                     runtime::Sig::rel32, 7, 8, 5};
const runtime::Sig SIG_INTRO_VT{"session.intro", "48 8D 05 ?? ?? ?? ?? 48 89 01 48 81 C1 E0 0D 00 00", 0x585bff0, runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_BOOL_TYPE{"vm.bool_type_desc", "48 8D 0D ?? ?? ?? ?? FF 50 10 39 C3 75 17 48 8B 07", 0x6129dc0, runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_EMPTY_NODE{"vm.empty_node_handle", "0F 85 ?? ?? ?? ?? 48 8B 1D ?? ?? ?? ?? C5 F8 57 C0 C5 F8 29 44 24 30 48 C7 44 24 40 00 00 00 00 48 8D 15 ?? ?? ?? ?? 4C 8D 05", 0x6ca0e90, runtime::Sig::riprel, 39, 42, 7};

constexpr size_t INTRO_SYNC = 0xcd0, SYNC_REQUEST = 0x80, SYNC_UI = 0xa0, INTRO_SENT = 0xef3, INTRO_ENGAGED = 0xef4;

using IntroUpdate_t = uint32_t *(void *state, uint32_t *link);
using NodeSetBool_t = void(void *node, const uint8_t *value, int a3, int a4);
using SyncComplete_t = bool(void *sync);

struct Resolved {
    uintptr_t intro_update = 0, node_set_bool = 0, sync_complete = 0, intro_vt = 0, bool_type = 0, empty_node = 0;
} ga;

std::once_flag once;
safetyhook::InlineHook h_intro;
std::atomic<void *> intro_state{nullptr};
std::atomic<bool> title_wanted{false}, title_engaged{false}, title_set{false};
std::atomic<uint32_t> title_view{0};
std::string title_why = "not set up";

// In game the title's view only comes up after a press, so the sync point alone never passes it: while the
// title is wanted and has not engaged 3 s after that, the virtual pad holds a for 250 ms and lets go for
// 1750 ms, until the title engages.
constexpr uint64_t PRESS_HOLD = 250, PRESS_GAP = 1750;
std::atomic<uint64_t> press_after{3000};
std::atomic<uint64_t> wanted_at{0};
uint64_t press_at = 0;
bool pressing = false;
int presses = 0;

void press_tick()
{
    uint64_t now = GetTickCount64(), since = wanted_at.load();
    if (!since || now - since < press_after.load() || !runtime::pad::why().empty())
        return;
    if (pressing && now - press_at >= PRESS_HOLD) {
        runtime::pad::release();
        pressing = false;
    } else if (!pressing && (!press_at || now - press_at >= PRESS_HOLD + PRESS_GAP)) {
        runtime::pad::set(runtime::pad::A);
        pressing = true;
        press_at = now;
        presses++;
    }
}

uint8_t *addr_of(uintptr_t a)
{
    return reinterpret_cast<uint8_t *>(a);
}

// A live bool node: not null, not the empty handle, not read-only (+0x26 bit 0), valid (vt+0x20) and
// holding the bool type (as the sync point's own checks).
bool bool_node(void *node)
{
    void *empty = *reinterpret_cast<void **>(ga.empty_node);
    if (!node || node == empty || !runtime::readable(node, 0x60) || (static_cast<uint8_t *>(node)[0x26] & 1))
        return false;
    auto **vt = *static_cast<void ***>(node);
    if (!runtime::readable(vt, 0x28) || !reinterpret_cast<bool (*)(void *)>(vt[4])(node))
        return false;
    void *holder = static_cast<void **>(node)[0xb];
    if (!holder || !runtime::readable(holder, 8))
        return false;
    auto **hvt = *static_cast<void ***>(holder);
    void *type = reinterpret_cast<void *(*)(void *)>(hvt[1])(holder);
    auto *bool_type = addr_of(ga.bool_type);
    auto **tvt = *reinterpret_cast<void ***>(type);
    auto **bvt = *reinterpret_cast<void ***>(bool_type);
    return reinterpret_cast<int (*)(void *)>(tvt[2])(type) == reinterpret_cast<int (*)(void *)>(bvt[2])(bool_type);
}

bool node_value(void *node)
{
    void *holder = static_cast<void **>(node)[0xb];
    auto **hvt = *static_cast<void ***>(holder);
    return *reinterpret_cast<const char *(*)(void *)>(hvt[7])(holder) != 0;
}

// The player manager (NePlayerManager, [[state+0xc88]+0x140]): +0x198 the primary player, which the title's
// press makes (sign-in state +0x1f0: 1 or 4 starts the platform sign-in, 3 completes it), +0x1b0 the lead local
// player, a player's +0x1c0 platform user handle and +0x1c8 flags (bit 0 user set, bit 1 logged in). Logged as it
// changes; with SWOFTY_DIRECT_SIGNIN=1 the title's first update starts the sign-in itself.
constexpr size_t PM_PRIMARY = 0x198, PM_LEAD = 0x1b0, PM_CANDIDATE = 0x1d0, PM_SIGNIN = 0x1f0, PL_USER = 0x1c0,
                 PL_FLAGS = 0x1c8;

uint8_t *player_manager(uint8_t *state)
{
    void *a = *reinterpret_cast<void **>(state + 0xc88);
    if (!a || !runtime::readable(a, 0x148))
        return nullptr;
    auto *m = *reinterpret_cast<uint8_t **>(static_cast<uint8_t *>(a) + 0x140);
    return m && runtime::readable(m, PM_SIGNIN + 8) ? m : nullptr;
}

void player_probe(uint8_t *state)
{
    static std::string last;
    static bool signin_tried = false;
    uint8_t *m = player_manager(state);
    if (!m)
        return;
    auto *primary = *reinterpret_cast<uint8_t **>(m + PM_PRIMARY);
    auto *lead = *reinterpret_cast<uint8_t **>(m + PM_LEAD);
    uint64_t user = 0;
    uint32_t flags = 0;
    if (lead && runtime::readable(lead, PL_FLAGS + 4)) {
        user = *reinterpret_cast<uint64_t *>(lead + PL_USER);
        flags = *reinterpret_cast<uint32_t *>(lead + PL_FLAGS);
    }
    uint32_t signin = *reinterpret_cast<uint32_t *>(m + PM_SIGNIN);
    void *candidate = *reinterpret_cast<void **>(m + PM_CANDIDATE);
    char line[200];
    snprintf(line, sizeof line, "sign-in state %u, candidate %p, primary %p, lead %p user %llx flags %x", signin,
             candidate, primary, lead, (unsigned long long)user, flags);
    if (last != line) {
        last = line;
        swofty::log("[title] player manager %p: %s", m, line);
    }
    char env[8] = {};
    if (!signin_tried && signin == 0 && !primary && GetEnvironmentVariableA("SWOFTY_DIRECT_SIGNIN", env, sizeof env) &&
        env[0] == '1') {
        signin_tried = true;
        *reinterpret_cast<uint32_t *>(m + PM_SIGNIN) = 1;
        swofty::log("[title] SWOFTY_DIRECT_SIGNIN: player manager sign-in state set to 1");
    }
}

void title_nodes(uint8_t *state)
{
    void *view = nullptr;
    void *a = *reinterpret_cast<void **>(state + 0xc88);
    if (a && runtime::readable(a, 0x148)) {
        void *b = *reinterpret_cast<void **>(static_cast<uint8_t *>(a) + 0x140);
        if (b && runtime::readable(b, 0x1a0))
            view = *reinterpret_cast<void **>(static_cast<uint8_t *>(b) + 0x198);
    }
    title_view = view ? 1 : 2;
    if (!view)
        return;
    uint8_t *sync = state + INTRO_SYNC;
    const uint8_t one = 1;
    for (size_t off : {SYNC_REQUEST, SYNC_UI}) {
        void *node = *reinterpret_cast<void **>(sync + off);
        if (!bool_node(node) || node_value(node))
            continue;
        reinterpret_cast<NodeSetBool_t *>(ga.node_set_bool)(node, &one, 0, 0);
        swofty::log("[title] set the sync point's %s node (sync_points.boot) to true",
                    off == SYNC_REQUEST ? "request (+0x80)" : "ui (+0xa0)");
    }
    if (!title_set.exchange(true))
        swofty::log("[title] sync point complete -> %d; the title sends its own request next",
                    int(reinterpret_cast<SyncComplete_t *>(ga.sync_complete)(sync)));
}

uint32_t *intro_detour(void *state, uint32_t *link)
{
    auto *st = static_cast<uint8_t *>(state);
    if (intro_state.exchange(state) != state)
        swofty::log("[title] StateIntro %p", state);
    if (title_wanted.load() && !st[INTRO_ENGAGED]) {
        player_probe(st);
        title_nodes(st);
        press_tick();
    }
    uint32_t *r = h_intro.call<uint32_t *>(state, link);
    if (st[INTRO_ENGAGED] && !title_engaged.exchange(true)) {
        if (pressing || presses) {
            runtime::pad::release();
            pressing = false;
        }
        swofty::log("[title] engaged (StateIntro+0xef4 set by the title's request callback) after %d press(es) of "
                    "the virtual pad's a; the front end comes next",
                    presses);
    }
    return r;
}

void title_setup()
{
    ga.intro_update = runtime::find(SIG_INTRO_UPDATE, "title", "title");
    ga.node_set_bool = runtime::find(SIG_NODE_SET_BOOL, "title", "title");
    ga.sync_complete = runtime::find(SIG_SYNC_COMPLETE, "title", "title");
    ga.intro_vt = runtime::find(SIG_INTRO_VT, "title", "title");
    ga.bool_type = runtime::find(SIG_BOOL_TYPE, "title", "title");
    ga.empty_node = runtime::find(SIG_EMPTY_NODE, "title", "title");
    if (!runtime::verified_build()) {
        title_why = "StateIntro's state layout (+0xcd0, +0xef3, +0xef4) is not verified on this build";
        return;
    }
    if (!ga.intro_update || !ga.intro_vt || !ga.node_set_bool || !ga.sync_complete || !ga.bool_type || !ga.empty_node) {
        title_why = "FeFlow_StateIntro_OnUpdate, Vm_NodeSet_Bool, Vm_SyncPoint_IsComplete or the bool node type did not "
                    "resolve on this build";
        return;
    }
    auto **vt = reinterpret_cast<void **>(ga.intro_vt);
    if (!runtime::readable(vt, 10 * 8) || vt[9] != addr_of(ga.intro_update)) {
        title_why = "vt_Sess_Intro slot 9 is not FeFlow_StateIntro_OnUpdate";
        return;
    }
    auto h = safetyhook::InlineHook::create(addr_of(ga.intro_update), reinterpret_cast<void *>(intro_detour));
    if (!h) {
        title_why = "the hook on FeFlow_StateIntro_OnUpdate failed";
        return;
    }
    h_intro = std::move(*h);
    title_why.clear();
}

}

namespace runtime::title {

void setup()
{
    std::call_once(once, title_setup);
}

const std::string &why()
{
    return title_why;
}

void want(bool on)
{
    if (on == title_wanted.exchange(on))
        return;
    if (on) {
        runtime::pad::setup();
        runtime::pad::claim(true);
        char env[16] = {};
        if (GetEnvironmentVariableA("SWOFTY_TITLE_PRESS_AFTER_MS", env, sizeof env))
            press_after = strtoull(env, nullptr, 10);
        wanted_at = GetTickCount64();
    } else {
        wanted_at = 0;
        runtime::pad::release();
        runtime::pad::claim(false);
    }
}

bool wanted()
{
    return title_wanted.load();
}

bool engaged()
{
    return title_engaged.load();
}

std::string state()
{
    if (title_engaged.load())
        return "engaged";
    auto *st = static_cast<uint8_t *>(intro_state.load());
    if (!st)
        return "waiting: StateIntro has not updated since the title hook went in";
    if (title_view.load() != 1)
        return "waiting: the title's view ([[StateIntro+0xc88]+0x140]+0x198) is not up";
    if (st[INTRO_SENT])
        return "waiting: the title's request (kind 0x2e) has not called back (StateIntro+0xef3 set, +0xef4 not)";
    return title_set.load() ? "waiting: the sync point is set and StateIntro has not sent its request"
                            : "waiting: the sync point's nodes are not set yet";
}

uintptr_t node_set_bool()
{
    return ga.node_set_bool;
}

bool complete_sync(void *sync, const char *what)
{
    if (!ga.node_set_bool || !ga.sync_complete || !ga.bool_type || !ga.empty_node || !sync)
        return false;
    auto *s = static_cast<uint8_t *>(sync);
    const uint8_t one = 1;
    for (size_t off : {SYNC_REQUEST, SYNC_UI}) {
        void *node = *reinterpret_cast<void **>(s + off);
        if (!bool_node(node) || node_value(node))
            continue;
        reinterpret_cast<NodeSetBool_t *>(ga.node_set_bool)(node, &one, 0, 0);
        swofty::log("[title] set %s's %s node to true", what, off == SYNC_REQUEST ? "request (+0x80)" : "ui (+0xa0)");
    }
    return reinterpret_cast<SyncComplete_t *>(ga.sync_complete)(sync);
}

}
