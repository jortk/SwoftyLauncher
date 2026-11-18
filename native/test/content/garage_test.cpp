#define ZSTD_STATIC_LINKING_ONLY
#include "../../src/core/apply/content/garage_build.hpp"

#include <cstdio>
#include <memory>

namespace content = swofty_core::apply::content;
namespace g = content::garage;
namespace ef = content::erpfile;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static const unsigned char PNG8X4[] = {0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a,0x00,0x00,0x00,0x0d,0x49,0x48,0x44,0x52,0x00,0x00,0x00,0x08,0x00,0x00,0x00,0x04,0x08,0x06,0x00,0x00,0x00,0xb3,0xcd,0x7e,0xf0,0x00,0x00,0x00,0x3f,0x49,0x44,0x41,0x54,0x78,0x9c,0x15,0xca,0x31,0x01,0xc0,0x30,0x08,0x00,0x30,0x94,0x4c,0x09,0x4a,0x50,0xc2,0x39,0x15,0x55,0x82,0x12,0x0c,0x6d,0xe9,0x91,0x2f,0x11,0xf1,0x7e,0x0f,0x49,0xd1,0x1c,0x86,0x25,0x22,0x05,0x92,0xa2,0x39,0x0c,0x9b,0x37,0xb4,0x40,0x52,0x34,0x87,0x61,0xfb,0x86,0x11,0x48,0x8a,0xe6,0x30,0x2c,0x3f,0xff,0x54,0x48,0x41,0x29,0x7f,0x3d,0xfb,0x00,0x00,0x00,0x00,0x49,0x45,0x4e,0x44,0xae,0x42,0x60,0x82};

static g::Image bc1(uint32_t w, uint32_t h, uint32_t levels, uint8_t seed)
{
    g::Image img;
    img.format = g::FMT_BC1_SRGB;
    img.width = w;
    img.height = h;
    img.levels = levels;
    img.pixels.resize(g::chain_bytes(img.format, w, h, levels));
    for (size_t i = 0; i < img.pixels.size(); i++)
        img.pixels[i] = char(seed + i * 7);
    return img;
}

struct Parsed {
    std::string bytes;
    ef::Source src;
    ef::Archive arc;
    std::string why;
    bool ok = false;
    explicit Parsed(std::string b) : bytes(std::move(b))
    {
        src.size = bytes.size();
        src.read = [this](uint64_t off, size_t n, char *out) {
            if (off + n > bytes.size())
                return false;
            std::memcpy(out, bytes.data() + off, n);
            return true;
        };
        ok = ef::open(src, arc, why);
    }
    std::string frag(const std::string &name, size_t i)
    {
        const ef::Entry *e = arc.find(name);
        std::string out, w;
        if (!e || !ef::fragment(src, arc, e->frags[i], out, w))
            return {};
        return out;
    }
};

