// Host-native unit test for the core's heap size table (native/src/core/heap_sizes_logic.hpp): which stock heap each
// factory site recognises, the size it picks from the machine and the heap's env var, the name read from the plain
// factory's frame and the log lines. Built and run by native/test/run_heap_sizes_test.sh.

#include "heap_sizes_logic.hpp"

#include <cstdio>
#include <cstring>

namespace hs = runtime::heap_sizes;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

constexpr uint64_t GiB = 1024ull * hs::kMiB;

static const hs::Spec &spec(const char *name)
{
    for (const hs::Spec &s : hs::kHeaps)
        if (!std::strcmp(s.name, name))
            return s;
    std::printf("no spec %s\n", name);
    std::exit(2);
}

int main()
{
    const hs::Spec &audio = spec("AUDIO_BUNDLE"), &save = spec("SAVE_TEMP_ALLOC"), &game = spec("GAME_DEFAULT");
    constexpr uint64_t M = hs::kMiB;

    CHECK(audio.stock == 45088768);
    CHECK(save.stock == 78643200);
    CHECK(game.stock == 408944640);
    CHECK(!std::strcmp(audio.env, "SWOFTY_AUDIO_BUNDLE_MB"));
    CHECK(!std::strcmp(save.env, "SWOFTY_SAVE_TEMP_MB"));
    CHECK(!std::strcmp(game.env, "SWOFTY_GAME_HEAP_MB"));
    for (size_t a = 0; a < hs::kHeapCount; a++)
        for (size_t b = a + 1; b < hs::kHeapCount; b++)
            CHECK(std::strcmp(hs::kHeaps[a].name, hs::kHeaps[b].name) && std::strcmp(hs::kHeaps[a].env, hs::kHeaps[b].env));

    using F = hs::Factory;
    CHECK(hs::find(F::plain, 43 * M, {0, 0, 2 * M}) == &audio);
    CHECK(!hs::find(F::plain, 43 * M, {1, 0, 2 * M}));
    CHECK(!hs::find(F::plain, 43 * M, {0, 0, 0}));
    CHECK(!hs::find(F::internal, 43 * M, {0, 0, 2 * M}));
    CHECK(!hs::find(F::plain, 96 * M, {0, 0, 2 * M}));
    CHECK(hs::find(F::internal, 75 * M, {1, 0, 0}) == &save);
    CHECK(!hs::find(F::internal, 75 * M, {0, 0, 0}));
    CHECK(!hs::find(F::internal, 75 * M, {1, 100, 0}));
    CHECK(!hs::find(F::internal, 200 * M, {1, 0, 0}));
    CHECK(!hs::find(F::internal, 3 * M, {1, 0, 0}));
    CHECK(!hs::find(F::plain, 75 * M, {1, 0, 0}));
    CHECK(hs::find(F::plain, 390 * M, {1, 100, 0}) == &game);
    CHECK(hs::find(F::plain, 390 * M, {1, 100, 64 * 1024}) == &game);
    CHECK(!hs::find(F::plain, 390 * M, {0, 100, 0}));
    CHECK(!hs::find(F::internal, 390 * M, {1, 100, 0}));
    CHECK(!hs::find(F::plain, 195 * M, {1, 0, 0}) && !hs::find(F::plain, 2600 * M, {1, 100, 0}));
    CHECK(!hs::find(F::plain, 1024 * M, {1, 100, 0}));

    {
        hs::Choice c = hs::choose(audio, nullptr, 32 * GiB);
        CHECK(c.raised(audio) && c.source == hs::Source::ram && c.bytes == 96 * M && !c.bad_env);
        CHECK(hs::choose(audio, "", 16 * GiB).bytes == 96 * M);
        CHECK(hs::choose(audio, nullptr, 12 * GiB).bytes == 96 * M);
        c = hs::choose(audio, nullptr, 8 * GiB);
        CHECK(c.raised(audio) && c.source == hs::Source::small_ram && c.bytes == 64 * M);
        CHECK(hs::choose(audio, nullptr, 0).bytes == 96 * M);
        c = hs::choose(audio, "128", 8 * GiB);
        CHECK(c.raised(audio) && c.source == hs::Source::env && c.bytes == 128 * M);
        CHECK(hs::choose(audio, "101", 32 * GiB).bytes == 102 * M);
        CHECK(hs::choose(audio, "100000", 32 * GiB).bytes == 512 * M);
        CHECK(hs::choose(audio, "44", 32 * GiB).raised(audio));
        for (const char *off : {"43", "20", "0", "off"}) {
            c = hs::choose(audio, off, 32 * GiB);
            CHECK(!c.raised(audio) && c.bytes == audio.stock);
        }
        c = hs::choose(audio, "lots", 32 * GiB);
        CHECK(c.bad_env && c.raised(audio) && c.bytes == 96 * M);
        c = hs::choose(audio, "96MB", 8 * GiB);
        CHECK(c.bad_env && c.bytes == 64 * M);
        for (const char *mb : {"64", "96", "101", "128", "512"}) {
            c = hs::choose(audio, mb, 32 * GiB);
            CHECK(c.bytes % audio.page == 0 && c.bytes >= std::strtoull(mb, nullptr, 10) * M);
        }
    }
    {
        hs::Choice c = hs::choose(save, nullptr, 32 * GiB);
        CHECK(c.raised(save) && c.bytes == 512 * M && c.source == hs::Source::ram);
        c = hs::choose(save, nullptr, 8 * GiB);
        CHECK(c.bytes == 320 * M && c.source == hs::Source::small_ram);
        CHECK(hs::choose(save, "", 32 * GiB).bytes == 512 * M);
        for (const char *off : {"0", "off", "75", "64"}) {
            c = hs::choose(save, off, 32 * GiB);
            CHECK(!c.raised(save) && c.bytes == save.stock);
        }
        c = hs::choose(save, "300", 32 * GiB);
        CHECK(c.raised(save) && c.bytes == 300 * M && c.source == hs::Source::env);
        CHECK(hs::choose(save, "9999", 32 * GiB).bytes == 1024 * M);
        c = hs::choose(save, "lots", 32 * GiB);
        CHECK(c.bad_env && c.raised(save) && c.bytes == 512 * M);
        CHECK(hs::choose(save, "12x", 32 * GiB).bad_env);
    }
    {
        hs::Choice c = hs::choose(game, nullptr, 32 * GiB);
        CHECK(c.raised(game) && c.bytes == 1024 * M && c.source == hs::Source::ram);
        c = hs::choose(game, nullptr, 8 * GiB);
        CHECK(c.raised(game) && c.bytes == 640 * M && c.source == hs::Source::small_ram);
        CHECK(hs::choose(game, nullptr, 0).bytes == 1024 * M);
        c = hs::choose(game, "1536", 8 * GiB);
        CHECK(c.bytes == 1536 * M && c.source == hs::Source::env);
        CHECK(hs::choose(game, "99999", 32 * GiB).bytes == 2600 * M);
        for (const char *off : {"0", "off", "390", "100"}) {
            c = hs::choose(game, off, 32 * GiB);
            CHECK(!c.raised(game) && c.bytes == game.stock);
        }
        CHECK(hs::choose(game, "391", 32 * GiB).bytes == 391 * M);
        CHECK(hs::choose(game, "big", 32 * GiB).bad_env);
        CHECK(hs::source_text(game, hs::Source::env) == "SWOFTY_GAME_HEAP_MB");
    }
    {
        hs::Choice c = hs::choose(audio, nullptr, 32 * GiB);
        CHECK(hs::apply_line(audio, {0, 0, 2 * M}, "AUDIO_BUNDLE", audio.stock, c) ==
              "[audio] heap 'AUDIO_BUNDLE' (virtual 0, pagesize 2097152): size 45088768 -> 100663296 bytes (96 MiB, default)");
        c = hs::choose(save, nullptr, 32 * GiB);
        CHECK(hs::apply_line(save, {1, 0, 0}, "", save.stock, c) ==
              "[saves] temp pool: heap SAVE_TEMP_ALLOC (virtual 1, expand 0%): size 78643200 -> 536870912 bytes (512 MiB, default)");
        c = hs::choose(game, "600", 32 * GiB);
        CHECK(hs::apply_line(game, {1, 100, 0}, "GAME_DEFAULT", game.stock, c) ==
              "[heap] heap 'GAME_DEFAULT' (virtual 1, pagesize 0): size 408944640 -> 629145600 bytes (600 MiB, SWOFTY_GAME_HEAP_MB)");
        CHECK(hs::kept_line(audio, nullptr) == "[audio] AUDIO_BUNDLE heap kept at the game's 43 MiB (SWOFTY_AUDIO_BUNDLE_MB=)");
        CHECK(hs::kept_line(save, "off") == "[saves] temp pool: SAVE_TEMP_ALLOC kept at the game's 75 MiB (SWOFTY_SAVE_TEMP_MB=off)");
        CHECK(hs::armed_line(game, hs::choose(game, nullptr, 32 * GiB)) ==
              "[heap] GAME_DEFAULT heap armed: 1024 MiB instead of the game's 390 MiB when system/memory_map.xml is read");
        CHECK(hs::bad_env_line(save) == "[saves] temp pool: SWOFTY_SAVE_TEMP_MB is not a number of MiB, 0 or off; using the default");
        CHECK(hs::late_line(audio, "window").find("the core loaded at the window stage") != std::string::npos);
        CHECK(hs::unresolved_line(game, "site_X") == "[heap] GAME_DEFAULT heap not raised: site_X does not resolve on this build");
    }
    {
        uint8_t frame[0x100];
        std::memset(frame, 0xcc, sizeof frame);
        const char *name = "AUDIO_BUNDLE";
        for (size_t k = 0; k < hs::kNameLen; k++)
            frame[hs::kNameSlots[k]] = static_cast<uint8_t>(name[k]);
        frame[hs::kNameSlots[hs::kNameLen]] = 0;
        char out[hs::kNameLen + 1];
        hs::frame_name(frame, out);
        CHECK(!std::strcmp(out, "AUDIO_BUNDLE"));
        std::memset(frame, 0, sizeof frame);
        hs::frame_name(frame, out);
        CHECK(!std::strcmp(out, "????????????"));
        bool distinct = true;
        for (size_t a = 0; a < hs::kNameLen + 1; a++)
            for (size_t b = a + 1; b < hs::kNameLen + 1; b++)
                distinct = distinct && hs::kNameSlots[a] != hs::kNameSlots[b];
        CHECK(distinct);
    }
    if (g_fail) {
        std::printf("%d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("heap sizes test: ok\n");
    return 0;
}
