#pragma once

#include "erp_file.hpp"
#include "logo_array.hpp"
#include "uidb.hpp"

#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <vector>

// The archives a set of team logos changes. Each logo is a slice for one stock texture array (an encoded image, or a
// copy of the slice a stock key already has) under a key in overrides.uidb. The result names, for every archive in
// the game's ui_package folder that carries a changed array and for databases.erp, the fragments to replace, and the
// archives whose contents it read: databases.erp and every archive that carries a wanted array. The archive list may
// name the boot archive too (see array_layers): the game draws from its copy of an array when it has one, so that
// copy gets the same slices at the same indices. A copy there that is an image with no view of its own is built with
// the view of a ui_package copy, and one that holds fewer slices than a ui_package copy of the same slice size takes
// that copy's slices for the keys it lacks.

namespace swofty_core::apply::content::logo_build {

inline constexpr const char *UI_FOLDER = "2025_asset_groups/ui_package/";
inline constexpr const char *DATABASES = "2025_asset_groups/ui_package/databases.erp";
inline constexpr const char *BOOT_ARCHIVE = "2025_asset_groups/vfx_package/common.erp";
inline constexpr const char *OVERRIDES = "eaid://ui_package/databases/overrides.uidb";
inline constexpr const char *ARRAY_URI = "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.";
inline constexpr const char *TEXTURE_PATH = "ui_package/art/textures/swofty/team_logos/";

struct Want {
    std::string array, key, donor;
    std::string rgba;
    uint32_t width = 0, height = 0;
    bool fallback = false;
};

struct Env {
    std::function<std::vector<std::string>()> list;
    std::function<std::shared_ptr<erpfile::Source>(const std::string &rel)> open;
    std::function<std::vector<std::string>(const std::string &rel)> overlays;
};

struct Result {
    std::map<std::string, erpfile::Changes> changes;
    std::set<std::string> read;
    std::vector<std::string> lines;
    int built = 0, refused = 0;
};

struct Refusals {
    std::map<std::string, std::pair<int, std::string>> by;

    void add(const std::string &reason, const std::string &example, int n = 1)
    {
        auto &r = by[reason];
        if (!r.first)
            r.second = example;
        r.first += n;
    }

