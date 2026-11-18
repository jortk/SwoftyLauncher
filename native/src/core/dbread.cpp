#include "runtime.hpp"

#include <swofty/mixin.hpp>

#include <swofty/mem.hpp>

#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

#include "dbread_logic.hpp"
#include "layout.hpp"

namespace {

namespace layout = runtime::layout;

const runtime::Sig S_GLOBAL{"g_Gdb_Database", "56 57 55 53 49 B8 25 23 22 84 E4 9C F2 CB 48 8B 15 ?? ?? ?? ?? 44 0F B6 11",
                            0x6cc4278, runtime::Sig::riprel, 0xe, 0x11, 7};
const runtime::Sig S_INSERT{"Gdb_HashMap_Insert",
                            "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC D8 00 00 00 C5 F8 29 B4 24 C0 00 00 00 4C 89 CE 44 88 44 24 37",
                            0x45a12d0};

constexpr size_t DB_MAP = 0xa8, DB_LOADED = 0xf0;

const uintptr_t KB_VT[6] = {0x5bee788, 0x5bee798, 0x5bee6b8, 0x5bee6b8, 0x5bee6b8, 0x56e6518};
const int PROBE_TAGS[] = {SWOFTY_DB_INT, SWOFTY_DB_FLOAT, SWOFTY_DB_BOOL, SWOFTY_DB_STRING};

using Insert_t = void *(void *db, uint64_t key, uint8_t tag, char *text);

std::once_flag resolved;
void **g_db;
Insert_t *insert_fn;
uintptr_t base;
std::atomic<bool> initialised;

std::mutex probe_lock;
void *probe_vt[6];
bool probed;

void resolve()
{
    base = swofty::api->module_base();
    g_db = reinterpret_cast<void **>(runtime::find(S_GLOBAL, "dbread"));
    insert_fn = reinterpret_cast<Insert_t *>(runtime::find(S_INSERT, "dbread"));
    initialised = true;
}

uint8_t *database()
{
    if (!runtime::code_ready())
        return nullptr;
    std::call_once(resolved, resolve);
    return g_db ? static_cast<uint8_t *>(*g_db) : nullptr;
}

bool loaded(uint8_t *db)
{
    return db && runtime::readable(db + DB_LOADED, 1) && db[DB_LOADED];
}

void *record(uint64_t key)
{
    uint8_t *db = database();
    if (!loaded(db))
        return nullptr;
    runtime::Robin map{db + DB_MAP};
    if (!map.sane())
        return nullptr;
    uint64_t *slot = map.find(key);
    return slot ? reinterpret_cast<void *>(slot[1]) : nullptr;
}

// On a build other than the analysed one the per-tag record classes are unknown, so learn them
// once by inserting a probe record per tag and reading its vtable back. This mutates the database
// and runs only at the window stage, the same controlled point gamedb probes; the analysed build
// never reaches it (it classifies by KB_VT).
void ensure_probe()
{
    if (probed || runtime::same_build())
        return;
    std::lock_guard<std::mutex> g(probe_lock);
    if (probed)
        return;
    uint8_t *db = database();
    if (loaded(db) && insert_fn)
        for (int t : PROBE_TAGS) {
            std::string path = "swofty.dbread.probe.t" + std::to_string(t);
            char text[] = "0";
            insert_fn(db, dbread::fnv64(path), uint8_t(t), text);
            void *v = record(dbread::fnv64(path));
            probe_vt[t] = v ? *static_cast<void **>(v) : nullptr;
        }
    probed = true;
}

int classify(void *rec)
{
    if (!rec)
        return -1;
    void *vt = *static_cast<void **>(rec);
    if (runtime::same_build()) {
        uintptr_t rva = reinterpret_cast<uintptr_t>(vt) - base;
        for (int t = 0; t < 6; t++)
            if (rva == KB_VT[t])
                return t == SWOFTY_DB_ENUM || t == SWOFTY_DB_ID ? SWOFTY_DB_STRING : t;
        return -1;
    }
    for (int t : PROBE_TAGS)
        if (probe_vt[t] && probe_vt[t] == vt)
            return t;
    return -1;
}

// String records (GdbStringRecord, k_Gdb_StringXorKey). The text is XOR-encoded in the record;
// the game decodes it into the record's own buffer at each read. The core decodes into its own
// copy instead and interns it per record, so a returned pointer stays valid for the session. The
// table is bounded; past the bound a string lives in a per-thread ring until the next db call.

constexpr size_t INTERN_MAX = 32768;
constexpr size_t INTERN_BYTES = size_t(8) << 20;
constexpr size_t RING = 8;

struct Interned {
    uint64_t size;
    uintptr_t enc;
    std::unique_ptr<char[]> text;
};

std::once_flag key_once;
const uint8_t *xor_key;
std::mutex text_lock;
std::unordered_map<uintptr_t, Interned> interned;
std::vector<std::unique_ptr<char[]>> retired;
size_t interned_bytes;
bool intern_full_logged;

const uint8_t *text_key()
{
    if (!runtime::code_ready())
        return nullptr;
    std::call_once(key_once, [] {
        if (!runtime::enabled("db", nullptr))
            return;
        uintptr_t k = swofty::api->resolve("k_Gdb_StringXorKey");
        xor_key = k && swofty::mem::readable(k, 4) ? reinterpret_cast<const uint8_t *>(k) : nullptr;
        if (!layout::GDB_STRING)
            swofty::log("[dbread] string reads unavailable: this core was built without the GdbStringRecord layout");
        else if (!xor_key)
            swofty::log("[dbread] string reads unavailable: k_Gdb_StringXorKey does not resolve on this build");
    });
    return layout::GDB_STRING ? xor_key : nullptr;
}

template <class R = swofty::game::GdbStringRecord> bool record_text(const void *rec, uint64_t &size, uintptr_t &enc)
{
    if constexpr (layout::gdb_string<R>) {
        auto at = reinterpret_cast<uintptr_t>(rec);
        return swofty::mem::read(at + offsetof(R, enc_size), size) && swofty::mem::read(at + offsetof(R, enc), enc);
    } else {
        (void)rec, (void)size, (void)enc;
        return false;
    }
}

char *keep(std::string_view text)
{
    auto p = std::make_unique<char[]>(text.size() + 1);
    memcpy(p.get(), text.data(), text.size());
    p[text.size()] = 0;
    return p.release();
}

const char *ring(const std::string &text)
{
    thread_local std::string slots[RING];
    thread_local size_t next;
    std::string &s = slots[next++ % RING];
    s = text;
    return s.c_str();
}

const char *decoded(const void *rec)
{
    const uint8_t *key = text_key();
    uint64_t size = 0;
    uintptr_t enc = 0;
    if (!key || !rec || !record_text(rec, size, enc) || !size || size > dbread::TEXT_MAX || !enc ||
        !swofty::mem::readable(enc, size_t(size)))
        return nullptr;
    auto at = reinterpret_cast<uintptr_t>(rec);
    std::lock_guard<std::mutex> g(text_lock);
    auto it = interned.find(at);
    if (it != interned.end() && it->second.size == size && it->second.enc == enc)
        return it->second.text.get();
    std::string text;
    if (!dbread::decode_text(reinterpret_cast<const uint8_t *>(enc), size, key, text))
        return nullptr;
    if (it != interned.end() && !strcmp(it->second.text.get(), text.c_str())) {
        it->second.size = size, it->second.enc = enc;
        return it->second.text.get();
    }
    if (interned.size() >= INTERN_MAX || interned_bytes + text.size() + 1 > INTERN_BYTES) {
        if (!intern_full_logged)
            swofty::log("[dbread] string table full (%zu strings, %zu bytes); further strings are valid until the next "
                        "db call on the calling thread",
                        interned.size(), interned_bytes),
                intern_full_logged = true;
        return ring(text);
    }
    std::unique_ptr<char[]> copy(keep(text));
    interned_bytes += text.size() + 1;
    if (it != interned.end()) {
        retired.push_back(std::move(it->second.text));
        it->second = {size, enc, std::move(copy)};
        return it->second.text.get();
    }
    return interned.emplace(at, Interned{size, enc, std::move(copy)}).first->second.text.get();
}

int fill(void *rec, SwoftyDbValue *out)
{
    int tag = classify(rec);
    if (tag < 0)
        return SWOFTY_ASSET_UNAVAILABLE;
    dbread::decode(rec, tag, out);
    if (out->tag == SWOFTY_DB_STRING)
        out->s = decoded(rec);
    return SWOFTY_ASSET_OK;
}

int db_available()
{
    if (!runtime::enabled("db", nullptr))
        return 0;
    return loaded(database()) ? 1 : 0;
}

int db_get_hash(uint64_t hash, SwoftyDbValue *out)
{
    if (!out)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    void *rec = record(hash);
    if (!rec)
        return SWOFTY_ASSET_INVALID;
    return fill(rec, out);
}

int db_get(const char *key, SwoftyDbValue *out)
{
    if (!key || !*key)
        return SWOFTY_ASSET_INVALID;
    return db_get_hash(dbread::fnv64(key, strlen(key)), out);
}

void *scalar(const char *key)
{
    if (!key || !*key)
        return nullptr;
    return record(dbread::fnv64(key, strlen(key)));
}

int db_int_at(const char *key, int64_t *out)
{
    if (!key || !*key || !out)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    void *rec = scalar(key);
    if (!rec)
        return SWOFTY_ASSET_INVALID;
    *out = dbread::read_i32(rec);
    return SWOFTY_ASSET_OK;
}

int db_float_at(const char *key, double *out)
{
    if (!key || !*key || !out)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    void *rec = scalar(key);
    if (!rec)
        return SWOFTY_ASSET_INVALID;
    *out = double(dbread::read_f32(rec));
    return SWOFTY_ASSET_OK;
}

int db_string_at(const char *key, const char **out)
{
    if (!key || !*key || !out)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!text_key())
        return SWOFTY_ASSET_UNAVAILABLE;
    void *rec = scalar(key);
    if (!rec)
        return SWOFTY_ASSET_INVALID;
    const char *s = decoded(rec);
    if (!s)
        return SWOFTY_ASSET_INVALID;
    *out = s;
    return SWOFTY_ASSET_OK;
}

