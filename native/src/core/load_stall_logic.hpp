#pragma once

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>

// The load-stall watchdog's trigger (load_stall.cpp). Watch takes every flow state transition the core sees
// (career.cpp's flow site) and every UI screen the game opens (ui.cpp), and poll says when a load has sat still
// long enough for a report of every thread's stack.
//
// A load is in progress while the newest screen is a loading screen or while StateLoadSession or StateUnloadSession is
// out of None. A loading screen's name holds "loading" (loading_transitions, loading) or it is the boot's
// splash_transition (boot_loading_screen), which the game shows after Press any button while it loads the profile and
// the main menu: 12.9 s at the median and 25.9 s at p90 in 382 real boots, and a 1.9.15 boot an addon held on it for
// 140 s got no report while only "loading" counted. Progress is any flow transition or screen open; the stall is the
// time since the last one while a load is in progress. The threshold is kLoadingMs while only a loading screen holds
// the load (the hand-off from "State launch exited" to StateLoadSession took at most 1.3 s in 24 real loads) and
// kSessionLoadMs while a session load or unload flow is out of None (real StateLoadSession to StateSession took 8.9 s
// at the median and up to 169 s across 468 loads). A stall gets its first report at the threshold and a second at
// kRepeatFactor times it; progress starts a new stall; a process gets at most kMaxReports.
//
// The game's own hang signals report at once, load or not (game). GameDeadlock is the game's WARN "The signal timed
// out, this is unexpected and may indicate a deadlock" (game_deadlock_line): in 20 real hangs it came 15.1 to 15.4 s
// after the last flow transition, and the game exited with 0xcfffffff 34 to 72 s after it (0xc0000005 from its
// watchdog 27 to 32 s after it where the grace did not hold), so the 180 s session threshold never saw these.
// GameWatchdog is the boot watchdog grace holding the game's kill (watchdog_grace.cpp): the frame tick has been
// silent past the game's 30 s with a flow in Entering or recently Updating. The first such signal in a stall gets a
// report at the next poll unless a report already came after it, the next report of that stall is due kFollowUpMs
// after it, and both count against kReportsPerStall and kMaxReports.

namespace runtime::load_stall {

constexpr uint64_t kLoadingMs = 45000;
constexpr uint64_t kSessionLoadMs = 180000;
constexpr uint64_t kRepeatFactor = 3;
constexpr int kReportsPerStall = 2;
constexpr int kMaxReports = 4;
constexpr uint64_t kFollowUpMs = 15000;
constexpr size_t kName = 64;
constexpr size_t kSlots = 32;

enum Status : uint32_t { None = 0, Entering = 1, Updating = 2, Exiting = 3 };
enum Trigger : uint32_t { Threshold = 0, GameDeadlock = 1, GameWatchdog = 2, FollowUp = 3, kTriggers = 4 };

inline const char *status_name(uint32_t s)
{
    static const char *const names[] = {"None", "Entering", "Updating", "Exiting"};
    return s < 4 ? names[s] : "?";
}

inline bool boot_loading_screen(const char *name)
{
    return name && !std::strcmp(name, "splash_transition");
}

inline bool loading_screen(const char *name)
{
    return name && (std::strstr(name, "loading") || boot_loading_screen(name));
}

inline bool session_load_flow(const char *name)
{
    return name && (!std::strcmp(name, "StateLoadSession") || !std::strcmp(name, "StateUnloadSession"));
}

inline bool game_deadlock_line(const char *line)
{
    if (!line || std::strncmp(line, "[game] ", 7))
        return false;
    const char *at = std::strstr(line + 7, "The signal timed out");
    return at && std::strstr(at, "deadlock");
}

struct Flow {
    uint32_t hash = 0;
    uint32_t status = None;
    uint64_t since = 0;
    char name[kName] = {};
    bool used = false;
};

struct Due {
    bool report = false;
    int index = 0;
    uint64_t stalled_ms = 0;
    uint64_t threshold_ms = 0;
    uint32_t trigger = Threshold;
    uint32_t cause = Threshold;
    uint64_t signal_ms = 0;
    uint64_t detail_ms = 0;
    uint64_t previous_ms = 0;
    bool moved = false;
};

inline size_t headline(const Due &d, char *out, size_t cap)
{
    if (!out || !cap)
        return 0;
    char last[96];
    if (d.moved)
        std::snprintf(last, sizeof last, "the last flow transition or screen open was %.1f s ago",
                      double(d.stalled_ms) / 1000);
    else
        std::snprintf(last, sizeof last, "no flow transition or screen open seen yet");
    const char *cause = d.cause == GameWatchdog ? "the game's watchdog" : "the game's deadlock warning";
    int n = 0;
    switch (d.trigger) {
    case GameDeadlock:
        n = std::snprintf(out, cap,
                          "the game logged \"The signal timed out, this is unexpected and may indicate a deadlock\" "
                          "%.1f s ago; %s",
                          double(d.signal_ms) / 1000, last);
        break;
    case GameWatchdog:
        n = std::snprintf(out, cap,
                          "the game's watchdog found the frame tick silent for %.1f s and the boot watchdog grace is "
                          "holding its kill; %s",
                          double(d.detail_ms) / 1000, last);
        break;
    case FollowUp:
        n = std::snprintf(out, cap, "%.1f s after report %d (asked for by %s), nothing has moved; %s",
                          double(d.previous_ms) / 1000, d.index - 1, cause, last);
        break;
    default:
        n = std::snprintf(out, cap,
                          "the load has not moved for %.1f s (no flow transition or screen open; threshold %.0f s)",
                          double(d.stalled_ms) / 1000, double(d.threshold_ms) / 1000);
        break;
    }
    return n < 0 ? 0 : size_t(n) >= cap ? cap - 1 : size_t(n);
}

inline void copy_name(char *dst, const char *src)
{
    std::snprintf(dst, kName, "%.*s", int(kName - 1), src ? src : "");
}

class Watch {
public:
    uint64_t loading_ms = kLoadingMs;
    uint64_t session_ms = kSessionLoadMs;

