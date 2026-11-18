#include "d3d12_hooks.hpp"

#include "hud_guard.hpp"
#include "runtime.hpp"
#include "rtas_guard_logic.hpp"
#include "srv_clamp_logic.hpp"
#include "perf.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <algorithm>
#include <atomic>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <unordered_set>
#include <vector>

namespace d3d12hooks {

namespace {

template <class Fn> struct Subs {
    Fn fns[16] = {};
    std::atomic<int> n{0};

    void add(Fn fn)
    {
        int at = n.load();
        if (at < 16) {
            fns[at] = fn;
            n.store(at + 1);
        }
    }
};

Subs<PresentFn> presents;
Subs<PresentedFn> presenteds;
Subs<ResizeFn> resizes;
Subs<ExecFn> execs;
Subs<BarrierFn> barriers;
Subs<SetNameFn> setnames;
Subs<TextureCopyFn> texture_copies;
Subs<ResetFn> resets;
Subs<RenderTargetsFn> render_targets;
Subs<RtvFn> rtvs;
Subs<UnreadQueueFn> unread_queues;

template <class T> void release(T *&p)
{
    if (p)
        p->Release();
    p = nullptr;
}

std::mutex lock;
std::vector<Swapchain> chains;
std::vector<std::pair<void *, UINT>> pending_color_space;

safetyhook::InlineHook h_present, h_present1, h_resize, h_resize1, h_color_space, h_create, h_create_hwnd, h_create_core,
    h_create_comp, h_exec, h_barrier, h_setname, h_texture_copy, h_resource_copy;
HMODULE impl_create, impl_hwnd, impl_core, impl_comp;
constexpr size_t RESOURCE_GETDESC_SLOT = 10, SRV_SLOT = 19, SETNAME_SLOT = 6, EXEC_SLOT = 10, BARRIER_SLOT = 26, RTV_SLOT = 20, OM_SLOT = 46;
constexpr size_t TEXTURE_COPY_SLOT = 16, RESOURCE_COPY_SLOT = 17;
thread_local int creating, presenting;
std::atomic<bool> in_present;

std::string self_module()
{
    static std::string name = module_of(reinterpret_cast<const void *>(&self_module));
    return name;
}

void captured(const char *how, IUnknown *device, void *sc, UINT w, UINT h, DXGI_FORMAT fmt, UINT buffers, HMODULE impl)
{
    void *frames[48];
    USHORT n = RtlCaptureStackBackTrace(0, 48, frames, nullptr);
    std::vector<std::string> mods;
    for (USHORT i = 0; i < n; i++)
        mods.push_back(module_of(frames[i]));
    char impl_path[MAX_PATH] = {};
    DWORD impl_n = impl ? GetModuleFileNameA(impl, impl_path, MAX_PATH) : 0;
    std::string impl_name(hud::guard::base_name(std::string_view(impl_path, impl_n)));
    std::string by = hud::guard::chain(mods, self_module(), impl_name);
    ID3D12CommandQueue *q = nullptr;
    if (!device || FAILED(device->QueryInterface(IID_PPV_ARGS(&q))) || !q) {
        swofty::log("[d3d12] swapchain %p (%s, by %s) was not created on a D3D12 command queue; ignored", sc, how,
                    by.c_str());
        return;
    }
    D3D12_COMMAND_QUEUE_DESC qd = q->GetDesc();
    q->Release();
    Swapchain c;
    c.sc = sc;
    c.queue = q;
    c.how = how;
    c.by = by;
    c.fg = hud::guard::fg_in(mods);
    c.streamline = hud::guard::streamline_in(mods);
    // FidelityFX's loader is on the stack whatever the settings, so it does not stand in for a non-direct queue.
    if (qd.Type != D3D12_COMMAND_LIST_TYPE_DIRECT && (c.fg.empty() || hud::guard::ffx_loader(c.fg)))
        c.fg = "a non-direct present queue";
    {
        std::lock_guard<std::mutex> g(lock);
        auto pend = std::find_if(pending_color_space.begin(), pending_color_space.end(),
                                 [&](const std::pair<void *, UINT> &x) { return x.first == sc; });
        if (pend != pending_color_space.end()) {
            c.color_space = pend->second;
            c.color_space_set = true;
            pending_color_space.erase(pend);
        }
        auto it = std::find_if(chains.begin(), chains.end(), [&](const Swapchain &x) { return x.sc == sc; });
        if (it != chains.end())
            *it = c;
        else
            chains.push_back(c);
    }
    swofty::log("[d3d12] swapchain %p created through %s on queue %p (type %d, %ux%u, format %d, %u buffer(s)); "
                "created by %s%s%s",
                sc, how, static_cast<void *>(q), int(qd.Type), w, h, int(fmt), buffers, by.c_str(),
                c.fg.empty() ? (c.streamline ? "; through Streamline's frame generation hook" : "")
                             : "; frame generation: ",
                c.fg.c_str());
}

HRESULT STDMETHODCALLTYPE create_hook(IDXGIFactory *f, IUnknown *dev, DXGI_SWAP_CHAIN_DESC *d, IDXGISwapChain **out)
{
    creating++;
    HRESULT hr = h_create.call<HRESULT>(f, dev, d, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("IDXGIFactory::CreateSwapChain", dev, *out, d->BufferDesc.Width, d->BufferDesc.Height,
                 d->BufferDesc.Format, d->BufferCount, impl_create);
    return hr;
}

HRESULT STDMETHODCALLTYPE create_hwnd_hook(IDXGIFactory2 *f, IUnknown *dev, HWND wnd, const DXGI_SWAP_CHAIN_DESC1 *d,
                                           const DXGI_SWAP_CHAIN_FULLSCREEN_DESC *fs, IDXGIOutput *o,
                                           IDXGISwapChain1 **out)
{
    creating++;
    HRESULT hr = h_create_hwnd.call<HRESULT>(f, dev, wnd, d, fs, o, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("CreateSwapChainForHwnd", dev, *out, d->Width, d->Height, d->Format, d->BufferCount, impl_hwnd);
    return hr;
}

HRESULT STDMETHODCALLTYPE create_core_hook(IDXGIFactory2 *f, IUnknown *dev, IUnknown *wnd,
                                           const DXGI_SWAP_CHAIN_DESC1 *d, IDXGIOutput *o, IDXGISwapChain1 **out)
{
    creating++;
    HRESULT hr = h_create_core.call<HRESULT>(f, dev, wnd, d, o, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("CreateSwapChainForCoreWindow", dev, *out, d->Width, d->Height, d->Format, d->BufferCount, impl_core);
    return hr;
}

HRESULT STDMETHODCALLTYPE create_comp_hook(IDXGIFactory2 *f, IUnknown *dev, const DXGI_SWAP_CHAIN_DESC1 *d,
                                           IDXGIOutput *o, IDXGISwapChain1 **out)
{
    creating++;
    HRESULT hr = h_create_comp.call<HRESULT>(f, dev, d, o, out);
    if (--creating == 0 && SUCCEEDED(hr) && out && *out && d)
        captured("CreateSwapChainForComposition", dev, *out, d->Width, d->Height, d->Format, d->BufferCount,
                 impl_comp);
    return hr;
}

using SlGetNativeInterface = int(__cdecl *)(void *proxy, void **native);

std::mutex execs_lock;
std::vector<hud::guard::ExecSeen> execs_seen;
std::atomic<bool> execs_wanted;

void count_exec(ID3D12CommandQueue *q)
{
    DWORD tid = GetCurrentThreadId();
    ULONGLONG now = GetTickCount64();
    auto same = [&](const hud::guard::ExecSeen &e) { return e.queue == q; };
    {
        std::lock_guard<std::mutex> g(execs_lock);
        auto it = std::find_if(execs_seen.begin(), execs_seen.end(), same);
        if (it != execs_seen.end()) {
            if (it->type == hud::guard::LIST_TYPE_DIRECT) {
                it->tid = tid;
                it->tick = now;
            }
            return;
        }
        if (execs_seen.size() >= 8)
            return;
    }
    int type = int(q->GetDesc().Type);
    std::lock_guard<std::mutex> g(execs_lock);
    if (execs_seen.size() < 8 && std::none_of(execs_seen.begin(), execs_seen.end(), same))
        execs_seen.push_back({q, type, tid, now});
}

bool recent_direct_queue(DWORD tid, hud::guard::ExecSeen &out)
{
    std::lock_guard<std::mutex> g(execs_lock);
    const hud::guard::ExecSeen *e = hud::guard::present_queue_from_execs(execs_seen, tid);
    if (e)
        out = *e;
    return e != nullptr;
}

std::vector<void *> refused, waiting;
bool unread_arm_asked;

bool listed(const std::vector<void *> &v, void *sc)
{
    return std::find(v.begin(), v.end(), sc) != v.end();
}

bool learn(IDXGISwapChain3 *sc, void *caller, const std::string &pfg, Swapchain &out)
{
    if (listed(refused, sc))
        return false;
    DWORD tid = GetCurrentThreadId();
    ID3D12CommandQueue *q = nullptr;
    HRESULT hr = sc->GetDevice(IID_PPV_ARGS(&q));
    const char *via = "IDXGISwapChain::GetDevice at Present";
    std::string native_note;
    if (FAILED(hr) || !q) {
        q = nullptr;
        HMODULE sl = GetModuleHandleW(L"sl.interposer.dll");
        auto get_native = sl ? reinterpret_cast<SlGetNativeInterface>(GetProcAddress(sl, "slGetNativeInterface"))
                             : nullptr;
        if (!sl)
            native_note = "sl.interposer.dll is not loaded";
        else if (!get_native)
            native_note = "sl.interposer.dll has no slGetNativeInterface";
        else {
            void *native = nullptr;
            int sr = get_native(sc, &native);
            if (!hud::guard::streamline_result_ok(sr) || !native) {
                char buf[64];
                snprintf(buf, sizeof buf, "slGetNativeInterface returned %d", sr);
                native_note = buf;
            } else {
                HRESULT nhr = static_cast<IDXGISwapChain *>(native)->GetDevice(IID_PPV_ARGS(&q));
                if (FAILED(nhr) || !q) {
                    q = nullptr;
                    char buf[96];
                    snprintf(buf, sizeof buf, "native swapchain %p GetDevice failed: 0x%08lx", native,
                             static_cast<unsigned long>(nhr));
                    native_note = buf;
                } else
                    via = "slGetNativeInterface + IDXGISwapChain::GetDevice at Present";
            }
        }
    }
    D3D12_COMMAND_QUEUE_DESC qd{};
    hud::guard::ExecSeen exec{};
    if (q) {
        qd = q->GetDesc();
        q->Release();
    } else {
        bool told = listed(waiting, sc);
        if (!told && waiting.size() < 16)
            waiting.push_back(sc);
        using hud::guard::UnreadQueue;
        UnreadQueue step = hud::guard::unread_queue_step(bool(h_exec), unread_arm_asked, false);
        if (step == UnreadQueue::Arm) {
            unread_arm_asked = true;
            for (int i = 0, k = unread_queues.n.load(); i < k; i++)
                if (unread_queues.fns[i](sc, caller))
                    break;
            step = hud::guard::unread_queue_step(bool(h_exec), unread_arm_asked, false);
        }
        if (step == UnreadQueue::Refuse) {
            refused.push_back(sc);
            swofty::log("[d3d12] swapchain %p was made before the creation hooks and its present queue cannot be read; "
                        "ignored (GetDevice: 0x%08lx; %s)",
                        static_cast<void *>(sc), static_cast<unsigned long>(hr), native_note.c_str());
            return false;
        }
        execs_wanted.store(true, std::memory_order_relaxed);
        bool seen = recent_direct_queue(tid, exec);
        if (hud::guard::unread_queue_step(true, unread_arm_asked, seen) == UnreadQueue::Wait) {
            if (!told)
                swofty::log("[d3d12] swapchain %p was made before the creation hooks and its present queue cannot be "
                            "read (GetDevice: 0x%08lx; %s); no direct command queue has executed command lists yet, "
                            "so the overlay waits",
                            static_cast<void *>(sc), static_cast<unsigned long>(hr), native_note.c_str());
            return false;
        }
        q = static_cast<ID3D12CommandQueue *>(exec.queue);
        qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
        via = exec.tid == tid ? "ID3D12CommandQueue::ExecuteCommandLists on the presenting thread"
                              : "ID3D12CommandQueue::ExecuteCommandLists, most recent direct queue";
    }
    std::string by = module_of(caller);
    out = Swapchain();
    out.sc = sc;
    out.queue = q;
    out.how = via;
    out.by = by;
    out.fg = hud::guard::learned_fg(qd.Type == D3D12_COMMAND_LIST_TYPE_DIRECT, pfg);
    out.streamline = hud::guard::streamline_caller(by);
    out.armed_late = exec.queue && unread_arm_asked;
    {
        std::lock_guard<std::mutex> g(lock);
        auto pend = std::find_if(pending_color_space.begin(), pending_color_space.end(),
                                 [&](const std::pair<void *, UINT> &x) { return x.first == sc; });
        if (pend != pending_color_space.end()) {
            out.color_space = pend->second;
            out.color_space_set = true;
            pending_color_space.erase(pend);
        }
        auto it = std::find_if(chains.begin(), chains.end(), [&](const Swapchain &x) { return x.sc == sc; });
        if (it != chains.end())
            *it = out;
        else
            chains.push_back(out);
    }
    if (exec.queue)
        swofty::log("[d3d12] swapchain %p was made before the creation hooks; its present queue %p is the direct queue "
                    "that executed command lists on thread %lu %llu ms before Present on thread %lu; presented from "
                    "%s%s%s",
                    static_cast<void *>(sc), static_cast<void *>(q), static_cast<unsigned long>(exec.tid),
                    static_cast<unsigned long long>(GetTickCount64() - exec.tick), static_cast<unsigned long>(tid),
                    by.c_str(), out.streamline ? "; through Streamline" : "",
                    out.armed_late ? "; the device hooks armed for it after the game named its back buffers, so the "
                                     "overlay draws on it at Present"
                                   : "");
    else
        swofty::log("[d3d12] swapchain %p was made before the creation hooks; its queue %p (type %d) is read from the "
                    "swapchain; presented from %s%s",
                    static_cast<void *>(sc), static_cast<void *>(q), int(qd.Type), by.c_str(),
                    out.streamline ? "; through Streamline" : "");
    return true;
}

std::string fg_caller(void *caller)
{
    static void *last;
    static std::string last_fg;
    if (caller == last)
        return last_fg;
    std::string name = module_of(caller);
    last = caller;
    last_fg = hud::guard::fg_module(name) && !hud::guard::streamline_hook(name) ? name : std::string();
    return last_fg;
}

struct PresentSeen {
    void *sc, *caller;
    DWORD tid;
    const char *how;
};

std::mutex present_lock;
std::vector<PresentSeen> present_seen;
ULONGLONG present_since;
unsigned present_count;
std::atomic<uint64_t> frames_presented{0};

void count_present(IDXGISwapChain3 *sc, void *caller, const char *how)
{
    DWORD tid = GetCurrentThreadId();
    ULONGLONG now = GetTickCount64();
    std::lock_guard<std::mutex> g(present_lock);
    present_count++;
    frames_presented.fetch_add(1, std::memory_order_relaxed);
    if (!present_since)
        present_since = now;
    bool fresh = present_seen.size() < 32 &&
                 std::none_of(present_seen.begin(), present_seen.end(), [&](const PresentSeen &p) {
                     return p.sc == sc && p.caller == caller && p.tid == tid && p.how == how;
                 });
    if (fresh) {
        present_seen.push_back({sc, caller, tid, how});
        swofty::log("[d3d12] %s on swapchain %p from %s (%p) on thread %lu", how, static_cast<void *>(sc),
                    module_of(caller).c_str(), caller, tid);
    }
    if (now - present_since >= 60000) {
        swofty::log("[d3d12] last %llu s: %u present(s), %.1f per second",
                    static_cast<unsigned long long>((now - present_since) / 1000), present_count,
                    present_count * 1000.0 / double(now - present_since));
        present_since = now;
        present_count = 0;
    }
}

// Present and Present1 both close a frame. One that runs inside the other on the same thread, as DXVK's Present
// calls its Present1, is the same frame and is dispatched once, by the outer call.
template <class Original>
HRESULT dispatch_present(IDXGISwapChain3 *sc, UINT sync, UINT flags, void *caller, const char *how, Original original)
{
    if (!hud::guard::dispatches_present(presenting))
        return original();
    uint64_t entry = perf::stamp();
    presenting++;
    if (!(flags & DXGI_PRESENT_TEST))
        count_present(sc, caller, how);
    if (!(flags & DXGI_PRESENT_TEST) && presents.n.load() && !in_present.exchange(true)) {
        std::string pfg = fg_caller(caller);
        Swapchain c;
        bool known = false;
        {
            std::lock_guard<std::mutex> g(lock);
            auto it = std::find_if(chains.begin(), chains.end(), [&](const Swapchain &x) { return x.sc == sc; });
            if (it != chains.end()) {
                if (it->fg.empty() && !pfg.empty())
                    it->fg = pfg + " (calls Present)";
                c = *it;
                known = true;
            }
        }
        if (!known)
            known = learn(sc, caller, pfg, c);
        for (int i = 0, n = presents.n.load(); i < n; i++)
            presents.fns[i](sc, known ? &c : nullptr, caller);
        in_present.store(false);
    }
    uint64_t before = perf::stamp();
    HRESULT hr = original();
    if (!(flags & DXGI_PRESENT_TEST))
        perf::presented(sc, sync, flags, hr, entry, before, perf::stamp());
    for (int i = 0, n = presenteds.n.load(); i < n; i++)
        presenteds.fns[i](sc, hr);
    presenting--;
    return hr;
}

HRESULT STDMETHODCALLTYPE present_hook(IDXGISwapChain3 *sc, UINT sync, UINT flags)
{
    return dispatch_present(sc, sync, flags, __builtin_return_address(0), "Present",
                            [&] { return h_present.call<HRESULT>(sc, sync, flags); });
}

HRESULT STDMETHODCALLTYPE present1_hook(IDXGISwapChain3 *sc, UINT sync, UINT flags,
                                        const DXGI_PRESENT_PARAMETERS *params)
{
    return dispatch_present(sc, sync, flags, __builtin_return_address(0), "Present1",
                            [&] { return h_present1.call<HRESULT>(sc, sync, flags, params); });
}

HRESULT STDMETHODCALLTYPE resize_hook(IDXGISwapChain3 *sc, UINT count, UINT w, UINT h, DXGI_FORMAT fmt, UINT flags)
{
    for (int i = 0, n = resizes.n.load(); i < n; i++)
        resizes.fns[i](sc);
    return h_resize.call<HRESULT>(sc, count, w, h, fmt, flags);
}

HRESULT STDMETHODCALLTYPE resize1_hook(IDXGISwapChain3 *sc, UINT count, UINT w, UINT h, DXGI_FORMAT fmt, UINT flags,
                                       const UINT *mask, IUnknown *const *queues)
{
    for (int i = 0, n = resizes.n.load(); i < n; i++)
        resizes.fns[i](sc);
    if (queues && count) {
        std::lock_guard<std::mutex> g(lock);
        for (auto &c : chains)
            if (c.sc == sc && c.fg.empty()) {
                c.fg = "ResizeBuffers1 with per-buffer present queues";
                swofty::log("[d3d12] swapchain %p now presents through per-buffer queues; features leave it alone",
                            static_cast<void *>(sc));
            }
    }
    return h_resize1.call<HRESULT>(sc, count, w, h, fmt, flags, mask, queues);
}

HRESULT STDMETHODCALLTYPE color_space_hook(IDXGISwapChain3 *sc, DXGI_COLOR_SPACE_TYPE cs)
{
    HRESULT hr = h_color_space.call<HRESULT>(sc, cs);
    std::lock_guard<std::mutex> g(lock);
    auto it = std::find_if(chains.begin(), chains.end(), [&](const Swapchain &x) { return x.sc == sc; });
    if (it == chains.end()) {
        auto pend = std::find_if(pending_color_space.begin(), pending_color_space.end(),
                                 [&](const std::pair<void *, UINT> &x) { return x.first == sc; });
        if (SUCCEEDED(hr)) {
            if (pend != pending_color_space.end())
                pend->second = UINT(cs);
            else if (pending_color_space.size() < 16)
                pending_color_space.push_back({sc, UINT(cs)});
        }
        swofty::log("[d3d12] SetColorSpace1(%d) on swapchain %p before its creation was recorded -> 0x%08lx",
                    int(cs), static_cast<void *>(sc), static_cast<unsigned long>(hr));
    } else if (SUCCEEDED(hr) && (!it->color_space_set || it->color_space != UINT(cs))) {
        it->color_space = UINT(cs);
        it->color_space_set = true;
        swofty::log("[d3d12] swapchain %p colour space %d", static_cast<void *>(sc), int(cs));
    } else if (FAILED(hr)) {
        swofty::log("[d3d12] SetColorSpace1(%d) on swapchain %p failed: 0x%08lx", int(cs), static_cast<void *>(sc),
                    static_cast<unsigned long>(hr));
    }
    return hr;
}

void STDMETHODCALLTYPE exec_hook(ID3D12CommandQueue *q, UINT n, ID3D12CommandList *const *lists)
{
    if (execs_wanted.load(std::memory_order_relaxed))
        count_exec(q);
    ExecPlan plan;
    for (int i = 0, k = execs.n.load(); i < k; i++)
        execs.fns[i](q, n, lists, plan);
    run_plan(plan, q, n, lists, [](ID3D12CommandQueue *queue, UINT count, ID3D12CommandList *const *first) {
        perf::Pause game;
        h_exec.call<void>(queue, count, first);
    });
}

Subs<BarrierObserverFn> barrier_observers;

void STDMETHODCALLTYPE barrier_hook(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *b)
{
    for (int i = 0, k = barrier_observers.n.load(); i < k; i++)
        barrier_observers.fns[i](list, n, b);
    for (int i = 0, k = barriers.n.load(); i < k; i++)
        if (barriers.fns[i](list, n, b))
            return;
    h_barrier.call<void>(list, n, b);
}

enum : uint8_t { ARG_TABLE = 1, ARG_CONSTANTS, ARG_CBV, ARG_SRV, ARG_UAV };
enum : size_t {
    RESET_SLOT = 10,
    CLEAR_STATE_SLOT = 11,
    PSO_SLOT = 25,
    HEAPS_SLOT = 28,
    COMPUTE_ROOT_SLOT = 29,
    GRAPHICS_ROOT_SLOT = 30,
    COMPUTE_TABLE_SLOT = 31,
    GRAPHICS_TABLE_SLOT = 32,
    COMPUTE_CONSTANT_SLOT = 33,
    COMPUTE_CONSTANTS_SLOT = 35,
    COMPUTE_CBV_SLOT = 37,
    COMPUTE_SRV_SLOT = 39,
    COMPUTE_UAV_SLOT = 41,
    PSO1_SLOT = 75,
};

safetyhook::InlineHook h_reset, h_clear_state, h_pso, h_heaps, h_compute_root, h_compute_table, h_compute_constant,
    h_compute_constants, h_compute_cbv, h_compute_srv, h_compute_uav, h_pso1, h_graphics_root, h_graphics_table;
std::mutex states_lock;
std::vector<std::pair<ID3D12GraphicsCommandList *, ListState *>> states;
thread_local ID3D12GraphicsCommandList *state_list;
thread_local ListState *state_of;

// A list's state, made on first sight. A list first seen anywhere but Reset is not known until its
// next Reset, as what it bound before the hooks saw it is lost.
ListState *tracked(ID3D12GraphicsCommandList *l)
{
    if (l == state_list)
        return state_of;
    std::lock_guard<std::mutex> g(states_lock);
    auto it = std::lower_bound(states.begin(), states.end(), l,
                               [](const std::pair<ID3D12GraphicsCommandList *, ListState *> &e,
                                  ID3D12GraphicsCommandList *k) { return e.first < k; });
    if (it == states.end() || it->first != l)
        it = states.insert(it, {l, new ListState()});
    state_list = l;
    state_of = it->second;
    return state_of;
}

void forget(ListState *s, ID3D12PipelineState *pso)
{
    s->known = true;
    s->pso = pso;
    s->state_object = nullptr;
    s->heaps = 0;
    s->compute_root = nullptr;
    s->args = 0;
    s->graphics_root = nullptr;
    s->tables = 0;
}

RootArg *arg_at(ListState *s, UINT index)
{
    if (index >= 64)
        return nullptr;
    if (index >= s->args) {
        for (UINT i = s->args; i <= index; i++)
            s->arg[i].kind = 0;
        s->args = index + 1;
    }
    return &s->arg[index];
}

HRESULT STDMETHODCALLTYPE reset_hook(ID3D12GraphicsCommandList *l, ID3D12CommandAllocator *a, ID3D12PipelineState *p)
{
    HRESULT hr = h_reset.call<HRESULT>(l, a, p);
    if (SUCCEEDED(hr)) {
        forget(tracked(l), p);
        for (int i = 0, k = resets.n.load(); i < k; i++)
            resets.fns[i](l);
    }
    return hr;
}

void STDMETHODCALLTYPE clear_state_hook(ID3D12GraphicsCommandList *l, ID3D12PipelineState *p)
{
    h_clear_state.call<void>(l, p);
    ListState *s = tracked(l);
    bool known = s->known;
    forget(s, p);
    s->known = known;
}

void STDMETHODCALLTYPE pso_hook(ID3D12GraphicsCommandList *l, ID3D12PipelineState *p)
{
    h_pso.call<void>(l, p);
    ListState *s = tracked(l);
    s->pso = p;
    s->state_object = nullptr;
}

void STDMETHODCALLTYPE pso1_hook(ID3D12GraphicsCommandList *l, ID3D12StateObject *o)
{
    h_pso1.call<void>(l, o);
    ListState *s = tracked(l);
    s->state_object = o;
    s->pso = nullptr;
}

void STDMETHODCALLTYPE heaps_hook(ID3D12GraphicsCommandList *l, UINT n, ID3D12DescriptorHeap *const *h)
{
    h_heaps.call<void>(l, n, h);
    ListState *s = tracked(l);
    s->heaps = std::min<UINT>(n, 2);
    for (UINT i = 0; i < s->heaps; i++)
        s->heap[i] = h[i];
}

void STDMETHODCALLTYPE compute_root_hook(ID3D12GraphicsCommandList *l, ID3D12RootSignature *r)
{
    h_compute_root.call<void>(l, r);
    ListState *s = tracked(l);
    if (s->compute_root != r)
        s->args = 0;
    s->compute_root = r;
}

void STDMETHODCALLTYPE graphics_root_hook(ID3D12GraphicsCommandList *l, ID3D12RootSignature *r)
{
    h_graphics_root.call<void>(l, r);
    ListState *s = tracked(l);
    if (s->graphics_root != r)
        s->tables = 0;
    s->graphics_root = r;
}

void STDMETHODCALLTYPE graphics_table_hook(ID3D12GraphicsCommandList *l, UINT i, D3D12_GPU_DESCRIPTOR_HANDLE h)
{
    h_graphics_table.call<void>(l, i, h);
    if (i >= 64)
        return;
    ListState *s = tracked(l);
    for (UINT k = s->tables; k < i; k++)
        s->table[k] = 0;
    s->table[i] = h.ptr;
    s->tables = std::max(s->tables, i + 1);
}

void STDMETHODCALLTYPE compute_table_hook(ID3D12GraphicsCommandList *l, UINT i, D3D12_GPU_DESCRIPTOR_HANDLE h)
{
    h_compute_table.call<void>(l, i, h);
    if (RootArg *a = arg_at(tracked(l), i)) {
        a->kind = ARG_TABLE;
        a->value = h.ptr;
    }
}

void STDMETHODCALLTYPE compute_constant_hook(ID3D12GraphicsCommandList *l, UINT i, UINT v, UINT at)
{
    h_compute_constant.call<void>(l, i, v, at);
    RootArg *a = arg_at(tracked(l), i);
    if (a && at < 64) {
        if (a->kind != ARG_CONSTANTS)
            a->count = 0;
        a->kind = ARG_CONSTANTS;
        a->c[at] = v;
        a->count = std::max(a->count, at + 1);
    }
}

void STDMETHODCALLTYPE compute_constants_hook(ID3D12GraphicsCommandList *l, UINT i, UINT n, const void *v, UINT at)
{
    h_compute_constants.call<void>(l, i, n, v, at);
    RootArg *a = arg_at(tracked(l), i);
    if (a && v && at + n <= 64) {
        if (a->kind != ARG_CONSTANTS)
            a->count = 0;
        a->kind = ARG_CONSTANTS;
        memcpy(&a->c[at], v, size_t(n) * 4);
        a->count = std::max(a->count, at + n);
    }
}

template <uint8_t KIND, safetyhook::InlineHook *H>
void STDMETHODCALLTYPE compute_view_hook(ID3D12GraphicsCommandList *l, UINT i, D3D12_GPU_VIRTUAL_ADDRESS va)
{
    H->call<void>(l, i, va);
    if (RootArg *a = arg_at(tracked(l), i)) {
        a->kind = KIND;
        a->value = va;
    }
}

safetyhook::InlineHook h_rtv, h_om;

void STDMETHODCALLTYPE rtv_hook(ID3D12Device *d, ID3D12Resource *r, const D3D12_RENDER_TARGET_VIEW_DESC *desc,
                                D3D12_CPU_DESCRIPTOR_HANDLE at)
{
    h_rtv.call<void>(d, r, desc, at);
    for (int i = 0, k = rtvs.n.load(); i < k; i++)
        rtvs.fns[i](r, at);
}

void STDMETHODCALLTYPE om_hook(ID3D12GraphicsCommandList *l, UINT n, const D3D12_CPU_DESCRIPTOR_HANDLE *rtv,
                               BOOL single, const D3D12_CPU_DESCRIPTOR_HANDLE *dsv)
{
    for (int i = 0, k = render_targets.n.load(); i < k; i++)
        render_targets.fns[i](l, n, rtv, single);
    h_om.call<void>(l, n, rtv, single, dsv);
}

HRESULT STDMETHODCALLTYPE setname_hook(ID3D12Object *o, LPCWSTR name)
{
    for (int i = 0, k = setnames.n.load(); i < k; i++)
        setnames.fns[i](o, name);
    return h_setname.call<HRESULT>(o, name);
}

void STDMETHODCALLTYPE texture_copy_hook(ID3D12GraphicsCommandList *list, const D3D12_TEXTURE_COPY_LOCATION *dst,
                                         UINT x, UINT y, UINT z, const D3D12_TEXTURE_COPY_LOCATION *src, const D3D12_BOX *box)
{
    h_texture_copy.call<void>(list, dst, x, y, z, src, box);
    for (int i = 0, n = texture_copies.n.load(); i < n; ++i) texture_copies.fns[i](list, dst, x, y, z, src, box);
}

void STDMETHODCALLTYPE resource_copy_hook(ID3D12GraphicsCommandList *list, ID3D12Resource *dst_resource, ID3D12Resource *src_resource)
{
    h_resource_copy.call<void>(list, dst_resource, src_resource);
    D3D12_TEXTURE_COPY_LOCATION dst{}, src{};
    dst.pResource = dst_resource; dst.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    src.pResource = src_resource; src.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    for (int i = 0, n = texture_copies.n.load(); i < n; ++i) texture_copies.fns[i](list, &dst, 0, 0, 0, &src, nullptr);
}

using SrvFn = void(STDMETHODCALLTYPE *)(ID3D12Device *, ID3D12Resource *, const D3D12_SHADER_RESOURCE_VIEW_DESC *, SIZE_T);
SrvFn srv_orig;
std::mutex srv_lock;
std::unordered_set<uint64_t> srv_seen;

const GUID kNameW = {0x4cca5fd8, 0x921f, 0x42c8, {0x85, 0x66, 0x70, 0xca, 0xf2, 0xa9, 0xb7, 0x41}};

std::string srv_name(ID3D12Resource *r)
{
    wchar_t w[128] = {};
    UINT size = sizeof w - sizeof(wchar_t);
    char out[160] = {};
    if (SUCCEEDED(r->GetPrivateData(kNameW, &size, w)) && size >= sizeof(wchar_t)) {
        int n = WideCharToMultiByte(CP_UTF8, 0, w, int(size / sizeof(wchar_t)), out, int(sizeof out) - 1, nullptr,
                                    nullptr);
        if (n > 0)
            return std::string(out, strnlen(out, size_t(n)));
    }
    char b[48];
    snprintf(b, sizeof b, "%p", static_cast<void *>(r));
    return b;
}

void srv_report(ID3D12Resource *r, const runtime::srv_clamp::View &v, const runtime::srv_clamp::Resource &res,
                const runtime::srv_clamp::Plan &p)
{
    using runtime::srv_clamp::Action;
    uint64_t key = (reinterpret_cast<uint64_t>(r) * 0x9e3779b97f4a7c15ull) ^ (uint64_t(v.slices) << 40) ^
                   (uint64_t(v.first_slice) << 20) ^ (uint64_t(v.mip_levels) << 8) ^ v.most_detailed_mip;
    {
        std::lock_guard<std::mutex> g(srv_lock);
        if (srv_seen.size() >= 4096 || !srv_seen.insert(key).second)
            return;
    }
    std::string name = srv_name(r);
    if (p.action == Action::Skip)
        swofty::log("[gfx] srv: array view from slice %u on a %u-slice texture %s; skipped", v.first_slice, res.slices,
                    name.c_str());
    else if (p.slices_changed)
        swofty::log("[gfx] srv: array view of %u slices on a %u-slice texture %s; clamped", v.slices, res.slices,
                    name.c_str());
    else
        swofty::log("[gfx] srv: view of mips %u+%u on a %u-mip texture %s; clamped", v.most_detailed_mip, v.mip_levels,
                    res.mips, name.c_str());
}

void STDMETHODCALLTYPE srv_hook(ID3D12Device *dev, ID3D12Resource *r, const D3D12_SHADER_RESOURCE_VIEW_DESC *d,
                                SIZE_T h)
{
    using namespace runtime::srv_clamp;
    if (!r || !d) {
        srv_orig(dev, r, d, h);
        return;
    }
    using GetDescFn = D3D12_RESOURCE_DESC *(STDMETHODCALLTYPE *)(ID3D12Resource *, D3D12_RESOURCE_DESC *);
    D3D12_RESOURCE_DESC rd{};
    reinterpret_cast<GetDescFn>((*reinterpret_cast<void ***>(r))[RESOURCE_GETDESC_SLOT])(r, &rd);
    View v;
    v.dimension = int(d->ViewDimension);
    switch (d->ViewDimension) {
    case D3D12_SRV_DIMENSION_TEXTURE1D:
        v.most_detailed_mip = d->Texture1D.MostDetailedMip;
        v.mip_levels = d->Texture1D.MipLevels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE1DARRAY:
        v.most_detailed_mip = d->Texture1DArray.MostDetailedMip;
        v.mip_levels = d->Texture1DArray.MipLevels;
        v.first_slice = d->Texture1DArray.FirstArraySlice;
        v.slices = d->Texture1DArray.ArraySize;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE2D:
        v.most_detailed_mip = d->Texture2D.MostDetailedMip;
        v.mip_levels = d->Texture2D.MipLevels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE2DARRAY:
        v.most_detailed_mip = d->Texture2DArray.MostDetailedMip;
        v.mip_levels = d->Texture2DArray.MipLevels;
        v.first_slice = d->Texture2DArray.FirstArraySlice;
        v.slices = d->Texture2DArray.ArraySize;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY:
        v.first_slice = d->Texture2DMSArray.FirstArraySlice;
        v.slices = d->Texture2DMSArray.ArraySize;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE3D:
        v.most_detailed_mip = d->Texture3D.MostDetailedMip;
        v.mip_levels = d->Texture3D.MipLevels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURECUBE:
        v.most_detailed_mip = d->TextureCube.MostDetailedMip;
        v.mip_levels = d->TextureCube.MipLevels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURECUBEARRAY:
        v.most_detailed_mip = d->TextureCubeArray.MostDetailedMip;
        v.mip_levels = d->TextureCubeArray.MipLevels;
        v.first_slice = d->TextureCubeArray.First2DArrayFace;
        v.slices = cube_slices(d->TextureCubeArray.NumCubes);
        break;
    default:
        srv_orig(dev, r, d, h);
        return;
    }
    Resource res;
    res.texture = rd.Dimension != D3D12_RESOURCE_DIMENSION_BUFFER && rd.Dimension != D3D12_RESOURCE_DIMENSION_UNKNOWN;
    res.volume = rd.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE3D;
    res.slices = rd.DepthOrArraySize;
    res.mips = rd.MipLevels;
    Plan p = plan(v, res);
    if (p.action == Action::Pass) {
        srv_orig(dev, r, d, h);
        return;
    }
    srv_report(r, v, res, p);
    if (p.action == Action::Skip)
        return;
    D3D12_SHADER_RESOURCE_VIEW_DESC c = *d;
    switch (d->ViewDimension) {
    case D3D12_SRV_DIMENSION_TEXTURE1D:
        c.Texture1D.MostDetailedMip = p.most_detailed_mip;
        c.Texture1D.MipLevels = p.mip_levels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE1DARRAY:
        c.Texture1DArray.MostDetailedMip = p.most_detailed_mip;
        c.Texture1DArray.MipLevels = p.mip_levels;
        c.Texture1DArray.ArraySize = p.slices;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE2D:
        c.Texture2D.MostDetailedMip = p.most_detailed_mip;
        c.Texture2D.MipLevels = p.mip_levels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE2DARRAY:
        c.Texture2DArray.MostDetailedMip = p.most_detailed_mip;
        c.Texture2DArray.MipLevels = p.mip_levels;
        c.Texture2DArray.ArraySize = p.slices;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY:
        c.Texture2DMSArray.ArraySize = p.slices;
        break;
    case D3D12_SRV_DIMENSION_TEXTURE3D:
        c.Texture3D.MostDetailedMip = p.most_detailed_mip;
        c.Texture3D.MipLevels = p.mip_levels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURECUBE:
        c.TextureCube.MostDetailedMip = p.most_detailed_mip;
        c.TextureCube.MipLevels = p.mip_levels;
        break;
    case D3D12_SRV_DIMENSION_TEXTURECUBEARRAY:
        c.TextureCubeArray.MostDetailedMip = p.most_detailed_mip;
        c.TextureCubeArray.MipLevels = p.mip_levels;
        c.TextureCubeArray.NumCubes = p.slices / 6;
        break;
    default:
        break;
    }
    srv_orig(dev, r, &c, h);
}

// The hook is made disabled and moved into its slot before it goes live, so a detour never runs while the
// slot it calls the original through is still empty.
bool install(safetyhook::InlineHook &h, void *target, void *detour, const char *what)
{
    if (!target || !detour) {
        swofty::log("[d3d12] hooking %s skipped: no target", what);
        return false;
    }
    auto made = safetyhook::InlineHook::create(target, detour, safetyhook::InlineHook::StartDisabled);
    if (!made) {
        swofty::log("[d3d12] hooking %s at %p failed", what, target);
        return false;
    }
    if (!made->original<void *>()) {
        swofty::log("[d3d12] hooking %s at %p failed: no trampoline", what, target);
        return false;
    }
    h = std::move(*made);
    if (!h.enable()) {
        swofty::log("[d3d12] hooking %s at %p failed: the hook did not enable", what, target);
        h = {};
        return false;
    }
    return true;
}

bool hook_factory(void **vt)
{
    struct Want {
        int slot;
        safetyhook::InlineHook *h;
        void *detour;
        HMODULE *impl;
        const char *what;
        bool required;
    } wants[] = {
        {15, &h_create_hwnd, reinterpret_cast<void *>(&create_hwnd_hook), &impl_hwnd, "CreateSwapChainForHwnd", true},
        {10, &h_create, reinterpret_cast<void *>(&create_hook), &impl_create, "CreateSwapChain", false},
        {16, &h_create_core, reinterpret_cast<void *>(&create_core_hook), &impl_core, "CreateSwapChainForCoreWindow",
         false},
        {24, &h_create_comp, reinterpret_cast<void *>(&create_comp_hook), &impl_comp, "CreateSwapChainForComposition",
         false},
    };
    std::vector<void *> done;
    std::string list;
    for (auto &w : wants) {
        void *target = vt[w.slot];
        if (std::find(done.begin(), done.end(), target) != done.end()) {
            swofty::log("[d3d12] IDXGIFactory %s shares its code with another hooked slot; not hooked twice", w.what);
            continue;
        }
        module_of(target, w.impl);
        if (!install(*w.h, target, w.detour, w.what)) {
            if (w.required)
                return false;
            continue;
        }
        done.push_back(target);
        char b[64];
        snprintf(b, sizeof b, "%s%s (vt+0x%x)", list.empty() ? "" : ", ", w.what, unsigned(w.slot * 8));
        list += b;
    }
    swofty::log("[d3d12] swapchain creation hooked on the DXGI factory (%s, in %s)", list.c_str(),
                module_of(vt[15]).c_str());
    return true;
}

using CreateFactoryFn = HRESULT(WINAPI *)(REFIID, void **);

std::mutex probe_lock;
ID3D12Device *probe_held;
int probe_scopes;

ID3D12Device *probe_device()
{
    std::lock_guard<std::mutex> g(probe_lock);
    if (probe_held) {
        probe_held->AddRef();
        return probe_held;
    }
    HMODULE d3d12 = LoadLibraryA("d3d12.dll");
    auto create = d3d12 ? reinterpret_cast<PFN_D3D12_CREATE_DEVICE>(GetProcAddress(d3d12, "D3D12CreateDevice")) : nullptr;
    ID3D12Device *dev = nullptr;
    if (!create || FAILED(create(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&dev))))
        return nullptr;
    if (probe_scopes) {
        dev->AddRef();
        probe_held = dev;
    }
    return dev;
}

}

void on_present(PresentFn fn)
{
    presents.add(fn);
}

void on_presented(PresentedFn fn)
{
    presenteds.add(fn);
}

void on_resize(ResizeFn fn)
{
    resizes.add(fn);
}

void on_exec(ExecFn fn)
{
    execs.add(fn);
}

void on_barrier(BarrierFn fn)
{
    barriers.add(fn);
}

void on_setname(SetNameFn fn)
{
    setnames.add(fn);
}

void on_texture_copy(TextureCopyFn fn) { texture_copies.add(fn); }

void on_render_targets(RenderTargetsFn fn)
{
    render_targets.add(fn);
}

void on_create_rtv(RtvFn fn)
{
    rtvs.add(fn);
}

void on_reset(ResetFn fn)
{
    resets.add(fn);
}

void on_barrier_observe(BarrierObserverFn fn)
{
    barrier_observers.add(fn);
}

void on_unread_queue(UnreadQueueFn fn)
{
    unread_queues.add(fn);
}

bool learning(void *sc)
{
    return listed(waiting, sc) && !listed(refused, sc);
}

void exec_original(ID3D12CommandQueue *q, UINT n, ID3D12CommandList *const *lists)
{
    if (h_exec && hud::guard::vtable_slot_is(q, EXEC_SLOT, h_exec.target()))
        h_exec.call<void>(q, n, lists);
    else
        q->ExecuteCommandLists(n, lists);
}

void barrier_original(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *b)
{
    if (h_barrier && hud::guard::vtable_slot_is(list, BARRIER_SLOT, h_barrier.target()))
        h_barrier.call<void>(list, n, b);
    else
        list->ResourceBarrier(n, b);
}

bool list_state(ID3D12GraphicsCommandList *list, ListState &out)
{
    if (!h_reset)
        return false;
    ListState *s = tracked(list);
    if (!s->known)
        return false;
    out.known = true;
    out.pso = s->pso;
    out.state_object = s->state_object;
    out.heaps = s->heaps;
    out.heap[0] = s->heap[0];
    out.heap[1] = s->heap[1];
    out.compute_root = s->compute_root;
    out.args = s->args;
    out.graphics_root = s->graphics_root;
    out.tables = s->tables;
    memcpy(out.table, s->table, size_t(s->tables) * 8);
    for (UINT i = 0; i < s->args; i++) {
        out.arg[i].kind = s->arg[i].kind;
        out.arg[i].count = s->arg[i].count;
        out.arg[i].value = s->arg[i].value;
        if (s->arg[i].kind == ARG_CONSTANTS)
            memcpy(out.arg[i].c, s->arg[i].c, size_t(s->arg[i].count) * 4);
    }
    return true;
}

// Puts a list's bindings back as list_state read them. The calls go through the hooks, so the list's
// tracked state ends as it was.
void restore_list_state(ID3D12GraphicsCommandList *l, const ListState &s)
{
    if (s.heaps)
        l->SetDescriptorHeaps(s.heaps, s.heap);
    if (s.graphics_root)
        for (UINT i = 0; i < s.tables; i++)
            if (s.table[i])
                l->SetGraphicsRootDescriptorTable(i, {s.table[i]});
    ID3D12GraphicsCommandList4 *l4 = nullptr;
    if (s.state_object && SUCCEEDED(l->QueryInterface(IID_PPV_ARGS(&l4)))) {
        l4->SetPipelineState1(s.state_object);
        l4->Release();
    } else if (s.pso)
        l->SetPipelineState(s.pso);
    if (!s.compute_root)
        return;
    l->SetComputeRootSignature(s.compute_root);
    for (UINT i = 0; i < s.args; i++) {
        const RootArg &a = s.arg[i];
        switch (a.kind) {
        case ARG_TABLE:
            l->SetComputeRootDescriptorTable(i, {a.value});
            break;
        case ARG_CONSTANTS:
            l->SetComputeRoot32BitConstants(i, a.count, a.c, 0);
            break;
        case ARG_CBV:
            l->SetComputeRootConstantBufferView(i, a.value);
            break;
        case ARG_SRV:
            l->SetComputeRootShaderResourceView(i, a.value);
            break;
        case ARG_UAV:
            l->SetComputeRootUnorderedAccessView(i, a.value);
            break;
        }
    }
}

std::string module_of(const void *addr, HMODULE *mod)
{
    HMODULE m = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            reinterpret_cast<LPCWSTR>(addr), &m) ||
        !m)
        return {};
    if (mod)
        *mod = m;
    char path[MAX_PATH] = {};
    DWORD n = GetModuleFileNameA(m, path, MAX_PATH);
    return std::string(hud::guard::base_name(std::string_view(path, n)));
}

ProbeScope::ProbeScope()
{
    std::lock_guard<std::mutex> g(probe_lock);
    ++probe_scopes;
}

ProbeScope::~ProbeScope()
{
    std::lock_guard<std::mutex> g(probe_lock);
    if (--probe_scopes == 0)
        release(probe_held);
}

bool arm_swapchain()
{
    static int state;
    if (state)
        return state > 0;
    state = -1;
    HMODULE dxgi = LoadLibraryA("dxgi.dll");
    auto create_factory = dxgi ? reinterpret_cast<CreateFactoryFn>(GetProcAddress(dxgi, "CreateDXGIFactory1")) : nullptr;
    if (!create_factory) {
        swofty::log("[d3d12] dxgi.dll!CreateDXGIFactory1 is unavailable; the swapchain hooks are not armed");
        return false;
    }
    WNDCLASSEXW wc{};
    wc.cbSize = sizeof wc;
    wc.lpfnWndProc = DefWindowProcW;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"SwoftyD3D12Probe";
    RegisterClassExW(&wc);
    HWND wnd = CreateWindowExW(0, wc.lpszClassName, L"", WS_OVERLAPPEDWINDOW, 0, 0, 64, 64, nullptr, nullptr,
                               wc.hInstance, nullptr);
    if (!wnd) {
        swofty::log("[d3d12] could not make a probe window to read the swapchain vtable");
        UnregisterClassW(wc.lpszClassName, wc.hInstance);
        return false;
    }
    ID3D12Device *dev = probe_device();
    ID3D12CommandQueue *q = nullptr;
    IDXGIFactory4 *factory = nullptr;
    IDXGISwapChain1 *swap1 = nullptr;
    IDXGISwapChain3 *swap = nullptr;
    bool ok = false;
    do {
        if (!dev)
            break;
        D3D12_COMMAND_QUEUE_DESC qd{};
        qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
        if (FAILED(dev->CreateCommandQueue(&qd, IID_PPV_ARGS(&q))))
            break;
        if (FAILED(create_factory(IID_PPV_ARGS(&factory))))
            break;
        DXGI_SWAP_CHAIN_DESC1 sd{};
        sd.BufferCount = 2;
        sd.Width = 64;
        sd.Height = 64;
        sd.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        sd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
        sd.SampleDesc.Count = 1;
        if (FAILED(factory->CreateSwapChainForHwnd(q, wnd, &sd, nullptr, nullptr, &swap1)))
            break;
        if (FAILED(swap1->QueryInterface(IID_PPV_ARGS(&swap))))
            break;
        void **sc_vt = *reinterpret_cast<void ***>(swap);
        void **f_vt = *reinterpret_cast<void ***>(factory);
        ok = hook_factory(f_vt) &&
             install(h_present, sc_vt[8], reinterpret_cast<void *>(&present_hook), "IDXGISwapChain::Present");
        if (!ok)
            break;
        bool present1 = sc_vt[22] != sc_vt[8] && install(h_present1, sc_vt[22], reinterpret_cast<void *>(&present1_hook),
                                                         "IDXGISwapChain1::Present1");
        install(h_resize, sc_vt[13], reinterpret_cast<void *>(&resize_hook), "IDXGISwapChain::ResizeBuffers");
        install(h_resize1, sc_vt[39], reinterpret_cast<void *>(&resize1_hook), "IDXGISwapChain3::ResizeBuffers1");
        install(h_color_space, sc_vt[38], reinterpret_cast<void *>(&color_space_hook),
                "IDXGISwapChain3::SetColorSpace1");
        swofty::log("[d3d12] swapchain hooks armed in %s: Present (vt+0x40), %sResizeBuffers (vt+0x68), "
                    "SetColorSpace1 (vt+0x130), ResizeBuffers1 (vt+0x138); only swapchains created after this point "
                    "are known",
                    module_of(sc_vt[8]).c_str(), present1 ? "Present1 (vt+0xb0), " : "");
    } while (false);
    release(swap);
    release(swap1);
    release(factory);
    release(q);
    release(dev);
    DestroyWindow(wnd);
    UnregisterClassW(wc.lpszClassName, wc.hInstance);
    state = ok ? 1 : -1;
    return ok;
}

bool arm_device()
{
    static int state;
    if (state)
        return state > 0;
    state = -1;
    ID3D12Device *dev = probe_device();
    ID3D12Resource *buffer = nullptr;
    ID3D12CommandAllocator *alloc = nullptr;
    ID3D12GraphicsCommandList *list = nullptr;
    ID3D12CommandQueue *q = nullptr;
    bool ok = false;
    if (dev) {
        D3D12_HEAP_PROPERTIES heap{};
        heap.Type = D3D12_HEAP_TYPE_UPLOAD;
        D3D12_RESOURCE_DESC bd{};
        bd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
        bd.Width = 256;
        bd.Height = 1;
        bd.DepthOrArraySize = 1;
        bd.MipLevels = 1;
        bd.SampleDesc.Count = 1;
        bd.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
        D3D12_COMMAND_QUEUE_DESC qd{};
        qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
        if (SUCCEEDED(dev->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &bd, D3D12_RESOURCE_STATE_GENERIC_READ,
                                                   nullptr, IID_PPV_ARGS(&buffer))) &&
            SUCCEEDED(dev->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&alloc))) &&
            SUCCEEDED(dev->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, alloc, nullptr, IID_PPV_ARGS(&list))) &&
            SUCCEEDED(dev->CreateCommandQueue(&qd, IID_PPV_ARGS(&q))))
            ok = install(h_setname, (*reinterpret_cast<void ***>(buffer))[SETNAME_SLOT],
                         reinterpret_cast<void *>(&setname_hook), "ID3D12Object::SetName") &&
                 install(h_barrier, (*reinterpret_cast<void ***>(list))[BARRIER_SLOT],
                         reinterpret_cast<void *>(&barrier_hook), "ID3D12GraphicsCommandList::ResourceBarrier") &&
                 install(h_exec, (*reinterpret_cast<void ***>(q))[EXEC_SLOT], reinterpret_cast<void *>(&exec_hook),
                         "ID3D12CommandQueue::ExecuteCommandLists");
        if (ok) {
            // Optional subscribers must not prevent existing device features
            // from arming if a copy entry cannot be hooked on a platform.
            install(h_texture_copy, (*reinterpret_cast<void ***>(list))[TEXTURE_COPY_SLOT],
                    reinterpret_cast<void *>(&texture_copy_hook), "ID3D12GraphicsCommandList::CopyTextureRegion");
            install(h_resource_copy, (*reinterpret_cast<void ***>(list))[RESOURCE_COPY_SLOT],
                    reinterpret_cast<void *>(&resource_copy_hook), "ID3D12GraphicsCommandList::CopyResource");
            swofty::log("[d3d12] texture copy observers: CopyTextureRegion %s, CopyResource %s",
                        h_texture_copy ? "armed" : "unavailable", h_resource_copy ? "armed" : "unavailable");
        }
        if (ok && install(h_rtv, (*reinterpret_cast<void ***>(dev))[RTV_SLOT], reinterpret_cast<void *>(&rtv_hook),
                          "ID3D12Device::CreateRenderTargetView") &&
            !install(h_om, (*reinterpret_cast<void ***>(list))[OM_SLOT], reinterpret_cast<void *>(&om_hook),
                     "ID3D12GraphicsCommandList::OMSetRenderTargets"))
            h_rtv = {};
        if (list)
            list->Close();
    }
    if (ok)
        swofty::log("[d3d12] device hooks armed: SetName (vt+0x30), ResourceBarrier (vt+0xd0), ExecuteCommandLists "
                    "(vt+0x50)%s",
                    h_om ? ", CreateRenderTargetView (vt+0xa0), OMSetRenderTargets (vt+0x170)" : "");
    release(q);
    release(list);
    release(alloc);
    release(buffer);
    release(dev);
    state = ok ? 1 : -1;
    return ok;
}

