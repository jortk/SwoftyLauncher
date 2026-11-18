#include "track_record_logic.hpp"

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
    runtime::track_record::Seen seen;
    CHECK(seen.first(1), "the first miss of a track is logged");
    CHECK(!seen.first(1), "the same track again is not logged");
    CHECK(seen.first(18), "another track is logged");
    CHECK(seen.first(255) && !seen.first(255), "track 255 is logged once");
    CHECK(seen.first(0), "track 0 is logged");

    using runtime::track_record::Round;
    using runtime::track_record::Verdict;
    using runtime::track_record::judge;
    Round ok;
    ok.track = 5;
    ok.table_known = true;
    ok.table_count = 20;
    ok.record = 0x1000;
    ok.record_readable = true;
    ok.node = 0x2000;
    ok.node_readable = true;
    CHECK(judge(ok) == Verdict::game, "a round with a readable record and node runs the game's code");

    Round null_record = ok;
    null_record.record = 0;
    null_record.record_readable = false;
    CHECK(judge(null_record) == Verdict::no_record, "a null record is no record");

    Round null_node = ok;
    null_node.node = 0;
    null_node.node_readable = false;
    CHECK(judge(null_node) == Verdict::game, "a null node is the game's own empty-node path");

    Round past = ok;
    past.track = 0x54;
    past.record = 0x1110000000;
    CHECK(judge(past) == Verdict::past_table, "a track id past the table is no record");
    past.track = 20;
    CHECK(judge(past) == Verdict::past_table, "the first id past the table is no record");
    past.track = 19;
    CHECK(judge(past) == Verdict::game, "the last id in the table runs the game's code");

    Round unknown = ok;
    unknown.table_known = false;
    unknown.track = 0x54;
    CHECK(judge(unknown) == Verdict::game, "an unreadable table does not condemn an otherwise good round");

    Round bad_record = ok;
    bad_record.record_readable = false;
    CHECK(judge(bad_record) == Verdict::bad_record, "an unreadable record is no record");

    Round bad_node = ok;
    bad_node.node = 0x1110000000;
    bad_node.node_readable = false;
    CHECK(judge(bad_node) == Verdict::bad_node, "a node pointer that cannot be read is no record");

    Round empty_table = ok;
    empty_table.table_count = 0;
    CHECK(judge(empty_table) == Verdict::past_table, "an empty table has no record for any track");
    return failures ? 1 : 0;
}