int db_bool_at(const char *key, int32_t *out)
{
    if (!key || !*key || !out)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    void *rec = scalar(key);
    if (!rec)
        return SWOFTY_ASSET_INVALID;
    *out = dbread::read_u8(rec) ? 1 : 0;
    return SWOFTY_ASSET_OK;
}

uint64_t row_count(const dbread::TableKeys &keys)
{
    void *rec = record(keys.count_key);
    return rec ? uint64_t(uint32_t(dbread::read_i32(rec))) : 0;
}

int db_open(const char *table, SwoftyDbCursor *cursor)
{
    if (!table || !*table || !cursor)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    dbread::TableKeys keys = dbread::table_keys(table);
    if (!keys.ok)
        return SWOFTY_ASSET_INVALID;
    cursor->table = keys.row_prefix;
    cursor->index = 0;
    cursor->count = row_count(keys);
    cursor->row = cursor->count ? record(dbread::label_key(keys.row_prefix, 0)) : nullptr;
    return cursor->count ? SWOFTY_ASSET_OK : SWOFTY_ASSET_LATE;
}

int db_row(const char *table, const char *label, SwoftyDbCursor *cursor)
{
    if (!table || !*table || !label || !*label || !cursor)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    dbread::TableKeys keys = dbread::table_keys(table);
    if (!keys.ok)
        return SWOFTY_ASSET_INVALID;
    uint64_t count = row_count(keys);
    int64_t i = dbread::find_row([](uint64_t key) { return record(key); }, keys.row_prefix, count, label);
    if (i < 0)
        return SWOFTY_ASSET_INVALID;
    cursor->table = keys.row_prefix;
    cursor->index = uint64_t(i);
    cursor->count = count;
    cursor->row = record(dbread::label_key(keys.row_prefix, uint64_t(i)));
    return SWOFTY_ASSET_OK;
}

