// Host-native unit test for the core's My Team vehicle base guard (native/src/core/myteam_vehicle_guard_logic.hpp).
// Built and run by native/test/run_myteam_vehicle_guard_test.sh.

#include "myteam_vehicle_guard_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>
#include <string>

namespace mv = runtime::myteam_vehicle;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Fake {
    std::map<uint64_t, uint64_t> q;
    std::map<uint64_t, uint32_t> d;
    std::map<uint64_t, std::string> s;
    std::map<std::string, mv::Row> rows;
    std::string forced_label;

    bool forced(const std::string &label)
    {
        return !forced_label.empty() && label == forced_label;
    }

    mv::Row vehicle_row(const std::string &label)
    {
        if (forced(label))
            return mv::Row::missing;
        auto it = rows.find(label);
        return it == rows.end() ? mv::Row::missing : it->second;
    }

    bool u64(uint64_t a, uint64_t &out)
    {
        auto it = q.find(a);
        if (it == q.end())
            return false;
        out = it->second;
        return true;
    }

    bool u32(uint64_t a, uint32_t &out)
    {
        auto it = d.find(a);
        if (it == d.end())
            return false;
        out = it->second;
        return true;
    }

    bool i32(uint64_t a, int32_t &out)
    {
        uint32_t v = 0;
        if (!u32(a, v))
            return false;
        out = static_cast<int32_t>(v);
        return true;
    }

    bool text(uint64_t a, std::string &out)
    {
        auto it = s.find(a);
        if (it == s.end())
            return false;
        out = it->second;
        return true;
    }
};

constexpr uint64_t RSP = 0x66aeb630, ROW = 0x66aee438, CONTAINER = 0x335f2f938, LABEL = 0x66aebbb0;

static Fake frame(uint32_t row_index, uint64_t base, uint32_t saved_stat)
{
    Fake f;
    f.d[ROW + mv::ROW_INDEX] = row_index;
    f.q[RSP + mv::FRAME_CONTAINER] = CONTAINER;
    f.q[RSP + mv::FRAME_INDEX] = base;
    f.d[RSP + mv::FRAME_STAT_ID] = saved_stat;
    f.q[RSP + mv::FRAME_LABEL] = LABEL;
    f.s[LABEL] = "myteam_mes_porsche_2026";
    return f;
}

