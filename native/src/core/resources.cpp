#include "runtime.hpp"

#include "apply/content/overlay_copies.hpp"
#include "erp_zstd.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>
#include <zstd.h>

#include <atomic>
#include <map>
#include <memory>
#include <mutex>
#include <set>
#include <unordered_map>
#include <unordered_set>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

namespace {

const runtime::Sig S_SITE{"site_Asset_ResMap_AfterFindOrCreate", "4C 8B BC 24 80 00 00 00 84 C0 74 83", 0x3a67976};

constexpr uint32_t ERPK = 0x4b505245;
constexpr uint8_t STORED = 0x91, ZSTD = 0x11;
constexpr int ZSTD_LEVEL = 3;
constexpr DWORD OVERLAY_LOG_MS = 5000;
constexpr size_t MAX_LOGS = 16, MAX_URI = 1023, MAX_TYPE = 15, MAX_FRAGS = 255, MIN_PACK = 64;

struct Frag {
    std::string file, packed;
    uint64_t size = 0;
    uint8_t comp = STORED;
    char fourcc[4] = {'t', 'e', 'm', 'p'};
    uint32_t flags = 0x10;
    bool has_fourcc = false, has_flags = false;
};

struct StockFrag {
    char fourcc[4];
    uint64_t size;
    uint32_t flags;
};

struct StockRes {
    char type[16] = {};
    uint32_t arg = 0;
    std::vector<StockFrag> frags;
    std::string erp;
};

struct Res {
    std::string uri, type, why, conflict;
    uint32_t arg = 0;
    size_t entry = 0;
    bool has_type = false, has_arg = false, explicit_erps = false, added = false, ok = true, have_stock = false;
    std::vector<Frag> frags;
    std::vector<std::string> erps, unlisted;
    StockRes stock;
    uint8_t digest[16] = {};
};

struct Prebuilt {
    std::string file;
    std::vector<std::string> erps;
};

struct Watch {
    std::string uri;
    uint64_t id;
    bool added;
    std::atomic<unsigned> logs{0};
};

std::vector<Res> declared;
std::vector<Prebuilt> prebuilt;
std::set<std::string> want_types;
std::map<std::string, std::set<std::string>> layouts;
runtime::Overlay overlay;
bool overlay_ok, trace_on;
HANDLE built_event;
std::once_flag build_once, copies_once;
std::atomic<bool> dropped{false};
std::unordered_map<uint64_t, std::unique_ptr<Watch>> watches;
std::vector<uint64_t> overlay_hashes;
std::atomic<bool> watches_ready{false};
safetyhook::MidHook site_hook;

uint32_t u32(const std::string &b, size_t o)
{
    uint32_t v;
    memcpy(&v, b.data() + o, 4);
    return v;
}

uint64_t u64(const std::string &b, size_t o)
{
    uint64_t v;
    memcpy(&v, b.data() + o, 8);
    return v;
}

std::string type_of(const StockRes &s)
{
    return std::string(s.type, strnlen(s.type, 16));
}

std::string cc(const char f[4])
{
    std::string s;
    for (int i = 0; i < 4; i++)
        s += f[i] >= 0x20 && f[i] < 0x7f ? f[i] : '?';
    return s;
}

std::string layout_key(const StockRes &s)
{
    std::string k = std::to_string(s.frags.size());
    for (auto &f : s.frags)
        k += " " + cc(f.fourcc) + "/0x" + runtime::hex16(f.flags).substr(10);
    return k;
}

std::string layout_key(const Res &r)
{
    std::string k = std::to_string(r.frags.size());
    for (auto &f : r.frags)
        k += " " + cc(f.fourcc) + "/0x" + runtime::hex16(f.flags).substr(10);
    return k;
}

std::string describe(const StockRes &s)
{
    std::string d = type_of(s) + " arg " + std::to_string(s.arg) + ", " + std::to_string(s.frags.size()) + " fragment(s)";
    for (auto &f : s.frags)
        d += " [" + cc(f.fourcc) + " " + std::to_string(f.size) + " bytes flags 0x" + runtime::hex16(f.flags).substr(10) + "]";
    return d;
}

bool same_layout(const StockRes &a, const StockRes &b)
{
    if (memcmp(a.type, b.type, 16) || a.arg != b.arg || a.frags.size() != b.frags.size())
        return false;
    for (size_t i = 0; i < a.frags.size(); i++)
        if (memcmp(a.frags[i].fourcc, b.frags[i].fourcc, 4) || a.frags[i].size != b.frags[i].size ||
            a.frags[i].flags != b.frags[i].flags)
            return false;
    return true;
}

bool read_index(const std::wstring &path, std::string &idx, uint16_t &ver)
{
    HANDLE f = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return false;
    std::string hdr(0x30, '\0');
    DWORD got = 0;
    bool ok = ReadFile(f, hdr.data(), 0x30, &got, nullptr) && got == 0x30 && u32(hdr, 0) == ERPK;
    if (ok) {
        ver = uint16_t(u32(hdr, 4));
        uint64_t off = u64(hdr, 0x10), size = u64(hdr, 0x18);
        ok = size >= 8 && size < (256u << 20);
        LARGE_INTEGER at;
        at.QuadPart = LONGLONG(off);
        if (ok && SetFilePointerEx(f, at, nullptr, FILE_BEGIN)) {
            idx.resize(size_t(size));
            ok = ReadFile(f, idx.data(), DWORD(size), &got, nullptr) && got == size;
        } else
            ok = false;
    }
    CloseHandle(f);
    return ok;
}

template <class F> bool each_entry(const std::string &idx, uint16_t ver, F fn)
{
    if (ver != 3 || idx.size() < 8)
        return false;
    uint32_t n = u32(idx, 0);
    size_t p = 8;
    for (uint32_t i = 0; i < n; i++) {
        if (p + 6 > idx.size())
            return false;
        uint32_t esz = u32(idx, p);
        uint16_t nlen = uint16_t(uint8_t(idx[p + 4]) | uint8_t(idx[p + 5]) << 8);
        size_t q = p + 6;
        if (p + 4 + esz > idx.size() || q + nlen + 21 > idx.size() || !nlen)
            return false;
        std::string_view uri(idx.data() + q, nlen - 1);
        q += nlen;
        StockRes r;
        memcpy(r.type, idx.data() + q, 16);
        q += 16;
        r.arg = u32(idx, q);
        q += 4;
        uint8_t nf = uint8_t(idx[q++]);
        if (q + size_t(nf) * 0x21 + 16 > idx.size())
            return false;
        for (uint8_t j = 0; j < nf; j++, q += 0x21) {
            StockFrag fr;
            memcpy(fr.fourcc, idx.data() + q, 4);
            fr.size = u64(idx, q + 12);
            fr.flags = u32(idx, q + 20);
            r.frags.push_back(fr);
        }
        q += 16;
        if (q != p + 4 + esz)
            return false;
        fn(uri, r);
        p = q;
    }
    return true;
}

void scan_dir(const std::wstring &dir, const std::string &rel, std::vector<std::string> &out, unsigned depth)
{
    if (depth > 10)
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
            scan_dir(dir + n + L"\\", r + "/", out, depth + 1);
        else if (r.size() > 4 && runtime::lower(r.substr(r.size() - 4)) == ".erp")
            out.push_back(r);
    } while (FindNextFileW(h, &fd));
    FindClose(h);
}

