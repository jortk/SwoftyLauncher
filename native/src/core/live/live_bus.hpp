#pragma once
#include "live_mem.hpp"
#include "live_state.hpp"

#include <mutex>
#include <vector>

namespace swofty_core::live::bus {

constexpr const char *COLLISION_MESSAGE = "r_car_to_car_col";
constexpr const char *STATUS_MESSAGE = "vehicle_status_changed";
constexpr unsigned CARS = 24;
constexpr double COLLISION_QUIET_MS = 4000.0;

constexpr uintptr_t bus_from_holder = 0x8, bus_active = 0x68, bus_allocator = 0x78, allocator_alloc = 0x18;
constexpr size_t delegate_size = 0x10, delegate_align = 8;

namespace msg {
constexpr size_t collision_impulse = 0x10, collision_a = 0x18, collision_b = 0x19, collision_size = 0x1a;
constexpr size_t status_vehicle = 0x10, status_code = 0x12, status_reason = 0x14, status_size = 0x15;
}

// Game_TelemetryUdp_OnCollisionMessage (0x142cf6990): the pair is dropped while the second car has had a collision
// event in the last four seconds; an emitted event stamps both cars.
struct Collisions {
    bool seen[CARS] = {};
    double last_ms[CARS] = {};

    bool take(const uint8_t *m, double now_ms, Event &out)
    {
        unsigned a = m[msg::collision_a], b = m[msg::collision_b];
        if (a >= CARS || b >= CARS)
            return false;
        if (seen[b] && now_ms - last_ms[b] <= COLLISION_QUIET_MS)
            return false;
        seen[a] = seen[b] = true;
        last_ms[a] = last_ms[b] = now_ms;
        out = {Event::collision, int64_t(a), int64_t(b)};
        return true;
    }

    void reset() { *this = Collisions{}; }
};

// UDP RTMT emitter (0x142cf6f00): a status change whose code is 5 or 7 is a retirement; the byte at +0x14 is the reason.
inline bool retirement_from(const uint8_t *m, Event &out)
{
    if ((m[msg::status_code] | 2) != 7 || m[msg::status_vehicle] >= CARS)
        return false;
    out = {Event::retirement, int64_t(m[msg::status_vehicle]), int64_t(m[msg::status_reason])};
    return true;
}

// Events handed over from the game's message callbacks to the flow tick.
class Queue {
public:
    void push_collision(const uint8_t *m, double now_ms)
    {
        Event e;
        std::lock_guard<std::mutex> g(lock_);
        if (collisions_.take(m, now_ms, e))
            push(e);
    }

    void push_retirement(const uint8_t *m)
    {
        Event e;
        if (!retirement_from(m, e))
            return;
        std::lock_guard<std::mutex> g(lock_);
        push(e);
    }

    std::vector<Event> drain()
    {
        std::vector<Event> out;
        std::lock_guard<std::mutex> g(lock_);
        out.swap(events_);
        return out;
    }

    void reset()
    {
        std::lock_guard<std::mutex> g(lock_);
        events_.clear();
        collisions_.reset();
    }

private:
    void push(const Event &e)
    {
        if (events_.size() < 256)
            events_.push_back(e);
    }

    std::mutex lock_;
    std::vector<Event> events_;
    Collisions collisions_;
};

// Subscribes our two callbacks to the game's message bus the way the game's own subscribe thunks do (0x142dd4b20):
// the delegate is a 0x10-byte, 8-aligned {fn, ctx} object that alloc() takes from the bus allocator (vtable +0x18),
// handed to Ne_Subscribe_BusMessage. A new bus object (a fresh process state) is subscribed again; the same bus never twice.
struct Ops {
    Mem mem;
    uintptr_t bus_slot = 0;
    uintptr_t (*alloc)(uintptr_t allocator_object, uintptr_t fn, uintptr_t ctx) = nullptr;
    void (*subscribe)(uintptr_t bus, const char *name, uintptr_t delegate) = nullptr;
    uintptr_t collision_fn = 0, status_fn = 0, ctx = 0;
};

class Subscriber {
public:
    bool armed() const { return bus_ != 0; }
    uintptr_t bus() const { return bus_; }

    bool arm(const Ops &o)
    {
        if (!o.bus_slot || !o.alloc || !o.subscribe || !o.collision_fn || !o.status_fn)
            return false;
        uintptr_t holder = 0, bus = 0, allocator = 0;
        uint8_t active = 0;
        if (!o.mem.get(o.bus_slot, holder) || !holder || !o.mem.get(holder + bus_from_holder, bus) || !bus ||
            !o.mem.get(bus + bus_active, active) || !active || !o.mem.get(bus + bus_allocator, allocator) || !allocator)
            return false;
        if (bus == bus_)
            return true;
        uintptr_t d1 = o.alloc(allocator, o.collision_fn, o.ctx);
        if (!d1)
            return false;
        uintptr_t d2 = o.alloc(allocator, o.status_fn, o.ctx);
        if (!d2)
            return false;
        o.subscribe(bus, COLLISION_MESSAGE, d1);
        o.subscribe(bus, STATUS_MESSAGE, d2);
        bus_ = bus;
        return true;
    }

private:
    uintptr_t bus_ = 0;
};

}
