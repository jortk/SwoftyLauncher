#include "runtime.hpp"
#include "sequencer_records_logic.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Career events sequencer record guard (re/db/globals.tsv site_Game_CareerEventsSequencerFirstRecord).
//
// Game_CareerEventsSequencer_Initialise (0x140c71e80) runs as a career session is entered. After the career hub scenes
// it adds the team mate asset groups: it looks the session's event up in the career event table (CareerEventData +0x2e8
// .. +0x2f0, records of 0x88 bytes), then collects every record of the table with the same +0x58 and +0x5a bytes as that
// record and the game's equality (0x1424ceb70 fills a pointer vector at [rsp+0x98], begin at [rsp+0xa0]), and reads the
// first collected record (mov rdx,[rax] at 0x140c72ea0). An event not in the table never gets here (the lookup jumps
// past this block at 0x140c72d51/0x140c72d61). The collection skips records whose +0x5e is 0x3fff and keeps only those
// the equality accepts, so an event whose own record is one of those, or fails the equality, collects nothing: begin is 0
// and the read faults with ACCESS_VIOLATION reading 0x0 at F1_25.exe+0xc72ea0 as the first session of the career is entered (a 2014 and a 2016
// Progressive Career with a third-party calendar package).
//
// The cleanup after the asset group call already tests the vector's begin for 0 (test rdx,rdx / je 0x140c72ee2). At the
// site a null rax resumes at that join (site_Game_CareerEventsSequencerRecordsJoin) with the asset groups of that event
// left out, which only costs the team mate scene assets. Nothing the skipped calls produce is read after the join.
//
// Team mate driver guard (site_Game_CareerEventsSequencerTeamMateDriver). The same block then looks the first collected
// record's driver up in the entrants system: Ent_Get_Driver_Thunk (0x1423dbcf0, rcx = r13 = the entrants system) takes
// the record's packed driver ref at +0x50 (domain byte, u16 stat id at +0x52) and returns the driver record, or 0 when
// the domain is above 2, the domain has no driver table, or the id is past the table's end or names an empty slot. The
// game passes that to Game_CareerEventResourceManager_AddDriverMorphData unchecked, which reads [driver+8] at +0x82 and
// faults reading 0x8 as the session is entered, at every load of a My Team or Progressive career whose team mate's
// driver the entrants system does not have.
//
// At the site (mov r8,rax before that call) a null rax resumes at the call's return
// (site_Game_CareerEventsSequencerTeamMateDriverJoin), where the collected vector is freed as usual. AddDriverMorphData
// itself does nothing for a driver whose head it does not find, so the skip only leaves out the team mate's driver
// morph assets. The team mate's domain and stat id are logged once.
//
// Player driver guard (site_Game_CareerEventsSequencerPlayerDriver). Further on the function finds the player's own
// entrant in the game module's entrant list (Sess_Match_EntrantIdentity_Thunk against a human key with the signed-in
// user's id block), looks its driver up the same way (0x140c73104, rcx = r15 = the entrants system, rdx = r12 = the
// entrant + 0x50) and passes the result to AddDriverMorphData unchecked again. At the mov r8,rax before that call a
// null rax resumes at the call's return (site_Game_CareerEventsSequencerPlayerDriverJoin), which is also where the
// function goes when it finds no player entrant. The player's domain and stat id are logged once.
//
// Recognition record guard (site_Team_RecognitionFirstRecord). The Driver Career recognition notification
// (Team_Build_RecognitionTeamIcon, 0x1420bb330, a view model method reached from Vm_Dispatch_BindingMessage) builds its
// one item the same way: it finds the player's entrant record in the career event table, collects the records that
// match it (0x1424ceb70 fills a pointer vector at [rsp+0xf0], begin at [rsp+0xf8]) and reads the first collected record
// (mov rax,[rax] at 0x1420bbb4a) for the driver ref it passes to the item's name and team icon setup (0x1416e17b0). A
// career whose session event collects nothing in the sequencer collects nothing here either, so the session the first
// guard let in faults reading 0x0 at Team_Build_RecognitionTeamIcon+0x81a when the notification is built, seconds to
// minutes into the session or as it ends.
//
// At the site a null rax resumes at the function's own exit for a notification it stops building
// (site_Team_RecognitionRecordsExit, the target of je 0x1420bc680 at 0x1420bbb95 and 0x1420bc117), which frees the
// collected vector when it has a begin and falls into the epilogue. The function changes rsp only in its prologue and
// epilogue and the exit reads nothing but the frame, so the frame is the one the game's own jump leaves. The item keeps
// the names and icon it was constructed with and gets no perk values. Logged once.
//
// All eight sites are exported by the bindings (L2 with an agree verify). SWOFTY_SEQUENCER_RECORDS_GUARD=0 turns all
// four guards off.

