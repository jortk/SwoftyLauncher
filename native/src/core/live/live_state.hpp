#pragma once
#include <swofty_api.h>

#include "live_layout.hpp"
#include "live_mem.hpp"
#include "live_badges.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

namespace swofty_core::live {

namespace L = layout;

constexpr unsigned MAX_CARS = SWOFTY_LIVE_MAX_CARS;
constexpr size_t MAX_ENTRIES = 64;

inline float from_bits(uint32_t u)
{
    float f;
    std::memcpy(&f, &u, sizeof f);
    return f;
}

inline bool within(float v, float lo, float hi)
{
    return std::isfinite(v) && v >= lo && v <= hi;
}

template <class T> inline T field(const uint8_t *buf, uintptr_t lo, uintptr_t off)
{
    T v;
    std::memcpy(&v, buf + (off - lo), sizeof v);
    return v;
}

inline int sector_of(const float *ends, int count, float metres)
{
    if (!std::isfinite(metres) || count < 1)
        return 0;
    for (int i = 0; i < count; i++)
        if (ends[i] >= metres)
            return i + 1;
    return 0;
}

inline char tyre_letter_of_visual(uint8_t v)
{
    return v == 16 ? 'S' : v == 17 ? 'M' : v == 18 ? 'H' : v == 7 ? 'I' : v == 8 ? 'W' : 0;
}

inline char tyre_letter_of_compound(int32_t c)
{
    return c == 3 ? 'S' : c == 4 ? 'M' : c == 5 ? 'H' : c == 7 ? 'I' : c == 8 ? 'W' : 0;
}

inline char checked_tyre_letter(uint8_t visual, int32_t compound)
{
    char a = tyre_letter_of_visual(visual), b = tyre_letter_of_compound(compound);
    return a && a == b ? a : 0;
}

// Length of the longest prefix of s[0..n) that is whole, valid UTF-8 and no longer than max bytes; npos when s holds a
// control character or an invalid sequence (the string is then not a name).
inline size_t utf8_prefix(const char *s, size_t n, size_t max)
{
    size_t i = 0, keep = 0;
    while (i < n) {
        unsigned char c = static_cast<unsigned char>(s[i]);
        size_t len = c < 0x80 ? 1 : (c & 0xe0) == 0xc0 ? 2 : (c & 0xf0) == 0xe0 ? 3 : (c & 0xf8) == 0xf0 ? 4 : 0;
        if (!len || (len == 1 && (c < 0x20 || c == 0x7f)) || (len == 2 && c < 0xc2) || (len == 4 && c > 0xf4) || i + len > n)
            return ~size_t(0);
        for (size_t k = 1; k < len; k++)
            if ((static_cast<unsigned char>(s[i + k]) & 0xc0) != 0x80)
                return ~size_t(0);
        if (len == 3) {
            unsigned char d = static_cast<unsigned char>(s[i + 1]);
            if ((c == 0xe0 && d < 0xa0) || (c == 0xed && d > 0x9f))
                return ~size_t(0);
        } else if (len == 4) {
            unsigned char d = static_cast<unsigned char>(s[i + 1]);
            if ((c == 0xf0 && d < 0x90) || (c == 0xf4 && d > 0x8f))
                return ~size_t(0);
        }
        i += len;
        if (i <= max)
            keep = i;
    }
    return keep;
}

struct Event {
    enum Kind : uint8_t { retirement, overtake, collision } kind;
    int64_t a, b;
};

struct Observed {
    bool valid = false;
    uint8_t position = 0, status = 0xff, pit = 0xff;
    float penalty = 0;
};

inline void derive_events(const Observed *prev, const Observed *now, uint32_t session_type, uint8_t safety_car,
                          std::vector<Event> &out, bool retirements_from_bus = false)
{
    for (unsigned i = 0; i < MAX_CARS && !retirements_from_bus; i++) {
        if (!prev[i].valid || !now[i].valid)
            continue;
        bool was = prev[i].status == 4 || prev[i].status == 7, is = now[i].status == 4 || now[i].status == 7;
        if (!was && is && prev[i].status != 0xff)
            out.push_back({Event::retirement, int64_t(i), 0});
    }
    if (session_type < 15 || session_type > 17 || safety_car == 3)
        return;
    auto on_track = [](const Observed &o) { return o.status == 2 && !(o.pit >= 2 && o.pit <= 15); };
    auto settled = [](const Observed &p, const Observed &n) {
        return p.status == n.status && p.penalty == n.penalty && p.position && n.position;
    };
    for (unsigned x = 0; x < MAX_CARS; x++) {
        const Observed &px = prev[x], &nx = now[x];
        if (!px.valid || !nx.valid || !settled(px, nx) || nx.position >= px.position || !on_track(px) || !on_track(nx))
            continue;
        for (unsigned y = 0; y < MAX_CARS; y++) {
            const Observed &py = prev[y], &ny = now[y];
            if (y == x || !py.valid || !ny.valid || !settled(py, ny) || !on_track(py) || !on_track(ny))
                continue;
            if (py.position < px.position && ny.position > nx.position)
                out.push_back({Event::overtake, int64_t(x), int64_t(y)});
        }
    }
}

struct SysRec {
    uint64_t have = 0;
    float speed = 0, rpm = 0, rpm_limit = 0, throttle = 0, brake = 0, clutch = 0, steer = 0, g_lat = 0, g_long = 0;
    float pos[3] = {}, vel[3] = {};
    int32_t gear = 0;
    uint8_t drs = 0;
};

struct ErsRec {
    uint64_t have = 0;
    float store = 0, capacity = 0, deploy = 0, harvest_k = 0, harvest_h = 0, max_deploy = 0;
    float harvested_lap = 0, deployed_lap = 0, harvest_limit_lap = 0;
    int32_t mode = 0;
    uint8_t cls = 0;
};

struct Progress {
    float total = 0;
    int lap = 0, plap = 0;
};

class Live {
public:
    Live(Mem mem, Sites sites, Gates gates) : m_(mem), s_(sites), g_(gates) { reset(); }

    SwoftyLiveSession ses;
    SwoftyLiveCar cars[MAX_CARS];
    bool present[MAX_CARS] = {};
    uint64_t seq = 0;

    std::vector<Event> take_events()
    {
        std::vector<Event> e;
        e.swap(events_);
        return e;
    }

    void set_retirements_from_bus(bool on) { retire_from_bus_ = on; }
    void add_event(const Event &e)
    {
        events_.push_back(e);
        if (events_.size() > 256)
            events_.erase(events_.begin(), events_.end() - 256);
    }

    bool active() const { return n_cars_ > 0; }
    const char *last_stop() const { return stop_; }

    void update(double now)
    {
        now_ = now;
        if (now < last_now_)
            reset_timers();
        last_now_ = now;
        uint8_t count = 0;
        uintptr_t slots[MAX_CARS];
        if (!s_.vehicle_table || !s_.vehicle_count || !m_.get(s_.vehicle_count, count) || !count || count > MAX_CARS ||
            !m_.read(s_.vehicle_table, slots, count * sizeof(uintptr_t))) {
            lose();
            return;
        }
        bool changed = count != slot_count_ || std::memcmp(slots, slots_, count * sizeof(uintptr_t)) != 0;
        if (changed || due(t_scan_, 0.1))
            scan(slots, count);
        if (g_[G_CONTROL] && due(t_ctrl_, 0.1))
            read_controllers();
        if (g_[G_SPECTATE])
            read_spectate();
        if (g_[G_CLASS] && due(t_cls_, 0.1))
            job_cls();
        if (g_[G_PROGRESS] && due(t_prog_, 0.05))
            job_progress();
        if (g_[G_TIMING] && due(t_timing_, 0.1))
            job_timing();
        if (g_[G_PIT] && due(t_pit_, 0.1))
            job_pit();
        if (g_[G_PENALTIES] && due(t_pen_, 0.25))
            job_penalties();
        if (g_[G_TYRES] && due(t_tyre_, 0.5))
            job_tyres();
        if (g_[G_FLAGS] && due(t_flags_, 0.25))
            job_flags();
        if (g_[G_MODE] && due(t_mode_, 0.25))
            job_mode();
        if (g_[G_TRACK] && due(t_track_, 1.0))
            job_track();
        read_car_systems(changed);
        compose();
        ++seq;
    }

    bool read_team_badge(uint32_t vehicle, char *key, size_t cap)
    {
        BadgeRef b;
        if (cap < 2 || !find_badge(vehicle, b) || b.kind == L::badge::my_team_kind || b.render)
            return false;
        return read_cstr(b.key, key, cap > 40 ? 40 : cap);
    }

