#pragma once

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string>
#include <unordered_map>
#include <vector>

// Read-only walk of the game's UI view-model registry that finds string leaves with a given text. It calls no game function:
// every number comes from copying bytes with the caller's `read(address, out, size)`, which must fail instead of faulting on an
// unmapped range (the core's SEH-guarded copy; the host test reads a fake heap).
//
// Layout, from the game's own code (Vm_GetOrCreate_ChildNode 0x1442b2a50, Vm_Set_String 0x140846270, FUN_140846570,
// Game_Vm_HolderGetCStrOrEmpty 0x14429db40, list lookup 0x1442b1130):
//   registry +8        root node pointer
//   every node         +0 vtable, +0x20 u32 name hash (djb2, seed 5381)
//   object node        vtable object_vt, 0xa8 bytes; children in a robin hood map: +0x70 entries (16 bytes: u32 key, child node
//                      pointer at +8), +0x78 info bytes (0 = empty slot), the info bytes start where the entries end
//   list node          vtable list_vt; +0x40 begin and +0x48 end of an array of child node pointers
//   string leaf node   vtable string_leaf_vt, 0x68 bytes; +0x58 binding pointer
//   string binding     0x10 bytes, vtable string_binding_vt (its slot 7 getter is `lea rax,[rcx+8]; ret`), value slot at +8
//   string record      refcounted: i16 refcount at +4, 20-bit length in the low bits of the u32 at +6, characters from +9, NUL end
//
// Ownership: the registry, its nodes, bindings and string records belong to the game (allocated, refcounted and freed by it,
// possibly by another thread while the walk runs). The walk never writes, retains, increments a refcount, calls a virtual
// function, or keeps a pointer after it returns; what it needs it copies into its own buffers (vectors and std::string owned by
// the walk) before looking at it, so a node freed mid-walk gives a failed copy or a rejected value, never a fault. Node
// kinds are told apart by the vtable pointer alone; anything else (reference nodes, int and bool leaves, unknown classes) is
// counted and skipped.

