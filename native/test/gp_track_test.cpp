// Host-native unit test for the Grand Prix remembered-track fallback (native/src/core/apply/gp_track_logic.hpp): the
// first source with rows decides, a weekend track stat id with no row or a dormant row is swapped for the stock
// track, a live row is left alone, the stock id may come from a later source, and the three 2026-11-11 crash bundles
// replay to a swap from every source they had. Built and run by native/test/run_gp_track_test.sh.

#include "apply/gp_track_logic.hpp"

#include <cstdio>

namespace gp = swofty_core::apply::gp_track;

static int failed = 0;

static void expect(bool ok, const char *what)
{
    std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failed++;
}

static bool has(const std::string &s, const char *part)
{
    return s.find(part) != std::string::npos;
}

int main()
{
    gp::Source table{"the tracks table (read at the window stage)", {{0, "melbourne"}, {2, "shanghai"}, {3, "bahrain"}, {35, "bahrain24"}, {45, "adelaide"}}, {35}};
    gp::Source base{"the track base", {{0, "melbourne"}, {3, "bahrain"}, {45, "adelaide"}}, {}};
    gp::Source stock = gp::stock_source({});
    gp::Source none{"the tracks table", {}, {}};

    gp::Fallback f = gp::fallback(52, {table, base, stock});
    expect(f.replace && f.stat_id == 0 && f.source == table.name, "stat id 52 with no row: replaced, decided by the tracks table");
    expect(has(f.line, "stat id 52 has no tracks row") && has(f.line, "melbourne (stat id 0)") && has(f.line, "decided by the tracks table (read at the window stage)"),
           "the line names the stale id, the stock track and the source");

    f = gp::fallback(35, {table, base, stock});
    expect(f.replace && f.stat_id == 0 && has(f.line, "dormant row bahrain24"), "a dormant row (bahrain24, stat id 35) is replaced and named");

    f = gp::fallback(3, {table, base, stock});
    expect(!f.replace && f.line.empty(), "a stock track with a live row is left alone");
    f = gp::fallback(45, {table, base, stock});
    expect(!f.replace && f.line.empty(), "an addon track with a live row is left alone");
    f = gp::fallback(-1, {table, base, stock});
    expect(!f.replace && f.line.empty(), "no weekend track (byte 0xff) changes nothing");
    f = gp::fallback(0xff, {table, base, stock});
    expect(!f.replace && f.line.empty(), "stat id 0xff changes nothing");

    f = gp::fallback(52, {none, base, stock});
    expect(f.replace && f.stat_id == 0 && f.source == "the track base", "an unreadable tracks table falls through to the track base");
    f = gp::fallback(52, {none, gp::Source{"the track base", {}, {}}, stock});
    expect(f.replace && f.stat_id == 0 && f.source == stock.name, "no table and no base: the shipping table decides");
    f = gp::fallback(52, {none, gp::Source{"the track base", {}, {}}, gp::Source{"the shipping tracks table and the track records", {}, {}}});
    expect(!f.replace && f.line.empty(), "no source with rows changes nothing and says nothing");

    gp::Source no_stock{"the tracks table", {{2, "shanghai"}, {3, "bahrain"}}, {}};
    f = gp::fallback(52, {no_stock, base});
    expect(f.replace && f.stat_id == 0 && has(f.line, "melbourne's stat id from the track base"), "the deciding source lacks melbourne: its id comes from a later source and the line says so");
    f = gp::fallback(52, {no_stock});
    expect(!f.replace && f.stat_id == -1 && has(f.line, "no source has a melbourne row"), "no melbourne anywhere: nothing written, the line says why");

    gp::Source moved{"the tracks table", {{7, "melbourne"}, {8, "shanghai"}}, {}};
    f = gp::fallback(52, {moved});
    expect(f.replace && f.stat_id == 7, "the stock track's stat id comes from the source, not a constant");
    gp::Source dormant_stock{"the tracks table", {{0, "melbourne"}, {2, "shanghai"}}, {0}};
    f = gp::fallback(52, {dormant_stock});
    expect(!f.replace && has(f.line, "no source has a melbourne row"), "a dormant melbourne row is not a target");

    // No false replacement: a live shipping row is raceable whichever source decides, even a partial one.
    gp::Source addons_only{"the track base", {{45, "adelaide"}, {57, "nycgp"}}, {}};
    f = gp::fallback(13, {none, addons_only, stock});
    expect(!f.replace && f.line.empty(), "a stock live track (suzuka, 13) with an addon-only base is untouched");
    f = gp::fallback(13, {gp::Source{"the tracks table", {{45, "adelaide"}}, {}}});
    expect(!f.replace && f.line.empty(), "a stock live track (suzuka) with a table that lacks it is untouched");
    f = gp::fallback(57, {none, addons_only, gp::stock_source({{57, "mod.nycgp/nycgp"}})});
    expect(!f.replace && f.line.empty(), "an addon track that is on (nycgp, 57) is untouched");
    f = gp::fallback(28, {none, addons_only, stock});
    expect(f.replace && f.stat_id == 0 && has(f.line, "dormant row portimao"), "a dormant stock row (portimao, 28) missing from an addon-only base is replaced and named");
    f = gp::fallback(35, {gp::Source{"the tracks table", {{0, "melbourne"}, {35, "mod.bahrain24/bahrain24"}}, {}}});
    expect(!f.replace && f.line.empty(), "a dormant slot filled by an addon's live row is untouched");
    f = gp::fallback(99, {none, addons_only, stock});
    expect(f.replace && f.stat_id == 0 && has(f.line, "stat id 99 has no tracks row"), "an unknown id (99) is replaced");

    // The shipping table: all 43 rows, 9 of them dormant, melbourne at 0; a registry record on a dormant slot makes
    // the slot live.
    expect(stock.rows.size() == 43 && stock.dormant.size() == 9 && stock.rows.at(0) == "melbourne" && stock.dormant.count(35), "stock_source: 43 rows, 9 dormant, melbourne 0, bahrain24 dormant");
    f = gp::fallback(37, {none, addons_only, stock});
    expect(!f.replace && f.line.empty(), "an unnamed shipping row (37) counts as live stock, as the track base lists it");
    gp::Source filled = gp::stock_source({{35, "mod.bahrain24/bahrain24"}, {57, "mod.nycgp/nycgp"}});
    expect(filled.rows.at(35) == "mod.bahrain24/bahrain24" && !filled.dormant.count(35) && filled.rows.at(57) == "mod.nycgp/nycgp", "stock_source: a track record fills its slot and adds its own row");

    // Replay of the three 2026-11-11 crash bundles from their logged facts: the tracks table read failed in all
    // three ("not readable yet"), so the table source is empty and the later sources decide.
    // yamasoorni 1570104821623685200: weekend stat id 52 (track.qiddiya, off); the base was read (nycgp 57 made
    // for 1.5) so it holds the stock tracks and nycgp; the registry has the nycgp record.
    gp::Source yam_base{"the track base", {{0, "melbourne"}, {3, "bahrain"}, {57, "nycgp"}}, {}};
    f = gp::fallback(52, {none, yam_base, gp::stock_source({{57, "mod.nycgp/nycgp"}})});
    expect(f.replace && f.stat_id == 0 && f.source == "the track base", "yamasoorni (id 52, qiddiya off): replaced by melbourne, decided by the track base");
    f = gp::fallback(52, {none, gp::Source{"the track base", {}, {}}, gp::stock_source({{57, "mod.nycgp/nycgp"}})});
    expect(f.replace && f.stat_id == 0 && f.source == stock.name, "yamasoorni with an empty base: the shipping table still replaces 52");
    // smallwingedbird 1570106123766341633: weekend stat id 83 (mod.lemans, off); no track records, base empty.
    f = gp::fallback(83, {none, gp::Source{"the track base", {}, {}}, gp::stock_source({})});
    expect(f.replace && f.stat_id == 0 && has(f.line, "stat id 83 has no tracks row"), "smallwingedbird (id 83, le mans off): replaced by melbourne from the shipping table");
    // happymanster 1570106767453585459: weekend stat id 35 (bahrain24, a dormant slot filled by another profile's
    // imported track); no track addons in this profile, base empty.
    f = gp::fallback(35, {none, gp::Source{"the track base", {}, {}}, gp::stock_source({})});
    expect(f.replace && f.stat_id == 0 && has(f.line, "dormant row bahrain24"), "happymanster (id 35, bahrain24 slot): replaced by melbourne as a dormant row");

    std::printf("%s\n", failed ? "FAILED" : "all passed");
    return failed ? 1 : 0;
}
