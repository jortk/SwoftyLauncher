// Host unit test for the core's per-car AI racecraft fields (ai_racecraft.hpp and car_live.hpp, ai.blocking.*) on a
// fake AI system laid out as the KB's: the record vector at S+0xa8..0xb0, stride 0x508, the vehicle index at R+0 and
// the driver config slots at R+0x238+off, each slot a pointer to a cell holding a pointer to the record whose float
// is at +8 (a curve key: the baked curve record). run_car_live_test.sh builds and runs it.
#include "../../src/core/hooks/car_live.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <set>
#include <string>
#include <utility>
#include <vector>

namespace cl = swofty_core::hooks::car_live;
namespace rc = swofty_core::hooks::ai_racecraft;

static int fails;
static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

struct Tag {};
using Ops = cl::Ops<Tag>;

constexpr int CARS = 3;
constexpr uintptr_t SLOT0 = rc::R_DRIVER + 0x90, SLOT2 = rc::R_DRIVER + 0xa0;

alignas(16) static uint8_t car_mem[CARS][0x600];
static uintptr_t table[32];
static uint32_t count;
static bool flow = true;
static int writes;
static std::set<uintptr_t> dead;
static std::vector<std::pair<uintptr_t, uintptr_t>> written;
static std::set<std::string> rows_off;

struct Db {
    alignas(16) uint8_t record[0x80];
    alignas(16) uint64_t cell;
};

struct World {
    alignas(16) uint8_t system[0x1a8];
    std::vector<uint8_t> records;
    Db db[CARS][3];
    Db extra[CARS][3];
    uint64_t shared_cell = 0;
    alignas(16) uint8_t shared_rec[0x40];
};

static void put(uint8_t *p, float v) { memcpy(p, &v, 4); }

static uint8_t *rec_at(World &w, int car) { return w.records.data() + size_t(car) * rc::STRIDE; }

static void fill_system(World &w, int n)
{
    uint64_t begin = reinterpret_cast<uintptr_t>(w.records.data());
    uint64_t end = begin + uint64_t(n) * rc::STRIDE;
    memcpy(w.system + rc::RECORDS_BEGIN, &begin, 8);
    memcpy(w.system + rc::RECORDS_END, &end, 8);
}

static void build(World &w, bool shared = false)
{
    w.records.assign(size_t(CARS) * rc::STRIDE, 0);
    float stock[3] = {0.875f, 0.975f, 30.0f};
    for (int c = 0; c < CARS; c++) {
        rec_at(w, c)[rc::R_VEHICLE_INDEX] = uint8_t(c);
        for (int s = 0; s < 3; s++) {
            Db &d = w.db[c][s];
            memset(&d, 0, sizeof d);
            memcpy(d.record + 8, &stock[s], 4);
            d.record[0x10] = uint8_t(0xa0 + s);
            d.cell = reinterpret_cast<uintptr_t>(d.record);
            uint64_t p = reinterpret_cast<uintptr_t>(&d.cell);
            memcpy(rec_at(w, c) + rc::R_DRIVER + 0x90 + 8 * s, &p, 8);
        }
    }
    (void)shared;
    constexpr uintptr_t EXTRA_OFF[3] = {0x268, 0x1d0, 0x1e0};
    for (int c = 0; c < CARS; c++)
        for (int s = 0; s < 3; s++) {
            Db &d = w.extra[c][s];
            memset(&d, 0, sizeof d);
            d.record[0x20] = 0xc0;
            d.record[0x6f] = uint8_t(0xd0 + s);
            d.record[0x7f] = uint8_t(0xe0 + s);
            if (s == 0) {
                int32_t n = 7;
                memcpy(d.record + 8, &n, 4);
            } else if (s == 1) {
                put(d.record + 0x10, 2.0f);
                put(d.record + 0x14, -0.25f);
                put(d.record + 0x18, 0.5f);
                put(d.record + 0x1c, 3.0f);
                put(d.record + 0x24, 1.0f);
                put(d.record + 0x28, 3.0f);
                for (int i = 0; i < 65; i++)
                    d.record[0x2c + i] = uint8_t(i * 3);
            } else
                put(d.record + 8, 12.5f);
            d.cell = reinterpret_cast<uintptr_t>(d.record);
            uint64_t p = reinterpret_cast<uintptr_t>(&d.cell);
            memcpy(rec_at(w, c) + rc::R_DRIVER + EXTRA_OFF[s], &p, 8);
        }
    fill_system(w, CARS);
}

