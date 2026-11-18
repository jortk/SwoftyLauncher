// The SDK's table version 7 operations (swofty/systems/career.hpp current_round, set_slots by team references,
// seats_at, apply_round) against a stand-in SwoftyCareerOps table on the host.

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <swofty/systems/career.hpp>

using namespace swofty;

static int failures = 0;
#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c);                                                    \
            ++failures;                                                                                                \
        }                                                                                                              \
    } while (0)

static int g_round = 3, g_rounds = 16, g_result = SWOFTY_ASSET_OK;
static std::string g_request, g_group;
static std::vector<uint32_t> g_slots;

static int available() { return 1; }
static int current_round(int32_t *index, int32_t *round, int32_t *rounds)
{
    if (g_result != SWOFTY_ASSET_OK)
        return g_result;
    *index = 0, *round = g_round, *rounds = g_rounds;
    return SWOFTY_ASSET_OK;
}
static int team_stat(const char *ref, uint32_t *out)
{
    std::string r = ref;
    if (r == "game/ferrari_1990")
        *out = 11;
    else if (r == "game/lotus_1990")
        *out = 12;
    else
        return SWOFTY_ASSET_INVALID;
    return SWOFTY_ASSET_OK;
}
static int set_slots(const char *group, const uint32_t *stats, size_t n)
{
    g_group = group;
    g_slots.assign(stats, stats + n);
    return SWOFTY_ASSET_OK;
}
static int apply_lineup(SwoftyText request, char *out, size_t cap, size_t *len)
{
    g_request.assign(request.data, request.size);
    const char *r = "{\"result\":\"ok\",\"changed\":1,\"unchanged\":1,\"refused\":0}";
    *len = std::strlen(r);
    if (cap < *len + 1)
        return SWOFTY_TOO_SMALL;
    std::memcpy(out, r, *len + 1);
    return SWOFTY_ASSET_OK;
}

static const SwoftyCareerOps kOps6 = {6,       available, nullptr, set_slots, apply_lineup, nullptr, nullptr, nullptr,
                                      nullptr, nullptr,   nullptr, nullptr,   nullptr,      nullptr, nullptr, nullptr, nullptr};
static const SwoftyCareerOps kOps7 = {7,       available, nullptr, set_slots, apply_lineup, nullptr, nullptr, nullptr,
                                      nullptr, nullptr,   nullptr, nullptr,   nullptr,      nullptr, current_round, team_stat, nullptr};

static Ref<drivers::Identity> D(const char *name) { return Ref<drivers::Identity>(std::string("game/") + name); }

static career::Season season()
{
    career::Season s;
    s.data_group = "f1_career_1990";
    s.year = 1990;
    s.teams.push_back({.slot = 1, .team = {}, .seats = {D("prost"), D("mansell")}});
    s.teams.push_back({.slot = 2, .team = {}, .seats = {D("warwick"), D("donnelly")}});
    s.teams.push_back({.slot = 3, .team = {}, .seats = {D("brabham"), D("modena")}});
    s.teams.push_back({.slot = 4, .team = {}, .seats = {D("pirro"), D("de_cesaris")}});
    s.changes.push_back({.round = 15, .slot = 2, .seat = 1, .out = D("donnelly"), .in = D("herbert"),
                         .kind = career::ChangeKind::stand_in, .until_round = 17});
    s.changes.push_back({.round = 1, .slot = 3, .seat = 0, .out = D("brabham"), .in = D("foitek"),
                         .kind = career::ChangeKind::stand_in, .until_round = 3});
    s.changes.push_back({.round = 1, .slot = 4, .seat = 0, .out = D("pirro"), .in = D("morbidelli"),
                         .kind = career::ChangeKind::stand_in});
    return s;
}

static std::string seat(const career::Season &s, int slot, int k) { return std::string(s.teams.find(slot)->seats[k].id()); }

