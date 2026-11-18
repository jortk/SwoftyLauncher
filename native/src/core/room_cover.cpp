#include "runtime.hpp"

#include "room_cover.hpp"
#include "room_cover_logic.hpp"

#include <swofty/mixin.hpp>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <mutex>

namespace runtime::room_cover {

namespace {

std::mutex lock;
Cover cover;
Kind kind = Kind::none;
Progress progress;
std::string subtitle = "Starting your session";
std::atomic<bool> armed{false};
uint64_t polled_at = 0;
Signal last = Signal::waiting;

constexpr uint32_t BG_TOP = 0x0b0d12ffu;
constexpr uint32_t BG_BOTTOM = 0x181b22ffu;
constexpr uint32_t TEXT = 0xe9ecf3ffu;
constexpr uint32_t MUTED = 0xa3a8b7ffu;
constexpr uint32_t FAINT = 0x737988ffu;
constexpr uint32_t ACCENT = 0x8b7fd6ffu;
constexpr uint32_t ACCENT_HI = 0xa196e6ffu;
constexpr uint32_t TRACK = 0x272b35ffu;

uint32_t with_alpha(uint32_t rgba, float k)
{
    uint32_t a = uint32_t(std::clamp(float(rgba & 0xff) * k, 0.0f, 255.0f));
    return (rgba & 0xffffff00u) | a;
}

hud::TextStyle style(float px, uint32_t rgba, float tracking = 0)
{
    hud::TextStyle s;
    s.px = px;
    s.rgba = rgba;
    s.tracking = px * tracking;
    return s;
}

float text_width(const hud::TextStyle &st, const std::string &t)
{
    float w = 0, h = 0;
    hud::measure(st, t, &w, &h);
    return w;
}

void draw(hud::Frame &f, uint64_t now, float alpha)
{
    hud::reset_state(f);
    f.opacity = alpha;
    float W = f.width, H = f.height, u = H / 1080.0f, cx = W * 0.5f;
    hud::rect_gradient(f, 0, 0, W, H, 0, BG_TOP, BG_BOTTOM, 0);

    hud::TextStyle word = style(112 * u, TEXT, 0.16f);
    std::string name = "SWOFTY";
    float ww = text_width(word, name);
    float wy = H * 0.30f;
    hud::text(f, cx - ww * 0.5f + word.tracking * 0.5f, wy, word, name);

    hud::TextStyle tag = style(26 * u, ACCENT_HI, 0.62f);
    std::string tagline = "LAUNCHER";
    float tw = text_width(tag, tagline);
    float ty = wy + 112 * u * 1.25f;
    hud::text(f, cx - tw * 0.5f + tag.tracking * 0.5f, ty, tag, tagline);
    float rule = 64 * u, ry = ty + 13 * u;
    hud::rect_rounded(f, cx - tw * 0.5f - 24 * u - rule, ry, rule, 2 * u, u, with_alpha(ACCENT, 0.55f));
    hud::rect_rounded(f, cx + tw * 0.5f + 24 * u, ry, rule, 2 * u, u, with_alpha(ACCENT, 0.55f));

    hud::TextStyle sub = style(34 * u, MUTED, 0.04f);
    float sw = text_width(sub, subtitle);
    float sy = H * 0.55f;
    hud::text(f, cx - sw * 0.5f, sy, sub, subtitle);

    float bw = std::min(W * 0.34f, 720 * u * 1.6f), bh = 8 * u, bx = cx - bw * 0.5f, by = H * 0.67f;
    float shown = progress.shown(now);
    int pct = progress.percent(now);

    hud::TextStyle status = style(24 * u, MUTED);
    hud::TextStyle pctst = style(24 * u, TEXT, 0.04f);
    std::string pt = std::to_string(pct) + "%";
    float ly = by - 24 * u - 34 * u;
    hud::text(f, bx, ly, status, progress.label());
    float pw = text_width(pctst, pt);
    hud::text(f, bx + bw - pw, ly, pctst, pt);

    hud::rect_rounded(f, bx, by, bw, bh, bh * 0.5f, TRACK);
    float fw = std::max(bh, bw * shown);
    hud::rect_gradient(f, bx, by, fw, bh, bh * 0.5f, ACCENT, ACCENT_HI, SWOFTY_GRADIENT_HORIZONTAL);
    float sweep = std::fmod(float(now % 1800) / 1800.0f, 1.0f);
    float sx = bx + (fw + 90 * u) * sweep - 90 * u, sl = std::max(sx, bx), sr = std::min(sx + 90 * u, bx + fw);
    if (!progress.completed() && sr - sl > bh)
        hud::rect_rounded(f, sl, by, sr - sl, bh, bh * 0.5f, with_alpha(0xffffffffu, 0.22f));
    hud::rect_rounded(f, bx + fw - bh, by - 2 * u, bh, bh + 4 * u, (bh + 4 * u) * 0.5f, with_alpha(ACCENT_HI, 0.35f));

    hud::TextStyle foot = style(20 * u, FAINT, 0.04f);
    std::string note = "This only takes a moment";
    float nw = text_width(foot, note);
    hud::text(f, cx - nw * 0.5f, by + 40 * u, foot, note);
}

}

void step(const char *what, float fraction, float ceiling)
{
    std::lock_guard<std::mutex> g(lock);
    progress.step(what, fraction, GetTickCount64(), ceiling);
}

Snapshot snapshot()
{
    Snapshot out;
    if (!armed.load())
        return out;
    std::lock_guard<std::mutex> g(lock);
    out.active = true;
    out.label = progress.label();
    out.percent = progress.target_percent();
    return out;
}

void set_subtitle(const char *text)
{
    if (!text || !*text)
        return;
    std::lock_guard<std::mutex> g(lock);
    subtitle = text;
}

bool setup()
{
    char env[1024] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_DIRECT_START", env, sizeof env);
    std::string value = n && n < sizeof env ? std::string(env) : std::string(), text;
    if (value.empty())
        text = read_file(swofty_root() + L"direct_start.json", size_t(1) << 16);
    else if (value[0] == '@')
        text = read_file(wide(value.c_str() + 1), size_t(1) << 16);
    Kind k = request_kind(value, text);
    if (k == Kind::none)
        return false;
    std::lock_guard<std::mutex> g(lock);
    kind = k;
    if (k == Kind::session) {
        json::Value v;
        bool tt = json::parse(text, v) && v["mode"].str() == "time_trial";
        subtitle = tt ? "Starting a Time Trial" : "Starting a Grand Prix session";
    }
    cover.arm(k == Kind::session ? SESSION_CAP_MS : CAP_MS);
    armed.store(true);
    return true;
}

bool active()
{
    return armed.load();
}

void paint(hud::Frame &f, uint64_t now)
{
    if (!armed.load())
        return;
    std::lock_guard<std::mutex> g(lock);
    if (now - polled_at >= 100) {
        polled_at = now;
        if (kind == Kind::session) {
            int sig = direct_start_cover_signal();
            last = sig < 0 ? Signal::failed : sig > 0 ? Signal::shown : Signal::waiting;
        } else {
            int sig = session_host_cover_signal();
            last = sig < 0 ? Signal::failed : sig > 0 ? Signal::shown : Signal::waiting;
        }
    }
    Event ev = Event::none;
    float alpha = cover.frame(now, last, &ev);
    const char *what = kind == Kind::session ? "test session" : "room launch";
    if (ev == Event::went_up)
        swofty::log("[cover] %s cover up (%.0fx%.0f)", what, double(f.width), double(f.height));
    else if (ev == Event::came_off)
        swofty::log("[cover] %s cover off: %s", what, why_name(cover.why()));
    if (!cover.drawing()) {
        armed.store(false);
        return;
    }
    if (alpha <= 0)
        return;
    if (last == Signal::shown)
        progress.complete("Ready", now);
    draw(f, now, alpha);
}

}
