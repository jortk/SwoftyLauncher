#include "lang_scope.hpp"

#include <cstdio>
#include <cstring>

namespace ls = swofty_core::lang_scope;
namespace hk = swofty_core::hooks;

int fails;

void expect(bool ok, const char *what)
{
    std::printf("%s %s\n", ok ? "ok  :" : "FAIL:", what);
    fails += !ok;
}

bool is(const char *got, const char *want)
{
    return want ? got && !std::strcmp(got, want) : !got;
}

ls::Variant variant(const char *all, const char *within_json)
{
    ls::Variant v;
    v.all = all;
    if (within_json) {
        swofty::json::Error err;
        std::string why;
        if (!ls::compile_within(swofty::json::parse(within_json, &err), v.within, &why))
            std::printf("FAIL: within %s does not compile: %s\n", within_json, why.c_str());
    }
    return v;
}

hk::Facts in_content(const char *content)
{
    hk::Facts f;
    f.mode = "career";
    f.content = content;
    f.chain = {content};
    return f;
}

int main()
{
    const char *sw13 = R"({"content": "swofty.swofty-seasons-2013/sw_2013"})";
    std::vector<ls::Variant> kers = {variant("KERS", sw13)};
    hk::Facts none;
    hk::Facts season = in_content("swofty.swofty-seasons-2013/sw_2013");
    hk::Facts other = in_content("swofty.progressive-career/f1_career_2015");
    hk::Facts tt = in_content("swofty.swofty-seasons-2013/sw_2013_tt");

    expect(ls::scoped(kers), "a record with a within is scoped");
    expect(is(ls::choose(kers, &season, "eng"), "KERS"), "a season's string shows in that season");
    expect(is(ls::choose(kers, &other, "eng"), nullptr), "a season's string stays out of another season's career");
    expect(is(ls::choose(kers, &none, "eng"), nullptr), "a season's string stays out of the main menu");
    expect(is(ls::choose(kers, &tt, "eng"), nullptr), "a season's string stays out of a sibling content it does not name");
    expect(is(ls::choose(kers, nullptr, "eng"), nullptr), "a scoped string with no facts does not show");

    std::vector<ls::Variant> both = {variant("Everywhere", nullptr), variant("Season", sw13)};
    expect(!ls::scoped({variant("Everywhere", nullptr)}), "a record without a within is not scoped");
    expect(is(ls::choose(both, &season, "eng"), "Season"), "a later scoped record wins inside its scope");
    expect(is(ls::choose(both, &other, "eng"), "Everywhere"), "outside it the earlier record shows");

    std::vector<ls::Variant> later = {variant("Season", sw13), variant("Everywhere", nullptr)};
    expect(is(ls::choose(later, &season, "eng"), "Everywhere"), "load order decides: a later unscoped record wins");

    hk::Facts by = in_content("b/y");
    std::vector<ls::Variant> array = {variant("Either", R"([{"any": [{"content": "a/x"}, {"content": "b/y"}]}])")};
    expect(is(ls::choose(array, &other, "eng"), nullptr) && is(ls::choose(array, &by, "eng"), "Either"),
           "a selector array within works");

    ls::Variant fre = variant(nullptr, sw13);
    fre.per.push_back({"fre", "Fre"});
    std::vector<ls::Variant> per = {variant("All", nullptr), fre};
    expect(is(ls::choose(per, &season, "FRE"), "Fre"), "a per-language text matches the code in any case");
    expect(is(ls::choose(per, &season, "eng"), "All"), "a scoped record without the current language falls back");

    std::shared_ptr<const hk::Selectors> bad;
    std::string why;
    swofty::json::Error err;
    expect(!ls::compile_within(swofty::json::parse(R"({"nope": 1})", &err), bad, &why), "an unknown selector is refused");
    expect(ls::compile_within(swofty::json::Value(), bad, &why) && !bad, "a null within means everywhere");

    std::printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
    return fails ? 1 : 0;
}
