#pragma once

#include "json.hpp"

#include <algorithm>
#include <array>
#include <cctype>
#include <cmath>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cwctype>
#include <iterator>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>
#include <type_traits>

// The parts of the post-process passes that are plain logic: which views a pass runs in, the key
// names a toggle option holds, texture formats and sizes, option values from settings text, and
// which addon's passes take the bytes an addon's code gives them.
namespace postfx {

enum View : uint32_t { UNKNOWN = 0, COCKPIT, TCAM, TCAM_OFFSET, CHASE_NEAR, CHASE_FAR, OTHER, VIEWS };
enum Flag : uint32_t { PAUSED = 1, REPLAY = 2, PHOTO = 4, MENU = 8, CUTSCENE = 16, SPECTATING = 32 };

inline constexpr std::string_view VIEW_NAMES[VIEWS] = {"unknown",    "cockpit",   "tcam", "tcam_offset",
                                                      "chase_near", "chase_far", "other"};
inline constexpr std::string_view FLAG_NAMES[] = {"paused", "replay", "photo", "menu", "cutscene", "spectating"};

inline uint32_t view_bit(std::string_view name)
{
    for (uint32_t i = 1; i < VIEWS; i++)
        if (VIEW_NAMES[i] == name)
            return 1u << i;
    return 0;
}

inline uint32_t flag_bit(std::string_view name)
{
    for (uint32_t i = 0; i < std::size(FLAG_NAMES); i++)
        if (FLAG_NAMES[i] == name)
            return 1u << i;
    return 0;
}

inline std::string flag_text(uint32_t flags)
{
    std::string out;
    for (uint32_t i = 0; i < std::size(FLAG_NAMES); i++)
        if (flags & (1u << i))
            out += (out.empty() ? "" : "+") + std::string(FLAG_NAMES[i]);
    return out.empty() ? "-" : out;
}

inline std::string_view view_text(uint32_t kind)
{
    return kind < VIEWS ? VIEW_NAMES[kind] : "?";
}

struct When {
    uint32_t views = 0, skip = 0;
};

// A pass with views runs only while the view is known and one of them; skip flags stop it while the
// view is known to have them. Without either it always runs.
inline const char *ineligible(const When &w, uint32_t kind, uint32_t flags, bool valid)
{
    if (w.views && !valid)
        return "the view is unknown";
    if (w.views && !(w.views & (1u << kind)))
        return "not in one of its views";
    if (valid && (w.skip & flags))
        return "a skipped state";
    return nullptr;
}

// Win32 virtual-key codes for a toggle option's key names; 0 for None and unknown names.
inline int key_code(std::string_view key)
{
    if (key.size() == 1 && ((key[0] >= 'A' && key[0] <= 'Z') || (key[0] >= '0' && key[0] <= '9')))
        return key[0];
    if (key.size() == 1 && key[0] >= 'a' && key[0] <= 'z')
        return key[0] - 'a' + 'A';
    if (key.size() >= 2 && key.size() <= 3 && (key[0] == 'F' || key[0] == 'f')) {
        int n = 0;
        for (char c : key.substr(1)) {
            if (c < '0' || c > '9')
                return 0;
            n = n * 10 + (c - '0');
        }
        return n >= 1 && n <= 24 ? 0x70 + n - 1 : 0;
    }
    struct Named {
        std::string_view name;
        int vk;
    };
    static constexpr Named NAMED[] = {{"Insert", 0x2D}, {"Home", 0x24},      {"End", 0x23},
                                      {"Page Up", 0x21}, {"Page Down", 0x22}, {"Scroll Lock", 0x91},
                                      {"Pause", 0x13}};
    for (auto &n : NAMED)
        if (n.name == key)
            return n.vk;
    return 0;
}

// DXGI_FORMAT values for the texture format names.
inline int format_of(std::string_view name)
{
    struct Named {
        std::string_view name;
        int dxgi;
    };
    static constexpr Named NAMED[] = {{"rgba8", 28}, {"rgba16f", 10}, {"rg16f", 34},      {"r16f", 54},
                                      {"r32f", 41},  {"rgb10a2", 24}, {"r11g11b10f", 26}};
    for (auto &n : NAMED)
        if (n.name == name)
            return n.dxgi;
    return 0;
}

struct Size {
    uint32_t w = 0, h = 0;
};

inline constexpr uint32_t MAX_MIPS = 12;

// The levels a w x h texture carries for a declared count: at least 1, at most MAX_MIPS and
// floor(log2(max(w, h))) + 1.
inline uint32_t mip_levels(uint32_t w, uint32_t h, uint32_t declared)
{
    uint32_t most = 1;
    for (uint32_t m = w > h ? w : h; m > 1; m >>= 1)
        most++;
    uint32_t cap = most < MAX_MIPS ? most : MAX_MIPS;
    return declared < 1 ? 1 : declared > cap ? cap : declared;
}

// The size of a level: each step halves, rounding down, at least 1.
inline Size mip_size(uint32_t w, uint32_t h, uint32_t level)
{
    uint32_t sw = level < 32 ? w >> level : 0, sh = level < 32 ? h >> level : 0;
    return {sw ? sw : 1u, sh ? sh : 1u};
}

// A texture's size for a picture: its own width and height, or the picture's scaled, at least 1x1.
inline Size texture_size(double scale, uint32_t width, uint32_t height, uint32_t pic_w, uint32_t pic_h)
{
    if (width && height)
        return {width, height};
    auto at = [&](uint32_t v) {
        double s = v * scale + 0.5;
        return s < 1 ? 1u : uint32_t(s);
    };
    return {at(pic_w), at(pic_h)};
}

// An option's value from the text of its current state in the settings: true and false read as 1 and
// 0, numbers as themselves; anything else is not a value.
inline bool option_value(std::string_view text, float &out)
{
    if (text == "true" || text == "false") {
        out = text == "true";
        return true;
    }
    std::string s(text);
    char *end = nullptr;
    double v = std::strtod(s.c_str(), &end);
    if (s.empty() || end != s.c_str() + s.size())
        return false;
    out = float(v);
    return true;
}

inline uint32_t groups(uint32_t size, uint32_t threads)
{
    return threads ? (size + threads - 1) / threads : 0;
}

inline constexpr uint64_t PRESENT_GONE_MS = 1000;

// Whether no Present has reached the core's swapchain hooks for a second since the passes were made (started),
// or since the last one.
inline bool presents_gone(uint64_t now, uint64_t present_at, uint64_t started)
{
    if (!started)
        return false;
    uint64_t last = present_at > started ? present_at : started;
    return now >= last && now - last >= PRESENT_GONE_MS;
}

// Frame generation that presents the game's frames and its own generated ones through the swapchain at its own
// pace: XeSS frame generation's libxess_fg.dll calls Present1 from its own thread, once per frame it shows, so its
// Presents are not the game's frames. `fg` is a Swapchain::fg: a module name, or one with " (calls Present)" after it.
inline bool presents_own_frames(std::string_view fg)
{
    std::string_view name = fg.substr(0, fg.find(' '));
    size_t at = name.find_last_of("\\/");
    name = at == std::string_view::npos ? name : name.substr(at + 1);
    constexpr std::string_view XESS_FG = "libxess_fg.dll";
    if (name.size() != XESS_FG.size())
        return false;
    for (size_t i = 0; i < name.size(); i++)
        if (char(std::tolower(static_cast<unsigned char>(name[i]))) != XESS_FG[i])
            return false;
    return true;
}

// What closes the passes' frame (frame_count, which the placement gate, the depth copy and the previous frame
// read). A swapchain that presents each of the game's frames once closes it at Present. While frame generation
// presents frames of its own (presents_own_frames) or no Present reaches the hooks, the game's own frames close
// it instead, where its lists are submitted: a list that makes a back buffer other than the last one a render
// target starts the next frame, one per frame the game draws whatever frame generation shows. The game's flow
// tick never closes it; while no Present arrives it only keeps the toggle keys and live settings running.
struct FrameClock {
    uint64_t present_at = 0, generated_at = 0;
    const void *drawn = nullptr;

