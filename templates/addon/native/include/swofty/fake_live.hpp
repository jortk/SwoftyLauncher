// SPDX-License-Identifier: MIT
#pragma once
#include <cstring>
#include <map>
#include <optional>
#include <string>
#include <vector>
#include <swofty/live.hpp>

// A fake core live state for host tests: set the session and cars the addon should see, install it, and
// swofty::live::* answers from it. One fake is installed at a time.
//
//   swofty::fake::Live fake;
//   fake.session().type = 15;
//   fake.add_car(1, 1).speed = 80.f;
//   fake.set_order({1, 0});
//   fake.install();
//   assert(swofty::live::car(1)->position == 1);
//   assert(swofty::live::order().ids.size() == 2);

namespace swofty::fake {

class Live {
public:
    Live()
    {
        std::memset(&session_, 0, sizeof session_);
        session_.version = SWOFTY_LIVE_VERSION;
        session_.size = sizeof session_;
        session_.player = session_.watched = session_.subject = -1;
        session_.safety_car = 0xff;
        session_.have = SWOFTY_LIVE_SHAVE_TYPE | SWOFTY_LIVE_SHAVE_CARS;
        std::memset(cars_, 0, sizeof cars_);
        std::memset(present_, 0, sizeof present_);
        table_ = {SWOFTY_LIVE_VERSION, &Live::t_available, &Live::t_session, &Live::t_car, &Live::t_subject,
                  &Live::t_turn,       &Live::t_track_turns, &Live::t_badge, &Live::t_seq,
                  &Live::t_driver_name, &Live::t_badge_texture, nullptr};
    }

    ~Live() { uninstall(); }
    Live(const Live &) = delete;
    Live &operator=(const Live &) = delete;

    void install()
    {
        current() = this;
        live::detail::set_table(&table_);
    }

    void uninstall()
    {
        if (current() == this) {
            current() = nullptr;
            live::detail::set_table(nullptr);
        }
    }

    void set_available(bool on) { available_ = on; }
    SwoftyLiveSession &session() { return session_; }

    SwoftyLiveCar &add_car(uint32_t vehicle, int32_t position = 0)
    {
        SwoftyLiveCar &c = cars_[vehicle];
        std::memset(&c, 0, sizeof c);
        c.version = SWOFTY_LIVE_VERSION;
        c.size = sizeof c;
        c.vehicle = vehicle;
        c.participant = 0xffffffffu;
        c.position = position;
        c.gap_leader_ms = c.gap_ahead_ms = SWOFTY_LIVE_NO_GAP;
        c.have = SWOFTY_LIVE_HAVE_IDENT | (position ? SWOFTY_LIVE_HAVE_CLASS : 0);
        c.flags = SWOFTY_LIVE_CAR_RACER;
        present_[vehicle] = true;
        session_.cars = 0;
        for (bool p : present_)
            session_.cars += p;
        return c;
    }

    SwoftyLiveCar &car(uint32_t vehicle) { return cars_[vehicle]; }

    void set_player(uint32_t vehicle)
    {
        session_.player = static_cast<int32_t>(vehicle);
        cars_[vehicle].flags |= SWOFTY_LIVE_CAR_PLAYER;
        session_.subject = session_.watched >= 0 ? session_.watched : session_.player;
        session_.have |= SWOFTY_LIVE_SHAVE_PLAYER;
    }

    void set_watched(uint32_t vehicle)
    {
        session_.watched = session_.subject = static_cast<int32_t>(vehicle);
        cars_[vehicle].flags |= SWOFTY_LIVE_CAR_WATCHED;
        session_.subject_generation++;
        session_.have |= SWOFTY_LIVE_SHAVE_WATCHED;
    }

    void set_order(const std::vector<uint32_t> &ids)
    {
        session_.classified = static_cast<uint32_t>(ids.size());
        for (size_t i = 0; i < ids.size() && i < SWOFTY_LIVE_MAX_CARS; i++) {
            session_.order[i] = ids[i];
            cars_[ids[i]].position = static_cast<int32_t>(i + 1);
            cars_[ids[i]].have |= SWOFTY_LIVE_HAVE_CLASS;
        }
        session_.have |= SWOFTY_LIVE_SHAVE_ORDER;
    }

    void set_turns(const std::vector<SwoftyLiveTurn> &turns) { turns_ = turns; }
    void set_badge(uint32_t vehicle, const std::string &key) { badges_[vehicle] = key; }
    void set_driver_name(uint32_t vehicle, const std::string &name) { names_[vehicle] = name; }
    // Answer as an older core does: version 1 has no driver_name member, version 2 no team_badge_texture member.
    void set_table_version(uint32_t v) { table_.version = v; }
    void set_badge_texture(uint32_t vehicle, const std::string &path) { textures_[vehicle] = path; }
    void advance() { frame_++; }

private:
    static Live *&current()
    {
        static Live *p = nullptr;
        return p;
    }

