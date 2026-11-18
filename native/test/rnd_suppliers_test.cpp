#include "rnd_suppliers_logic.hpp"

#include <cstdio>

static int failures;

#define CHECK(cond, msg)                                                                                                \
    do {                                                                                                                \
        if (cond) {                                                                                                     \
            printf("ok  : %s\n", msg);                                                                                  \
        } else {                                                                                                        \
            printf("FAIL: %s\n", msg);                                                                                  \
            failures++;                                                                                                 \
        }                                                                                                               \
    } while (0)

int main()
{
    using namespace runtime::rnd_suppliers;
    CHECK(SLOTS == 384, "a tree's pointer array and a collector's buffer hold 0xc00 / 8 = 384");
    CHECK(trees(PER_DRIVER_UNIQUE) == 2 && trees(1) == 1 && trees(0) == 1, "only PerDriverUnique builds two trees");
    CHECK(budget(PER_DRIVER_UNIQUE) == 192 && budget(1) == 384, "two trees share the 384 slots");

    CHECK(names("Mercedes,BWT Mercedes", "BWT Mercedes"), "a linked engine further down the list matches");
    CHECK(names("Audi_2026, RedBull_Ford_2026", "RedBull_Ford_2026"), "blanks after a comma are ignored");
    CHECK(!names("Mercedes,BWT Mercedes", "BWT"), "a partial name does not match");
    CHECK(!names("Ferrari", ""), "an unknown engine matches nothing");

    CHECK(table_of("upgrades.myteam_2025.engine_suppliers.trees.engine_supplier_upgrades_3") ==
              "upgrades.myteam_2025.engine_suppliers.trees",
          "a supplier row gives its table");
    CHECK(table_of("upgrades.myteam_2025.trees.user_defined").empty(), "a team tree path is not a supplier row");
    CHECK(table_of("x.engine_supplier_upgrades_").empty() && table_of("x.engine_supplier_upgrades_2a").empty(),
          "a row without a plain index is not a supplier row");

    // Game_UpgradesComponent_Deserialise: key = (stat & 0x3fff) << 16 | kind.
    CHECK(vehicle_stat((177u << 16) | 0) == 177 && vehicle_stat((507u << 16) | 1) == 507, "the stat id is the high half");

    // Stock myteam_2025: 86 team rows, suppliers ferrari 6, honda 8, mercedes 6, renault 6.
    std::vector<Supplier> stock = {{"s0", 6, false}, {"s1", 8, false}, {"s2", 6, true}, {"s3", 6, false}};
    Plan p = plan(86, budget(PER_DRIVER_UNIQUE), stock);
    CHECK(!p.over && p.used == 112 && p.load == std::vector<bool>(4, true), "stock rows all load, as in the game");

    // 4 stock + 12 added suppliers of 8 rows: 86 + 26 + 96 = 208 > 192.
    std::vector<Supplier> many = stock;
    many[2].chosen = false;
    for (int i = 0; i < 12; i++)
        many.push_back({"a" + std::to_string(i), 8, i == 11});
    p = plan(86, 192, many);
    CHECK(p.over, "sixteen suppliers pass the two-tree budget");
    CHECK(p.load[15], "the team's own supplier loads even though it is the table's last row");
    CHECK(p.used <= 192, "the tree stays within its budget");
    CHECK(p.used == 86 + 8 + 26 + 8 * 9, "the others fill the room in table order");
    CHECK(!p.load[13] && !p.load[14], "the suppliers past the room stay out");
    CHECK(!p.short_of_chosen, "the own supplier fitted");

    // The same table in a one-tree group fits.
    p = plan(86, budget(1), many);
    CHECK(!p.over && p.used == 208, "one tree has room for all 208 rows");

    // No vehicle engine known: plain guard, table order.
    for (auto &s : many)
        s.chosen = false;
    p = plan(86, 192, many);
    CHECK(p.over && p.used <= 192 && p.load[0] && !p.load[15], "without a known engine the first suppliers load");

    // A team tree that alone passes the budget loads no supplier and says so.
    p = plan(190, 192, {{"s0", 6, true}});
    CHECK(p.over && !p.load[0] && p.short_of_chosen, "an oversized team tree leaves even its own supplier out");
    return failures ? 1 : 0;
}
