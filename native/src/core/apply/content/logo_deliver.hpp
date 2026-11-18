#pragma once

#include "erp_file.hpp"
#include "logo_cache.hpp"
#include "overlay_copies.hpp"

#include <functional>
#include <map>
#include <set>
#include <string>
#include <utility>
#include <vector>

// How the archives of a team logo build reach the game. A stock archive is redirected to its built copy. A launcher
// overlay archive (swofty/assets/resources/N.erp) is never redirected: the resource overlay registers its built copy
// under the copy's own path in place of the overlay. The game reads an array's image from the first archive that
// holds it and its view from the first that holds one, so an array grows in every archive that carries it or in none.

namespace swofty_core::apply::content::logo_deliver {

inline std::set<std::string> arrays_of(const erpfile::Changes &changes, const std::string &array_uri)
{
    std::set<std::string> out;
    for (auto &[name, frags] : changes) {
        if (name.compare(0, array_uri.size(), array_uri) != 0)
            continue;
        std::string rest = name.substr(array_uri.size());
        for (const char *end : {".texarray.image", ".texarray"}) {
            size_t n = std::char_traits<char>::length(end);
            if (rest.size() > n && rest.compare(rest.size() - n, n, end) == 0) {
                out.insert(rest.substr(0, rest.size() - n));
                break;
            }
        }
    }
    return out;
}

struct Failed {
    std::string rel, why;
};

// One build written to disk: the copies that were written, the archives whose copy could not be written, and the
// arrays each changed archive carries. A build read back from the cache does not know its arrays.
struct Attempt {
    logo_cache::Routes routes;
    std::vector<Failed> unwritten;
    std::map<std::string, std::set<std::string>> arrays;
    bool cached = false;
};

struct Sink {
    std::function<bool(const std::string &rel)> overlay;
    std::function<bool(const std::string &rel, const std::string &file, std::string &why)> route;
    std::function<void()> unroute;
    std::function<bool(const logo_cache::Routes &copies, std::string &why)> offer;
};

struct Outcome {
    Attempt last;
    std::vector<Failed> failed;
    std::set<std::string> dropped;
    std::vector<std::string> lines;
};

// Delivers a build. When an archive that carries an array cannot be written, redirected or registered, every redirect
// is withdrawn and `make` builds again without that array, so it stays at its stock size in every archive. The
// overlay copies are offered last and in one call: after they are accepted nothing can fail.
inline Outcome deliver(Attempt first, const std::function<Attempt(const std::set<std::string> &dropped)> &make, const Sink &sink)
{
    Outcome out;
    Attempt a = std::move(first);
    for (;;) {
        std::vector<Failed> failed = a.unwritten;
        auto blamed = [&] {
            std::map<std::string, Failed> arrays;
            for (auto &f : failed) {
                auto it = a.arrays.find(f.rel);
                if (it != a.arrays.end())
                    for (auto &array : it->second)
                        arrays.emplace(array, f);
            }
            return arrays;
        };
        auto again = [&] { return a.cached ? !failed.empty() : !blamed().empty(); };
        bool routed = false;
        if (!again()) {
            routed = true;
            logo_cache::Routes copies;
            for (auto &[rel, file] : a.routes) {
                std::string why;
                if (sink.overlay(rel))
                    copies.emplace_back(rel, file);
                else if (!sink.route(rel, file, why))
                    failed.push_back({rel, why});
            }
            std::string why;
            if (!again() && !sink.offer(copies, why))
                for (auto &c : copies)
                    failed.push_back({c.first, why});
        }
        if (!again()) {
            out.last = std::move(a);
            out.failed = std::move(failed);
            return out;
        }
        if (routed)
            sink.unroute();
        size_t before = out.dropped.size();
        if (a.cached)
            out.lines.push_back("team.logo: the cached archives could not all be delivered (" + failed[0].rel + ": " + failed[0].why +
                                "); built again");
        else
            for (auto &[array, f] : blamed()) {
                out.dropped.insert(array);
                out.lines.push_back("team.logo: " + array + " is left at its stock size in every archive this boot: " + f.rel +
                                    " could not be delivered (" + f.why + "), and the game must read the array's image and view at the same size");
            }
        if (!a.cached && out.dropped.size() == before) {
            out.failed = std::move(failed);
            return out;
        }
        a = make(out.dropped);
    }
}

}