namespace runtime::ui_registry {

struct Layout {
    uint64_t object_vt = 0, list_vt = 0, string_leaf_vt = 0, string_binding_vt = 0;
};

struct Limits {
    size_t max_nodes = 30000;
    int max_depth = 12;
    size_t max_slots = size_t(1) << 16;
    size_t max_list = 4096;
    size_t max_text = 80;
};

constexpr int CHAIN = 14;

struct Hit {
    uint64_t node = 0, parent = 0;
    int depth = 0;
    uint32_t chain[CHAIN] = {};
    std::string value;
};

struct Stats {
    size_t visited = 0, objects = 0, lists = 0, string_leaves = 0, strings_read = 0, matches = 0, skipped_kinds = 0, failed_reads = 0,
           bad_maps = 0;
    uint64_t root_vt = 0;
    uint64_t unknown_vt[4] = {};
};

template <class Read>
bool get(Read &read, uint64_t at, void *out, size_t n)
{
    if (at < 0x10000 || at >= 0x800000000000ull || at + n < at)
        return false;
    return read(at, out, n);
}

template <class Read>
bool u64_at(Read &read, uint64_t at, uint64_t &v)
{
    return get(read, at, &v, 8);
}

template <class Read>
bool string_record(Read &read, uint64_t rec, size_t max_text, std::string &out)
{
    uint8_t head[16];
    if (!get(read, rec, head, sizeof head))
        return false;
    int16_t refs;
    uint32_t word;
    std::memcpy(&refs, head + 4, 2);
    std::memcpy(&word, head + 6, 4);
    size_t len = word & 0xfffff;
    if (refs <= 0 || len == 0 || len > max_text)
        return false;
    std::vector<uint8_t> text(9 + len + 1);
    if (!get(read, rec, text.data(), text.size()) || text[9 + len] != 0)
        return false;
    for (size_t i = 0; i < len; i++)
        if (text[9 + i] < 0x20)
            return false;
    out.assign(reinterpret_cast<const char *>(text.data() + 9), len);
    return true;
}

template <class Read>
size_t find_string(Read &read, uint64_t registry, const Layout &lay, const std::string &needle, std::vector<Hit> &hits, size_t cap,
                   Stats &st, const Limits &lim = {}, bool every_string = false)
{
    st = Stats{};
    uint64_t root = 0;
    if ((needle.empty() && !every_string) || !u64_at(read, registry + 8, root) || !root)
        return 0;
    struct Item {
        uint64_t node, parent;
        int depth;
        uint32_t chain[CHAIN];
    };
    std::vector<Item> stack;
    std::unordered_map<uint64_t, bool> seen;
    Item first{root, 0, 0, {}};
    stack.push_back(first);
    auto push = [&](const Item &from, uint32_t key, uint64_t child) {
        if (!child || from.depth + 1 > lim.max_depth)
            return;
        Item next = from;
        next.node = child;
        next.parent = from.node;
        next.depth = from.depth + 1;
        next.chain[from.depth < CHAIN ? from.depth : CHAIN - 1] = key;
        stack.push_back(next);
    };
    while (!stack.empty() && st.visited < lim.max_nodes) {
        Item it = stack.back();
        stack.pop_back();
        if (!seen.emplace(it.node, true).second)
            continue;
        uint64_t vt = 0;
        if (!u64_at(read, it.node, vt)) {
            st.failed_reads++;
            continue;
        }
        st.visited++;
        if (it.depth == 0)
            st.root_vt = vt;
        if (vt == lay.string_leaf_vt) {
            st.string_leaves++;
            uint64_t binding = 0, bvt = 0, rec = 0;
            std::string text;
            if (!u64_at(read, it.node + 0x58, binding) || !u64_at(read, binding, bvt) || bvt != lay.string_binding_vt ||
                !u64_at(read, binding + 8, rec) || !string_record(read, rec, lim.max_text, text))
                continue;
            st.strings_read++;
            if (!every_string && text != needle)
                continue;
            st.matches++;
            if (hits.size() < cap) {
                Hit h;
                h.node = it.node;
                h.parent = it.parent;
                h.depth = it.depth;
                std::memcpy(h.chain, it.chain, sizeof h.chain);
                h.value = text;
                hits.push_back(h);
            }
        } else if (vt == lay.object_vt) {
            st.objects++;
            uint64_t entries = 0, info = 0, mask = 0;
            if (!u64_at(read, it.node + 0x70, entries) || !u64_at(read, it.node + 0x78, info) ||
                !u64_at(read, it.node + 0x88, mask)) {
                st.failed_reads++;
                continue;
            }
            if (entries == info)
                continue;
            if (!entries || !info || info < entries || (entries & 7) || (info - entries) % 16) {
                st.bad_maps++;
                continue;
            }
            size_t slots = size_t((info - entries) / 16);
            if (slots == 0 || slots > lim.max_slots || mask >= slots) {
                st.bad_maps++;
                continue;
            }
            std::vector<uint8_t> flags(slots), table(slots * 16);
            if (!get(read, info, flags.data(), slots) || !get(read, entries, table.data(), table.size())) {
                st.failed_reads++;
                continue;
            }
            for (size_t i = 0; i < slots; i++) {
                if (!flags[i])
                    continue;
                uint32_t key;
                uint64_t child;
                std::memcpy(&key, table.data() + i * 16, 4);
                std::memcpy(&child, table.data() + i * 16 + 8, 8);
                push(it, key, child);
            }
        } else if (vt == lay.list_vt) {
            st.lists++;
            uint64_t begin = 0, end = 0;
            if (!u64_at(read, it.node + 0x40, begin) || !u64_at(read, it.node + 0x48, end)) {
                st.failed_reads++;
                continue;
            }
            if (!begin || end < begin || (begin & 7) || (end - begin) / 8 > lim.max_list) {
                if (begin || end)
                    st.bad_maps++;
                continue;
            }
            size_t count = size_t((end - begin) / 8);
            std::vector<uint64_t> kids(count);
            if (count && !get(read, begin, kids.data(), count * 8)) {
                st.failed_reads++;
                continue;
            }
            for (size_t i = 0; i < count; i++) {
                uint32_t key = 0;
                if (kids[i] && !get(read, kids[i] + 0x20, &key, 4))
                    key = uint32_t(i);
                push(it, key, kids[i]);
            }
        } else {
            st.skipped_kinds++;
            for (auto &u : st.unknown_vt)
                if (u == vt)
                    goto known;
            for (auto &u : st.unknown_vt)
                if (!u) {
                    u = vt;
                    break;
                }
        known:;
        }
    }
    return hits.size();
}

inline uint32_t djb2(const std::string &s)
{
    uint32_t h = 5381;
    for (unsigned char c : s)
        h = h * 33 + c;
    return h;
}

}