bool arm_srv_clamp()
{
    static int state;
    if (state)
        return state > 0;
    state = -1;
    char env[8] = {};
    GetEnvironmentVariableA("SWOFTY_SRV_CLAMP", env, sizeof env);
    if (!runtime::srv_clamp::enabled(env)) {
        swofty::log("[gfx] srv clamp is off (SWOFTY_SRV_CLAMP=1 turns it on)");
        return false;
    }
    ID3D12Device *dev = probe_device();
    void **vt = dev ? *reinterpret_cast<void ***>(dev) : nullptr;
    void *srv_target = vt ? vt[SRV_SLOT] : nullptr;
    HMODULE srv_mod = nullptr;
    module_of(srv_target, &srv_mod);
    unsigned long long srv_offset = reinterpret_cast<uintptr_t>(srv_target) - reinterpret_cast<uintptr_t>(srv_mod);
    DWORD old = 0;
    bool ok = vt && VirtualProtect(&vt[SRV_SLOT], sizeof(void *), PAGE_READWRITE, &old);
    if (ok) {
        srv_orig = reinterpret_cast<SrvFn>(srv_target);
        vt[SRV_SLOT] = reinterpret_cast<void *>(&srv_hook);
        VirtualProtect(&vt[SRV_SLOT], sizeof(void *), old, &old);
        swofty::log("[gfx] srv clamp armed: ID3D12Device::CreateShaderResourceView (vt+0x98, vtable entry for %s+0x%llx) "
                    "patched; array and mip views past their texture are clamped",
                    module_of(srv_target).c_str(), srv_offset);
    } else
        swofty::log("[gfx] srv clamp: could not patch the ID3D12Device vtable");
    release(dev);
    state = ok ? 1 : -1;
    return ok;
}

