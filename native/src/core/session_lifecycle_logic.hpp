#pragma once
#include <cstdint>
#include <string_view>

namespace runtime::session_life {

// The live session's kind: set when StateLoadSession enters and again at Game_GameModule_OnSessionEnter, NONE once the
// session ends or unloads. Modules that only work in one kind of session read it as one atomic load.
enum Kind : int { NONE = 0, TIME_TRIAL = 1, OTHER = 2 };

inline int kind_of(std::string_view mode)
{
    return mode == "time_trial" ? TIME_TRIAL : OTHER;
}

inline bool time_trial_live(int kind)
{
    return kind == TIME_TRIAL;
}

inline bool session_live(int kind)
{
    return kind != NONE;
}

constexpr int64_t END_EXITED = 0;
constexpr int64_t END_RESTART = 1;
constexpr int64_t END_UNLOAD = 2;

struct Emit {
    bool end = false;
    bool unload = false;
    int64_t reason = END_EXITED;
};

class Lifecycle {
public:
    bool on_start()
    {
        bool restart = open_;
        open_ = true;
        seen_ = true;
        return restart;
    }

    Emit on_session_exiting()
    {
        Emit e;
        if (open_) {
            open_ = false;
            e.end = true;
        }
        return e;
    }

    Emit on_unload_entering()
    {
        Emit e = on_session_exiting();
        if (e.end)
            e.reason = END_UNLOAD;
        if (seen_) {
            seen_ = false;
            e.unload = true;
        }
        return e;
    }

    bool open() const { return open_; }

private:
    bool open_ = false;
    bool seen_ = false;
};

}
