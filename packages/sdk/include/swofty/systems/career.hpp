// SPDX-License-Identifier: MIT
#pragma once
#include <array>
#include <atomic>
#include <chrono>
#include <map>
#include <algorithm>
#include <optional>
#include <tuple>
#include <vector>
#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/career_ops.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/forward.hpp>
#include <swofty/systems/ops.hpp>
#include <swofty/systems/teams.hpp>

// Career: modes, contents, seasons and their boxes, calendars, ratings, characters and wizards. The 1.x career library (swofty/career.hpp) keeps
// its names in this namespace: its Result is the operations' result here, and its Mode enum stays the 1.x
// mode, so the career.mode record struct is ModeRecord. apply_teams, apply_lineup, set_driver_ratings,
// swap_drivers, report_season, set_seat, set_car_number and set_dropped_scores call the core's career operations
// (SwoftyCoreCareerOps) and return unavailable on a core without them.
// A season's teams name team.identity records (new teams included) in up to 12 career team slots.

namespace swofty::career {

enum class Kind { driver_career, my_team };
SWOFTY_ENUM(Kind, driver_career, my_team)
enum class Freeze { per_season, at_creation };
SWOFTY_ENUM(Freeze, per_season, at_creation)
enum class AfterLast { market, hold, end };
SWOFTY_ENUM(AfterLast, market, hold, end)
enum class ChangeKind { swap, stand_in, move, replace };
SWOFTY_ENUM(ChangeKind, swap, stand_in, move, replace)
enum class Careers { driver, my_team, both };
SWOFTY_ENUM(Careers, driver, my_team, both)

struct ModeRecord {
    static constexpr Version version{1, 0};
    std::string data_group;
    std::string save_container;
    Text title;
    Kind kind = Kind::driver_career;
    Freeze freeze = Freeze::per_season;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(ModeRecord, data_group, save_container, title, kind, freeze, stat_id)
inline Registry<ModeRecord> modes{"career.mode", {.applies = Applies::boot}};

struct Requirement {
    std::string id;
    std::string range;
    std::string why;
};
SWOFTY_RECORD(Requirement, id, range, why)

struct Content {
    static constexpr Version version{1, 1};
    Ref<ModeRecord> mode;
    Path tile;
    Ref<Content> model;
    Ref<Calendar> calendar;
    AfterLast after_last = AfterLast::market;
    Text title;
    Text description;
    std::string data_group;
    std::string suffix;
    std::string save_container;
    int car_group = -1;
    int badge = 0;
    std::string entitlement;
    Careers careers = Careers::driver;
    std::vector<Requirement> needs;
    json::Value extra;
};
SWOFTY_RECORD(Content, mode, tile, model, calendar, after_last, title, description, data_group, suffix, save_container,
              car_group, badge, entitlement, careers, needs)
inline Registry<Content> contents{"career.content", {.applies = Applies::boot}};

struct SeasonTeam {
    int slot = 0;
    Ref<teams::Identity> team;
    std::array<Ref<drivers::Identity>, 2> seats;
};
SWOFTY_RECORD(SeasonTeam, slot, team, seats)

struct SeasonChange {
    int round = 0, slot = 0, seat = 0;
    Ref<drivers::Identity> out, in;
    ChangeKind kind = ChangeKind::swap;
    std::optional<int> until_round;
};
SWOFTY_RECORD(SeasonChange, round, slot, seat, out, in, kind, until_round)

struct Season {
    static constexpr Version version{1, 1};
    std::string data_group;                    // left out of a year pack's season: a career mode makes it its own with a variant
    int year = 0;
    Ref<::swofty::content::Package> box;
    Ref<Calendar> calendar;
    Keyed<SeasonTeam, &SeasonTeam::slot> teams;
    Keyed<SeasonChange, &SeasonChange::round, &SeasonChange::slot, &SeasonChange::seat> changes;
    json::Value extra;
};
SWOFTY_RECORD(Season, data_group, year, box, calendar, teams, changes)
inline Registry<Season> seasons{"career.season",
                                {.applies = Applies::read, .snapshot = true, .snapshot_budget = 65536}};

struct Round {
    int round = 0;
    Text race;
    Ref<tracks::Venue> venue;
    std::chrono::year_month_day date;
};
SWOFTY_RECORD(Round, round, race, venue, date)

struct Calendar {
    static constexpr Version version{1, 1};
    std::string data_group;
    Keyed<Round, &Round::round> rounds;
    int seasons = 10;
    std::vector<int> transfer_windows;
    json::Value extra;
};
SWOFTY_RECORD(Calendar, data_group, rounds, seasons, transfer_windows)
inline Registry<Calendar> calendars{"career.calendar", {.applies = Applies::boot}};

struct DriverRatings {
    static constexpr Version version{1, 0};
    Ref<Season> season;
    Ref<drivers::Identity> driver;
    float experience = 0, racecraft = 0, awareness = 0, pace = 0;
    json::Value extra;
};
SWOFTY_RECORD(DriverRatings, season, driver, experience, racecraft, awareness, pace)
inline Registry<DriverRatings> ratings{"career.driver_ratings",
                                       {.applies = Applies::read, .snapshot = true, .snapshot_budget = 65536}};

struct CharacterSwap {
    std::string character;
    std::string with;
};
SWOFTY_RECORD(CharacterSwap, character, with)

struct Character {
    static constexpr Version version{1, 0};
    Ref<Character> replaces;
    Text first_name, last_name;
    Path portrait;
    std::vector<std::string> mute;
    std::string data_group;
    Keyed<CharacterSwap, &CharacterSwap::character> swaps;
    std::map<std::string, std::map<std::string, std::string>> strings;
    json::Value extra;
};
SWOFTY_RECORD(Character, replaces, first_name, last_name, portrait, mute, data_group, swaps, strings)
SWOFTY_RECORD_REGISTRY(Character, "career.character")
inline Registry<Character> characters{"career.character", {.applies = Applies::content}};

struct WizardStep {
    std::string route;
    Text title;
    std::vector<std::string> options;
};
SWOFTY_RECORD(WizardStep, route, title, options)

struct WizardRoute {
    std::string from;
    std::string to;
};
SWOFTY_RECORD(WizardRoute, from, to)

struct WizardSwap {
    int at = 0;
    int with = 0;
};
SWOFTY_RECORD(WizardSwap, at, with)

struct Wizard {
    static constexpr Version version{1, 0};
    Ref<Content> content;
    std::vector<WizardStep> steps;
    std::string data_group;
    bool starts_in_f1 = false;
    Keyed<WizardRoute, &WizardRoute::from> tile_routes;
    Keyed<WizardSwap, &WizardSwap::at> replace_steps;
    Keyed<WizardSwap, &WizardSwap::at> force_options;
    Keyed<WizardRoute, &WizardRoute::from> confirm_routes;
    std::vector<std::string> hide_driver_tabs;
    json::Value extra;
};
SWOFTY_RECORD(Wizard, content, steps, data_group, starts_in_f1, tile_routes, replace_steps, force_options, confirm_routes,
              hide_driver_tabs)
inline Registry<Wizard> wizards{"career.wizard", {.applies = Applies::content}};

struct HubTab {
    static constexpr Version version{1, 0};
    std::string name;
    std::string title;
    std::string data_group;
    std::vector<std::string> options;
    json::Value extra;
};
SWOFTY_RECORD(HubTab, name, title, data_group, options)
SWOFTY_RECORD_REGISTRY(HubTab, "hub.tab")
inline Registry<HubTab> hub_tabs{"hub.tab", {.applies = Applies::read}};

// R&D trees and the driver market are database rows a career reads when it is created and each time it loads, so
// both registries apply at boot. A tree is upgrades.<data_group>.trees.<token>.<department>.upgrade_N; a market group
// is driver_market.driver_market.DataGroup[<data_group>]. Each game mode reads its own group; an empty data_group
// means every group.
enum class UpgradeDepartment { aerodynamics, chassis, durability, engine };
SWOFTY_ENUM(UpgradeDepartment, aerodynamics, chassis, durability, engine)
enum class UpgradeSize { minor, major, ultimate };
SWOFTY_ENUM(UpgradeSize, minor, major, ultimate)

struct RndNode {
    std::string label;                         // a stock node's label edits that node; a new label adds a node
    UpgradeDepartment department = UpgradeDepartment::aerodynamics;
    std::string from;                          // a new node copies this node's row; empty: the department's first node
    std::string name;                          // upgrade_name, the card's lang key stem; empty keeps the row's
    std::string type;                          // upgrade_type, one the stock trees use ("Drag", "Engine Power"); empty keeps
    std::optional<UpgradeSize> size;
    std::optional<int> cost;                   // resource points to research it
    std::optional<int> cost_to_develop;
    std::optional<float> weighting;            // how often the AI picks it
    std::optional<int> research_days, develop_days;
    std::optional<int> unlock_at_facility_level;
    std::optional<float> x, y;                 // grid_coordinates: its cell in the tree
    std::vector<std::string> links;            // the nodes it unlocks; empty keeps a stock node's, a new node gets none
    std::vector<std::string> after;            // nodes of the tree (the hub is "default") whose links gain this node
};
SWOFTY_RECORD(RndNode, label, department, from, name, type, size, cost, cost_to_develop, weighting, research_days,
              develop_days, unlock_at_facility_level, x, y, links, after)

struct RndTree {
    static constexpr Version version{1, 0};
    std::string data_group;                    // the upgrades group; empty: 2022, myteam_2025, 2026 and myteam_om_xm
    std::string token;                         // mercedes ferrari redbull williams tororosso renault haas mclaren sauber
                                               // force_india gm_cadillac user_defined
    Keyed<RndNode, &RndNode::label> nodes;
    json::Value extra;
};
SWOFTY_RECORD(RndTree, data_group, token, nodes)
inline Registry<RndTree> rnd_trees{"career.rnd.tree", {.applies = Applies::boot}};

struct Retirement {
    Ref<drivers::Identity> driver;
    std::chrono::year_month_day retire;        // the day he retires
    std::chrono::year_month_day announce;      // the day the news says so
};
SWOFTY_RECORD(Retirement, driver, retire, announce)

struct Firing {
    Ref<drivers::Identity> driver;
    int season_index = 0;                      // the career season he loses his seat in, 0 the first
};
SWOFTY_RECORD(Firing, driver, season_index)

struct TeamRating {
    Ref<teams::Identity> team;
    int rating = 0;                            // the driver rating the team asks for in the market
};
SWOFTY_RECORD(TeamRating, team, rating)

struct Market {
    static constexpr Version version{1, 0};
    std::string data_group;                    // the market group; empty: every group (2025 and 2026)
    std::optional<std::vector<Retirement>> retirements;   // every retirement schedule's drivers
    std::optional<std::vector<Firing>> firings;           // every firing schedule's drivers
    Keyed<TeamRating, &TeamRating::team> team_ratings;
    json::Value extra;
};
SWOFTY_RECORD(Market, data_group, retirements, firings, team_ratings)
inline Registry<Market> markets{"career.market", {.applies = Applies::boot}};

namespace events {

struct SeasonStart {
    static constexpr const char *point = "career.season.start";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int32_t year;
    int32_t season_index;
};

struct AddActivity {
    static constexpr const char *point = "career.calendar.add_activity";
    static constexpr uint32_t version = 1;
    static constexpr uint8_t contract_event = 0x0f;
    uint32_t size, payload_version;
    void *calendar;
    const void *activity;
    uint32_t driver;
    uint32_t day;
    uint8_t type;
    bool player;
};

struct ContractDecide {
    static constexpr const char *point = "career.contract.decide";
    static constexpr uint32_t version = 1;
    static constexpr uint32_t negotiate = 0x5e3c3c2d;
    static constexpr uint32_t ends_this_season = 0xd2985448;
    static constexpr uint32_t next = 0x7c9b1ec4;
    uint32_t size, payload_version;
    void *state;
    uint32_t link;
};

struct ContractSign {
    static constexpr const char *point = "career.contract.sign";
    static constexpr uint32_t version = 1;
    static constexpr uint32_t work_out_replaced = 0x3fff0000;
    uint32_t size, payload_version;
    void *contracts;
    void *contract;
    uint32_t driver;
    uint32_t replaced;
    uint32_t team;
    uint8_t type;
    bool notify;
};

struct StandingsRebuild {
    static constexpr const char *point = "career.standings.rebuild";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    void *championship;
};

#define SWOFTY_CAREER_EVENT_(T, name)                                                                                  \
    struct T {                                                                                                         \
        static constexpr const char *point = name;                                                                     \
        static constexpr uint32_t version = 1;                                                                         \
        uint32_t size, payload_version;                                                                                \
    };
SWOFTY_CAREER_EVENT_(Created, "career.created")
SWOFTY_CAREER_EVENT_(Loaded, "career.loaded")
SWOFTY_CAREER_EVENT_(Unloaded, "career.unloaded")
SWOFTY_CAREER_EVENT_(SeasonEnd, "career.season.end")
SWOFTY_CAREER_EVENT_(SeasonBreak, "career.season.break")
SWOFTY_CAREER_EVENT_(Saving, "career.saving")
SWOFTY_CAREER_EVENT_(Saved, "career.saved")
SWOFTY_CAREER_EVENT_(Missing, "career.missing")
#undef SWOFTY_CAREER_EVENT_

}

struct ApplyReport {
    size_t changed = 0, unchanged = 0, refused = 0;
    std::vector<std::string> refusals;
};

namespace detail {

inline std::atomic<const SwoftyCareerSaves *> &saves_override()
{
    static std::atomic<const SwoftyCareerSaves *> t{nullptr};
    return t;
}

inline void set_saves_table(const SwoftyCareerSaves *t) { saves_override() = t; }

inline const SwoftyCareerSaves *saves_table()
{
    if (auto t = saves_override().load())
        return t->version >= 1 ? t : nullptr;
#if defined(_WIN32)
    static std::atomic<const SwoftyCareerSaves *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreCareerSavesFn>(GetProcAddress(core, "SwoftyCoreCareerSaves")) : nullptr;
    const SwoftyCareerSaves *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
#else
    return nullptr;
#endif
}

inline const SwoftyCareerSaves *live_saves()
{
    auto t = saves_table();
    return t && t->available && t->available() ? t : nullptr;
}

template <class Fn> inline std::string grow(Fn &&fn)
{
    std::string out(2048, '\0');
    size_t len = 0;
    int r = fn(out.data(), out.size(), &len);
    if (r == SWOFTY_TOO_SMALL) {
        out.assign(len + 1, '\0');
        r = fn(out.data(), out.size(), &len);
    }
    if (r != SWOFTY_ASSET_OK || len > out.size())
        return {};
    out.resize(len);
    return out;
}

}

// The core's career operations (swofty_api.h SwoftyCareerOps, the core export SwoftyCoreCareerOps). On a core
// without them every call returns unavailable.
inline const SwoftyCareerOps *ops() { return detail::live_ops(); }

// A data group's career team slots: slot N (1-based) races as team stat team_stats[N-1]. apply_lineup reads
// the table of the season's data group.
inline Result set_slots(std::string_view data_group, const std::vector<uint32_t> &team_stats)
{
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    return detail::ops_result(t->set_slots(std::string(data_group).c_str(), team_stats.data(), team_stats.size()));
}

// The same by team references: slot N (1-based) races as the team the Nth reference names ("game/<label>", or
// a team.identity id). invalid when the game has no row for a reference; unavailable on a core without table
// version 7.
inline Result set_slots(std::string_view data_group, const std::vector<Ref<teams::Identity>> &teams)
{
    auto t = detail::live_ops();
    if (!t || t->version < 7 || !t->team_stat)
        return Result::unavailable;
    std::vector<uint32_t> stats;
    for (const auto &team : teams) {
        std::string id(team.id());
        uint32_t stat = 0;
        if (id.empty() || t->team_stat(id.c_str(), &stat) != SWOFTY_ASSET_OK)
            return Result::invalid;
        stats.push_back(stat);
    }
    return set_slots(data_group, stats);
}

// Where the loaded career's season is: season_index (0 the first season of the career), round (1-based, the
// race weekend the career is at: the next race to run) and rounds (the season's weekend count).
struct RoundPosition {
    int season_index = 0, round = 0, rounds = 0;
};

// The loaded Driver Career's round, or nothing while no career is loaded, its season has no built weekends or
// the career is busy; read it at an idle Team HQ. Needs table version 7.
inline std::optional<RoundPosition> current_round()
{
    auto t = detail::live_ops();
    if (!t || t->version < 7 || !t->current_round)
        return std::nullopt;
    int32_t index = 0, round = 0, rounds = 0;
    if (t->current_round(&index, &round, &rounds) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return RoundPosition{index, round, rounds};
}

// The season's seats as they are at `round`: every listed team keeps its slot and gets the drivers its `changes`
// with round <= `round` leave on its seats. In the order of round, slot and seat: replace puts `in` on the seat
// for the rest of the season; swap puts `in` on the seat and gives his old seat to the seat's previous driver;
// move puts `in` on the seat and leaves his old seat empty until another change fills it; stand_in puts `in` on
// the seat from its round up to (not including) until_round, one race when until_round is absent, then the seat's
// driver before it returns. A season without changes returns its seats as they are.
inline Season seats_at(const Season &season, int round)
{
    Season out = season;
    out.changes.clear();
    struct Cover {
        size_t team;
        int seat, until;
        Ref<drivers::Identity> in;
    };
    std::vector<Cover> covers;
    std::vector<SeasonChange> due;
    for (const SeasonChange &c : season.changes)
        if (c.round <= round)
            due.push_back(c);
    std::stable_sort(due.begin(), due.end(), [](const SeasonChange &a, const SeasonChange &b) {
        return std::tie(a.round, a.slot, a.seat) < std::tie(b.round, b.slot, b.seat);
    });
    auto find_team = [&](int slot) -> SeasonTeam * { return out.teams.find(slot); };
    for (const SeasonChange &c : due) {
        SeasonTeam *team = find_team(c.slot);
        if (!team || c.seat < 0 || c.seat > 1)
            continue;
        if (c.kind == ChangeKind::stand_in) {
            covers.push_back({size_t(team - out.teams.items().data()), c.seat, c.until_round.value_or(c.round + 1), c.in});
            continue;
        }
        Ref<drivers::Identity> previous = team->seats[c.seat];
        if (c.kind == ChangeKind::swap || c.kind == ChangeKind::move)
            for (SeasonTeam &other : out.teams)
                for (int k = 0; k < 2; ++k)
                    if (&other.seats[k] != &team->seats[c.seat] && other.seats[k].id() == c.in.id())
                        other.seats[k] = c.kind == ChangeKind::swap ? previous : Ref<drivers::Identity>("");
        team->seats[c.seat] = c.in;
    }
    for (const Cover &c : covers)
        if (round < c.until)
            out.teams.items()[c.team].seats[c.seat] = c.in;
    return out;
}

// apply_lineup for the season's seats at `round` (seats_at): the changes due by then are written, a stand-in's
// regular driver returns when his window ends. The player's seat is never given away (apply_lineup refuses it).
// Call it at an idle Team HQ with the round from current_round, after set_slots; a seat already right is
// unchanged, so calling it again is harmless.
inline Result apply_round(const Season &season, int round, ApplyReport *report = nullptr);

namespace detail {

using SeasonOpFn = int (*)(SwoftyText, char *, size_t, size_t *);

inline Result run_season_op(SeasonOpFn fn, const Season &season, ApplyReport *report)
{
    std::string request = "{\"season\":" + to_json(season) + "}";
    std::string out(1024, '\0');
    size_t len = 0;
    int r = fn({request.data(), request.size()}, out.data(), out.size(), &len);
    if (r == SWOFTY_TOO_SMALL) {
        out.assign(len + 1, '\0');
        r = fn({request.data(), request.size()}, out.data(), out.size(), &len);
    }
    if (report && len < out.size()) {
        json::Value v = json::parse(std::string_view(out.data(), len));
        report->changed = static_cast<size_t>(v["changed"].as_int());
        report->unchanged = static_cast<size_t>(v["unchanged"].as_int());
        report->refused = static_cast<size_t>(v["refused"].as_int());
        if (const json::Array *list = v["refusals"].if_array())
            for (const json::Value &why : *list)
                report->refusals.emplace_back(why.as_string());
    }
    return ops_result(r);
}

}

// The season's teams race in its data group's career: each listed slot (1-12) races as its team (the slot
// table and the Championship's participants), and a team with no career rows gets them, built from its
// donor's. An empty team leaves the slot out of the Championship; a slot the season doesn't list keeps its
// team. Call it before apply_lineup. unavailable on a core without the call (table version 3).
inline Result apply_teams(const Season &season, ApplyReport *report = nullptr)
{
    if (report)
        *report = {};
    auto t = detail::live_ops();
    if (!t || t->version < 3 || !t->apply_teams)
        return Result::unavailable;
    return detail::run_season_op(t->apply_teams, season, report);
}

inline Result apply_lineup(const Season &season, ApplyReport *report = nullptr)
{
    if (report)
        *report = {};
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    return detail::run_season_op(t->apply_lineup, season, report);
}

inline Result apply_round(const Season &season, int round, ApplyReport *report)
{
    return apply_lineup(seats_at(season, round), report);
}

namespace detail {

inline Result seat_request(const SwoftyCareerOps *t, const std::string &request)
{
    char out[2048];
    size_t len = 0;
    int r = t->set_seat({request.data(), request.size()}, out, sizeof out, &len);
    return r == SWOFTY_TOO_SMALL ? Result::ok : ops_result(r);
}

}

// The driver takes race seat `seat` (0 or 1) of career team slot `slot` (1-based, the loaded career's data
// group's slot table): a race driver exchanges seats with the seat's driver; a free agent takes the seat and its
// driver leaves the grid. Driver Career only; unavailable on a core without the call (table version 4).
inline Result set_seat(int slot, int seat, Ref<drivers::Identity> driver)
{
    auto t = detail::live_ops();
    if (!t || t->version < 4 || !t->set_seat)
        return Result::unavailable;
    uint32_t stat = 0;
    if (!detail::stat_of(t, driver, stat))
        return Result::invalid;
    return detail::seat_request(t, "{\"driver\":" + std::to_string(stat) + ",\"slot\":" + std::to_string(slot) +
                                       ",\"seat\":" + std::to_string(seat) + "}");
}

// The same by team stat id, for a career without a slot table (a stock career).
inline Result set_seat_of_team(uint32_t team_stat, int seat, Ref<drivers::Identity> driver)
{
    auto t = detail::live_ops();
    if (!t || t->version < 4 || !t->set_seat)
        return Result::unavailable;
    uint32_t stat = 0;
    if (!detail::stat_of(t, driver, stat))
        return Result::invalid;
    return detail::seat_request(t, "{\"driver\":" + std::to_string(stat) + ",\"team\":" + std::to_string(team_stat) +
                                       ",\"seat\":" + std::to_string(seat) + "}");
}

inline Result set_driver_ratings(Ref<drivers::Identity> driver, const DriverRatings &ratings)
{
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    uint32_t stat = 0;
    if (!detail::stat_of(t, driver, stat))
        return Result::invalid;
    const uint16_t ids[] = {0, 1, 3, 4};
    const float values[] = {ratings.experience, ratings.racecraft, ratings.awareness, ratings.pace};
    return detail::ops_result(t->set_driver_ratings(stat, ids, values, 4));
}

// The driver's racing number (1-99) on his career row and the current weekend's copy. refused when another
// driver holds it, unless swap, which gives that driver this driver's old number. Unavailable on a core
// without the call (table version 4).
inline Result set_car_number(Ref<drivers::Identity> driver, int number, bool swap = false)
{
    auto t = detail::live_ops();
    if (!t || t->version < 4 || !t->set_car_number)
        return Result::unavailable;
    uint32_t stat = 0;
    if (!detail::stat_of(t, driver, stat))
        return Result::invalid;
    return detail::ops_result(t->set_car_number(stat, number, swap ? SWOFTY_NUMBER_SWAP : 0));
}

inline Result swap_drivers(Ref<drivers::Identity> a, Ref<drivers::Identity> b)
{
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    uint32_t sa = 0, sb = 0;
    if (!detail::stat_of(t, a, sa) || !detail::stat_of(t, b, sb))
        return Result::invalid;
    return detail::ops_result(t->swap_drivers(sa, sb));
}

// While the loaded career is filed under data_group, the drivers' championship counts each driver's best `best`
// results (the constructors' championship counts every result); 0 counts every result. unavailable on a core
// without the call (table version 7).
inline Result set_dropped_scores(std::string_view data_group, int best)
{
    auto t = detail::live_ops();
    if (!t || t->version < 7 || !t->set_dropped_scores)
        return Result::unavailable;
    std::string group(data_group);
    return detail::ops_result(t->set_dropped_scores(group.c_str(), best < 0 ? 0u : uint32_t(best)));
}

// The season at the Championship's season_index is year, from the addon that owns the active career's data
// group: the core fires career.season.start once per career and year, activates the season's records and
// freezes the season. refused from any other addon; unavailable on a core without the call (table version 2).
inline Result report_season(int year, int season_index)
{
    auto t = detail::live_ops();
    if (!t || t->version < 2 || !t->report_season)
        return Result::unavailable;
    return detail::ops_result(t->report_season(year, season_index));
}

// Career saves (since 1.7.0, experimental). The core keeps an index of the careers it has seen, in the
// launcher's data folder ($DATA/careers/index.json), and an addon's own data for each career beside it.
// Every call reports unavailable on a core without the table (SwoftyCareerSaves, the core export
// SwoftyCoreCareerSaves), and the index is empty then.
//
//   auto here = career::saves::current();                 // "f1_career_progressive:7731"
//   career::saves::need("swofty.progressive", "^1.3.0", "its calendar");
//   auto mine = career::saves::data("swofty.progressive");
//   mine.set(here, json::parse(R"({"driver":"max"})"), 2);
//   auto back = mine.get(here);                           // {version, value}
//
// career.saving fires before the game writes the profile (a handler may call data(...).set for the loaded
// career: the core writes it with the index), career.saved when the profile's containers have been written,
// and career.missing when the index knew a career the game's profile load did not find.
namespace saves {

struct Need {
    std::string id, range, why;
};

struct Entry {
    std::string career, mode, content, team, driver, label, status;
    int season = 0, round = 0;
    int64_t last_saved = 0;
    std::vector<Need> addons;
};

namespace detail {

inline Entry entry_of(const json::Value &v)
{
    Entry e;
    e.career = std::string(v["career"].as_string());
    e.mode = std::string(v["mode"].as_string());
    e.content = std::string(v["content"].as_string());
    e.team = std::string(v["team"].as_string());
    e.driver = std::string(v["driver"].as_string());
    e.label = std::string(v["label"].as_string());
    e.status = std::string(v["status"].as_string());
    e.season = static_cast<int>(v["season"].as_int());
    e.round = static_cast<int>(v["round"].as_int());
    e.last_saved = v["last_saved"].as_int();
    if (const json::Array *list = v["addons"].if_array())
        for (const json::Value &a : *list)
            e.addons.push_back({std::string(a["id"].as_string()), std::string(a["need"].as_string()),
                                std::string(a["why"].as_string())});
    return e;
}

inline std::vector<Entry> entries_of(const std::string &text)
{
    std::vector<Entry> out;
    if (text.empty())
        return out;
    json::Value v = json::parse(text);
    if (const json::Array *list = v["careers"].if_array())
        for (const json::Value &c : *list)
            out.push_back(entry_of(c));
    return out;
}

}

inline bool available() { return ::swofty::career::detail::live_saves() != nullptr; }

inline std::vector<Entry> index()
{
    auto t = ::swofty::career::detail::live_saves();
    if (!t || !t->index)
        return {};
    return detail::entries_of(::swofty::career::detail::grow(
        [&](char *buf, size_t cap, size_t *len) { return t->index(buf, cap, len); }));
}

inline std::vector<Entry> missing()
{
    auto t = ::swofty::career::detail::live_saves();
    if (!t || !t->missing)
        return {};
    return detail::entries_of(::swofty::career::detail::grow(
        [&](char *buf, size_t cap, size_t *len) { return t->missing(buf, cap, len); }));
}

inline std::string current()
{
    auto t = ::swofty::career::detail::live_saves();
    if (!t || !t->current)
        return {};
    return ::swofty::career::detail::grow([&](char *buf, size_t cap, size_t *len) { return t->current(buf, cap, len); });
}

inline std::optional<Entry> find(std::string_view career)
{
    for (Entry &e : index())
        if (e.career == career)
            return std::move(e);
    return std::nullopt;
}

struct Description {
    std::optional<std::string> driver, team, label;
    std::optional<int> season, round;
};

inline Result describe(const Description &d)
{
    auto t = ::swofty::career::detail::live_saves();
    if (!t || !t->describe)
        return Result::unavailable;
    json::Value v = json::Value::object();
    if (d.driver)
        v["driver"] = json::Value(*d.driver);
    if (d.team)
        v["team"] = json::Value(*d.team);
    if (d.label)
        v["label"] = json::Value(*d.label);
    if (d.season)
        v["season"] = json::Value(int64_t(*d.season));
    if (d.round)
        v["round"] = json::Value(int64_t(*d.round));
    std::string text = json::dump(v);
    return ::swofty::career::detail::ops_result(t->describe({text.data(), text.size()}));
}

inline Result need(std::string_view addon, std::string_view range = {}, std::string_view why = {})
{
    auto t = ::swofty::career::detail::live_saves();
    if (!t || !t->need)
        return Result::unavailable;
    return ::swofty::career::detail::ops_result(
        t->need(std::string(addon).c_str(), std::string(range).c_str(), std::string(why).c_str()));
}

struct Stored {
    uint32_t version = 0;
    json::Value value;
};

class Data {
public:
    explicit Data(std::string addon) : addon_(std::move(addon)) {}

    std::optional<Stored> get(std::string_view career) const
    {
        auto t = ::swofty::career::detail::live_saves();
        if (!t || !t->data_get)
            return std::nullopt;
        std::string career_id(career);
        uint32_t version = 0;
        std::string text = ::swofty::career::detail::grow([&](char *buf, size_t cap, size_t *len) {
            return t->data_get(career_id.c_str(), addon_.c_str(), buf, cap, len, &version);
        });
        if (text.empty())
            return std::nullopt;
        return Stored{version, json::parse(text)};
    }

    std::optional<Stored> get_current() const { return get(current()); }

    Result set(std::string_view career, const json::Value &value, uint32_t version = 1) const
    {
        auto t = ::swofty::career::detail::live_saves();
        if (!t || !t->data_set)
            return Result::unavailable;
        std::string career_id(career), text = json::dump(value);
        return ::swofty::career::detail::ops_result(
            t->data_set(career_id.c_str(), addon_.c_str(), version, {text.data(), text.size()}));
    }

    Result set_current(const json::Value &value, uint32_t version = 1) const { return set(current(), value, version); }

    Result remove(std::string_view career) const
    {
        auto t = ::swofty::career::detail::live_saves();
        if (!t || !t->data_remove)
            return Result::unavailable;
        std::string career_id(career);
        return ::swofty::career::detail::ops_result(t->data_remove(career_id.c_str(), addon_.c_str()));
    }

private:
    std::string addon_;
};

inline Data data(std::string_view addon) { return Data(std::string(addon)); }

}

}
