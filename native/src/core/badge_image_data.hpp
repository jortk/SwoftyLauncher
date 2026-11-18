#pragma once
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace badge_images::data {
// Metadata tracks at most 1024 recognised names; at most 32 GPU copies can be
// in flight. Completed runtime pixels/resources have their own 32-entry bound.
inline constexpr unsigned SIDE = 64, PIXELS = SIDE * SIDE * 4, CACHE = 1024, PENDING = 32;

// D3D debug names may include the engine's resource URI or a .image suffix.
// Match the complete Live API identifier, not a table/index substring.
inline std::optional<std::string> reference(std::string_view name)
{
    constexpr std::string_view prefix = "render_badge_";
    auto at = name.find(prefix);
    if (at == std::string_view::npos)
        return {};
    if (at && name[at - 1] != '/')
        return {};
    auto suffix = name.substr(at + prefix.size());
    if (suffix.ends_with(".image"))
        suffix.remove_suffix(6);
    if (suffix == "default.tga")
        return "eaid://ui_package/art/textures/render_badge_default.tga";
    auto space = suffix.find(' ');
    if (space == std::string_view::npos)
        return {};
    auto table = suffix.substr(0, space), number = suffix.substr(space + 1);
    if ((table != "ePreLoaded" && table != "eUserDefined" && table != "eNetworkReplicated") || number.empty() || number.size() > 5)
        return {};
    unsigned index = 0;
    for (unsigned char c : number) {
        if (c < '0' || c > '9')
            return {};
        index = index * 10 + c - '0';
    }
    if (index >= 0x3fff)
        return {};
    return "eaid://ui_package/art/textures/render_badge_" + std::string(table) + " " + std::to_string(index);
}

// Tight RGBA rows, preserving alpha and aspect ratio. Nearest-neighbour is
// intentional: the small HUD badge should not acquire a colour fringe at alpha edges.
inline std::vector<uint8_t> pixels(const uint8_t *source, unsigned width, unsigned height, unsigned pitch,
                                   bool bgra, unsigned &out_w, unsigned &out_h)
{
    if (!source || !width || !height || width > 512 || height > 512 || pitch < width * 4)
        return {};
    const unsigned longest = std::max(width, height);
    out_w = longest <= SIDE ? width : std::max(1u, width * SIDE / longest);
    out_h = longest <= SIDE ? height : std::max(1u, height * SIDE / longest);
    std::vector<uint8_t> out(size_t(out_w) * out_h * 4);
    for (unsigned y = 0; y < out_h; y++)
        for (unsigned x = 0; x < out_w; x++) {
            const uint8_t *from = source + size_t(y * height / out_h) * pitch + (x * width / out_w) * 4;
            uint8_t *to = out.data() + (size_t(y) * out_w + x) * 4;
            std::memcpy(to, from, 4);
            if (bgra)
                std::swap(to[0], to[2]);
        }
    return out;
}
}