    void flush(std::vector<std::string> &lines) const
    {
        for (auto &[reason, r] : by)
            lines.push_back("team.logo: " + std::to_string(r.first) + " slot(s) refused: " + reason + " (first " + r.second + ")");
    }
};

inline std::string list_path(const std::string &array)
{
    std::string path = "overrides.Contents.", rest = array;
    size_t dot;
    while ((dot = rest.find('.')) != std::string::npos) {
        path += rest.substr(0, dot) + ".Contents.";
        rest = rest.substr(dot + 1);
    }
    return path + rest + ".Contents";
}

inline std::vector<std::string> array_layers(const Env &env, const std::string &rel)
{
    std::vector<std::string> layers{rel}, out;
    if (env.overlays)
        for (auto &o : env.overlays(rel))
            if (std::find(layers.begin(), layers.end(), o) == layers.end())
                layers.push_back(o);
    size_t prefix = std::char_traits<char>::length(ARRAY_URI);
    for (auto &l : layers) {
        auto src = env.open(l);
        erpfile::Archive arc;
        std::string why;
        if (!src || !erpfile::open(*src, arc, why))
            continue;
        if (std::any_of(arc.entries.begin(), arc.entries.end(), [&](const erpfile::Entry &e) { return e.name.compare(0, prefix, ARRAY_URI) == 0; }))
            out.push_back(l);
    }
    return out;
}

inline bool build(const Env &env, const std::vector<Want> &wants, Result &out)
{
    using erpfile::Archive;
    struct Open {
        std::shared_ptr<erpfile::Source> src;
        Archive arc;
    };
    std::map<std::string, Open> opened;
    std::string why;
    Refusals refusals;
    struct Flush {
        Refusals &r;
        Result &out;
        ~Flush() { r.flush(out.lines); }
    } flush{refusals, out};
    auto refuse = [&](const std::string &reason, const std::string &example, int n = 1) {
        out.refused += n;
        refusals.add(reason, example, n);
    };
    auto get = [&](const std::string &rel) -> Open * {
        auto it = opened.find(rel);
        if (it != opened.end())
            return &it->second;
        auto src = env.open(rel);
        Open o;
        if (!src || !erpfile::open(*src, o.arc, why))
            return nullptr;
        o.src = src;
        return &opened.emplace(rel, std::move(o)).first->second;
    };
    auto layers_of = [&](const std::string &rel) {
        std::vector<std::string> layers{rel};
        if (env.overlays)
            for (auto &o : env.overlays(rel))
                if (std::find(layers.begin(), layers.end(), o) == layers.end())
                    layers.push_back(o);
        return layers;
    };
    struct Found {
        Open *open = nullptr;
        const erpfile::Entry *entry = nullptr;
    };
    auto top = [&](const std::vector<std::string> &layers, const std::string &name) {
        Found f;
        for (auto &l : layers) {
            Open *o = get(l);
            const erpfile::Entry *e = o ? o->arc.find(name) : nullptr;
            if (e)
                f = {o, e};
        }
        return f;
    };
    auto carrying = [&](const std::vector<std::string> &layers, const std::string &name) {
        std::vector<std::string> out_layers;
        for (auto &l : layers) {
            Open *o = get(l);
            if (o && o->arc.find(name))
                out_layers.push_back(l);
        }
        return out_layers;
    };
    std::vector<std::string> db_layers = layers_of(DATABASES);
    Open *db = get(DATABASES);
    if (db)
        out.read.insert(DATABASES);
    Found uf = db ? top(db_layers, OVERRIDES) : Found{};
    const erpfile::Entry *ue = uf.entry;
    std::string uidb_bytes;
    if (!ue || ue->frags.empty() || !erpfile::fragment(*uf.open->src, uf.open->arc, ue->frags[0], uidb_bytes, why)) {
        refuse("overrides.uidb is not in " + std::string(DATABASES) + (why.empty() ? "" : " (" + why + ")"),
               wants.empty() ? "" : wants[0].array + "/" + wants[0].key, int(wants.size()));
        return false;
    }
    uidb::Pkg pkg;
    if (!pkg.read(uidb_bytes)) {
        refuse("overrides.uidb does not read: " + pkg.why, wants.empty() ? "" : wants[0].array + "/" + wants[0].key, int(wants.size()));
        return false;
    }
    std::vector<std::string> files = env.list();
    std::map<std::string, std::vector<const Want *>> by_array;
    for (auto &w : wants)
        by_array[w.array].push_back(&w);
    bool uidb_changed = false;
    std::set<std::string> viewed;
    for (auto &[array, list] : by_array) {
        uidb::Node *node = pkg.find(list_path(array));
        if (!node || node->kind != uidb::Node::array) {
            refuse("overrides.uidb has no list " + array, array + "/" + list[0]->key, int(list.size()));
            continue;
        }
        std::vector<std::string> names;
        for (auto &e : node->list)
            names.push_back(uidb::Pkg::text_of(e, "Name"));
        std::string image = ARRAY_URI + array + ".texarray.image", view = ARRAY_URI + array + ".texarray";
        struct Copy {
            logo_array::Stock stock;
            logo_array::Shape shape;
            std::vector<std::string> carriers;
            std::vector<logo_array::Slice> add;
            bool outside = false;
            uint32_t had = 0;
        };
        std::vector<Copy> copies;
        std::map<std::string, std::vector<std::string>> skipped;
        std::map<std::string, std::string> repaired;
        std::map<std::string, std::vector<std::string>> image_in, view_in;
        auto where = [](const std::vector<std::string> &rels) {
            return rels[0] + (rels.size() > 1 ? " and " + std::to_string(rels.size() - 1) + " more archive(s)" : "");
        };
        for (auto &rel : files) {
            if (!get(rel))
                continue;
            std::vector<std::string> layers = layers_of(rel);
            Found fi = top(layers, image), fv = top(layers, view);
            if (!fi.entry && !fv.entry)
                continue;
            for (auto &l : carrying(layers, image))
                image_in[rel].push_back(l), out.read.insert(l);
            for (auto &l : carrying(layers, view))
                view_in[rel].push_back(l), out.read.insert(l);
            out.read.insert(rel);
            const erpfile::Entry *ei = fi.entry, *ev = fv.entry;
            bool outside = rel.compare(0, std::char_traits<char>::length(UI_FOLDER), UI_FOLDER) != 0;
            bool lent = ei && !ev && !copies.empty() && outside;
            logo_array::Stock got;
            if (!ei || (!ev && !lent) || ei->frags.size() != 2 || (ev && ev->frags.size() != 1) ||
                !erpfile::fragment(*fi.open->src, fi.open->arc, ei->frags[0], got.head, why) ||
                !erpfile::fragment(*fi.open->src, fi.open->arc, ei->frags[1], got.data, why) ||
                (ev && !erpfile::fragment(*fv.open->src, fv.open->arc, ev->frags[0], got.view, why))) {
                skipped["a layout this build does not read"].push_back(rel);
                continue;
            }
            if (lent) {
                got.view = copies.front().stock.view;
                if (got.head.size() < logo_array::HEAD_SIZE || got.view.size() < logo_array::VIEW_COUNT + 4) {
                    skipped["a layout this build does not read"].push_back(rel);
                    continue;
                }
                uint32_t held = logo_array::le32(got.head, logo_array::HEAD_COUNT);
                logo_array::put32(got.view, logo_array::VIEW_COUNT, held);
                out.lines.push_back("team.logo: " + array + " in " + rel + " is an image of " + std::to_string(held) +
                                    " slice(s) with no view; it is built with the view of " + copies.front().carriers[0]);
            }
            uint32_t said = 0;
            if (logo_array::repair_view(got, said)) {
                repaired[rel] = got.view;
                if (viewed.insert(array).second)
                    out.lines.push_back("team.logo: " + array + " view said " + std::to_string(said) + " slices for " +
                                        std::to_string(logo_array::le32(got.head, logo_array::HEAD_COUNT)) + "; fixed");
            }
            auto same = std::find_if(copies.begin(), copies.end(), [&](const Copy &c) {
                return c.stock.head == got.head && c.stock.data == got.data && c.stock.view == got.view;
            });
            if (same != copies.end()) {
                same->carriers.push_back(rel);
                same->outside = same->outside && outside;
                continue;
            }
            logo_array::Shape shape;
            if (!logo_array::shape_of_images(got, shape, why)) {
                skipped[why].push_back(rel);
                continue;
            }
            copies.push_back({std::move(got), shape, {rel}, {}, outside, shape.count});
        }
        for (auto &[rel, bytes] : repaired)
            for (auto &l : view_in[rel])
                out.changes[l][view] = {bytes};
        size_t building = 0;
        for (auto &c : copies)
            building += c.carriers.size();
        for (auto &[reason, rels] : skipped)
            out.lines.push_back("team.logo: skipped " + array + " in " + where(rels) + ": " + reason + "; left as it is" +
                                (copies.empty() ? "" : ", the other " + std::to_string(building) + " archive(s) build"));
        if (copies.empty()) {
            std::string reason = skipped.empty()       ? array + " is in no archive of the ui_package folder"
                                 : skipped.size() == 1 ? array + ": " + skipped.begin()->first
                                                       : "no archive's copy of " + array + " can grow";
            refuse(reason, array + "/" + list[0]->key, int(list.size()));
            continue;
        }
        size_t keys = names.size();
        uint32_t most = 0;
        for (auto &c : copies)
            most = std::max(most, c.shape.count);
        if (copies.size() > 1) {
            std::string counts;
            for (auto &c : copies)
                counts += (counts.empty() ? "" : ", ") + std::to_string(c.shape.count);
            out.lines.push_back("team.logo: the " + std::to_string(building) + " archives that carry " + array + " hold " +
                                std::to_string(copies.size()) + " different copies (" + counts +
                                " slices); each keeps its slices and grows to the same key list");
        }
        if (names.size() < most) {
            size_t unnamed = most - names.size();
            std::string first = std::to_string(names.size()), failed;
            std::set<std::string> used(names.begin(), names.end());
            for (size_t n = names.size(); n < most && failed.empty(); n++) {
                std::string pad = "swofty_pad_" + std::to_string(n);
                while (!used.insert(pad).second)
                    pad += "_";
                if (!pkg.append_entry(*node, pad, std::string(TEXTURE_PATH) + pad + ".tif", failed))
                    break;
                names.push_back(pad);
            }
            if (!failed.empty()) {
                refuse(array + ": " + failed, array + "/" + list[0]->key, int(list.size()));
                continue;
            }
            uidb_changed = true;
            out.lines.push_back("team.logo: " + array + " holds " + std::to_string(most) + " slices for " + std::to_string(keys) +
                                " keys: other addons added slices without keys, so " + std::to_string(unnamed) +
                                " placeholder key(s) from slice " + first + " on name them and new logos land on their own keys");
        }
        std::map<std::pair<uint32_t, uint32_t>, std::string> blanks;
        auto fill = [&](Copy &c) {
            std::string in = copies.size() > 1 ? " in " + where(c.carriers) : "";
            const Copy *donor = nullptr;
            if (c.outside)
                for (auto &d : copies)
                    if (!d.outside && d.shape.width == c.shape.width && d.shape.height == c.shape.height && d.had > c.had &&
                        (!donor || d.had > donor->had))
                        donor = &d;
            if (donor) {
                out.lines.push_back("team.logo: " + array + in + " holds " + std::to_string(c.had) + " slice(s) for " +
                                    std::to_string(names.size()) + " key(s); slices " + std::to_string(c.had) + ".." +
                                    std::to_string(donor->had - 1) + " are copied from " + donor->carriers[0] +
                                    " so every key reads its own logo from this copy too");
                c.stock.data += donor->stock.data.substr(size_t(c.had) * c.shape.slice_bytes());
                logo_array::put32(c.stock.head, logo_array::HEAD_COUNT, uint32_t(names.size()));
                logo_array::put32(c.stock.view, logo_array::VIEW_COUNT, uint32_t(names.size()));
                c.shape.count = uint32_t(names.size());
                c.stock.names = names;
                return;
            }
            if (c.shape.count < keys)
                out.lines.push_back("team.logo: " + array + in + " lists " + std::to_string(keys) + " keys but its image holds " +
                                    std::to_string(c.shape.count) + " slice(s): another addon replaced the whole array, so " +
                                    std::to_string(keys - c.shape.count) + " key(s) from " + names[c.shape.count] +
                                    " on have no image of their own; padded with blank slices so new logos land on their own keys");
            else if (c.shape.count < names.size())
                out.lines.push_back("team.logo: " + array + in + " holds " + std::to_string(c.shape.count) + " slice(s), " +
                                    std::to_string(names.size() - c.shape.count) +
                                    " fewer than another archive's copy; padded with blank slices so new logos land on the same slice in every archive");
            if (c.shape.count < names.size()) {
                auto [b, fresh] = blanks.try_emplace({c.shape.width, c.shape.height});
                if (fresh)
                    b->second = bc7::encode_logo(reinterpret_cast<const uint8_t *>(std::string(c.shape.slice_bytes() * 4, '\0').data()),
                                                 int(c.shape.width), int(c.shape.height));
                logo_array::pad(c.stock, c.shape, b->second, uint32_t(names.size()));
                c.shape.count = uint32_t(names.size());
            }
            c.stock.names = names;
        };
        for (auto &c : copies)
            if (!c.outside)
                fill(c);
        for (auto &c : copies)
            if (c.outside)
                fill(c);
        std::set<std::string> seen(names.begin(), names.end());
        std::vector<const Want *> used;
        for (auto *w : list) {
            if (!seen.insert(w->key).second) {
                out.lines.push_back("team.logo: " + array + " lists " + w->key + " already; its slice is kept");
                continue;
            }
            std::vector<std::string> slices(copies.size());
            if (!w->donor.empty()) {
                auto at = std::find(names.begin(), names.end(), w->donor);
                if (at == names.end()) {
                    out.lines.push_back("team.logo: " + array + " has no stock key " + w->donor + " for " + w->key +
                                        (w->fallback ? "; the donor fallback is left out" : ""));
                    if (!w->fallback)
                        refuse(array + " has no stock key to copy", w->donor + " for " + w->key);
                    seen.erase(w->key);
                    continue;
                }
                for (size_t i = 0; i < copies.size(); i++) {
                    size_t bytes = copies[i].shape.slice_bytes();
                    slices[i] = copies[i].stock.data.substr(size_t(at - names.begin()) * bytes, bytes);
                }
            } else {
                std::map<std::pair<uint32_t, uint32_t>, std::string> encoded;
                bool ok = true;
                for (size_t i = 0; i < copies.size() && ok; i++) {
                    const logo_array::Shape &shape = copies[i].shape;
                    auto [e, fresh] = encoded.try_emplace({shape.width, shape.height});
                    bool scaled = false;
                    if (fresh && !(ok = logo_array::slice_from_rgba(shape, reinterpret_cast<const uint8_t *>(w->rgba.data()), w->width,
                                                                    w->height, e->second, why, &scaled)))
                        break;
                    if (scaled)
                        out.lines.push_back("team.logo: " + w->key + " scaled " + std::to_string(w->width) + "x" + std::to_string(w->height) +
                                            " -> " + std::to_string(shape.width) + "x" + std::to_string(shape.height) + " for " + array);
                    slices[i] = e->second;
                }
                if (!ok) {
                    refuse(array + ": " + why, w->key);
                    seen.erase(w->key);
                    continue;
                }
            }
            for (size_t i = 0; i < copies.size(); i++)
                copies[i].add.push_back({w->key, std::move(slices[i])});
            used.push_back(w);
        }
        if (used.empty())
            continue;
        size_t grew = 0, archives = 0;
        for (auto &c : copies) {
            logo_array::Stock grown;
            if (!logo_array::grow(c.stock, c.add, grown, why)) {
                if (copies.size() > 1)
                    out.lines.push_back("team.logo: skipped " + array + " in " + where(c.carriers) + ": " + why + "; left as it is");
                continue;
            }
            for (auto &rel : c.carriers) {
                for (auto &l : image_in[rel])
                    out.changes[l][image] = {grown.head, grown.data};
                for (auto &l : view_in[rel])
                    out.changes[l][view] = {grown.view};
            }
            grew++;
            archives += c.carriers.size();
        }
        if (!grew) {
            refuse(array + ": " + why, array + "/" + used[0]->key, int(used.size()));
            continue;
        }
        for (size_t at = 0; at < used.size(); at++)
            out.lines.push_back("team.logo: " + array + " slice " + std::to_string(names.size() + at) + " = " + used[at]->key +
                                (used[at]->donor.empty() ? " (its image)" : " (copy of " + used[at]->donor + ", slice " +
                                 std::to_string(std::find(names.begin(), names.end(), used[at]->donor) - names.begin()) + ")"));
        for (auto *w : used) {
            std::string e;
            if (!pkg.append_entry(*node, w->key, std::string(TEXTURE_PATH) + w->key + ".tif", e)) {
                refuse(array + ": " + e, w->key);
            }
        }
        uidb_changed = true;
        out.built += int(used.size());
        out.lines.push_back("team.logo: " + array + " grows by " + std::to_string(used.size()) + " slice(s) in " + std::to_string(archives) +
                            " archive(s)" + (grew > 1 ? " (" + std::to_string(grew) + " different copies)" : ""));
    }
    if (uidb_changed) {
        std::string written = pkg.write();
        for (auto &l : carrying(db_layers, OVERRIDES)) {
            out.changes[l][OVERRIDES] = {written};
            out.read.insert(l);
        }
    }
    return true;
}

}
