// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#include "garage_vehicle.hpp"
#include "garage_rig.hpp"
#include "garage_central.hpp"

#include <cassert>
#include <cstring>
#include <map>
#include <string>
#include <vector>

using namespace runtime::garage_placement;

struct Fixture {
    std::map<Address, std::vector<unsigned char>> saved;

    template<class T> void put(Address address, const T& value) {
        auto& bytes = saved[address];
        bytes.resize(sizeof(T));
        std::memcpy(bytes.data(), &value, bytes.size());
    }

    template<class T> bool operator()(Address address, T& value) const {
        auto it = saved.find(address);
        if (it == saved.end() || it->second.size() != sizeof(T))
            return false;
        std::memcpy(&value, it->second.data(), sizeof(T));
        return true;
    }
};

Fixture* current_fixture;
constexpr Address mb_actor = 0x90000, eb_actor = 0xa0000;
void fixture_crew_setter(void* object, const float* rows, std::uint32_t enabled)
{
    const auto actor = reinterpret_cast<Address>(object);
    assert((actor == mb_actor || actor == eb_actor) && enabled == 1);
    assert(reinterpret_cast<Address>(rows) % 16 == 0);
    std::array<float, 12> packed{};
    std::copy_n(rows, 12, packed.begin());
    Address model{};
    assert((*current_fixture)(actor + 0x228, model));
    current_fixture->put(model + 0x10, packed);
}

void test_central_crew()
{
    constexpr Address instance = 0x10000, scene = 0x20000, car = 0x30000, bank = 0x40000, module = 0x50000;
    constexpr Address resource = 0x60000, list = 0x70000, defs = 0x80000, table = 0xb0000;
    constexpr Address mb_def = 0xc0000, eb_def = 0xd0000;
    CrewIds ids{7, 8, 9};
    Owner owner{instance, scene, car, bank, 2, 0, module};
    Fixture f;
    current_fixture = &f;
    Matrix world{};
    world[0] = world[5] = world[10] = world[15] = 1;
    f.put(module + 0x60, bank);
    f.put(instance + 0x11c, owner.garage);
    f.put(instance + 0x1a8, car);
    f.put(instance + 0xc8, scene);
    f.put(scene + 0x20, world);
    f.put(scene + 0xa0, resource);
    f.put(scene + 0x78, list);
    f.put(resource + 0x100, std::uint64_t{2});
    f.put(resource + 0x50, defs);
    f.put(list, eb_actor);
    f.put(list + 8, mb_actor);
    f.put(mb_actor + 0x248, ids.mb_variant);
    f.put(eb_actor + 0x248, ids.eb1);
    f.put(mb_actor + 0x538, mb_def);
    f.put(eb_actor + 0x538, Address{});
    f.put(defs, eb_def);
    f.put(eb_def + 0x40, world);
    f.put(mb_def + 0x40, world);
    for (Address member : {mb_actor, eb_actor}) {
        f.put(member, table);
        f.put(member + 0x228, member + 0x100000);
        std::array<float,12> packed{};
        packed[0] = packed[5] = packed[10] = 1;
        f.put(member + 0x100010, packed);
    }
    Address setter = reinterpret_cast<Address>(&fixture_crew_setter);
    f.put(table + 0x70, setter);
    const auto scope = [](const Owner&) { return true; };
    const std::array<CrewDelta,2> deltas{CrewDelta{.2f,0,0},CrewDelta{-.3f,0,0}};
    CrewPlacement saved{};
    assert(prepare_central_crew(owner, ids, world, deltas, f, scope, saved));
    auto push = [&](CrewRole role, const Matrix& w) {
        return push_central_world(owner, saved, role, w, f, [&](Address t) { return t == table; },
                                  [&](Address fn) { return fn == setter; });
    };
    assert(set_central_crew(owner, saved, false, f, scope, push) == CrewResult::applied);
    CrewReadback readback{};
    assert(observe_central_crew(owner, saved, f, readback));
    assert(readback.worlds[0][12] == .2f && readback.worlds[1][12] == -.3f);
    assert(readback.target_error_m[0] == 0. && readback.target_error_m[1] == 0.);
    assert(prepare_central_crew(owner, ids, world, deltas, f, scope, saved));
    assert(saved.members[0].target[12] == .2f); // no double shift
    assert(set_central_crew(owner, saved, true, f, scope, push) == CrewResult::restored);
    bool fail_second = true;
    auto once = [&](CrewRole role, const Matrix& w) {
        if (role == CrewRole::eb1 && fail_second) {
            fail_second = false;
            return false;
        }
        return push(role, w);
    };
    assert(set_central_crew(owner, saved, false, f, scope, once) == CrewResult::rolled_back);
    assert(observe_central_crew(owner, saved, f, readback) && readback.native_error_m[0] == 0.);
    f.put(instance + 0xc8, scene + 8);
    assert(set_central_crew(owner, saved, false, f, scope, push) == CrewResult::unavailable);
    current_fixture = nullptr;
}

