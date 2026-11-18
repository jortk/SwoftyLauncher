#include "player_name_logic.hpp"

#include <cstdio>
#include <cstring>

namespace pn = runtime::player_name;

static int failed = 0;
static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

int main()
{
    std::string out;
    expect(pn::sanitize("Swofty", out) && out == "Swofty", "a plain name");
    expect(pn::sanitize("  Swofty   Racing \n", out) && out == "Swofty Racing", "whitespace is trimmed and collapsed");
    expect(pn::sanitize("Jos\xc3\xa9 \xc3\x96zil", out) && out == "Jos\xc3\xa9 \xc3\x96zil", "Latin-1 letters are kept");
    expect(pn::sanitize("A very long discord display name indeed", out) && out == "A very long discord disp", "cut at 24 characters");
    expect(pn::sanitize("\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9",
                       out) &&
                   out.size() == 48,
               "the cut never splits a multibyte character");
    expect(!pn::sanitize("", out) && !pn::sanitize("   ", out), "empty names are not used");
    expect(!pn::sanitize("Swofty \xf0\x9f\x8f\x8e", out) && out.empty(), "an emoji falls back to the stock name");
    expect(!pn::sanitize("\xe6\x97\xa5\xe6\x9c\xac", out), "characters outside the font fall back");
    expect(!pn::sanitize("bad\xff", out) && !pn::sanitize("trunc\xc3", out), "invalid UTF-8 falls back");
    expect(!pn::sanitize("a\x01" "b", out), "control characters fall back");
    expect(!pn::sanitize("5 \xc3\x97 3", out), "the multiplication sign is not a letter the font is assumed to draw");
    {
        std::string out;
        expect(pn::display_source("Swofty", "Discord", out) && out == "Swofty", "the environment text wins over the account name");
        expect(pn::display_source(nullptr, "Discord", out) && out == "Discord", "no environment text uses the account name");
        expect(pn::display_source("", "Discord", out) && out == "Discord", "an empty environment text uses the account name");
        expect(!pn::display_source("0", "Discord", out), "0 turns the feature off");
        expect(!pn::display_source(nullptr, "", out), "nothing to show");

        runtime::presence_id::Id me;
        me.type = 1;
        me.value[0] = 76561198803768583ull;
        uint8_t id[0x50] = {};
        id[0] = 1;
        uint64_t mask = 0x1234567890abcdefull, v = me.value[0] ^ mask;
        std::memcpy(id + 8, &v, 8);
        std::memcpy(id + 0x10, &mask, 8);
        expect(pn::is_local_id(id, me), "the presence's own id is local");
        uint64_t other = me.value[0] + 1 ^ mask;
        std::memcpy(id + 8, &other, 8);
        expect(!pn::is_local_id(id, me), "another user's id is not local");
        std::memcpy(id + 8, &v, 8);
        id[0x39] = 1;
        expect(!pn::is_local_id(id, me), "a different flag is another id");
        id[0x39] = 0;
        id[0] = 3;
        expect(!pn::is_local_id(id, me), "another type is another id");
        id[0] = 1;
        id[0x4c] = 1;
        expect(!pn::is_local_id(id, me), "only kind 0 keys are compared");
        id[0x4c] = 0;
        runtime::presence_id::Id none;
        expect(!pn::is_local_id(id, none), "an unknown presence matches nothing");
        runtime::presence_id::Id zero = me;
        zero.value[0] = 0;
        uint8_t blank[0x50] = {};
        blank[0] = 1;
        expect(!pn::is_local_id(blank, zero), "a zero id is never local");
        {
            runtime::presence_id::Id flagged = me;
            flagged.flag39 = true;
            auto back = pn::id_of(pn::id_tag(flagged), flagged.value[0]);
            expect(back.type == 1 && back.flag39 && back.value[0] == me.value[0], "the packed id keeps type, flag39 and value0");
            auto plain = pn::id_of(pn::id_tag(me), me.value[0]);
            expect(pn::is_local_id(id, plain), "the packed id matches the local key like the full id");
            id[0x39] = 1;
            expect(!pn::is_local_id(id, plain) && pn::is_local_id(id, back), "the packed flag39 is compared");
            id[0x39] = 0;
        }
        expect(pn::account_reread(false, false, false, false), "the account file is read once at start");
        expect(!pn::account_reread(true, true, true, true), "an unchanged account file is not read again");
        expect(pn::account_reread(true, true, true, false), "a new write time reads it again");
        expect(pn::account_reread(true, false, true, false) && pn::account_reread(true, true, false, false),
               "an account file that appears or disappears is read again");
        expect(!pn::account_reread(true, false, false, false), "a missing account file is not read again");
        expect(pn::lookup_renames(true, true, 7, 7), "an armed rename in its own Time Trial session renames");
        expect(!pn::lookup_renames(false, true, 7, 7), "no rename outside a Time Trial session");
        expect(!pn::lookup_renames(true, false, 7, 7), "no rename when none is armed");
        expect(!pn::lookup_renames(true, true, 6, 7), "a rename armed for an earlier session does not carry over");
        {
            pn::SwapPlan p;
            expect(pn::swap_due(p, false, 1, "Stock", "Shown", true, 0) == pn::SwapWhy::none, "no swap outside a Time Trial on our content");
            expect(pn::swap_due(p, true, 1, "", "Shown", true, 0) == pn::SwapWhy::none, "no swap before the stock name is known");
            expect(pn::swap_due(p, true, 1, "Stock", "", true, 0) == pn::SwapWhy::none, "no swap without a name to show");
            expect(pn::swap_due(p, true, 1, "Stock", "Stock", true, 0) == pn::SwapWhy::none, "no swap when the names are equal");
            expect(pn::swap_due(p, true, 1, "AVeryLongStockName", "Shown", true, 0) == pn::SwapWhy::none, "no swap past the small string size");
            expect(pn::swap_due(p, true, 1, "Stock", "Shown", false, 0) == pn::SwapWhy::session, "the first need in a session swaps");
            pn::swap_taken(p, pn::SwapWhy::session, 1, "Stock", "Shown", 1000);
            expect(pn::swap_due(p, true, 1, "Stock", "Shown", false, 500000) == pn::SwapWhy::none, "no second scan for the same need");
            expect(pn::swap_due(p, true, 1, "Stock", "Shown", true, 1000 + pn::SWAP_GAP_MS - 1) == pn::SwapWhy::none,
                   "a visible stock name waits for the gap");
            expect(pn::swap_due(p, true, 1, "Stock", "Shown", true, 1000 + pn::SWAP_GAP_MS) == pn::SwapWhy::visible,
                   "a visible stock name after the gap scans again");
            uint64_t t = 1000;
            for (int i = 0; i < pn::MAX_REVISITS; i++) {
                t += pn::SWAP_GAP_MS;
                expect(pn::swap_due(p, true, 1, "Stock", "Shown", true, t) == pn::SwapWhy::visible, "a revisit within the cap");
                pn::swap_taken(p, pn::SwapWhy::visible, 1, "Stock", "Shown", t);
            }
            expect(pn::swap_due(p, true, 1, "Stock", "Shown", true, t + 10 * pn::SWAP_GAP_MS) == pn::SwapWhy::none,
                   "at most MAX_REVISITS revisits a session");
            expect(pn::swap_due(p, true, 1, "Stock", "Other", false, t) == pn::SwapWhy::renamed, "a new name to show scans again");
            expect(pn::swap_due(p, false, 2, "Stock", "Shown", true, t) == pn::SwapWhy::none, "a career or Grand Prix session never scans");
            expect(pn::swap_due(p, true, 2, "Stock", "Shown", false, t) == pn::SwapWhy::session, "the next Time Trial session scans once");
            pn::swap_taken(p, pn::SwapWhy::session, 2, "Stock", "Shown", t);
            expect(p.revisits == 0, "a new session starts with no revisits");
            expect(pn::swap_continues(true, 2, 2) && !pn::swap_continues(false, 2, 2) && !pn::swap_continues(true, 2, 3),
                   "a scan stops when its session is over");
        }
        expect(pn::root_allowed(pn::djb2("network_drivers")) && pn::root_allowed(pn::djb2("players")) && !pn::root_allowed(pn::djb2("hud")),
               "only the driver roots are rewritten");
        expect(pn::leaf_name(0xd23bb5d8) && !strcmp(pn::leaf_name(0xd23bb5d8), "full_name"), "leaf hash to name");
        expect(pn::leaf_name(pn::djb2("gamertag")) && !pn::leaf_name(pn::djb2("team")), "only name leaves");
    }
    if (failed)
        return 1;
    std::printf("all passed\n");
}
