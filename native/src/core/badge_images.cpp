#include "badge_images.hpp"
#include "badge_image_data.hpp"
#include "badge_archive.hpp"
#include <future>
#include <chrono>
#include "d3d12_hooks.hpp"
#include "runtime.hpp"
#include <swofty/live_images.h>
#include <swofty/json.hpp>
#include <wrl/client.h>
#include <atomic>
#include <deque>
#include <memory>
#include <mutex>

// Runtime badges use the same named-D3D-resource path as grade.cpp's tonemap
// reader. No engine object layouts, texture-table offsets, or game writes.
namespace badge_images
{
namespace
{
using Microsoft::WRL::ComPtr;
std::mutex mu;
std::once_flag registered;
std::atomic<bool> armed{false};
std::map<std::string, std::string> redirects, copies;
uint64_t source_revision = 1, image_revision = 0;
uint64_t epoch = 0;
uint64_t submission_serial = 0;
std::string source_json;
std::atomic<unsigned> badge_names{0};
std::map<std::string, unsigned> misses;
struct Audit
{
    unsigned named{}, barriers{}, uploads{}, queued{}, published{};
    unsigned width{}, height{}, format{}, list_type{}, before{}, after{};
    const char *reason = "no_barrier_seen";
};
std::map<std::string, Audit> audits;
// These weak identities are for diagnosis only. Capture always checks the
// resource's current private name; an old pointer cannot supply pixel identity.
std::map<ID3D12Resource *, std::string> observed_names;
std::map<std::string, uint64_t> name_versions;
uint64_t name_serial = 0;
void invalidate_named(ID3D12Resource *resource, const std::optional<std::string> &reference);
std::optional<std::string> resource_name(ID3D12Resource *resource);
Audit *audit_for(const std::string &reference)
{
    auto at = audits.find(reference);
    if (at != audits.end())
        return &at->second;
    return audits.size() < data::CACHE ? &audits[reference] : nullptr;
}

void named(ID3D12Object *object, LPCWSTR name)
{
    if (!name || !object)
        return;
    std::string text;
    for (size_t i = 0; i < 256 && name[i]; ++i)
    {
        if (name[i] > 127)
            return;
        text += char(name[i]);
    }
    std::string lower = text;
    std::transform(lower.begin(), lower.end(), lower.begin(),
                   [](unsigned char c) { return char(c >= 'A' && c <= 'Z' ? c + 32 : c); });
    {
        std::lock_guard guard(mu);
        observed_names.erase(reinterpret_cast<ID3D12Resource *>(object));
    }
    if (lower.find("badge") == std::string::npos)
        return;
    badge_names.fetch_add(1);
    ComPtr<ID3D12Resource> resource;
    if (FAILED(object->QueryInterface(IID_PPV_ARGS(&resource))))
        return;
    const auto desc = resource->GetDesc();
    auto reference = data::reference(text);
    {
        std::lock_guard guard(mu);
        invalidate_named(resource.Get(), reference);
    }
    if (reference)
    {
        std::lock_guard guard(mu);
        auto audit = audit_for(*reference);
        if (observed_names.size() < data::CACHE)
            observed_names[resource.Get()] = *reference;
        if (!audit)
            return;
        audit->width = unsigned(desc.Width);
        audit->height = desc.Height;
        audit->format = unsigned(desc.Format);
        if (++audit->named != 1)
            return;
    }
    else
    {
        // Unrelated geometry and ingredient textures must not consume the
        // diagnostic budget for actual render_badge_<table> <index> resources.
        static std::atomic<unsigned> ingredients{0};
        if (desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || ingredients.fetch_add(1) >= 8)
            return;
    }
    char line[512];
    std::snprintf(line, sizeof line, "named texture '%s': %llux%u layers=%u format=%u; Live reference recognised=%s",
                  text.c_str(), static_cast<unsigned long long>(desc.Width), desc.Height,
                  unsigned(desc.DepthOrArraySize), unsigned(desc.Format), reference ? "yes" : "no");
    trace(line);
}

struct Image
{
    // A captured asset can outlive a session. Keep its exact resource identity,
    // not a raw address which could be reused by a different texture.
    ComPtr<ID3D12Resource> resource;
    unsigned width{}, height{};
    uint64_t revision{}, used{}, serial{};
    std::vector<uint8_t> rgba;
};
struct Readback
{
    std::string reference;
    ComPtr<ID3D12GraphicsCommandList> list;
    ComPtr<ID3D12Resource> source, buffer, identity;
    ComPtr<ID3D12Fence> fence;
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT footprint{};
    unsigned width{}, height{};
    bool bgra{}, submitted{}, scheduled{}, signal_failed{};
    uint64_t epoch{};
    uint64_t serial{};
    uint64_t name_version{};
};
constexpr size_t RUNTIME_CACHE = 32; // Bound retained 512x512 GPU resources as well as CPU pixels.
std::map<std::string, Image> images;
std::deque<std::shared_ptr<Readback>> pending;

uint64_t version_for(const std::string &reference)
{
    auto at = name_versions.find(reference);
    if (at != name_versions.end())
        return at->second;
    if (name_versions.size() >= data::CACHE)
        return UINT64_MAX;
    return name_versions.emplace(reference, ++name_serial).first->second;
}

void invalidate_named(ID3D12Resource *resource, const std::optional<std::string> &reference)
{
    for (auto it = images.begin(); it != images.end();)
        if (it->second.resource.Get() == resource || (reference && it->first == *reference))
            it = images.erase(it);
        else
            ++it;
    if (reference && (name_versions.contains(*reference) || name_versions.size() < data::CACHE))
        name_versions[*reference] = ++name_serial;
}

void source_changed()
{
    images.clear();
    ++epoch; // Profile/resource-route changes retire in-flight captures too.
    source_json.clear();
    ++source_revision;
}

void poll()
{
    for (auto it = pending.begin(); it != pending.end();)
    {
        auto &job = **it;
        // A failed Signal does not prove a queued copy is finished. Keep its
        // resources alive (within the fixed pending bound) rather than free them.
        if (!job.submitted || job.signal_failed || job.fence->GetCompletedValue() == 0)
        {
            ++it;
            continue;
        }
        if (job.fence->GetCompletedValue() == UINT64_MAX)
        {
            it = pending.erase(it); // removed device: its work cannot execute
            continue;
        }
        uint8_t *mapped = nullptr;
        const size_t bytes = size_t(job.footprint.Offset) +
                             size_t(job.footprint.Footprint.RowPitch) * (job.height - 1) + size_t(job.width) * 4;
        D3D12_RANGE all{0, bytes};
        if (SUCCEEDED(job.buffer->Map(0, &all, reinterpret_cast<void **>(&mapped))))
        {
            unsigned w = 0, h = 0;
            auto rgba = data::pixels(mapped + job.footprint.Offset, job.width, job.height,
                                     job.footprint.Footprint.RowPitch, job.bgra, w, h);
            D3D12_RANGE none{0, 0};
            job.buffer->Unmap(0, &none);
            if (!rgba.empty() && job.epoch == epoch && resource_name(job.identity.Get()) == job.reference &&
                version_for(job.reference) == job.name_version && job.name_version != UINT64_MAX)
            {
                auto found = images.find(job.reference);
                if (found == images.end() && images.size() == RUNTIME_CACHE)
                {
                    auto old = std::min_element(images.begin(), images.end(),
                                                [](auto &a, auto &b) { return a.second.used < b.second.used; });
                    images.erase(old);
                }
                auto &image = images[job.reference];
                image.resource = job.identity;
                if (job.serial >= image.serial && (image.width != w || image.height != h || image.rgba != rgba))
                {
                    image.width = w;
                    image.height = h;
                    image.rgba = std::move(rgba);
                    image.revision = ++image_revision;
                }
                image.serial = std::max(image.serial, job.serial);
                image.used = GetTickCount64();
                if (auto audit = audit_for(job.reference))
                {
                    audit->reason = "published";
                    if (++audit->published == 1)
                        trace(("published pixels '" + job.reference + "'").c_str());
                }
            }
        }
        it = pending.erase(it);
    }
}

std::optional<std::string> resource_name(ID3D12Resource *resource)
{
    static const GUID name_guid = {0x4cca5fd8, 0x921f, 0x42c8, {0x85, 0x66, 0x70, 0xca, 0xf2, 0xa9, 0xb7, 0x41}};
    wchar_t name[256]{};
    UINT size = sizeof name - sizeof(wchar_t);
    if (FAILED(resource->GetPrivateData(name_guid, &size, name)) || size > sizeof name - sizeof(wchar_t) ||
        size % sizeof(wchar_t))
    {
        // D3D12 also supports the narrow debug-name GUID directly.
        static const GUID ansi_guid = {0x429b8c22, 0x9188, 0x4b0c, {0x87, 0x42, 0xac, 0xb0, 0xbf, 0x85, 0xc2, 0x00}};
        char narrow[256]{};
        size = sizeof narrow - 1;
        if (FAILED(resource->GetPrivateData(ansi_guid, &size, narrow)) || size > sizeof narrow - 1)
            return {};
        return data::reference(std::string_view(narrow, strnlen(narrow, size)));
    }
    std::string ascii;
    for (unsigned i = 0; i < size / sizeof(wchar_t) && name[i]; ++i)
    {
        if (name[i] > 127)
            return {};
        ascii += char(name[i]);
    }
    return data::reference(ascii);
}

void texture_copy(ID3D12GraphicsCommandList *list, const D3D12_TEXTURE_COPY_LOCATION *dst, UINT x, UINT y, UINT z,
                  const D3D12_TEXTURE_COPY_LOCATION *src, const D3D12_BOX *box)
{
    // Snapshot the source of the game's actual copy. It is already COPY_SOURCE
    // for that operation, so no guessed destination state or extra transition
    // is required, including uploads which use implicit promotion/decay.
    if (!armed.load() || !list || !dst || !src || !dst->pResource || !src->pResource ||
        dst->Type != D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX || dst->SubresourceIndex != 0)
        return;
    const auto desc = dst->pResource->GetDesc();
    const bool bgra = desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM || desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM_SRGB;
    if (desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || !desc.Width || !desc.Height || desc.Width > 512 ||
        desc.Height > 512 ||
        (!bgra && desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM && desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM_SRGB))
        return;
    std::lock_guard guard(mu);
    poll();
    auto reference = resource_name(dst->pResource);
    if (!reference)
        return;
    auto audit = audit_for(*reference);
    if (audit)
    {
        ++audit->uploads;
        audit->width = unsigned(desc.Width);
        audit->height = desc.Height;
        audit->format = unsigned(desc.Format);
        audit->list_type = unsigned(list->GetType());
    }
    auto reject = [&](const char *reason)
    {
        if (audit)
            audit->reason = reason;
    };
    if (list->GetType() == D3D12_COMMAND_LIST_TYPE_BUNDLE)
    {
        reject("upload_in_bundle");
        return;
    }
    if (x || y || z || desc.DepthOrArraySize != 1 || desc.SampleDesc.Count != 1 || !desc.MipLevels)
    {
        reject("upload_partial_destination");
        return;
    }
    if (box && (box->left || box->top || box->front || box->right != desc.Width || box->bottom != desc.Height ||
                box->back != 1))
    {
        reject("upload_partial_source");
        return;
    }
    if (pending.size() >= data::PENDING)
    {
        reject("pending_limit");
        return;
    }
    auto job = std::make_shared<Readback>();
    job->reference = *reference;
    job->source = src->pResource;
    job->identity = dst->pResource;
    job->name_version = version_for(*reference);
    if (job->name_version == UINT64_MAX)
        return;
    job->list = list;
    job->width = unsigned(desc.Width);
    job->height = desc.Height;
    job->bgra = bgra;
    job->epoch = epoch;
    ComPtr<ID3D12Device> device;
    if (FAILED(dst->pResource->GetDevice(IID_PPV_ARGS(&device))))
    {
        reject("upload_device_unavailable");
        return;
    }
    const auto source_desc = src->pResource->GetDesc();
    UINT64 bytes = 0;
    if (src->Type == D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT)
    {
        const auto &f = src->PlacedFootprint.Footprint;
        const bool source_format =
            bgra ? (f.Format == DXGI_FORMAT_B8G8R8A8_UNORM || f.Format == DXGI_FORMAT_B8G8R8A8_UNORM_SRGB ||
                    f.Format == DXGI_FORMAT_B8G8R8A8_TYPELESS)
                 : (f.Format == DXGI_FORMAT_R8G8B8A8_UNORM || f.Format == DXGI_FORMAT_R8G8B8A8_UNORM_SRGB ||
                    f.Format == DXGI_FORMAT_R8G8B8A8_TYPELESS);
        if (source_desc.Dimension != D3D12_RESOURCE_DIMENSION_BUFFER || f.Width != desc.Width ||
            f.Height != desc.Height || f.Depth != 1 || f.RowPitch < desc.Width * 4 || f.RowPitch > 4096 ||
            !source_format)
        {
            reject("upload_unsupported_footprint");
            return;
        }
        bytes = UINT64(f.RowPitch) * (desc.Height - 1) + desc.Width * 4;
        if (src->PlacedFootprint.Offset > source_desc.Width || bytes > source_desc.Width - src->PlacedFootprint.Offset)
        {
            reject("upload_source_bounds");
            return;
        }
        job->footprint = src->PlacedFootprint;
        job->footprint.Offset = 0;
    }
    else if (src->Type == D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX)
    {
        if (src->SubresourceIndex != 0 || source_desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D ||
            source_desc.Width != desc.Width || source_desc.Height != desc.Height || source_desc.DepthOrArraySize != 1 ||
            source_desc.SampleDesc.Count != 1)
        {
            reject("upload_unsupported_texture_source");
            return;
        }
        device->GetCopyableFootprints(&desc, 0, 1, 0, &job->footprint, nullptr, nullptr, &bytes);
    }
    else
    {
        reject("upload_unknown_source");
        return;
    }
    D3D12_HEAP_PROPERTIES heap{};
    heap.Type = D3D12_HEAP_TYPE_READBACK;
    D3D12_RESOURCE_DESC buffer{};
    buffer.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    buffer.Width = bytes;
    buffer.Height = buffer.DepthOrArraySize = buffer.MipLevels = 1;
    buffer.SampleDesc.Count = 1;
    buffer.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    if (FAILED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &buffer, D3D12_RESOURCE_STATE_COPY_DEST,
                                               nullptr, IID_PPV_ARGS(&job->buffer))) ||
        FAILED(device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&job->fence))))
    {
        reject("upload_readback_allocation_failed");
        return;
    }
    if (src->Type == D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT)
    {
        list->CopyBufferRegion(job->buffer.Get(), 0, src->pResource, src->PlacedFootprint.Offset, bytes);
    }
    else
    {
        D3D12_TEXTURE_COPY_LOCATION target{};
        target.pResource = job->buffer.Get();
        target.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        target.PlacedFootprint = job->footprint;
        list->CopyTextureRegion(&target, 0, 0, 0, src, box);
    }
    pending.push_back(std::move(job));
    if (audit)
    {
        ++audit->queued;
        audit->reason = "upload_copy_queued";
    }
}

