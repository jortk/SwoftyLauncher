#include <swofty/mem.hpp>
#include <swofty_api.h>

#include <atomic>

namespace swofty::mem::shared {

namespace {

constexpr size_t slots = 65536;
constexpr uint64_t refused_ttl_ms = 32;
constexpr unsigned stamp_bits = 26;
constexpr uint64_t stamp_mask = (uint64_t(1) << stamp_bits) - 1;
constexpr uint64_t ok_bit = uint64_t(1) << stamp_bits;
constexpr unsigned page_shift = stamp_bits + 1;
constexpr uintptr_t lowest = 0x10000, highest = 0x7ffffffeffffull;

std::atomic<uint64_t> table[slots];
std::atomic<uint64_t> resident_answers{0}, region_answers{0};

struct WorkingSetEntry {
    void *address;
    uintptr_t flags;
};

using QueryWorkingSet = BOOL(WINAPI *)(HANDLE, void *, DWORD);

QueryWorkingSet query_working_set()
{
    static const QueryWorkingSet fn = reinterpret_cast<QueryWorkingSet>(
        reinterpret_cast<void *>(GetProcAddress(GetModuleHandleW(L"kernel32.dll"), "K32QueryWorkingSetEx")));
    return fn;
}

bool readable_protection(DWORD p)
{
    return (p & 0xEE) && !(p & (PAGE_GUARD | PAGE_NOACCESS));
}

int resident(uintptr_t page)
{
    QueryWorkingSet fn = query_working_set();
    if (!fn)
        return -1;
    WorkingSetEntry e{reinterpret_cast<void *>(page << 12), 0};
    if (!fn(GetCurrentProcess(), &e, sizeof e) || !(e.flags & 1))
        return -1;
    resident_answers.fetch_add(1, std::memory_order_relaxed);
    return readable_protection(DWORD((e.flags >> 4) & 0x7ff)) ? 1 : 0;
}

bool committed(uintptr_t page)
{
    region_answers.fetch_add(1, std::memory_order_relaxed);
    MEMORY_BASIC_INFORMATION m{};
    if (VirtualQuery(reinterpret_cast<const void *>(page << 12), &m, sizeof m) != sizeof m)
        return false;
    return m.State == MEM_COMMIT && readable_protection(m.Protect);
}

bool page_readable(uintptr_t page, uint64_t now)
{
    std::atomic<uint64_t> &slot = table[(page ^ (page >> 14)) % slots];
    uint64_t v = slot.load(std::memory_order_relaxed);
    if ((v >> page_shift) == page && ((now - v) & stamp_mask) < ((v & ok_bit) ? ttl_ms : refused_ttl_ms))
        return v & ok_bit;
    int r = resident(page);
    bool ok = r >= 0 ? r == 1 : committed(page);
    slot.store(uint64_t(page) << page_shift | (ok ? ok_bit : 0) | (now & stamp_mask), std::memory_order_relaxed);
    return ok;
}

}

bool readable(uintptr_t p, size_t n)
{
    if (!n)
        n = 1;
    if (p < lowest || p + n < p || p + n - 1 > highest)
        return false;
    uint64_t now = GetTickCount64();
    for (uintptr_t page = p >> 12, last = (p + n - 1) >> 12; page <= last; page++)
        if (!page_readable(page, now))
            return false;
    return true;
}

void invalidate()
{
    for (auto &slot : table)
        slot.store(0, std::memory_order_relaxed);
}

Counts counts()
{
    return {resident_answers.load(std::memory_order_relaxed), region_answers.load(std::memory_order_relaxed)};
}

namespace {

int export_readable(const void *p, size_t n)
{
    return readable(reinterpret_cast<uintptr_t>(p), n) ? 1 : 0;
}

const SwoftyMem export_table = {SWOFTY_MEM_VERSION, export_readable, invalidate};

}

}

extern "C" __declspec(dllexport) const SwoftyMem *SwoftyCoreMem(void)
{
    return &swofty::mem::shared::export_table;
}
