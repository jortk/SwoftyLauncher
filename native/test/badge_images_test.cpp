#define WIN32_LEAN_AND_MEAN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include "../src/core/badge_images.hpp"
#include "../src/core/badge_image_data.hpp"
#include "../src/core/d3d12_hooks.hpp"
#include "../src/core/runtime.hpp"
#include <swofty/live_images.h>
#include <wrl/client.h>
#include <dxgi1_6.h>
#include <cassert>
#include <cstdio>
#include <memory>
#include <fstream>
#include <iterator>
#include <filesystem>

using Microsoft::WRL::ComPtr;
namespace d3d12hooks {
BarrierObserverFn observer;
ExecFn exec;
ResetFn reset;
SetNameFn setname;
TextureCopyFn texture_copy;
void on_barrier_observe(BarrierObserverFn fn) { observer = fn; }
void on_exec(ExecFn fn) { exec = fn; }
void on_reset(ResetFn fn) { reset = fn; }
void on_setname(SetNameFn fn) { setname = fn; }
void on_texture_copy(TextureCopyFn fn) { texture_copy = fn; }
bool arm_device() { return true; }
bool arm_list_state() { return true; }
void barrier_original(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *b) { list->ResourceBarrier(n, b); }
}
namespace runtime {
std::wstring under_swofty(const std::string &) { return {}; }
std::string read_file(const std::wstring &, size_t limit) {
    // The cross-language stock-archive fixture carries the selected install's
    // current fingerprint, even after a real game launch creates assets.json.
    char path[32768];
    const auto size = GetEnvironmentVariableA("SWOFTY_BADGE_FIXTURE_ASSETS", path, sizeof path);
    if (!size || size >= sizeof path) return {};
    std::ifstream file(path, std::ios::binary);
    std::string bytes((std::istreambuf_iterator<char>(file)), {});
    return bytes.size() <= limit ? bytes : std::string{};
}
const std::wstring &game_root() {
    static const std::wstring root=[] { wchar_t path[32768]{};auto n=GetEnvironmentVariableW(L"SWOFTY_BADGE_FIXTURE_ASSETS",path,32768);return n && n<32768 ? std::filesystem::path(path).parent_path().parent_path().wstring():std::wstring{}; }();return root;
}
const json::Value &config() { static const auto value=[] { json::Value v;json::parse(read_file({},16u<<20),v);return v; }();return value; }
uint64_t fnv64(const char *s, size_t n) { uint64_t h = 0xcbf29ce484222325ull; for (size_t i = 0; i < n; i++) h = (h ^ uint8_t(s[i])) * 0x100000001b3ull; return h; }
std::string hex16(uint64_t h) { char b[24]; std::snprintf(b, sizeof b, "0x%016llx", static_cast<unsigned long long>(h)); return b; }
}

namespace badge_images {
std::vector<std::string> traces;
void trace(const char *message) { traces.emplace_back(message); std::printf("[badges] %s\n", message); }
}