    bool game_frames(uint64_t now, uint64_t started) const
    {
        if (generated_at && now >= generated_at && now - generated_at < PRESENT_GONE_MS)
            return true;
        return presents_gone(now, present_at, started);
    }

    // A Present the hooks dispatch, from frame generation's own presenter or not: true when it closes the frame.
    bool present(uint64_t now, bool generated, uint64_t started)
    {
        present_at = now;
        if (generated)
            generated_at = now;
        return !game_frames(now, started);
    }

    // A submitted list that made back buffer back a render target (nullptr for none): true when it closes the frame.
    bool submit(const void *back, uint64_t now, uint64_t started)
    {
        if (!back || back == drawn)
            return false;
        drawn = back;
        return game_frames(now, started);
    }

    // The game's flow tick: true when it runs the toggle keys and live settings, as no Present arrives.
    bool flow(uint64_t now, uint64_t started) const
    {
        return presents_gone(now, present_at, started);
    }
};

// A frame count far enough back that the passes count as not run lately.
inline constexpr unsigned LONG_AGO = 0u - 8;

// Whether the passes ran on a back buffer at frame at, seen at frame now, within this use of it. A back
// buffer comes round again two or more frames later.
inline bool placed_lately(unsigned now, unsigned at)
{
    return now - at < 2;
}

// The HUD's read of the finished picture, which the game makes while paused and under panels over the race: the
// passes run at that read. A frame's lists are recorded in parallel, so the list with the HUD-less step cannot
// tell whether the HUD's list of the same frame reads the picture; while the read holds, the step leaves the passes
// to it. It holds for the frame of a read and the one after. The submission of the picture's list comes after the
// lists recorded before it, so it knows whether the HUD read the picture in this frame yet: when it did, the passes
// ran there. In the frame after a read without one so far (the latch), the HUD's list may still read the picture,
// so the submission leaves the passes waiting for it (BackPlace): a read that comes places them, and without one they
// run at the fallbacks, so the first frame without a read keeps them too. Either way the back buffer is placed once.
struct HudRead {
    unsigned at = LONG_AGO;

    void read(unsigned now) { at = now; }
    bool holds(unsigned now) const { return now - at <= 1; }
    // The HUD read the picture in frame now itself.
    bool read_in(unsigned now) const { return at == now; }
    // A read of the frame before holds the in-list places off and none came in this frame yet.
    bool latch(unsigned now) const { return holds(now) && !read_in(now); }
};

// What the submission of a picture's list does with a fresh back buffer: nothing (the passes were placed), the
// passes between the game's lists right after it, or wait for the list that closes the HUD-less step.
enum PictureEnd : uint8_t { END_NONE, END_BETWEEN, END_DEFER };

// What the submission of a picture's list asks BackPlace::picture_submitted. When the list ends with the HUD-less
// copy still in UAV, the passes wait for the step's closing list while a Present can still fall back (not
// game_frames), and otherwise run right after the list. In the HUD read's latch (HudRead::latch) they always wait:
// running them right after the list would take the place of a read the HUD's list still makes in this frame, and
// with it the hold that keeps the read the one place on track. Without a Present to fall back on, a latch frame
// whose HUD's list the core does not see goes without them, as every latch frame did before.
struct PictureWait {
    bool open = false, may_defer = false;
};
inline PictureWait picture_wait(bool step_open, bool latch, bool game_frames)
{
    return {step_open || latch, latch || (step_open && !game_frames)};
}

// Where one back buffer's passes go in a frame. It is fresh from the barrier that makes it a render target until
// the passes are placed on it. A picture's list can end inside the HUD-less step; the list that closes the step
// then sometimes draws the rest of the picture into the back buffer, over anything placed after the picture's list.
// So that submission only opens the step: the closing list runs the passes in it while the back buffer is bound
// there (the split step); when it doesn't, the core's own list runs them right after the closing list. A list that
// ends with the back buffer out of render target while they still wait is the one that draws the HUD and hands the
// frame to Present: they run right before it. Only a frame whose lists the core never saw do that reaches Present
// with them waiting, and they run there, over the HUD: the picture keeps its look and the HUD alone differs for
// that frame, where skipping them would change the whole picture. `target` is whether the lists submitted since
// the step opened left the back buffer a render target.
struct BackPlace {
    bool fresh = false, open = false, target = false;
    unsigned placed_at = LONG_AGO, opened_at = LONG_AGO;

    // The barrier that makes the back buffer a render target for a new frame.
    void drawn(unsigned now)
    {
        fresh = !placed_lately(now, placed_at);
        open = false;
        target = true;
    }

    // Whether a list that closes the HUD-less step with this back buffer bound runs the passes in it.
    bool split_step(unsigned now) const { return fresh || !placed_lately(now, placed_at); }

    void place(unsigned now)
    {
        fresh = false;
        placed_at = now;
    }

    // The list the passes were recorded into was reset unsubmitted.
    void unplace()
    {
        fresh = true;
        placed_at = LONG_AGO;
    }

    // The picture's list is submitted with the back buffer still a render target; step_open when it ends with the
    // HUD-less copy still in UAV. The step opens even when a closing list recorded first already holds the passes,
    // so a reset of that list leaves them to the fallbacks.
    PictureEnd picture_submitted(unsigned now, bool step_open, bool may_defer)
    {
        if (step_open && may_defer) {
            open = true;
            opened_at = now;
            target = true;
            return fresh ? END_DEFER : END_NONE;
        }
        if (!fresh)
            return END_NONE;
        place(now);
        return END_BETWEEN;
    }

