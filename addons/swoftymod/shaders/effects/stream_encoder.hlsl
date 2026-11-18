#include "swofty/params.hlsli"

static const float3 LUMA = float3(0.2126, 0.7152, 0.0722);

float3 perceptual(float3 encoded)
{
    return pow(max(to_linear(encoded), 0), 1.0 / 2.2);
}

float4 half_res(float2 uv, uint2 pixel)
{
    int2 p = int2(pixel) * 2;
    float3 sum = perceptual(scene_at(p).rgb) + perceptual(scene_at(p + int2(1, 0)).rgb) +
                 perceptual(scene_at(p + int2(0, 1)).rgb) + perceptual(scene_at(p + int2(1, 1)).rgb);
    float3 c = min(sum * 0.25, 1000);
    return float4(c, dot(c, LUMA));
}

float4 diff(float2 uv, uint2 pixel)
{
    float d = has_previous() ? abs(input_at(0, int2(pixel)).a - input_at(1, int2(pixel)).a) : 0;
    return float4(d, 0, 0, 1);
}

float4 keep(float2 uv, uint2 pixel)
{
    return input_at(0, int2(pixel));
}

float4 motion(float2 uv, uint2 pixel)
{
    float raw = input_lod(0, uv, 2).r * 0.6 + input_lod(0, uv, 3).r * 0.4;
    float old = input_at(1, int2(pixel)).r;
    float dt = clamp(swofty_frametime, 0, 0.1);
    float rate = raw > old ? 25.0 : 3.0;
    float next = has_previous() ? lerp(old, raw, 1 - exp(-dt * rate)) : 0;
    return float4(next, 0, 0, 1);
}

float4 motion_keep(float2 uv, uint2 pixel)
{
    return input_at(0, int2(pixel));
}

float4 encode(float2 uv, uint2 pixel)
{
    float4 original = scene_at(int2(pixel));
    float s = param_strength();
    if (s <= 0)
        return original;

    float3 x = perceptual(original.rgb);
    float3 o = x;
    bool wide = param_block_size() == BLOCK_SIZE_16X16;
    float block = wide ? 16.0 : 8.0;
    float lod_block = wide ? 3.0 : 2.0;
    float2 size = float2(swofty_scene_size);
    float2 block_uv = (floor(float2(pixel) / block) + 0.5) * block / size;

    float sens = param_motion_sensitivity();
    float moving = saturate((input(1, block_uv).r - 0.004) * lerp(8.0, 60.0, sens));

    if (param_chroma())
    {
        float3 half_c = input(0, uv).rgb;
        float3 subsampled = dot(x, LUMA) + half_c - dot(half_c, LUMA);
        o = lerp(o, subsampled, s);
    }

    float k = saturate(param_blocking() * s * (0.15 + 1.1 * moving));
    if (k > 0)
    {
        float3 mean = input_lod_point(0, block_uv, lod_block).rgb;
        float3 low = input_lod(0, uv, lod_block).rgb;
        o += clamp((mean - low) * k, -0.06, 0.06);
    }

    float b = param_banding() * s;
    if (b > 0)
    {
        float fine = dot(input_lod(0, uv, 1).rgb, LUMA);
        float coarse = dot(input_lod(0, uv, 3).rgb, LUMA);
        float flat_area = 1 - smoothstep(0.01, 0.045, abs(fine - coarse));
        float step_size = 1.0 / lerp(256.0, 20.0, b);
        float quantised = floor(fine / step_size + 0.5) * step_size;
        o += (quantised - fine) * flat_area;
    }

    if (has_previous())
    {
        float w = moving * s * param_smear() * 0.4;
        float3 before = perceptual(input(2, uv).rgb);
        o = lerp(o, before, w);
    }

    float3 result = from_linear(pow(max(o, 0), 2.2));
    if (swofty_encoding == SWOFTY_POSTFX_SDR)
        result = saturate(result);
    return float4(result, original.a);
}