void barrier(ID3D12GraphicsCommandList *list, UINT n, const D3D12_RESOURCE_BARRIER *steps)
{
    if (!armed.load() || !list)
        return;
    std::lock_guard guard(mu);
    poll();
    for (UINT i = 0; steps && i < n; ++i)
    {
        const auto &step = steps[i];
        if (step.Type != D3D12_RESOURCE_BARRIER_TYPE_TRANSITION)
            continue;
        auto *resource = step.Transition.pResource;
        if (!resource)
            continue;
        const auto desc = resource->GetDesc();
        auto known = observed_names.find(resource);
        const bool eligible =
            desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D && desc.Width <= 4096 && desc.Height <= 4096 &&
            (step.Transition.StateBefore & (D3D12_RESOURCE_STATE_RENDER_TARGET | D3D12_RESOURCE_STATE_COPY_DEST |
                                            D3D12_RESOURCE_STATE_UNORDERED_ACCESS)) &&
            ((step.Transition.StateAfter &
              (D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE)) ||
             step.Transition.StateAfter == D3D12_RESOURCE_STATE_COMMON);
        if (known == observed_names.end() && !eligible)
            continue;
        auto reference = resource_name(resource);
        Audit *audit = reference                       ? audit_for(*reference)
                       : known != observed_names.end() ? audit_for(known->second)
                                                       : nullptr;
        if (audit)
        {
            ++audit->barriers;
            audit->width = unsigned(desc.Width);
            audit->height = desc.Height;
            audit->format = unsigned(desc.Format);
            audit->list_type = unsigned(list->GetType());
            audit->before = unsigned(step.Transition.StateBefore);
            audit->after = unsigned(step.Transition.StateAfter);
        }
        auto reject = [&](const char *reason)
        {
            if (audit)
                audit->reason = reason;
        };
        if (!reference)
        {
            reject("private_name_unavailable");
            continue;
        }
        if (list->GetType() != D3D12_COMMAND_LIST_TYPE_DIRECT)
        {
            reject("not_direct_queue");
            continue;
        }
        if (step.Flags != D3D12_RESOURCE_BARRIER_FLAG_NONE)
        {
            reject("split_barrier");
            continue;
        }
        if (step.Transition.Subresource != D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES && step.Transition.Subresource != 0)
        {
            reject("not_mip_zero");
            continue;
        }
        if (!(step.Transition.StateBefore & (D3D12_RESOURCE_STATE_RENDER_TARGET | D3D12_RESOURCE_STATE_COPY_DEST |
                                             D3D12_RESOURCE_STATE_UNORDERED_ACCESS)))
        {
            reject("not_written_to_read");
            continue;
        }
        if (!(step.Transition.StateAfter &
              (D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE)))
        {
            reject("not_shader_read");
            continue;
        }
        if (desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || !desc.Width || !desc.Height || desc.Width > 512 ||
            desc.Height > 512 || desc.DepthOrArraySize != 1 || desc.SampleDesc.Count != 1 || desc.MipLevels < 1)
        {
            reject("unsupported_shape");
            continue;
        }
        const bool bgra = desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM || desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM_SRGB;
        if (!bgra && desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM && desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM_SRGB)
        {
            reject("unsupported_format");
            continue;
        }
        if (pending.size() >= data::PENDING)
        {
            reject("pending_limit");
            continue;
        }
        auto job = std::make_shared<Readback>();
        job->reference = *reference;
        job->source = resource;
        job->identity = resource;
        job->name_version = version_for(*reference);
        if (job->name_version == UINT64_MAX)
            continue;
        job->list = list;
        job->width = unsigned(desc.Width);
        job->height = desc.Height;
        job->bgra = bgra;
        job->epoch = epoch;
        ComPtr<ID3D12Device> device;
        if (FAILED(resource->GetDevice(IID_PPV_ARGS(&device))))
        {
            reject("device_unavailable");
            continue;
        }
        UINT64 bytes = 0;
        device->GetCopyableFootprints(&desc, 0, 1, 0, &job->footprint, nullptr, nullptr, &bytes);
        D3D12_HEAP_PROPERTIES heap{};
        heap.Type = D3D12_HEAP_TYPE_READBACK;
        D3D12_RESOURCE_DESC buffer{};
        buffer.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
        buffer.Width = bytes;
        buffer.Height = buffer.DepthOrArraySize = buffer.MipLevels = 1;
        buffer.SampleDesc.Count = 1;
        buffer.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
        if (FAILED(device->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &buffer, D3D12_RESOURCE_STATE_COPY_DEST,
                                                   nullptr, IID_PPV_ARGS(&job->buffer))) ||
            FAILED(device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&job->fence))))
        {
            reject("readback_allocation_failed");
            continue;
        }
        auto move = step;
        move.Transition.StateAfter = D3D12_RESOURCE_STATE_COPY_SOURCE;
        d3d12hooks::barrier_original(list, 1, &move);
        D3D12_TEXTURE_COPY_LOCATION from{}, to{};
        from.pResource = resource;
        from.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        to.pResource = job->buffer.Get();
        to.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        to.PlacedFootprint = job->footprint;
        list->CopyTextureRegion(&to, 0, 0, 0, &from, nullptr);
        std::swap(move.Transition.StateBefore, move.Transition.StateAfter);
        d3d12hooks::barrier_original(list, 1, &move);
        pending.push_back(std::move(job));
        if (audit)
        {
            ++audit->queued;
            audit->reason = "queued";
        }
    }
    // normal game transitions still execute exactly once
}