    // A list submitted after the picture's moved the back buffer.
    void moved(bool still_target)
    {
        if (open)
            target = still_target;
    }

    // Whether the passes still wait for the step that the picture's list left open.
    bool waits(unsigned now) const { return open && fresh && now - opened_at < 2; }

    // A list that closed the step is submitted: true when the passes run between, right after it.
    bool step_closed(unsigned now, bool direct)
    {
        if (!direct || !target || !waits(now))
            return false;
        place(now);
        return true;
    }

    // A list submitted after the picture's ends with the back buffer out of render target: true when the passes
    // run between, right before it. Asked before moved().
    bool target_left(unsigned now, bool direct)
    {
        if (!direct || !target || !waits(now))
            return false;
        place(now);
        return true;
    }

    // The back buffer is presented: true when the passes run at Present.
    bool presented(unsigned now)
    {
        if (!waits(now))
            return false;
        place(now);
        return true;
    }
};

// A frame generation swapchain (FidelityFX's, Streamline's) hands the game back buffers of its own and copies each
// frame into its real back buffer inside Present, so the buffer a Present shows is never one the game drew into and
// never matches the back buffer whose passes wait. The shown buffer still holds that picture when frame generation
// is off (each Present shows the frame the game just drew), the step opened in the frame this Present closes, no
// passes were recorded since the last Present, and the two buffers have the same size and format.
inline bool shown_is_waiting(bool generating, unsigned runs, unsigned now, unsigned opened_at, bool same_shape)
{
    return !generating && !runs && now == opened_at && same_shape;
}

// Why a presented frame went without the passes, most specific first.
enum Idle : uint8_t {
    IDLE_NONE,
    IDLE_KILL,
    IDLE_KEY,
    IDLE_PASS_OFF,
    IDLE_RULE,
    IDLE_OPTION,
    IDLE_ADDON_KEY,
    IDLE_VIEW,
    IDLE_PASSES,
    IDLE_BUSY,
    IDLE_LIST_STATE,
    IDLE_UI_TOGETHER,
    IDLE_HUD_READ,
    IDLE_NO_PICTURE,
    IDLE_UNPLACED,
    IDLE_COUNT
};

// What a frame knows when it closes without the passes: the kill file, the shader toggle key, what stopped the
// place that tried to run them (IDLE_NONE when none tried), whether the HUD's read held the other places off, and
// whether the game submitted a picture list.
struct IdleFacts {
    bool killed = false, key_off = false, hud_read = false, picture = false;
    Idle tried = IDLE_NONE;
};

inline Idle idle_reason(const IdleFacts &f)
{
    if (f.killed)
        return IDLE_KILL;
    if (f.key_off)
        return IDLE_KEY;
    if (f.tried != IDLE_NONE)
        return f.tried;
    if (f.hud_read)
        return IDLE_HUD_READ;
    return f.picture ? IDLE_UNPLACED : IDLE_NO_PICTURE;
}

// Why no pass of a frame is live, from the reasons its passes gave (one bit per Idle): the reason they share, or
// IDLE_PASSES when they differ.
inline Idle idle_of_passes(unsigned reasons)
{
    for (Idle one : {IDLE_PASS_OFF, IDLE_RULE, IDLE_OPTION, IDLE_ADDON_KEY, IDLE_VIEW})
        if (reasons == 1u << one)
            return one;
    return IDLE_PASSES;
}

inline const char *idle_text(Idle why)
{
    switch (why) {
    case IDLE_KILL:
        return "swofty/postfx.off exists";
    case IDLE_KEY:
        return "the shader toggle key turned them off";
    case IDLE_PASS_OFF:
        return "every pass is off, over its GPU budget or without a pipeline";
    case IDLE_RULE:
        return "a rule turned every pass off";
    case IDLE_OPTION:
        return "an option turns every pass off";
    case IDLE_ADDON_KEY:
        return "an addon's shader key turned every pass off";
    case IDLE_VIEW:
        return "no pass is for this view";
    case IDLE_PASSES:
        return "every pass is off or not for this view";
    case IDLE_BUSY:
        return "every slot is in flight";
    case IDLE_LIST_STATE:
        return "the game's list has state from before the hooks";
    case IDLE_UI_TOGETHER:
        return "the picture and the HUD are in one list without a HUD-less step";
    case IDLE_HUD_READ:
        return "the HUD read the picture a frame before and not in this one (pause latch)";
    case IDLE_NO_PICTURE:
        return "the game submitted no picture list";
    case IDLE_UNPLACED:
        return "no list of the frame had a place for them";
    default:
        return "";
    }
}

// The passes going off and on again, for one log line per change. The first time they run has its own line.
struct OnOff {
    enum Change : uint8_t { SAME, STOPPED, AGAIN };
    bool on = false, ran_before = false;
    unsigned off_at = 0;
    Idle why = IDLE_NONE;

