#include "logos.hpp"

#include "../files_api.hpp"
#include "../../runtime.hpp"
#include "../../badge_images.hpp"
#include "logo_build.hpp"
#include "logo_cache.hpp"
#include "logo_deliver.hpp"
#include "png_straight.hpp"

#include <swofty/mixin.hpp>

#include <algorithm>
#include <map>
#include <memory>
#include <set>

namespace swofty_core::apply::content {

namespace {

constexpr const char *CACHE_ADDON = "swofty.logos";
constexpr const char *ROUTE_ID = "logos";

struct FileSource {
    HANDLE h = INVALID_HANDLE_VALUE;
    erpfile::Source src;
    ~FileSource()
    {
        if (h != INVALID_HANDLE_VALUE)
            CloseHandle(h);
    }
};

std::wstring game_file(const std::string &rel)
{
    std::string p = rel;
    std::replace(p.begin(), p.end(), '/', '\\');
    return runtime::game_root() + L"\\" + runtime::wide(p);
}

void list_erps(const std::wstring &dir, const std::string &rel, std::vector<std::string> &out, unsigned depth)
{
    if (depth > 4)
        return;
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"*").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE)
        return;
    do {
        std::wstring n = fd.cFileName;
        if (n == L"." || n == L"..")
            continue;
        std::string r = rel + runtime::narrow(n);
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
            list_erps(dir + n + L"\\", r + "/", out, depth + 1);
        else if (r.size() > 4 && runtime::lower(r.substr(r.size() - 4)) == ".erp")
            out.push_back(r);
    } while (FindNextFileW(h, &fd));
    FindClose(h);
    std::sort(out.begin(), out.end());
}

using logo_cache::stamp;

bool stat_file(const std::string &rel, uint64_t &size, uint64_t &time)
{
    WIN32_FILE_ATTRIBUTE_DATA a{};
    if (!GetFileAttributesExW(game_file(rel).c_str(), GetFileExInfoStandard, &a))
        return false;
    size = (uint64_t(a.nFileSizeHigh) << 32) | a.nFileSizeLow;
    time = (uint64_t(a.ftLastWriteTime.dwHighDateTime) << 32) | a.ftLastWriteTime.dwLowDateTime;
    return true;
}

bool hash_file(const std::string &rel, uint64_t &hash)
{
    HANDLE h = CreateFileW(game_file(rel).c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, FILE_FLAG_SEQUENTIAL_SCAN, nullptr);
    if (h == INVALID_HANDLE_VALUE)
        return false;
    std::vector<char> buf(size_t(1) << 20);
    logo_cache::Bytes bytes;
    DWORD got = 0;
    bool ok = true;
    while ((ok = ReadFile(h, buf.data(), DWORD(buf.size()), &got, nullptr)) && got)
        bytes.add(buf.data(), got);
    CloseHandle(h);
    if (ok)
        hash = bytes.done();
    return ok;
}

std::string flat(const std::string &rel)
{
    std::string folder = logo_build::UI_FOLDER;
    std::string s = rel.compare(0, folder.size(), folder) == 0 ? rel.substr(folder.size()) : rel;
    std::replace(s.begin(), s.end(), '/', '_');
    return s;
}

std::string swofty_base()
{
    std::string root = runtime::narrow(runtime::game_root()), sw = runtime::narrow(runtime::swofty_root());
    if (runtime::lower(sw).rfind(runtime::lower(root), 0) != 0)
        return {};
    std::string base = sw.substr(root.size());
    std::replace(base.begin(), base.end(), '\\', '/');
    while (!base.empty() && base.front() == '/')
        base.erase(base.begin());
    if (!base.empty() && base.back() != '/')
        base += '/';
    return base;
}

std::map<std::string, std::vector<std::string>> overlay_archives(const std::string &base)
{
    std::map<std::string, std::vector<std::string>> out;
    if (base.empty())
        return out;
    for (auto &j : runtime::config()["resource_erps"].a) {
        std::string file = j["file"].str();
        if (file.empty())
            continue;
        std::replace(file.begin(), file.end(), '\\', '/');
        std::string rel = base + file;
        for (auto &x : j["erps"].a)
            if (x.type == json::Value::Str) {
                auto &list = out[runtime::lower(x.s)];
                if (std::find(list.begin(), list.end(), rel) == list.end())
                    list.push_back(rel);
            }
    }
    return out;
}

}

