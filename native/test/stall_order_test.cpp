// Host-native unit test for the load-stall report's running-threads section (native/src/core/stall_order_logic.hpp):
// which threads it lists first and in what order, which top frames count as a wait, and the register, unwound-frame
// and frame-stack text, replayed from the 1.9.39 hang entering Monaco's race (stoykebn__, case
// 1570788625938124871), where job thread 17860 spun under RtlCaptureStackBackTrace in 0x1445acb00 while the report
// listed it among 219 threads. Built and run by native/test/run_stall_order_test.sh.

#include "stall_order_logic.hpp"

#include <cstdio>
#include <cstring>

namespace so = runtime::stall_order;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

static void waits()
{
    CHECK(so::wait_export("ZwWaitForMultipleObjects"));
    CHECK(so::wait_export("ZwWaitForAlertByThreadId"));
    CHECK(so::wait_export("NtWaitForSingleObject"));
    CHECK(so::wait_export("ZwRemoveIoCompletion"));
    CHECK(so::wait_export("NtRemoveIoCompletionEx"));
    CHECK(so::wait_export("GetQueuedCompletionStatus"));
    CHECK(so::wait_export("RtlSleepConditionVariableSRW"));
    CHECK(so::wait_export("SleepEx"));
    CHECK(!so::wait_export("RtlCaptureStackBackTrace"));
    CHECK(!so::wait_export("RtlWalkFrameChain"));
    CHECK(!so::wait_export("BaseThreadInitThunk"));
    CHECK(!so::wait_export(""));
    CHECK(!so::wait_export(nullptr));
}

static void order()
{
    so::Thread t[8];
    t[0] = {true, true, true};
    t[1] = {true, false, false};
    t[2] = {true, false, true};
    t[3] = {false, false, true};
    t[4] = {true, true, false};
    t[5] = {true, false, true};
    t[6] = {false, false, false};
    t[7] = {true, false, false};
    int pick[8] = {};
    int k = so::running_first(t, 8, pick);
    CHECK(k == 4);
    CHECK(pick[0] == 2 && pick[1] == 5);
    CHECK(pick[2] == 1 && pick[3] == 7);

    so::Thread monaco[219];
    for (so::Thread &x : monaco)
        x = {true, true, true};
    monaco[0] = {false, false, false};
    monaco[57] = {true, false, true};
    k = so::running_first(monaco, 219, pick);
    CHECK(k == 1);
    CHECK(pick[0] == 57);

    k = so::running_first(t, 0, pick);
    CHECK(k == 0);
}

static void text()
{
    uint64_t r[so::kRegs] = {};
    for (int i = 0; i < so::kRegs; i++)
        r[i] = 0x1000 + i;
    r[so::Rip] = 0x7ff8bb87007a;
    r[so::Rsp] = 0x6923ef40;
    char b[2048];
    size_t n = so::regs_text(r, b, sizeof b);
    CHECK(n == std::strlen(b));
    CHECK(std::strstr(b, "    rax=0000000000001000 rbx=0000000000001001 rcx=0000000000001002 rdx=0000000000001003\n"));
    CHECK(std::strstr(b, "    rsi=0000000000001004 rdi=0000000000001005 rbp=0000000000001006 rsp=000000006923ef40\n"));
    CHECK(std::strstr(b, "    r12=000000000000100c r13=000000000000100d r14=000000000000100e r15=000000000000100f\n"));
    CHECK(std::strstr(b, "    rip=00007ff8bb87007a efl=0000000000001011\n"));
    int lines = 0;
    for (const char *p = b; *p; p++)
        lines += *p == '\n';
    CHECK(lines == 5);

    so::Kept k;
    so::kept_text(k, b, sizeof b);
    CHECK(!std::strcmp(b, "  no frame in F1_25.exe\n"));
    k.frame = 5;
    k.rip = 0x1445acd11;
    k.rsp = 0x6923f760;
    k.rdi = 0x7;
    k.rsi = 0x2a1b0c8;
    k.r12 = 0x2a1b178;
    so::kept_text(k, b, sizeof b);
    CHECK(std::strstr(b, "  at game frame #5 (rip=00000001445acd11 rsp=000000006923f760), the non-volatile registers"));
    CHECK(std::strstr(b, "rsi=0000000002a1b0c8 rdi=0000000000000007\n"));
    CHECK(std::strstr(b, "    r12=0000000002a1b178 "));

    uint64_t q[so::kFrameQwords] = {};
    q[6] = 0x3c0de0000;
    so::stack_text(0x6923f760, q, 7, b, sizeof b);
    CHECK(std::strstr(b, "  that frame's stack from rsp=000000006923f760 (7 qwords):\n"));
    CHECK(std::strstr(b, "    +00=0000000000000000 +08=0000000000000000 +10=0000000000000000 +18=0000000000000000\n"));
    CHECK(std::strstr(b, "    +20=0000000000000000 +28=0000000000000000 +30=00000003c0de0000\n"));
    so::stack_text(0x6923f760, q, 0, b, sizeof b);
    CHECK(!std::strcmp(b, "  that frame's stack is not readable\n"));

    so::heading(1, 1, b, sizeof b);
    CHECK(std::strstr(b, "\n== Threads not in a wait (1; 1 with a frame in F1_25.exe, listed first"));
    CHECK(!std::strstr(b, "none"));
    so::heading(0, 0, b, sizeof b);
    CHECK(std::strstr(b, "  none: every thread read is in a wait\n"));

    char small[24];
    n = so::regs_text(r, small, sizeof small);
    CHECK(n == sizeof small - 1);
    CHECK(std::strlen(small) == sizeof small - 1);
}

int main()
{
    waits();
    order();
    text();
    if (g_fail) {
        std::printf("stall order test: %d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("stall order test: ok\n");
    return 0;
}
