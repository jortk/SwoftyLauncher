#pragma once
#include <stddef.h>

#define SWOFTY_GATE_MIN_SIZE ((unsigned long long)64 << 20)

static int swofty_build_supported(unsigned long long size, unsigned long stamp)
{
    for (size_t i = 0; i < sizeof SWOFTY_SUPPORTED_BUILDS / sizeof SWOFTY_SUPPORTED_BUILDS[0]; i++)
        if (SWOFTY_SUPPORTED_BUILDS[i].size == size && SWOFTY_SUPPORTED_BUILDS[i].stamp == stamp)
            return 1;
    return 0;
}

static int swofty_bypass_value(const char *v)
{
    while (*v == ' ' || *v == '\t')
        v++;
    return *v && !(v[0] == '0' && (v[1] == 0 || v[1] == ' ' || v[1] == '\t'));
}

static int swofty_build_blocked(unsigned long long size, unsigned long stamp, const char *bypass)
{
    if (size < SWOFTY_GATE_MIN_SIZE || swofty_build_supported(size, stamp))
        return 0;
    return !(bypass && swofty_bypass_value(bypass));
}
