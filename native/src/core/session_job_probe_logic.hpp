#pragma once

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>

// The wheel tyre instance probe (session_job_probe.cpp). Four 1.9.6-1.9.39 hangs entering the next session of a race
// weekend had a job thread inside game function 0x1445acb00 (Game_Vfx_UpdateWheelTyreInstance), which the renderer
// module's per-vehicle job 0x1401717b0 calls once per wheel, for three minutes until the game's watchdog killed the game. The function walks an outer list of
// 0x20-byte items whose count is (end - begin) >> 5, tested unsigned against the index, and for every outer index scans
// an inner list of entry pointers, each with a u32 list; every miss captures a stack trace. An outer list whose end is
// below its begin gives a count the unsigned test never reaches, and a huge one runs for hours. The probe reads both
// lists on entry and logs their sizes and pointers once per distinct value, with a [warning] line when a size is
// negative, huge or not a whole number of items.
//
// The function runs for every wheel of every car every frame on the game's job threads, so the probe is armed only
// while a session is entered: Window arms when StateSession or StateLoadSession goes to Entering (the next session of
// a weekend enters through StateSession alone, and the four hangs stopped there) for kArmMs, cut to kSettleMs after
// StateSession reaches Updating. Outside the window a call costs one relaxed atomic load. Inside it the dedupe is a
// fixed array of atomic keys taken by compare-and-swap, with no lock; the kPerSession-th distinct value disarms the
// window for the rest of that session, and so does the deadline, checked against the caller's tick.

namespace runtime::session_job_probe {

constexpr uint64_t kOuterItem = 0x20;
constexpr uint64_t kInnerItem = 8;
constexpr uint64_t kListItem = 4;
constexpr int64_t kHuge = int64_t(1) << 16;
constexpr int kPerSession = 8;
constexpr uint64_t kArmMs = 20000;
constexpr uint64_t kSettleMs = 10000;
constexpr int kListsRead = 64;

enum Gate : uint32_t {
    Runs = 0,
    NoVehicle = 1,
    NoOwner = 2,
    NoHolder = 3,
    NoState = 4,
    FlagOff = 5,
    NoLists = 6,
    Pending = 7,
    OuterEmpty = 8,
    Unread = 9,
};

inline const char *gate_text(uint32_t g)
{
    switch (g) {
    case Runs:
        return "the nested loop runs";
    case NoVehicle:
        return "no loop: its vehicle id is 0xff";
    case NoOwner:
        return "no loop: its fifth argument is null";
    case NoHolder:
        return "no loop: the wheel's slot holds no object";
    case NoState:
        return "no loop: the object's +0x48 is null";
    case FlagOff:
        return "no loop: the +0x31 flag is off";
    case NoLists:
        return "no loop: the object's +0x40 is null";
    case Pending:
        return "no loop: its pending list is not empty";
    case OuterEmpty:
        return "no loop: the outer list is empty";
    default:
        return "not read: a pointer on the way is not readable";
    }
}

struct Span {
    uint64_t begin = 0, end = 0;
    bool read = false;

