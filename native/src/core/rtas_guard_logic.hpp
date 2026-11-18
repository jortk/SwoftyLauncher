#pragma once

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

// A ray tracing build reads every active geometry's vertex and index buffers, the instance list of a
// top-level build and the structure an update starts from, and writes the destination, the scratch
// memory and any postbuild info. A null address in any of them makes the GPU touch address 0: on
// NVIDIA under Windows that is a page fault and the device is removed (DXGI_ERROR_DEVICE_HUNG).
// The guard gives a bottom-level geometry with a null buffer zero primitives, so the build reads
// nothing for it. An update build must keep the primitive counts of the build it updates, so a
// geometry emptied in a build stays empty in every update of it. A top-level build with a null
// instance list builds no instances. A build with a null destination, scratch or update source is
// not sent, and postbuild info with a null destination is dropped.

namespace runtime::rtas_guard {

enum GeoType : int { kTriangles = 0, kAabbs = 1 };
enum Level : int { kTop = 0, kBottom = 1 };

struct Geo {
    int type = kTriangles;
    uint64_t vertices = 0;
    uint32_t vertex_count = 0;
    uint64_t indices = 0;
    uint32_t index_count = 0;
    bool indexed = false;
    uint64_t aabbs = 0;
    uint64_t aabb_count = 0;
};

enum class Fault { none, vertices, indices, aabbs };

inline const char *fault_name(Fault f)
{
    switch (f) {
    case Fault::vertices:
        return "a null vertex buffer";
    case Fault::indices:
        return "a null index buffer";
    case Fault::aabbs:
        return "a null AABB buffer";
    case Fault::none:
        break;
    }
    return "no fault";
}

inline Fault fault(const Geo &g)
{
    if (g.type == kAabbs)
        return g.aabb_count && !g.aabbs ? Fault::aabbs : Fault::none;
    if (g.type != kTriangles)
        return Fault::none;
    if (g.indexed) {
        if (!g.index_count)
            return Fault::none;
        if (!g.vertices)
            return Fault::vertices;
        return g.indices ? Fault::none : Fault::indices;
    }
    return g.vertex_count && !g.vertices ? Fault::vertices : Fault::none;
}

inline void empty(Geo &g)
{
    g.vertex_count = 0;
    g.index_count = 0;
    g.aabb_count = 0;
}

struct Build {
    uint64_t dest = 0, source = 0;
    bool update = false;
};

struct Plan {
    std::vector<uint32_t> emptied;
    uint32_t faulty = 0, kept_empty = 0;
};

// The bottom-level structures (by destination address) with geometries the guard emptied, and which.
class Book {
public:
    Plan plan(const Build &b, const std::vector<Geo> &geos)
    {
        Plan p;
        std::vector<bool> marked(geos.size(), false);
        for (uint32_t i = 0; i < geos.size(); i++)
            if (fault(geos[i]) != Fault::none) {
                marked[i] = true;
                p.faulty++;
            }
        if (b.update) {
            auto it = emptied_.find(b.source ? b.source : b.dest);
            if (it != emptied_.end())
                for (uint32_t i : it->second)
                    if (i < marked.size() && !marked[i]) {
                        marked[i] = true;
                        p.kept_empty++;
                    }
        }
        for (uint32_t i = 0; i < marked.size(); i++)
            if (marked[i])
                p.emptied.push_back(i);
        if (p.emptied.empty())
            emptied_.erase(b.dest);
        else
            emptied_[b.dest] = p.emptied;
        return p;
    }

    size_t tracked() const { return emptied_.size(); }

private:
    std::unordered_map<uint64_t, std::vector<uint32_t>> emptied_;
};

// The primitive counts of a bottom-level build's geometries, hashed one geometry at a time.
struct Shape {
    uint64_t hash = 1469598103934665603ull;

    void mix(uint64_t v) { hash = (hash ^ v) * 1099511628211ull; }

    void add(const Geo &g)
    {
        mix(uint64_t(g.type));
        mix(g.vertex_count);
        mix(g.index_count);
        mix(g.aabb_count);
    }
};

// The shapes the build log has named, each once, up to the cap.
class Trace {
public:
    explicit Trace(uint32_t cap) : cap_(cap) {}

