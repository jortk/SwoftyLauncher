// Host-native unit test for the core's shader resource view clamp (native/src/core/srv_clamp_logic.hpp).
// Built and run by native/test/run_srv_clamp_test.sh.

#include "srv_clamp_logic.hpp"

#include <cstdio>

namespace sc = runtime::srv_clamp;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

int main()
{
    sc::Resource tex{true, false, 24, 1};
    sc::View v{sc::kTexture2DArray, 0, 34, 0, 1};
    auto p = sc::plan(v, tex);
    CHECK(p.action == sc::Action::Clamp && p.slices == 24 && p.slices_changed && !p.mips_changed);

    v.slices = 24;
    CHECK(sc::plan(v, tex).action == sc::Action::Pass);
    v.slices = 1;
    v.first_slice = 23;
    CHECK(sc::plan(v, tex).action == sc::Action::Pass);
    v.slices = 5;
    p = sc::plan(v, tex);
    CHECK(p.action == sc::Action::Clamp && p.slices == 1);
    v.first_slice = 24;
    CHECK(sc::plan(v, tex).action == sc::Action::Skip);
    v.first_slice = 99;
    CHECK(sc::plan(v, tex).action == sc::Action::Skip);

    v = {sc::kTexture2DArray, 4, sc::kAll, 0, 1};
    CHECK(sc::plan(v, tex).action == sc::Action::Pass);
    v = {sc::kTexture2DArray, 0xfffffff0u, 0x20, 0, 1};
    CHECK(sc::plan(v, tex).action == sc::Action::Skip);
    v = {sc::kTexture2DArray, 20, 0xfffffff0u, 0, 1};
    CHECK(sc::plan(v, tex).slices == 4);

    v = {sc::kTexture1DArray, 0, 25, 0, 1};
    CHECK(sc::plan(v, tex).slices == 24);
    v = {sc::kTexture2DMSArray, 0, 25, 0, 1};
    CHECK(sc::plan(v, tex).slices == 24);
    v = {sc::kTextureCubeArray, 6, sc::cube_slices(4), 0, 1};
    CHECK(sc::plan(v, tex).slices == 18);

    sc::Resource mipped{true, false, 1, 5};
    v = {sc::kTexture2D, 0, 0, 0, 8};
    p = sc::plan(v, mipped);
    CHECK(p.action == sc::Action::Clamp && p.mip_levels == 5 && p.mips_changed && !p.slices_changed);
    v = {sc::kTexture2D, 0, 0, 2, sc::kAll};
    CHECK(sc::plan(v, mipped).action == sc::Action::Pass);
    v = {sc::kTexture2D, 0, 0, 2, 5};
    p = sc::plan(v, mipped);
    CHECK(p.mip_levels == 3 && p.most_detailed_mip == 2);
    v = {sc::kTexture2D, 0, 0, 9, 1};
    p = sc::plan(v, mipped);
    CHECK(p.action == sc::Action::Clamp && p.most_detailed_mip == 4 && p.mip_levels == 1);
    v = {sc::kTexture2DMSArray, 0, 2, 7, 3};
    CHECK(sc::plan(v, {true, false, 4, 1}).action == sc::Action::Pass);

    v = {sc::kBuffer, 0, 0, 0, 1};
    CHECK(sc::plan(v, {false, false, 1, 1}).action == sc::Action::Pass);
    v = {sc::kTexture2DArray, 0, 34, 0, 1};
    CHECK(sc::plan(v, {false, false, 1, 1}).action == sc::Action::Pass);
    CHECK(sc::plan(v, {true, true, 8, 1}).action == sc::Action::Pass);
    CHECK(sc::plan(v, {true, false, 0, 0}).action == sc::Action::Pass);
    v = {sc::kTexture3D, 0, 0, 0, 1};
    CHECK(sc::plan(v, {true, false, 4, 1}).action == sc::Action::Pass);

    CHECK(!sc::enabled(nullptr));
    CHECK(!sc::enabled(""));
    CHECK(sc::enabled("1"));
    CHECK(!sc::enabled("0"));
    CHECK(!sc::enabled("11"));

    if (g_fail)
        return 1;
    std::printf("srv clamp: ok\n");
    return 0;
}