int main()
{
    expect(mv::row_missing(-1), "a row with index -1 (Gdb_Find_RowByLabel's miss) is missing");
    expect(!mv::row_missing(0), "row index 0 is a found row");
    expect(!mv::row_missing(1234), "a later row index is a found row");
    expect(mv::stat_id(0x4000) == 0 && mv::stat_id(0x4123) == 0x123, "the saved stat_id keeps its low 14 bits as the id");

    mv::Skip skip;
    Fake missing = frame(0xffffffffu, 0, 0x4000);
    expect(mv::skip_base(missing, RSP, ROW, skip), "4m7rul's frame (row 'Invalid', index -1) skips the base");
    expect(skip.container == CONTAINER && skip.index == 0, "the skip restores the loop's container and base index from the frame");
    expect(skip.stat_id == 0 && skip.label == "myteam_mes_porsche_2026", "the skip names the base's stat id and the missing vehicle row");

    Fake later = frame(0xffffffffu, 3, 0x4007);
    expect(mv::skip_base(later, RSP, ROW, skip) && skip.index == 3 && skip.stat_id == 7, "a later base keeps its own index and stat id");

    Fake found = frame(57, 0, 0x4000);
    expect(!mv::skip_base(found, RSP, ROW, skip), "a base whose vehicle row exists runs the game's code");

    Fake unread = frame(0xffffffffu, 0, 0x4000);
    unread.d.erase(ROW + mv::ROW_INDEX);
    expect(!mv::skip_base(unread, RSP, ROW, skip), "an unreadable row is left to the game");

    Fake no_container = frame(0xffffffffu, 0, 0x4000);
    no_container.q[RSP + mv::FRAME_CONTAINER] = 0;
    expect(!mv::skip_base(no_container, RSP, ROW, skip), "a frame without the list container is left to the game");

    Fake no_index = frame(0xffffffffu, 0, 0x4000);
    no_index.q.erase(RSP + mv::FRAME_INDEX);
    expect(!mv::skip_base(no_index, RSP, ROW, skip), "a frame without the base index is left to the game");

    Fake no_label = frame(0xffffffffu, 2, 0x4000);
    no_label.s.clear();
    expect(!mv::skip_base(no_label, RSP, ROW, skip) && skip.left && skip.label == "(unreadable)", "an unreadable label is left to the game");

    Fake empty_label = frame(0xffffffffu, 2, 0x4000);
    empty_label.s[LABEL] = "";
    expect(!mv::skip_base(empty_label, RSP, ROW, skip) && skip.left && skip.label == "(unreadable)", "an empty file_name is left to the game");

    Fake stock = frame(0xffffffffu, 0, 0x4000);
    stock.s[LABEL] = "myteam_mercedes";
    expect(!mv::skip_base(stock, RSP, ROW, skip) && skip.left && skip.label == "myteam_mercedes",
           "cosj_06's stock vehicle 'myteam_mercedes' with a missing-looking row is left to the game");

    Fake stock26 = frame(0xffffffffu, 1, 0x4000);
    stock26.s[LABEL] = "myteam_honda_2026";
    expect(!mv::skip_base(stock26, RSP, ROW, skip) && skip.left, "cosj_06's 'myteam_honda_2026' is left to the game");

    Fake forced = frame(0xffffffffu, 0, 0x4000);
    forced.s[LABEL] = "myteam_honda_2026";
    forced.rows["myteam_honda_2026"] = mv::Row::present;
    forced.forced_label = "myteam_honda_2026";
    expect(mv::skip_base(forced, RSP, ROW, skip) && skip.label == "myteam_honda_2026" && skip.stat_id == 0,
           "the keep test car (SWOFTY_TEST_MYTEAM_KEEP_LABEL) is handled like an addon car whose row is gone");

    Fake present_mes = frame(0xffffffffu, 8, 0x4000);
    present_mes.s[LABEL] = "myteam_mes_custom_2";
    present_mes.rows["myteam_mes_custom_2"] = mv::Row::present;
    expect(!mv::skip_base(present_mes, RSP, ROW, skip) && skip.left,
           "JamesF1's MES base whose entrants.vehicles row is in the game data is left to the game");

    Fake unknown_mes = frame(0xffffffffu, 8, 0x4000);
    unknown_mes.rows["myteam_mes_porsche_2026"] = mv::Row::unknown;
    expect(!mv::skip_base(unknown_mes, RSP, ROW, skip) && skip.left,
           "an MES base is left to the game while the database can't be read");

    Fake found_stock = frame(5, 0, 0x4000);
    found_stock.s[LABEL] = "myteam_mercedes";
    expect(!mv::skip_base(found_stock, RSP, ROW, skip) && !skip.left, "a found stock row is not reported as left");

    const mv::KnownAddon *mes = mv::addon_for("myteam_mes_porsche_2026");
    expect(mes && !std::strcmp(mes->name, "More Engine Suppliers") && !std::strcmp(mes->id, "mildtdesign.more-engine-suppliers"),
           "a myteam_mes_ vehicle is named as More Engine Suppliers");
    expect(!mv::addon_for("myteam_ferrari") && !mv::addon_for("mes_porsche") && !mv::addon_for(""),
           "other vehicles name no addon");

    mv::Labels labels;
    expect(labels.first("myteam_mes_porsche_2026"), "the first skip of a vehicle is logged");
    expect(!labels.first("myteam_mes_porsche_2026"), "a second skip of the same vehicle is not");
    expect(labels.first("myteam_mes_bmw_2026"), "another missing vehicle is logged");
    for (size_t i = labels.seen.size(); i < mv::LOG_LABELS; i++)
        labels.first("vehicle_" + std::to_string(i));
    expect(labels.last(), "the 32nd vehicle is the last logged");
    expect(!labels.first("vehicle_extra"), "vehicles past 32 are not logged");

    expect(mv::switch_off("0") && mv::switch_off("off") && !mv::switch_off("1") && !mv::switch_off(nullptr), "the switch reads 0 or off");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
