#include "runtime.hpp"

#include <windows.h>

#include <chrono>
#include <cstring>
#include <memory>
#include <mutex>
#include <set>
#include <string>

#include <swofty/mixin.hpp>

#include "career_saves_logic.hpp"
#include "hooks/core_events.hpp"
#include "hooks/hooks.hpp"
#include "registry/table.hpp"

// Career saves (since 1.7.0): the career index and the per-career addon data.
//
// The index is $DATA/careers/index.json, the launcher's data folder (state_root; the game's swofty folder
// when the launcher named none), in the career index shape of docs/std/dependencies.md: one record per
// career id with the addons it needs. Fields the launcher or an older core wrote stay as they were. Each
// career's addon data is $DATA/careers/data/<career>/<addon>.json, {"version": n, "data": ...}.
//
// The list follows the game's save slots: at the end of a good profile load, from every mode's slot manager
// (Game_SaveSlotManager_Deserialise), and after each good profile save, from the same managers (save_splice.cpp
// keeps them). A slot the list did not hold is added; a career whose slot is gone is "deleted" when this session
// listed it before and "missing" when it did not. career.loaded makes or refreshes the loaded career's record
// (taking over the record the slot list made for it), before each profile save career.saving refreshes it, and
// career.saved stamps last_saved. career.missing fires when a list newly marks careers missing.

namespace {

namespace cs = runtime::career_saves;
namespace hk = swofty_core::hooks;
namespace reg = swofty_core::registry;
using swofty::json::Value;

std::mutex mu;
cs::Index index_;
bool index_read = false;
std::string current_id;
std::set<std::string> present;
std::string last_error;
std::string session_id = std::to_string(std::chrono::duration_cast<std::chrono::milliseconds>(
                                            std::chrono::system_clock::now().time_since_epoch())
                                            .count());
std::once_flag started;

struct Described {
    std::string driver, team, label;
    int season = 0, round = 0;
};
Described described;

std::wstring root()
{
    const std::wstring &state = runtime::state_root();
    return state.empty() ? runtime::swofty_root() : state;
}

std::wstring path_of(const std::string &rel)
{
    std::wstring w = root() + runtime::wide(rel);
    for (wchar_t &c : w)
        if (c == L'/')
            c = L'\\';
    return w;
}

void make_dirs(const std::wstring &dir)
{
    for (size_t i = dir.find_first_of(L"\\/", 3); i != std::wstring::npos; i = dir.find_first_of(L"\\/", i + 1))
        CreateDirectoryW(dir.substr(0, i).c_str(), nullptr);
    CreateDirectoryW(dir.c_str(), nullptr);
}

bool put(const std::string &rel, const std::string &text)
{
    std::wstring file = path_of(rel);
    size_t cut = file.find_last_of(L'\\');
    if (cut != std::wstring::npos)
        make_dirs(file.substr(0, cut));
    return runtime::write_file(file, text);
}

int64_t now_ms()
{
    return std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::system_clock::now().time_since_epoch()).count();
}

void read_index_locked()
{
    if (index_read)
        return;
    index_read = true;
    std::string text = runtime::read_file(path_of("careers/index.json"), size_t(32) << 20);
    if (!index_.load(text)) {
        swofty::log("[saves] careers/index.json is not a career index; starting a new one (the old file is kept as "
                    "index.bad.json)");
        put("careers/index.bad.json", text);
        index_.load("");
    }
}

bool flush_locked()
{
    read_index_locked();
    if (!put("careers/index.json", index_.to_json())) {
        if (last_error.empty())
            swofty::log("[saves] could not write careers/index.json");
        last_error = "write";
        return false;
    }
    last_error.clear();
    return true;
}

std::string owner_of(const std::string &data_group)
{
    if (data_group.empty())
        return {};
    std::string owner = runtime::career_content_owner(data_group);
    if (owner.empty()) {
        reg::Store &st = reg::store();
        for (auto &id : st.list("career.mode", "")) {
            reg::Record r = st.get("career.mode", id);
            if (r.status == reg::Status::ok && r.value["data_group"].as_string() == data_group) {
                owner = id.substr(0, id.find('/'));
                break;
            }
        }
    }
    return owner == "game" || owner == "swofty.core" ? std::string() : owner;
}

std::string content_of(const std::string &data_group, std::vector<cs::Need> &needs)
{
    reg::Store &st = reg::store();
    for (auto &id : st.list("career.content", "")) {
        reg::Record r = st.get("career.content", id);
        if (r.status != reg::Status::ok || r.value["data_group"].as_string() != data_group)
            continue;
        if (const swofty::json::Array *list = r.value["needs"].if_array())
            for (const Value &n : *list)
                if (cs::addon_id(n["id"].as_string()))
                    needs.push_back({std::string(n["id"].as_string()), std::string(n["range"].as_string()),
                                     std::string(n["why"].as_string())});
        return id;
    }
    return {};
}

