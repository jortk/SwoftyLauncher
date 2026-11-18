#pragma once
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstring>

#include "swofty_api.h"

// Career and flow-state tracking without the game: the flow-state site, the Team HQ answer site
// and the component poll feed facts in, and settle() reports which career events to publish. The
// clock is a millisecond value supplied by the caller, so the host test drives it directly.

namespace runtime::career {

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    for (; *s; ++s)
        h = h * 33 + static_cast<uint8_t>(*s);
    return h;
}

enum Slot : int {
    TeamHq,
    TeamHqUpdate,
    TeamHqTimeAdvance,
    TeamHqToSession,
    TeamHqEndOfSeason,
    TeamHqSeasonBreak,
    Session,
    LoadSession,
    UnloadSession,
    CareerHubUpdate,
    SlotCount
};

struct Known {
    const char *name;
    uint32_t hash;
};

inline constexpr Known KNOWN[SlotCount] = {
    {"StateTeamHQ", djb2("StateTeamHQ")},
    {"StateTeamHQUpdate", djb2("StateTeamHQUpdate")},
    {"StateTeamHQTimeAdvance", djb2("StateTeamHQTimeAdvance")},
    {"StateTeamHQToSession", djb2("StateTeamHQToSession")},
    {"StateTeamHQEndOfSeason", djb2("StateTeamHQEndOfSeason")},
    {"StateTeamHQSeasonBreak", djb2("StateTeamHQSeasonBreak")},
    {"StateSession", djb2("StateSession")},
    {"StateLoadSession", djb2("StateLoadSession")},
    {"StateUnloadSession", djb2("StateUnloadSession")},
    {"SessionStateCareerHubUpdate", djb2("SessionStateCareerHubUpdate")},
};

static_assert(djb2("StateTeamHQUpdate") == 0x0e245469u, "state name hashing drifted");
static_assert(djb2("StateTeamHQToSession") == 0xc5dd1dcdu, "state name hashing drifted");

constexpr uint64_t COMPONENT_GRACE_MS = 4000;
constexpr size_t MAX_STATES = 256;
constexpr uint64_t POLL_MS = 250;
constexpr uint64_t SETTLE_MS = 8000;
constexpr uint64_t IDLE_POLL_MS = 30000;

static_assert(SETTLE_MS > COMPONENT_GRACE_MS + POLL_MS, "the poll must outlast the component grace");

// When the component poll runs. A career loads, unloads or changes around flow-state transitions,
// so after one the poll runs every POLL_MS for SETTLE_MS; otherwise every IDLE_POLL_MS, and never in
// a session, where none of that happens. Each poll costs a few guarded reads of the game's heap,
// which on Windows can take milliseconds, so polling all the time stuttered every frame it landed on.
class PollSchedule {
public:
    void transition(uint64_t now) { until_.store(now + SETTLE_MS, std::memory_order_relaxed); }

    bool due(uint64_t now, uint64_t last, bool session) const
    {
        if (now < until_.load(std::memory_order_relaxed))
            return now - last >= POLL_MS;
        return !session && now - last >= IDLE_POLL_MS;
    }

private:
    std::atomic<uint64_t> until_{0};
};

// Where the game is, for the component poll: in a career, elsewhere (Time Trial, Grand Prix, story, F1 World: no career
// can be loaded there) or unknown (the main menu, a mode not listed, no read yet). The poll reads the game everywhere but
// elsewhere; there it reports no career without a read, which is what a read finds, so a career that was loaded still
// unloads.
enum class Where { unknown, career, elsewhere };

inline bool poll_reads(Where w)
{
    return w != Where::elsewhere;
}

struct Components {
    bool entity = false;
    bool career = false;
    uint32_t mode = SWOFTY_CAREER_NONE;
    uint32_t humans = 0;
    bool player = false;
    uint32_t driver = 0, team = 0;
};

struct Edges {
    bool unloaded = false;
    bool loaded = false;
    int hub = -1;
    uint32_t old_mode = SWOFTY_CAREER_NONE;
};

