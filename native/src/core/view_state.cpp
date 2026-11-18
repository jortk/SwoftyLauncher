#include "view_state.hpp"

#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <mutex>
#include <string>

#include "career_state.hpp"
#include "pause_edge.hpp"
#include "view_flow_logic.hpp"

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

namespace {

using view_flow::G_CUTSCENE;
using view_flow::G_DRIVING;
using view_flow::G_PAUSE;
using view_flow::G_REPLAY;
using view_flow::G_SESSION;
using view_flow::G_SPECTATE;
using view_flow::TRACKED;
using view_flow::TRACKED_N;
using view_flow::GROUP_MASK_ALL;

const runtime::Sig S_PHOTO_ON{"site_Game_PhotoModeActiveSet",
                              "C5 F8 29 85 80 03 00 00 C6 85 94 03 00 00 01", 0x2bdba77, runtime::Sig::addr, 0, 8};
const runtime::Sig S_PHOTO_OFF{"site_Game_PhotoModeActiveClear", "C6 86 25 04 00 00 00 C6 86 94 03 00 00 00",
                               0x2bdc327, runtime::Sig::addr, 0, 7};
const runtime::Sig S_CAMERA{"site_Game_UseCameraSettingCall", "48 89 F1 B2 0F E8 ?? ?? ?? ?? 90 48 83 C4 70",
                            0x2424bbd};
const runtime::Sig S_PAUSE{"Game_App_SetPauseReason", "41 57 41 56 41 54 56 57 53 48 83 EC 28 89 D3 48 89 CE 84 D2",
                           0x80e110};

const char *const CAMERA_NAMES[8] = {"unknown",  "nose_cam", "nose_cam_alt", "cockpit_cam",
                                     "tv_pod", "tv_pod_offset", "near_chase", "far_chase"};

safetyhook::MidHook h_photo_on, h_photo_off, h_camera, h_pause;

std::atomic<uint64_t> packed{0};
std::atomic<uint64_t> active{0};
std::atomic<int> camera_value{-1};
std::atomic<bool> photo{false};
std::atomic<bool> flow_seen{false};
std::atomic<uint32_t> pause_mask{0};
std::mutex change_lock;
bool trust_camera;

constexpr uint64_t pack(uint32_t kind, uint32_t flags, bool valid, uint32_t serial)
{
    return uint64_t(kind & 0xff) | uint64_t(flags & 0xff) << 8 | uint64_t(valid ? 1 : 0) << 16 | uint64_t(serial) << 32;
}

view::State unpack(uint64_t p)
{
    view::State s;
    s.kind = uint32_t(p & 0xff);
    s.flags = uint32_t(p >> 8 & 0xff);
    s.valid = (p >> 16 & 1) != 0;
    s.serial = uint32_t(p >> 32);
    return s;
}

uint32_t groups_active()
{
    uint64_t bits = active.load(std::memory_order_relaxed);
    uint32_t out = 0;
    for (size_t i = 0; i < TRACKED_N; i++)
        if (bits >> i & 1)
            out |= TRACKED[i].group;
    return out & GROUP_MASK_ALL;
}

uint32_t camera_kind(int value)
{
    switch (value) {
    case 3:
        return view::COCKPIT;
    case 4:
        return view::TCAM;
    case 5:
        return view::TCAM_OFFSET;
    case 6:
        return view::CHASE_NEAR;
    case 7:
        return view::CHASE_FAR;
    case 1:
    case 2:
        return view::OTHER;
    default:
        return view::UNKNOWN;
    }
}

std::string flag_text(uint32_t flags)
{
    static const struct {
        uint32_t bit;
        const char *name;
    } NAMES[] = {{view::PAUSED, "paused"},     {view::REPLAY, "replay"},       {view::PHOTO, "photo"},
                 {view::MENU, "menu"},         {view::CUTSCENE, "cutscene"},   {view::SPECTATING, "spectating"}};
    std::string out;
    for (auto &n : NAMES)
        if (flags & n.bit)
            out += (out.empty() ? "" : "|") + std::string(n.name);
    return out.empty() ? "none" : out;
}

const char *kind_text(uint32_t kind)
{
    static const char *const NAMES[] = {"unknown", "cockpit", "tcam", "tcam_offset", "chase_near", "chase_far", "other"};
    return kind < 7 ? NAMES[kind] : "unknown";
}

void recompute()
{
    uint32_t g = groups_active();
    bool in_session = g & G_SESSION;
    bool driving = g & G_DRIVING;
    bool paused = (g & G_PAUSE) && in_session;
    bool replay = g & G_REPLAY;
    bool cutscene = g & G_CUTSCENE;
    bool spectating = g & G_SPECTATE;
    bool is_photo = photo.load(std::memory_order_relaxed);

    uint32_t flags = 0;
    if (paused)
        flags |= view::PAUSED;
    if (replay)
        flags |= view::REPLAY;
    if (is_photo)
        flags |= view::PHOTO;
    if (cutscene)
        flags |= view::CUTSCENE;
    if (spectating)
        flags |= view::SPECTATING;
    bool player_view = in_session && (driving || paused);
    if (!in_session || !(player_view || replay || cutscene || spectating || is_photo))
        flags |= view::MENU;

    uint32_t kind = view::UNKNOWN;
    bool valid = flow_seen.load(std::memory_order_relaxed);
    if (flags & view::MENU) {
        // MENU comes from no recognised session state, which is also what an unrecognised one looks like.
        kind = view::UNKNOWN;
        valid = false;
    } else if (replay || is_photo || cutscene || spectating) {
        kind = view::OTHER;
    } else {
        int cam = camera_value.load(std::memory_order_relaxed);
        kind = trust_camera ? camera_kind(cam) : view::UNKNOWN;
        if (kind == view::UNKNOWN)
            valid = false;
    }

    std::lock_guard<std::mutex> lock(change_lock);
    view::State old = unpack(packed.load(std::memory_order_relaxed));
    if (old.kind == kind && old.flags == flags && old.valid == valid && old.serial)
        return;
    uint32_t serial = old.serial + 1;
    packed.store(pack(kind, flags, valid, serial), std::memory_order_release);
    swofty::log("[view] kind=%s flags=%s valid=%d serial=%u", kind_text(kind), flag_text(flags).c_str(), valid ? 1 : 0,
                serial);
}

void photo_on_mid(safetyhook::Context &)
{
    photo.store(true, std::memory_order_relaxed);
    recompute();
}

void photo_off_mid(safetyhook::Context &)
{
    photo.store(false, std::memory_order_relaxed);
    recompute();
}

void camera_mid(safetyhook::Context &ctx)
{
    int value = int(uint32_t(ctx.r8));
    bool secondary = (ctx.r9 & 0xff) != 0;
    if (secondary)
        return;
    if (value < 0 || value > 7)
        value = 0;
    int was = camera_value.exchange(value, std::memory_order_relaxed);
    if (was != value)
        swofty::log("[view] camera setting %s (%d)%s", CAMERA_NAMES[value], value,
                    trust_camera ? "" : "; static only, not used for kind");
    recompute();
}

void pause_mid(safetyhook::Context &ctx)
{
    bool set = (ctx.rdx & 0xff) != 0;
    int bit = int(uint32_t(ctx.r8));
    if (bit < 0 || bit >= 32)
        return;
    uint32_t before = pause_mask.load(std::memory_order_relaxed);
    uint32_t mask = set ? (before | 1u << bit) : (before & ~(1u << bit));
    pause_mask.store(mask, std::memory_order_relaxed);
    static std::atomic<int> logged{0};
    if (logged.fetch_add(1, std::memory_order_relaxed) < 64)
        swofty::log("[view] pause reason bit %d %s, mask %08x", bit, set ? "set" : "cleared", mask);
    switch (view::pause_edge(before, mask)) {
    case view::PauseEdge::paused:
        runtime::events_fire(SWOFTY_EVENT_PAUSE, mask, bit, nullptr);
        break;
    case view::PauseEdge::resumed:
        runtime::events_fire(SWOFTY_EVENT_RESUME, 0, bit, nullptr);
        break;
    case view::PauseEdge::none:
        break;
    }
}

safetyhook::MidHook install(const runtime::Sig &sig, safetyhook::MidHookFn fn)
{
    uintptr_t at = runtime::find(sig, "view state", "view");
    if (!at)
        return {};
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[view] %s at base+0x%llx: the hook failed: %s", sig.what,
                    (unsigned long long)(at - swofty::api->module_base()), runtime::hook_error(h.error()).c_str());
        return {};
    }
    return std::move(*h);
}

}

