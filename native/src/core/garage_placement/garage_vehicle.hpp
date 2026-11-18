// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once
#include "garage_members.hpp"
namespace runtime::garage_placement::vehicle {
using Result = swofty::garage_placement::Result;
struct Pose { Address scene{}; Matrix scene_world{}, car_world{}; };
enum class Step { verify, active, restore, rollback };
struct Change {
    Pose native, prior;
    Matrix expected{};
    Step step{Step::verify};
    uint64_t since{};
    bool prior_override{};
    unsigned observations{};
};

template<class Read>
bool capture(const Owner &o, uint32_t vehicle_name, Read &&read, Pose &out)
{
    if (vehicle_name==CrewIds::missing || !steam_26566308::owns(read,o)) return false;
    Address scene{}, resource{}, list{}, actor{}, defs{}, def{};
    uint64_t count{}; uint32_t name{};
    Pose pose{};
    if (!read(o.instance+0xd0,scene) || !scene || scene==o.crew_scene ||
        !read(scene+0xa0,resource) || !resource || !read(resource+0x100,count) || count!=1 ||
        !read(scene+0x78,list) || !list || !read(list,actor) || actor!=o.car_actor ||
        !read(resource+0x50,defs) || !defs || !read(defs,def) || !def ||
        !read(def+0x38,name) || name!=vehicle_name) return false;
    for (unsigned field : {0xd8u,0xe0u,0xf0u}) {
        Address other{};
        if (!read(o.instance+field,other) || other==scene) return false;
    }
    if (!read(scene+0x20,pose.scene_world) || !rig_detail::affine(pose.scene_world) ||
        !parked_car_world(read,o,pose.car_world) || !rig_detail::affine(pose.car_world)) return false;
    pose.scene=scene;
    out=pose;
    return true;
}
// Solve A * result = B for the row-vector convention. Apply the same relative
// world transform to the vehicle-only scene to place its evaluated bone.
inline bool relative(const Matrix &a,const Matrix &b,Matrix &result)
{
    if (!rig_detail::affine(a) || !rig_detail::affine(b)) return false;
    double m[4][8]{};
    for (unsigned r=0; r<4; ++r) {
        std::copy_n(a.begin()+r*4,4,m[r]);
        std::copy_n(b.begin()+r*4,4,m[r]+4);
    }
    for (unsigned c=0; c<4; ++c) {
        unsigned p=c;
        for (unsigned r=c+1; r<4; ++r) if (std::abs(m[r][c])>std::abs(m[p][c])) p=r;
        if (std::abs(m[p][c])<1.e-8) return false;
        if (p!=c) std::swap(m[p],m[c]);
        const auto pivot=m[c][c];
        for (auto &x : m[c]) x/=pivot;
        for (unsigned r=0; r<4; ++r) if (r!=c) {
            const auto factor=m[r][c];
            for (unsigned k=0;k<8;++k) m[r][k]-=factor*m[c][k];
        }
    }
    for (unsigned r=0;r<4;++r) for (unsigned c=0;c<4;++c) result[r*4+c]=float(m[r][4+c]);
    return rig_detail::affine(result);
}
template<class Read,class Write,class Refresh>
Result assign(const Owner &o,uint32_t name,const Pose &before,const Matrix &world,Read &&read,Write &&write,Refresh &&refresh)
{
    if (!rig_detail::affine(world)) return Result::invalid;
    Pose current{};
    if (!capture(o,name,read,current) || current.scene!=before.scene) return Result::unavailable;
    auto store = [&](const Matrix &target) {
        Matrix readback{};
        return write(current.scene+0x20,target) && refresh(current.scene) && read(current.scene+0x20,readback) &&
               members::agrees(readback,target);
    };
    if (store(world)) return Result::pending;
    return store(before.scene_world) ? Result::rolled_back : Result::rollback_failed;
}
inline bool reached(const Matrix &car,const Matrix &expected)
{
    if (crew_detail::xyz_error(car,expected)>.02) return false;
    for (unsigned i=0;i<12;++i) if (std::abs(car[i]-expected[i])>.009) return false;
    return true;
}
enum class Verification { wait, active, restored, previous, revert, failed };
inline Verification verify(Change &c,const Matrix *observed,uint64_t now)
{
    if (c.step==Step::active) return Verification::active;
    if (observed && reached(*observed,c.expected)) {
        if (c.step==Step::restore) return Verification::restored;
        if (c.step==Step::rollback) {
            if (c.prior_override) c.step=Step::active;
            return Verification::previous;
        }
        c.step=Step::active;
        return Verification::active;
    }
    if (++c.observations<2 || now-c.since<2000) return Verification::wait;
    return c.step==Step::verify ? Verification::revert : Verification::failed;
}
} // namespace runtime::garage_placement::vehicle