    bool read_team_badge_texture(uint32_t vehicle, char *path, size_t cap)
    {
        if (path && cap)
            path[0] = 0;
        BadgeRef b;
        if (!path || cap < 2 || !find_badge(vehicle, b))
            return false;
        char id[40];
        int n = -1;
        if (!b.render) {
            if (read_cstr(b.key, id, sizeof id))
                n = std::snprintf(path, cap, "%s%s", L::badge::catalogue_prefix, id);
        } else if (uint32_t badge = 0; g_[G_BADGE_RENDER] && m_.get(b.entrant + L::badge::entrant_badge, badge)) {
            if (b.kind == L::badge::my_team_kind) {
                uintptr_t label = 0;
                char name[64]{};
                if (!g_[G_MODE] || !m_.get(b.mode + L::mode::label, label) || !read_cstr(label, name, sizeof name))
                    return false;
                if (!std::strncmp(name, "my_team_career", 14) && (name[14] == 0 || name[14] == '_')) {
                    uint64_t slot = 0;
                    uint32_t mode_id = 0;
                    if (!s_.my_team_saved_badge || !current_badge_slot(m_, b.game, s_.image_lo, s_.image_hi, slot) ||
                        !m_.get(b.game + 0x2a8, mode_id) ||
                        !assigned_badge(m_, customisation_system(), mode_id, slot, badge))
                        return false;
                }
            }
            unsigned table = badge & 0xff, index = badge >> 16;
            if (index == L::badge::badge_none)
                n = std::snprintf(path, cap, "%s", L::badge::render_default);
            else if (table <= L::badge::max_kind)
                n = std::snprintf(path, cap, "%s%s %u", L::badge::render_prefix, L::badge::table_names[table], index);
        }
        if (n > 0 && size_t(n) < cap)
            return true;
        path[0] = 0;
        return false;
    }

    // The driver name the game shows for a car (Sess_Get_CarDriverName): the player name for a human-driven car, else
    // the display name of the driver record the car's session entrant points at. UTF-8, cut at a character boundary to
    // fit cap - 1 bytes.
    bool read_driver_name(uint32_t vehicle, char *out, size_t cap)
    {
        namespace D = L::driver;
        if (!active() || !g_[G_DRIVER_NAMES] || !s_.key_game_mode || !s_.key_team_tables || !s_.key_session_component ||
            vehicle >= MAX_CARS || cap < 2)
            return false;
        uintptr_t vh = find(c_s1_, s_.key_session_component), pv[2];
        size_t pn = 0;
        if (!vh || !m_.read(vh + D::participants, pv, sizeof pv) || !vec_ok(pv[0], pv[1], 8, D::max_participants, pn) ||
            vehicle >= pn)
            return false;
        uintptr_t parts[D::max_participants], participant = 0;
        if (!m_.read(pv[0], parts, pn * 8))
            return false;
        for (size_t i = 0; i < pn && !participant; i++) {
            uintptr_t phys = 0, car = 0;
            uint8_t vid = 0xff;
            if (parts[i] && m_.get(parts[i] + D::participant_physics, phys) && phys &&
                m_.get(phys + D::physics_car, car) && car && m_.get(car + L::car::vehicle_id, vid) && vid == vehicle)
                participant = parts[i];
        }
        uint8_t flags = 0;
        if (!participant || !m_.get(participant + D::participant_flags, flags))
            return false;
        uintptr_t name = 0;
        if (flags & D::human_flag) {
            if (!m_.get(vh + D::player_name, name))
                return false;
        } else {
            uintptr_t gmc = find(c_mode_, s_.key_game_mode), mode = 0, game = 0, ev[2];
            size_t n = 0;
            if (!gmc || !m_.get(gmc + L::badge::mode, mode) || !mode || !m_.get(mode + L::badge::game, game) || !game ||
                !m_.read(game + L::badge::entrants, ev, sizeof ev) || !vec_ok(ev[0], ev[1], L::badge::entrant_stride, 512, n))
                return false;
            uintptr_t entrant = 0;
            for (size_t i = 0; i < n && !entrant; i++) {
                uint8_t vid = 0xff;
                if (m_.get(ev[0] + i * L::badge::entrant_stride + L::badge::entrant_vid, vid) && vid == vehicle)
                    entrant = ev[0] + i * L::badge::entrant_stride;
            }
            uint32_t ref = 0;
            if (!entrant || !m_.get(entrant + D::entrant_ref, ref) || (ref & 0xff) > D::max_domain)
                return false;
            uintptr_t es = find(c_teams_, s_.key_team_tables), table = 0, vec[2], rec = 0;
            size_t count = 0, id = ref >> 16;
            if (!es || !m_.get(es + D::sets + (ref & 0xff) * D::set_stride, table) || !table ||
                !m_.read(table + D::table_vec, vec, sizeof vec) || !vec_ok(vec[0], vec[1], 16, 0x10000, count) ||
                id >= count || !m_.get(vec[0] + id * 16 + D::slot_record, rec) || !rec || !m_.get(rec + D::record_name, name))
                return false;
        }
        char buf[D::max_name];
        size_t len = 0;
        if (!name || !read_text(name, buf, sizeof buf, len) || !len)
            return false;
        size_t keep = utf8_prefix(buf, len, cap - 1);
        if (keep == ~size_t(0) || !keep)
            return false;
        std::memcpy(out, buf, keep);
        out[keep] = 0;
        return true;
    }

private:
    // Copies the NUL-terminated string at p into out (at most cap - 1 bytes before the NUL); false when it is unreadable or
    // has no NUL within cap bytes.
    bool read_text(uintptr_t p, char *out, size_t cap, size_t &len)
    {
        size_t n = 0;
        while (n < cap) {
            size_t chunk = std::min<size_t>(cap - n, 0x1000 - ((p + n) & 0xfff));
            if (!m_.read(p + n, out + n, chunk))
                return false;
            for (size_t i = 0; i < chunk; i++)
                if (!out[n + i]) {
                    len = n + i;
                    return true;
                }
            n += chunk;
        }
        return false;
    }

    struct Slot {
        uintptr_t value = 0;
        size_t slot = ~size_t(0);
        double last_search = -1e9;
    };
    struct CarRef {
        uintptr_t ptr = 0;
        uint8_t id = 0;
        uint32_t is_ai = 1;
        uintptr_t eng = 0, gb = 0, ch = 0, st = 0, es = 0, aero = 0;
        uint8_t flags = 0;
        float throttle = 0, brake = 0, clutch = 0;
    };
    struct Cls {
        bool have = false, tpen = false;
        uint8_t status = 0xff, pos = 0;
        float penalty = 0;
    };
    struct Gap {
        bool leader = false, ahead = false;
        int32_t leader_ms = 0, ahead_ms = 0;
    };
    struct Pen {
        bool have = false;
        uint8_t warnings = 0, limits = 0, dt = 0, sg = 0;
    };
    struct Tyre {
        bool have = false;
        uint8_t visual = 0xff, actual = 0xff;
        char letter = 0;
    };

    Mem m_;
    Sites s_;
    Gates g_;
    double now_ = 0, last_now_ = -1e18;
    double t_scan_ = -1e9, t_ctrl_ = -1e9, t_cls_ = -1e9, t_prog_ = -1e9, t_timing_ = -1e9, t_pit_ = -1e9, t_pen_ = -1e9,
           t_tyre_ = -1e9, t_flags_ = -1e9, t_mode_ = -1e9, t_track_ = -1e9, t_cars_ = -1e9;
    const char *stop_ = "";

    uintptr_t slots_[MAX_CARS] = {};
    uint32_t slot_count_ = 0;
    int n_cars_ = 0;
    CarRef refs_[MAX_CARS];
    bool field_ers_ = false;
    size_t entrants_ = 0;

    Slot c_results_, c_spectator_, c_timing_, c_pit_, c_sc_, c_log_, c_s1_, c_tyres_, c_marshal_, c_red_, c_mode_, c_teams_;
    Slot c_customisation_;
    uintptr_t customisation_key_ = 0;
    double customisation_scan_ = -2.0;

    SysRec sys_[MAX_CARS];
    ErsRec ers_[MAX_CARS];
    bool prog_have_[MAX_CARS] = {};
    Progress prog_[MAX_CARS];
    Cls cls_[MAX_CARS];
    bool cls_ok_ = false;
    uint32_t order_[MAX_CARS] = {};
    uint32_t classified_ = 0;
    Gap gap_[MAX_CARS];
    uint8_t pit_[MAX_CARS];
    Pen pen_[MAX_CARS];
    Tyre tyre_[MAX_CARS];
    float sector_end_[8] = {};
    int sector_count_ = 0;
    float sector_track_ = 0;
    bool sectors_ok_ = false;
    uint8_t yellow_sectors_ = 0;
    int zones_ = 0, zones_red_ = 0, yellow_ = 0;
    bool zones_ok_ = false;
    uint8_t sc_raw_ = 0xff, red_raw_ = 0xff;
    uint8_t game_mode_ = 0;
    bool game_mode_ok_ = false;
    char track_name_[48] = {};
    bool track_name_ok_ = false;
    float track_length_ = 0;
    bool track_length_ok_ = false;
    uint32_t type_ = 0;
    bool type_ok_ = false;
    int ctrl_hits_ = -1, ctrl_vid_ = -1;
    bool spect_found_ = false, spect_read_ = false;
    uint8_t spect_ui_ = 0, spect_active_ = 0, spect_vehicle_ = 0xff;
    uint32_t spect_gap_ = 0;
    uint8_t pit_status_[MAX_CARS] = {};
    uint8_t pit_stops_[MAX_CARS] = {};
    bool pit_lane_ok_[MAX_CARS] = {};
    bool retire_from_bus_ = false;
    uint32_t player_gen_ = 0, subject_gen_ = 0;
    uintptr_t player_ptr_ = 0, subject_ptr_ = 0;
    int player_ = -1, watched_ = -1;
    Observed observed_[MAX_CARS];
    bool observed_valid_ = false;
    std::vector<Event> events_;

