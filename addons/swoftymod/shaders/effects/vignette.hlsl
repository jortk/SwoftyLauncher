#include "swofty/params.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float aspect = float(swofty_size.x) / float(swofty_size.y);
    float2 p = (uv - 0.5) * 2;
    p.x *= lerp(1, aspect, param_roundness());
    float corner = length(float2(lerp(1, aspect, param_roundness()), 1));
    float d = length(p) / corner;
    float mask = smoothstep(param_radius(), param_radius() + max(param_softness(), 0.01), d);
    float3 rgb = to_linear(c.rgb);
    float3 tint = srgb_to_linear(saturate(param_color()));
    rgb = lerp(rgb, tint, mask * saturate(param_amount()));
    return float4(from_linear(max(rgb, 0)), c.a);
}
