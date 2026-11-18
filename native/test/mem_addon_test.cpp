// Test for swofty::mem in an addon (no SWOFTY_MEM_SHARED): readable() and invalidate() use the core's table
// through the SwoftyCoreMem export of swofty_core.dll when it is loaded, and the header's own per-thread caches
// when it is not. run_mem_pages_test.sh runs it once per mode:
//   core    loads core/swofty_core.dll (mem_addon_core.cpp with src/core/mem_pages.cpp): every new page is
//           answered by the core's table, a cached page is not looked up again, and invalidate() reaches it.
//   absent  loads nothing: the header's own checks answer.
//   v0      loads fake/swofty_core.dll with a version 0 table, nullfn with a version 1 table of null functions:
//           the header's own checks answer and the fake's functions are never called.
#include <swofty/mem.hpp>

#include <atomic>
#include <cstdio>
#include <cstring>
#include <random>
#include <string>
#include <thread>
#include <vector>

static int failures;

static void expect(bool ok, const char *what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    fflush(stdout);
    failures += !ok;
}

static HMODULE load(const wchar_t *dir)
{
    wchar_t self[MAX_PATH] = {};
    GetModuleFileNameW(nullptr, self, MAX_PATH);
    std::wstring path(self);
    path.resize(path.find_last_of(L"\\/") + 1);
    return LoadLibraryW((path + dir + L"\\swofty_core.dll").c_str());
}

template <class F> static F export_of(HMODULE dll, const char *name)
{
    return reinterpret_cast<F>(reinterpret_cast<void *>(GetProcAddress(dll, name)));
}

static void common()
{
    namespace mem = swofty::mem;
    static const uint64_t value = 0x1122334455667788ull;
    uint64_t got = 0;
    expect(mem::readable(&value, 8) && mem::read(uintptr_t(&value), got) && got == value, "a static is readable and reads back");
    expect(!mem::readable(uintptr_t(0), 8), "null is refused");
    expect(!mem::readable(uintptr_t(&value), size_t(-1)), "a size that wraps is refused");

    auto *reserved = static_cast<char *>(VirtualAlloc(nullptr, 0x10000, MEM_RESERVE, PAGE_NOACCESS));
    expect(!mem::readable(reserved, 8) && !mem::readable(reserved + 0x8000, 8), "reserved pages are refused");

    auto *guard = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE | PAGE_GUARD));
    MEMORY_BASIC_INFORMATION m{};
    expect(!mem::readable(guard, 8) && VirtualQuery(guard, &m, sizeof m) == sizeof m && (m.Protect & PAGE_GUARD),
           "a guard page is refused and stays armed");

    auto *name = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    strcpy(name, "f1_career_progressive");
    std::string s;
    expect(mem::c_string(name, s, 64) && s == "f1_career_progressive", "c_string reads a name");

    auto *pool = static_cast<char *>(VirtualAlloc(nullptr, 16 << 20, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    memset(pool, 2, 16 << 20);
    std::atomic<unsigned> wrong{0};
    std::vector<std::thread> threads;
    for (int t = 0; t < 8; t++)
        threads.emplace_back([&, t] {
            std::mt19937_64 rng(t);
            for (int i = 0; i < 20000; i++) {
                bool bad = (i & 7) == 0;
                bool ok = bad ? mem::readable(uintptr_t(reserved) + rng() % 0xfff8, 8)
                              : mem::readable(uintptr_t(pool) + rng() % ((16 << 20) - 64), 64);
                wrong += bad ? ok : !ok;
            }
        });
    for (auto &th : threads)
        th.join();
    expect(!wrong.load(), "8 threads get the right answer for 160,000 mixed checks");

    auto *heap = static_cast<char *>(VirtualAlloc(nullptr, 0x10000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    memset(heap, 1, 0x10000);
    expect(mem::readable(heap, 0x10000), "a touched committed region is readable");
    VirtualFree(heap, 0, MEM_RELEASE);
    mem::invalidate();
    expect(!mem::readable(heap, 8), "a freed region is refused after invalidate()");
}

int main(int argc, char **argv)
{
    namespace mem = swofty::mem;
    const char *mode = argc > 1 ? argv[1] : "";
    if (!strcmp(mode, "core")) {
        HMODULE core = load(L"core");
        expect(core != nullptr, "core/swofty_core.dll loads");
        if (!core)
            return 1;
        auto answers = export_of<uint64_t (*)()>(core, "mem_test_answers");
        auto get = export_of<SwoftyCoreMemFn>(core, "SwoftyCoreMem");
        expect(answers && get && get() && get()->version == SWOFTY_MEM_VERSION, "the core exports SwoftyCoreMem, version 1");
        if (!answers)
            return 1;
        auto *page = static_cast<char *>(VirtualAlloc(nullptr, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
        page[0] = 1;
        uint64_t a0 = answers();
        bool first = mem::readable(page, 8);
        uint64_t a1 = answers();
        bool again = mem::readable(page, 8);
        uint64_t a2 = answers();
        mem::invalidate();
        bool after = mem::readable(page, 8);
        uint64_t a3 = answers();
        expect(first && again && after, "a committed page is readable through the core's table");
        expect(a1 == a0 + 1, "the first check of a page is one answer of the core's table");
        expect(a2 == a1, "a second check of the page is answered from the core's table, not looked up again");
        expect(a3 == a2 + 1, "invalidate() drops the core's answer, so the page is looked up again");
        uint64_t before = answers();
        common();
        expect(answers() > before, "the checks from 8 threads were answered by the core's table");
    } else if (!strcmp(mode, "absent")) {
        expect(!GetModuleHandleW(L"swofty_core.dll"), "no swofty_core.dll is loaded");
        common();
    } else if (!strcmp(mode, "v0") || !strcmp(mode, "nullfn")) {
        SetEnvironmentVariableA("MEM_TEST_FAKE", mode);
        HMODULE fake = load(L"fake");
        expect(fake != nullptr, "fake/swofty_core.dll loads");
        if (!fake)
            return 1;
        auto calls = export_of<uint64_t (*)()>(fake, "mem_fake_calls");
        common();
        expect(calls && calls() == 0, !strcmp(mode, "v0") ? "a version 0 table is never called"
                                                          : "a table with null functions is never called");
    } else {
        printf("usage: mem_addon_test core|absent|v0|nullfn\n");
        return 2;
    }
    printf("mem addon test (%s): %s\n", mode, failures ? "FAILED" : "passed");
    return failures ? 1 : 0;
}