void refresh_locked(const hk::Facts &f)
{
    if (current_id.empty())
        return;
    const std::string &id = current_id;
    std::string group = f.career ? f.career->data_group : std::string();
    index_.set_text(id, "mode", f.mode);
    index_.set_text(id, "content", group);
    std::vector<cs::Need> needs;
    std::string record = content_of(group, needs);
    index_.set_text(id, "content_record", record);
    std::string owner = owner_of(group);
    if (!owner.empty())
        index_.need(id, {owner, "", record.empty() ? "its career mode" : "career content " + group});
    for (auto &n : needs)
        index_.need(id, n);
    std::string driver = described.driver, team = described.team;
    if (driver.empty() && f.player)
        driver = f.player->driver;
    if (team.empty() && f.player)
        team = f.player->team;
    index_.set_text(id, "driver", driver);
    index_.set_text(id, "team", team);
    int season = described.season ? described.season : (f.career ? f.career->year : 0);
    int round = described.round ? described.round : (f.career ? f.career->round : 0);
    if (season)
        index_.set_int(id, "season", season);
    if (round)
        index_.set_int(id, "round", round);
    std::string label = described.label.empty() ? cs::label_of(group, driver, team, season) : described.label;
    index_.set_text(id, "label", label);
    index_.touch(id)["status"] = Value(cs::STATUS_PRESENT);
    present.insert(id);
}

std::shared_ptr<const hk::Facts> facts_now() { return hk::core().current(); }

int on_loaded(void *, SwoftyHookCall *)
{
    auto f = facts_now();
    if (!f || !f->career || f->career->id.empty())
        return 0;
    std::lock_guard<std::mutex> g(mu);
    read_index_locked();
    current_id = f->career->id;
    if (index_.adopt(current_id))
        swofty::log("[saves] career %s was listed from the save slots; its record now carries this id", current_id.c_str());
    described = {};
    refresh_locked(*f);
    if (flush_locked())
        swofty::log("[saves] career %s is in the career index (%zu career(s))", current_id.c_str(), index_.size());
    return 0;
}

int on_unloaded(void *, SwoftyHookCall *)
{
    std::lock_guard<std::mutex> g(mu);
    current_id.clear();
    described = {};
    return 0;
}

void start()
{
    std::call_once(started, [] {
        uint32_t token = 0;
        hk::core().attach("swofty.core", "career.loaded", SWOFTY_PHASE_AFTER, 900000, "", on_loaded, nullptr, &token);
        hk::core().attach("swofty.core", "career.unloaded", SWOFTY_PHASE_AFTER, 900000, "", on_unloaded, nullptr, &token);
    });
}

int copy_out(const std::string &text, char *out, size_t cap, size_t *len)
{
    if (len)
        *len = text.size();
    if (!out || cap < text.size() + 1)
        return SWOFTY_TOO_SMALL;
    std::memcpy(out, text.data(), text.size());
    out[text.size()] = 0;
    return SWOFTY_ASSET_OK;
}

int api_available(void) { return 1; }

int api_index(char *json, size_t cap, size_t *len)
{
    std::lock_guard<std::mutex> g(mu);
    read_index_locked();
    return copy_out(index_.to_json(), json, cap, len);
}

int api_current(char *id, size_t cap, size_t *len)
{
    std::lock_guard<std::mutex> g(mu);
    if (current_id.empty())
        return SWOFTY_ASSET_LATE;
    return copy_out(current_id, id, cap, len);
}

int api_missing(char *json, size_t cap, size_t *len)
{
    std::lock_guard<std::mutex> g(mu);
    read_index_locked();
    return copy_out(index_.only_json(index_.missing_ids()), json, cap, len);
}

int api_describe(SwoftyText request)
{
    swofty::json::Error e;
    Value v = swofty::json::parse(std::string_view(request.data, request.size), &e);
    if (!e.ok() || !v.is_object())
        return SWOFTY_ASSET_INVALID;
    std::lock_guard<std::mutex> g(mu);
    if (current_id.empty())
        return SWOFTY_ASSET_LATE;
    if (v.contains("driver"))
        described.driver = std::string(v["driver"].as_string());
    if (v.contains("team"))
        described.team = std::string(v["team"].as_string());
    if (v.contains("label"))
        described.label = std::string(v["label"].as_string());
    if (v.contains("season"))
        described.season = int(v["season"].as_int());
    if (v.contains("round"))
        described.round = int(v["round"].as_int());
    if (auto f = facts_now())
        refresh_locked(*f);
    return SWOFTY_ASSET_OK;
}