int main()
{
    CHECK(!career::current_round());
    career::detail::set_ops_table(&kOps6);
    CHECK(!career::current_round());
    CHECK(career::set_slots("g", std::vector<Ref<teams::Identity>>{}) == career::Result::unavailable);

    career::detail::set_ops_table(&kOps7);
    auto here = career::current_round();
    CHECK(here && here->round == 3 && here->rounds == 16 && here->season_index == 0);
    g_result = SWOFTY_ASSET_LATE;
    CHECK(!career::current_round());
    g_result = SWOFTY_ASSET_OK;

    CHECK(career::set_slots("f1_career_1990", std::vector<Ref<teams::Identity>>{Ref<teams::Identity>("game/ferrari_1990"),
                                                                              Ref<teams::Identity>("game/lotus_1990")}) ==
          career::Result::ok);
    CHECK(g_group == "f1_career_1990" && g_slots == (std::vector<uint32_t>{11, 12}));
    CHECK(career::set_slots("f1_career_1990", std::vector<Ref<teams::Identity>>{Ref<teams::Identity>("game/nobody")}) ==
          career::Result::invalid);

    career::Season base = season();
    career::Season r1 = career::seats_at(base, 1);
    CHECK(seat(r1, 3, 0) == "game/foitek" && seat(r1, 4, 0) == "game/morbidelli" && seat(r1, 2, 1) == "game/donnelly");
    career::Season r2 = career::seats_at(base, 2);
    CHECK(seat(r2, 3, 0) == "game/foitek" && seat(r2, 4, 0) == "game/pirro");
    career::Season r3 = career::seats_at(base, 3);
    CHECK(seat(r3, 3, 0) == "game/brabham" && seat(r3, 4, 0) == "game/pirro" && seat(r3, 3, 1) == "game/modena");
    CHECK(seat(career::seats_at(base, 14), 2, 1) == "game/donnelly");
    CHECK(seat(career::seats_at(base, 15), 2, 1) == "game/herbert");
    CHECK(seat(career::seats_at(base, 16), 2, 1) == "game/herbert");
    CHECK(base.changes.size() == 3 && career::seats_at(base, 1).changes.empty());

    career::Season rep = season();
    rep.changes.clear();
    rep.changes.push_back({.round = 5, .slot = 1, .seat = 1, .out = D("mansell"), .in = D("x"), .kind = career::ChangeKind::replace});
    CHECK(seat(career::seats_at(rep, 4), 1, 1) == "game/mansell" && seat(career::seats_at(rep, 16), 1, 1) == "game/x");

    career::Season sw = season();
    sw.changes.clear();
    sw.changes.push_back({.round = 2, .slot = 1, .seat = 0, .out = D("prost"), .in = D("warwick"), .kind = career::ChangeKind::swap});
    career::Season swapped = career::seats_at(sw, 2);
    CHECK(seat(swapped, 1, 0) == "game/warwick" && seat(swapped, 2, 0) == "game/prost");

    career::Season mv = season();
    mv.changes.clear();
    mv.changes.push_back({.round = 2, .slot = 1, .seat = 0, .out = D("prost"), .in = D("warwick"), .kind = career::ChangeKind::move});
    career::Season moved = career::seats_at(mv, 2);
    CHECK(seat(moved, 1, 0) == "game/warwick" && seat(moved, 2, 0).empty());

    career::ApplyReport report;
    CHECK(career::apply_round(base, 2, &report) == career::Result::ok && report.changed == 1 && report.unchanged == 1);
    CHECK(g_request.find("\"game/foitek\"") != std::string::npos && g_request.find("\"game/pirro\"") != std::string::npos &&
          g_request.find("morbidelli") == std::string::npos);

    career::detail::set_ops_table(nullptr);
    if (failures) {
        std::printf("sdk_ops_v7_test: %d failure(s)\n", failures);
        return 1;
    }
    std::printf("sdk_ops_v7_test: current_round, set_slots by reference, seats_at and apply_round: ok\n");
    return 0;
}