class Tracker {
public:
    int slot_of(uint32_t hash) const
    {
        for (int i = 0; i < SlotCount; ++i)
            if (KNOWN[i].hash == hash)
                return i;
        return -1;
    }

    const char *known_name(uint32_t hash) const
    {
        int s = slot_of(hash);
        return s < 0 ? nullptr : KNOWN[s].name;
    }

    void flow(uint32_t hash, uint32_t from, uint32_t to, const char *name, bool named, uint64_t now)
    {
        to &= 3;
        last_.hash = hash;
        last_.from = from & 3;
        last_.to = to;
        last_.named = named;
        last_.transitions = ++transitions_;
        snprintf(last_.name, sizeof last_.name, "%s", name ? name : "");
        size_t i = 0;
        while (i < states_ && seen_[i].hash != hash)
            ++i;
        if (i == states_ && states_ < MAX_STATES)
            seen_[states_++].hash = hash;
        if (i < states_) {
            seen_[i].status = to;
            seen_[i].since = now;
        }
        int slot = slot_of(hash);
        if (slot < 0)
            return;
        if (to == SWOFTY_FLOW_ENTERING) {
            if (slot == TeamHq || slot == TeamHqUpdate || slot == CareerHubUpdate)
                flow_career_ = true;
            if (slot >= TeamHqUpdate && slot <= TeamHqSeasonBreak)
                last_sub_ = slot;
        }
        if (to == SWOFTY_FLOW_NONE && slot == TeamHq && last_sub_ != TeamHqToSession) {
            flow_career_ = false;
            team_hq_mode_ = SWOFTY_CAREER_NONE;
        }
    }

    void team_hq_answer(bool my_team) { team_hq_mode_ = my_team ? SWOFTY_CAREER_MY_TEAM : SWOFTY_CAREER_DRIVER; }

    void components(const Components &c, uint64_t now)
    {
        comp_ = c;
        if (c.entity)
            comp_seen_ = true;
        if (!c.career && status(TeamHqUpdate) == SWOFTY_FLOW_UPDATING) {
            if (!without_since_)
                without_since_ = now;
            if (!comp_broken_ && now - without_since_ > COMPONENT_GRACE_MS)
                comp_broken_ = true;
        } else {
            without_since_ = 0;
            if (c.career)
                comp_broken_ = false;
        }
    }

    uint32_t status(int slot) const { return status_of(KNOWN[slot].hash); }

    uint32_t status_of(uint32_t hash) const
    {
        for (size_t i = 0; i < states_; ++i)
            if (seen_[i].hash == hash)
                return seen_[i].status;
        return SWOFTY_FLOW_NONE;
    }

    bool status_of(uint32_t hash, uint32_t &status, uint64_t &age, uint64_t now) const
    {
        for (size_t i = 0; i < states_; ++i)
            if (seen_[i].hash == hash) {
                status = seen_[i].status;
                age = now >= seen_[i].since ? now - seen_[i].since : 0;
                return true;
            }
        status = SWOFTY_FLOW_NONE;
        age = 0;
        return false;
    }

    bool flow_career() const
    {
        return flow_career_ || status(TeamHq) || status(TeamHqUpdate) || status(CareerHubUpdate);
    }

    bool loaded() const
    {
        if (comp_seen_ && !comp_broken_)
            return comp_.career;
        return flow_career();
    }

    uint32_t mode() const
    {
        if (!loaded())
            return SWOFTY_CAREER_NONE;
        return comp_.mode != SWOFTY_CAREER_NONE ? comp_.mode : team_hq_mode_;
    }

    bool busy() const
    {
        return status(TeamHqTimeAdvance) || status(TeamHqToSession) || status(TeamHqEndOfSeason) ||
               status(TeamHqSeasonBreak);
    }

    bool session() const { return status(Session) || status(LoadSession) || status(UnloadSession); }

    // The poll follows every transition outside a session and a session state's own, not the pause
    // menus and screens inside one.
    bool settles(uint32_t hash) const
    {
        int s = slot_of(hash);
        return !session() || s == Session || s == LoadSession || s == UnloadSession;
    }

