#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstring>
#include <string>
#include <vector>

#include "apply/moments.hpp"
#include "direct_start_tt.hpp"
#include "direct_start_tt_logic.hpp"
#include "era_logic.hpp"
#include "tt_loaded_logic.hpp"

// The direct start's Time Trial (a `session` request with "mode": "time_trial"): the Time Trial mode made
// current, then its setup written where the front end's screens leave it, so StateFrontendWorld's 'session'
// link loads a Time Trial at the chosen track with the chosen box and team.
//
// The Time Trial mode entity E (game mode "time_trial", vtable 0x145648300) holds the setup the screens build:
// E+0x2b0 the game setup mode (setter 0x14079f300), E+0xe10 the entrant setup mode, E+0xe18 the selected entrant
// list. Its session settings (slot 0xd8, body 0x148731f80: session 18, ruleset 2) take the track from
// 0x1407ddbf0, which reads the VM string game_mode_data.current_game_mode.track_info.current_track.track_file_name
// and finds the record of that name (+0x78) in E's track system (E+0xf58, 16-byte entries at +0x28..+0x30, the
// record at +8). Sess_TimeTrialMode_FillDescriptor (0x142936760) takes the player's car from the Time Trial
// component's hot load holder (see tt_component), else from the local player's entrant selection (a static pair of
// 0xc0-byte records at 0x14604eac0, guard 0x14604ec50, team ref +0x68 engaged +0x6c) and a per-team vehicle table
// built for the screens' list. Team refs pack a kind byte with a u16 index into [[[E+0xf80]+0x58]+0x2c8+kind*0x10]
// (16-byte entries at +8, the team's name at entry+0x18); an entrant record carries its team's ref at +0x58 and its
// vehicle's at +0x5c. Once the garage is up the front end's main scene is despawned and the intro fade released.

namespace runtime {
std::vector<era::Box> era_boxes();
}

bool ui_vm_set_path(const std::string &path, int type, const void *value);

namespace runtime {
std::string hook_error(const safetyhook::InlineHook::Error &e);
}