    bool want(uint64_t shape)
    {
        if (named_ >= cap_ || !seen_.insert(shape).second)
            return false;
        named_++;
        return true;
    }

    bool full() const { return named_ >= cap_; }
    uint32_t named() const { return named_; }

private:
    uint32_t cap_, named_ = 0;
    std::unordered_set<uint64_t> seen_;
};

constexpr int kBriefGeos = 12;
constexpr uint32_t kPerformUpdate = 0x20;

struct Brief {
    int type = kTriangles;
    uint32_t vertices = 0, indices = 0, aabbs = 0, stride = 0, flags = 0;
    int vertex_format = 0, index_format = 0;
    bool transform = false;
};

struct Entry {
    uint64_t seq = 0, frame = 0, list = 0, dest = 0;
    uint32_t flags = 0, count = 0;
    bool top = false;
    Brief geo[kBriefGeos];
};

inline bool same_format(const Brief &a, const Brief &b)
{
    return a.vertex_format == b.vertex_format && a.stride == b.stride && a.index_format == b.index_format &&
           a.flags == b.flags && a.transform == b.transform;
}

// The geometries of a build: each one's counts, and its formats wherever they differ from the first triangle geometry's.
inline std::string geometry_text(const Entry &e)
{
    std::string s;
    char b[96];
    const Brief *first = nullptr;
    uint32_t shown = e.count < uint32_t(kBriefGeos) ? e.count : uint32_t(kBriefGeos);
    for (uint32_t i = 0; i < shown; i++) {
        const Brief &g = e.geo[i];
        if (g.type != kTriangles) {
            std::snprintf(b, sizeof b, " aabb%u", g.aabbs);
            s += b;
            continue;
        }
        std::snprintf(b, sizeof b, " v%u/i%u", g.vertices, g.indices);
        s += b;
        if (!first || !same_format(*first, g)) {
            std::snprintf(b, sizeof b, "(vf%d s%u if%d fl%u%s)", g.vertex_format, g.stride, g.index_format, g.flags,
                          g.transform ? " t" : "");
            s += b;
        }
        if (!first)
            first = &g;
    }
    if (shown < e.count) {
        std::snprintf(b, sizeof b, " +%u more", e.count - shown);
        s += b;
    }
    return s;
}

// The newest builds, kept so a crash report can show what the GPU was building when the device was removed.
template <size_t N> class Recent {
public:
    void push(const Entry &e)
    {
        ring_[pushed_ % N] = e;
        pushed_++;
    }

    uint64_t pushed() const { return pushed_; }

    std::vector<Entry> oldest_first() const
    {
        std::vector<Entry> v;
        for (uint64_t i = pushed_ > N ? pushed_ - N : 0; i < pushed_; i++)
            v.push_back(ring_[i % N]);
        return v;
    }

private:
    std::array<Entry, N> ring_{};
    uint64_t pushed_ = 0;
};

// Each build's place among the builds on its command list in its frame, counted from 1.
inline std::vector<uint32_t> list_ordinals(const std::vector<Entry> &v)
{
    std::vector<uint32_t> out(v.size(), 1);
    std::unordered_map<uint64_t, std::pair<uint64_t, uint32_t>> at;
    for (size_t i = 0; i < v.size(); i++) {
        auto &a = at[v[i].list];
        if (a.second && a.first == v[i].frame)
            a.second++;
        else
            a = {v[i].frame, 1};
        out[i] = a.second;
    }
    return out;
}

// The newest builds of each command list, so a list the game records rarely keeps its builds however many builds
// the busy lists record. Past the list cap, the list with the oldest newest build is dropped.
template <size_t PerList, size_t Lists> class ByList {
public:
    struct Kept {
        uint64_t list = 0, total = 0;
        std::vector<Entry> builds;
    };

    void push(const Entry &e)
    {
        auto it = lists_.find(e.list);
        if (it == lists_.end()) {
            if (lists_.size() >= Lists) {
                auto old = lists_.begin();
                for (auto i = lists_.begin(); i != lists_.end(); ++i)
                    if (i->second.last < old->second.last)
                        old = i;
                lists_.erase(old);
            }
            it = lists_.try_emplace(e.list).first;
        }
        it->second.ring.push(e);
        it->second.last = e.seq;
        pushed_++;
    }

    uint64_t pushed() const { return pushed_; }

    std::vector<Kept> newest_lists_first() const
    {
        std::vector<std::pair<uint64_t, uint64_t>> order;
        for (const auto &[list, slot] : lists_)
            order.push_back({slot.last, list});
        std::sort(order.rbegin(), order.rend());
        std::vector<Kept> out;
        for (const auto &o : order) {
            const Slot &slot = lists_.at(o.second);
            out.push_back({o.second, slot.ring.pushed(), slot.ring.oldest_first()});
        }
        return out;
    }

private:
    struct Slot {
        Recent<PerList> ring;
        uint64_t last = 0;
    };
    std::unordered_map<uint64_t, Slot> lists_;
    uint64_t pushed_ = 0;
};

// The build the GPU stopped on, for each command list DRED shows unfinished. Its build #nth of `of` is #nth of the
// list's newest kept frame when that frame keeps exactly `of` builds; otherwise the build is not known.
template <typename KeptList, typename Stops> std::string stopped_lines(const KeptList &lists, const Stops &stops)
{
    std::string s;
    char b[240];
    for (const auto &st : stops) {
        const typename KeptList::value_type *kept = nullptr;
        for (const auto &k : lists)
            if (k.list == st.list)
                kept = &k;
        if (!kept || kept->builds.empty()) {
            std::snprintf(b, sizeof b, "  the GPU stopped on list 0x%llx's build #%u of %u; no build of that list was kept\n",
                          (unsigned long long)st.list, st.nth, st.of);
            s += b;
            continue;
        }
        uint64_t frame = kept->builds.back().frame;
        size_t first = kept->builds.size();
        while (first && kept->builds[first - 1].frame == frame)
            first--;
        size_t in_frame = kept->builds.size() - first;
        bool whole = !(first == 0 && kept->total > kept->builds.size());
        if (!whole || in_frame != st.of || !st.nth || st.nth > st.of) {
            std::snprintf(b, sizeof b,
                          "  the GPU stopped on list 0x%llx's build #%u of %u; its newest kept frame %llu has %zu%s build(s), "
                          "so which build that is is not known\n",
                          (unsigned long long)st.list, st.nth, st.of, (unsigned long long)frame, in_frame, whole ? "" : "+");
            s += b;
            continue;
        }
        const Entry &e = kept->builds[first + st.nth - 1];
        std::snprintf(b, sizeof b, "  the GPU stopped on list 0x%llx's build #%u of %u: build %llu frame %llu %s%s into 0x%llx (flags 0x%x), %u %s:",
                      (unsigned long long)st.list, st.nth, st.of, (unsigned long long)e.seq, (unsigned long long)e.frame,
                      e.top ? "top-level" : "bottom-level", (e.flags & kPerformUpdate) ? " update" : "", (unsigned long long)e.dest,
                      e.flags, e.count, e.top ? "instances" : "geometries");
        s += b;
        if (!e.top)
            s += geometry_text(e);
        s += "\n";
    }
    return s;
}

template <typename KeptList>
std::string recent_section(const KeptList &lists, uint64_t pushed, size_t max_lists = 12, const std::string &stopped = {})
{
    if (lists.empty())
        return stopped.empty() ? std::string() : "\n" + stopped;
    std::string s;
    char b[200];
    std::snprintf(b, sizeof b,
                  "\n  ray tracing builds before the crash, %zu command list(s), the list with the newest build first; "
                  "%llu build(s) in all\n",
                  lists.size(), (unsigned long long)pushed);
    s += b;
    s += "  each list shows its newest builds, oldest first; #n is the build's place on its list in that frame\n";
    s += stopped;
    size_t shown_lists = 0;
    for (const auto &k : lists) {
        if (shown_lists++ == max_lists) {
            std::snprintf(b, sizeof b, "    %zu more command list(s) not shown\n", lists.size() - max_lists);
            s += b;
            break;
        }
        std::snprintf(b, sizeof b, "    list 0x%llx: the newest %zu of %llu build(s)\n", (unsigned long long)k.list,
                      k.builds.size(), (unsigned long long)k.total);
        s += b;
        bool cut = k.total > k.builds.size();
        std::vector<uint32_t> nth = list_ordinals(k.builds);
        for (size_t i = 0; i < k.builds.size(); i++) {
            const Entry &e = k.builds[i];
            bool unsure = cut && e.frame == k.builds.front().frame;
            std::snprintf(b, sizeof b, "      build %llu frame %llu #%u%s %s%s into 0x%llx (flags 0x%x), %u %s:",
                          (unsigned long long)e.seq, (unsigned long long)e.frame, nth[i], unsure ? "+" : "",
                          e.top ? "top-level" : "bottom-level", (e.flags & kPerformUpdate) ? " update" : "",
                          (unsigned long long)e.dest, e.flags, e.count, e.top ? "instances" : "geometries");
            s += b;
            if (!e.top)
                s += geometry_text(e);
            s += "\n";
        }
    }
    return s;
}

// The ray tracing options in the game's hardware_settings_config.xml: each <rt_NAME enabled="..."/> as "NAME on"
// or "NAME off", in file order.
inline std::string rt_options(const std::string &xml)
{
    std::string out;
    for (size_t at = xml.find("<rt_"); at != std::string::npos; at = xml.find("<rt_", at + 4)) {
        size_t close = xml.find('>', at), name_end = xml.find_first_of(" \t\r\n/>", at + 4);
        if (close == std::string::npos || name_end == std::string::npos || name_end > close)
            break;
        std::string tag = xml.substr(at, close - at);
        size_t en = tag.find("enabled=\"");
        size_t end = en == std::string::npos ? std::string::npos : tag.find('"', en + 9);
        if (end == std::string::npos)
            continue;
        std::string value = tag.substr(en + 9, end - en - 9);
        out += (out.empty() ? "" : ", ") + xml.substr(at + 4, name_end - at - 4) +
               (value == "true" ? " on" : value == "false" ? " off" : " " + value);
    }
    return out;
}

struct Call {
    int level = kBottom;
    uint64_t dest = 0, scratch = 0, source = 0, instances = 0;
    uint32_t count = 0;
    bool update = false;
};

enum class Hole { none, dest, scratch, source, instances };

inline const char *hole_name(Hole h)
{
    switch (h) {
    case Hole::dest:
        return "a null destination";
    case Hole::scratch:
        return "null scratch memory";
    case Hole::source:
        return "an update with a null source structure";
    case Hole::instances:
        return "a null instance list";
    case Hole::none:
        break;
    }
    return "no hole";
}

inline Hole hole(const Call &c)
{
    if (!c.dest)
        return Hole::dest;
    if (!c.scratch)
        return Hole::scratch;
    if (c.update && !c.source)
        return Hole::source;
    if (c.level == kTop && c.count && !c.instances)
        return Hole::instances;
    return Hole::none;
}

enum class Act { send, skip, no_instances };

inline Act act(const Call &c)
{
    switch (hole(c)) {
    case Hole::none:
        return Act::send;
    case Hole::instances:
        return c.update ? Act::skip : Act::no_instances;
    default:
        return Act::skip;
    }
}

inline const char *act_name(Act a)
{
    switch (a) {
    case Act::skip:
        return "not sent";
    case Act::no_instances:
        return "built with no instances";
    case Act::send:
        break;
    }
    return "sent";
}

inline std::vector<uint32_t> kept_posts(const std::vector<uint64_t> &dests)
{
    std::vector<uint32_t> kept;
    for (uint32_t i = 0; i < dests.size(); i++)
        if (dests[i])
            kept.push_back(i);
    return kept;
}

inline bool detailed(uint64_t nth, uint64_t limit) { return nth <= limit; }

inline bool milestone(uint64_t nth) { return nth && !(nth & (nth - 1)); }

enum class Mode { off, log, on };

inline bool same(const char *a, const char *b)
{
    for (; *a && *b; a++, b++)
        if ((*a | 0x20) != (*b | 0x20))
            return false;
    return !*a && !*b;
}

inline Mode mode(const char *env)
{
    if (!env)
        return Mode::on;
    if (same(env, "0") || same(env, "off"))
        return Mode::off;
    if (same(env, "log"))
        return Mode::log;
    return Mode::on;
}

}