    bool hub() const
    {
        return loaded() && mode() != SWOFTY_CAREER_NONE && status(TeamHqUpdate) == SWOFTY_FLOW_UPDATING && !busy() &&
               !session();
    }

    void key(char (&out)[32]) const
    {
        uint32_t m = mode();
        out[0] = 0;
        if (m == SWOFTY_CAREER_NONE || !comp_.player)
            return;
        if (m == SWOFTY_CAREER_MY_TEAM)
            snprintf(out, sizeof out, "mt-%08x-%08x", comp_.team, comp_.driver);
        else
            snprintf(out, sizeof out, "dc-%08x", comp_.driver);
    }

    SwoftyCareerState state() const
    {
        SwoftyCareerState s{};
        s.loaded = loaded();
        s.mode = mode();
        s.hub = hub();
        s.humans = loaded() ? comp_.humans : 0;
        s.has_player = loaded() && comp_.player;
        s.player_driver = s.has_player ? comp_.driver : 0;
        s.player_team = s.has_player ? comp_.team : 0;
        s.components = !comp_seen_ ? SWOFTY_CAREER_COMPONENTS_UNSEEN
                       : comp_broken_ ? SWOFTY_CAREER_COMPONENTS_FALLBACK
                                      : SWOFTY_CAREER_COMPONENTS_LIVE;
        s.component_mode = comp_.career ? comp_.mode : SWOFTY_CAREER_NONE;
        s.team_hq_mode = team_hq_mode_;
        s.flow_career = flow_career();
        key(s.key);
        return s;
    }

    const SwoftyFlowState *last() const { return transitions_ ? &last_ : nullptr; }

    // Compare the derived facts with what was published last and report the events to fire, in
    // order: hub left, unloaded, loaded, then hub entered. A career counts as loaded for the events
    // once its mode is known; a different mode or career key while loaded is an unload followed by a
    // load, and the hub is entered again on the next settle.
    Edges settle()
    {
        Edges e;
        SwoftyCareerState s = state();
        bool live = s.loaded && s.mode != SWOFTY_CAREER_NONE;
        bool changed = live && pub_loaded_ &&
                       (s.mode != pub_mode_ || (s.key[0] && pub_key_[0] && strcmp(s.key, pub_key_)));
        if (pub_loaded_ && (!live || changed)) {
            if (pub_hub_) {
                e.hub = 0;
                pub_hub_ = false;
            }
            e.unloaded = true;
            e.old_mode = pub_mode_;
            pub_loaded_ = false;
        }
        if (live && !pub_loaded_) {
            e.loaded = true;
            pub_loaded_ = true;
            pub_mode_ = s.mode;
            pub_key_[0] = 0;
        }
        if (pub_loaded_ && s.key[0] && !pub_key_[0])
            memcpy(pub_key_, s.key, sizeof pub_key_);
        bool hub = pub_loaded_ && s.hub;
        if (hub != pub_hub_ && !e.unloaded) {
            e.hub = hub;
            pub_hub_ = hub;
        }
        if (!pub_loaded_)
            pub_mode_ = SWOFTY_CAREER_NONE;
        return e;
    }

    uint64_t transitions() const { return transitions_; }

private:
    struct Seen {
        uint32_t hash = 0;
        uint32_t status = 0;
        uint64_t since = 0;
    };

    Seen seen_[MAX_STATES];
    size_t states_ = 0;
    SwoftyFlowState last_{};
    uint64_t transitions_ = 0;
    bool flow_career_ = false;
    int last_sub_ = -1;
    uint32_t team_hq_mode_ = SWOFTY_CAREER_NONE;
    Components comp_;
    bool comp_seen_ = false;
    bool comp_broken_ = false;
    uint64_t without_since_ = 0;
    bool pub_loaded_ = false;
    bool pub_hub_ = false;
    uint32_t pub_mode_ = SWOFTY_CAREER_NONE;
    char pub_key_[32] = {};
};

}