    Change frame(bool ran, unsigned now, Idle reason)
    {
        if (ran == on)
            return SAME;
        on = ran;
        if (!ran) {
            off_at = now;
            why = reason;
            return STOPPED;
        }
        return std::exchange(ran_before, true) ? AGAIN : SAME;
    }
};

// The count line's window: a minute for the first half hour, five minutes for the rest of the session.
inline constexpr unsigned STAT_MINUTES = 30, ONOFF_LINES = 20;
inline unsigned stat_window_ms(unsigned lines)
{
    return lines < STAT_MINUTES ? 60000 : 300000;
}

// A view a slot wrote into one entry of its descriptor tables: the resource and its format.
template <class R, class F> struct Written {
    R *r = nullptr;
    F f{};
    bool set = false;
};

// Whether the entry holding v must be written again for r in format f, noting it as written when it
// must: a frame whose views are the ones its slot wrote last time keeps them.
template <class R, class F> bool rewrite(Written<R, F> &v, R *r, F f)
{
    if (v.set && v.r == r && v.f == f)
        return false;
    v = {r, f, true};
    return true;
}

// The depth and UI barriers one list moved since the locked path last looked at it, written without a
// lock by the thread recording the list.
template <class L, class R, class S> struct SideMark {
    std::atomic<L *> list{nullptr};
    std::atomic<R *> depth{nullptr}, bound{nullptr};
    std::atomic<S> depth_state{};
    std::atomic<bool> ui{false}, marked{false};
};

// The lists' side marks. A list keeps its entry once it has one; a list that finds no room makes the
// table full for good, and from then on every batch goes through the lock.
template <class L, class R, class S, size_t N = 1024, size_t PROBES = 32> class SideMarks {
    SideMark<L, R, S> at_[N];
    std::atomic<bool> full_{false};

public:
    SideMark<L, R, S> *find(L *list, bool add)
    {
        size_t h = size_t((uint64_t(uintptr_t(list)) >> 4) * 0x9E3779B97F4A7C15ull >> 32);
        for (size_t i = 0; i < PROBES && i < N; i++) {
            SideMark<L, R, S> &s = at_[(h + i) % N];
            L *k = s.list.load(std::memory_order_acquire);
            if (k == list)
                return &s;
            if (k)
                continue;
            if (!add)
                return nullptr;
            if (s.list.compare_exchange_strong(k, list, std::memory_order_acq_rel) || k == list)
                return &s;
        }
        if (add)
            full_.store(true, std::memory_order_relaxed);
        return nullptr;
    }

    bool full() const { return full_.load(std::memory_order_relaxed); }
};

// Notes one barrier of a batch in a list's side mark the way the locked path notes it in the list's
// Mark: a depth barrier always, a UI barrier once the list has a Mark or a depth barrier.
template <class L, class R, class S>
void note(SideMark<L, R, S> &s, bool &marked, bool depth, bool ui, R *r, S after)
{
    if (depth) {
        s.depth_state.store(after, std::memory_order_relaxed);
        s.depth.store(r, std::memory_order_relaxed);
        marked = true;
    }
    if (ui && marked)
        s.ui.store(true, std::memory_order_relaxed);
}

// Makes at point to r with a reference of its own on r and lets go of the one it held, so a resource
// the game releases stays alive while the core still points at it.
template <class T> void hold(T *&at, std::type_identity_t<T> *r)
{
    if (at == r)
        return;
    if (r)
        r->AddRef();
    if (at)
        at->Release();
    at = r;
}


inline constexpr int IN_NONE = -1, IN_ORIGINAL = -2, IN_PREVIOUS = -3;
inline constexpr size_t MAX_INPUTS = 8, SLOTS = 32;

// Root constant indices of swofty_options and swofty_params in the SwoftyPostFx cbuffer.
inline constexpr size_t OPTIONS_AT = 8, PARAMS_AT = 36, CONSTANT_COUNT = 56;
// Root constant index of swofty_data_size: the bytes of the pass's data in t10, 0 without any.
inline constexpr size_t DATA_SIZE_AT = 35;

// The addon whose DLL is at `module`. The loader loads an addon's DLLs from <game folder>\swofty\addons\<id>\, the
// game folder being the exe's (`exe`), so it's the folder right under that one which holds the DLL, matched from the
// start of the path and without case: a DLL in a deeper folder named like another addon is still its own addon's.
// Empty for a DLL anywhere else (the game's, Windows', one straight in swofty\addons\). Paths as GetModuleFileNameW
// gives them.
inline std::string addon_of_module(std::wstring module, std::wstring exe)
{
    for (std::wstring *p : {&module, &exe})
        for (auto &ch : *p)
            ch = ch == L'/' ? L'\\' : wchar_t(towlower(ch));
    size_t slash = exe.rfind(L'\\');
    if (slash == std::wstring::npos)
        return {};
    std::wstring root = exe.substr(0, slash) + L"\\swofty\\addons\\";
    if (module.size() <= root.size() || module.compare(0, root.size(), root) != 0)
        return {};
    size_t end = module.find(L'\\', root.size());
    if (end == std::wstring::npos || end == root.size())
        return {};
    std::string id;
    for (size_t i = root.size(); i < end; i++)
        id += module[i] < 0x80 ? char(module[i]) : '?';
    return id;
}

// The bytes addons' code gives their passes, by addon id and pass name. Until the core has read the profile's passes
// (an addon's on-load can come first) it can't tell a pass the profile has from one it hasn't, so it keeps the bytes
// of any pass, of EARLY_PASSES passes an addon at most; listed() then drops those of passes the profile hasn't, and
// from then on only passes it has get any.
class DataStore {
public:
    static constexpr size_t EARLY_PASSES = 16;

    static std::string key(const std::string &addon_id, const std::string &pass) { return addon_id + '\n' + pass; }

    bool is_listed() const { return listed_; }

    // Before the passes are listed: keeps the bytes for any pass (size 0: drops them). False when that would be more
    // than EARLY_PASSES passes of the addon; nothing is kept then.
    bool set_early(const std::string &k, const void *data, size_t size)
    {
        if (size && !bytes_.count(k)) {
            std::string_view addon(k.data(), k.find('\n') + 1);  // "<addon id>\n"
            size_t n = 0;
            for (const auto &e : bytes_)
                n += std::string_view(e.first).substr(0, addon.size()) == addon;
            if (n >= EARLY_PASSES)
                return false;
        }
        set(k, data, size);
        return true;
    }

    // Keeps the bytes of a pass the profile has (size 0: drops them).
    void set(const std::string &k, const void *data, size_t size)
    {
        if (size)
            bytes_[k].assign(static_cast<const char *>(data), size);
        else
            bytes_.erase(k);
    }

    // The passes are listed: has(key) says which the profile has. The bytes of the others go.
    template <class Has> void listed(Has has)
    {
        listed_ = true;
        for (auto it = bytes_.begin(); it != bytes_.end();)
            it = has(it->first) ? std::next(it) : bytes_.erase(it);
    }

    const std::string *find(const std::string &k) const
    {
        auto it = bytes_.find(k);
        return it == bytes_.end() ? nullptr : &it->second;
    }

    bool empty() const { return bytes_.empty(); }
    size_t size() const { return bytes_.size(); }

private:
    std::unordered_map<std::string, std::string> bytes_;
    bool listed_ = false;
};

struct TexDecl {
    std::string name, file;
    double scale = 1;
    uint32_t width = 0, height = 0;
    int format = 10;
    bool persistent = false;
    uint32_t mips = 1;

