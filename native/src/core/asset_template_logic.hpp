#pragma once

#include <algorithm>
#include <string>
#include <vector>

// Pure logic of the archives a declared asset group takes from its template (assets.cpp); unit-tested on its own
// (native/test/asset_template_test.cpp).

namespace runtime::asset_template {

// The package of a path under asset_groups/ or 2025_asset_groups/, or "" for any other path.
inline std::string package_of(const std::string &path)
{
    for (const char *root : {"asset_groups/", "2025_asset_groups/"}) {
        std::string r = root;
        if (path.rfind(r, 0) == 0) {
            size_t slash = path.find('/', r.size());
            return slash == std::string::npos ? std::string() : path.substr(r.size(), slash - r.size());
        }
    }
    return {};
}

// A stock car group lists one shared archive of its package's teams/ folder before the car's own archive: the 2025
// My Team cars list teams/common_fom.erp, the only archive with the shared car (teams/fom_car/), and every other car
// lists teams/common.erp. A group in the teams/ folder of its template's package is a copy of that car, so it takes
// the template's archives that sit right in that teams/ folder and that it does not list itself, in the template's
// order. Any other group takes nothing from its template.
inline std::vector<std::string> shared_from_template(const std::string &group, const std::string &tmpl,
                                                     const std::vector<std::string> &tmpl_erps,
                                                     const std::vector<std::string> &declared)
{
    std::vector<std::string> out;
    std::string pkg = package_of(group);
    if (pkg.empty() || package_of(tmpl) != pkg)
        return out;
    std::string teams = "asset_groups/" + pkg + "/teams/";
    if (group.rfind(teams, 0) != 0 || tmpl.rfind(teams, 0) != 0)
        return out;
    std::string shared = "2025_" + teams;
    for (auto &erp : tmpl_erps) {
        if (erp.rfind(shared, 0) != 0 || erp.size() <= shared.size() + 4 ||
            erp.compare(erp.size() - 4, 4, ".erp") != 0 || erp.find('/', shared.size()) != std::string::npos)
            continue;
        if (std::find(declared.begin(), declared.end(), erp) == declared.end() &&
            std::find(out.begin(), out.end(), erp) == out.end())
            out.push_back(erp);
    }
    return out;
}

}
