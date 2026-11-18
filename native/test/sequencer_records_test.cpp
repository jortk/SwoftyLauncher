#include "sequencer_records_logic.hpp"

#include <cstdio>
#include <cstring>
#include <initializer_list>

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
    using namespace runtime::sequencer_records;
    CHECK(skips(0), "an empty record collection is skipped");
    CHECK(!skips(0x7ff600001000), "a collection with a first record runs the game's code");
    bool said = false;
    CHECK(says(0, said), "the first empty collection is logged");
    CHECK(!says(0, said), "a second empty collection is not logged");
    bool other = false;
    CHECK(!says(0x1000, other) && !other, "a collection with records is not logged");

    CHECK(skips(0), "a team mate with no driver record skips the morph data call");
    CHECK(!skips(0x3462d4f48), "a team mate with a driver record runs the game's morph data call");
    bool driver_said = false;
    CHECK(says(0, driver_said) && !says(0, driver_said), "a missing team mate driver is logged once");
    CHECK(driver_count(0x1000, 0x1000 + 25 * 0x10) == 25, "a driver table counts its 16-byte slots");
    CHECK(driver_count(0, 0) == 0, "an empty driver table counts no drivers");
    CHECK(driver_count(0x2000, 0x1000) == 0, "a driver table whose end is before its begin counts no drivers");
    CHECK(miss(3, 0x1000, 25, 2) == Miss::domain, "a domain above 2 names no driver");
    CHECK(miss(0xff, 0, 0, 0) == Miss::domain, "the domain test comes before the table test");
    CHECK(miss(1, 0, 0, 2) == Miss::no_table, "a domain with no driver table names no driver");
    CHECK(miss(0, 0x1000, 25, 25) == Miss::past_end, "a stat id equal to the table's count is past its end");
    CHECK(miss(0, 0x1000, 25, 0xffff) == Miss::past_end, "a large stat id is past the table's end");
    CHECK(miss(0, 0x1000, 0, 0) == Miss::past_end, "an empty table has no slot for stat id 0");
    CHECK(miss(2, 0x1000, 25, 24) == Miss::empty_slot, "a stat id inside the table names an empty slot");
    bool texts = true;
    for (Miss m : {Miss::unread, Miss::domain, Miss::no_table, Miss::past_end, Miss::empty_slot})
        texts = texts && miss_text(m) && *miss_text(m);
    CHECK(texts, "every miss has a log text");
    CHECK(miss_text(Miss::past_end) != miss_text(Miss::empty_slot), "the past-end and empty-slot texts differ");

    bool player_said = false;
    CHECK(says(0, player_said) && !says(0, player_said), "a missing player driver is logged once");
    bool mate_said = false;
    CHECK(says(0, mate_said), "the player's log does not use up the team mate's");
    CHECK(std::strcmp(who(Driver::player), "the player's own driver") == 0, "the player hook names the player");
    CHECK(std::strcmp(who(Driver::team_mate), "the team mate's driver") == 0, "the team mate hook names the team mate");

    bool recognition_said = false;
    CHECK(skips(0), "a recognition notification with an empty record collection leaves through the function's exit");
    CHECK(!skips(0x3410e9c80), "a recognition notification with a first record runs the game's code");
    CHECK(says(0, recognition_said) && !says(0, recognition_said), "an empty recognition collection is logged once");
    bool sequencer_said = false;
    CHECK(says(0, sequencer_said), "the recognition log does not use up the sequencer's");
    return failures ? 1 : 0;
}