void execute(ID3D12CommandQueue *queue, UINT n, ID3D12CommandList *const *lists, d3d12hooks::ExecPlan &plan)
{
    const auto type = queue->GetDesc().Type;
    if (type != D3D12_COMMAND_LIST_TYPE_DIRECT && type != D3D12_COMMAND_LIST_TYPE_COPY &&
        type != D3D12_COMMAND_LIST_TYPE_COMPUTE)
        return;
    std::lock_guard guard(mu);
    poll();
    for (auto &job : pending)
    {
        if (job->submitted || job->scheduled)
            continue;
        for (UINT i = 0; i < n; ++i)
            if (static_cast<ID3D12CommandList *>(job->list.Get()) == lists[i])
            {
                job->scheduled = true;
                plan.after(i,
                           [job](ID3D12CommandQueue *q)
                           {
                               std::lock_guard lock(mu);
                               job->serial = ++submission_serial;
                               job->signal_failed = FAILED(q->Signal(job->fence.Get(), 1));
                               job->submitted = true;
                           });
                break;
            }
    }
}

void reset(ID3D12GraphicsCommandList *list)
{
    std::lock_guard guard(mu);
    std::erase_if(pending, [&](auto &p) { return !p->scheduled && p->list.Get() == list; });
}

int start()
{
    std::call_once(registered,
                   []
                   {
                       d3d12hooks::on_barrier_observe(barrier);
                       d3d12hooks::on_exec(execute);
                       d3d12hooks::on_reset(reset);
                       d3d12hooks::on_setname(named);
                       d3d12hooks::on_texture_copy(texture_copy);
                   });
    if (!armed.load())
        armed.store(d3d12hooks::arm_device() && d3d12hooks::arm_list_state());
    return armed.load() ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
}