std::wstring game_path(const std::string &rel)
{
    std::wstring p = runtime::game_root() + runtime::wide(rel);
    std::replace(p.begin(), p.end(), L'/', L'\\');
    return p;
}

std::string package_of(const std::string &uri)
{
    if (uri.rfind("eaid://", 0) != 0)
        return {};
    size_t slash = uri.find('/', 7);
    return slash == std::string::npos ? std::string() : uri.substr(7, slash - 7);
}

Res parse_entry(const json::Value &j, size_t entry)
{
    Res r;
    r.entry = entry;
    r.uri = j["uri"].str();
    if (j["type"].type == json::Value::Str)
        r.type = j["type"].s, r.has_type = true;
    if (j["arg"].type == json::Value::Num)
        r.arg = uint32_t(j["arg"].num()), r.has_arg = true;
    auto frag = [&](const json::Value &v) {
        Frag f;
        if (v.type == json::Value::Str)
            f.file = v.s;
        else {
            f.file = v["file"].str();
            std::string c = v["fourcc"].str();
            if (!c.empty()) {
                memset(f.fourcc, 0, 4);
                memcpy(f.fourcc, c.data(), std::min<size_t>(c.size(), 4));
                f.has_fourcc = true;
            }
            if (v["flags"].type == json::Value::Num)
                f.flags = uint32_t(v["flags"].num()), f.has_flags = true;
        }
        r.frags.push_back(std::move(f));
    };
    if (j["file"].type == json::Value::Str)
        frag(j["file"]);
    for (auto &x : j["fragments"].a)
        frag(x);
    for (auto &x : j["erps"].a)
        if (x.type == json::Value::Str)
            r.erps.push_back(runtime::lower(x.s));
    r.explicit_erps = !r.erps.empty();
    if (r.uri.rfind("eaid://", 0) != 0)
        r.ok = false, r.why = "the uri must start with eaid://";
    else if (r.uri.size() > MAX_URI)
        r.ok = false, r.why = "the uri is longer than 1023 characters";
    else if (r.frags.empty())
        r.ok = false, r.why = "no \"file\" or \"fragments\" given";
    else if (r.frags.size() > MAX_FRAGS)
        r.ok = false, r.why = "more than 255 fragments";
    else if (r.has_type && (r.type.empty() || r.type.size() > MAX_TYPE))
        r.ok = false, r.why = "\"type\" must be 1 to 15 characters (the index field is char[16], zero padded)";
    return r;
}

