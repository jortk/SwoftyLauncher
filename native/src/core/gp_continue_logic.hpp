#pragma once

#include <cstdint>
#include <string>

namespace runtime::gp_continue {

constexpr const char *F1_WORLD = "f1_world";
constexpr int LOG_CAP = 32;
constexpr uint64_t PRESS_MS = 3000;

enum class Check { car_offline, offline_warning };
enum class Verdict { no_match, keep, keep_unknown, skip };

inline Verdict decide(uint64_t car_category, uint64_t setup_category, uint64_t f1_world_category)
{
    if (!setup_category || car_category != setup_category)
        return Verdict::no_match;
    if (!f1_world_category)
        return Verdict::keep_unknown;
    return car_category == f1_world_category ? Verdict::keep : Verdict::skip;
}

inline const char *check_name(Check check)
{
    return check == Check::car_offline ? "F1 World car check" : "F1 World offline warning";
}

inline std::string line(Check check, Verdict verdict, const std::string &setup_mode)
{
    const std::string mode = setup_mode.empty() ? "?" : setup_mode;
    const std::string what = check_name(check);
    switch (verdict) {
    case Verdict::skip:
        return "continue: " + what + " skipped (setup mode " + mode + " is not the F1 World car)";
    case Verdict::keep:
        return "continue: " + what + " kept (setup mode " + mode + " is the F1 World car), so the game asks to go online";
    case Verdict::keep_unknown:
        return "continue: " + what + " left to the game (setup mode " + mode + "; no f1_world setup mode to compare with)";
    case Verdict::no_match:
        break;
    }
    return "continue: no " + what + " (setup mode " + mode + " is not on the F1 World car's category)";
}

struct Press {
    uint64_t frame = 0;
    uint64_t at = 0;
    Verdict verdict = Verdict::no_match;
    std::string setup_mode;
    bool seen = false;
};

inline bool same_press(const Press &last, uint64_t frame, uint64_t now, Verdict verdict, const std::string &setup_mode)
{
    return last.seen && last.frame == frame && now >= last.at && now - last.at < PRESS_MS && last.verdict == verdict &&
           last.setup_mode == setup_mode;
}

inline bool switch_off(const char *value)
{
    if (!value)
        return false;
    std::string v(value);
    return v == "0" || v == "off" || v == "false";
}

}
