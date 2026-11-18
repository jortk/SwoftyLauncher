#pragma once

#include <cstdint>
#include <cstdio>
#include <string>

namespace runtime::leaderboard_guard {

constexpr int LOG_FIRST = 4;

enum class Say { none, line, last };

inline bool board_missing(uint64_t board)
{
    return board == 0;
}

inline bool tab_child_missing(uint64_t child)
{
    return child == 0;
}

inline bool reference_board_missing(uint64_t board)
{
    return board == 0;
}

constexpr uintptr_t LIST_BOARD_SET = 0xf0;

inline bool reference_list_unboarded(uint8_t engaged)
{
    return engaged == 0;
}

inline std::string reference_list_line(uint64_t presenter, const std::string &mode, const std::string &setup_mode,
                                       const std::string &list, int64_t cars)
{
    auto or_unread = [](const std::string &s) { return s.empty() ? std::string("?") : s; };
    char at[32];
    std::snprintf(at, sizeof at, "0x%llx", (unsigned long long)presenter);
    return std::string("Time Trial presenter ") + at + ": the selected entrant list " + or_unread(list) + " (" +
           (cars < 0 ? std::string("?") : std::to_string(cars)) + " cars) of setup mode " + or_unread(setup_mode) +
           " in game mode " + or_unread(mode) +
           " has no leaderboard; the reference is left unset instead of the game's bad optional access";
}

inline Say say(int occurrence)
{
    if (occurrence < 1 || occurrence > LOG_FIRST)
        return Say::none;
    return occurrence == LOG_FIRST ? Say::last : Say::line;
}

inline bool switch_off(const char *value)
{
    if (!value)
        return false;
    return value[0] == '0' && value[1] == 0 ? true : value[0] == 'o' && value[1] == 'f' && value[2] == 'f' && value[3] == 0;
}

}
