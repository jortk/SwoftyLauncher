#pragma once
#ifdef _MSC_VER
#pragma warning(push)
#pragma warning(disable : 4310 4458) // existing ERP/UIDB reader warnings
#endif
#include "apply/content/erp_file.hpp"
#include "apply/content/uidb.hpp"
#ifdef _MSC_VER
#pragma warning(pop)
#endif
#include "badge_image_data.hpp"
#include <filesystem>
#include <windows.h>
#include <map>
#include <memory>
#include <set>

#ifdef _MSC_VER
#pragma warning(push, 0)
#endif
#define BCDEC_STATIC
#define BCDEC_IMPLEMENTATION
#include "../../third_party/bcdec/bcdec.h"
#ifdef _MSC_VER
#pragma warning(pop)
#endif

// Core-side interpretation of Live team_badge_texture references. No UI or
// table policy. Reads existing archives only, following the active router.
namespace badge_images::archive
{
namespace erp = swofty_core::apply::content::erpfile;
namespace uidb = swofty_core::apply::content::uidb;
inline constexpr const char *PREFIX = "#overrides.osd.timing_tower.team_badges_hud.";
inline constexpr const char *DATABASE = "eaid://ui_package/databases/overrides.uidb";
inline constexpr const char *ARRAY =
    "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.osd.timing_tower.team_badges_hud.texarray.image";
struct Pixels
{
    unsigned width{}, height{};
    std::vector<uint8_t> rgba;
};
struct Env
{
    std::filesystem::path root;
    std::map<std::string, std::string> routes;
    std::vector<std::string> overlays;
};
inline bool safe(const std::string &s)
{
    if (s.empty() || s.front() == '/' || s.find_first_of(":\\") != s.npos || !s.ends_with(".erp"))
        return false;
    size_t at = 0;
    while (at < s.size())
    {
        auto end = s.find('/', at);
        auto part = s.substr(at, end == s.npos ? s.size() - at : end - at);
        if (part.empty() || part == "." || part == ".." ||
            std::any_of(part.begin(), part.end(), [](unsigned char c) { return c < 32; }))
            return false;
        if (end == s.npos)
            break;
        at = end + 1;
    }
    return true;
}
struct Open
{
    HANDLE file = INVALID_HANDLE_VALUE;
    erp::Source src;
    erp::Archive arc;
    std::string why;
    Open() = default;
    Open(const Open &) = delete;
    Open &operator=(const Open &) = delete;
    ~Open()
    {
        if (file != INVALID_HANDLE_VALUE)
            CloseHandle(file);
    }
    bool read(const std::filesystem::path &path)
    {
        file = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING,
                           FILE_FLAG_RANDOM_ACCESS, nullptr);
        LARGE_INTEGER size{};
        if (file == INVALID_HANDLE_VALUE || !GetFileSizeEx(file, &size) || size.QuadPart < 0)
            return false;
        src.size = uint64_t(size.QuadPart);
        src.read = [this](uint64_t off, size_t n, char *out)
        {
            if (off > src.size || n > src.size - off)
                return false;
            LARGE_INTEGER at{};
            at.QuadPart = LONGLONG(off);
            DWORD got{};
            return n <= MAXDWORD && SetFilePointerEx(file, at, nullptr, FILE_BEGIN) &&
                   ReadFile(file, out, DWORD(n), &got, nullptr) && got == n;
        };
        return erp::open(src, arc, why);
    }
    bool resource(const std::string &name, std::vector<std::string> &out)
    {
        auto entry = arc.find(name);
        if (!entry)
            return false;
        std::vector<std::string> result;
        if (entry->frags.size() > 8)
            return false;
        for (auto &f : entry->frags)
        {
            if (f.size > (64u << 20) || f.packed > (64u << 20) || f.off > src.size || arc.data_off > src.size - f.off ||
                f.packed > src.size - arc.data_off - f.off)
                return false;
            std::string bytes, reason;
            if (!erp::fragment(src, arc, f, bytes, reason) || bytes.size() != f.size)
                return false;
            result.push_back(std::move(bytes));
        }
        out = std::move(result);
        return true;
    }
};

