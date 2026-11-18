#include "standings_drop_logic.hpp"

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

using namespace runtime::standings_drop;

static std::vector<PointsRound> season(const std::vector<int> &per_round, uint32_t key)
{
    std::vector<PointsRound> rounds;
    for (int p : per_round) {
        PointsSession practice{3, {{key, 0}}}, race{15, {{key, p}}};
        rounds.push_back({practice, race});
    }
    return rounds;
}

int main()
{
    CHECK(best_sum({1, 9, 4}, 2) == 13, "best_sum keeps the two best of three");
    CHECK(best_sum({1, 9, 4}, 11) == 14, "best_sum with fewer values than the limit sums them all");
    CHECK(best_sum({}, 11) == 0, "best_sum of nothing is 0");

    const uint32_t key = 0x00050001;
    std::vector<int> pts(16, 0);
    for (int i = 0; i < 16; i++)
        pts[i] = (i % 3 == 0) ? 9 : (i % 3 == 1 ? 6 : 0);
    auto rounds = season(pts, key);
    CHECK(counted_total(rounds, 10, 15, key, 11) == 9 * 4 + 6 * 4 + 0 * 3 + 0, "all of the first 11 rounds count");
    int64_t gross = 0;
    for (int p : pts)
        gross += p;
    int32_t counted = counted_total(rounds, 15, 15, key, 11);
    CHECK(gross == 9 * 6 + 6 * 5 && counted <= gross, "the 16 round total never exceeds the gross total");
    CHECK(counted == 9 * 6 + 6 * 5, "the five dropped results are the zeros, so no points are lost");

    std::vector<int> all_six(16, 6);
    int64_t six_gross = 0;
    for (int p : all_six)
        six_gross += p;
    CHECK(six_gross == 96 && counted_total(season(all_six, key), 15, 15, key, 11) < six_gross, "sixteen sixes drop five");
    CHECK(counted_total(season(all_six, key), 15, 15, key, 11) == 66, "sixteen sixes count as eleven");
    CHECK(counted_total(season(all_six, key), 11, 15, key, 11) == 66, "twelve sixes count as eleven");
    CHECK(counted_total(season(all_six, key), 10, 15, key, 11) == 66, "eleven sixes count all");
    CHECK(counted_total(season(all_six, key), 9, 15, key, 11) == 60, "ten sixes count all");

    std::vector<int> late(16, 0);
    late[0] = 1;
    late[1] = 1;
    late[2] = 1;
    late[3] = 1;
    late[4] = 1;
    late[5] = 1;
    for (int i = 6; i < 16; i++)
        late[i] = 9;
    CHECK(counted_total(season(late, key), 15, 15, key, 11) == 90 + 1, "the ten wins and the best of the six ones count");

    auto cut = season(all_six, key);
    CHECK(counted_total(cut, 0, 3, key, 11) == 0, "a session cutoff before the race leaves the round at 0");
    CHECK(counted_total(cut, 0, 15, key, 11) == 6, "the cutoff at the race includes it");

    CHECK(counted_total(rounds, 15, 15, 0x00990002, 11) == 0, "a driver with no records has 0");
    CHECK(same_key(0x00050001, 0x00050201), "the game's key match ignores bits 8 to 15");
    CHECK(!same_key(0x00050001, 0x00060001), "the key match compares the high half");
    return failures ? 1 : 0;
}