void read_config()
{
    std::unordered_map<std::string, size_t> at;
    std::vector<const json::Value *> list;
    for (auto &j : runtime::config()["resources"].a)
        list.push_back(&j);
    for (auto &j : runtime::sdk()["resources"].a)
        list.push_back(&j);
    for (size_t i = 0; i < list.size(); i++) {
        Res r = parse_entry(*list[i], i);
        if (r.has_type)
            want_types.insert(r.type);
        auto it = r.uri.empty() ? at.end() : at.find(r.uri);
        if (it == at.end()) {
            if (!r.uri.empty())
                at[r.uri] = declared.size();
            declared.push_back(std::move(r));
            continue;
        }
        swofty::log("[assets] resources: %s is declared again: entry %zu replaces entry %zu (the later declaration wins)",
                    r.uri.c_str(), i, declared[it->second].entry);
        declared[it->second] = std::move(r);
    }
    for (auto &j : runtime::config()["resource_erps"].a) {
        Prebuilt p;
        p.file = runtime::lower(j["file"].str());
        for (auto &x : j["erps"].a)
            if (x.type == json::Value::Str)
                p.erps.push_back(runtime::lower(x.s));
        prebuilt.push_back(std::move(p));
    }
}

void lookup_stock()
{
    std::map<std::string, std::vector<Res *>> by_package;
    std::unordered_map<std::string, Res *> by_uri;
    for (auto &r : declared) {
        if (!r.ok)
            continue;
        by_uri[r.uri] = &r;
        if (!r.explicit_erps)
            by_package[package_of(r.uri)].push_back(&r);
    }
    auto learn = [&](const StockRes &s) {
        if (!want_types.empty()) {
            std::string t = type_of(s);
            if (want_types.count(t))
                layouts[t].insert(layout_key(s));
        }
    };
    auto note = [&](Res &r, const StockRes &s, const std::string &erp) {
        if (!r.have_stock) {
            r.stock = s;
            r.stock.erp = erp;
            r.have_stock = true;
        } else if (r.conflict.empty() && !same_layout(r.stock, s))
            r.conflict = erp + " has " + describe(s) + ", " + r.stock.erp + " has " + describe(r.stock);
    };
    struct Index {
        bool readable = false;
        std::unordered_map<std::string, StockRes> by_uri;
    };
    std::map<std::string, Index> indexes;
    for (auto &r : declared) {
        if (!r.ok || !r.explicit_erps)
            continue;
        for (auto &erp : r.erps) {
            auto it = indexes.find(erp);
            if (it == indexes.end()) {
                it = indexes.emplace(erp, Index{}).first;
                std::string idx;
                uint16_t ver = 0;
                if (read_index(game_path(erp), idx, ver)) {
                    it->second.readable = true;
                    each_entry(idx, ver, [&](std::string_view uri, const StockRes &s) {
                        learn(s);
                        it->second.by_uri.emplace(uri, s);
                    });
                }
            }
            auto hit = it->second.by_uri.find(r.uri);
            if (hit != it->second.by_uri.end())
                note(r, hit->second, erp);
            else
                r.unlisted.push_back(erp + (it->second.readable ? "" : " (not a readable loose ERP)"));
        }
    }
    for (auto &[pkg, list] : by_package) {
        DWORD t0 = GetTickCount();
        std::string rel = "2025_asset_groups/" + pkg + "/";
        std::vector<std::string> files;
        if (!pkg.empty())
            scan_dir(game_path(rel), rel, files, 0);
        std::unordered_set<std::string_view> want;
        for (auto *r : list)
            want.insert(r->uri);
        size_t readable_count = 0;
        for (auto &f : files) {
            std::string idx;
            uint16_t ver = 0;
            if (!read_index(game_path(f), idx, ver))
                continue;
            readable_count++;
            std::string lf = runtime::lower(f);
            each_entry(idx, ver, [&](std::string_view uri, const StockRes &s) {
                learn(s);
                if (!want.count(uri))
                    return;
                Res *r = by_uri[std::string(uri)];
                r->erps.push_back(lf);
                note(*r, s, lf);
            });
        }
        swofty::log("[assets] resources: scanned %zu loose ERP index(es) under %s in %lu ms for %zu uri(s) (ERPs inside "
                    "game.dat are not scanned)",
                    readable_count, rel.c_str(), (unsigned long)(GetTickCount() - t0), list.size());
    }
}

void pack(Frag &f, std::string &&data)
{
    f.size = data.size();
    if (data.size() >= MIN_PACK) {
        std::string out;
        if (swofty_core::erp_zstd::compress(data.data(), data.size(), ZSTD_LEVEL, out) && out.size() < data.size()) {
            out.shrink_to_fit();
            f.packed = std::move(out);
            f.comp = ZSTD;
            return;
        }
    }
    f.packed = std::move(data);
    f.comp = STORED;
}

