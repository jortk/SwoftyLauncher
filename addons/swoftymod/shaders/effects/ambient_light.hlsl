#include "swofty/params.hlsli"

// Bloom is a bright pass into a half-size texture, three 13-tap downsamples to 1/16 and tent upsamples back.
// Eye adaptation meters the scene's log luminance down to a 1x1 texture and eases a persistent gain towards it.

float3 decoded(float2 uv)
{
    return to_linear(scene(uv).rgb);
}

float3 bright_part(float3 c)
{
    float peak = max(c.r, max(c.g, c.b));
    float edge = lerp(0.5, 1.0, param_al_threshold());
    float knee = 0.2;
    float soft = clamp(peak - edge + knee, 0, 2 * knee);
    soft = soft * soft / (4 * knee);
    float weight = max(soft, peak - edge);
    return c * max(weight, 0) / max(peak, 1e-4);
}

float4 bright(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(swofty_scene_size);
    float3 v = bright_part(decoded(uv)) * 0.25;
    v += bright_part(decoded(uv + t * float2(-1, 1))) * 0.1875;
    v += bright_part(decoded(uv + t * float2(1, 1))) * 0.1875;
    v += bright_part(decoded(uv + t * float2(-1, -1))) * 0.1875;
    v += bright_part(decoded(uv + t * float2(1, -1))) * 0.1875;
    return float4(min(v, 64), 1);
}

float4 down(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(input_size(0));
    float3 a = input(0, uv + t * float2(-2, 2)).rgb, b = input(0, uv + t * float2(0, 2)).rgb;
    float3 c = input(0, uv + t * float2(2, 2)).rgb, d = input(0, uv + t * float2(-2, 0)).rgb;
    float3 e = input(0, uv).rgb, f = input(0, uv + t * float2(2, 0)).rgb;
    float3 g = input(0, uv + t * float2(-2, -2)).rgb, h = input(0, uv + t * float2(0, -2)).rgb;
    float3 i = input(0, uv + t * float2(2, -2)).rgb;
    float3 j = input(0, uv + t * float2(-1, 1)).rgb, k = input(0, uv + t * float2(1, 1)).rgb;
    float3 l = input(0, uv + t * float2(-1, -1)).rgb, m = input(0, uv + t * float2(1, -1)).rgb;
    float3 v = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
    return float4(v, 1);
}

float4 up(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(input_size(0));
    float3 v = input(0, uv).rgb * 4;
    v += (input(0, uv + t * float2(-1, 0)).rgb + input(0, uv + t * float2(1, 0)).rgb +
          input(0, uv + t * float2(0, -1)).rgb + input(0, uv + t * float2(0, 1)).rgb) * 2;
    v += input(0, uv + t * float2(-1, -1)).rgb + input(0, uv + t * float2(1, -1)).rgb +
         input(0, uv + t * float2(-1, 1)).rgb + input(0, uv + t * float2(1, 1)).rgb;
    return float4(v / 16 + input(1, uv).rgb * 0.5, 1);
}

float3 flare_sample(float2 uv, float thresh)
{
    float3 c = input(0, uv).rgb;
    float peak = max(c.r, max(c.g, c.b));
    return c * saturate((peak - thresh) / max(peak, 1e-4));
}

float4 flare(float2 uv, uint2 pixel)
{
    float2 dir = 0.5 - uv;
    float thresh = lerp(0.1, 0.8, param_al_lens_thresh());
    float3 sum = 0;
    [unroll] for (int i = 0; i < 4; i++)
    {
        float2 o = uv + dir * (0.35 * (i + 1));
        float w = pow(saturate(1 - length(o - 0.5) / 0.7071), 3) / (1 + i);
        float3 s;
        s.r = flare_sample(o + dir * 0.004, thresh).r;
        s.g = flare_sample(o, thresh).g;
        s.b = flare_sample(o - dir * 0.004, thresh).b;
        sum += s * w;
    }
    float2 ring = uv + normalize(dir + 1e-5) * 0.4;
    float halo_w = pow(saturate(1 - length(ring - 0.5) / 0.5), 3);
    float3 halo;
    halo.r = flare_sample(ring + dir * 0.006, thresh).r;
    halo.g = flare_sample(ring, thresh).g;
    halo.b = flare_sample(ring - dir * 0.006, thresh).b;
    sum += halo * halo_w * 0.25;
    return float4(sum, 1);
}

