#define ZSTD_STATIC_LINKING_ONLY
#include "../../src/core/apply/content/logo_build.hpp"
#include "../../src/core/apply/content/png_straight.hpp"

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <set>
#include <sstream>

namespace content = swofty_core::apply::content;
namespace lb = content::logo_build;
namespace ef = content::erpfile;
namespace fs = std::filesystem;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static std::string slurp(const fs::path &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream s;
    s << f.rdbuf();
    return s.str();
}

static std::shared_ptr<ef::Source> open_file(const fs::path &path)
{
    if (!fs::exists(path))
        return nullptr;
    auto in = std::make_shared<std::ifstream>(path, std::ios::binary);
    auto src = std::make_shared<ef::Source>();
    src->size = fs::file_size(path);
    src->read = [in](uint64_t off, size_t n, char *out) {
        in->clear();
        in->seekg(std::streamoff(off));
        in->read(out, std::streamsize(n));
        return size_t(in->gcount()) == n;
    };
    return src;
}

static std::string rewrite_to_string(const ef::Source &src, const ef::Archive &arc, const ef::Changes &changes, std::string &why)
{
    std::string out;
    ef::rewrite(src, arc, changes, [&](const char *p, size_t n) { out.append(p, n); return true; }, why);
    return out;
}

int main(int argc, char **argv)
{
    fs::path dir = argc > 1 ? argv[1] : "content/bc7";
    fs::path game = dir / "game";
    std::string why;

    std::string raw = slurp(dir / "shape64.rgba"), png_bytes = slurp(dir / "shape64.png"), decoded;
    const char *px = raw.data() + raw.find('\n') + 1;
    uint32_t w = 0, h = 0;
    check(content::png::decode(png_bytes, decoded, w, h, why) && w == 64 && h == 64 && decoded == std::string(px, 64 * 64 * 4),
          "a PNG decodes to straight alpha with zero colour under alpha 0");

    std::vector<std::string> list = {"2025_asset_groups/ui_package/databases.erp", "2025_asset_groups/ui_package/flow_a.erp",
                                     "2025_asset_groups/ui_package/flow_none.erp", "2025_asset_groups/ui_package/screens/b.erp"};
    for (auto &rel : list) {
        auto src = open_file(game / rel);
        ef::Archive arc;
        bool ok = src && ef::open(*src, arc, why);
        check(ok && rewrite_to_string(*src, arc, {}, why) == slurp(game / rel), rel + ": rewriting with no change gives the same bytes");
    }

    {
        uint64_t seed = 7;
        auto noisy = [&](size_t n) {
            std::string out(n, '\0');
            for (size_t i = 0; i < n; i++) {
                seed = seed * 6364136223846793005ull + 1442695040888963407ull;
                out[i] = i % 4 ? "livery_"[i % 7] : char(seed >> 56);
            }
            return out;
        };
        for (size_t n : {size_t(0), size_t(300), size_t(70) << 10, size_t(1) << 20, (size_t(3) << 20) + 5}) {
            for (int level : {3, ef::ZSTD_LEVEL, 19}) {
                std::string data = noisy(n), z, back(n, '\0');
                ZSTD_FrameHeader h{};
                bool ok = swofty_core::erp_zstd::compress(data.data(), n, level, z) && ZSTD_getFrameHeader(&h, z.data(), z.size()) == 0;
                size_t buffer = ok ? ZSTD_decodingBufferSize_min(h.windowSize, h.frameContentSize) + std::max<size_t>(h.blockSizeMax, 4) : 0;
                ok = ok && h.frameContentSize == n && h.windowSize <= (1u << 20) && buffer <= 1441856 &&
                     ZSTD_decompress(back.data(), n, z.data(), z.size()) == n && back == data;
                check(ok, std::to_string(n) + " bytes at level " + std::to_string(level) + ": the frame names its size and a window of " +
                              std::to_string(h.windowSize) + " bytes, a " + std::to_string(buffer) + "-byte decoder buffer");
            }
        }
    }

    std::string uidb = slurp(dir / "expect/overrides.uidb");
    content::uidb::Pkg pkg;
    check(pkg.read(uidb) && pkg.write() == uidb, "overrides.uidb reads and writes to the same bytes");

    {
        namespace la = content::logo_array;
        auto px_of = [](const std::string &img, uint32_t w, uint32_t x, uint32_t y) {
            return reinterpret_cast<const uint8_t *>(img.data() + (size_t(y) * w + x) * 4);
        };
        std::string solid(8 * 8 * 4, '\0');
        for (size_t i = 0; i < 64; i++) {
            solid[i * 4] = char(200);
            solid[i * 4 + 1] = 100;
            solid[i * 4 + 2] = 50;
            solid[i * 4 + 3] = char(255);
        }
        std::string same = la::fit_rgba(reinterpret_cast<const uint8_t *>(solid.data()), 8, 8, 8, 8);
        check(same == solid, "fit at the exact size keeps every pixel");
        std::string small = la::fit_rgba(reinterpret_cast<const uint8_t *>(solid.data()), 8, 8, 4, 4);
        bool flat = true;
        for (uint32_t y = 0; y < 4; y++)
            for (uint32_t x = 0; x < 4; x++) {
                auto p = px_of(small, 4, x, y);
                flat &= p[0] == 200 && p[1] == 100 && p[2] == 50 && p[3] == 255;
            }
        check(flat, "shrinking an opaque image keeps colour and alpha at the edges");
        std::string big = la::fit_rgba(reinterpret_cast<const uint8_t *>(solid.data()), 8, 8, 16, 16);
        flat = true;
        for (uint32_t y = 0; y < 16; y++)
            for (uint32_t x = 0; x < 16; x++) {
                auto p = px_of(big, 16, x, y);
                flat &= p[0] == 200 && p[1] == 100 && p[2] == 50 && p[3] == 255;
            }
        check(flat, "growing an opaque image keeps colour and alpha at the edges");
        std::string checker(2 * 2 * 4, '\0');
        for (int i = 0; i < 4; i++) {
            bool white = (i == 0 || i == 3);
            checker[i * 4] = checker[i * 4 + 1] = checker[i * 4 + 2] = white ? char(255) : 0;
            checker[i * 4 + 3] = char(255);
        }
        std::string avg = la::fit_rgba(reinterpret_cast<const uint8_t *>(checker.data()), 2, 2, 1, 1);
        check(px_of(avg, 1, 0, 0)[0] >= 127 && px_of(avg, 1, 0, 0)[0] <= 128 && px_of(avg, 1, 0, 0)[3] == 255,
              "shrinking averages the covered pixels (area average)");
        std::string wide(8 * 4 * 4, '\0');
        for (size_t i = 0; i < 32; i++) {
            wide[i * 4] = char(255);
            wide[i * 4 + 3] = char(255);
        }
        std::string fit = la::fit_rgba(reinterpret_cast<const uint8_t *>(wide.data()), 8, 4, 16, 16);
        bool top_clear = true, middle = true, bottom_clear = true;
        for (uint32_t x = 0; x < 16; x++) {
            for (uint32_t y = 0; y < 4; y++)
                top_clear &= px_of(fit, 16, x, y)[3] == 0;
            for (uint32_t y = 4; y < 12; y++)
                middle &= px_of(fit, 16, x, y)[3] == 255 && px_of(fit, 16, x, y)[0] == 255;
            for (uint32_t y = 12; y < 16; y++)
                bottom_clear &= px_of(fit, 16, x, y)[3] == 0;
        }
        check(top_clear && middle && bottom_clear, "a non-square image keeps its aspect, is centred and the padding is transparent");
        std::string half(4 * 4 * 4, '\0');
        for (uint32_t y = 0; y < 4; y++)
            for (uint32_t x = 2; x < 4; x++) {
                size_t i = (size_t(y) * 4 + x) * 4;
                half[i] = char(255);
                half[i + 3] = char(255);
            }
        std::string edge = la::fit_rgba(reinterpret_cast<const uint8_t *>(half.data()), 4, 4, 2, 2);
        check(px_of(edge, 2, 0, 0)[3] == 0 && px_of(edge, 2, 0, 0)[0] == 0 && px_of(edge, 2, 1, 0)[3] == 255 && px_of(edge, 2, 1, 0)[0] == 255,
              "transparent pixels do not bleed colour into opaque ones (straight alpha)");
    }

    lb::Env env;
    env.list = [&] { return list; };
    env.open = [&](const std::string &rel) { return open_file(game / rel); };
    std::vector<lb::Want> wants(2);
    wants[0] = {"teams.driver_select_team_logos", "newteam", "", std::string(px, 64 * 64 * 4), 64, 64};
    wants[1].array = "teams.driver_select_team_logos";
    wants[1].key = "newalias";
    wants[1].donor = "b";
    lb::Result res;
    check(lb::build(env, wants, res) && res.built == 2 && res.refused == 0, "two slices are built");
    check(res.changes.size() == 3 && res.changes.count(lb::DATABASES) && res.changes.count(list[1]) && res.changes.count(list[3]) &&
              !res.changes.count(list[2]),
          "the two carriers and databases.erp change, the archive without the array does not");
    check(res.read == std::set<std::string>{lb::DATABASES, list[1], list[3]}, "the build reads databases.erp and the two carriers");

    fs::path out = argc > 2 ? argv[2] : "../build/bc7/logo_out";
    fs::create_directories(out);
    for (auto &[rel, changes] : res.changes) {
        auto src = open_file(game / rel);
        ef::Archive arc;
        ef::open(*src, arc, why);
        std::string built = rewrite_to_string(*src, arc, changes, why);
        std::string name = rel.substr(rel.rfind('/') + 1);
        if (rel.find("screens/") != std::string::npos)
            name = "screens_" + name;
        std::ofstream(out / name, std::ios::binary) << built;
    }
    check(true, "archives written to " + out.string() + " for check_logo_build.py");

    {
        fs::path ov = out / "overlay";
        fs::create_directories(ov / "ov");
        std::vector<lb::Want> first(1);
        first[0] = {"teams.driver_select_team_logos", "newteam", "", std::string(px, 64 * 64 * 4), 64, 64};
        lb::Result r1;
        lb::build(env, first, r1);
        auto count_of = [&](const fs::path &file, const std::string &rel_image) -> uint32_t {
            auto src = open_file(file);
            ef::Archive arc;
            std::string head, w2;
            if (!src || !ef::open(*src, arc, w2))
                return 0;
            const ef::Entry *e = arc.find(rel_image);
            if (!e || e->frags.empty() || !ef::fragment(*src, arc, e->frags[0], head, w2))
                return 0;
            return content::logo_array::le32(head, content::logo_array::HEAD_COUNT);
        };
        std::string image = std::string(lb::ARRAY_URI) + "teams.driver_select_team_logos.texarray.image";
        for (auto &[rel, changes] : r1.changes) {
            auto src = open_file(game / rel);
            ef::Archive arc;
            ef::open(*src, arc, why);
            std::string name = "ov/" + std::to_string(std::hash<std::string>{}(rel)) + ".erp";
            std::ofstream(ov / name, std::ios::binary) << rewrite_to_string(*src, arc, changes, why);
        }
        auto overlay_of = [&](const std::string &rel) { return "ov/" + std::to_string(std::hash<std::string>{}(rel)) + ".erp"; };
        uint32_t stock_count = count_of(game / list[1], image);
        lb::Env layered;
        layered.list = [&] { return list; };
        layered.open = [&](const std::string &rel) { return rel.rfind("ov/", 0) == 0 ? open_file(ov / rel) : open_file(game / rel); };
        layered.overlays = [&](const std::string &rel) {
            std::vector<std::string> v;
            if (r1.changes.count(rel))
                v.push_back(overlay_of(rel));
            return v;
        };
        std::vector<lb::Want> second(1);
        second[0].array = "teams.driver_select_team_logos";
        second[0].key = "second";
        second[0].donor = "b";
        lb::Result r5;
        check(lb::build(layered, second, r5) && r5.built == 1 && r5.refused == 0, "a slice is added on top of an overlay that already grew the array");
        std::string ovrel = overlay_of(list[1]);
        bool both = r5.changes.count(ovrel) && r5.changes.count(list[1]);
        check(both && r5.read.count(ovrel), "the overlay archive and the stock archive both change and the overlay is read");
        if (both) {
            for (auto &[rel, changes] : r5.changes) {
                auto src = layered.open(rel);
                ef::Archive arc;
                ef::open(*src, arc, why);
                std::ofstream(ov / ("out_" + std::to_string(std::hash<std::string>{}(rel)) + ".erp"), std::ios::binary)
                    << rewrite_to_string(*src, arc, changes, why);
            }
            uint32_t over = count_of(ov / ("out_" + std::to_string(std::hash<std::string>{}(ovrel)) + ".erp"), image);
            uint32_t stock_out = count_of(ov / ("out_" + std::to_string(std::hash<std::string>{}(list[1])) + ".erp"), image);
            check(over == stock_count + 2, "the overlay's array keeps the slice it already had and gains the new one (" + std::to_string(over) + " of " + std::to_string(stock_count) + " + 2)");
            check(stock_out == over, "the stock archive carries the same grown array, so the game gets it from either");
        }
        lb::Result r6, r7;
        lb::build(env, second, r6);
        lb::Env nolayer = layered;
        nolayer.overlays = nullptr;
        nolayer.open = env.open;
        lb::build(nolayer, second, r7);
        bool same = r6.changes.size() == r7.changes.size();
        for (auto &[rel, changes] : r6.changes)
            same = same && r7.changes.count(rel) && r7.changes[rel] == changes;
        check(same, "with no overlay the changes are the ones built from the stock archives");
    }

    {
        fs::path ov = out / "desync";
        fs::create_directories(ov / "ov");
        std::string array = "teams.driver_select_team_logos";
        std::string image = std::string(lb::ARRAY_URI) + array + ".texarray.image";
        std::vector<lb::Want> grow_names(2);
        for (int i = 0; i < 2; i++) {
            grow_names[i].array = array;
            grow_names[i].key = "ghost" + std::to_string(i);
            grow_names[i].donor = "b";
        }
        lb::Result names_only;
        lb::build(env, grow_names, names_only);
        auto db_src = open_file(game / lb::DATABASES);
        ef::Archive db_arc;
        ef::open(*db_src, db_arc, why);
        std::string dbtext = rewrite_to_string(*db_src, db_arc, names_only.changes[lb::DATABASES], why);
        std::ofstream(ov / "ov/databases.erp", std::ios::binary) << dbtext;
        lb::Env desync;
        desync.list = [&] { return list; };
        desync.open = [&](const std::string &rel) { return rel.rfind("ov/", 0) == 0 ? open_file(ov / rel) : open_file(game / rel); };
        desync.overlays = [&](const std::string &rel) {
            return rel == lb::DATABASES ? std::vector<std::string>{"ov/databases.erp"} : std::vector<std::string>{};
        };
        auto names_of = [&](const std::string &db_bytes) {
            content::uidb::Pkg q;
            q.read(db_bytes);
            auto *n = q.find(lb::list_path(array));
            return n ? n->list.size() : size_t(0);
        };
        auto stock_names = [&] {
            auto s2 = open_file(game / lb::DATABASES);
            ef::Archive a;
            std::string b;
            ef::open(*s2, a, why);
            ef::fragment(*s2, a, a.find(lb::OVERRIDES)->frags[0], b, why);
            return names_of(b);
        }();
        std::vector<lb::Want> fresh(1);
        fresh[0].array = array;
        fresh[0].key = "fresh";
        fresh[0].donor = "b";
        lb::Result rd;
        bool ok = lb::build(desync, fresh, rd);
        check(ok && rd.built == 1 && rd.refused == 0, "an array shorter than its key list gets the new slice instead of a refusal");
        bool said = std::any_of(rd.lines.begin(), rd.lines.end(), [](const std::string &l) {
            return l.find("lists ") != std::string::npos && l.find("another addon replaced the whole array") != std::string::npos &&
                   l.find("ghost0") != std::string::npos;
        });
        check(said, "the padding names the first key that has no image");
        uint32_t count = 0;
        for (auto &[rel, changes] : rd.changes) {
            auto it = changes.find(image);
            if (it != changes.end())
                count = content::logo_array::le32(*it->second[0], content::logo_array::HEAD_COUNT);
        }
        size_t names_after = 0;
        for (auto &[rel, changes] : rd.changes) {
            auto it = changes.find(lb::OVERRIDES);
            if (it != changes.end())
                names_after = names_of(*it->second[0]);
        }
        check(count && names_after == count && names_after == stock_names + 3,
              "the new key sits at its own slice: " + std::to_string(names_after) + " keys, " + std::to_string(count) + " slices, stock " +
                  std::to_string(stock_names));

        lb::Result nope;
        std::vector<lb::Want> gone(3);
        for (auto &g : gone) {
            g.array = "teams.nothing";
            g.donor = "b";
        }
        gone[0].key = "x0";
        gone[1].key = "x1";
        gone[2].key = "x2";
        lb::build(env, gone, nope);
        check(nope.refused == 3 && std::count_if(nope.lines.begin(), nope.lines.end(), [](const std::string &l) {
                  return l == "team.logo: 3 slot(s) refused: overrides.uidb has no list teams.nothing (first teams.nothing/x0)";
              }) == 1,
              "refusals for one reason log one line with the count and the first key");
    }

    {
        fs::path ov = out / "viewfix";
        fs::create_directories(ov / "ov");
        std::string array = "teams.driver_select_team_logos";
        std::string image = std::string(lb::ARRAY_URI) + array + ".texarray.image", view = std::string(lb::ARRAY_URI) + array + ".texarray";
        std::string stock_rel = list[1], over_rel = "ov/flow_a.erp";
        auto frag = [&](const fs::path &file, const std::string &name, size_t n) {
            auto src = open_file(file);
            ef::Archive arc;
            std::string bytes, w2;
            if (src && ef::open(*src, arc, w2)) {
                const ef::Entry *e = arc.find(name);
                if (e && e->frags.size() > n)
                    ef::fragment(*src, arc, e->frags[n], bytes, w2);
            }
            return bytes;
        };
        std::string stock_view = frag(game / stock_rel, view, 0);
        uint32_t slices = content::logo_array::le32(frag(game / stock_rel, image, 0), content::logo_array::HEAD_COUNT);
        std::string wide = stock_view;
        content::logo_array::put32(wide, content::logo_array::VIEW_COUNT, slices + 10);
        {
            auto src = open_file(game / stock_rel);
            ef::Archive arc;
            ef::open(*src, arc, why);
            ef::Changes edit;
            edit[view] = {wide};
            std::ofstream(ov / over_rel, std::ios::binary) << rewrite_to_string(*src, arc, edit, why);
        }
        lb::Env skewed;
        skewed.list = [&] { return list; };
        skewed.open = [&](const std::string &rel) { return rel.rfind("ov/", 0) == 0 ? open_file(ov / rel) : open_file(game / rel); };
        skewed.overlays = [&](const std::string &rel) {
            return rel == stock_rel ? std::vector<std::string>{over_rel} : std::vector<std::string>{};
        };
        auto view_count_in = [&](const lb::Result &r) {
            std::set<uint32_t> seen;
            for (auto &[rel, changes] : r.changes) {
                auto it = changes.find(view);
                if (it != changes.end())
                    seen.insert(content::logo_array::le32(*it->second[0], content::logo_array::VIEW_COUNT));
            }
            return seen;
        };
        auto head_count_in = [&](const lb::Result &r) {
            std::set<uint32_t> seen;
            for (auto &[rel, changes] : r.changes) {
                auto it = changes.find(image);
                if (it != changes.end())
                    seen.insert(content::logo_array::le32(*it->second[0], content::logo_array::HEAD_COUNT));
            }
            return seen;
        };
        std::vector<lb::Want> add(1);
        add[0].array = array;
        add[0].key = "viewfixed";
        add[0].donor = "b";
        lb::Result fixed;
        check(lb::build(skewed, add, fixed) && fixed.built == 1 && fixed.refused == 0,
              "an array whose view is ten above its slices is repaired and grown instead of refused");
        std::string said = "team.logo: " + array + " view said " + std::to_string(slices + 10) + " slices for " + std::to_string(slices) + "; fixed";
        check(std::count(fixed.lines.begin(), fixed.lines.end(), said) == 1, "the repair is logged once for the array");
        check(view_count_in(fixed) == std::set<uint32_t>{slices + 1} && head_count_in(fixed) == std::set<uint32_t>{slices + 1},
              "every archive gets the grown image and a view that matches it");

        std::vector<lb::Want> refused(1);
        refused[0].array = array;
        refused[0].key = "nokey";
        refused[0].donor = "no_such_stock_key";
        lb::Result left;
        lb::build(skewed, refused, left);
        check(left.refused == 1 && left.built == 0 && view_count_in(left) == std::set<uint32_t>{slices},
              "a refused slot still leaves the array with a view that matches its image");

        lb::Result untouched;
        lb::build(env, refused, untouched);
        check(untouched.changes.empty(),
              "a stock array that is already consistent is never rewritten for its view");
    }

    {
        auto bad = wants;
        bad[0].width = 32;
        lb::Result r2;
        lb::build(env, bad, r2);
        bool said = false;
        for (auto &l : r2.lines)
            said |= l == "team.logo: newteam scaled 32x64 -> 64x64 for teams.driver_select_team_logos";
        check(r2.refused == 0 && r2.built == 2 && said, "an image of another size is scaled to the slice and logged once");
        lb::Result r2x;
        lb::build(env, wants, r2x);
        check(r2x.built == 2 && std::none_of(r2x.lines.begin(), r2x.lines.end(), [](const std::string &l) { return l.find("scaled") != std::string::npos; }),
              "an image of the exact size is not scaled and nothing is logged");
        bad = wants;
        bad[1].donor = "zz";
        lb::Result r3;
        lb::build(env, bad, r3);
        check(r3.refused == 1, "a donor key that is not listed is refused");
        bad[1].fallback = true;
        lb::Result r3f;
        lb::build(env, bad, r3f);
        check(r3f.refused == 0, "a donor fallback whose key is not listed is left out without a refusal");
        bad = wants;
        bad[0].array = "teams.nothing";
        lb::Result r4;
        lb::build(env, bad, r4);
        check(r4.refused >= 1, "an array that is not in overrides.uidb is refused");
    }

    if (const char *g = std::getenv("F1_GAME")) {
        fs::path ui = fs::path(g) / "2025_asset_groups/ui_package";
        auto src = open_file(ui / "databases.erp");
        ef::Archive arc;
        check(src && ef::open(*src, arc, why) && rewrite_to_string(*src, arc, {}, why) == slurp(ui / "databases.erp"),
              "the game's databases.erp rewrites to the same bytes");
        const ef::Entry *e = arc.find(lb::OVERRIDES);
        std::string real;
        content::uidb::Pkg p;
        check(e && ef::fragment(*src, arc, e->frags[0], real, why) && p.read(real) && p.write() == real, "the game's overrides.uidb writes to the same bytes");
        std::vector<std::string> all;
        for (auto &it : fs::recursive_directory_iterator(ui))
            if (it.path().extension() == ".erp")
                all.push_back("2025_asset_groups/ui_package/" + fs::relative(it.path(), ui).generic_string());
        std::sort(all.begin(), all.end());
        lb::Env real_env;
        real_env.list = [&] { return all; };
        real_env.open = [&](const std::string &rel) { return open_file(fs::path(g) / rel); };
        std::vector<lb::Want> real_wants;
        for (auto *a : {"teams.driver_select_team_logos", "osd.timing_tower.team_badges_hud", "team_logos.driver_career_2024_team_select",
                        "teams.badges_broadcast"}) {
            lb::Want w;
            w.array = a;
            w.key = "w26_probe";
            w.donor = std::string(a) == "osd.timing_tower.team_badges_hud" ? "red_bull" : "williams";
            real_wants.push_back(w);
        }
        lb::Result rr;
        lb::build(real_env, real_wants, rr);
        for (auto &l : rr.lines)
            std::printf("      %s\n", l.c_str());
        check(rr.built == 4 && rr.refused == 0 && rr.changes.count(lb::DATABASES), "the game's own archives build four probe slices");
    }
    {
        fs::path ov = out / "stacked";
        fs::create_directories(ov / "ov");
        std::string array = "teams.driver_select_team_logos";
        std::string image = std::string(lb::ARRAY_URI) + array + ".texarray.image";
        auto frag_of = [&](const fs::path &file, const std::string &name, size_t n) {
            auto src = open_file(file);
            ef::Archive arc;
            std::string bytes, w2;
            if (src && ef::open(*src, arc, w2)) {
                const ef::Entry *e = arc.find(name);
                if (e && e->frags.size() > n)
                    ef::fragment(*src, arc, e->frags[n], bytes, w2);
            }
            return bytes;
        };
        auto names_in = [&](const std::string &db_bytes) {
            content::uidb::Pkg q;
            q.read(db_bytes);
            std::vector<std::string> names;
            if (auto *n = q.find(lb::list_path(array)))
                for (auto &e : n->list)
                    names.push_back(content::uidb::Pkg::text_of(e, "Name"));
            return names;
        };
        std::string stock_head = frag_of(game / list[1], image, 0), stock_data = frag_of(game / list[1], image, 1);
        uint32_t stock_count = content::logo_array::le32(stock_head, content::logo_array::HEAD_COUNT);
        std::string db_stock = frag_of(game / lb::DATABASES, lb::OVERRIDES, 0);
        std::vector<std::string> stock_names = names_in(db_stock);
        size_t slice = stock_data.size() / stock_count;
        check(stock_names.size() == stock_count && stock_names.size() >= 2, "the stacked fixture starts as a stock array of named slices");

        std::vector<lb::Want> others(3);
        for (size_t i = 0; i < others.size(); i++) {
            others[i].array = array;
            others[i].key = "addon_" + std::to_string(i);
            others[i].donor = stock_names[1];
        }
        lb::Result appended;
        lb::build(env, others, appended);
        for (auto &[rel, changes] : appended.changes) {
            if (rel == lb::DATABASES)
                continue;
            auto src = open_file(game / rel);
            ef::Archive arc;
            ef::open(*src, arc, why);
            std::string name = "ov/" + std::to_string(std::hash<std::string>{}(rel)) + ".erp";
            std::ofstream(ov / name, std::ios::binary) << rewrite_to_string(*src, arc, changes, why);
        }
        lb::Env stacked;
        stacked.list = [&] { return list; };
        stacked.open = [&](const std::string &rel) { return rel.rfind("ov/", 0) == 0 ? open_file(ov / rel) : open_file(game / rel); };
        stacked.overlays = [&](const std::string &rel) {
            return !appended.changes.count(rel) || rel == lb::DATABASES
                       ? std::vector<std::string>{}
                       : std::vector<std::string>{"ov/" + std::to_string(std::hash<std::string>{}(rel)) + ".erp"};
        };

        std::vector<lb::Want> teams(3);
        for (auto &t : teams)
            t.array = array;
        teams[0].key = "team_a";
        teams[0].donor = stock_names[0];
        teams[0].fallback = true;
        teams[1].key = "team_b";
        teams[1].donor = stock_names[1];
        teams[1].fallback = true;
        teams[2].key = "team_c";
        teams[2].donor = stock_names[0];
        teams[2].fallback = true;
        lb::Result res;
        bool ok = lb::build(stacked, teams, res);
        check(ok && res.built == 3 && res.refused == 0, "teams with a donor fallback build on an array other addons grew without keys");
        std::vector<std::string> names_out;
        std::string head_out, data_out;
        for (auto &[rel, changes] : res.changes) {
            auto n = changes.find(lb::OVERRIDES);
            if (n != changes.end())
                names_out = names_in(*n->second[0]);
            auto i = changes.find(image);
            if (i != changes.end()) {
                head_out = *i->second[0];
                data_out = *i->second[1];
            }
        }
        uint32_t count_out = head_out.size() >= 32 ? content::logo_array::le32(head_out, content::logo_array::HEAD_COUNT) : 0;
        check(count_out == stock_count + 3 + 3 && names_out.size() == count_out && data_out.size() == size_t(count_out) * slice,
              "the grown array has one key for every slice (" + std::to_string(names_out.size()) + " keys, " + std::to_string(count_out) + " slices)");
        for (auto &t : teams) {
            auto at = std::find(names_out.begin(), names_out.end(), t.key);
            size_t index = size_t(at - names_out.begin());
            auto donor = std::find(stock_names.begin(), stock_names.end(), t.donor);
            size_t donor_index = size_t(donor - stock_names.begin());
            check(at != names_out.end() && (index + 1) * slice <= data_out.size() &&
                      data_out.compare(index * slice, slice, stock_data, donor_index * slice, slice) == 0,
                  t.key + " reads its donor's stock slice " + std::to_string(donor_index) + " at slice " + std::to_string(index));
        }
        check(std::any_of(res.lines.begin(), res.lines.end(), [](const std::string &l) {
                  return l.find("placeholder key(s)") != std::string::npos;
              }) && std::any_of(res.lines.begin(), res.lines.end(), [](const std::string &l) {
                  return l.find("team_a = ") != std::string::npos || l.find("= team_a (copy of") != std::string::npos;
              }),
              "the build logs the placeholder keys and the slice every new key lands on");
    }
    {
        namespace la = content::logo_array;
        fs::path dir_out = out / "carriers";
        fs::create_directories(dir_out);
        std::string array = "teams.driver_select_team_logos";
        std::string image = std::string(lb::ARRAY_URI) + array + ".texarray.image", view = std::string(lb::ARRAY_URI) + array + ".texarray";
        auto frag_of = [&](const fs::path &file, const std::string &name, size_t n) {
            auto src = open_file(file);
            ef::Archive arc;
            std::string bytes, w2;
            if (src && ef::open(*src, arc, w2)) {
                const ef::Entry *e = arc.find(name);
                if (e && e->frags.size() > n)
                    ef::fragment(*src, arc, e->frags[n], bytes, w2);
            }
            return bytes;
        };
        auto names_in = [&](const std::string &db_bytes) {
            content::uidb::Pkg q;
            q.read(db_bytes);
            std::vector<std::string> names;
            if (auto *n = q.find(lb::list_path(array)))
                for (auto &e : n->list)
                    names.push_back(content::uidb::Pkg::text_of(e, "Name"));
            return names;
        };
        std::string head0 = frag_of(game / list[1], image, 0), data0 = frag_of(game / list[1], image, 1), view0 = frag_of(game / list[1], view, 0);
        uint32_t count0 = la::le32(head0, la::HEAD_COUNT), side = la::le32(head0, la::HEAD_FORMAT + 4);
        size_t slice = size_t(side) * la::le32(head0, la::HEAD_FORMAT + 8);
        std::vector<std::string> names0 = names_in(frag_of(game / lb::DATABASES, lb::OVERRIDES, 0));
        size_t donor_at = size_t(std::find(names0.begin(), names0.end(), "b") - names0.begin());
        check(names0.size() == count0 && count0 >= 3 && donor_at < count0 && data0.size() == count0 * slice,
              "the carriers fixture starts as one stock array of named slices with the donor key b");
        auto copy_with = [&](uint32_t count, const std::string &data, uint32_t format, uint32_t size) {
            std::string head = head0, vw = view0;
            la::put32(head, la::HEAD_COUNT, count);
            la::put32(head, la::HEAD_FORMAT, format);
            la::put32(head, la::HEAD_FORMAT + 4, size);
            la::put32(head, la::HEAD_FORMAT + 8, size);
            la::put32(vw, la::VIEW_COUNT, count);
            return std::vector<std::string>{head, data, vw};
        };
        auto write_copy = [&](const std::string &rel, const std::string &name, const std::vector<std::string> &frags) {
            auto src = open_file(game / rel);
            ef::Archive arc;
            ef::open(*src, arc, why);
            ef::Changes edit;
            edit[image] = {frags[0], frags[1]};
            edit[view] = {frags[2]};
            std::ofstream(dir_out / name, std::ios::binary) << rewrite_to_string(*src, arc, edit, why);
            return dir_out / name;
        };
        auto env_with = [&](const std::map<std::string, fs::path> &swapped) {
            lb::Env e;
            e.list = [&] { return list; };
            e.open = [&, swapped](const std::string &rel) {
                auto it = swapped.find(rel);
                return open_file(it != swapped.end() ? it->second : game / rel);
            };
            return e;
        };
        auto grown_of = [&](lb::Result &r, const std::string &rel, std::string &head, std::string &data, std::string &vw) {
            auto c = r.changes.find(rel);
            if (c == r.changes.end() || !c->second.count(image) || !c->second.count(view))
                return false;
            head = *c->second[image][0];
            data = *c->second[image][1];
            vw = *c->second[view][0];
            return true;
        };
        auto has_line = [](const lb::Result &r, std::initializer_list<std::string> parts) {
            return std::count_if(r.lines.begin(), r.lines.end(), [&](const std::string &l) {
                return std::all_of(parts.begin(), parts.end(), [&](const std::string &p) { return l.find(p) != std::string::npos; });
            });
        };
        std::vector<lb::Want> two(2);
        two[0] = {array, "carrier_img", "", std::string(px, 64 * 64 * 4), 64, 64};
        two[1].array = array;
        two[1].key = "carrier_copy";
        two[1].donor = "b";
        std::string zeros(slice * 4, '\0');
        std::string blank = content::bc7::encode_logo(reinterpret_cast<const uint8_t *>(zeros.data()), int(side), int(side));

        std::string short_data = data0.substr(0, (count0 - 1) * slice), long_data = data0 + std::string(slice, char(0x5a)) + std::string(slice, char(0xa5));
        auto short_copy = copy_with(count0 - 1, short_data, la::FORMAT_BC7, side), long_copy = copy_with(count0 + 2, long_data, la::FORMAT_BC7, side);
        lb::Env split = env_with({{list[1], write_copy(list[1], "short_flow_a.erp", short_copy)},
                                  {list[3], write_copy(list[3], "long_screens_b.erp", long_copy)}});
        lb::Result rs;
        check(lb::build(split, two, rs) && rs.built == 2 && rs.refused == 0,
              "two carriers whose copies hold different slice counts both build instead of refusing every slot");
        std::string ha, da, va, hb, db, vb;
        bool both = grown_of(rs, list[1], ha, da, va) && grown_of(rs, list[3], hb, db, vb);
        uint32_t total = count0 + 2 + 2;
        std::vector<std::string> names_out = rs.changes.count(lb::DATABASES) ? names_in(*rs.changes[lb::DATABASES][lb::OVERRIDES][0]) : std::vector<std::string>{};
        check(both && la::le32(ha, la::HEAD_COUNT) == total && la::le32(hb, la::HEAD_COUNT) == total && la::le32(va, la::VIEW_COUNT) == total &&
                  la::le32(vb, la::VIEW_COUNT) == total && da.size() == total * slice && db.size() == total * slice,
              "both copies grow to the same " + std::to_string(total) + " slices with views that match");
        check(names_out.size() == total && names_out[count0 + 2] == "carrier_img" && names_out[count0 + 3] == "carrier_copy" &&
                  names_out[count0].rfind("swofty_pad_", 0) == 0 && names_out[count0 + 1].rfind("swofty_pad_", 0) == 0,
              "the one key list names the extra slices and puts the new keys at slices " + std::to_string(count0 + 2) + " and " +
                  std::to_string(count0 + 3));
        check(both && da.compare(0, short_data.size(), short_data) == 0 && db.compare(0, long_data.size(), long_data) == 0,
              "each copy keeps its own existing slices byte for byte");
        check(both && da.compare((count0 - 1) * slice, slice, blank) == 0 && da.compare(count0 * slice, slice, blank) == 0 &&
                  da.compare((count0 + 1) * slice, slice, blank) == 0,
              "the shorter copy is padded with blank slices up to the longer copy's count");
        check(both && da.compare((count0 + 2) * slice, 2 * slice, db, (count0 + 2) * slice, 2 * slice) == 0 &&
                  db.compare((count0 + 3) * slice, slice, long_data, donor_at * slice, slice) == 0,
              "the new logos sit at the same slice in both copies and the donor copy is slice " + std::to_string(donor_at));
        check(has_line(rs, {"hold 2 different copies (" + std::to_string(count0 - 1) + ", " + std::to_string(count0 + 2) + " slices)"}) == 1 &&
                  has_line(rs, {"in 2025_asset_groups/ui_package/flow_a.erp lists ", "another addon replaced the whole array"}) == 1 &&
                  has_line(rs, {"(2 different copies)"}) == 1 && has_line(rs, {"refused"}) == 0,
              "the build logs the differing copies and the padding of the short one, and refuses nothing");

        std::string wide_data;
        for (uint32_t i = 0; i < count0; i++)
            wide_data += std::string(slice * 4, char(i + 1));
        lb::Env sized = env_with({{list[3], write_copy(list[3], "wide_screens_b.erp", copy_with(count0, wide_data, la::FORMAT_BC7, side * 2))}});
        lb::Result rw;
        check(lb::build(sized, two, rw) && rw.built == 2 && rw.refused == 0 && grown_of(rw, list[1], ha, da, va) && grown_of(rw, list[3], hb, db, vb) &&
                  da.size() == (count0 + 2) * slice && db.size() == (count0 + 2) * slice * 4 &&
                  db.compare((count0 + 1) * slice * 4, slice * 4, wide_data, donor_at * slice * 4, slice * 4) == 0 &&
                  has_line(rw, {"carrier_img scaled 64x64 -> " + std::to_string(side * 2)}) == 1,
              "a copy at another slice size gets the logo encoded at its own size and the donor slice from its own image");

        lb::Env odd = env_with({{list[1], write_copy(list[1], "bc1_flow_a.erp", copy_with(count0, data0, 71, side))}});
        lb::Result ro;
        check(lb::build(odd, two, ro) && ro.built == 2 && ro.refused == 0 && !ro.changes.count(list[1]) && grown_of(ro, list[3], hb, db, vb) &&
                  la::le32(hb, la::HEAD_COUNT) == count0 + 2,
              "a carrier in another format is left as it is and the other carrier still builds");
        check(has_line(ro, {"team.logo: skipped teams.driver_select_team_logos in 2025_asset_groups/ui_package/flow_a.erp: unexpected stock array (format 71",
                            "left as it is, the other 1 archive(s) build"}) == 1 &&
                  has_line(ro, {"skipped"}) == 1,
              "the skipped carrier is logged in one line");

        lb::Env none = env_with({{list[1], write_copy(list[1], "bc1_a.erp", copy_with(count0, data0, 71, side))},
                                 {list[3], write_copy(list[3], "bc1_b.erp", copy_with(count0, data0, 71, side))}});
        lb::Result rn;
        lb::build(none, two, rn);
        check(rn.built == 0 && rn.refused == 2 && rn.changes.empty() &&
                  has_line(rn, {"2 slot(s) refused: teams.driver_select_team_logos: unexpected stock array (format 71"}) == 1,
              "when no carrier's copy can grow the slots are refused with the reason");

        std::string boot_rel = lb::BOOT_ARCHIVE;
        std::vector<std::string> with_boot = list;
        with_boot.push_back(boot_rel);
        auto boot_env = [&](const std::map<std::string, fs::path> &swapped, const std::vector<std::string> &layers = {}) {
            lb::Env e;
            e.list = [&] { return with_boot; };
            e.open = [&, swapped](const std::string &rel) {
                auto it = swapped.find(rel);
                return open_file(it != swapped.end() ? it->second : game / rel);
            };
            e.overlays = [&, layers](const std::string &rel) { return rel == boot_rel ? layers : std::vector<std::string>{}; };
            return e;
        };
        std::string own_data = std::string(slice, char(0x11)) + data0.substr(slice);
        fs::path boot_same = write_copy(list[1], "boot_game_sized.erp", copy_with(count0, own_data, la::FORMAT_BC7, side));
        lb::Result plain, rb;
        lb::build(env, two, plain);
        check(lb::array_layers(boot_env({}), boot_rel).empty() &&
                  lb::array_layers(boot_env({{boot_rel, boot_same}}), boot_rel) == std::vector<std::string>{boot_rel} &&
                  lb::array_layers(boot_env({{"ov/boot.erp", boot_same}}, {"ov/boot.erp"}), boot_rel) == std::vector<std::string>{"ov/boot.erp"},
              "the boot archive is named only when it or one of its overlay layers carries a logo array");
        lb::Env absent = boot_env({});
        lb::Result ra;
        bool unchanged = lb::build(absent, two, ra) && ra.built == plain.built && ra.refused == plain.refused && ra.lines == plain.lines &&
                         ra.read == plain.read && ra.changes.size() == plain.changes.size();
        for (auto &[rel, changes] : plain.changes) {
            unchanged = unchanged && ra.changes.count(rel) && ra.changes[rel].size() == changes.size();
            for (auto &[name, frags] : changes) {
                unchanged = unchanged && ra.changes[rel].count(name) && ra.changes[rel][name].size() == frags.size();
                for (size_t i = 0; unchanged && i < frags.size(); i++)
                    unchanged = *ra.changes[rel][name][i] == *frags[i];
            }
        }
        check(unchanged, "with no boot copy the build writes the same bytes and lines as before");

        check(lb::build(boot_env({{boot_rel, boot_same}}), two, rb) && rb.built == 2 && rb.refused == 0 && grown_of(rb, boot_rel, ha, da, va) &&
                  grown_of(rb, list[1], hb, db, vb) && rb.read.count(boot_rel),
              "a game-sized boot copy builds with the ui_package copies");
        check(la::le32(ha, la::HEAD_COUNT) == count0 + 2 && la::le32(va, la::VIEW_COUNT) == count0 + 2 && da.size() == (count0 + 2) * slice &&
                  da.compare(0, own_data.size(), own_data) == 0 && da.compare(count0 * slice, 2 * slice, db, count0 * slice, 2 * slice) == 0 &&
                  db.compare(0, data0.size(), data0) == 0,
              "the boot copy keeps its own slices and gets the new logos at the slices the ui_package copy has them");
        names_out = names_in(*rb.changes[lb::DATABASES][lb::OVERRIDES][0]);
        check(names_out.size() == count0 + 2 && names_out[count0] == "carrier_img" && names_out[count0 + 1] == "carrier_copy" &&
                  has_line(rb, {"hold 2 different copies"}) == 1 && has_line(rb, {"grows by 2 slice(s) in 3 archive(s)"}) == 1,
              "the key list grows once and the build counts the boot archive");

        fs::path boot_long = write_copy(list[1], "boot_grown.erp", long_copy);
        lb::Result rg;
        check(lb::build(boot_env({{boot_rel, game / list[2]}, {"ov/boot.erp", boot_long}}, {"ov/boot.erp"}), two, rg) && rg.built == 2 && rg.refused == 0 &&
                  grown_of(rg, "ov/boot.erp", ha, da, va) && grown_of(rg, list[1], hb, db, vb) && !rg.changes.count(boot_rel) &&
                  la::le32(ha, la::HEAD_COUNT) == total && la::le32(hb, la::HEAD_COUNT) == total && la::le32(va, la::VIEW_COUNT) == total &&
                  da.compare(0, long_data.size(), long_data) == 0 && da.compare((count0 + 2) * slice, 2 * slice, db, (count0 + 2) * slice, 2 * slice) == 0,
              "a boot copy another addon grew in an overlay layer keeps its slices and both copies put the new logos at slice " +
                  std::to_string(count0 + 2));

        lb::Result rf;
        lb::Env behind = boot_env({{list[1], write_copy(list[1], "ui_long_a.erp", long_copy)},
                                   {list[3], write_copy(list[3], "ui_long_b.erp", long_copy)},
                                   {boot_rel, boot_same}});
        check(lb::build(behind, two, rf) && rf.built == 2 && rf.refused == 0 && grown_of(rf, boot_rel, ha, da, va) && grown_of(rf, list[1], hb, db, vb) &&
                  la::le32(ha, la::HEAD_COUNT) == total && la::le32(va, la::VIEW_COUNT) == total && da.size() == total * slice &&
                  da.compare(0, own_data.size(), own_data) == 0 && da.compare(count0 * slice, 4 * slice, db, count0 * slice, 4 * slice) == 0,
              "a boot copy shorter than the ui_package copies takes their slices for the keys it lacks and grows to all " + std::to_string(total));
        check(has_line(rf, {"in " + boot_rel + " holds " + std::to_string(count0) + " slice(s) for " + std::to_string(count0 + 2) + " key(s)",
                            "are copied from " + list[1]}) == 1 &&
                  has_line(rf, {"padded with blank"}) == 0,
              "the fill of the short boot copy is logged in one line with the archive and the counts");

        std::string viewless = slurp(boot_same), view_name = view + std::string(1, '\0');
        size_t name_at = viewless.find(view_name);
        check(name_at != std::string::npos && viewless.find(view_name, name_at + 1) == std::string::npos, "the fixture names the view once");
        viewless[name_at + view.size() - 1] = 'x';
        std::ofstream(dir_out / "boot_viewless.erp", std::ios::binary) << viewless;
        lb::Result rv;
        bool built = lb::build(boot_env({{boot_rel, dir_out / "boot_viewless.erp"}}), two, rv) && rv.built == 2 && rv.refused == 0;
        auto bc = rv.changes.find(boot_rel);
        check(built && bc != rv.changes.end() && bc->second.count(image) && !bc->second.count(view) &&
                  la::le32(*bc->second[image][0], la::HEAD_COUNT) == count0 + 2 && bc->second[image][1]->size() == (count0 + 2) * slice &&
                  bc->second[image][1]->compare(0, own_data.size(), own_data) == 0 && grown_of(rv, list[1], hb, db, vb) &&
                  la::le32(vb, la::VIEW_COUNT) == count0 + 2,
              "a boot image with no view of its own grows whole to the count of the ui_package view the game reads with it");
        check(has_line(rv, {"in " + boot_rel + " is an image of " + std::to_string(count0) + " slice(s) with no view"}) == 1 &&
                  has_line(rv, {"skipped"}) == 0,
              "the borrowed view is logged in one line with the archive and the count");

        lb::Result rx;
        lb::build(boot_env({{boot_rel, write_copy(list[1], "boot_bc1.erp", copy_with(count0, data0, 71, side))}}), two, rx);
        check(rx.built == 2 && rx.refused == 0 && !rx.changes.count(boot_rel) && grown_of(rx, list[1], hb, db, vb) &&
                  has_line(rx, {"skipped teams.driver_select_team_logos in " + boot_rel + ": unexpected stock array (format 71"}) == 1,
              "a boot copy this build cannot grow is left whole and named in one line");
    }
    return failures ? 1 : 0;
}
