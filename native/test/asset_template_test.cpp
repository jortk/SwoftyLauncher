#include "asset_template_logic.hpp"

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
    using runtime::asset_template::package_of;
    using runtime::asset_template::shared_from_template;
    using V = std::vector<std::string>;
    const std::string P = "2025_asset_groups/f1_2025_vehicle_package/teams/";
    const std::string G = "asset_groups/f1_2025_vehicle_package/teams/";
    const std::string MYTEAM = G + "myteam_mercedes/wep/myteam_mercedes.assetgroup";
    const V MYTEAM_ERPS = {P + "common_fom.erp", P + "myteam_mercedes/wep/myteam_mercedes.erp"};
    const std::string COPY = G + "mes242_mercedes/wep/mes242_mercedes.assetgroup";
    const std::string COPY_ERP = P + "mes242_mercedes/wep/mes242_mercedes.erp";

    CHECK(package_of(MYTEAM) == "f1_2025_vehicle_package" && package_of(P + "common.erp") == "f1_2025_vehicle_package",
          "the package is the first folder under asset_groups/ or 2025_asset_groups/");
    CHECK(package_of("ui_package/x.erp").empty() && package_of("asset_groups/x").empty(),
          "a path outside the roots or without a folder has no package");

    V added = shared_from_template(COPY, MYTEAM, MYTEAM_ERPS, {P + "common.erp", COPY_ERP});
    CHECK(added == V{P + "common_fom.erp"},
          "a 2025 My Team car copy that lists teams/common.erp takes teams/common_fom.erp from its template");
    CHECK(shared_from_template(COPY, MYTEAM, MYTEAM_ERPS, {P + "common_fom.erp", COPY_ERP}).empty(),
          "a copy that already lists the shared archive takes nothing");
    CHECK(shared_from_template(COPY, MYTEAM, MYTEAM_ERPS, {COPY_ERP}) == V{P + "common_fom.erp"},
          "the template's own archive is never taken");

    const std::string SLOT = G + "myteam_mercedes/wep/myteam_mercedes_lx01.assetgroup";
    const V SLOT_ERPS = {P + "common.erp", P + "myteam_mercedes/wep/myteam_mercedes_lx01.erp"};
    CHECK(shared_from_template(SLOT, MYTEAM, MYTEAM_ERPS, SLOT_ERPS) == V{P + "common_fom.erp"},
          "a livery slot of a 2025 My Team car takes teams/common_fom.erp");

    const std::string RACE = G + "mercedes/wep/mercedes.assetgroup";
    const V RACE_ERPS = {P + "common.erp", P + "mercedes/wep/mercedes.erp"};
    const std::string CS = "asset_groups/cs_2016_vehicle_package/teams/mercedes/wep/mercedes.assetgroup";
    CHECK(shared_from_template(CS, RACE, RACE_ERPS,
                               {"2025_asset_groups/cs_2016_vehicle_package/teams/common.erp",
                                "2025_asset_groups/cs_2016_vehicle_package/teams/mercedes/wep/mercedes.erp"})
              .empty(),
          "a car in another package keeps only the archives it lists");
    CHECK(shared_from_template(G + "x/wep/x.assetgroup", RACE, RACE_ERPS, {P + "x/wep/x.erp"}) == V{P + "common.erp"},
          "a race car copy in the same package takes teams/common.erp");

    const std::string Q = "2025_asset_groups/environment_package/tracks/bahrain/";
    CHECK(shared_from_template("asset_groups/environment_package/tracks/new/wep/new_f1.assetgroup",
                               "asset_groups/environment_package/tracks/bahrain/wep/bahrain_f1.assetgroup",
                               {Q + "bahrain_common.erp", Q + "wep/bahrain_f1.erp"},
                               {"2025_asset_groups/environment_package/tracks/new/wep/new_f1.erp"})
              .empty(),
          "a track group takes nothing from a stock track template");
    const V MIXED = {P + "common_fom.erp", P + "sub/deep.erp", P + "common_fom.erp", P + "x.erpx"};
    CHECK(shared_from_template(COPY, MYTEAM, MIXED, {}) == V{P + "common_fom.erp"},
          "only archives right in teams/ are taken, once");
    CHECK(shared_from_template(COPY, "", MYTEAM_ERPS, {}).empty(), "no template, nothing taken");

    if (failures)
        printf("%d failure(s)\n", failures);
    return failures ? 1 : 0;
}
