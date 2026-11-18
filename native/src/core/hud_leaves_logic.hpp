#pragma once
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstring>
#include <functional>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

namespace runtime::hud_leaves {

enum Type : uint32_t { TEXT = 0, BOOL = 1, INT = 2 };

constexpr uint32_t MAX_TEXT = 200;
constexpr uint32_t DEFAULT_CAP_PER_TICK = 256;
constexpr uint64_t RETRY_MS = 1000;
constexpr uint64_t RESYNC_MS = 10000;
constexpr const char *PREFIXES[] = {"ui_menu_data.hud_", "view_model_data.hud.", "view_model_data.hud_"};

enum Result : int { OK = 0, INVALID = 1 };

inline bool path_ok(const std::string &path)
{
    bool listed = std::any_of(std::begin(PREFIXES), std::end(PREFIXES), [&](const char *p) {
        return path.rfind(p, 0) == 0 && path.size() > strlen(p);
    });
    return listed && path.find_first_of(" []") == std::string::npos && path.back() != '.';
}

struct Leaf {
    std::string path, by;
    uint32_t type = TEXT;
    uint32_t cap = MAX_TEXT;
    std::string want_text, sent_text;
    int32_t want_num = 0, sent_num = 0;
    bool has_want = false, has_sent = false, queued = false, failed = false;
    uint32_t group = 0, row_group = 0;
};

struct Stats {
    uint32_t applied_last_tick = 0, queued = 0, dropped = 0, registered = 0, cap_per_tick = 0, failed = 0;
    uint32_t generation = 0, resyncs = 0;
};

// The registered-leaf table. Values are stored per leaf; a set that equals what was last written is
// dropped on the spot. The write queue is the dirty list, in the order leaves became dirty, so it holds at
// most one entry per leaf and cannot overflow; over the per-tick cap the rest waits for the next tick.
// A group (leaves_begin..leaves_end) is applied whole in one tick.
class Table {
public:
    using Apply = std::function<bool(const Leaf &)>;

    int register_leaf(const std::string &path, uint32_t type, uint32_t cap, const std::string &by, uint32_t &id)
    {
        if (type > INT || !path_ok(path))
            return INVALID;
        std::lock_guard<std::mutex> g(lock_);
        auto it = index_.find(path);
        if (it != index_.end()) {
            if (leaves_[it->second].type != type)
                return INVALID;
            id = it->second;
            return OK;
        }
        Leaf l;
        l.path = path;
        l.by = by;
        l.type = type;
        l.cap = cap == 0 || cap > MAX_TEXT ? MAX_TEXT : cap;
        id = uint32_t(leaves_.size());
        index_[path] = id;
        leaves_.push_back(std::move(l));
        return OK;
    }

    int type_of(const std::string &path) const
    {
        std::lock_guard<std::mutex> g(lock_);
        auto it = index_.find(path);
        return it == index_.end() ? -1 : int(leaves_[it->second].type);
    }

    void begin()
    {
        if (depth_()++ == 0)
            group_() = next_group_.fetch_add(1) + 1;
    }

    void end()
    {
        if (depth_() > 0 && --depth_() == 0)
            group_() = 0;
    }

    int set(const uint32_t *ids, const void *const *values, uint32_t n)
    {
        int rc = OK;
        std::lock_guard<std::mutex> g(lock_);
        for (uint32_t i = 0; i < n; i++) {
            if (ids[i] >= leaves_.size() || !values[i]) {
                rc = INVALID;
                continue;
            }
            Leaf &l = leaves_[ids[i]];
            if (l.type == TEXT) {
                std::string t(static_cast<const char *>(values[i]));
                if (t.size() > l.cap)
                    t.resize(l.cap);
                if (l.has_want && l.want_text == t)
                    continue;
                l.want_text = std::move(t);
            } else {
                int32_t v = *static_cast<const int32_t *>(values[i]);
                if (l.type == BOOL)
                    v = v != 0;
                if (l.has_want && l.want_num == v)
                    continue;
                l.want_num = v;
            }
            l.has_want = true;
            l.row_group = group_();
            if (differs(l))
                enqueue(ids[i], l);
        }
        return rc;
    }

    void rebuilt()
    {
        std::lock_guard<std::mutex> g(lock_);
        generation_++;
        resend_all();
    }