int api_need(const char *addon, const char *range, const char *why)
{
    if (!addon || !cs::addon_id(addon))
        return SWOFTY_ASSET_INVALID;
    std::lock_guard<std::mutex> g(mu);
    if (current_id.empty())
        return SWOFTY_ASSET_LATE;
    read_index_locked();
    index_.need(current_id, {addon, range ? range : "", why ? why : ""});
    return SWOFTY_ASSET_OK;
}

bool data_args(const char *career, const char *addon)
{
    return career && addon && cs::plain_id(career) && cs::addon_id(addon);
}

int api_data_get(const char *career, const char *addon, char *json, size_t cap, size_t *len, uint32_t *version)
{
    if (!data_args(career, addon))
        return SWOFTY_ASSET_INVALID;
    std::string text = runtime::read_file(path_of(cs::data_file(career, addon)), cs::MAX_DATA + 1024);
    uint32_t v = 0;
    std::string value;
    if (text.empty() || !cs::unwrap_data(text, v, value))
        return SWOFTY_ASSET_INVALID;
    if (version)
        *version = v;
    return copy_out(value, json, cap, len);
}

int api_data_set(const char *career, const char *addon, uint32_t version, SwoftyText json)
{
    if (!data_args(career, addon) || json.size > cs::MAX_DATA)
        return SWOFTY_ASSET_INVALID;
    swofty::json::Error e;
    Value v = swofty::json::parse(std::string_view(json.data, json.size), &e);
    if (!e.ok())
        return SWOFTY_ASSET_INVALID;
    {
        std::lock_guard<std::mutex> g(mu);
        read_index_locked();
        if (!index_.knows(career))
            return SWOFTY_REFUSED;
    }
    return put(cs::data_file(career, addon), cs::wrap_data(version, v)) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
}

int api_data_remove(const char *career, const char *addon)
{
    if (!data_args(career, addon))
        return SWOFTY_ASSET_INVALID;
    DeleteFileW(path_of(cs::data_file(career, addon)).c_str());
    return SWOFTY_ASSET_OK;
}

const SwoftyCareerSaves table = {SWOFTY_CAREER_SAVES_VERSION, api_available, api_index,      api_current,
                                 api_describe,                api_need,      api_data_get,   api_data_set,
                                 api_data_remove,             api_missing};

}

namespace runtime {

void career_saves_start() { start(); }

void career_saves_saving()
{
    hk::events::career_saving();
    std::lock_guard<std::mutex> g(mu);
    if (current_id.empty())
        return;
    read_index_locked();
    if (auto f = facts_now())
        refresh_locked(*f);
    flush_locked();
}

void career_saves_listed(const std::vector<career_saves::SlotRead> &slots, bool complete, bool at_load)
{
    cs::ListResult r;
    {
        std::lock_guard<std::mutex> g(mu);
        read_index_locked();
        cs::ListContext ctx;
        ctx.session = session_id;
        ctx.current = current_id;
        ctx.now_ms = now_ms();
        ctx.complete = complete;
        ctx.at_load = at_load;
        r = index_.apply_list(slots, ctx);
        for (auto &id : r.added)
            swofty::log("[saves] career %s is in the game's save slots and was not in the career list", id.c_str());
        for (auto &id : r.back)
            swofty::log("[saves] career %s is in the game's save slots again", id.c_str());
        for (auto &id : r.deleted)
            swofty::log("[saves] career %s was deleted in game (its save slot is gone)", id.c_str());
        for (auto &id : r.missing)
            swofty::log("[saves] career %s is in the career list and the %s did not find it", id.c_str(),
                        at_load ? "profile load" : "profile save");
        if (r.changed || at_load || !slots.empty())
            flush_locked();
    }
    if (!r.missing.empty())
        hk::events::career_missing();
}

std::string career_saves_id_for_slot(const std::string &key)
{
    std::lock_guard<std::mutex> g(mu);
    read_index_locked();
    return index_.career_for_slot(key);
}

void career_saves_saved()
{
    {
        std::lock_guard<std::mutex> g(mu);
        if (!current_id.empty()) {
            read_index_locked();
            index_.saved(current_id, now_ms());
            flush_locked();
        }
    }
    hk::events::career_saved();
}

}

extern "C" __declspec(dllexport) const SwoftyCareerSaves *SwoftyCoreCareerSaves(void)
{
    return &table;
}
