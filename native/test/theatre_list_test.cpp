// Host-native unit test for the core's theatre list and rows guards (native/src/core/theatre_list_logic.hpp).
// Built and run by native/test/run_theatre_list_test.sh.

#include "theatre_list_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>
#include <vector>

namespace tl = runtime::theatre_list;
namespace gs = runtime::garage_setup;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Memory {
    std::map<uint64_t, uint64_t> words;
    std::map<uint64_t, uint32_t> dwords;
    int reads = 0;
};

static bool read(uint64_t address, void *out, size_t n, void *user)
{
    Memory *m = static_cast<Memory *>(user);
    m->reads++;
    if (n == 8) {
        auto it = m->words.find(address);
        if (it == m->words.end())
            return false;
        memcpy(out, &it->second, 8);
        return true;
    }
    auto it = m->dwords.find(address);
    if (it == m->dwords.end())
        return false;
    memcpy(out, &it->second, 4);
    return true;
}

constexpr uint64_t ROWS_BEGIN = 0x346e9a900;

struct Rows {
    std::vector<uint64_t> slot_id;
    int past_end = 0;
};

static void get_row(Rows &r, uint64_t index, bool guard)
{
    if (r.slot_id.size() <= index)
        r.slot_id.push_back(~0ull);
    uint64_t at = index;
    if (guard)
        at = tl::row_index(index, ROWS_BEGIN, ROWS_BEGIN + r.slot_id.size() * tl::ROW_SIZE);
    if (at >= r.slot_id.size()) {
        r.past_end++;
        return;
    }
    r.slot_id[at] = index;
}

static Rows build(const std::vector<bool> &has_slot, uint64_t pad_to, bool guard, Rows r = {})
{
    uint64_t built = 0;
    for (uint64_t i = 0; i < has_slot.size(); i++) {
        if (!has_slot[i])
            continue;
        get_row(r, i, guard);
        built++;
    }
    for (uint64_t i = built; i < pad_to; i++)
        get_row(r, i, guard);
    return r;
}

static bool rows_are(const Rows &r, std::vector<uint64_t> want)
{
    return r.past_end == 0 && r.slot_id == want;
}

int main()
{
    expect(tl::row_index(0, ROWS_BEGIN, ROWS_BEGIN + tl::ROW_SIZE) == 0, "the index of the only row is kept");
    expect(tl::row_index(1, ROWS_BEGIN, ROWS_BEGIN + tl::ROW_SIZE) == 0,
           "the report's index 1 with one row (r14 = 0x4c8) goes to the row just appended");
    expect(tl::row_index(4, ROWS_BEGIN, ROWS_BEGIN + 3 * tl::ROW_SIZE) == 2, "an index two past the end goes to the last row");
    expect(tl::row_index(2, ROWS_BEGIN, ROWS_BEGIN + 5 * tl::ROW_SIZE) == 2, "an index inside the rows is kept");
    expect(tl::row_index(3, ROWS_BEGIN, ROWS_BEGIN) == 3, "an empty row array is left to the game");
    expect(tl::row_index(3, ROWS_BEGIN, ROWS_BEGIN - tl::ROW_SIZE) == 3, "an end below the begin is left to the game");
    expect(tl::row_index(3, ROWS_BEGIN, ROWS_BEGIN + tl::ROW_SIZE - 8) == 3, "less than one row is left to the game");

    expect(rows_are(build({true, true, true}, 3, false), {0, 1, 2}), "without an orphan the game's own rows are in order");
    expect(rows_are(build({true, true, true}, 3, true), {0, 1, 2}), "without an orphan the guard changes nothing");
    expect(rows_are(build({true}, 3, true), {0, 1, 2}), "a short list is padded as the game pads it");

    expect(build({false, true, true}, 3, false).past_end == 2, "unguarded, the items after an orphan read past the rows");
    expect(rows_are(build({false, true, true}, 3, true), {1, 2, 2}),
           "the report's list: the items after the orphan take rows 0 and 1 and keep their own slot_id");
    expect(rows_are(build({true, false, true, true, false, true}, 5, true), {0, 2, 3, 5, 4}),
           "orphans in the middle of the saved list leave no gap and no row past the end");
    expect(rows_are(build({true, true, false}, 3, true), {0, 1, 2}), "an orphan at the end needs no move");
    expect(rows_are(build({false, false, false}, 3, true), {0, 1, 2}), "a list of only orphans shows the padding rows");

    Rows open_before = build({true, true, true, true}, 3, true);
    expect(rows_are(build({true, false, true, true}, 3, true, open_before), {0, 1, 2, 3}),
           "rows kept from an earlier build are reused in place");

    const uint64_t no_slot = 0x142bfc761, poison = 0xdeadbeefcafef00dull;
    Memory m;
    const uint64_t entry = 0x329bc98d0, node = 0x329e981d0, info = 0x32a000000;
    m.words[entry + gs::NODE_AT] = node;
    m.words[node + gs::INFO_AT] = info;
    m.dwords[info + gs::ID_AT] = 7;

    tl::Exit exit{poison, poison};
    expect(tl::entry_step(entry, no_slot, read, &m, &exit), "an entry that reads down to its id runs the game's code");
    expect(exit.rax == poison && exit.rip == poison, "a usable entry leaves the exit untouched");

    exit = {poison, poison};
    expect(!tl::entry_step(0x2a00002f33000020ull, no_slot, read, &m, &exit), "the non-canonical entry from the reports is skipped");
    expect(exit.rax == 0, "the skip answers with a null slot record");
    expect(exit.rip == no_slot, "the skip continues at the function's epilogue");

    exit = {poison, poison};
    expect(!tl::entry_step(0, no_slot, read, &m, &exit) && exit.rip == no_slot, "a null entry is skipped");
    expect(!tl::entry_step(0xffffffffffffffffull, no_slot, read, &m, &exit), "an all-ones entry is skipped");
    expect(!tl::entry_step(0x9d9ec88, no_slot, read, &m, &exit), "a canonical entry that does not read is skipped");

    Memory counted = m;
    counted.reads = 0;
    tl::entry_step(entry, no_slot, read, &counted, &exit);
    expect(counted.reads == 3, "a usable entry costs exactly three reads, one per link");
    Memory refused = m;
    refused.reads = 0;
    tl::entry_step(0x2a00002f33000020ull, no_slot, read, &refused, &exit);
    expect(refused.reads == 0, "a non-canonical entry is refused without reading");

    Memory badnode = m;
    badnode.words[entry + gs::NODE_AT] = 0x2a00002f33000020ull;
    expect(!tl::entry_step(entry, no_slot, read, &badnode, &exit), "a node that is not a pointer is skipped");

    Memory noinfo = m;
    noinfo.words.erase(node + gs::INFO_AT);
    expect(!tl::entry_step(entry, no_slot, read, &noinfo, &exit), "a node whose info does not read is skipped");

    Memory noid = m;
    noid.dwords.clear();
    expect(!tl::entry_step(entry, no_slot, read, &noid, &exit), "an info whose id does not read is skipped");

    tl::Skips s;
    int lines = 0, last = 0;
    for (int i = 0; i < 500; i++) {
        int n = s.note();
        if (s.logged(n))
            lines++;
        if (s.last(n))
            last++;
    }
    expect(lines == tl::LOG_FIRST, "five hundred skips produce exactly the capped number of lines");
    expect(last == 1, "the capped line is marked once");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
