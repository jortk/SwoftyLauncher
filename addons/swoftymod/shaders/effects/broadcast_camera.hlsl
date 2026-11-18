#include "swofty/params.hlsli"

static const float SIGNAL_GAMMA = 2.2;

float3 to_signal(float3 linear_rgb)
{
    return pow(max(linear_rgb, 0), 1.0 / SIGNAL_GAMMA);
}

float3 from_signal(float3 signal)
{
    return pow(max(signal, 0), SIGNAL_GAMMA);
}

float4 meter(float2 uv, uint2 pixel)
{
    float2 cell = 1.0 / float2(64, 32);
    float mean = 0, mean2 = 0, high = 0;
    [unroll] for (int y = 0; y < 4; y++)
        [unroll] for (int x = 0; x < 4; x++)
        {
            float2 at = uv + cell * ((float2(x, y) + 0.5) / 4 - 0.5);
            float3 s = to_signal(to_linear(scene(at).rgb));
            float p = min(dot(s, float3(0.2126, 0.7152, 0.0722)), 1.5);
            mean += p;
            mean2 += p * p;
            high += smoothstep(0.78, 0.95, p);
        }
    return float4(mean, mean2, high, 16) / 16;
}

float4 ease(float2 uv, uint2 pixel)
{
    float4 m = input_lod(0, float2(0.5, 0.5), 6);
    float mean = m.x;
    float spread = sqrt(max(m.y - m.x * m.x, 0));
    float3 target = float3(mean, spread, m.z);
    float4 state = input_at(1, int2(0, 0));
    float dt = clamp(swofty_frametime, 0, 0.1);
    float k = 1 - exp(-dt / max(param_auto_speed(), 0.05));
    return state.w > 0.5 ? float4(state.xyz + (target - state.xyz) * k, 1) : float4(target, 1);
}

float4 store(float2 uv, uint2 pixel)
{
    return input_at(0, int2(0, 0));
}

float auto_push(float3 metered)
{
    float bright = smoothstep(0.3, 0.6, metered.x);
    float contrasty = smoothstep(0.1, 0.26, metered.y);
    float flat_scene = 1 - smoothstep(0.07, 0.2, metered.y);
    float harsh = saturate(bright * contrasty + smoothstep(0.02, 0.14, metered.z) * contrasty);
    return saturate(harsh) - flat_scene;
}

float3 knee(float3 signal, float knee_at, float slope, float clip_signal, float saturation)
{
    float m = max(max(signal.r, signal.g), signal.b);
    if (m <= knee_at || clip_signal <= knee_at || slope >= 1)
        return signal;
    float room = clip_signal - knee_at;
    float squeezed = knee_at + room * (1 - exp(-(m - knee_at) * slope / room));
    float ratio = squeezed / max(m, 1e-5);
    float3 scaled = signal * ratio;
    float weight = saturate((1 - ratio) * 2);
    float grey = dot(scaled, float3(0.2126, 0.7152, 0.0722));
    return max(grey + (scaled - grey) * lerp(1, saturation, weight), 0);
}

float3 black_level(float3 p, float pedestal, float stretch)
{
    float3 t = saturate(p / 0.3);
    float3 shaped = pow(t, 1 + 0.8 * stretch) * 0.3;
    float3 w = 1 - smoothstep(0.15, 0.3, p);
    p = lerp(p, shaped, w);
    return pedestal + p * (1 - pedestal);
}

float3 meter_view(float3 rgb, float2 uv, float4 state, float push)
{
    float2 origin = float2(0.03, 0.86);
    float2 size = float2(0.2, 0.018);
    float values[4] = {state.x, saturate(state.y * 3), saturate(state.z * 4), push * 0.5 + 0.5};
    float3 colours[4] = {float3(0.9, 0.9, 0.9), float3(0.4, 0.8, 1), float3(1, 0.85, 0.3), float3(1, 0.4, 0.4)};
    float2 panel_lo = origin - 0.008;
    float2 panel_hi = origin + float2(size.x, size.y * 7 + 0.006) + 0.008;
    if (all(uv >= panel_lo) && all(uv <= panel_hi))
        rgb = rgb * 0.35;
    [unroll] for (int i = 0; i < 4; i++)
    {
        float2 lo = origin + float2(0, size.y * 2 * i);
        if (uv.y >= lo.y && uv.y <= lo.y + size.y && uv.x >= lo.x && uv.x <= lo.x + size.x)
            rgb = uv.x - lo.x <= size.x * saturate(values[i]) ? to_linear(colours[i] * 0.9) : rgb * 0.6;
    }
    return rgb;
}

float4 apply(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float4 state = input_at(0, int2(0, 0));
    float auto_amount = state.w > 0.5 ? param_auto_strength() : 0;
    float push = state.w > 0.5 ? auto_push(state.xyz) : 0;

    float3 rgb = max(to_linear(c.rgb), 0) * exp2(param_exposure());

    float3 balance = float3(1 + 0.18 * param_temperature(), 1 - 0.1 * param_tint(), 1 - 0.18 * param_temperature());
    rgb *= balance / max(luminance(balance), 1e-4);

    float3 p = to_signal(rgb);
    float pedestal = max(param_pedestal() - auto_amount * 0.03 * push, 0);
    float stretch = clamp(param_black_stretch() + auto_amount * 0.5 * push, -1, 1);
    p = black_level(p, pedestal, stretch);
    p = pow(max(p, 0), 1.0 / max(param_gamma(), 0.1));
    p = max(0.5 + (p - 0.5) * param_contrast(), 0);

    float knee_at = clamp(param_knee_point() - auto_amount * 0.2 * push, 0.3, 1.6);
    float clip_signal = pow(max(param_white_clip(), 0.01), 1.0 / SIGNAL_GAMMA);
    p = knee(p, knee_at, param_knee_slope(), clip_signal, param_knee_saturation());

    float grey = dot(p, float3(0.2126, 0.7152, 0.0722));
    p = max(grey + (p - grey) * param_saturation(), 0);

    float3 outc = from_signal(p);
    if (param_view() == VIEW_METER)
        outc = meter_view(outc, uv, state, push);
    float3 encoded = from_linear(outc);
    if (swofty_encoding == SWOFTY_POSTFX_SDR)
        encoded = saturate(encoded);
    return float4(encoded, c.a);
}