namespace {

namespace tt = runtime::direct_start::tt;
using runtime::direct_start::Setup;

constexpr const char *MODE = "time_trial";
constexpr const char *TRACK_NODE = "game_mode_data.current_game_mode.track_info.current_track.track_file_name";
constexpr const char *WEATHER_NODE = "settings.current_settings.tt_weather.value";
// The game addresses the Time Trial setup uses, each found by a pattern that matches once on the Steam and on the EA build
// (re/tools/sigverify.py). The two functions' entries are jump thunks, so they are found from a call to them.
const runtime::Sig SIG_SET_SETUP_MODE{"game.entity_set_setup_mode_thunk_14079f300",
                                      "E8 ?? ?? ?? ?? 48 8D 84 24 20 03 00 00 48 C7 84 24 10 03 00 00 80 00 00 00", 0x79f300,
                                      runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_CURRENT_TRACK{"game.entity_current_track_thunk_1407ddbf0",
                                     "E8 ?? ?? ?? ?? 44 0F B6 40 01 44 89 86 F0 11 00 00", 0x7ddbf0, runtime::Sig::rel32, 0, 1, 5};
const runtime::Sig SIG_PLAYER_ENTRANTS{"game.player_entrants_14604eac0", "48 8D 1D ?? ?? ?? ?? 48 8D 54 24 28 48 89 D9 45 31 C9",
                                       0x604eac0, runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_PLAYER_GUARD{"game.player_entrants_guard_14604ec50",
                                    "8B 05 ?? ?? ?? ?? 8B 0D ?? ?? ?? ?? 65 4C 8B 04 25 58 00 00 00 49 8B 0C C8 8B 89 04 00 00 00 48 "
                                    "3B 54 24 58",
                                    0x604ec50, runtime::Sig::riprel, 0, 2, 6};
const runtime::Sig SIG_TT_COMPONENT_VT{"game.vt_time_trial_component_1457b13d0",
                                       "48 8D 05 ?? ?? ?? ?? 48 89 01 48 8D 05 ?? ?? ?? ?? 48 89 41 18 4C 8B B1 78 03 00 00",
                                       0x57b13d0, runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig SIG_GAME_ROOT{"game.root_14601db08",
                                 "48 8B 05 ?? ?? ?? ?? 48 8B 40 60 48 8B 88 D0 09 00 00 FF 15 ?? ?? ?? ?? 48 8B 05 ?? ?? ?? ?? 48 "
                                 "8B 40 60",
                                 0x601db08, runtime::Sig::riprel, 0, 3, 7};
constexpr uintptr_t MODES_BEGIN = 0x58, MODES_END = 0x60, MODES_ENTRY_MODE = 0x38, MODE_ENTITY = 0x260, MODE_NAME = 0x278;
constexpr uintptr_t ENTITY_MODE = 0xf08, ENTITY_TRACKS = 0xf58, ENTITY_TEAMS = 0xf80, ENTITY_SETUP_MODE = 0x2b0,
                    ENTITY_ENTRANT_SETUP = 0xe10, ENTITY_LIST = 0xe18;
constexpr uintptr_t GSM_HOLDER = 0xd8, GSM_ESMS = 0x10, GSM_ESM_COUNT = 0x70, ESM_LISTS = 0x130, ESM_LIST_COUNT = 0x230,
                    LIST_NAME = 0x28, LIST_BEGIN = 0x10, LIST_END = 0x18, RECORD = 0x88, RECORD_TEAM = 0x58,
                    TRACK_NAME = 0x78, PLAYER_STRIDE = 0xc0, PLAYER_TEAM = 0x68, PLAYER_TEAM_SET = 0x6c,
                    RECORD_VEHICLE = 0x5c;
constexpr uintptr_t MODE_COMPONENTS = 0x238, TT_HOT_LOAD = 0x298, HOT_VEHICLE = 0x2be,
                    HOT_VEHICLE_SET = 0x2c2, HOT_TEAM = 0x2c4, HOT_TEAM_SET = 0x2c8;
constexpr size_t ESM_SLOTS = 32, GSM_SLOTS = 32, GSM_ESM_SLOTS = (GSM_ESM_COUNT - GSM_ESMS) / 8;

using SetMode_t = void(void *modes, void *mode);
using CurrentTrack_t = uint8_t *(void *entity);

template <class T> T at(const void *p, uintptr_t off)
{
    return *reinterpret_cast<const T *>(static_cast<const uint8_t *>(p) + off);
}

struct Game {
    uintptr_t set_setup_mode = 0, current_track = 0, player_entrants = 0, player_guard = 0, tt_component_vt = 0, root = 0;
    bool ok = false;
};

const Game &game()
{
    static const Game g = [] {
        Game r;
        if (!runtime::verified_build()) {
            swofty::log("[direct] Time Trial setup off: its struct offsets are checked on the Steam and EA builds and this "
                        "F1_25.exe is another build");
            return r;
        }
        r.set_setup_mode = runtime::find(SIG_SET_SETUP_MODE, "Time Trial setup", "direct");
        r.current_track = runtime::find(SIG_CURRENT_TRACK, "Time Trial setup", "direct");
        r.player_entrants = runtime::find(SIG_PLAYER_ENTRANTS, "Time Trial setup", "direct");
        r.player_guard = runtime::find(SIG_PLAYER_GUARD, "Time Trial setup", "direct");
        r.tt_component_vt = runtime::find(SIG_TT_COMPONENT_VT, "Time Trial setup", "direct");
        r.root = runtime::find(SIG_GAME_ROOT, "Time Trial setup", "direct");
        r.ok = r.set_setup_mode && r.current_track && r.player_entrants && r.player_guard && r.tt_component_vt && r.root;
        if (!r.ok)
            swofty::log("[direct] Time Trial setup off: one of its addresses did not resolve on this build");
        return r;
    }();
    return g;
}

template <class T> T *ptr(uintptr_t a)
{
    return reinterpret_cast<T *>(a);
}

const char *c_name(const void *p)
{
    return p && swofty::mem::readable(p, 1) ? static_cast<const char *>(p) : nullptr;
}

const char *holder_name(const uint8_t *obj, uintptr_t holder_off)
{
    if (!obj || !swofty::mem::readable(obj + holder_off, 8))
        return nullptr;
    auto *holder = at<const uint8_t *>(obj, holder_off);
    return holder && swofty::mem::readable(holder, 0x18) ? c_name(at<const char *>(holder, 0x10)) : nullptr;
}

void despawn_main_scene(const char *when);

struct State {
    tt::Step step = tt::Step::mode;
    uint64_t started = 0, mode_set = 0;
    uint8_t *mode = nullptr, *entity = nullptr, *list = nullptr;
    bool placed = false, replay = false;
} st;

// The game modes context's modes (vector at +0x58..+0x60, each entry's mode at +0x38, its label at mode+0x278 and its
// entity at mode+0x260). Game_GameModes_FindByName (0x1407d0de0) returns only championship-type entities.
uint8_t *mode_named(void *modes, const char *want, std::vector<std::string> &names)
{
    auto *ctx = static_cast<uint8_t *>(modes);
    if (!swofty::mem::readable(ctx, MODES_END + 8))
        return nullptr;
    uint8_t *found = nullptr;
    for (auto **p = at<uint8_t **>(ctx, MODES_BEGIN), **end = at<uint8_t **>(ctx, MODES_END); p && p < end; ++p) {
        if (!swofty::mem::readable(p, 8) || !*p || !swofty::mem::readable(*p, MODES_ENTRY_MODE + 8))
            continue;
        auto *mode = at<uint8_t *>(*p, MODES_ENTRY_MODE);
        if (!mode || !swofty::mem::readable(mode, MODE_NAME + 8))
            continue;
        const char *name = c_name(at<const char *>(mode, MODE_NAME));
        if (!name)
            continue;
        names.push_back(name);
        if (!found && !std::strcmp(name, want))
            found = mode;
    }
    return found;
}

// The VM node is not writable through the core's path setter (current_game_mode is a link), so the Time Trial's track
// getter is wrapped: while it still reads the track the profile left (stale), it answers the chosen record. A track
// the player picks later in the session (a hot load rewrites the node) reads differently and passes through.
safetyhook::InlineHook h_track;
std::atomic<uint8_t *> forced_track{nullptr}, stale_track{nullptr};
std::atomic<bool> told_forced{false};

uint8_t *track_detour(void *e)
{
    uint8_t *r = h_track.call<uint8_t *>(e);
    uint8_t *want = forced_track.load();
    if (!want || r == want || r != stale_track.load())
        return r;
    if (!told_forced.exchange(true))
        swofty::log("[direct] Time Trial: the track getter read %p; it answers the chosen track %p", r, want);
    return want;
}

bool force_track(uint8_t *rec, uint8_t *stale)
{
    forced_track = rec;
    stale_track = stale;
    if (h_track)
        return true;
    auto h = safetyhook::InlineHook::create(ptr<void>(game().current_track), reinterpret_cast<void *>(track_detour));
    if (!h) {
        swofty::log("[direct] Time Trial: the track getter hook failed: %s", runtime::hook_error(h.error()).c_str());
        forced_track = nullptr;
        return false;
    }
    h_track = std::move(*h);
    return true;
}

uint8_t *track_record(uint8_t *e, const std::string &want, std::vector<std::string> &names)
{
    auto *tracks = at<uint8_t *>(e, ENTITY_TRACKS);
    if (!tracks || !swofty::mem::readable(tracks, 0x38))
        return nullptr;
    uint8_t *found = nullptr;
    for (auto *q = at<uint8_t *>(tracks, 0x28), *end = at<uint8_t *>(tracks, 0x30); q && q < end; q += 16) {
        if (!swofty::mem::readable(q, 16))
            break;
        auto *rec = at<uint8_t *>(q, 8);
        const char *name = rec && swofty::mem::readable(rec, TRACK_NAME + 8) ? c_name(at<const char *>(rec, TRACK_NAME)) : nullptr;
        if (!name)
            continue;
        names.push_back(name);
        if (!found && tt::lower(name) == tt::lower(want))
            found = rec;
    }
    return found;
}

std::vector<uint8_t *> setup_modes(uint8_t *e)
{
    std::vector<uint8_t *> out;
    auto **evt = at<void **>(e, 0);
    auto *vec = reinterpret_cast<uint8_t *(*)(void *)>(evt[0x1d0 / 8])(e);
    if (!vec || !swofty::mem::readable(vec, 0x18))
        return out;
    for (auto **p = at<uint8_t **>(vec, 8), **end = at<uint8_t **>(vec, 0x10); p && p != end && out.size() < GSM_SLOTS; ++p)
        if (*p && swofty::mem::readable(*p, GSM_HOLDER + 8))
            out.push_back(*p);
    return out;
}

struct ListAt {
    uint8_t *esm = nullptr, *list = nullptr;
};

ListAt find_list(uint8_t *gsm, const std::string &want, std::vector<std::string> &names)
{
    ListAt hit;
    if (!swofty::mem::readable(gsm, GSM_ESM_COUNT + 8))
        return hit;
    int64_t n = at<int64_t>(gsm, GSM_ESM_COUNT);
    auto **esms = reinterpret_cast<uint8_t **>(gsm + GSM_ESMS);
    for (int64_t i = 0; i < n && i < int64_t(GSM_ESM_SLOTS); ++i) {
        uint8_t *esm = esms[i];
        if (!esm || !swofty::mem::readable(esm, ESM_LIST_COUNT + 8))
            continue;
        int64_t lists = at<int64_t>(esm, ESM_LIST_COUNT);
        for (int64_t j = 0; j < lists && j < int64_t(ESM_SLOTS); ++j) {
            auto *list = at<uint8_t *>(esm, ESM_LISTS + size_t(j) * 8);
            const char *name = holder_name(list, LIST_NAME);
            if (!name)
                continue;
            names.push_back(name);
            if (!hit.list && want == name)
                hit = {esm, list};
        }
    }
    return hit;
}

std::string team_name(uint8_t *e, uint32_t ref)
{
    uint8_t kind = tt::ref_kind(ref);
    uint16_t index = tt::ref_index(ref);
    auto *teams = at<uint8_t *>(e, ENTITY_TEAMS);
    if (kind >= 3 || !teams || !swofty::mem::readable(teams, 0x60))
        return {};
    auto *sys = at<uint8_t *>(teams, 0x58);
    if (!sys || !swofty::mem::readable(sys + 0x2c8 + kind * 0x10, 8))
        return {};
    auto *table = at<uint8_t *>(sys, 0x2c8 + kind * 0x10);
    if (!table || !swofty::mem::readable(table, 0x18))
        return {};
    auto *begin = at<uint8_t *>(table, 8), *end = at<uint8_t *>(table, 0x10);
    if (!begin || end < begin || size_t(end - begin) / 16 <= index || !swofty::mem::readable(begin + index * 16, 16))
        return {};
    auto *entry = at<uint8_t *>(begin + index * 16, 8);
    const char *name = entry && swofty::mem::readable(entry, 0x20) ? c_name(at<const char *>(entry, 0x18)) : nullptr;
    return name ? std::string(name) : std::string();
}

std::string team_label(uint16_t stat)
{
    const SwoftyDb *db = runtime::db_table();
    if (!db || !db->available || !db->available())
        return {};
    int64_t count = 0;
    if (db->int_at("entrants.teams._team_instancecount_", &count) != SWOFTY_ASSET_OK || count <= 0 || count > 8192)
        return {};
    for (int64_t i = 0; i < count; ++i) {
        std::string row = "entrants.teams.team_" + std::to_string(i);
        int64_t id = -1;
        const char *name = nullptr;
        if (db->int_at((row + ".stat_id").c_str(), &id) == SWOFTY_ASSET_OK && id == stat &&
            db->string_at((row + "._metadata_").c_str(), &name) == SWOFTY_ASSET_OK && name && strnlen(name, 128) < 128)
            return name;
    }
    return {};
}

int player_index()
{
    uintptr_t slot = swofty::api->resolve("g_Game_Manager"), mgr = 0;
    if (!slot || !swofty::mem::readable(reinterpret_cast<void *>(slot), 8) || !(mgr = *reinterpret_cast<uintptr_t *>(slot)))
        return 0;
    auto *m = reinterpret_cast<uint8_t *>(mgr);
    auto *users = swofty::mem::readable(m + 0x148, 8) ? at<uint8_t *>(m, 0x140) : nullptr;
    if (!users || !swofty::mem::readable(users, 0x1b0))
        return 0;
    auto *user = at<uint8_t *>(users, 0x1a8);
    if (!user)
        user = at<uint8_t *>(users, 0x198);
    if (!user || !swofty::mem::readable(user, 0x28))
        return 0;
    auto *owner = at<uint8_t *>(user, 0x20);
    if (!owner || !swofty::mem::readable(owner, 0x1a8) || at<uint8_t *>(owner, 0x198) == user)
        return 0;
    return at<uint8_t *>(owner, 0x1a0) == user ? 1 : 0;
}

// The Time Trial component (vtable 0x1457b13d0) among the mode's components (+0x238..+0x240), whose +0x298 is
// the hot load holder Sess_TimeTrialMode_FillDescriptor reads first: the player's vehicle ref at +0x2be (engaged
// +0x2c2) and team ref at +0x2c4 (engaged +0x2c8), as the in-session hot load (0x14291a320) writes them. Without
// them the fill takes the team from the player's entrant selection and the vehicle from a per-team table built for
// the list the screens selected, which a list set here never reaches, and the session faults building the AI's
// team curves on a player with no vehicle (Ai_Build_TeamPerformanceCurves+0xa3).
uint8_t *tt_component(uint8_t *mode)
{
    if (!mode || !swofty::mem::readable(mode, MODE_COMPONENTS + 0x10))
        return nullptr;
    if (!game().ok)
        return nullptr;
    auto *vt = ptr<void>(game().tt_component_vt);
    for (auto **p = at<uint8_t **>(mode, MODE_COMPONENTS), **end = at<uint8_t **>(mode, MODE_COMPONENTS + 8);
         p && p < end && swofty::mem::readable(p, 8); ++p)
        if (*p && swofty::mem::readable(*p, TT_HOT_LOAD + 8) && at<void *>(*p, 0) == vt)
            return *p;
    return nullptr;
}

Setup set_team(uint8_t *e, uint8_t *list, const std::string &want, std::string &why)
{
    auto *begin = at<uint8_t *>(list, LIST_BEGIN), *end = at<uint8_t *>(list, LIST_END);
    if (!begin || end <= begin || size_t(end - begin) % RECORD || !swofty::mem::readable(begin, size_t(end - begin))) {
        why = "the selected entrant list holds no entrant records";
        return Setup::failed;
    }
    std::vector<std::string> labels;
    std::vector<uint32_t> refs, vehicles;
    for (auto *r = begin; r < end; r += RECORD) {
        uint32_t ref = at<uint32_t>(r, RECORD_TEAM);
        if (tt::ref_index(ref) == tt::NO_REF || std::find(refs.begin(), refs.end(), ref) != refs.end())
            continue;
        std::string name = team_name(e, ref);
        if (name.empty())
            name = team_label(tt::ref_index(ref));
        if (name.empty())
            name = "team#" + std::to_string(tt::ref_kind(ref)) + "." + std::to_string(tt::ref_index(ref));
        labels.push_back(name);
        refs.push_back(ref);
        vehicles.push_back(at<uint32_t>(r, RECORD_VEHICLE));
    }
    int i = want.empty() ? (labels.empty() ? -1 : 0) : tt::pick_team(labels, want);
    if (i < 0) {
        why = "the list has no team '" + want + "' (teams: " + tt::joined(labels) + ")";
        return Setup::failed;
    }
    uint32_t team = refs[size_t(i)], vehicle = vehicles[size_t(i)];
    if (tt::ref_index(vehicle) == tt::NO_REF) {
        why = "the list's " + labels[size_t(i)] + " entrant has no vehicle";
        return Setup::failed;
    }
    uint8_t *comp = tt_component(st.mode);
    if (!comp) {
        why = "the Time Trial component (vtable 0x1457b13d0) is not among the mode's components";
        return Setup::failed;
    }
    auto *hot = at<uint8_t *>(comp, TT_HOT_LOAD);
    if (!hot || !swofty::mem::readable(hot, HOT_TEAM_SET + 8)) {
        why = "the Time Trial component has no hot load holder";
        return Setup::failed;
    }
    *reinterpret_cast<uint32_t *>(hot + HOT_VEHICLE) = vehicle;
    hot[HOT_VEHICLE_SET] = 1;
    *reinterpret_cast<uint32_t *>(hot + HOT_TEAM) = team;
    hot[HOT_TEAM_SET] = 1;
    int guard = *ptr<int>(game().player_guard), p = player_index();
    if (guard != 0 && guard != -1) {
        auto *sel = ptr<uint8_t>(game().player_entrants) + size_t(p) * PLAYER_STRIDE;
        *reinterpret_cast<uint32_t *>(sel + PLAYER_TEAM) = team;
        sel[PLAYER_TEAM_SET] = 1;
    }
    swofty::log("[direct] Time Trial: team %s (team ref %08x, vehicle ref %08x) in the hot load holder %p of %p%s",
                labels[size_t(i)].c_str(), team, vehicle, hot, comp,
                guard != 0 && guard != -1 ? " and the player's entrant selection" : "");
    return Setup::done;
}

Setup place(uint8_t *e, const json::Value &request, std::string &why)
{
    std::string box = request["box"].str(), track = request["track"].str(),
                setup_name = request["setup_mode"].str("swofty_f1");
    if (!box.empty()) {
        std::vector<tt::BoxLists> boxes;
        for (const auto &b : runtime::era_boxes())
            boxes.push_back({b.entrant_list, b.time_trial});
        std::string want = tt::list_for(boxes, box);
        std::vector<uint8_t *> gsms;
        std::vector<std::string> modes;
        std::vector<std::vector<std::string>> offered;
        for (uint8_t *m : setup_modes(e)) {
            const char *name = holder_name(m, GSM_HOLDER);
            if (!name)
                continue;
            gsms.push_back(m);
            modes.push_back(name);
            offered.emplace_back();
            find_list(m, want, offered.back());
        }
        bool named = !request["setup_mode"].str().empty();
        int pick = tt::pick_setup_mode(modes, offered, setup_name, named, want);
        if (pick < 0) {
            auto it = std::find(modes.begin(), modes.end(), setup_name);
            if (it == modes.end() && named)
                why = "the Time Trial mode has no game setup mode '" + setup_name + "' (it has " + tt::joined(modes) + ")";
            else {
                why = "no setup mode offers the entrant list '" + want + "' (";
                for (size_t k = 0; k < modes.size(); ++k)
                    why += (k ? "; " : "") + modes[k] + ": " + tt::joined(offered[k]);
                why += ")";
            }
            return Setup::failed;
        }
        uint8_t *gsm = gsms[size_t(pick)];
        setup_name = modes[size_t(pick)];
        std::vector<std::string> lists;
        ListAt hit = find_list(gsm, want, lists);
        if (!hit.list) {
            why = "the setup mode " + setup_name + " offers no entrant list '" + want + "' (it offers " +
                  tt::joined(lists) + ")";
            return Setup::failed;
        }
        ptr<void(void *, void *, int)>(game().set_setup_mode)(e, gsm, 0);
        if (at<uint8_t *>(e, ENTITY_SETUP_MODE) != gsm)
            swofty::log("[direct] Time Trial: the setup mode setter left %p, not %p, at E+0x2b0",
                        at<uint8_t *>(e, ENTITY_SETUP_MODE), gsm);
        *reinterpret_cast<uint8_t **>(e + ENTITY_ENTRANT_SETUP) = hit.esm;
        *reinterpret_cast<uint8_t **>(e + ENTITY_LIST) = hit.list;
        st.list = hit.list;
        swofty::log("[direct] Time Trial: setup mode %s (%p), entrant setup mode %p, list %s (%p)%s", setup_name.c_str(),
                    gsm, hit.esm, want.c_str(), hit.list, want == box ? "" : (" for box " + box).c_str());
        swofty_core::apply::moments::box_picked(want.c_str());
    } else
        st.list = at<uint8_t *>(e, ENTITY_LIST);
    if (!track.empty()) {
        std::vector<std::string> names;
        uint8_t *rec = track_record(e, track, names);
        if (!rec) {
            why = "the Time Trial mode has no track '" + track + "' (it has " + tt::joined(names, 40) + ")";
            return Setup::failed;
        }
        const char *file = c_name(at<const char *>(rec, TRACK_NAME));
        bool written = ui_vm_set_path(TRACK_NODE, 0, file);
        uint8_t *now = ptr<CurrentTrack_t>(game().current_track)(e);
        const char *reads = now && swofty::mem::readable(now, TRACK_NAME + 8) ? c_name(at<const char *>(now, TRACK_NAME)) : nullptr;
        swofty::log("[direct] Time Trial: track %s (%p): the VM node %s, the getter reads %s", file, rec,
                    written ? "written" : "NOT written", reads ? reads : "nothing");
        if (now != rec && !force_track(rec, now)) {
            why = std::string("the Time Trial's track could not be set to ") + file;
            return Setup::failed;
        }
    }
    std::string weather = request["weather"].str();
    if (!weather.empty()) {
        const char *value = tt::weather_value(weather);
        if (!value) {
            why = "the Time Trial has no weather '" + weather + "' (clear or wet)";
            return Setup::failed;
        }
        bool ok = ui_vm_set_path(WEATHER_NODE, 0, value);
        swofty::log("[direct] Time Trial: weather %s %s", value, ok ? "set" : "NOT set (the VM node is missing)");
    }
    return Setup::done;
}

Setup apply(uint8_t *e, const json::Value &request, std::string &why)
{
    if (!st.placed) {
        Setup s = place(e, request, why);
        if (s != Setup::done)
            return s;
        st.placed = true;
    }
    std::string team = request["team"].str();
    if (!st.list) {
        if (team.empty())
            return Setup::done;
        why = "no entrant list is selected to pick the team from";
        return Setup::failed;
    }
    return set_team(e, st.list, team, why);
}

}

namespace runtime::direct_start::tt {

Setup setup(const json::Value &request, std::string &why)
{
    if (st.step == Step::done)
        return Setup::done;
    if (!game().ok) {
        why = "the Time Trial setup's addresses did not resolve on this F1_25.exe";
        return Setup::failed;
    }
    if (!st.started)
        st.started = GetTickCount64();
    DirectStartContext ctx = runtime::direct_start_context();
    uintptr_t set_mode = runtime::direct_start_set_mode();
    if (!set_mode) {
        why = "SetCurrentGameMode did not resolve";
        return Setup::failed;
    }
    if (!ctx.game_modes)
        return Setup::pending;
    if (st.step == Step::mode) {
        std::vector<std::string> names;
        uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
        uint8_t *e = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
        if (!e || !swofty::mem::readable(e, ENTITY_TEAMS + 8)) {
            if (GetTickCount64() - st.started < MODE_WAIT_MS)
                return Setup::pending;
            why = "the game has no game mode 'time_trial' (modes: " + tt::joined(names, 60) + ")";
            return Setup::failed;
        }
        st.mode = mode;
        st.entity = e;
        reinterpret_cast<SetMode_t *>(set_mode)(ctx.game_modes, mode);
        st.mode_set = GetTickCount64();
        st.step = Step::settle;
        swofty::log("[direct] Time Trial: the Time Trial mode is current (mode %p, entity %p, entity's mode %p)", mode, e,
                    at<void *>(e, ENTITY_MODE));
        return Setup::pending;
    }
    if (st.step == Step::settle) {
        uint8_t *tracks = at<uint8_t *>(st.entity, ENTITY_TRACKS);
        bool ready = tracks && swofty::mem::readable(tracks, 0x38) && at<uint8_t *>(tracks, 0x28) != at<uint8_t *>(tracks, 0x30);
        if (!ready || GetTickCount64() - st.mode_set < SETTLE_MS) {
            if (GetTickCount64() - st.mode_set < MODE_WAIT_MS)
                return Setup::pending;
            why = "the Time Trial mode's track system stayed empty";
            return Setup::failed;
        }
        st.step = Step::apply;
    }
    Setup s = apply(st.entity, request, why);
    if (s == Setup::done) {
        st.step = Step::done;
        if (!st.replay)
            despawn_main_scene("at the setup");
        swofty::log("[direct] Time Trial: set up %llu ms after the mode was made current",
                    (unsigned long long)(GetTickCount64() - st.mode_set));
    }
    return s;
}

}

namespace {

std::atomic<bool> tt_armed{false}, garage_seen{false};

// The Time Trial's intro states fade out for Broadcast (bit 2), BroadcastFull (bit 3) and CareerHub (bit 14)
// (Sess_IntroVideo_OnStateEnter 0x142d6cfc0) and fade back in from the intro video's end (0x142d6db20) or the
// front end's loading screen (FrontEnd, bit 1), neither of which a direct start runs: the garage stays black behind
// the fade's backdrop. Once the garage screen is up the core releases those reasons on the fade owner
// [[g_Game_Root]+0xa0] (mask +0x7b8, state +0x7bc, curve +0x7c4) and fades in when none is left.
constexpr uintptr_t ROOT_FADE = 0xa0, FADE_MASK = 0x7b8, FADE_STATE = 0x7bc, FADE_CURVE = 0x7c4,
                    FADE_HELD = 0x7cc;
constexpr uint32_t FADE_REASONS = (1u << 1) | (1u << 2) | (1u << 3) | (1u << 14), FADE_IN = 3, FADE_CURVE_DEFAULT = 0xd47eb226;

void release_fade()
{
    if (!game().ok)
        return;
    auto *root = *ptr<uint8_t *>(game().root);
    auto *owner = root && swofty::mem::readable(root, ROOT_FADE + 8) ? at<uint8_t *>(root, ROOT_FADE) : nullptr;
    if (!owner || !swofty::mem::readable(owner + FADE_MASK, 0x18)) {
        swofty::log("[direct] Time Trial: no fade owner to fade the garage in");
        return;
    }
    auto &mask = *reinterpret_cast<uint32_t *>(owner + FADE_MASK);
    uint32_t was = mask, state = at<uint32_t>(owner, FADE_STATE);
    mask = was & ~FADE_REASONS;
    if (!mask) {
        *reinterpret_cast<uint32_t *>(owner + FADE_STATE) = FADE_IN;
        owner[FADE_HELD] = 0;
        *reinterpret_cast<uint32_t *>(owner + FADE_CURVE) = FADE_CURVE_DEFAULT;
    }
    swofty::log("[direct] Time Trial: fade owner %p: mask %08x -> %08x, state %u -> %u", owner, was, mask, state,
                at<uint32_t>(owner, FADE_STATE));
}

void despawn_main_scene(const char *when)
{
    runtime::direct_start_despawn_frontend_scene(when);
}

void on_screen(const char *name)
{
    if (!tt_armed.load() || !name || std::strcmp(name, "garage_cockpit") || garage_seen.exchange(true))
        return;
    despawn_main_scene("as the garage opens");
    release_fade();
}

const bool screen_registered = (runtime::direct_start_register_time_trial_screen(on_screen), true);

Setup registered(std::string &why)
{
    static json::Value request;
    static bool parsed = false, usable = false;
    if (!parsed) {
        parsed = true;
        usable = json::parse(runtime::direct_start_request_text(), request);
    }
    if (!usable) {
        why = "the direct start request is not JSON";
        return Setup::failed;
    }
    tt_armed = true;
    return tt::setup(request, why);
}

const bool registered_at_load = (runtime::direct_start_register_time_trial(registered), true);

}

namespace runtime::direct_start::tt {

void reset_for_replay()
{
    st = State{};
    st.replay = true;
}

uintptr_t game_root()
{
    return game().root;
}

uintptr_t set_setup_mode()
{
    return game().set_setup_mode;
}

std::string current_track()
{
    if (!game().ok)
        return {};
    DirectStartContext ctx = runtime::direct_start_context();
    if (!ctx.game_modes)
        return {};
    std::vector<std::string> names;
    uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
    uint8_t *e = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
    if (!e || !swofty::mem::readable(e, ENTITY_TEAMS + 8))
        return {};
    uint8_t *now = ptr<CurrentTrack_t>(game().current_track)(e);
    const char *name = now && swofty::mem::readable(now, TRACK_NAME + 8) ? c_name(at<const char *>(now, TRACK_NAME)) : nullptr;
    return name ? name : "";
}

int track_stat()
{
    if (!game().ok)
        return -1;
    DirectStartContext ctx = runtime::direct_start_context();
    if (!ctx.game_modes)
        return -1;
    std::vector<std::string> names;
    uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
    uint8_t *e = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
    if (!e || !swofty::mem::readable(e, ENTITY_TEAMS + 8))
        return -1;
    uint8_t *now = ptr<CurrentTrack_t>(game().current_track)(e);
    if (!now || !swofty::mem::readable(now, TRACK_NAME + 8)) {
        std::string picked = runtime::track_select_time_trial();
        names.clear();
        now = picked.empty() ? nullptr : track_record(e, picked, names);
    }
    return now && swofty::mem::readable(now, 2) ? int(now[1]) : -1;
}

std::string session_track()
{
    std::string code = current_track();
    return code.empty() ? runtime::track_select_time_trial() : code;
}

void mode_objects(void *&mode_out, void *&entity_out)
{
    mode_out = entity_out = nullptr;
    if (!game().ok)
        return;
    DirectStartContext ctx = runtime::direct_start_context();
    if (!ctx.game_modes)
        return;
    std::vector<std::string> names;
    uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
    mode_out = mode;
    entity_out = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
}

std::string player_list()
{
    if (!game().ok)
        return {};
    DirectStartContext ctx = runtime::direct_start_context();
    if (!ctx.game_modes)
        return {};
    std::vector<std::string> names;
    uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
    uint8_t *e = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
    if (!e || !swofty::mem::readable(e, ENTITY_TEAMS + 8))
        return {};
    auto *list = at<uint8_t *>(e, ENTITY_LIST);
    const char *name = list ? holder_name(list, LIST_NAME) : nullptr;
    return name ? name : "";
}

bool player_team(uint32_t &ref, std::string &name)
{
    name.clear();
    if (!game().ok)
        return false;
    DirectStartContext ctx = runtime::direct_start_context();
    if (!ctx.game_modes)
        return false;
    std::vector<std::string> names;
    uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
    uint8_t *e = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
    int guard = *ptr<int>(game().player_guard);
    if (!e || !swofty::mem::readable(e, ENTITY_TEAMS + 8) || guard == 0 || guard == -1)
        return false;
    auto *sel = ptr<uint8_t>(game().player_entrants) + size_t(player_index()) * PLAYER_STRIDE;
    if (!swofty::mem::readable(sel, PLAYER_TEAM_SET + 1) || !sel[PLAYER_TEAM_SET])
        return false;
    ref = at<uint32_t>(sel, PLAYER_TEAM);
    // The selection is the front end's. A session the game started without it (its own change of car from a results
    // screen) loads a list the selected team may not be in; that selection names no car of this session.
    std::vector<uint32_t> teams;
    auto *list = at<uint8_t *>(e, ENTITY_LIST);
    if (list && swofty::mem::readable(list, LIST_END + 8)) {
        auto *begin = at<uint8_t *>(list, LIST_BEGIN), *end = at<uint8_t *>(list, LIST_END);
        if (begin && end > begin && size_t(end - begin) % RECORD == 0 && size_t(end - begin) / RECORD <= 4096 &&
            swofty::mem::readable(begin, size_t(end - begin)))
            for (auto *r = begin; r < end; r += RECORD)
                teams.push_back(at<uint32_t>(r, RECORD_TEAM));
    }
    if (!runtime::tt_loaded::selection_in_list(teams, ref))
        return false;
    name = team_name(e, ref);
    return true;
}

std::string player_team()
{
    uint32_t ref = 0;
    std::string name;
    player_team(ref, name);
    return name;
}

std::string team_of(uint32_t ref)
{
    if (!game().ok)
        return {};
    DirectStartContext ctx = runtime::direct_start_context();
    if (!ctx.game_modes)
        return {};
    std::vector<std::string> names;
    uint8_t *mode = mode_named(ctx.game_modes, MODE, names);
    uint8_t *e = mode && swofty::mem::readable(mode, MODE_ENTITY + 8) ? at<uint8_t *>(mode, MODE_ENTITY) : nullptr;
    if (!e || !swofty::mem::readable(e, ENTITY_TEAMS + 8))
        return {};
    return team_name(e, ref);
}

}
