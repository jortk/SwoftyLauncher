// Host unit test for the era picker logic (era_logic.hpp): the box check, the default title and image,
// the box order, the Time Trial list and car limit, the press guard, the addon row's handle links, the DRS
// group swap and the restore stack of list cuts. run_events_test.sh builds and runs it.
#include "../src/core/era_logic.hpp"

#include <cstdio>
#include <string>
#include <vector>

namespace era = runtime::era;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static era::Box box(const char *list, const char *title = "", const char *image = "", int32_t order = 0)
{
    return era::Box{list, title, image, order, "test", 0, {}};
}

// A game setup mode object as Sess_Parse_GameSetupMode builds it: string fields are holders whose +0x10 is the label.
struct Mode {
    alignas(8) uint8_t mem[0x200] = {};
    era::DrsHolder own;
    explicit Mode(const char *group)
    {
        own.text = group;
        *reinterpret_cast<void **>(mem + era::MODE_DRS_GROUP) = &own;
    }
    void *field() const { return *reinterpret_cast<void *const *>(mem + era::MODE_DRS_GROUP); }
    std::string group() const { return era::holder_text(field()) ? era::holder_text(field()) : ""; }
};

static void drs_swap_tests()
{
    era::DrsSwaps swaps;
    Mode a("f1_2024"), b("f1_2024");
    era::DrsHolder *never = swaps.holder("swofty_drs_never_1a2b3c4d");
    expect(never && era::holder_text(never) == std::string("swofty_drs_never_1a2b3c4d") &&
               swaps.holder("swofty_drs_never_1a2b3c4d") == never,
           "one holder per label, at a stable address, its +0x10 the label");
    expect(!swaps.holder("") && !swaps.holder(std::string(64, 'x')), "an empty label or one past 63 characters has no holder");

    swaps.swap(a.mem, never);
    swaps.swap(b.mem, never);
    expect(a.group() == "swofty_drs_never_1a2b3c4d" && b.group() == "swofty_drs_never_1a2b3c4d" && swaps.active() == 2,
           "a press points every host mode's DRS group at the rule's group");

    era::DrsHolder *late = swaps.holder("swofty_drs_late_00ff00ff");
    swaps.swap(a.mem, late);
    expect(a.group() == "swofty_drs_late_00ff00ff" && swaps.active() == 2, "a second swap without a put back keeps the game's holder once");
    expect(swaps.put_back() == 2 && a.field() == &a.own && b.field() == &b.own && swaps.active() == 0,
           "put back restores the game's holder in every swapped mode");
    expect(swaps.put_back() == 0 && a.group() == "f1_2024", "a second put back changes nothing");

    swaps.swap(a.mem, never);
    era::DrsHolder other;
    other.text = "f2_2023";
    *reinterpret_cast<void **>(a.mem + era::MODE_DRS_GROUP) = &other;
    expect(swaps.put_back() == 0 && a.group() == "f2_2023", "put back leaves a field the game changed since the swap");

    swaps.swap(nullptr, never);
    swaps.swap(b.mem, nullptr);
    expect(swaps.active() == 0 && b.field() == &b.own, "no mode or no holder: nothing is swapped");
    expect(era::MODE_DRS_GROUP == 0x1e0 && era::MODE_NAME == 0xd8 && era::HOLDER_TEXT == 0x10,
           "GameSetupMode +0x1e0 DRS group and +0xd8 name holders, text at holder +0x10 (re/db structs/GameSetupMode.h)");
}

static void warmer_swap_tests()
{
    era::WarmerSwaps swaps;
    Mode a("f1_2024"), b("f1_2024");
    a.mem[era::MODE_TYRE_WARMERS] = 1;
    b.mem[era::MODE_TYRE_WARMERS] = 1;
    swaps.swap(a.mem, false);
    swaps.swap(b.mem, false);
    expect(a.mem[era::MODE_TYRE_WARMERS] == 0 && b.mem[era::MODE_TYRE_WARMERS] == 0 && swaps.active() == 2,
           "a press writes the box's tyre warmer rule into every host mode");
    swaps.swap(a.mem, true);
    expect(a.mem[era::MODE_TYRE_WARMERS] == 1 && swaps.active() == 2, "a second press without a put back keeps the game's value once");
    expect(swaps.put_back() == 2 && a.mem[era::MODE_TYRE_WARMERS] == 1 && b.mem[era::MODE_TYRE_WARMERS] == 1 && swaps.active() == 0,
           "put back restores the game's value in every swapped mode");

    a.mem[era::MODE_TYRE_WARMERS] = 0;
    swaps.swap(a.mem, true);
    a.mem[era::MODE_TYRE_WARMERS] = 0;                               // the game reloaded the mode since the press
    expect(swaps.put_back() == 0 && a.mem[era::MODE_TYRE_WARMERS] == 0, "put back leaves a byte the game changed since the swap");

    swaps.swap(a.mem, true);
    a.mem[era::MODE_TYRE_WARMERS] = 7;
    expect(swaps.forget() == 1 && swaps.active() == 0 && a.mem[era::MODE_TYRE_WARMERS] == 7 && swaps.put_back() == 0,
           "forget drops the swaps and writes nothing into the modes");
    swaps.swap(nullptr, true);
    expect(swaps.active() == 0, "no mode: nothing is swapped");
    expect(era::MODE_TYRE_WARMERS == 0x1c8 && era::MODE_TYRE_WARMERS < era::MODE_DRS_GROUP,
           "GameSetupMode +0x1c8 has_tyre_warmers (re/db structs/GameSetupMode.h)");
}

static void time_limit_swap_tests()
{
    era::TimeLimitSwaps swaps;
    Mode a("swofty_f1"), b("swofty_f1");
    auto lim = [](Mode &m) { return reinterpret_cast<uint32_t *>(m.mem + era::MODE_RACE_TIME_LIMIT); };
    for (int i = 0; i < 3; ++i)
        lim(a)[i] = lim(b)[i] = 7200;
    lim(a)[3] = 3600;                                                 // the sprint limit right behind (+0xc0)
    swaps.swap(a.mem, 10800);
    swaps.swap(b.mem, 10800);
    expect(lim(a)[0] == 10800 && lim(a)[1] == 10800 && lim(a)[2] == 10800 && lim(b)[2] == 10800 && lim(a)[3] == 3600 &&
               swaps.active() == 2,
           "a press writes the box's race time limit into the three values of every host mode, not the sprint limit");
    swaps.swap(a.mem, 5400);
    expect(lim(a)[0] == 5400 && swaps.active() == 2, "a second press without a put back keeps the game's values once");
    expect(swaps.put_back() == 2 && lim(a)[0] == 7200 && lim(a)[2] == 7200 && lim(b)[1] == 7200 && swaps.active() == 0,
           "put back restores the game's limits in every swapped mode");
    swaps.swap(a.mem, 10800);
    lim(a)[1] = 7200;                                                 // the game reloaded the mode since the press
    expect(swaps.put_back() == 0 && lim(a)[0] == 10800, "put back leaves values the game changed since the swap");
    swaps.swap(a.mem, 10800);
    expect(swaps.forget() == 1 && swaps.put_back() == 0 && lim(a)[0] == 10800, "forget writes nothing into the modes");
    swaps.swap(nullptr, 10800);
    expect(swaps.active() == 0, "no mode: nothing is swapped");
    expect(era::MODE_RACE_TIME_LIMIT == 0xb4 && era::MODE_RACE_TIME_LIMIT + 4 * era::RACE_TIME_LIMITS == 0xc0 &&
               era::MODE_RACE_TIME_LIMIT + 12 <= era::MODE_NAME,
           "GameSetupMode +0xb4 race_time_limit_in_seconds[3], sprint limits from +0xc0 (re/db structs/GameSetupMode.h)");
}

static void weekend_swap_tests()
{
    era::WeekendSwaps swaps;
    Mode host("swofty_f1"), one("swofty_f1_one_practice"), two("swofty_f1_one_shot");
    auto q = [](Mode &m) { return reinterpret_cast<uint64_t *>(m.mem + era::MODE_WEEKEND_OPTIONS); };
    for (uint64_t i = 0; i < 4; ++i) {
        q(host)[i] = 0x100 + i;
        q(one)[i] = 0x200 + i;
        q(two)[i] = 0x300 + i;
    }
    q(host)[4] = 0xaaaa;                                              // the field right behind (+0x1b0) stays put
    auto tag = [](Mode &m) { return reinterpret_cast<uint64_t *>(m.mem + era::MODE_OPTIONS_TAG); };
    *tag(host) = 0xf1, *tag(one) = 0xf50, *tag(two) = 0xf53;
    expect(swaps.swap(host.mem, one.mem) && q(host)[0] == 0x200 && q(host)[3] == 0x203 && q(one)[0] == 0x100 &&
               q(one)[3] == 0x103 && q(host)[4] == 0xaaaa && swaps.active() == 1,
           "a press exchanges the four weekend option qwords of the host and the helper mode");
    expect(*tag(host) == 0xf50 && *tag(one) == 0xf1, "a press exchanges the options tag (+0x00) too: the menu offers the helper's values");
    expect(swaps.swap(host.mem, one.mem) && q(host)[0] == 0x200 && q(one)[0] == 0x100,
           "pressing the same box again keeps the exchange (no double swap)");
    expect(swaps.swap(host.mem, two.mem) && q(host)[0] == 0x300 && q(two)[0] == 0x100 && q(one)[0] == 0x200 &&
               swaps.helper() == two.mem,
           "a box with another helper puts the first helper's container back before it exchanges");
    expect(swaps.put_back() == 1 && q(host)[0] == 0x100 && q(host)[3] == 0x103 && q(two)[0] == 0x300 && *tag(host) == 0xf1 &&
               *tag(two) == 0xf53 && *tag(one) == 0xf50 && swaps.active() == 0,
           "put back gives every mode its own container again");
    expect(swaps.put_back() == 0, "nothing held: put back writes nothing");

    swaps.swap(host.mem, one.mem);
    q(host)[2] = 0xdead;                                              // the game rebuilt the host's container
    expect(swaps.put_back() == 0 && q(host)[2] == 0xdead && q(one)[0] == 0x100,
           "put back leaves the modes alone when the game changed them since the swap");
    for (uint64_t i = 0; i < 4; ++i) {
        q(host)[i] = 0x100 + i;
        q(one)[i] = 0x200 + i;
    }

    swaps.swap(host.mem, one.mem);
    expect(swaps.forget() == 1 && swaps.put_back() == 0 && q(host)[0] == 0x200 && q(one)[0] == 0x100,
           "forget drops the pair without writing (each container is still owned by exactly one mode)");
    expect(!swaps.swap(host.mem, host.mem) && !swaps.swap(nullptr, one.mem) && !swaps.swap(host.mem, nullptr) &&
               swaps.active() == 0,
           "no host, no helper or the host itself: nothing is exchanged");
    expect(era::MODE_WEEKEND_OPTIONS == 0x190 && era::WEEKEND_SWAP_OFFSETS[era::WEEKEND_OPTION_QWORDS - 1] == 0x1a8 &&
               era::MODE_WEEKEND_OPTIONS + 0x20 <= era::MODE_TYRE_WARMERS && era::WEEKEND_SWAP_OFFSETS[0] == 0,
           "GameSetupMode +0x00 options tag and +0x190..+0x1a8 weekend options container (re/db structs/GameSetupMode.h)");
}

