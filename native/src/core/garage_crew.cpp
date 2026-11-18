#include "garage_crew_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

// Garage crew guard (re/db/globals.tsv site_Game_GarageCrew_SpawnVector).
//
// Game_GarageCrewModule_SpawnGarageCrew collects the session's assignment records for one garage into a vector of
// 16-byte entries {record, slot}, then creates one crew instance per entry and stores it at [rsp + index*8 + 0x290]
// with no bound. The array holds the two cars a garage has; the slot after it holds the vector's own pointer, which
// the epilogue frees through GAME_DEFAULT. A team that fields a third car puts a third record (slot 2) in its garage,
// the third crew pointer overwrites the vector pointer, and the free fails with "Invalid address for this heap" as the
// session loads. Sess_Set_GarageSlotEntrant already ignores slot >= 2, so only the crew spawn trips over it.
//
// At the site (rcx = vector begin, r13 = vector end, ebp = garage id) the guard keeps the first two entries whose slot
// is below 2 and lowers r13 over the rest. The extra car still races; its garage crew is not spawned.
//
// The crew instance of a garage is built from the team's crew template, found through the session's team tables
// ([[module + 0x48] + 0x2c8 + kind * 0x10], row team * 0x10 + 8, the record's kind at +0x58 and team at +0x5a); the
// function reads [template + 0x560] with no null test, so a car whose team has no template faults the session load
// at SpawnGarageCrew+0xce0. The guard drops those entries from the vector as well; the module is at [rsp + 0xc0].
//
// The pit stops module sets its crews up from fn 0x1403c69e0, which walks the 32 garages and, for each, scans the
// garage's entry vector ([rsp + 0xa8]) for the first entry with a team crew template (the same chain as above); rdi is
// that template or 0. It is passed to Game_PitStops_SetupCrew as the fifth argument, which reads [template + 0x560]
// at +0x12fb with no null test. A garage whose entries have no template (an empty garage, or a team the session has
// no crew template for) faults the session load there ("reading 0x560"). At site_Game_PitStops_CrewTemplateRead the
// guard sends such a garage to site_Game_PitStops_CrewTemplateSkip, the game's own test r12,r12 ahead of the free of
// the entry vector (an empty garage has no vector and takes the game's je), and the loop moves on to the next garage;
// the garage gets no pit stops crew and its cars still race. The loop carries r13 (the address of the key at
// [rsp + 0xc0], which the skipped body would have set back at 0x1403c7745) and sil (the byte at [rsp + 0x4b], reloaded
// at 0x1403c77e0) into the next garage, so the guard restores both along with r12 = [rsp + 0xa8].
//
// Game_GarageCrewModule_SpawnGarageCrew keeps the garage crew scene it looked up at [self + 0xc8]. A track without
// the garage scenes (an imported track: findScene fails for garagecrew, garagecrew_vehicle, _vehicleLocater,
// _ui_equipment and garagegantry) leaves it null. The code after the lookup handles null, but the compiler hoisted
// movzx esi, [rdi + 0xe0] above the test at 0x14038ba27, so the null scene faults at +0x2d30. At
// site_Game_GarageCrew_SceneRead the guard sets esi = 0 and continues at site_Game_GarageCrew_SceneTest, which takes
// the game's own null path.
//
// SWOFTY_GARAGE_CREW_GUARD=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace gc = runtime::garage_crew;

constexpr const char *MIXIN = "swofty_core.dll (garage crew guard)";
constexpr const char *SITE = "site_Game_GarageCrew_SpawnVector";
constexpr size_t MAX_ENTRIES = 256;
constexpr size_t REC_DRIVER = 0x52;
constexpr size_t REC_CAR_NUMBER = 0x83;
constexpr const char *SETUP_SITE = "site_Game_PitStops_CrewTemplateRead";
constexpr const char *SETUP_SKIP = "site_Game_PitStops_CrewTemplateSkip";
constexpr const char *SCENE_SITE = "site_Game_GarageCrew_SceneRead";
constexpr const char *SCENE_TEST = "site_Game_GarageCrew_SceneTest";
constexpr int LOG_FIRST = 8;
constexpr uint64_t MODULE_VEHICLE_SCENE = 0xd0, MODULE_EQUIPMENT_SCENE = 0xe0;

bool offered = false, settled = false, wanted = false;
std::atomic<int> dropped{0};
std::vector<uint32_t> seen;
uintptr_t setup_skip = 0;
gc::SetupSkips setup_seen;
uintptr_t scene_test = 0;
std::atomic<int> scene_skips{0};

struct Log {
    uint32_t garage;
};

bool read_memory(uint64_t address, void *out, size_t n, void *)
{
    if (!runtime::readable(reinterpret_cast<const void *>(address), n))
        return false;
    std::memcpy(out, reinterpret_cast<const void *>(address), n);
    return true;
}