void test_root_transaction()
{
    constexpr Address instance = 0x10000, crew = 0x20000, car = 0x30000;
    constexpr Address actor = 0xe0000, renderer = 0xf0000;
    Owner owner{instance, crew, car, 0x40000, 4, 1, 0x300000};
    Matrix before{};
    before[0] = before[5] = before[10] = before[15] = 1;
    auto root = before;
    root[0] = root[10] = 0;
    root[2] = 1;
    root[8] = -1;
    root[12] = 4;
    Fixture f;
    f.put(owner.module + 0x60, owner.pit_bank);
    f.put(instance + 0x11c, owner.garage);
    f.put(instance + 0x1a8, car);
    f.put(instance + 0x1c0, before);
    f.put(instance + 0xe8, actor);
    f.put(actor + 0x38, renderer);
    f.put(renderer + 0x1a80, before);
    f.put(renderer + 0x1ac1, std::uint8_t{0});
    std::array<Address, 5> scenes{};
    for (unsigned i = 0; i < scenes.size(); ++i) {
        scenes[i] = crew + i * 0x10000;
        Address resource = 0x90000 + i * 0x10000;
        auto definition = before;
        definition[12] = float(i + 1);
        f.put(instance + rig_detail::scenes[i], scenes[i]);
        f.put(scenes[i] + 0xa0, resource);
        f.put(resource + 0x140, definition);
        f.put(scenes[i] + 0x20, before);
    }
    const Fixture original = f;
    int writes = 0, refreshes = 0, fail_write = -1, fail_refresh = -1;
    const auto write = [&](Address address, const auto& value) {
        f.put(address, value);
        return ++writes != fail_write; // memory may already be touched on failure
    };
    const auto scope = [&](const Owner& current) { return current.instance == instance; };
    const auto refresh = [&](Address scene) {
        bool ours = false;
        for (Address expected : scenes) {
            if (expected == scene)
                ours = true;
        }
        assert(ours);
        return ++refreshes != fail_refresh;
    };
    assert(set_garage_root(owner, root, f, write, scope, refresh) == RootResult::applied);
    assert(writes == 8 && refreshes == 5);
    Matrix observed{};
    std::uint8_t dirty{};
    assert(f(instance + 0x1c0, observed) && observed == root);
    assert(f(renderer + 0x1a80, observed) && observed == root);
    assert(f(renderer + 0x1ac1, dirty) && dirty == 1);
    for (unsigned i = 0; i < scenes.size(); ++i) {
        assert(f(scenes[i] + 0x20, observed));
        assert(observed[12] == 4 && observed[14] == float(i + 1));
    }
    for (int nth = 1; nth <= 8; ++nth) {
        f = original;
        writes = refreshes = 0;
        fail_write = nth;
        assert(set_garage_root(owner, root, f, write, scope, refresh) == RootResult::rolled_back);
        assert(f.saved == original.saved && refreshes == 0);
    }
    fail_write = -1;
    for (int nth = 1; nth <= 5; ++nth) {
        f = original;
        writes = refreshes = 0;
        fail_refresh = nth;
        assert(set_garage_root(owner, root, f, write, scope, refresh) == RootResult::rolled_back);
        assert(f.saved == original.saved && refreshes == nth + 5);
    }
    fail_refresh = -1;
    f = original;
    writes = refreshes = 0;
    assert(set_garage_root(owner, root, f, write, [](const Owner&) { return false; }, refresh) ==
           RootResult::unavailable);
    assert(writes == 0 && refreshes == 0);
    f.saved.erase(scenes[4] + 0x20); // no earlier scene is touched when a later read fails
    assert(set_garage_root(owner, root, f, write, scope, refresh) == RootResult::unavailable);
    assert(writes == 0 && refreshes == 0);
    f = original;
    f.put(instance + 0x1a8, Address{});
    assert(set_garage_root(owner, root, f, write, scope, refresh) == RootResult::unavailable);
    assert(writes == 0 && refreshes == 0);
    f = original;
    f.put(owner.module + 0x60, owner.pit_bank + 8);
    assert(set_garage_root(owner, root, f, write, scope, refresh) == RootResult::unavailable);
    assert(writes == 0 && refreshes == 0);
    f = original;
    assert(set_garage_root(owner, root, f, [](Address, const auto&) { return false; }, scope, refresh) ==
           RootResult::rollback_failed);
}

