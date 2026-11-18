#pragma once

#include <map>
#include <set>
#include <string>
#include <utility>
#include <vector>

// A new Grand Prix keeps the last Grand Prix's track in its weekend (the track byte of Career_Create_SeasonWeekend,
// Weekend+0x278): the track select only rewrites it on a selection change, and a player who confirms the highlighted
// track or never opens the screen races the remembered one. When that track's row is gone (its addon off or
// uninstalled) or is a dormant stock row with no track archive (an imported track's slot in another profile),
// TrackDataSystem has no usable item for it and Game_ChampionshipGameMode_BuildSessionSettings reads +0x78 of null.
// The decision below swaps such a track for the stock track before the game reads the weekend. It takes the
// sources in order of trust and the first one that knows any rows decides: the tracks table as the game has it
// (read live or cached from an earlier read), the launcher's track base, and last the shipping tracks table plus the
// registry's track records. Every line names the source that decided.

namespace swofty_core::apply::gp_track {

constexpr const char *STOCK = "melbourne";

struct StockRow {
    int stat_id;
    const char *code;
    bool dormant;
};

// tracks.Track of the shipping F1 25 gamedatabases.2025.bin, all 43 rows: the 9 dormant rows are apply/tracks/seed.hpp
// DORMANT (no track archive), every other row counts as live stock the way the track base lists it (34 stock tracks);
// the nine rows whose label is not a known track code are kept live under a stock_<id> name.
inline constexpr StockRow STOCK_ROWS[] = {
    {0, "melbourne", false},    {1, "paul_ricard", true},     {2, "shanghai", false},      {3, "bahrain", false},
    {4, "catalunya", false},    {5, "monaco", false},         {6, "montreal", false},      {7, "silverstone", false},
    {8, "hanoi", true},         {9, "hungaroring", false},    {10, "spa_francorchamps", false}, {11, "monza", false},
    {12, "singapore", false},   {13, "suzuka", false},        {14, "abu_dhabi", false},    {15, "texas", false},
    {16, "brazil", false},      {17, "austria", false},       {18, "sochi", true},         {19, "mexico", false},
    {20, "baku", false},        {21, "bahrain_pad", true},    {22, "silverstone_int", true}, {23, "texas_nat", true},
    {24, "suzuka_east", true},  {25, "stock_25", false},      {26, "zandvoort", false},    {27, "imola", false},
    {28, "portimao", true},     {29, "jeddah", false},        {30, "miami", false},        {31, "las_vegas", false},
    {32, "losail", false},      {33, "stock_33", false},      {34, "stock_34", false},     {35, "bahrain24", true},
    {36, "stock_36", false},    {37, "stock_37", false},      {38, "stock_38", false},     {39, "stock_39", false},
    {40, "stock_40", false},    {41, "stock_41", false},      {42, "madrid", false},
};

struct Source {
    std::string name;
    std::map<int, std::string> rows;
    std::set<int> dormant;
};

struct Fallback {
    bool replace = false;
    int stat_id = -1;
    std::string line;
    std::string source;
};

inline const StockRow *stock_row(int stat_id)
{
    for (const StockRow &r : STOCK_ROWS)
        if (r.stat_id == stat_id)
            return &r;
    return nullptr;
}

inline const StockRow *stock_row_by_code(const std::string &code)
{
    for (const StockRow &r : STOCK_ROWS)
        if (code == r.code)
            return &r;
    return nullptr;
}

inline Source stock_source(const std::vector<std::pair<int, std::string>> &records)
{
    Source s{"the shipping tracks table and the track records", {}, {}};
    for (const StockRow &r : STOCK_ROWS) {
        s.rows[r.stat_id] = r.code;
        if (r.dormant)
            s.dormant.insert(r.stat_id);
    }
    for (auto &[id, record] : records)
        if (id >= 0 && id < 0xff) {
            s.rows[id] = record;
            s.dormant.erase(id);
        }
    return s;
}

inline Fallback fallback(int weekend_stat_id, const std::vector<Source> &sources, const char *stock = STOCK)
{
    Fallback f;
    if (weekend_stat_id < 0 || weekend_stat_id >= 0xff)
        return f;
    const Source *decider = nullptr;
    for (const Source &s : sources)
        if (!s.rows.empty()) {
            decider = &s;
            break;
        }
    if (!decider)
        return f;
    f.source = decider->name;
    const StockRow *shipping = stock_row(weekend_stat_id);
    bool has_row = decider->rows.count(weekend_stat_id) > 0, dormant = decider->dormant.count(weekend_stat_id) > 0;
    // A live row of the shipping table is always raceable, whatever a partial source knows; a dormant one is dormant
    // unless the deciding source lists a live row for it (an addon's record fills the slot).
    if (shipping && !shipping->dormant)
        return f;
    if (has_row && !dormant)
        return f;
    if (!has_row && shipping)
        dormant = true;
    std::string from;
    for (const Source &s : sources) {
        for (auto &[id, label] : s.rows)
            if (label == stock && !s.dormant.count(id)) {
                f.stat_id = id;
                break;
            }
        if (f.stat_id >= 0) {
            from = &s == decider ? std::string() : " with " + std::string(stock) + "'s stat id from " + s.name;
            break;
        }
    }
    std::string id = std::to_string(weekend_stat_id);
    std::string code = has_row ? decider->rows.at(weekend_stat_id) : shipping ? shipping->code : std::string();
    std::string why = dormant ? " is the dormant row " + code +
                                    " with no track archive (the addon that fills it is off in this profile)"
                              : " has no tracks row (its track addon is off or not installed)";
    if (f.stat_id < 0) {
        f.line = "the Grand Prix weekend's track stat id " + id + why + " and no source has a " + stock +
                 " row to race instead; the session load goes on (decided by " + decider->name + ")";
        return f;
    }
    f.replace = true;
    f.line = "the Grand Prix weekend's track stat id " + id + why + "; the last Grand Prix raced it, this one starts at " + stock +
             " (stat id " + std::to_string(f.stat_id) + ") instead (decided by " + decider->name + from + ")";
    return f;
}

}
