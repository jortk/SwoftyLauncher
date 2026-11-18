#pragma once
#include <optional>
#include <swofty/hud_format.hpp>
#include <swofty/hud_frame.hpp>
#include <swofty/hud_leaves.hpp>
#include <swofty/live.hpp>
#include <swofty/mixin.hpp>

namespace dial {

namespace hud = swofty::hud;
namespace live = swofty::live;

struct Leaves {
    hud::Leaves L{"view_model_data.hud_dial_"};
    hud::TextLeaf gear = L.text("gear", 2);
    hud::BoolLeaf pit = L.boolean("pit");
};

inline const char *gear_text(int gear, char out[2])
{
    if (gear < 0)
        return "R";
    if (gear == 0)
        return "N";
    out[0] = static_cast<char>('0' + (gear > 9 ? 9 : gear));
    out[1] = 0;
    return out;
}

class Dial {
public:
    void frame(double dt)
    {
        std::optional<live::Car> me = live::available() ? live::subject() : std::nullopt;
        std::optional<live::Session> session = live::session();

        hud::Gate gate;
        gate.live = me.has_value() && session.has_value();
        if (session) {
            gate.session_serial = session->generation;
            gate.subject_serial = session->subject_generation;
            gate.time_trial = hud::format::is_time_trial(session->type);
        }

        if (!frame_.begin(gate, dt)) {
            blank();
            return;
        }

        char digit[2];
        if (live::has(*me, SWOFTY_LIVE_HAVE_GEAR))
            leaves_.L.set(leaves_.gear, gear_text(me->gear, digit));
        leaves_.L.set(leaves_.pit, live::in_pit(*me));
        leaves_.L.flush();

        if (frame_.end()) {
            auto cost = frame_.cost();
            swofty::log("[hud-dial] frame %.0f us mean, %.0f us max over %d frames", cost.mean_us, cost.max_us, cost.frames);
        }
    }

private:
    void blank()
    {
        leaves_.L.set(leaves_.gear, "");
        leaves_.L.set(leaves_.pit, false);
        leaves_.L.flush();
    }

    Leaves leaves_;
    hud::Frame frame_;
};

}