    bool operator==(const TexDecl &o) const
    {
        return name == o.name && file == o.file && scale == o.scale && width == o.width && height == o.height &&
               format == o.format && persistent == o.persistent && mips == o.mips;
    }
};

struct PassDecl {
    std::string name, dxil, output;
    std::vector<std::string> inputs;
    uint32_t threads[2] = {8, 8};
    bool raw = false;
    double budget_ms = 1.5;
};

struct EffectDecl {
    std::string id, name, addon;
    unsigned slots = 0;
    std::vector<TexDecl> textures;
    std::vector<PassDecl> passes;
};

// The effects of swofty/shaders/effects.json by id; an entry without an id or without passes is left out.
inline std::vector<EffectDecl> parse_effects(const std::string &text)
{
    std::vector<EffectDecl> out;
    json::Value root;
    if (!json::parse(text, root))
        return out;
    for (auto &e : root["effects"].a) {
        EffectDecl d;
        d.id = e["id"].str();
        d.name = e["name"].str(d.id.c_str());
        d.addon = e["addon"].str();
        d.slots = unsigned(e["slots"].num());
        for (auto &t : e["textures"].a) {
            TexDecl x;
            x.name = t["name"].str();
            x.file = t["file"].str();
            if (t["scale"].type == json::Value::Num)
                x.scale = t["scale"].n;
            if (t["width"].type == json::Value::Num && t["height"].type == json::Value::Num) {
                x.width = uint32_t(t["width"].n);
                x.height = uint32_t(t["height"].n);
            }
            x.persistent = t["persistent"].type == json::Value::Bool && t["persistent"].b;
            if (t["mips"].type == json::Value::Num && t["mips"].n >= 1)
                x.mips = t["mips"].n > MAX_MIPS ? MAX_MIPS : uint32_t(t["mips"].n);
            int f = format_of(t["format"].str("rgba16f"));
            x.format = f ? f : 10;
            d.textures.push_back(std::move(x));
        }
        for (auto &p : e["passes"].a) {
            PassDecl x;
            x.name = p["name"].str("pass");
            x.dxil = p["dxil"].str();
            x.output = p["output"].str();
            for (auto &in : p["inputs"].a)
                x.inputs.push_back(in.str());
            const auto &th = p["threads"].a;
            if (th.size() == 2 && th[0].n >= 1 && th[1].n >= 1) {
                x.threads[0] = uint32_t(th[0].n);
                x.threads[1] = uint32_t(th[1].n);
            }
            x.raw = p["raw"].type == json::Value::Bool && p["raw"].b;
            if (p["budget_ms"].type == json::Value::Num && p["budget_ms"].n > 0)
                x.budget_ms = p["budget_ms"].n;
            d.passes.push_back(std::move(x));
        }
        if (!d.id.empty() && !d.passes.empty())
            out.push_back(std::move(d));
    }
    return out;
}

struct InstanceSpec {
    std::string effect, label;
    std::vector<float> params;
    int index = 0;
};

enum SignalKind : uint8_t { SK_BOOL, SK_NAME, SK_NUMBER, SK_EVENT };

struct SignalInfo {
    std::string_view id;
    SignalKind kind;
    std::string_view values;
    std::string_view unit;
    bool in_game;
};

enum Sig : int {
    S_VIEW_CAMERA, S_VIEW_REPLAY, S_VIEW_PAUSED, S_VIEW_PHOTO, S_VIEW_MENU, S_VIEW_CUTSCENE, S_VIEW_SPECTATING,
    S_SESSION_START, S_RACE_START, S_SESSION_LAP, S_SESSION_END,
    S_CAR_SPEED, S_CAR_RPM, S_CAR_GEAR, S_CAR_THROTTLE, S_CAR_BRAKE, S_CAR_STEER, S_CAR_G_LATERAL, S_CAR_G_LONG,
    S_CAR_SURFACE, S_CAR_IMPACT, S_CAR_DAMAGE, S_CAR_IN_PIT_LANE, S_CAR_OFF_TRACK,
    S_SESSION_FLAG, S_SESSION_SAFETY_CAR, S_SESSION_WEATHER, S_SESSION_WETNESS, S_SESSION_FLASHBACK,
    S_SESSION_CHEQUERED,
    S_CAR_WETNESS,
    SIGNAL_COUNT
};

inline constexpr SignalInfo SIGNALS[SIGNAL_COUNT] = {
    {"view.camera", SK_NAME, "cockpit tcam tcam_offset chase_near chase_far other", "", false},
    {"view.replay", SK_BOOL, "", "", true},
    {"view.paused", SK_BOOL, "", "", true},
    {"view.photo", SK_BOOL, "", "", true},
    {"view.menu", SK_BOOL, "", "", true},
    {"view.cutscene", SK_BOOL, "", "", true},
    {"view.spectating", SK_BOOL, "", "", true},
    {"session.start", SK_EVENT, "", "", true},
    {"race.start", SK_EVENT, "", "", true},
    {"session.lap", SK_EVENT, "", "", true},
    {"session.end", SK_EVENT, "", "", true},
    {"car.speed", SK_NUMBER, "", "km/h", true},
    {"car.rpm", SK_NUMBER, "", "", true},
    {"car.gear", SK_NUMBER, "", "", true},
    {"car.throttle", SK_NUMBER, "", "", true},
    {"car.brake", SK_NUMBER, "", "", true},
    {"car.steer", SK_NUMBER, "", "", true},
    {"car.g_lateral", SK_NUMBER, "", "g", true},
    {"car.g_long", SK_NUMBER, "", "g", true},
    {"car.surface", SK_NAME, "tarmac kerb rumble grass gravel sand", "", true},
    {"car.impact", SK_EVENT, "", "g", false},
    {"car.damage", SK_EVENT, "", "", false},
    {"car.in_pit_lane", SK_BOOL, "", "", false},
    {"car.off_track", SK_BOOL, "", "", false},
    {"session.flag", SK_NAME, "none green blue yellow red", "", false},
    {"session.safety_car", SK_NAME, "none sc vsc formation", "", false},
    {"session.weather", SK_NAME, "clear light_cloud overcast light_rain heavy_rain storm", "", false},
    {"session.wetness", SK_NUMBER, "", "", true},
    {"session.flashback", SK_EVENT, "", "", false},
    {"session.chequered", SK_EVENT, "", "", false},
    {"car.wetness", SK_NUMBER, "", "", true},
};

inline int signal_index(std::string_view id)
{
    for (int i = 0; i < SIGNAL_COUNT; i++)
        if (SIGNALS[i].id == id)
            return i;
    return -1;
}

// The name of value i of a name signal ("" past the end).
inline std::string_view signal_value_name(int signal, size_t i)
{
    std::string_view v = SIGNALS[signal].values;
    size_t at = 0;
    for (size_t n = 0;; n++) {
        size_t end = v.find(' ', at);
        std::string_view word = v.substr(at, end == std::string_view::npos ? end : end - at);
        if (n == i)
            return word;
        if (end == std::string_view::npos)
            return {};
        at = end + 1;
    }
}

// The game's surface id (UDP m_surfaceType) as a value of car.surface: tarmac 0, kerb 1, rumble 2, grass 3,
// gravel 4, sand 5; -1 for no value (12 = no contact, or an id past the table).
inline int surface_value(int game_id)
{
    static constexpr int8_t MAP[12] = {0, 1, 0, 4, 4, 4, 5, 3, 0, 0, 0, 2};
    return game_id >= 0 && game_id < 12 ? MAP[game_id] : -1;
}

// The game keeps wheels as RL RR FL FR; the contract's order is FL FR RL RR.
inline void wheels_contract_order(const uint8_t game[4], uint8_t out[4])
{
    out[0] = game[2];
    out[1] = game[3];
    out[2] = game[0];
    out[3] = game[1];
}

enum CondOp : uint8_t { OP_TRUE, OP_IS, OP_ABOVE, OP_BELOW, OP_BETWEEN, OP_EVENT };

struct RuleCond {
    int signal = 0;
    CondOp op = OP_TRUE;
    uint32_t mask = 0;
    int wheels = 1;
    double value = 0, lo = 0, hi = 0;
};

enum SetMode : uint8_t { SET_OFF, SET_BLEND, SET_ROUND, SET_SWITCH };

struct RuleSet {
    int instance = 0;
    int slot = 0;
    SetMode mode = SET_BLEND;
    std::vector<float> values;
};

struct RuleSpec {
    std::string name;
    int priority = 0, order = 0;
    bool event = false;
    std::vector<RuleCond> when;
    bool has_ramp = false;
    int ramp_signal = 0;
    double ramp_from = 0, ramp_to = 1;
    double min_ms = 0, hold_ms = 0, for_ms = 400, fade_in_ms = 150, fade_out_ms = 300;
    std::vector<RuleSet> sets;
};

// The config swofty/shaders/active.json says runs now.
struct ActiveSpec {
    uint64_t serial = 0;
    std::string name, source, forced_by, toggle_key;
    std::vector<InstanceSpec> instances;
    std::vector<RuleSpec> rules;
};

inline bool parse_rule(const json::Value &r, RuleSpec &out)
{
    if (r.type != json::Value::Obj)
        return false;
    out = {};
    out.name = r["name"].str();
    out.priority = int(r["priority"].num());
    out.order = int(r["order"].num());
    out.event = r["event"].type == json::Value::Bool && r["event"].b;
    auto num = [&](const json::Value &v, double def) { return v.type == json::Value::Num ? v.n : def; };
    out.min_ms = num(r["min_ms"], 0);
    out.hold_ms = num(r["hold_ms"], 0);
    out.for_ms = num(r["for_ms"], 400);
    out.fade_in_ms = num(r["fade_in_ms"], 150);
    out.fade_out_ms = num(r["fade_out_ms"], 300);
    for (auto &w : r["when"].a) {
        RuleCond c;
        c.signal = signal_index(w["signal"].str());
        if (c.signal < 0)
            return false;
        std::string op = w["op"].str();
        c.op = op == "true"      ? OP_TRUE
               : op == "is"      ? OP_IS
               : op == "above"   ? OP_ABOVE
               : op == "below"   ? OP_BELOW
               : op == "between" ? OP_BETWEEN
               : op == "event"   ? OP_EVENT
                                 : OP_TRUE;
        if (op != "true" && op != "is" && op != "above" && op != "below" && op != "between" && op != "event")
            return false;
        for (auto &v : w["values"].a)
            if (v.type == json::Value::Num && v.n >= 0 && v.n < 32)
                c.mask |= 1u << unsigned(v.n);
        c.wheels = w["wheels"].type == json::Value::Num ? int(w["wheels"].n) : 1;
        c.value = num(w["value"], 0);
        c.lo = num(w["lo"], 0);
        c.hi = num(w["hi"], 0);
        out.when.push_back(c);
    }
    const json::Value &ramp = r["ramp"];
    if (ramp.type == json::Value::Obj) {
        out.ramp_signal = signal_index(ramp["signal"].str());
        if (out.ramp_signal < 0)
            return false;
        out.has_ramp = true;
        out.ramp_from = num(ramp["from"], 0);
        out.ramp_to = num(ramp["to"], 1);
    }
    for (auto &s : r["set"].a) {
        RuleSet x;
        x.instance = int(s["instance"].num(-1));
        if (x.instance < 0)
            continue;
        if (s["enabled"].type == json::Value::Bool && !s["enabled"].b) {
            x.mode = SET_OFF;
            out.sets.push_back(std::move(x));
            continue;
        }
        std::string mode = s["mode"].str();
        x.mode = mode == "round" ? SET_ROUND : mode == "switch" ? SET_SWITCH : SET_BLEND;
        x.slot = int(s["slot"].num(-1));
        for (auto &v : s["values"].a)
            x.values.push_back(v.type == json::Value::Num ? float(v.n) : 0.0f);
        if (x.slot < 0 || x.values.empty() || size_t(x.slot) + x.values.size() > SLOTS)
            continue;
        out.sets.push_back(std::move(x));
    }
    return true;
}

inline bool parse_active(const std::string &text, ActiveSpec &out)
{
    json::Value root;
    if (!json::parse(text, root) || root.type != json::Value::Obj)
        return false;
    out = {};
    out.serial = uint64_t(root["serial"].num());
    out.name = root["name"].str();
    out.source = root["source"].str();
    out.forced_by = root["forced_by"].str();
    out.toggle_key = root["toggle_key"].str();
    for (auto &i : root["instances"].a) {
        InstanceSpec s;
        s.effect = i["effect"].str();
        s.label = i["label"].str();
        for (auto &v : i["params"].a)
            s.params.push_back(v.type == json::Value::Num ? float(v.n) : 0.0f);
        s.index = int(&i - root["instances"].a.data());
        if (!s.effect.empty())
            out.instances.push_back(std::move(s));
    }
    for (auto &r : root["rules"].a) {
        RuleSpec rule;
        if (parse_rule(r, rule))
            out.rules.push_back(std::move(rule));
    }
    return true;
}

// An instance's params as the 32 option slots: missing ones are 0, extra ones are dropped.
inline void pack_slots(const std::vector<float> &params, float out[SLOTS])
{
    for (size_t i = 0; i < SLOTS; i++)
        out[i] = i < params.size() ? params[i] : 0.0f;
}

// One texture of one instance. Its name is "<instance>/<texture>".
struct PlanTex {
    int instance = 0;
    std::string effect, name;
    TexDecl decl;
};

// One pass of one instance. Inputs and output index the plan's textures, or are IN_ORIGINAL /
// IN_PREVIOUS / IN_NONE (an output of IN_NONE writes the picture).
struct PlanPass {
    int instance = 0;
    const PassDecl *decl = nullptr;
    int inputs[MAX_INPUTS] = {IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE, IN_NONE};
    size_t n_inputs = 0;
    int output = IN_NONE;
};

struct PlanInstance {
    int source = 0;
    std::string effect, label;
    const EffectDecl *decl = nullptr;
    float options[SLOTS] = {};
};

struct Plan {
    std::vector<PlanInstance> instances;
    std::vector<PlanTex> textures;
    std::vector<PlanPass> passes;
    std::vector<std::string> dropped;

