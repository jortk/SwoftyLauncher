// Test for the core's shared page table behind swofty::mem::readable (src/core/mem_pages.cpp, built with
// SWOFTY_MEM_SHARED as native/build.sh builds the core). Runs natively on Windows and under Wine: answers for good,
// freed, reserved, no-access, guard and never-touched pages; no exception is ever raised and guard pages stay
// armed; 8 threads agree; and the cost of a cached check, a working set miss and a VirtualQuery on a large
// committed region.
#include <swofty/mem.hpp>

#include <atomic>
#include <cstdio>
#include <cstring>
#include <random>
#include <thread>
#include <vector>

static int failures;
static std::atomic<unsigned> exceptions{0};

static void expect(bool ok, const char *what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    fflush(stdout);
    failures += !ok;
}

static LONG CALLBACK count_exceptions(EXCEPTION_POINTERS *)
{
    exceptions.fetch_add(1);
    return EXCEPTION_CONTINUE_SEARCH;
}

static double now_us()
{
    LARGE_INTEGER f, c;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return double(c.QuadPart) * 1e6 / double(f.QuadPart);
}

static bool guarded(const void *p)
{
    MEMORY_BASIC_INFORMATION m{};
    return VirtualQuery(p, &m, sizeof m) == sizeof m && (m.Protect & PAGE_GUARD);
}

int main()
{
    namespace mem = swofty::mem;
    void *veh = AddVectoredExceptionHandler(1, count_exceptions);

    static const uint64_t value = 0x1122334455667788ull;
    uint64_t got = 0;
    expect(mem::readable(&value, 8) && mem::read(uintptr_t(&value), got) && got == value, "a static is readable and reads back");
    expect(!mem::readable(uintptr_t(0), 8) && !mem::readable(uintptr_t(0x1000), 8), "null and the first 64 KB are refused");
    expect(!mem::readable(uintptr_t(0xffff800000000000ull), 8) && !mem::readable(uintptr_t(0x7ffffffff000ull), 8),
           "kernel and top-of-user-space addresses are refused");
    expect(!mem::readable(uintptr_t(&value), size_t(-1)), "a size that wraps is refused");

    auto *heap = static_cast<char *>(VirtualAlloc(nullptr, 0x10000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    memset(heap, 1, 0x10000);
    expect(mem::readable(heap, 0x10000), "a touched committed region is readable");
    VirtualFree(heap, 0, MEM_RELEASE);
    mem::invalidate();
    expect(!mem::readable(heap, 8), "a freed region is refused after invalidate()");

    auto *later = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    later[0] = 1;
    Sleep(50);
    expect(mem::readable(later, 8), "memory allocated where a refused page was is readable 50 ms later");
    VirtualFree(later, 0, MEM_RELEASE);
    Sleep(mem::ttl_ms + 100);
    expect(!mem::readable(later, 8), "a freed region is refused once the answer expires");

    auto *fresh = static_cast<char *>(VirtualAlloc(nullptr, 0x4000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    expect(mem::readable(fresh + 0x2000, 8), "a committed page never touched (not resident) is readable");

    auto *span = static_cast<char *>(VirtualAlloc(nullptr, 0x3000, MEM_RESERVE, PAGE_NOACCESS));
    VirtualAlloc(span, 0x1000, MEM_COMMIT, PAGE_READWRITE);
    span[0] = 1;
    expect(mem::readable(span, 0x1000) && !mem::readable(span + 0xff8, 16) && !mem::readable(span + 0x1000, 8),
           "a read that runs from a committed page into a reserved one is refused");

    auto *noaccess = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    noaccess[0] = 1;
    DWORD was = 0;
    VirtualProtect(noaccess, 0x1000, PAGE_NOACCESS, &was);
    expect(!mem::readable(noaccess, 8), "a no-access page is refused");

    auto *guard_cold = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE | PAGE_GUARD));
    auto *guard_warm = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    guard_warm[0] = 1;
    VirtualProtect(guard_warm, 0x1000, PAGE_READWRITE | PAGE_GUARD, &was);
    expect(!mem::readable(guard_cold, 8) && !mem::readable(guard_warm, 8), "guard pages are refused, resident or not");
    expect(guarded(guard_cold) && guarded(guard_warm), "both guard pages are still armed");

    auto *pool = static_cast<char *>(VirtualAlloc(nullptr, 64 << 20, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    memset(pool, 2, 64 << 20);
    std::atomic<unsigned> wrong{0};
    std::vector<std::thread> threads;
    for (int t = 0; t < 8; t++)
        threads.emplace_back([&, t] {
            std::mt19937_64 rng(t);
            for (int i = 0; i < 200000; i++) {
                uintptr_t at = uintptr_t(pool) + rng() % ((64 << 20) - 64);
                bool bad = (i & 7) == 0;
                bool ok = bad ? mem::readable(uintptr_t(span) + 0x1000 + rng() % 0x2000, 8) : mem::readable(at, 64);
                wrong += bad ? ok : !ok;
            }
        });
    for (auto &th : threads)
        th.join();
    expect(!wrong.load(), "8 threads get the right answer for 1.6 million mixed checks");
    expect(!exceptions.load(), "no exception was raised by any check");
    RemoveVectoredExceptionHandler(veh);

    const size_t big = size_t(2) << 30;
    auto *heap2 = static_cast<char *>(VirtualAlloc(nullptr, big, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    if (!heap2) {
        printf("      2 GB commit refused; timing skipped\n");
    } else {
        for (size_t o = 0; o < big; o += 0x1000)
            heap2[o] = 1;
        std::mt19937_64 rng(7);
        std::vector<uintptr_t> at(20000);
        for (auto &a : at)
            a = uintptr_t(heap2) + (rng() % (big / 0x1000)) * 0x1000 + 0x80;
        mem::invalidate();
        auto before = mem::shared::counts();
        double t0 = now_us();
        unsigned ok = 0;
        for (auto a : at)
            ok += mem::readable(a, 8);
        double miss = (now_us() - t0) / at.size();
        auto after = mem::shared::counts();
        t0 = now_us();
        for (int r = 0; r < 10; r++)
            for (auto a : at)
                ok += mem::readable(a, 8);
        double hit = (now_us() - t0) / (at.size() * 10);
        t0 = now_us();
        MEMORY_BASIC_INFORMATION m{};
        for (size_t i = 0; i < 2000; i++)
            ok += VirtualQuery(reinterpret_cast<void *>(at[i]), &m, sizeof m) == sizeof m;
        double vq = (now_us() - t0) / 2000;
        printf("      2 GB committed heap: cached check %.3f us, first check of a page %.2f us (%llu working set, %llu "
               "VirtualQuery answers), VirtualQuery %.1f us\n",
               hit, miss, (unsigned long long)(after.resident - before.resident),
               (unsigned long long)(after.region - before.region), vq);
        expect(ok == at.size() * 11 + 2000, "every page of the 2 GB heap is readable");
    }
    printf("mem pages test: %s\n", failures ? "FAILED" : "passed");
    return failures ? 1 : 0;
}
