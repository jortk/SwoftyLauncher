#include "swofty/params.hlsli"

float project(float theta, float k)
{
    if (k > 1e-3)
        return tan(k * theta) / k;
    if (k < -1e-3)
        return sin(k * theta) / k;
    return theta;
}

float unproject(float r, float k, out bool valid)
{
    valid = true;
    if (k > 1e-3)
        return atan(k * r) / k;
    if (k < -1e-3)
    {
        float a = k * r;
        valid = abs(a) <= 1;
        return asin(clamp(a, -1, 1)) / k;
    }
    return r;
}

float half_width_tan(float aspect)
{
    float t = tan(radians(clamp(param_fov_angle(), 1, 175)) * 0.5);
    switch (param_fov_type())
    {
    case FOV_TYPE_DIAGONAL:
        return t / sqrt(1 + 1 / (aspect * aspect));
    case FOV_TYPE_VERTICAL:
        return t * aspect;
    case FOV_TYPE_HORIZONTAL_4_3:
        return t * aspect * 0.75;
    default:
        return t;
    }
}

float source_radius(float rho, float k, float edge, float tan_edge, out bool valid)
{
    float theta = unproject(rho * edge, k, valid);
    valid = valid && theta < 1.5;
    return valid ? tan(theta) / tan_edge : 1e4;
}

float2 source_point(float2 q, float k, float edge, float tan_edge, float ky, out bool valid)
{
    float rho = length(q);
    float2 dir = rho > 1e-6 ? q / rho : float2(0, 0);
    float2 s = dir * source_radius(rho, k, edge, tan_edge, valid);
    s.y = lerp(q.y, s.y, ky);
    return s;
}

float overshoot(float2 q, float k, float edge, float tan_edge, float ky, float aspect)
{
    bool valid;
    float2 s = source_point(q, k, edge, tan_edge, ky, valid);
    return valid ? max(abs(s.x), abs(s.y) * aspect) : 1e4;
}

float4 effect(float2 uv, uint2 pixel)
{
    float aspect = float(swofty_size.x) / float(swofty_size.y);
    float k = param_k();
    float tan_edge = half_width_tan(aspect);
    float edge = project(atan(tan_edge), k);
    float ky = param_ky();

    float zoom = 1;
    if (param_cropping_factor() > 0)
    {
        float fit = 1;
        [unroll] for (int i = 0; i < 5; i++)
        {
            float o = max(overshoot(float2(1, 1 / aspect) * fit, k, edge, tan_edge, ky, aspect),
                          overshoot(float2(0, 1 / aspect) * fit, k, edge, tan_edge, ky, aspect));
            fit /= clamp(o, 1, 4);
        }
        zoom = lerp(1, fit, param_cropping_factor());
    }

    float2 q = (uv - 0.5) * 2;
    q.y /= aspect;
    float vig = 1 - param_vignette_intensity() * smoothstep(0.5 + param_vignette_offset() * 0.5, 1.2, length(q * float2(1, aspect)) / 1.414);
    q *= zoom;

    bool valid;
    float2 s = source_point(q, k, edge, tan_edge, ky, valid);
    float2 suv = float2(s.x, s.y * aspect) * 0.5 + 0.5;
    bool inside = valid && all(suv >= 0) && all(suv <= 1);

    float2 corner = abs(uv - 0.5) * 2;
    float radius = param_border_corner();
    if (radius > 0)
    {
        float2 d = max(corner - (1 - radius), 0) / max(radius, 1e-4);
        inside = inside && dot(d, d) <= 1;
    }

    float4 border = param_border_color();
    float3 border_rgb = from_linear(srgb_to_linear(border.rgb));
    float3 rgb;
    if (inside)
        rgb = scene(suv).rgb;
    else if (param_mirror_border() && valid)
    {
        float2 m = 1 - abs(frac(suv * 0.5) * 2 - 1);
        rgb = lerp(scene(m).rgb, border_rgb, border.a);
    }
    else
        rgb = lerp(scene(uv).rgb, border_rgb, border.a);

    if (inside)
        rgb = from_linear(to_linear(rgb) * vig);
    return float4(rgb, 1);
}