    uint32_t generation() const
    {
        std::lock_guard<std::mutex> g(lock_);
        return generation_;
    }

    void drain(uint64_t now_ms, const Apply &apply)
    {
        std::lock_guard<std::mutex> g(lock_);
        if (now_ms >= next_retry_ && failed_n_) {
            next_retry_ = now_ms + RETRY_MS;
            for (uint32_t i = 0; i < leaves_.size(); i++)
                if (leaves_[i].failed && leaves_[i].has_want)
                    enqueue(i, leaves_[i]);
        }
        if (!leaves_.empty() && now_ms >= next_resync_) {
            if (next_resync_)
                resend_all(), resyncs_++;
            next_resync_ = now_ms + RESYNC_MS;
        }
        applied_ = 0;
        size_t done = 0;
        while (done < dirty_.size()) {
            uint32_t grp = leaves_[dirty_[done]].group;
            size_t end = done + 1;
            if (grp)
                while (end < dirty_.size() && leaves_[dirty_[end]].group == grp)
                    end++;
            size_t count = end - done;
            if (applied_ && applied_ + count > cap_)
                break;
            for (size_t k = done; k < end; k++) {
                Leaf &l = leaves_[dirty_[k]];
                l.queued = false;
                l.group = 0;
                if (!differs(l))
                    continue;
                bool ok = apply(l);
                if (ok) {
                    if (l.failed)
                        failed_n_--;
                    l.failed = false;
                    l.has_sent = true;
                    l.sent_text = l.want_text;
                    l.sent_num = l.want_num;
                } else {
                    if (!l.failed) {
                        l.failed = true;
                        failed_n_++;
                    }
                    next_retry_ = now_ms + RETRY_MS;
                }
                applied_++;
            }
            done = end;
        }
        dirty_.erase(dirty_.begin(), dirty_.begin() + ptrdiff_t(done));
    }

    Stats stats() const
    {
        std::lock_guard<std::mutex> g(lock_);
        Stats s;
        s.applied_last_tick = applied_;
        s.queued = uint32_t(dirty_.size());
        s.dropped = 0;
        s.registered = uint32_t(leaves_.size());
        s.cap_per_tick = cap_;
        s.failed = failed_n_;
        s.generation = generation_;
        s.resyncs = resyncs_;
        return s;
    }

    void set_cap(uint32_t cap)
    {
        std::lock_guard<std::mutex> g(lock_);
        cap_ = cap ? cap : DEFAULT_CAP_PER_TICK;
    }

    size_t size() const
    {
        std::lock_guard<std::mutex> g(lock_);
        return leaves_.size();
    }

    std::vector<std::string> parents() const
    {
        std::lock_guard<std::mutex> g(lock_);
        std::vector<std::string> out;
        for (auto &l : leaves_) {
            std::string p = l.path.substr(0, l.path.find_last_of('.'));
            if (std::find(out.begin(), out.end(), p) == out.end())
                out.push_back(std::move(p));
        }
        return out;
    }

private:
    static bool differs(const Leaf &l)
    {
        if (!l.has_sent)
            return true;
        return l.type == TEXT ? l.want_text != l.sent_text : l.want_num != l.sent_num;
    }

    void enqueue(uint32_t id, Leaf &l)
    {
        if (l.queued)
            return;
        l.queued = true;
        l.group = l.row_group;
        dirty_.push_back(id);
    }

    void resend_all()
    {
        for (uint32_t i = 0; i < leaves_.size(); i++) {
            leaves_[i].has_sent = false;
            if (leaves_[i].has_want)
                enqueue(i, leaves_[i]);
        }
    }

    static int &depth_()
    {
        thread_local int d = 0;
        return d;
    }

    static uint32_t &group_()
    {
        thread_local uint32_t g = 0;
        return g;
    }

    mutable std::mutex lock_;
    std::vector<Leaf> leaves_;
    std::unordered_map<std::string, uint32_t> index_;
    std::vector<uint32_t> dirty_;
    uint32_t cap_ = DEFAULT_CAP_PER_TICK, applied_ = 0, generation_ = 1, failed_n_ = 0, resyncs_ = 0;
    uint64_t next_retry_ = 0, next_resync_ = 0;
    inline static std::atomic<uint32_t> next_group_{0};
};

}