void fail(Res &r, const std::string &why)
{
    r.ok = false;
    r.why = why;
}

void check_replacement(Res &r)
{
    if (!r.conflict.empty())
        return fail(r, "the stock ERPs disagree on its layout (" + r.conflict + ")");
    std::string st = type_of(r.stock);
    if (r.has_type && r.type != st)
        return fail(r, "\"type\" " + r.type + " differs from the stock type " + st + " in " + r.stock.erp +
                           " (the type picks the engine's handler; a replacement keeps the stock type)");
    r.type = st;
    if (r.has_arg && r.arg != r.stock.arg)
        return fail(r, "\"arg\" " + std::to_string(r.arg) + " differs from the stock create argument " +
                           std::to_string(r.stock.arg));
    r.arg = r.stock.arg;
    if (r.frags.size() != r.stock.frags.size())
        return fail(r, "the stock entry in " + r.stock.erp + " has " + std::to_string(r.stock.frags.size()) +
                           " fragment(s), the mod gives " + std::to_string(r.frags.size()) +
                           "; a different fragment layout is not proven safe for any type");
    for (size_t i = 0; i < r.frags.size(); i++) {
        auto &f = r.frags[i];
        auto &s = r.stock.frags[i];
        if (f.has_fourcc && memcmp(f.fourcc, s.fourcc, 4))
            return fail(r, "fragment " + std::to_string(i) + " fourcc '" + cc(f.fourcc) + "' differs from the stock '" +
                               cc(s.fourcc) + "'");
        if (f.has_flags && f.flags != s.flags)
            return fail(r, "fragment " + std::to_string(i) + " flags 0x" + runtime::hex16(f.flags).substr(10) +
                               " differ from the stock 0x" + runtime::hex16(s.flags).substr(10));
        memcpy(f.fourcc, s.fourcc, 4);
        f.flags = s.flags;
        if (f.size != s.size)
            return fail(r, "fragment " + std::to_string(i) + " (" + f.file + ") is " + std::to_string(f.size) +
                               " bytes, the stock one " + std::to_string(s.size) +
                               "; stock ERPs that bind to the replacement read it with the stock layout, so sizes must match");
    }
}

void check_addition(Res &r)
{
    if (!r.has_type)
        return fail(r, "not found in any scanned stock ERP; give \"type\" (and fragment fourcc/flags) to add it as a new "
                       "resource");
    if (r.erps.empty())
        return fail(r, "an added resource needs \"erps\": the stock ERP(s) whose groups load it");
    auto it = layouts.find(r.type);
    if (it == layouts.end())
        return fail(r, "type " + r.type + " is not used by any resource in the scanned stock ERPs; unknown types make the "
                           "engine read past its handler table");
    std::string key = layout_key(r);
    if (!it->second.count(key)) {
        std::string seen;
        for (auto &k : it->second)
            if (seen.size() < 200)
                seen += (seen.empty() ? "" : "; ") + k;
        return fail(r, "no stock " + r.type + " resource in the scanned ERPs has the fragment layout " + key + " (seen: " +
                           seen + ")");
    }
    r.added = true;
}

void settle(Res &r)
{
    if (!r.ok)
        return;
    for (auto &f : r.frags) {
        uint64_t size = 0;
        if (!runtime::file_size(runtime::under_swofty(f.file), &size) || !size)
            return fail(r, "fragment file " + f.file + " is missing or empty");
        f.size = size;
    }
    if (r.have_stock) {
        for (auto &u : r.unlisted)
            swofty::log("[assets] resources: %s: listed ERP %s does not contain it; the overlay still loads with that ERP's "
                        "groups",
                        r.uri.c_str(), u.c_str());
        check_replacement(r);
    } else
        check_addition(r);
    if (!r.ok)
        return;
    std::string all;
    for (auto &f : r.frags) {
        std::string data = runtime::read_file(runtime::under_swofty(f.file));
        if (data.size() != f.size)
            return fail(r, "fragment file " + f.file + " could not be read");
        if (all.size() < (size_t(64) << 20))
            all += data;
        pack(f, std::move(data));
    }
    runtime::md5(all.data(), all.size(), r.digest);
}

std::string index_of(const std::vector<Res *> &list, uint64_t &data_size)
{
    std::string idx;
    uint32_t nfrag = 0;
    data_size = 0;
    for (auto *r : list) {
        std::string e;
        uint16_t nlen = uint16_t(r->uri.size() + 1);
        e.append(reinterpret_cast<const char *>(&nlen), 2);
        e.append(r->uri.c_str(), r->uri.size() + 1);
        char type[16] = {};
        memcpy(type, r->type.data(), r->type.size());
        e.append(type, 16);
        e.append(reinterpret_cast<const char *>(&r->arg), 4);
        e.push_back(char(uint8_t(r->frags.size())));
        for (auto &f : r->frags) {
            uint64_t off = data_size, size = f.size, packed = f.packed.size();
            e.append(f.fourcc, 4);
            e.append(reinterpret_cast<const char *>(&off), 8);
            e.append(reinterpret_cast<const char *>(&size), 8);
            e.append(reinterpret_cast<const char *>(&f.flags), 4);
            e.push_back(char(f.comp));
            e.append(reinterpret_cast<const char *>(&packed), 8);
            data_size += packed;
            nfrag++;
        }
        e.append(reinterpret_cast<const char *>(r->digest), 16);
        uint32_t esz = uint32_t(e.size());
        idx.append(reinterpret_cast<const char *>(&esz), 4);
        idx += e;
    }
    uint32_t head[2] = {uint32_t(list.size()), nfrag};
    idx.insert(0, reinterpret_cast<const char *>(head), 8);
    return idx;
}

