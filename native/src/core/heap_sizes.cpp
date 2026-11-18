#include "heap_sizes_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <vector>

namespace {

namespace hs = runtime::heap_sizes;

constexpr const char *MIXIN = "swofty_core.dll (heap sizes)";
constexpr const char *SITE_PLAIN = "site_Ne_MemoryMapHeapSize";
constexpr const char *SITE_INTERNAL = "site_Ne_MemoryMapInternalHeapSize";

hs::Choice choices[hs::kHeapCount];
std::atomic<bool> applied[hs::kHeapCount];
bool offered = false, settled = false, armed[hs::kHeapCount];

hs::Config read_config(const uint8_t *config)
{
    hs::Config c;
    std::memcpy(&c.virt, config + hs::kConfigVirtual, sizeof c.virt);
    std::memcpy(&c.expand, config + hs::kConfigExpand, sizeof c.expand);
    std::memcpy(&c.pagesize, config + hs::kConfigPagesize, sizeof c.pagesize);
    return c;
}

uint64_t apply(SwoftyContext *c, hs::Factory factory, uint64_t size)
{
    hs::Config config = read_config(reinterpret_cast<const uint8_t *>(c->rdi));
    const hs::Spec *spec = hs::find(factory, size, config);
    if (!spec)
        return size;
    size_t i = spec - hs::kHeaps;
    if (!armed[i] || applied[i].exchange(true))
        return size;
    char frame[hs::kNameLen + 1] = {};
    if (factory == hs::Factory::plain)
        hs::frame_name(reinterpret_cast<const uint8_t *>(c->rsp), frame);
    swofty::log("%s", hs::apply_line(*spec, config, frame, size, choices[i]).c_str());
    return choices[i].bytes;
}

void plain_size(SwoftyContext *c)
{
    c->r9 = apply(c, hs::Factory::plain, c->r9);
}

void internal_size(SwoftyContext *c)
{
    c->r15 = apply(c, hs::Factory::internal, c->r15);
}

}

namespace runtime {

std::vector<CoreMixins> heap_sizes_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    MEMORYSTATUSEX mem = {};
    mem.dwLength = sizeof mem;
    uint64_t physical = GlobalMemoryStatusEx(&mem) ? mem.ullTotalPhys : 0;
    bool early = load_stage() == "early";
    bool plain = false, internal = false;
    bool resolved[2] = {};
    bool checked[2] = {};
    for (size_t i = 0; i < hs::kHeapCount; i++) {
        const hs::Spec &s = hs::kHeaps[i];
        char v[16] = {};
        DWORD n = GetEnvironmentVariableA(s.env, v, sizeof v);
        const char *env = n && n < sizeof v ? v : nullptr;
        choices[i] = hs::choose(s, env, physical);
        if (n >= sizeof v || choices[i].bad_env)
            swofty::log("%s", hs::bad_env_line(s).c_str());
        if (!choices[i].raised(s)) {
            swofty::log("%s", hs::kept_line(s, env).c_str());
            continue;
        }
        if (!early) {
            swofty::log("%s", hs::late_line(s, load_stage().c_str()).c_str());
            continue;
        }
        size_t f = s.factory == hs::Factory::plain ? 0 : 1;
        const char *site = f == 0 ? SITE_PLAIN : SITE_INTERNAL;
        if (!checked[f]) {
            checked[f] = true;
            resolved[f] = swofty::api->resolve(site) != 0;
        }
        if (!resolved[f]) {
            swofty::log("%s", hs::unresolved_line(s, site).c_str());
            continue;
        }
        armed[i] = true;
        (f == 0 ? plain : internal) = true;
    }
    if (!plain && !internal)
        return out;
    CoreMixins set{MIXIN, {}};
    if (plain)
        set.mixins.push_back({SITE_PLAIN, SWOFTY_MIXIN_SITE, 100, (void *)&plain_size, nullptr});
    if (internal)
        set.mixins.push_back({SITE_INTERNAL, SWOFTY_MIXIN_SITE, 100, (void *)&internal_size, nullptr});
    out.push_back(std::move(set));
    return out;
}

void heap_sizes_commit()
{
    if (!offered || settled)
        return;
    settled = true;
    if (!core_mixins_active(MIXIN))
        return;
    for (size_t i = 0; i < hs::kHeapCount; i++)
        if (armed[i])
            swofty::log("%s", hs::armed_line(hs::kHeaps[i], choices[i]).c_str());
}

bool save_heap_raised(uint64_t &bytes)
{
    constexpr size_t i = 1;
    static_assert(hs::kHeaps[i].name[0] == 'S');
    bool on = applied[i].load();
    bytes = on ? choices[i].bytes : hs::kHeaps[i].stock;
    return on;
}

}
