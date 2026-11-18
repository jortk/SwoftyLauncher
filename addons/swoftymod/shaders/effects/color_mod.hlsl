#include "swofty/params.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 p = pow(max(to_linear(c.rgb), 0), 1.0 / 2.2);
    float3 w = float3(0.2126, 0.7152, 0.0722);
    float3 stretched = (p - 0.5) * param_contrast() + 0.5;
    float stretched_y = dot(stretched, w);
    p = stretched_y + (stretched - stretched_y) / max(dot(param_contrast(), w), 0.05);
    p = pow(max(p, 0), pow(max(param_gamma(), 0.01), 0.6));
    p += param_brightness();
    float py = dot(p, w);
    p = py + (p - py) * param_chroma();
    float3 rgb = pow(max(p, 0), 2.2);
    return float4(from_linear(rgb), c.a);
}
