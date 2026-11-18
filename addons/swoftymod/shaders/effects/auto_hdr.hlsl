#include "swofty/params.hlsli"

float shoulder(float x, float start, float peak, float power)
{
    if (x <= start)
        return x;
    float room = max(peak - start, 1e-3);
    float s = (x - start) / room;
    return start + room * s / pow(1 + pow(s, power), 1 / power);
}

float inverse_tonemap(float d, float white)
{
    if (param_inverse_tonemap_method() == INVERSE_TONEMAP_METHOD_POWER)
        return d * (1 + (white - 1) * d * d * d);
    float w2 = white * white;
    float a = 1 - d;
    return (sqrt(a * a + 4 * d / w2) - a) * w2 * 0.5;
}

float forward_tonemap(float l, float white)
{
    if (param_inverse_tonemap_method() == INVERSE_TONEMAP_METHOD_POWER)
        return l / (1 + (white - 1) * saturate(l / white));
    return l * (1 + l / (white * white)) / (1 + l);
}

bool is_hdr()
{
    return swofty_encoding != SWOFTY_POSTFX_SDR;
}

float source_peak_nits()
{
    return is_hdr() ? max(param_hdr_source_peak_white(), 1.0) : max(param_output_white_level_nits(), 1.0);
}

float nits_per_unit()
{
    return is_hdr() ? 80.0 : source_peak_nits();
}

float expansion_input(float y)
{
    return saturate(max(y, 0) * nits_per_unit() / source_peak_nits());
}

float expanded_nits(float y)
{
    float peak = source_peak_nits();
    float nits = max(y, 0) * nits_per_unit();
    float d = saturate(nits / peak);
    float deep = 1 - smoothstep(0, 0.6, d);
    d = lerp(d, pow(d, param_shadow_tuning()), deep);
    float l = inverse_tonemap(d, param_tonemapper_white_level());
    if (param_auto_hdr_method() == AUTO_HDR_METHOD_HIGHLIGHTS)
        l = lerp(d, l, smoothstep(0.4, 1.0, d));
    float out_nits = l * peak + max(nits - peak, 0);
    float max_nits = param_auto_hdr_max_nits();
    return shoulder(out_nits, param_auto_hdr_shoulder_start_alpha() * max_nits, max_nits, param_auto_hdr_shoulder_pow());
}

float sdr_remap(float y)
{
    float mid = 0.18;
    float depth = max(param_shadow_tuning() - 1, 0);
    float below = pow(saturate(y / mid), min(depth, 2));
    float shadow_gain = lerp(1, 0.35 + 0.65 * below, saturate(depth));
    float x = y * shadow_gain;
    float start = param_auto_hdr_shoulder_start_alpha();
    float top = 1 + (1 - start) * 0.25;
    float power = max(param_auto_hdr_shoulder_pow(), 1);
    float rolled = shoulder(x, start, top, power);
    float reference = shoulder(1.0, start, top, power);
    return rolled / lerp(1, reference, smoothstep(start, 1, x));
}

float remap(float y)
{
    if (y <= param_black_floor_luminance())
        return y;
    if (!is_hdr())
        return sdr_remap(y);
    float nits = expanded_nits(y);
    float display_peak = param_hdr_peak_white();
    if (param_hdr_tonemap())
        nits = shoulder(nits, param_hdr_highlights_shoulder_start_alpha() * display_peak, display_peak,
                        param_auto_hdr_shoulder_pow());
    return nits / 80.0;
}

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = max(to_linear(c.rgb), 0);
    float y = luminance(rgb);
    float3 per_channel = float3(remap(rgb.r), remap(rgb.g), remap(rgb.b));
    float3 by_luminance = rgb * (remap(y) / max(y, param_black_floor_luminance()));
    float3 o = lerp(per_channel, by_luminance, saturate(param_inverse_tonemap_color_conservation()));

    float grey = luminance(o);
    float highlight = is_hdr() ? smoothstep(0.5, 1.0, expansion_input(y)) : smoothstep(0.3, 0.9, y);
    float sat = lerp(1, param_highlight_saturation(), highlight);
    if (is_hdr())
        sat *= param_extra_hdr_saturation();
    o = max(grey + (o - grey) * sat, 0);
    if (!is_hdr())
        o = saturate(o);
    return float4(from_linear(o), c.a);
}
