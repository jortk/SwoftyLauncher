#include <cassert>
#include <cstdio>

#include <swofty/fake_hud.hpp>
#include <swofty/fake_live.hpp>

#include "../src/dial.hpp"

static const char *GEAR = "view_model_data.hud_dial_gear";
static const char *PIT = "view_model_data.hud_dial_pit";

int main()
{
    swofty::fake::Hud core;
    swofty::fake::Clock clock;
    swofty::fake::Live fake;
    dial::Dial dial;

    fake.session().type = 15;
    auto &car = fake.add_car(0, 1);
    car.gear = 6;
    car.have |= SWOFTY_LIVE_HAVE_GEAR;
    fake.set_order({0});
    fake.set_player(0);
    fake.install();

    dial.frame(0.016);
    assert(core.text(GEAR) == "6");
    assert(core.number(PIT) == 0);

    core.clear();
    dial.frame(0.016);
    assert(core.writes().empty());

    fake.car(0).gear = -1;
    fake.car(0).have |= SWOFTY_LIVE_HAVE_PIT;
    fake.car(0).pit_state = 3;
    dial.frame(0.016);
    assert(core.text(GEAR) == "R");
    assert(core.number(PIT) == 1);

    fake.car(0).gear = 0;
    dial.frame(0.016);
    assert(core.text(GEAR) == "N");

    fake.set_available(false);
    dial.frame(0.016);
    assert(core.text(GEAR).empty());
    assert(core.number(PIT) == 0);

    fake.set_available(true);
    fake.session().type = 18;
    core.rebuild();
    dial.frame(0.016);
    assert(core.writes().size() >= 2);

    swofty::fake::Hud old{swofty::fake::Hud::v1};
    dial::Dial old_dial;
    fake.car(0).gear = 3;
    old_dial.frame(0.016);
    assert(old.text(GEAR) == "3");

    std::puts("hud-dial test: ok");
}
