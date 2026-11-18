#include "ui_registry_walk.hpp"

#include <cstdio>
#include <map>

using namespace runtime::ui_registry;

namespace {

int failures = 0;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            failures++;                                                                                                \
        }                                                                                                              \
    } while (0)

struct Heap {
    std::map<uint64_t, std::vector<uint8_t>> blocks;
    uint64_t next = 0x200000000ull;
    size_t reads = 0;

    uint64_t alloc(size_t n)
    {
        uint64_t at = next;
        next += (n + 0xff) & ~size_t(0xff);
        next += 0x100;
        blocks[at] = std::vector<uint8_t>(n, 0);
        return at;
    }
    void put(uint64_t at, size_t off, const void *v, size_t n)
    {
        std::memcpy(blocks.at(at).data() + off, v, n);
    }
    void u64(uint64_t at, size_t off, uint64_t v) { put(at, off, &v, 8); }
    void u32(uint64_t at, size_t off, uint32_t v) { put(at, off, &v, 4); }
    bool read(uint64_t at, void *out, size_t n)
    {
        reads++;
        auto it = blocks.upper_bound(at);
        if (it == blocks.begin())
            return false;
        --it;
        if (at < it->first || at + n > it->first + it->second.size())
            return false;
        std::memcpy(out, it->second.data() + (at - it->first), n);
        return true;
    }
    std::map<uint64_t, std::vector<uint8_t>> snapshot() const { return blocks; }
};

const Layout LAY{0x5a70080, 0x5a6ff90, 0x565f200, 0x5660370};

uint64_t string_record(Heap &h, const std::string &text, int16_t refs = 2)
{
    uint64_t r = h.alloc(16 + text.size() + 2);
    h.put(r, 4, &refs, 2);
    uint32_t w = uint32_t(text.size());
    h.put(r, 6, &w, 4);
    h.put(r, 9, text.data(), text.size());
    return r;
}

uint64_t string_leaf(Heap &h, const std::string &text, uint64_t binding_vt = LAY.string_binding_vt, int16_t refs = 2)
{
    uint64_t node = h.alloc(0x68), binding = h.alloc(0x10);
    h.u64(node, 0, LAY.string_leaf_vt);
    h.u64(node, 0x58, binding);
    h.u64(binding, 0, binding_vt);
    h.u64(binding, 8, string_record(h, text, refs));
    return node;
}

uint64_t object(Heap &h, const std::vector<std::pair<std::string, uint64_t>> &kids, size_t slots = 16)
{
    uint64_t node = h.alloc(0xa8), entries = h.alloc(slots * 16 + slots + 1);
    h.u64(node, 0, LAY.object_vt);
    h.u64(node, 0x70, entries);
    h.u64(node, 0x78, entries + slots * 16);
    h.u64(node, 0x88, slots - 1);
    size_t i = 0;
    for (auto &k : kids) {
        h.u32(entries, i * 16, djb2(k.first));
        h.u64(entries, i * 16 + 8, k.second);
        uint8_t flag = 1;
        h.put(entries + 0, slots * 16 + i, &flag, 1);
        i += 2;
    }
    return node;
}

uint64_t list(Heap &h, const std::vector<uint64_t> &kids)
{
    uint64_t node = h.alloc(0x80), arr = h.alloc(kids.size() * 8 + 8);
    h.u64(node, 0, LAY.list_vt);
    h.u64(node, 0x40, arr);
    h.u64(node, 0x48, arr + kids.size() * 8);
    for (size_t i = 0; i < kids.size(); i++)
        h.u64(arr, i * 8, kids[i]);
    return node;
}

uint64_t registry(Heap &h, uint64_t root)
{
    uint64_t reg = h.alloc(0x20);
    h.u64(reg, 8, root);
    return reg;
}

size_t walk(Heap &h, uint64_t reg, const std::string &needle, std::vector<Hit> &hits, Stats &st, const Limits &lim = {})
{
    auto snap = h.snapshot();
    auto rd = [&](uint64_t at, void *out, size_t n) { return h.read(at, out, n); };
    size_t n = find_string(rd, reg, LAY, needle, hits, 16, st, lim);
    CHECK(snap == h.snapshot());
    return n;
}

void finds_the_stock_name_with_its_path()
{
    Heap h;
    uint64_t name = string_leaf(h, "Swooftyy"), other = string_leaf(h, "Williams");
    uint64_t pill = object(h, {{"driver_name", name}, {"team", other}});
    uint64_t hud = object(h, {{"pill_0", pill}});
    uint64_t root = object(h, {{"hud", hud}});
    std::vector<Hit> hits;
    Stats st;
    CHECK(walk(h, registry(h, root), "Swooftyy", hits, st) == 1);
    CHECK(hits.size() == 1 && hits[0].node == name && hits[0].parent == pill && hits[0].depth == 3);
    CHECK(hits[0].chain[0] == djb2("hud") && hits[0].chain[1] == djb2("pill_0") && hits[0].chain[2] == djb2("driver_name"));
    CHECK(st.string_leaves == 2 && st.strings_read == 2 && st.matches == 1 && st.objects == 3);
    CHECK(st.root_vt == LAY.object_vt);
}

