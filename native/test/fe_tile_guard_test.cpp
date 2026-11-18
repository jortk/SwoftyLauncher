// Host-native unit test for the core's front-end tile list guard (native/src/core/fe_tile_guard_logic.hpp): which
// entrant-list slot arrays hold a label, which database answers are the miss pointer, how often a label is logged and
// how the env switch parses. Built and run by native/test/run_fe_tile_guard_test.sh.

#include "fe_tile_guard_logic.hpp"

#include <cstdio>
#include <map>

namespace ft = runtime::fe_tile;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Fake {
    std::map<uint64_t, uint64_t> words;
    std::map<uint64_t, std::string> texts;

    bool u64(uint64_t a, uint64_t &out)
    {
        auto it = words.find(a);
        if (it == words.end())
            return false;
        out = it->second;
        return true;
    }

    bool text(uint64_t a, std::string &out)
    {
        auto it = texts.find(a);
        if (it == texts.end())
            return false;
        out = it->second;
        return true;
    }

    void list(uint64_t slot, uint64_t record, const char *name)
    {
        uint64_t box = record + 0x1000, chars = record + 0x2000;
        words[slot + ft::SLOT_RECORD] = record;
        words[record + ft::RECORD_NAME] = box;
        words[box + ft::NAME_CHARS] = chars;
        texts[chars] = name;
    }

    void empty(uint64_t slot)
    {
        words[slot + ft::SLOT_RECORD] = 0;
    }
};

int main()
{
    Fake m;
    const uint64_t base = 0x100000;
    m.list(base + 0x00, 0x200000, "f1_2026");
    m.empty(base + 0x10);
    m.list(base + 0x20, 0x210000, "f3_2026");
    const uint64_t end = base + 0x30;

    expect(ft::list_present(m, base, end, "f1_2026"), "a list at the first slot is present");
    expect(ft::list_present(m, base, end, "f3_2026"), "a list after an empty slot is present");
    expect(!ft::list_present(m, base, end, "f2_2026"), "a list the import removed is not present");
    expect(!ft::list_present(m, base, end, ""), "an empty label is not present");
    expect(!ft::list_present(m, 0, end, "f1_2026"), "a missing slot array holds nothing");
    expect(!ft::list_present(m, end, end, "f1_2026"), "a scan that starts at the array end holds nothing");
    expect(!ft::list_present(m, end + 0x10, end, "f1_2026"), "a scan that starts past the array end holds nothing");

    Fake broken = m;
    broken.words.erase(0x200000 + ft::RECORD_NAME);
    expect(!ft::list_present(broken, base, end, "f3_2026"), "an unreadable record before the label ends the scan as missing");

    Fake huge;
    expect(!ft::list_present(huge, base, base + 0x10 * 100000ull, "f1_2026"), "an unreadable first slot is missing");

    expect(ft::canonical(0), "null is canonical");
    expect(ft::canonical(0x00007ff612345678ull), "a user pointer is canonical");
    expect(ft::canonical(0xffff800000000000ull), "a kernel-half pointer is canonical");
    expect(!ft::canonical(0x08000f000800000eull), "the database miss value is not canonical");
    expect(!ft::canonical(0xffffffffffff0000ull & ~(1ull << 47)), "a pointer with mixed top bits is not canonical");

    ft::Labels labels;
    expect(labels.first("f2_2026"), "the first sighting of a label is logged");
    expect(!labels.first("f2_2026"), "the same label is not logged twice");
    expect(labels.first("f3_2026"), "another label is logged");
    bool capped = false;
    for (int i = 0; i < 100; i++)
        capped |= !labels.first("label" + std::to_string(i)) && labels.last();
    expect(capped && labels.seen.size() == ft::LOG_LABELS, "labels past the cap are not logged");

    expect(ft::switch_off("0"), "0 turns the guard off");
    expect(ft::switch_off("off"), "off turns the guard off");
    expect(!ft::switch_off("1"), "1 leaves the guard on");
    expect(!ft::switch_off("00"), "00 leaves the guard on");
    expect(!ft::switch_off(nullptr), "an unset switch leaves the guard on");

    std::printf("%s\n", failed ? "FAILED" : "all ok");
    return failed ? 1 : 0;
}
