// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#pragma once
#include "steam_26566308.hpp"
namespace runtime::garage_placement::members {
namespace sdk = swofty::garage_placement;
using Role = sdk::Mechanic;
using Result = sdk::Result;
constexpr unsigned role_count = unsigned(Role::count);
inline bool conflicts_with_pair(Role role, bool pair) { return pair && (role==Role::mb || role==Role::eb1); }
template<class Edits> bool conflicts_with_member(const Edits &edits)
{
    return edits[unsigned(Role::mb)] || edits[unsigned(Role::eb1)];
}
struct Names { uint32_t primary{CrewIds::missing}, alternate{CrewIds::missing}; };
using NamesByRole = std::array<Names, role_count>;
template<class Node> NamesByRole names(Node node)
{
    // MACRO_CREW[_LOW_QUALITY] instances in garagecrew / lowq_garagecrew.
    // EB2 has animation clips but no named human actor in those scene definitions.
    static constexpr const char *literal[] = {"MB", "EB1", "FJ1", "FJ2", "FL:", "FR:", "RJ:", "RL:", "RR:", "ENG", nullptr};
    NamesByRole out{};
    for (unsigned i = 0; i < role_count; ++i)
        if (literal[i]) out[i].primary = node(literal[i]);
    out[unsigned(Role::mb)].alternate = node("MB:");
    return out;
}
struct Actor { Address value{}; unsigned index{}; };
struct Selection {
    sdk::Mechanics roles{};
    std::array<Actor, role_count> actors{};
};
template<class Read>
bool list(const Owner &owner, const NamesByRole &ids, Read &&read, Selection &out)
{
    if (!steam_26566308::owns(read, owner)) return false;
    Address resource{}, actors{};
    uint64_t count{};
    if (!read(owner.crew_scene+0xa0,resource) || !resource || !read(resource+0x100,count) ||
        count>256 || (count && (!read(owner.crew_scene+0x78,actors) || !actors))) return false;
    Selection found{};
    for (unsigned i = 0; i < count; ++i) {
        Address actor{}; uint32_t name{};
        if (!read(actors+sizeof(Address)*i, actor)) return false;
        if (!actor) continue;
        if (!read(actor+0x248, name)) return false;
        if (name==CrewIds::missing) continue;
        uint32_t candidates{};
        for (unsigned k=0; k<role_count; ++k)
            if (name==ids[k].primary || name==ids[k].alternate) candidates |= 1u<<k;
        if (candidates & (candidates-1u)) { found.roles.ambiguous |= candidates; continue; }
        for (unsigned k=0; k<role_count; ++k) {
            const uint32_t bit=1u<<k;
            if (!(candidates & bit)) continue;
            if (found.actors[k].value) found.roles.ambiguous |= bit;
            else found.actors[k]={actor,i};
            found.roles.present |= bit;
        }
    }
    found.roles.present &= ~found.roles.ambiguous;
    out=found;
    return true;
}
template<class Read>
Result select(const Owner &owner, const NamesByRole &ids, Role role, Read &&read, Actor &out)
{
    if (unsigned(role)>=role_count) return Result::invalid;
    Selection s{};
    if (!list(owner,ids,read,s)) return Result::unavailable;
    const auto bit=1u<<unsigned(role);
    if (s.roles.ambiguous & bit) return Result::ambiguous;
    if (!(s.roles.present & bit)) return Result::missing;
    out=s.actors[unsigned(role)];
    return Result::ok;
}
struct Edit { Role role{}; Actor actor{}; Matrix native{}, before{}, target{}; };
template<class Read>
Result prepare(const Owner &o, const NamesByRole &ids, Role role, Read &&read, Edit &out)
{
    Edit e{};
    e.role=role;
    Result r=select(o,ids,role,read,e.actor);
    if (r!=Result::ok) return r;
    Address resource{}, defs{}, definition{};
    Matrix local{}, scene{};
    if (!read(e.actor.value+0x538,definition) ||
        (!definition && (!read(o.crew_scene+0xa0,resource) || !resource || !read(resource+0x50,defs) || !defs ||
                         !read(defs+sizeof(Address)*e.actor.index,definition))) || !definition ||
        !read(definition+0x40,local) || !rig_detail::affine(local) ||
        !read(o.crew_scene+0x20,scene) || !rig_detail::affine(scene) ||
        !crew_detail::model_world(read,e.actor.value,e.before)) return Result::unavailable;
    e.native=rig_detail::world_from_definition(local,scene);
    if (!rig_detail::affine(e.native)) return Result::unavailable;
    e.target=e.native;
    out=e;
    return Result::ok;
}
inline bool agrees(const Matrix &a, const Matrix &b)
{
    if (crew_detail::xyz_error(a,b)>.001) return false;
    for (unsigned i=0;i<12;++i) if (std::abs(a[i]-b[i])>.0001) return false;
    return true;
}
template<class Read,class Push>
Result apply(const Owner &o, const NamesByRole &ids, const Edit &change, Read &&read, Push &&push)
{
    if (!rig_detail::affine(change.target)) return Result::invalid;
    Actor initial{};
    const auto initial_status=select(o,ids,change.role,read,initial);
    if (initial_status!=Result::ok) return initial_status;
    if (initial.value!=change.actor.value || initial.index!=change.actor.index) return Result::unavailable;
    auto assign = [&](const Matrix &world) {
        Actor current{};
        Matrix actual{};
        return select(o,ids,change.role,read,current)==Result::ok && current.value==change.actor.value &&
               current.index==change.actor.index && push(current.value,world) &&
               crew_detail::model_world(read,current.value,actual) && agrees(actual,world);
    };
    if (assign(change.target)) return Result::ok;
    return assign(change.before) ? Result::rolled_back : Result::rollback_failed;
}
} // namespace runtime::garage_placement::members