    bool uses_previous() const
    {
        for (auto &p : passes)
            for (size_t i = 0; i < p.n_inputs; i++)
                if (p.inputs[i] == IN_PREVIOUS)
                    return true;
        return false;
    }
};

inline std::string instance_name(const std::string &effect, const std::string &label)
{
    return label.empty() ? effect : effect + "." + label;
}

// The instance plan for an active config. An instance whose effect is unknown, or has a pass that names
// an input that is not there, reads its own output or writes a texture from a file, is dropped with the
// reason in dropped; the rest keep their order. Instance numbers are positions among the kept instances.
inline Plan make_plan(const ActiveSpec &active, const std::vector<EffectDecl> &effects)
{
    Plan plan;
    for (size_t n = 0; n < active.instances.size(); n++) {
        const InstanceSpec &spec = active.instances[n];
        std::string who = instance_name(spec.effect, spec.label);
        const EffectDecl *fx = nullptr;
        for (auto &e : effects)
            if (e.id == spec.effect) {
                fx = &e;
                break;
            }
        if (!fx) {
            plan.dropped.push_back(who + ": no such effect");
            continue;
        }
        size_t tex_base = plan.textures.size(), pass_base = plan.passes.size();
        const char *bad = nullptr;
        auto texture_named = [&](const std::string &name) {
            if (name == "original")
                return IN_ORIGINAL;
            if (name == "previous")
                return IN_PREVIOUS;
            for (size_t i = tex_base; i < plan.textures.size(); i++)
                if (plan.textures[i].decl.name == name)
                    return int(i);
            return IN_NONE;
        };
        for (auto &t : fx->textures) {
            PlanTex x;
            x.effect = fx->id;
            x.decl = t;
            plan.textures.push_back(std::move(x));
        }
        for (auto &pd : fx->passes) {
            PlanPass p;
            p.decl = &pd;
            for (auto &in : pd.inputs) {
                int t = texture_named(in);
                if (t == IN_NONE || p.n_inputs >= MAX_INPUTS)
                    bad = "a pass names an input that is not there";
                else
                    p.inputs[p.n_inputs++] = t;
            }
            if (!pd.output.empty()) {
                p.output = texture_named(pd.output);
                if (p.output < 0 || !plan.textures[size_t(p.output)].decl.file.empty())
                    bad = "a pass writes a texture that is not there or comes from a file";
                for (size_t i = 0; i < p.n_inputs; i++)
                    if (p.output >= 0 && p.inputs[i] == p.output)
                        bad = "a pass reads the texture it writes";
            }
            plan.passes.push_back(p);
        }
        if (fx->slots > SLOTS)
            bad = "it needs more than 32 option slots";
        if (bad) {
            plan.dropped.push_back(who + ": " + bad);
            plan.textures.resize(tex_base);
            plan.passes.resize(pass_base);
            continue;
        }
        PlanInstance inst;
        inst.effect = spec.effect;
        inst.label = spec.label;
        inst.source = spec.index;
        inst.decl = fx;
        pack_slots(spec.params, inst.options);
        plan.instances.push_back(std::move(inst));
        for (size_t i = pass_base; i < plan.passes.size(); i++)
            plan.passes[i].instance = int(plan.instances.size() - 1);
        for (size_t i = tex_base; i < plan.textures.size(); i++) {
            plan.textures[i].instance = int(plan.instances.size() - 1);
            plan.textures[i].name = std::to_string(plan.instances.size() - 1) + "/" + plan.textures[i].decl.name;
        }
    }
    return plan;
}

// What decides the passes, textures and pipelines a plan needs: two plans with the same shape differ
// only in their option values.
inline std::string plan_shape(const Plan &plan)
{
    std::string out;
    for (auto &i : plan.instances) {
        out += instance_name(i.effect, i.label) + "{";
        for (auto &p : i.decl->passes)
            out += p.dxil + ";";
        out += "}";
    }
    return out;
}

// For each texture of a new plan, the texture of the old plan it takes over (-1 for none): the same
// instance position running the same effect with the same texture declaration.
inline std::vector<int> reuse_textures(const Plan &old, const Plan &now)
{
    std::vector<int> from(now.textures.size(), -1);
    for (size_t i = 0; i < now.textures.size(); i++)
        for (size_t j = 0; j < old.textures.size(); j++)
            if (old.textures[j].name == now.textures[i].name && old.textures[j].effect == now.textures[i].effect &&
                old.textures[j].decl == now.textures[i].decl) {
                from[i] = int(j);
                break;
            }
    return from;
}

// Pass title for logs: "<config> · <effect>[.<label>] · <pass>".
inline std::string pass_title(const std::string &config, const std::string &effect, const std::string &label,
                              const std::string &pass)
{
    return config + " \xC2\xB7 " + instance_name(effect, label) + " \xC2\xB7 " + pass;
}

// Scales an RGB24 picture to dw x dh by area-weighted averaging: every output pixel is the mean of the
// source area it covers, partial pixels by their share.
inline void box_scale_rgb(const uint8_t *src, uint32_t sw, uint32_t sh, uint8_t *dst, uint32_t dw, uint32_t dh)
{
    if (!sw || !sh || !dw || !dh)
        return;
    std::vector<float> row(size_t(sw) * 3);
    double fx = double(sw) / dw, fy = double(sh) / dh;
    for (uint32_t dy = 0; dy < dh; dy++) {
        std::fill(row.begin(), row.end(), 0.0f);
        double y0 = dy * fy, y1 = y0 + fy;
        for (uint32_t sy = uint32_t(y0); sy < sh && sy < y1; sy++) {
            double w = std::min<double>(sy + 1, y1) - std::max<double>(sy, y0);
            const uint8_t *line = src + size_t(sy) * sw * 3;
            for (size_t i = 0; i < size_t(sw) * 3; i++)
                row[i] += float(w) * line[i];
        }
        for (uint32_t dx = 0; dx < dw; dx++) {
            double x0 = dx * fx, x1 = x0 + fx, acc[3] = {};
            for (uint32_t sx = uint32_t(x0); sx < sw && sx < x1; sx++) {
                double w = std::min<double>(sx + 1, x1) - std::max<double>(sx, x0);
                for (int c = 0; c < 3; c++)
                    acc[c] += w * row[size_t(sx) * 3 + c];
            }
            for (int c = 0; c < 3; c++) {
                double v = acc[c] / (fx * fy) + 0.5;
                dst[(size_t(dy) * dw + dx) * 3 + c] = uint8_t(v > 255 ? 255 : v);
            }
        }
    }
}

// What the signals read at one presented frame. A signal without a value has has[] false: conditions on
// it fail and its events never occur. Bool signals hold 0 or 1, name signals the index of the value,
// numbers the number; car.surface holds one index per wheel (FL FR RL RR) in wheel[] (-1 for none). An
// event holds the magnitude of each occurrence this frame.
struct Signals {
    static constexpr size_t MAX_EVENTS = 4;
    bool has[SIGNAL_COUNT];
    double num[SIGNAL_COUNT];
    int wheel[4];
    uint8_t n_events[SIGNAL_COUNT];
    double magnitude[SIGNAL_COUNT][MAX_EVENTS];