int sources(char *out, uint32_t cap, uint64_t *revision)
{
    if (!out || !cap || !revision)
        return SWOFTY_ASSET_INVALID;
    std::lock_guard guard(mu);
    if (source_json.empty())
    {
        auto doc = swofty::json::Value::object();
        const auto bytes = runtime::read_file(runtime::under_swofty("assets.json"), size_t(16) << 20);
        doc["assets_hash"] = swofty::json::Value(runtime::hex16(runtime::fnv64(bytes)));
        auto routes = swofty::json::Value::object();
        for (auto &[from, to] : redirects)
            if (from.ends_with(".erp") && (from.starts_with("2025_asset_groups/ui_package/") ||
                                           from == "2025_asset_groups/vfx_package/common.erp"))
                routes[from] = swofty::json::Value(to);
        for (auto &[from, to] : copies)
            routes[from] = swofty::json::Value(to);
        doc["routes"] = routes;
        source_json = swofty::json::dump(doc);
    }
    if (source_json.size() >= cap)
        return SWOFTY_ASSET_INVALID;
    std::memcpy(out, source_json.c_str(), source_json.size() + 1);
    *revision = source_revision;
    return SWOFTY_ASSET_OK;
}

struct ArchiveWork
{
    std::shared_ptr<archive::Catalogue> catalogue;
    std::map<std::string, archive::Pixels> result;
    uint64_t source{}, session{};
};
struct ArchiveAccess
{
    std::mutex lock;
    std::future<ArchiveWork> pending;
    std::shared_ptr<archive::Catalogue> catalogue;
    std::map<std::string, archive::Pixels> cache;
    std::set<std::string> waiting;
    uint64_t source{}, session{}, serial{};
};
ArchiveAccess archive_access;

