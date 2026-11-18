#pragma once
#include <cmath>

namespace runtime::axes {

// Three basis vectors of a body, one per row of c, as read from memory.
inline bool unit_axes(const float m[3][3])
{
    for (int c = 0; c < 3; c++) {
        float len = m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2];
        if (!(len > 0.95f && len < 1.05f))
            return false;
    }
    for (int a = 0; a < 3; a++)
        for (int b = a + 1; b < 3; b++)
            if (std::fabs(m[a][0] * m[b][0] + m[a][1] * m[b][1] + m[a][2] * m[b][2]) > 0.05f)
                return false;
    return true;
}

// The matrix whose columns are the three basis vectors, as a unit quaternion (x, y, z, w).
inline void quaternion_of(const float c[3][3], float q[4])
{
    float m00 = c[0][0], m10 = c[0][1], m20 = c[0][2], m01 = c[1][0], m11 = c[1][1], m21 = c[1][2], m02 = c[2][0],
          m12 = c[2][1], m22 = c[2][2];
    float tr = m00 + m11 + m22;
    if (tr > 0) {
        float s = std::sqrt(tr + 1.0f) * 2;
        q[3] = 0.25f * s;
        q[0] = (m21 - m12) / s;
        q[1] = (m02 - m20) / s;
        q[2] = (m10 - m01) / s;
    } else if (m00 > m11 && m00 > m22) {
        float s = std::sqrt(1.0f + m00 - m11 - m22) * 2;
        q[3] = (m21 - m12) / s;
        q[0] = 0.25f * s;
        q[1] = (m01 + m10) / s;
        q[2] = (m02 + m20) / s;
    } else if (m11 > m22) {
        float s = std::sqrt(1.0f + m11 - m00 - m22) * 2;
        q[3] = (m02 - m20) / s;
        q[0] = (m01 + m10) / s;
        q[1] = 0.25f * s;
        q[2] = (m12 + m21) / s;
    } else {
        float s = std::sqrt(1.0f + m22 - m00 - m11) * 2;
        q[3] = (m10 - m01) / s;
        q[0] = (m02 + m20) / s;
        q[1] = (m12 + m21) / s;
        q[2] = 0.25f * s;
    }
}

}
