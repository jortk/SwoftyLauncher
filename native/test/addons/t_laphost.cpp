#include "wine_test.hpp"

// A lap module stand-in: it attaches to the core's lap host (SwoftyCoreLapHost), offers one hook set when the core
// collects them and logs what the core tells it.

namespace {

const SwoftyLapHost *host = nullptr;
bool offered = false, ticked = false;

// The set holds one on-load entry, so it installs nothing in the stand-in game; the core does not call it.
void loaded() {}

void offer()
{
    if (offered)
        return;
    offered = true;
    const SwoftyMixin set[] = {{nullptr, SWOFTY_MIXIN_ON_LOAD, 0, reinterpret_cast<void *>(&loaded), nullptr}};
    int ok = host->add_hooks("t_laphost.dll (lap host set)", set, 1);
    swofty::log("[t_laphost] offer: add_hooks answered %d, code ready %d", ok, host->code_ready());
}

void settle()
{
    static bool told = false;
    if (!told && host->hooks_active("t_laphost.dll (lap host set)")) {
        told = true;
        swofty::log("[t_laphost] settle: the hook set is active");
    }
}

void tick()
{
    if (ticked)
        return;
    ticked = true;
    swofty::log("[t_laphost] the first flow tick");
}

int notice_wanted()
{
    swofty::log("[t_laphost] the overlay setup asked whether a notice line is wanted");
    return 0;
}

const SwoftyLapClient client = {sizeof(SwoftyLapClient), 1, offer, settle, tick, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, notice_wanted};

}

SWOFTY_ON_LOAD
{
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreLapHostFn>(reinterpret_cast<void *>(GetProcAddress(core, "SwoftyCoreLapHost"))) : nullptr;
    host = get ? get() : nullptr;
    if (!host) {
        swofty::log("[t_laphost] no lap host table");
        return;
    }
    std::string swofty_root = host->swofty_root(), files_root = host->files_root();
    swofty::log("[t_laphost] lap host version %u, size %s, swofty root %s, files root %s, build bits %u, attach %d", host->version,
                host->size >= sizeof(SwoftyLapHost) ? "ok" : "short", swofty_root.empty() ? "empty" : "set", files_root.empty() ? "empty" : "set",
                host->build(), host->attach(&client));
    host->notice("");
}

SWOFTY_ADDON()