// The store of every parsed setup mode (g_Game_Scenarios +0x40): 16-byte entries {allocator, GameSetupMode *} by stat
// id, null where no row has the id. A weekend helper listed in no game mode is found here.
static void stored_mode_tests()
{
    struct Named {
        alignas(8) uint8_t mem[0x200] = {};
        era::DrsHolder label;
        explicit Named(const char *name)
        {
            label.text = name;
            *reinterpret_cast<void **>(mem + era::MODE_NAME) = &label;
        }
    };
    Named f1("f1"), host("swofty_f1"), helper("f50_wk"), longer("f50_wk_2");
    struct Entry {
        void *allocator;
        uint8_t *mode;
    };
    static_assert(sizeof(Entry) == era::STORE_SETUP_MODE_ENTRY && offsetof(Entry, mode) == era::STORE_SETUP_MODE_PTR);
    Entry entries[22] = {};
    entries[0].mode = f1.mem;
    entries[20].mode = host.mem;
    entries[21].mode = helper.mem;
    entries[7].mode = longer.mem;
    alignas(8) uint8_t registry[0x80] = {};
    auto set_range = [&](Entry *b, Entry *e) {
        *reinterpret_cast<Entry **>(registry + era::STORE_SETUP_MODES_BEGIN) = b;
        *reinterpret_cast<Entry **>(registry + era::STORE_SETUP_MODES_END) = e;
    };
    set_range(entries, entries + 22);
    auto all = [](const void *, size_t) { return true; };
    size_t seen = 0;
    expect(era::find_stored_mode(registry, "f50_wk", all, &seen) == helper.mem && seen == 4,
           "the helper mode (stat id 21, listed in no game mode) is found in the store; null ids are skipped");
    expect(era::find_stored_mode(registry, "swofty_f1", all) == host.mem, "the host mode is in the store too");
    std::vector<std::string> labels;
    expect(!era::find_stored_mode(registry, "f51_wk", all, &seen, &labels) && seen == 4 &&
               labels == std::vector<std::string>{"f1", "f50_wk_2", "swofty_f1", "f50_wk"},
           "a helper that is not stored: the labels of the stored modes are collected for the log");
    expect(!era::find_stored_mode(registry, "f50", all) && !era::find_stored_mode(registry, "f50_wk_", all),
           "a label must match whole, not as a prefix");
    expect(!era::find_stored_mode(registry, "", all) && !era::find_stored_mode(nullptr, "f50_wk", all),
           "no name or no registry: nothing found");
    expect(!era::find_stored_mode(registry, "f50_wk", [](const void *, size_t) { return false; }),
           "unreadable memory: nothing is read");
    set_range(entries + 22, entries);
    expect(!era::find_stored_mode(registry, "f50_wk", all), "a store whose end lies before its begin is not walked");
    set_range(entries, reinterpret_cast<Entry *>(reinterpret_cast<uint8_t *>(entries) + 24));
    expect(!era::find_stored_mode(registry, "f1", all), "a store that is not a whole number of entries is not walked");
    set_range(entries, entries + 22);
    expect(era::STORE_SETUP_MODES_BEGIN == 0x48 && era::STORE_SETUP_MODES_END == 0x50 && era::MODE_NAME == 0xd8,
           "Sess_Load_GameSetupModes (0x142a148f0) store at g_Game_Scenarios +0x40: begin +0x48, end +0x50");
}

// A parsed points level as the game holds it: rule byte, bonus, the points column (i32s at +0x08..+0x10).
struct Column {
    uint8_t mem[0x18] = {};
    std::vector<int32_t> v;
    explicit Column(std::vector<int32_t> p) : v(std::move(p)) { v.resize(26, 0); }
    uint8_t *link()                                  // after the column sits in its final place
    {
        int32_t *b = v.data(), *e = v.data() + v.size();
        std::memcpy(mem + era::COLUMN_BEGIN, &b, 8);
        std::memcpy(mem + era::COLUMN_END, &e, 8);
        return mem;
    }
};

struct Preset {
    std::vector<Column> cols;
    std::vector<uint8_t> levels;
    void fill(uint8_t *at, uint8_t rule, int32_t bonus)
    {
        levels.assign(cols.size() * era::LEVEL_SIZE, 0);
        for (size_t i = 0; i < cols.size(); i++) {
            uint8_t *l = levels.data() + i * era::LEVEL_SIZE;
            l[era::LEVEL_RULE] = rule;
            std::memcpy(l + era::LEVEL_BONUS, &bonus, 4);
            uint8_t *c = cols[i].link();
            std::memcpy(l + era::LEVEL_POINTS, &c, 8);
        }
        uint8_t *b = levels.data(), *e = levels.data() + levels.size();
        std::memcpy(at + era::PRESET_LEVELS_BEGIN, &b, 8);
        std::memcpy(at + era::PRESET_LEVELS_END, &e, 8);
    }
};

static void points_tests()
{
    era::Readable ok = [](const void *p, size_t) { return p != nullptr; };
    std::vector<uint8_t> presets(3 * era::PRESET_SIZE, 0);
    Preset stock{{Column({25, 18, 15, 12, 10, 8, 6, 4, 2, 1}), Column({19, 14, 12}), Column({13, 10, 8}), Column({6, 5, 4})}};
    Preset other{{Column({8, 6, 4, 3, 2}), Column({8, 6, 4, 3, 2}), Column({8, 6, 4, 3, 2}), Column({8, 6, 4, 3, 2})}};
    Preset era1950{{Column({8, 6, 4, 3, 2}), Column({8, 6, 4, 3, 2}), Column({8, 6, 4, 3, 2}), Column({8, 6, 4, 3, 2})}};
    stock.fill(presets.data(), 0, 0);
    other.fill(presets.data() + era::PRESET_SIZE, 0, 0);                          // same points, no fastest lap point
    era1950.fill(presets.data() + 2 * era::PRESET_SIZE, 1, 1);
    std::vector<int32_t> full = {8, 6, 4, 3, 2};
    full.resize(26, 0);
    era::PointsWant want{full, {full, full, full}, 1, 1};
    const uint8_t *found = era::find_points_preset(ok, presets.data(), presets.data() + presets.size(), want);
    expect(found == presets.data() + 2 * era::PRESET_SIZE, "the box's preset is found by its points, fastest lap rule and bonus");
    want.bonus = 2;
    expect(!era::find_points_preset(ok, presets.data(), presets.data() + presets.size(), want), "another bonus: no preset");
    want.bonus = 1;
    want.reduced[2][0] = 4;
    expect(!era::find_points_preset(ok, presets.data(), presets.data() + presets.size(), want), "other reduced points: no preset");
    want.reduced[2][0] = 8;
    expect(!era::find_points_preset(ok, presets.data(), presets.data() + presets.size() - 1, want) &&
               !era::find_points_preset(ok, nullptr, nullptr, want),
           "a preset vector that is not whole presets, or none: no preset");

    // two weekend structures, the second a sprint weekend; sessions: practice, race, race_2 without a preset
    std::vector<uint8_t> mode(0x200, 0), weekends(2 * era::WEEKEND_SIZE, 0), s0(3 * era::SESSION_SIZE, 0), s1(era::SESSION_SIZE, 0);
    auto session = [](std::vector<uint8_t> &v, size_t i, uint64_t preset, uint32_t id) {
        std::memcpy(v.data() + i * era::SESSION_SIZE, &preset, 8);
        std::memcpy(v.data() + i * era::SESSION_SIZE + era::SESSION_ID, &id, 4);
    };
    uint64_t stock_ptr = reinterpret_cast<uint64_t>(presets.data());
    session(s0, 0, stock_ptr, 1);
    session(s0, 1, stock_ptr, 15);
    session(s0, 2, 0, 16);
    session(s1, 0, stock_ptr, 15);
    auto span = [](uint8_t *at, size_t b, size_t e, std::vector<uint8_t> &v) {
        uint8_t *p = v.data(), *q = v.data() + v.size();
        std::memcpy(at + b, &p, 8);
        std::memcpy(at + e, &q, 8);
    };
    span(weekends.data(), era::WEEKEND_SESSIONS_BEGIN, era::WEEKEND_SESSIONS_END, s0);
    span(weekends.data() + era::WEEKEND_SIZE, era::WEEKEND_SESSIONS_BEGIN, era::WEEKEND_SESSIONS_END, s1);
    weekends[era::WEEKEND_SIZE + era::WEEKEND_SPRINT] = 1;
    span(mode.data(), era::MODE_WEEKENDS_BEGIN, era::MODE_WEEKENDS_END, weekends);
    auto races = era::race_sessions(ok, mode.data());
    expect(races.size() == 1 && races[0] == reinterpret_cast<uint64_t *>(s0.data() + era::SESSION_SIZE),
           "only the race of the non-sprint weekend that holds a preset is swapped");

    era::PointsSwaps swaps;
    uint64_t ours = reinterpret_cast<uint64_t>(found);
    for (uint64_t *s : races)
        swaps.swap(s, ours);
    expect(*races[0] == ours && swaps.active() == 1, "a press points the race at the box's preset");
    swaps.swap(races[0], ours);
    expect(swaps.put_back() == 1 && *races[0] == stock_ptr && swaps.active() == 0, "put back restores the game's preset once");
    swaps.swap(races[0], ours);
    *races[0] = 0x1234;                                                         // the game parsed the mode again
    expect(swaps.put_back() == 0 && *races[0] == 0x1234, "put back leaves a pointer the game changed since the swap");
    swaps.swap(races[0], ours);
    expect(swaps.forget() == 1 && *races[0] == ours && swaps.put_back() == 0, "forget writes nothing");
    expect(era::MODE_WEEKENDS_BEGIN == 0x198 && era::WEEKEND_SIZE == 0x88 && era::SESSION_RACE == 15,
           "GameSetupMode +0x190 weekend structures (re/db structs/GameSetupMode.h, WeekendStructure.h)");
}