int archive_image(const char *reference, uint8_t *rgba, uint32_t cap, uint32_t *width, uint32_t *height,
                  uint64_t *revision)
{
    archive::Env env;
    uint64_t source, session;
    {
        std::lock_guard guard(mu);
        source = source_revision;
        session = epoch;
        env.routes = redirects;
        for (const auto &[from, to] : copies)
            env.routes[from] = to;
    }
    env.root = runtime::game_root();
    for (auto &entry : runtime::config()["resource_erps"].a)
    {
        bool relevant = false;
        for (auto &erp : entry["erps"].a)
            if (erp.str().starts_with("2025_asset_groups/ui_package/"))
                relevant = true;
        auto rel = entry["file"].str();
        std::replace(rel.begin(), rel.end(), '\\', '/');
        if (relevant)
            env.overlays.push_back("swofty/" + rel);
    }
    auto &state = archive_access;
    std::lock_guard guard(state.lock);
    if (state.source != source || state.session != session)
    {
        state.source = source;
        state.session = session;
        state.catalogue.reset();
        state.cache.clear();
        state.waiting.clear();
        ++state.serial;
    }
    if (state.pending.valid() && state.pending.wait_for(std::chrono::seconds(0)) == std::future_status::ready)
    {
        auto result = state.pending.get();
        if (result.source == source && result.session == session)
        {
            state.catalogue = std::move(result.catalogue);
            for (auto &[key, pixels] : result.result)
            {
                state.cache[key] = std::move(pixels);
                state.waiting.erase(key);
            }
            ++state.serial;
        }
    }
    auto found = state.cache.find(reference);
    if (found != state.cache.end())
    {
        const auto &pixels = found->second;
        if (pixels.rgba.empty())
            return SWOFTY_ASSET_UNAVAILABLE;
        if (cap < pixels.rgba.size())
            return SWOFTY_ASSET_INVALID;
        std::memcpy(rgba, pixels.rgba.data(), pixels.rgba.size());
        *width = pixels.width;
        *height = pixels.height;
        *revision = state.serial;
        return SWOFTY_ASSET_OK;
    }
    if (state.cache.size() >= 128 || state.waiting.size() >= 32)
        return SWOFTY_ASSET_UNAVAILABLE;
    state.waiting.insert(reference);
    if (!state.pending.valid())
    {
        auto keys = state.waiting;
        auto catalogue = state.catalogue;
        state.pending = std::async(
            std::launch::async,
            [env = std::move(env), keys = std::move(keys), catalogue = std::move(catalogue), source, session]() mutable
            {
                ArchiveWork result;
                result.source = source;
                result.session = session;
                try
                {
                    std::unique_ptr<archive::Resources> resources;
                    auto get_resources = [&]() -> archive::Resources &
                    {
                        if (!resources)
                            resources = std::make_unique<archive::Resources>(env);
                        return *resources;
                    };
                    if (!catalogue)
                    {
                        auto loaded = std::make_shared<archive::Catalogue>();
                        if (loaded->load(get_resources()))
                            catalogue = std::move(loaded);
                    }
                    result.catalogue = catalogue;
                    for (auto &key : keys)
                    {
                        archive::Pixels pixels;
                        if (key.starts_with(archive::PREFIX))
                        {
                            if (catalogue)
                                catalogue->decode(key, pixels);
                        }
                        else
                        {
                            std::vector<std::string> fragments;
                            archive::Surface surface;
                            if (get_resources().read(key + ".image", fragments) && surface.load(fragments))
                                surface.decode(0, pixels);
                        }
                        result.result.emplace(key, std::move(pixels));
                    }
                }
                catch (...)
                {
                    for (auto &key : keys)
                        result.result.emplace(key, archive::Pixels{});
                }
                return result;
            });
    }
    return SWOFTY_ASSET_LATE;
}