void log_dropped(void *ctx, uint64_t record, uint32_t slot, gc::Drop why)
{
    uint32_t garage = static_cast<Log *>(ctx)->garage;
    uint16_t driver = 0xffff, team = 0xffff;
    uint8_t number = 0;
    if (record) {
        mem::read(record + REC_DRIVER, driver);
        mem::read(record + gc::REC_TEAM, team);
        mem::read(record + REC_CAR_NUMBER, number);
    }
    uint32_t key = uint32_t(driver) << 16 | team;
    for (uint32_t k : seen)
        if (k == key)
            return;
    seen.push_back(key);
    int n = ++dropped;
    if (n > LOG_FIRST)
        return;
    const char *tail = n == LOG_FIRST ? " (further drops are not logged)" : "";
    if (why == gc::Drop::no_template)
        swofty::log("[garage] garage %u: team %u car #%u (driver %u) has no crew template in the session; its crew is "
                    "not spawned, the car still races%s",
                    unsigned(garage), unsigned(team), unsigned(number), unsigned(driver), tail);
    else
        swofty::log("[garage] garage %u: team %u car #%u (driver %u) is the %s car of its garage; its crew is not "
                    "spawned, the car still races%s",
                    unsigned(garage), unsigned(team), unsigned(number), unsigned(driver),
                    slot == 2 ? "third" : "extra", tail);
}

void spawn_vector(SwoftyContext *c)
{
    uintptr_t begin = c->rcx, end = c->r13;
    if (!begin || end <= begin || (end - begin) % gc::ENTRY_SIZE)
        return;
    size_t n = (end - begin) / gc::ENTRY_SIZE;
    if (n > MAX_ENTRIES)
        return;
    uint64_t session = 0;
    bool known = gc::session_of(c->rsp, read_memory, nullptr, &session);
    Log log{uint32_t(c->rbp)};
    size_t kept = gc::compact(reinterpret_cast<uint8_t *>(begin), n, session, known, read_memory, nullptr, log_dropped,
                              &log);
    if (kept != n)
        c->r13 = begin + kept * gc::ENTRY_SIZE;
}

void setup_template(SwoftyContext *c)
{
    if (!gc::setup_template_missing(c->rdi))
        return;
    uint64_t begin = 0;
    uint8_t flag = 0;
    if (!read_memory(c->rsp + gc::SETUP_VECTOR_BEGIN, &begin, sizeof begin, nullptr) ||
        !read_memory(c->rsp + gc::SETUP_FLAG, &flag, sizeof flag, nullptr))
        return;
    if (setup_seen.first(c->r14)) {
        uint64_t record = 0;
        uint16_t team = 0xffff;
        uint8_t number = 0;
        if (gc::user_pointer(begin) && read_memory(begin, &record, sizeof record, nullptr) && gc::user_pointer(record)) {
            mem::read(record + gc::REC_TEAM, team);
            mem::read(record + REC_CAR_NUMBER, number);
        }
        if (team == 0xffff)
            swofty::log("[garage] garage %u has no cars with a crew template; its pit stops crew is not set up",
                        unsigned(c->r14));
        else
            swofty::log("[garage] garage %u: team %u car #%u has no pit stops crew template; its pit stops crew is not "
                        "set up, the car still races",
                        unsigned(c->r14), unsigned(team), unsigned(number));
    }
    c->r12 = begin;
    c->r13 = c->rsp + gc::SETUP_KEY;
    c->rsi = flag;
    c->rip = setup_skip;
}

void scene_read(SwoftyContext *c)
{
    if (!gc::scene_missing(c->rdi))
        return;
    int n = ++scene_skips;
    if (n <= 3) {
        uint64_t vehicle = 0, equipment = 0;
        mem::read(c->rbp + MODULE_VEHICLE_SCENE, vehicle);
        mem::read(c->rbp + MODULE_EQUIPMENT_SCENE, equipment);
        swofty::log("[garage] the garage crew scene is missing for this track (garage crew scene 0x%llx, vehicle scene "
                    "0x%llx, equipment scene 0x%llx); no garage crew is shown%s",
                    (unsigned long long)c->rdi, (unsigned long long)vehicle, (unsigned long long)equipment,
                    n == 3 ? " (further ones are not logged)" : "");
    }
    c->rsi = 0;
    c->rip = scene_test;
}

}

namespace runtime {

std::vector<CoreMixins> garage_crew_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_GARAGE_CREW_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[garage] crew guard off (SWOFTY_GARAGE_CREW_GUARD=%s)", v);
        return out;
    }
    if (!swofty::api->resolve(SITE)) {
        swofty::log("[garage] crew guard off: needs %s, which this build does not resolve", SITE);
        return out;
    }
    wanted = true;
    CoreMixins set{MIXIN, {{SITE, SWOFTY_MIXIN_SITE, 100, (void *)&spawn_vector, nullptr}}};
    setup_skip = swofty::api->resolve(SETUP_SKIP);
    if (setup_skip && swofty::api->resolve(SETUP_SITE))
        set.mixins.push_back({SETUP_SITE, SWOFTY_MIXIN_SITE, 100, (void *)&setup_template, nullptr});
    else
        swofty::log("[garage] pit stops crew guard off: needs %s and %s, which this build does not resolve",
                    SETUP_SITE, SETUP_SKIP);
    scene_test = swofty::api->resolve(SCENE_TEST);
    if (scene_test && swofty::api->resolve(SCENE_SITE))
        set.mixins.push_back({SCENE_SITE, SWOFTY_MIXIN_SITE, 100, (void *)&scene_read, nullptr});
    else
        swofty::log("[garage] crew scene guard off: needs %s and %s, which this build does not resolve", SCENE_SITE,
                    SCENE_TEST);
    out.push_back(set);
    return out;
}

void garage_crew_commit()
{
    if (!offered || settled || !wanted)
        return;
    settled = true;
    swofty::log("[garage] crew guard %s",
                core_mixins_active(MIXIN)
                    ? "armed: a garage's crew is spawned for its first two cars; a third car is left without one"
                    : "did not install (see the [core] lines)");
}

}
