// Host unit test for the pause edge detection (pause_edge.hpp) that decides when the core fires
// game.pause and game.resume from the game's pause reason mask.
#include "../src/core/pause_edge.hpp"

#include <cstdio>
#include <string>

using view::PauseEdge;
using view::pause_edge;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

struct Tracker {
    uint32_t mask = 0;
    int pauses = 0, resumes = 0;

    void reason(int bit, bool set)
    {
        uint32_t before = mask;
        mask = set ? (before | 1u << bit) : (before & ~(1u << bit));
        switch (pause_edge(before, mask)) {
        case PauseEdge::paused: pauses++; break;
        case PauseEdge::resumed: resumes++; break;
        case PauseEdge::none: break;
        }
    }
};

int main()
{
    expect(pause_edge(0, 1) == PauseEdge::paused, "0 to a mask is a pause");
    expect(pause_edge(0, 0x80000000u) == PauseEdge::paused, "bit 31 alone is a pause");
    expect(pause_edge(1, 0) == PauseEdge::resumed, "a mask to 0 is a resume");
    expect(pause_edge(0, 0) == PauseEdge::none, "0 to 0 is no edge");
    expect(pause_edge(1, 3) == PauseEdge::none, "another bit joining is no edge");
    expect(pause_edge(3, 1) == PauseEdge::none, "a bit leaving while another holds is no edge");
    expect(pause_edge(5, 5) == PauseEdge::none, "an unchanged mask is no edge");

    Tracker t;
    t.reason(0, true);
    expect(t.pauses == 1 && t.resumes == 0, "first reason fires pause once");
    t.reason(3, true);
    t.reason(3, true);
    expect(t.pauses == 1 && t.resumes == 0, "further reasons do not refire pause");
    t.reason(0, false);
    expect(t.resumes == 0 && t.mask == 8, "one reason clearing while another holds does not resume");
    t.reason(3, false);
    expect(t.pauses == 1 && t.resumes == 1 && t.mask == 0, "last reason clearing fires resume once");
    t.reason(3, false);
    expect(t.resumes == 1, "clearing an unset reason fires nothing");
    t.reason(2, true);
    expect(t.pauses == 2, "a second pause fires again");

    printf("%d failure(s)\n", fails);
    return fails != 0;
}
