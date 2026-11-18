#pragma once

#include "myteam_vehicle_heal_logic.hpp"

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

namespace runtime::myteam_keep {

namespace mh = runtime::myteam_heal;

constexpr const char *VEHICLES_TABLE = "entrants.vehicles";
constexpr const char *VEHICLES_ROW = "vehicle";
constexpr uint64_t ROW_NAME = 0x100;
constexpr uint64_t ROW_COUNT = 0x200;
constexpr uint64_t ROW_FOUND = 0x204;
constexpr size_t ROW_SIZE = 0x208;
constexpr size_t ROW_TEXT = 0x100;
constexpr uint64_t VEHICLE_FILE_NAME = mh::VEHICLE_FILE_NAME;
constexpr uint64_t VEHICLE_ENGINE = mh::VEHICLE_ENGINE;
constexpr uint64_t VEHICLE_BASE = mh::VEHICLE_BASE;
constexpr size_t SAVED_BYTES = 0xcf6;
constexpr size_t MAX_SHADOWS = 64;
constexpr size_t LOG_IDS = 32;

inline uint32_t handle(uint32_t stat_id)
{
    return (stat_id & 0x3fff) << 16 | mh::KIND_CUSTOM;
}

inline uint32_t base_of(const uint8_t *record)
{
    uint32_t v = 0;
    std::memcpy(&v, record + VEHICLE_BASE, sizeof v);
    return v;
}

inline std::string text_at(const uint8_t *p, size_t max)
{
    size_t n = 0;
    while (n < max && p[n])
        n++;
    return std::string(reinterpret_cast<const char *>(p), n);
}

inline bool build_row(uint8_t *row, const std::string &container, const std::string &name, uint64_t index)
{
    std::string row_name = name + "_" + std::to_string(index);
    if (container.empty() || name.empty() || container.size() >= ROW_TEXT || row_name.size() >= ROW_TEXT)
        return false;
    std::memset(row, 0, ROW_SIZE);
    std::memcpy(row, container.data(), container.size());
    std::memcpy(row + ROW_NAME, row_name.data(), row_name.size());
    int32_t count = -1;
    std::memcpy(row + ROW_COUNT, &count, sizeof count);
    row[ROW_FOUND] = 1;
    return true;
}

struct Stock {
    std::string label;
    uint32_t handle = mh::NO_VEHICLE;
    uint8_t engine = 0;
    uint64_t row_index = 0;
};

inline bool pick(const mh::Stock &table, const std::string &original, mh::Source &out)
{
    mh::Stock others;
    for (const auto &e : table.myteam)
        if (e.second != original)
            others.myteam.push_back(e);
    return mh::pick(others, mh::ends_with(original, "_2026"), out);
}

struct Pending {
    bool live = false;
    uint32_t handle = 0;
    std::string original;
    Stock stock;
};

struct Shadow {
    uint64_t live = 0;
    uint32_t handle = 0;
    std::string original;
    std::string stock;
    uint32_t stock_handle = mh::NO_VEHICLE;
    uint8_t saved_engine = 0;
    uint32_t saved_base = 0;
    std::vector<uint8_t> bytes;
};

inline void patch(uint8_t *record, const Stock &stock)
{
    std::memset(record + VEHICLE_FILE_NAME, 0, mh::LABEL_CHARS);
    std::memcpy(record + VEHICLE_FILE_NAME, stock.label.data(), stock.label.size() < mh::LABEL_CHARS ? stock.label.size() : mh::LABEL_CHARS - 1);
    record[VEHICLE_ENGINE] = stock.engine;
    std::memcpy(record + VEHICLE_BASE, &stock.handle, sizeof stock.handle);
}

inline Shadow shadow_of(uint64_t live, const uint8_t *record, const Pending &p)
{
    Shadow s;
    s.live = live;
    s.handle = p.handle;
    s.original = p.original;
    s.stock = p.stock.label;
    s.stock_handle = p.stock.handle;
    s.saved_engine = record[VEHICLE_ENGINE];
    s.saved_base = base_of(record);
    s.bytes.assign(record, record + SAVED_BYTES);
    std::memset(s.bytes.data() + VEHICLE_FILE_NAME, 0, mh::LABEL_CHARS);
    std::memcpy(s.bytes.data() + VEHICLE_FILE_NAME, p.original.data(),
                p.original.size() < mh::LABEL_CHARS ? p.original.size() : mh::LABEL_CHARS - 1);
    return s;
}

inline bool matches(const Shadow &s, uint64_t live, const uint8_t *record)
{
    uint32_t h = 0;
    std::memcpy(&h, record, sizeof h);
    return live == s.live && h == s.handle && base_of(record) == s.stock_handle &&
           text_at(record + VEHICLE_FILE_NAME, mh::LABEL_CHARS) == s.stock;
}

struct Shadows {
    std::vector<Shadow> list;

    Shadows()
    {
        list.reserve(MAX_SHADOWS);
    }

    bool room(uint64_t live) const
    {
        for (const Shadow &x : list)
            if (x.live == live)
                return true;
        return list.size() < MAX_SHADOWS;
    }

    bool put(Shadow s)
    {
        for (Shadow &x : list)
            if (x.live == s.live) {
                x = std::move(s);
                return true;
            }
        if (list.size() >= MAX_SHADOWS)
            return false;
        list.push_back(std::move(s));
        return true;
    }

    const Shadow *find(uint64_t live, const uint8_t *record) const
    {
        for (const Shadow &s : list)
            if (matches(s, live, record))
                return &s;
        return nullptr;
    }
};

inline bool parse_stock(const char *text, Stock &out)
{
    std::string t = text ? text : "";
    size_t a = t.find(':'), b = a == std::string::npos ? a : t.find(':', a + 1), c = b == std::string::npos ? b : t.find(':', b + 1);
    if (c == std::string::npos || a == 0)
        return false;
    char *end = nullptr;
    std::string h = t.substr(a + 1, b - a - 1), e = t.substr(b + 1, c - b - 1), r = t.substr(c + 1);
    unsigned long hv = std::strtoul(h.c_str(), &end, 0);
    if (h.empty() || *end)
        return false;
    unsigned long ev = std::strtoul(e.c_str(), &end, 0);
    if (e.empty() || *end || ev > 0xff)
        return false;
    unsigned long long rv = std::strtoull(r.c_str(), &end, 0);
    if (r.empty() || *end)
        return false;
    out.label = t.substr(0, a);
    out.handle = uint32_t(hv);
    out.engine = uint8_t(ev);
    out.row_index = rv;
    return true;
}

inline bool forced(const char *test_label, const std::string &label)
{
    return test_label && *test_label && label == test_label;
}

}