    Signals() { clear(); }

    void clear()
    {
        for (int i = 0; i < SIGNAL_COUNT; i++) {
            has[i] = false;
            num[i] = 0;
            n_events[i] = 0;
        }
        for (int &w : wheel)
            w = -1;
    }

    void clear_events()
    {
        for (int i = 0; i < SIGNAL_COUNT; i++)
            n_events[i] = 0;
    }

    void set(int signal, double v)
    {
        has[signal] = true;
        num[signal] = v;
    }

    void set_surface(const int w[4])
    {
        has[S_CAR_SURFACE] = true;
        for (int i = 0; i < 4; i++)
            wheel[i] = w[i];
    }

    void add_event(int signal, double magnitude_ = 0)
    {
        if (n_events[signal] < MAX_EVENTS)
            magnitude[signal][n_events[signal]++] = magnitude_;
    }
};

// The rules of an active config, stepped once per presented frame (docs: shader events contract, section
// 4). reset() puts every rule off with weight 0 and takes the instances' base options; step() moves the
// rules by dt and leaves the blended options and the instances a rule turned off in options() and off().
class RuleEngine {
public:
    struct State {
        bool on = false;
        bool changed = false;
        double w = 0, a = 0, left = 0, true_for = 0, false_for = 0, target = 0;
    };