static uint64_t slot_of(World &w, int car, uintptr_t off)
{
    uint64_t p;
    memcpy(&p, rec_at(w, car) + off, 8);
    return p;
}

static float db_float(World &w, int car, int s)
{
    float v;
    memcpy(&v, w.db[car][s].record + 8, 4);
    return v;
}

static cl::Env make_env()
{
    cl::Env e;
    e.read = [](uintptr_t a, void *out, size_t n) {
        for (uintptr_t d : dead)
            if (a >= d && a < d + 0x10000)
                return false;
        memcpy(out, reinterpret_cast<const void *>(a), n);
        return true;
    };
    e.write = [](uintptr_t a, const void *in, size_t n) {
        for (uintptr_t d : dead)
            if (a >= d && a < d + 0x10000)
                return false;
        writes++;
        uintptr_t v = 0;
        memcpy(&v, in, n < 8 ? n : 8);
        written.emplace_back(a, v);
        memcpy(reinterpret_cast<void *>(a), in, n);
        return true;
    };
    e.flow_thread = [] { return flow; };
    e.enabled = [] { return true; };
    e.row_ready = [](const char *row) { return !rows_off.count(row); };
    e.table = reinterpret_cast<uintptr_t>(table);
    e.count = reinterpret_cast<uintptr_t>(&count);
    return e;
}

static void build_cars()
{
    memset(car_mem, 0, sizeof car_mem);
    for (int i = 0; i < CARS; i++) {
        car_mem[i][cl::CAR_VEHICLE_ID] = uint8_t(i);
        table[i] = reinterpret_cast<uintptr_t>(car_mem[i]);
    }
    count = CARS;
}

static void settle()
{
    for (uint32_t i = 0; i < rc::SETTLE_FRAMES + 1; i++)
        Ops::racecraft_tick();
    Ops::flush();
}

static float get(uint32_t car, uint32_t id, int *r = nullptr)
{
    float v = -1;
    int x = Ops::op_get(car, nullptr, id, &v);
    if (r)
        *r = x;
    return v;
}