    void reset()
    {
        std::memset(&ses, 0, sizeof ses);
        std::memset(cars, 0, sizeof cars);
        std::memset(pit_, 0xff, sizeof pit_);
        std::memset(pit_lane_ok_, 0, sizeof pit_lane_ok_);
        reset_timers();
    }

    void reset_timers()
    {
        t_scan_ = t_ctrl_ = t_cls_ = t_prog_ = t_timing_ = t_pit_ = t_pen_ = t_tyre_ = t_flags_ = t_mode_ = t_track_ = t_cars_ = -1e9;
    }

    bool due(double &t, double period)
    {
        if (now_ - t < period - 1e-9 && now_ >= t)
            return false;
        t = now_;
        return true;
    }

    void lose()
    {
        if (n_cars_ == 0 && slot_count_ == 0 && !player_ptr_)
            return;
        n_cars_ = 0;
        slot_count_ = 0;
        field_ers_ = false;
        entrants_ = 0;
        cls_ok_ = false;
        classified_ = 0;
        observed_valid_ = false;
        events_.clear();
        c_results_ = c_spectator_ = c_timing_ = c_pit_ = c_sc_ = c_log_ = c_s1_ = c_tyres_ = c_marshal_ = c_red_ = c_mode_ =
            c_teams_ = c_customisation_ = Slot{};
        customisation_key_ = 0;
        customisation_scan_ = -2.0;
        if (player_ptr_) {
            player_ptr_ = 0;
            player_gen_++;
        }
        if (subject_ptr_) {
            subject_ptr_ = 0;
            subject_gen_++;
        }
        player_ = watched_ = -1;
        for (auto &x : sys_)
            x = SysRec{};
        for (auto &x : ers_)
            x = ErsRec{};
        std::memset(prog_have_, 0, sizeof prog_have_);
        std::memset(present, 0, sizeof present);
        std::memset(&ses, 0, sizeof ses);
        std::memset(cars, 0, sizeof cars);
        for (auto &c : cls_)
            c = Cls{};
        for (auto &g : gap_)
            g = Gap{};
        for (auto &p : pen_)
            p = Pen{};
        for (auto &t : tyre_)
            t = Tyre{};
        std::memset(pit_, 0xff, sizeof pit_);
        std::memset(pit_lane_ok_, 0, sizeof pit_lane_ok_);
        ++seq;
    }

    bool read_cstr(uintptr_t p, char *out, size_t cap)
    {
        size_t n = 0;
        while (n + 1 < cap) {
            size_t chunk = std::min<size_t>(cap - 1 - n, 0x1000 - ((p + n) & 0xfff));
            if (!m_.read(p + n, out + n, chunk))
                return false;
            for (size_t i = 0; i < chunk; i++) {
                unsigned char c = static_cast<unsigned char>(out[n + i]);
                if (!c) {
                    out[n + i] = 0;
                    return n + i > 0;
                }
                if (c < 0x21 || c > 0x7e)
                    return false;
            }
            n += chunk;
        }
        return false;
    }

    struct BadgeRef {
        unsigned kind = 0;
        uintptr_t entrant = 0, key = 0, mode = 0, game = 0;
        uint8_t render = 0;
    };

    bool find_badge(uint32_t vehicle, BadgeRef &out)
    {
        if (!active() || !g_[G_BADGES] || !s_.key_game_mode || !s_.key_team_tables || vehicle >= MAX_CARS)
            return false;
        uintptr_t gmc = find(c_mode_, s_.key_game_mode);
        uintptr_t mode = 0, game = 0, ev[2];
        if (!gmc || !m_.get(gmc + L::badge::mode, mode) || !mode || !m_.get(mode + L::badge::game, game) || !game ||
            !m_.read(game + L::badge::entrants, ev, sizeof ev) || ev[1] <= ev[0] ||
            (ev[1] - ev[0]) % L::badge::entrant_stride || (ev[1] - ev[0]) / L::badge::entrant_stride > 512)
            return false;
        size_t n = (ev[1] - ev[0]) / L::badge::entrant_stride;
        for (size_t i = 0; i < n; i++) {
            uint8_t vid = 0xff;
            uint32_t team = 0;
            uintptr_t e = ev[0] + i * L::badge::entrant_stride;
            if (!m_.get(e + L::badge::entrant_vid, vid) || vid != vehicle)
                continue;
            if (!m_.get(e + L::badge::entrant_team, team))
                return false;
            unsigned kind = team & 0xff, index = team >> 16;
            if (kind > L::badge::max_kind)
                return false;
            out.kind = kind;
            out.entrant = e;
            out.mode = mode;
            out.game = game;
            if (kind == L::badge::my_team_kind) {
                // User-defined teams have no catalogue key. The HUD's rendered
                // fallback uses this entrant's badge key, not the team index.
                out.render = 1;
                return true;
            }
            uintptr_t tt = find(c_teams_, s_.key_team_tables), table = 0, vec[2], rec = 0;
            return tt && m_.get(tt + L::badge::tables + kind * 16, table) && table &&
                   m_.read(table + L::badge::table_vec, vec, sizeof vec) && vec[1] > vec[0] &&
                   index < (vec[1] - vec[0]) / 16 && m_.get(vec[0] + uintptr_t(index) * 16 + L::badge::slot_record, rec) && rec &&
                   m_.get(rec + L::badge::record_render, out.render) && m_.get(rec + L::badge::record_key, out.key) &&
                   (out.render || out.key);
        }
        return false;
    }

    uintptr_t customisation_system()
    {
        if (customisation_key_)
            return find(c_customisation_, customisation_key_);
        if (now_ >= customisation_scan_ && now_ - customisation_scan_ < 1.0)
            return 0;
        customisation_scan_ = now_;
        uintptr_t manager = 0, registry = 0, entries[2]{};
        if (!m_.get(s_.game_manager, manager) || !manager ||
            !m_.get(manager + L::registry::from_manager, registry) || !registry ||
            !m_.read(registry + L::registry::slots, entries, sizeof entries)) return 0;
        if (!entries[0] || (entries[0] & 7) || entries[1] <= entries[0] ||
            (entries[1] - entries[0]) % 16 || (entries[1] - entries[0]) / 16 > L::registry::max_slots) return 0;
        uintptr_t match = 0;
        for (size_t i = 0, count = (entries[1] - entries[0]) / 16; i < count; ++i) {
            uint8_t used = 0;
            uintptr_t key = 0, name_pointer = 0;
            char name[64]{};
            if (!m_.get(entries[1] + i, used) || !used || !m_.get(entries[0] + i * 16, key) || !key ||
                !m_.get(key + 8, name_pointer) || !read_cstr(name_pointer, name, sizeof name)) continue;
            if (std::strcmp(name, "playerCustomisationInfoSystem")) continue;
            if (match) return 0;
            match = key;
        }
        customisation_key_ = match;
        return match ? find(c_customisation_, match) : 0;
    }

    uintptr_t find(Slot &c, uintptr_t key)
    {
        uintptr_t cached = c.value;
        c.value = 0;
        uintptr_t mgr = 0, reg = 0, sl[2] = {};
        if (!key || !s_.game_manager || !m_.get(s_.game_manager, mgr) || !mgr ||
            !m_.get(mgr + L::registry::from_manager, reg) || !reg || !m_.read(reg + L::registry::slots, sl, sizeof sl))
            return 0;
        uintptr_t base = sl[0], info = sl[1];
        if (!base || (base & 7) || info <= base || (info - base) % 16 || (info - base) / 16 > L::registry::max_slots)
            return 0;
        size_t n = (info - base) / 16;
        uintptr_t kv[2];
        uint8_t used = 0;
        if (cached && c.slot < n && m_.read(base + c.slot * 16, kv, sizeof kv) && kv[0] == key && kv[1] == cached &&
            m_.get(info + c.slot, used) && used) {
            c.value = cached;
            return cached;
        }
        c.slot = ~size_t(0);
        if (now_ - c.last_search < 1.0 && now_ >= c.last_search)
            return 0;
        c.last_search = now_;
        uintptr_t chunk[2 * 64];
        for (size_t i = 0; i < n; i += 64) {
            size_t k = std::min<size_t>(64, n - i);
            if (!m_.read(base + i * 16, chunk, k * 16))
                return 0;
            for (size_t j = 0; j < k; j++) {
                uintptr_t value = chunk[2 * j + 1];
                if (chunk[2 * j] == key && value && !(value & 7) && m_.get(info + i + j, used) && used) {
                    c.value = value;
                    c.slot = i + j;
                    return value;
                }
            }
        }
        return 0;
    }