    int64_t bytes() const
    {
        return int64_t(end - begin);
    }
};

enum Verdict : uint32_t { Ok = 0, Negative = 1, Huge = 2, Ragged = 3, NotRead = 4 };

inline Verdict verdict(const Span &s, uint64_t item)
{
    if (!s.read)
        return NotRead;
    int64_t b = s.bytes();
    if (b < 0)
        return Negative;
    if (b / int64_t(item) > kHuge)
        return Huge;
    if (b % int64_t(item))
        return Ragged;
    return Ok;
}

inline int64_t items(const Span &s, uint64_t item)
{
    return s.bytes() / int64_t(item);
}

inline uint64_t loop_count(const Span &s)
{
    return uint64_t(s.bytes() >> 5);
}

struct Entry {
    uint32_t vehicle = 0;
    uint32_t wheel = 0;
    uint32_t gate = Unread;
    uint64_t holder = 0;
    uint64_t lists = 0;
    Span outer;
    Span inner;
    int lists_read = 0;
    int64_t list_max = 0;
    int64_t list_min = 0;
    int lists_bad = 0;
};

inline uint64_t key(const Entry &e)
{
    uint64_t h = 1469598103934665603ull;
    auto mix = [&](uint64_t v) {
        for (int i = 0; i < 8; i++) {
            h ^= (v >> (i * 8)) & 0xff;
            h *= 1099511628211ull;
        }
    };
    mix(e.gate);
    mix(e.outer.read ? uint64_t(e.outer.bytes()) : ~0ull);
    mix(e.inner.read ? uint64_t(e.inner.bytes()) : ~0ull);
    return h | 1;
}

inline bool bad(const Entry &e)
{
    Verdict o = verdict(e.outer, kOuterItem), i = verdict(e.inner, kInnerItem);
    return (o != Ok && o != NotRead) || (i != Ok && i != NotRead) || e.lists_bad || e.list_max > kHuge;
}

enum Admit : uint32_t { Repeat = 0, First = 1, FirstLast = 2 };

inline bool arms(uint32_t to, const char *name)
{
    return to == 1 && name && (!std::strcmp(name, "StateSession") || !std::strcmp(name, "StateLoadSession"));
}

inline bool settles(uint32_t to, const char *name)
{
    return to == 2 && name && !std::strcmp(name, "StateSession");
}

class Window {
public:
    void flow(uint32_t to, const char *name, uint64_t now)
    {
        if (arms(to, name)) {
            armed_.store(0, std::memory_order_relaxed);
            for (auto &k : keys_)
                k.store(0, std::memory_order_relaxed);
            until_.store(now + kArmMs, std::memory_order_relaxed);
            uint32_t g = gen_.load(std::memory_order_relaxed) + 1;
            gen_.store(g ? g : 1, std::memory_order_relaxed);
            armed_.store(g ? g : 1, std::memory_order_release);
        } else if (settles(to, name) && armed_.load(std::memory_order_relaxed)) {
            uint64_t settle = now + kSettleMs;
            if (settle < until_.load(std::memory_order_relaxed))
                until_.store(settle, std::memory_order_relaxed);
        }
    }

    uint32_t armed() const
    {
        return armed_.load(std::memory_order_relaxed);
    }

    bool live(uint32_t g, uint64_t now)
    {
        if (!g)
            return false;
        std::atomic_thread_fence(std::memory_order_acquire);
        if (now < until_.load(std::memory_order_relaxed))
            return true;
        disarm(g);
        return false;
    }

    Admit admit(uint32_t g, uint64_t key)
    {
        uint64_t k = (key ^ (uint64_t(g) * 0x9e3779b97f4a7c15ull)) | 1;
        for (int i = 0; i < kPerSession; i++) {
            uint64_t have = keys_[i].load(std::memory_order_acquire);
            if (have == k)
                return Repeat;
            if (have)
                continue;
            uint64_t expected = 0;
            if (keys_[i].compare_exchange_strong(expected, k, std::memory_order_acq_rel)) {
                if (i < kPerSession - 1)
                    return First;
                disarm(g);
                return FirstLast;
            }
            if (expected == k)
                return Repeat;
        }
        disarm(g);
        return Repeat;
    }

private:
    std::atomic<uint32_t> armed_{0};
    std::atomic<uint32_t> gen_{0};
    std::atomic<uint64_t> until_{0};
    std::atomic<uint64_t> keys_[kPerSession] = {};