class Resources
{
    std::vector<std::unique_ptr<Open>> stock, overlays;
    std::unique_ptr<Open> boot;
    bool valid = true;
    static std::unique_ptr<Open> open(const Env &env, const std::string &rel)
    {
        if (!safe(rel))
            return {};
        auto target = rel;
        auto route = env.routes.find(rel);
        if (route != env.routes.end())
        {
            target = route->second;
            if (!safe(target) || !target.starts_with("swofty/"))
                return {};
            std::error_code ec;
            auto root = std::filesystem::weakly_canonical(env.root / "swofty", ec);
            if (ec)
                return {};
            auto resolved = std::filesystem::canonical(env.root / target, ec);
            if (ec)
                return {};
            auto relative = resolved.lexically_relative(root);
            if (relative.empty() || relative.is_absolute() || *relative.begin() == "..")
                return {};
        }
        auto file = std::make_unique<Open>();
        if (!file->read(env.root / target))
            return {};
        return file;
    }

  public:
    std::string failure;
    bool available() const
    {
        return valid;
    }
    size_t count() const
    {
        return stock.size();
    }
    explicit Resources(const Env &env)
    {
        std::vector<std::string> paths;
        std::error_code ec;
        const auto folder = env.root / "2025_asset_groups/ui_package";
        for (std::filesystem::recursive_directory_iterator it(folder, ec), end; !ec && it != end; it.increment(ec))
        {
            if (it.depth() > 4)
            {
                it.disable_recursion_pending();
                continue;
            }
            if (it->is_regular_file(ec) && it->path().extension() == ".erp")
                paths.push_back(it->path().lexically_relative(env.root).generic_string());
            if (paths.size() > 8192)
            {
                valid = false;
                return;
            }
        }
        if (ec)
        {
            valid = false;
            return;
        }
        std::sort(paths.begin(), paths.end());
        for (auto &path : paths)
        {
            auto f = open(env, path);
            if (!f)
            {
                failure = path;
                valid = false;
                return;
            }
            stock.push_back(std::move(f));
        }
        for (auto &path : env.overlays)
        {
            auto f = open(env, path);
            if (!f)
            {
                valid = false;
                return;
            }
            overlays.push_back(std::move(f));
        }
        const std::string bootpath = "2025_asset_groups/vfx_package/common.erp";
        if (env.routes.contains(bootpath))
        {
            boot = open(env, bootpath);
            if (!boot)
                valid = false;
        }
    }
    bool read(const std::string &name, std::vector<std::string> &result)
    {
        if (!valid)
            return false;
        for (auto it = overlays.rbegin(); it != overlays.rend(); ++it)
        {
            if ((*it)->arc.find(name))
                return (*it)->resource(name, result);
        }
        bool found = false;
        for (auto &file : stock)
        {
            if (!file->arc.find(name))
                continue;
            std::vector<std::string> bytes;
            if (!file->resource(name, bytes))
                return false;
            if (found && bytes != result)
                return false;
            result = std::move(bytes);
            found = true;
        }
        if (!found && boot && boot->arc.find(name))
            return boot->resource(name, result);
        return found;
    }
};

