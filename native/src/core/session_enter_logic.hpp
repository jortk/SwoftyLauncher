#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

namespace runtime::session_enter {

constexpr uint64_t GROUPS_BEGIN_AT = 0x108;
constexpr uint64_t GROUPS_END_AT = 0x110;
constexpr uint64_t GROUP_MAP_AT = 0x8;
constexpr uint64_t NODE_LEFT = 0x0;
constexpr uint64_t NODE_PARENT = 0x8;
constexpr uint64_t NODE_RIGHT = 0x10;
constexpr uint64_t NODE_ISNIL = 0x19;
constexpr uint64_t NODE_KEY = 0x20;
constexpr uint64_t INDEX_AT = 0x60;
constexpr uint64_t USER_BASE = 0x10000;
constexpr uint64_t USER_END = 0x800000000000ull;
constexpr uint32_t NO_INDEX = 0xffffffffu;
constexpr int MAX_DEPTH = 96;
constexpr int LOG_FIRST = 4;
constexpr uint64_t GROUP_SIZE_AT = 0x10;
constexpr uint64_t MEMO_TTL_MS = 2000;
constexpr size_t MEMO_MAX = 512;

using Read = bool (*)(uint64_t address, void *out, size_t n, void *user);

struct Fix {
    bool replace = false;
    uint64_t key = 0;
    uint32_t index = NO_INDEX;
    bool decided = false;
};

inline bool user_pointer(uint64_t p)
{
    return p >= USER_BASE && p < USER_END;
}

inline bool word(Read read, void *user, uint64_t address, uint64_t *out)
{
    return read(address, out, sizeof *out, user);
}

inline bool nil(Read read, void *user, uint64_t node, bool *out)
{
    uint8_t b = 0;
    if (!user_pointer(node) || !read(node + NODE_ISNIL, &b, 1, user))
        return false;
    *out = b != 0;
    return true;
}

inline uint32_t slot_index(Read read, void *user, uint64_t rsp)
{
    uint64_t index = 0;
    return word(read, user, rsp + INDEX_AT, &index) ? static_cast<uint32_t>(index & 0xff) : NO_INDEX;
}

inline Fix decide(uint64_t manager, uint64_t rsp, uint64_t hash, Read read, void *user)
{
    Fix f;
    uint64_t begin = 0, end = 0, group = 0, head = 0, node = 0;
    if (!user_pointer(manager) || !word(read, user, manager + GROUPS_BEGIN_AT, &begin) ||
        !word(read, user, manager + GROUPS_END_AT, &end) || begin == end || !word(read, user, begin, &group) ||
        !user_pointer(group) || !word(read, user, group + GROUP_MAP_AT, &head) || !user_pointer(head) ||
        !word(read, user, head + NODE_PARENT, &node))
        return f;
    uint64_t best = head;
    for (int depth = 0;; depth++) {
        bool isnil = false;
        if (depth >= MAX_DEPTH || !nil(read, user, node, &isnil))
            return f;
        if (isnil)
            break;
        uint64_t key = 0, next = 0;
        if (!word(read, user, node + NODE_KEY, &key))
            return f;
        if (key >= hash) {
            best = node;
            if (!word(read, user, node + NODE_LEFT, &next))
                return f;
        } else if (!word(read, user, node + NODE_RIGHT, &next)) {
            return f;
        }
        node = next;
    }
    if (best != head) {
        uint64_t key = 0;
        if (!word(read, user, best + NODE_KEY, &key))
            return f;
        if (key == hash) {
            f.decided = true;
            return f;
        }
    }
    uint64_t first = 0, first_key = 0;
    bool isnil = false;
    if (!word(read, user, head + NODE_LEFT, &first) || first == head || !nil(read, user, first, &isnil) || isnil ||
        !word(read, user, first + NODE_KEY, &first_key))
        return f;
    f.index = slot_index(read, user, rsp);
    f.replace = true;
    f.key = first_key;
    f.decided = true;
    return f;
}

struct Print {
    uint64_t manager = 0, begin = 0, end = 0, group = 0, head = 0, root = 0, size = 0, root_key = 0, leftmost = 0;

    bool operator==(const Print &o) const
    {
        return manager == o.manager && begin == o.begin && end == o.end && group == o.group && head == o.head &&
               root == o.root && size == o.size && root_key == o.root_key && leftmost == o.leftmost;
    }
};

inline bool fingerprint(uint64_t manager, Read read, void *user, Print *out)
{
    Print p;
    p.manager = manager;
    if (!user_pointer(manager) || !word(read, user, manager + GROUPS_BEGIN_AT, &p.begin) ||
        !word(read, user, manager + GROUPS_END_AT, &p.end) || p.begin == p.end || !word(read, user, p.begin, &p.group) ||
        !user_pointer(p.group) || !word(read, user, p.group + GROUP_MAP_AT, &p.head) || !user_pointer(p.head) ||
        !word(read, user, p.head + NODE_PARENT, &p.root) || !word(read, user, p.head + NODE_LEFT, &p.leftmost))
        return false;
    word(read, user, p.group + GROUP_SIZE_AT, &p.size);
    bool isnil = false;
    if (nil(read, user, p.root, &isnil) && !isnil)
        word(read, user, p.root + NODE_KEY, &p.root_key);
    *out = p;
    return true;
}

struct Memo {
    struct Entry {
        uint64_t hash;
        bool replace;
        uint64_t key;
    };

    bool have = false;
    Print print;
    uint64_t stamp = 0;
    std::vector<Entry> entries;
    int hits = 0, misses = 0;

    Fix decide(uint64_t manager, uint64_t rsp, uint64_t hash, uint64_t now_ms, Read read, void *user)
    {
        Print p;
        if (!fingerprint(manager, read, user, &p))
            return Fix{};
        if (!have || !(p == print) || now_ms - stamp >= MEMO_TTL_MS || entries.size() >= MEMO_MAX) {
            have = true;
            print = p;
            stamp = now_ms;
            entries.clear();
        }
        for (const Entry &e : entries)
            if (e.hash == hash) {
                hits++;
                Fix f;
                f.decided = true;
                f.replace = e.replace;
                if (e.replace) {
                    f.key = e.key;
                    f.index = slot_index(read, user, rsp);
                }
                return f;
            }
        misses++;
        Fix f = session_enter::decide(manager, rsp, hash, read, user);
        if (f.decided)
            entries.push_back({hash, f.replace, f.key});
        return f;
    }
};

struct Skips {
    int count = 0;

    int note()
    {
        return ++count;
    }

    bool logged(int n) const
    {
        return n <= LOG_FIRST;
    }

    bool last(int n) const
    {
        return n == LOG_FIRST;
    }
};

}