void lists_every_string_when_asked()
{
    Heap h;
    uint64_t a = string_leaf(h, "one"), b = string_leaf(h, "two");
    uint64_t root = object(h, {{"a", a}, {"b", b}});
    std::vector<Hit> hits;
    Stats st;
    auto rd = [&](uint64_t at, void *out, size_t n) { return h.read(at, out, n); };
    CHECK(find_string(rd, registry(h, root), LAY, "", hits, 16, st, Limits{}, true) == 2);
    CHECK(hits[0].value == "one" || hits[0].value == "two");
}

void walks_lists_and_reports_unknown_kinds()
{
    Heap h;
    uint64_t a = string_leaf(h, "Swooftyy"), b = string_leaf(h, "Swooftyy");
    uint64_t ints = h.alloc(0x68);
    h.u64(ints, 0, 0x5aaaaaa);
    uint64_t rows = list(h, {a, ints, b});
    uint64_t root = object(h, {{"rows", rows}});
    std::vector<Hit> hits;
    Stats st;
    CHECK(walk(h, registry(h, root), "Swooftyy", hits, st) == 2);
    CHECK(st.lists == 1 && st.skipped_kinds == 1 && st.unknown_vt[0] == 0x5aaaaaa);
}

void rejects_dead_and_foreign_values()
{
    Heap h;
    uint64_t dead = string_leaf(h, "Swooftyy", LAY.string_binding_vt, 0);
    uint64_t foreign = string_leaf(h, "Swooftyy", 0x1234567);
    uint64_t good = string_leaf(h, "Swooftyy");
    uint64_t root = object(h, {{"dead", dead}, {"foreign", foreign}, {"good", good}});
    std::vector<Hit> hits;
    Stats st;
    CHECK(walk(h, registry(h, root), "Swooftyy", hits, st) == 1);
    CHECK(hits[0].node == good);
}

void survives_freed_and_garbage_pointers()
{
    Heap h;
    uint64_t good = string_leaf(h, "Swooftyy");
    uint64_t torn = h.alloc(0x68);
    h.u64(torn, 0, LAY.string_leaf_vt);
    h.u64(torn, 0x58, 0xdeadbeef00ull);
    uint64_t junk_map = h.alloc(0xa8);
    h.u64(junk_map, 0, LAY.object_vt);
    h.u64(junk_map, 0x70, 0x300000001ull);
    h.u64(junk_map, 0x78, 0x7ffffff00000ull);
    uint64_t huge = h.alloc(0xa8);
    h.u64(huge, 0, LAY.object_vt);
    h.u64(huge, 0x70, 0x200000008ull);
    h.u64(huge, 0x78, 0x200000008ull + (uint64_t(1) << 40));
    uint64_t root = object(h, {{"torn", torn}, {"junk", junk_map}, {"huge", huge}, {"gone", 0x400000000ull}, {"low", 0x20}, {"good", good}});
    std::vector<Hit> hits;
    Stats st;
    CHECK(walk(h, registry(h, root), "Swooftyy", hits, st) == 1);
    CHECK(st.bad_maps >= 2 && st.failed_reads >= 1);
}

void stops_on_cycles_and_node_limits()
{
    Heap h;
    uint64_t a = object(h, {}), b = object(h, {{"a", a}});
    h.u64(a, 0x70, h.blocks.at(b).size() ? 0 : 0);
    uint64_t loop = object(h, {{"self", 0}});
    uint64_t entries = 0;
    {
        uint64_t e;
        std::memcpy(&e, h.blocks.at(loop).data() + 0x70, 8);
        entries = e;
    }
    h.u64(entries, 8, loop);
    uint64_t root = object(h, {{"loop", loop}, {"b", b}});
    std::vector<Hit> hits;
    Stats st;
    CHECK(walk(h, registry(h, root), "x", hits, st) == 0);
    CHECK(st.visited <= 5);

    Heap w;
    std::vector<std::pair<std::string, uint64_t>> kids;
    for (int i = 0; i < 6; i++)
        kids.push_back({"k" + std::to_string(i), string_leaf(w, "n" + std::to_string(i))});
    uint64_t r = object(w, kids, 16);
    Limits lim;
    lim.max_nodes = 3;
    Stats st2;
    walk(w, registry(w, r), "n5", hits, st2, lim);
    CHECK(st2.visited == 3);
}

void empty_and_missing_registry()
{
    Heap h;
    uint64_t empty = h.alloc(0xa8);
    h.u64(empty, 0, LAY.object_vt);
    uint64_t inline_at = empty + 0x88;
    h.u64(empty, 0x70, inline_at);
    h.u64(empty, 0x78, inline_at);
    std::vector<Hit> hits;
    Stats st;
    CHECK(walk(h, registry(h, empty), "Swooftyy", hits, st) == 0 && st.objects == 1 && st.bad_maps == 0);
    CHECK(walk(h, h.alloc(0x20), "Swooftyy", hits, st) == 0);
    auto none = [](uint64_t, void *, size_t) { return false; };
    CHECK(find_string(none, 0x200000000ull, LAY, "Swooftyy", hits, 4, st) == 0);
    CHECK(find_string(none, 0, LAY, "", hits, 4, st) == 0);
}

}

int main()
{
    finds_the_stock_name_with_its_path();
    lists_every_string_when_asked();
    walks_lists_and_reports_unknown_kinds();
    rejects_dead_and_foreign_values();
    survives_freed_and_garbage_pointers();
    stops_on_cycles_and_node_limits();
    empty_and_missing_registry();
    if (failures)
        return 1;
    std::puts("ui registry walk: ok");
    return 0;
}
