// The swofty_core.dll stand-ins that mem_addon_test.exe loads (see run_mem_pages_test.sh). Built with
// SWOFTY_MEM_SHARED and src/core/mem_pages.cpp, it is the core's own SwoftyCoreMem export plus mem_test_answers,
// the count of page answers its table looked up. Built with MEM_ADDON_FAKE_CORE, it is a core whose SwoftyCoreMem
// gives a table an addon must not use (version 0 when MEM_TEST_FAKE is "v0", null functions when it is "nullfn")
// and whose functions count their calls in mem_fake_calls and answer readable for every address.
#include <swofty_api.h>

#include <atomic>
#include <cstdint>
#include <cstring>

#ifdef MEM_ADDON_FAKE_CORE

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

static std::atomic<uint64_t> calls{0};

static int fake_readable(const void *, size_t)
{
    calls.fetch_add(1);
    return 1;
}

static void fake_invalidate()
{
    calls.fetch_add(1);
}

static const SwoftyMem old_table = {0, fake_readable, fake_invalidate};
static const SwoftyMem empty_table = {SWOFTY_MEM_VERSION, nullptr, nullptr};

extern "C" __declspec(dllexport) const SwoftyMem *SwoftyCoreMem(void)
{
    char v[16] = {};
    GetEnvironmentVariableA("MEM_TEST_FAKE", v, sizeof v);
    return std::strcmp(v, "nullfn") ? &old_table : &empty_table;
}

extern "C" __declspec(dllexport) uint64_t mem_fake_calls(void)
{
    return calls.load();
}

#else

#include <swofty/mem.hpp>

extern "C" __declspec(dllexport) uint64_t mem_test_answers(void)
{
    swofty::mem::shared::Counts c = swofty::mem::shared::counts();
    return c.resident + c.region;
}

#endif
