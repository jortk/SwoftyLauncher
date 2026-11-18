#include "swofty/params.hlsli"

float perceptual(float3 rgb)
{
    return pow(max(luminance(rgb), 0), 1.0 / 2.2);
}

float4 meter(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(swofty_scene_size);
    float lo = 1e4, hi = 0, sum = 0;
    [unroll] for (int y = 0; y < 4; y++)
        [unroll] for (int x = 0; x < 4; x++)
        {
            float p = perceptual(to_linear(scene(uv + t * (float2(x, y) - 1.5) * 4).rgb));
            lo = min(lo, p);
            hi = max(hi, p);
            sum += p;
        }
    return float4(lo, hi, sum / 16, 1);
}

float4 meter2(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(input_size(0));
    float3 sum = 0;
    [unroll] for (int y = 0; y < 8; y++)
        [unroll] for (int x = 0; x < 8; x++)
            sum += input(0, uv + t * (float2(x, y) - 3.5)).rgb;
    return float4(sum / 64, 1);
}

float4 stats_calc(float2 uv, uint2 pixel)
{
    uint2 size = input_size(0);
    float3 sum = 0;
    for (uint y = 0; y < size.y; y++)
        for (uint x = 0; x < size.x; x++)
            sum += input_at(0, int2(x, y)).rgb;
    float3 target = sum / float(size.x * size.y);
    float4 state = input_at(1, int2(0, 0));
    float dt = clamp(swofty_frametime, 0, 0.1);
    float3 eased = state.xyz + (target - state.xyz) * (1 - exp(-dt * 2));
    float limit = param_max_variance() * dt;
    float3 next = state.xyz + clamp(eased - state.xyz, -limit, limit);
    return state.w > 0.5 ? float4(next, 1) : float4(target, 1);
}

float4 stats_store(float2 uv, uint2 pixel)
{
    return input(0, float2(0.5, 0.5));
}

float zone_gain(float peak, float result)
{
    return result / max(peak, 0.05);
}

float4 apply(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float4 stats = input_at(0, int2(0, 0));
    if (stats.w < 0.5)
        return c;
    float3 rgb = max(to_linear(c.rgb), 0);
    float y_lin = luminance(rgb);
    if (y_lin < 1e-5)
        return c;
    float p = pow(y_lin, 1.0 / 2.2);

    float black = saturate(stats.x) * param_minimum();
    float white = 1 - (1 - saturate(stats.y)) * param_maximum();
    white = max(white, black + 0.1);
    float v = (p - black) / (white - black);
    float vc = saturate(v);

    float wd = param_dark_threshold() > 0 ? 1 - smoothstep(0, param_dark_threshold(), vc) : 0;
    float wl = param_light_threshold() > 0 ? smoothstep(param_light_threshold(), 1, vc) : 0;
    float wm = saturate(1 - wd - wl);
    float norm = max(wd + wl + wm, 1e-4);
    float g = (wd * zone_gain(param_dark_peak(), param_dark_max()) + wl * zone_gain(param_light_peak(), param_light_max()) +
               wm * zone_gain(param_mid_peak(), param_mid_max())) / norm;
    float shaped = vc * (1 + (g - 1) * (1 - pow(vc, 6)));
    float out_p = shaped + max(v - 1, 0);

    float ratio = pow(max(out_p, 0), 2.2) / y_lin;
    return float4(from_linear(rgb * min(ratio, 8)), c.a);
}
