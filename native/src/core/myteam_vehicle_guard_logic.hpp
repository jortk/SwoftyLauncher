#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace runtime::myteam_vehicle {

constexpr uint64_t FRAME_CONTAINER = 0x30;
constexpr uint64_t FRAME_INDEX = 0x38;
constexpr uint64_t FRAME_STAT_ID = 0x44;
constexpr uint64_t FRAME_LABEL = 0xb8;
constexpr uint64_t ROW_INDEX = 0x200;
constexpr uint32_t STAT_ID_MASK = 0x3fff;
constexpr size_t LABEL_CHARS = 0x100;
constexpr size_t LOG_LABELS = 32;

inline bool switch_off(const char *value)
{
    if (!value)
        return false;
    return value[0] == '0' && value[1] == 0 ? true : value[0] == 'o' && value[1] == 'f' && value[2] == 'f' && value[3] == 0;
}

inline bool row_missing(int32_t index)
{
    return index < 0;
}

inline uint32_t stat_id(uint32_t saved)
{
    return saved & STAT_ID_MASK;
}

struct KnownAddon {
    const char *prefix;
    const char *name;
    const char *id;
};

inline constexpr KnownAddon KNOWN_ADDONS[] = {
    {"myteam_mes_", "More Engine Suppliers", "mildtdesign.more-engine-suppliers"},
};

inline const KnownAddon *addon_for(const std::string &label)
{
    for (const KnownAddon &a : KNOWN_ADDONS)
        if (label.rfind(a.prefix, 0) == 0)
            return &a;
    return nullptr;
}

enum class Row { present, missing, unknown };

struct Skip {
    uint64_t container = 0;
    uint64_t index = 0;
    uint32_t stat_id = 0;
    std::string label;
    bool left = false;
};

template <class Mem> bool skip_base(Mem &mem, uint64_t rsp, uint64_t row, Skip &out)
{
    out.left = false;
    int32_t index = 0;
    if (!mem.i32(row + ROW_INDEX, index) || !row_missing(index))
        return false;
    uint64_t container = 0, base = 0, label = 0;
    uint32_t saved = 0;
    if (!mem.u64(rsp + FRAME_CONTAINER, container) || !container || !mem.u64(rsp + FRAME_INDEX, base) || !mem.u32(rsp + FRAME_STAT_ID, saved))
        return false;
    out.container = container;
    out.index = base;
    out.stat_id = stat_id(saved);
    out.label.clear();
    if (!mem.u64(rsp + FRAME_LABEL, label) || !label || !mem.text(label, out.label) || out.label.empty())
        out.label = "(unreadable)";
    if (!(addon_for(out.label) || mem.forced(out.label)) || mem.vehicle_row(out.label) != Row::missing) {
        out.left = true;
        return false;
    }
    return true;
}

struct Labels {
    std::vector<std::string> seen;

    bool first(const std::string &label)
    {
        for (const std::string &s : seen)
            if (s == label)
                return false;
        if (seen.size() >= LOG_LABELS)
            return false;
        seen.push_back(label);
        return true;
    }

    bool last() const
    {
        return seen.size() == LOG_LABELS;
    }
};

}
