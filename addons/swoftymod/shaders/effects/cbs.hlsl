#include "swofty/params.hlsli"

float curve(float x, float contrast)
{
    float smooth_x = x * x * (3 - 2 * x);
    float flat_x = 0.5 - sin(asin(clamp(1 - 2 * x, -1, 1)) / 3);
    return contrast >= 0 ? lerp(x, smooth_x, contrast) : lerp(x, flat_x, -contrast * 0.45);
}

float3 rolloff(float3 x, float knee)
{
    float3 over = max(x - knee, 0);
    return min(x, knee) + (1 - knee) * (1 - exp(-over / (1 - knee)));
}

float3 rgb_to_hsv(float3 c)
{
    float mx = max(c.r, max(c.g, c.b)), mn = min(c.r, min(c.g, c.b));
    float d = mx - mn;
    float h = 0;
    if (d > 1e-5)
    {
        if (mx == c.r)
            h = frac((c.g - c.b) / d / 6 + 1);
        else if (mx == c.g)
            h = (c.b - c.r) / d / 6 + 1.0 / 3;
        else
            h = (c.r - c.g) / d / 6 + 2.0 / 3;
    }
    return float3(h, mx > 1e-5 ? d / mx : 0, mx);
}

float hue_weight(float h, float centre, float width)
{
    float d = abs(h - centre);
    d = min(d, 1 - d);
    float t = saturate(1 - d / width);
    return t * t * (3 - 2 * t);
}

float hue_saturation(float h)
{
    float s = 0;
    s += param_sat_r() * hue_weight(h, 0.0, 1.0 / 6);
    s += param_sat_y() * hue_weight(h, 1.0 / 6, 1.0 / 6);
    s += param_sat_g() * hue_weight(h, 1.0 / 3, 1.0 / 6);
    s += param_sat_a() * hue_weight(h, 0.5, 1.0 / 6);
    s += param_sat_b() * hue_weight(h, 2.0 / 3, 1.0 / 6);
    s += param_sat_p() * hue_weight(h, 0.77, 1.0 / 8);
    s += param_sat_m() * hue_weight(h, 0.875, 1.0 / 8);
    s += param_sat_custom() * hue_weight(h, param_huemid(), max(param_huerange() * 0.5, 0.005));
    return s;
}

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float far = 0;
    if (param_enable_depth() && has_depth())
    {
        float span = max(param_depth_end() - param_depth_start(), 1e-4);
        far = pow(saturate((param_depth_end() - scene_depth(uv)) / span), param_depth_curve());
    }
    float exposure = lerp(param_exposure(), param_exposure_d(), far);
    float contrast = lerp(param_contrast(), param_contrast_d(), far);
    float brightness = lerp(param_brightness(), param_brightness_d(), far);
    float saturation = lerp(param_saturation(), param_saturation_d(), far);
    float vibrance = lerp(param_vibrance(), param_vibrance_d(), far);

    float3 rgb = max(to_linear(c.rgb), 0) * exp2(exposure);
    rgb = lerp(rgb, rolloff(rgb, 0.3), saturate(exposure * 2));
    float peak = max(1.0, max(rgb.r, max(rgb.g, rgb.b)));
    float3 p = pow(rgb / peak, 1.0 / 2.2);

    float y = dot(p, float3(0.2126, 0.7152, 0.0722));
    float shaped = curve(saturate(y), contrast);
    shaped = shaped + brightness * 0.2 * (1 - shaped);
    shaped = max(shaped, 0);
    p *= min(shaped / max(y, 1e-4), 1.3);
    p += shaped - dot(p, float3(0.2126, 0.7152, 0.0722));

    float3 hsv = rgb_to_hsv(saturate(p));
    float s = saturation + hue_saturation(hsv.x) * 1.1 * smoothstep(0.02, 0.2, hsv.y);
    float dull = 1 - hsv.y;
    s += vibrance * 0.3 * dull * dull;
    float luma = dot(p, float3(0.2126, 0.7152, 0.0722));
    p = luma + (p - luma) * max(1 + s, 0);
    p.r *= 1 + 0.1 * param_tint();
    p.b *= 1 + 0.1 * param_tint();
    p.g *= 1 - 0.1 * param_tint();

    if (param_enable_dither() && swofty_encoding == SWOFTY_POSTFX_SDR)
    {
        float n = random(pixel, 7u) + random(pixel, 11u) - 1.0;
        p += n * param_dither_strength() / 255.0;
    }
    rgb = pow(max(p, 0), 2.2) * peak;
    return float4(from_linear(rgb), c.a);
}