    void screen(const char *name, uint64_t now)
    {
        copy_name(screen_, name);
        screen_at_ = now;
        progress(now);
    }

    void flow(uint32_t hash, uint32_t to, const char *name, uint64_t now)
    {
        copy_name(last_flow_, name);
        last_to_ = to;
        last_flow_at_ = now;
        flows_seen_ = true;
        Flow *slot = find(hash);
        if (to == None) {
            if (slot)
                *slot = Flow{};
        } else {
            if (!slot)
                slot = take();
            if (slot->hash != hash || slot->status != to)
                slot->since = now;
            slot->hash = hash;
            slot->status = to;
            slot->used = true;
            copy_name(slot->name, name);
        }
        progress(now);
    }

    const Flow *session_flow() const
    {
        for (const Flow &f : flows_)
            if (f.used && session_load_flow(f.name))
                return &f;
        return nullptr;
    }

    bool in_loading_screen() const
    {
        return loading_screen(screen_);
    }

    bool in_boot_loading_screen() const
    {
        return boot_loading_screen(screen_);
    }

    uint64_t threshold() const
    {
        if (session_flow())
            return session_ms;
        return in_loading_screen() ? loading_ms : 0;
    }

    void game(uint32_t trigger, uint64_t at, uint64_t detail_ms)
    {
        if ((trigger != GameDeadlock && trigger != GameWatchdog) || game_set_ || (moved_ && at < progress_))
            return;
        game_set_ = true;
        game_at_ = at;
        game_why_ = trigger;
        game_ms_ = detail_ms;
    }

    Due poll(uint64_t now)
    {
        Due d;
        if (total_ >= kMaxReports || stall_reports_ >= kReportsPerStall || now < progress_)
            return d;
        uint64_t stalled = now - progress_;
        if (game_set_) {
            bool fresh = !stall_reports_ || reported_at_ < game_at_;
            if (!fresh && now < reported_at_ + kFollowUpMs)
                return d;
            d.trigger = fresh ? game_why_ : FollowUp;
            d.cause = game_why_;
            d.signal_ms = now >= game_at_ ? now - game_at_ : 0;
            d.detail_ms = game_ms_;
            d.previous_ms = fresh ? 0 : now - reported_at_;
            return take_report(d, now, stalled, 0);
        }
        uint64_t t = threshold();
        if (!t)
            return d;
        uint64_t due = stall_reports_ == 0 ? t : t * kRepeatFactor;
        if (stalled < due)
            return d;
        return take_report(d, now, stalled, t);
    }

