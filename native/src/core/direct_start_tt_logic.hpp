#pragma once

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// The direct start's Time Trial (direct_start_tt.cpp): the pure parts. A box names an era box's Grand Prix
// list or its own Time Trial list; a team is named by a word of its row label (mclaren finds mclaren_1990).

namespace runtime::direct_start::tt {

struct BoxLists {
    std::string entrant_list, time_trial;
};

inline std::string lower(std::string s)
{
    std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) { return char(std::tolower(c)); });
    return s;
}

inline std::string list_for(const std::vector<BoxLists> &boxes, const std::string &box)
{
    for (const auto &b : boxes)
        if (b.entrant_list == box || (!b.time_trial.empty() && b.time_trial == box))
            return b.time_trial.empty() ? b.entrant_list : b.time_trial;
    return box;
}

// The setup mode for a list: the wanted one when it offers the list or the request named it, else the first
// setup mode that offers the list; -1 when none does. offered[i] holds modes[i]'s lists.
inline int pick_setup_mode(const std::vector<std::string> &modes, const std::vector<std::vector<std::string>> &offered,
                           const std::string &want, bool named, const std::string &list)
{
    auto offers = [&](size_t i) {
        return i < offered.size() && std::find(offered[i].begin(), offered[i].end(), list) != offered[i].end();
    };
    for (size_t i = 0; i < modes.size(); ++i)
        if (modes[i] == want && (named || offers(i)))
            return int(i);
    if (named)
        return -1;
    for (size_t i = 0; i < modes.size(); ++i)
        if (offers(i))
            return int(i);
    return -1;
}

inline bool word_of(const std::string &label, const std::string &want)
{
    for (size_t at = label.find(want); at != std::string::npos; at = label.find(want, at + 1)) {
        bool start = at == 0 || label[at - 1] == '_';
        size_t end = at + want.size();
        bool stop = end == label.size() || label[end] == '_';
        if (start && stop)
            return true;
    }
    return false;
}

// The index of the team `want` names among `labels`: the label itself, else the first label that holds it as
// an underscore-separated word, else the first that holds it at all; -1 when none does. Case is ignored.
inline int pick_team(const std::vector<std::string> &labels, const std::string &want)
{
    std::string w = lower(want);
    if (w.empty())
        return -1;
    for (size_t i = 0; i < labels.size(); ++i)
        if (lower(labels[i]) == w)
            return int(i);
    for (size_t i = 0; i < labels.size(); ++i)
        if (word_of(lower(labels[i]), w))
            return int(i);
    for (size_t i = 0; i < labels.size(); ++i)
        if (lower(labels[i]).find(w) != std::string::npos)
            return int(i);
    return -1;
}

inline std::string joined(const std::vector<std::string> &names, size_t most = 24)
{
    std::string out;
    for (size_t i = 0; i < names.size() && i < most; ++i)
        out += (i ? ", " : "") + names[i];
    if (names.size() > most)
        out += ", ...";
    return out.empty() ? "none" : out;
}

// The Time Trial weather (the VM string settings.current_settings.tt_weather.value is "dry" or "wet") for the
// request's weather; null for an empty or unknown one.
inline const char *weather_value(const std::string &weather)
{
    std::string w = lower(weather);
    if (w == "clear" || w == "dry")
        return "dry";
    if (w == "wet" || w == "rain")
        return "wet";
    return nullptr;
}

// A packed team or vehicle reference: the kind byte low, the u16 index or stat id in the high half.
constexpr uint8_t ref_kind(uint32_t ref)
{
    return uint8_t(ref & 0xff);
}

constexpr uint16_t ref_index(uint32_t ref)
{
    return uint16_t(ref >> 16);
}

constexpr uint16_t NO_REF = 0x3fff;

// The setup's steps, one per flow tick: the mode made current, its components awaited, then everything applied.
enum class Step : uint8_t { mode, settle, apply, done };

constexpr uint64_t SETTLE_MS = 1500, MODE_WAIT_MS = 20000;

}