int image(const char *reference, uint8_t *rgba, uint32_t cap, uint32_t *width, uint32_t *height, uint64_t *revision)
{
    if (!reference || !rgba || !width || !height || !revision || strnlen(reference, 160) >= 160)
        return SWOFTY_ASSET_INVALID;
    if (std::string_view(reference).starts_with(archive::PREFIX))
        return archive_image(reference, rgba, cap, width, height, revision);
    if (!data::reference(reference))
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!armed.load())
        start();
    std::unique_lock guard(mu);
    poll();
    auto found = images.find(reference);
    if (found != images.end() && resource_name(found->second.resource.Get()) != reference)
    {
        images.erase(found);
        found = images.end();
    }
    if (found == images.end())
    {
        if (std::string_view(reference) == "eaid://ui_package/art/textures/render_badge_default.tga")
        {
            guard.unlock();
            return archive_image(reference, rgba, cap, width, height, revision);
        }
        if (misses.size() < 32 || misses.contains(reference))
        {
            auto count = ++misses[reference];
            if (count == 1 || count == 120 || count == 3600)
            {
                char line[512];
                const auto at = audits.find(reference);
                const Audit empty{};
                const auto &a = at == audits.end() ? empty : at->second;
                std::snprintf(line, sizeof line,
                              "image unavailable '%s': miss=%u cached=%zu pending=%zu named=%u shape=%ux%u format=%u "
                              "barriers=%u uploads=%u list=%u states=%x->%x queued=%u published=%u reason=%s",
                              reference, count, images.size(), pending.size(), a.named, a.width, a.height, a.format,
                              a.barriers, a.uploads, a.list_type, a.before, a.after, a.queued, a.published, a.reason);
                trace(line);
            }
        }
        return SWOFTY_ASSET_UNAVAILABLE;
    }
    auto &image = found->second;
    if (cap < image.rgba.size())
        return SWOFTY_ASSET_INVALID;
    std::memcpy(rgba, image.rgba.data(), image.rgba.size());
    *width = image.width;
    *height = image.height;
    *revision = image.revision;
    image.used = GetTickCount64();
    return SWOFTY_ASSET_OK;
}