int main()
{
    build_cars();
    World w;
    build(w);
    Ops::bind(make_env());
    uint32_t id = 0;
    expect(Ops::op_field("ai.blocking.max_blocking_distance", &id) == SWOFTY_ASSET_OK && id == SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE,
           "the four fields resolve by path");
    expect(Ops::op_field("ai.blocking.fraction_of_follow_speed_at_race_start", &id) == SWOFTY_ASSET_OK &&
               id == SWOFTY_CAR_AI_BLOCKING_FRACTION_RACE_START,
           "the race start fraction resolves by path");

    int r = 0;
    get(0, SWOFTY_CAR_AI_BLOCKING_FRACTION, &r);
    expect(r == SWOFTY_ASSET_UNAVAILABLE, "get is unavailable before the AI system is captured");
    uint32_t t1 = 0;
    expect(Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 40.0f, 0, &t1) == SWOFTY_ASSET_OK,
           "a layer made before the capture is accepted");
    expect(writes == 0, "nothing is written before the AI system is captured");

    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w.system));
    for (uint32_t i = 0; i < rc::SETTLE_FRAMES - 2; i++)
        Ops::racecraft_tick();
    Ops::flush();
    expect(writes == 0, "nothing is written while the build is still settling");
    uint64_t original1 = slot_of(w, 1, SLOT2);
    settle();
    expect(slot_of(w, 1, SLOT2) != original1, "once settled the car's slot points at an own cell");
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 40.0f, "get returns the value the AI reads");
    expect(db_float(w, 1, 2) == 30.0f, "the game's cell is never written");
    expect(slot_of(w, 0, SLOT2) == uint64_t(reinterpret_cast<uintptr_t>(&w.db[0][2].cell)) && get(0, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 30.0f,
           "another car's slot is untouched");
    expect(slot_of(w, 1, SLOT0) == uint64_t(reinterpret_cast<uintptr_t>(&w.db[1][0].cell)), "another field's slot is untouched");
    {
        uint64_t cellp = slot_of(w, 1, SLOT2), recp = 0;
        memcpy(&recp, reinterpret_cast<void *>(cellp), 8);
        expect(recp != 0 && recp != reinterpret_cast<uintptr_t>(w.db[1][2].record) &&
                   reinterpret_cast<uint8_t *>(recp)[0x10] == 0xa2,
               "the own record is a copy of the original's first bytes");
    }

    uint32_t t2 = 0, t3 = 0;
    Ops::op_layer("a", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SCALE, 0.5f, 5, &t2);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 20.0f, "layers compose by priority: set 40 then scale 0.5");
    Ops::op_layer("b", SWOFTY_CAR_ALL, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_ADD, -0.1f, 0, &t3);
    expect(get(0, SWOFTY_CAR_AI_BLOCKING_FRACTION) > 0.774f && get(0, SWOFTY_CAR_AI_BLOCKING_FRACTION) < 0.776f &&
               get(2, SWOFTY_CAR_AI_BLOCKING_FRACTION) > 0.774f && get(2, SWOFTY_CAR_AI_BLOCKING_FRACTION) < 0.776f,
           "an all-cars add layer uses each car's own original as the base");
    expect(db_float(w, 0, 0) == 0.875f, "still no write to the game's cells");

    Ops::op_unlayer(t2);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 40.0f, "dropping one layer recomposes the rest");
    Ops::op_unlayer(t1);
    expect(slot_of(w, 1, SLOT2) == original1, "the last layer going puts the original pointer back");
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 30.0f, "the AI reads the game's value again");
    Ops::op_unlayer(t3);
    expect(slot_of(w, 0, SLOT0) == uint64_t(reinterpret_cast<uintptr_t>(&w.db[0][0].cell)), "all slots are back");

    int before = writes;
    uint32_t t4 = 0;
    Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 55.0f, 0, &t4);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 55.0f && writes == before + 1,
           "a second swap writes the slot once and reuses the cell");
    uint64_t swapped = slot_of(w, 1, SLOT2);
    {
        uint8_t *old = rec_at(w, 1);
        std::vector<uint8_t> moved(w.records.size() + 4 * rc::STRIDE, 0);
        memcpy(moved.data(), w.records.data(), w.records.size());
        w.records.swap(moved);
        fill_system(w, CARS);
        (void)old;
    }
    Ops::racecraft_tick();
    expect(slot_of(w, 1, SLOT2) == swapped && get(1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE) == 55.0f,
           "records moved by a regrow keep their swapped slot");
    Ops::op_unlayer(t4);
    expect(slot_of(w, 1, SLOT2) == original1, "a layer dropped after the records moved restores the original in the new buffer");

    rec_at(w, 2)[rc::R_VEHICLE_INDEX] = 9;
    before = writes;
    uint32_t t5 = 0;
    Ops::op_layer("lap1", 2, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 55.0f, 0, &t5);
    int rr = 0;
    get(2, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, &rr);
    expect(writes == before && rr == SWOFTY_ASSET_UNAVAILABLE, "a record whose vehicle index is not the car's is refused");
    rec_at(w, 2)[rc::R_VEHICLE_INDEX] = 2;
    Ops::op_unlayer(t5);

    uint32_t t6 = 0;
    Ops::op_layer("lap1", 0, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.5f, 0, &t6);
    uint64_t game_ptr = reinterpret_cast<uintptr_t>(&w.db[2][0].cell);
    memcpy(rec_at(w, 0) + SLOT0, &game_ptr, 8);
    Ops::op_unlayer(t6);
    expect(slot_of(w, 0, SLOT0) == game_ptr, "a slot the game rewrote is not overwritten on restore");
    {
        uint64_t original = reinterpret_cast<uintptr_t>(&w.db[0][0].cell);
        memcpy(rec_at(w, 0) + SLOT0, &original, 8);
    }

    uint32_t t7 = 0;
    flow = false;
    before = writes;
    Ops::op_layer("addon", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.6f, 0, &t7);
    expect(writes == before, "a layer from another thread writes nothing");
    flow = true;
    Ops::flush();
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION) == 0.6f, "the next flow thread flush applies it");
    expect(Ops::op_set(1, nullptr, SWOFTY_CAR_AI_BLOCKING_FRACTION, 0.7f) == SWOFTY_ASSET_UNAVAILABLE,
           "set is not offered, layers only");

    Ops::racecraft_forget();
    expect(Ops::racecraft.retired_cells() > 0, "a forgotten session retires its cells instead of freeing them");
    int h = 0;
    get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION, &h);
    expect(h == SWOFTY_ASSET_UNAVAILABLE, "after the reset nothing is read until the next AI system");
    World w2;
    build(w2);
    std::vector<uint8_t> w1_records = w.records;
    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    settle();
    expect(w.records == w1_records, "the old session's records are never touched after the reset");
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION) == 0.6f &&
               slot_of(w2, 1, SLOT0) != uint64_t(reinterpret_cast<uintptr_t>(&w2.db[1][0].cell)),
           "a layer still held applies to the next session's cars");
    Ops::op_unlayer(t7);
    expect(slot_of(w2, 1, SLOT0) == uint64_t(reinterpret_cast<uintptr_t>(&w2.db[1][0].cell)), "and restores them");

    uint32_t t8 = 0;
    Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t8);
    before = writes;
    fill_system(w2, 2);
    Ops::racecraft_tick();
    Ops::flush();
    get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION, &h);
    expect(h == SWOFTY_ASSET_UNAVAILABLE && writes == before, "a changed record count drops all state and writes nothing");
    fill_system(w2, CARS);
    Ops::op_unlayer(t8);

    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    settle();
    uint32_t t9 = 0;
    Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t9);
    expect(get(1, SWOFTY_CAR_AI_BLOCKING_FRACTION) == 0.1f, "a push at an unchanged count keeps the state");
    dead.insert(reinterpret_cast<uintptr_t>(w2.system) & ~uintptr_t(0xffff));
    before = writes;
    Ops::racecraft_tick();
    Ops::op_unlayer(t9);
    expect(writes == before, "an unreadable AI system is forgotten and not written to");
    dead.clear();

    Ops::bind(make_env());
    Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w2.system));
    settle();
    rows_off.insert("AiCarRecord.driver_config");
    uint32_t t10 = 0;
    expect(Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t10) == SWOFTY_ASSET_UNAVAILABLE,
           "the fields are off until their KB rows have an agree verify row");
    rows_off.clear();
    rows_off.insert("AiDriverConfig.blocking_max_blocking_distance");
    expect(Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, SWOFTY_CAR_OP_SET, 10.0f, 0, &t10) ==
                   SWOFTY_ASSET_UNAVAILABLE &&
               Ops::op_layer("lap1", 1, SWOFTY_CAR_AI_BLOCKING_FRACTION, SWOFTY_CAR_OP_SET, 0.1f, 0, &t10) == SWOFTY_ASSET_OK,
           "each field has its own row");
    rows_off.clear();

    {
        constexpr uint32_t QUEUE = SWOFTY_CAR_AI_SPACE_SEEKING_TOTAL_QUEUE_NEEDED, OVERLAP = SWOFTY_CAR_AI_OVERTAKE_OVERLAP_PERCENTAGE,
                           SLOWDOWN = SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SLOWDOWN_SPEED_DIFF;
        constexpr uintptr_t QUEUE_SLOT = rc::R_DRIVER + 0x268, OVERLAP_SLOT = rc::R_DRIVER + 0x1d0, SLOWDOWN_SLOT = rc::R_DRIVER + 0x1e0;

        expect(rc::FIELD_COUNT == 59, "the driver table has the four blocking floats, 43 more and 12 more blocking keys");
        bool table_ok = true;
        for (auto &f : cl::FIELDS) {
            if (f.kind != cl::Kind::cell)
                continue;
            const rc::Config &c = rc::DRIVER[f.vslot];
            table_ok = table_ok && f.vslot < rc::FIELD_COUNT && std::string("ai.") + c.key == f.name && (c.offset & 7) == 0 &&
                       c.offset < 0x2a0 && rc::slot(c.key) == f.vslot;
        }
        expect(table_ok, "every cell field names its table row, offsets are slots inside the 0x2a0 driver config");
        std::set<uint32_t> ids;
        std::set<uintptr_t> offsets;
        int cells = 0;
        for (auto &f : cl::FIELDS)
            if (f.kind == cl::Kind::cell) {
                cells++;
                ids.insert(f.id);
                offsets.insert(rc::DRIVER[f.vslot].offset);
            }
        expect(cells == 59 && ids.size() == 59 && offsets.size() == 59, "ids and driver offsets are unique");
        expect(rc::DRIVER[rc::slot("overtake.overlap_percentage")].offset == 0x1d0 &&
                   rc::DRIVER[rc::slot("overtake.overlap_percentage")].type == rc::Type::curve &&
                   rc::DRIVER[rc::slot("space_seeking.total_queue_needed")].type == rc::Type::i32 &&
                   rc::DRIVER[rc::slot("blocking.max_blocking_distance")].offset == 0xa0 &&
                   rc::DRIVER[rc::slot("blocking.start_blocking_intersection_time")].offset == 0x40 &&
                   rc::DRIVER[rc::slot("blocking.start_blocking_intersection_time")].type == rc::Type::flt &&
                   rc::DRIVER[rc::slot("blocking.start_blocking_minimum_time_to_corner")].offset == 0x50 &&
                   rc::DRIVER[rc::slot("blocking.start_blocking_minimum_time_to_corner")].type == rc::Type::curve &&
                   rc::DRIVER[rc::slot("blocking.blocking_offset_from_car_behind")].offset == 0x70 &&
                   rc::DRIVER[rc::slot("blocking.blocking_offset_from_car_behind")].type == rc::Type::curve &&
                   rc::DRIVER[rc::slot("blocking.avoidance_decrease_rate")].offset == 0xb0 &&
                   rc::DRIVER[rc::slot("blocking.merge_back_to_racing_line")].offset == 0xb8 &&
                   rc::DRIVER[rc::slot("blocking.merge_back_to_racing_line")].type == rc::Type::curve &&
                   rc::slot("overtake.nothing") == rc::FIELD_COUNT,
               "table lookup by key gives the offset and type");
        uint32_t qid = 0;
        expect(Ops::op_field("ai.space_seeking.total_queue_needed", &qid) == SWOFTY_ASSET_OK && qid == QUEUE &&
                   Ops::op_field("ai.alongside.min_presence_within_buffer", &qid) == SWOFTY_ASSET_OK &&
                   Ops::op_field("ai.overtake.oppertunist_overtake_track_speed_difference", &qid) == SWOFTY_ASSET_OK &&
                   Ops::op_field("ai.blocking.blocking_threat_time_modifier", &qid) == SWOFTY_ASSET_OK &&
                   qid == SWOFTY_CAR_AI_BLOCKING_BLOCKING_THREAT_TIME_MODIFIER,
               "the new fields resolve by path");

        World w3;
        build(w3);
        Ops::bind(make_env());
        Ops::racecraft_pushed(reinterpret_cast<uintptr_t>(w3.system));
        settle();
        auto slot3 = [&](int car, uintptr_t off) { return slot_of(w3, car, off); };
        auto db3 = [&](int car, int s) { return uint64_t(reinterpret_cast<uintptr_t>(&w3.extra[car][s].cell)); };
        auto bytes_of = [&](int car, int s) { return std::vector<uint8_t>(w3.extra[car][s].record, w3.extra[car][s].record + 0x80); };

        expect(get(1, QUEUE) == 7.0f && get(1, SLOWDOWN) == 12.5f, "get reads an int cell as its integer and a float cell as its float");
        expect(get(1, OVERLAP) == 49.0f, "get on an untouched curve field is the curve at x = 0 (LUT sample 96 * 0.5 + 1)");

        std::vector<uint8_t> stock_int = bytes_of(1, 0), stock_curve = bytes_of(1, 1), stock_float = bytes_of(1, 2);
        uint32_t ta = 0, tb = 0, tc = 0;
        Ops::op_layer("a", 1, QUEUE, SWOFTY_CAR_OP_SET, 3.4f, 0, &ta);
        expect(slot3(1, QUEUE_SLOT) != db3(1, 0) && get(1, QUEUE) == 3.0f, "an int field is written as an integer, rounded to nearest");
        {
            uint64_t cellp = slot3(1, QUEUE_SLOT), recp = 0;
            int32_t n = 0;
            memcpy(&recp, reinterpret_cast<void *>(cellp), 8);
            memcpy(&n, reinterpret_cast<void *>(recp + 8), 4);
            expect(n == 3 && reinterpret_cast<uint8_t *>(recp)[0x7f] == 0xe0, "the int cell holds 3 in the game's own layout");
        }
        Ops::op_layer("a2", 1, QUEUE, SWOFTY_CAR_OP_SET, 3.6f, 1, &tb);
        expect(get(1, QUEUE) == 4.0f, "3.6 rounds to 4");
        Ops::op_unlayer(tb);
        Ops::op_layer("c", 1, SLOWDOWN, SWOFTY_CAR_OP_ADD, 2.5f, 0, &tc);
        expect(get(1, SLOWDOWN) == 15.0f, "a float field composes on the game's value");
        expect(bytes_of(1, 0) == stock_int && bytes_of(1, 2) == stock_float, "the game's int and float cells are never written");

        uint32_t tcur = 0;
        Ops::op_layer("lap1", 1, OVERLAP, SWOFTY_CAR_OP_SET, 5.0f, 0, &tcur);
        uint64_t curp = slot3(1, OVERLAP_SLOT);
        expect(curp != db3(1, 1) && get(1, OVERLAP) == 5.0f, "a layer on a curve field points the slot at an own cell");
        uint64_t recp = 0;
        memcpy(&recp, reinterpret_cast<void *>(curp), 8);
        const uint8_t *own = reinterpret_cast<const uint8_t *>(recp);
        bool flat = true;
        for (float x : {0.0f, 0.37f, 1.0f, -2.0f, 0.5f, 100.0f, 1e-7f, 0.9999f, 3.4e38f})
            flat = flat && rc::curve_at(own, x) == 5.0f;
        expect(flat, "the flat curve evaluates to the value at x = 0, 0.37, 1 and beyond, by the game's evaluator arithmetic");
        expect(rc::float_at(own + 0x18) == 0.0f && rc::float_at(own + 0x24) == 5.0f && rc::float_at(own + 0x1c) == 5.0f &&
                   rc::float_at(own + 0x28) == 5.0f,
               "step 0, min, last value and max are the value");
        bool lut_zero = true;
        for (int i = 0; i < 65; i++)
            lut_zero = lut_zero && own[0x2c + i] == 0;
        expect(lut_zero, "the 64-sample LUT and the interpolation sample after it are zero");
        expect(rc::float_at(own + 0x10) == 2.0f && rc::float_at(own + 0x14) == -0.25f && own[0x20] == 0xc0 && own[0x6f] == 0xd1,
               "scale, offset and the rest of the record are the original's");
        expect(bytes_of(1, 1) == stock_curve, "the game's curve record is never written");
        {
            const uint8_t *orig = w3.extra[1][1].record;
            bool matches = rc::curve_at(orig, 0.0f) == 49.0f && rc::curve_at(orig, 1.0f) == 3.0f &&
                           rc::curve_at(orig, 0.37f) == 3.0f && std::fabs(rc::curve_at(orig, 0.1f) - 68.2f) < 0.01f;
            expect(matches, "the evaluator reproduces the original curve: x = 0 mid-table, 0.1 interpolated, 0.37 and 1 the last value");
        }
        uint32_t tscale = 0;
        Ops::op_layer("s", 1, OVERLAP, SWOFTY_CAR_OP_SCALE, 2.0f, 5, &tscale);
        expect(get(1, OVERLAP) == 10.0f, "layers compose on a curve from the value the set layer gave");
        Ops::op_unlayer(tcur);
        expect(get(1, OVERLAP) == 98.0f, "dropping the set layer leaves the scale on the curve's stock value at x = 0 (49 * 2)");
        Ops::op_unlayer(tscale);
        expect(slot3(1, OVERLAP_SLOT) == db3(1, 1) && get(1, OVERLAP) == 49.0f, "the last curve layer going puts the game's curve back");
        Ops::op_unlayer(ta);
        Ops::op_unlayer(tc);
        expect(slot3(1, QUEUE_SLOT) == db3(1, 0) && slot3(1, SLOWDOWN_SLOT) == db3(1, 2) && get(1, QUEUE) == 7.0f &&
                   get(1, SLOWDOWN) == 12.5f,
               "every slot is back and the AI reads the game's values");

        uint32_t tclamp = 0;
        Ops::op_layer("c", 1, OVERLAP, SWOFTY_CAR_OP_SET, 1000.0f, 0, &tclamp);
        expect(get(1, OVERLAP) == 100.0f, "a curve value is clamped to the field's range");
        Ops::op_unlayer(tclamp);

        put(w3.extra[2][2].record + 8, 0.0f);
        uint32_t bits = 3;
        memcpy(w3.extra[2][2].record + 8, &bits, 4);
        int before3 = writes;
        uint32_t tbad = 0;
        int gr = 0;
        get(2, SLOWDOWN, &gr);
        Ops::op_layer("bad", 2, SLOWDOWN, SWOFTY_CAR_OP_SET, 1.0f, 0, &tbad);
        expect(gr == SWOFTY_ASSET_UNAVAILABLE && writes == before3 && slot3(2, SLOWDOWN_SLOT) == db3(2, 2),
               "a float field whose cell holds a denormal (an int read as a float) is refused and not written");
        Ops::op_unlayer(tbad);
        uint32_t bigint = 0x3f800000u;
        memcpy(w3.extra[2][0].record + 8, &bigint, 4);
        Ops::op_layer("bad", 2, QUEUE, SWOFTY_CAR_OP_SET, 2.0f, 0, &tbad);
        get(2, QUEUE, &gr);
        expect(gr == SWOFTY_ASSET_UNAVAILABLE && writes == before3 && slot3(2, QUEUE_SLOT) == db3(2, 0),
               "an int field whose cell holds float bits is refused and not written");
        Ops::op_unlayer(tbad);
        put(w3.extra[2][1].record + 0x18, std::nanf(""));
        Ops::op_layer("bad", 2, OVERLAP, SWOFTY_CAR_OP_SET, 2.0f, 0, &tbad);
        get(2, OVERLAP, &gr);
        expect(gr == SWOFTY_ASSET_UNAVAILABLE && writes == before3 && slot3(2, OVERLAP_SLOT) == db3(2, 1),
               "a curve field whose record has a non-finite step is refused and not written");
        Ops::op_unlayer(tbad);

        rows_off.insert("AiDriverConfig.overtake_overlap_percentage");
        uint32_t tg1 = 0, tg2 = 0;
        expect(Ops::op_layer("g", 1, OVERLAP, SWOFTY_CAR_OP_SET, 5.0f, 0, &tg1) == SWOFTY_ASSET_UNAVAILABLE &&
                   Ops::op_layer("g", 1, QUEUE, SWOFTY_CAR_OP_SET, 5.0f, 0, &tg2) == SWOFTY_ASSET_OK,
               "a new field is gated by its own KB row");
        rows_off.clear();
        Ops::op_unlayer(tg2);
        for (const char *row : {"AiDriverConfig.space_seeking_total_queue_needed", "AiDriverConfig.alongside_yield_overtake_rate"}) {
            rows_off.insert(row);
        }
        uint32_t tg3 = 0;
        expect(Ops::op_layer("g", 1, QUEUE, SWOFTY_CAR_OP_SET, 5.0f, 0, &tg3) == SWOFTY_ASSET_UNAVAILABLE &&
                   Ops::op_layer("g", 1, SLOWDOWN, SWOFTY_CAR_OP_SET, 5.0f, 0, &tg3) == SWOFTY_ASSET_OK,
               "each row gates only its field");
        rows_off.clear();
        Ops::op_unlayer(tg3);
        Ops::racecraft_forget();
    }

    printf("%s\n", fails ? "FAILED" : "all ok");
    return fails ? 1 : 0;
}
