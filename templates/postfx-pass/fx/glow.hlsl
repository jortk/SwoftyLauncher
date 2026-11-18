#include "swofty/postfx.hlsli"

float4 down(float2 uv, uint2 pixel)
{
    float2 texel = 1.0 / float2(input_size(0));
    float3 sum = 0;
    sum += to_linear(input(0, uv + float2(-1, -1) * texel).rgb);
    sum += to_linear(input(0, uv + float2(1, -1) * texel).rgb);
    sum += to_linear(input(0, uv + float2(-1, 1) * texel).rgb);
    sum += to_linear(input(0, uv + float2(1, 1) * texel).rgb);
    float3 rgb = sum * 0.25;
    return float4(max(rgb - 0.8, 0), 1);
}

float4 up(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    rgb += input(0, uv).rgb * (option(0) / 100) * 2;
    return float4(from_linear(rgb), c.a);
}