uint64_t fnv_more(uint64_t h, const std::string &s)
{
    for (unsigned char c : s) {
        h ^= c;
        h *= 0x100000001b3ull;
    }
    return h;
}

bool write_all(HANDLE f, const std::string &s)
{
    size_t done = 0;
    while (done < s.size()) {
        DWORD put = 0, want = DWORD(std::min<size_t>(s.size() - done, 1u << 30));
        if (!WriteFile(f, s.data() + done, want, &put, nullptr) || !put)
            return false;
        done += put;
    }
    return true;
}

bool write_erp(const std::vector<Res *> &list, const std::string &rel_dir, const std::wstring &dir, runtime::OverlayErp &out,
               bool &reused)
{
    uint64_t data_size = 0;
    std::string idx = index_of(list, data_size);
    std::string hdr(0x30, '\0');
    uint32_t magic = ERPK, ver = 3;
    uint64_t io = 0x30, isz = idx.size(), dof = 0x30 + idx.size();
    memcpy(hdr.data(), &magic, 4);
    memcpy(hdr.data() + 4, &ver, 4);
    memcpy(hdr.data() + 0x10, &io, 8);
    memcpy(hdr.data() + 0x18, &isz, 8);
    memcpy(hdr.data() + 0x20, &dof, 8);
    uint64_t h = fnv_more(fnv_more(0xcbf29ce484222325ull, hdr), idx);
    for (auto *r : list)
        for (auto &f : r->frags)
            h = fnv_more(h, f.packed);
    std::string name = "rt_" + runtime::hex16(h).substr(2) + ".erp";
    out.path = rel_dir + name;
    out.size = dof + data_size;
    out.resources = list.size();
    std::wstring file = dir + L"\\" + runtime::wide(name);
    uint64_t have = 0;
    reused = runtime::file_size(file, &have) && have == out.size;
    if (reused)
        return true;
    std::wstring tmp = file + L".tmp" + std::to_wstring(GetCurrentProcessId());
    HANDLE f = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return false;
    bool ok = write_all(f, hdr) && write_all(f, idx);
    for (auto *r : list)
        for (auto &fr : r->frags)
            ok = ok && write_all(f, fr.packed);
    CloseHandle(f);
    if (!ok || !MoveFileExW(tmp.c_str(), file.c_str(), MOVEFILE_REPLACE_EXISTING)) {
        DWORD e = GetLastError();
        DeleteFileW(tmp.c_str());
        SetLastError(e);
        return false;
    }
    return true;
}

void clean_cache(const std::wstring &dir, const std::set<std::string> &keep)
{
    size_t removed = 0, busy = 0;
    for (const wchar_t *pat : {L"\\rt_*.erp", L"\\resources.erp"}) {
        WIN32_FIND_DATAW fd;
        HANDLE h = FindFirstFileW((dir + pat).c_str(), &fd);
        if (h == INVALID_HANDLE_VALUE)
            continue;
        do {
            std::string n = runtime::lower(runtime::narrow(fd.cFileName));
            if (keep.count(n))
                continue;
            if (DeleteFileW((dir + L"\\" + fd.cFileName).c_str()))
                removed++;
            else
                busy++;
        } while (FindNextFileW(h, &fd));
        FindClose(h);
    }
    if (removed || busy)
        swofty::log("[assets] resources: cache: %zu stale overlay ERP(s) removed, %zu in use by another process and kept",
                    removed, busy);
}

void watch(const std::string &uri, bool added)
{
    auto w = std::make_unique<Watch>();
    w->uri = uri;
    uint8_t d[16];
    runtime::md5(uri.data(), uri.size(), d);
    w->id = 0;
    for (int i = 0; i < 8; i++)
        w->id = w->id << 8 | d[i];
    w->added = added;
    watches[runtime::fnv64(uri)] = std::move(w);
}

