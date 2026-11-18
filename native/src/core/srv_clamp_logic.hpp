#pragma once

#include <cstdint>

namespace runtime::srv_clamp {

constexpr uint32_t kAll = 0xffffffffu;

enum Dimension : int {
    kUnknown = 0,
    kBuffer = 1,
    kTexture1D = 2,
    kTexture1DArray = 3,
    kTexture2D = 4,
    kTexture2DArray = 5,
    kTexture2DMS = 6,
    kTexture2DMSArray = 7,
    kTexture3D = 8,
    kTextureCube = 9,
    kTextureCubeArray = 10,
};

struct View {
    int dimension = kUnknown;
    uint32_t first_slice = 0;
    uint32_t slices = 0;
    uint32_t most_detailed_mip = 0;
    uint32_t mip_levels = 1;
};

struct Resource {
    bool texture = false;
    bool volume = false;
    uint32_t slices = 1;
    uint32_t mips = 1;
};

enum class Action { Pass, Clamp, Skip };

struct Plan {
    Action action = Action::Pass;
    uint32_t slices = 0;
    uint32_t most_detailed_mip = 0;
    uint32_t mip_levels = 0;
    bool slices_changed = false;
    bool mips_changed = false;
};

inline bool is_array(int d)
{
    return d == kTexture1DArray || d == kTexture2DArray || d == kTexture2DMSArray || d == kTextureCubeArray;
}

inline bool has_mips(int d)
{
    return d == kTexture1D || d == kTexture1DArray || d == kTexture2D || d == kTexture2DArray || d == kTexture3D ||
           d == kTextureCube || d == kTextureCubeArray;
}

inline uint32_t cube_slices(uint32_t cubes)
{
    return cubes > kAll / 6 ? kAll : cubes * 6;
}

inline Plan plan(const View &v, const Resource &r)
{
    Plan p;
    p.slices = v.slices;
    p.most_detailed_mip = v.most_detailed_mip;
    p.mip_levels = v.mip_levels;
    if (!r.texture || r.volume || v.dimension == kBuffer || v.dimension == kUnknown || v.dimension == kTexture3D)
        return p;
    if (r.slices == 0 || r.mips == 0)
        return p;
    if (is_array(v.dimension) && v.slices != kAll) {
        if (v.first_slice >= r.slices) {
            p.action = Action::Skip;
            return p;
        }
        uint64_t end = uint64_t(v.first_slice) + v.slices;
        if (end > r.slices) {
            p.slices = r.slices - v.first_slice;
            p.slices_changed = true;
        }
    }
    if (has_mips(v.dimension)) {
        if (v.most_detailed_mip >= r.mips) {
            p.most_detailed_mip = r.mips - 1;
            p.mips_changed = true;
        }
        if (v.mip_levels != kAll && uint64_t(p.most_detailed_mip) + v.mip_levels > r.mips) {
            p.mip_levels = r.mips - p.most_detailed_mip;
            p.mips_changed = true;
        }
    }
    if (p.slices_changed || p.mips_changed)
        p.action = Action::Clamp;
    return p;
}

inline bool enabled(const char *env)
{
    return env && env[0] == '1' && !env[1];
}

}