    void reset(const std::vector<RuleSpec> &rules, const Plan &plan)
    {
        rules_ = &rules;
        state_.assign(rules.size(), State{});
        base_.resize(plan.instances.size());
        for (size_t i = 0; i < plan.instances.size(); i++)
            std::copy(plan.instances[i].options, plan.instances[i].options + SLOTS, base_[i].begin());
        opts_ = base_;
        off_.assign(plan.instances.size(), 0);
        plan_of_.assign(rules.size(), {});
        order_.clear();
        for (size_t r = 0; r < rules.size(); r++) {
            for (auto &set : rules[r].sets) {
                int at = -1;
                for (size_t i = 0; i < plan.instances.size(); i++)
                    if (plan.instances[i].source == set.instance)
                        at = int(i);
                plan_of_[r].push_back(at);
            }
            order_.push_back(r);
        }
        std::stable_sort(order_.begin(), order_.end(), [&](size_t x, size_t y) {
            if (rules[x].priority != rules[y].priority)
                return rules[x].priority < rules[y].priority;
            return rules[x].order < rules[y].order;
        });
    }

    void step(const Signals &sig, double frame_ms)
    {
        if (!rules_)
            return;
        double dt = std::clamp(frame_ms, 0.0, 100.0);
        bool any = false;
        for (size_t r = 0; r < rules_->size(); r++) {
            const RuleSpec &rule = (*rules_)[r];
            State &st = state_[r];
            bool was = st.on;
            if (rule.event) {
                bool hit = false;
                for (auto &c : rule.when)
                    hit |= event_passes(c, sig);
                st.left = hit ? rule.for_ms : std::max(0.0, st.left - dt);
                st.on = st.left > 0;
                st.target = st.on ? 1 : 0;
            } else if (rule.has_ramp) {
                double t = 0;
                if (sig.has[rule.ramp_signal] && rule.ramp_to != rule.ramp_from)
                    t = std::clamp((sig.num[rule.ramp_signal] - rule.ramp_from) / (rule.ramp_to - rule.ramp_from), 0.0,
                                   1.0);
                st.target = t;
                st.on = t > 0;
            } else {
                bool cond = true;
                for (auto &c : rule.when)
                    cond = cond && holds(c, sig);
                st.true_for = cond ? st.true_for + dt : 0;
                st.false_for = cond ? 0 : st.false_for + dt;
                if (!st.on && cond && st.true_for >= rule.min_ms)
                    st.on = true;
                if (st.on && !cond && st.false_for > rule.hold_ms)
                    st.on = false;
                st.target = st.on ? 1 : 0;
            }
            st.changed = st.on != was;
            if (st.target > st.w)
                st.w = rule.fade_in_ms > 0 ? std::min(st.target, st.w + dt / rule.fade_in_ms) : st.target;
            else if (st.target < st.w)
                st.w = rule.fade_out_ms > 0 ? std::max(st.target, st.w - dt / rule.fade_out_ms) : st.target;
            st.a = st.w * st.w * (3 - 2 * st.w);
            any |= st.a > 0;
        }
        opts_ = base_;
        std::fill(off_.begin(), off_.end(), 0);
        if (!any)
            return;
        for (size_t r : order_) {
            double a = state_[r].a;
            if (a <= 0)
                continue;
            const RuleSpec &rule = (*rules_)[r];
            for (size_t k = 0; k < rule.sets.size(); k++) {
                int at = plan_of_[r][k];
                if (at < 0)
                    continue;
                const RuleSet &set = rule.sets[k];
                if (set.mode == SET_OFF) {
                    if (a >= 0.5)
                        off_[size_t(at)] = 1;
                    continue;
                }
                for (size_t c = 0; c < set.values.size(); c++) {
                    float &v = opts_[size_t(at)][size_t(set.slot) + c];
                    float target = set.values[c];
                    if (set.mode == SET_SWITCH) {
                        if (a >= 0.5)
                            v = target;
                    } else {
                        v = v + (target - v) * float(a);
                        if (set.mode == SET_ROUND)
                            v = std::round(v);
                    }
                }
            }
        }
    }

    size_t rules() const { return state_.size(); }
    const State &state(size_t rule) const { return state_[rule]; }
    const float *options(size_t instance) const { return opts_[instance].data(); }
    bool off(size_t instance) const { return off_[instance] != 0; }

private:
    static bool number_holds(const RuleCond &c, double x)
    {
        switch (c.op) {
        case OP_ABOVE:
            return x > c.value;
        case OP_BELOW:
            return x < c.value;
        case OP_BETWEEN:
            return x > c.lo && x < c.hi;
        default:
            return false;
        }
    }

    static bool holds(const RuleCond &c, const Signals &sig)
    {
        if (!sig.has[c.signal])
            return false;
        switch (c.op) {
        case OP_TRUE:
            return sig.num[c.signal] != 0;
        case OP_IS: {
            if (c.signal == S_CAR_SURFACE) {
                int n = 0;
                for (int w : sig.wheel)
                    n += w >= 0 && w < 32 && (c.mask >> w & 1);
                return n >= c.wheels;
            }
            int v = int(sig.num[c.signal]);
            return v >= 0 && v < 32 && (c.mask >> v & 1);
        }
        default:
            return number_holds(c, sig.num[c.signal]);
        }
    }

    static bool event_passes(const RuleCond &c, const Signals &sig)
    {
        for (uint8_t i = 0; i < sig.n_events[c.signal]; i++)
            if (c.op == OP_EVENT || number_holds(c, sig.magnitude[c.signal][i]))
                return true;
        return false;
    }

    const std::vector<RuleSpec> *rules_ = nullptr;
    std::vector<State> state_;
    std::vector<std::array<float, SLOTS>> base_, opts_;
    std::vector<char> off_;
    std::vector<std::vector<int>> plan_of_;
    std::vector<size_t> order_;
};

}