static void tyre_change_tests()
{
    era::TyreChangeSwaps swaps;
    Mode a("f1_2024"), b("f1_2024");
    for (size_t i = 0; i < era::TYRE_REQUIREMENT_BYTES; i++) {
        a.mem[era::MODE_TYRE_REQUIREMENTS + i] = uint8_t(i % 3 == 0);
        b.mem[era::MODE_TYRE_REQUIREMENTS + i] = 1;
    }
    a.mem[era::MODE_TYRE_REQUIREMENTS - 1] = 7;                          // tyre_setup_enabled, next to the flags
    a.mem[era::MODE_TYRE_REQUIREMENTS + era::TYRE_REQUIREMENT_BYTES] = 9;
    auto all = [](Mode &m, uint8_t v) {
        for (size_t i = 0; i < era::TYRE_REQUIREMENT_BYTES; i++)
            if (m.mem[era::MODE_TYRE_REQUIREMENTS + i] != v)
                return false;
        return true;
    };
    swaps.swap(a.mem, false);
    swaps.swap(b.mem, false);
    expect(all(a, 0) && all(b, 0) && swaps.active() == 2 && a.mem[era::MODE_TYRE_REQUIREMENTS - 1] == 7 &&
               a.mem[era::MODE_TYRE_REQUIREMENTS + era::TYRE_REQUIREMENT_BYTES] == 9,
           "a press clears the 39 requirement flags of every host mode and nothing next to them");
    swaps.swap(a.mem, true);
    expect(all(a, 1) && swaps.active() == 2, "a second press keeps the game's flags once");
    expect(swaps.put_back() == 2 && a.mem[era::MODE_TYRE_REQUIREMENTS] == 1 && a.mem[era::MODE_TYRE_REQUIREMENTS + 1] == 0 &&
               all(b, 1) && swaps.active() == 0,
           "put back restores the game's flags in every swapped mode");
    swaps.swap(a.mem, false);
    a.mem[era::MODE_TYRE_REQUIREMENTS + 5] = 1;                          // the game reloaded the mode since the press
    expect(swaps.put_back() == 0 && a.mem[era::MODE_TYRE_REQUIREMENTS + 5] == 1, "put back leaves flags the game changed");
    swaps.swap(a.mem, true);
    expect(swaps.forget() == 1 && all(a, 1) && swaps.put_back() == 0, "forget writes nothing");
    swaps.swap(nullptr, true);
    expect(swaps.active() == 0, "no mode: nothing is swapped");
    expect(era::MODE_TYRE_REQUIREMENTS == 0x8b && era::MODE_TYRE_REQUIREMENTS + era::TYRE_REQUIREMENT_BYTES == 0xb2 &&
               era::MODE_TYRE_REQUIREMENTS + era::TYRE_REQUIREMENT_BYTES < era::MODE_TYRE_WARMERS,
           "GameSetupMode +0x8b..+0xb1 tyre regulation requirements (re/db structs/GameSetupMode.h)");
}

static void setup_guard_tests()
{
    era::DrsSwaps swaps;
    Mode a("f1_2024"), b("f1_2024");
    era::DrsHolder *never = swaps.holder("swofty_drs_never_1a2b3c4d");
    swaps.swap(a.mem, never);
    swaps.swap(b.mem, never);
    void *reused = reinterpret_cast<void *>(uintptr_t(0x5eed5eed5eed));
    *reinterpret_cast<void **>(a.mem + era::MODE_DRS_GROUP) = reused;
    *reinterpret_cast<void **>(b.mem + era::MODE_DRS_GROUP) = never;
    expect(swaps.forget() == 2 && swaps.active() == 0 && a.field() == reused && b.field() == never,
           "forget drops every swap and writes nothing into the modes, even one that still holds the rule's holder");
    expect(swaps.put_back() == 0 && b.field() == never, "a put back after forget has nothing to write back");
    expect(swaps.holder("swofty_drs_never_1a2b3c4d") == never, "forget keeps the holders at their addresses");
    swaps.swap(a.mem, never);
    expect(swaps.active() == 1 && a.field() == never, "a press after a rebuild swaps the new mode again");

    expect(era::availability_drop_line({}).empty(), "a rebuild with nothing kept logs nothing");
    std::string line = era::availability_drop_line({2, 3, true});
    expect(line.find("2 narrowed entrant setup mode(s)") != std::string::npos &&
               line.find("3 DRS group swap(s)") != std::string::npos && line.find("the Time Trial hold") != std::string::npos &&
               line.find("without writing them back") != std::string::npos,
           "the drop line names the narrowing, the DRS swaps and the Time Trial hold: " + line);
    std::string drs_only = era::availability_drop_line({0, 1, false});
    expect(drs_only.find("1 DRS group swap(s) without") != std::string::npos && drs_only.find("narrowed") == std::string::npos,
           "the drop line names only what was kept: " + drs_only);
}

// A VM node as the game constructs a reference node (0x14083c570): its own handle at +0x8 heads its holder list.
struct Node {
    alignas(8) uint8_t mem[0x30] = {};
    Node()
    {
        self() = {mem, nullptr, nullptr};
        holders() = &self();
        refs() = 2;
    }
    era::Handle &self() { return *reinterpret_cast<era::Handle *>(mem + 8); }
    void *&holders() { return *reinterpret_cast<void **>(mem + era::NODE_HOLDERS); }
    int16_t &refs() { return *reinterpret_cast<int16_t *>(mem + era::NODE_REFS); }
};

static Node &node_of(uint8_t *mem)
{
    return *reinterpret_cast<Node *>(mem);
}

struct Set {
    era::Handle list, selected;
};

// The handle part of FeTile_Clear_EraTileSet_Thunk (0x14bd31910): unlink from the holder list, release.
static void game_unlink(era::Handle &h)
{
    if (!h.node)
        return;
    auto *next = static_cast<era::Handle *>(h.next), *prev = static_cast<era::Handle *>(h.prev);
    if (prev)
        prev->next = next;
    if (next)
        next->prev = prev;
    if (node_of(h.node).holders() == &h)
        node_of(h.node).holders() = next;
    node_of(h.node).refs()--;
    h = {};
}

// The holder walk of the game's node teardown (0x1442b3e30): every holder but the node's own is cut loose
// and releases the node it holds.
static void game_teardown(Node &n)
{
    for (void *p = n.holders(); p;) {
        auto *h = static_cast<era::Handle *>(p);
        p = h->next;
        h->next = h->prev = nullptr;
        if (h == &n.self() || !h->node)
            continue;
        if (node_of(h->node).holders() == h)
            node_of(h->node).holders() = nullptr;
        node_of(h->node).refs()--;
        h->node = nullptr;
    }
}

static std::vector<era::Handle *> chain(Node &n)
{
    std::vector<era::Handle *> out;
    for (void *p = n.holders(); p && out.size() < 16; p = static_cast<era::Handle *>(p)->next)
        out.push_back(static_cast<era::Handle *>(p));
    return out;
}

static bool consistent(Node &n)
{
    void *prev = nullptr;
    for (era::Handle *h : chain(n)) {
        if (h->prev != prev || h->node != n.mem)
            return false;
        prev = h;
    }
    return chain(n).size() < 16;
}

static bool holds(Node &n, const era::Handle &h)
{
    for (era::Handle *p : chain(n))
        if (p == &h)
            return true;
    return false;
}

