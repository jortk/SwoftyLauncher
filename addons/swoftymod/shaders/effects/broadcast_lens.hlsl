#include "swofty/params.hlsli"

float broadcast_lines()
{
    switch (param_resolution())
    {
    case RESOLUTION_1080: return 1080.0;
    case RESOLUTION_720: return 720.0;
    case RESOLUTION_576: return 576.0;
    default: return 1.0e6;
    }
}

float reduction()
{
    return max(float(swofty_scene_size.y) / broadcast_lines(), 1.0);
}

float4 grab(float2 uv, uint2 pixel)
{
    float3 c = to_linear(scene_at(int2(pixel)).rgb);
    return float4(clamp(c, 0, 60000), 1);
}

float3 tap(float2 uv, float lod)
{
    return input_lod(0, uv, lod).rgb;
}

float4 cubic(float t)
{
    float t2 = t * t, t3 = t2 * t;
    return float4(-0.5 * t3 + t2 - 0.5 * t, 1.5 * t3 - 2.5 * t2 + 1, -1.5 * t3 + 2 * t2 + 0.5 * t, 0.5 * t3 - 0.5 * t2);
}

float3 picture(float2 uv, float r, float lod)
{
    if (r <= 1.001)
        return tap(uv, lod);
    float2 grid = float2(swofty_scene_size) / r;
    float2 g = uv * grid - 0.5;
    float2 base = floor(g);
    float2 f = g - base;
    if (param_resample() == RESAMPLE_SMOOTH)
    {
        float3 top = lerp(tap((base + float2(0.5, 0.5)) / grid, lod), tap((base + float2(1.5, 0.5)) / grid, lod), f.x);
        float3 bot = lerp(tap((base + float2(0.5, 1.5)) / grid, lod), tap((base + float2(1.5, 1.5)) / grid, lod), f.x);
        return lerp(top, bot, f.y);
    }
    float4 wx = cubic(f.x), wy = cubic(f.y);
    float3 sum = 0;
    [unroll] for (int j = 0; j < 4; j++)
    {
        float3 row = 0;
        [unroll] for (int i = 0; i < 4; i++)
            row += tap((base + float2(i - 1, j - 1) + 0.5) / grid, lod) * wx[i];
        sum += row * wy[j];
    }
    return max(sum, 0);
}

float speck(float2 p, uint salt)
{
    float2 i = floor(p), f = frac(p);
    f = f * f * (3 - 2 * f);
    uint2 c = uint2(int2(i) + 4096);
    float a = random(c, salt) + random(c, salt + 31u) - 1;
    float b = random(c + uint2(1, 0), salt) + random(c + uint2(1, 0), salt + 31u) - 1;
    float d = random(c + uint2(0, 1), salt) + random(c + uint2(0, 1), salt + 31u) - 1;
    float e = random(c + uint2(1, 1), salt) + random(c + uint2(1, 1), salt + 31u) - 1;
    return lerp(lerp(a, b, f.x), lerp(d, e, f.x), f.y);
}

float4 lens(float2 uv, uint2 pixel)
{
    float4 original = scene_at(int2(pixel));
    float r = reduction();
    bool neutral = r <= 1.001 && param_softness() <= 0 && param_aberration() <= 0 && param_detail() <= 0 &&
                   param_noise() <= 0 && param_vignette() <= 0;
    if (neutral)
        return original;

    float lod = log2(r * (1 + param_softness()));
    float3 c;
    if (param_aberration() > 0)
    {
        float2 n = (uv - 0.5) * 2;
        float edge = length(n) * 0.70710678;
        float2 shift = n * (edge * param_aberration() * 0.35355339);
        c = float3(picture(uv + shift, r, lod).r, picture(uv, r, lod).g, picture(uv - shift, r, lod).b);
    }
    else
        c = picture(uv, r, lod);

    float2 q = (uv - 0.5) * 2;
    float rim = dot(q, q) * 0.5;
    c *= 1 - param_vignette() * rim;

    float3 p = pow(max(c, 0), 1.0 / 2.2);
    if (param_detail() > 0)
    {
        float3 soft = pow(max(picture(uv, r, lod + 1.0), 0), 1.0 / 2.2);
        float hp = dot(p - soft, float3(0.2126, 0.7152, 0.0722));
        p += clamp(hp * param_detail() * 1.5, -param_detail_limit(), param_detail_limit());
    }
    if (param_noise() > 0)
    {
        float2 cell = float2(pixel) / (r * param_noise_size());
        float luma = saturate(dot(p, float3(0.2126, 0.7152, 0.0722)));
        float weight = lerp(1.0, 0.2, smoothstep(0.05, 0.8, luma));
        float3 grain = speck(cell, 1u) + float3(speck(cell, 2u), 0, speck(cell, 3u)) * 0.35;
        p += grain * (param_noise() * 0.1 * weight);
    }
    c = pow(max(p, 0), 2.2);
    float3 o = from_linear(c);
    if (swofty_encoding == SWOFTY_POSTFX_SDR)
        o = saturate(o);
    return float4(o, original.a);
}