void clear()
{
    std::lock_guard guard(mu);
    char line[128];
    std::snprintf(line, sizeof line, "session clear: cached=%zu pending=%zu", images.size(), pending.size());
    trace(line);
    misses.clear();
    ++epoch;
    images.clear();
    // Submitted jobs own GPU resources until their fence completes. Their old
    // epoch prevents their pixels from reappearing after session unload.
    poll();
}
} // namespace

void session_unloaded()
{
    std::lock_guard guard(mu);
    // The game uploads career badges before loading the track and can reuse
    // those same named resources across GP/menu/career transitions. Retire
    // session work, but retain completed, resource-owned asset captures. Live
    // session gates still clear driver rows/references; explicit api.clear()
    // and changed routes continue to discard every cached image.
    ++epoch;
    misses.clear();
    poll();
    trace("session unload: retired pending epoch; retained bounded resource-owned badge assets");
}

void file_routes(const std::map<std::string, std::string> &routes)
{
    std::lock_guard guard(mu);
    if (redirects != routes)
    {
        redirects = routes;
        source_changed();
    }
}
void overlay_routes(const std::map<std::string, std::string> &routes)
{
    std::lock_guard guard(mu);
    if (copies != routes)
    {
        copies = routes;
        source_changed();
    }
}
} // namespace badge_images

const SwoftyLiveImages *badge_images::api()
{
    static const SwoftyLiveImages api{1, badge_images::start, badge_images::sources, badge_images::image,
                                      badge_images::clear};
    return &api;
}