int db_step(SwoftyDbCursor *cursor)
{
    if (!cursor)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    if (cursor->index + 1 >= cursor->count)
        return SWOFTY_ASSET_LATE;
    cursor->index++;
    cursor->row = record(dbread::label_key(cursor->table, cursor->index));
    return SWOFTY_ASSET_OK;
}

int db_cell(const SwoftyDbCursor *cursor, const char *field, SwoftyDbValue *out)
{
    if (!cursor || !field || !*field || !out)
        return SWOFTY_ASSET_INVALID;
    if (!db_available())
        return SWOFTY_ASSET_UNAVAILABLE;
    if (cursor->index >= cursor->count)
        return SWOFTY_ASSET_INVALID;
    void *rec = record(dbread::cell_key(cursor->table, cursor->index, field));
    if (!rec)
        return SWOFTY_ASSET_INVALID;
    return fill(rec, out);
}

const char *db_row_name(const SwoftyDbCursor *cursor)
{
    if (!cursor || !cursor->row || !db_available())
        return nullptr;
    return decoded(cursor->row);
}

const SwoftyDb table = {db_available, db_get,      db_get_hash, db_int_at, db_float_at, db_string_at,
                        db_bool_at,   db_open,     db_row,      db_step,   db_cell,     db_row_name};

const char *tag_name(uint8_t tag)
{
    switch (tag) {
    case SWOFTY_DB_INT:
        return "int";
    case SWOFTY_DB_FLOAT:
        return "float";
    case SWOFTY_DB_BOOL:
        return "bool";
    default:
        return "string";
    }
}

