// Host-native unit test for the core's session enter guard (native/src/core/session_enter_logic.hpp).
// Built and run by native/test/run_session_enter_test.sh.

#include "session_enter_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>
#include <vector>

namespace se = runtime::session_enter;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Memory {
    std::map<uint64_t, uint8_t> bytes;
    int reads = 0;

    void put(uint64_t a, const void *p, size_t n)
    {
        for (size_t i = 0; i < n; i++)
            bytes[a + i] = static_cast<const uint8_t *>(p)[i];
    }

    void put64(uint64_t a, uint64_t v)
    {
        put(a, &v, 8);
    }
};

static bool read(uint64_t address, void *out, size_t n, void *user)
{
    Memory *m = static_cast<Memory *>(user);
    m->reads++;
    for (size_t i = 0; i < n; i++) {
        auto it = m->bytes.find(address + i);
        if (it == m->bytes.end())
            return false;
        static_cast<uint8_t *>(out)[i] = it->second;
    }
    return true;
}

static constexpr uint64_t MANAGER = 0x100000;
static constexpr uint64_t GROUPS = 0x110000;
static constexpr uint64_t GROUP = 0x120000;
static constexpr uint64_t HEAD = 0x130000;
static constexpr uint64_t RSP = 0x66e4e7d0;

struct Tree {
    uint64_t next = 0x200000;
    uint64_t build(Memory &m, const std::vector<uint64_t> &keys, size_t lo, size_t hi)
    {
        if (lo >= hi)
            return HEAD;
        size_t mid = (lo + hi) / 2;
        uint64_t n = next;
        next += 0x40;
        m.put64(n + se::NODE_LEFT, build(m, keys, lo, mid));
        m.put64(n + se::NODE_RIGHT, build(m, keys, mid + 1, hi));
        m.put64(n + se::NODE_KEY, keys[mid]);
        uint8_t zero = 0;
        m.put(n + se::NODE_ISNIL, &zero, 1);
        return n;
    }
};

static Memory world(std::vector<uint64_t> keys, uint64_t index = 5, bool groups = true)
{
    Memory m;
    Tree t;
    uint64_t root = t.build(m, keys, 0, keys.size());
    uint64_t leftmost = HEAD;
    if (!keys.empty()) {
        uint64_t n = root;
        for (;;) {
            uint64_t l = 0;
            read(n + se::NODE_LEFT, &l, 8, &m);
            if (l == HEAD)
                break;
            n = l;
        }
        leftmost = n;
    }
    uint8_t one = 1;
    m.put(HEAD + se::NODE_ISNIL, &one, 1);
    m.put64(HEAD + se::NODE_LEFT, leftmost);
    m.put64(HEAD + se::NODE_PARENT, root);
    m.put64(GROUP + se::GROUP_MAP_AT, HEAD);
    m.put64(MANAGER + se::GROUPS_BEGIN_AT, GROUPS);
    m.put64(MANAGER + se::GROUPS_END_AT, groups ? GROUPS + 8 : GROUPS);
    m.put64(GROUPS, GROUP);
    m.put64(RSP + se::INDEX_AT, index);
    return m;
}