    int reports() const
    {
        return total_;
    }

    uint64_t last_progress() const
    {
        return progress_;
    }

    size_t describe(char *out, size_t cap, uint64_t now) const
    {
        size_t at = 0;
        auto put = [&](const char *f, auto... a) {
            if (at + 1 >= cap)
                return;
            int n = std::snprintf(out + at, cap - at, f, a...);
            at = n < 0 ? at : (at + size_t(n) >= cap ? cap - 1 : at + size_t(n));
        };
        auto ago = [&](uint64_t t) { return double(now >= t ? now - t : 0) / 1000.0; };
        if (screen_[0])
            put("newest screen \"%s\", opened %.1f s ago%s\n", screen_, ago(screen_at_),
                in_loading_screen() ? " (a loading screen)" : "");
        else
            put("%s", "newest screen: none seen\n");
        if (flows_seen_)
            put("last flow transition: \"%s\" -> %s, %.1f s ago\n", last_flow_, status_name(last_to_), ago(last_flow_at_));
        else
            put("%s", "last flow transition: none seen\n");
        const Flow *s = session_flow();
        if (s)
            put("session load flow: \"%s\" in %s for %.1f s\n", s->name, status_name(s->status), ago(s->since));
        else
            put("%s", "session load flow: none (StateLoadSession and StateUnloadSession are in None)\n");
        int n = 0;
        for (const Flow &f : flows_)
            if (f.used && (f.status == Entering || f.status == Exiting))
                n++;
        put("flows in Entering or Exiting: %d\n", n);
        for (const Flow &f : flows_)
            if (f.used && (f.status == Entering || f.status == Exiting))
                put("  \"%s\" in %s for %.1f s\n", f.name, status_name(f.status), ago(f.since));
        n = 0;
        for (const Flow &f : flows_)
            if (f.used && f.status == Updating)
                n++;
        put("flows in Updating: %d\n", n);
        for (const Flow &f : flows_)
            if (f.used && f.status == Updating)
                put("  \"%s\" for %.1f s\n", f.name, ago(f.since));
        if (game_set_)
            put("game hang signal: %s, %.1f s ago\n",
                game_why_ == GameWatchdog ? "the watchdog grace holding the game's kill" : "the game's deadlock warning",
                ago(game_at_));
        return at;
    }

private:
    char screen_[kName] = {};
    uint64_t screen_at_ = 0;
    char last_flow_[kName] = {};
    uint32_t last_to_ = None;
    uint64_t last_flow_at_ = 0;
    bool flows_seen_ = false;
    uint64_t progress_ = 0;
    bool moved_ = false;
    int stall_reports_ = 0;
    int total_ = 0;
    uint64_t reported_at_ = 0;
    bool game_set_ = false;
    uint64_t game_at_ = 0;
    uint32_t game_why_ = Threshold;
    uint64_t game_ms_ = 0;
    Flow flows_[kSlots];

    void progress(uint64_t now)
    {
        progress_ = now;
        moved_ = true;
        stall_reports_ = 0;
        game_set_ = false;
    }

    Due take_report(Due d, uint64_t now, uint64_t stalled, uint64_t threshold)
    {
        stall_reports_++;
        total_++;
        reported_at_ = now;
        d.report = true;
        d.index = stall_reports_;
        d.stalled_ms = stalled;
        d.threshold_ms = threshold;
        d.moved = moved_;
        return d;
    }

    Flow *find(uint32_t hash)
    {
        for (Flow &f : flows_)
            if (f.used && f.hash == hash)
                return &f;
        return nullptr;
    }

    Flow *take()
    {
        Flow *oldest = &flows_[0];
        for (Flow &f : flows_) {
            if (!f.used)
                return &f;
            if (!session_load_flow(f.name) && (session_load_flow(oldest->name) || f.since < oldest->since))
                oldest = &f;
        }
        *oldest = Flow{};
        return oldest;
    }
};

}
