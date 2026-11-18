#pragma once
#include <cstdint>

namespace view {

enum class PauseEdge { none, paused, resumed };

constexpr PauseEdge pause_edge(uint32_t before, uint32_t after)
{
    if (before == 0 && after != 0)
        return PauseEdge::paused;
    if (before != 0 && after == 0)
        return PauseEdge::resumed;
    return PauseEdge::none;
}

}
