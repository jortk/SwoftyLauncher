// Builds src/core/settings.cpp with the registry store and reads an option the way an addon's hot wrap does.
// The stand-in runtime::older_store() counts every trip settings.cpp makes into the registry: reads must take
// one trip per change, and a read while another thread holds the store's lock must not wait for it.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include "../src/core/registry/older.hpp"
#include "../src/core/registry/table.hpp"
#include "swofty_api.h"

#include <atomic>
#include <chrono>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <thread>

namespace reg = swofty_core::registry;
namespace older = swofty_core::registry::older;
using namespace std::chrono_literals;

void settings_attach(SwoftyApi *a, const std::wstring &dir);
int settings_get(const char *id, uint32_t type, SwoftySettingValue *out);
int settings_watch(const char *id, SwoftySettingFn fn, void *user);
bool settings_command(uint32_t hash);

namespace {

std::atomic<int> trips{0};
int failures = 0;

void check(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    std::fflush(stdout);
    failures += !ok;
}

void test_log(const char *fmt, ...)
{
    va_list ap;
    va_start(ap, fmt);
    std::printf("  log: ");
    std::vprintf(fmt, ap);
    std::printf("\n");
    va_end(ap);
}

uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    for (; *s; s++)
        h = h * 33 + (unsigned char)*s;
    return h;
}

std::string days()
{
    SwoftySettingValue v{};
    return settings_get("days", SWOFTY_SETTING_CHOICE, &v) && v.choice ? v.choice : "<none>";
}

bool toggle()
{
    SwoftySettingValue v{};
    return settings_get("on", SWOFTY_SETTING_TOGGLE, &v) && v.toggle;
}

std::string seen_in_watcher;

void on_days(const char *, const SwoftySettingValue *v, void *)
{
    seen_in_watcher = std::string(v->choice) + "/" + days();
}

int many(int n, std::string *last = nullptr)
{
    int before = trips;
    std::string v;
    for (int i = 0; i < n; i++)
        v = days();
    if (last)
        *last = v;
    return trips - before;
}

std::atomic<bool> block_now{false}, blocked{false}, release{false};

int edit_blocks(void *, SwoftyText in, char *out, size_t cap, size_t *len)
{
    if (block_now.exchange(false)) {
        blocked = true;
        while (!release)
            std::this_thread::sleep_for(1ms);
    }
    *len = in.size;
    if (cap < in.size)
        return SWOFTY_TOO_SMALL;
    std::memcpy(out, in.data, in.size);
    return SWOFTY_ASSET_OK;
}

std::atomic<std::thread::id> flow_id;

bool on_flow() { return std::this_thread::get_id() == flow_id.load(); }

template <class F> bool finishes(F f, std::chrono::milliseconds limit)
{
    auto done = std::make_shared<std::atomic<bool>>(false);
    std::thread([f, done]() mutable {
        f();
        *done = true;
    }).detach();
    for (auto t = std::chrono::steady_clock::now(); std::chrono::steady_clock::now() - t < limit;) {
        if (*done)
            return true;
        std::this_thread::sleep_for(2ms);
    }
    return false;
}

int layer_value(const char *json, uint32_t *handle)
{
    reg::LayerSpec spec;
    spec.addon = "other.addon";
    spec.registry = older::OPTION;
    spec.id = "ex.memo/days";
    spec.change.is_patch = true;
    spec.change.patch = swofty::json::parse(json);
    spec.source = "settings_get_test";
    return reg::store().layer(spec, handle);
}

}

namespace runtime {
reg::Store &older_store()
{
    trips++;
    static bool once = (older::declare(reg::store()), true);
    (void)once;
    return reg::store();
}
}

bool ui_vm_set_path(const std::string &, int, const void *) { return true; }
void hub_option_changed(const std::string &, const std::string &) {}

