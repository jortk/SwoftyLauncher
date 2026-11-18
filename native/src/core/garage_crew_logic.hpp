#pragma once

#include <cstddef>
#include <cstdint>
#include <cstring>

namespace runtime::garage_crew {

constexpr uint64_t USER_BASE = 0x10000;
constexpr uint64_t USER_END = 0x800000000000ull;
constexpr size_t ENTRY_SIZE = 0x10;
constexpr size_t ENTRY_SLOT = 0x8;
constexpr uint32_t CREW_SLOTS = 2;
constexpr uint64_t FRAME_MODULE = 0xc0;
constexpr uint64_t MODULE_SESSION = 0x48;
constexpr uint64_t SESSION_TABLES = 0x2c8;
constexpr uint64_t TABLE_STRIDE = 0x10;
constexpr uint32_t TABLE_KINDS = 3;
constexpr uint64_t TABLE_BEGIN = 0x8;
constexpr uint64_t TABLE_END = 0x10;
constexpr uint64_t ROW_SIZE = 0x10;
constexpr uint64_t ROW_TEMPLATE = 0x8;
constexpr uint64_t REC_KIND = 0x58;
constexpr uint64_t REC_TEAM = 0x5a;

using Read = bool (*)(uint64_t address, void *out, size_t n, void *user);

enum class Drop { none, extra_car, no_template };

inline bool user_pointer(uint64_t p)
{
    return p >= USER_BASE && p < USER_END;
}

inline bool has_template(uint64_t session, uint64_t record, Read read, void *user)
{
    uint8_t kind = 0;
    uint16_t team = 0;
    if (!read(record + REC_KIND, &kind, sizeof kind, user) || !read(record + REC_TEAM, &team, sizeof team, user))
        return false;
    if (kind >= TABLE_KINDS)
        return false;
    uint64_t table = 0;
    if (!read(session + SESSION_TABLES + kind * TABLE_STRIDE, &table, sizeof table, user) || !user_pointer(table))
        return false;
    uint64_t begin = 0, end = 0;
    if (!read(table + TABLE_BEGIN, &begin, sizeof begin, user) || !read(table + TABLE_END, &end, sizeof end, user) ||
        end < begin || team >= (end - begin) / ROW_SIZE)
        return false;
    uint64_t tmpl = 0;
    return read(begin + team * ROW_SIZE + ROW_TEMPLATE, &tmpl, sizeof tmpl, user) && user_pointer(tmpl);
}

inline bool session_of(uint64_t rsp, Read read, void *user, uint64_t *session)
{
    uint64_t module = 0;
    return user_pointer(rsp) && read(rsp + FRAME_MODULE, &module, sizeof module, user) && user_pointer(module) &&
           read(module + MODULE_SESSION, session, sizeof *session, user) && user_pointer(*session);
}

constexpr uint32_t GARAGES = 0x20;
constexpr uint64_t SETUP_VECTOR_BEGIN = 0xa8;
constexpr uint64_t SETUP_KEY = 0xc0;
constexpr uint64_t SETUP_FLAG = 0x4b;

inline bool setup_template_missing(uint64_t tmpl)
{
    return !user_pointer(tmpl);
}

inline bool scene_missing(uint64_t scene)
{
    return !user_pointer(scene);
}

struct SetupSkips {
    uint32_t seen = 0;

    bool first(uint64_t garage)
    {
        if (garage >= GARAGES)
            return false;
        uint32_t bit = 1u << garage;
        if (seen & bit)
            return false;
        seen |= bit;
        return true;
    }
};

using OnDrop = void (*)(void *ctx, uint64_t record, uint32_t slot, Drop why);

inline size_t compact(uint8_t *entries, size_t n, uint64_t session, bool known, Read read, void *user, OnDrop on_drop,
                      void *ctx)
{
    size_t kept = 0;
    for (size_t i = 0; i < n; i++) {
        uint8_t *src = entries + i * ENTRY_SIZE;
        uint64_t record = 0;
        uint32_t slot = 0;
        std::memcpy(&record, src, sizeof record);
        std::memcpy(&slot, src + ENTRY_SLOT, sizeof slot);
        Drop why = Drop::none;
        if (known && !has_template(session, record, read, user))
            why = Drop::no_template;
        else if (slot >= CREW_SLOTS || kept >= CREW_SLOTS)
            why = Drop::extra_car;
        if (why != Drop::none) {
            if (on_drop)
                on_drop(ctx, record, slot, why);
            continue;
        }
        if (kept != i)
            std::memmove(entries + kept * ENTRY_SIZE, src, ENTRY_SIZE);
        kept++;
    }
    return kept;
}

}
