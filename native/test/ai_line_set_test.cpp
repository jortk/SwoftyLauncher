// Host-native unit test for the core's AI line set guard (native/src/core/ai_line_set_logic.hpp).
// Built and run by native/test/run_ai_line_set_test.sh.

#include "ai_line_set_logic.hpp"

#include <cstdio>
#include <map>

namespace al = runtime::ai_line_set;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

struct Memory {
    std::map<uint64_t, uint64_t> words;
};

static bool read(uint64_t address, void *out, size_t n, void *user)
{
    Memory *m = static_cast<Memory *>(user);
    if (n != 8)
        return false;
    auto it = m->words.find(address);
    if (it == m->words.end())
        return false;
    *static_cast<uint64_t *>(out) = it->second;
    return true;
}

static bool write(uint64_t address, const void *in, size_t n, void *user)
{
    Memory *m = static_cast<Memory *>(user);
    if (n != 8 || !m->words.count(address))
        return false;
    m->words[address] = *static_cast<const uint64_t *>(in);
    return true;
}

constexpr uint64_t BUILDER = 0x200000;
constexpr uint64_t RECORDS = 0x300000;

static uint64_t brake_id(unsigned record)
{
    return 0x7963c8f708f50000ull + record;
}

static Memory build(unsigned records)
{
    Memory m;
    m.words[BUILDER + al::RECORDS_BEGIN_AT] = RECORDS;
    m.words[BUILDER + al::RECORDS_END_AT] = RECORDS + records * al::RECORD_SIZE;
    for (unsigned i = 0; i < records; i++) {
        m.words[RECORDS + i * al::RECORD_SIZE + al::BRAKE_SETTINGS_ID_AT] = brake_id(i);
        for (uint64_t slot : al::SLOTS)
            m.words[RECORDS + i * al::RECORD_SIZE + slot] = 0x1000000 + i * 0x1000 + slot;
    }
    return m;
}

static uint64_t slot(Memory &m, unsigned record, uint64_t s)
{
    return m.words[RECORDS + record * al::RECORD_SIZE + s];
}

