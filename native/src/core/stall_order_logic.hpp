#pragma once

#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>

// The load-stall report's running-threads section (crash.cpp, runtime::stall_report). A hang where one job thread
// spins while the main thread waits on it reads as "every thread idle" in a report that lists 200 threads in snapshot
// order (four 1.9.6-1.9.39 hangs entering a race weekend's next session, all with a job thread inside game function
// 0x1445acb00 under RtlCaptureStackBackTrace). So the report lists first every thread whose top frames are not in a
// wait, those with a frame in F1_25.exe ahead of the rest, each with the registers it was suspended with, the
// non-volatile registers the unwind restored at its first game frame, the first qwords of that frame's stack and its
// top frames. The thread table below it stays as it was.

namespace runtime::stall_order {

constexpr int kTopFrames = 16;
constexpr int kFrameQwords = 24;

inline bool wait_export(const char *name)
{
    static const char *const waits[] = {"NtWait",
                                        "ZwWait",
                                        "NtDelayExecution",
                                        "ZwDelayExecution",
                                        "NtRemoveIoCompletion",
                                        "ZwRemoveIoCompletion",
                                        "GetQueuedCompletionStatus",
                                        "NtSignalAndWait",
                                        "ZwSignalAndWait",
                                        "NtUserMsgWaitForMultipleObjects",
                                        "RtlSleepConditionVariable",
                                        "RtlAcquireSRWLock",
                                        "RtlEnterCriticalSection",
                                        "RtlpWaitOnCriticalSection",
                                        "NtReadFile",
                                        "NtDeviceIoControlFile",
                                        "NtFsControlFile",
                                        "WaitOnAddress",
                                        "RtlWaitOnAddress",
                                        "SleepEx",
                                        "WaitForSingleObject",
                                        "WaitForMultipleObjects",
                                        "SleepConditionVariable"};
    if (!name)
        return false;
    for (const char *p : waits)
        if (!std::strncmp(name, p, std::strlen(p)))
            return true;
    return false;
}

struct Thread {
    bool read = false;
    bool waiting = false;
    bool game = false;
};

inline int running_first(const Thread *t, int n, int *out)
{
    int k = 0;
    for (int pass = 0; pass < 2; pass++)
        for (int i = 0; i < n; i++)
            if (t[i].read && !t[i].waiting && t[i].game == (pass == 0))
                out[k++] = i;
    return k;
}

enum Reg { Rax, Rbx, Rcx, Rdx, Rsi, Rdi, Rbp, Rsp, R8, R9, R10, R11, R12, R13, R14, R15, Rip, EFlags, kRegs };

inline const char *reg_name(int r)
{
    static const char *const names[kRegs] = {"rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8",
                                             "r9",  "r10", "r11", "r12", "r13", "r14", "r15", "rip", "efl"};
    return r >= 0 && r < kRegs ? names[r] : "?";
}

struct Kept {
    int frame = -1;
    uint64_t rip = 0, rsp = 0;
    uint64_t rbx = 0, rbp = 0, rsi = 0, rdi = 0, r12 = 0, r13 = 0, r14 = 0, r15 = 0;
};

class Out {
public:
    Out(char *b, size_t cap) : b_(b), cap_(cap)
    {
        if (b_ && cap_)
            b_[0] = 0;
    }

    void put(const char *f, ...) __attribute__((format(printf, 2, 3)))
    {
        if (!b_ || at_ + 1 >= cap_)
            return;
        va_list ap;
        va_start(ap, f);
        int k = std::vsnprintf(b_ + at_, cap_ - at_, f, ap);
        va_end(ap);
        if (k > 0)
            at_ = at_ + size_t(k) >= cap_ ? cap_ - 1 : at_ + size_t(k);
    }

    size_t size() const
    {
        return at_;
    }

private:
    char *b_;
    size_t cap_;
    size_t at_ = 0;
};

inline size_t regs_text(const uint64_t *r, char *out, size_t cap)
{
    Out o(out, cap);
    for (int i = 0; i < kRegs; i++)
        o.put("%s%s=%016llx%s", i % 4 ? " " : "    ", reg_name(i), (unsigned long long)r[i],
              i % 4 == 3 || i == kRegs - 1 ? "\n" : "");
    return o.size();
}

inline size_t kept_text(const Kept &k, char *out, size_t cap)
{
    Out o(out, cap);
    if (k.frame < 0) {
        o.put("  no frame in F1_25.exe\n");
        return o.size();
    }
    o.put("  at game frame #%d (rip=%016llx rsp=%016llx), the non-volatile registers the unwind restored:\n", k.frame,
          (unsigned long long)k.rip, (unsigned long long)k.rsp);
    o.put("    rbx=%016llx rbp=%016llx rsi=%016llx rdi=%016llx\n", (unsigned long long)k.rbx,
          (unsigned long long)k.rbp, (unsigned long long)k.rsi, (unsigned long long)k.rdi);
    o.put("    r12=%016llx r13=%016llx r14=%016llx r15=%016llx\n", (unsigned long long)k.r12,
          (unsigned long long)k.r13, (unsigned long long)k.r14, (unsigned long long)k.r15);
    return o.size();
}

inline size_t stack_text(uint64_t rsp, const uint64_t *q, int n, char *out, size_t cap)
{
    Out o(out, cap);
    if (n <= 0) {
        o.put("  that frame's stack is not readable\n");
        return o.size();
    }
    o.put("  that frame's stack from rsp=%016llx (%d qwords):\n", (unsigned long long)rsp, n);
    for (int i = 0; i < n; i++)
        o.put("%s+%02x=%016llx%s", i % 4 ? " " : "    ", i * 8, (unsigned long long)q[i],
              i % 4 == 3 || i == n - 1 ? "\n" : "");
    return o.size();
}

inline size_t heading(int running, int game, char *out, size_t cap)
{
    Out o(out, cap);
    o.put("\n== Threads not in a wait (%d; %d with a frame in F1_25.exe, listed first, each with its registers; "
          "every thread is in the table below)\n",
          running, game);
    if (!running)
        o.put("  none: every thread read is in a wait\n");
    return o.size();
}

}
