// The turn lookup is adapted from Corners::official_for and Corners::reset in Project Halo by nyannoying
// (https://github.com/nyannoying1337/f1-25-project-halo, native/src/halo_track.cpp). Uses work from Project Halo by
// nyannoying; see THIRD_PARTY.md. The learned-bend fallback of the original is not ported.
#pragma once
#include <swofty_api.h>

#include "live_corners_data.hpp"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <iterator>
#include <string>
#include <vector>

namespace swofty_core::live::turns {

constexpr float AHEAD_M = 220.f, BEHIND_M = 15.f;

struct Alias {
    int track;
    const char *words[4];
};

inline const Alias *aliases(size_t &n)
{
    static const Alias table[] = {
        {0, {"melbourne", "australia", "albert"}}, {2, {"shanghai", "china"}}, {3, {"sakhir", "bahrain"}},
        {4, {"catalunya", "barcelona", "spain"}}, {5, {"monaco", "montecarlo"}}, {6, {"montreal", "canada"}},
        {7, {"silverstone", "britain", "greatbritain"}}, {9, {"hungaroring", "hungary", "budapest"}},
        {10, {"spa", "belgium", "francorchamps"}}, {11, {"monza", "italy"}}, {12, {"singapore", "marinabay"}},
        {13, {"suzuka", "japan"}}, {14, {"abudhabi", "yas", "yasmarina"}}, {15, {"austin", "texas", "cota"}},
        {16, {"interlagos", "brazil", "saopaulo"}}, {17, {"spielberg", "austria", "redbullring"}},
        {19, {"mexico", "mexicocity"}}, {20, {"baku", "azerbaijan"}}, {26, {"zandvoort", "netherlands"}},
        {27, {"imola", "emilia", "emiliaromagna"}}, {29, {"jeddah", "saudi", "saudiarabia"}}, {30, {"miami"}},
        {31, {"vegas", "lasvegas"}}, {32, {"losail", "qatar"}},
    };
    n = std::size(table);
    return table;
}

inline int index_of_track(int track)
{
    for (int i = 0; i < int(std::size(kTracks)); i++)
        if (kTracks[i].track == track)
            return i;
    return -1;
}

inline int official_for(const char *name, float len)
{
    std::vector<std::string> words;
    std::string w;
    for (const char *p = name ? name : "";; ++p) {
        char c = *p;
        if ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9'))
            w += c;
        else if (c >= 'A' && c <= 'Z')
            w += char(c - 'A' + 'a');
        else {
            if (!w.empty())
                words.push_back(w);
            w.clear();
            if (!c)
                break;
        }
    }
    size_t single = words.size();
    for (size_t i = 0; i + 1 < single; i++)
        words.push_back(words[i] + words[i + 1]);
    for (const std::string &x : words)
        if (x == "reverse" || x == "rev" || x == "short")
            return -1;
    int by_name = -1;
    size_t na;
    const Alias *al = aliases(na);
    for (size_t k = 0; k < na; k++)
        for (const char *alias : al[k].words)
            if (alias && std::find(words.begin(), words.end(), std::string(alias)) != words.end())
                by_name = index_of_track(al[k].track);
    if (by_name >= 0)
        return len <= 0 || std::fabs(len - kTracks[by_name].length_m) <= 0.03f * kTracks[by_name].length_m ? by_name : -1;
    int best = -1, near = 0;
    for (int i = 0; i < int(std::size(kTracks)); i++) {
        float d = std::fabs(len - kTracks[i].length_m) / kTracks[i].length_m;
        if (d <= 0.015f) {
            near++;
            best = i;
        } else if (d <= 0.02f)
            near++;
    }
    return near == 1 ? best : -1;
}

struct Table {
    int official = -1;
    float length = 0;
    int track = -1;

    void reset(const char *name, float length_m)
    {
        length = length_m;
        official = official_for(name, length_m);
        track = official >= 0 ? kTracks[official].track : -1;
        if (official >= 0 && length <= 0)
            length = kTracks[official].length_m;
    }

    int count() const { return official >= 0 ? kTracks[official].count : 0; }

    void fill(int i, SwoftyLiveTurn &t, float lap_m) const
    {
        const Corner &c = kTracks[official].corners[i];
        std::memset(&t, 0, sizeof t);
        t.version = SWOFTY_LIVE_VERSION;
        t.size = sizeof t;
        t.number = c.number;
        t.letter = c.letter;
        t.right = c.left ? 0 : 1;
        t.source = SWOFTY_LIVE_TURN_OFFICIAL;
        t.apex_lap_m = c.frac * length;
        t.distance_ahead_m = t.apex_lap_m - lap_m;
        if (c.letter)
            std::snprintf(t.label, sizeof t.label, "%d%c", c.number, c.letter);
        else
            std::snprintf(t.label, sizeof t.label, "%d", c.number);
    }

    bool at(float lap_m, SwoftyLiveTurn &out) const
    {
        if (official < 0 || !std::isfinite(lap_m))
            return false;
        int n = count();
        int best = -1;
        float best_d = 0;
        for (int i = 0; i < n; i++) {
            float apex = kTracks[official].corners[i].frac * length;
            for (float shift : {0.f, -length, length}) {
                float d = apex + shift - lap_m;
                if (d < -BEHIND_M || d > AHEAD_M)
                    continue;
                if (best < 0 || d < best_d) {
                    best = i;
                    best_d = d;
                }
            }
        }
        if (best < 0)
            return false;
        fill(best, out, lap_m);
        out.distance_ahead_m = best_d;
        return true;
    }
};

}
