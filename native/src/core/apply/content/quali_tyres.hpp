#pragma once

#include "lower.hpp"

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// rules.qualifying_tyres (content.package 1.5): dry compound options a box's cars may use only in qualifying, the
// qualifying tyres of the late 1970s to 1991. The game data cannot say "qualifying only": a per-session preset
// (tyres.allocation_tyre_presets.TyresPerSessionPreset_N, Practice1..Race3, one count per class option) hands sets out
// per session, every set is tagged with the session that handed it out (set record +0x00), and the stock usable check
// in mode 0 (Car_Tyre_CheckCompoundClassRule 0x142e88730) lets a race use a qualifying-tagged set. So the box's
// package gives the qualifying option sets only in Qualifying1..3 (data), and the core refuses those options in race
// sessions (15..17, g_Sess_TyreRulesSessionType): a set record whose option (+0x04) is in the mask is not usable, and
// the garage / AI set choice (Sess_Tyres_ClosestAvailableCompound) skips the option in its preference list.
//
// Option order is the class's: 0 softer, 1 medium, 2 harder (3 intermediate, 4 wet are never qualifying only).
// Pure, for the host tests.

namespace swofty_core::apply::content {

inline constexpr const char *QUALI_OPTION_NAMES[3] = {"softer", "medium", "harder"};
inline constexpr uint32_t QUALI_RACE_FIRST = 15, QUALI_RACE_LAST = 17;   // session ids Race, Race2, Race3
inline constexpr uint32_t QUALI_NO_SLOT = 0xffffffffu;                    // a skipped preference entry

struct QualiTyresPick {
    std::string package;
    uint8_t mask = 0;          // bit n = class option n is qualifying only; 0 = no rule
    std::string line;
};

// The qualifying tyre rule for the box whose entrant list was pressed: the last content.package (in the order given)
// with that entrant list, and its rules.qualifying_tyres when that is an array of option names. Unknown names are
// left out (the schema refuses them); an empty or missing array, or null, is no rule.
inline QualiTyresPick quali_tyres_pick(const std::string &entrant_list,
                                       const std::vector<std::pair<std::string, const Value *>> &packages)
{
    QualiTyresPick p;
    const Value *found = nullptr;
    for (auto &[id, rec] : packages)
        if (rec && (*rec)["entrant_list"].is_string() && (*rec)["entrant_list"].as_string() == entrant_list) {
            p.package = id;
            found = rec;
        }
    if (!found) {
        p.line = "box " + entrant_list + ": no content.package; every dry compound may race";
        return p;
    }
    const Value &q = (*found)["rules"]["qualifying_tyres"];
    std::string names;
    if (q.is_array())
        for (size_t i = 0; i < q.size(); ++i) {
            if (!q[i].is_string())
                continue;
            for (uint8_t o = 0; o < 3; ++o)
                if (q[i].as_string() == QUALI_OPTION_NAMES[o] && !(p.mask & (1u << o))) {
                    p.mask |= uint8_t(1u << o);
                    names += (names.empty() ? "" : ", ") + std::string(QUALI_OPTION_NAMES[o]);
                }
        }
    if (!p.mask) {
        p.line = "box " + entrant_list + " (" + p.package + "): no qualifying tyre rule; every dry compound may race";
        return p;
    }
    p.line = "box " + entrant_list + " (" + p.package + "): " + names +
             " only in qualifying, refused in races (rules.qualifying_tyres)";
    return p;
}

inline bool quali_race_session(uint32_t session)
{
    return session >= QUALI_RACE_FIRST && session <= QUALI_RACE_LAST;
}

// A set of class option `option` may not be used in `session`: a race, and the option is qualifying only.
inline bool quali_refused(uint32_t session, uint32_t option, uint8_t mask)
{
    return mask && option < 3 && (mask >> option & 1u) && quali_race_session(session);
}

// The preference list Sess_Tyres_ClosestAvailableCompound tries (five option indices): in a race, each qualifying
// only option becomes QUALI_NO_SLOT, which the game skips like an index past the allocation's count (a negative
// index fails its range test). Returns how many entries were skipped; 0 leaves `out` equal to `in`.
inline int quali_filter_preference(uint32_t session, uint8_t mask, const uint32_t in[5], uint32_t out[5])
{
    int skipped = 0;
    for (int i = 0; i < 5; ++i) {
        out[i] = in[i];
        if (quali_refused(session, in[i], mask)) {
            out[i] = QUALI_NO_SLOT;
            ++skipped;
        }
    }
    return skipped;
}

}