namespace view {

State current()
{
    return unpack(packed.load(std::memory_order_acquire));
}

uint32_t pause_reasons()
{
    return pause_mask.load(std::memory_order_relaxed);
}

void flow(uint32_t hash, uint32_t to)
{
    for (size_t i = 0; i < TRACKED_N; i++) {
        if (TRACKED[i].hash != hash)
            continue;
        uint64_t bit = uint64_t(1) << i;
        if (to == 1 || to == 2)
            active.fetch_or(bit, std::memory_order_relaxed);
        else if (to == 0 || to == 3)
            active.fetch_and(~bit, std::memory_order_relaxed);
        if (TRACKED[i].group == G_REPLAY && (to == 0 || to == 3) && photo.exchange(false))
            swofty::log("[view] photo mode cleared by the replay state leaving");
        break;
    }
    flow_seen.store(true, std::memory_order_relaxed);
    recompute();
}

void setup()
{
    char trust[8] = {};
    trust_camera = GetEnvironmentVariableA("SWOFTY_VIEW_TRUST_CAMERA", trust, sizeof trust) && trust[0] == '1';
    h_photo_on = install(S_PHOTO_ON, photo_on_mid);
    h_photo_off = install(S_PHOTO_OFF, photo_off_mid);
    h_camera = install(S_CAMERA, camera_mid);
    h_pause = install(S_PAUSE, pause_mid);
    swofty::log("[view] setup: photo %s/%s, camera setting %s%s, pause reasons %s; flow states come from the career flow "
                "site",
                h_photo_on ? "armed" : "off", h_photo_off ? "armed" : "off", h_camera ? "armed" : "off",
                trust_camera ? " (trusted for the view kind)" : " (static only: the view kind stays unknown, valid=0 on track)",
                h_pause ? "armed" : "off");
    recompute();
}

}

extern "C" __declspec(dllexport) uint32_t SwoftyCorePauseReasons(void)
{
    return view::pause_reasons();
}