float4 meter(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(swofty_scene_size);
    float sum = 0;
    [unroll] for (int y = 0; y < 4; y++)
        [unroll] for (int x = 0; x < 4; x++)
        {
            float3 c = decoded(uv + t * (float2(x, y) - 1.5) * 4);
            sum += log2(max(luminance(c), 1e-4));
        }
    return float4(sum / 16, 0, 0, 1);
}

float4 meter2(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(input_size(0));
    float sum = 0;
    [unroll] for (int y = 0; y < 8; y++)
        [unroll] for (int x = 0; x < 8; x++)
            sum += input(0, uv + t * (float2(x, y) - 3.5)).r;
    return float4(sum / 64, 0, 0, 1);
}

float4 adapt_calc(float2 uv, uint2 pixel)
{
    uint2 size = input_size(0);
    float sum = 0;
    for (uint y = 0; y < size.y; y++)
        for (uint x = 0; x < size.x; x++)
            sum += input_at(0, int2(x, y)).r;
    float average = exp2(sum / float(size.x * size.y));
    float strength = param_al_adapt() / 8;
    float target = clamp(strength * 0.2 * log2(0.18 / max(average, 1e-4)), -0.5, 0.5);
    float4 state = input_at(1, int2(0, 0));
    float dt = clamp(swofty_frametime, 0, 0.1);
    float speed = target < state.x ? 3.0 : 1.2;
    float eased = state.y > 0.5 ? state.x + (target - state.x) * (1 - exp(-dt * speed)) : target;
    return float4(eased, 1, 0, 1);
}

float4 adapt_store(float2 uv, uint2 pixel)
{
    return input(0, float2(0.5, 0.5));
}

float hash21(float2 p)
{
    p = frac(p * float2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return frac(p.x * p.y);
}

float value_noise(float2 p)
{
    float2 i = floor(p), f = frac(p);
    f = f * f * (3 - 2 * f);
    return lerp(lerp(hash21(i), hash21(i + float2(1, 0)), f.x), lerp(hash21(i + float2(0, 1)), hash21(i + float2(1, 1)), f.x), f.y);
}

float dirt(float2 uv)
{
    float aspect = float(swofty_size.x) / float(swofty_size.y);
    float2 p = uv * float2(aspect, 1);
    if (param_al_dirt_tex() == AL_DIRT_TEX_SPECKS)
    {
        float2 g = p * 26;
        float2 cell = floor(g);
        float r = hash21(cell);
        float2 centre = 0.5 + (float2(hash21(cell + 3.1), hash21(cell + 7.7)) - 0.5) * 0.6;
        float spot = smoothstep(0.32 * r + 0.05, 0.0, length(frac(g) - centre));
        return r > 0.55 ? spot : 0;
    }
    float n = value_noise(p * 4) * 0.55 + value_noise(p * 9 + 5.2) * 0.3 + value_noise(p * 21 + 1.7) * 0.15;
    float m = smoothstep(0.45, 0.8, n);
    return m * m;
}

float4 apply(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    float4 state = input_at(2, int2(0, 0));
    float gain = (param_al_adaptation() && state.y > 0.5) ? exp2(state.x) : 1.0;
    float black = param_al_adapt_base_black_lvl() == 0 ? 0.0 : (param_al_adapt_base_black_lvl() == 1 ? 0.02 : 0.06);
    float applied = gain * (1 + param_al_adapt_base_mult());
    float fade = black > 0 ? smoothstep(0, black, luminance(rgb)) : 1.0;
    rgb *= lerp(1.0, applied, fade);

    float3 glow = input(0, uv).rgb;
    float mask = param_al_dirt() ? dirt(uv) * param_al_dirt_ov_int() : 0;
    float3 add = glow * (param_al_int() * 0.2 + param_al_dirt_int() * 0.2 * mask);
    if (param_al_lens())
        add += input(1, uv).rgb * (param_al_lens_int() * 0.01) * (1 + mask);
    if (param_al_adaptive())
        add *= clamp(gain, 0.5, 2.0);
    if (param_al_vibrance())
    {
        float grey = luminance(add);
        add = max(grey + (add - grey) * 1.5, 0);
    }
    rgb += add / (1 + 0.5 * max(rgb, 0));

    float3 out_c = from_linear(max(rgb, 0));
    if (param_al_dither() && swofty_encoding == SWOFTY_POSTFX_SDR)
        out_c += (random(pixel, 5u) + random(pixel, 9u) - 1.0) / 255.0;
    return float4(out_c, c.a);
}
