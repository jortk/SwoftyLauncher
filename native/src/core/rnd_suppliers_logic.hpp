#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

// Pure logic of the R&D supplier budget (rnd_suppliers_guard.cpp); no game memory here, so it is unit-tested on its own
// (native/test/rnd_suppliers_test.cpp).

namespace runtime::rnd_suppliers {

// An upgrade tree keeps a pointer to each of its instances in a fixed array of 0xc00 bytes (tree + 0x30, count at
// tree + 0xc30) that Career_Instantiate_UpgradeTrees (0x14352cac0) appends to with no bound check, and the R&D
// collectors (0x143536fe0 and ten more with the same 0xc00 stack buffer) copy the instances of every tree of a model
// into one 0xc00 buffer, again unchecked. 0xc00 / 8 = 384.
constexpr int SLOTS = 0x180;
// Engine byte Career_Load_UpgradeTreeNodes gets for the team's own tree; a supplier row passes its engine's stat id.
constexpr uint8_t OWN = 0xff;
// Upgrades model +8 (UpgradesComponent +0x3e0, operating_mode): PerDriverUnique builds a second tree.
constexpr uint8_t PER_DRIVER_UNIQUE = 2;

constexpr const char *DEPARTMENTS[] = {"aerodynamics", "chassis", "durability", "engine", "fake"};
constexpr const char *SUPPLIER_ROW = "engine_supplier_upgrades";

inline int trees(uint8_t mode)
{
    return mode == PER_DRIVER_UNIQUE ? 2 : 1;
}

// Instances one tree may hold so that neither the tree's own array nor a collector over all trees passes 384.
inline int budget(uint8_t mode)
{
    return SLOTS / trees(mode);
}

inline std::string_view trimmed(std::string_view s)
{
    while (!s.empty() && (s.front() == ' ' || s.front() == '\t'))
        s.remove_prefix(1);
    while (!s.empty() && (s.back() == ' ' || s.back() == '\t'))
        s.remove_suffix(1);
    return s;
}

// True when the comma list `list` (engines_linked_to) names `engine`.
inline bool names(std::string_view list, std::string_view engine)
{
    if (engine.empty())
        return false;
    while (true) {
        size_t comma = list.find(',');
        if (trimmed(list.substr(0, comma)) == engine)
            return true;
        if (comma == std::string_view::npos)
            return false;
        list.remove_prefix(comma + 1);
    }
}

// The supplier table of a supplier row path: "<group>.engine_suppliers.trees.engine_supplier_upgrades_<i>" gives
// "<group>.engine_suppliers.trees"; empty when the path is not a supplier row.
inline std::string table_of(std::string_view row)
{
    size_t dot = row.rfind('.');
    if (dot == std::string_view::npos)
        return {};
    std::string_view leaf = row.substr(dot + 1);
    std::string_view stem = SUPPLIER_ROW;
    if (leaf.size() <= stem.size() + 1 || leaf.substr(0, stem.size()) != stem || leaf[stem.size()] != '_')
        return {};
    for (char c : leaf.substr(stem.size() + 1))
        if (c < '0' || c > '9')
            return {};
    return std::string(row.substr(0, dot));
}

// A vehicle key as Career_Add_TeamUpgrades gets it: (stat id & 0x3fff) << 16 | kind (Game_UpgradesComponent_Deserialise
// builds it so at 0x14123fd44..0x14123fd5e).
inline uint32_t vehicle_stat(uint32_t key)
{
    return (key >> 16) & 0x3fff;
}

struct Supplier {
    std::string row;
    int rows = 0;
    bool chosen = false;
};

struct Plan {
    std::vector<bool> load;
    int used = 0;      // instances one tree gets: team rows plus the loaded suppliers' rows
    bool over = false; // stock behaviour would have passed the budget
    bool short_of_chosen = false; // a chosen supplier did not fit even after every other supplier was left out
};

// Which supplier rows a tree loads. All of them while the team's rows and every supplier's rows fit the budget: the
// game's own behaviour, unchanged. Past it the chosen supplier's rows (the team's engine) go first, then the others in
// table order while they fit; a supplier whose rows do not fit stays out of this tree.
inline Plan plan(int team, int limit, const std::vector<Supplier> &suppliers)
{
    Plan p;
    int total = team;
    for (const Supplier &s : suppliers)
        total += s.rows;
    p.over = total > limit;
    p.load.assign(suppliers.size(), !p.over);
    if (!p.over) {
        p.used = total;
        return p;
    }
    p.used = team;
    for (bool chosen : {true, false})
        for (size_t i = 0; i < suppliers.size(); i++) {
            const Supplier &s = suppliers[i];
            if (s.chosen != chosen)
                continue;
            if (p.used + s.rows <= limit) {
                p.load[i] = true;
                p.used += s.rows;
            } else if (chosen) {
                p.short_of_chosen = true;
            }
        }
    return p;
}

}
