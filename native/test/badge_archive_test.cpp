#ifndef NOMINMAX
#define NOMINMAX
#endif
#include "../src/core/badge_archive.hpp"
#include <cassert>
#include <cstdio>
#include <fstream>

static void write_erp(const std::filesystem::path &path, const std::string &value)
{
    std::string name = "eaid://ui_package/test.uidb";
    auto put = [](std::string &s, auto value) { s.append(reinterpret_cast<const char *>(&value), sizeof value); };
    std::string entry(4, '\0');
    put(entry, uint16_t(name.size() + 1));
    entry += name;
    entry += '\0';
    entry.append(20, '\0');
    entry += '\1';
    entry += "temp";
    put(entry, uint64_t(0));
    put(entry, uint64_t(value.size()));
    put(entry, uint32_t(16));
    entry += '\x10';
    put(entry, uint64_t(value.size()));
    entry.append(16, '\0');
    const uint32_t length = uint32_t(entry.size() - 4);
    std::memcpy(entry.data(), &length, 4);
    std::string index;
    put(index, uint32_t(1));
    put(index, uint32_t(1));
    index += entry;
    std::string bytes = "ERPK";
    put(bytes, uint32_t(3));
    bytes.append(8, '\0');
    put(bytes, uint64_t(48));
    put(bytes, uint64_t(index.size()));
    put(bytes, uint64_t(48 + index.size()));
    bytes.append(8, '\0');
    bytes += index;
    bytes += value;
    std::ofstream file(path, std::ios::binary);
    file.write(bytes.data(), std::streamsize(bytes.size()));
    assert(file.good());
}

static void source_selection()
{
    using namespace badge_images;
    namespace fs = std::filesystem;
    const auto temp = fs::canonical(fs::temp_directory_path());
    const auto root = temp / ("swofty-live-image-test-" + std::to_string(GetCurrentProcessId()) + "-" +
                              std::to_string(GetTickCount64()));
    fs::create_directories(root / "2025_asset_groups/ui_package");
    fs::create_directories(root / "swofty");
    write_erp(root / "2025_asset_groups/ui_package/a.erp", "stock");
    write_erp(root / "swofty/active.erp", "redirected");
    write_erp(root / "swofty/overlay.erp", "overlay");
    archive::Env env;
    env.root = root;
    auto read = [&](const std::string &expected)
    {
        archive::Resources resources(env);
        std::vector<std::string> data;
        return resources.read("eaid://ui_package/test.uidb", data) && data == std::vector<std::string>{expected};
    };
    assert(read("stock"));
    env.routes["2025_asset_groups/ui_package/a.erp"] = "swofty/active.erp";
    assert(read("redirected"));
    env.routes.clear();
    write_erp(root / "2025_asset_groups/ui_package/b.erp", "conflicting");
    {
        archive::Resources resources(env);
        std::vector<std::string> data;
        assert(!resources.read("eaid://ui_package/test.uidb", data));
    }
    env.overlays = {"swofty/overlay.erp"};
    assert(read("overlay"));
    env.routes["swofty/overlay.erp"] = "swofty/active.erp";
    assert(read("redirected"));
    env.routes["swofty/overlay.erp"] = "swofty/../../outside.erp";
    {
        archive::Resources resources(env);
        std::vector<std::string> data;
        assert(!resources.read("eaid://ui_package/test.uidb", data));
    }
    // Resolve and check the exact temporary fixture before removing its tree.
    const auto resolved = fs::canonical(root);
    assert(resolved.parent_path() == temp && resolved.filename().wstring().starts_with(L"swofty-live-image-test-"));
    fs::remove_all(resolved);
    std::puts("PASS: active redirects, accepted overlay copies, conflicting stock and escaping routes");
}

int main(int argc, char **argv)
{
    setvbuf(stdout, nullptr, _IONBF, 0);
    source_selection();
    using namespace badge_images;
    std::vector<std::string> fragments(2);
    fragments[0].resize(40);
    fragments[1].resize(32);
    for (auto [at, v] : {std::pair{8, 55u}, {12, 4u}, {16, 4u}, {24, 1u}, {28, 2u}})
        std::memcpy(fragments[0].data() + at, &v, 4);
    fragments[1][9] = char(-8);
    fragments[1][11] = char(-8);
    fragments[1][16] = char(-1);
    fragments[1][17] = char(-1);
    fragments[1][24] = char(-32);
    fragments[1][25] = 7;
    fragments[1][26] = char(-32);
    fragments[1][27] = 7;
    archive::Surface surface;
    assert(surface.load(fragments));
    archive::Pixels pixels;
    assert(surface.decode(0, pixels) && pixels.width == 4 && pixels.height == 4);
    for (size_t p = 0; p < pixels.rgba.size(); p += 4)
        assert(pixels.rgba[p] == 255 && pixels.rgba[p + 3] == 0);
    assert(surface.decode(1, pixels));
    for (size_t p = 0; p < pixels.rgba.size(); p += 4)
        assert(pixels.rgba[p + 1] == 255 && pixels.rgba[p + 3] == 255);
    assert(!surface.decode(2, pixels));
    fragments[1].pop_back();
    assert(!surface.load(fragments));
    assert(!archive::safe("../a.erp") && !archive::safe("C:/a.erp") && !archive::safe("swofty/../../a.erp"));
    std::puts("PASS: native archive slice selection, alpha, malformed dimensions and safe paths");
    if (argc == 2)
    {
        archive::Env env;
        env.root = argv[1];
        for (const auto &entry :
             std::filesystem::recursive_directory_iterator(env.root / "2025_asset_groups/ui_package"))
        {
            if (entry.path().extension() != ".erp")
                continue;
            archive::Open file;
            if (!file.read(entry.path()))
                std::printf("OPEN FAIL %s: %s\n", entry.path().string().c_str(), file.why.c_str());
        }
        archive::Resources resources(env);
        std::printf("resources valid=%d count=%zu\n", int(resources.available()), resources.count());
        std::printf("failed path=%s\n", resources.failure.c_str());
        std::vector<std::string> parts;
        const auto read = resources.read(archive::DATABASE, parts);
        std::printf("database=%d fragments=%zu\n", int(read), parts.size());
        if (read)
            for (auto &part : parts)
                std::printf("fragment=%zu magic=%02x %02x %02x %02x\n", part.size(), uint8_t(part[0]), uint8_t(part[1]),
                            uint8_t(part[2]), uint8_t(part[3]));
        archive::Catalogue catalogue;
        const auto loaded = catalogue.load(resources);
        std::printf("catalogue=%d indices=%zu size=%ux%u layers=%u\n", int(loaded), catalogue.indices.size(),
                    catalogue.surface.width, catalogue.surface.height, catalogue.surface.layers);
        assert(loaded);
        unsigned count = 0;
        for (auto &[key, index] : catalogue.indices)
        {
            assert(index >= 0);
            assert(catalogue.decode(std::string(archive::PREFIX) + key, pixels));
            std::printf("decoded %s %ux%u\n", key.c_str(), pixels.width, pixels.height);
            char golden[4]{};
            if (GetEnvironmentVariableA("SWOFTY_BADGE_GOLDEN",golden,sizeof golden))
            {
                std::printf("RGBA %s %u %u ", key.c_str(), pixels.width, pixels.height);
                for (auto byte : pixels.rgba)
                    std::printf("%02x", unsigned(byte));
                std::puts("");
            }
            ++count;
        }
        std::printf("PASS: %u installed catalogue images decoded without extraction\n", count);
    }
}
