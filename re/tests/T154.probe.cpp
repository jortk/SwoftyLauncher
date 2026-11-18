// SPDX-License-Identifier: MIT
// Test consumer only: uses the core API exclusively, with no independent readers.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <initializer_list>
#include <swofty_api.h>

namespace {
const SwoftyApi* api;
const SwoftyLive* live;
uint64_t next_sample;
void event(const SwoftyEvent* e,void*) {
 if(e&&e->name) api->log("[live-build-check] event %s",e->name);
}
void frame(double,void*) {
 const auto now=GetTickCount64();if(now<next_sample)return;next_sample=now+5000;
 if(!live||!live->available||!live->available()) {api->log("[live-build-check] unavailable");return;}
 SwoftyLiveSession s{};uint64_t before=0,after=0;
 if(!live->session||!live->car||!live->seq||live->seq(&before)!=SWOFTY_ASSET_OK||live->session(&s)!=SWOFTY_ASSET_OK) {
  api->log("[live-build-check] snapshot read failed");return;
 }
 SwoftyLiveCar cars[32]{};bool present[32]{};
 for(unsigned id=0;id<32;id++) present[id]=live->car(id,&cars[id])==SWOFTY_ASSET_OK;
 if(live->seq(&after)!=SWOFTY_ASSET_OK||before!=after){api->log("[live-build-check] snapshot changed; retry next sample");return;}
 api->log("[live-build-check] snapshot seq=%llu session_have=0x%llx cars=%u player=%d",before,s.have,s.cars,s.player);
 api->log("[live-build-check] session type=%u racers=%u watched=%d subject=%d generation=%u track=\"%s\" length=%.1f mode=%u safety=%u red=%u yellow=%u sectors=%u classified=%u on_track=%u",
          s.type,s.racers,s.watched,s.subject,s.generation,s.track_name,s.track_length_m,s.game_mode,s.safety_car,s.red_flag,s.yellow_sectors,s.sector_count,s.classified,s.on_track);
 SwoftyLiveCar subject{};int subject_result=live->subject?live->subject(&subject):SWOFTY_ASSET_UNAVAILABLE;
 api->log("[live-build-check] subject result=%d vehicle=%u have=0x%llx",subject_result,subject.vehicle,subject.have);
 uint32_t turns=0;int turns_result=live->track_turns?live->track_turns(&turns,nullptr,0):SWOFTY_ASSET_UNAVAILABLE;
 SwoftyLiveTurn list[64]{};
 int list_result=turns_result==SWOFTY_ASSET_OK?live->track_turns(&turns,list,64):turns_result;
 api->log("[live-build-check] track_turns count_result=%d list_result=%d count=%u first=\"%s\"",turns_result,list_result,turns,list[0].label);
 for(unsigned id=0;id<32;id++) if(present[id]) {
  const auto& c=cars[id];
  api->log("[live-build-check] car=%u have=0x%llx pos=%d status=%u actual=%u visual=%u phase=%u pit=%u stops=%u flags=0x%x",
           c.vehicle,c.have,c.position,c.result_status,c.tyre_actual,c.tyre_visual,c.pit_state,c.pit_status,c.pit_stops,c.flags);
  api->log("[live-build-check] motion car=%u speed=%.2f rpm=%.0f limit=%.0f gear=%d throttle=%.3f brake=%.3f clutch=%.3f steer=%.3f g_lat=%.3f g_long=%.3f drs=%u world=%.2f,%.2f,%.2f velocity=%.2f,%.2f,%.2f",
           id,c.speed,c.rpm,c.rpm_limit,c.gear,c.throttle,c.brake,c.clutch,c.steer,c.g_lat,c.g_long,c.drs,c.world_pos[0],c.world_pos[1],c.world_pos[2],c.world_vel[0],c.world_vel[1],c.world_vel[2]);
  api->log("[live-build-check] ers car=%u store=%.0f capacity=%.0f mode=%d deploy=%.0f harvest_k=%.0f harvest_h=%.0f max_deploy=%.0f harvested_lap=%.0f deployed_lap=%.0f limit_lap=%.0f",
           id,c.ers_store,c.ers_capacity,c.ers_mode,c.ers_deploy,c.ers_harvest_k,c.ers_harvest_h,c.ers_max_deploy,c.ers_harvested_lap,c.ers_deployed_lap,c.ers_harvest_limit_lap);
  api->log("[live-build-check] progress car=%u lap=%d lap_m=%.2f total_m=%.2f sector=%u leader_ms=%d ahead_ms=%d penalty_s=%.2f dt=%u sg=%u warnings=%u limits=%u",
           id,c.lap,c.lap_m,c.total_m,c.sector,c.gap_leader_ms,c.gap_ahead_ms,c.time_penalty_s,c.drive_through,c.stop_go,c.warnings,c.track_limit_warnings);
  char driver[128]{},badge[48]{};
  int dr=live->version>=2&&live->driver_name?live->driver_name(id,driver,sizeof driver):SWOFTY_ASSET_UNAVAILABLE;
  int br=live->team_badge?live->team_badge(id,badge,sizeof badge):SWOFTY_ASSET_UNAVAILABLE;
  api->log("[live-build-check] identity car=%u driver_result=%d driver=\"%s\" badge_result=%d badge=\"%s\"",id,dr,driver,br,badge);
  if(int(id)==s.subject) {
   SwoftyLiveTurn turn{};int tr=live->turn?live->turn(id,&turn):SWOFTY_ASSET_UNAVAILABLE;
   api->log("[live-build-check] turn car=%u result=%d label=\"%s\" distance=%.1f apex=%.1f",id,tr,turn.label,turn.distance_ahead_m,turn.apex_lap_m);
  }
 }
}
void load() {
 auto module=GetModuleHandleW(L"swofty_core.dll");
 auto get=module?reinterpret_cast<const SwoftyLive*(*)()>(GetProcAddress(module,"SwoftyCoreLive")):nullptr;
 live=get?get():nullptr;if(live&&live->version<1)live=nullptr;
 api->log("[live-build-check] API-only probe loaded, table version=%u",live?live->version:0);
 uint32_t token=0;
 if(api->events&&api->events->on_frame)api->events->on_frame(frame,nullptr,&token);
 if(api->events&&api->events->subscribe)for(const char* name:{"session.start","session.end","session.unload","pause","resume"})api->events->subscribe(name,event,nullptr,&token);
}
}
extern "C" __declspec(dllexport) const uint32_t SwoftyAddonApi=SWOFTY_API_VERSION;
extern "C" __declspec(dllexport) int SwoftyAddonInit(const SwoftyApi* value) {
 if(!value||value->version<SWOFTY_API_VERSION||!value->register_mixins||!value->log)return 0;
 api=value;SwoftyMixin mixin{nullptr,SWOFTY_MIXIN_ON_LOAD,0,reinterpret_cast<void*>(load),nullptr};
 return api->register_mixins(&mixin,1);
}