struct Surface
{
    unsigned format{}, width{}, height{}, layers{};
    size_t stride{}, block{};
    std::string bytes;
    bool load(const std::vector<std::string> &fragments)
    {
        if (fragments.size() < 2 || fragments[0].size() < 32)
            return false;
        auto &h = fragments[0];
        format = erp::rd32(h, 8);
        width = erp::rd32(h, 12);
        height = erp::rd32(h, 16);
        const auto levels = erp::rd32(h, 24);
        layers = erp::rd32(h, 28);
        if (!width || !height || width > 512 || height > 512 || !layers || layers > 4096 || !levels || levels > 32)
            return false;
        if (format == 52 || format == 54)
            block = 8;
        else if (format == 55 || format == 57 || format == 65 || format == 69 || format == 70)
            block = 16;
        else
            return false;
        stride = 0;
        for (unsigned level = 0; level < levels; level++)
            stride +=
                size_t((std::max(1u, width >> level) + 3) / 4) * ((std::max(1u, height >> level) + 3) / 4) * block;
        if (stride * layers > (64u << 20) || fragments[1].size() != stride * layers)
            return false;
        bytes = fragments[1];
        return true;
    }
    bool decode(size_t index, Pixels &out) const
    {
        if (index >= layers || !stride || bytes.size() != stride * layers)
            return false;
        std::vector<uint8_t> rgba(size_t(width) * height * 4);
        size_t offset = index * stride;
        for (unsigned y = 0; y < height; y += 4)
            for (unsigned x = 0; x < width; x += 4)
            {
                alignas(16) uint8_t decoded[64]{};
                auto src = bytes.data() + offset;
                offset += block;
                if (format == 52 || format == 54)
                    bcdec_bc1(src, decoded, 16);
                else if (format == 55 || format == 57)
                    bcdec_bc3(src, decoded, 16);
                else if (format == 65)
                {
                    uint8_t rg[32]{};
                    bcdec_bc5(src, rg, 8);
                    for (unsigned p = 0; p < 16; p++)
                    {
                        decoded[p * 4] = rg[p * 2];
                        decoded[p * 4 + 1] = rg[p * 2 + 1];
                        decoded[p * 4 + 3] = 255;
                    }
                }
                else
                    bcdec_bc7(src, decoded, 16);
                for (unsigned by = 0; by < 4 && y + by < height; by++)
                    for (unsigned bx = 0; bx < 4 && x + bx < width; bx++)
                        std::memcpy(&rgba[(size_t(y + by) * width + x + bx) * 4], &decoded[(by * 4 + bx) * 4], 4);
            }
        out.rgba = data::pixels(rgba.data(), width, height, width * 4, false, out.width, out.height);
        return !out.rgba.empty();
    }
};

struct Catalogue
{
    std::map<std::string, int> indices;
    Surface surface;
    bool load(Resources &resources)
    {
        std::vector<std::string> fragments;
        if (!resources.read(DATABASE, fragments) || fragments.empty() || fragments[0].size() > (16u << 20))
            return false;
        uidb::Pkg pkg;
        if (!pkg.read(fragments[0]))
            return false;
        const auto *node = &pkg.root;
        for (const char *key :
             {"overrides", "Contents", "osd", "Contents", "timing_tower", "Contents", "team_badges_hud", "Contents"})
        {
            if (node->kind != uidb::Node::object)
                return false;
            auto it =
                std::find_if(node->items.begin(), node->items.end(), [&](auto &p) { return p.first.value == key; });
            if (it == node->items.end())
                return false;
            node = &it->second;
        }
        if (node->kind != uidb::Node::array || node->list.size() > 4096)
            return false;
        for (size_t index = 0; index < node->list.size(); index++)
        {
            auto &row = node->list[index];
            if (row.kind != uidb::Node::object)
                continue;
            for (auto &[key, value] : row.items)
                if (key.value == "Name" && value.kind == uidb::Node::value && value.val.type == "stri")
                {
                    auto [it, inserted] = indices.emplace(value.val.value, int(index));
                    if (!inserted)
                        it->second = -1;
                }
        }
        return resources.read(ARRAY, fragments) && surface.load(fragments);
    }
    bool decode(const std::string &reference, Pixels &out) const
    {
        if (!reference.starts_with(PREFIX))
            return false;
        auto it = indices.find(reference.substr(std::strlen(PREFIX)));
        return it != indices.end() && it->second >= 0 && surface.decode(size_t(it->second), out);
    }
};
} // namespace badge_images::archive