bool arm_list_state()
{
    static int state;
    if (state)
        return state > 0;
    state = -1;
    ID3D12Device *dev = probe_device();
    ID3D12CommandAllocator *alloc = nullptr;
    ID3D12GraphicsCommandList *list = nullptr;
    ID3D12GraphicsCommandList4 *list4 = nullptr;
    bool ok = false;
    if (dev && SUCCEEDED(dev->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&alloc))) &&
        SUCCEEDED(dev->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, alloc, nullptr, IID_PPV_ARGS(&list))) &&
        SUCCEEDED(list->QueryInterface(IID_PPV_ARGS(&list4)))) {
        void **vt = *reinterpret_cast<void ***>(list), **vt4 = *reinterpret_cast<void ***>(list4);
        struct Want {
            size_t slot;
            safetyhook::InlineHook *h;
            void *detour;
        } want[] = {
            {RESET_SLOT, &h_reset, reinterpret_cast<void *>(&reset_hook)},
            {CLEAR_STATE_SLOT, &h_clear_state, reinterpret_cast<void *>(&clear_state_hook)},
            {PSO_SLOT, &h_pso, reinterpret_cast<void *>(&pso_hook)},
            {HEAPS_SLOT, &h_heaps, reinterpret_cast<void *>(&heaps_hook)},
            {COMPUTE_ROOT_SLOT, &h_compute_root, reinterpret_cast<void *>(&compute_root_hook)},
            {COMPUTE_TABLE_SLOT, &h_compute_table, reinterpret_cast<void *>(&compute_table_hook)},
            {GRAPHICS_ROOT_SLOT, &h_graphics_root, reinterpret_cast<void *>(&graphics_root_hook)},
            {GRAPHICS_TABLE_SLOT, &h_graphics_table, reinterpret_cast<void *>(&graphics_table_hook)},
            {COMPUTE_CONSTANT_SLOT, &h_compute_constant, reinterpret_cast<void *>(&compute_constant_hook)},
            {COMPUTE_CONSTANTS_SLOT, &h_compute_constants, reinterpret_cast<void *>(&compute_constants_hook)},
            {COMPUTE_CBV_SLOT, &h_compute_cbv, reinterpret_cast<void *>(&compute_view_hook<ARG_CBV, &h_compute_cbv>)},
            {COMPUTE_SRV_SLOT, &h_compute_srv, reinterpret_cast<void *>(&compute_view_hook<ARG_SRV, &h_compute_srv>)},
            {COMPUTE_UAV_SLOT, &h_compute_uav, reinterpret_cast<void *>(&compute_view_hook<ARG_UAV, &h_compute_uav>)},
            {PSO1_SLOT, &h_pso1, reinterpret_cast<void *>(&pso1_hook)},
        };
        ok = true;
        for (auto &a : want)
            for (auto &b : want)
                if (&a != &b && (a.slot == PSO1_SLOT ? vt4 : vt)[a.slot] == (b.slot == PSO1_SLOT ? vt4 : vt)[b.slot]) {
                    swofty::log("[d3d12] command list slots vt+0x%zx and vt+0x%zx share their code; list state is not "
                                "followed",
                                a.slot * 8, b.slot * 8);
                    ok = false;
                }
        for (auto &w : want)
            if (ok)
                ok = install(*w.h, (w.slot == PSO1_SLOT ? vt4 : vt)[w.slot], w.detour, "ID3D12GraphicsCommandList state");
        if (!ok)
            for (auto &w : want)
                *w.h = {};
        list->Close();
    }
    if (ok)
        swofty::log("[d3d12] command list state hooks armed: Reset, ClearState, SetPipelineState(1), "
                    "SetDescriptorHeaps, the compute root bindings and the graphics descriptor tables");
    release(list4);
    release(list);
    release(alloc);
    release(dev);
    state = ok ? 1 : -1;
    return ok;
}

}

