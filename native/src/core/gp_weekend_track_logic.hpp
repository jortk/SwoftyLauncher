#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace runtime::gp_weekend_track {

constexpr const char *STOCK = "melbourne";
constexpr std::size_t MAX_ITEMS = 0x100;
constexpr int LOG_CAP = 16;

struct Item {
    bool present = false;
    std::string name;
};

struct Decision {
    bool missing = false;
    bool repair = false;
    int to = -1;
};

// Where the weekend's track is read: the tyre allocation at the start of the weekend calls Ne_FatalAbort on a track
// with no record; the session settings pass the null record on and fault reading its name (+0x78).
enum class Site { tyre_allocation, session_settings };

inline const char *reader(Site site)
{
    return site == Site::tyre_allocation ? "the tyre allocation" : "the session settings";
}

inline bool present(int track, const std::vector<Item> &items)
{
    return track >= 0 && std::size_t(track) < items.size() && items[std::size_t(track)].present;
}

inline int stock_id(const std::vector<Item> &items, std::string_view stock = STOCK)
{
    int first = -1;
    for (std::size_t i = 0; i < items.size(); ++i) {
        if (!items[i].present)
            continue;
        if (items[i].name == stock)
            return int(i);
        if (first < 0)
            first = int(i);
    }
    return first;
}

inline Decision decide(int track, const std::vector<Item> &items)
{
    Decision d;
    if (present(track, items))
        return d;
    d.missing = true;
    d.to = stock_id(items);
    d.repair = d.to >= 0;
    return d;
}

inline std::string line(const Decision &d, int track, std::size_t items, const std::string &mode, Site site,
                        const std::string &label, const std::string &to_name)
{
    std::string head = "the " + (mode.empty() ? std::string("?") : mode) + " weekend races track stat id " +
                       std::to_string(track) + (label.empty() ? std::string() : " (" + label + ")") +
                       ", which has no track data record on this install (" + std::to_string(items) +
                       " record slot(s); its track addon is off or not installed)";
    if (d.repair)
        return head + "; its track is now " + (to_name.empty() ? std::string("?") : to_name) + " (stat id " +
               std::to_string(d.to) + ") so " + reader(site) +
               (site == Site::tyre_allocation ? " reads" : " read") + " a track the game has";
    return head + "; no track data record to put in its place, so the game " +
           (site == Site::tyre_allocation ? "aborts in its tyre allocation" : "faults in its session settings");
}

inline bool switch_off(std::string_view value)
{
    return value == "0" || value == "off" || value == "false" || value == "no";
}

}