    static int t_available() { return current() && current()->available_ ? 1 : 0; }

    static int t_session(SwoftyLiveSession *out)
    {
        if (!t_available())
            return SWOFTY_ASSET_UNAVAILABLE;
        *out = current()->session_;
        return SWOFTY_ASSET_OK;
    }

    static int t_car(uint32_t vehicle, SwoftyLiveCar *out)
    {
        if (!t_available())
            return SWOFTY_ASSET_UNAVAILABLE;
        if (vehicle >= SWOFTY_LIVE_MAX_CARS)
            return SWOFTY_ASSET_INVALID;
        if (!current()->present_[vehicle])
            return SWOFTY_ASSET_UNAVAILABLE;
        *out = current()->cars_[vehicle];
        return SWOFTY_ASSET_OK;
    }

    static int t_subject(SwoftyLiveCar *out)
    {
        if (!t_available() || current()->session_.subject < 0)
            return SWOFTY_ASSET_UNAVAILABLE;
        return t_car(static_cast<uint32_t>(current()->session_.subject), out);
    }

    static int t_turn(uint32_t vehicle, SwoftyLiveTurn *out)
    {
        if (!t_available() || vehicle >= SWOFTY_LIVE_MAX_CARS || !current()->present_[vehicle])
            return SWOFTY_ASSET_UNAVAILABLE;
        float lap_m = current()->cars_[vehicle].lap_m;
        const SwoftyLiveTurn *best = nullptr;
        float best_d = 0;
        for (const SwoftyLiveTurn &t : current()->turns_) {
            float d = t.apex_lap_m - lap_m;
            if (d < -15.f || d > 220.f)
                continue;
            if (!best || d < best_d) {
                best = &t;
                best_d = d;
            }
        }
        if (!best)
            return SWOFTY_ASSET_UNAVAILABLE;
        *out = *best;
        out->distance_ahead_m = best_d;
        return SWOFTY_ASSET_OK;
    }

    static int t_track_turns(uint32_t *count, SwoftyLiveTurn *out, uint32_t cap)
    {
        if (!t_available() || current()->turns_.empty())
            return SWOFTY_ASSET_UNAVAILABLE;
        *count = static_cast<uint32_t>(current()->turns_.size());
        for (uint32_t i = 0; out && i < cap && i < *count; i++)
            out[i] = current()->turns_[i];
        return SWOFTY_ASSET_OK;
    }

    static int t_badge(uint32_t vehicle, char *key, uint32_t cap)
    {
        if (!t_available())
            return SWOFTY_ASSET_UNAVAILABLE;
        auto it = current()->badges_.find(vehicle);
        if (it == current()->badges_.end() || it->second.size() + 1 > cap)
            return SWOFTY_ASSET_UNAVAILABLE;
        std::memcpy(key, it->second.c_str(), it->second.size() + 1);
        return SWOFTY_ASSET_OK;
    }

    static int t_driver_name(uint32_t vehicle, char *out, uint32_t cap)
    {
        if (!t_available())
            return SWOFTY_ASSET_UNAVAILABLE;
        if (!out || cap < 2 || vehicle >= SWOFTY_LIVE_MAX_CARS)
            return SWOFTY_ASSET_INVALID;
        auto it = current()->names_.find(vehicle);
        if (it == current()->names_.end())
            return SWOFTY_ASSET_UNAVAILABLE;
        size_t n = it->second.size() < cap - 1 ? it->second.size() : cap - 1;
        std::memcpy(out, it->second.data(), n);
        out[n] = 0;
        return SWOFTY_ASSET_OK;
    }

    static int t_badge_texture(uint32_t vehicle, char *path, uint32_t cap)
    {
        if (!t_available())
            return SWOFTY_ASSET_UNAVAILABLE;
        auto it = current()->textures_.find(vehicle);
        std::string s;
        if (it != current()->textures_.end())
            s = it->second;
        else if (auto key = current()->badges_.find(vehicle); key != current()->badges_.end())
            s = "#overrides.osd.timing_tower.team_badges_hud." + key->second;
        if (s.empty() || s.size() + 1 > cap)
            return SWOFTY_ASSET_UNAVAILABLE;
        std::memcpy(path, s.c_str(), s.size() + 1);
        return SWOFTY_ASSET_OK;
    }

    static int t_seq(uint64_t *frame)
    {
        if (!t_available())
            return SWOFTY_ASSET_UNAVAILABLE;
        *frame = current()->frame_;
        return SWOFTY_ASSET_OK;
    }

    bool available_ = true;
    uint64_t frame_ = 1;
    SwoftyLiveSession session_;
    SwoftyLiveCar cars_[SWOFTY_LIVE_MAX_CARS];
    bool present_[SWOFTY_LIVE_MAX_CARS];
    std::vector<SwoftyLiveTurn> turns_;
    std::map<uint32_t, std::string> badges_, textures_;
    std::map<uint32_t, std::string> names_;
    SwoftyLive table_;
};

}
