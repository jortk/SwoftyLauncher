// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

// Per-frame plumbing for an addon that draws a HUD from live state: gating (is there anything to show),
// subject and session serials (reset eased state when they move), cadences (run a job every N seconds, at
// most K due jobs a frame) and a cost meter with a budget. One Frame per addon, driven from on_frame.
//
//   swofty::hud::Frame frame;
//   void on_frame(float dt) {
//       if (!frame.begin(gate_from_live(), dt)) { blank(); return; }
//       if (frame.changed_subject()) smoothers.reset();
//       if (frame.due(slow)) refresh_names();
//       ...
//       frame.end();
//   }
//
// The Gate is filled by the addon. When swofty::live (std lib live state) exists, a Gate is built from its
// session: live = a session is running and a car is read, serials = its generation counters.
//
// The cadence, cost and transition-log rules are the generic parts of Project Halo's halo_hud.cpp. Uses work
// from Project Halo by nyannoying, https://github.com/nyannoying1337/f1-25-project-halo

namespace swofty::clock {

inline double (*source)() = nullptr;

inline double steady()
{
    return std::chrono::duration<double>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

// Seconds from a monotonic origin. Tests replace the source (swofty::fake::Clock).
inline double now()
{
    return source ? source() : steady();
}

}

namespace swofty::hud {

struct Gate {
    bool live = false;
    bool spectating = false;
    bool paused = false;
    bool time_trial = false;
    uint32_t session_serial = 0;
    uint32_t subject_serial = 0;
};

struct Cadence {
    double period;
    double next = 0;
    explicit Cadence(double period_s = 1.0) : period(period_s) {}
};

struct Cost {
    float mean_us = 0, max_us = 0;
    int frames = 0;
};

struct FrameBudget {
    double max_us = 2000;
    int max_due = 2;
};

class Frame {
public:
    explicit Frame(FrameBudget b = FrameBudget{}) : budget_(b) {}

    // Call first in on_frame. False when nothing should be shown (no live session, or live gating is off):
    // blank your leaves and return. dt in seconds.
    bool begin(const Gate &g, double dt)
    {
        t0_ = clock::now();
        now_ += std::max(0.0, dt);
        due_used_ = 0;
        subject_changed_ = have_gate_ && (g.session_serial != gate_.session_serial || g.subject_serial != gate_.subject_serial);
        live_changed_ = !have_gate_ || g.live != gate_.live;
        gate_ = g;
        have_gate_ = true;
        return g.live;
    }

    const Gate &gate() const { return gate_; }
    double now() const { return now_; }
    bool changed_subject() const { return subject_changed_; }
    bool changed_live() const { return live_changed_; }

    // True when the cadence is due and a slot is free this frame. At most max_due cadences fire per frame; a
    // cadence that did not fit stays due and fires next frame.
    bool due(Cadence &c)
    {
        if (now_ < c.next || due_used_ >= budget_.max_due)
            return false;
        due_used_++;
        c.next = std::max(c.next + c.period, now_);
        return true;
    }

    double elapsed_us() const { return (clock::now() - t0_) * 1e6; }
    bool has_time() const { return elapsed_us() < budget_.max_us; }

    // Close the frame. True when a periodic summary is due (every 10 s): read cost() and log it.
    bool end()
    {
        double us = elapsed_us();
        sum_us_ += us;
        cost_frames_++;
        max_us_ = std::max(max_us_, float(us));
        over_budget_ += us > budget_.max_us;
        if (now_ - last_summary_ < 10.0)
            return false;
        last_summary_ = now_;
        summary_ = {float(sum_us_ / std::max(1, cost_frames_)), max_us_, cost_frames_};
        sum_us_ = 0;
        max_us_ = 0;
        cost_frames_ = 0;
        return true;
    }

    // Mean and max frame cost over the last summary window.
    Cost cost() const { return summary_; }
    int frames_over_budget() const { return over_budget_; }

    // Log lines for state changes, one line per change: if (frame.watch("tower", on)) log(frame.note()).
    // The first call for a label reports the initial value too.
    bool watch(const char *label, int64_t value)
    {
        for (auto &w : watched_)
            if (w.label == label) {
                if (w.value == value)
                    return false;
                std::snprintf(note_, sizeof note_, "%s: %lld -> %lld", label, (long long)w.value, (long long)value);
                w.value = value;
                return true;
            }
        watched_.push_back({label, value});
        std::snprintf(note_, sizeof note_, "%s: %lld", label, (long long)value);
        return true;
    }
    const char *note() const { return note_; }

private:
    struct Watched {
        std::string label;
        int64_t value;
    };

    FrameBudget budget_;
    Gate gate_;
    bool have_gate_ = false, subject_changed_ = false, live_changed_ = false;
    double now_ = 0, t0_ = 0, last_summary_ = 0, sum_us_ = 0;
    float max_us_ = 0;
    int due_used_ = 0, cost_frames_ = 0, over_budget_ = 0;
    Cost summary_;
    std::vector<Watched> watched_;
    char note_[96] = {};
};

}
