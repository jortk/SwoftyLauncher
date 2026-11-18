#include "swofty/params.hlsli"

// The glare: a bright-pass into a half-size texture, four 13-tap downsamples to 1/32, then tent
// upsamples back, each adding the level above (Jimenez, "Next Generation Post Processing in Call of
// Duty: Advanced Warfare"), and a six-point star drawn from the quarter-size level.

float3 decoded(float2 uv)
{
    return to_linear(scene(uv).rgb);
}

float3 bright(float3 c, float threshold)
{
    float peak = max(c.r, max(c.g, c.b));
    float knee = threshold * 0.5;
    float soft = clamp(peak - threshold + knee, 0, 2 * knee);
    soft = soft * soft / (4 * knee + 1e-4);
    return c * max(soft, peak - threshold) / max(peak, 1e-4);
}

float karis(float3 c)
{
    return 1 / (1 + luminance(c));
}

float4 prefilter(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(swofty_scene_size);
    float threshold = param_threshold() / 100;
    float3 a = decoded(uv + t * float2(-2, 2)), b = decoded(uv + t * float2(0, 2)), c = decoded(uv + t * float2(2, 2));
    float3 d = decoded(uv + t * float2(-2, 0)), e = decoded(uv), f = decoded(uv + t * float2(2, 0));
    float3 g = decoded(uv + t * float2(-2, -2)), h = decoded(uv + t * float2(0, -2)), i = decoded(uv + t * float2(2, -2));
    float3 j = decoded(uv + t * float2(-1, 1)), k = decoded(uv + t * float2(1, 1));
    float3 l = decoded(uv + t * float2(-1, -1)), m = decoded(uv + t * float2(1, -1));
    float3 groups[5] = {(j + k + l + m) * 0.25, (a + b + d + e) * 0.25, (b + c + e + f) * 0.25,
                        (d + e + g + h) * 0.25, (e + f + h + i) * 0.25};
    float weights[5] = {0.5, 0.125, 0.125, 0.125, 0.125};
    float3 sum = 0;
    float total = 0;
    [unroll] for (int n = 0; n < 5; n++)
    {
        float3 v = bright(groups[n], threshold);
        float w = weights[n] * karis(v);
        sum += v * w;
        total += w;
    }
    return float4(sum / max(total, 1e-4), 1);
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
    return float4(v / 16 + input(1, uv).rgb, 1);
}

float4 star(float2 uv, uint2 pixel)
{
    if (param_star() <= 0)
        return 0;
    float2 t = 1.0 / float2(input_size(0));
    float aspect = float(swofty_scene_size.x) / float(swofty_scene_size.y);
    const float tilt = radians(3);
    float3 sum = 0;
    [unroll] for (int s = 0; s < 6; s++)
    {
        float a = tilt + s * radians(60);
        float2 dir = float2(cos(a), sin(a) * aspect) * t;
        [unroll] for (int i = 1; i <= 16; i++)
            sum += input(0, uv + dir * i * 1.5).rgb * exp(-i * 0.22);
    }
    return float4(sum / 6, 1);
}

float4 apply(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    rgb += input(0, uv).rgb * (param_bloom() / 100 * 0.35) + input(1, uv).rgb * (param_star() / 100 * 0.6);
    return float4(from_linear(max(rgb, 0)), c.a);
}
