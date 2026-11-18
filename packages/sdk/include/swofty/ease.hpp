// SPDX-License-Identifier: MIT
#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <optional>

// Smoothing and timing helpers for HUD values. Header only, no game access, so they run on the host in tests.
// Every helper that needs the time takes it as an argument (seconds, any monotonic origin): drive them from
// frame dt sums or from swofty::fake::Clock.
//
// The ease helpers are the generic parts of Project Halo's value pipeline (hud_model.cpp, gmeter.cpp,
// tower_model.cpp). Uses work from Project Halo by nyannoying, https://github.com/nyannoying1337/f1-25-project-halo

namespace swofty::ease {

// Exponential smoothing: x moves toward target with time constant tau seconds. dt is clamped to 0..0.1 so a
// hitch cannot jump the value; tau <= 0 follows the target at once.
inline float toward(float x, float target, float dt, float tau)
{
    if (tau <= 0.f)
        return target;
    dt = std::clamp(dt, 0.f, 0.1f);
    return target + (x - target) * std::exp(-dt / tau);
}

// An eased channel. The first update after construction or reset() lands on the target instead of
// sliding up from zero.
struct Smoothed {
    float tau = 0.05f;
    float v = 0.f;
    bool seeded = false;

    explicit Smoothed(float tau_s = 0.05f) : tau(tau_s) {}
    float update(float target, float dt)
    {
        v = seeded ? toward(v, target, dt, tau) : target;
        seeded = true;
        return v;
    }
    void reset() { seeded = false; v = 0.f; }
    void reset(float to) { seeded = true; v = to; }
};

// Median of the last three values (of two, their mean). Removes one-sample spikes.
struct Median3 {
    std::array<float, 3> a{};
    int n = 0, head = 0;

    float push(float x)
    {
        a[head] = x;
        head = (head + 1) % 3;
        n = std::min(n + 1, 3);
        if (n == 1)
            return x;
        if (n == 2)
            return 0.5f * (a[0] + a[1]);
        return std::max(std::min(a[0], a[1]), std::min(std::max(a[0], a[1]), a[2]));
    }
    void reset() { n = head = 0; }
};

// True when the value is not the same as last time, bit for bit for floats. A paused game repeats the same
// record, so filters that should only see fresh values gate on this.
template <class T> struct Fresh {
    T last{};
    bool have = false;

    bool update(const T &now)
    {
        bool same = have && std::memcmp(&last, &now, sizeof(T)) == 0;
        last = now;
        have = true;
        return !same;
    }
    void reset() { have = false; }
};

// Keeps the last valid value for a while. set() on every valid read; get() answers while the value is younger
// than max_age seconds. For torn reads, a stale tyre cache, a yellow flag that blinks.
template <class T> struct Hold {
    T value{};
    double at = -1e18;
    bool have = false;

    void set(const T &v, double now)
    {
        value = v;
        at = now;
        have = true;
    }
    std::optional<T> get(double now, double max_age) const
    {
        if (have && now - at <= max_age)
            return value;
        return std::nullopt;
    }
    T get_or(double now, double max_age, const T &fallback) const
    {
        auto v = get(now, max_age);
        return v ? *v : fallback;
    }
    void reset() { have = false; }
};

// On for a number of seconds after trigger(); trigger again to restart. The position arrows (4 s).
struct Pulse {
    double until = -1e18;

    void trigger(double now, double seconds) { until = now + seconds; }
    bool on(double now) const { return now < until; }
    void clear() { until = -1e18; }
};

// A short history of samples for a rate of change: the battery derivative, a gap trend.
template <size_t N = 32> struct Window {
    std::array<double, N> t{};
    std::array<float, N> v{};
    size_t count = 0, head = 0;

    void push(double now, float x)
    {
        t[head] = now;
        v[head] = x;
        head = (head + 1) % N;
        count = std::min(count + 1, N);
    }
    // Change per second between the oldest sample within span seconds of now and the newest. Empty until two
    // samples at least min_dt apart exist.
    std::optional<float> rate(double now, double span, double min_dt = 0.05) const
    {
        if (count < 2)
            return std::nullopt;
        size_t newest = (head + N - 1) % N;
        size_t oldest = newest;
        for (size_t k = 1; k < count; k++) {
            size_t i = (newest + N - k) % N;
            if (now - t[i] > span)
                break;
            oldest = i;
        }
        double dt = t[newest] - t[oldest];
        if (oldest == newest || dt < min_dt)
            return std::nullopt;
        return float((v[newest] - v[oldest]) / dt);
    }
    void reset() { count = head = 0; }
};

// How many of steps stripes light for x in 0..1 (rounded): 0.5 of 10 is 5. Halo's stripe_level / cap_level.
inline int level(float x01, int steps)
{
    return int(std::lround(std::clamp(x01, 0.f, 1.f) * float(steps)));
}

}
