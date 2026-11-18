#include <swofty/events.hpp>
#include <swofty/mixin.hpp>

#include "dial.hpp"

namespace {

dial::Dial the_dial;

void on_frame(double dt, void *)
{
    the_dial.frame(dt);
}

}

SWOFTY_ON_LOAD
{
    swofty::events::on_frame(on_frame);
}

SWOFTY_ADDON()