    uintptr_t system_of(uintptr_t car, uintptr_t value)
    {
        uintptr_t b[2];
        if (!value || (value & 7) || !m_.read(value, b, sizeof b) || (b[0] & 7) || !b[0] || b[1] != car)
            return 0;
        if (s_.image_hi && (b[0] < s_.image_lo || b[0] >= s_.image_hi))
            return 0;
        return value;
    }

    bool read_ref(uintptr_t p, uint32_t index, CarRef &c)
    {
        uint8_t b[L::car::hi - L::car::lo];
        if (!p || (p & 7) || !m_.read(p + L::car::lo, b, sizeof b))
            return false;
        uintptr_t lock = 0, svt = 0;
        if (!m_.get(p + L::car::lock, lock) || lock != p + L::car::lock_cs)
            return false;
        if (s_.vt_car_systems && (!m_.get(p + L::car::systems_vt, svt) || svt != s_.vt_car_systems))
            return false;
        if (field<uint8_t>(b, L::car::lo, L::car::vehicle_id) != index)
            return false;
        c.ptr = p;
        c.id = uint8_t(index);
        c.is_ai = field<uint32_t>(b, L::car::lo, L::car::is_ai);
        c.eng = system_of(p, field<uintptr_t>(b, L::car::lo, L::car::engine));
        c.gb = system_of(p, field<uintptr_t>(b, L::car::lo, L::car::gearbox));
        c.ch = system_of(p, field<uintptr_t>(b, L::car::lo, L::car::chassis));
        c.st = system_of(p, field<uintptr_t>(b, L::car::lo, L::car::steering));
        c.es = system_of(p, field<uintptr_t>(b, L::car::lo, L::car::ers));
        c.aero = system_of(p, field<uintptr_t>(b, L::car::lo, L::car::f1_aero));
        c.flags = field<uint8_t>(b, L::car::lo, L::car::input_flags);
        c.throttle = field<float>(b, L::car::lo, L::car::throttle);
        c.brake = field<float>(b, L::car::lo, L::car::brake);
        c.clutch = field<float>(b, L::car::lo, L::car::clutch);
        return true;
    }

    bool racer(unsigned id) const
    {
        const CarRef *r = ref_of(id);
        return r && (r->es || (!field_ers_ && id < entrants_));
    }

    const CarRef *ref_of(unsigned id) const
    {
        for (int i = 0; i < n_cars_; i++)
            if (refs_[i].id == id)
                return &refs_[i];
        return nullptr;
    }

    void scan(const uintptr_t *slots, uint32_t count)
    {
        slot_count_ = count;
        std::memcpy(slots_, slots, count * sizeof(uintptr_t));
        n_cars_ = 0;
        field_ers_ = false;
        for (uint32_t i = 0; i < count; i++) {
            CarRef c;
            if (read_ref(slots_[i], i, c)) {
                refs_[n_cars_++] = c;
                field_ers_ = field_ers_ || c.es;
            }
        }
    }

    bool vec_ok(uintptr_t b, uintptr_t e, size_t stride, size_t max, size_t &n)
    {
        if (!b || (b & 7) || e < b || (e - b) % stride || (e - b) / stride > max)
            return false;
        n = (e - b) / stride;
        return true;
    }

    void read_controllers()
    {
        ctrl_hits_ = -1;
        ctrl_vid_ = -1;
        uintptr_t sys = 0, vt = 0, cv = 0, sets = 0;
        uint8_t built = 0;
        if (!s_.controllers || !s_.vt_controllers || !m_.get(s_.controllers, sys) || !sys || (sys & 7) ||
            !m_.get(sys, vt) || vt != s_.vt_controllers)
            return;
        ctrl_hits_ = 0;
        uint32_t c[3];
        if (!m_.get(sys + L::controllers::built, built) || !built || !m_.get(sys + L::controllers::count_vec, cv) || !cv ||
            (cv & 7) || !m_.read(cv + L::controllers::count_lo, c, sizeof c) ||
            !m_.get(sys + L::controllers::sets, sets) || !sets || (sets & 7))
            return;
        size_t n = uint8_t((c[2] - c[0]) >> 3);
        uintptr_t set[MAX_CARS];
        if (!n || n > MAX_CARS || !m_.read(sets, set, n * sizeof(uintptr_t)))
            return;
        for (size_t i = 0; i < n; i++) {
            uint8_t b[L::controllers::set_vehicle + 1 - L::controllers::set_human];
            if (!set[i] || (set[i] & 7) || !m_.read(set[i] + L::controllers::set_human, b, sizeof b))
                continue;
            uintptr_t hc = field<uintptr_t>(b, L::controllers::set_human, L::controllers::set_human), hvt = 0;
            if (!hc || (hc & 7) || field<uint8_t>(b, L::controllers::set_human, L::controllers::set_vehicle) != i ||
                !m_.get(hc, hvt) || hvt != s_.vt_human)
                continue;
            ctrl_hits_++;
            ctrl_vid_ = int(i);
        }
        if (ctrl_hits_ != 1)
            ctrl_vid_ = -1;
    }

    void read_spectate()
    {
        spect_found_ = spect_read_ = false;
        uintptr_t d = find(c_spectator_, s_.key_spectator);
        uint8_t b[L::spectate::hi - L::spectate::lo];
        spect_found_ = d != 0;
        if (!d || !m_.read(d + L::spectate::lo, b, sizeof b))
            return;
        spect_read_ = true;
        spect_ui_ = field<uint8_t>(b, L::spectate::lo, L::spectate::ui);
        spect_active_ = field<uint8_t>(b, L::spectate::lo, L::spectate::active);
        spect_vehicle_ = field<uint8_t>(b, L::spectate::lo, L::spectate::vehicle);
        spect_gap_ = field<uint32_t>(b, L::spectate::lo, L::spectate::gap);
    }

    void job_cls()
    {
        for (auto &c : cls_)
            c = Cls{};
        cls_ok_ = false;
        classified_ = 0;
        uintptr_t r = find(c_results_, s_.key_results), v[2];
        size_t ne = 0;
        if (!r || !m_.read(r + L::results::vec_begin, v, sizeof v) || !vec_ok(v[0], v[1], 8, MAX_ENTRIES, ne) || !ne)
            return;
        entrants_ = ne;
        uintptr_t ptr[MAX_ENTRIES];
        if (!m_.read(v[0], ptr, ne * 8))
            return;
        int8_t owner[MAX_ENTRIES + 2];
        std::memset(owner, -1, sizeof owner);
        int maxpos = 0;
        bool dup = false;
        for (unsigned vid = 0; vid < MAX_CARS && vid < ne; vid++) {
            uintptr_t p = ptr[vid];
            uint8_t b[L::results::hi];
            if (!racer(vid) || !p || (p & 7) || !m_.read(p, b, sizeof b) || b[L::results::vehicle] != vid)
                continue;
            Cls &c = cls_[vid];
            c.have = true;
            uint8_t st = b[L::results::status];
            c.status = st <= 7 ? st : 0xff;
            uint64_t bits = field<uint64_t>(b, 0, L::results::penalty_a) ^ field<uint64_t>(b, 0, L::results::penalty_b);
            double d;
            std::memcpy(&d, &bits, sizeof d);
            if (std::isfinite(d) && d >= 0.0 && d <= 3600.0) {
                c.tpen = true;
                c.penalty = float(d);
            }
            uint32_t pos = (field<uint32_t>(b, 0, L::results::pos_a) ^ field<uint32_t>(b, 0, L::results::pos_b)) + 1;
            if (pos < 1 || pos > ne || pos > MAX_ENTRIES)
                continue;
            if (owner[pos] >= 0) {
                dup = true;
                continue;
            }
            owner[pos] = int8_t(vid);
            c.pos = uint8_t(pos);
            maxpos = std::max(maxpos, int(pos));
        }
        if (dup) {
            for (auto &c : cls_)
                c.pos = 0;
            return;
        }
        for (int p = 1; p <= maxpos; p++)
            if (owner[p] >= 0 && classified_ < MAX_CARS)
                order_[classified_++] = uint32_t(owner[p]);
        cls_ok_ = classified_ > 0;
        if (cls_ok_)
            cls_stamp_++;
    }

    bool read_progress_entry(uintptr_t entry, Progress &p)
    {
        uint8_t b[L::progress::hi - L::progress::lo];
        if (!entry || (entry & 7) || !m_.read(entry + L::progress::lo, b, sizeof b))
            return false;
        auto s16 = [&](uintptr_t off) { return int(int16_t(field<uint16_t>(b, L::progress::lo, off))); };
        p.lap = int(int16_t(field<uint16_t>(b, L::progress::lo, L::progress::lap_a) ^
                            field<uint16_t>(b, L::progress::lo, L::progress::lap_b)));
        p.plap = int(int16_t(field<uint16_t>(b, L::progress::lo, L::progress::plap_a) ^
                             field<uint16_t>(b, L::progress::lo, L::progress::plap_b))) +
                 s16(L::progress::plap_delta);
        p.total = from_bits(field<uint32_t>(b, L::progress::lo, L::progress::total_a) ^
                            field<uint32_t>(b, L::progress::lo, L::progress::total_b));
        return within(p.total, -2.0e4f, 2.0e7f) && p.lap >= -2 && p.lap <= 1000 && p.plap >= -2 && p.plap <= 1000;
    }

