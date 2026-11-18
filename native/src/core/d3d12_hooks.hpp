#pragma once
#include <windows.h>

#include <d3d12.h>
#include <dxgi1_6.h>

#include "dred_logic.hpp"
#include "exec_plan.hpp"

#include <string>

// The core's one set of D3D12 / DXGI hooks. Features subscribe instead of hooking the same entry points
// themselves. Subscribe before arming; callbacks run on the game's thread inside the hooked call.
namespace d3d12hooks {

struct Swapchain {
    void *sc = nullptr;
    ID3D12CommandQueue *queue = nullptr;
    std::string how, by, fg;
    bool streamline = false;
    // Its queue was learned from ExecuteCommandLists after the device hooks armed on demand for it.
    bool armed_late = false;
    UINT color_space = 0;
    bool color_space_set = false;
};

using PresentFn = void (*)(IDXGISwapChain3 *sc, const Swapchain *known, void *caller);
using PresentedFn = void (*)(IDXGISwapChain3 *sc, HRESULT hr);
using ResizeFn = void (*)(IDXGISwapChain3 *sc);
using ExecPlan = BasicExecPlan<ID3D12CommandQueue>;
// Appends the steps it wants run around the game's lists of one ExecuteCommandLists call to the plan.
using ExecFn = void (*)(ID3D12CommandQueue *q, UINT n, ID3D12CommandList *const *lists, ExecPlan &plan);
using BarrierFn = bool (*)(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *barriers);
// Observers run before a replacing subscriber can consume the original batch.
using BarrierObserverFn = void (*)(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *barriers);
using SetNameFn = void (*)(ID3D12Object *object, LPCWSTR name);
// Runs after the game's copy is recorded. CopyResource supplies mip-zero
// locations; subscribers must validate the resource dimensions themselves.
using TextureCopyFn = void (*)(ID3D12GraphicsCommandList *, const D3D12_TEXTURE_COPY_LOCATION *, UINT, UINT, UINT,
                              const D3D12_TEXTURE_COPY_LOCATION *, const D3D12_BOX *);
using ResetFn = void (*)(ID3D12GraphicsCommandList *list);
using RenderTargetsFn = void (*)(ID3D12GraphicsCommandList *list, UINT n, const D3D12_CPU_DESCRIPTOR_HANDLE *rtvs,
                                 BOOL single_range);
using RtvFn = void (*)(ID3D12Resource *res, D3D12_CPU_DESCRIPTOR_HANDLE at);
// Asked once, the first time a swapchain whose present queue cannot be read reaches Present while
// ExecuteCommandLists is not hooked; true when it armed the device hooks, so the queue is learned from them.
using UnreadQueueFn = bool (*)(IDXGISwapChain3 *sc, void *caller);

void on_present(PresentFn fn);
void on_presented(PresentedFn fn);
void on_resize(ResizeFn fn);
void on_exec(ExecFn fn);
void on_barrier(BarrierFn fn);
void on_barrier_observe(BarrierObserverFn fn);
void on_setname(SetNameFn fn);
void on_texture_copy(TextureCopyFn fn);
void on_reset(ResetFn fn);

// The newest ray tracing builds, for a crash report, led by the build each unfinished command list stopped on;
// empty before the game builds any.
std::string rtas_recent_text(const std::vector<runtime::dred::BuildStop> &stops);
void on_render_targets(RenderTargetsFn fn);
void on_create_rtv(RtvFn fn);
void on_unread_queue(UnreadQueueFn fn);

// Whether a swapchain whose present queue cannot be read waits for a direct queue to execute command lists.
bool learning(void *sc);

bool arm_swapchain();
bool arm_device();
bool arm_srv_clamp();

// While one is alive, arm_device and arm_swapchain share one throwaway probe
// device: creating a D3D12 device takes about half a second on the main thread.
struct ProbeScope {
    ProbeScope();
    ~ProbeScope();
    ProbeScope(const ProbeScope &) = delete;
    ProbeScope &operator=(const ProbeScope &) = delete;
};

void exec_original(ID3D12CommandQueue *q, UINT n, ID3D12CommandList *const *lists);
void barrier_original(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *barriers);

// What the core's compute work in a game's command list changes: the pipeline, the descriptor heaps
// and the compute root signature with its arguments, and the graphics descriptor tables, which are
// set again after the heaps come back. Once arm_list_state has run, every list's are
// followed from its Reset; a feature saves them before recording into the game's list and puts them
// back afterwards, so the game's next draw or dispatch finds its own bindings.
struct RootArg {
    uint8_t kind = 0;
    UINT count = 0;
    UINT64 value = 0;
    uint32_t c[64] = {};
};

struct ListState {
    bool known = false;
    ID3D12PipelineState *pso = nullptr;
    ID3D12StateObject *state_object = nullptr;
    UINT heaps = 0;
    ID3D12DescriptorHeap *heap[2] = {};
    ID3D12RootSignature *compute_root = nullptr;
    UINT args = 0;
    RootArg arg[64];
    ID3D12RootSignature *graphics_root = nullptr;
    UINT tables = 0;
    UINT64 table[64] = {};
};

bool arm_list_state();
bool list_state(ID3D12GraphicsCommandList *list, ListState &out);
void restore_list_state(ID3D12GraphicsCommandList *list, const ListState &s);

std::string module_of(const void *addr, HMODULE *mod = nullptr);

}
