#pragma once
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <string>

#include "json.hpp"

namespace runtime::room_cover {

constexpr uint64_t FADE_MS = 300;
constexpr uint64_t CAP_MS = 150000;
constexpr uint64_t SESSION_CAP_MS = 180000;
constexpr uint64_t SETTLE_MS = 1500;

enum class Signal { waiting, shown, failed };
enum class Why { none, shown, failed, timeout };
enum class Event { none, went_up, came_off };

inline const char *why_name(Why w)
{
    return w == Why::shown ? "shown" : w == Why::failed ? "failed" : w == Why::timeout ? "timeout" : "none";
}

enum class Kind { none, room, session };

inline Kind request_kind(const std::string &env, const std::string &file_text)
{
    std::string target;
    json::Value v;
    if (!env.empty() && env[0] != '@')
        target = env;
    else if (!file_text.empty() && json::parse(file_text, v))
        target = v["target"].str();
    return target == "room" ? Kind::room : target == "session" ? Kind::session : Kind::none;
}

inline bool request_is_room(const std::string &env, const std::string &file_text)
{
    return request_kind(env, file_text) == Kind::room;
}

constexpr uint64_t EASE_MS = 260;
constexpr uint64_t CREEP_MS = 7000;
constexpr float CREEP_SPAN = 0.10f;
constexpr float CREEP_LIMIT = 0.97f;
constexpr float START_FRACTION = 0.03f;

class Progress {
public:
    Progress() : label_("Starting F1 25"), target_(START_FRACTION), ceiling_(0.2f) {}

    void step(const char *what, float fraction, uint64_t now, float ceiling = -1.0f)
    {
        if (done_)
            return;
        fraction = std::clamp(fraction, 0.0f, CREEP_LIMIT);
        if (fraction < target_)
            return;
        if (what && label_ != what)
            label_ = what;
        if (fraction == target_)
            return;
        settle(now);
        target_ = fraction;
        at_ = now;
        float c = ceiling >= 0 ? ceiling : fraction + CREEP_SPAN;
        ceiling_ = std::clamp(c, fraction, std::max(fraction, CREEP_LIMIT));
    }

    void complete(const char *what, uint64_t now)
    {
        if (done_)
            return;
        settle(now);
        done_ = true;
        target_ = ceiling_ = 1.0f;
        if (what)
            label_ = what;
    }

    float shown(uint64_t now)
    {
        settle(now);
        return shown_;
    }

    int percent(uint64_t now)
    {
        int p = int(shown(now) * 100.0f + 0.5f);
        return done_ ? std::min(p, 100) : std::clamp(p, 0, 99);
    }

    const std::string &label() const { return label_; }
    float target() const { return target_; }
    int target_percent() const
    {
        return done_ ? 100 : std::clamp(int(target_ * 100.0f + 0.5f), 0, 99);
    }
    bool completed() const { return done_; }

private:
    float goal(uint64_t now) const
    {
        if (done_)
            return 1.0f;
        float t = float(now > at_ ? now - at_ : 0) / float(CREEP_MS);
        return target_ + (ceiling_ - target_) * (1.0f - std::exp(-t));
    }

    void settle(uint64_t now)
    {
        if (!anchored_) {
            anchored_ = true;
            clock_ = at_ = now;
            return;
        }
        if (now <= clock_)
            return;
        float dt = float(now - clock_);
        clock_ = now;
        float g = goal(now);
        if (g > shown_)
            shown_ += (g - shown_) * (1.0f - std::exp(-dt / float(EASE_MS)));
        if (done_ && shown_ > 0.999f)
            shown_ = 1.0f;
    }

    std::string label_;
    float target_, ceiling_;
    float shown_ = 0;
    uint64_t at_ = 0, clock_ = 0;
    bool done_ = false, anchored_ = false;
};

class Cover {
public:
    void arm(uint64_t cap_ms = CAP_MS)
    {
        if (phase_ == Phase::idle) {
            phase_ = Phase::armed;
            cap_ = cap_ms;
        }
    }

    bool armed() const { return phase_ != Phase::idle; }
    bool up() const { return phase_ == Phase::up; }
    bool drawing() const { return phase_ == Phase::armed || phase_ == Phase::up || phase_ == Phase::fading; }
    Why why() const { return why_; }

    float frame(uint64_t now, Signal s, Event *ev = nullptr)
    {
        Event e = Event::none;
        float alpha = step(now, s, e);
        if (ev)
            *ev = e;
        return alpha;
    }

private:
    enum class Phase { idle, armed, up, fading, off };

    float step(uint64_t now, Signal s, Event &e)
    {
        switch (phase_) {
        case Phase::idle:
        case Phase::off:
            return 0;
        case Phase::armed:
            phase_ = Phase::up;
            up_at_ = now;
            e = Event::went_up;
            return 1;
        case Phase::up:
            if (s == Signal::failed)
                return finish(now, Why::failed, 0, e);
            if (s == Signal::shown) {
                if (!seen_)
                    seen_at_ = now;
                seen_ = true;
            }
            if (seen_ && now >= seen_at_ + SETTLE_MS)
                return finish(now, Why::shown, FADE_MS, e);
            if (now >= up_at_ + cap_)
                return finish(now, Why::timeout, FADE_MS, e);
            return 1;
        case Phase::fading: {
            uint64_t t = now - off_at_;
            if (t >= FADE_MS) {
                phase_ = Phase::off;
                return 0;
            }
            return 1.0f - float(t) / float(FADE_MS);
        }
        }
        return 0;
    }

    float finish(uint64_t now, Why why, uint64_t fade, Event &e)
    {
        why_ = why;
        off_at_ = now;
        e = Event::came_off;
        phase_ = fade ? Phase::fading : Phase::off;
        return fade ? 1.0f : 0.0f;
    }

    Phase phase_ = Phase::idle;
    Why why_ = Why::none;
    uint64_t up_at_ = 0, off_at_ = 0, seen_at_ = 0, cap_ = CAP_MS;
    bool seen_ = false;
};

}
