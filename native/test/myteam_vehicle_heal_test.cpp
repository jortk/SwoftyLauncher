// Host-native unit test for the core's My Team vehicle heal (native/src/core/myteam_vehicle_heal_logic.hpp) and the
// game manager's entrants system displacement it reads the cars through (native/src/core/game_manager_layout.hpp).
// Built and run by native/test/run_myteam_vehicle_heal_test.sh.

#include "game_manager_layout.hpp"
#include "myteam_vehicle_heal_logic.hpp"

#include <cstdio>
#include <map>
#include <string>
#include <vector>

namespace mh = runtime::myteam_heal;

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

    bool text(uint64_t a, std::string &out)
    {
        auto it = s.find(a);
        if (it == s.end())
            return false;
        out = it->second;
        return true;
    }
};

constexpr uint64_t ENTRANTS = 0x340d01000, STOCK_TABLE = 0x340e00000, CUSTOM_TABLE = 0x340e10000, STOCK_SLOTS = 0x341000000,
                   CUSTOM_SLOTS = 0x342000000, VEHICLES = 0x350000000, LIST = 0x360000000;

static uint64_t car(int i)
{
    return VEHICLES + static_cast<uint64_t>(i) * 0x1000;
}

static uint32_t stock_handle(uint16_t id)
{
    return static_cast<uint32_t>(id) << 16;
}

static uint32_t custom_handle(uint16_t id)
{
    return static_cast<uint32_t>(id) << 16 | mh::KIND_CUSTOM;
}

