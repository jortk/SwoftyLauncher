#pragma once
#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>
#include <vector>

#include "json.hpp"

// Pure logic of the test save sandbox (save_sandbox.cpp, re/db/subsystems/core.md#save-sandbox): the
// run token `swofty test` hands the game, the checks on the save device before its Steam remote storage
// interface is replaced, and the folder that stands in for Steam remote storage. No Windows calls, so
// the unit test runs on the host.

namespace runtime::save_sandbox {

constexpr const char *OFF_ENV = "SWOFTY_LAUNCHER_SAVES";
constexpr unsigned EXIT_TEST = 91;
constexpr unsigned EXIT_LAUNCHER = 93;
constexpr const char *STATUS_FILE = "launcher-saves.json";
constexpr const char *RUN_ENV = "SWOFTY_TEST_RUN";
constexpr const char *TOKEN_ENV = "SWOFTY_TEST_TOKEN";
constexpr const char *RUN_FILE = "run.json";
constexpr const char *REMOTE = "remote";
constexpr const char *TMP_SUFFIX = ".swofty-tmp";
constexpr size_t USER_DATA_CAP = 0x104;
constexpr size_t NAME_CAP = 260;
constexpr int SLOTS = 64;
constexpr int64_t QUOTA = int64_t(1) << 30;

// ISteamRemoteStorage (STEAMREMOTESTORAGE_INTERFACE_VERSION016) vtable slots, in declaration order.
enum Slot : int {
    FileWrite = 0,
    FileRead = 1,
    FileWriteAsync = 2,
    FileReadAsync = 3,
    FileReadAsyncComplete = 4,
    FileForget = 5,
    FileDelete = 6,
    FileShare = 7,
    SetSyncPlatforms = 8,
    FileWriteStreamOpen = 9,
    FileWriteStreamWriteChunk = 10,
    FileWriteStreamClose = 11,
    FileWriteStreamCancel = 12,
    FileExists = 13,
    FilePersisted = 14,
    GetFileSize = 15,
    GetFileTimestamp = 16,
    GetSyncPlatforms = 17,
    GetFileCount = 18,
    GetFileNameAndSize = 19,
    GetQuota = 20,
    IsCloudEnabledForAccount = 21,
    IsCloudEnabledForApp = 22,
    SetCloudEnabledForApp = 23,
    GetLocalFileChangeCount = 55,
    GetLocalFileChange = 56,
    BeginFileWriteBatch = 57,
    EndFileWriteBatch = 58,
};

inline const char *slot_name(int i)
{
    static const char *const first[] = {
        "FileWrite",          "FileRead",           "FileWriteAsync",
        "FileReadAsync",      "FileReadAsyncComplete", "FileForget",
        "FileDelete",         "FileShare",          "SetSyncPlatforms",
        "FileWriteStreamOpen", "FileWriteStreamWriteChunk", "FileWriteStreamClose",
        "FileWriteStreamCancel", "FileExists",      "FilePersisted",
        "GetFileSize",        "GetFileTimestamp",   "GetSyncPlatforms",
        "GetFileCount",       "GetFileNameAndSize", "GetQuota",
        "IsCloudEnabledForAccount", "IsCloudEnabledForApp", "SetCloudEnabledForApp"};
    if (i >= 0 && i < int(sizeof first / sizeof *first))
        return first[i];
    switch (i) {
    case GetLocalFileChangeCount:
        return "GetLocalFileChangeCount";
    case GetLocalFileChange:
        return "GetLocalFileChange";
    case BeginFileWriteBatch:
        return "BeginFileWriteBatch";
    case EndFileWriteBatch:
        return "EndFileWriteBatch";
    default:
        return i >= 24 && i <= 54 ? "UGC or workshop call" : "unknown";
    }
}

// The slots the sandbox serves from its folder. Every other slot answers 0 (false, no call, no file)
// and is logged; none is ever forwarded to Steam.
inline bool served(int i)
{
    switch (i) {
    case FileWrite:
    case FileRead:
    case FileForget:
    case FileDelete:
    case SetSyncPlatforms:
    case FileExists:
    case FilePersisted:
    case GetFileSize:
    case GetFileTimestamp:
    case GetSyncPlatforms:
    case GetFileCount:
    case GetFileNameAndSize:
    case GetQuota:
    case IsCloudEnabledForAccount:
    case IsCloudEnabledForApp:
    case SetCloudEnabledForApp:
    case GetLocalFileChangeCount:
    case BeginFileWriteBatch:
    case EndFileWriteBatch:
        return true;
    default:
        return false;
    }
}

struct Run {
    bool ok = false;
    std::string why, id, token, saves, exe_sha256;
    // The save sandbox is required: run.json names a saves folder.
    bool sandbox = false;
    // The test agent's protocol and its test DLLs, relative to the run's agent/ folder.
    int agent_v = 0;
    std::vector<std::string> dlls;
};

// A test DLL path in run.json: relative, inside the run's agent folder.
inline bool valid_dll(std::string_view d)
{
    return !d.empty() && d.size() <= 200 && d.find("..") == std::string_view::npos && d[0] != '/' && d[0] != '\\' &&
           d.find(':') == std::string_view::npos;
}

inline bool simple_name(std::string_view n)
{
    if (n.empty() || n.size() > 64 || n == "." || n == "..")
        return false;
    return std::all_of(n.begin(), n.end(), [](char c) {
        return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '-' || c == '_' ||
               c == '.';
    });
}

// run.json in the run folder (docs/design/std-2/test-suite.md "The test run"):
// {"schema":1,"run":"<id>","token":"<hex>","saves":"saves","exe_sha256":"...","agent":{"v":1,"dlls":[...]}}.
// The token in the environment must equal the file's, so a stale variable alone never switches test
// mode on and a run folder alone never does either. "saves" asks for the save sandbox; without it the
// run is the test agent's only.
inline Run parse_run(std::string_view text, std::string_view token)
{
    Run r;
    json::Value doc;
    if (text.empty())
        return r.why = "the run folder has no readable run.json", r;
    if (!json::parse(std::string(text), doc) || doc.type != json::Value::Obj)
        return r.why = "run.json does not parse as an object", r;
    const json::Value &schema = doc["schema"];
    if (schema.type != json::Value::Num || schema.n != 1)
        return r.why = "run.json has no schema 1", r;
    const json::Value &t = doc["token"];
    if (t.type != json::Value::Str || t.s.size() < 16)
        return r.why = "run.json has no token", r;
    if (token.empty())
        return r.why = std::string("the environment has no ") + TOKEN_ENV, r;
    if (t.s != token)
        return r.why = std::string("the token in ") + TOKEN_ENV + " is not this run's", r;
    const json::Value &saves = doc["saves"];
    if (saves.type != json::Value::Null && (saves.type != json::Value::Str || !simple_name(saves.s)))
        return r.why = "run.json's saves is not a plain folder name", r;
    const json::Value &agent = doc["agent"];
    if (agent.type == json::Value::Obj) {
        r.agent_v = int(agent["v"].num(0));
        for (auto &d : agent["dlls"].a) {
            if (d.type != json::Value::Str || !valid_dll(d.s))
                return r.why = "run.json names a test DLL outside the run's agent folder: " + d.str(), r;
            r.dlls.push_back(d.s);
        }
    }
    r.token = t.s;
    r.sandbox = saves.type == json::Value::Str;
    r.saves = r.sandbox ? saves.s : std::string();
    if (doc["run"].type == json::Value::Str)
        r.id = doc["run"].s;
    if (!simple_name(r.id))
        return r.why = "run.json's run is not a plain name", r;
    if (doc["exe_sha256"].type == json::Value::Str)
        r.exe_sha256 = doc["exe_sha256"].s;
    r.ok = true;
    return r;
}

// Whether run.json still holds the run's token: `swofty test` removes it when the run ends, which
// stops the agent.
inline bool still_armed(std::string_view text, std::string_view token)
{
    json::Value doc;
    return !text.empty() && json::parse(std::string(text), doc) && doc.type == json::Value::Obj &&
           doc["token"].type == json::Value::Str && !token.empty() && doc["token"].s == token;
}

// The launcher's saves (swofty/settings.json): {"launcher_saves":{"profile":"default","remote":"Z:\\...\\saves\\default\\remote"}}.
// The block is there when the profile uses launcher saves; the core then serves the profile save from
// `remote` with the same proxy the test sandbox uses.
struct Launcher {
    bool present = false, ok = false;
    std::string why, profile, remote;
};

inline std::vector<std::string> split_path(std::string_view p)
{
    std::vector<std::string> out;
    std::string cur;
    for (char c : p) {
        if (c == '/' || c == '\\') {
            if (!cur.empty())
                out.push_back(cur);
            cur.clear();
        } else
            cur += c;
    }
    if (!cur.empty())
        out.push_back(cur);
    return out;
}

inline bool ansi_path(std::string_view p);
inline bool utf8_path(std::string_view p);

// The store of a launcher profile ends in saves/<profile>/remote, so a setting that names any other folder
// is refused.
inline bool store_path_ok(std::string_view remote, std::string_view profile)
{
    if (remote.empty() || remote.size() > 400 || !utf8_path(remote))
        return false;
    auto parts = split_path(remote);
    if (parts.size() < 3)
        return false;
    for (size_t i = 0; i < parts.size(); i++)
        if (parts[i] == "." || parts[i] == ".." || (i > 0 && parts[i].find(':') != std::string::npos))
            return false;
    size_t n = parts.size();
    return parts[n - 1] == REMOTE && parts[n - 2] == profile && parts[n - 3] == "saves";
}

inline Launcher parse_launcher(std::string_view text)
{
    Launcher l;
    json::Value doc;
    if (text.empty() || !json::parse(std::string(text), doc) || doc.type != json::Value::Obj)
        return l;
    const json::Value &b = doc["launcher_saves"];
    if (b.type == json::Value::Null)
        return l;
    l.present = true;
    if (b.type != json::Value::Obj)
        return l.why = "settings.json's launcher_saves is not an object", l;
    if (b["profile"].type != json::Value::Str || !simple_name(b["profile"].s))
        return l.why = "settings.json's launcher_saves has no plain profile name", l;
    l.profile = b["profile"].s;
    if (b["remote"].type != json::Value::Str)
        return l.why = "settings.json's launcher_saves names no remote folder", l;
    l.remote = b["remote"].s;
    if (!store_path_ok(l.remote, l.profile))
        return l.why = "settings.json's launcher_saves names a folder that is not saves/" + l.profile + "/remote: " + l.remote, l;
    l.ok = true;
    return l;
}

enum class Mode { off, test, launcher, fail };

struct Selection {
    Mode mode = Mode::off;
    std::string why;
};

// Which save mode a session runs in. A test run keeps its own sandbox and never mixes with the launcher's.
// `ea_backend` says the EA copy's storage can be served; until it can, an EA copy keeps its own saves.
inline Selection select_mode(bool test_env, const Launcher &l, bool opted_out, std::string_view distribution, bool ea_backend)
{
    if (test_env)
        return {Mode::test, ""};
    if (!l.present)
        return {Mode::off, ""};
    if (opted_out)
        return {Mode::off, "this session runs on Steam's saves (the launcher asked for it once)"};
    if (distribution == "ea" && !ea_backend)
        return {Mode::off, "launcher saves are not available on the EA copy yet; this session uses the EA copy's own saves"};
    if (!l.ok)
        return {Mode::fail, l.why};
    return {Mode::launcher, ""};
}

// A path the game can open: a Unix path (the CLI under Proton) maps to Wine's Z: drive.
inline std::string windows_path(std::string p)
{
    if (!p.empty() && p[0] == '/')
        p = "Z:" + p;
    std::replace(p.begin(), p.end(), '/', '\\');
    while (p.size() > 3 && p.back() == '\\')
        p.pop_back();
    return p;
}

inline bool ansi_path(std::string_view p)
{
    return !p.empty() && std::all_of(p.begin(), p.end(), [](char c) { return c >= 0x20 && c < 0x7f; });
}

// A store path from settings.json is UTF-8 (a Windows user folder can be "C:\\Users\\Hüseyin"); the sandbox opens it
// through wide paths, so any well-formed UTF-8 without control characters is accepted.
inline bool utf8_path(std::string_view p)
{
    if (p.empty())
        return false;
    for (size_t i = 0; i < p.size();) {
        auto c = static_cast<unsigned char>(p[i]);
        if (c < 0x80) {
            if (c < 0x20 || c == 0x7f)
                return false;
            i++;
            continue;
        }
        size_t n = c >= 0xc2 && c <= 0xdf ? 2 : c >= 0xe0 && c <= 0xef ? 3 : c >= 0xf0 && c <= 0xf4 ? 4 : 0;
        if (!n || i + n > p.size())
            return false;
        for (size_t k = 1; k < n; k++)
            if ((static_cast<unsigned char>(p[i + k]) & 0xc0) != 0x80)
                return false;
        i += n;
    }
    return true;
}

// A Steam remote storage file name: one flat name, as the save device builds them
// (<title dir>@<save>#<file>). Anything that could leave the folder is refused.
inline bool valid_name(std::string_view n)
{
    if (n.empty() || n.size() > NAME_CAP || n == "." || n == "..")
        return false;
    if (n.size() > strlen(TMP_SUFFIX) && n.substr(n.size() - strlen(TMP_SUFFIX)) == TMP_SUFFIX)
        return false;
    for (char c : n)
        if (static_cast<unsigned char>(c) < 0x20 || c == '/' || c == '\\' || c == ':' || c == '*' || c == '?' ||
            c == '"' || c == '<' || c == '>' || c == '|')
            return false;
    return n.find("..") == std::string_view::npos;
}


// The EA copy's save device (vtable bound as vt_Ne_SaveSystem2Win) has no Steam helper: it opens <title dir>\\<save>\\<file> in place.
// Its title dir is the wide string at save system+0xc0 (device+0x18 = save system+0x50, so config+0x70), an
// inline buffer of config+0x68 characters whose storage sits at config+0x78.
constexpr size_t EA_DEVICE_CONFIG = 0x18;
constexpr size_t EA_CONFIG_CAP = 0x68;
constexpr size_t EA_CONFIG_PTR = 0x70;
constexpr size_t EA_CONFIG_INLINE = 0x78;
constexpr size_t EA_TITLE_CAP = 0x104;
constexpr const char *EA_TITLE_TAIL = "savegame\\ea";
constexpr const char *EA_DIR_NAME = "ea";

struct EaDevice {
    bool readable = false;
    uintptr_t vtable = 0, start_slot = 0, start = 0;
    uint64_t cap = 0;
    uintptr_t ptr = 0, inline_at = 0;
    std::wstring title;
};

inline bool ends_with_nocase(const std::wstring &s, std::wstring_view tail)
{
    if (s.size() < tail.size())
        return false;
    for (size_t i = 0; i < tail.size(); i++) {
        wchar_t a = s[s.size() - tail.size() + i], b = tail[i];
        if (a >= L'A' && a <= L'Z')
            a += 32;
        if (b >= L'A' && b <= L'Z')
            b += 32;
        if (a != b)
            return false;
    }
    return true;
}

inline std::string check_ea_device(const EaDevice &d, uintptr_t expected_vtable)
{
    if (!d.readable)
        return "the EA save device is not readable";
    if (!d.start || d.start_slot != d.start)
        return "the save device's vtable slot +0x08 is not Ne_SaveSystem2Win_Start: not the NeSaveSystem2Win device";
    if (expected_vtable && d.vtable != expected_vtable)
        return "the save device's vtable is not the EA build's NeSaveSystem2Win";
    if (d.cap != EA_TITLE_CAP)
        return "the save device's title folder buffer is not 260 characters: not the layout the EA backend was written for";
    if (!d.ptr || d.ptr != d.inline_at)
        return "the save device's title folder is not held inline in its save system";
    if (!ends_with_nocase(d.title, L"\\savegame\\ea"))
        return "the save device's title folder does not end in savegame\\ea: it is not the EA copy's save folder";
    return {};
}

// The title folder the EA device is pointed at for a store `.../saves/<profile>/remote`: its sibling `ea`, whose
// `profile` folder is the profile save. Anything else is refused.
inline std::string ea_title_dir(std::string_view remote, std::string_view profile)
{
    if (!store_path_ok(remote, profile))
        return {};
    std::string p(remote);
    while (!p.empty() && (p.back() == '\\' || p.back() == '/'))
        p.pop_back();
    size_t cut = p.find_last_of("\\/");
    if (cut == std::string::npos)
        return {};
    std::string out = p.substr(0, cut + 1) + EA_DIR_NAME;
    return out.size() < EA_TITLE_CAP ? out : std::string();
}

// What the core reads from the save device at Ne_SaveSystem2Win_Start, before it changes anything.
struct Device {
    bool readable = false;
    uintptr_t vtable = 0, start_slot = 0, start = 0;
    uintptr_t helper = 0, iface = 0, iface_vtable = 0;
    bool iface_vtable_in_game = false, iface_slots_ok = false;
    uint8_t store_in_cloud = 0;
    size_t user_data_len = 0;
};

inline std::string check_device(const Device &d)
{
    if (!d.readable)
        return "the save device is not readable";
    if (!d.start || d.start_slot != d.start)
        return "the save device's vtable slot +0x08 is not Ne_SaveSystem2Win_Start: not the NeSaveSystem2Win device";
    if (!d.helper)
        return "the save device has no Steam helper (+0x30): this build saves locally, which the sandbox does not cover";
    if (d.store_in_cloud != 1)
        return "the save device's m_storeInCloud (+0x3e8) is off: it would read and write the player's local save "
               "folder (Steam.cloudEnabled is false), which the sandbox does not cover";
    if (!d.iface)
        return "the Steam helper holds no remote storage interface (+0x108)";
    if (d.iface_vtable_in_game)
        return "the remote storage interface's vtable lies in the game image: not Steam's interface";
    if (!d.iface_slots_ok)
        return "the remote storage interface's first slots are not code";
    if (d.user_data_len >= USER_DATA_CAP)
        return "the user data folder (+0x00) is not a terminated string";
    return {};
}

// The folder that stands in for Steam remote storage: one file per remote name, written whole through a
// temporary file and a rename, read back byte for byte. The save device's files (the encrypted data file
// and its security index) are opaque bytes here.
class RemoteDir {
public:
    RemoteDir() = default;
    explicit RemoteDir(std::filesystem::path root) : root_(std::move(root)) {}

