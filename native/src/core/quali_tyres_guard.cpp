#include "runtime.hpp"

#include <windows.h>

#include "apply/content/appliers.hpp"
#include "apply/content/quali_tyres.hpp"

#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

// Qualifying tyres per box (content.package 1.5 rules.qualifying_tyres, apply/content/quali_tyres.hpp).
//
// A box's package names the dry class options its cars may use only in qualifying (the late 1970s to 1991 qualifying
// tyres). The era picker's box press stores the box's option mask here (quali_tyres_box); a stock box press, a box
// without the rule and the era picker's put backs store 0. In a race session (g_Sess_TyreRulesSessionType 15..17) the
// core then refuses those options in three places that pick or offer a tyre set (re/db/functions.tsv):
//
// - Car_Tyre_IsCompoundAllowed 0x142e88630 (set usable now; 16 callers: the MFD pit tyre options, the garage, the
//   race helper): a set whose option (set record +0x04) is masked is not usable;
// - Car_Tyre_CheckCompoundClassRule 0x142e88730 (the mode-0 session tag check; the least-worn picker 0x142e8ab90 calls
//   it directly, not through 0x142e88630): the same;
// - Sess_Tyres_ClosestAvailableCompound 0x14368fb40 (the garage set choice and AI run planning): masked options in the
//   five-entry preference list become 0xffffffff, which its range test skips (test edi,edi / sets at 0x150dc4bc9).
//
// The sets themselves are left alone: the per-session preset (tyres.allocation_tyre_presets, data) hands the
// qualifying option out only in Qualifying1..3. Outside races, without a box mask, and for options 3..4 every wrap runs
// the game's code unchanged.
//
// The hooks are offered at the first mixin commit after a content.package with rules.qualifying_tyres was registered,
// so a game without such a box runs none of them. SWOFTY_QUALI_TYRES=0 turns them off.

namespace {

namespace content = swofty_core::apply::content;

constexpr const char *MIXIN_ALLOWED = "swofty_core.dll (qualifying tyres: set usable)";
constexpr const char *MIXIN_CLASS = "swofty_core.dll (qualifying tyres: session tag check)";
constexpr const char *MIXIN_CLOSEST = "swofty_core.dll (qualifying tyres: set choice)";
constexpr const char *ALLOWED = "Car_Tyre_IsCompoundAllowed";
constexpr const char *CLASS_RULE = "Car_Tyre_CheckCompoundClassRule";
constexpr const char *CLOSEST = "Sess_Tyres_ClosestAvailableCompound";
constexpr const char *SESSION_TYPE = "g_Sess_TyreRulesSessionType";
constexpr size_t SET_OPTION = 0x04;   // set record (0x6c bytes): u32 class option; +0x00 session tag, +0x09 flags
constexpr int LOG_FIRST = 8;

using SetCheck = uint64_t(const uint8_t *set);
using Closest = uint32_t(void *tyre_rules, uint8_t vehicle, const uint32_t *preference, float wear_scale);
SetCheck *orig_allowed = nullptr, *orig_class = nullptr;
Closest *orig_closest = nullptr;

std::atomic<uint8_t> g_mask{0};
const uint32_t *g_session = nullptr;
std::atomic<int> refused{0};
bool offered = false, settled = false, wanted = false;
std::mutex line_mu;
std::string last_line;

uint32_t session_now()
{
    return g_session ? *g_session : 0;
}

void log_refusal(const char *where, uint32_t option, uint32_t session)
{
    int n = ++refused;
    if (n > LOG_FIRST)
        return;
    swofty::log("[tyres] %s: option %u (%s) is qualifying only, refused in session %u%s", where, option,
                option < 3 ? content::QUALI_OPTION_NAMES[option] : "?", session,
                n == LOG_FIRST ? " (further refusals are not logged)" : "");
}

bool refused_set(const uint8_t *set, const char *where)
{
    uint8_t mask = g_mask.load(std::memory_order_relaxed);
    if (!mask || !set)
        return false;
    uint32_t session = session_now(), option = 0;
    memcpy(&option, set + SET_OPTION, sizeof option);
    if (!content::quali_refused(session, option, mask))
        return false;
    log_refusal(where, option, session);
    return true;
}

uint64_t allowed(const uint8_t *set)
{
    if (refused_set(set, "set usable"))
        return 0;
    return orig_allowed(set);
}

uint64_t class_rule(const uint8_t *set)
{
    if (refused_set(set, "session tag check"))
        return 0;
    return orig_class(set);
}

uint32_t closest(void *tyre_rules, uint8_t vehicle, const uint32_t *preference, float wear_scale)
{
    uint8_t mask = g_mask.load(std::memory_order_relaxed);
    if (!mask || !preference)
        return orig_closest(tyre_rules, vehicle, preference, wear_scale);
    uint32_t session = session_now(), own[5];
    int skipped = content::quali_filter_preference(session, mask, preference, own);
    if (!skipped)
        return orig_closest(tyre_rules, vehicle, preference, wear_scale);
    for (int i = 0; i < 5; ++i)
        if (own[i] == content::QUALI_NO_SLOT && preference[i] != content::QUALI_NO_SLOT) {
            log_refusal("set choice", preference[i], session);
            break;
        }
    return orig_closest(tyre_rules, vehicle, own, wear_scale);
}

bool turned_off()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_QUALI_TYRES", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[tyres] qualifying tyre hooks off (SWOFTY_QUALI_TYRES=%s)", v);
        return true;
    }
    return false;
}

}

