#pragma once

#include <cstddef>
#include <cstdint>
#include <cstring>

// The boot watchdog grace (re/db/globals.tsv site_Ne_WatchdogKillPath). The game's watchdog thread
// (Ne_Watchdog_ThreadLoop) wakes every second, reads the time since the frame tick last fed it (Ne_Watchdog_Tick)
// and kills the process when that exceeds WatchDog.delay (30000 ms). A front-end flow that sits in Entering for
// longer than that on a big modded profile stalls the frame tick for the whole load, and the game dies mid-load.
//
// Flows tracks the flows now in Entering or Updating. judge: the kill is held while a flow entered Entering no more
// than kEnteringMaxMs ago, or entered Updating no more than kUpdatingMaxMs ago (a session or front-end load keeps
// its heavy work in Updating after the flow leaves Entering), and the frame tick has been silent for at most kCapMs;
// past the cap the game's own kill runs. Every refusal has a Verdict so the log can name it. bucket: which
// kBucketMs-wide slice of the stall the silence is in, so the log speaks once per slice.

namespace runtime::watchdog_grace {

constexpr uint32_t kCapMs = 180000;
constexpr uint64_t kEnteringMaxMs = 600000;
constexpr uint64_t kUpdatingMaxMs = 300000;
constexpr uint32_t kBucketMs = 30000;
constexpr size_t kSlots = 16;
constexpr size_t kName = 48;

enum class Verdict { Hold, CapReached, NoFlow, TooOld, LockBusy, Shutdown };

inline const char *verdict_name(Verdict v)
{
    switch (v) {
    case Verdict::Hold: return "held";
    case Verdict::CapReached: return "cap reached";
    case Verdict::NoFlow: return "no flow in Entering or Updating";
    case Verdict::TooOld: return "newest flow too old";
    case Verdict::LockBusy: return "flow table busy";
    case Verdict::Shutdown: return "shutdown flag set";
    }
    return "?";
}

struct Entering {
    uint32_t hash = 0;
    uint64_t since = 0;
    char name[kName] = {};
    bool used = false;
    uint32_t state = 0;
};

struct Latest {
    uint64_t entering = 0;
    uint64_t updating = 0;
};

struct Judgement {
    Verdict verdict = Verdict::NoFlow;
    uint64_t age = 0;
    uint32_t state = 0;
};

inline Judgement judge(uint32_t elapsed_ms, uint64_t now, const Latest &l)
{
    Judgement j;
    if (elapsed_ms > kCapMs) {
        j.verdict = Verdict::CapReached;
        return j;
    }
    bool seen = false;
    uint64_t best_age = 0;
    uint32_t best_state = 0;
    const uint64_t since[2] = {l.entering, l.updating};
    const uint64_t max[2] = {kEnteringMaxMs, kUpdatingMaxMs};
    for (int i = 0; i < 2; i++) {
        if (!since[i])
            continue;
        uint64_t age = now >= since[i] ? now - since[i] : 0;
        if (age <= max[i]) {
            if (j.verdict != Verdict::Hold || age < j.age) {
                j.verdict = Verdict::Hold;
                j.age = age;
                j.state = uint32_t(i + 1);
            }
        } else if (!seen || age < best_age) {
            seen = true;
            best_age = age;
            best_state = uint32_t(i + 1);
        }
    }
    if (j.verdict == Verdict::Hold)
        return j;
    if (seen) {
        j.verdict = Verdict::TooOld;
        j.age = best_age;
        j.state = best_state;
    } else {
        j.verdict = Verdict::NoFlow;
    }
    return j;
}

class Flows {
public:
    void flow(uint32_t hash, uint32_t to, uint64_t now, const char *name)
    {
        for (auto &e : slots_)
            if (e.used && e.hash == hash) {
                e.used = false;
                break;
            }
        if (to != 1 && to != 2)
            return;
        Entering *put = nullptr;
        for (auto &e : slots_)
            if (!e.used) {
                put = &e;
                break;
            }
        if (!put) {
            put = &slots_[0];
            for (auto &e : slots_)
                if (e.since < put->since)
                    put = &e;
        }
        put->used = true;
        put->hash = hash;
        put->since = now;
        put->state = to;
        std::strncpy(put->name, name ? name : "", kName - 1);
        put->name[kName - 1] = 0;
    }

    Latest latest() const
    {
        Latest l;
        for (auto &e : slots_) {
            if (!e.used)
                continue;
            uint64_t &t = e.state == 1 ? l.entering : l.updating;
            if (e.since > t)
                t = e.since;
        }
        return l;
    }

    const Entering *newest(uint64_t now) const
    {
        const Entering *best = nullptr;
        for (auto &e : slots_) {
            uint64_t max = e.state == 1 ? kEnteringMaxMs : kUpdatingMaxMs;
            if (!e.used || now < e.since || now - e.since > max)
                continue;
            if (!best || e.since > best->since)
                best = &e;
        }
        return best;
    }

    const Entering *allows(uint32_t elapsed_ms, uint64_t now) const
    {
        return elapsed_ms <= kCapMs ? newest(now) : nullptr;
    }

    Judgement judge(uint32_t elapsed_ms, uint64_t now) const
    {
        return runtime::watchdog_grace::judge(elapsed_ms, now, latest());
    }

    size_t open(uint64_t now) const
    {
        size_t n = 0;
        for (auto &e : slots_) {
            uint64_t max = e.state == 1 ? kEnteringMaxMs : kUpdatingMaxMs;
            if (e.used && now >= e.since && now - e.since <= max)
                n++;
        }
        return n;
    }

private:
    Entering slots_[kSlots];
};

inline uint32_t bucket(uint32_t elapsed_ms)
{
    return elapsed_ms / kBucketMs;
}

}