int main(int argc, char **argv)
{
    setvbuf(stdout, nullptr, _IONBF, 0);
    using namespace badge_images;
    assert(data::reference("eaid://ui_package/art/textures/render_badge_ePreLoaded 7.image") == "eaid://ui_package/art/textures/render_badge_ePreLoaded 7");
    assert(!data::reference("render_badge_ePreLoaded 7wrong"));
    assert(!data::reference("render_badge_eNetworkReplicated 16383"));
    const bool real = argc == 2;
    HMODULE core = real ? LoadLibraryA(argv[1]) : nullptr;
    assert(!real || core);
    const SwoftyLiveImages *api=nullptr;
    if(real) {
        auto getter=reinterpret_cast<const SwoftyLive *(*)()>(GetProcAddress(core,"SwoftyCoreLive"));
        assert(getter && !GetProcAddress(core,"SwoftyCoreBadgeImages"));
        auto table=getter();assert(table && table->version>=4 && table->images);api=table->images();
    } else api=badge_images::api();
    assert(api && api->version == 1 && api->start() == SWOFTY_ASSET_OK);
    std::vector<char> json(32768);
    uint64_t revision = 0;
    if (!real) {
    file_routes({{"2025_asset_groups/ui_package/databases.erp", "swofty/addons/swofty.logos/current_db.erp"}});
    overlay_routes({{"swofty/assets/resources/0.erp", "swofty/addons/swofty.logos/current_overlay.erp"}});
    assert(api->sources(json.data(), unsigned(json.size()), &revision) == SWOFTY_ASSET_OK);
    assert(std::strstr(json.data(), "current_db.erp") && std::strstr(json.data(), "current_overlay.erp"));
    const auto before = revision;
    file_routes({}); overlay_routes({});
    assert(api->sources(json.data(), unsigned(json.size()), &revision) == SWOFTY_ASSET_OK && revision > before);
    assert(!std::strstr(json.data(), "current_db.erp"));
    }

    ComPtr<IDXGIFactory4> factory;
    assert(SUCCEEDED(CreateDXGIFactory1(IID_PPV_ARGS(&factory))));
    ComPtr<IDXGIAdapter> warp;
    assert(SUCCEEDED(factory->EnumWarpAdapter(IID_PPV_ARGS(&warp))));
    ComPtr<ID3D12Device> device;
    assert(SUCCEEDED(D3D12CreateDevice(real ? nullptr : warp.Get(), D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&device))));
    ComPtr<ID3D12CommandQueue> queue;
    D3D12_COMMAND_QUEUE_DESC qd{}; qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    assert(SUCCEEDED(device->CreateCommandQueue(&qd, IID_PPV_ARGS(&queue))));
    ComPtr<ID3D12Fence> completed;
    assert(SUCCEEDED(device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&completed))));
    uint64_t submission = 0;
    auto run = [&](const wchar_t *name, DXGI_FORMAT format, UINT subresource, bool retire, bool discard, bool ansi = false) {
        api->clear();
        ComPtr<ID3D12CommandAllocator> allocator;
        ComPtr<ID3D12GraphicsCommandList> list;
        assert(SUCCEEDED(device->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&allocator))));
        assert(SUCCEEDED(device->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, allocator.Get(), nullptr, IID_PPV_ARGS(&list))));
        D3D12_RESOURCE_DESC desc{};
        desc.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D; desc.Width = 128; desc.Height = 64;
        desc.DepthOrArraySize = desc.MipLevels = 1; desc.Format = format; desc.SampleDesc.Count = 1;
        desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
        D3D12_HEAP_PROPERTIES heap{}; heap.Type = D3D12_HEAP_TYPE_DEFAULT;
        ComPtr<ID3D12Resource> texture;
        assert(SUCCEEDED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr, IID_PPV_ARGS(&texture))));
        if (ansi) {
            const GUID guid = {0x429b8c22, 0x9188, 0x4b0c, {0x87, 0x42, 0xac, 0xb0, 0xbf, 0x85, 0xc2, 0x00}};
            std::string narrow;
            for (const wchar_t *p = name; *p; ++p) narrow += char(*p);
            assert(SUCCEEDED(texture->SetPrivateData(guid, unsigned(narrow.size()), narrow.data())));
        } else {
            assert(SUCCEEDED(texture->SetName(name)));
            if (!real) d3d12hooks::setname(texture.Get(), name);
        }
        D3D12_DESCRIPTOR_HEAP_DESC hd{}; hd.NumDescriptors = 1; hd.Type = D3D12_DESCRIPTOR_HEAP_TYPE_RTV;
        ComPtr<ID3D12DescriptorHeap> rtv;
        assert(SUCCEEDED(device->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtv))));
        device->CreateRenderTargetView(texture.Get(), nullptr, rtv->GetCPUDescriptorHandleForHeapStart());
        const float colour[4] = {0.25f, 0.5f, 0.75f, 0.5f};
        list->ClearRenderTargetView(rtv->GetCPUDescriptorHandleForHeapStart(), colour, 0, nullptr);
        D3D12_RESOURCE_BARRIER step{};
        step.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
        step.Transition = {texture.Get(), subresource, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE};
        if (!real) d3d12hooks::observer(list.Get(), 1, &step);
        list->ResourceBarrier(1, &step);
        assert(SUCCEEDED(list->Close()));
        if (retire) api->clear();
        if (discard) {
            assert(SUCCEEDED(allocator->Reset()));
            assert(SUCCEEDED(list->Reset(allocator.Get(), nullptr)));
            if (!real) d3d12hooks::reset(list.Get());
            assert(SUCCEEDED(list->Close()));
        }
        uint8_t pixels[data::PIXELS]{};
        uint32_t w = 0, h = 0; uint64_t image_rev = 0;
        std::string named;
        for (const wchar_t *p=name;*p;++p) named+=char(*p);
        const auto reference = data::reference(named).value();
        const auto before_submit=api->image(reference.c_str(), pixels, sizeof pixels, &w, &h, &image_rev);
        assert(before_submit == SWOFTY_ASSET_UNAVAILABLE || (reference.ends_with("default.tga") && before_submit == SWOFTY_ASSET_LATE));
        ID3D12CommandList *lists[] = {list.Get()};
        d3d12hooks::ExecPlan plan;
        if (!real) d3d12hooks::exec(queue.Get(), 1, lists, plan);
        d3d12hooks::run_plan(plan, queue.Get(), 1, lists, [](auto *q, UINT n, auto *const *ls) { q->ExecuteCommandLists(n, ls); });
        assert(SUCCEEDED(queue->Signal(completed.Get(), ++submission)));
        const auto deadline = GetTickCount64() + 5000;
        while (completed->GetCompletedValue() < submission && GetTickCount64() < deadline) Sleep(1);
        assert(completed->GetCompletedValue() >= submission);
        auto result = api->image(reference.c_str(), pixels, sizeof pixels, &w, &h, &image_rev);
        if (retire || discard) { assert(result == SWOFTY_ASSET_UNAVAILABLE); return; }
        assert(result == SWOFTY_ASSET_OK && w == 64 && h == 32 && image_rev != 0);
        for (unsigned p = 0; p < w * h * 4; p += 4) {
            const bool same = std::abs(int(pixels[p])-64)<=1 && std::abs(int(pixels[p+1])-128)<=1 &&
                std::abs(int(pixels[p+2])-191)<=1 && std::abs(int(pixels[p+3])-128)<=1;
            if (!same) { std::printf("Unexpected GPU pixels at %u: %u %u %u %u; real=%d format=%u\n",p,
                unsigned(pixels[p]),unsigned(pixels[p+1]),unsigned(pixels[p+2]),unsigned(pixels[p+3]),int(real),unsigned(format));std::fflush(stdout); }
            assert(same);
        }
        assert(api->image(reference.c_str(), pixels, 4, &w, &h, &image_rev) == SWOFTY_ASSET_INVALID);
        if (!real && reference.ends_with("eUserDefined 3")) {
            // Real game regression: the badge is uploaded before a GP session,
            // then reused by My Team without another upload after menu unload.
            const auto previous_revision = image_rev;
            badge_images::session_unloaded();
            assert(api->image(reference.c_str(), pixels, sizeof pixels, &w, &h, &image_rev) == SWOFTY_ASSET_OK);
            assert(image_rev == previous_revision && w == 64 && h == 32);
            // A new resource reusing that reference must invalidate old pixels.
            ComPtr<ID3D12Resource> replacement;
            assert(SUCCEEDED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc,
                D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr, IID_PPV_ARGS(&replacement))));
            assert(SUCCEEDED(replacement->SetName(name)));
            d3d12hooks::setname(replacement.Get(), name);
            assert(api->image(reference.c_str(), pixels, sizeof pixels, &w, &h, &image_rev) == SWOFTY_ASSET_UNAVAILABLE);
            std::puts("PASS: completed resource-owned badge survives session unload; replacement cannot reuse old pixels");
        }
    };
    run(L"eaid://ui_package/art/textures/render_badge_ePreLoaded 7.image", DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, false, false);
    run(L"eaid://ui_package/art/textures/render_badge_ePreLoaded 7.image", DXGI_FORMAT_B8G8R8A8_UNORM, 0, false, false);
    run(L"eaid://ui_package/art/textures/render_badge_ePreLoaded 7.image", DXGI_FORMAT_R8G8B8A8_UNORM, 0, true, false);
    run(L"eaid://ui_package/art/textures/render_badge_ePreLoaded 7.image", DXGI_FORMAT_R8G8B8A8_UNORM, 0, false, true);
    run(L"eaid://ui_package/art/textures/render_badge_eUserDefined 3.image", DXGI_FORMAT_R8G8B8A8_UNORM, 0, false, false);
    run(L"eaid://ui_package/art/textures/render_badge_eNetworkReplicated 17.image", DXGI_FORMAT_R8G8B8A8_UNORM, 0, false, false);
    run(L"eaid://ui_package/art/textures/render_badge_default.tga.image", DXGI_FORMAT_R8G8B8A8_UNORM, 0, false, false);
    run(L"eaid://ui_package/art/textures/render_badge_eUserDefined 91.image", DXGI_FORMAT_R8G8B8A8_UNORM, 0, false, false, true);
    if (!real) {
        // Ingredient assets must not hide the final badge's diagnostic. A
        // block-compressed final texture must report its actual rejection.
        ComPtr<ID3D12Resource> texture;
        D3D12_RESOURCE_DESC desc{};
        desc.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D; desc.Width = desc.Height = 64;
        desc.DepthOrArraySize = desc.MipLevels = 1; desc.Format = DXGI_FORMAT_BC7_UNORM_SRGB; desc.SampleDesc.Count = 1;
        D3D12_HEAP_PROPERTIES heap{}; heap.Type = D3D12_HEAP_TYPE_DEFAULT;
        assert(SUCCEEDED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_COPY_DEST,
                                                        nullptr, IID_PPV_ARGS(&texture))));
        for (unsigned i = 0; i < 40; ++i) d3d12hooks::setname(texture.Get(), L"ingredient_badge.image");
        const wchar_t *name = L"eaid://ui_package/art/textures/render_badge_eUserDefined 17.image";
        assert(SUCCEEDED(texture->SetName(name)));
        d3d12hooks::setname(texture.Get(), name);
        assert(std::any_of(traces.begin(), traces.end(), [](const auto &s) {
            return s.find("eUserDefined 17.image") != std::string::npos && s.find("recognised=yes") != std::string::npos;
        }));
        ComPtr<ID3D12CommandAllocator> allocator;
        ComPtr<ID3D12GraphicsCommandList> list;
        assert(SUCCEEDED(device->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&allocator))));
        assert(SUCCEEDED(device->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, allocator.Get(), nullptr, IID_PPV_ARGS(&list))));
        D3D12_RESOURCE_BARRIER step{};
        step.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
        step.Transition = {texture.Get(), 0, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE};
        d3d12hooks::observer(list.Get(), 1, &step);
        list->ResourceBarrier(1, &step);
        assert(SUCCEEDED(list->Close()));
        uint8_t pixels[data::PIXELS]{}; uint32_t w = 0, h = 0; uint64_t rev = 0;
        assert(api->image("eaid://ui_package/art/textures/render_badge_eUserDefined 17", pixels, sizeof pixels, &w, &h, &rev) == SWOFTY_ASSET_UNAVAILABLE);
        assert(traces.back().find("format=99") != std::string::npos && traces.back().find("reason=unsupported_format") != std::string::npos);
    }
    auto upload = [&](D3D12_COMMAND_LIST_TYPE type, unsigned width, unsigned height, bool bgra, bool resource_copy, bool retire = false, bool discard = false) {
        api->clear();
        D3D12_COMMAND_QUEUE_DESC qd{}; qd.Type = type;
        ComPtr<ID3D12CommandQueue> upload_queue;
        ComPtr<ID3D12CommandAllocator> allocator;
        ComPtr<ID3D12GraphicsCommandList> list;
        assert(SUCCEEDED(device->CreateCommandQueue(&qd, IID_PPV_ARGS(&upload_queue))));
        assert(SUCCEEDED(device->CreateCommandAllocator(type, IID_PPV_ARGS(&allocator))));
        assert(SUCCEEDED(device->CreateCommandList(0, type, allocator.Get(), nullptr, IID_PPV_ARGS(&list))));
        D3D12_RESOURCE_DESC desc{};
        desc.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D; desc.Width = width; desc.Height = height;
        desc.DepthOrArraySize = desc.MipLevels = 1; desc.SampleDesc.Count = 1;
        desc.Format = bgra ? DXGI_FORMAT_B8G8R8A8_UNORM_SRGB : DXGI_FORMAT_R8G8B8A8_UNORM_SRGB;
        D3D12_HEAP_PROPERTIES heap{}; heap.Type = D3D12_HEAP_TYPE_DEFAULT;
        ComPtr<ID3D12Resource> target, intermediate;
        assert(SUCCEEDED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_COMMON, nullptr, IID_PPV_ARGS(&target))));
        const wchar_t *name = L"eaid://ui_package/art/textures/render_badge_eUserDefined 1.image";
        assert(SUCCEEDED(target->SetName(name)));
        if (!real) d3d12hooks::setname(target.Get(), name);
        const unsigned pitch = (width * 4 + 255) & ~255u;
        const UINT64 offset = 512, bytes = offset + UINT64(pitch) * height;
        D3D12_RESOURCE_DESC buffer{}; buffer.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER; buffer.Width = bytes;
        buffer.Height = buffer.DepthOrArraySize = buffer.MipLevels = 1; buffer.SampleDesc.Count = 1; buffer.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
        heap.Type = D3D12_HEAP_TYPE_UPLOAD;
        ComPtr<ID3D12Resource> source;
        assert(SUCCEEDED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &buffer, D3D12_RESOURCE_STATE_GENERIC_READ, nullptr, IID_PPV_ARGS(&source))));
        uint8_t *mapped = nullptr; D3D12_RANGE none{0, 0};
        assert(SUCCEEDED(source->Map(0, &none, reinterpret_cast<void **>(&mapped))));
        std::memset(mapped, 0xee, size_t(bytes));
        for (unsigned y = 0; y < height; ++y) for (unsigned x = 0; x < width; ++x) {
            auto p = mapped + offset + size_t(y) * pitch + x * 4;
            p[0] = bgra ? 191 : 64; p[1] = 128; p[2] = bgra ? 64 : 191; p[3] = 128;
        }
        D3D12_RANGE written{0, size_t(bytes)}; source->Unmap(0, &written);
        D3D12_TEXTURE_COPY_LOCATION dst{}, src{};
        dst.pResource = target.Get(); dst.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        src.pResource = source.Get(); src.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        src.PlacedFootprint.Offset = offset; src.PlacedFootprint.Footprint = {desc.Format, width, height, 1, pitch};
        if (resource_copy) {
            heap.Type = D3D12_HEAP_TYPE_DEFAULT;
            assert(SUCCEEDED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_COMMON, nullptr, IID_PPV_ARGS(&intermediate))));
            D3D12_TEXTURE_COPY_LOCATION temporary = dst; temporary.pResource = intermediate.Get();
            list->CopyTextureRegion(&temporary, 0, 0, 0, &src, nullptr);
            if (!real) d3d12hooks::texture_copy(list.Get(), &temporary, 0, 0, 0, &src, nullptr);
            D3D12_RESOURCE_BARRIER step{}; step.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
            step.Transition = {intermediate.Get(), 0, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COPY_SOURCE};
            if (!real) d3d12hooks::observer(list.Get(), 1, &step);
            list->ResourceBarrier(1, &step);
            list->CopyResource(target.Get(), intermediate.Get());
            src = temporary;
            if (!real) d3d12hooks::texture_copy(list.Get(), &dst, 0, 0, 0, &src, nullptr);
        } else {
            // No ResourceBarrier for the named target: this reproduces the
            // real game's missing-barrier path using implicit promotion.
            list->CopyTextureRegion(&dst, 0, 0, 0, &src, nullptr);
            if (!real) d3d12hooks::texture_copy(list.Get(), &dst, 0, 0, 0, &src, nullptr);
        }
        assert(SUCCEEDED(list->Close()));
        if (retire) api->clear();
        if (discard) {
            assert(SUCCEEDED(allocator->Reset())); assert(SUCCEEDED(list->Reset(allocator.Get(), nullptr)));
            if (!real) d3d12hooks::reset(list.Get());
            assert(SUCCEEDED(list->Close()));
        }
        uint8_t pixels[data::PIXELS]{}; uint32_t w = 0, h = 0; uint64_t rev = 0;
        constexpr const char *reference = "eaid://ui_package/art/textures/render_badge_eUserDefined 1";
        assert(api->image(reference, pixels, sizeof pixels, &w, &h, &rev) == SWOFTY_ASSET_UNAVAILABLE);
        ID3D12CommandList *lists[] = {list.Get()}; d3d12hooks::ExecPlan plan;
        if (!real) d3d12hooks::exec(upload_queue.Get(), 1, lists, plan);
        d3d12hooks::run_plan(plan, upload_queue.Get(), 1, lists, [](auto *q, UINT n, auto *const *ls) { q->ExecuteCommandLists(n, ls); });
        assert(SUCCEEDED(upload_queue->Signal(completed.Get(), ++submission)));
        const auto deadline = GetTickCount64() + 5000;
        while (completed->GetCompletedValue() < submission && GetTickCount64() < deadline) Sleep(1);
        assert(completed->GetCompletedValue() >= submission);
        auto result = api->image(reference, pixels, sizeof pixels, &w, &h, &rev);
        if (retire || discard) { assert(result == SWOFTY_ASSET_UNAVAILABLE); return; }
        const unsigned longest = std::max(width, height);
        assert(result == SWOFTY_ASSET_OK && w == width * data::SIDE / longest && h == height * data::SIDE / longest && rev);
        for (unsigned i = 0; i < w * h * 4; i += 4)
            assert(pixels[i] == 64 && pixels[i+1] == 128 && pixels[i+2] == 191 && pixels[i+3] == 128);
    };
    upload(D3D12_COMMAND_LIST_TYPE_DIRECT, 512, 512, false, false);
    upload(D3D12_COMMAND_LIST_TYPE_COPY, 73, 31, true, false);
    upload(D3D12_COMMAND_LIST_TYPE_DIRECT, 128, 64, false, true);
    upload(D3D12_COMMAND_LIST_TYPE_COPY, 128, 64, false, true);
    upload(D3D12_COMMAND_LIST_TYPE_COPY, 128, 64, false, false, true);
    upload(D3D12_COMMAND_LIST_TYPE_COPY, 128, 64, false, false, false, true);
    // Leave upload-captured My Team pixels for the cross-language probe.
    upload(D3D12_COMMAND_LIST_TYPE_COPY, 128, 64, false, false);
    std::puts("PASS: barrier-free uploads, direct/copy queues, buffer and texture sources, padded rows, offsets, alpha, retirement and discarded lists");
    std::puts(real ? "PASS: full core DLL and real D3D12 detours, named textures, fences and session retirement; no image files" :
        "PASS: real D3D12 WARP copies, RGBA/BGRA, alpha, row pitch, aspect, fences, unloaded epochs, discarded lists, source replacement; no image files");
    return 0;
}
