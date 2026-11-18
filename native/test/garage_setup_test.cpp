// Host-native unit test for the core's garage setup guard (native/src/core/garage_setup_logic.hpp).
// Built and run by native/test/run_garage_setup_test.sh.

#include "garage_setup_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>

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

int main()
{
    Memory m;
    const uint64_t entry = 0x329bc98d0, node = 0x329e981d0, info = 0x32a000000;
    m.words[entry + gs::NODE_AT] = node;
    m.words[node + gs::INFO_AT] = info;
    m.dwords[info + gs::ID_AT] = 7;

    expect(gs::entry_usable(entry, read, &m), "an entry that reads down to its id runs the game's code");
    expect(!gs::entry_usable(0x2a00002f33000020ull, read, &m), "the non-canonical entry from the reports is skipped");
    expect(!gs::entry_usable(0, read, &m), "a null entry is skipped");
    expect(!gs::entry_usable(0xffffffffffffffffull, read, &m), "an all-ones entry is skipped");
    expect(!gs::entry_usable(0x9d9ec88, read, &m), "a canonical entry that does not read is skipped");

    Memory counted = m;
    counted.reads = 0;
    gs::entry_usable(entry, read, &counted);
    expect(counted.reads == 3, "a usable entry costs exactly three reads, one per link");
    Memory refused = m;
    refused.reads = 0;
    gs::entry_usable(0x2a00002f33000020ull, read, &refused);
    expect(refused.reads == 0, "a non-canonical entry is refused without reading");

    Memory badnode = m;
    badnode.words[entry + gs::NODE_AT] = 0x2a00002f33000020ull;
    expect(!gs::entry_usable(entry, read, &badnode), "a node that is not a pointer is skipped");

    Memory noinfo = m;
    noinfo.words.erase(node + gs::INFO_AT);
    expect(!gs::entry_usable(entry, read, &noinfo), "a node whose info does not read is skipped");

    Memory noid = m;
    noid.dwords.clear();
    expect(!gs::entry_usable(entry, read, &noid), "an info whose id does not read is skipped");

    gs::Skips s;
    int lines = 0, last = 0;
    for (int i = 0; i < 500; i++) {
        int n = s.note();
        if (s.logged(n))
            lines++;
        if (s.last(n))
            last++;
    }
    expect(lines == gs::LOG_FIRST, "five hundred skips produce exactly the capped number of lines");
    expect(last == 1, "the capped line is marked once");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