int main()
{
    wchar_t exe[MAX_PATH];
    DWORD n = GetModuleFileNameW(nullptr, exe, MAX_PATH);
    std::wstring path(exe, n), dir = path.substr(0, path.find_last_of(L"\\/") + 1) + L"settings-get\\";
    std::wstring name = path.substr(path.find_last_of(L"\\/") + 1);
    CreateDirectoryW(dir.c_str(), nullptr);
    CreateDirectoryW((dir + L"a").c_str(), nullptr);
    CreateDirectoryW((dir + L"a\\b").c_str(), nullptr);
    std::string dll(name.begin(), name.end());
    std::string doc = R"({"schema": 1, "profile": "memo", "addons": [{"id": "ex.memo", "dlls": [")" + dll + R"("], "options": [
        {"id": "days", "type": "choice", "current": 0, "states": [
            {"value": "short", "text": "Short", "command": "memo_days_short"},
            {"value": "long", "text": "Long", "command": "memo_days_long"}]},
        {"id": "on", "type": "toggle", "current": 1, "states": [
            {"value": false, "text": "OFF", "command": "memo_on_off"},
            {"value": true, "text": "ON", "command": "memo_on_on"}]}]}]})";
    if (FILE *f = _wfopen((dir + L"settings.json").c_str(), L"wb")) {
        std::fwrite(doc.data(), 1, doc.size(), f);
        std::fclose(f);
    }
    SwoftyApi api{};
    api.log = test_log;
    settings_attach(&api, dir + L"a\\b\\");
    reg::Store &st = reg::store();
    st.declare("test.block", swofty::json::parse(R"({"x-registry":"test.block","type":"object"})"));
    st.add_base("test.block", "b", swofty::json::parse(R"({"n":0})"));
    reg::LayerSpec sp;
    sp.addon = "a.blocker";
    sp.registry = "test.block";
    sp.id = "game/b";
    sp.change.is_patch = false;
    sp.change.fn = edit_blocks;
    st.layer(sp, nullptr);
    st.set_flow_thread(on_flow);
    check(settings_watch("days", on_days, nullptr) == 1, "watch_setting: the watcher is registered");

    std::string last;
    int t = many(200000, &last);
    std::printf("  200000 reads: %d registry trip(s), value %s\n", t, last.c_str());
    check(t == 1 && last == "short", "200000 reads with nothing changed take the registry path once");

    int before = trips;
    bool on = true;
    for (int i = 0; i < 1000; i++)
        on = on && toggle();
    check(on && trips - before == 1, "another option of the same addon keeps its own value: one trip for 1000 reads");

    auto t0 = std::chrono::steady_clock::now();
    many(1000000);
    double hit = std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - t0).count() / 1e6;
    t0 = std::chrono::steady_clock::now();
    for (int i = 0; i < 100000; i++)
        (void)older::effective(st, older::OPTION, "ex.memo/days");
    double registry = std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - t0).count() / 1e5;
    std::printf("  a memo read costs %.1f ns; the registry read the old settings_get made on every call costs %.1f ns\n",
                hit, registry);

    before = trips;
    check(settings_command(djb2("memo_days_long")), "settings command: days = long");
    check(seen_in_watcher == "long/long", "the watcher gets the new value, and a read inside it already gives it");
    t = many(1000, &last);
    check(last == "long" && trips - before == 2 && t <= 1,
          "a settings write invalidates: the put and one read take the registry path, then reads hit the memo");
    before = trips;
    on = toggle();
    check(on && trips - before == 1, "the write moved every option's stamp, so the other option reads once again");

    uint32_t handle = 0;
    check(layer_value(R"({"value":"short"})", &handle) == SWOFTY_ASSET_OK, "another addon layers days = short");
    t = many(1000, &last);
    check(last == "short" && t == 1, "a registry layer invalidates: the layered value after one trip");
    check(st.clear_layer("other.addon", older::OPTION, "ex.memo/days", reg::Scope::global) == SWOFTY_ASSET_OK,
          "the layer is cleared");
    t = many(1000, &last);
    check(last == "long" && t == 1, "clearing the layer invalidates: the player's value again after one trip");

    reg::Context ctx = st.context();
    ctx.profile = "other";
    st.set_context(ctx);
    t = many(1000, &last);
    check(last == "long" && t == 1, "a profile change in the store's context invalidates: one trip");

    t = many(1000, &last);
    check(t == 0, "with nothing changed since, reads take no trip at all");

    block_now = true;
    std::thread holder([&] { (void)st.get("test.block", "game/b"); });
    for (int i = 0; i < 2000 && !blocked; i++)
        std::this_thread::sleep_for(1ms);
    check(blocked, "another thread is inside a registry edit, holding the store's lock");
    bool store_waits = !finishes([&] { (void)st.status(older::OPTION, "ex.memo/days"); }, 300ms);
    check(store_waits, "a registry read now waits for that thread");
    std::atomic<int> got{0};
    bool done = finishes(
        [&] {
            for (int i = 0; i < 100000; i++)
                got += days() == "long";
        },
        3000ms);
    check(done && got == 100000, "100000 settings reads on a third thread finish without waiting for the store's lock");
    release = true;
    holder.join();
    std::this_thread::sleep_for(50ms);

    st.begin_career_load();
    before = trips;
    std::atomic<bool> flow_done{false};
    std::string flow_value;
    std::thread flow([&] {
        flow_id = std::this_thread::get_id();
        flow_value = days();
        flow_done = true;
    });
    std::this_thread::sleep_for(200ms);
    check(!flow_done, "during a career load a read on the flow thread skips the memo and waits, as the store's get does");
    st.end_career_load();
    flow.join();
    check(flow_value == "long" && trips - before == 1, "after the load the flow thread's read finishes with one trip");
    flow_id = std::thread::id();
    t = many(1000, &last);
    check(last == "long" && t <= 1, "after the load reads hit the memo again");

    SwoftySettingValue v{};
    check(settings_get("days", SWOFTY_SETTING_TOGGLE, &v) == 0, "a type mismatch still returns 0");
    check(settings_get("missing", SWOFTY_SETTING_TOGGLE, &v) == 0, "an undeclared option still returns 0");
    std::printf("%s\n", failures ? "settings_get test: FAILED" : "settings_get test: passed");
    return failures ? 1 : 0;
}