    const std::filesystem::path &root() const { return root_; }

    bool write(std::string_view name, const void *data, int32_t size) const
    {
        if (!valid_name(name) || size < 0 || (size && !data))
            return false;
        std::error_code ec;
        std::filesystem::create_directories(root_, ec);
        auto final = path(name), tmp = path(std::string(name) + TMP_SUFFIX);
        {
            std::ofstream f(tmp, std::ios::binary | std::ios::trunc);
            if (!f)
                return false;
            f.write(static_cast<const char *>(data), size);
            if (!f.flush())
                return false;
        }
        std::filesystem::rename(tmp, final, ec);
        if (ec) {
            std::filesystem::remove(tmp, ec);
            return false;
        }
        return true;
    }

    int32_t read(std::string_view name, void *out, int32_t cap) const
    {
        if (!valid_name(name) || cap <= 0 || !out)
            return 0;
        std::ifstream f(path(name), std::ios::binary);
        if (!f)
            return 0;
        f.read(static_cast<char *>(out), cap);
        return int32_t(f.gcount());
    }

    bool remove(std::string_view name) const
    {
        std::error_code ec;
        return valid_name(name) && std::filesystem::remove(path(name), ec) && !ec;
    }

    bool exists(std::string_view name) const
    {
        std::error_code ec;
        return valid_name(name) && std::filesystem::is_regular_file(path(name), ec);
    }