namespace {

namespace mem = swofty::mem;
namespace sr = runtime::sequencer_records;

constexpr const char *SITE_FIRST = "site_Game_CareerEventsSequencerFirstRecord";
constexpr const char *SITE_JOIN = "site_Game_CareerEventsSequencerRecordsJoin";
constexpr const char *SITE_DRIVER = "site_Game_CareerEventsSequencerTeamMateDriver";
constexpr const char *SITE_DRIVER_JOIN = "site_Game_CareerEventsSequencerTeamMateDriverJoin";
constexpr const char *SITE_PLAYER = "site_Game_CareerEventsSequencerPlayerDriver";
constexpr const char *SITE_PLAYER_JOIN = "site_Game_CareerEventsSequencerPlayerDriverJoin";
constexpr const char *SITE_RECOGNITION = "site_Team_RecognitionFirstRecord";
constexpr const char *SITE_RECOGNITION_EXIT = "site_Team_RecognitionRecordsExit";
constexpr uintptr_t RECORDS_BEGIN = 0xa0;
constexpr uintptr_t DRIVER_REF = 0x50;
constexpr uintptr_t DRIVER_TABLES = 0x268;

uintptr_t join_at = 0;
uintptr_t driver_join_at = 0;
uintptr_t player_join_at = 0;
uintptr_t recognition_exit_at = 0;
bool said = false;
bool said_driver = false;
bool said_player = false;
bool said_recognition = false;
std::mutex said_lock;
safetyhook::MidHook h_first;
safetyhook::MidHook h_driver;
safetyhook::MidHook h_player;
safetyhook::MidHook h_recognition;

void first_record(safetyhook::Context &c)
{
    if (!sr::skips(c.rax))
        return;
    c.rip = join_at;
    bool first;
    {
        std::lock_guard<std::mutex> hold(said_lock);
        first = sr::says(c.rax, said);
    }
    if (first)
        swofty::log("[career] events sequencer: the session's event record collects no matching record of the career event "
                    "table (its track or calendar entry is missing); the team mate asset groups are left out");
}

void log_missing(const char *whose, uintptr_t ref, uintptr_t entrants)
{
    uint8_t domain = 0;
    uint16_t id = 0;
    if (!ref || !mem::read(ref, domain) || !mem::read(ref + 2, id)) {
        swofty::log("[career] events sequencer: %s is not in the entrants system (%s); its driver morph assets are "
                    "left out",
                    whose, sr::miss_text(sr::Miss::unread));
        return;
    }
    uintptr_t table = 0;
    uintptr_t items[2] = {};
    if (domain <= 2 && mem::read(entrants + DRIVER_TABLES + domain * 0x10, table) && table)
        mem::read(table + 8, items);
    uint64_t count = sr::driver_count(items[0], items[1]);
    swofty::log("[career] events sequencer: %s (domain %u, stat id %u) is not in the entrants system: %s (%llu "
                "drivers); its driver morph assets are left out",
                whose, domain, id, sr::miss_text(sr::miss(domain, table, count, id)), (unsigned long long)count);
}

bool first_miss(uint64_t driver, bool &flag)
{
    std::lock_guard<std::mutex> hold(said_lock);
    return sr::says(driver, flag);
}

void team_mate_driver(safetyhook::Context &c)
{
    if (!sr::skips(c.rax))
        return;
    c.rip = driver_join_at;
    if (!first_miss(c.rax, said_driver))
        return;
    uintptr_t begin = 0, record = 0;
    bool found = mem::read(c.rsp + RECORDS_BEGIN, begin) && mem::read(begin, record) && record;
    log_missing(sr::who(sr::Driver::team_mate), found ? record + DRIVER_REF : 0, c.r13);
}

void player_driver(safetyhook::Context &c)
{
    if (!sr::skips(c.rax))
        return;
    c.rip = player_join_at;
    if (first_miss(c.rax, said_player))
        log_missing(sr::who(sr::Driver::player), c.r12, c.r15);
}

void recognition_record(safetyhook::Context &c)
{
    if (!sr::skips(c.rax))
        return;
    c.rip = recognition_exit_at;
    if (first_miss(c.rax, said_recognition))
        swofty::log("[career] recognition notification: the player's entrant record collects no matching record of the "
                    "career event table (its track or calendar entry is missing); the notification is left without its "
                    "names, team icon and perk values");
}

bool arm(const char *what, const char *site, const char *join, safetyhook::MidHookFn fn, safetyhook::MidHook &hook,
         uintptr_t &resume)
{
    uintptr_t at = swofty::api->resolve(site);
    resume = swofty::api->resolve(join);
    if (!at || !resume) {
        resume = 0;
        swofty::log("[career] %s off: needs %s and %s, which this build does not resolve", what, site, join);
        return false;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[career] %s: the hook failed: %s", what, runtime::hook_error(h.error()).c_str());
        return false;
    }
    hook = std::move(*h);
    return true;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_SEQUENCER_RECORDS_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] events sequencer record, team mate driver and player driver guards and the recognition "
                    "record guard off (SWOFTY_SEQUENCER_RECORDS_GUARD=%s)",
                    v);
        return;
    }
    if (arm("events sequencer record guard", SITE_FIRST, SITE_JOIN, first_record, h_first, join_at))
        swofty::log("[career] events sequencer record guard armed: a session whose event has no record in the career event "
                    "table enters without the team mate asset groups instead of faulting");
    if (arm("events sequencer team mate driver guard", SITE_DRIVER, SITE_DRIVER_JOIN, team_mate_driver, h_driver, driver_join_at))
        swofty::log("[career] events sequencer team mate driver guard armed: a session whose team mate's driver is not "
                    "in the entrants system enters without the team mate's driver morph assets instead of faulting");
    if (arm("events sequencer player driver guard", SITE_PLAYER, SITE_PLAYER_JOIN, player_driver, h_player, player_join_at))
        swofty::log("[career] events sequencer player driver guard armed: a session whose player's own driver is not "
                    "in the entrants system enters without the player's driver morph assets instead of faulting");
    if (arm("recognition record guard", SITE_RECOGNITION, SITE_RECOGNITION_EXIT, recognition_record, h_recognition,
            recognition_exit_at))
        swofty::log("[career] recognition record guard armed: a recognition notification whose player record collects "
                    "no matching record of the career event table is left unbuilt instead of faulting");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("events sequencer record guard", setup);
}
