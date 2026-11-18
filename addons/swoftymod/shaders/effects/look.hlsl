#include "swofty/params.hlsli"

// The look on the picture: a white balance,
// a brightness lift that keeps white, contrast around mid grey and saturation, in linear light.

float3 kelvin_rgb(float kelvin)
{
    float t = kelvin / 100;
    float r = t <= 66 ? 255 : 329.698727446 * pow(t - 60, -0.1332047592);
    float g = t <= 66 ? 99.4708025861 * log(t) - 161.1195681661 : 288.1221695283 * pow(t - 60, -0.0755148492);
    float b = t >= 66 ? 255 : (t <= 19 ? 0 : 138.5177312231 * log(t - 10) - 305.0447927307);
    return srgb_to_linear(saturate(float3(r, g, b) / 255));
}

float3 white_balance(float3 rgb, float kelvin)
{
    float3 gain = kelvin_rgb(kelvin) / kelvin_rgb(6500);
    gain /= luminance(gain);
    return rgb * gain;
}

float3 lift(float3 rgb, float brightness)
{
    float3 c = min(rgb, 1);
    return rgb + c * brightness / (1 + (brightness - 1) * c) - c;
}

float3 contrast(float3 rgb, float amount)
{
    float3 mid = 0.18;
    return mid * pow(max(rgb, 0) / mid, amount);
}

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    float3 base = rgb;
    rgb = white_balance(rgb, param_white_balance());
    rgb = lift(rgb, param_brightness() / 100);
    rgb = contrast(rgb, param_contrast() / 100);
    rgb = lerp(luminance(rgb).xxx, rgb, param_saturation() / 100);
    rgb = lerp(base, max(rgb, 0), param_strength() / 100);
    return float4(from_linear(rgb), c.a);
}