void test_individuals_and_vehicle()
{
    namespace sdk=swofty::garage_placement;
    using Role=sdk::Mechanic;
    using R=sdk::Result;
    constexpr Address instance=0x10000, scene=0x20000, car=0x30000, bank=0x40000, module=0x50000;
    constexpr Address resource=0x60000, list=0x70000, defs=0x80000, eb=0x90000, fl=0xa0000, prop=0xb0000;
    Owner owner{instance,scene,car,bank,2,0,module};
    Fixture f;
    Matrix identity{};
    identity[0]=identity[5]=identity[10]=identity[15]=1;
    f.put(module+0x60,bank); f.put(instance+0x11c,owner.garage);
    f.put(instance+0xc8,scene); f.put(instance+0x1a8,car);
    f.put(scene+0x20,identity); f.put(scene+0xa0,resource);
    f.put(resource+0x50,defs); f.put(resource+0x100,uint64_t{3}); f.put(scene+0x78,list);
    f.put(list,eb); f.put(list+8,fl); f.put(list+16,prop);
    auto ids=members::names([](const char *name) {
        static const std::map<std::string,uint32_t> keys{{"EB1",18},{"FL:",21},{"MB:",25},{"MB",26}};
        auto it=keys.find(name);
        return it==keys.end() ? CrewIds::missing : it->second;
    });
    f.put(eb+0x248,uint32_t{18}); f.put(fl+0x248,uint32_t{21}); f.put(prop+0x248,uint32_t{444});
    auto set_model=[&](Address actor,const Matrix &world) {
        const Address model=actor+0x100000;
        std::array<float,12> rows{};
        for (unsigned r=0;r<3;++r) for (unsigned c=0;c<4;++c) rows[r*4+c]=world[c*4+r];
        f.put(actor+0x228,model); f.put(model+0x10,rows);
    };
    for (auto actor:{eb,fl,prop}) {
        set_model(actor,identity);
        Address def=actor+0x200000;
        f.put(actor+0x538,def); f.put(def+0x40,identity);
    }
    members::Selection s{}; members::Actor selected{};
    assert(members::list(owner,ids,f,s) && s.roles.contains(Role::fl) && s.roles.contains(Role::eb1));
    assert(members::select(owner,ids,Role::mb,f,selected)==R::missing);
    assert(members::select(owner,ids,Role::eb2,f,selected)==R::missing);
    assert(sdk::mechanic_from_name("FL")==Role::fl && !sdk::mechanic_from_name("FL:") &&
           !sdk::mechanic_from_name("FL_BRAKE_PROP") && sdk::mechanic_name(Role::fl)=="FL");
    f.put(prop+0x248,uint32_t{21});
    assert(members::list(owner,ids,f,s) && !s.roles.contains(Role::fl) && s.roles.contains(Role::eb1));
    assert(members::select(owner,ids,Role::fl,f,selected)==R::ambiguous);
    f.put(prop+0x248,uint32_t{444});

    members::Edit change{};
    assert(members::prepare(owner,ids,Role::fl,f,change)==R::ok);
    change.target[12]=20;
    auto push=[&](Address actor,const Matrix &world){ assert(actor==fl);set_model(actor,world); return true; };
    assert(members::apply(owner,ids,change,f,push)==R::ok);
    Matrix observed{};
    assert(crew_detail::model_world(f,eb,observed) && observed==identity);
    assert(crew_detail::model_world(f,prop,observed) && observed==identity);
    assert(members::prepare(owner,ids,Role::fl,f,change)==R::ok && change.native==identity && change.before[12]==20);
    change.target[12]=40;
    unsigned attempts{};
    auto fail_once=[&](Address actor,const Matrix &m){push(actor,m);return ++attempts!=1;};
    assert(members::apply(owner,ids,change,f,fail_once)==R::rolled_back);
    assert(crew_detail::model_world(f,fl,observed) && observed[12]==20);
    assert(members::apply(owner,ids,change,f,[](Address,const Matrix&){return false;})==R::rollback_failed);
    change.target=change.native;
    assert(members::apply(owner,ids,change,f,push)==R::ok);
    assert(crew_detail::model_world(f,fl,observed) && observed==identity);
    std::array<std::optional<members::Edit>,members::role_count> active{};
    active[unsigned(Role::fl)]=change;
    assert(!members::conflicts_with_member(active) && !members::conflicts_with_pair(Role::fl,true));
    active[unsigned(Role::mb)]=change;
    assert(members::conflicts_with_member(active) && members::conflicts_with_pair(Role::eb1,true));

    constexpr Address vehicle_scene=0xc0000,resource_v=0xd0000,actors_v=0xe0000,defs_v=0xf0000,def=0x11000;
    f.put(instance+0xd0,vehicle_scene); f.put(instance+0xd8,Address{0x135000}); f.put(instance+0xe0,Address{}); f.put(instance+0xf0,Address{});
    f.put(vehicle_scene+0xa0,resource_v); f.put(vehicle_scene+0x78,actors_v); f.put(actors_v,car);
    f.put(resource_v+0x100,uint64_t{1}); f.put(resource_v+0x50,defs_v); f.put(defs_v,def); f.put(def+0x38,uint32_t{34});
    f.put(vehicle_scene+0x20,identity); f.put(car+0x228,Address{0x130000});
    f.put(car+0x678,int32_t{3}); f.put(car+0x660,uint8_t{1});
    auto base_car=identity; base_car[13]=.4f; f.put(car+0x620,base_car);
    vehicle::Pose prior{};
    assert(vehicle::capture(owner,34,f,prior) && prior.car_world==base_car);
    f.put(instance+0xd8,vehicle_scene); assert(!vehicle::capture(owner,34,f,prior)); f.put(instance+0xd8,Address{0x135000});
    f.put(resource_v+0x100,uint64_t{2}); assert(!vehicle::capture(owner,34,f,prior)); f.put(resource_v+0x100,uint64_t{1});
    f.put(actors_v,eb); assert(!vehicle::capture(owner,34,f,prior)); f.put(actors_v,car);
    assert(!vehicle::capture(owner,35,f,prior));
    Matrix target=base_car, relative{};
    target[0]=target[10]=0;target[2]=-1;target[8]=1;target[12]=46;target[14]=2;
    assert(vehicle::relative(prior.car_world,target,relative));
    assert(members::agrees(rig_detail::world_from_definition(base_car,relative),target));
    unsigned writes{},refreshes{};
    auto write=[&](Address p,const Matrix &m){assert(p==vehicle_scene+0x20);++writes;f.put(p,m);return true;};
    auto refresh=[&](Address s){assert(s==vehicle_scene);++refreshes;return true;};
    assert(vehicle::assign(owner,34,prior,relative,f,write,refresh)==R::pending && writes==1 && refreshes==1);
    assert(f(scene+0x20,observed) && observed==identity && crew_detail::model_world(f,fl,observed) && observed==identity);
    unsigned refresh_count{};
    auto fail_refresh=[&](Address){return ++refresh_count!=1;};
    assert(vehicle::assign(owner,34,prior,relative,f,write,fail_refresh)==R::rolled_back && f(vehicle_scene+0x20,observed) && observed==identity);
    vehicle::Change change_car{prior,prior,target,vehicle::Step::verify,1000,false,0};
    assert(vehicle::verify(change_car,&base_car,1010)==vehicle::Verification::wait);
    assert(vehicle::verify(change_car,&target,1020)==vehicle::Verification::active);
    change_car.step=vehicle::Step::verify;
    assert(vehicle::verify(change_car,nullptr,3100)==vehicle::Verification::revert);
    change_car.step=vehicle::Step::rollback;change_car.expected=base_car;change_car.prior_override=true;
    assert(vehicle::verify(change_car,&base_car,3150)==vehicle::Verification::previous && change_car.step==vehicle::Step::active);
    change_car.step=vehicle::Step::restore;
    assert(vehicle::verify(change_car,&base_car,3160)==vehicle::Verification::restored);
    assert(vehicle::verify(change_car,&target,8000)==vehicle::Verification::failed);
}

