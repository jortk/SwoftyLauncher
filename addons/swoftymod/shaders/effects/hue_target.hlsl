#include "swofty/params.hlsli"

float3 rgb_to_hsv(float3 c)
{
    float hi = max(max(c.r, c.g), c.b);
    float lo = min(min(c.r, c.g), c.b);
    float d = hi - lo;
    float h = 0;
    if (d > 1e-5)
    {
        if (hi == c.r)
            h = (c.g - c.b) / d;
        else if (hi == c.g)
            h = 2 + (c.b - c.r) / d;
        else
            h = 4 + (c.r - c.g) / d;
        h = frac(h / 6.0) * 360.0;
    }
    return float3(h, hi > 1e-5 ? d / hi : 0, hi);
}

float3 hsv_to_rgb(float3 hsv)
{
    float3 rgb = saturate(abs(frac(hsv.x / 360.0 + float3(1, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0);
    return hsv.z * lerp(float3(1, 1, 1), rgb, hsv.y);
}

float hue_delta(float from, float to)
{
    return fmod(to - from + 540.0, 360.0) - 180.0;
}

float vertical_weight(uint mode, float y)
{
    if (mode == 0)
        return 1 - smoothstep(0.35, 0.7, y);
    if (mode == 1)
        return smoothstep(0.3, 0.65, y);
    return 1;
}

float band_weight(float3 hsv, float hue, float width, float min_sat, float min_val, float vertical, float pull)
{
    float half_width = max(width * 0.5, 1);
    float by_hue = 1 - smoothstep(half_width * 0.5, half_width, abs(hue_delta(hue, hsv.x)));
    float by_sat = smoothstep(min_sat - 0.06, min_sat + 0.06, hsv.y) * smoothstep(0.01, 0.05, hsv.y);
    float by_val = smoothstep(min_val - 0.06, min_val + 0.06, hsv.z);
    return by_hue * by_sat * by_val * vertical * pull;
}

float3 pull_band(float3 hsv, float weight, float target_hue, float target_sat)
{
    hsv.x = hsv.x + hue_delta(hsv.x, target_hue) * weight;
    hsv.y = lerp(hsv.y, target_sat, weight);
    return hsv;
}

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    bool a_on = param_a_enabled() && param_a_pull() > 0;
    bool b_on = param_b_enabled() && param_b_pull() > 0;
    if (!a_on && !b_on)
        return c;

    float3 lin = max(to_linear(c.rgb), 0);
    float3 hsv = rgb_to_hsv(pow(lin, 1.0 / 2.2));
    float3 moved = hsv;
    float total = 0;
    if (a_on)
    {
        float w = band_weight(moved, param_a_hue(), param_a_width(), param_a_min_saturation(), param_a_min_brightness(),
                              vertical_weight(uint(param_a_vertical()), uv.y), param_a_pull());
        moved = pull_band(moved, w, param_a_target_hue(), param_a_target_saturation());
        total += w;
    }
    if (b_on)
    {
        float w = band_weight(moved, param_b_hue(), param_b_width(), param_b_min_saturation(), param_b_min_brightness(),
                              vertical_weight(uint(param_b_vertical()), uv.y), param_b_pull());
        moved = pull_band(moved, w, param_b_target_hue(), param_b_target_saturation());
        total += w;
    }
    if (total <= 0)
        return c;

    float3 shifted = pow(hsv_to_rgb(moved), 2.2);
    float gain = luminance(lin) / max(luminance(shifted), 1e-5);
    float peak = max(max(shifted.r, shifted.g), shifted.b);
    float limit = swofty_encoding == SWOFTY_POSTFX_SDR ? 1.0 / max(peak, 1e-5) : 8.0;
    shifted *= lerp(1, min(gain, max(limit, 1)), saturate(total * 4));
    float3 encoded = from_linear(shifted);
    if (swofty_encoding == SWOFTY_POSTFX_SDR)
        encoded = saturate(encoded);
    return float4(encoded, c.a);
}