void load_prebuilt()
{
    DWORD t0 = GetTickCount();
    std::string root = runtime::lower(runtime::narrow(runtime::game_root())),
                sw = runtime::lower(runtime::narrow(runtime::swofty_root()));
    if (sw.rfind(root, 0) != 0) {
        swofty::log("[assets] resources: the swofty folder %s is not under the game root %s; %zu prebuilt overlay ERP(s) "
                    "NOT loaded, their target ERPs load stock data",
                    sw.c_str(), root.c_str(), prebuilt.size());
        return;
    }
    std::map<std::string, std::unordered_set<std::string>> stock;
    size_t loaded = 0;
    for (auto &p : prebuilt) {
        std::wstring file = runtime::under_swofty(p.file);
        std::string idx, why;
        uint16_t ver = 0;
        uint64_t size = 0;
        std::vector<std::string> uris;
        if (p.file.empty() || p.erps.empty())
            why = "it needs \"file\" and \"erps\"";
        else if (!runtime::file_size(file, &size) || !read_index(file, idx, ver))
            why = "the file is missing or not an ERP";
        else if (!each_entry(idx, ver, [&](std::string_view uri, const StockRes &) { uris.emplace_back(uri); }))
            why = "its index (version " + std::to_string(ver) + ") does not parse";
        else if (uris.empty())
            why = "it holds no resource";
        if (!why.empty()) {
            swofty::log("[assets] resources: prebuilt overlay ERP %s NOT loaded: %s; its %zu target ERP(s) load stock data",
                        p.file.c_str(), why.c_str(), p.erps.size());
            continue;
        }
        size_t replaced = 0;
        for (auto &t : p.erps)
            if (!stock.count(t)) {
                auto &set = stock[t];
                std::string tidx;
                uint16_t tver = 0;
                if (read_index(game_path(t), tidx, tver))
                    each_entry(tidx, tver, [&](std::string_view uri, const StockRes &) { set.emplace(uri); });
            }
        for (auto &u : uris) {
            bool in_stock = false;
            for (auto &t : p.erps)
                in_stock = in_stock || stock[t].count(u);
            if (in_stock)
                replaced++;
            watch(u, !in_stock);
        }
        overlay.replaced += replaced;
        overlay.added += uris.size() - replaced;
        runtime::OverlayErp e;
        e.path = sw.substr(root.size()) + p.file;
        e.size = size;
        e.resources = uris.size();
        e.targets = p.erps;
        std::string tl;
        for (auto &t : p.erps)
            if (tl.size() < 400)
                tl += (tl.empty() ? "" : ", ") + t;
        swofty::log("[assets] resources: overlay ERP %s prebuilt by the launcher (%llu bytes, %zu resource(s): %zu replaced, "
                    "%zu added) for %zu target ERP(s): %s",
                    e.path.c_str(), (unsigned long long)size, uris.size(), replaced, uris.size() - replaced,
                    p.erps.size(), tl.c_str());
        overlay.erps.push_back(std::move(e));
        loaded++;
    }
    if (loaded)
        overlay_ok = true;
    swofty::log("[assets] resources: %zu of %zu prebuilt overlay ERP(s) loaded in %lu ms", loaded, prebuilt.size(),
                (unsigned long)(GetTickCount() - t0));
}

void build_declared();

void build_overlays()
{
    if (!prebuilt.empty())
        load_prebuilt();
    if (!declared.empty())
        build_declared();
}

