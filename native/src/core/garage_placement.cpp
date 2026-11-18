// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#include "runtime.hpp"
#include "apply/moments.hpp"
#include "garage_placement/steam_26566308.hpp"
#include "garage_placement/garage_crew_pose.hpp"
#include "garage_placement/garage_vehicle.hpp"
#include <swofty/mixin.hpp>
#include <swofty/mem.hpp>
#include <atomic>
#include <memory>
#include <mutex>
#include <optional>

namespace {
using namespace runtime::garage_placement;
namespace backend = runtime::garage_placement::steam_26566308;
namespace sdk = swofty::garage_placement;
using R = sdk::Result;
constexpr const char *mixin = "swofty_core.dll (garage placement)";
constexpr const char *scene_site = "site_Game_GarageCrew_SceneRead";
constexpr const char *sequence_fn = "Game_GarageCrewInstance_PlaySequence";
constexpr const char *update_fn = "Game_PitBoxCarActor_Update";
using Play = uint64_t (*)(void *, const char *, int32_t, float, float);
using Update = void (*)(void *);
using Refresh = void (*)(void *, uint8_t);
using Node = uint32_t (*)(const char *);
Play play_original{};
Update update_original{};
Refresh refresh{};
Node node{};
runtime::garage_crew_pose::FindNode find_node{};
std::atomic<bool> placement_enabled{false};
auto read_value = [](Address at, auto &out) { return swofty::mem::read(at, out); };

struct SavedCrew {
    CrewPlacement placement;
    std::string track;
    backend::Bank bank;
};
struct Scope { std::string track; backend::Bank bank; };
using CarStep=vehicle::Step;
using SavedCar=vehicle::Change;
std::atomic<unsigned> active_edits{};
struct Live {
    Owner owner;
    uint64_t generation;
    std::atomic<bool> idle{false};
    // Contending/reentrant callbacks skip this pass. Session unload only drops the
    // shared owner, and never waits for or dereferences a rig being torn down.
    std::mutex operation;
    std::atomic<bool> departure{false};
    std::optional<SavedCrew> crew;
    std::array<std::optional<members::Edit>, members::role_count> individual{};
    std::optional<SavedCar> vehicle;
    R car_result{R::unavailable};
    std::optional<Scope> edit_scope;
    bool counted{};
    bool has_individual() const
    {
        return std::any_of(individual.begin(), individual.end(), [](const auto &e) { return bool(e); });
    }
    void sync()
    {
        const bool extra = vehicle || has_individual();
        if (!extra) edit_scope.reset();
        const bool active = extra || crew;
        if (counted != active) {
            if (active) ++active_edits;
            else --active_edits;
            counted=active;
        }
    }
    void remember(SavedCrew saved) { crew=std::move(saved); sync(); }
    void forget() { crew.reset(); sync(); }
    ~Live() { if (counted) --active_edits; }
};
using LivePtr = std::shared_ptr<Live>;
std::mutex state_mutex;
std::array<LivePtr, 64> bays;
struct Sequence { Address instance{}; ULONGLONG tick{}; bool idle{}; };
std::array<Sequence, 64> early_sequences{};
thread_local LivePtr updating;
struct Visit {
    uint64_t ticket;
    LivePtr live;
    std::string track;
    backend::Bank bank;
};
thread_local Visit *visiting = nullptr;
std::atomic<uint64_t> tickets{0};
uint64_t generations{}; // Protected by state_mutex and monotonic across unloads.

bool current(const LivePtr &live)
{
    std::lock_guard lock(state_mutex);
    const auto &o = live->owner;
    return bays[o.garage * 2 + o.native_slot] == live;
}
bool same_track(const std::string &id)
{
    return swofty_core::apply::moments::session_mode_track().second == id;
}
bool scope(const LivePtr &live, const backend::Bank &bank)
{
    return current(live) && backend::owns(read_value, live->owner) && backend::matches(read_value, live->owner, bank);
}
Visit *context(uint64_t ticket)
{
    return visiting && visiting->ticket == ticket && visiting->live == updating &&
           current(updating) && backend::owns(read_value, updating->owner) ? visiting : nullptr;
}
bool page(Address at, bool code)
{
    MEMORY_BASIC_INFORMATION p{};
    if (!VirtualQuery(reinterpret_cast<void *>(at), &p, sizeof(p)) || p.State != MEM_COMMIT ||
        p.Type != MEM_IMAGE || p.AllocationBase != GetModuleHandleW(nullptr) || (p.Protect & PAGE_GUARD))
        return false;
    DWORD access = p.Protect & 0xff;
    return code ? (access == PAGE_EXECUTE || access == PAGE_EXECUTE_READ || access == PAGE_EXECUTE_READWRITE ||
                   access == PAGE_EXECUTE_WRITECOPY) : (access == PAGE_READONLY || access == PAGE_EXECUTE_READ);
}
auto write_field = [](Address at, const auto &field) {
    MEMORY_BASIC_INFORMATION p{};
    if (!VirtualQuery(reinterpret_cast<void *>(at), &p, sizeof(p)) || p.State != MEM_COMMIT ||
        (p.Protect & PAGE_GUARD) || (p.Protect & 0xff) != PAGE_READWRITE ||
        sizeof(field) > p.RegionSize - (at - reinterpret_cast<Address>(p.BaseAddress)))
        return false;
    std::memcpy(reinterpret_cast<void *>(at), &field, sizeof(field));
    return true;
};
CrewIds ids()
{
    return {node("MB"), node("EB1"), node("MB:")};
}
R adjust(const LivePtr &live, const SavedCrew &saved, bool restore)
{
    auto scoped = [&](const Owner &) { return scope(live, saved.bank) && (restore || live->idle.load()); };
    auto push = [&](CrewRole role, const Matrix &world) {
        return push_central_world(live->owner, saved.placement, role, world, read_value,
                                  [](Address p) { return page(p, false); }, [](Address p) { return page(p, true); });
    };
    switch (set_central_crew(live->owner, saved.placement, restore, read_value, scoped, push)) {
    case CrewResult::applied:
    case CrewResult::restored: return R::ok;
    case CrewResult::rolled_back: return R::rolled_back;
    case CrewResult::rollback_failed: return R::rollback_failed;
    default: return R::unavailable;
    }
}

bool restore_departure(const LivePtr &live);
void check_car(const LivePtr &live);

void capture(SwoftyContext *c)
{
    Owner owner{};
    bool captured = owner_at_scene_read(read_value, c->rbp, c->rdi, c->rsp, owner);
    std::lock_guard lock(state_mutex);
    for (auto &old : bays)
        if (old && old->owner.instance == c->rbp)
            old.reset();
    if (!captured)
        return;
    auto live = std::make_shared<Live>();
    live->owner = owner;
    live->generation = ++generations;
    for (auto &e : early_sequences)
        if (e.instance == owner.instance) {
            live->idle = GetTickCount64() - e.tick <= 1000 && e.idle;
            e = {};
            break;
        }
    bays[owner.garage * 2 + owner.native_slot] = std::move(live);
}

uint64_t play(void *instance, const char *name, int32_t arg, float start, float blend)
{
    std::string sequence;
    bool idle = swofty::mem::c_string(name, sequence, 96) &&
                (sequence == "InGarageIdle" || sequence == "InGarageIdleInstant" ||
                 sequence == "InGarageIdleMirrored" || sequence == "InGarageIdleInstantMirrored");
    uint64_t result = play_original(instance, name, arg, start, blend);
    LivePtr live;
    {
        std::lock_guard lock(state_mutex);
        const Address key = reinterpret_cast<Address>(instance);
        for (const auto &b : bays)
            if (b && b->owner.instance == key) { live = b; break; }
        if (live) {
            live->idle = idle;
        } else {
            auto place = std::find_if(early_sequences.begin(), early_sequences.end(),
                                      [=](const Sequence &e) { return e.instance == key; });
            if (place == early_sequences.end())
                place = std::min_element(early_sequences.begin(), early_sequences.end(),
                                         [](const auto &a, const auto &b) { return a.tick < b.tick; });
            *place = {key, GetTickCount64(), idle};
        }
    }
    if (live && !idle) {
        live->departure = true;
        std::unique_lock operation(live->operation, std::try_to_lock);
        if (operation && restore_departure(live)) live->departure = false;
    }
    return result;
}

void update(void *actor)
{
    update_original(actor);
    const bool listened = runtime::garage_placement_listening();
    if (updating || (!listened && !active_edits.load(std::memory_order_relaxed)))
        return;
    LivePtr live;
    {
        std::lock_guard lock(state_mutex);
        for (const auto &b : bays)
            if (b && b->owner.car_actor == reinterpret_cast<Address>(actor)) {
                if (live) return; // Two live bays claim the same update. Neither is authorized.
                live = b;
            }
    }
    if (!live)
        return;
    std::unique_lock operation(live->operation, std::try_to_lock);
    if (!operation)
        return;
    updating = live;
    struct End { ~End() { updating.reset(); } } end;
    if (live->vehicle) check_car(live);
    if (live->departure && restore_departure(live)) live->departure = false;
    if (listened)
        runtime::events_fire(SWOFTY_EVENT_GARAGE_PARKED_CAR, live->owner.garage, live->owner.native_slot, nullptr);
}
R visit(const char *track, const sdk::Pit *pits, uint32_t count, uint32_t garage, uint32_t slot,
        sdk::Visitor callback, void *user)
{
    if (!track || !*track || !callback || slot > 1 || garage >= 32)
        return R::invalid;
    if (visiting || !updating || updating->owner.garage != garage || updating->owner.native_slot != slot)
        return R::unavailable;
    Visit v{++tickets, updating, track, {}};
    if (!backend::authored(pits, count, garage, v.bank))
        return R::invalid;
    if (!same_track(v.track) || !scope(v.live, v.bank))
        return R::unavailable;
    visiting = &v;
    struct End { ~End() { visiting = nullptr; } } end;
    callback(sdk::Bay{v.ticket, v.live->generation, garage, slot, v.live->idle.load()}, user);
    return R::ok;
}
void copy(const Matrix &m, sdk::WorldMatrix &out) { std::copy(m.begin(), m.end(), out.values); }
R pose(uint64_t ticket, sdk::WorldMatrix *out, bool car)
{
    auto v = context(ticket);
    Matrix m{};
    if (!v || !out)
        return R::unavailable;
    const auto &o = v->live->owner;
    if (!(car ? parked_car_world(read_value, o, m) : read_value(o.instance + 0x1c0, m)) || !rig_detail::affine(m))
        return R::unavailable;
    copy(m, *out);
    return R::ok;
}
R root(uint64_t t, sdk::WorldMatrix *out) { return pose(t, out, false); }
R car(uint64_t t, sdk::WorldMatrix *out) { return pose(t, out, true); }
R set_root(uint64_t t, const sdk::WorldMatrix *m)
{
    auto v = context(t);
    if (!v || !m || !v->live->idle || v->live->departure || v->live->crew)
        return R::unavailable;
    if (v->live->vehicle || v->live->has_individual()) return R::conflict;
    Matrix after{};
    std::copy_n(m->values, 16, after.begin());
    auto scoped = [&](const Owner &) { return scope(v->live, v->bank) && v->live->idle.load(); };
    auto propagate = [](Address scene) { refresh(reinterpret_cast<void *>(scene), 1); return true; };
    switch (set_garage_root(v->live->owner, after, read_value, write_field, scoped, propagate)) {
    case RootResult::applied: return R::ok;
    case RootResult::invalid_transform: return R::invalid;
    case RootResult::rolled_back: return R::rolled_back;
    case RootResult::rollback_failed: return R::rollback_failed;
    default: return R::unavailable;
    }
}
R read_crew(uint64_t t, sdk::ActorRoots *out)
{
    auto v = context(t);
    if (!v || !out)
        return R::unavailable;
    CentralCrew roles{};
    Matrix mb{}, eb1{};
    if (!central_crew(read_value, v->live->owner, ids(), roles) ||
        !crew_detail::model_world(read_value, roles.mb, mb) || !crew_detail::model_world(read_value, roles.eb1, eb1))
        return R::unavailable;
    copy(mb, out->mb);
    copy(eb1, out->eb1);
    return R::ok;
}
bool actor_pelvis(Address actor, sdk::Position &out)
{
    if (!find_node) return false;
    auto read = [](uint64_t p, void *dst, size_t n, void *) {
        if (!swofty::mem::readable(static_cast<Address>(p),n)) return false;
        std::memcpy(dst,reinterpret_cast<const void *>(p),n);
        return true;
    };
    runtime::garage_crew_pose::Position absolute{};
    if (!runtime::garage_crew_pose::read_pelvis(read,nullptr,actor,node("Bip01"),find_node,absolute)) return false;
    out={absolute.x,absolute.y,absolute.z};
    return true;
}
R read_pelvis(uint64_t t, sdk::CrewPositions *out)
{
    auto v = context(t);
    if (!v || !out || !find_node)
        return R::unavailable;
    CentralCrew roles{};
    if (!central_crew(read_value, v->live->owner, ids(), roles))
        return R::unavailable;
    sdk::CrewPositions both{};
    if (!actor_pelvis(roles.mb,both.mb) || !actor_pelvis(roles.eb1,both.eb1)) return R::unavailable;
    *out=both;
    return R::ok;
}
R set_crew(uint64_t t, const sdk::Offsets *offsets)
{
    auto v = context(t);
    if (!v || !offsets || !v->live->idle || v->live->departure)
        return R::unavailable;
    if (members::conflicts_with_member(v->live->individual))
        return R::conflict;
    Matrix car_world{};
    const auto &o = v->live->owner;
    std::array<CrewDelta, 2> deltas;
    std::copy_n(offsets->mb, 3, deltas[0].begin());
    std::copy_n(offsets->eb1, 3, deltas[1].begin());
    SavedCrew saved{{}, v->track, v->bank};
    auto scoped = [&](const Owner &) { return scope(v->live, v->bank) && v->live->idle.load(); };
    if (!parked_car_world(read_value, o, car_world) ||
        !prepare_central_crew(o, ids(), car_world, deltas, read_value, scoped, saved.placement))
        return R::unavailable;
    auto r = adjust(v->live, saved, false);
    if (r == R::ok || r == R::rollback_failed)
        v->live->remember(std::move(saved));
    else if (r == R::rolled_back)
        v->live->forget();
    return r;
}
const members::NamesByRole &role_ids()
{
    // The engine's model node intern table is process-global. Resolve only on the
    // first scoped game callback after the build and named model functions are verified.
    static const auto table=members::names(node);
    return table;
}
R list_members(uint64_t t,sdk::Mechanics *out)
{
    auto v=context(t);
    members::Selection found{};
    if (!v || !out || !members::list(v->live->owner,role_ids(),read_value,found)) return R::unavailable;
    *out=found.roles;
    return R::ok;
}
R read_member(uint64_t t,members::Role role,sdk::WorldMatrix *out)
{
    auto v=context(t);
    if (!v || !out) return R::unavailable;
    members::Actor actor{};
    auto r=members::select(v->live->owner,role_ids(),role,read_value,actor);
    if (r!=R::ok) return r;
    Matrix world{};
    if (!crew_detail::model_world(read_value,actor.value,world)) return R::unavailable;
    copy(world,*out);
    return R::ok;
}
R read_member_pelvis(uint64_t t,members::Role role,sdk::Position *out)
{
    auto v=context(t);
    if (!v || !out) return R::unavailable;
    members::Actor actor{};
    auto r=members::select(v->live->owner,role_ids(),role,read_value,actor);
    if (r!=R::ok) return r;
    return actor_pelvis(actor.value,*out) ? R::ok : R::unavailable;
}
R move_member(const LivePtr &live,const members::Edit &change)
{
    auto push=[](Address actor,const Matrix &m) {
        return crew_detail::set_actor_world(actor,m,read_value,[](Address p){return page(p,false);},
                                            [](Address p){return page(p,true);});
    };
    return members::apply(live->owner,role_ids(),change,read_value,push);
}
R edit_member(uint64_t t,members::Role role,const sdk::WorldMatrix *absolute,const sdk::Position *delta)
{
    auto v=context(t);
    if (!v || !v->live->idle || v->live->departure || !scope(v->live,v->bank)) return R::unavailable;
    if (unsigned(role)>=members::role_count || (!absolute && !delta)) return R::invalid;
    if (members::conflicts_with_pair(role,bool(v->live->crew))) return R::conflict;
    members::Edit change{};
    auto r=members::prepare(v->live->owner,role_ids(),role,read_value,change);
    if (r!=R::ok) return r;
    // The allowlist comes from human scene macros. An actor also needs the
    // character skeleton expected by those authored roles before we move it.
    sdk::Position pelvis{};
    if (!actor_pelvis(change.actor.value,pelvis)) return R::unavailable;
    auto &saved=v->live->individual[unsigned(role)];
    if (saved && saved->actor.value!=change.actor.value) { saved.reset(); v->live->sync(); }
    if (saved) change.native=saved->native;
    if (absolute) std::copy_n(absolute->values,16,change.target.begin());
    else {
        change.target=change.native;
        change.target[12]+=delta->x; change.target[13]+=delta->y; change.target[14]+=delta->z;
    }
    r=move_member(v->live,change);
    if (r==R::ok || r==R::rollback_failed) {
        if (!v->live->edit_scope) v->live->edit_scope=Scope{v->track,v->bank};
        saved=change;
        v->live->sync();
    }
    return r;
}
R set_member(uint64_t t,members::Role role,const sdk::WorldMatrix *m) { return edit_member(t,role,m,nullptr); }
R offset_member(uint64_t t,members::Role role,const sdk::Position *d) { return edit_member(t,role,nullptr,d); }
R restore_one(const LivePtr &live,members::Role role)
{
    auto &saved=live->individual[unsigned(role)];
    members::Edit now{};
    auto r=members::prepare(live->owner,role_ids(),role,read_value,now);
    if (r==R::missing || (r==R::ok && saved && saved->actor.value!=now.actor.value)) {
        saved.reset(); live->sync();
        return r==R::missing ? r : R::unavailable;
    }
    if (r!=R::ok) return r;
    if (!saved) return R::ok;
    now.target=saved->native;
    r=move_member(live,now);
    if (r==R::ok) { saved.reset(); live->sync(); }
    return r;
}
R restore_member(uint64_t t,members::Role role)
{
    auto v=context(t);
    if (!v || !scope(v->live,v->bank)) return R::unavailable;
    if (unsigned(role)>=members::role_count) return R::invalid;
    if (members::conflicts_with_pair(role,bool(v->live->crew))) return R::conflict;
    return restore_one(v->live,role);
}
uint32_t vehicle_id() { static const auto id=node("vehicle"); return id; }
R assign_vehicle(const LivePtr &live,const vehicle::Pose &prior,const Matrix &world)
{
    auto propagate=[](Address scene){ refresh(reinterpret_cast<void *>(scene),1); return true; };
    return vehicle::assign(live->owner,vehicle_id(),prior,world,read_value,write_field,propagate);
}
R set_car(uint64_t t,const sdk::WorldMatrix *target)
{
    auto v=context(t);
    if (!v || !target || !v->live->idle || v->live->departure || !scope(v->live,v->bank)) return R::unavailable;
    const auto &live=v->live;
    if (live->vehicle && live->vehicle->step!=CarStep::active) return live->car_result;
    vehicle::Pose before{};
    if (!vehicle::capture(live->owner,vehicle_id(),read_value,before)) return R::unavailable;
    Matrix world{},relative{};
    std::copy_n(target->values,16,world.begin());
    if (!vehicle::relative(before.car_world,world,relative)) return R::invalid;
    const Matrix after=rig_detail::world_from_definition(before.scene_world,relative);
    const auto result=assign_vehicle(live,before,after);
    live->car_result=result;
    if (result==R::pending || result==R::rolled_back || result==R::rollback_failed) {
        const bool reverting=result==R::rolled_back;
        SavedCar saved{live->vehicle ? live->vehicle->native : before,before,
                       reverting ? before.car_world : world,reverting ? CarStep::rollback : CarStep::verify,
                       GetTickCount64(),bool(live->vehicle),0};
        if (!live->edit_scope) live->edit_scope=Scope{v->track,v->bank};
        live->vehicle=saved;
        live->car_result=reverting ? R::pending : result;
        live->sync();
    }
    return live->car_result;
}
R begin_car_restore(const LivePtr &live,bool rollback)
{
    auto &saved=*live->vehicle;
    vehicle::Pose before{};
    if (!vehicle::capture(live->owner,vehicle_id(),read_value,before) || before.scene!=saved.native.scene)
        return R::unavailable;
    const auto &target=rollback ? saved.prior : saved.native;
    auto r=assign_vehicle(live,before,target.scene_world);
    if (r==R::pending) {
        saved.expected=target.car_world;
        saved.since=GetTickCount64(); saved.observations=0;
        saved.step=rollback ? CarStep::rollback : CarStep::restore;
    }
    live->car_result=r;
    return r;
}
R restore_car(uint64_t t)
{
    auto v=context(t);
    if (!v || !scope(v->live,v->bank)) return R::unavailable;
    if (!v->live->vehicle) return R::ok;
    if (v->live->vehicle->step==CarStep::restore && v->live->car_result==R::pending) return R::pending;
    return begin_car_restore(v->live,false);
}
R car_result(uint64_t t)
{
    auto v=context(t);
    return v ? v->live->car_result : R::unavailable;
}
void check_car(const LivePtr &live)
{
    auto &saved=*live->vehicle;
    if (saved.step==CarStep::active || !live->idle || live->car_result!=R::pending ||
        !live->edit_scope || !scope(live,live->edit_scope->bank)) return;
    vehicle::Pose actual{};
    const bool observed=vehicle::capture(live->owner,vehicle_id(),read_value,actual) && actual.scene==saved.native.scene;
    switch (vehicle::verify(saved,observed ? &actual.car_world : nullptr,GetTickCount64())) {
    case vehicle::Verification::wait: return;
    case vehicle::Verification::active: live->car_result=R::ok; break;
    case vehicle::Verification::restored: live->vehicle.reset(); live->car_result=R::ok; break;
    case vehicle::Verification::previous:
        if (!saved.prior_override) live->vehicle.reset();
        live->car_result=R::rolled_back; break;
    case vehicle::Verification::revert:
        if (begin_car_restore(live,true)!=R::pending) live->car_result=R::rollback_failed;
        break;
    case vehicle::Verification::failed: live->car_result=R::rollback_failed; break;
    }
    live->sync();
}
bool restore_departure(const LivePtr &live)
{
    bool ok=true;
    if (live->crew) {
        if (same_track(live->crew->track) && adjust(live,*live->crew,true)==R::ok) live->forget();
        else ok=false;
    }
    if (!live->edit_scope) return ok;
    if (!same_track(live->edit_scope->track) || !scope(live,live->edit_scope->bank)) return false;
    for (unsigned i=0;i<members::role_count;++i)
        if (live->individual[i] && restore_one(live,members::Role(i))!=R::ok && live->individual[i]) ok=false;
    if (live->vehicle && !(live->vehicle->step==CarStep::restore && live->car_result==R::pending))
        if (begin_car_restore(live,false)!=R::pending) ok=false;
    return ok;
}
R restore_crew(uint64_t t)
{
    auto v = context(t);
    if (!v || !v->live->crew)
        return R::unavailable;
    auto r = adjust(v->live, *v->live->crew, true);
    if (r == R::ok)
        v->live->forget();
    return r;
}
}

