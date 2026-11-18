#pragma once

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

namespace runtime::heap_sizes {

constexpr uint64_t kMiB = 1048576;
constexpr uint64_t kSmallRam = 12ull * 1024 * kMiB;

constexpr size_t kConfigVirtual = 0x4;
constexpr size_t kConfigExpand = 0x8;
constexpr size_t kConfigPagesize = 0x20;

constexpr size_t kNameLen = 12;
constexpr uint32_t kNameSlots[kNameLen + 1] = {0x60, 0x6f, 0x6a, 0x6e, 0x64, 0x6d, 0x68,
                                              0x6c, 0x80, 0xa7, 0x9c, 0xa6, 0x90};

enum class Factory : uint8_t { plain, internal };
enum class Virtual : uint8_t { any, off, on };

struct Spec {
    const char *name;
    Factory factory;
    uint64_t stock;
    Virtual virt;
    uint32_t pagesize;
    bool expand_zero;
    uint32_t default_mb;
    uint32_t small_mb;
    uint32_t max_mb;
    uint64_t page;
    const char *env;
    const char *tag;
    const char *label;
};

constexpr Spec kHeaps[] = {
    {"AUDIO_BUNDLE", Factory::plain, 43 * kMiB, Virtual::off, 2 * (uint32_t)kMiB, false, 96, 64, 512, 2 * kMiB,
     "SWOFTY_AUDIO_BUNDLE_MB", "[audio] ", "AUDIO_BUNDLE heap"},
    {"SAVE_TEMP_ALLOC", Factory::internal, 75 * kMiB, Virtual::on, 0, true, 512, 320, 1024, 1, "SWOFTY_SAVE_TEMP_MB",
     "[saves] temp pool: ", "SAVE_TEMP_ALLOC"},
    {"GAME_DEFAULT", Factory::plain, 390 * kMiB, Virtual::on, 0, false, 1024, 640, 2600, 1, "SWOFTY_GAME_HEAP_MB", "[heap] ",
     "GAME_DEFAULT heap"},
};
constexpr size_t kHeapCount = sizeof kHeaps / sizeof kHeaps[0];

struct Config {
    uint32_t virt = 0, expand = 0, pagesize = 0;
};

enum class Source : uint8_t { off, ram, small_ram, env };

struct Choice {
    uint64_t bytes = 0;
    Source source = Source::off;
    bool bad_env = false;
    bool raised(const Spec &s) const { return source != Source::off && bytes > s.stock; }
};

inline bool matches(const Spec &s, Factory f, uint64_t size, const Config &c)
{
    if (s.factory != f || size != s.stock)
        return false;
    if (s.virt == Virtual::off && c.virt != 0)
        return false;
    if (s.virt == Virtual::on && c.virt == 0)
        return false;
    if (s.pagesize && c.pagesize != s.pagesize)
        return false;
    if (s.expand_zero && c.expand != 0)
        return false;
    return true;
}

inline const Spec *find(Factory f, uint64_t size, const Config &c)
{
    for (const Spec &s : kHeaps)
        if (matches(s, f, size, c))
            return &s;
    return nullptr;
}

inline uint64_t round_up(uint64_t bytes, uint64_t page)
{
    return (bytes + page - 1) / page * page;
}

inline Choice choose(const Spec &s, const char *env, uint64_t physical_bytes)
{
    Choice c;
    c.bytes = s.stock;
    if (env && *env) {
        if (!std::strcmp(env, "0") || !std::strcmp(env, "off"))
            return c;
        char *end = nullptr;
        unsigned long long mb = std::strtoull(env, &end, 10);
        if (end != env && *end == '\0') {
            if (mb > s.max_mb)
                mb = s.max_mb;
            if (mb * kMiB <= s.stock)
                return c;
            c.bytes = round_up(mb * kMiB, s.page);
            c.source = Source::env;
            return c;
        }
        c.bad_env = true;
    }
    bool small = physical_bytes != 0 && physical_bytes < kSmallRam && s.small_mb != s.default_mb;
    c.bytes = round_up((small ? s.small_mb : s.default_mb) * kMiB, s.page);
    c.source = small ? Source::small_ram : Source::ram;
    return c;
}

inline std::string source_text(const Spec &s, Source src)
{
    switch (src) {
    case Source::off:
        return "the game's size";
    case Source::ram:
        return "default";
    case Source::small_ram:
        return "default for a machine under 12 GiB";
    case Source::env:
        return s.env;
    }
    return "";
}

inline void frame_name(const uint8_t *rsp, char (&out)[kNameLen + 1])
{
    for (size_t k = 0; k < kNameLen; k++) {
        char ch = static_cast<char>(rsp[kNameSlots[k]]);
        out[k] = ch >= 0x20 && ch < 0x7f ? ch : '?';
    }
    out[kNameLen] = '\0';
}

inline std::string apply_line(const Spec &s, const Config &c, const char *frame, uint64_t old_bytes,
                              const Choice &ch)
{
    char b[512];
    char what[160];
    if (s.factory == Factory::plain)
        std::snprintf(what, sizeof what, "heap '%s' (virtual %u, pagesize %u)", frame, c.virt, c.pagesize);
    else
        std::snprintf(what, sizeof what, "heap %s (virtual %u, expand %u%%)", s.name, c.virt, c.expand);
    std::snprintf(b, sizeof b, "%s%s: size %llu -> %llu bytes (%llu MiB, %s)", s.tag, what,
                  (unsigned long long)old_bytes, (unsigned long long)ch.bytes, (unsigned long long)(ch.bytes / kMiB),
                  source_text(s, ch.source).c_str());
    return b;
}

inline std::string kept_line(const Spec &s, const char *env)
{
    char b[400];
    std::snprintf(b, sizeof b, "%s%s kept at the game's %llu MiB (%s=%s)", s.tag, s.label,
                  (unsigned long long)(s.stock / kMiB), s.env, env ? env : "");
    return b;
}

inline std::string bad_env_line(const Spec &s)
{
    return std::string(s.tag) + s.env + " is not a number of MiB, 0 or off; using the default";
}

inline std::string late_line(const Spec &s, const char *stage)
{
    return std::string(s.tag) + s.label + " not raised: the core loaded at the " + stage +
           " stage, after system/memory_map.xml made the heaps (the early stage loads it before)";
}

inline std::string unresolved_line(const Spec &s, const char *site)
{
    return std::string(s.tag) + s.label + " not raised: " + site + " does not resolve on this build";
}

inline std::string armed_line(const Spec &s, const Choice &ch)
{
    char b[400];
    std::snprintf(b, sizeof b,
                  "%s%s armed: %llu MiB instead of the game's %llu MiB when system/memory_map.xml is read", s.tag,
                  s.label, (unsigned long long)(ch.bytes / kMiB), (unsigned long long)(s.stock / kMiB));
    return b;
}

}
