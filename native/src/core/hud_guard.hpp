#pragma once
#include <cstdint>
#include <cstdlib>
#include <string>
#include <string_view>
#include <vector>

namespace hud::guard {

constexpr uint64_t REMOVED_AFTER_DRAW_MS = 10000;

inline constexpr std::string_view FG_MODULES[] = {
    "sl.dlss_g.dll",
    "nvngx_dlssg.dll",
    "amd_fidelityfx_dx12.dll",
    "amd_fidelityfx_framegeneration_dx12.dll",
    "ffx_frameinterpolation_x64.dll",
    "ffx_fsr3_x64.dll",
    "libxess_fg.dll",
    "dlssg_to_fsr3_amd_is_better.dll",
};

inline bool same_name(std::string_view a, std::string_view b)
{
    if (a.size() != b.size())
        return false;
    for (size_t i = 0; i < a.size(); i++) {
        char x = a[i], y = b[i];
        if (x >= 'A' && x <= 'Z')
            x = char(x - 'A' + 'a');
        if (y >= 'A' && y <= 'Z')
            y = char(y - 'A' + 'a');
        if (x != y)
            return false;
    }
    return true;
}

inline std::string_view base_name(std::string_view path)
{
    size_t at = path.find_last_of("\\/");
    return at == std::string_view::npos ? path : path.substr(at + 1);
}

inline bool fg_module(std::string_view path)
{
    std::string_view name = base_name(path);
    for (auto m : FG_MODULES)
        if (same_name(name, m))
            return true;
    return false;
}

inline bool streamline_hook(std::string_view path)
{
    return same_name(base_name(path), "sl.dlss_g.dll");
}

// FidelityFX's API loader (FSR 3.1). F1 25 creates its swapchain through it for upscaling too, so it is on the
// creation stack whatever the settings; like Streamline's hook it passes the game's frames through while the
// game's settings have frame generation off. `fg` is a module name, or one with " (calls Present)" after it.
inline bool ffx_loader(std::string_view fg)
{
    std::string_view name = base_name(fg);
    return same_name(name.substr(0, name.find(' ')), "amd_fidelityfx_dx12.dll");
}

// A swapchain made before the core's creation hooks has no creation stack, so Streamline is read off the
// module that calls Present: its interposer, common layer or frame generation hook.
inline bool streamline_caller(std::string_view path)
{
    std::string_view name = base_name(path);
    return same_name(name, "sl.interposer.dll") || same_name(name, "sl.common.dll") || streamline_hook(name);
}

// sl::Result::eOk is 0; Streamline's other codes are all failures.
inline bool streamline_result_ok(int result)
{
    return result == 0;
}

// What a queue learned from a swapchain's own GetDevice makes of the swapchain: only a direct queue is drawn on.
inline std::string learned_fg(bool direct_queue, std::string_view present_fg)
{
    if (!present_fg.empty())
        return std::string(present_fg) + " (calls Present)";
    return direct_queue ? std::string() : std::string("a non-direct present queue");
}

inline std::string fg_in(const std::vector<std::string> &modules)
{
    for (auto &m : modules)
        if (fg_module(m) && !streamline_hook(m))
            return std::string(base_name(m));
    return {};
}

inline bool streamline_in(const std::vector<std::string> &modules)
{
    for (auto &m : modules)
        if (streamline_hook(m))
            return true;
    return false;
}

// Overlays that patch the D3D12 entry points the core's device hooks patch. RivaTuner Statistics Server's
// hook attaches when a window is created and hooks over whatever is in the prologue; a prologue the core
// hooked first sends its trampoline to address 0, so the device hooks are not armed while it is in the process.
inline constexpr std::string_view DEVICE_HOOK_RIVALS[] = {"RTSSHooks64.dll", "RTSSHooks.dll"};

inline std::string_view device_hook_rival(std::string_view path)
{
    std::string_view name = base_name(path);
    for (auto r : DEVICE_HOOK_RIVALS)
        if (same_name(name, r))
            return r;
    return {};
}

inline std::string device_hook_rival_in(const std::vector<std::string> &modules)
{
    for (auto &m : modules)
        if (std::string_view r = device_hook_rival(m); !r.empty())
            return std::string(r);
    return {};
}

// Whether the device hooks arm: a rival overlay in the process keeps them off unless SWOFTY_OVERLAY_FG /
// "overlay_fg" forces them on (forced is -1 when unset).
inline bool device_hooks_allowed(std::string_view rival, int forced)
{
    return rival.empty() || forced > 0;
}

// What asks for the present hook at setup. An overlay addon that is on, a frame capture and a direct start's
// cover screen need it; the main menu panel and video card only decorate the home screen.
struct ArmReasons {
    bool overlay = false, capture = false, cover = false, menu = false;
    bool any() const { return overlay || capture || cover || menu; }
    bool menu_only() const { return menu && !overlay && !capture && !cover; }
};

// Whether the present hook stays off for a rival overlay. RivaTuner's hook attaches to the probe window
// arm_swapchain opens and patches the swapchain's entry points the core hooks; on some machines the game then
// jumps to an address that is not code two seconds after it makes its swapchain. So the swapchain hooks are not
// armed for what only draws: an overlay addon, a direct start's cover, the main menu panel and video card. They
// draw nothing and the game and the addons' other work run. A frame capture is asked for by name and arms, and
// SWOFTY_OVERLAY_RIVAL / "overlay_rival" arms the hook anyway (forced is -1 when unset).
inline bool present_hook_yields(const ArmReasons &reasons, std::string_view rival, int forced = -1)
{
    return reasons.any() && !reasons.capture && !rival.empty() && forced <= 0;
}

// Present layers that make the game's swapchain on a path the core's creation hooks miss, and the swapchain
// does not name its queue at Present either. Its present queue is then learned from ExecuteCommandLists,
// which takes the device hooks.
inline constexpr std::string_view PRESENT_LAYERS[] = {"NvPresent64.dll"};

inline std::string present_layer_in(const std::vector<std::string> &modules)
{
    for (auto &m : modules)
        for (auto l : PRESENT_LAYERS)
            if (same_name(base_name(m), l))
                return std::string(l);
    return {};
}

inline bool device_hooks_wanted_for(const std::vector<std::string> &modules)
{
    return !present_layer_in(modules).empty();
}

// NVIDIA's modules that present the game's frames on a swapchain the creation hooks can miss: the NVIDIA app's
// present layer, DLSS Frame Generation and Streamline, in the order the log names them.
inline constexpr std::string_view NVIDIA_FG_SOURCES[] = {"NvPresent64.dll", "nvngx_dlssg.dll", "sl.dlss_g.dll",
                                                         "sl.interposer.dll"};

inline bool nvidia_fg_source(std::string_view path)
{
    std::string_view name = base_name(path);
    for (auto s : NVIDIA_FG_SOURCES)
        if (same_name(name, s))
            return true;
    return false;
}

// Whether the device hooks arm when a swapchain whose present queue cannot be read first reaches Present, and
// why: `source` names the NVIDIA modules in the process, `refused` says why they stay off. AMD's and Intel's
// frame generation calling Present, or SWOFTY_OVERLAY_FG / "overlay_fg" set to 0, keep them off.
struct OnDemandArm {
    std::string source, refused;
    bool arms() const { return !source.empty(); }
};

inline OnDemandArm on_demand_arm(const std::vector<std::string> &modules, std::string_view present_caller, int forced)
{
    OnDemandArm out;
    if (forced == 0) {
        out.refused = "SWOFTY_OVERLAY_FG=0 / \"overlay_fg\": false";
        return out;
    }
    if (fg_module(present_caller) && !nvidia_fg_source(present_caller)) {
        out.refused = std::string(base_name(present_caller)) + " presents it (AMD's or Intel's frame generation)";
        return out;
    }
    for (auto s : NVIDIA_FG_SOURCES)
        for (auto &m : modules)
            if (same_name(base_name(m), s)) {
                if (!out.source.empty())
                    out.source += ", ";
                out.source += s;
                break;
            }
    if (out.source.empty())
        out.refused = "no NVIDIA frame generation, Streamline or present layer module is in the process";
    return out;
}

// Whether a Present or Present1 closes the frame: not one made inside another on the same thread (DXVK's Present
// calls its Present1), which is the same frame.
inline bool dispatches_present(int presenting_depth)
{
    return presenting_depth == 0;
}

// What learn does with a swapchain whose present queue cannot be read: ask once to arm the device hooks, wait
// for a direct queue to execute command lists once they are armed, learn that queue, or refuse the swapchain.
enum class UnreadQueue { Arm, Wait, Learn, Refuse };

inline UnreadQueue unread_queue_step(bool exec_hooked, bool arm_asked, bool direct_queue_seen)
{
    if (exec_hooked)
        return direct_queue_seen ? UnreadQueue::Learn : UnreadQueue::Wait;
    return arm_asked ? UnreadQueue::Refuse : UnreadQueue::Arm;
}

constexpr int LIST_TYPE_DIRECT = 0;

// A command queue seen in ExecuteCommandLists: its type from one GetDesc, and the thread and tick of its
// last call while it is direct.
struct ExecSeen {
    void *queue;
    int type;
    uint32_t tid;
    uint64_t tick;
};

// The present queue of a swapchain the creation hooks missed: the direct queue that last executed command
// lists on the presenting thread, else the direct queue that executed most recently; nullptr when none has.
inline const ExecSeen *present_queue_from_execs(const std::vector<ExecSeen> &seen, uint32_t tid)
{
    const ExecSeen *same = nullptr, *newest = nullptr;
    for (auto &e : seen) {
        if (e.type != LIST_TYPE_DIRECT)
            continue;
        if (e.tid == tid && (!same || e.tick > same->tick))
            same = &e;
        if (!newest || e.tick > newest->tick)
            newest = &e;
    }
    return same ? same : newest;
}

inline bool xml_int(const std::string &text, const char *element, const char *attr, int &out)
{
    size_t at = text.find(std::string("<") + element + " ");
    if (at == std::string::npos)
        return false;
    size_t end = text.find('>', at);
    std::string key = std::string(" ") + attr + "=\"";
    size_t k = text.find(key, at);
    if (k == std::string::npos || k > end)
        return false;
    out = atoi(text.c_str() + k + key.size());
    return true;
}

struct FgSetting {
    bool known = false;
    int mode = -1, multi = -1;
    bool on() const { return !known || mode != 0; }
    const char *word() const { return !known ? "unknown" : mode != 0 ? "on" : "off"; }
};

inline int hdr_mode(const std::string &hardware_settings)
{
    int mode = -1;
    xml_int(hardware_settings, "hdr", "mode", mode);
    return mode;
}

inline constexpr unsigned COLOR_SPACE_UNSET = ~0u;

enum Output : unsigned { SDR = 0, HDR10_PQ = 1, SCRGB = 2 };

inline Output output_for(int format, unsigned color_space, int game_hdr_mode)
{
    constexpr unsigned G2084_P2020 = 12;
    if (format == 24)
        return color_space == G2084_P2020 || (color_space == COLOR_SPACE_UNSET && game_hdr_mode > 0) ? HDR10_PQ : SDR;
    if (format == 10)
        return color_space == G2084_P2020 ? HDR10_PQ : SCRGB;
    return SDR;
}

inline FgSetting fg_setting(const std::string &hardware_settings)
{
    FgSetting f;
    f.known = xml_int(hardware_settings, "frame_gen", "mode", f.mode);
    xml_int(hardware_settings, "multi_frame_gen", "value", f.multi);
    return f;
}

// -1 when SWOFTY_OVERLAY_FG / "overlay_fg" is unset. Unset, it follows the game's setting at start.
inline bool before_fg_wanted(int forced, const FgSetting &game)
{
    if (forced >= 0)
        return forced != 0;
    return game.known && game.mode != 0;
}

inline std::string chain(const std::vector<std::string> &modules, std::string_view skip_a, std::string_view skip_b)
{
    std::string out;
    std::string_view last;
    for (auto &m : modules) {
        std::string_view name = base_name(m);
        if (name.empty() || same_name(name, skip_a) || same_name(name, skip_b) || same_name(name, last))
            continue;
        if (!out.empty())
            out += " <- ";
        out += name;
        last = name;
    }
    return out.empty() ? "unknown" : out;
}

inline bool removal_after_draw(uint64_t now_ms, uint64_t last_draw_ms)
{
    return last_draw_ms && now_ms >= last_draw_ms && now_ms - last_draw_ms <= REMOVED_AFTER_DRAW_MS;
}

// Whether a COM object's own vtable sends this slot to fn. A hook's trampoline runs the hooked
// implementation's body, which reads its this pointer as its own object type, so the core calls it only
// for an object of that type; a wrapper such as Streamline's queue proxy goes through its own vtable.
inline bool vtable_slot_is(const void *object, size_t slot, const void *fn)
{
    return object && fn && (*static_cast<const void *const *const *>(object))[slot] == fn;
}

enum class Gate { Direct, FrameGenNamed, FrameGenStreamline };

inline Gate gate_for(bool fg_named, bool streamline, bool fg_setting_off)
{
    if (fg_named)
        return Gate::FrameGenNamed;
    return streamline && !fg_setting_off ? Gate::FrameGenStreamline : Gate::Direct;
}

// The gate for a swapchain from its frame generation tag (Swapchain::fg) and Streamline flag. FidelityFX's
// loader passes through like Streamline's hook. fg_setting_off() is asked only when it decides the gate (it reads
// the game's hardware settings).
template <class FgOff> inline Gate gate_for_swapchain(std::string_view fg, bool streamline, FgOff fg_setting_off)
{
    const bool ffx = ffx_loader(fg), named = !fg.empty() && !ffx, through = streamline || ffx;
    return gate_for(named, through, named || !through || fg_setting_off());
}

// A swapchain whose queue was learned after the device hooks armed on demand for it: the game named its back
// buffers before they armed, so drawing before frame generation has none to draw into, and it is drawn on at Present.
template <class FgOff> inline Gate gate_for_swapchain(std::string_view fg, bool streamline, bool armed_late, FgOff fg_setting_off)
{
    return armed_late ? Gate::Direct : gate_for_swapchain(fg, streamline, fg_setting_off);
}

inline bool draws_before_present(Gate gate, bool device_hooks)
{
    return gate != Gate::Direct && device_hooks;
}

inline bool draws_at_present(Gate gate)
{
    return gate == Gate::Direct;
}

constexpr unsigned STATE_PRESENT = 0, STATE_RENDER_TARGET = 0x4;
constexpr unsigned BARRIER_FLAG_BEGIN_ONLY = 0x1;
constexpr uint64_t PENDING_MAX_AGE_MS = 250;

inline bool presents_back_buffer(unsigned before, unsigned after, unsigned flags)
{
    return before == STATE_RENDER_TARGET && after == STATE_PRESENT && !(flags & BARRIER_FLAG_BEGIN_ONLY);
}

inline unsigned state_after_draw(unsigned left_in)
{
    return left_in;
}

inline bool pending_fresh(uint64_t now_ms, uint64_t built_ms)
{
    return now_ms >= built_ms && now_ms - built_ms <= PENDING_MAX_AGE_MS;
}

inline bool target_fits(unsigned frame_w, unsigned frame_h, uint64_t res_w, unsigned res_h)
{
    return frame_w && frame_h && frame_w == res_w && frame_h == res_h;
}

inline int renderable_format(int format)
{
    switch (format) {
    case 9:
        return 10;
    case 23:
        return 24;
    case 27:
        return 28;
    case 90:
        return 87;
    default:
        return format;
    }
}

// The batch's list that presents a back buffer, last one wins; n when none does.
inline size_t presenting_list(const std::vector<bool> &presents)
{
    for (size_t i = presents.size(); i-- > 0;)
        if (presents[i])
            return i;
    return presents.size();
}

}