void inspect_key(const json::Value &j)
{
    std::string key = j["key"].str();
    std::string as = j["as"].str();
    if (key.empty())
        return;
    SwoftyDbValue v{};
    int r = db_get(key.c_str(), &v);
    if (r != SWOFTY_ASSET_OK) {
        swofty::log("[dbread] inspect %s as %s: get %s", key.c_str(), as.c_str(),
                    r == SWOFTY_ASSET_INVALID ? "invalid" : r == SWOFTY_ASSET_UNAVAILABLE ? "unavailable" : "late");
    } else if (v.tag == SWOFTY_DB_STRING) {
        swofty::log("[dbread] inspect %s as %s: get ok tag string s=\"%s\" meta=0x%08x", key.c_str(), as.c_str(),
                    v.s ? v.s : "(null)", unsigned(v.i));
    } else {
        swofty::log("[dbread] inspect %s as %s: get ok tag %s i=%lld f=%g", key.c_str(), as.c_str(), tag_name(v.tag),
                    (long long)v.i, v.f);
    }
    if (as == "int") {
        int64_t out = 0;
        int t = db_int_at(key.c_str(), &out);
        swofty::log("[dbread] inspect %s: int_at %s %lld", key.c_str(), t == SWOFTY_ASSET_OK ? "ok" : "invalid",
                    (long long)out);
    } else if (as == "float") {
        double out = 0;
        int t = db_float_at(key.c_str(), &out);
        swofty::log("[dbread] inspect %s: float_at %s %g", key.c_str(), t == SWOFTY_ASSET_OK ? "ok" : "invalid", out);
    } else if (as == "bool") {
        int32_t out = 0;
        int t = db_bool_at(key.c_str(), &out);
        swofty::log("[dbread] inspect %s: bool_at %s %s", key.c_str(), t == SWOFTY_ASSET_OK ? "ok" : "invalid",
                    out ? "true" : "false");
    } else if (as == "string") {
        const char *out = nullptr;
        int t = db_string_at(key.c_str(), &out);
        swofty::log("[dbread] inspect %s: string_at %s \"%s\"", key.c_str(),
                    t == SWOFTY_ASSET_OK ? "ok" : t == SWOFTY_ASSET_UNAVAILABLE ? "unavailable" : "invalid",
                    out ? out : "(null)");
    }
}

void inspect_table(const json::Value &j)
{
    std::string tbl = j["table"].str();
    std::string field = j["field"].str();
    if (tbl.empty() || field.empty())
        return;
    if (j["label"].type == json::Value::Str) {
        std::string label = j["label"].str();
        SwoftyDbCursor c{};
        int r = db_row(tbl.c_str(), label.c_str(), &c);
        if (r != SWOFTY_ASSET_OK) {
            swofty::log("[dbread] table %s row \"%s\": not found", tbl.c_str(), label.c_str());
            return;
        }
        SwoftyDbValue v{};
        db_cell(&c, field.c_str(), &v);
        const char *name = db_row_name(&c);
        swofty::log("[dbread] table %s row \"%s\": ok index %llu name=\"%s\" %s i=%lld", tbl.c_str(), label.c_str(),
                    (unsigned long long)c.index, name ? name : "(null)", field.c_str(), (long long)v.i);
        return;
    }
    SwoftyDbCursor c{};
    int r = db_open(tbl.c_str(), &c);
    swofty::log("[dbread] table %s field %s: open %s, %llu row(s)", tbl.c_str(), field.c_str(),
                r == SWOFTY_ASSET_OK ? "ok" : r == SWOFTY_ASSET_LATE ? "late" : "unavailable",
                (unsigned long long)c.count);
    for (; r == SWOFTY_ASSET_OK; r = db_step(&c)) {
        SwoftyDbValue v{};
        int cr = db_cell(&c, field.c_str(), &v);
        const char *name = db_row_name(&c);
        swofty::log("[dbread] table %s[%llu] name=\"%s\" %s %s i=%lld", tbl.c_str(), (unsigned long long)c.index,
                    name ? name : "(null)", field.c_str(), cr == SWOFTY_ASSET_OK ? "ok tag int" : "miss", (long long)v.i);
    }
}

void inspect(int stage)
{
    if (stage != runtime::STAGE_WINDOW)
        return;
    if (!db_available()) {
        swofty::log("[dbread] window stage: database not loaded; nothing to inspect");
        return;
    }
    ensure_probe();
    const json::Value &keys = runtime::config()["db_inspect"];
    const json::Value &tables = runtime::config()["db_inspect_tables"];
    if (keys.a.empty() && tables.a.empty()) {
        swofty::log("[dbread] window stage: database ready (available), no \"db_inspect\" entries in %s",
                    runtime::config_source().c_str());
        return;
    }
    swofty::log("[dbread] window stage: database ready; %zu key(s), %zu table(s) to inspect", keys.a.size(),
                tables.a.size());
    for (auto &j : keys.a)
        inspect_key(j);
    for (auto &j : tables.a)
        inspect_table(j);
}

}

namespace runtime {

const SwoftyDb *db_table()
{
    return &table;
}

bool db_initialised()
{
    return initialised;
}

void dbread_setup()
{
    on_stage(inspect);
}

}
