#include "../../../../../native/src/core/apply/content/bc7.hpp"

#include <fstream>
#include <vector>

namespace bc7 = swofty_core::apply::content::bc7;

static uint64_t state = 0x9e3779b97f4a7c15ull;
static uint32_t next()
{
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    return uint32_t(state >> 32);
}

static void write(const std::string &name, const std::vector<uint8_t> &rgba, int w, int h)
{
    std::ofstream in(name + ".rgba", std::ios::binary);
    in << w << " " << h << "\n";
    in.write(reinterpret_cast<const char *>(rgba.data()), std::streamsize(rgba.size()));
    std::ofstream a(name + ".bc7", std::ios::binary);
    std::string s = bc7::encode_rgba(rgba.data(), w, h);
    a.write(s.data(), std::streamsize(s.size()));
    std::ofstream b(name + ".logo.bc7", std::ios::binary);
    s = bc7::encode_logo(rgba.data(), w, h);
    b.write(s.data(), std::streamsize(s.size()));
}

int main()
{
    const int w = 32, h = 32;
    std::vector<uint8_t> v(w * h * 4);
    for (auto &x : v)
        x = uint8_t(next());
    for (int i = 3; i < w * h * 4; i += 4)
        v[i] = 255;
    write("opaque_noise", v, w, h);

    for (auto &x : v)
        x = uint8_t(next());
    write("alpha_noise", v, w, h);

    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++) {
            uint8_t *p = &v[(y * w + x) * 4];
            bool inside = (x - 16) * (x - 16) + (y - 16) * (y - 16) < 100;
            p[0] = uint8_t(next());
            p[1] = uint8_t(next());
            p[2] = uint8_t(next());
            p[3] = inside ? uint8_t(160 + next() % 96) : 0;
        }
    write("transparent_random_rgb", v, w, h);

    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++) {
            uint8_t *p = &v[(y * w + x) * 4];
            bool left = x < 16;
            p[0] = left ? 240 : 20;
            p[1] = left ? 240 : 20;
            p[2] = left ? 240 : 20;
            p[3] = uint8_t((x * 8 + y * 3) & 0xff);
            if (y >= 24)
                p[0] = p[1] = p[2] = 90, p[3] = left ? 255 : 0;
        }
    write("soft_edges", v, w, h);

    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++) {
            uint8_t *p = &v[(y * w + x) * 4];
            p[0] = uint8_t(x * 8);
            p[1] = uint8_t(y * 8);
            p[2] = uint8_t((x + y) * 4);
            p[3] = 255;
        }
    write("gradient", v, w, h);
    return 0;
}
