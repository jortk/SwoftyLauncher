#include "runtime.hpp"

#include <swofty/mixin.hpp>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <memory>
#include <mutex>
#include <thread>

#include "log_csv_logic.hpp"

// The CSV recorder behind swofty/log_csv.hpp: files under swofty/addons/<addon id>/diag/, a ring per file
// (log_csv_logic.hpp), and one writer thread that formats and writes every ring about ten times a second.

std::string settings_addon(const void *from, std::string &dll);

namespace {

struct File {
    std::unique_ptr<runtime::log_csv::Recorder> rec;
    HANDLE handle = INVALID_HANDLE_VALUE;
    std::string id, name;
    bool closed = false;
};

std::mutex files_lock;
std::vector<std::unique_ptr<File>> files;
std::once_flag writer_started;
std::condition_variable wake;

std::string stamp()
{
    SYSTEMTIME t;
    GetLocalTime(&t);
    char b[32];
    snprintf(b, sizeof b, "%04u%02u%02u-%02u%02u%02u", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
    return b;
}

void write_block(File &f, const std::string &s)
{
    DWORD n;
    WriteFile(f.handle, s.data(), DWORD(s.size()), &n, nullptr);
}

void writer()
{
    for (;;) {
        {
            std::unique_lock<std::mutex> g(files_lock);
            wake.wait_for(g, std::chrono::milliseconds(100));
        }
        std::vector<File *> now;
        {
            std::lock_guard<std::mutex> g(files_lock);
            for (auto &f : files)
                if (f && f->handle != INVALID_HANDLE_VALUE)
                    now.push_back(f.get());
        }
        for (File *f : now) {
            f->rec->drain([&](const std::string &s) { write_block(*f, s); });
            if (f->closed) {
                CloseHandle(f->handle);
                f->handle = INVALID_HANDLE_VALUE;
            }
        }
    }
}

File *file(uint32_t h)
{
    std::lock_guard<std::mutex> g(files_lock);
    return h < files.size() && files[h] && files[h]->handle != INVALID_HANDLE_VALUE ? files[h].get() : nullptr;
}

int csv_open(const char *name, const char *const *columns, const uint32_t *types, uint32_t n, uint32_t *handle)
{
    if (!name || !*name || !columns || !types || !handle || !n || n > 512)
        return SWOFTY_ASSET_INVALID;
    for (const char *p = name; *p; p++)
        if (!(isalnum(static_cast<unsigned char>(*p)) || *p == '-' || *p == '_'))
            return SWOFTY_ASSET_INVALID;
    std::string dll, id = settings_addon(__builtin_return_address(0), dll);
    if (id.empty())
        return SWOFTY_ASSET_UNAVAILABLE;
    std::wstring path = runtime::storage_write_path(id, std::string("diag/") + name + "-" + stamp() + ".csv");
    if (path.empty())
        return SWOFTY_ASSET_INVALID;
    HANDLE h = CreateFileW(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) {
        swofty::log("[csv] %s: cannot create %s (error %lu)", id.c_str(), runtime::narrow(path).c_str(), GetLastError());
        return SWOFTY_ASSET_UNAVAILABLE;
    }
    std::vector<std::string> names;
    std::vector<uint32_t> ty;
    for (uint32_t i = 0; i < n; i++) {
        names.emplace_back(columns[i] ? columns[i] : "");
        ty.push_back(types[i] <= runtime::log_csv::BOOL ? types[i] : 0);
    }
    auto f = std::make_unique<File>();
    f->rec = std::make_unique<runtime::log_csv::Recorder>(std::move(names), std::move(ty));
    f->handle = h;
    f->id = id;
    f->name = name;
    write_block(*f, f->rec->header());
    std::call_once(writer_started, [] { std::thread(writer).detach(); });
    std::lock_guard<std::mutex> g(files_lock);
    *handle = uint32_t(files.size());
    files.push_back(std::move(f));
    swofty::log("[csv] %s: recording %s (%u columns) to %s", id.c_str(), name, n, runtime::narrow(path).c_str());
    return SWOFTY_ASSET_OK;
}

int csv_i32(uint32_t h, uint32_t c, int32_t v)
{
    File *f = file(h);
    return f && f->rec->set_i32(c, v) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID;
}

int csv_f32(uint32_t h, uint32_t c, float v)
{
    File *f = file(h);
    return f && f->rec->set_f32(c, v) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID;
}

int csv_text(uint32_t h, uint32_t c, const char *s, uint32_t len)
{
    File *f = file(h);
    return f && s && f->rec->set_text(c, s, len) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_INVALID;
}

int csv_commit(uint32_t h)
{
    File *f = file(h);
    if (!f)
        return SWOFTY_ASSET_INVALID;
    f->rec->commit();
    return SWOFTY_ASSET_OK;
}

uint64_t csv_dropped(uint32_t h)
{
    File *f = file(h);
    return f ? f->rec->dropped() : 0;
}

void csv_close(uint32_t h)
{
    if (File *f = file(h))
        f->closed = true;
}

double csv_clock()
{
    static const double freq = [] {
        LARGE_INTEGER f;
        QueryPerformanceFrequency(&f);
        return double(f.QuadPart);
    }();
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    return double(t.QuadPart) / freq;
}

const SwoftyCsv csv_table = {SWOFTY_CSV_VERSION, csv_open, csv_i32, csv_f32, csv_text, csv_commit, csv_dropped, csv_close, csv_clock};

}

extern "C" __declspec(dllexport) const SwoftyCsv *SwoftyCoreCsv(void)
{
    return &csv_table;
}
