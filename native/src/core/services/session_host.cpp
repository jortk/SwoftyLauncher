#include "../room_cover.hpp"
#include "../runtime.hpp"
#include "../virtual_pad.hpp"

#include <swofty/mixin.hpp>

#include <atomic>
#include <cstring>
#include <string>

// The session host table (swofty_api.h SwoftySessionHost, the core export SwoftyCoreSessionHost): what a
// plugin that sets up an online session needs from the core beyond the SDK. The core's own hooks report
// to the plugin's listeners through the runtime::session_host_* calls below; with no plugin they do nothing.

bool ui_vm_set_path(const std::string &path, int type, const void *value);

namespace runtime {
bool car_category_choose(const std::string &value, std::string &why);
}

namespace {

template <class F> struct Listener {
    std::atomic<F> fn{nullptr};
    std::atomic<void *> user{nullptr};

    void set(F f, void *u)
    {
        fn.store(nullptr);
        user.store(u);
        fn.store(f);
    }
};

Listener<void (*)(void *)> at_ready, at_exit;
Listener<void (*)(uint32_t, uint32_t, void *)> flow;
Listener<void (*)(const char *, void *)> screen;
Listener<int (*)(const char *, void *, void *, void *)> message;
Listener<void (*)(void *, void *)> home;
Listener<SwoftySessionSetupFn> setup;
Listener<SwoftySessionSignalFn> signal;
std::atomic<bool> is_ready{false};

void copy(char *out, size_t cap, const std::string &text)
{
    if (!out || !cap)
        return;
    size_t n = text.size() < cap - 1 ? text.size() : cap - 1;
    std::memcpy(out, text.data(), n);
    out[n] = 0;
}

const wchar_t *api_root()
{
    return runtime::swofty_root().c_str();
}

void api_on_ready(void (*fn)(void *), void *user)
{
    if (is_ready.load()) {
        if (fn)
            fn(user);
        return;
    }
    at_ready.set(fn, user);
}

void api_on_flow(void (*fn)(uint32_t, uint32_t, void *), void *user)
{
    flow.set(fn, user);
}

void api_on_screen(void (*fn)(const char *, void *), void *user)
{
    screen.set(fn, user);
}

void api_on_message(int (*fn)(const char *, void *, void *, void *), void *user)
{
    message.set(fn, user);
}

void api_on_home(void (*fn)(void *, void *), void *user)
{
    home.set(fn, user);
}

void api_on_exit(void (*fn)(void *), void *user)
{
    at_exit.set(fn, user);
}

int api_pad_claim(int on)
{
    runtime::pad::setup();
    if (!runtime::pad::why().empty())
        return 0;
    runtime::pad::claim(on != 0);
    return 1;
}

const char *api_pad_error()
{
    runtime::pad::setup();
    return runtime::pad::why().c_str();
}

void api_pad_set(uint16_t buttons, uint8_t lt, uint8_t rt)
{
    runtime::pad::set(buttons, lt, rt);
}

void api_pad_release()
{
    runtime::pad::release();
}

runtime::direct_start::Setup poll_setup(std::string &why)
{
    auto fn = setup.fn.load();
    if (!fn) {
        why = "the session plugin dropped its setup";
        return runtime::direct_start::Setup::failed;
    }
    char text[512] = {};
    int r = fn(setup.user.load(), text, sizeof text);
    if (r == 1)
        return runtime::direct_start::Setup::done;
    if (r == 0)
        return runtime::direct_start::Setup::pending;
    why = text;
    return runtime::direct_start::Setup::failed;
}

void api_direct_start_setup(SwoftySessionSetupFn fn, void *user)
{
    setup.set(fn, user);
    runtime::direct_start_register_room(fn ? &poll_setup : nullptr);
}

void api_cover_step(const char *what, float fraction, float ceiling)
{
    if (what)
        runtime::room_cover::step(what, fraction, ceiling);
}

int api_cover_state(char *label, size_t cap, int *percent)
{
    auto s = runtime::room_cover::snapshot();
    if (!s.active)
        return 0;
    copy(label, cap, s.label);
    if (percent)
        *percent = s.percent;
    return 1;
}

void api_cover_signal(SwoftySessionSignalFn fn, void *user)
{
    signal.set(fn, user);
}

int api_ui_set(const char *path, int type, const void *value)
{
    return path && value && ui_vm_set_path(path, type, value);
}

int api_car_category(const char *value, char *why, size_t cap)
{
    std::string reason;
    if (runtime::car_category_choose(value ? value : "", reason))
        return 1;
    copy(why, cap, reason);
    return 0;
}

const SwoftySessionHost g_table = {
    sizeof(SwoftySessionHost), SWOFTY_SESSION_HOST_VERSION, api_root,        api_on_ready,    api_on_flow,
    api_on_screen,             api_on_message,              api_on_home,     api_on_exit,     api_pad_claim,
    api_pad_error,             api_pad_set,                 api_pad_release, api_direct_start_setup,
    api_cover_step,            api_cover_state,             api_cover_signal, api_ui_set,     api_car_category,
};

}

extern "C" __declspec(dllexport) const SwoftySessionHost *SwoftyCoreSessionHost(void)
{
    return &g_table;
}

namespace runtime {

void session_host_ready()
{
    if (is_ready.exchange(true))
        return;
    if (auto fn = at_ready.fn.load())
        fn(at_ready.user.load());
}

void session_host_flow(uint32_t hash, uint32_t to)
{
    if (auto fn = flow.fn.load())
        fn(hash, to, flow.user.load());
}

void session_host_screen(const char *name)
{
    if (auto fn = screen.fn.load(); fn && name)
        fn(name, screen.user.load());
}

bool session_host_message(const char *id, void *reply, void *ctx)
{
    auto fn = message.fn.load();
    return fn && id && fn(id, reply, ctx, message.user.load()) != 0;
}

void session_host_home(void *vm)
{
    if (auto fn = home.fn.load(); fn && vm)
        fn(vm, home.user.load());
}

void session_host_exit()
{
    if (auto fn = at_exit.fn.load())
        fn(at_exit.user.load());
}

int session_host_cover_signal()
{
    auto fn = signal.fn.load();
    return fn ? fn(signal.user.load()) : -1;
}

}
