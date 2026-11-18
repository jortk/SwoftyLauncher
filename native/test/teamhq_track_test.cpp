#include "teamhq_track_logic.hpp"

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
    using namespace runtime::teamhq_track;
    CHECK(skips(Load::record, 0, 0), "a record load with no record is skipped");
    CHECK(!skips(Load::record, 0x1000, 0), "a record load with a record runs the game's code");
    CHECK(skips(Load::track, 0x1000, 0), "a track load with no track is skipped");
    CHECK(skips(Load::track, 0, 0), "a track load with no record is skipped");
    CHECK(!skips(Load::track, 0x1000, 0x2000), "a track load with a track runs the game's code");
    CHECK(classify(0, 0) == Miss::record && classify(0x1000, 0) == Miss::track && classify(0x1000, 0x2000) == Miss::none,
          "the miss is the record, then the track");

    runtime::track_record::Seen seen;
    CHECK(says(Load::track, 0x1000, 0, seen, 6), "the first track miss of track 6 is logged");
    CHECK(!says(Load::track, 0x1000, 0, seen, 6), "track 6 again is not logged");
    CHECK(!says(Load::track, 0, 0, seen, 9), "a track load after a record miss is not logged");
    CHECK(says(Load::record, 0, 0, seen, 1), "the first record miss of track 1 is logged");
    CHECK(!says(Load::record, 0, 0, seen, 1), "track 1 again is not logged");
    CHECK(!says(Load::record, 0x1000, 0, seen, 2), "a record that exists is not logged");
    CHECK(!says(Load::track, 0x1000, 0x2000, seen, 3), "a track that exists is not logged");

    runtime::track_record::Seen vm_seen;
    CHECK(says(Load::record, 0, 0, vm_seen, 6), "the view model site logs track 6 on its own once");
    CHECK(!says(Load::record, 0, 0, vm_seen, 6), "the view model site does not log track 6 twice");
    CHECK(!says(Load::record, 0x1000, 0, vm_seen, 7), "the view model site does not log a record that exists");
    return failures ? 1 : 0;
}
