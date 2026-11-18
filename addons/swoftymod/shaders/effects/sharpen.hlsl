#include "swofty/params.hlsli"

float3 perceptual(int2 pixel)
{
    return pow(max(to_linear(scene_at(pixel).rgb), 0), 1.0 / 2.2);
}

float4 effect(float2 uv, uint2 pixel)
{
    int2 p = int2(pixel);
    float4 original = scene_at(p);
    float3 e = perceptual(p);
    float3 b = perceptual(p + int2(0, -1)), d = perceptual(p + int2(-1, 0));
    float3 f = perceptual(p + int2(1, 0)), h = perceptual(p + int2(0, 1));
    float3 mn = min(e, min(min(b, d), min(f, h)));
    float3 mx = max(e, max(max(b, d), max(f, h)));
    float3 headroom = min(mn, 1 - mx) / max(mx, 1e-4);
    float3 amp = sqrt(saturate(headroom));
    float3 w = -amp / lerp(8, 5, saturate(param_sharpness()));
    float3 sharp = (e + (b + d + f + h) * w) / (1 + 4 * w);
    float3 out_p = max(e + clamp(sharp - e, -param_limit(), param_limit()), 0);
    return float4(from_linear(pow(out_p, 2.2)), original.a);
}
