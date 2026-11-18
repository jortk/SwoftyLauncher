#define ZSTD_STATIC_LINKING_ONLY
#include "../../src/core/apply/content/logo_build.hpp"
#include "../../src/core/apply/content/logo_deliver.hpp"
#include "../../src/core/apply/files.hpp"

#include <cstdio>
#include <filesystem>
#include <fstream>
#include <sstream>

namespace content = swofty_core::apply::content;
namespace lb = content::logo_build;
namespace ld = content::logo_deliver;
namespace la = content::logo_array;
namespace ef = content::erpfile;
namespace files = swofty_core::apply::files;
namespace fs = std::filesystem;

static int failures = 0, checks = 0;

static void check(bool ok, const std::string &what)
{
    checks++;
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static std::string slurp(const fs::path &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream s;
    s << f.rdbuf();
    return s.str();
}

static std::shared_ptr<ef::Source> open_bytes(std::shared_ptr<std::string> bytes)
{
    auto src = std::make_shared<ef::Source>();
    src->size = bytes->size();
    src->read = [bytes](uint64_t off, size_t n, char *out) {
        if (off + n > bytes->size())
            return false;
        std::memcpy(out, bytes->data() + off, n);
        return true;
    };
    return src;
}

static const std::string ARRAY = "teams.driver_select_team_logos";
static const std::string IMAGE = std::string(lb::ARRAY_URI) + ARRAY + ".texarray.image", VIEW = std::string(lb::ARRAY_URI) + ARRAY + ".texarray";
static const std::string DB = lb::DATABASES, FLOW = "2025_asset_groups/ui_package/flow_a.erp", NONE = "2025_asset_groups/ui_package/flow_none.erp",
                         SCREEN = "2025_asset_groups/ui_package/screens/b.erp", OVERLAY = "swofty/assets/resources/0.erp";
static const std::string ADDON = "swofty.logos", COPIES = "swofty/addons/swofty.logos/";

struct Counts {
    int image = -1, view = -1;
};

// One boot: the game folder, the copies the logo build wrote, the file route and the resource overlay.
struct Boot {
    std::map<std::string, std::shared_ptr<std::string>> game, written;
    std::map<std::string, std::string> routes;
    std::set<std::string> no_route, no_write;
    bool with_overlay = true;
    files::Router router;
    ld::OverlayCopies copies;
    std::string registered;
    std::vector<lb::Want> wants;
    int builds = 0;
    lb::Result last;

    Boot(const fs::path &root, bool overlay) : with_overlay(overlay)
    {
        for (auto &rel : {DB, FLOW, NONE, SCREEN, OVERLAY})
            game[rel] = std::make_shared<std::string>(slurp(root / rel));
        files::Sink s;
        s.exists = [this](const std::string &target) { return written.count(target.substr(target.rfind('/') + 1)) != 0; };
        s.in_game = [this](const std::string &p) { return game.count(p) != 0; };
        s.route = [this](const std::string &p, const files::Claim *w, std::string &why) {
            if (w && no_route.count(p))
                return why = "the core's file route is not available on this build", false;
            if (w)
                routes[p] = w->file;
            else
                routes.erase(p);
            return true;
        };
        router.set_sink(s);
        if (with_overlay)
            registered = OVERLAY;
    }

    lb::Env env()
    {
        lb::Env e;
        e.list = [] { return std::vector<std::string>{DB, FLOW, NONE, SCREEN}; };
        e.open = [this](const std::string &rel) { return game.count(rel) ? open_bytes(game[rel]) : nullptr; };
        e.overlays = [this](const std::string &rel) {
            return with_overlay && rel == FLOW ? std::vector<std::string>{OVERLAY} : std::vector<std::string>{};
        };
        return e;
    }

    static std::string file_of(const std::string &rel)
    {
        std::string flat = rel;
        std::replace(flat.begin(), flat.end(), '/', '_');
        return "logos_" + flat;
    }

    ld::Attempt make(const std::set<std::string> &dropped)
    {
        builds++;
        std::vector<lb::Want> kept;
        for (auto &w : wants)
            if (!dropped.count(w.array))
                kept.push_back(w);
        last = {};
        lb::build(env(), kept, last);
        ld::Attempt a;
        for (auto &[rel, changes] : last.changes) {
            a.arrays[rel] = ld::arrays_of(changes, lb::ARRAY_URI);
            if (no_write.count(rel)) {
                a.unwritten.push_back({rel, "it could not be rewritten"});
                continue;
            }
            auto src = open_bytes(game[rel]);
            ef::Archive arc;
            std::string why, bytes;
            ef::open(*src, arc, why);
            ef::rewrite(*src, arc, changes, [&](const char *p, size_t n) { bytes.append(p, n); return true; }, why);
            written[file_of(rel)] = std::make_shared<std::string>(std::move(bytes));
            a.routes.emplace_back(rel, file_of(rel));
        }
        return a;
    }

    ld::Sink sink()
    {
        ld::Sink s;
        s.overlay = [this](const std::string &rel) { return with_overlay && rel == OVERLAY; };
        s.route = [this](const std::string &rel, const std::string &file, std::string &why) {
            auto o = router.redirect(rel, ADDON, file, "team.logo", "logos", "boot");
            why = o.line;
            return !o.refused();
        };
        s.unroute = [this] { router.withdraw(ADDON, "team.logo", "logos"); };
        s.offer = [this](const logo_routes &list, std::string &why) {
            std::map<std::string, ld::Copy> held;
            for (auto &[rel, file] : list)
                held[ld::overlay_key(rel)] = {COPIES + file, written[file]->size()};
            if (copies.offer(held))
                return true;
            why = "the resource overlay was handed to the game before the logo build ended";
            return false;
        };
        return s;
    }

    using logo_routes = content::logo_cache::Routes;

    ld::Outcome deliver(ld::Attempt first)
    {
        return ld::deliver(std::move(first), [this](const std::set<std::string> &dropped) { return make(dropped); }, sink());
    }

    ld::Outcome deliver() { return deliver(make({})); }

    // The resource overlay hands its archives to the manifest.
    void register_overlay()
    {
        auto taken = copies.take();
        auto it = taken.find(OVERLAY);
        if (with_overlay && it != taken.end())
            registered = it->second.path;
    }

    std::shared_ptr<std::string> served(const std::string &path)
    {
        if (path.rfind(COPIES, 0) == 0)
            return written[path.substr(COPIES.size())];
        auto r = routes.find(path);
        return r != routes.end() ? written[r->second] : game[path];
    }

    // What the game reads for a group: the image of the first archive that holds one and the view of the first that
    // holds one, the registered overlay before the group's own archive.
    Counts reads(const std::string &rel)
    {
        Counts c;
        std::vector<std::string> order;
        if (!registered.empty() && rel == FLOW)
            order.push_back(registered);
        order.push_back(rel);
        for (auto &path : order) {
            auto bytes = served(path);
            auto src = open_bytes(bytes);
            ef::Archive arc;
            std::string why, b;
            if (!ef::open(*src, arc, why))
                continue;
            if (auto *e = arc.find(IMAGE); e && c.image < 0 && ef::fragment(*src, arc, e->frags[0], b, why))
                c.image = int(la::le32(b, la::HEAD_COUNT));
            if (auto *e = arc.find(VIEW); e && c.view < 0 && ef::fragment(*src, arc, e->frags[0], b, why))
                c.view = int(la::le32(b, la::VIEW_COUNT));
        }
        return c;
    }
};

static size_t lines_with(const std::vector<std::string> &lines, std::initializer_list<std::string> parts)
{
    return size_t(std::count_if(lines.begin(), lines.end(), [&](const std::string &l) {
        return std::all_of(parts.begin(), parts.end(), [&](const std::string &p) { return l.find(p) != std::string::npos; });
    }));
}

static std::string show(Counts c)
{
    return "image " + std::to_string(c.image) + ", view " + std::to_string(c.view);
}

int main(int argc, char **argv)
{
    fs::path dir = argc > 1 ? argv[1] : "content/bc7";
    fs::path root = dir / "game";
    std::string raw = slurp(dir / "shape64.rgba");
    const char *px = raw.data() + raw.find('\n') + 1;
    std::vector<lb::Want> wants(2);
    wants[0] = {ARRAY, "bdy_s5_team_ferrari", "", std::string(px, 64 * 64 * 4), 64, 64};
    wants[1].array = ARRAY;
    wants[1].key = "bdy_s5_team_lotus";
    wants[1].donor = "b";

    {
        Boot stock(root, true);
        stock.register_overlay();
        Counts c = stock.reads(FLOW);
        check(c.image == 4 && c.view == 3, "the fixture: an overlay that holds the array's image only (4 slices) over an archive whose view says 3 (" +
                                               show(c) + ")");
    }

    {
        // The boot that crashed: every copy is redirected one by one and the overlay's copy is refused.
        Boot old(root, true);
        old.wants = wants;
        ld::Attempt a = old.make({});
        bool overlay_refused = false;
        for (auto &[rel, file] : a.routes) {
            auto o = old.router.redirect(rel, ADDON, file, "team.logo", "logos", "boot");
            if (rel == OVERLAY)
                overlay_refused = o.refused() && o.line.find("is inside swofty/") != std::string::npos;
        }
        old.register_overlay();
        Counts c = old.reads(FLOW);
        check(a.arrays[OVERLAY].count(ARRAY) && a.arrays[FLOW].count(ARRAY) && overlay_refused,
              "the build grows the array in the overlay too and the file route refuses a path inside swofty/");
        check(c.image == 4 && c.view == 6, "redirect by redirect, the game reads the overlay's stale image under the grown view (" + show(c) + ")");
    }

    {
        Boot boot(root, true);
        boot.wants = wants;
        ld::Outcome done = boot.deliver();
        boot.register_overlay();
        Counts flow = boot.reads(FLOW), screen = boot.reads(SCREEN);
        check(done.dropped.empty() && done.failed.empty() && boot.builds == 1 && done.last.routes.size() == 4,
              "an image-only overlay layer: all four copies are delivered in one build");
        check(boot.registered == COPIES + Boot::file_of(OVERLAY) && !boot.routes.count(OVERLAY),
              "the overlay's copy is registered in the overlay's place and is not a file route");
        check(flow.image == 6 && flow.view == 6, "the game reads the grown image and the grown view (" + show(flow) + ")");
        check(screen.image == 6 && screen.view == 6, "an archive with no overlay reads its own grown image and view (" + show(screen) + ")");
        check(boot.copies.offer({{OVERLAY, {COPIES + Boot::file_of(OVERLAY), boot.written[Boot::file_of(OVERLAY)]->size()}}}) &&
                  !boot.copies.offer({{OVERLAY, {COPIES + "logos_other.erp", 1}}}) && !boot.copies.offer({}),
              "after the overlay is registered only the copies it took are accepted");
    }

    {
        // The resource overlay is registered before the logo build ends: the copy can no longer be used.
        Boot late(root, true);
        late.wants = wants;
        late.register_overlay();
        ld::Outcome done = late.deliver();
        Counts flow = late.reads(FLOW), screen = late.reads(SCREEN);
        check(done.dropped == std::set<std::string>{ARRAY} && late.builds == 2, "an overlay copy that cannot be registered drops the array and builds again");
        check(late.routes.empty() && late.router.size() == 0 && done.last.routes.empty() && late.registered == OVERLAY,
              "no archive is redirected and the stock overlay stays registered");
        check(flow.image == 4 && flow.view == 3 && screen.image == 3 && screen.view == 3,
              "every archive reads the array as it does with no team logo (" + show(flow) + "; " + show(screen) + ")");
        check(lines_with(done.lines, {"team.logo: " + ARRAY + " is left at its stock size in every archive this boot", OVERLAY,
                                      "handed to the game before the logo build ended"}) == 1 && done.lines.size() == 1,
              "one line names the array, the archive and the reason");
        check(late.last.refused == 0 && late.last.built == 0 && late.last.changes.empty(), "the second build has nothing left to change");
    }

    {
        Boot blocked(root, true);
        blocked.wants = wants;
        blocked.no_route.insert(SCREEN);
        ld::Outcome done = blocked.deliver();
        blocked.register_overlay();
        Counts flow = blocked.reads(FLOW), screen = blocked.reads(SCREEN);
        check(done.dropped == std::set<std::string>{ARRAY} && blocked.routes.empty() && blocked.router.size() == 0 && blocked.registered == OVERLAY,
              "a stock archive that cannot be redirected drops the array in every archive, the overlay included");
        check(flow.image == 4 && flow.view == 3 && screen.image == 3 && screen.view == 3,
              "every archive reads the stock array (" + show(flow) + "; " + show(screen) + ")");
        check(lines_with(done.lines, {ARRAY, SCREEN, "not available on this build"}) == 1, "the line names the archive that could not be redirected");
    }

    {
        Boot unwritten(root, true);
        unwritten.wants = wants;
        unwritten.no_write.insert(OVERLAY);
        ld::Outcome done = unwritten.deliver();
        unwritten.register_overlay();
        Counts flow = unwritten.reads(FLOW);
        check(done.dropped == std::set<std::string>{ARRAY} && unwritten.routes.empty() && unwritten.registered == OVERLAY && flow.image == 4 &&
                  flow.view == 3 && lines_with(done.lines, {ARRAY, OVERLAY, "could not be rewritten"}) == 1,
              "an overlay copy that cannot be written drops the array before anything is redirected");
    }

    {
        Boot plain(root, false);
        plain.wants = wants;
        lb::Result direct;
        lb::build(plain.env(), wants, direct);
        ld::Outcome done = plain.deliver();
        plain.register_overlay();
        bool same = done.last.routes.size() == direct.changes.size() && plain.last.lines == direct.lines && plain.builds == 1 && done.lines.empty() &&
                    done.dropped.empty() && done.failed.empty();
        for (auto &[rel, changes] : direct.changes) {
            auto src = open_bytes(plain.game[rel]);
            ef::Archive arc;
            std::string why, bytes;
            ef::open(*src, arc, why);
            ef::rewrite(*src, arc, changes, [&](const char *p, size_t n) { bytes.append(p, n); return true; }, why);
            same = same && plain.routes.count(rel) && *plain.written[plain.routes[rel]] == bytes;
        }
        Counts flow = plain.reads(FLOW);
        check(same && plain.routes.size() == 3 && plain.registered.empty(),
              "with no overlay the same three copies are built once, byte for byte, and each is redirected");
        check(flow.image == 5 && flow.view == 5, "with no overlay the game reads the grown image and view (" + show(flow) + ")");
    }

    {
        Boot cached(root, true);
        cached.wants = wants;
        ld::Attempt first = cached.make({});
        first.arrays.clear();
        first.cached = true;
        cached.builds = 0;
        ld::Outcome done = cached.deliver(first);
        cached.register_overlay();
        Counts flow = cached.reads(FLOW);
        check(cached.builds == 0 && done.last.cached && done.failed.empty() && flow.image == 6 && flow.view == 6,
              "cached copies are delivered without a build, the overlay's copy included");

        Boot stale(root, true);
        stale.wants = wants;
        first = stale.make({});
        first.arrays.clear();
        first.cached = true;
        stale.builds = 0;
        stale.register_overlay();
        done = stale.deliver(first);
        flow = stale.reads(FLOW);
        check(stale.builds == 2 && !done.last.cached && done.dropped == std::set<std::string>{ARRAY} && stale.routes.empty() && flow.image == 4 &&
                  flow.view == 3 && lines_with(done.lines, {"cached archives could not all be delivered"}) == 1,
              "cached copies that cannot all be delivered are built again, and the array is then dropped");
    }

    {
        // Two arrays in synthetic archives: only the array of the archive that fails is dropped.
        std::set<std::string> routed;
        int makes = 0;
        bool offered = false;
        auto make = [&](const std::set<std::string> &dropped) {
            makes++;
            ld::Attempt a;
            a.routes = {{"db.erp", "f_db"}};
            a.arrays["db.erp"] = {};
            if (!dropped.count("x")) {
                a.routes.emplace_back("p.erp", "f_p");
                a.routes.emplace_back("shared.erp", "f_shared");
                a.arrays["p.erp"] = {"x"};
                a.arrays["shared.erp"] = {"x", "y"};
            } else {
                a.routes.emplace_back("shared.erp", "f_shared");
                a.arrays["shared.erp"] = {"y"};
            }
            a.routes.emplace_back("q.erp", "f_q");
            a.arrays["q.erp"] = {"y"};
            return a;
        };
        ld::Sink sink;
        sink.overlay = [](const std::string &) { return false; };
        sink.route = [&](const std::string &rel, const std::string &, std::string &why) {
            if (rel == "p.erp" || rel == "db.erp")
                return why = "no route", false;
            routed.insert(rel);
            return true;
        };
        sink.unroute = [&] { routed.clear(); };
        sink.offer = [&](const content::logo_cache::Routes &list, std::string &) { return offered = list.empty(); };
        ld::Outcome done = ld::deliver(make({}), make, sink);
        check(done.dropped == std::set<std::string>{"x"} && makes == 2 && routed == std::set<std::string>{"shared.erp", "q.erp"} && offered,
              "only the array of the archive that failed is dropped; the other array is delivered");
        check(done.failed.size() == 1 && done.failed[0].rel == "db.erp" && done.last.routes.size() == 3,
              "an archive that carries no array and cannot be redirected fails alone, without another build");
    }

    {
        std::set<std::string> a = ld::arrays_of({{IMAGE, {}}, {VIEW, {}}, {lb::OVERRIDES, {}},
                                                 {std::string(lb::ARRAY_URI) + "team_logos.x.texarray", {}}}, lb::ARRAY_URI);
        check(a == std::set<std::string>{ARRAY, "team_logos.x"} && ld::overlay_key("\\Swofty\\Assets/Resources/0.ERP") == OVERLAY,
              "an archive's arrays are the images and views it changes; an overlay is keyed by its lower-case game path");
    }

    std::printf("logo deliver: %d check(s), %d failure(s)\n", checks, failures);
    return failures ? 1 : 0;
}