    void disarm(uint32_t g)
    {
        armed_.compare_exchange_strong(g, 0, std::memory_order_relaxed);
    }
};

static_assert(std::atomic<uint64_t>::is_always_lock_free && std::atomic<uint32_t>::is_always_lock_free);

inline size_t clip(int n, size_t cap)
{
    return n < 0 ? 0 : size_t(n) >= cap ? cap - 1 : size_t(n);
}

inline const char *verdict_text(Verdict v)
{
    switch (v) {
    case Negative:
        return "NEGATIVE";
    case Huge:
        return "HUGE";
    case Ragged:
        return "not a whole number of items";
    case NotRead:
        return "not read";
    default:
        return "ok";
    }
}

inline size_t line(const Entry &e, uint64_t serial, char *out, size_t cap)
{
    if (!out || !cap)
        return 0;
    if (e.gate != Runs && !e.outer.read)
        return clip(std::snprintf(out, cap,
                                  "[session-job] 0x1445acb00 entry, session %llu: vehicle %u, wheel %u; %s (object "
                                  "0x%llx, lists 0x%llx)",
                                  (unsigned long long)serial, e.vehicle, e.wheel, gate_text(e.gate),
                                  (unsigned long long)e.holder, (unsigned long long)e.lists),
                    cap);
    return clip(std::snprintf(out, cap,
                              "[session-job] 0x1445acb00 entry, session %llu: vehicle %u, wheel %u; %s; outer list "
                              "0x%llx..0x%llx (%lld bytes, %lld items, %s, loop count %llu); inner list 0x%llx..0x%llx "
                              "(%lld bytes, %lld entries, %s); %d entry lists read, %lld..%lld u32s, %d bad (object "
                              "0x%llx, lists 0x%llx)",
                              (unsigned long long)serial, e.vehicle, e.wheel, gate_text(e.gate),
                              (unsigned long long)e.outer.begin, (unsigned long long)e.outer.end,
                              (long long)e.outer.bytes(), (long long)items(e.outer, kOuterItem),
                              verdict_text(verdict(e.outer, kOuterItem)), (unsigned long long)loop_count(e.outer),
                              (unsigned long long)e.inner.begin, (unsigned long long)e.inner.end,
                              (long long)e.inner.bytes(), (long long)items(e.inner, kInnerItem),
                              verdict_text(verdict(e.inner, kInnerItem)), e.lists_read, (long long)e.list_min,
                              (long long)e.list_max, e.lists_bad, (unsigned long long)e.holder,
                              (unsigned long long)e.lists),
                cap);
}

inline size_t warning(const Entry &e, uint64_t serial, char *out, size_t cap)
{
    if (!out || !cap)
        return 0;
    Verdict o = verdict(e.outer, kOuterItem), i = verdict(e.inner, kInnerItem);
    const char *why = o == Negative ? "the outer list's end is below its begin, so its count is negative and the "
                                      "loop's unsigned exit test never ends it"
                      : o == Huge   ? "the outer list's count is huge, so the loop runs for hours"
                      : i == Negative ? "the inner list's end is below its begin, so its scan runs off the list"
                      : i == Huge     ? "the inner list's count is huge"
                      : e.lists_bad   ? "an entry's u32 list has its end below its begin or is not whole u32s"
                      : e.list_max > kHuge ? "an entry's u32 list is huge"
                      : o == Ragged   ? "the outer list is not a whole number of 0x20-byte items"
                                      : "the inner list is not a whole number of pointers";
    return clip(std::snprintf(out, cap,
                              "[session-job] [warning] 0x1445acb00 entry, session %llu, vehicle %u, wheel %u: %s (outer %lld "
                              "bytes at 0x%llx, inner %lld bytes at 0x%llx, longest entry list %lld u32s)",
                              (unsigned long long)serial, e.vehicle, e.wheel, why, (long long)e.outer.bytes(),
                              (unsigned long long)e.outer.begin, (long long)e.inner.bytes(),
                              (unsigned long long)e.inner.begin, (long long)e.list_max),
                cap);
}

inline size_t last_line(uint64_t serial, char *out, size_t cap)
{
    if (!out || !cap)
        return 0;
    return clip(std::snprintf(out, cap,
                              "[session-job] 0x1445acb00: %d distinct values logged in session %llu; the probe is "
                              "off until the next session load",
                              kPerSession, (unsigned long long)serial),
                cap);
}

}
