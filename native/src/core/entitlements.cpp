#include "entitlements_logic.hpp"
#include "runtime.hpp"

#include <swofty/game.hpp>
#include <swofty/mem.hpp>

#include <atomic>
#include <cstdint>
#include <string>

#include <swofty_api.h>

namespace {

namespace mem = swofty::mem;
namespace ent = runtime::entitlements;

struct Game {
    bool u64(uint64_t a, uint64_t &out)
    {
        return mem::read(static_cast<uintptr_t>(a), out);
    }
};

std::atomic<int> f1_2026{ent::UNKNOWN};
std::atomic<uint64_t> frontend_at{0};
ent::Settle settle;
bool reported_missing = false;

int owned_op(const char *name)
{
    return ent::tracked(name) ? f1_2026.load() : ent::UNKNOWN;
}

const SwoftyContentOps table = {SWOFTY_CONTENT_OPS_VERSION, owned_op};

void publish(int state)
{
    swofty::log("[entitlements] %s: %s", ent::F1_2026, ent::word(state));
    std::string body = ent::json(state);
    if (!body.empty() && !runtime::write_file(runtime::swofty_root() + L"entitlements.json", body))
        swofty::log("[entitlements] swofty\\entitlements.json not written");
}

}

namespace runtime {

void entitlements_flow(uint32_t hash, uint32_t to)
{
    uint64_t none = 0;
    if (hash == ent::STATE_FRONTEND && (to == 1 || to == 2))
        frontend_at.compare_exchange_strong(none, GetTickCount64());
}

void entitlements_tick()
{
    static uint64_t last_read = 0;
    int now = f1_2026.load();
    if (now == ent::OWNED)
        return;
    uint64_t ms = GetTickCount64();
    if (!ent::read_due(now, session_life::session_live(session_kind()), last_read, ms) || !code_ready() ||
        !ent::frontend_settled(frontend_at.load(), ms))
        return;
    last_read = ms;
    mem::Trust trust(HEAP_TRUST_MS);
    uintptr_t slot = swofty::api->resolve("g_FeTile_Ctx");
    if (!slot || !swofty::game::FeTile_Check_EntrantListUsable.address()) {
        if (!reported_missing) {
            reported_missing = true;
            swofty::log("[entitlements] %s: unknown, this build does not resolve g_FeTile_Ctx and FeTile_Check_EntrantListUsable", ent::F1_2026);
        }
        return;
    }
    uint64_t ctx = 0;
    Game game;
    if (!game.u64(slot, ctx) || !ent::ctx_ready(game, ctx))
        return;
    bool usable = (swofty::game::FeTile_Check_EntrantListUsable(reinterpret_cast<void *>(ctx), ent::F1_2026) & 0xff) != 0;
    if (settle.read(usable)) {
        f1_2026 = settle.state();
        publish(settle.state());
    }
}

}

extern "C" __declspec(dllexport) const SwoftyContentOps *SwoftyCoreContentOps(void)
{
    return &table;
}