void build_declared()
{
    DWORD t0 = GetTickCount();
    lookup_stock();
    std::vector<Res *> good;
    for (auto &r : declared) {
        settle(r);
        if (!r.ok) {
            swofty::log("[assets] resources: skipped %s: %s", r.uri.c_str(), r.why.c_str());
            continue;
        }
        std::sort(r.erps.begin(), r.erps.end());
        r.erps.erase(std::unique(r.erps.begin(), r.erps.end()), r.erps.end());
        std::string where, packed;
        uint64_t bytes = 0, stored = 0;
        for (auto &e : r.erps)
            where += (where.empty() ? "" : ", ") + e;
        for (auto &f : r.frags)
            bytes += f.size, stored += f.packed.size();
        swofty::log("[assets] resources: %s %s (%s, arg %u, %zu fragment(s), %llu bytes, %llu stored%s)%s%s; stock ERP(s): %s",
                    r.added ? "add" : "replace", r.uri.c_str(), r.type.c_str(), r.arg, r.frags.size(),
                    (unsigned long long)bytes, (unsigned long long)stored, stored < bytes ? " zstd" : "",
                    r.have_stock ? ", metadata from " : "", r.have_stock ? r.stock.erp.c_str() : "",
                    where.empty() ? "none" : where.c_str());
        good.push_back(&r);
        r.added ? overlay.added++ : overlay.replaced++;
    }
    if (good.empty()) {
        swofty::log("[assets] resources: nothing to load (%zu declared)", declared.size());
        return;
    }
    std::string root = runtime::lower(runtime::narrow(runtime::game_root())),
                sw = runtime::lower(runtime::narrow(runtime::swofty_root()));
    if (sw.rfind(root, 0) != 0) {
        swofty::log("[assets] resources: the swofty folder %s is not under the game root %s; the overlay ERPs cannot be "
                    "served by the game's file system",
                    sw.c_str(), root.c_str());
        return;
    }
    std::string rel_dir = sw.substr(root.size()) + "cache/";
    std::wstring dir = runtime::swofty_root() + L"cache";
    CreateDirectoryW(dir.c_str(), nullptr);
    std::map<std::string, std::vector<size_t>> by_target;
    for (size_t i = 0; i < good.size(); i++)
        for (auto &e : good[i]->erps)
            by_target[e].push_back(i);
    std::map<std::vector<size_t>, std::vector<std::string>> sets;
    for (auto &[t, members] : by_target)
        sets[members].push_back(t);
    std::set<std::string> keep;
    size_t written = 0, reused_count = 0;
    for (auto &[members, targets] : sets) {
        std::vector<Res *> list;
        for (size_t i : members)
            list.push_back(good[i]);
        runtime::OverlayErp e;
        bool reused = false;
        if (!write_erp(list, rel_dir, dir, e, reused)) {
            swofty::log("[assets] resources: writing %s failed (error %lu); %zu target ERP(s) load stock data",
                        e.path.c_str(), GetLastError(), targets.size());
            continue;
        }
        e.targets = targets;
        reused ? reused_count++ : written++;
        keep.insert(e.path.substr(rel_dir.size()));
        std::string uris, tl;
        for (auto *r : list)
            if (uris.size() < 400)
                uris += (uris.empty() ? "" : ", ") + r->uri;
        for (auto &t : targets)
            if (tl.size() < 400)
                tl += (tl.empty() ? "" : ", ") + t;
        swofty::log("[assets] resources: overlay ERP %s %s (%llu bytes, %zu resource(s): %s) for %zu target ERP(s): %s",
                    e.path.c_str(), reused ? "unchanged" : "written", (unsigned long long)e.size, list.size(), uris.c_str(),
                    targets.size(), tl.c_str());
        overlay.erps.push_back(std::move(e));
    }
    clean_cache(dir, keep);
    if (overlay.erps.empty())
        return;
    overlay_ok = true;
    for (auto *r : good)
        watch(r->uri, r->added);
    for (auto *r : good)
        for (auto &f : r->frags)
            std::string().swap(f.packed);
    swofty::log("[assets] resources: %zu overlay ERP(s) (%zu written, %zu unchanged) for %zu target ERP(s); %zu replaced, %zu "
                "added, in %lu ms",
                overlay.erps.size(), written, reused_count, by_target.size(), overlay.replaced, overlay.added,
                (unsigned long)(GetTickCount() - t0));
}

void build()
{
    try {
        build_overlays();
    } catch (const std::exception &e) {
        overlay_ok = false;
        overlay.erps.clear();
        swofty::log("[assets] resources: overlay build failed (%s); every replaced uri loads its stock data", e.what());
    }
    if (dropped.load())
        swofty::log("[assets] resources: the overlay build finished after the manifest parse stopped waiting for it; "
                    "nothing is registered this session");
}

DWORD WINAPI build_thread(LPVOID)
{
    std::call_once(build_once, build);
    SetEvent(built_event);
    return 0;
}

void site(safetyhook::Context &c)
{
    if (!watches_ready.load(std::memory_order_acquire))
        return;
    auto idx = reinterpret_cast<const uint8_t *>(c.rdi + c.rsi);
    const char *uri = *reinterpret_cast<const char *const *>(idx + 8);
    if (!uri)
        return;
    auto it = watches.find(runtime::fnv64(uri, strnlen(uri, 1024)));
    if (it == watches.end() || it->second->uri != uri)
        return;
    Watch *w = it->second.get();
    uint64_t id = *reinterpret_cast<const uint64_t *>(idx);
    auto info = *reinterpret_cast<const uint64_t *const *>(c.r13 + 0x330);
    uint64_t erp = info ? info[0] : 0;
    auto res = *reinterpret_cast<const uint8_t *const *>(c.rsp + 0x80);
    bool created = uint8_t(c.rax) != 0,
         ours = erp && std::find(overlay_hashes.begin(), overlay_hashes.end(), erp) != overlay_hashes.end();
    int refs = res ? *reinterpret_cast<const int32_t *>(res + 0x1c) : -1;
    int status = res ? res[0x22] : -1;
    if (w->logs.fetch_add(1) >= MAX_LOGS)
        return;
    std::string name = erp ? runtime::erp_name(erp) : std::string("?");
    const char *what = created ? (ours ? "CREATED from the overlay (first reader)"
                                       : "created by a stock ERP, NOT the overlay: the stock data wins until every holder releases it")
                               : (ours ? "the overlay found it ALREADY LOADED: the copy already in memory stays"
                                       : "bound to the existing copy (the stock bytes are skipped)");
    swofty::log("[assets] resource %s: %s; erp %s (0x%016llx), NeResource %p refs %d status %d, id 0x%016llx (md5 %s)",
                w->uri.c_str(), what, name.c_str(), (unsigned long long)erp, static_cast<const void *>(res), refs, status,
                (unsigned long long)id, id == w->id ? "match" : "MISMATCH");
}