int main()
{
    {
        Memory m = world({10, 20, 30, 40, 50});
        se::Fix f = se::decide(MANAGER, RSP, 30, read, &m);
        expect(!f.replace, "a hash group 0 lists runs the game's code");
        f = se::decide(MANAGER, RSP, 10, read, &m);
        expect(!f.replace, "the leftmost key itself is found");
        f = se::decide(MANAGER, RSP, 50, read, &m);
        expect(!f.replace, "the largest key is found");
    }
    {
        Memory m = world({10, 20, 30, 40, 50});
        se::Fix f = se::decide(MANAGER, RSP, 35, read, &m);
        expect(f.replace && f.key == 10, "a hash between keys borrows the leftmost key");
        expect(f.index == 5, "the fix carries the vehicle index");
        f = se::decide(MANAGER, RSP, 99, read, &m);
        expect(f.replace && f.key == 10, "a hash above every key borrows the leftmost key");
        f = se::decide(MANAGER, RSP, 1, read, &m);
        expect(f.replace && f.key == 10, "a hash below every key borrows the leftmost key");
    }
    {
        Memory m = world({77});
        expect(!se::decide(MANAGER, RSP, 77, read, &m).replace, "a one-node map finds its key");
        se::Fix f = se::decide(MANAGER, RSP, 78, read, &m);
        expect(f.replace && f.key == 77, "a one-node map lends its only key");
    }
    {
        Memory m = world({});
        expect(!se::decide(MANAGER, RSP, 5, read, &m).replace, "an empty map is left to the game");
    }
    {
        Memory m = world({10, 20}, 5, false);
        expect(!se::decide(MANAGER, RSP, 5, read, &m).replace, "no asset groups is left to the game");
    }
    {
        Memory m;
        expect(!se::decide(MANAGER, RSP, 5, read, &m).replace, "unreadable memory is left to the game");
        expect(!se::decide(0, RSP, 5, read, &m).replace, "a null manager is left to the game");
    }
    {
        Memory m = world({10, 20, 30});
        m.bytes.erase(RSP + se::INDEX_AT);
        se::Fix f = se::decide(MANAGER, RSP, 25, read, &m);
        expect(f.replace && f.index == se::NO_INDEX, "an unreadable index still fixes and reports no index");
    }
    {
        Memory m = world({10, 20, 30});
        uint64_t root = 0;
        read(HEAD + se::NODE_PARENT, &root, 8, &m);
        m.put64(root + se::NODE_LEFT, root);
        m.put64(root + se::NODE_KEY, 100);
        expect(!se::decide(MANAGER, RSP, 5, read, &m).replace, "a cyclic map is left to the game");
    }
    {
        Memory m = world({10, 20, 30, 40, 50, 60, 70, 80, 90});
        se::Memo memo;
        se::Fix a = memo.decide(MANAGER, RSP, 35, 1000, read, &m);
        expect(a.replace && a.key == 10 && a.index == 5 && memo.misses == 1, "the first lookup walks the map");
        int before = m.reads;
        se::Fix b = memo.decide(MANAGER, RSP, 35, 1100, read, &m);
        expect(b.replace && b.key == 10 && b.index == 5 && memo.hits == 1, "a repeated hash is answered from the memo");
        int per_hit = m.reads - before;
        before = m.reads;
        se::decide(MANAGER, RSP, 35, read, &m);
        expect(per_hit < m.reads - before, "a memo hit reads less than a full walk");
        m.put64(RSP + se::INDEX_AT, 9);
        expect(memo.decide(MANAGER, RSP, 35, 1200, read, &m).index == 9, "a memo hit reports the current vehicle index");
        expect(!memo.decide(MANAGER, RSP, 30, 1200, read, &m).replace && memo.misses == 2, "a found hash is memoised as no fix");
        expect(!memo.decide(MANAGER, RSP, 30, 1300, read, &m).replace && memo.hits == 3, "a found hash hits the memo");
    }
    {
        Memory m = world({10, 20, 30});
        se::Memo memo;
        memo.decide(MANAGER, RSP, 25, 1000, read, &m);
        memo.decide(MANAGER, RSP, 25, 1000 + se::MEMO_TTL_MS, read, &m);
        expect(memo.misses == 2, "the memo expires after its lifetime");
    }
    {
        Memory m = world({10, 20, 30});
        se::Memo memo;
        expect(memo.decide(MANAGER, RSP, 25, 1000, read, &m).key == 10, "the memo fixes before the map changes");
        uint64_t root = 0;
        read(HEAD + se::NODE_PARENT, &root, 8, &m);
        m.put64(root + se::NODE_KEY, 25);
        m.put64(GROUP + se::GROUP_SIZE_AT, 4);
        se::Fix f = memo.decide(MANAGER, RSP, 25, 1100, read, &m);
        expect(memo.misses == 2 && !f.replace, "a changed map drops the memo and finds the new key");
    }
    {
        Memory m = world({10, 20, 30});
        se::Memo memo;
        m.put64(MANAGER + se::GROUPS_BEGIN_AT, 0);
        expect(!memo.decide(MANAGER, RSP, 25, 1000, read, &m).replace && memo.entries.empty(), "unreadable groups are not memoised");
        m.put64(MANAGER + se::GROUPS_BEGIN_AT, GROUPS);
        expect(memo.decide(MANAGER, RSP, 25, 1010, read, &m).replace, "the next lookup retries after a failed read");
    }
    {
        Memory m = world({10, 20, 30});
        se::Memo memo;
        for (uint64_t h = 1; h <= se::MEMO_MAX + 10; h++)
            memo.decide(MANAGER, RSP, h * 100 + 5, 1000, read, &m);
        expect(memo.entries.size() <= se::MEMO_MAX, "the memo is bounded");
    }
    {
        Memory m;
        se::Memo memo;
        expect(!memo.decide(MANAGER, RSP, 5, 0, read, &m).replace && !memo.decide(0, RSP, 5, 0, read, &m).replace,
               "the memo leaves unreadable memory and a null manager to the game");
    }
    {
        std::vector<uint64_t> keys;
        for (uint64_t k = 1; k <= 4000; k++)
            keys.push_back(k * 10);
        Memory m = world(keys);
        se::Memo memo;
        memo.decide(MANAGER, RSP, 15, 1000, read, &m);
        int per = m.reads;
        m.reads = 0;
        memo.decide(MANAGER, RSP, 25, 1001, read, &m);
        expect(per < 80 && m.reads < 80, "a decision over a 4000-entry group map reads a bounded number of words");
    }
    {
        se::Skips k;
        int a = k.note(), b = k.note();
        expect(k.logged(a) && k.logged(b) && !k.last(b), "early fixes are logged");
        for (int i = 2; i < se::LOG_FIRST; i++)
            b = k.note();
        expect(k.last(b) && k.logged(b), "the last logged fix says so");
        expect(!k.logged(k.note()), "later fixes are not logged");
    }
    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