static void continue_tests()
{
    std::vector<uint16_t> saved{518, 517, 514, 520, 512, 519, 518, 513, 513, 516, 520, 511, 510, 519, 514, 515, 515, 511, 517,
                                512, 516, 510};
    std::vector<era::ListTeams> lists{
        {"f1_2025", {0, 1, 2, 3, 4, 5, 6, 7, 8, 9}},
        {"cs_f1_2017", {521, 522, 523, 524, 525, 526, 527, 528, 529, 530}},
        {"cs_f1_2016", {510, 511, 512, 513, 514, 515, 516, 517, 518, 519, 520, era::NO_TEAM}},
    };
    era::ContinuePick p = era::continue_list(lists, saved, "", "swofty_f1");
    expect(p.index == 2 && p.line.find("loading cs_f1_2016, which holds all 11 teams") != std::string::npos,
           "Continue with an empty list loads the list holding every team of the saved entrants: " + p.line);
    p = era::continue_list(lists, saved, "cs_f1_2017", "swofty_f1");
    expect(p.index == 2, "the teams win over a pressed box whose list doesn't hold them");
    std::vector<era::ListTeams> twice{lists[2], {"cs_f1_2016_copy", lists[2].teams}};
    p = era::continue_list(twice, saved, "cs_f1_2016_copy", "swofty_f1");
    expect(p.index == 1, "of two lists holding the teams, the pressed box's is loaded");
    std::vector<uint16_t> moved{646, 652, 559, 572, 600, 601, 602, 603, 604, 605, 606};
    p = era::continue_list(lists, moved, "cs_f1_2017", "swofty_f1");
    expect(p.index == 1 && p.line.find("the box pressed last") != std::string::npos,
           "teams no list holds fall back to the pressed box's list: " + p.line);
    p = era::continue_list(lists, moved, "", "swofty_f1");
    expect(p.index < 0 && p.line.find("no box was pressed") != std::string::npos, "nothing to load is said: " + p.line);
    std::string stop = era::continue_stop("", p, true);
    expect(stop.find("Grand Prix load stopped: ") == 0 && stop.find("646") != std::string::npos &&
               stop.find("start a new Grand Prix") != std::string::npos,
           "no list and no press stops the load with the teams it looked for: " + stop);
    expect(era::continue_stop("", p, false).empty(), "a list the game set is never stopped for");
    p = era::continue_list(lists, {}, "", "swofty_f1");
    expect(p.index < 0, "no saved entrants match no list");
    p = era::continue_list(lists, {era::NO_TEAM, 0}, "", "f1");
    expect(p.index < 0, "unresolved team ids match no list");

    std::vector<std::string> names{"f1_2025", "cs_f1_2016", "f1_career_progressive"};
    era::CareerListPick c = era::career_list(names, "f1_career_progressive", {}, "f1_26");
    expect(c.index == 2 && c.line.find("named like the career's data group") != std::string::npos,
           "a Driver Career with an empty list loads the list named like its data group: " + c.line);
    c = era::career_list(names, "other_group", {"cs_f1_2016"}, "f1_26");
    expect(c.index == 1 && c.line.find("content the career runs") != std::string::npos,
           "without a list for the data group, the list of the career's content is loaded: " + c.line);
    c = era::career_list(names, "", {"nothing"}, "f1_26");
    expect(c.index < 0 && c.line.find("(none)") != std::string::npos, "no data group and no matching content picks nothing");
    c = era::career_list({}, "f1_career_progressive", {}, "");
    expect(c.index < 0 && c.line.find("offers 0 lists") != std::string::npos && c.line.find("setup mode ?") != std::string::npos,
           "a setup mode without lists picks nothing: " + c.line);
    expect(era::career_stop(c).find("Driver Career load stopped: ") == 0 &&
               era::career_stop(c).find("instead of crashing") != std::string::npos,
           "no list stops the load with the reason");

    era::IdAliases aliases;
    std::string doc = R"({"version":1,"tables":{"teams":{"646":510,"652":511,"559":512,"3":9,"0":4,"99999":5},"vehicles":{"700":701}}})";
    expect(era::parse_aliases(doc, aliases), "the alias file parses");
    expect(aliases.teams.size() == 4 && !aliases.teams.count(0) && !aliases.teams.count(uint16_t(99999)) && aliases.teams.at(646) == 510 && aliases.vehicles.at(700) == 701,
           "ids outside the saved range or zero are left out");
    expect(!era::parse_aliases("{\"version\":2}", aliases) && !era::parse_aliases("nope", aliases),
           "an unknown version or text is refused");
    std::vector<uint16_t> hashed{646, 652, 559, 646, 652};
    era::AliasedPick ap = era::continue_list_aliased(lists, hashed, "", "swofty_f1", aliases);
    expect(ap.remapped && ap.pick.index == 2 && ap.teams == std::vector<uint16_t>({510, 511, 512, 510, 511}) &&
               ap.pick.line.find("after mapping the saved team ids 559,646,652 to 510,511,512") != std::string::npos,
           "saved hashed teams map to the current ones and a list holds them: " + ap.pick.line);
    ap = era::continue_list_aliased(lists, saved, "", "swofty_f1", aliases);
    expect(!ap.remapped && ap.pick.index == 2 && ap.teams == saved, "teams a list already holds are not mapped");
    ap = era::continue_list_aliased(lists, hashed, "", "swofty_f1", era::IdAliases{});
    expect(!ap.remapped && ap.pick.index < 0, "without aliases the teams stay unmatched");
    expect(era::saved_predates(aliases, {9, 646, 4}) && !era::saved_predates(aliases, {9, 4, 510}) &&
               !era::saved_predates(aliases, {0, era::NO_TEAM}) && !era::saved_predates(era::IdAliases{}, hashed),
           "a career vector is mapped only when one of its teams is an earlier id");
    expect(era::saved_predates(aliases, {9, 4}, {700, 5}, {}) && !era::saved_predates(aliases, {9, 4}, {701, 5}, {}) &&
               !era::saved_predates(aliases, {9, 4}, {era::NO_TEAM, 0}, {era::NO_TEAM}),
           "a career vector whose teams kept their ids is mapped when one of its vehicles is an earlier id");
    std::vector<uint16_t> stray{646, 4000};
    ap = era::continue_list_aliased(lists, stray, "cs_f1_2017", "swofty_f1", aliases);
    expect(!ap.remapped && ap.pick.index == 1, "a mapping no list holds falls back to the pressed box as before");

    namespace trk = swofty_core::apply::tracks;
    trk::Base base;
    expect(era::track_missing(base, "game/bahrain24").empty(), "a base not built yet stops nothing");
    base.ready = true;
    trk::BaseTrack bahrain;
    bahrain.id = "game/bahrain";
    bahrain.code = "bahrain";
    bahrain.stat_id = 3;
    trk::BaseTrack hockenheim;
    hockenheim.id = "mod.hockenheim/hockenheim";
    hockenheim.code = "hockenheim";
    hockenheim.stat_id = 43;
    hockenheim.kind = trk::Kind::older;
    base.tracks = {bahrain, hockenheim};
    base.dormant = {"game/bahrain24", "game/sochi"};
    expect(era::track_missing(base, "game/bahrain").empty(), "a stock track of the base loads");
    expect(era::track_missing(base, "game/hockenheim").empty(), "a track made for 1.5 found by its code loads");
    expect(era::track_missing(base, "mod.hockenheim/hockenheim").empty(), "a track found by its id loads");
    expect(era::track_missing(base, "").empty(), "an unread track stops nothing");
    std::string gone = era::track_missing(base, "game/bahrain24");
    expect(gone.find("game/bahrain24 is not a track of this build") != std::string::npos &&
               gone.find("dormant row with no track archive") != std::string::npos,
           "a saved weekend at a dormant row whose addon is off is named: " + gone);
    trk::BaseTrack sepang;
    sepang.id = "mod.bahrain24/bahrain24";
    sepang.code = "bahrain24";
    sepang.kind = trk::Kind::record;
    base.tracks.push_back(sepang);
    expect(era::track_missing(base, "game/bahrain24").empty(), "the same row filled by its track record loads");
    base.tracks.pop_back();
    trk::Base none;
    none.ready = true;
    expect(era::track_missing(none, "game/melbourne").empty() && era::track_missing(none, "game/silverstone").empty(),
           "a stock track loads when the base holds no tracks (no track addon on)");
    expect(!era::track_missing(none, "game/bahrain24").empty(), "a dormant row still stops with no track addon on");
    expect(!era::track_missing(none, "mod.hockenheim/hockenheim").empty(), "an addon track whose addon is off still stops");
    expect(era::track_missing(base, "game/melbourne").empty(), "a stock track missing from a built base loads");
    era::ContinuePick fine;
    fine.index = 0;
    stop = era::continue_stop(gone, fine, false);
    expect(stop.find("Grand Prix load stopped: the saved weekend's track game/bahrain24") == 0,
           "a missing track stops the load even when the list is set: " + stop);
    stop = era::continue_stop(gone, p, true);
    expect(stop.find("bahrain24") != std::string::npos && stop.find("no list holds") != std::string::npos,
           "both reasons are named together: " + stop);
}

struct GridSetup {
    alignas(8) uint8_t mem[0x300] = {};
    explicit GridSetup(int32_t limit) { set(limit); }
    void set(int32_t v) { *reinterpret_cast<int32_t *>(mem + era::ENTRANT_SETUP_MAX_ENTRANTS) = v; }
    int32_t get() const { return *reinterpret_cast<const int32_t *>(mem + era::ENTRANT_SETUP_MAX_ENTRANTS); }
};

// An entrant setup mode's list array (the lists at +0x130, their count at +0x230, max_entrants at +0x268) over 16 fake
// list objects, laid out like swofty_f1's Time Trial mode in the illanori bundle: the 24-car bdy_s5_2010 first.
struct Esm {
    alignas(8) uint8_t mem[0x300] = {};
    int objects[era::ESM_LIST_SLOTS] = {};
    std::vector<void *> original;
    explicit Esm(size_t n, int32_t max_entrants = 24)
    {
        for (size_t i = 0; i < n; ++i)
            original.push_back(&objects[i]);
        reset();
        limit() = max_entrants;
    }
    void reset()
    {
        memcpy(era::esm_lists(mem), original.data(), original.size() * sizeof(void *));
        era::esm_list_count(mem) = int64_t(original.size());
    }
    void *list(size_t i) { return &objects[i]; }
    int32_t &limit() { return *reinterpret_cast<int32_t *>(mem + era::ENTRANT_SETUP_MAX_ENTRANTS); }
    std::vector<void *> now()
    {
        int64_t n = era::esm_list_count(mem);
        void **arr = era::esm_lists(mem);
        return std::vector<void *>(arr, arr + std::max<int64_t>(n, 0));
    }
    bool whole() { return now() == original; }
};

struct TtLoad {
    era::ListCut stack[16];
    size_t used = 0;
    era::TtHold held;
    bool holding = false;
    std::vector<const era::ListCut *> failed;
    auto on_failed()
    {
        return [this](const era::ListCut &n) { failed.push_back(&n); };
    }
    void load(Esm &esm, size_t fit_slot, const std::vector<size_t> &cut)
    {
        held = {};
        held.esm = esm.mem;
        era::swap_into_first(held, esm.mem, fit_slot);
        held.limit_was = esm.limit();
        esm.limit() = era::time_trial_limit(esm.limit());
        holding = true;
        for (size_t i : cut)
            era::cut_list(stack, used, std::size(stack), esm.mem, esm.list(i));
    }
};

