// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <string_view>

#include <swofty/mixin.hpp>

// One struct for an addon's options. Declares the SWOFTY_SETTINGs, fills the struct on read() with choices
// mapped to enums and numbers clamped, and counts changes so a frame loop reads the options only when one
// moved instead of polling on a timer. Option ids are the manifest's options[].id.
//
//   enum class Mode { full, compact, top };
//   #define TOWER_OPTIONS(B, I, F, C)
//       B(header, true)                                          toggle
//       I(rows, 5, 3, 22)                                        integer slider, clamped to 3..22
//       F(opacity, 1.0f, 0.0f, 1.0f)                             float slider
//       C(mode, Mode, Mode::compact, "full", "compact", "top")   choice
//   (each entry on its own line, a backslash at the end of every line but the last)
//   SWOFTY_OPTIONS(TowerOptions, TOWER_OPTIONS)
//
//   uint32_t seen = 0;
//   TowerOptions opts = TowerOptions::read();
//   void on_frame() { if (TowerOptions::changed(seen)) opts = TowerOptions::read(); }
//
// A choice's strings are the manifest's choice values; the enum value is the string's position, so declare the
// enumerators in the same order starting at 0. An unknown string keeps the default.

#define SWOFTY_OPT_FIELD_B(key, def) bool key = def;
#define SWOFTY_OPT_FIELD_I(key, def, lo, hi) int key = def;
#define SWOFTY_OPT_FIELD_F(key, def, lo, hi) float key = def;
#define SWOFTY_OPT_FIELD_C(key, E, def, ...) E key = def;

#define SWOFTY_OPT_DECL_B(key, def)                                                                                    \
    static inline const ::swofty::setting<bool> opt_##key{#key, def};                                                   \
    static inline const ::swofty::detail::watch watch_##key{&opt_##key.id, [](const SwoftySettingValue &) { changes()++; }};
#define SWOFTY_OPT_DECL_I(key, def, lo, hi)                                                                            \
    static inline const ::swofty::setting<int> opt_##key{#key, def};                                                    \
    static inline const ::swofty::detail::watch watch_##key{&opt_##key.id, [](const SwoftySettingValue &) { changes()++; }};
#define SWOFTY_OPT_DECL_F(key, def, lo, hi)                                                                            \
    static inline const ::swofty::setting<float> opt_##key{#key, def};                                                  \
    static inline const ::swofty::detail::watch watch_##key{&opt_##key.id, [](const SwoftySettingValue &) { changes()++; }};
#define SWOFTY_OPT_DECL_C(key, E, def, ...)                                                                            \
    static constexpr std::string_view names_##key[] = {__VA_ARGS__};                                                   \
    static inline const ::swofty::setting<std::string_view> opt_##key{#key, names_##key[size_t(def)]};                   \
    static inline const ::swofty::detail::watch watch_##key{&opt_##key.id, [](const SwoftySettingValue &) { changes()++; }}; \
    static E pick_##key(std::string_view v)                                                                            \
    {                                                                                                                 \
        for (size_t i = 0; i < sizeof names_##key / sizeof names_##key[0]; i++)                                         \
            if (names_##key[i] == v)                                                                                   \
                return static_cast<E>(i);                                                                             \
        return def;                                                                                                   \
    }

#define SWOFTY_OPT_READ_B(key, def) o.key = opt_##key.get();
#define SWOFTY_OPT_READ_I(key, def, lo, hi) o.key = std::clamp<int>(opt_##key.get(), lo, hi);
#define SWOFTY_OPT_READ_F(key, def, lo, hi) o.key = std::clamp<float>(opt_##key.get(), lo, hi);
#define SWOFTY_OPT_READ_C(key, E, def, ...) o.key = pick_##key(opt_##key.get());

#define SWOFTY_OPTIONS(Name, LIST)                                                                                    \
    struct Name {                                                                                                     \
        LIST(SWOFTY_OPT_FIELD_B, SWOFTY_OPT_FIELD_I, SWOFTY_OPT_FIELD_F, SWOFTY_OPT_FIELD_C)                          \
        static std::atomic<uint32_t> &changes()                                                                       \
        {                                                                                                             \
            static std::atomic<uint32_t> n{0};                                                                        \
            return n;                                                                                                 \
        }                                                                                                             \
        LIST(SWOFTY_OPT_DECL_B, SWOFTY_OPT_DECL_I, SWOFTY_OPT_DECL_F, SWOFTY_OPT_DECL_C)                              \
        static Name read()                                                                                            \
        {                                                                                                             \
            Name o;                                                                                                   \
            LIST(SWOFTY_OPT_READ_B, SWOFTY_OPT_READ_I, SWOFTY_OPT_READ_F, SWOFTY_OPT_READ_C)                          \
            return o;                                                                                                 \
        }                                                                                                             \
        static bool changed(uint32_t &seen)                                                                           \
        {                                                                                                             \
            uint32_t now = changes().load();                                                                          \
            bool moved = now != seen;                                                                                 \
            seen = now;                                                                                               \
            return moved;                                                                                             \
        }                                                                                                             \
    }