void test_build_scope_and_parked_car()
{
    namespace b = runtime::garage_placement::steam_26566308;
    constexpr auto sha = "46009bd31f49d93a92d73cbd0b66131d2b37964a92da7293168d3c3a872d9389";
    assert(b::supported("steam", sha) && !b::supported("ea", sha) && !b::supported("steam", "new-build"));
    swofty::garage_placement::Pit pits[2]{};
    for (auto &pit : pits)
        pit.world.values[0] = pit.world.values[5] = pit.world.values[10] = pit.world.values[15] = 1;
    pits[0].garage = 31;
    pits[1].garage = 2;
    b::Bank bank{};
    assert(b::authored(pits, 2, 31, bank) && !b::authored(pits, 2, 1, bank));
    pits[1].garage = 31;
    assert(!b::authored(pits, 2, 31, bank));
    Owner owner{0x1000, 0x1100, 0x1200, 0x2000, 31, 1, 0x3000};
    Fixture f;
    f.put(owner.module + 0x60, owner.pit_bank);
    for (unsigned i = 0; i < 32; ++i) {
        Address record = (i == 2 || i == 31) ? (0x10000 + 0x100 * i) : 0;
        f.put(owner.pit_bank + 0x838 + sizeof(Address) * i, record);
        if (record) f.put(record + 0x20, bank.matrices[i]);
    }
    assert(b::matches(f, owner, bank));
    f.put(owner.pit_bank + 0x838, Address{0xbeef});
    assert(!b::matches(f, owner, bank));
    f.put(owner.pit_bank + 0x838, Address{});
    auto m = bank.matrices[31];
    m[12] += .0006f;
    f.put(0x11f20, m);
    assert(!b::matches(f, owner, bank));
    f.put(owner.car_actor + 0x228, Address{0x4000});
    f.put(owner.car_actor + 0x678, int32_t{0});
    f.put(owner.car_actor + 0x660, uint8_t{1});
    f.put(owner.car_actor + 0x620, m);
    Matrix observed{};
    assert(parked_car_world(f, owner, observed) && observed == m);
    // Without the cached result, reproduce selection of evaluated and prior frame buffers.
    f.put(owner.car_actor + 0x660, uint8_t{});
    f.put(0x40d0, Address{0x5000});
    f.put(0x5058, Address{0x6000});
    f.put(0x6000, uint8_t{0});
    f.put(0x5030, Address{0x7000});
    f.put(0x5018, Address{0x8000});
    f.put(0x5028, Address{0x9000});
    std::array<float, 12> identity{};
    identity[0] = identity[5] = identity[10] = 1;
    f.put(0x4010, identity);
    identity[3] = 11;
    f.put(0x8000, identity);
    identity[3] = 12;
    f.put(0x9000, identity);
    f.put(0x7000, uint64_t{1});
    assert(parked_car_world(f, owner, observed) && observed[12] == 11);
    f.put(0x7000, uint64_t{});
    assert(parked_car_world(f, owner, observed) && observed[12] == 12);
    f.put(0x6000, uint8_t{3});
    assert(!parked_car_world(f, owner, observed));
}