static void cut_restore_tests()
{
    {
        Esm esm(16);
        TtLoad tt;
        tt.load(esm, 5, {7, 9, 12, 14});
        expect(esm.now().size() == 12 && esm.now()[0] == esm.list(5) && tt.used == 1 && esm.limit() == 23,
               "the 2010 load swaps the Time Trial list into slot 0, cuts 4 lists after it and lowers max_entrants");
        era::CutsPutBack r = era::end_hold(tt.stack, tt.used, tt.held, tt.holding, true, tt.on_failed());
        expect(r.put == 1 && r.failed == 0 && tt.failed.empty() && esm.whole() && esm.limit() == 24 && tt.used == 0 &&
                   !tt.holding,
               "the unload puts the cut back before the swap: all 16 lists are back in their order, max_entrants 24");
    }
    {
        Esm esm(16);
        TtLoad tt;
        tt.load(esm, 5, {7, 9, 12, 14});
        era::put_back_hold(tt.held);
        tt.holding = false;
        era::CutsPutBack r = era::put_back_cuts(tt.stack, tt.used, true, tt.on_failed());
        auto out = tt.failed.empty() ? std::vector<void *>{} : era::left_out(*tt.failed[0]);
        expect(r.put == 0 && r.failed == 1 && esm.now().size() == 12 && tt.used == 0,
               "the old order (swap first, then the cut) changes the array, so the cut is not put back and not counted");
        expect(out == std::vector<void *>{esm.list(7), esm.list(9), esm.list(12), esm.list(14)},
               "the failed entry names the 4 lists it left out, in the mode's order");
    }
    {
        Esm esm(16);
        TtLoad tt;
        tt.load(esm, 5, {1, 2, 9});
        era::CutsPutBack r = era::end_hold(tt.stack, tt.used, tt.held, tt.holding, true, tt.on_failed());
        expect(r.put == 1 && r.failed == 0 && esm.whole(),
               "a cut before the swapped slot shifts it, and the cut-first order still puts every list back unswapped");
    }
    {
        Esm esm(16);
        TtLoad tt;
        tt.load(esm, 5, {1, 2, 9});
        era::put_back_hold(tt.held);
        era::put_back_cuts(tt.stack, tt.used, true, tt.on_failed());
        expect(esm.now().size() == 16 && esm.now()[0] == esm.list(5) && !esm.whole(),
               "the old order with a cut before the swapped slot leaves the Time Trial list stuck in slot 0");
    }
    {
        Esm esm(16);
        TtLoad tt;
        for (size_t i : {3, 4})
            era::cut_list(tt.stack, tt.used, std::size(tt.stack), esm.mem, esm.list(i));
        expect(tt.used == 1 && tt.stack[0].kept == 14 && tt.stack[0].count == 16,
               "a second cut of the same mode extends its entry instead of pushing another");
        tt.load(esm, 6, {10, 11});
        expect(tt.used == 1 && tt.stack[0].kept == 12, "the load's cut extends the entry of a stock press's keep");
        era::CutsPutBack r = era::end_hold(tt.stack, tt.used, tt.held, tt.holding, true, tt.on_failed());
        expect(r.put == 1 && esm.whole() && esm.limit() == 24,
               "the entry made before the swap puts the whole array back; the swap then finds nothing to undo");
    }
    {
        Esm host(16), other(8);
        TtLoad tt;
        tt.load(host, 5, {7, 9});
        era::cut_list(tt.stack, tt.used, std::size(tt.stack), other.mem, other.list(2));
        era::ListCut &narrowed = tt.stack[tt.used++];
        narrowed = {};
        Esm press(4);
        narrowed.setup = press.mem;
        narrowed.count = 4;
        narrowed.kept = 1;
        memcpy(narrowed.lists, press.original.data(), 4 * sizeof(void *));
        narrowed.kept_lists[0] = press.list(2);
        era::esm_lists(press.mem)[0] = press.list(2);
        era::esm_list_count(press.mem) = 1;
        era::esm_lists(other.mem)[0] = other.list(7);
        era::CutsPutBack r = era::end_hold(tt.stack, tt.used, tt.held, tt.holding, true, tt.on_failed());
        expect(r.put == 1 && r.failed == 1 && tt.failed.size() == 1 && host.whole() && !other.whole(),
               "a mode the game changed since is not overwritten: 1 of 2 Time Trial entries put back, 1 counted failed");
        expect(tt.used == 1 && tt.stack[0].setup == press.mem && era::esm_list_count(press.mem) == 1,
               "the unload leaves a box press's narrowing on the stack and in its mode");
        era::put_back_cuts(tt.stack, tt.used, false, tt.on_failed());
        expect(tt.used == 0 && press.whole(), "a restore of every entry puts the narrowing back too");
    }
    {
        Esm esm(16);
        TtLoad tt;
        era::cut_list(tt.stack, tt.used, std::size(tt.stack), esm.mem, esm.list(3));
        era::CutsPutBack r = era::end_hold(tt.stack, tt.used, tt.held, tt.holding, false, tt.on_failed());
        expect(r.put == 0 && tt.used == 1 && esm.now().size() == 15,
               "a press without a held swap leaves the Time Trial cuts for the unload");
        tt.used = 0;
        esm.reset();
        tt.load(esm, 5, {8});
        r = era::end_hold(tt.stack, tt.used, tt.held, tt.holding, false, tt.on_failed());
        expect(r.put == 1 && tt.used == 0 && esm.whole() && !tt.holding,
               "a press or a load that ends a held swap puts that load's cut back first");
    }
    {
        Esm esm(4);
        era::ListCut full[2];
        size_t used = 2;
        full[0].setup = full[1].setup = nullptr;
        expect(!era::cut_list(full, used, 2, esm.mem, esm.list(1)) && esm.whole() && used == 2,
               "with the restore stack full a cut is not made");
    }
    std::string line = era::put_back_failed_line({"bdx_s5_2012", "bdx_s5_2012_tt", "ms_f1_2014", "f1_2022_supercar"}, 12, 12, true);
    expect(line.find("Time Trial guard") != std::string::npos &&
               line.find("bdx_s5_2012, bdx_s5_2012_tt, ms_f1_2014, f1_2022_supercar stay out") != std::string::npos &&
               line.find("12 now, 12 left") != std::string::npos,
           "a failed put-back names the lists that stay out (" + line + ")");
}

static void grid_limit_tests()
{
    GridSetup press(20);
    era::GridLimit pressed;
    std::string line = pressed.raise(press.mem, 22);
    expect(press.get() == 22 && !line.empty() && line.find("20 -> 22") != std::string::npos &&
               line.find("clamped") == std::string::npos,
           "a box press on a 22-car list raises max_entrants from 20 to 22");
    pressed.restore();
    expect(press.get() == 20, "the put-back restores max_entrants to 20");
    pressed.restore();
    expect(press.get() == 20, "a second put-back changes nothing");

    GridSetup small(20);
    era::GridLimit none;
    expect(none.raise(small.mem, 18).empty() && small.get() == 20, "a list within the limit leaves max_entrants alone");
    expect(none.raise(small.mem, 20).empty() && small.get() == 20, "a list equal to the limit leaves max_entrants alone");
    expect(none.raise(small.mem, 0).empty() && none.raise(nullptr, 24).empty(), "an unread car count or setup mode changes nothing");
    GridSetup unset(0);
    expect(none.raise(unset.mem, 24).empty() && unset.get() == 0, "a max_entrants of 0 is left alone");

    GridSetup clamp(20);
    era::GridLimit clamped;
    line = clamped.raise(clamp.mem, 40);
    expect(clamp.get() == 22 && line.find("clamped to 22 cars") != std::string::npos,
           "a 40-car list clamps max_entrants to the stock 22 cars and says so");
    clamped.restore();
    expect(clamp.get() == 20, "the clamped limit is restored too");
    GridSetup at_table(26);
    expect(none.raise(at_table.mem, 40).empty() && at_table.get() == 26, "a limit already at the table size is not lowered or logged");

    GridSetup moved(20);
    era::GridLimit stale;
    stale.raise(moved.mem, 21);
    moved.set(22);
    stale.restore();
    expect(moved.get() == 22, "a limit someone else changed since the raise is not overwritten on put-back");

    GridSetup career(20);
    era::GridLimit box, cont;
    box.raise(career.mem, 22);
    expect(cont.raise(career.mem, 22).empty() && career.get() == 22, "the Continue path finds a limit a box press already raised");
    cont.restore();
    expect(career.get() == 22, "...and its put-back leaves the press's raise to the press's put-back");
    box.restore();
    expect(career.get() == 20, "the press's put-back restores it");

    GridSetup resume(20);
    era::GridLimit load;
    line = load.raise(resume.mem, 22);
    expect(resume.get() == 22 && line.find("clamped") == std::string::npos,
           "the Continue path (no box press) raises max_entrants to a 22-car list's count");
    line = load.raise(resume.mem, 24);
    expect(resume.get() == 22 && line.find("clamped to 22 cars") != std::string::npos,
           "a 24-car list is clamped to the stock 22 cars");
    line = load.raise(resume.mem, 21);
    expect(resume.get() == 21 && line.find("20 -> 21") != std::string::npos, "a new raise first puts the earlier one back");
    load.restore();
    expect(resume.get() == 20, "the Continue path's put-back restores max_entrants to 20");
}

static void front_end_leak_tests()
{
    struct Pick {
        bool narrowed = false;
        std::string last_pick, selected;
    };
    auto press = [](Pick &p, const char *list) {
        p.narrowed = true;
        p.last_pick = list;
        p.selected = list;
    };
    auto load = [](Pick &p, const char *mode) {
        era::FrontEndLeave l = era::leaving_front_end(mode, !p.selected.empty());
        if (l.restore)
            p.narrowed = false;
        if (l.clear_selected)
            p.selected.clear();
        if (l.forget_pick)
            p.last_pick.clear();
    };
    auto clean = [](const Pick &p) { return !p.narrowed && p.last_pick.empty() && p.selected.empty(); };

    Pick gp;
    press(gp, "cs_f1_2023");
    load(gp, "grand_prix_23");
    expect(gp.narrowed && gp.last_pick == "cs_f1_2023" && gp.selected == "cs_f1_2023",
           "a Grand Prix session load keeps the box pick");
    load(gp, "driver_career_progressive");
    expect(clean(gp), "GP box press then career Continue: narrowing, last pick and selected list are put back");

    Pick tt;
    press(tt, "bdy_s5_2010_tt");
    load(tt, "time_trial");
    expect(tt.narrowed && tt.selected == "bdy_s5_2010_tt", "a Time Trial session load keeps the box pick");
    load(tt, "driver_career_progressive");
    expect(clean(tt), "TT box press then career New: everything is put back");

    Pick two;
    press(two, "ms_f1_2015");
    press(two, "bdy_s5_2010");
    load(two, "driver_career_progressive");
    expect(clean(two), "two box presses then a career load: nothing of either survives");

    Pick none;
    era::FrontEndLeave l = era::leaving_front_end("my_team_career_25", false);
    expect(l.restore && !l.clear_selected && l.forget_pick, "a career load with no box list selected leaves the selection alone");
    expect(!era::leaving_front_end("grand_prix_splitscreen", true).restore && !era::leaving_front_end("solo_testing", true).restore,
           "Grand Prix and Time Trial front ends are not left");
}

