#pragma once

#include <swofty/json.hpp>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <deque>
#include <string>
#include <vector>

namespace runtime::toast {

constexpr int64_t MAX_AGE_S = 10;
constexpr uint64_t SHOW_MS = 5000;
constexpr float SHOW_SECONDS = 5.0f;
constexpr size_t MAX_QUEUED = 4;
constexpr size_t REMEMBER = 32;

struct Message {
    std::string id, content, track, weather;
    int64_t lap_ms = 0;
    int64_t position = 0;
    int64_t total = 0;
    int64_t improvement_ms = 0;
    bool record = false;
    int64_t created = 0;
};

enum class Status { ok, not_object, no_id, no_lap, no_position };

inline Status parse(const std::string &text, Message &out)
{
    swofty::json::Value v = swofty::json::parse(text);
    if (!v.is_object())
        return Status::not_object;
    Message m;
    m.id = std::string(v["id"].as_string());
    if (m.id.empty())
        return Status::no_id;
    m.lap_ms = v["lap_ms"].as_int(0);
    if (m.lap_ms <= 0)
        return Status::no_lap;
    m.position = v["position"].as_int(0);
    if (m.position < 1)
        return Status::no_position;
    m.total = std::max<int64_t>(0, v["total"].as_int(0));
    m.improvement_ms = std::max<int64_t>(0, v["improvement_ms"].as_int(0));
    m.record = v["record"].as_bool(false) || m.position == 1;
    m.created = v["created"].as_int(0);
    const auto &b = v["board"];
    m.content = std::string(b["content"].as_string());
    m.track = std::string(b["track"].as_string());
    m.weather = std::string(b["weather"].as_string());
    out = std::move(m);
    return Status::ok;
}

inline std::string format_time(int64_t ms)
{
    if (ms < 0)
        ms = 0;
    char b[32];
    std::snprintf(b, sizeof b, "%lld:%02lld.%03lld", (long long)(ms / 60000), (long long)(ms / 1000 % 60), (long long)(ms % 1000));
    return b;
}

inline std::string format_gain(int64_t ms)
{
    char b[32];
    std::snprintf(b, sizeof b, "-%lld.%03lld", (long long)(ms / 1000), (long long)(ms % 1000));
    return b;
}

inline std::string title_of(const Message &m)
{
    return m.record ? "NEW RECORD" : "NEW PERSONAL BEST";
}

inline std::string detail_of(const Message &m)
{
    std::string s = "P" + std::to_string(m.position);
    if (m.total >= m.position)
        s += " of " + std::to_string(m.total);
    s += " - " + format_time(m.lap_ms);
    if (m.improvement_ms > 0)
        s += "  " + format_gain(m.improvement_ms);
    return s;
}

struct Snapshot {
    bool visible = false;
    bool started = false;
    Message message;
};

class Machine {
  public:
    bool offer(const Message &m, int64_t now_unix)
    {
        if (seen(m.id))
            return false;
        remember(m.id);
        if (m.created > 0 && now_unix - m.created > MAX_AGE_S)
            return false;
        if (queued_.size() >= MAX_QUEUED)
            return false;
        queued_.push_back(m);
        return true;
    }

    Snapshot step(uint64_t now_ms, int64_t now_unix)
    {
        if (showing_ && now_ms - started_ >= SHOW_MS)
            showing_ = false;
        bool fresh = false;
        while (!showing_ && !queued_.empty()) {
            Message next = std::move(queued_.front());
            queued_.pop_front();
            if (next.created > 0 && now_unix - next.created > MAX_AGE_S)
                continue;
            current_ = std::move(next);
            started_ = now_ms;
            showing_ = true;
            fresh = true;
        }
        if (!showing_)
            return {};
        return {true, fresh, current_};
    }

    bool active() const { return showing_ || !queued_.empty(); }

    void clear()
    {
        queued_.clear();
        showing_ = false;
    }

  private:
    bool seen(const std::string &id) const { return std::find(ids_.begin(), ids_.end(), id) != ids_.end(); }

    void remember(const std::string &id)
    {
        ids_.push_back(id);
        if (ids_.size() > REMEMBER)
            ids_.erase(ids_.begin());
    }

    std::deque<Message> queued_;
    std::vector<std::string> ids_;
    Message current_;
    uint64_t started_ = 0;
    bool showing_ = false;
};

}
