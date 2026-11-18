#pragma once

#include <cstddef>
#include <string>
#include <utility>
#include <vector>

#include "facts.hpp"
#include "ids.hpp"
#include "room.hpp"

namespace swofty_core::apply::moments {

void attach();
std::size_t boot_plan();
bool boot_work();
void boot_load(room::Report &report);
void boot_without_database(room::Report report);
void write_room(const room::Report &report);
void status(const std::string &addon, const std::string &line);
void left_out(const std::string &addon, int teams, int vehicles, int lists);

void career_loaded(const char *key, unsigned mode);
void career_unloaded();
void career_season(int year);
void career_renamed(const char *from, const char *to);
void box_picked(const char *entrant_list);
void box_picked(const char *entrant_list, const void *mode_context);
void session_load(const char *source);
std::pair<std::string, std::string> session_mode_track();
// The kind of game mode the game is in: kept by a watch on its own thread (started by mode_watch) and by every session load.
facts::ModeKind mode_kind();
void mode_watch();
void grand_prix_track_check();
std::string current_mode(const char *where);
// The tracks table label of a stat id once the table was read in this run, else empty; never reads the database.
std::string cached_track_label(int stat_id);
void session_end(const char *source);
void window();
int write_reports(const char *when);
void crash_reports();

ids::CareerMap map_for_career(const std::string &career);
std::vector<ids::Clash> adopt(const std::string &career, const ids::CareerMap &map);

}