namespace d3d12hooks {

namespace {

namespace rg = runtime::rtas_guard;

constexpr size_t BUILD_RTAS_SLOT = 72;
constexpr int RTAS_DETAILED = 8;
safetyhook::InlineHook h_build_rtas;
rg::Mode rtas_mode = rg::Mode::on;
std::mutex rtas_lock;
rg::Book rtas_book;
std::atomic<size_t> rtas_tracked{0};
std::atomic<uint64_t> rtas_builds{0}, rtas_faulty{0}, rtas_holes{0}, rtas_bad_posts{0};
uint64_t rtas_trace;
constexpr uint32_t RTAS_NAMED = 800;
rg::Trace rtas_named{RTAS_NAMED};
std::atomic<bool> rtas_named_full{false};
rg::ByList<64, 32> rtas_recent;
std::atomic<uint64_t> rtas_all{0};

using RtasDesc = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC;
using GeoDesc = D3D12_RAYTRACING_GEOMETRY_DESC;
using PostDesc = D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_DESC;

const GeoDesc &geo_at(const D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS &in, UINT i)
{
    return in.DescsLayout == D3D12_ELEMENTS_LAYOUT_ARRAY ? in.pGeometryDescs[i] : *in.ppGeometryDescs[i];
}

rg::Geo geo_of(const GeoDesc &d)
{
    rg::Geo g;
    if (d.Type == D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES) {
        g.type = rg::kTriangles;
        g.vertices = d.Triangles.VertexBuffer.StartAddress;
        g.vertex_count = d.Triangles.VertexCount;
        g.indices = d.Triangles.IndexBuffer;
        g.index_count = d.Triangles.IndexCount;
        g.indexed = d.Triangles.IndexFormat != DXGI_FORMAT_UNKNOWN;
    } else if (d.Type == D3D12_RAYTRACING_GEOMETRY_TYPE_PROCEDURAL_PRIMITIVE_AABBS) {
        g.type = rg::kAabbs;
        g.aabbs = d.AABBs.AABBs.StartAddress;
        g.aabb_count = d.AABBs.AABBCount;
    } else
        g.type = -1;
    return g;
}

const char *level_name(const RtasDesc &desc)
{
    return desc.Inputs.Type == D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL ? "top-level" : "bottom-level";
}

void log_hole(uint64_t nth, const RtasDesc &desc, const rg::Call &c, rg::Act a)
{
    if (!rg::detailed(nth, RTAS_DETAILED)) {
        if (rg::milestone(nth))
            swofty::log("[gfx] rtas guard: %llu builds so far had a null destination, scratch, source or instance list",
                        (unsigned long long)nth);
        return;
    }
    const char *what = rtas_mode == rg::Mode::log ? "sent as is (SWOFTY_RTAS_GUARD=log)" : rg::act_name(a);
    swofty::log("[gfx] rtas guard: %s build%s has %s (dest 0x%llx, scratch 0x%llx, source 0x%llx, %u %s at 0x%llx); %s",
                level_name(desc), c.update ? " (update)" : "", rg::hole_name(rg::hole(c)), (unsigned long long)c.dest,
                (unsigned long long)c.scratch, (unsigned long long)c.source, c.count,
                c.level == rg::kTop ? "instances" : "geometries", (unsigned long long)c.instances, what);
}

void log_bad_posts(uint64_t nth, const RtasDesc &desc, UINT posts, size_t kept)
{
    if (!rg::detailed(nth, RTAS_DETAILED)) {
        if (rg::milestone(nth))
            swofty::log("[gfx] rtas guard: %llu builds so far asked for postbuild info into a null buffer",
                        (unsigned long long)nth);
        return;
    }
    swofty::log("[gfx] rtas guard: %s build into 0x%llx asks for %u postbuild info(s), %zu into a null buffer; %s",
                level_name(desc), (unsigned long long)desc.DestAccelerationStructureData, posts, posts - kept,
                rtas_mode == rg::Mode::log ? "asked as is (SWOFTY_RTAS_GUARD=log)" : "those are dropped");
}

void log_faulty(uint64_t nth, const RtasDesc &desc, const std::vector<rg::Geo> &geos, const rg::Plan &plan)
{
    if (!rg::detailed(nth, RTAS_DETAILED)) {
        if (rg::milestone(nth))
            swofty::log("[gfx] rtas guard: %llu bottom-level builds so far had geometries with a null buffer",
                        (unsigned long long)nth);
        return;
    }
    uint32_t first = plan.emptied.empty() ? 0 : plan.emptied.front();
    for (uint32_t i = 0; i < geos.size(); i++)
        if (rg::fault(geos[i]) != rg::Fault::none) {
            first = i;
            break;
        }
    const rg::Geo &g = geos[first];
    const char *what = rtas_mode == rg::Mode::log
                           ? "left as is (SWOFTY_RTAS_GUARD=log); on NVIDIA under Windows the GPU reads address 0 and "
                             "the device is removed"
                           : "emptied, so the build reads nothing for them";
    swofty::log("[gfx] rtas guard: bottom-level build into 0x%llx%s (%zu geometries): %u with a null buffer%s, first #%u "
                "has %s (%u vertices at 0x%llx, %u indices at 0x%llx); %s",
                (unsigned long long)desc.DestAccelerationStructureData,
                (desc.Inputs.Flags & D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE) ? " (update)" : "",
                geos.size(), plan.faulty,
                plan.kept_empty ? ", plus geometries emptied in the build it updates" : "", first,
                rg::fault_name(rg::fault(g)), g.vertex_count, (unsigned long long)g.vertices, g.index_count,
                (unsigned long long)g.indices, what);
}

rg::Entry entry_of(const void *list, const RtasDesc &desc, bool top)
{
    const auto &in = desc.Inputs;
    rg::Entry e;
    e.seq = rtas_all.fetch_add(1, std::memory_order_relaxed);
    e.frame = frames_presented.load(std::memory_order_relaxed);
    e.list = reinterpret_cast<uint64_t>(list);
    e.dest = desc.DestAccelerationStructureData;
    e.flags = unsigned(in.Flags);
    e.count = in.NumDescs;
    e.top = top;
    if (top)
        return e;
    for (UINT i = 0; i < in.NumDescs && i < UINT(rg::kBriefGeos); i++) {
        const GeoDesc &g = geo_at(in, i);
        rg::Brief &b = e.geo[i];
        b.flags = unsigned(g.Flags);
        if (g.Type == D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES) {
            const auto &t = g.Triangles;
            b.vertices = t.VertexCount;
            b.indices = t.IndexCount;
            b.stride = uint32_t(t.VertexBuffer.StrideInBytes);
            b.vertex_format = int(t.VertexFormat);
            b.index_format = int(t.IndexFormat);
            b.transform = t.Transform3x4 != 0;
        } else {
            b.type = rg::kAabbs;
            b.aabbs = uint32_t(g.AABBs.AABBCount);
        }
    }
    return e;
}

void name_build(const rg::Entry &e, uint64_t nth, const RtasDesc &desc)
{
    const auto &in = desc.Inputs;
    rg::Shape shape;
    for (UINT i = 0; i < in.NumDescs; i++)
        shape.add(geo_of(geo_at(in, i)));
    bool want, full;
    {
        std::lock_guard<std::mutex> g(rtas_lock);
        want = rtas_named.want(shape.hash);
        full = rtas_named.full();
    }
    if (full)
        rtas_named_full.store(true, std::memory_order_relaxed);
    if (!want)
        return;
    swofty::log("[gfx] rtas build %llu%s on list 0x%llx into 0x%llx (flags 0x%x), %u geometries:%s", (unsigned long long)nth,
                (e.flags & D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE) ? " (update)" : "",
                (unsigned long long)e.list, (unsigned long long)e.dest, e.flags, e.count, rg::geometry_text(e).c_str());
    if (full)
        swofty::log("[gfx] rtas build log: %u shapes named; later ones are not", RTAS_NAMED);
}

void STDMETHODCALLTYPE build_rtas_hook(ID3D12GraphicsCommandList4 *l, const RtasDesc *desc, UINT posts,
                                       const PostDesc *post)
{
    if (!desc) {
        h_build_rtas.call<void>(l, desc, posts, post);
        return;
    }
    bool top = desc->Inputs.Type == D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL;
    rg::Call call{top ? rg::kTop : rg::kBottom,
                  desc->DestAccelerationStructureData,
                  desc->ScratchAccelerationStructureData,
                  desc->SourceAccelerationStructureData,
                  top ? desc->Inputs.InstanceDescs : 0,
                  desc->Inputs.NumDescs,
                  (desc->Inputs.Flags & D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE) != 0};
    rg::Entry entry = entry_of(l, *desc, top);
    {
        std::lock_guard<std::mutex> g(rtas_lock);
        rtas_recent.push(entry);
    }
    rg::Act a = rg::act(call);
    if (a != rg::Act::send) {
        log_hole(rtas_holes.fetch_add(1, std::memory_order_relaxed) + 1, *desc, call, a);
        if (rtas_mode == rg::Mode::on && a == rg::Act::skip)
            return;
    }
    std::vector<PostDesc> kept_post;
    if (posts && post) {
        std::vector<uint64_t> dests(posts);
        for (UINT i = 0; i < posts; i++)
            dests[i] = post[i].DestBuffer;
        std::vector<uint32_t> kept = rg::kept_posts(dests);
        if (kept.size() != posts) {
            log_bad_posts(rtas_bad_posts.fetch_add(1, std::memory_order_relaxed) + 1, *desc, posts, kept.size());
            if (rtas_mode == rg::Mode::on) {
                for (uint32_t i : kept)
                    kept_post.push_back(post[i]);
                posts = UINT(kept_post.size());
                post = kept_post.empty() ? nullptr : kept_post.data();
            }
        }
    }
    if (a == rg::Act::no_instances && rtas_mode == rg::Mode::on) {
        RtasDesc patched = *desc;
        patched.Inputs.NumDescs = 0;
        h_build_rtas.call<void>(l, &patched, posts, post);
        return;
    }
    if (top || !desc->Inputs.NumDescs) {
        h_build_rtas.call<void>(l, desc, posts, post);
        return;
    }
    uint64_t nth = rtas_builds.fetch_add(1, std::memory_order_relaxed);
    if (nth == 0)
        swofty::log("[gfx] rtas guard: the game builds bottom-level ray tracing structures (first: %u geometries)",
                    desc->Inputs.NumDescs);
    const auto &in = desc->Inputs;
    if (nth < rtas_trace) {
        char line[1024];
        int at = snprintf(line, sizeof line, "[gfx] rtas trace %llu: dest 0x%llx src 0x%llx scratch 0x%llx flags 0x%x, %u geometries:",
                          (unsigned long long)nth, (unsigned long long)desc->DestAccelerationStructureData,
                          (unsigned long long)desc->SourceAccelerationStructureData,
                          (unsigned long long)desc->ScratchAccelerationStructureData, unsigned(in.Flags), in.NumDescs);
        for (UINT i = 0; i < in.NumDescs && i < 6 && at > 0 && at < int(sizeof line) - 160; i++) {
            const GeoDesc &g = geo_at(in, i);
            if (g.Type == D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES)
                at += snprintf(line + at, sizeof line - at, " [vb 0x%llx s%llu n%u f%d ib 0x%llx n%u f%d t 0x%llx fl%u]",
                               (unsigned long long)g.Triangles.VertexBuffer.StartAddress,
                               (unsigned long long)g.Triangles.VertexBuffer.StrideInBytes, g.Triangles.VertexCount,
                               int(g.Triangles.VertexFormat), (unsigned long long)g.Triangles.IndexBuffer,
                               g.Triangles.IndexCount, int(g.Triangles.IndexFormat),
                               (unsigned long long)g.Triangles.Transform3x4, unsigned(g.Flags));
            else
                at += snprintf(line + at, sizeof line - at, " [aabb 0x%llx n%llu]",
                               (unsigned long long)g.AABBs.AABBs.StartAddress, (unsigned long long)g.AABBs.AABBCount);
        }
        swofty::log("%s", line);
    }
    if (!rtas_named_full.load(std::memory_order_relaxed))
        name_build(entry, nth, *desc);
    bool any = false;
    for (UINT i = 0; i < in.NumDescs && !any; i++)
        any = rg::fault(geo_of(geo_at(in, i))) != rg::Fault::none;
    if (!any && !rtas_tracked.load(std::memory_order_acquire)) {
        h_build_rtas.call<void>(l, desc, posts, post);
        return;
    }
    std::vector<rg::Geo> geos(in.NumDescs);
    for (UINT i = 0; i < in.NumDescs; i++)
        geos[i] = geo_of(geo_at(in, i));
    rg::Build b{desc->DestAccelerationStructureData, desc->SourceAccelerationStructureData,
                (in.Flags & D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE) != 0};
    rg::Plan plan;
    if (rtas_mode == rg::Mode::on) {
        std::lock_guard<std::mutex> g(rtas_lock);
        plan = rtas_book.plan(b, geos);
        rtas_tracked.store(rtas_book.tracked(), std::memory_order_release);
    } else
        for (uint32_t i = 0; i < geos.size(); i++)
            if (rg::fault(geos[i]) != rg::Fault::none) {
                plan.emptied.push_back(i);
                plan.faulty++;
            }
    if (plan.faulty)
        log_faulty(rtas_faulty.fetch_add(1, std::memory_order_relaxed) + 1, *desc, geos, plan);
    if (rtas_mode != rg::Mode::on || plan.emptied.empty()) {
        h_build_rtas.call<void>(l, desc, posts, post);
        return;
    }
    std::vector<GeoDesc> copy(in.NumDescs);
    for (UINT i = 0; i < in.NumDescs; i++)
        copy[i] = geo_at(in, i);
    for (uint32_t i : plan.emptied) {
        if (copy[i].Type == D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES) {
            copy[i].Triangles.IndexCount = 0;
            copy[i].Triangles.VertexCount = 0;
        } else if (copy[i].Type == D3D12_RAYTRACING_GEOMETRY_TYPE_PROCEDURAL_PRIMITIVE_AABBS)
            copy[i].AABBs.AABBCount = 0;
    }
    RtasDesc patched = *desc;
    patched.Inputs.DescsLayout = D3D12_ELEMENTS_LAYOUT_ARRAY;
    patched.Inputs.pGeometryDescs = copy.data();
    h_build_rtas.call<void>(l, &patched, posts, post);
}

}

bool arm_rtas_guard()
{
    static int state;
    if (state)
        return state > 0;
    state = -1;
    char env[8] = {};
    GetEnvironmentVariableA("SWOFTY_RTAS_GUARD", env, sizeof env);
    rtas_mode = rg::mode(env[0] ? env : nullptr);
    char trace[16] = {};
    if (GetEnvironmentVariableA("SWOFTY_RTAS_TRACE", trace, sizeof trace))
        rtas_trace = strtoull(trace, nullptr, 10);
    std::wstring settings = runtime::hardware_settings_file();
    std::string rt = settings.empty() ? std::string() : rg::rt_options(runtime::read_file(settings, 1 << 20));
    swofty::log("[gfx] the game's hardware settings have ray tracing options: %s", rt.empty() ? "none found" : rt.c_str());
    if (rtas_mode == rg::Mode::off) {
        swofty::log("[gfx] rtas guard is off (SWOFTY_RTAS_GUARD=0)");
        return false;
    }
    ID3D12Device *dev = probe_device();
    ID3D12CommandAllocator *alloc = nullptr;
    ID3D12GraphicsCommandList *list = nullptr;
    ID3D12GraphicsCommandList4 *list4 = nullptr;
    bool ok = false;
    if (dev && SUCCEEDED(dev->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&alloc))) &&
        SUCCEEDED(dev->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, alloc, nullptr, IID_PPV_ARGS(&list))) &&
        SUCCEEDED(list->QueryInterface(IID_PPV_ARGS(&list4)))) {
        void *target = (*reinterpret_cast<void ***>(list4))[BUILD_RTAS_SLOT];
        ok = install(h_build_rtas, target, reinterpret_cast<void *>(&build_rtas_hook),
                     "ID3D12GraphicsCommandList4::BuildRaytracingAccelerationStructure");
        if (ok)
            swofty::log("[gfx] rtas guard armed%s: BuildRaytracingAccelerationStructure (vt+0x%zx, in %s); builds "
                        "with a null buffer, destination, scratch, source or instance list are %s",
                        rtas_mode == rg::Mode::log ? " in log mode" : "", BUILD_RTAS_SLOT * 8, module_of(target).c_str(),
                        rtas_mode == rg::Mode::log ? "logged" : "logged and kept off address 0");
        list->Close();
    } else
        swofty::log("[gfx] rtas guard: no ID3D12GraphicsCommandList4 to hook; not armed");
    release(list4);
    release(list);
    release(alloc);
    release(dev);
    state = ok ? 1 : -1;
    return ok;
}

std::string rtas_recent_text(const std::vector<runtime::dred::BuildStop> &stops)
{
    std::vector<rg::ByList<64, 32>::Kept> lists;
    uint64_t pushed;
    {
        std::unique_lock<std::mutex> g(rtas_lock, std::try_to_lock);
        lists = rtas_recent.newest_lists_first();
        pushed = rtas_recent.pushed();
    }
    return rg::recent_section(lists, pushed, 12, rg::stopped_lines(lists, stops));
}

}

namespace runtime {

void rtas_guard_setup()
{
    d3d12hooks::arm_rtas_guard();
}

void srv_clamp_setup()
{
    d3d12hooks::arm_srv_clamp();
}

}