// The team logo build grows texture arrays in copies of the archives that carry them. An overlay archive is read
// before its targets, so its copy is registered in its place; from here on the logo build can offer no other copy.
void use_logo_copies()
{
    auto copies = swofty_core::apply::content::logo_deliver::overlay_copies().take();
    for (auto &e : overlay.erps) {
        auto it = copies.find(swofty_core::apply::content::logo_deliver::overlay_key(e.path));
        if (it == copies.end())
            continue;
        swofty::log("[assets] resources: overlay ERP %s is registered as its team.logo copy %s (%llu bytes): the copy holds "
                    "the texture array(s) the logo build grew",
                    e.path.c_str(), it->second.path.c_str(), (unsigned long long)it->second.size);
        e.path = it->second.path;
        e.size = it->second.size;
    }
}

}

namespace runtime {

const Overlay *resource_overlay(DWORD wait_ms)
{
    if (declared.empty() && prebuilt.empty())
        return nullptr;
    if (built_event && wait_ms) {
        DWORD t0 = GetTickCount(), r;
        while ((r = WaitForSingleObject(built_event, std::min<DWORD>(OVERLAY_LOG_MS, wait_ms))) == WAIT_TIMEOUT &&
               GetTickCount() - t0 < wait_ms)
            swofty::log("[assets] resources: the overlay build is still running after %lu ms; the manifest parse waits "
                        "for it (no lock held while waiting)",
                        (unsigned long)(GetTickCount() - t0));
        if (r != WAIT_OBJECT_0) {
            overlay_dropped("the overlay build was still running after " + std::to_string(GetTickCount() - t0) +
                            " ms (the wait holds no lock)");
            return nullptr;
        }
        if (GetTickCount() - t0 > 20)
            swofty::log("[assets] resources: waited %lu ms for the overlay build (no lock held while waiting)",
                        (unsigned long)(GetTickCount() - t0));
    } else {
        DWORD t0 = GetTickCount();
        std::call_once(build_once, build);
        if (GetTickCount() - t0 > 20)
            swofty::log("[assets] resources: built the overlay on this thread in %lu ms", (unsigned long)(GetTickCount() - t0));
    }
    if (!overlay_ok) {
        overlay_dropped("the overlay build produced no ERP");
        return nullptr;
    }
    std::call_once(copies_once, use_logo_copies);
    return &overlay;
}

void overlay_dropped(const std::string &why)
{
    dropped.store(true);
    swofty::log("[assets] resources: overlay DROPPED: %s. The manifest keeps its stock ERP lists, every replaced uri loads "
                "its stock data this session and the first-reader trace stays silent; this is not a load-order (T013) "
                "result",
                why.c_str());
}

void overlay_registered(const std::vector<uint64_t> &erp_hashes, size_t groups)
{
    overlay_hashes = erp_hashes;
    if (trace_on)
        watches_ready.store(true, std::memory_order_release);
    swofty::log("[assets] resources: %zu overlay ERP(s) registered, prepended in %zu manifest group(s); first-reader trace %s",
                erp_hashes.size(), groups, trace_on && site_hook ? "watching" : "off");
}

}

namespace {

void setup()
{
    read_config();
    if (declared.empty() && prebuilt.empty())
        return;
    if (!declared.empty())
        swofty::log("[assets] resources: %zu declared in %s", declared.size(), runtime::config_source().c_str());
    if (!prebuilt.empty())
        swofty::log("[assets] resources: %zu prebuilt overlay ERP(s) in %s", prebuilt.size(), runtime::config_source().c_str());
    const json::Value &tr = runtime::config()["resources_trace"];
    trace_on = tr.type == json::Value::Bool && tr.b;
    built_event = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    HANDLE t = built_event ? CreateThread(nullptr, 0, build_thread, nullptr, 0, nullptr) : nullptr;
    if (t)
        CloseHandle(t);
    else {
        if (built_event)
            CloseHandle(built_event), built_event = nullptr;
        std::call_once(build_once, build);
    }
    if (!trace_on) {
        swofty::log("[assets] resources: first-reader trace off (\"resources_trace\": true turns it on)");
        return;
    }
    uintptr_t at = runtime::find(S_SITE, "resources");
    if (!at)
        return;
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), site);
    if (!h) {
        swofty::log("[assets] resources: first-reader trace site at base+0x%llx failed: %s",
                    (unsigned long long)(at - swofty::api->module_base()), runtime::hook_error(h.error()).c_str());
        return;
    }
    site_hook = std::move(*h);
    swofty::log("[assets] resources: first-reader trace on at base+0x%llx (site_Asset_ResMap_AfterFindOrCreate); it "
                "watches the declared uri(s) once the overlay ERPs are registered",
                (unsigned long long)(at - swofty::api->module_base()));
}

}

void runtime::resources_setup()
{
    setup();
}
