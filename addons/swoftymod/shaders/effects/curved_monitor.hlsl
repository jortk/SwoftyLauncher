#include "swofty/params.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float aspect = float(swofty_size.x) / float(swofty_size.y);
    float height = max(param_monitor_height(), 1);
    float width = height * aspect;
    float radius = max(param_monitor_curvature() * 0.1, 1);
    float distance_cm = max(param_view_distance(), 1);

    float2 q = (uv - 0.5) * 2;
    float arc = q.x * width * 0.5;
    float phi = clamp(arc / radius, -1.5, 1.5);
    float x = radius * sin(phi);
    float depth = max(distance_cm - radius * (1 - cos(phi)), 1e-2);
    float reach = length(float2(x, depth));
    float y = q.y * height * 0.5;

    float2 s;
    s.x = distance_cm * (x / depth) / (width * 0.5);
    s.y = distance_cm * (y / reach) / (height * 0.5);
    s /= max(param_border_zoom(), 0.01);
    float2 suv = s * 0.5 + 0.5;
    bool inside = all(suv >= 0) && all(suv <= 1);

    float4 border = param_border_color();
    float3 border_rgb = from_linear(srgb_to_linear(border.rgb));
    float3 rgb;
    if (inside)
        rgb = scene(suv).rgb;
    else
    {
        float3 behind = scene(param_mirror_border() ? 1 - abs(frac(suv * 0.5) * 2 - 1) : uv).rgb;
        rgb = lerp(behind, border_rgb, border.a);
        rgb = from_linear(to_linear(rgb) * (1 - saturate(param_background_dim())));
    }
    return float4(rgb, 1);
}