int main()
{
    const std::string root = "eaid://environment_package/core_assets/objects/textures/";
    g::Image img = bc1(16, 8, 5, 3);
    std::string dds = g::write_dds(img);
    g::Image back;
    std::string why;
    check(g::read_dds(dds, back, why) && back.format == img.format && back.width == 16 && back.height == 8 && back.levels == 5 &&
              back.pixels == img.pixels,
          "a DDS written from an image reads back the same format, size, mips and pixels");
    check(g::chain_bytes(g::FMT_BC1_SRGB, 4096, 4096, 13) == 11184824, "a 4096 square BC1 chain of 13 levels is the stock 11184824 bytes");

    std::string truncated = dds.substr(0, dds.size() - 1);
    check(!g::read_dds(truncated, back, why) && why.find("needed") != std::string::npos, "a DDS short of its pixel data is refused: " + why);
    check(!g::read_dds("not a dds file at all, long enough to pass the size check................................................................................................", back, why),
          "a file that is not a DDS is refused");
    std::string odd = dds;
    uint32_t dxgi = 28;
    std::memcpy(&odd[128], &dxgi, 4);
    check(!g::read_dds(odd, back, why) && why.find("DXGI format 28") != std::string::npos, "an uncompressed DXGI format is refused: " + why);

    std::vector<g::Input> files = {{"garage_liveries/garage_livery_renault_a_01_da.dds", dds},
                                   {"pitlane/pitstop_gantry_renault_a_d.dds", g::write_dds(bc1(8, 8, 4, 9))},
                                   {"broken.dds", "DDS "}};
    g::Built built = g::build(files, root, true);
    check(built.refused == 1 && built.names.size() == 2 && !built.erp.empty(), "two textures pack and the broken file is refused");
    Parsed p(built.erp);
    check(p.ok && p.arc.entries.size() == 4, "the archive parses with a surface and a view per texture: " + p.why);
    const std::string surface = root + "garage_liveries/garage_livery_renault_a_01_da.tif.image",
                      view = root + "garage_liveries/garage_livery_renault_a_01_da.tif";
    check(p.arc.find(surface) && p.arc.find(view), "the names are the root, the file's path and .tif.image or .tif");
    const ef::Entry *e = p.arc.find(surface);
    check(e && e->mid.compare(0, 13, "GfxSurfaceRes") == 0 && e->frags.size() == 3 && e->frags[0].size == 40, "the surface has the 40 byte header, a pixel fragment and a streaming descriptor");
    std::string head = p.frag(surface, 0);
    uint32_t want[] = {2, 0x108, g::FMT_BC1_SRGB, 16, 8, 1, 5, 1, 0, 0};
    check(head.size() == 40 && std::memcmp(head.data(), want, 40) == 0, "its header holds the format, size and mip count");
    check(p.frag(surface, 1) == img.pixels, "its pixel fragment is every mip level as the DDS has it");
    std::string desc = p.frag(surface, 2);
    const std::string mips = "environment_package/core_assets/objects/textures/garage_liveries/garage_livery_renault_a_01_da.tif.2025.mipmaps";
    check(desc.size() >= 256 && uint8_t(desc[0]) == mips.size() && desc.substr(1, mips.size()) == mips && desc[1 + mips.size()] == '\0' &&
              g::le32(desc, 2 + mips.size()) == 0,
          "its streaming descriptor names the stock .mipmaps file below the root, with no streamed level, in a fragment as large as the stock ones");
    float one = 0;
    std::memcpy(&one, desc.data() + 2 + mips.size() + 8, 4);
    check(one == 1.0f, "the descriptor ends with the stock 1.0 field");
    g::Built plain = g::build(files, root, true, false);
    check(Parsed(plain.erp).arc.find(surface)->frags.size() == 2 && plain.erp != built.erp, "a build without descriptors keeps two fragments");
    std::string srv = p.frag(view, 0);
    check(srv.size() == 24 + surface.size() + 1 && g::le32(srv, 0) == 5 && g::le32(srv, 4) == g::FMT_BC1_SRGB && g::le32(srv, 12) == 5 &&
              srv.substr(24, surface.size()) == surface,
          "the view holds the format, the mip count and the surface name");
    const ef::Entry *v = p.arc.find(view);
    check(v && v->mid.compare(0, 14, "GfxSRVResource") == 0 && g::le32(v->mid, 16) == 2 &&
              g::le32(p.arc.find(surface)->mid, 16) == 9,
          "the resource types and create arguments are the stock ones (9 for a surface, 2 for a view)");
    check(g::build(files, root, true).erp == built.erp, "the same files build the same archive");
    check(g::build(files, root, true).erp != g::build({files[0]}, root, true).erp, "another file set builds another archive");

    std::vector<g::Input> pngs = {{"main_front_end/names/f1_team_names/f1_teamname_alpine.png", std::string(reinterpret_cast<const char *>(PNG8X4), sizeof PNG8X4)}};
    g::Built made = g::build(pngs, root, true);
    check(made.refused == 0 && made.names.size() == 1, "a PNG packs");
    Parsed q(made.erp);
    std::string png_surface = root + "main_front_end/names/f1_team_names/f1_teamname_alpine.tif.image";
    std::string pj = q.frag(png_surface, 0);
    check(q.ok && pj.size() == 40 && g::le32(pj, 8) == g::FMT_BC7_SRGB && g::le32(pj, 12) == 8 && g::le32(pj, 16) == 4 && g::le32(pj, 24) == 4,
          "a PNG becomes sRGB BC7 with a mip chain down to 1x1");
    check(q.frag(png_surface, 1).size() == g::chain_bytes(g::FMT_BC7_SRGB, 8, 4, 4), "its pixel fragment holds every level");
    check(g::le32(g::build(pngs, root, false).erp.empty() ? std::string(40, '\0') : Parsed(g::build(pngs, root, false).erp).frag(png_surface, 0), 8) == g::FMT_BC7,
          "srgb false encodes linear BC7");
    std::printf("%s\n", failures ? "FAILED" : "all passed");
    return failures ? 1 : 0;
}
