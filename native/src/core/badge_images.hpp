#pragma once
#include <map>
#include <swofty/live_images.h>
#include <string>

namespace badge_images {
const SwoftyLiveImages *api();
void session_unloaded();
void trace(const char *message);
// These are receipts from the active router and accepted overlay copies. They
// deliberately do not enumerate logos_*.manifest or choose cached files by age.
void file_routes(const std::map<std::string, std::string> &routes);
void overlay_routes(const std::map<std::string, std::string> &routes);
}
