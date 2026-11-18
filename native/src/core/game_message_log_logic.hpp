#pragma once

#include <cstddef>
#include <cstring>
#include <string>
#include <vector>

// Which stock game messages the core logs (native/src/core/popup.cpp, the Ui_Message_Construct wrap, which is
// Game_UiMessage_DisplayMessage 0x1442d9bf0 in re/db/functions.tsv). Every message the game raises by its UIMD id is
// logged once per id per session, so a player's loader.log names the popup they saw
// ("[popup] game message 'continue_solo_gp_f1w_offline_warning'"). Messages the core raises itself (swofty_message,
// swofty_toast_*, the beta_motd fallback) are not stock messages and are left out.

namespace runtime::game_message_log {

constexpr size_t MAX_IDS = 96;
constexpr size_t MAX_ID_CHARS = 96;
constexpr const char *OWN_PREFIX = "swofty_";

inline bool printable(const char *id)
{
    if (!id || !id[0])
        return false;
    for (size_t i = 0; id[i]; i++) {
        if (i >= MAX_ID_CHARS)
            return false;
        unsigned char c = static_cast<unsigned char>(id[i]);
        if (c < 0x20 || c > 0x7e)
            return false;
    }
    return true;
}

inline bool stock(const char *id, bool raised_by_core)
{
    return !raised_by_core && printable(id) && std::strncmp(id, OWN_PREFIX, std::strlen(OWN_PREFIX)) != 0;
}

struct Seen {
    std::vector<std::string> ids;

    bool first(const char *id)
    {
        for (const std::string &s : ids)
            if (s == id)
                return false;
        if (ids.size() >= MAX_IDS)
            return false;
        ids.emplace_back(id);
        return true;
    }

    bool last() const
    {
        return ids.size() == MAX_IDS;
    }
};

}