void deliver_logos(const std::vector<LogoImage> &logos, LogoReport &report)
{
    std::vector<logo_build::Want> wants;
    std::set<std::string> seen;
    logo_build::Refusals refusals;
    uint64_t key = stamp(0xcbf29ce484222325ull, logo_cache::CACHE_FORMAT);
    for (auto &l : logos) {
        if (!seen.insert(l.array + "|" + l.key).second)
            continue;
        logo_build::Want w;
        w.array = l.array;
        w.key = l.key;
        key = stamp(key, l.array + "|" + l.key + "|" + l.stock + (l.fallback ? "|f" : ""));
        if (l.stock.empty()) {
            std::string bytes = runtime::read_file(runtime::under_swofty("addons/" + l.addon + "/" + l.file), size_t(64) << 20), why;
            if (bytes.empty() || !png::decode(bytes, w.rgba, w.width, w.height, why)) {
                refusals.add(l.context + " image does not decode: " + (why.empty() ? "it is empty" : why), l.id + " " + l.file);
                report.refused++;
                continue;
            }
            key = stamp(key, bytes);
        } else {
            w.donor = l.stock;
            w.fallback = l.fallback;
        }
        wants.push_back(std::move(w));
    }
    refusals.flush(report.lines);
    if (wants.empty())
        return;
    std::vector<std::string> files;
    list_erps(game_file(logo_build::UI_FOLDER), logo_build::UI_FOLDER, files, 0);
    std::string base = swofty_base();
    auto overlays = overlay_archives(base);
    std::set<std::string> overlay_rels;
    for (auto &[target, list] : overlays)
        overlay_rels.insert(list.begin(), list.end());
    logo_build::Env env;
    env.list = [&] { return files; };
    env.overlays = [&](const std::string &rel) {
        auto it = overlays.find(runtime::lower(rel));
        return it == overlays.end() ? std::vector<std::string>{} : it->second;
    };
    env.open = [&](const std::string &rel) -> std::shared_ptr<erpfile::Source> {
        auto fs = std::make_shared<FileSource>();
        fs->h = CreateFileW(game_file(rel).c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, 0, nullptr);
        LARGE_INTEGER size;
        if (fs->h == INVALID_HANDLE_VALUE || !GetFileSizeEx(fs->h, &size))
            return nullptr;
        fs->src.size = uint64_t(size.QuadPart);
        HANDLE h = fs->h;
        fs->src.read = [h](uint64_t off, size_t n, char *out) {
            LARGE_INTEGER at;
            at.QuadPart = LONGLONG(off);
            DWORD got = 0;
            return SetFilePointerEx(h, at, nullptr, FILE_BEGIN) && ReadFile(h, out, DWORD(n), &got, nullptr) && got == n;
        };
        return std::shared_ptr<erpfile::Source>(fs, &fs->src);
    };
    std::vector<std::string> boot = logo_build::array_layers(env, logo_build::BOOT_ARCHIVE);
    if (!boot.empty()) {
        files.push_back(logo_build::BOOT_ARCHIVE);
        report.lines.push_back("team.logo: the boot archive " + std::string(logo_build::BOOT_ARCHIVE) + " carries logo arrays in " + boot[0] +
                               (boot.size() > 1 ? " and " + std::to_string(boot.size() - 1) + " more layer(s)" : "") +
                               "; its copies get the same slices as the ui_package ones");
    }
    std::vector<std::string> keyed_files = files;
    for (auto &rel : files) {
        auto it = overlays.find(runtime::lower(rel));
        if (it != overlays.end())
            keyed_files.insert(keyed_files.end(), it->second.begin(), it->second.end());
    }
    std::string dir = "addons/" + std::string(CACHE_ADDON) + "/";
    std::wstring dirw = runtime::under_swofty("addons");
    CreateDirectoryW(dirw.c_str(), nullptr);
    CreateDirectoryW(runtime::under_swofty(dir).c_str(), nullptr);
    std::wstring stamps_path = runtime::under_swofty(dir + logo_cache::STAMPS);
    DeleteFileW(runtime::under_swofty(dir + logo_cache::OLD_STAMPS).c_str());
    std::string stamps_before = runtime::read_file(stamps_path, size_t(16) << 20);
    logo_cache::Env stamp_env{stat_file, hash_file};
    auto key_inputs = [&](const std::set<std::string> &inputs, const logo_cache::Stamps &before) {
        DWORD t0 = GetTickCount();
        logo_cache::Keyed k = logo_cache::key_archives(key, keyed_files, inputs, stamp_env, before);
        if (k.hashed)
            report.lines.push_back("team.logo: " + std::to_string(k.hashed) + " input archive(s) (" + std::to_string(k.hashed_bytes >> 20) +
                                   " MB) hashed in " + std::to_string(GetTickCount() - t0) + " ms");
        return k;
    };
    logo_cache::Stamps before = logo_cache::parse(stamps_before);
    std::set<std::string> inputs = logo_cache::inputs_of(before);
    inputs.insert(logo_build::DATABASES);
    logo_cache::Keyed keyed = key_inputs(inputs, before);
    std::string id = runtime::hex16(keyed.key).substr(2);
    std::string manifest_name = "logos_" + id + ".manifest";
    std::string manifest = runtime::read_file(runtime::under_swofty(dir + manifest_name), size_t(1) << 20);
    logo_cache::Routes routes;
    int cached_built = -1;
    if (!manifest.empty())
        logo_cache::manifest_parse(manifest, [&](const std::string &file) {
            return GetFileAttributesW(runtime::under_swofty(dir + file).c_str()) != INVALID_FILE_ATTRIBUTES;
        }, cached_built, routes);
    struct Built {
        std::vector<std::string> lines;
        int built = 0, refused = 0;
        size_t changes = 0;
        DWORD t0 = 0;
    } made;
    bool layered = false;
    auto make = [&](const std::set<std::string> &dropped) {
        logo_deliver::Attempt a;
        made = {};
        made.t0 = GetTickCount();
        DeleteFileW(runtime::under_swofty(dir + manifest_name).c_str());
        std::vector<logo_build::Want> kept;
        for (auto &w : wants)
            if (dropped.count(w.array))
                made.refused++;
            else
                kept.push_back(w);
        logo_build::Result res;
        logo_build::build(env, kept, res);
        made.lines = res.lines;
        made.refused += res.refused;
        made.built = res.built;
        made.changes = res.changes.size();
        std::set<std::string> read = res.read;
        read.insert(logo_build::DATABASES);
        if (read != inputs) {
            inputs = read;
            keyed = key_inputs(read, keyed.stamps);
            id = runtime::hex16(keyed.key).substr(2);
            manifest_name = "logos_" + id + ".manifest";
        }
        layered = false;
        for (auto &[rel, changes] : res.changes) {
            auto src = env.open(rel);
            erpfile::Archive arc;
            std::string why;
            std::string file = "logos_" + id + "_" + flat(rel);
            std::wstring path = runtime::under_swofty(dir + file), tmp = path + L".tmp";
            HANDLE out = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
            bool ok = src && out != INVALID_HANDLE_VALUE && erpfile::open(*src, arc, why) &&
                      erpfile::rewrite(*src, arc, changes, [&](const char *p, size_t n) {
                          DWORD put = 0;
                          while (n) {
                              DWORD want = DWORD(std::min<size_t>(n, size_t(1) << 24));
                              if (!WriteFile(out, p, want, &put, nullptr) || !put)
                                  return false;
                              p += put;
                              n -= put;
                          }
                          return true;
                      }, why);
            if (out != INVALID_HANDLE_VALUE)
                CloseHandle(out);
            a.arrays[rel] = logo_deliver::arrays_of(changes, logo_build::ARRAY_URI);
            if (!ok || !MoveFileExW(tmp.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING)) {
                DeleteFileW(tmp.c_str());
                made.lines.push_back("team.logo: " + rel + " could not be rewritten" + (why.empty() ? "" : ": " + why));
                made.refused++;
                a.unwritten.push_back({rel, "it could not be rewritten" + (why.empty() ? "" : ": " + why)});
                continue;
            }
            layered = layered || overlay_rels.count(rel);
            a.routes.emplace_back(rel, file);
        }
        return a;
    };
    logo_deliver::Sink sink;
    sink.overlay = [&](const std::string &rel) { return overlay_rels.count(rel) != 0; };
    sink.route = [&](const std::string &rel, const std::string &file, std::string &why) {
        auto o = files::redirect(rel, CACHE_ADDON, file, "team.logo", ROUTE_ID, "boot");
        if (!o.refused() && !(layered && o.kind == files::Kind::shadowed))
            return true;
        why = o.line;
        return false;
    };
    sink.unroute = [&] { files::withdraw(CACHE_ADDON, "team.logo", ROUTE_ID); };
    sink.offer = [&](const logo_cache::Routes &copies, std::string &why) {
        std::map<std::string, logo_deliver::Copy> held;
        for (auto &[rel, file] : copies) {
            logo_deliver::Copy c;
            c.path = logo_deliver::overlay_key(base + dir + file);
            if (!runtime::file_size(runtime::under_swofty(dir + file), &c.size) || 6 + c.path.size() > files::GAME_PATH_MAX) {
                why = "its copy " + c.path + " is missing or its path is too long for the game";
                return false;
            }
            held[logo_deliver::overlay_key(rel)] = std::move(c);
        }
        if (logo_deliver::overlay_copies().offer(held)) {
            std::map<std::string, std::string> active;
            for (const auto &[rel, copy] : held)
                active[rel] = copy.path;
            badge_images::overlay_routes(active);
            return true;
        }
        why = "the resource overlay was handed to the game before the logo build ended";
        return false;
    };
    logo_deliver::Attempt first;
    if (!routes.empty()) {
        first.routes = routes;
        first.cached = true;
        for (auto &r : routes)
            layered = layered || overlay_rels.count(r.first);
    } else
        first = make({});
    logo_deliver::Outcome done = logo_deliver::deliver(std::move(first), make, sink);
    routes = done.last.routes;
    if (done.last.cached) {
        report.built = cached_built;
        report.lines.push_back("team.logo: " + std::to_string(routes.size()) + " cached archive(s) of " + id + " reused");
    } else {
        for (auto &l : made.lines)
            report.lines.push_back(l);
        for (auto &l : done.lines)
            report.lines.push_back(l);
        report.refused += made.refused;
        if (logo_cache::cacheable(routes.size(), made.changes, made.refused))
            runtime::write_file(runtime::under_swofty(dir + manifest_name), logo_cache::manifest_text(made.built, routes));
        report.built = made.built;
        report.lines.push_back("team.logo: " + std::to_string(made.built) + " slice(s) built into " + std::to_string(routes.size()) +
                               " archive(s) in " + std::to_string(GetTickCount() - made.t0) + " ms");
    }
    for (auto &f : done.failed) {
        bool unwritten = std::any_of(done.last.unwritten.begin(), done.last.unwritten.end(), [&](auto &u) { return u.rel == f.rel; });
        if (unwritten)
            continue;
        report.refused++;
        report.lines.push_back("team.logo: " + f.rel + " is not delivered: " + f.why);
    }
    std::string stamps_after = logo_cache::text(keyed.stamps);
    if (stamps_after != stamps_before)
        runtime::write_file(stamps_path, stamps_after);
    std::set<std::string> keep;
    for (auto &r : routes)
        keep.insert(r.second);
    keep.insert(manifest_name);
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((runtime::under_swofty(dir) + L"logos_*").c_str(), &fd);
    if (h != INVALID_HANDLE_VALUE) {
        do {
            std::string n = runtime::narrow(fd.cFileName);
            if (!keep.count(n))
                DeleteFileW((runtime::under_swofty(dir) + runtime::wide(n)).c_str());
        } while (FindNextFileW(h, &fd));
        FindClose(h);
    }
}

}