    void job_progress()
    {
        std::memset(prog_have_, 0, sizeof prog_have_);
        uintptr_t mgr = 0, sys = 0, v[2];
        size_t n = 0;
        uintptr_t ptr[MAX_ENTRIES];
        if (!s_.game_manager || !m_.get(s_.game_manager, mgr) || !mgr ||
            !m_.get(mgr + (s_.progress_from_manager ? s_.progress_from_manager : L::progress::from_manager), sys) || !sys || !m_.read(sys + L::progress::vec_begin, v, sizeof v) ||
            !vec_ok(v[0], v[1], 8, MAX_ENTRIES, n) || !n || !m_.read(v[0], ptr, n * 8))
            return;
        for (unsigned vid = 0; vid < MAX_CARS && vid < n; vid++) {
            if (!ref_of(vid))
                continue;
            Progress p;
            if (read_progress_entry(ptr[vid], p)) {
                prog_[vid] = p;
                prog_have_[vid] = true;
            }
        }
    }

    void job_timing()
    {
        for (auto &g : gap_)
            g = Gap{};
        uintptr_t t = find(c_timing_, s_.key_timing), v[2];
        size_t n = 0;
        static uint8_t buf[MAX_ENTRIES * L::timing::stride];
        if (!t || !m_.read(t + L::timing::vec_begin, v, sizeof v) || !vec_ok(v[0], v[1], L::timing::stride, MAX_ENTRIES, n) ||
            (n && !m_.read(v[0], buf, n * L::timing::stride)))
            return;
        bool seen[MAX_CARS] = {};
        for (size_t i = 0; i < n; i++) {
            const uint8_t *e = buf + i * L::timing::stride;
            uint8_t vid = e[L::timing::vehicle];
            if (vid >= MAX_CARS || seen[vid])
                continue;
            seen[vid] = true;
            Gap &g = gap_[vid];
            int64_t lm = field<int64_t>(e, 0, L::timing::leader_ms), am = field<int64_t>(e, 0, L::timing::ahead_ms);
            g.leader = e[L::timing::leader_valid] != 0 && lm >= 0 && lm <= INT32_MAX;
            g.ahead = e[L::timing::ahead_valid] != 0 && am >= 0 && am <= INT32_MAX;
            g.leader_ms = g.leader ? int32_t(lm) : 0;
            g.ahead_ms = g.ahead ? int32_t(am) : 0;
        }
        uintptr_t sc = find(c_sc_, s_.key_safety_car);
        uint8_t b = 0xff;
        sc_raw_ = sc && m_.get(sc + L::safety_car::state, b) && b <= 3 ? b : 0xff;
    }

    void job_pit()
    {
        std::memset(pit_, 0xff, sizeof pit_);
        std::memset(pit_lane_ok_, 0, sizeof pit_lane_ok_);
        uintptr_t s = find(c_pit_, s_.key_pit);
        uintptr_t rec[MAX_CARS * 2];
        if (!s || !m_.read(s + L::pit::records, rec, sizeof rec))
            return;
        uintptr_t lane = 0;
        if (g_[G_PIT_LANE] && (!m_.get(s + L::pit::lane_object, lane) || !lane))
            lane = 0;
        for (unsigned vid = 0; vid < MAX_CARS; vid++) {
            uintptr_t q = rec[vid * 2];
            uint32_t st = 0;
            if (!racer(vid) || !q || (q & 3) || !m_.get(q + L::pit::state, st) || st > 15)
                continue;
            pit_[vid] = uint8_t(st);
            uintptr_t flags = 0;
            uint8_t in_lane = 0;
            uint32_t stops = 0;
            if (g_[G_PIT_LANE] && lane && vid < L::pit::max_slots &&
                m_.get(lane + L::pit::lane_flags + vid * L::pit::lane_flag_stride, flags) && flags &&
                m_.get(flags + L::pit::in_lane, in_lane) && m_.get(q + L::pit::stops, stops)) {
                pit_status_[vid] = !in_lane ? 0 : st == 4 ? 2 : 1;
                pit_stops_[vid] = uint8_t(stops & 0xff);
                pit_lane_ok_[vid] = true;
            }
        }
    }

    void job_penalties()
    {
        for (auto &p : pen_)
            p = Pen{};
        uintptr_t z = find(c_log_, s_.key_penalty_log), v[2];
        size_t n = 0;
        if (z && m_.read(z + L::penalty_log::vec_begin, v, sizeof v) && vec_ok(v[0], v[1], L::penalty_log::stride, 1024, n)) {
            static uint8_t chunk[64 * L::penalty_log::stride];
            bool ok = true;
            uint8_t warn[MAX_CARS] = {}, lim[MAX_CARS] = {};
            for (size_t i = 0; i < n && ok; i += 64) {
                size_t c = std::min<size_t>(64, n - i);
                if (!m_.read(v[0] + i * L::penalty_log::stride, chunk, c * L::penalty_log::stride)) {
                    ok = false;
                    break;
                }
                for (size_t j = 0; j < c; j++) {
                    const uint8_t *r = chunk + j * L::penalty_log::stride;
                    uint8_t vid = r[L::penalty_log::vehicle], kind = r[L::penalty_log::kind],
                            inf = r[L::penalty_log::infringement];
                    if (vid < MAX_CARS && kind == L::penalty_log::warning) {
                        if (warn[vid] < 255)
                            warn[vid]++;
                        if (inf <= 0x1d && ((L::penalty_log::track_limits_mask >> inf) & 1u) && lim[vid] < 255)
                            lim[vid]++;
                    }
                }
            }
            if (ok)
                for (unsigned i = 0; i < MAX_CARS; i++) {
                    pen_[i].have = true;
                    pen_[i].warnings = warn[i];
                    pen_[i].limits = lim[i];
                }
        }
        uintptr_t s1 = find(c_s1_, s_.key_session_component), obj[3];
        if (s1 && m_.read(s1 + L::misconduct::drive_through, obj, sizeof obj) && obj[0] && obj[2] && !(obj[0] & 7) &&
            !(obj[2] & 7)) {
            uintptr_t dv[2], sv[2];
            size_t dn = 0, sn = 0;
            if (m_.read(obj[0] + L::misconduct::vec_begin, dv, sizeof dv) &&
                vec_ok(dv[0], dv[1], L::misconduct::dt_stride, 256, dn) &&
                m_.read(obj[2] + L::misconduct::vec_begin, sv, sizeof sv) &&
                vec_ok(sv[0], sv[1], L::misconduct::sg_stride, 256, sn)) {
                size_t dc = std::min<size_t>(dn, MAX_CARS), sc = std::min<size_t>(sn, MAX_CARS);
                uint8_t drec[MAX_CARS * L::misconduct::dt_stride], srec[MAX_CARS * L::misconduct::sg_stride];
                if ((!dc || m_.read(dv[0], drec, dc * L::misconduct::dt_stride)) &&
                    (!sc || m_.read(sv[0], srec, sc * L::misconduct::sg_stride)))
                    for (unsigned vid = 0; vid < MAX_CARS; vid++) {
                        Pen &p = pen_[vid];
                        p.have = true;
                        if (vid < dc) {
                            const uint8_t *r = drec + vid * L::misconduct::dt_stride;
                            p.dt = uint8_t((r[0x08] != 0xff) + (r[0x10] != 0xff) + (r[0x18] != 0xff) + (r[0x20] != 0xff));
                        }
                        if (vid < sc)
                            p.sg = srec[vid * L::misconduct::sg_stride + 3];
                    }
            }
        }
    }