namespace runtime {
std::vector<CoreMixins> garage_placement_mixins()
{
    if (placement_enabled || !code_ready() || !backend::supported(build_distribution(), build_exe_sha()))
        return {};
    auto resolve = swofty::api->resolve;
    if (!resolve(scene_site) || !resolve(sequence_fn) || !resolve(update_fn))
        return {};
    refresh = reinterpret_cast<Refresh>(resolve("Game_Scene_UpdateInstanceTransformsBatch"));
    node = reinterpret_cast<Node>(resolve("Game_Get_ModelNodeId"));
    find_node = reinterpret_cast<runtime::garage_crew_pose::FindNode>(resolve("Ne_NodeTable_FindIndexByHash"));
    if (!refresh || !node)
        return {};
    placement_enabled = true;
    return {{mixin, {{scene_site, SWOFTY_MIXIN_SITE, 0, (void *)&capture, nullptr},
                     {sequence_fn, SWOFTY_MIXIN_WRAP, 0, (void *)&play, (void **)&play_original},
                     {update_fn, SWOFTY_MIXIN_WRAP, 0, (void *)&update, (void **)&update_original}}}};
}
void garage_placement_reset()
{
    std::lock_guard lock(state_mutex);
    bays = {};
    early_sequences = {};
}
}

extern "C" __declspec(dllexport) const sdk::detail::Ops *SwoftyCoreGaragePlacement()
{
    static const sdk::detail::Ops api{1, &visit, &root, &car, &set_root, &read_crew, &set_crew, &restore_crew, &read_pelvis};
    return placement_enabled ? &api : nullptr;
}

extern "C" __declspec(dllexport) const sdk::detail::OpsV2 *SwoftyCoreGaragePlacementV2()
{
    static const sdk::detail::OpsV2 api{2,&list_members,&read_member,&read_member_pelvis,&set_member,
                                       &offset_member,&restore_member,&set_car,&restore_car,&car_result};
    return placement_enabled ? &api : nullptr;
}