int main()
{
    constexpr Address instance = 0x10000, scene = 0x20000, stack = 0x30000, module = 0x40000;
    constexpr Address bank = 0x50000, car = 0x60000, list = 0x70000, resource = 0x80000;
    constexpr Address mb = 0x90000, eb1 = 0xa0000, pit = 0xb0000;
    Fixture f;
    f.put(instance + 0xc8, scene);
    f.put(stack + 0xc0, module);
    f.put(module + 0x60, bank);
    f.put(instance + 0x11c, std::uint32_t{0});
    f.put(stack + 0xa8, std::uint32_t{1});
    f.put(instance + 0x1a8, car);
    Owner owner;
    assert(owner_at_scene_read(f, instance, scene, stack, owner));
    assert(owner.car_actor == car && owner.garage == 0 && owner.native_slot == 1 && owner.pit_bank == bank);
    assert(!owner_at_scene_read(f, instance, scene + 1, stack, owner));
    f.put(stack + 0xa8, std::uint32_t{2});
    assert(!owner_at_scene_read(f, instance, scene, stack, owner));
    // Both failed calls leave the last complete result intact.
    assert(owner.native_slot == 1);
    Matrix matrix{}, out{};
    matrix[0] = matrix[5] = matrix[10] = matrix[15] = 1;
    matrix[12] = 327.735f;
    f.put(bank + 0x838, pit);
    f.put(pit + 0x20, matrix);
    assert(pit_transform(f, owner, 0, out) == PitState::present && out == matrix);
    assert(pit_transform(f, owner, 1, out) == PitState::unreadable);
    f.put(bank + 0x838 + sizeof(Address), Address{});
    assert(pit_transform(f, owner, 1, out) == PitState::absent);
    assert(pit_transform(f, owner, 32, out) == PitState::unreadable);
    matrix[3] = std::numeric_limits<float>::quiet_NaN();
    f.put(pit + 0x20, matrix);
    assert(pit_transform(f, owner, 0, out) == PitState::unreadable);

    CrewIds ids{17, 21, 18};
    f.put(scene + 0xa0, resource);
    f.put(scene + 0x78, list);
    f.put(resource + 0x100, std::uint64_t{3});
    f.put(list, eb1);
    f.put(list + sizeof(Address), Address{});
    f.put(list + 2 * sizeof(Address), mb);
    f.put(eb1 + 0x248, ids.eb1);
    f.put(mb + 0x248, ids.mb);
    CentralCrew actors{};
    assert(central_crew(f, owner, ids, actors) && actors.mb == mb && actors.eb1 == eb1);
    f.put(mb + 0x248, ids.mb_variant);
    assert(central_crew(f, owner, ids, actors));
    assert(central_crew(f, owner, CrewIds{CrewIds::missing, 21, 18}, actors));
    f.put(list + sizeof(Address), mb);
    assert(!central_crew(f, owner, ids, actors));
    f.put(list + sizeof(Address), Address{});
    f.put(instance + 0xc8, scene + 8);
    assert(!central_crew(f, owner, ids, actors));
    test_individuals_and_vehicle();
    test_build_scope_and_parked_car();
    test_root_transaction();
    test_central_crew();
}