namespace runtime {

// The era picker's box press: the pressed box's qualifying-only option mask (0 = none) and the line to log.
void quali_tyres_box(uint8_t mask, const std::string &line)
{
    uint8_t before = g_mask.exchange(mask);
    std::lock_guard g(line_mu);
    if (line.empty() || (line == last_line && before == mask))
        return;
    last_line = line;
    swofty::log("[tyres] %s%s", line.c_str(), mask && !wanted ? "; not applied (the qualifying tyre hooks are not offered)" : "");
}

std::vector<CoreMixins> quali_tyres_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready() || !swofty_core::apply::content::any_quali_tyres())
        return out;
    offered = true;
    if (turned_off())
        return out;
    uintptr_t session = swofty::api->resolve(SESSION_TYPE);
    if (!session) {
        swofty::log("[tyres] qualifying tyre hooks off: needs %s, which this build does not resolve", SESSION_TYPE);
        return out;
    }
    g_session = reinterpret_cast<const uint32_t *>(session);
    wanted = true;
    if (swofty::api->resolve(ALLOWED))
        out.push_back({MIXIN_ALLOWED, {{ALLOWED, SWOFTY_MIXIN_WRAP, 100, (void *)&allowed, (void **)&orig_allowed}}});
    if (swofty::api->resolve(CLASS_RULE))
        out.push_back({MIXIN_CLASS, {{CLASS_RULE, SWOFTY_MIXIN_WRAP, 100, (void *)&class_rule, (void **)&orig_class}}});
    if (swofty::api->resolve(CLOSEST))
        out.push_back({MIXIN_CLOSEST, {{CLOSEST, SWOFTY_MIXIN_WRAP, 100, (void *)&closest, (void **)&orig_closest}}});
    if (out.empty()) {
        wanted = false;
        swofty::log("[tyres] qualifying tyre hooks off: this build resolves none of %s, %s, %s", ALLOWED, CLASS_RULE, CLOSEST);
    }
    return out;
}

void quali_tyres_commit()
{
    if (!offered || settled || !wanted)
        return;
    settled = true;
    swofty::log("[tyres] qualifying tyre hooks: set usable %s, session tag check %s, set choice %s",
                core_mixins_active(MIXIN_ALLOWED) ? "armed" : "not installed",
                core_mixins_active(MIXIN_CLASS) ? "armed" : "not installed",
                core_mixins_active(MIXIN_CLOSEST) ? "armed" : "not installed");
}

}
