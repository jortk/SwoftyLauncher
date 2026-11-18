#include "swofty/params.hlsli"

float noise(uint2 cell, uint salt)
{
    return random(cell, salt) + random(cell, salt + 17u) - 1.0;
}

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    uint2 cell = uint2(float2(pixel) / max(param_size(), 1.0));
    float3 n = param_colored() ? float3(noise(cell, 1u), noise(cell, 2u), noise(cell, 3u)) : noise(cell, 1u).xxx;
    float3 p = pow(max(to_linear(c.rgb), 0), 1.0 / 2.2);
    float luma = saturate(dot(p, float3(0.2126, 0.7152, 0.0722)));
    float response = lerp(1, 4 * luma * (1 - luma), param_luma_response());
    p = max(p + n * (param_amount() * 0.12 * response), 0);
    return float4(from_linear(pow(p, 2.2)), c.a);
}