static Fake game(const std::vector<std::string> &stock, size_t custom_slots)
{
    Fake f;
    f.q[ENTRANTS + mh::TABLES] = STOCK_TABLE;
    f.q[ENTRANTS + mh::TABLES + mh::TABLE_STRIDE] = CUSTOM_TABLE;
    f.q[ENTRANTS + mh::TABLES + 2 * mh::TABLE_STRIDE] = 0;
    f.q[STOCK_TABLE + mh::SLOTS_BEGIN] = STOCK_SLOTS;
    f.q[STOCK_TABLE + mh::SLOTS_END] = STOCK_SLOTS + stock.size() * mh::SLOT_SIZE;
    for (size_t i = 0; i < stock.size(); i++) {
        f.q[STOCK_SLOTS + i * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = stock[i].empty() ? 0 : car(static_cast<int>(i));
        if (!stock[i].empty()) {
            f.s[car(static_cast<int>(i)) + mh::VEHICLE_FILE_NAME] = stock[i];
            f.d[car(static_cast<int>(i))] = stock_handle(static_cast<uint16_t>(i));
        }
    }
    f.q[CUSTOM_TABLE + mh::SLOTS_BEGIN] = CUSTOM_SLOTS;
    f.q[CUSTOM_TABLE + mh::SLOTS_END] = CUSTOM_SLOTS + custom_slots * mh::SLOT_SIZE;
    for (size_t i = 0; i < custom_slots; i++)
        f.q[CUSTOM_SLOTS + i * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = 0;
    return f;
}

static void entrants(Fake &f, const std::vector<uint32_t> &handles)
{
    for (size_t i = 0; i < handles.size(); i++)
        f.d[LIST + i * mh::ENTRANT_SIZE + mh::ENTRANT_VEHICLE] = handles[i];
}

static uint64_t list_end(size_t n)
{
    return LIST + n * mh::ENTRANT_SIZE;
}

int main()
{
    namespace gm = runtime::game_manager;
    expect(gm::entrants_system("steam") == 0x290, "the entrants system is [g_Game_Manager]+0x290 on Steam");
    expect(gm::entrants_system("ea") == 0x260, "and +0x260 on the EA App build (George's and jhxsh's empty heal reads)");
    expect(gm::entrants_system("unknown") == 0x290, "an unknown build keeps the KB displacement");
    expect(gm::tail_field(0x2d0, "ea") == 0x2a0 && gm::tail_field(0x2e8, "ea") == 0x2b8 && gm::tail_field(0x278, "ea") == 0x248,
           "every EA qword field from Steam +0x278 on is 0x30 lower (the live progress system +0x2d0 is +0x2a0)");
    expect(gm::tail_field(0x2d0, "steam") == 0x2d0 && gm::tail_field(0x290, "unknown") == 0x290,
           "Steam and an unknown build read every field where the KB has it");

    expect(mh::kind(0x4000) == 0 && mh::kind(custom_handle(7)) == 1 && mh::id(custom_handle(7)) == 7,
           "a vehicle handle is the kind in its low byte and the stat id in its high word");
    expect(mh::healable(custom_handle(0)) && mh::healable(custom_handle(8)), "My Team handles (kind 1) are healed");
    expect(!mh::healable(stock_handle(5)) && !mh::healable(0x3fff0000u), "stock handles and the empty handle are not");
    expect(!mh::healable(custom_handle(0x3fff)) && !mh::healable(custom_handle(mh::MAX_ID)), "an out-of-range custom id is not");

    std::vector<std::string> stock = {"ferrari",           "mercedes",          "",
                                      "myteam_ferrari",    "myteam_mercedes",   "myteam_honda",
                                      "ferrari_2026",      "mercedes_2026",     "myteam_mes_nissan_2026",
                                      "myteam_honda_2026", "myteam_mercedes_2026"};
    Fake f = game(stock, 2);
    expect(mh::vehicle(f, ENTRANTS, stock_handle(1)) == car(1), "a stock handle resolves through the kind 0 table");
    expect(mh::vehicle(f, ENTRANTS, stock_handle(2)) == 0, "an empty stock slot resolves to nothing");
    expect(mh::vehicle(f, ENTRANTS, stock_handle(99)) == 0, "a stat id past the table resolves to nothing");
    expect(mh::vehicle(f, ENTRANTS, custom_handle(1)) == 0, "silversquared's lost My Team car (custom 1) resolves to nothing");
    expect(mh::vehicle(f, ENTRANTS, 0x00000002u) == 0, "a kind with no table resolves to nothing");

    mh::Stock s = mh::stock(f, ENTRANTS);
    expect(s.myteam.size() == 5, "the stock My Team cars are listed, the empty slot, other cars and addon cars are not");
    bool mes = false;
    for (const auto &e : s.myteam)
        mes = mes || e.second == "myteam_mes_nissan_2026";
    expect(!mes, "a More Engine Suppliers car is never a source");

    entrants(f, {stock_handle(6), stock_handle(7), custom_handle(1), custom_handle(1)});
    expect(mh::season_2026(f, ENTRANTS, LIST, list_end(4)), "a session of 2026 stock cars is a 2026 season");
    mh::Source src;
    expect(mh::plan(f, ENTRANTS, LIST, list_end(4), src) && src.label == "myteam_mercedes_2026" && src.vehicle == car(10) && src.y2026,
           "a 2026 session rebuilds the car from myteam_mercedes_2026");

    Fake f25 = game(stock, 2);
    entrants(f25, {stock_handle(0), stock_handle(1), custom_handle(0), custom_handle(0)});
    expect(!mh::season_2026(f25, ENTRANTS, LIST, list_end(4)), "a session of 2025 stock cars is not a 2026 season");
    expect(mh::plan(f25, ENTRANTS, LIST, list_end(4), src) && src.label == "myteam_mercedes" && src.vehicle == car(4) && !src.y2026,
           "a 2025 session rebuilds the car from myteam_mercedes");

    std::vector<std::string> no_mercedes26 = {"ferrari_2026", "myteam_mercedes", "myteam_audi_2026", "myteam_honda_2026"};
    Fake fa = game(no_mercedes26, 1);
    entrants(fa, {stock_handle(0), custom_handle(0)});
    expect(mh::plan(fa, ENTRANTS, LIST, list_end(2), src) && src.label == "myteam_honda_2026",
           "without myteam_mercedes_2026 the next 2026 My Team car in the preference list is used");

    std::vector<std::string> other26 = {"ferrari_2026", "myteam_mercedes", "myteam_cadillac_2026"};
    Fake fo = game(other26, 1);
    entrants(fo, {stock_handle(0), custom_handle(0)});
    expect(mh::plan(fo, ENTRANTS, LIST, list_end(2), src) && src.label == "myteam_cadillac_2026",
           "a 2026 session prefers any 2026 My Team car over a 2025 one");

    std::vector<std::string> only25 = {"ferrari_2026", "myteam_ferrari"};
    Fake fe = game(only25, 1);
    entrants(fe, {stock_handle(0), custom_handle(0)});
    expect(mh::plan(fe, ENTRANTS, LIST, list_end(2), src) && src.label == "myteam_ferrari" && src.y2026,
           "a 2026 session with only 2025 My Team cars takes a 2025 one");

    std::vector<std::string> none = {"ferrari", "mercedes", "myteam_mes_porsche_2026"};
    Fake fn = game(none, 1);
    entrants(fn, {stock_handle(0), custom_handle(0)});
    expect(!mh::plan(fn, ENTRANTS, LIST, list_end(2), src), "with no stock My Team car there is nothing to rebuild from");

    Fake empty_list = game(stock, 2);
    expect(!mh::season_2026(empty_list, ENTRANTS, 0, 0) && !mh::season_2026(empty_list, ENTRANTS, LIST, LIST),
           "an empty or unreadable entrant list reads as a 2025 season");

    Fake no_tables;
    expect(mh::vehicle(no_tables, ENTRANTS, custom_handle(0)) == 0 && mh::stock(no_tables, ENTRANTS).myteam.empty(),
           "an unreadable entrants system resolves nothing and lists no source");

    Fake torn = game(stock, 2);
    torn.q[STOCK_TABLE + mh::SLOTS_END] = STOCK_SLOTS + 8;
    expect(mh::stock(torn, ENTRANTS).myteam.empty(), "a slot vector whose size is not whole slots is not read");

    Fake career = game(stock, 2);
    entrants(career, {stock_handle(6), stock_handle(7), custom_handle(4), custom_handle(4), stock_handle(1), custom_handle(0)});
    std::vector<uint32_t> lost = mh::missing(career, ENTRANTS, LIST, list_end(6));
    expect(lost.size() == 2 && lost[0] == custom_handle(4) && lost[1] == custom_handle(0),
           "cazslip18's career: the kind 1 cars its entrants hold that resolve to nothing are listed once each");

    mh::Census cs = mh::census(career, LIST, list_end(6));
    expect(cs.entrants == 6 && cs.on_my_team == 3 && cs.ids.size() == 2 && cs.ids[0] == 4 && cs.ids[1] == 0,
           "the career census counts every entrant on a My Team car (sma's teammate included) and each car once");
    expect(mh::census(career, 0, 0).entrants == 0, "an unreadable entrant range counts nothing");
    expect(cs.kind1 == 3 && cs.stock == 3 && cs.empty == 0 && cs.unread == 0 && cs.humans == 0 &&
               cs.human_vehicle == mh::NO_VEHICLE,
           "the census counts kind 1 and stock cars, and no human when no entrant has the human flag");

    Fake ea_mes = game(stock, 2);
    entrants(ea_mes, {stock_handle(6), stock_handle(7), mh::NO_VEHICLE, custom_handle(0x3fff)});
    ea_mes.d[LIST + 1 * mh::ENTRANT_SIZE + mh::ENTRANT_FLAGS] = mh::HUMAN_FLAG | 0x7;
    mh::Census ms = mh::census(ea_mes, LIST, list_end(5));
    expect(ms.entrants == 5 && ms.on_my_team == 0 && ms.kind1 == 1 && ms.stock == 2 && ms.empty == 1 && ms.unread == 1,
           "an EA MES career with no My Team car on its grid: no My Team entrant, an out-of-range kind 1, an empty car and "
           "an unreadable record are counted apart");
    expect(ms.humans == 1 && ms.human_vehicle == stock_handle(7), "the human entrant's car is the one with the human flag");

    mh::Load load;
    load.first = LIST, load.end = list_end(5), load.manager = 0x2a0000000, load.offset = 0x260, load.system = ENTRANTS;
    expect(mh::heal_stop(load, true, cs) == nullptr, "an armed heal with My Team entrants prints its census");
    expect(std::string(mh::heal_stop(load, true, ms)) == "no entrant is on a My Team car",
           "an armed heal with no My Team entrant says so");
    expect(std::string(mh::heal_stop(load, false, cs)) == "the heal is not set up", "a heal that never armed says so");
    mh::Load chain = load;
    chain.stop = "the championship entity reads null";
    expect(std::string(mh::heal_stop(chain, false, cs)) == "the championship entity reads null",
           "a career read that stopped first is the reason, ahead of the heal's own");

    auto yes = [](uint64_t, uint64_t) { return true; };
    auto no = [](uint64_t, uint64_t) { return false; };
    expect(std::string(mh::range_stop(false, LIST, list_end(2), 128, yes)) == "the entity's entrant range is unreadable",
           "an entrant range whose pointers can't be read");
    expect(std::string(mh::range_stop(true, 0, 0, 128, yes)) == "the entity has no entrant list (begin null)",
           "an entity with no entrant list yet");
    expect(std::string(mh::range_stop(true, LIST, LIST + 0x40, 128, yes)) ==
               "the entity's entrant range is not whole entrant records",
           "a range that is not whole records");
    expect(std::string(mh::range_stop(true, list_end(2), LIST, 128, yes)) ==
               "the entity's entrant range is not whole entrant records",
           "a range that ends before it begins");
    expect(std::string(mh::range_stop(true, LIST, list_end(129), 128, yes)) ==
               "the entity's entrant range holds more entrants than a grid",
           "a range past 128 entrants");
    expect(mh::range_stop(true, LIST, list_end(214), mh::MAX_ENTRANTS, yes) == nullptr,
           "a range of 214 entrants (a More Driver Models career) is a real grid, not a stop");
    expect(std::string(mh::range_stop(true, LIST, list_end(mh::MAX_ENTRANTS + 1), mh::MAX_ENTRANTS, yes)) ==
               "the entity's entrant range holds more entrants than a grid",
           "a range past the entrant bound is still a garbage range");
    {
        Fake big = game(stock, 2);
        std::vector<uint32_t> handles(214, stock_handle(0));
        handles[200] = custom_handle(1);
        entrants(big, handles);
        mh::Census bc = mh::census(big, LIST, list_end(214));
        expect(bc.entrants == 214 && bc.on_my_team == 1 && bc.ids.size() == 1 && bc.ids[0] == 1 && bc.stock == 213,
               "the census reads all 214 entrants, so a My Team car past the 128th is counted");
        expect(mh::missing(big, ENTRANTS, LIST, list_end(214)).size() == 1,
               "a lost My Team car past the 128th entrant is found to rebuild");
    }
    expect(std::string(mh::range_stop(true, LIST, list_end(22), 128, no)) == "the entity's entrant records are not readable",
           "records that are not readable");
    expect(mh::range_stop(true, LIST, list_end(22), 128, yes) == nullptr && mh::range_stop(true, LIST, LIST, 128, no) == nullptr,
           "a readable range, or an allocated empty one, goes on to the census");

    mh::Tables mt_tables = mh::tables(ea_mes, ENTRANTS);
    std::string line = mh::stopped(mh::heal_stop(load, true, ms), load, ms, mt_tables, mh::loaded(ea_mes, ENTRANTS, mh::KIND_CUSTOM),
                                   "armed");
    std::printf("      %s\n", line.c_str());
    expect(line.rfind("no census as the career loaded: no entrant is on a My Team car (heal armed; manager 0x2a0000000, ", 0) == 0,
           "the line names the reason, the heal state and the manager");
    expect(line.find("entrants 0x360000000..0x3600002a8, entrants system [manager+0x260] 0x340d01000") != std::string::npos,
           "and the entrant range and the entrants system with the offset it was read at");
    expect(line.find("5 entrants read: 1 on kind 1 cars, 2 on stock cars, 1 on no car, 1 unread, 1 human (car 00070000)") !=
               std::string::npos,
           "and the census counts and the human's car");
    expect(line.find("stock table 11 slots, My Team table 2 slots with 0 cars)") != std::string::npos,
           "and both vehicle tables, with the My Team cars loaded");
    expect(mh::stopped("the championship entity reads null", mh::Load{}, mh::Census{}, mh::Tables{}, 0,
                       "never offered, the game code reads encrypted still")
                   .find("0 entrants read: 0 on kind 1 cars, 0 on stock cars, 0 on no car, 0 unread, no human") !=
               std::string::npos,
           "a stop before the entrant list reads zero counts and no human");

    Fake loaded = game(stock, 4);
    loaded.q[CUSTOM_SLOTS + 1 * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = car(1);
    loaded.q[CUSTOM_SLOTS + 3 * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = car(3);
    expect(mh::loaded(loaded, ENTRANTS, mh::KIND_CUSTOM) == 2 && mh::loaded(no_tables, ENTRANTS, mh::KIND_CUSTOM) == 0,
           "the My Team cars loaded are the filled kind 1 slots");

    Fake whole = game(stock, 5);
    whole.q[CUSTOM_SLOTS + 4 * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = car(4);
    whole.q[CUSTOM_SLOTS + 0 * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = car(5);
    entrants(whole, {stock_handle(6), custom_handle(4), custom_handle(4), custom_handle(0)});
    expect(mh::missing(whole, ENTRANTS, LIST, list_end(4)).empty(), "a career whose cars all resolve rebuilds nothing");

    Fake stock_only = game(stock, 2);
    entrants(stock_only, {stock_handle(0), stock_handle(1), 0x3fff0000u, stock_handle(99)});
    expect(mh::missing(stock_only, ENTRANTS, LIST, list_end(4)).empty(),
           "a Driver Career or Grand Prix grid (stock and empty handles only) rebuilds nothing");

    Fake torn_list = game(stock, 2);
    entrants(torn_list, {custom_handle(1)});
    expect(mh::missing(torn_list, ENTRANTS, LIST, LIST + 0x40).empty() && mh::missing(torn_list, ENTRANTS, 0, 0).empty(),
           "an entrant range that is empty or not whole records is not read");

    Fake s26 = game(stock, 2);
    entrants(s26, {stock_handle(6), custom_handle(1)});
    expect(mh::plan(s26, ENTRANTS, LIST, list_end(2), src) && src.handle == stock_handle(10),
           "the rebuilt car's base is the stock car it was copied from (myteam_mercedes_2026, stat id 10)");
    uint8_t data[mh::VEHICLE_DATA_SIZE] = {};
    mh::set_base(data, stock_handle(10));
    const uint8_t *b = data + (mh::VEHICLE_BASE - mh::VEHICLE_DATA);
    expect(b[0] == 0 && b[1] == 0 && b[2] == 10 && b[3] == 0, "the base handle lands at record +0xcf2 (data +0xce2)");

    Fake based = game(stock, 3);
    uint64_t mt = VEHICLES + 0x100000;
    based.q[CUSTOM_SLOTS + 2 * mh::SLOT_SIZE + mh::SLOT_VEHICLE] = mt;
    based.s[mt + mh::VEHICLE_FILE_NAME] = "myteam_mercedes_2026";
    based.d[mt] = custom_handle(2);
    based.d[mt + mh::VEHICLE_BASE] = mh::NO_VEHICLE;
    entrants(based, {stock_handle(6), custom_handle(2), custom_handle(2)});
    std::vector<uint64_t> cars = mh::held(based, ENTRANTS, LIST, list_end(3));
    expect(cars.size() == 1 && cars[0] == mt, "the career's resolved My Team cars are listed once");
    expect(mh::unbased(based, ENTRANTS, mt), "a My Team car rebuilt by 1.9.25 (base 0x3fff0000) has no base car");
    std::string bl;
    expect(!mh::base_label(based, ENTRANTS, mt, bl) && bl.empty(), "a car with no base car has no base label");
    expect(mh::base_for(based, ENTRANTS, mt) == stock_handle(10), "its base is the stock car of the same name");
    based.d[mt + mh::VEHICLE_BASE] = stock_handle(10);
    expect(!mh::unbased(based, ENTRANTS, mt), "a My Team car with a stock base is left alone");
    expect(mh::base_label(based, ENTRANTS, mt, bl) && bl == "myteam_mercedes_2026", "its base label is the stock car's");
    based.d[mt + mh::VEHICLE_BASE] = stock_handle(2);
    expect(mh::unbased(based, ENTRANTS, mt), "a base on an empty stock slot (a vehicle that is not loaded) is no base car");
    expect(!mh::base_label(based, ENTRANTS, mt, bl), "and has no base label");
    based.d[mt + mh::VEHICLE_BASE] = stock_handle(200);
    expect(mh::unbased(based, ENTRANTS, mt), "a base past the stock table is no base car");
    based.d[mt + mh::VEHICLE_BASE] = custom_handle(2);
    expect(mh::unbased(based, ENTRANTS, mt), "a base that is a My Team car is no base car");
    expect(!mh::unbased(based, ENTRANTS, 0) && !mh::unbased(no_tables, ENTRANTS, mt), "an unreadable car is not rebased");
    based.d[mt + mh::VEHICLE_BASE] = stock_handle(10);
    based.s[mt + mh::VEHICLE_FILE_NAME] = "myteam_mes_porsche_2026";
    expect(mh::base_for(based, ENTRANTS, mt) == mh::NO_VEHICLE, "an addon car whose own vehicle is not loaded gets no base");
    based.s[mt + mh::VEHICLE_FILE_NAME] = "myteam_mes_nissan_2026";
    expect(mh::base_for(based, ENTRANTS, mt) == stock_handle(8),
           "George's More Engine Suppliers car takes its own myteam_mes_ vehicle (loaded, addon on) as its base");
    based.s[mt + mh::VEHICLE_FILE_NAME] = "ferrari_2026";
    expect(mh::base_for(based, ENTRANTS, mt) == mh::NO_VEHICLE, "a car that is not a My Team car gets no base");
    mh::Tables tb = mh::tables(based, ENTRANTS);
    expect(tb.stock == STOCK_TABLE && tb.stock_slots == stock.size() && tb.custom == CUSTOM_TABLE && tb.custom_slots == 3,
           "the diagnostic reads both tables and their slot counts");
    expect(mh::tables(no_tables, ENTRANTS).stock == 0, "an unreadable entrants system reads as no tables");

    expect(mh::replaced(0) && mh::replaced(2) && mh::replaced(4), "ReplaceVehicle's replaced, added and unchanged results count");
    expect(!mh::replaced(1) && !mh::replaced(3) && !mh::replaced(5), "its no-table, failed-store and preloaded results do not");

    mh::Ids ids;
    expect(ids.first(1) && !ids.first(1) && ids.first(8), "each rebuilt car is logged once");
    for (uint16_t i = 100; ids.seen.size() < mh::LOG_IDS; i++)
        ids.first(i);
    expect(!ids.first(999), "cars past 32 are not logged");

    expect(mh::switch_off("0") && mh::switch_off("off") && !mh::switch_off("1") && !mh::switch_off(nullptr), "the switch reads 0 or off");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