    int32_t size(std::string_view name) const
    {
        std::error_code ec;
        if (!exists(name))
            return 0;
        auto n = std::filesystem::file_size(path(name), ec);
        return ec || n > uint64_t(INT32_MAX) ? 0 : int32_t(n);
    }

    int64_t timestamp(std::string_view name) const
    {
        std::error_code ec;
        if (!exists(name))
            return 0;
        auto t = std::filesystem::last_write_time(path(name), ec);
        if (ec)
            return 0;
        auto sys = std::chrono::time_point_cast<std::chrono::seconds>(
            t - std::filesystem::file_time_type::clock::now() + std::chrono::system_clock::now());
        return int64_t(sys.time_since_epoch().count());
    }

    uint64_t used() const
    {
        uint64_t n = 0;
        for (auto &[name, size] : scan())
            n += uint64_t(size);
        return n;
    }

    // GetFileCount takes a snapshot that GetFileNameAndSize indexes, as Steam's own list does.
    int32_t count()
    {
        listing_ = scan();
        return int32_t(listing_.size());
    }

    const char *name_at(int32_t i, int32_t *size)
    {
        if (listing_.empty())
            listing_ = scan();
        if (i < 0 || size_t(i) >= listing_.size()) {
            if (size)
                *size = 0;
            return "";
        }
        if (size)
            *size = listing_[size_t(i)].second;
        return listing_[size_t(i)].first.c_str();
    }

private:
    std::filesystem::path path(std::string_view name) const { return root_ / std::filesystem::path(std::string(name)); }

    std::vector<std::pair<std::string, int32_t>> scan() const
    {
        std::vector<std::pair<std::string, int32_t>> out;
        std::error_code ec;
        for (auto it = std::filesystem::directory_iterator(root_, ec); !ec && it != std::filesystem::directory_iterator();
             it.increment(ec)) {
            std::error_code e2;
            if (!it->is_regular_file(e2))
                continue;
            std::string n = it->path().filename().string();
            if (!valid_name(n))
                continue;
            auto sz = it->file_size(e2);
            out.emplace_back(n, e2 || sz > uint64_t(INT32_MAX) ? 0 : int32_t(sz));
        }
        std::sort(out.begin(), out.end());
        return out;
    }

    std::filesystem::path root_;
    std::vector<std::pair<std::string, int32_t>> listing_;
};

}
