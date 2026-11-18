#pragma once

#include "bc7.hpp"
#include "erp_file.hpp"
#include "png_straight.hpp"

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <cstring>
#include <string>
#include <string_view>
#include <vector>

// Packs a garage art folder into one ERP archive of texture resources. Each .dds or .png file becomes the pair the
// game's textures are: a GfxSurfaceRes <name>.tif.image (a 40 byte header fragment and the pixel fragment holding
// every mip level, largest first) and a GfxSRVResource <name>.tif (a 24 byte header and the surface's name). The
// names are the root plus the file's path below the folder, so a stock texture's name is the file's. A .dds keeps its
// own format and mip chain; a .png is encoded to BC7 with a box-filtered mip chain. Pure: the files come in as bytes.

namespace swofty_core::apply::content::garage {

inline constexpr uint32_t ERPK = 0x4b505245;
inline constexpr uint8_t STORED = 0x91, ZSTD = 0x11;
inline constexpr int ZSTD_LEVEL = 3;
inline constexpr size_t MIN_PACK = 64;
inline constexpr uint32_t SURFACE_ARG = 9, SRV_ARG = 2;
inline constexpr uint32_t FMT_BC1 = 52, FMT_BC1_SRGB = 54, FMT_BC3 = 55, FMT_BC3_SRGB = 57, FMT_BC5 = 65, FMT_BC7 = 69,
                          FMT_BC7_SRGB = 70;

struct Image {
    uint32_t format = 0, width = 0, height = 0, levels = 1;
    std::string pixels;
};

struct Input {
    std::string rel, bytes;
};

struct Built {
    std::string erp;
    std::vector<std::string> names, lines;
    int refused = 0;
};

inline uint32_t le32(std::string_view s, size_t at)
{
    uint32_t v = 0;
    if (at + 4 <= s.size())
        std::memcpy(&v, s.data() + at, 4);
    return v;
}

inline size_t block_bytes(uint32_t format)
{
    switch (format) {
    case FMT_BC1:
    case FMT_BC1_SRGB:
        return 8;
    case FMT_BC3:
    case FMT_BC3_SRGB:
    case FMT_BC5:
    case FMT_BC7:
    case FMT_BC7_SRGB:
        return 16;
    }
    return 0;
}

inline size_t level_bytes(uint32_t format, uint32_t w, uint32_t h)
{
    return size_t((w + 3) / 4) * size_t((h + 3) / 4) * block_bytes(format);
}

inline size_t chain_bytes(uint32_t format, uint32_t w, uint32_t h, uint32_t levels)
{
    size_t total = 0;
    for (uint32_t i = 0; i < levels; i++) {
        total += level_bytes(format, w, h);
        w = std::max(w / 2, 1u);
        h = std::max(h / 2, 1u);
    }
    return total;
}

inline uint32_t from_dxgi(uint32_t dxgi)
{
    switch (dxgi) {
    case 71:
        return FMT_BC1;
    case 72:
        return FMT_BC1_SRGB;
    case 77:
        return FMT_BC3;
    case 78:
        return FMT_BC3_SRGB;
    case 83:
        return FMT_BC5;
    case 98:
        return FMT_BC7;
    case 99:
        return FMT_BC7_SRGB;
    }
    return 0;
}

inline uint32_t to_dxgi(uint32_t format)
{
    switch (format) {
    case FMT_BC1:
        return 71;
    case FMT_BC1_SRGB:
        return 72;
    case FMT_BC3:
        return 77;
    case FMT_BC3_SRGB:
        return 78;
    case FMT_BC5:
        return 83;
    case FMT_BC7:
        return 98;
    case FMT_BC7_SRGB:
        return 99;
    }
    return 0;
}

inline bool read_dds(std::string_view b, Image &out, std::string &why)
{
    if (b.size() < 128 || b.substr(0, 4) != "DDS " || le32(b, 4) != 124)
        return why = "not a DDS file", false;
    out.height = le32(b, 12);
    out.width = le32(b, 16);
    out.levels = std::max(le32(b, 28), 1u);
    size_t data = 128;
    uint32_t four = le32(b, 84);
    if (four == 0x30315844) {
        if (b.size() < 148)
            return why = "the DX10 header is cut short", false;
        uint32_t dxgi = le32(b, 128);
        out.format = from_dxgi(dxgi);
        if (!out.format)
            return why = "DXGI format " + std::to_string(dxgi) + " is not BC1, BC3, BC5 or BC7", false;
        if (le32(b, 136) > 1 || le32(b, 140) > 1)
            return why = "texture arrays and cube maps are not supported", false;
        data = 148;
    } else if (four == 0x31545844)
        out.format = FMT_BC1_SRGB;
    else if (four == 0x35545844)
        out.format = FMT_BC3_SRGB;
    else if (four == 0x32495441 || four == 0x55354342)
        out.format = FMT_BC5;
    else
        return why = "the DDS pixel format is not BC1, BC3, BC5 or BC7", false;
    if (!out.width || !out.height || out.width > 16384 || out.height > 16384 || out.levels > 15)
        return why = "the size " + std::to_string(out.width) + "x" + std::to_string(out.height) + " is not supported", false;
    size_t want = chain_bytes(out.format, out.width, out.height, out.levels);
    if (b.size() - data < want)
        return why = "the pixel data is " + std::to_string(b.size() - data) + " bytes, " + std::to_string(want) + " needed", false;
    out.pixels.assign(b.data() + data, want);
    return true;
}

inline std::string write_dds(const Image &img)
{
    std::string h(148, '\0');
    auto put = [&](size_t at, uint32_t v) { std::memcpy(&h[at], &v, 4); };
    std::memcpy(&h[0], "DDS ", 4);
    put(4, 124);
    put(8, 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000);
    put(12, img.height);
    put(16, img.width);
    put(20, uint32_t(level_bytes(img.format, img.width, img.height)));
    put(28, img.levels);
    put(76, 32);
    put(80, 4);
    put(84, 0x30315844);
    put(108, 0x1000 | 0x8 | (img.levels > 1 ? 0x400000 : 0));
    put(128, to_dxgi(img.format));
    put(132, 3);
    put(140, 1);
    return h + img.pixels;
}

inline bool encode_png(std::string_view bytes, bool srgb, Image &out, std::string &why)
{
    std::string rgba;
    uint32_t w = 0, h = 0;
    if (!png::decode(bytes, rgba, w, h, why))
        return false;
    if (!w || !h || w > 8192 || h > 8192)
        return why = "the size " + std::to_string(w) + "x" + std::to_string(h) + " is not supported", false;
    out.format = srgb ? FMT_BC7_SRGB : FMT_BC7;
    out.width = w;
    out.height = h;
    out.levels = 1;
    for (uint32_t m = std::max(w, h); m > 1; m /= 2)
        out.levels++;
    std::string level = rgba;
    uint32_t lw = w, lh = h;
    for (uint32_t i = 0; i < out.levels; i++) {
        uint32_t pw = (lw + 3) / 4 * 4, ph = (lh + 3) / 4 * 4;
        std::string padded(size_t(pw) * ph * 4, '\0');
        for (uint32_t y = 0; y < ph; y++)
            for (uint32_t x = 0; x < pw; x++)
                std::memcpy(&padded[(size_t(y) * pw + x) * 4], &level[(size_t(std::min(y, lh - 1)) * lw + std::min(x, lw - 1)) * 4], 4);
        out.pixels += bc7::encode_rgba(reinterpret_cast<const uint8_t *>(padded.data()), int(pw), int(ph));
        if (i + 1 == out.levels)
            break;
        uint32_t nw = std::max(lw / 2, 1u), nh = std::max(lh / 2, 1u);
        std::string next(size_t(nw) * nh * 4, '\0');
        for (uint32_t y = 0; y < nh; y++)
            for (uint32_t x = 0; x < nw; x++)
                for (int c = 0; c < 4; c++) {
                    uint32_t sum = 0, n = 0;
                    for (uint32_t dy = 0; dy < 2; dy++)
                        for (uint32_t dx = 0; dx < 2; dx++) {
                            uint32_t sx = std::min(x * 2 + dx, lw - 1), sy = std::min(y * 2 + dy, lh - 1);
                            sum += uint8_t(level[(size_t(sy) * lw + sx) * 4 + c]);
                            n++;
                        }
                    next[(size_t(y) * nw + x) * 4 + c] = char((sum + n / 2) / n);
                }
        level = std::move(next);
        lw = nw;
        lh = nh;
    }
    return true;
}

inline std::string surface_header(const Image &img)
{
    std::string h(40, '\0');
    uint32_t v[] = {2, 0x108, img.format, img.width, img.height, 1, img.levels, 1};
    std::memcpy(h.data(), v, sizeof v);
    return h;
}

inline std::string stream_descriptor(const std::string &root, const std::string &stem)
{
    constexpr size_t STOCK_LEN = 288;
    std::string path = root;
    if (path.rfind("eaid://", 0) == 0)
        path.erase(0, 7);
    path += stem + ".tif.2025.mipmaps";
    std::string d;
    d.push_back(char(uint8_t(std::min<size_t>(path.size(), 255))));
    d += path.substr(0, 255);
    d.push_back('\0');
    d.append(4, '\0');
    d.append(4, '\0');
    const float one = 1.0f;
    d.append(reinterpret_cast<const char *>(&one), 4);
    if (d.size() < STOCK_LEN)
        d.append(STOCK_LEN - d.size(), '\0');
    return d;
}

inline std::string srv_data(const Image &img, const std::string &surface_name)
{
    std::string d(24, '\0');
    uint32_t v[] = {5, img.format, 0, img.levels, 0, 0};
    std::memcpy(d.data(), v, sizeof v);
    return d + surface_name + '\0';
}

struct Res {
    std::string name, type;
    uint32_t arg = 0;
    std::vector<std::string> raw, packed;
    std::vector<uint8_t> comp;
};

inline Res make_res(const std::string &name, const char *type, uint32_t arg, std::vector<std::string> frags)
{
    Res r;
    r.name = name;
    r.type = type;
    r.arg = arg;
    for (auto &f : frags) {
        std::string z;
        if (f.size() >= MIN_PACK && erp_zstd::compress(f.data(), f.size(), ZSTD_LEVEL, z) && z.size() < f.size()) {
            r.packed.push_back(std::move(z));
            r.comp.push_back(ZSTD);
        } else {
            r.packed.push_back(f);
            r.comp.push_back(STORED);
        }
        r.raw.push_back(std::move(f));
    }
    return r;
}

inline std::string write_erp(const std::vector<Res> &list)
{
    std::string idx;
    uint64_t data_size = 0;
    uint32_t nfrag = 0;
    for (auto &r : list) {
        std::string e;
        uint16_t nlen = uint16_t(r.name.size() + 1);
        e.append(reinterpret_cast<const char *>(&nlen), 2);
        e.append(r.name.c_str(), r.name.size() + 1);
        char type[16] = {};
        std::memcpy(type, r.type.data(), std::min<size_t>(r.type.size(), 15));
        e.append(type, 16);
        e.append(reinterpret_cast<const char *>(&r.arg), 4);
        e.push_back(char(uint8_t(r.raw.size())));
        std::string all;
        for (size_t i = 0; i < r.raw.size(); i++) {
            uint64_t off = data_size, size = r.raw[i].size(), packed = r.packed[i].size();
            uint32_t flags = 0x10;
            e.append("temp", 4);
            e.append(reinterpret_cast<const char *>(&off), 8);
            e.append(reinterpret_cast<const char *>(&size), 8);
            e.append(reinterpret_cast<const char *>(&flags), 4);
            e.push_back(char(r.comp[i]));
            e.append(reinterpret_cast<const char *>(&packed), 8);
            data_size += packed;
            nfrag++;
            all += r.raw[i];
        }
        char digest[16];
        erpfile::md5(all, digest);
        e.append(digest, 16);
        uint32_t esz = uint32_t(e.size());
        idx.append(reinterpret_cast<const char *>(&esz), 4);
        idx += e;
    }
    uint32_t head[2] = {uint32_t(list.size()), nfrag};
    idx.insert(0, reinterpret_cast<const char *>(head), 8);
    std::string hdr(0x30, '\0');
    uint32_t magic = ERPK, ver = 3;
    uint64_t io = 0x30, isz = idx.size(), dof = 0x30 + idx.size();
    std::memcpy(hdr.data(), &magic, 4);
    std::memcpy(hdr.data() + 4, &ver, 4);
    std::memcpy(hdr.data() + 0x10, &io, 8);
    std::memcpy(hdr.data() + 0x18, &isz, 8);
    std::memcpy(hdr.data() + 0x20, &dof, 8);
    std::string out = hdr + idx;
    for (auto &r : list)
        for (auto &p : r.packed)
            out += p;
    return out;
}

inline std::string extension_of(const std::string &rel)
{
    auto dot = rel.rfind('.');
    std::string e = dot == std::string::npos ? std::string() : rel.substr(dot);
    for (auto &c : e)
        c = char(std::tolower(static_cast<unsigned char>(c)));
    return e;
}

inline bool texture_file(const std::string &rel)
{
    std::string e = extension_of(rel);
    return e == ".dds" || e == ".png";
}

inline Built build(const std::vector<Input> &files, const std::string &root, bool srgb, bool descriptor = true)
{
    Built out;
    std::vector<Res> list;
    for (auto &f : files) {
        std::string stem = f.rel.substr(0, f.rel.size() - extension_of(f.rel).size());
        std::string why;
        Image img;
        bool ok = extension_of(f.rel) == ".dds" ? read_dds(f.bytes, img, why) : encode_png(f.bytes, srgb, img, why);
        if (!ok) {
            out.lines.push_back(f.rel + ": " + why);
            out.refused++;
            continue;
        }
        std::string surface = root + stem + ".tif.image", view = root + stem + ".tif";
        list.push_back(make_res(surface, "GfxSurfaceRes", SURFACE_ARG, descriptor ? std::vector<std::string>{surface_header(img), img.pixels, stream_descriptor(root, stem)}
                                                                    : std::vector<std::string>{surface_header(img), img.pixels}));
        list.push_back(make_res(view, "GfxSRVResource", SRV_ARG, {srv_data(img, surface)}));
        out.names.push_back(surface);
    }
    if (!list.empty())
        out.erp = write_erp(list);
    return out;
}

}
