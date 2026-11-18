#include "../../src/core/apply/content/boot.hpp"

#include <cstdio>
#include <map>

using namespace swofty_core;
using swofty::json::Value;
namespace content = apply::content;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static Value vehicle()
{
    return Value::object({{"label", Value("sw22_sauber")}, {"team_from", Value("cs22_sauber")}});
}

static std::string tower_of(const content::Plan &p)
{
    for (std::size_t i = 0; i < p.ops.size(); i++) {
        const Value &op = p.ops[i];
        if (op.is_object() && op["op"].is_string() && op["op"].as_string() == "add-row" &&
            op["set"]["_metadata_"].as_string() == std::string("sw22_sauber") && op["set"]["timing_tower_texture_path"].is_string())
            return std::string(op["set"]["timing_tower_texture_path"].as_string());
    }
    return {};
}

int main()
{
    std::map<std::string, std::string> db = {
        {"entrants.teams.team[_metadata_=cs22_sauber]._metadata_", "cs22_sauber"},
        {"entrants.teams.team[_metadata_=cs22_sauber].timing_tower_texture_path", "cs22_sauber"}};
    content::Read read = [&](const std::string &path) -> std::optional<std::string> {
        auto it = db.find(path);
        return it == db.end() ? std::nullopt : std::optional<std::string>(it->second);
    };
    Value rec = vehicle();
    {
        content::Plan p;
        content::lower_vehicle_team(rec, read, p);
        check(p.ops.size() >= 1 && tower_of(p).empty(), "no identity: the copied row keeps the base row's tower badge");
        check(!p.lines.empty() && p.lines[0].find("timing tower") == std::string::npos, "and the plan line names no badge");
    }
    {
        content::Plan p;
        content::lower_vehicle_team(rec, read, p, "sw.addon/swt22_sauber", "swt22_sauber");
        check(tower_of(p) == "swt22_sauber", "an identity with its own badge: the row takes the identity's key");
        check(!p.lines.empty() && p.lines[0].find("timing tower badge swt22_sauber from sw.addon/swt22_sauber") != std::string::npos,
              "and the plan line names the badge and the identity");
    }
    {
        db.erase("entrants.teams.team[_metadata_=cs22_sauber].timing_tower_texture_path");
        content::Plan p;
        content::lower_vehicle_team(rec, read, p, "sw.addon/swt22_sauber", "swt22_sauber");
        check(tower_of(p).empty(), "a base row without the field is left as it is");
    }
    {
        Value own = Value::object({{"images", Value::object({{"hud", Value("hud.png")}})}});
        auto b = content::tower_badge(&own, "swt22_sauber");
        check(b.image && b.key == "swt22_sauber", "a logo with a hud file gives the identity's label");
        Value stock = Value::object({{"images", Value::object({{"hud", Value::object({{"stock", Value("ferrari")}})}})}});
        auto s = content::tower_badge(&stock, "swt22_ferrari");
        check(!s.image && s.key == "ferrari", "a logo with a stock hud key gives that key");
        Value none = Value::object({});
        auto n = content::tower_badge(&none, "swt22_x");
        check(n.key.empty(), "a logo with no tower image gives no key");
        check(content::tower_badge(nullptr, "swt22_x").key.empty(), "no logo gives no key");
    }
    return failures ? 1 : 0;
}