    void read_tyre(uintptr_t V, Tyre &t)
    {
        uint8_t b[L::tyres::hi - L::tyres::lo];
        auto F = [&](uintptr_t off) { return off - L::tyres::lo; };
        if (!V || (V & 7) || !m_.read(V + L::tyres::lo, b, sizeof b))
            return;
        uint64_t version = field<uint64_t>(b, L::tyres::lo, L::tyres::version), count = field<uint64_t>(b, L::tyres::lo, L::tyres::count),
                 built_for = field<uint64_t>(b, L::tyres::lo, L::tyres::built_for), built = field<uint64_t>(b, L::tyres::lo, L::tyres::built);
        uintptr_t sb = field<uintptr_t>(b, L::tyres::lo, L::tyres::set_begin), se = field<uintptr_t>(b, L::tyres::lo, L::tyres::set_end);
        if (!built || built_for != version)
            return;
        uint32_t id = 0;
        if (count == 0 || count > 5 || !sb || se < sb || se - sb < L::tyres::set_stride || !m_.get(se - 0x6c, id))
            return;
        const uint8_t *list = b + F(L::tyres::list), *desc = nullptr;
        for (size_t i = 0; i < count && !desc; i++)
            if (field<uint32_t>(list, 0, i * L::tyres::desc_stride) == id)
                desc = list + i * L::tyres::desc_stride;
        uint32_t fallback = id >= 3 ? 4 : 1;
        for (size_t i = 0; i < count && !desc; i++)
            if (field<uint32_t>(list, 0, i * L::tyres::desc_stride) == fallback)
                desc = list + i * L::tyres::desc_stride;
        if (!desc)
            return;
        uintptr_t actual = field<uintptr_t>(desc, 0, L::tyres::desc_physical),
                  visual = field<uintptr_t>(desc, 0, L::tyres::desc_visual);
        uint8_t vr[0x54];
        if (!visual || !m_.read(visual, vr, sizeof vr))
            return;
        int32_t c = field<int32_t>(vr, 0, L::tyres::visual_compound);
        uint8_t act = 0xff;
        if (c < -1 || c > 8 || (actual && !m_.get(actual, act)))
            return;
        t.have = true;
        t.visual = vr[L::tyres::visual_stat_id];
        t.actual = act;
        t.letter = checked_tyre_letter(vr[L::tyres::visual_stat_id], c);
    }

    void job_tyres()
    {
        for (auto &t : tyre_)
            t = Tyre{};
        uintptr_t ts = find(c_tyres_, s_.key_tyres), v[2];
        size_t n = 0;
        uintptr_t ptr[MAX_ENTRIES];
        if (!ts || !m_.read(ts + L::tyres::per_vehicle, v, sizeof v) || !vec_ok(v[0], v[1], 8, MAX_ENTRIES, n) ||
            (n && !m_.read(v[0], ptr, n * 8)))
            return;
        for (unsigned vid = 0; vid < MAX_CARS && vid < n; vid++)
            if (racer(vid))
                read_tyre(ptr[vid], tyre_[vid]);
    }

    void job_flags()
    {
        if (s_.sector_block && sector_block_contiguous() && !sectors_ok_) {
            uint8_t b[L::track::sector_block];
            if (m_.read(s_.sector_block, b, sizeof b))
                sectors_ok_ = sector_table_from(b);
        }
        zones_ok_ = false;
        zones_ = zones_red_ = yellow_ = 0;
        yellow_sectors_ = 0;
        uintptr_t mz = find(c_marshal_, s_.key_marshal), v[2];
        size_t n = 0;
        static uint8_t zbuf[L::marshal::max_zones * L::marshal::stride];
        if (mz && m_.read(mz + L::marshal::vec_begin, v, sizeof v) &&
            vec_ok(v[0], v[1], L::marshal::stride, L::marshal::max_zones, n) &&
            (!n || m_.read(v[0], zbuf, n * L::marshal::stride))) {
            bool ok = true;
            uint8_t ysec = 0;
            int yellow = 0, red = 0;
            for (size_t i = 0; i < n; i++) {
                uintptr_t z = field<uintptr_t>(zbuf, 0, i * L::marshal::stride + L::marshal::zone);
                uint8_t zb[0x18];
                int32_t flag = -1;
                if (!z || (z & 7) || !m_.read(z, zb, sizeof zb) ||
                    (flag = field<int32_t>(zb, 0, L::marshal::zone_flag)) < 0 || flag > 4) {
                    ok = false;
                    break;
                }
                red += flag == 4;
                if (flag != 3)
                    continue;
                yellow++;
                uintptr_t def = field<uintptr_t>(zb, 0, L::marshal::zone_def);
                float start = -1.f;
                if (sectors_ok_ && def && !(def & 3) && m_.get(def + L::marshal::def_start, start) && std::isfinite(start) &&
                    start >= 0.f && start <= 1.f) {
                    int s = sector_of(sector_end_, sector_count_, sector_track_ * start);
                    if (s)
                        ysec |= uint8_t(1u << (s - 1));
                }
            }
            if (ok) {
                zones_ok_ = true;
                zones_ = int(n);
                zones_red_ = red;
                yellow_ = yellow;
                yellow_sectors_ = ysec;
            }
        }
        uintptr_t rf = find(c_red_, s_.key_red_flag);
        uint8_t r = 0xff;
        red_raw_ = rf && m_.get(rf + L::red_flag::state, r) && r <= 2 ? r : 0xff;
    }

    bool sector_block_contiguous() const
    {
        return s_.sector_count == s_.sector_block + L::track::sector_count &&
               (!s_.track_length || s_.track_length == s_.sector_block + L::track::sector_length);
    }

    bool sector_table_from(const uint8_t *b)
    {
        uint32_t count = field<uint32_t>(b, 0, L::track::sector_count);
        float len = field<float>(b, 0, L::track::sector_length);
        if (count < 1 || count > 8 || !within(len, 500.f, 20000.f))
            return false;
        float prev = 0;
        for (uint32_t i = 0; i < count; i++) {
            float e = field<float>(b, 0, L::track::sector_end + i * 4);
            if (!std::isfinite(e) || e <= 0 || e < prev)
                return false;
            sector_end_[i] = prev = e;
        }
        sector_count_ = int(count);
        sector_track_ = len;
        return true;
    }

    void job_mode()
    {
        game_mode_ok_ = false;
        game_mode_ = 0;
        uintptr_t gmc = find(c_mode_, s_.key_game_mode), mode = 0, label = 0;
        if (!gmc || !m_.get(gmc + L::mode::from_context, mode) || !mode || (mode & 7) ||
            !m_.get(mode + L::mode::label, label) || !label)
            return;
        char b[32];
        size_t n = 0;
        while (n + 1 < sizeof b) {
            size_t chunk = std::min<size_t>(sizeof b - 1 - n, 0x1000 - ((label + n) & 0xfff));
            if (!m_.read(label + n, b + n, chunk))
                return;
            bool end = false;
            for (size_t i = 0; i < chunk && !end; i++)
                end = !b[n + i];
            n += chunk;
            if (end)
                break;
        }
        b[sizeof b - 1] = 0;
        size_t len = std::strlen(b);
        if (!len || len >= sizeof b - 1)
            return;
        for (size_t i = 0; i < len; i++) {
            char ch = b[i];
            if (!((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '_'))
                return;
        }
        game_mode_ok_ = true;
        game_mode_ = !std::strcmp(b, "time_trial") ? SWOFTY_LIVE_MODE_TIME_TRIAL
                     : !std::strncmp(b, "grand_prix", 10) ? SWOFTY_LIVE_MODE_GRAND_PRIX
                                                          : SWOFTY_LIVE_MODE_UNKNOWN;
    }

    void job_track()
    {
        float len = 0;
        track_length_ok_ = s_.track_length && m_.get(s_.track_length, len) && within(len, 500.f, 20000.f);
        track_length_ = track_length_ok_ ? len : 0;
        uint32_t type = 0;
        type_ok_ = s_.session_type && m_.get(s_.session_type, type) && type <= 64;
        type_ = type_ok_ ? type : 0;
        uintptr_t w = 0, a = 0, t = 0, str = 0;
        track_name_ok_ = g_[G_TRACK_NAME] && s_.track_world && m_.get(s_.track_world, w) && w && m_.get(w + L::track::world_to_a, a) && a &&
                         m_.get(a + L::track::a_to_b, t) && t && m_.get(t + L::track::b_to_name, str) && str &&
                         read_name(str);
        if (!track_name_ok_)
            track_name_[0] = 0;
    }

    bool read_name(uintptr_t p)
    {
        char tmp[sizeof track_name_];
        size_t n = 0;
        while (n + 1 < sizeof tmp) {
            size_t chunk = std::min<size_t>(sizeof tmp - 1 - n, 0x1000 - ((p + n) & 0xfff));
            if (!m_.read(p + n, tmp + n, chunk))
                return false;
            for (size_t i = 0; i < chunk; i++) {
                unsigned char c = static_cast<unsigned char>(tmp[n + i]);
                if (!c) {
                    std::memcpy(track_name_, tmp, n + i + 1);
                    return n + i > 0;
                }
                if (c < 0x20 || c == 0x7f)
                    return false;
            }
            n += chunk;
        }
        return false;
    }

    static bool obf_float(const Mem &m, uintptr_t at, float &out)
    {
        uint32_t a, b;
        if (!m.get(at, a) || !m.get(at + 8, b))
            return false;
        out = from_bits(a ^ b);
        return std::isfinite(out);
    }

    void read_car_systems(bool force)
    {
        bool every = due(t_cars_, 0.1) || force;
        int subject = subject_index();
        for (int i = 0; i < n_cars_; i++) {
            CarRef &r = refs_[i];
            if (!every && int(r.id) != subject && int(r.id) != player_)
                continue;
            CarRef fresh;
            if (read_ref(slots_[r.id], r.id, fresh))
                r = fresh;
            if (g_[G_CAR_SYSTEMS])
                read_sys(r, sys_[r.id]);
            if (g_[G_ERS])
                read_ers(r, ers_[r.id]);
        }
    }

    void read_sys(const CarRef &c, SysRec &o)
    {
        o = SysRec{};
        uint32_t gi = 0;
        bool gear_ok = c.gb && m_.get(c.gb + L::gearbox::gear_index, gi) && gi <= 10;
        if (gear_ok) {
            o.gear = int(gi) - 1;
            o.have |= SWOFTY_LIVE_HAVE_GEAR;
        }
        if (within(c.throttle, -0.05f, 1.05f) && within(c.brake, -0.05f, 1.05f)) {
            bool swap = gear_ok && gi == 0 && (c.flags & 0x88) == 0x08;
            o.throttle = std::clamp(swap ? c.brake : c.throttle, 0.f, 1.f);
            o.brake = std::clamp(swap ? c.throttle : c.brake, 0.f, 1.f);
            o.have |= SWOFTY_LIVE_HAVE_PEDALS;
        }
        if (within(c.clutch, -0.05f, 1.05f)) {
            o.clutch = std::clamp(c.clutch, 0.f, 1.f);
            o.have |= SWOFTY_LIVE_HAVE_CLUTCH;
        }
        float rate = 0, limiter = 0;
        if (c.eng && m_.get(c.eng + L::engine::rate, rate) && within(rate, 0.f, 3000.f)) {
            o.rpm = rate * 9.5492966f;
            if (c.eng && m_.get(c.eng + L::engine::limiter, limiter) && within(limiter, 300.f, 3000.f)) {
                o.rpm_limit = limiter * 9.5492966f;
                o.have |= SWOFTY_LIVE_HAVE_RPM;
            }
        }
        if (c.ch) {
            float g[2];
            if (m_.read(c.ch + L::chassis::g_lat, g, sizeof g) && within(g[0], -12.f, 12.f) && within(g[1], -12.f, 12.f)) {
                o.g_lat = g[0];
                o.g_long = g[1];
                o.have |= SWOFTY_LIVE_HAVE_G;
            }
            uintptr_t body = 0, state = 0;
            float vel[3];
            if (m_.get(c.ch + L::chassis::body, body) && body && m_.get(body + L::chassis::body_state, state) && state &&
                m_.read(state + L::chassis::state_velocity, vel, sizeof vel) && within(vel[0], -200.f, 200.f) &&
                within(vel[1], -200.f, 200.f) && within(vel[2], -200.f, 200.f)) {
                float s = std::sqrt(vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]);
                if (s <= 160.f) {
                    o.speed = s;
                    o.have |= SWOFTY_LIVE_HAVE_SPEED;
                    if (g_[G_WORLD]) {
                        float pos[3];
                        if (m_.read(state + L::chassis::state_position, pos, sizeof pos) && within(pos[0], -1e6f, 1e6f) &&
                            within(pos[1], -1e6f, 1e6f) && within(pos[2], -1e6f, 1e6f)) {
                            std::memcpy(o.pos, pos, sizeof pos);
                            std::memcpy(o.vel, vel, sizeof vel);
                            o.have |= SWOFTY_LIVE_HAVE_WORLD;
                        }
                    }
                }
            }
        }
        float angle = 0, lock = 0;
        if (c.st && obf_float(m_, c.st + L::steering::angle, angle) && obf_float(m_, c.st + L::steering::lock, lock) &&
            within(lock, 0.05f, 1.6f) && within(angle, -1.7f, 1.7f)) {
            o.steer = std::clamp(-angle / lock, -1.f, 1.f);
            o.have |= SWOFTY_LIVE_HAVE_STEER;
        }
        if (g_[G_AERO] && c.aero) {
            uint32_t w = 0;
            if (m_.get(c.aero + L::aero::drs_request, w)) {
                o.drs = uint8_t(w & 1);
                o.have |= SWOFTY_LIVE_HAVE_DRS;
            }
        }
    }