int main()
{
    {
        Memory m = build(21);
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.ran && r.filled == 0 && r.missing == 0, "a record with all six pointers is left alone");
        expect(r.index == 20, "the last record is the one pushed");
    }
    {
        Memory m = build(21);
        for (uint64_t s : al::SLOTS)
            m.words[RECORDS + 20 * al::RECORD_SIZE + s] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 6 && r.missing == 0, "a third car with no pointers borrows all six");
        expect(slot(m, 20, 0x58) == slot(m, 0, 0x58), "it borrows the first car's line cache");
        expect(slot(m, 20, 0x80) == slot(m, 0, 0x80), "it borrows the first car's last table");
        expect(slot(m, 19, 0x58) != 0 && slot(m, 19, 0x58) != slot(m, 0, 0x58), "the other records are untouched");
    }
    {
        Memory m = build(22);
        for (unsigned i : {17u, 19u})
            for (uint64_t s : al::SLOTS)
                m.words[RECORDS + i * al::RECORD_SIZE + s] = 0;
        m.words[BUILDER + al::RECORDS_END_AT] = RECORDS + 18 * al::RECORD_SIZE;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.index == 17 && r.filled == 6 && r.brake_settings, "an eleventh team's car borrows the tables and the brake line settings");
        expect(slot(m, 17, al::BRAKE_SETTINGS_ID_AT) == brake_id(0), "its brake settings id is the one of the car whose tables it borrows");
        expect(slot(m, 16, al::BRAKE_SETTINGS_ID_AT) == brake_id(16) && slot(m, 0, al::BRAKE_SETTINGS_ID_AT) == brake_id(0), "listed cars keep their own brake settings id");
        m.words[BUILDER + al::RECORDS_END_AT] = RECORDS + 20 * al::RECORD_SIZE;
        r = al::repair(read, write, &m, BUILDER);
        expect(r.index == 19 && r.brake_settings && slot(m, 19, al::BRAKE_SETTINGS_ID_AT) == brake_id(0), "its team mate borrows the same brake settings id");
        expect(slot(m, 17, al::BRAKE_SETTINGS_ID_AT) == brake_id(0) && slot(m, 18, al::BRAKE_SETTINGS_ID_AT) == brake_id(18), "a later push leaves the earlier records as they were");
    }
    {
        Memory m = build(5);
        m.words[RECORDS + 4 * al::RECORD_SIZE + 0x60] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 1 && !r.brake_settings && slot(m, 4, al::BRAKE_SETTINGS_ID_AT) == brake_id(4), "a listed car with one empty table keeps its brake settings id");
    }
    {
        Memory m = build(6);
        for (unsigned i = 0; i < 3; i++)
            for (uint64_t s : al::SLOTS)
                m.words[RECORDS + i * al::RECORD_SIZE + s] = 0;
        al::repair(read, write, &m, BUILDER);
        expect(slot(m, 0, al::BRAKE_SETTINGS_ID_AT) == brake_id(3) && slot(m, 2, al::BRAKE_SETTINGS_ID_AT) == brake_id(3), "backfilled first cars take the brake settings id of the donor");
    }
    {
        Memory m = build(3);
        for (unsigned i = 0; i < 3; i++)
            for (uint64_t s : al::SLOTS)
                m.words[RECORDS + i * al::RECORD_SIZE + s] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(!r.brake_settings && slot(m, 2, al::BRAKE_SETTINGS_ID_AT) == brake_id(2), "with no donor the brake settings id is kept");
    }
    {
        Memory m = build(3);
        for (uint64_t s : al::SLOTS)
            m.words[RECORDS + 2 * al::RECORD_SIZE + s] = 0;
        m.words[RECORDS + al::BRAKE_SETTINGS_ID_AT] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 6 && !r.brake_settings && slot(m, 2, al::BRAKE_SETTINGS_ID_AT) == brake_id(2), "a donor with no brake settings id lends none");
    }
    {
        Memory m = build(5);
        m.words[RECORDS + 4 * al::RECORD_SIZE + 0x78] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 0 && r.missing == 0 && slot(m, 4, 0x78) == 0, "a listed car's empty optional table (+0x78) is left alone");
    }
    {
        Memory m = build(5);
        m.words[RECORDS + 4 * al::RECORD_SIZE + 0x60] = 0;
        m.words[RECORDS + 0 * al::RECORD_SIZE + 0x60] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 1 && slot(m, 4, 0x60) == slot(m, 1, 0x60), "only the null pointer is replaced, from the first record that has one");
    }
    {
        Memory m = build(1);
        m.words[RECORDS + 0x58] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.ran && r.filled == 0 && r.missing == 1 && slot(m, 0, 0x58) == 0, "with no donor the slot stays empty and is counted");
    }
    {
        Memory m = build(6);
        for (unsigned i = 0; i < 3; i++)
            for (uint64_t s : al::SLOTS)
                m.words[RECORDS + i * al::RECORD_SIZE + s] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 0 && r.backfilled == 3, "first-slot cars with no pointers are backfilled when a later record has them");
        expect(slot(m, 0, 0x58) == slot(m, 3, 0x58) && slot(m, 2, 0x80) == slot(m, 3, 0x80), "they borrow the first record that has pointers");
        expect(slot(m, 3, 0x58) == 0x1000000 + 3 * 0x1000 + 0x58, "the donor is untouched");
    }
    {
        Memory m = build(3);
        for (unsigned i = 0; i < 3; i++)
            for (uint64_t s : al::SLOTS)
                m.words[RECORDS + i * al::RECORD_SIZE + s] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.ran && r.filled == 0 && r.missing == 6 && r.backfilled == 0, "with every car missing nothing is written");
        bool clean = true;
        for (unsigned i = 0; i < 3; i++)
            for (uint64_t s : al::SLOTS)
                clean = clean && slot(m, i, s) == 0;
        expect(clean, "all records stay zero");
    }
    {
        Memory m = build(6);
        for (unsigned i : {0u, 1u, 4u, 5u})
            for (uint64_t s : al::SLOTS)
                m.words[RECORDS + i * al::RECORD_SIZE + s] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.filled == 6 && r.backfilled == 3, "a mixed vector fills the last record and the earlier empty ones");
        expect(slot(m, 0, 0x60) == slot(m, 2, 0x60) && slot(m, 5, 0x60) == slot(m, 2, 0x60), "both borrow the earliest car that has pointers");
    }
    {
        Memory m = build(4);
        m.words[RECORDS + 1 * al::RECORD_SIZE + 0x78] = 0;
        al::Repair r = al::repair(read, write, &m, BUILDER);
        expect(r.backfilled == 0 && slot(m, 1, 0x78) == 0, "an earlier listed car's empty optional table stays empty");
    }
    {
        Memory m = build(3);
        m.words[BUILDER + al::RECORDS_END_AT] = RECORDS + 3 * al::RECORD_SIZE + 8;
        expect(!al::repair(read, write, &m, BUILDER).ran, "a vector whose size is not a whole number of records is ignored");
    }
    {
        Memory m = build(3);
        m.words[BUILDER + al::RECORDS_END_AT] = RECORDS;
        expect(!al::repair(read, write, &m, BUILDER).ran, "an empty vector is ignored");
    }
    {
        Memory m;
        expect(!al::repair(read, write, &m, BUILDER).ran, "an unreadable builder is ignored");
        expect(!al::repair(read, write, &m, 0).ran, "a null builder is ignored");
    }
    return failed ? 1 : 0;
}
