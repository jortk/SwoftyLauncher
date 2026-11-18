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
 for(unsigned id=0;id<32;id++) if(present[id]) {
  const auto& c=cars[id];
  api->log("[live-build-check] car=%u have=0x%llx pos=%d status=%u actual=%u visual=%u phase=%u pit=%u stops=%u flags=0x%x",
           c.vehicle,c.have,c.position,c.result_status,c.tyre_actual,c.tyre_visual,c.pit_state,c.pit_status,c.pit_stops,c.flags);
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