    void read_ers(const CarRef &c, ErsRec &e)
    {
        e = ErsRec{};
        if (!c.es)
            return;
        e.cls = 1;
        uint8_t b[L::ers::live_hi - L::ers::live_lo];
        float cfg[2];
        if (m_.read(c.es + L::ers::live_lo, b, sizeof b)) {
            float energy = field<float>(b, L::ers::live_lo, L::ers::energy), hk = field<float>(b, L::ers::live_lo, L::ers::harvest_k),
                  hh = field<float>(b, L::ers::live_lo, L::ers::harvest_h), dp = field<float>(b, L::ers::live_lo, L::ers::deploy),
                  hl = field<float>(b, L::ers::live_lo, L::ers::harvest_lap), dl = field<float>(b, L::ers::live_lo, L::ers::deploy_lap),
                  ll = field<float>(b, L::ers::live_lo, L::ers::lap_limit);
            int32_t mode = -1;
            if (s_.ers_mode && s_.ers_mode != L::ers::mode)
                m_.get(c.es + s_.ers_mode, mode);
            else
                mode = field<int32_t>(b, L::ers::live_lo, L::ers::mode);
            if (within(energy, -1.0e5f, 2.0e7f)) {
                e.store = std::max(0.f, energy);
                e.have |= SWOFTY_LIVE_HAVE_ERS_STORE;
            }
            if (within(hk, -2.0e6f, 2.0e6f) && within(hh, -2.0e6f, 2.0e6f) && within(dp, -2.0e6f, 2.0e6f)) {
                e.harvest_k = hk;
                e.harvest_h = hh;
                e.deploy = dp;
                e.have |= SWOFTY_LIVE_HAVE_ERS_POWER;
            }
            if (mode >= 0 && mode <= 3) {
                e.mode = mode;
                e.have |= SWOFTY_LIVE_HAVE_ERS_MODE;
            }
            if (within(hl, -1.0e5f, 1.0e8f) && within(dl, -1.0e5f, 1.0e8f) && within(ll, 0.f, 1.0e8f)) {
                e.harvested_lap = hl;
                e.deployed_lap = dl;
                e.harvest_limit_lap = ll;
                e.have |= SWOFTY_LIVE_HAVE_ERS_LAP;
            }
        }
        if (m_.get(c.es + L::ers::max_deploy, cfg[0]) && within(cfg[0], 1.0e4f, 2.0e6f))
            e.max_deploy = cfg[0];
        if (m_.get(c.es + L::ers::capacity, cfg[1]) && within(cfg[1], 1.0e5f, 5.0e7f))
            e.capacity = cfg[1];
    }

    int find_player(bool &ai)
    {
        ai = false;
        int held = -1;
        for (int i = 0; i < n_cars_; i++)
            if (player_ptr_ && refs_[i].ptr == player_ptr_)
                held = i;
        int me = -1;
        if (g_[G_CONTROL] && ctrl_hits_ == 1 && (me = index_of(ctrl_vid_)) >= 0) {
        } else if (g_[G_CONTROL] && ctrl_hits_ >= 2)
            me = -1;
        else
            me = held;
        return me;
    }

    int index_of(int vid) const
    {
        for (int i = 0; i < n_cars_; i++)
            if (refs_[i].id == vid)
                return i;
        return -1;
    }

    int subject_index() const { return subject_vid_; }
    int subject_vid_ = -1;

    int watched_vid() const
    {
        if (!g_[G_SPECTATE] || !spect_read_ || !spect_ui_ || !spect_active_ || spect_vehicle_ >= slot_count_)
            return -1;
        return index_of(spect_vehicle_) >= 0 ? int(spect_vehicle_) : -1;
    }