int main()
{
    std::vector<era::Box> have;

    era::Box a = box("cs_f1_2016", "lng_cs_f1_2016", "", 2016);
    expect(era::check(a, have).empty(), "a box with a list, a title and an order is accepted");
    expect(era::image_of(a) == "#overrides.home_screen.tiles_solo_6x6[cs_f1_2016]", "an empty image is the list's tiles_solo_6x6 entry");
    a.seq = have.size();
    have.push_back(a);

    era::Box untitled = box("cs_f1_2017");
    expect(era::check(untitled, have).empty() && untitled.title == "lng_cs_f1_2017", "an empty title is lng_<entrant list>");

    era::Box dup = box("cs_f1_2016", "lng_other");
    expect(!era::check(dup, have).empty(), "a second box for the same entrant list is refused");

    for (const char *bad : {"", "has space", "dots.in.name", "slash/name", "0123456789012345678901234567890123456789012345678901234567890123"}) {
        era::Box b = box(bad);
        expect(!era::check(b, have).empty(), std::string("entrant list '") + bad + "' is refused");
    }

    era::Box image = box("cs_f1_2018", "lng_cs_f1_2018", "#overrides.home_screen.tiles_solo_6x6[f1]");
    expect(era::check(image, have).empty() && era::image_of(image) == "#overrides.home_screen.tiles_solo_6x6[f1]",
           "a given image is kept");

    era::Box nul = box("cs_f1_2019");
    nul.title = std::string("lng_a\0b", 7);
    expect(!era::check(nul, have).empty(), "a title with a NUL byte is refused");

    std::vector<era::Box> full;
    for (size_t i = 0; i < era::MAX_BOXES; i++)
        full.push_back(box(("l" + std::to_string(i)).c_str()));
    era::Box over = box("one_more");
    expect(!era::check(over, full).empty(), "a box past the 255th is refused");

    std::vector<era::Box> mixed = {box("y2018", "", "", 2018), box("y2016", "", "", 2016), box("late_a"), box("y2017", "", "", 2017),
                                   box("late_b")};
    for (size_t i = 0; i < mixed.size(); i++)
        mixed[i].seq = i;
    std::vector<era::Box> sorted = era::ordered(mixed);
    std::string got;
    for (const auto &b : sorted)
        got += b.entrant_list + " ";
    expect(got == "late_a late_b y2016 y2017 y2018 ", "boxes sort by order, then by registration (" + got + ")");

    expect(era::press_refusal(true, true, 13, 11, 2).empty(), "a press with the last build's stock and addon tiles goes through");
    expect(!era::press_refusal(false, true, 13, 11, 2).empty(), "a press from another view model is refused");
    expect(!era::press_refusal(true, false, 13, 11, 2).empty(), "a press without the addon nodes is refused");
    expect(!era::press_refusal(true, true, 11, 11, 0).empty(), "a press with no addon boxes is refused");
    expect(!era::press_refusal(true, true, 12, 11, 2).empty(), "a press with fewer tiles than the last build is refused");

    {
        Node a_list, a_sel, b_list, b_sel;
        era::Handle a_ui{}, b_ui{};
        era::link(a_ui, a_list.mem);
        era::link(b_ui, b_list.mem);
        int16_t a0 = a_list.refs(), as0 = a_sel.refs(), b0 = b_list.refs(), bs0 = b_sel.refs();
        Set set{};
        int clears = 0;
        auto clear = [&](Set &s) {
            game_unlink(s.list);
            game_unlink(s.selected);
            clears++;
        };
        auto build = [&](Node &list, Node &sel) {
            era::relink(set, list.mem, clear);
            era::link(set.selected, sel.mem);
        };

        build(a_list, a_sel);
        expect(!clears && a_list.holders() == &set.list && a_sel.holders() == &set.selected && consistent(a_list) &&
                   consistent(a_sel) && a_list.refs() == a0 + 1,
               "the first build links the set at the head of A's holder lists");

        build(b_list, b_sel);
        expect(clears == 1, "a build for view model B clears the set that A's nodes still hold");
        expect(!holds(a_list, set.list) && !holds(a_sel, set.selected) && consistent(a_list) && consistent(a_sel) &&
                   chain(a_list).size() == 2 && a_list.refs() == a0 && a_sel.refs() == as0,
               "A's nodes no longer reference the set and have their references back");
        expect(b_list.holders() == &set.list && b_sel.holders() == &set.selected && consistent(b_list) &&
                   consistent(b_sel) && b_list.refs() == b0 + 1 && b_sel.refs() == bs0 + 1,
               "B's nodes hold the set once, with consistent holder lists");

        game_teardown(a_list);
        game_teardown(a_sel);
        expect(set.list.node == b_list.mem && set.selected.node == b_sel.mem && b_sel.self().node == b_sel.mem &&
                   consistent(b_list) && consistent(b_sel) && b_list.refs() == b0 + 1 && b_sel.refs() == bs0 + 1,
               "tearing A down leaves B's nodes and the set alone");

        build(b_list, b_sel);
        expect(clears == 2 && chain(b_list).size() == 3 && consistent(b_list) && b_list.refs() == b0 + 1,
               "a rebuild of B links the set once");

        game_teardown(b_list);
        game_teardown(b_sel);
        expect(!set.list.node && !set.list.next && !set.selected.node && !set.selected.prev,
               "tearing B down unlinks the set and releases its nodes");
        Node c_list, c_sel;
        build(c_list, c_sel);
        expect(clears == 2 && consistent(c_list) && consistent(c_sel) && c_sel.holders() == &set.selected,
               "a build after the game tore the set's nodes down links them without a clear");

        Node old_a, old_b;
        Set old{};
        era::link(old.selected, old_a.mem);
        old = {};
        era::link(old.selected, old_b.mem);
        int16_t ob = old_b.refs();
        game_teardown(old_a);
        expect(old_b.refs() < ob && !old_b.self().node,
               "1.5.4's reset left A's holder list running into B's, so tearing A down released B's node");
    }

    {
        std::vector<era::Box> none;
        era::Box tt = box("bdx_2012", "lng_bdx_2012", "", 2012);
        tt.time_trial = "bdx_2012_tt";
        expect(era::check(tt, none).empty() && tt.time_trial == "bdx_2012_tt", "a box with a Time Trial list is accepted");
        expect(era::list_for(tt, true) == "bdx_2012_tt" && era::list_for(tt, false) == "bdx_2012",
               "the Time Trial picker uses the box's Time Trial list, the Grand Prix picker its own");
        era::Box plain = box("cs_f1_2016");
        expect(era::check(plain, none).empty() && era::list_for(plain, true) == "cs_f1_2016",
               "a box without a Time Trial list uses its own list in Time Trial");
        era::Box same = box("cs_f1_2017");
        same.time_trial = "cs_f1_2017";
        expect(era::check(same, none).empty() && same.time_trial.empty(), "a Time Trial list equal to the box's list is dropped");
        era::Box bad = box("cs_f1_2018");
        bad.time_trial = "has space";
        expect(!era::check(bad, none).empty(), "a Time Trial list that is not a plain name is refused");
        std::vector<era::Box> shared = {tt};
        era::Box other = box("bdx_2012_alt");
        other.time_trial = "bdx_2012_tt";
        expect(era::check(other, shared).empty(), "two boxes may share a Time Trial list");

        expect(era::TT_MAX_CARS == 23, "Time Trial holds 23 cars of a list (26 slots, 3 the game adds)");
        expect(era::time_trial_refusal(plain, 22).empty() && era::time_trial_refusal(plain, 23).empty(),
               "a list of 22 or 23 cars is offered in Time Trial");
        std::string why = era::time_trial_refusal(plain, 24);
        expect(why.find("cs_f1_2016 holds 24 cars") != std::string::npos && why.find("at most 23") != std::string::npos &&
                   why.find("Give the box a Time Trial entrant list") != std::string::npos,
               "a 24-car list without a Time Trial list is refused, naming the fix (" + why + ")");
        why = era::time_trial_refusal(tt, 24);
        expect(why.find("bdx_2012_tt holds 24 cars") != std::string::npos && why.find("Give the box") == std::string::npos,
               "an oversized Time Trial list is refused by its own name (" + why + ")");
        expect(era::time_trial_refusal(plain, -1).empty(), "an unread car count refuses nothing");

        era::Box both = box("bdx_any"), tt_only = box("bdx_tt_only"), gp_only = box("bdx_gp_only");
        tt_only.only = era::ONLY_TIME_TRIAL;
        gp_only.only = era::ONLY_GRAND_PRIX;
        expect(era::check(tt_only, none).empty() && era::check(gp_only, none).empty(), "a box with only set is accepted");
        expect(era::shown(both, false) && era::shown(both, true), "a box without only shows on both pickers");
        expect(!era::shown(tt_only, false) && era::shown(tt_only, true), "a Time Trial only box is off the Grand Prix picker and on the Time Trial picker");
        expect(era::shown(gp_only, false) && !era::shown(gp_only, true), "a Grand Prix only box is on the Grand Prix picker and off the Time Trial picker");
        era::Box odd = box("bdx_odd");
        odd.only = 3;
        expect(!era::check(odd, none).empty(), "an only past 2 is refused");

        expect(era::entrant_count(0x1000, 0x1000 + 24 * 0x88) == 24, "24 entrant records of 0x88 bytes count 24");
        expect(era::entrant_count(0x1000, 0x1000) == 0, "an empty list counts 0");
        expect(era::entrant_count(0, 0x88) < 0 && era::entrant_count(0x2000, 0x1000) < 0 &&
                   era::entrant_count(0x1000, 0x1000 + 0x89) < 0 && era::entrant_count(0x1000, 0x1000 + 65 * 0x88) < 0,
               "a null, reversed, misaligned or over-long entrant vector is unread");

        std::vector<era::ListCars> host = {{"cs_f1_2016", 22}, {"cs_f1_2017", 20}, {"bdx_s5_2012", 24}, {"unread", -1}};
        expect(!era::oversized(23) && era::oversized(24) && !era::oversized(-1), "past 23 cars is oversized, an unread count is not");
        auto keep = era::time_trial_keep(host);
        expect(keep == std::vector<size_t>{0, 1, 3},
               "a stock press in Time Trial keeps swofty_f1's lists of at most 23 cars and the unread ones, and drops the 24-car list");
        std::vector<era::ListCars> fine = {{"cs_f1_2016", 22}, {"cs_f1_2017", 20}};
        expect(era::time_trial_keep(fine).size() == fine.size(), "lists that all fit are all kept");
        std::vector<era::ListCars> big = {{"a", 24}, {"b", 30}};
        expect(era::time_trial_keep(big).empty(), "lists that all overflow keep nothing");
        expect(era::time_trial_narrows(big, era::time_trial_keep(big)),
               "lists that all overflow still narrow swofty_f1, to no list at all, so a stock Time Trial takes none of their teams");
        std::vector<era::ListCars> only_2012 = {{"bdx_s5_2012", 24}};
        expect(era::time_trial_narrows(only_2012, era::time_trial_keep(only_2012)) && era::time_trial_keep(only_2012).empty(),
               "F1 2012 Season alone (swofty_f1 offers only bdx_s5_2012, 24 cars) leaves swofty_f1 offering nothing in Time Trial");
        expect(era::time_trial_narrows(host, keep) && !era::time_trial_narrows(fine, era::time_trial_keep(fine)) &&
                   !era::time_trial_narrows({}, {}),
               "a mix narrows to the lists that fit; lists that all fit, or none at all, are left as they are");

        std::vector<era::ListCars> esm = {{"bdx_s5_2012", 24}, {"cs_f1_2016", 22}, {"f1_2025", 20}, {"tt_list", 12}};
        expect(era::time_trial_fallback(esm, {"f1_2025"}) == 2, "an oversized session list falls back to the box pressed");
        expect(era::time_trial_fallback(esm, {"bdx_s5_2012", "tt_list"}) == 3,
               "an oversized box pressed falls back to its Time Trial list");
        expect(era::time_trial_fallback(esm, {"missing"}) == 1, "with no preferred list, the first list that fits");
        expect(era::time_trial_fallback(esm, {}) == 1, "with no box pressed, the first list that fits");
        std::vector<era::ListCars> none_fit = {{"bdx_s5_2012", 24}, {"unread", -1}};
        expect(era::time_trial_fallback(none_fit, {"unread"}) == -1, "an unread list is never a fallback, nor is an oversized one");

        std::string swap = era::time_trial_swap("bdx_s5_2012", 24, "swofty_f1", "cs_f1_2016", 0);
        expect(swap.find("bdx_s5_2012 (setup mode swofty_f1) holds 24 cars") != std::string::npos &&
                   swap.find("at most 23") != std::string::npos && swap.find("loading cs_f1_2016 instead") != std::string::npos &&
                   swap.find("max_entrants") == std::string::npos,
               "the swap line names the list, its setup mode, its count and the list loaded instead (" + swap + ")");
        swap = era::time_trial_swap("bdx_s5_2012", 24, "bdx_2012_mode", "", 24);
        expect(swap.find("offers no list of at most 23 cars") != std::string::npos &&
                   swap.find("max_entrants 24 -> 23 until the session ends") != std::string::npos,
               "with nothing to load instead, the line says the session draws at most 23 cars (" + swap + ")");

        expect(era::time_trial_limit(24) == 23 && era::time_trial_limit(-1) == 23 && era::time_trial_limit(0) == 23,
               "an entrant setup mode allowing more than 23 cars, or any number, is held to 23 for a Time Trial session");
        expect(era::time_trial_limit(23) == 23 && era::time_trial_limit(20) == 20 && era::time_trial_limit(5) == 5,
               "a limit of at most 23 is left as it is");

        uintptr_t b = 0x10000;
        expect(era::session_vehicles(b, b + 27 * 8) == 27 && era::session_vehicles(b, b + 25 * 8) == 25 &&
                   era::session_vehicles(b, b) == 0 && era::session_vehicles(b + 8, b) == 0,
               "the session's vehicle count is its pointer vector's length, 0 when empty or reversed");
        expect(era::vehicles_end_within_table(b, b + 27 * 8) == b + 26 * 8,
               "27 vehicles (a 24-car list plus the game's 3) are handed over as the first 26, the table's size");
        expect(era::vehicles_end_within_table(b, b + 26 * 8) == b + 26 * 8 && era::vehicles_end_within_table(b, b + 25 * 8) == b + 25 * 8,
               "26 vehicles or fewer are handed over as they are");

        auto refs = [](uint32_t first, size_t n) {
            std::vector<uint32_t> r;
            for (size_t i = 0; i < n; ++i)
                r.push_back((first + uint32_t(i)) << 16);
            return r;
        };
        era::TtList custom{"f1_custom_team", true, false, refs(1, 11)};
        era::TtList f1_2025{"f1_2025", false, false, refs(1, 10)};
        auto stock = era::time_trial_budget({custom, f1_2025});
        expect(stock.dropped.empty() && stock.cars == 11 && stock.cars + era::TT_OWN_ENTRIES == 15,
               "a stock F1 2025 Time Trial draws 11 distinct cars (f1_2025's 10 are among f1_custom_team's 11): 15 vehicles");
        era::TtList bdx{"bdx_s5_2012", false, true, refs(700, 12)};
        auto with_2012 = era::time_trial_budget({custom, f1_2025, bdx});
        expect(with_2012.dropped == std::vector<size_t>{2} && with_2012.cars == 11,
               "F1 2012 Season's 12 cars would make 27 vehicles: its list is left out and the session keeps 15");
        era::TtList a{"ex_a", false, true, refs(800, 10)}, bb{"ex_b", false, true, refs(820, 10)};
        auto two = era::time_trial_budget({custom, a, bb});
        expect(two.dropped == std::vector<size_t>{2} && two.cars == 21,
               "two kept boxes of f1_2025's leaderboard (15 + 10 + 10): the first fits (25 vehicles), the second is left out");
        {
            era::TtList own{"swofty_cs_2016_list", true, true, refs(2000, 22)};
            auto alone = era::time_trial_budget({own});
            expect(alone.dropped.empty() && alone.cars == 22 && alone.cars + era::TT_OWN_ENTRIES == 26,
                   "a box on a leaderboard of its own keeps its 22 cars: 26 vehicles, the full table");
            expect(era::leaderboard_text(6) == "6, added" && era::leaderboard_text(1) == "1, classic" &&
                       era::leaderboard_text(4) == "4",
                   "the log names a descriptor an addon added, and the stock ones");
        }
        era::TtList wide{"ex_23", false, true, refs(900, 12)};
        wide.refs.resize(23, wide.refs.back());
        expect(era::distinct_cars(wide.refs) == 12 && era::time_trial_budget({custom, wide}).dropped == std::vector<size_t>{1},
               "a 23-car list over 12 cars passes the 23-car check but makes 27 vehicles: it is left out");
        era::TtList other_mode{"ex_mode_list", false, false, refs(950, 12)};
        auto foreign = era::time_trial_budget({custom, other_mode});
        expect(foreign.dropped == std::vector<size_t>{1} && foreign.cars == 11,
               "another addon's setup mode on the Time Trial mode with a 12-car list of the leaderboard is left out too");
        era::TtList stock_extra{"stock_extra", false, false, refs(400, 5)}, addon_small{"ex_small", false, true, refs(420, 8)};
        auto order = era::time_trial_budget({custom, addon_small, stock_extra});
        expect(order.dropped == std::vector<size_t>{1} && order.cars == 16,
               "stock lists join before addon lists, so the addon list is the one left out (11 + 5, then 8 would pass 22)");
        era::TtList big_selected{"ex_selected", true, true, refs(1000, 24)};
        auto sel = era::time_trial_budget({big_selected, f1_2025});
        expect(sel.dropped == std::vector<size_t>{1} && sel.cars == 24,
               "the selected list always stays; the others are left out when it fills the table by itself");
        {
            era::TtList prev{"cs_f1_2020", true, false, refs(3000, 20)};
            std::vector<era::ListCars> esm = {{"bdx_s5_2012", 24}, {"bdx_s5_2012_tt", 12}, {"cs_f1_2020", 20}};
            era::ListCars session{"cs_f1_2020", 20}, first{"bdx_s5_2012", 24};
            int64_t loaded = era::time_trial_loaded(session, first, esm, {"f1_2025", "bdx_s5_2012_tt"});
            expect(loaded == 1 && esm[size_t(loaded)].name == "bdx_s5_2012_tt",
                   "after a previous session on cs_f1_2020, a 24-car first list swaps to its _tt list, which is what loads");
            expect(era::time_trial_loaded({"cs_f1_2020", 20}, {"cs_f1_2020", 20}, esm, {}) == -1,
                   "no swap while the selected and first lists fit: the selected list is what loads");
            era::TtList tt{"bdx_s5_2012_tt", true, true, refs(700, 12)};
            era::TtList stock_a{"cs_f1_2021", false, false, refs(3100, 10)}, stock_b{"cs_f1_2022", false, false, refs(3200, 10)};
            auto fresh = era::time_trial_budget({tt, stock_a, stock_b});
            expect(fresh.cars + era::TT_OWN_ENTRIES <= era::TT_TABLE && fresh.dropped == std::vector<size_t>{2},
                   "the budget built for the list that loads (12 cars) keeps the table within 26 and leaves out what does not fit");
            auto wrong = era::time_trial_budget({prev, stock_a});
            expect(wrong.cars + era::TT_OWN_ENTRIES + 12 > era::TT_TABLE,
                   "a budget built for the pre-swap list leaves room that the swapped list's 12 cars then overflow");
        }
        era::TtList empty_refs{"ex_empty", false, true, {0x3fff0000u, 0x3fff0000u}};
        expect(era::time_trial_budget({custom, empty_refs}).dropped.empty() && era::distinct_cars(empty_refs.refs) == 0,
               "entrants without a vehicle ref (0x3fff) take no slot");
        std::string bl = era::time_trial_budget_line({custom, f1_2025, bdx}, with_2012, 0);
        expect(bl == "the Time Trial session's list f1_custom_team (leaderboard 0, f1) draws 11 cars from 2 list(s) of its leaderboard; "
                     "with the game's 4 entries 15 of the 26-slot vehicle table; left out until the session unloads: bdx_s5_2012 (12 cars)",
               "the budget line names the list, leaderboard, cars, vehicles and what it left out (" + bl + ")");
        {
            alignas(8) uint8_t slots[4 * era::OVERRIDE_SLOT] = {};
            uint8_t meta[4] = {0, 3, 0, 5};
            int base = 0, f1_custom_team = 0, other = 0;
            slots[1 * era::OVERRIDE_SLOT] = 9;
            *reinterpret_cast<void **>(slots + 1 * era::OVERRIDE_SLOT + 8) = &f1_custom_team;
            slots[3 * era::OVERRIDE_SLOT] = 12;
            *reinterpret_cast<void **>(slots + 3 * era::OVERRIDE_SLOT + 8) = &other;
            slots[0] = 7;
            *reinterpret_cast<void **>(slots + 8) = &base;
            expect(era::override_for(9, slots, meta, 4).to == &f1_custom_team && era::override_for(12, slots, meta, 4).to == &other,
                   "an entrant setup mode with an override row is swapped for its override, as slot 0x1d8 does");
            expect(!era::override_for(7, slots, meta, 4).found && !era::override_for(8, slots, meta, 4).found,
                   "a slot whose metadata byte is 0 is empty, so stale key bytes there match nothing; no row, no swap");
            expect(!era::override_for(9, nullptr, meta, 4).found &&
                       !era::override_for(9, slots, meta, era::MAX_OVERRIDE_SLOTS + 1).found,
                   "an unread or over-long override map swaps nothing");
            *reinterpret_cast<void **>(slots + 3 * era::OVERRIDE_SLOT + 8) = nullptr;
            auto null_row = era::override_for(12, slots, meta, 4);
            expect(null_row.found && !null_row.to,
                   "a row whose override is null is still installed, as the game does: that mode slot is left empty");
        }
        expect(era::ref_key(0x02a5001fu) == era::ref_key(0x02a5ff1fu) && era::ref_key(0x02a5001fu) != era::ref_key(0x02a60020u),
               "vehicle refs are the same car when byte +0 and the u16 +2 match, whatever byte +1 holds");
        era::TtList pool{"entrants system pools", false, false, refs(1, 3)};
        pool.fixed = true;
        pool.refs.push_back(60u << 16);
        auto with_pool = era::time_trial_budget({custom, pool, bdx});
        expect(with_pool.cars == 12 && with_pool.dropped == std::vector<size_t>{2},
               "entrants system pool refs always count (11 + 1 new) and never leave; the 12-car list still does");
        std::string pl = era::time_trial_budget_line({custom, pool, bdx}, with_pool, 3);
        expect(pl.find("from 1 list(s) of its leaderboard and the entrants system pools (4)") != std::string::npos &&
                   pl.find("16 of the 26-slot") != std::string::npos,
               "the budget line names the pools and their cars (" + pl + ")");
        {
            std::vector<uint8_t> pool(3001 * era::POOL_SLOT, 0);
            uint32_t high = 3000u << 16, low = 5u << 16;
            *reinterpret_cast<uint32_t **>(pool.data() + 3000 * era::POOL_SLOT + 8) = &high;
            *reinterpret_cast<uint32_t **>(pool.data() + 5 * era::POOL_SLOT + 8) = &low;
            auto read = [](uintptr_t a, auto &v) {
                std::memcpy(&v, reinterpret_cast<const void *>(a), sizeof(v));
                return true;
            };
            auto b0 = reinterpret_cast<uintptr_t>(pool.data());
            auto got = era::pool_refs_from(b0, b0 + pool.size(), read);
            expect(got.refs == std::vector<uint32_t>{low, high} && !got.capped,
                   "a pool is as long as its highest vehicle stat id + 1: an addon car at id 3000 is counted");
            std::vector<uint8_t> huge((era::MAX_POOL + 10) * era::POOL_SLOT, 0);
            uint32_t past = 1u << 16;
            *reinterpret_cast<uint32_t **>(huge.data() + (era::MAX_POOL + 5) * era::POOL_SLOT + 8) = &past;
            auto hb = reinterpret_cast<uintptr_t>(huge.data());
            auto capped = era::pool_refs_from(hb, hb + huge.size(), read);
            expect(capped.capped && capped.refs.empty(), "a pool past 8192 slots is counted to 8192 and reported as capped");
            expect(era::pool_refs_from(b0, b0 + 0x18, read).refs.empty() && era::pool_refs_from(0, 0x100, read).refs.empty(),
                   "a misaligned or null pool counts nothing");
        }
        {
            era::SessionTrace bt;
            bt.where = "Sess_Setup_VehicleComponents";
            bt.budget = 15;
            bt.vehicles = 27;
            expect(era::budget_mismatch(bt) ==
                       "Time Trial fill at Sess_Setup_VehicleComponents made 27 vehicles where the budget expected 15",
                   "a fill that made more vehicles than the budget expected logs a line");
            bt.vehicles = 15;
            expect(era::budget_mismatch(bt).empty(), "a fill that matches the budget logs nothing more");
            bt.vehicles = -1;
            bt.gm_session = 27;
            bt.where = "StateLoadSession Exiting";
            expect(!era::budget_mismatch(bt).empty(), "before the vehicle setup the descriptor's participants are compared");
            bt.gm_session = 0;
            expect(era::budget_mismatch(bt).empty(), "an empty descriptor (no fill yet) is not a mismatch");
            bt.budget = -1;
            bt.gm_session = 27;
            expect(era::budget_mismatch(bt).empty(), "outside a budgeted Time Trial nothing is compared");

            int64_t previous = 25;
            bool any = false;
            for (auto [where, seen] : {std::pair<const char *, int64_t>{"StateLoadSession Entering", 25},
                                       {"StateLoadSession Updating", 25},
                                       {"StateLoadSession Exiting", 15},
                                       {"StateSession Entering", 15}}) {
                era::SessionTrace st;
                st.where = where;
                st.budget = 15;
                st.gm_session = seen;
                st.previous = previous;
                any = any || !era::budget_mismatch(st).empty();
                previous = seen;
            }
            expect(!any, "a Time Trial after a 2012 GP (participants 25, 25, then the fill's 15, 15) logs no mismatch: "
                         "the GP's 25 before the fill are not compared");
            era::SessionTrace early;
            early.where = "StateLoadSession Entering";
            early.budget = 15;
            early.gm_session = 27;
            early.previous = 25;
            expect(!era::budget_mismatch(early).empty(), "a count that changed before Entering (a fill there) is compared");
            early.where = "StateLoadSession Exiting";
            early.gm_session = 25;
            early.previous = 25;
            expect(!era::budget_mismatch(early).empty(), "from Exiting on, any count is compared, changed or not");
            expect(era::leaderboard_text(0) == "0, f1" && era::leaderboard_text(1) == "1, classic" &&
                       era::leaderboard_text(4) == "4" && era::leaderboard_text(40) == "40, added",
                   "stock leaderboard ids print with their names (f1 is 0); ids past them print as added");
        }
        expect(era::vehicle_flags(era::TAKES_LIST, 0) == era::TAKES_LIST &&
                   era::vehicle_flags(0, era::TAKES_POOL_A) == era::TAKES_POOL_A && era::vehicle_flags(0, 0) == 0,
               "the fill's human record reads the player or the second player flag word, so both are taken together");
        expect(era::cut_recordable(15, 16, false) && !era::cut_recordable(16, 16, false) && era::cut_recordable(16, 16, true),
               "a cut needs a free restore slot, or an entry already holding that mode; with the stack full it is skipped");
        expect(era::LIST_LEADERBOARD == 0xe0 && era::LIST_LEADERBOARD_SET == 0xf0 && era::ENTRANT_VEHICLE == 0x5c &&
                   era::TT_ROOM == 22,
               "a list's leaderboard is +0xe0 (engaged +0xf0), an entrant's vehicle ref +0x5c, and 22 cars fit beside the 4 entries");

        expect(era::trace_flow("StateLoadSession", 1) && era::trace_flow("StateLoadSession", 2) &&
                   era::trace_flow("StateLoadSession", 3) && era::trace_flow("StateSession", 1),
               "the session trace runs at StateLoadSession Entering, Updating and Exiting and at StateSession Entering");
        expect(era::trace_flow("StateSession", 3) && era::trace_flow("StateUnloadSession", 1) &&
                   era::trace_flow("StateUnloadSession", 2) && era::trace_flow("StateUnloadSession", 3),
               "a restart or retry is traced too: StateSession Exiting and every StateUnloadSession step");
        expect(!era::trace_flow("StateLoadSession", 0) && !era::trace_flow("StateSession", 2) &&
                   !era::trace_flow("StateUnloadSession", 0) && !era::trace_flow("#957880c6", 1) && !era::trace_flow("", 1),
               "no trace when a state leaves, while a session runs or on menu flows");
        expect(era::time_trial_cut_off("StateUnloadSession", 3) && !era::time_trial_cut_off("StateUnloadSession", 1) &&
                   !era::time_trial_cut_off("StateUnloadSession", 2) && !era::time_trial_cut_off("StateSession", 3) &&
                   !era::time_trial_cut_off("StateLoadSession", 3),
               "the Time Trial cut comes off at StateUnloadSession Exiting, after the onStateEnter keep check, not at Entering");
        uintptr_t d = 0x200000;
        expect(era::participant_count(d, d + 27 * era::PARTICIPANT) == 27 && era::participant_count(d, d) == 0,
               "a descriptor's participants count in 0x6780-byte steps");
        expect(era::participant_count(0, 0x6780) < 0 && era::participant_count(d, d + 0x6781) < 0 &&
                   era::participant_count(d + 8, d) < 0 && era::participant_count(d, d + 65 * era::PARTICIPANT) < 0,
               "a null, misaligned, reversed or over-long participant vector is unread");
        era::SessionTrace tr;
        tr.where = "StateLoadSession Exiting";
        tr.mode = "time_trial";
        tr.entrants = 3;
        tr.teams = {700, 701, 1};
        tr.list = "f1_custom_team";
        tr.list_cars = 22;
        tr.setup_mode = "f1";
        tr.gm_session = 27;
        tr.module_session = 27;
        std::string tl = era::session_trace_line(tr);
        expect(tl == "session trace StateLoadSession Exiting (time_trial): entrants 3 (teams 700,701,1), list f1_custom_team "
                     "(22 cars) of setup mode f1; participants: game manager 27, game module 27 (next -); session vehicles -",
               "the trace line names the step, mode, entrants with their teams, list, setup mode and participant counts (" +
                   tl + ")");
        expect(era::session_trace_line(era::SessionTrace{}).find("(?): entrants -, list ? (- cars)") != std::string::npos,
               "unread values print as - and ?");
        expect(era::MGR_SESSION_BEGIN == 0x2e0 && era::ENTRANT_TEAM == 0x5a && era::DESC_PARTICIPANTS_BEGIN == 0xad8 &&
                   era::GM_SESSION == 0x1a8 && era::GM_MODULE == 0x140,
               "the trace reads EntrantsManager +0x2e0, an entrant's team at +0x5a and descriptor participants at +0xad8");

        for (const char *m : {"time_trial", "solo_testing"})
            expect(era::time_trial_mode(m), std::string(m) + " is a Time Trial picker");
        for (const char *m : {"grand_prix_23", "grand_prix_splitscreen", "", "my_team_career_25"})
            expect(!era::time_trial_mode(m), std::string("'") + m + "' is not a Time Trial picker");
    }

    expect(era::ADDON_PRESS == 0x8fb68dffu && era::SELECT_PRESS == 0x2b0357au && era::ADDON_TILES == 0xf2614e2bu,
           "djb2 of addon_select_pressed, select_pressed and addon_tiles");

    drs_swap_tests();
    warmer_swap_tests();
    points_tests();
    tyre_change_tests();
    time_limit_swap_tests();
    weekend_swap_tests();
    stored_mode_tests();
    continue_tests();
    setup_guard_tests();
    grid_limit_tests();
    cut_restore_tests();
    front_end_leak_tests();

    printf("%s\n", fails ? "era_logic_test: FAILED" : "era_logic_test: all passed");
    return fails ? 1 : 0;
}