    void compose()
    {
        bool ai = false;
        int me = find_player(ai);
        int new_player = me >= 0 ? int(refs_[me].id) : -1;
        if (me >= 0 ? refs_[me].ptr != player_ptr_ : player_ptr_ != 0)
            player_gen_++;
        player_ptr_ = me >= 0 ? refs_[me].ptr : 0;
        player_ = new_player;
        watched_ = watched_vid();
        int subject = watched_ >= 0 ? watched_ : player_;
        subject_vid_ = subject;
        uintptr_t sp = subject >= 0 ? refs_[index_of(subject)].ptr : 0;
        if (sp != subject_ptr_) {
            subject_gen_++;
            subject_ptr_ = sp;
        }

        std::memset(&ses, 0, sizeof ses);
        ses.version = SWOFTY_LIVE_VERSION;
        ses.size = sizeof ses;
        ses.player = player_;
        ses.watched = watched_;
        ses.subject = subject;
        ses.generation = player_gen_;
        ses.subject_generation = subject_gen_;
        ses.cars = uint32_t(n_cars_);
        ses.have |= SWOFTY_LIVE_SHAVE_CARS;
        if (g_[G_CONTROL] || me >= 0)
            ses.have |= SWOFTY_LIVE_SHAVE_PLAYER;
        if (g_[G_SPECTATE] && spect_read_) {
            ses.have |= SWOFTY_LIVE_SHAVE_WATCHED;
            if (g_[G_SPECTATE_GAP])
                ses.spectator_gap_mode = uint8_t(spect_gap_ > 255 ? 0 : spect_gap_);
        }
        unsigned racers = 0;
        for (int i = 0; i < n_cars_; i++)
            racers += racer(refs_[i].id);
        ses.racers = racers;
        if (type_ok_) {
            ses.type = game_mode_ok_ && game_mode_ == SWOFTY_LIVE_MODE_TIME_TRIAL ? 18 : type_;
            ses.have |= SWOFTY_LIVE_SHAVE_TYPE;
        }
        if (track_length_ok_) {
            ses.track_length_m = track_length_;
            ses.have |= SWOFTY_LIVE_SHAVE_TRACK_LENGTH;
        }
        if (track_name_ok_) {
            std::memcpy(ses.track_name, track_name_, sizeof ses.track_name);
            ses.have |= SWOFTY_LIVE_SHAVE_TRACK_NAME;
        }
        if (game_mode_ok_) {
            ses.game_mode = game_mode_;
            ses.have |= SWOFTY_LIVE_SHAVE_GAME_MODE;
        }
        if (sectors_ok_) {
            ses.sector_count = uint32_t(sector_count_);
            std::memcpy(ses.sector_end_m, sector_end_, sizeof sector_end_);
            ses.have |= SWOFTY_LIVE_SHAVE_SECTORS;
        }
        if (zones_ok_) {
            ses.yellow_sectors = yellow_sectors_;
            ses.have |= SWOFTY_LIVE_SHAVE_YELLOW;
        }
        if (red_raw_ != 0xff || zones_ok_) {
            uint8_t red = red_raw_ == 1 || red_raw_ == 2 ? red_raw_ : uint8_t(zones_ok_ && zones_ > 0 && zones_red_ == zones_ ? 1 : 0);
            ses.red_flag = red;
            ses.have |= SWOFTY_LIVE_SHAVE_RED_FLAG;
        }
        ses.safety_car = 0xff;
        if (sc_raw_ != 0xff) {
            ses.safety_car = ses.red_flag ? 0 : sc_raw_;
            ses.have |= SWOFTY_LIVE_SHAVE_SAFETY_CAR;
        }
        if (cls_ok_) {
            ses.classified = classified_;
            std::memcpy(ses.order, order_, sizeof order_);
            ses.have |= SWOFTY_LIVE_SHAVE_ORDER;
            const Cls &lead = cls_[order_[0]];
            ses.chequered = lead.have && lead.status == 3;
        }
        uint32_t road[MAX_CARS];
        float total[MAX_CARS];
        unsigned nroad = 0;
        for (int i = 0; i < n_cars_; i++) {
            unsigned id = refs_[i].id;
            if (!racer(id) || !prog_have_[id])
                continue;
            road[nroad] = id;
            total[nroad++] = prog_[id].total;
        }
        for (unsigned i = 1; i < nroad; i++)
            for (unsigned j = i; j > 0 && total[j] > total[j - 1]; j--) {
                std::swap(total[j], total[j - 1]);
                std::swap(road[j], road[j - 1]);
            }
        if (nroad) {
            std::memcpy(ses.road_order, road, nroad * sizeof(uint32_t));
            ses.on_track = nroad;
            ses.have |= SWOFTY_LIVE_SHAVE_ROAD_ORDER;
        }

        std::memset(cars, 0, sizeof cars);
        std::memset(present, 0, sizeof present);
        for (int i = 0; i < n_cars_; i++) {
            const CarRef &r = refs_[i];
            unsigned id = r.id;
            SwoftyLiveCar &c = cars[id];
            present[id] = true;
            c.version = SWOFTY_LIVE_VERSION;
            c.size = sizeof c;
            c.vehicle = id;
            c.participant = 0xffffffffu;
            c.have = SWOFTY_LIVE_HAVE_IDENT;
            c.flags = uint8_t((int(id) == player_ ? SWOFTY_LIVE_CAR_PLAYER : 0) |
                              (r.is_ai && (int(id) != player_ || ai) ? SWOFTY_LIVE_CAR_AI_DRIVING : 0) |
                              (int(id) == watched_ ? SWOFTY_LIVE_CAR_WATCHED : 0) | (racer(id) ? SWOFTY_LIVE_CAR_RACER : 0));
            c.ers_class = r.es ? 1 : 0;
            const SysRec &s = sys_[id];
            c.have |= s.have;
            c.speed = s.speed;
            c.rpm = s.rpm;
            c.rpm_limit = s.rpm_limit;
            c.gear = s.gear;
            c.throttle = s.throttle;
            c.brake = s.brake;
            c.clutch = s.clutch;
            c.steer = s.steer;
            c.g_lat = s.g_lat;
            c.g_long = s.g_long;
            std::memcpy(c.world_pos, s.pos, sizeof s.pos);
            std::memcpy(c.world_vel, s.vel, sizeof s.vel);
            c.drs = s.drs;
            const ErsRec &e = ers_[id];
            c.have |= e.have;
            c.ers_store = e.store;
            c.ers_capacity = e.capacity;
            c.ers_deploy = e.deploy;
            c.ers_harvest_k = e.harvest_k;
            c.ers_harvest_h = e.harvest_h;
            c.ers_max_deploy = e.max_deploy;
            c.ers_harvested_lap = e.harvested_lap;
            c.ers_deployed_lap = e.deployed_lap;
            c.ers_harvest_limit_lap = e.harvest_limit_lap;
            c.ers_mode = e.mode;
            if (prog_have_[id]) {
                c.have |= SWOFTY_LIVE_HAVE_PROGRESS;
                c.lap = prog_[id].lap;
                c.total_m = prog_[id].total;
                if (track_length_ok_) {
                    float L_ = track_length_;
                    c.lap_m = std::clamp(prog_[id].total - float(prog_[id].plap) * L_, 0.f, L_);
                    if (sectors_ok_) {
                        int sec = sector_of(sector_end_, sector_count_, c.lap_m);
                        if (sec) {
                            c.sector = uint8_t(sec);
                            c.have |= SWOFTY_LIVE_HAVE_SECTOR;
                        }
                    }
                } else
                    c.have &= ~SWOFTY_LIVE_HAVE_PROGRESS;
            }
            const Cls &k = cls_[id];
            if (k.have) {
                c.have |= SWOFTY_LIVE_HAVE_CLASS;
                c.position = k.pos;
                c.result_status = k.status;
                c.time_penalty_s = k.tpen ? k.penalty : 0;
            }
            c.pit_state = pit_[id];
            if (pit_[id] != 0xff)
                c.have |= SWOFTY_LIVE_HAVE_PIT;
            if (pit_lane_ok_[id]) {
                c.have |= SWOFTY_LIVE_HAVE_PIT_LANE;
                c.pit_status = pit_status_[id];
                c.pit_stops = pit_stops_[id];
            }
            c.gap_leader_ms = c.gap_ahead_ms = SWOFTY_LIVE_NO_GAP;
            const Gap &g = gap_[id];
            bool ok_status = k.have && (k.status == 2 || k.status == 3) && !(pit_[id] >= 2 && pit_[id] <= 15);
            if (g.leader && ok_status) {
                c.gap_leader_ms = g.leader_ms;
                c.have |= SWOFTY_LIVE_HAVE_GAP_LEADER;
            }
            if (g.ahead && ok_status) {
                c.gap_ahead_ms = g.ahead_ms;
                c.have |= SWOFTY_LIVE_HAVE_GAP_AHEAD;
            }
            const Pen &p = pen_[id];
            if (p.have) {
                c.have |= SWOFTY_LIVE_HAVE_PENALTIES;
                c.warnings = p.warnings;
                c.track_limit_warnings = p.limits;
                c.drive_through = p.dt;
                c.stop_go = p.sg;
            }
            const Tyre &t = tyre_[id];
            c.tyre_visual = t.visual;
            c.tyre_actual = t.actual;
            if (t.have) {
                c.have |= SWOFTY_LIVE_HAVE_TYRE;
                c.tyre_letter = t.letter;
            }
        }

        if (g_[G_CLASS] && cls_ok_) {
            Observed now_obs[MAX_CARS];
            for (int i = 0; i < n_cars_; i++) {
                unsigned id = refs_[i].id;
                const Cls &k = cls_[id];
                if (!k.have)
                    continue;
                now_obs[id] = {true, k.pos, k.status, pit_[id], k.tpen ? k.penalty : 0.f};
            }
            if (observed_valid_ && last_generation_ == player_gen_ && cls_stamp_ != cls_stamp_seen_) {
                std::vector<Event> fresh;
                derive_events(observed_, now_obs, ses.type, ses.safety_car, fresh, retire_from_bus_);
                events_.insert(events_.end(), fresh.begin(), fresh.end());
            }
            if (cls_stamp_ != cls_stamp_seen_) {
                std::memcpy(observed_, now_obs, sizeof observed_);
                observed_valid_ = true;
                cls_stamp_seen_ = cls_stamp_;
            }
            last_generation_ = player_gen_;
        }
    }

    uint64_t cls_stamp_ = 0, cls_stamp_seen_ = ~uint64_t(0);
    uint32_t last_generation_ = 0;
};

}
