/* SPDX-License-Identifier: MIT */
#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SWOFTY_API_VERSION 9

enum {
    SWOFTY_MIXIN_WRAP = 1,
    SWOFTY_MIXIN_SITE = 2,
    SWOFTY_MIXIN_VTABLE = 3,
    SWOFTY_MIXIN_REDIRECT = 4,
    SWOFTY_MIXIN_PATCH = 5,
    SWOFTY_MIXIN_ON_LOAD = 6,
};
#define SWOFTY_MIXIN_OVERWRITE 0x100u
#define SWOFTY_MIXIN_KIND(k) ((k) & 0xffu)
#define SWOFTY_MIXIN_ARG(k) ((k) >> 16)
#define SWOFTY_MIXIN_WITH_ARG(k, arg) ((uint32_t)(k) | (uint32_t)(arg) << 16)

typedef struct SwoftyMixin {
    const char *target;
    uint32_t kind;
    int32_t priority;
    void *handler;
    void **orig;
} SwoftyMixin;

typedef union SwoftyXmm {
    uint8_t u8[16];
    uint32_t u32[4];
    uint64_t u64[2];
    float f32[4];
    double f64[2];
} SwoftyXmm;

typedef struct SwoftyContext {
    SwoftyXmm xmm0, xmm1, xmm2, xmm3, xmm4, xmm5, xmm6, xmm7, xmm8, xmm9, xmm10, xmm11, xmm12, xmm13, xmm14, xmm15;
    uintptr_t rflags, r15, r14, r13, r12, r11, r10, r9, r8, rdi, rsi, rdx, rcx, rbx, rax, rbp, rsp, trampoline_rsp, rip;
} SwoftyContext;

enum {
    SWOFTY_SETTING_TOGGLE = 1,
    SWOFTY_SETTING_SLIDER = 2,
    SWOFTY_SETTING_CHOICE = 3,
};

typedef struct SwoftySettingValue {
    uint32_t type;
    int32_t toggle;
    double slider;
    const char *choice;
} SwoftySettingValue;

typedef void (*SwoftySettingFn)(const char *id, const SwoftySettingValue *value, void *user);

enum {
    SWOFTY_ASSET_OK = 0,
    SWOFTY_ASSET_LATE = 1,
    SWOFTY_ASSET_UNAVAILABLE = 2,
    SWOFTY_ASSET_INVALID = 3,
};

typedef struct SwoftyFragment {
    const char *file;
    const char *fourcc;
    uint32_t flags;
} SwoftyFragment;

typedef struct SwoftyAssets {
    int (*available)(const char *feature);
    int (*register_group)(const char *group, const char *const *erps, size_t erp_count, const char *const *deps,
                          size_t dep_count, const char *templ);
    int (*put_resource)(const char *uri, const SwoftyFragment *fragments, size_t fragment_count, const char *const *erps,
                        size_t erp_count);
    int (*db_set)(const char *key, const char *type, const char *value);
    int (*db_set_raw)(uint64_t hash, uint8_t tag, const void *value, size_t size);
    int (*lang_set)(const char *language, const char *key, const char *text);
    int (*lang_table)(const char *language, const char *file);
    int (*overlay_file)(const char *game_path, const char *file);
} SwoftyAssets;

/* The standard-library tables below use the SWOFTY_ASSET_* result codes: ok, late,
   unavailable (older core, or the feature is off), invalid (bad argument). Each table's
   available() returns non-zero only when its feature is on and the core implements it. */

#define SWOFTY_EVENT_SESSION_START "session.start"
#define SWOFTY_EVENT_GARAGE_PARKED_CAR "garage.parked_car.evaluated"
#define SWOFTY_EVENT_SESSION_END "session.end"
#define SWOFTY_EVENT_SESSION_UNLOAD "session.unload"
#define SWOFTY_EVENT_RACE_START "race.start"
#define SWOFTY_EVENT_LAP "lap"
#define SWOFTY_EVENT_FLAG "flag"
#define SWOFTY_EVENT_STATE "state"
#define SWOFTY_EVENT_SCREEN "screen"
#define SWOFTY_EVENT_PAUSE "pause"
#define SWOFTY_EVENT_RESUME "resume"
#define SWOFTY_EVENT_RESULTS "results"
#define SWOFTY_EVENT_FLOW_STATE "flow.state"
#define SWOFTY_EVENT_CAREER_LOADED "career.loaded"
#define SWOFTY_EVENT_CAREER_UNLOADED "career.unloaded"
#define SWOFTY_EVENT_CAREER_HUB "career.hub"
#define SWOFTY_EVENT_CAREER_STARTING "career.starting"
#define SWOFTY_EVENT_TELEMETRY_OVERTAKE "telemetry.overtake"
#define SWOFTY_EVENT_TELEMETRY_COLLISION "telemetry.collision"
#define SWOFTY_EVENT_TELEMETRY_RETIREMENT "telemetry.retirement"

typedef struct SwoftyEvent {
    const char *name;
    uint64_t frame;
    double time;
    int64_t a, b;
    const void *data;
} SwoftyEvent;

typedef void (*SwoftyEventFn)(const SwoftyEvent *ev, void *user);
typedef void (*SwoftyFrameFn)(double dt, void *user);

typedef struct SwoftyEvents {
    int (*available)(void);
    int (*subscribe)(const char *event, SwoftyEventFn fn, void *user, uint32_t *token);
    int (*on_frame)(SwoftyFrameFn fn, void *user, uint32_t *token);
    int (*unsubscribe)(uint32_t token);
} SwoftyEvents;

typedef struct SwoftyPaint SwoftyPaint;
typedef void (*SwoftyPaintFn)(SwoftyPaint *paint, void *user);

/* Overlay v2: the SwoftyOverlay entries after image() exist when the running swofty_core.dll exports
   SwoftyCoreOverlayVersion and it returns 2 or more (swofty/overlay.hpp checks); the v3 entries after
   ui_scale() exist when it returns 3 or more. A text style and an input snapshot are read and written
   up to their size field, so later fields can be added. */
#define SWOFTY_OVERLAY_VERSION 3

enum {
    SWOFTY_TEXT_LEFT = 0x00,
    SWOFTY_TEXT_CENTER = 0x01,
    SWOFTY_TEXT_RIGHT = 0x02,
    SWOFTY_TEXT_TOP = 0x00,
    SWOFTY_TEXT_MIDDLE = 0x10,
    SWOFTY_TEXT_BASELINE = 0x20,
    SWOFTY_TEXT_BOTTOM = 0x30,
};

enum {
    SWOFTY_GRADIENT_VERTICAL = 0,
    SWOFTY_GRADIENT_HORIZONTAL = 1,
};

enum {
    SWOFTY_POLYLINE_CLOSED = 1,
};

enum {
    SWOFTY_MOUSE_LEFT = 1,
    SWOFTY_MOUSE_RIGHT = 2,
    SWOFTY_MOUSE_MIDDLE = 4,
};

/* Gamepad buttons in SwoftyInput.pad_buttons / pad_pressed: the XInput XINPUT_GAMEPAD_* bits. */
enum {
    SWOFTY_PAD_UP = 0x0001,
    SWOFTY_PAD_DOWN = 0x0002,
    SWOFTY_PAD_LEFT = 0x0004,
    SWOFTY_PAD_RIGHT = 0x0008,
    SWOFTY_PAD_START = 0x0010,
    SWOFTY_PAD_BACK = 0x0020,
    SWOFTY_PAD_LTHUMB = 0x0040,
    SWOFTY_PAD_RTHUMB = 0x0080,
    SWOFTY_PAD_LB = 0x0100,
    SWOFTY_PAD_RB = 0x0200,
    SWOFTY_PAD_A = 0x1000,
    SWOFTY_PAD_B = 0x2000,
    SWOFTY_PAD_X = 0x4000,
    SWOFTY_PAD_Y = 0x8000,
};

typedef struct SwoftyInput {
    uint32_t size;
    uint32_t focused;
    float x, y;
    float wheel;
    uint32_t buttons;
    uint32_t pressed;
    uint32_t released;
    uint32_t char_count;
    uint32_t chars[16];
    uint8_t keys_down[32];
    uint8_t keys_pressed[32];
    uint8_t keys_typed[32];
    uint32_t pad_connected;
    uint32_t pad_buttons;
    uint32_t pad_pressed;
    float pad_lx, pad_ly, pad_rx, pad_ry;
    float pad_lt, pad_rt;
} SwoftyInput;

typedef struct SwoftyTextStyle {
    uint32_t size;
    uint32_t align;
    float px;
    uint32_t rgba;
    float tracking;
    float line_height;
    uint32_t outline_rgba;
    float outline_px;
    uint32_t shadow_rgba;
    float shadow_dx, shadow_dy, shadow_blur;
    float max_width;
    uint32_t font;
} SwoftyTextStyle;

typedef struct SwoftyOverlay {
    int (*available)(void);
    int (*on_paint)(SwoftyPaintFn fn, void *user, uint32_t *token);
    int (*remove)(uint32_t token);
    void (*size)(const SwoftyPaint *paint, float *width, float *height);
    void (*rect)(SwoftyPaint *paint, float x, float y, float w, float h, uint32_t rgba);
    void (*rect_outline)(SwoftyPaint *paint, float x, float y, float w, float h, float thickness, uint32_t rgba);
    void (*line)(SwoftyPaint *paint, float x0, float y0, float x1, float y1, float thickness, uint32_t rgba);
    void (*text)(SwoftyPaint *paint, float x, float y, float size, uint32_t rgba, const char *utf8);
    void (*measure)(const SwoftyPaint *paint, float size, const char *utf8, float *width, float *height);
    int (*load_image)(const char *id, const char *file);
    int (*image)(SwoftyPaint *paint, const char *id, float x, float y, float w, float h, uint32_t tint);
    /* v2 */
    void (*rect_rounded)(SwoftyPaint *paint, float x, float y, float w, float h, float radius, uint32_t rgba);
    void (*rect_rounded_outline)(SwoftyPaint *paint, float x, float y, float w, float h, float radius, float thickness,
                                 uint32_t rgba);
    void (*rect_gradient)(SwoftyPaint *paint, float x, float y, float w, float h, float radius, uint32_t from,
                          uint32_t to, uint32_t flags);
    void (*circle)(SwoftyPaint *paint, float cx, float cy, float radius, uint32_t rgba);
    void (*circle_outline)(SwoftyPaint *paint, float cx, float cy, float radius, float thickness, uint32_t rgba);
    void (*arc)(SwoftyPaint *paint, float cx, float cy, float radius, float from, float to, float thickness,
                uint32_t rgba);
    void (*polyline)(SwoftyPaint *paint, const float *xy, uint32_t count, float thickness, uint32_t rgba,
                     uint32_t flags);
    void (*polygon)(SwoftyPaint *paint, const float *xy, uint32_t count, uint32_t rgba);
    void (*text_styled)(SwoftyPaint *paint, float x, float y, const SwoftyTextStyle *style, const char *utf8);
    void (*measure_styled)(const SwoftyPaint *paint, const SwoftyTextStyle *style, const char *utf8, float *width,
                           float *height);
    int (*image_uv)(SwoftyPaint *paint, const char *id, float x, float y, float w, float h, float u0, float v0,
                    float u1, float v1, uint32_t tint);
    int (*image_nine)(SwoftyPaint *paint, const char *id, float x, float y, float w, float h, float left, float top,
                      float right, float bottom, float scale, uint32_t tint);
    int (*image_size)(const char *id, float *width, float *height);
    void (*push_clip)(SwoftyPaint *paint, float x, float y, float w, float h);
    void (*pop_clip)(SwoftyPaint *paint);
    void (*push_transform)(SwoftyPaint *paint, const float *m);
    void (*pop_transform)(SwoftyPaint *paint);
    void (*push_opacity)(SwoftyPaint *paint, float opacity);
    void (*pop_opacity)(SwoftyPaint *paint);
    void (*safe_area)(const SwoftyPaint *paint, float *x, float *y, float *w, float *h);
    float (*ui_scale)(const SwoftyPaint *paint);
    /* v3 */
    int (*input)(const SwoftyPaint *paint, SwoftyInput *out);
    int (*focus)(int on);
} SwoftyOverlay;

#define SWOFTY_TELEMETRY_VERSION 2

/* SwoftyCarState.flags. Bits 0 and 1 say which UDP packets have filled the car. */
#define SWOFTY_CAR_FLAG_LAP_DATA (1u << 0)
#define SWOFTY_CAR_FLAG_STATUS (1u << 1)
#define SWOFTY_CAR_FLAG_IN_PIT_LANE (1u << 2)
#define SWOFTY_CAR_FLAG_RETIRED (1u << 3)
#define SWOFTY_CAR_FLAG_DISQUALIFIED (1u << 4)
#define SWOFTY_CAR_FLAG_FINISHED (1u << 5)
#define SWOFTY_CAR_FLAG_IN_GARAGE (1u << 6)
#define SWOFTY_CAR_FLAG_DRS_ALLOWED (1u << 7)
#define SWOFTY_CAR_FLAG_LAP_INVALID (1u << 8)

/* SwoftySessionState.flag. */
#define SWOFTY_FLAG_NONE 0u
#define SWOFTY_FLAG_GREEN 1u
#define SWOFTY_FLAG_BLUE 2u
#define SWOFTY_FLAG_YELLOW 3u
#define SWOFTY_FLAG_RED 4u

typedef struct SwoftyCarState {
    uint32_t version;
    uint32_t flags;
    float speed;
    float rpm;
    float max_rpm;
    int32_t gear;
    float throttle, brake, clutch, steer;
    float tyre_temp[4];
    float tyre_wear[4];
    float brake_temp[4];
    float fuel;
    float fuel_laps;
    int32_t position;
    uint32_t lap;
    uint32_t sector;
    float last_lap_time, current_lap_time, best_lap_time;
    float sector_time[3];
    float lap_distance;
    float world_pos[3];
    float world_vel[3];
    /* version 2 */
    uint8_t tyre_compound, tyre_visual, tyre_age_laps, pit_status;
    uint8_t result_status, driver_status, pit_stops, warnings;
    uint8_t penalty_seconds, drs, grid_position, ai_controlled;
    uint8_t drive_through_pens, stop_go_pens, corner_cut_warnings;
    int8_t fia_flag;
    uint32_t gap_front_ms, gap_leader_ms;
    float ers_store;
    float reserved;
} SwoftyCarState;

typedef struct SwoftySessionState {
    uint32_t version;
    uint32_t session_type;
    uint32_t track;
    uint32_t flag;
    uint32_t weather;
    uint32_t total_laps;
    uint32_t participant_count;
    uint32_t player_index;
    float session_time;
    float session_time_left;
    float track_temp, air_temp;
    /* version 2 */
    uint32_t safety_car_status;
    uint32_t track_length;
    uint32_t udp_format;
    uint32_t packets_seen;
    uint32_t pit_speed_limit;
    uint32_t red_flag;
    uint32_t reserved[2];
} SwoftySessionState;

typedef struct SwoftyParticipant {
    uint32_t version;
    uint32_t flags;
    char name[48];
    uint32_t team;
    int32_t position;
    uint32_t lap;
    float lap_distance;
    float best_lap_time;
    /* version 2 */
    uint32_t driver_id;
    uint8_t race_number, ai_controlled, nationality, my_team;
    float reserved[2];
} SwoftyParticipant;

typedef struct SwoftyTelemetryFeed {
    uint32_t version;
    uint32_t size;
    uint64_t seq;
    uint64_t frame;
    SwoftySessionState session;
    SwoftyCarState cars[24];
    SwoftyParticipant participants[24];
} SwoftyTelemetryFeed;

typedef struct SwoftyTelemetry {
    int (*available)(void);
    int (*session)(SwoftySessionState *out);
    int (*car)(uint32_t index, SwoftyCarState *out);
    int (*participant)(uint32_t index, SwoftyParticipant *out);
    int (*shared_memory)(const char **name);
} SwoftyTelemetry;

enum {
    SWOFTY_DB_INT = 0,
    SWOFTY_DB_FLOAT = 1,
    SWOFTY_DB_STRING = 2,
    SWOFTY_DB_ENUM = 3,
    SWOFTY_DB_ID = 4,
    SWOFTY_DB_BOOL = 5,
};

typedef struct SwoftyDbValue {
    uint8_t tag;
    int64_t i;
    double f;
    const char *s;
} SwoftyDbValue;

typedef struct SwoftyDbCursor {
    uint64_t table;
    uint64_t index;
    uint64_t count;
    const void *row;
} SwoftyDbCursor;

typedef struct SwoftyDb {
    int (*available)(void);
    int (*get)(const char *key, SwoftyDbValue *out);
    int (*get_hash)(uint64_t hash, SwoftyDbValue *out);
    int (*int_at)(const char *key, int64_t *out);
    int (*float_at)(const char *key, double *out);
    int (*string_at)(const char *key, const char **out);
    int (*bool_at)(const char *key, int32_t *out);
    int (*open)(const char *table, SwoftyDbCursor *cursor);
    int (*row)(const char *table, const char *label, SwoftyDbCursor *cursor);
    int (*step)(SwoftyDbCursor *cursor);
    int (*cell)(const SwoftyDbCursor *cursor, const char *field, SwoftyDbValue *out);
    const char *(*row_name)(const SwoftyDbCursor *cursor);
} SwoftyDb;

typedef struct SwoftyStorage {
    int (*available)(void);
    int (*read)(const char *path, void *buf, size_t cap, size_t *len);
    int (*write)(const char *path, const void *data, size_t len);
    int (*append)(const char *path, const void *data, size_t len);
    int (*remove)(const char *path);
    int (*size)(const char *path, uint64_t *out);
    int (*list)(const char *dir, char *buf, size_t cap, size_t *len);
} SwoftyStorage;

enum {
    SWOFTY_FLOW_NONE = 0,
    SWOFTY_FLOW_ENTERING = 1,
    SWOFTY_FLOW_UPDATING = 2,
    SWOFTY_FLOW_EXITING = 3,
};

enum {
    SWOFTY_CAREER_NONE = 0,
    SWOFTY_CAREER_MY_TEAM = 1,
    SWOFTY_CAREER_DRIVER = 2,
};

enum {
    SWOFTY_CAREER_COMPONENTS_UNSEEN = 0,
    SWOFTY_CAREER_COMPONENTS_LIVE = 1,
    SWOFTY_CAREER_COMPONENTS_FALLBACK = 2,
};

typedef struct SwoftyFlowState {
    uint32_t hash;
    uint32_t from;
    uint32_t to;
    uint32_t named;
    uint64_t transitions;
    char name[64];
} SwoftyFlowState;

typedef struct SwoftyCareerState {
    uint32_t loaded;
    uint32_t mode;
    uint32_t hub;
    uint32_t humans;
    uint32_t player_driver;
    uint32_t player_team;
    uint32_t has_player;
    uint32_t components;
    uint32_t component_mode;
    uint32_t team_hq_mode;
    uint32_t flow_career;
    uint32_t reserved;
    char key[32];
} SwoftyCareerState;

enum {
    SWOFTY_CAREER_START_PICK = 0,
    SWOFTY_CAREER_START_NEW = 1,
    SWOFTY_CAREER_START_CONTINUE = 2,
};

typedef struct SwoftyCareerStart {
    uint32_t mode;
    uint32_t kind;
    uint32_t blocked;
    uint32_t reserved;
    char route[64];
} SwoftyCareerStart;

typedef struct SwoftyCareer {
    int (*available)(void);
    int (*state)(SwoftyCareerState *out);
    int (*flow)(SwoftyFlowState *out);
    int (*flow_status)(uint32_t hash, uint32_t *status, uint64_t *age_ms);
    void *(*entity)(void);
    void *(*component)(const char *type);
    void *(*component_by_desc)(const void *desc);
    int (*post)(void *msg);
    int (*block)(uint32_t mode, const char *title, const char *body);
    int (*unblock)(uint32_t mode);
} SwoftyCareer;

typedef struct SwoftyPopup {
    int (*available)(void);
    int (*show)(const char *title, const char *body);
} SwoftyPopup;

/* Race HUD (swofty/hud.hpp), table version 1. Not part of SwoftyApi: the table comes from the core's
   SwoftyCoreHud export, so an addon built against this header also loads on an older core. Fields are only
   ever appended; version is the number of the last layout the core fills, so check it before calling a
   field added after version 1.
   Version 1: available; toast shows text (and an optional second line, detail) in the race HUD's central
   notification for about seconds, rounded to 2, 3, 4, 5, 6, 8, 10, 12, 15, 20 or 30, with icon NULL/"" for
   the default or a race_flags name (generic, diagonal, black, checkpoint, chequered, fast_lap,
   formation_lap, pp_icon, red, safety_car, virtual_safety_car, virtual_safety_car_green, flag_rival_won);
   set writes one HUD data leaf under ui_menu_data.hud_* or view_model_data.hud*: type SWOFTY_HUD_TEXT with
   a const char *, SWOFTY_HUD_BOOL or SWOFTY_HUD_INT with a const int32_t *. Strings are UTF-8, copied and
   capped at 200 bytes.
   Version 2 (the registered leaf table; fields after set): leaves_register declares n leaves (path, type,
   text_cap) and fills ids[i] with a handle; a path already registered with the same type returns its
   handle, with another type is invalid, and a path outside the hud prefixes is invalid. leaves_set writes n
   values by handle (const char * for text, const int32_t * for bool and int; no allocation, one lock):
   the core keeps the last written value per leaf and applies only what differs. leaves_begin/leaves_end
   (nestable, per thread) make every set in between one group, applied whole in a single tick. The core
   applies at most cap_per_tick leaves a tick and defers the rest, never drops one. generation is bumped
   when the game rebuilds its view model (session load, unload, a hud node recreated); the core then
   re-sends every registered leaf itself. stats fills a SwoftyHudStats (set size first; the core fills the
   fields it knows). The engine exposes no float or colour view-model setter, so there are no such types;
   pack a colour into an int leaf. Registered leaves are type-checked against v1 set too (a write of
   another type is invalid); the engine's own binding types cannot be read, so a template binding of
   another type is still caught only by swofty dev check. */
typedef struct SwoftyHudLeaf {
    uint32_t size;
    const char *path;
    uint32_t type;
    uint32_t text_cap;
} SwoftyHudLeaf;
typedef struct SwoftyHudStats {
    uint32_t size;
    uint32_t applied_last_tick;
    uint32_t queued;
    uint32_t dropped;
    uint32_t registered;
    uint32_t cap_per_tick;
    uint32_t failed;
    uint32_t generation;
    uint32_t resyncs;
    uint32_t v1_queued;
} SwoftyHudStats;
#define SWOFTY_API_HUD 1
#define SWOFTY_HUD_VERSION 2
enum { SWOFTY_HUD_TEXT = 0, SWOFTY_HUD_BOOL = 1, SWOFTY_HUD_INT = 2 };
typedef struct SwoftyHud {
    uint32_t version;
    int (*available)(void);
    int (*toast)(const char *text, const char *detail, float seconds, const char *icon);
    int (*set)(const char *path, uint32_t type, const void *value);
    /* version 2 */
    int (*leaves_register)(const SwoftyHudLeaf *leaves, uint32_t n, uint32_t *ids);
    int (*leaves_set)(const uint32_t *ids, const void *const *values, uint32_t n);
    int (*leaves_begin)(void);
    int (*leaves_end)(void);
    uint32_t (*generation)(void);
    int (*stats)(SwoftyHudStats *out);
} SwoftyHud;

/* CSV recorder (swofty/log_csv.hpp). Not part of SwoftyApi: the table comes from the core's SwoftyCoreCsv
   export, so an addon built against this header also loads on an older core. Fields are only ever appended.
   Version 1: open declares a file swofty/addons/<addon id>/diag/<name>-<timestamp>.csv with n columns (names
   and SWOFTY_CSV_* types) and fills handle. The row is built on the calling thread: row_i32, row_f32 and
   row_text (pointer and byte length, truncated to 47 bytes) set one column of the row under construction,
   row_commit copies it into a ring read by the core's writer thread, which formats and writes it; a full ring
   drops the row and counts it (dropped). Booleans are i32 0/1. clock is a monotonic time in seconds. One
   writer thread per handle. */
#define SWOFTY_CSV_VERSION 1
enum { SWOFTY_CSV_I32 = 0, SWOFTY_CSV_F32 = 1, SWOFTY_CSV_TEXT = 2, SWOFTY_CSV_BOOL = 3 };
typedef struct SwoftyCsv {
    uint32_t version;
    int (*open)(const char *name, const char *const *columns, const uint32_t *types, uint32_t n, uint32_t *handle);
    int (*row_i32)(uint32_t handle, uint32_t column, int32_t value);
    int (*row_f32)(uint32_t handle, uint32_t column, float value);
    int (*row_text)(uint32_t handle, uint32_t column, const char *text, uint32_t len);
    int (*row_commit)(uint32_t handle);
    uint64_t (*dropped)(uint32_t handle);
    void (*close)(uint32_t handle);
    double (*clock)(void);
} SwoftyCsv;

/* Team HQ hub tabs (swofty/hub.hpp). Present on cores that export SwoftyCoreHub; the table is read through
   GetProcAddress, so an addon built against this header also loads on an older core and sees it absent.
   An addon's manifest `hub_tab` {name, title, options, data_group} makes the core add a Team HQ laptop tab
   for careers of data_group (after STANDINGS), whose page template is swofty_hub_<name> and whose data the
   core publishes at view_model_data.swofty_hub_<name> (title, count, o<k>.label/value/description).
   Version 1: available; hide_rnd(on) hides the R&D tab of Team HQ while on is set: Team HQ builds it
   hidden, and an open Team HQ hides or shows it on the next frame (cores before the live path leave it out
   from the next Team HQ entry instead); set_option(addon, option, value) picks the option state whose
   value is `value` (a choice value, "true"/"false", or a number) through the same command ADDON SETTINGS
   runs. */
#define SWOFTY_API_HUB 1
#define SWOFTY_HUB_VERSION 1
typedef struct SwoftyHub {
    uint32_t version;
    int (*available)(void);
    int (*hide_rnd)(int on);
    int (*set_option)(const char *addon, const char *option, const char *value);
} SwoftyHub;

/* Runtime string overrides (API v8): the localised text a lng_* key resolves to, changed while the
   game runs. language NULL or "*" means every language; otherwise a language code (eng, fre, ...)
   that applies while it is the game's current language and wins over "*". The text is UTF-8 and is
   copied. A change shows wherever the game looks the key up next; refresh() re-sends the current
   language to the game's language listeners so bound labels read their text again (game thread). */
typedef struct SwoftyLang {
    int (*available)(void);
    int (*override_set)(const char *language, const char *key, const char *text);
    int (*override_clear)(const char *language, const char *key);
    int (*override_clear_all)(void);
    int (*refresh)(void);
} SwoftyLang;

/* Extra career contents (no API version of its own: the table is read only when the core exports
   SwoftyCoreCareerContent, see swofty/career_content.hpp). A content is a tile on the career
   content select after the stock 2025 and 2026 ones, with its own save slots (its data group). The
   core builds the tile, presses it, routes it, passes its mode through the driver-career gate and
   answers the save-slot predicate for it. Strings are copied. */
#define SWOFTY_EVENT_CAREER_CONTENT_PRESSED "career.content.pressed"

enum {
    SWOFTY_CAREER_CONTENT_MODEL_2026 = 0,
    SWOFTY_CAREER_CONTENT_MODEL_2025 = 1,
};

enum {
    SWOFTY_CAREER_CONTENT_DRIVER = 1,
    SWOFTY_CAREER_CONTENT_MY_TEAM = 2,
};

enum {
    SWOFTY_CAREER_CONTENT_QUEUED = 0,
    SWOFTY_CAREER_CONTENT_LIVE = 1,
    SWOFTY_CAREER_CONTENT_OFF = 2,
};

typedef struct SwoftyCareerContentDesc {
    uint32_t size;
    uint32_t model;
    const char *id;
    const char *title;
    const char *description;
    const char *data_group;
    const char *suffix;
    const char *mode;
    const char *entitlement;
    int32_t car_group;
    uint32_t badge;
    uint32_t careers;
} SwoftyCareerContentDesc;

typedef struct SwoftyCareerContentInfo {
    uint32_t index;
    uint32_t state;
    int64_t mode_stat_id;
    int64_t data_group_index;
    uint32_t tiles;
    uint32_t presses;
    char id[64];
    char data_group[64];
    char why[192];
} SwoftyCareerContentInfo;

typedef struct SwoftyCareerContent {
    int (*available)(void);
    int (*add)(const SwoftyCareerContentDesc *desc);
    int (*info)(const char *id, SwoftyCareerContentInfo *out);
    int (*data_group)(const void *career, char *out, size_t cap);
    int (*pressed)(char *out, size_t cap);
} SwoftyCareerContent;

/* Per-career character overrides (swofty/characters.hpp). Present on cores that have them, NULL
   otherwise; SWOFTY_API_CHARACTERS marks a header that declares the table. Every entry is keyed by
   a career data group (the CareerSetupComponent data group, e.g. f1_career_2026) and applies only
   while the loaded career's data group is that one. swap() builds the character's UI ref (portrait,
   names, job) from another entrants.characters row under the character's own stat_id; replacement
   NULL drops the swap. mute() plays a speaker's lines at speech volume 0. string_set() and
   strings_json() hold runtime string overrides (SwoftyLang) that are set while the group's career
   is loaded; text NULL drops one. clear() drops every entry of the group that the calling module
   made. refresh() rebuilds the character list now (game thread). */
#define SWOFTY_API_CHARACTERS 1
typedef struct SwoftyCharacters {
    int (*available)(void);
    int (*career_group)(char *out, size_t cap);
    int (*swap)(const char *group, const char *character, const char *replacement);
    int (*mute)(const char *group, const char *speaker, int on);
    int (*string_set)(const char *group, const char *language, const char *key, const char *text);
    int (*strings_json)(const char *group, const char *json, size_t len);
    int (*clear)(const char *group);
    int (*refresh)(void);
} SwoftyCharacters;

/* Career wizard control (swofty/career_wizard.hpp). Present on cores that have it, NULL otherwise;
   SWOFTY_API_CAREER_WIZARD marks a header that declares the table. Every entry is keyed by the data
   group of the career content tile the wizard was opened from (wizard state +0x128). starts_in_f1()
   answers yes for the group (no F2 choice, no team-icons step). tile_route() makes the tile open
   route `to` instead of `from` (NULL or "" for any route); the route must exist in the front end's
   routes. replace_step() pushes step `with` where the wizard would push `step`; force_option()
   records option `with` where the player picked `option`. confirm_route() makes the confirm step
   open `to` instead of `from`. hide_driver_tab() drops a Choose Driver header tab. Passing the
   stock value back (on 0, with == step, to NULL) drops an entry. clear() drops every entry of the
   group that the calling module made. group() names the data group of the last career tile. */
#define SWOFTY_API_CAREER_WIZARD 1
typedef struct SwoftyCareerWizard {
    int (*available)(void);
    int (*group)(char *out, size_t cap);
    int (*starts_in_f1)(const char *group, int on);
    int (*tile_route)(const char *group, const char *from, const char *to);
    int (*replace_step)(const char *group, uint32_t step, uint32_t with);
    int (*force_option)(const char *group, uint32_t option, uint32_t with);
    int (*confirm_route)(const char *group, const char *from, const char *to);
    int (*hide_driver_tab)(const char *group, const char *tab, int on);
    int (*clear)(const char *group);
} SwoftyCareerWizard;

/* Era picker boxes (swofty/era.hpp). Present on cores that have them, NULL otherwise; the table is read
   only when the core exports SwoftyCoreEra, and SWOFTY_API_ERA marks a header that declares it. A box
   is a box in the Addons row of the Grand Prix / Time Trial era picker that selects entrant_list, which
   must be in the entrant lists of the core's host setup mode swofty_f1; the core skips, with a log
   line, a box whose list the screen's mode does not offer there. title is a lng_* key (NULL or "" for
   lng_<entrant_list>), image a texture path (NULL or "" for
   #overrides.home_screen.tiles_solo_6x6[<entrant_list>]), order sorts the boxes (then registration
   order). time_trial_entrant_list (since SWOFTY_API_ERA 2; NULL or "" for entrant_list) is the list the
   box selects on a Time Trial picker; it must be in swofty_f1's entrant lists too. On a Time Trial
   picker the core skips a box whose list holds more than 23 cars. only (since SWOFTY_API_ERA 3) is
   SWOFTY_ERA_ONLY_ANY (0, both pickers), SWOFTY_ERA_ONLY_GRAND_PRIX or SWOFTY_ERA_ONLY_TIME_TRIAL: the
   core leaves the box off the other picker; any other value is refused. A core that exports no
   SwoftyCoreEraOnly ignores the field. A descriptor that ends before time_trial_entrant_list (size
   0x28) is read without it, one that ends before only (size 0x30) without only. Strings are copied. */
#define SWOFTY_API_ERA 3
#define SWOFTY_ERA_ONLY_ANY 0
#define SWOFTY_ERA_ONLY_GRAND_PRIX 1
#define SWOFTY_ERA_ONLY_TIME_TRIAL 2
typedef struct SwoftyEraBox {
    uint32_t size;
    const char *entrant_list;
    const char *title;
    const char *image;
    int32_t order;
    const char *time_trial_entrant_list;
    uint32_t only;
} SwoftyEraBox;

typedef struct SwoftyEra {
    int (*available)(void);
    int (*add_box)(const SwoftyEraBox *box);
    int (*count)(void);
} SwoftyEra;

/* Standard library 1.6 (API v9): the registry, hook points and services. Each table is appended to
   SwoftyApi and also returned by a core export (SwoftyCoreRegistry, SwoftyCoreHooks, SwoftyCoreServices),
   so an addon built against this header also loads on an older core and reads the three libraries as
   unavailable there. A table's version is the number of the last layout the core fills; fields are only
   ever appended. Results are the SWOFTY_ASSET_* codes plus SWOFTY_TOO_SMALL: *len then holds the size
   needed, and the caller retries with a bigger buffer. The core copies every string and JSON text it is
   given; values come back in the caller's buffer. A refusal leaves its reason in last_error for the
   calling thread. Records, edits and selectors cross the ABI as JSON text: a record is its struct's
   SWOFTY_RECORD fields by name, a Ref is the record's full id ("game/ferrari_2025") or null, a keyed
   list is an array matched by its x-key fields, and a pointer names a keyed element as "slot=1"
   ("/teams/slot=1/seats/0"). Selectors (layer filters, handler scopes) are a JSON array whose entries
   must all match: {"content": id}, {"content_exact": id}, {"owner": addon}, {"data_group": name},
   {"mode": "time_trial"}, {"session_type": ["race"]}, {"track": id}, {"year": [from, to]},
   {"setup_mode": id}, {"vehicle_package": id}, {"any": [selector, ...]}, {"not": selector}. */
#define SWOFTY_REGISTRY_VERSION 1
#define SWOFTY_HOOKS_VERSION 1
#define SWOFTY_SERVICES_VERSION 1

enum { SWOFTY_TOO_SMALL = 4 };

enum { SWOFTY_SCOPE_GLOBAL = 0, SWOFTY_SCOPE_PROFILE = 1, SWOFTY_SCOPE_CAREER = 2, SWOFTY_SCOPE_AUTOMATIC = 3 };
enum { SWOFTY_APPLIES_BOOT = 0, SWOFTY_APPLIES_CONTENT = 1, SWOFTY_APPLIES_SESSION = 2, SWOFTY_APPLIES_READ = 3 };
enum {
    SWOFTY_STATUS_OK = 0,
    SWOFTY_STATUS_NO_RECORD = 1,
    SWOFTY_STATUS_NO_REGISTRY = 2,
    SWOFTY_STATUS_INVALID = 3,
    SWOFTY_STATUS_LATE = 4,
};
enum { SWOFTY_PHASE_BEFORE = 0, SWOFTY_PHASE_REPLACE = 1, SWOFTY_PHASE_AFTER = 2 };
enum { SWOFTY_POINT_NOTIFY = 0, SWOFTY_POINT_MODIFY = 1, SWOFTY_POINT_CANCEL = 2 };

typedef struct SwoftyText {
    const char *data;
    size_t size;
} SwoftyText;

typedef int (*SwoftyProvideFn)(void *user, const char *registry, const char *id, char *out, size_t cap, size_t *len);
typedef void (*SwoftyListFn)(void *user, const char *registry, void (*add)(void *ctx, const char *id), void *ctx);
typedef void (*SwoftyRecordFn)(void *user, const char *registry, const char *id);
typedef int (*SwoftyConvertFn)(void *user, SwoftyText in, char *out, size_t cap, size_t *len);
typedef void (*SwoftyCheckFn)(void *user, SwoftyText value, void (*error)(void *ctx, const char *path, const char *msg),
                              void *ctx);
typedef int (*SwoftyEditFn)(void *user, SwoftyText in, char *out, size_t cap, size_t *len);
typedef int (*SwoftyMatchFn)(void *user, SwoftyText base);

/* One layer, as layer_ex takes it. id names one record; otherwise the layer is a selector over every
   record that passes all of match (run on the base value), prefix, tag (with tag_field, the JSON pointer
   of a Ref field whose target must be in the tag; NULL for the record itself) and is not in except.
   The change is edit (code) or patch (a JSON merge patch with keyed arrays and $delete). in is a
   selector array (LayerHandle::in), empty for everywhere. */
typedef struct SwoftyLayerDesc {
    uint32_t size;
    uint32_t scope;
    const char *registry;
    const char *id;
    SwoftyMatchFn match;
    void *match_user;
    const char *prefix;
    const char *const *except;
    size_t except_count;
    const char *tag;
    const char *tag_field;
    SwoftyText in;
    SwoftyEditFn edit;
    void *user;
    SwoftyText patch;
} SwoftyLayerDesc;

/* Registry table, version 1. declare: a registry's schema (JSON Schema subset with x-registry, x-version,
   x-applies, x-snapshot, x-snapshot-budget, x-key, x-ref); idempotent, the core keeps the highest minor of
   each major and its own schema for the registries it owns. add, variant, provide: records under the
   calling addon's own keys, the id comes back as "<addon id>/<key>". layer, layer_fn, layer_select,
   layer_ex: changes to any record, run on its effective value in load order. get takes
   "<name>@<major>" or "<name>". status returns a SWOFTY_STATUS_* value (not a result code). first and
   next list ids under a prefix in id order. option is the calling addon's option; called inside an
   edit it also records that the edit re-runs when the option changes. layer_ex returns the layer's
   handle; layer_in narrows it to where its selector array matches (refused on an Applies::boot
   registry). protect names fields of an own record that other addons' selectors skip. tag_* work on
   shared tags ("c/f1_cars"); tag_members writes a JSON array of ids. variant_patch is variant with a
   JSON merge patch. stat_id is the record's allocated stat id. */
typedef struct SwoftyRegistry {
    uint32_t version;
    int (*available)(void);
    int (*declare)(const char *registry, SwoftyText schema_json);
    int (*check)(const char *registry, SwoftyCheckFn fn, void *user);
    int (*adapter)(const char *registry, uint32_t from_major, uint32_t to_major, SwoftyConvertFn up,
                   SwoftyConvertFn down, void *user);
    int (*provide)(const char *registry, const char *id, SwoftyText value_json);
    int (*provide_fn)(const char *registry, SwoftyProvideFn fn, SwoftyListFn list, void *user);
    int (*invalidate)(const char *registry, const char *id);
    int (*layer)(const char *registry, const char *id, SwoftyText patch_json, uint32_t scope);
    int (*clear_layer)(const char *registry, const char *id, uint32_t scope);
    int (*get)(const char *registry_at_major, const char *id, char *out, size_t cap, size_t *len);
    int (*status)(const char *registry, const char *id);
    int (*origin)(const char *registry, const char *id, const char *pointer, char *addon, size_t cap);
    int (*first)(const char *registry, const char *prefix, char *id, size_t cap);
    int (*next)(const char *registry, const char *prefix, const char *after, char *id, size_t cap);
    int (*watch)(const char *registry, SwoftyRecordFn fn, void *user, uint32_t *token);
    int (*unwatch)(uint32_t token);
    int (*last_error)(char *out, size_t cap);
    int (*add)(const char *registry, const char *key, SwoftyText value_json, char *id, size_t cap);
    int (*variant)(const char *registry, const char *key, const char *base_id, SwoftyEditFn edit, void *user,
                   char *id, size_t cap);
    int (*layer_fn)(const char *registry, const char *id, SwoftyEditFn edit, void *user, uint32_t scope);
    int (*layer_select)(const char *registry, SwoftyMatchFn match, const char *prefix, const char *const *except,
                        size_t except_count, SwoftyEditFn edit, void *user, uint32_t scope);
    int (*option)(const char *id, uint32_t type, SwoftySettingValue *out);
    int (*migrate)(const char *registry, uint32_t from_major, uint32_t to_major, SwoftyConvertFn fn, void *user);
    int (*layer_ex)(const SwoftyLayerDesc *desc, uint32_t *layer);
    int (*layer_in)(uint32_t layer, SwoftyText selectors_json);
    int (*protect)(const char *registry, const char *id, const char *const *fields, size_t count);
    int (*tag_add)(const char *registry, const char *tag, const char *id);
    int (*tag_contains)(const char *registry, const char *tag, const char *id, int *out);
    int (*tag_members)(const char *registry, const char *tag, char *out, size_t cap, size_t *len);
    int (*variant_patch)(const char *registry, const char *key, const char *base_id, SwoftyText patch_json, char *id,
                         size_t cap);
    int (*stat_id)(const char *registry, const char *id, int64_t *out);
} SwoftyRegistry;

/* One call of a hook point or event, as a SwoftyHookFn sees it. payload is the point's struct, which
   starts with uint32_t size and uint32_t payload_version. replaced is set by the core after the replace
   phase. context is the moment's context, for the hooks table's context and context_in. */
typedef struct SwoftyHookCall {
    uint32_t size;
    uint32_t phase;
    const char *point;
    void *payload;
    int replaced;
    const void *context;
} SwoftyHookCall;

typedef int (*SwoftyHookFn)(void *user, SwoftyHookCall *call);

/* Hooks table, version 1. Events are hook points: attach a handler to a phase (SWOFTY_PHASE_*) with a
   priority (higher first); in the replace phase a non-zero return means handled. attach_in also takes a
   selector array: the core calls the handler only where it matches. define declares an addon point
   under the addon's own id (define_in with its kind, SWOFTY_POINT_*, and the scope it fires in). fire
   runs the chain on payload; *replaced is non-zero when a replace handler handled it. live is OK when
   the point is installed and runs its subscribers. context writes one part of a call's context as JSON:
   "mode", "content", "career", "weekend", "session", "player", "participants", or "record:<registry>";
   context_in sets *out to 1 when the context matches the selector array. */
typedef struct SwoftyHooks {
    uint32_t version;
    int (*available)(void);
    int (*live)(const char *point, uint32_t *payload_version);
    int (*attach)(const char *point, uint32_t phase, int32_t priority, SwoftyHookFn fn, void *user, uint32_t *token);
    int (*detach)(uint32_t token);
    int (*define)(const char *point, uint32_t payload_version);
    int (*fire)(const char *point, void *payload, int *replaced);
    int (*attach_in)(const char *point, uint32_t phase, int32_t priority, SwoftyText selectors_json, SwoftyHookFn fn,
                     void *user, uint32_t *token);
    int (*define_in)(const char *point, uint32_t payload_version, uint32_t kind, SwoftyText scope_json);
    int (*context)(const void *context, const char *part, char *out, size_t cap, size_t *len);
    int (*context_in)(const void *context, SwoftyText selectors_json, int *out);
} SwoftyHooks;

typedef void (*SwoftyServiceFn)(void *user, const void *table, size_t size, uint32_t version);

/* Services table, version 1. publish: a table of function pointers under a name and version (the table
   starts with uint32_t size and stays valid while the addon is loaded). when: fn runs on the flow thread
   once a table of that name, of at least min_version and min_size, is published; at once when it
   already is. */
typedef struct SwoftyServices {
    uint32_t version;
    int (*available)(void);
    int (*publish)(const char *name, uint32_t version, const void *table, size_t size);
    int (*when)(const char *name, uint32_t min_version, size_t min_size, SwoftyServiceFn fn, void *user);
} SwoftyServices;

/* Career operations (standard library 1.6), from the core export SwoftyCoreCareerOps; not a SwoftyApi
   field. Table version 1. Every call runs on the game's flow thread (an events, hook or on_frame callback;
   SWOFTY_ASSET_LATE from any other thread), reports SWOFTY_ASSET_UNAVAILABLE and logs the binding when one
   it needs does not resolve on the running build, and writes one [career-ops] log line.
   driver_stat: a driver reference ("game/<label>", "person/<label>") to the stat id of the drivers table row
   with that label. set_slots: a data group's career team slots, slot N (1-based) racing as team stat
   team_stats[N-1]; apply_lineup reads it for a season record. apply_lineup takes a JSON request, either
   {"season": <a career.season record>} (the core plans the seats) or {"lineup": {...}} (the seats, releases,
   numbers, cars and looks in stat ids, run as given), and writes a JSON report to out (SWOFTY_TOO_SMALL:
   *len holds the size needed). set_driver_ratings: one driver's base values of count statistics by game
   stat id (experience 0, racecraft 1, awareness 3, pace 4, focus 5). swap_drivers: two seated drivers
   exchange seats. Stat ids are the upper 16 bits of the game's keys.
   Table version 2 adds report_season: the addon that owns the active career's data group (its career
   content or career.mode record) says the season at the Championship's season_index is year; the core
   then fires career.season.start once per career and year, activates the season's records and freezes
   its snapshot. Any other caller gets SWOFTY_REFUSED; no career loaded, SWOFTY_ASSET_LATE.
   Table version 3 adds apply_teams: the request {"season": <a career.season record>}; each listed slot
   (1-12) races as its team (the data group's slot table, the Championship's participants), and a team
   with no career rows gets them, built from its donor's (contracts, R&D, standings, initial cash, renown
   and the side data found by team label); an empty team leaves the slot out of the Championship. The
   report has apply_lineup's shape, counted per slot. A core that has not implemented it leaves it NULL. The
   request {"start": {"data_group": "<group>", "year": <start season>}} dresses the slot rows of that season's
   added teams (a slot naming a team.identity an addon made) for the new-career wizard, and year 0 takes the dress
   off; the season start then dresses the same rows the same way. The report counts "dressed" and "undressed".
   Table version 4 adds set_seat, set_car_number and set_track_fuel. set_seat takes {"driver": <stat>,
   "team": <stat> or "slot": <1-based slot of the loaded career's data group>, "seat": 0 or 1, "car": <stat,
   optional>}: the driver takes that race seat in Driver Career; the seat's driver takes his old seat, or
   leaves the grid when he had none. The report is apply_lineup's with "holder": "swapped", "released" or
   "none". set_car_number: the driver's racing number (1-99) on his career row and the current weekend's
   copy; a number another row holds is SWOFTY_REFUSED unless flags has SWOFTY_NUMBER_SWAP, which gives that
   row the driver's old number. set_track_fuel takes {"track": "<code>" or "*", "set": "<data set>",
   "from": "<data set>"} (the fuel of the same track in another data set: burn per lap, race fuel rows and
   qualifying fuel), {"track", "set", "burn_per_lap": <kg>}, or {"copy": [{"set", "from"}, ...], "track"}
   (every pair checked before anything is written), or {"restore": true}; "why" names the reason in the
   log. The data sets are f1_2025, sprint_f1_2025, f1_2026 and sprint_f1_2026. The core puts the bytes
   back on career.unloaded and on restore. Table version 5 adds set_logo(team_id, logo_id): the team.identity
   record's logo for the rest of the session is the team.logo record (both ids as the registry spells them);
   SWOFTY_REFUSED when either record does not exist.
   Table version 6 adds set_look: the driver (a driver reference: "game/<label>", "person/<key>" or an addon's
   driver.identity id) wears the look (a look reference: "game/<label>" or an addon's driver.look id) in Driver
   Career: his entrant row's look, which every place that shows his body reads (grid presentation, portraits,
   cutscenes, the garage). The call refuses a look the game did not load (SWOFTY_ASSET_INVALID) and a driver the
   career does not seat. The report is one [career-ops] log line.
   Table version 7 adds current_round and team_stat. current_round: the loaded Driver Career's Championship
   season (season_index, 0 the first) and its current race weekend: round is the 1-based position of the weekend
   the career is at (the next race to run) and rounds the season's weekend count; SWOFTY_ASSET_LATE when no
   career is loaded or its season has no built weekends yet (also while the career is busy). team_stat: a team
   reference ("game/<label>", or an addon's team.identity id) to the stat id of the teams table row with that
   label; SWOFTY_ASSET_INVALID when the game has no such row. Version 7 also adds
   set_dropped_scores(data_group, best): while the loaded career is filed under data_group, the drivers'
   championship counts each driver's best `best` results (0 counts every result): the cumulative totals, and so
   the order, use the counted sum of the best per-round totals; the constructors' championship and the
   per-session points stay gross. */
#define SWOFTY_CAREER_OPS_VERSION 7
enum { SWOFTY_REFUSED = 5 };
enum { SWOFTY_NUMBER_SWAP = 1 };

typedef struct SwoftyCareerOps {
    uint32_t version;
    int (*available)(void);
    int (*driver_stat)(const char *driver_ref, uint32_t *out);
    int (*set_slots)(const char *data_group, const uint32_t *team_stats, size_t count);
    int (*apply_lineup)(SwoftyText request_json, char *report, size_t cap, size_t *len);
    int (*set_driver_ratings)(uint32_t driver_stat, const uint16_t *stat_ids, const float *values, size_t count);
    int (*swap_drivers)(uint32_t a_stat, uint32_t b_stat);
    int (*report_season)(int32_t year, int32_t season_index);
    int (*apply_teams)(SwoftyText request_json, char *report, size_t cap, size_t *len);
    int (*set_seat)(SwoftyText request_json, char *report, size_t cap, size_t *len);
    int (*set_car_number)(uint32_t driver_stat, int32_t number, uint32_t flags);
    int (*set_track_fuel)(SwoftyText request_json, char *report, size_t cap, size_t *len);
    int (*set_logo)(const char *team_id, const char *logo_id);
    int (*set_look)(const char *driver_ref, const char *look_ref);
    int (*current_round)(int32_t *season_index, int32_t *round, int32_t *rounds);
    int (*team_stat)(const char *team_ref, uint32_t *out);
    int (*set_dropped_scores)(const char *data_group, uint32_t best);
} SwoftyCareerOps;

/* Career saves (standard library 1.7.0, experimental), from the core export SwoftyCoreCareerSaves; not a
   SwoftyApi field. Table version 1. The core keeps a career index in the launcher's data folder
   ($DATA/careers/index.json, written at each profile load, career.loaded and profile save) and, beside it,
   one data file per career and addon ($DATA/careers/data/<career>/<addon>.json). index: the career index as
   JSON, {"version": 1, "careers": [{"career", "mode", "content", "team", "driver", "season", "round",
   "last_saved", "addons": [{"id", "need", "why"}], "status"}]}, written to json (SWOFTY_TOO_SMALL: *len holds
   the size needed). current: the loaded career's id, SWOFTY_ASSET_LATE with no career loaded. describe: a JSON
   request {"driver", "team", "season", "round", "label"} (every key optional) naming the loaded career for the
   index. need: the loaded career needs the addon (a version range, may be empty) for the reason; the index
   lists it and the launcher warns before Play when the addon is off. data_get/data_set/data_remove: an addon's
   own JSON for a career, versioned by the addon (the version is stored and handed back; the addon migrates).
   data_set is refused (SWOFTY_REFUSED) for a career the index does not know; data_get on a career with no data
   reports SWOFTY_ASSET_INVALID. missing: the careers the index knew that the last profile load did not find,
   as the index's career records. Calls from any thread. */
#define SWOFTY_CAREER_SAVES_VERSION 1

typedef struct SwoftyCareerSaves {
    uint32_t version;
    int (*available)(void);
    int (*index)(char *json, size_t cap, size_t *len);
    int (*current)(char *id, size_t cap, size_t *len);
    int (*describe)(SwoftyText request_json);
    int (*need)(const char *addon, const char *range, const char *why);
    int (*data_get)(const char *career, const char *addon, char *json, size_t cap, size_t *len, uint32_t *version);
    int (*data_set)(const char *career, const char *addon, uint32_t version, SwoftyText json);
    int (*data_remove)(const char *career, const char *addon);
    int (*missing)(char *json, size_t cap, size_t *len);
} SwoftyCareerSaves;

/* Session operations (standard library 1.6), from the core export SwoftyCoreSessionOps; not a SwoftyApi
   field. Table version 1. Every call runs on the game's flow thread and reports SWOFTY_ASSET_UNAVAILABLE
   when a game function it needs does not resolve on the running build. control: the mode (0 human, 1 AI)
   and the game's controller flag bits (PIT_LANE is bit 0) of the car with that index, read fresh from its
   controller set; SWOFTY_ASSET_LATE while the sets are not built (before the session's first build, or
   between a destroy and the next build). set_control: mode 0 or 1 for the car; the core queues it to the
   next frame when called from inside the controller swap, never applies it between the sets' destroy and
   build, and returns SWOFTY_REFUSED when a flag the core did not set keeps the car on AI. It reaches the
   game through Game_Set_VehicleController, so session.player.control fires for it. The spectator calls
   work on the session's spectator director: spectator_set_active turns the director and its HUD on or
   off; spectator_follow moves it to the car; spectator_set_camera takes 1 (TV pod) or 2 (broadcast);
   spectator_current reads the car (index, or -1 for none) and the camera (-1 for none) while the director
   is active, else SWOFTY_ASSET_LATE. Version 2 adds disqualify: it calls the game's Sess_Rules_Disqualify for the
   car on the session's rules object, which the core captures when the game initialises its rule manager; the
   car goes to the AI with the DISQUALIFIED reason. SWOFTY_ASSET_LATE before the session has a rule manager or
   while the car has no controller set, SWOFTY_ASSET_UNAVAILABLE when the binding does not resolve; queued to the
   next frame like set_control. Version 1 is the members before it and keeps its layout. */
#define SWOFTY_SESSION_OPS_VERSION 2

typedef struct SwoftySessionOps {
    uint32_t version;
    int (*available)(void);
    int (*control)(uint32_t car, uint32_t *mode, uint32_t *flags);
    int (*set_control)(uint32_t car, uint32_t mode);
    int (*spectator_set_active)(int on);
    int (*spectator_follow)(uint32_t car);
    int (*spectator_set_camera)(int32_t camera);
    int (*spectator_current)(int32_t *car, int32_t *camera);
    int (*disqualify)(uint32_t car);
} SwoftySessionOps;

/* Content ownership (standard library 1.8, experimental), from the core export SwoftyCoreContentOps; not a
   SwoftyApi member. owned reads whether the player owns the content behind an entitlement name such as
   "f1_2026" (the F1 2026 season content): SWOFTY_OWNERSHIP_OWNED, SWOFTY_OWNERSHIP_NOT_OWNED, or
   SWOFTY_OWNERSHIP_UNKNOWN before the game has the answer, for a name the core does not track, or on a build
   where the game binding does not resolve. The core reads the game's own answer on the flow thread and
   caches it; the call is a cache read and is safe from any thread. */
#define SWOFTY_CONTENT_OPS_VERSION 1
#define SWOFTY_OWNERSHIP_UNKNOWN 0
#define SWOFTY_OWNERSHIP_OWNED 1
#define SWOFTY_OWNERSHIP_NOT_OWNED 2

typedef struct SwoftyContentOps {
    uint32_t version;
    int (*owned)(const char *entitlement);
} SwoftyContentOps;

/* Live game state (standard library, osd), from the core export SwoftyCoreLive; not a SwoftyApi field, so
   SWOFTY_API_VERSION is unchanged and the table carries its own version. The core reads the game's memory
   (fault-free copies, verified sites only) on its frame tick into one cached snapshot; every call copies from
   that snapshot, so reads are cheap and consistent from any thread. A car is the game's vehicle id (the slot of
   g_Sess_VehicleTable), the same index as SwoftyCarOps and not the UDP participant index. car and subject
   return the same struct for any vehicle, the watched car included. Every struct starts with version and size and
   ends in reserved; a later version fills reserved and never moves an earlier member. A bit of have says the
   value behind it was read this frame; a clear bit means the game did not give it for that car and the field is
   zero, never another car's value. Calls answer SWOFTY_ASSET_UNAVAILABLE while no session is live, the build is
   not the one the offsets are from, or the group is off because a KB row it needs has no independent verify row
   yet (the core logs "live <group> off: needs <row>" once), and SWOFTY_ASSET_INVALID for a bad argument.
   Table version 4 appends images (see swofty/live_images.h); version 3 appends team_badge_texture.
   Version 2 adds driver_name and version 1 is the members up to seq; each keeps its layout, so
   check version >= 3 before calling team_badge_texture and version >= 2 before calling driver_name.
   session: the session header. car: one car by vehicle id. subject: the watched car while the spectator HUD is
   up, else the player's. turn: the official turn at that car's lap_m (SWOFTY_ASSET_UNAVAILABLE for a track
   without a table). track_turns: the whole turn list of the loaded track; count is the turns in the list on
   return, at most cap are written. team_badge: the timing tower team badge key of that car (the string after
   "#overrides.osd.timing_tower.team_badges_hud."), NUL-terminated within cap. seq: the snapshot counter, which
   changes on every refresh. driver_name (version 2): the driver name the game shows for that car, as UTF-8: the
   player's name for a car a human drives, else the display name of the driver record of the car's entrant; at most
   cap - 1 bytes, cut at a character boundary, then NUL. It reads the game's memory at the call (not the snapshot), so
   a timing tower can fetch one name per visible row.
   team_badge_texture (version 3): the texture path the race HUD timing pill binds for
   that car, NUL-terminated within cap: "#overrides.osd.timing_tower.team_badges_hud.<key>" for a catalogue team,
   "eaid://ui_package/art/textures/render_badge_<table> <index>" (or render_badge_default.tga) for a team record
   with show_custom_badge_in_UI set, or for a user-defined team's badge. My Team careers on the verified
   Steam 26566308 build use the saved assignment for the active mode and loaded career slot; an unavailable
   saved assignment is not replaced by the entrant fallback.
   SWOFTY_ASSET_UNAVAILABLE when the reference cannot be read or the path does not fit cap. */
#define SWOFTY_LIVE_VERSION 4
#define SWOFTY_LIVE_MAX_CARS 32
#define SWOFTY_LIVE_NO_GAP INT32_MIN

#define SWOFTY_LIVE_HAVE_IDENT (1ull << 0)
#define SWOFTY_LIVE_HAVE_SPEED (1ull << 1)
#define SWOFTY_LIVE_HAVE_GEAR (1ull << 2)
#define SWOFTY_LIVE_HAVE_RPM (1ull << 3)
#define SWOFTY_LIVE_HAVE_PEDALS (1ull << 4)
#define SWOFTY_LIVE_HAVE_STEER (1ull << 5)
#define SWOFTY_LIVE_HAVE_G (1ull << 6)
#define SWOFTY_LIVE_HAVE_ERS_STORE (1ull << 7)
#define SWOFTY_LIVE_HAVE_ERS_POWER (1ull << 8)
#define SWOFTY_LIVE_HAVE_ERS_LAP (1ull << 9)
#define SWOFTY_LIVE_HAVE_ERS_MODE (1ull << 10)
#define SWOFTY_LIVE_HAVE_PROGRESS (1ull << 11)
#define SWOFTY_LIVE_HAVE_CLASS (1ull << 12)
#define SWOFTY_LIVE_HAVE_GAP_LEADER (1ull << 13)
#define SWOFTY_LIVE_HAVE_PIT (1ull << 14)
#define SWOFTY_LIVE_HAVE_TYRE (1ull << 15)
#define SWOFTY_LIVE_HAVE_PENALTIES (1ull << 16)
#define SWOFTY_LIVE_HAVE_TYRE_WEAR (1ull << 17)
#define SWOFTY_LIVE_HAVE_BOOST (1ull << 18)
#define SWOFTY_LIVE_HAVE_AERO_MODE (1ull << 19)
#define SWOFTY_LIVE_HAVE_DRS (1ull << 20)
#define SWOFTY_LIVE_HAVE_IDENTITY (1ull << 21)
#define SWOFTY_LIVE_HAVE_CLUTCH (1ull << 22)
#define SWOFTY_LIVE_HAVE_WORLD (1ull << 23)
#define SWOFTY_LIVE_HAVE_SECTOR (1ull << 24)
#define SWOFTY_LIVE_HAVE_GAP_AHEAD (1ull << 25)
#define SWOFTY_LIVE_HAVE_PIT_LANE (1ull << 26)

#define SWOFTY_LIVE_CAR_PLAYER (1u << 0)
#define SWOFTY_LIVE_CAR_AI_DRIVING (1u << 1)
#define SWOFTY_LIVE_CAR_WATCHED (1u << 2)
#define SWOFTY_LIVE_CAR_RACER (1u << 3)

typedef struct SwoftyLiveCar {
    uint32_t version;
    uint32_t size;
    uint64_t have;
    uint32_t vehicle;
    uint32_t participant;
    uint8_t flags;
    uint8_t ers_class;
    uint8_t pad[2];
    float speed;
    float rpm, rpm_limit;
    int32_t gear;
    float throttle, brake, clutch;
    float steer;
    float g_lat, g_long;
    float world_pos[3], world_vel[3];
    float ers_store, ers_capacity;
    float ers_deploy, ers_harvest_k, ers_harvest_h, ers_max_deploy;
    float ers_harvested_lap, ers_deployed_lap, ers_harvest_limit_lap;
    int32_t ers_mode;
    int32_t lap;
    float lap_m;
    float total_m;
    int32_t position;
    uint8_t result_status;
    uint8_t pit_state;
    uint8_t tyre_visual;
    uint8_t tyre_actual;
    float time_penalty_s;
    int32_t gap_leader_ms, gap_ahead_ms;
    uint8_t drive_through, stop_go, warnings, track_limit_warnings;
    char tyre_letter;
    uint8_t sector;
    uint8_t drs;
    uint8_t pad2;
    uint8_t pit_status;
    uint8_t pit_stops;
    uint8_t pad3[2];
    uint32_t reserved[7];
} SwoftyLiveCar;

#define SWOFTY_LIVE_SHAVE_TYPE (1ull << 0)
#define SWOFTY_LIVE_SHAVE_CARS (1ull << 1)
#define SWOFTY_LIVE_SHAVE_PLAYER (1ull << 2)
#define SWOFTY_LIVE_SHAVE_WATCHED (1ull << 3)
#define SWOFTY_LIVE_SHAVE_TRACK_LENGTH (1ull << 4)
#define SWOFTY_LIVE_SHAVE_SAFETY_CAR (1ull << 5)
#define SWOFTY_LIVE_SHAVE_RED_FLAG (1ull << 6)
#define SWOFTY_LIVE_SHAVE_YELLOW (1ull << 7)
#define SWOFTY_LIVE_SHAVE_GAME_MODE (1ull << 8)
#define SWOFTY_LIVE_SHAVE_TRACK_NAME (1ull << 9)
#define SWOFTY_LIVE_SHAVE_SECTORS (1ull << 10)
#define SWOFTY_LIVE_SHAVE_ORDER (1ull << 11)
#define SWOFTY_LIVE_SHAVE_ROAD_ORDER (1ull << 12)

#define SWOFTY_LIVE_MODE_UNKNOWN 0
#define SWOFTY_LIVE_MODE_GRAND_PRIX 1
#define SWOFTY_LIVE_MODE_TIME_TRIAL 2

typedef struct SwoftyLiveSession {
    uint32_t version;
    uint32_t size;
    uint64_t have;
    uint32_t type;
    uint32_t cars, racers;
    int32_t player;
    int32_t watched;
    int32_t subject;
    uint32_t generation;
    uint32_t subject_generation;
    float track_length_m;
    uint8_t safety_car;
    uint8_t red_flag;
    uint8_t yellow_sectors;
    uint8_t chequered;
    uint8_t game_mode;
    uint8_t spectator_gap_mode;
    uint8_t pad[2];
    char track_name[48];
    float sector_end_m[8];
    uint32_t sector_count;
    uint32_t order[SWOFTY_LIVE_MAX_CARS];
    uint32_t classified;
    uint32_t road_order[SWOFTY_LIVE_MAX_CARS];
    uint32_t on_track;
    uint32_t reserved[8];
} SwoftyLiveSession;

#define SWOFTY_LIVE_TURN_OFFICIAL 1
#define SWOFTY_LIVE_TURN_LEARNED 2

typedef struct SwoftyLiveTurn {
    uint32_t version;
    uint32_t size;
    int32_t number;
    char letter;
    uint8_t right;
    uint8_t source;
    uint8_t pad;
    float distance_ahead_m;
    float apex_lap_m;
    char label[8];
    uint32_t reserved[4];
} SwoftyLiveTurn;

typedef struct SwoftyLiveImages SwoftyLiveImages;
typedef struct SwoftyLive {
    uint32_t version;
    int (*available)(void);
    int (*session)(SwoftyLiveSession *out);
    int (*car)(uint32_t vehicle, SwoftyLiveCar *out);
    int (*subject)(SwoftyLiveCar *out);
    int (*turn)(uint32_t vehicle, SwoftyLiveTurn *out);
    int (*track_turns)(uint32_t *count, SwoftyLiveTurn *out, uint32_t cap);
    int (*team_badge)(uint32_t vehicle, char *key, uint32_t cap);
    int (*seq)(uint64_t *frame);
    int (*driver_name)(uint32_t vehicle, char *out, uint32_t cap);
    int (*team_badge_texture)(uint32_t vehicle, char *path, uint32_t cap);
    const SwoftyLiveImages *(*images)(void);
} SwoftyLive;

/* Live car operations (standard library 1.6, experimental), from the core export SwoftyCoreCarOps; not a SwoftyApi
   field. Table version 2; version 1 is the first six members and keeps its layout. A car is the game's vehicle id
   (CarLive.vehicle_id, the slot of g_Sess_VehicleTable), dense from 0 in create order; the handle is that slot's
   CarLive and identifies the car while it exists (the game moves its last car into a destroyed car's slot). count:
   the cars in g_Sess_VehicleCount. car: the handle of the car with that index and its is_ai word (1 while the AI
   drives it, the player's car included). get and set take the index and optionally the handle (NULL skips the
   check) and one of the SWOFTY_CAR_* field ids; SWOFTY_ASSET_LATE when the slot is empty, holds another car or a
   system's owner does not match, SWOFTY_ASSET_INVALID for an unknown field or a value outside its range,
   SWOFTY_ASSET_UNAVAILABLE when the car has no such system, the running exe is not the build the field offsets were
   read from, or the field is off because a KB row it needs has no independent verify row yet (the core logs
   "<field> off: needs <row>" once). set writes one dword (an obfuscated value is re-encoded under its current key),
   only on the game's flow thread (else SWOFTY_ASSET_LATE), and the game reads it on its next use; the game
   overwrites it when it rebuilds the car, replays its R&D upgrades or applies a setup. SWOFTY_CAR_ENGINE_LIMITER is
   the handling value (rad/s); set also re-runs the engine's derived values (Car_Engine_ComputeDerived_Thunk), and
   answers SWOFTY_ASSET_UNAVAILABLE without writing when that function does not resolve. SWOFTY_CAR_FINAL_DRIVE and
   SWOFTY_CAR_GEAR_RATIO_1 + n (handling values) rebuild the gearbox ratio table after the write
   (Car_Tuning_SetFinalDrive 0x20|0x2000, no gear change); SWOFTY_ASSET_UNAVAILABLE when that function does not
   resolve or the gearbox builds its table from setup ratios.

   Version 2 members:
   field: the id of a field by its Handling path ("steering.max_steering_lock", "aero.lift_scale"), the path
   being the name car.profile records and the docs use; SWOFTY_ASSET_INVALID for an unknown path.
   layer: the core keeps a layer (owner, car index or SWOFTY_CAR_ALL, field, op, value, priority) and applies it
   to the matching cars now (on the flow thread) and again after every car.built, car.reset, R&D upgrade replay
   and setup apply, so the value outlives the game's own resets. Layers of one field compose by priority (lower
   first, so the higher one wins a set), then by the order they were added: set replaces the value, scale
   multiplies it, add adds to it. The base is the game's value after its last reset (a virtual field's neutral
   value: engine.power_scale 1, engine.limiter_offset_rpm 0, ai.grip_scale 1). A value outside the field's range
   is clamped when applied. token: the handle unlayer takes. SWOFTY_ASSET_UNAVAILABLE for an off field.
   unlayer: removes a layer by token and restores the cars it changed.
   refit_tyres: writes the TyrePhysicsGroup string of the car's Chassis and refits its tyres
   (Car_Tyre_SetWearFactor); SWOFTY_ASSET_UNAVAILABLE until a car-to-tyre-component lookup is bound.
   SWOFTY_CAR_TYRES_*_GRIP_SCALE is off: the tyre solver (Car_Suspension_SolveWheels) never reads
   CarSuspTyreType.grip_multiplier, only the AI snapshot and force feedback do.
   SWOFTY_CAR_CHASSIS_BALLAST_KG rebuilds the mass without fuel until the next fuel update.
   Virtual fields (engine.power_scale, engine.limiter_offset_rpm, ai.grip_scale) write nothing to game data: the
   core's handlers on site_Car_EngineDriveTorque and site_Ai_TableGripInput read them per car. */
#define SWOFTY_CAR_OPS_VERSION 4
#define SWOFTY_CAR_ALL 0xffffffffu

#define SWOFTY_CAR_STEERING_LOCK 1u
#define SWOFTY_CAR_CLUTCH_TORQUE 2u
#define SWOFTY_CAR_DIFF_ON_THROTTLE 3u
#define SWOFTY_CAR_DIFF_OFF_THROTTLE 4u
#define SWOFTY_CAR_TURBO_MAX_BOOST 5u
#define SWOFTY_CAR_FUEL_MASS 6u
#define SWOFTY_CAR_WING_SPEED_THRESHOLD 7u
#define SWOFTY_CAR_WING_DAMAGE_SCALE 8u
#define SWOFTY_CAR_WING_CHASSIS_DAMAGE_MULTIPLE 9u
#define SWOFTY_CAR_WING_SIDE_DETACH_SCALE 10u
#define SWOFTY_CAR_WING_FULL_DETACH_SCALE 11u
#define SWOFTY_CAR_SUSP_SPEED_THRESHOLD 12u
#define SWOFTY_CAR_SUSP_FRONT_DAMAGE_SCALE 13u
#define SWOFTY_CAR_SUSP_REAR_DAMAGE_SCALE 14u
#define SWOFTY_CAR_SUSP_WHEEL_DETACH_SCALE 15u
#define SWOFTY_CAR_SUSP_PARTS_DETACH_SCALE 16u
#define SWOFTY_CAR_SUSP_LINEAR_FRACTURE_LIMIT 17u
#define SWOFTY_CAR_SUSP_ANGULAR_FRACTURE_LIMIT 18u
#define SWOFTY_CAR_ENGINE_LIMITER 19u
#define SWOFTY_CAR_FINAL_DRIVE 20u
#define SWOFTY_CAR_GEAR_RATIO_1 21u /* 1st; 2nd..9th are 22u..29u */
#define SWOFTY_CAR_AERO_LIFT_SCALE 30u
#define SWOFTY_CAR_AERO_DRAG_SCALE 31u
#define SWOFTY_CAR_ENGINE_POWER_SCALE 32u
#define SWOFTY_CAR_ENGINE_LIMITER_OFFSET_RPM 33u
#define SWOFTY_CAR_AI_GRIP_SCALE 34u
#define SWOFTY_CAR_TYRES_FRONT_GRIP_SCALE 35u
#define SWOFTY_CAR_TYRES_REAR_GRIP_SCALE 36u
#define SWOFTY_CAR_CHASSIS_BALLAST_KG 37u
#define SWOFTY_CAR_BRAKES_BIAS 38u
#define SWOFTY_CAR_BRAKES_PRESSURE 39u
#define SWOFTY_CAR_SUSP_FRONT_CAMBER 40u
#define SWOFTY_CAR_SUSP_FRONT_TOE 41u
#define SWOFTY_CAR_SUSP_FRONT_ANTI_ROLL 42u
#define SWOFTY_CAR_SUSP_FRONT_RIDE_HEIGHT_OFFSET 43u
#define SWOFTY_CAR_SUSP_REAR_CAMBER 44u
#define SWOFTY_CAR_SUSP_REAR_TOE 45u
#define SWOFTY_CAR_SUSP_REAR_ANTI_ROLL 46u
#define SWOFTY_CAR_SUSP_REAR_RIDE_HEIGHT_OFFSET 47u

/* Version 3: the FuelECU mode tables (CarFuelEcu +0x28, stride 0x10 per mode, 0x40 per kind), 16 ids from
   SWOFTY_CAR_FUEL_BASE: base + kind * 4 + mode, kind 0 burn, 1 power, 2 revs, 3 temperature; mode 0 lean, 1 standard,
   2 rich, 3 max. Paths "fuel_ecu.fuel_burn_scale.lean" ... "fuel_ecu.temperature_scale.max". A write re-runs the
   active mode (Car_FuelEcu_SetMode_Thunk to another mode and back), so the car's engine and fuel flow follow. */
#define SWOFTY_CAR_FUEL_BASE 48u
#define SWOFTY_CAR_FUEL_BURN_LEAN 48u
#define SWOFTY_CAR_FUEL_POWER_LEAN 52u
#define SWOFTY_CAR_FUEL_REVS_LEAN 56u
#define SWOFTY_CAR_FUEL_TEMPERATURE_LEAN 60u

/* Version 4: the F1Aero slipstream scales (CarObfFloat at F1Aero +0xe8..+0x128), read afresh by the car step. */
#define SWOFTY_CAR_AERO_SLIPSTREAM_DRAG 64u
#define SWOFTY_CAR_AERO_SLIPSTREAM_FRONT_WING 65u
#define SWOFTY_CAR_AERO_SLIPSTREAM_REAR_WING 66u
#define SWOFTY_CAR_AERO_SLIPSTREAM_LIFT 67u
#define SWOFTY_CAR_AERO_SLIPSTREAM_CAR_WIDTH 68u

/* Version 5: the blocking evaluator's per-car AI config floats (ai.blocking.*), read afresh on every AI tick. Layers
   only (set is SWOFTY_ASSET_UNAVAILABLE): the core points the car's slot in its AI record at an own cell and puts
   the game's cell back when the last layer goes. get returns the value the AI reads. SWOFTY_ASSET_UNAVAILABLE
   until the AI system is built and settled. */
#define SWOFTY_CAR_AI_BLOCKING_FRACTION 69u
#define SWOFTY_CAR_AI_BLOCKING_FRACTION_RACE_START 70u
#define SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE 71u
#define SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE_RACE_START 72u

/* Version 6: the rest of the AI driver config's per-car slots, "ai.<group>.<key>" with the game's key spelling, one id
   each from 73 to 115 (alongside, divebomb_defense, offset_variation, overtake, space_seeking). Same rules as
   version 5. A curve slot (the *_graph, *_pull_out_time, overlap_percentage, concede_speed_diff,
   concede_time_vs_behind, min_presence_within_buffer and max_presence_alongside keys) is made a flat curve at the
   composed value; get returns the curve at x = 0 and a layer's base is that value. The space_seeking counts
   car_in_front_needed and total_queue_needed are int cells: the value is rounded to the nearest integer. */
#define SWOFTY_CAR_AI_ALONGSIDE_SPEED_DIFFERENCE_FOR_LIMITED_ZONE_YIELDING 73u
#define SWOFTY_CAR_AI_ALONGSIDE_YIELD_OVERTAKE_RATE 74u
#define SWOFTY_CAR_AI_ALONGSIDE_MIN_PRESENCE_WITHIN_BUFFER 75u
#define SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_PRESENCE_ALONGSIDE 76u
#define SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TIME_TO_IGNORE 77u
#define SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TRACK_OFFSET 78u
#define SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TRACK_RADIUS 79u
#define SWOFTY_CAR_AI_OFFSET_VARIATION_MAX_OFFSET 80u
#define SWOFTY_CAR_AI_OFFSET_VARIATION_OFFSET_LIMIT_AT_CORNER_START 81u
#define SWOFTY_CAR_AI_OFFSET_VARIATION_OFFSET_LIMIT_BLEND_DISTANCE 82u
#define SWOFTY_CAR_AI_OFFSET_VARIATION_HALF_WIDTH_SCALING_GRAPH 83u
#define SWOFTY_CAR_AI_OFFSET_VARIATION_RADIUS_SCALAR_GRAPH 84u
#define SWOFTY_CAR_AI_OVERTAKE_ATTACKER_CLOSE_ENOUGH_CAR_LENGTHS 85u
#define SWOFTY_CAR_AI_OVERTAKE_BRAKING_ZONE_ABORT_OVERTAKE_TIME 86u
#define SWOFTY_CAR_AI_OVERTAKE_CLOSE_RANGE_CAR_LENGTHS_TO_CONTINUE_OVERTAKE 87u
#define SWOFTY_CAR_AI_OVERTAKE_CLOSE_RANGE_CAR_LENGTHS_TO_START_OVERTAKE 88u
#define SWOFTY_CAR_AI_OVERTAKE_OPPERTUNIST_OVERTAKE_TIME 89u
#define SWOFTY_CAR_AI_OVERTAKE_OPPERTUNIST_OVERTAKE_TRACK_SPEED_DIFFERENCE 90u
#define SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_CONTINUE_TIME_ADJUSTMENT 91u
#define SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_FULL_HEAT_TIME_ADJUSTMENT 92u
#define SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_INITIATE_TIME_ADJUSTMENT 93u
#define SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_PULL_OUT_TIME 94u
#define SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_CONTINUE_TIME_ADJUSTMENT 95u
#define SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_FULL_HEAT_TIME_ADJUSTMENT 96u
#define SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_INITIATE_TIME_ADJUSTMENT 97u
#define SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_PULL_OUT_TIME 98u
#define SWOFTY_CAR_AI_OVERTAKE_OVERLAP_PERCENTAGE 99u
#define SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SPEED_DIFF 100u
#define SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SLOWDOWN_SPEED_DIFF 101u
#define SWOFTY_CAR_AI_OVERTAKE_CONCEDE_TIME_VS_BEHIND 102u
#define SWOFTY_CAR_AI_SPACE_SEEKING_ADDED_BUFFER 103u
#define SWOFTY_CAR_AI_SPACE_SEEKING_SPEED_BELOW_FOLLOW 104u
#define SWOFTY_CAR_AI_SPACE_SEEKING_FIRST_SPEED_BELOW_FOLLOW 105u
#define SWOFTY_CAR_AI_SPACE_SEEKING_CAR_IN_FRONT_NEEDED 106u
#define SWOFTY_CAR_AI_SPACE_SEEKING_REMEMBER_CROWED 107u
#define SWOFTY_CAR_AI_SPACE_SEEKING_CONCEDE_OVERTAKE_BLOCKED_BY_CROWED_TIME 108u
#define SWOFTY_CAR_AI_SPACE_SEEKING_TOTAL_QUEUE_NEEDED 109u
#define SWOFTY_CAR_AI_SPACE_SEEKING_DISTANCE_BRAKING_POINT 110u
#define SWOFTY_CAR_AI_SPACE_SEEKING_TIME_TO_SWITCH_SIDES 111u
#define SWOFTY_CAR_AI_SPACE_SEEKING_TRACK_DISTANCE_NEEDED 112u
#define SWOFTY_CAR_AI_SPACE_SEEKING_TRACK_ABORT_DISTANCE 113u
#define SWOFTY_CAR_AI_SPACE_SEEKING_CARS_MOVING_AWAY_SPEED_TO_IGNORE 114u
#define SWOFTY_CAR_AI_SPACE_SEEKING_CAN_START_TRACK_RADIUS 115u

/* Version 6, continued: the rest of the blocking group, ids 116 to 127, same rules. The curve keys (start_blocking_minimum_time_to_corner, end_blocking_time_to_corner_vs_offset, blocking_offset_from_car_behind, merge_back_to_racing_line) are made flat like the other curve slots. */
#define SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_INTERSECTION_TIME 116u
#define SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_MAXIMUM_TIME_TO_CORNER 117u
#define SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_MINIMUM_TIME_TO_CORNER 118u
#define SWOFTY_CAR_AI_BLOCKING_TIME_TO_CORNER_FOR_END_MERGE_RATE 119u
#define SWOFTY_CAR_AI_BLOCKING_TIME_TO_CORNER_FOR_START_MERGE_RATE 120u
#define SWOFTY_CAR_AI_BLOCKING_END_BLOCKING_TIME_TO_CORNER_VS_OFFSET 121u
#define SWOFTY_CAR_AI_BLOCKING_BLOCKING_OFFSET_FROM_CAR_BEHIND 122u
#define SWOFTY_CAR_AI_BLOCKING_BLOCKING_SPEED_DIFFERENTIAL 123u
#define SWOFTY_CAR_AI_BLOCKING_BLOCKING_THREAT_TIME_MODIFIER 124u
#define SWOFTY_CAR_AI_BLOCKING_BLOCKING_VARIABLE_OFFSET_FRACTION 125u
#define SWOFTY_CAR_AI_BLOCKING_AVOIDANCE_DECREASE_RATE 126u
#define SWOFTY_CAR_AI_BLOCKING_MERGE_BACK_TO_RACING_LINE 127u

#define SWOFTY_CAR_FUEL_MIX_LEAN 1u
#define SWOFTY_CAR_FUEL_MIX_STANDARD 2u
#define SWOFTY_CAR_FUEL_MIX_RICH 4u
#define SWOFTY_CAR_FUEL_MIX_MAX 8u

#define SWOFTY_CAR_OP_SET 0u
#define SWOFTY_CAR_OP_SCALE 1u
#define SWOFTY_CAR_OP_ADD 2u

typedef struct SwoftyCarOps {
    uint32_t version;
    int (*available)(void);
    int (*count)(uint32_t *cars);
    int (*car)(uint32_t index, void **handle, uint32_t *is_ai);
    int (*get)(uint32_t index, const void *handle, uint32_t field, float *value);
    int (*set)(uint32_t index, const void *handle, uint32_t field, float value);
    /* version 2 */
    int (*field)(const char *path, uint32_t *id);
    int (*layer)(const char *owner, uint32_t index, uint32_t field, uint32_t op, float value, int32_t priority,
                 uint32_t *token);
    int (*unlayer)(uint32_t token);
    int (*refit_tyres)(uint32_t index, const char *group);
    /* version 3. unlock_fuel_mixes: the mixes in mask (SWOFTY_CAR_FUEL_MIX_*) are offered on the MFD of every car
       while the owner's mask stays set; the core clears EngineModeAvail's entry of each (car, mix) every frame.
       A mask of 0 withdraws the owner's request. SWOFTY_ASSET_UNAVAILABLE when the table or the exe build is not
       the KB's. fuel_mix: the car's current mix (0 lean .. 3 max) in *mode. */
    int (*unlock_fuel_mixes)(const char *owner, uint32_t mask);
    int (*fuel_mix)(uint32_t index, uint32_t *mode);
    /* version 4. set_fuel_mix: switches the car to mix `mode` (0 lean .. 3 max) through the game's own mode switch.
       Only on the flow thread (SWOFTY_ASSET_LATE otherwise). SWOFTY_ASSET_INVALID for a mode above 3 or a mix the
       MFD does not offer the car (unlock_fuel_mixes makes one offered); SWOFTY_ASSET_UNAVAILABLE when the core has
       no mode switch on this build. */
    int (*set_fuel_mix)(uint32_t index, uint32_t mode);
} SwoftyCarOps;

/* Memory checks (standard library, swofty/mem.hpp), from the core export SwoftyCoreMem; not a SwoftyApi field, so
   SWOFTY_API_VERSION is unchanged and the table carries its own version. The core keeps one page table shared by
   every thread of every module, filled from the process working set (K32QueryWorkingSetEx, constant time per page)
   and from VirtualQuery only for pages that are not resident. It never touches a page to test it, so guard pages
   stay armed. An answer stays good for 250 ms, a refused one for 32 ms. Both calls are safe from any thread.
   Table version 1.
   readable: nonzero when all n bytes at p are committed and readable (an n of 0 counts as 1); 0 for a null
   pointer, an address in the first 64 KB or above user space, a range that wraps, and pages marked PAGE_GUARD or
   PAGE_NOACCESS. invalidate: drops every answer in the table, for every thread. */
#define SWOFTY_MEM_VERSION 1

typedef struct SwoftyMem {
    uint32_t version;
    int (*readable)(const void *p, size_t n);
    void (*invalidate)(void);
} SwoftyMem;

typedef struct SwoftyApi {
    uint32_t version;
    void (*log)(const char *fmt, ...);
    uintptr_t (*module_base)(void);
    int (*scan)(const char *section, const char *pattern, uintptr_t *out);
    uintptr_t (*resolve)(const char *id);
    int (*register_mixins)(const SwoftyMixin *mixins, size_t count);
    void *(*fields)(const void *obj, size_t size);
    int (*setting)(const char *id, uint32_t type, SwoftySettingValue *out);
    int (*watch_setting)(const char *id, SwoftySettingFn fn, void *user);
    const SwoftyAssets *assets;
    const SwoftyEvents *events;
    const SwoftyOverlay *overlay;
    const SwoftyTelemetry *telemetry;
    const SwoftyDb *db;
    const SwoftyStorage *storage;
    const SwoftyCareer *career;
    const SwoftyPopup *popup;
    const SwoftyLang *lang;
    const SwoftyCareerContent *career_content;
    const SwoftyCharacters *characters;
    const SwoftyCareerWizard *career_wizard;
    const SwoftyEra *era;
    const SwoftyRegistry *registry;
    const SwoftyHooks *hooks;
    const SwoftyServices *services;
} SwoftyApi;

typedef int (*SwoftyAddonInitFn)(const SwoftyApi *api);
typedef SwoftyAddonInitFn SwoftyModInitFn;

/* Lap host, from the core export SwoftyCoreLapHost; not a SwoftyApi field. Table version 1. What a lap module
   (a plugin that records laps and serves lap boards) needs from the core beyond the SDK. A string a function
   returns belongs to the calling thread until its next call of this table; never NULL. NULL table: a core
   without it.
   attach: the module's callbacks; one client, the last attach wins, NULL detaches. The core calls offer while
   it collects hook sets (the module calls add_hooks there) and settle once they are installed or dropped; tick
   on every flow tick; session_enter and session_watch(1) as a session starts, session_end as it ends;
   lap_line when a car starts a new lap; recording, board_session and player_id are asked by the core's toasts
   and name display.
   add_hooks: a named hook set installed like the core's own (hooks_active says whether it is in place).
   build: bit 0 the KB build, bit 1 a build the layouts are checked on. feature: a core feature switch.
   swofty_root, files_root: the game's swofty folder and the folder the launcher collects files from, UTF-8.
   mode_kind: 0 unknown, 1 time trial, 2 career, 3 other; it starts the mode watch. view_flags: the view state's
   flags. time_trial_*: the Time Trial mode's selection (track codes, entrant list, the player's team and a team
   reference's label), its track record stat (-1), setup(request JSON) for a session of that mode (0 pending,
   1 done, -1 failed with why) and setup_reset. press_list: an entrant list counts as the pressed box.
   session_unlisted: the session's list no box names. track: the track base's entry at index (0 past the end;
   kind 0 stock, 1 record, 2 older). content_ids: the stat ids of a content id registry. car, motion: the
   player's car this frame. sealing, recorded: fire laps.sealing and laps.recorded for a lap file the module
   wrote. copy: a guarded memory write. notice: one line of text the core paints at the top of the overlay frame
   (NULL or empty: none); the core asks the client notice_wanted once, at overlay setup, whether to arm the
   overlay for it, and gives screen every UI screen that opens. name_tick: the core's player name display step, run
   from the module's in-session tick (probe 1: its probe step). */
typedef struct SwoftyLapClient {
    uint32_t size, version;
    void (*offer)(void);
    void (*settle)(void);
    void (*tick)(void);
    void (*session_enter)(void);
    void (*session_watch)(int live);
    void (*session_end)(void);
    void (*lap_line)(int car);
    int (*recording)(void);
    int (*board_session)(void);
    int (*player_id)(void *out, uint32_t size);
    void (*screen)(const char *name);
    int (*notice_wanted)(void);
} SwoftyLapClient;

typedef struct SwoftyLapTrack {
    const char *id, *code, *base, *addon, *name;
    int32_t stat_id;
    uint32_t kind, time_trial;
} SwoftyLapTrack;

typedef struct SwoftyLapCar {
    uint32_t size, has;
    float wetness, wheel_wetness, speed_kmh, throttle, brake, steer, rpm, max_rpm, g_lateral, g_long;
    int32_t gear, player, ai;
    uint8_t surface[4];
} SwoftyLapCar;

typedef struct SwoftyLapMotion {
    uint32_t size, has;
    float pos[3], vel[3], angvel[3], quat[4], spin[4], slip_angle[4], slip_ratio[4], drs;
    int32_t player, ai;
} SwoftyLapMotion;

typedef void (*SwoftyLapFlowLinkFn)(uint8_t *state, uint32_t *link);
typedef int32_t (*SwoftyLapSectionFn)(void *user, const char *tag, const void *bytes, uint32_t len);

typedef struct SwoftyLapHost {
    uint32_t size, version;
    int (*attach)(const SwoftyLapClient *client);
    int (*add_hooks)(const char *name, const SwoftyMixin *mixins, size_t count);
    int (*hooks_active)(const char *name);
    int (*when_code_ready)(const char *what, void (*fn)(void));
    int (*code_ready)(void);
    uint32_t (*build)(void);
    const char *(*build_sha)(void);
    int (*feature)(const char *name);
    const char *(*swofty_root)(void);
    const char *(*files_root)(void);
    int (*session_kind)(void);
    int (*mode_kind)(void);
    const char *(*session_mode)(void);
    const char *(*session_content)(void);
    const char *(*content_title)(const char *content);
    const char *(*player_label)(uint32_t what); /* 0 team, 1 vehicle, 2 driver */
    uint32_t (*view_flags)(void);
    void *(*game_manager)(void);
    int (*flow_link_listen)(SwoftyLapFlowLinkFn listener);
    const char *(*time_trial_track)(int session);
    const char *(*time_trial_list)(void);
    int (*time_trial_team)(uint32_t *ref, const char **label);
    const char *(*time_trial_team_of)(uint32_t ref);
    int (*time_trial_track_stat)(void);
    int (*time_trial_setup)(const char *request, const char **why);
    void (*time_trial_setup_reset)(void);
    void (*press_list)(const char *entrant_list);
    int (*session_unlisted)(const char **list);
    int (*track)(uint32_t index, SwoftyLapTrack *out);
    uint32_t (*content_ids)(const char *registry, int64_t *out, uint32_t cap);
    int (*car)(SwoftyLapCar *out);
    int (*motion)(SwoftyLapMotion *out);
    uint32_t (*car_state)(void); /* bit 0 the car table resolves, bit 1 KB fields */
    uintptr_t (*player_car)(void);
    int (*sealing)(const char *path, int32_t time_ms, const uint8_t *body, uint32_t body_len, SwoftyLapSectionFn add_section,
                   void *user);
    int (*recorded)(const char *path, int32_t time_ms, int32_t valid);
    int (*copy)(void *to, const void *from, size_t n);
    void (*notice)(const char *text);
    void (*name_tick)(int probe);
} SwoftyLapHost;

#define SWOFTY_LAP_HOST_VERSION 1
#define SWOFTY_LAP_CAR_SPEED 1u
#define SWOFTY_LAP_CAR_PEDALS 2u
#define SWOFTY_LAP_CAR_STEER 4u
#define SWOFTY_LAP_CAR_GEAR 8u
#define SWOFTY_LAP_CAR_RPM 16u
#define SWOFTY_LAP_CAR_G 32u
#define SWOFTY_LAP_CAR_SURFACE 64u
#define SWOFTY_LAP_CAR_WETNESS 128u
#define SWOFTY_LAP_CAR_WHEEL_WETNESS 256u
#define SWOFTY_LAP_MOTION_POS 1u
#define SWOFTY_LAP_MOTION_VEL 2u
#define SWOFTY_LAP_MOTION_ANGVEL 4u
#define SWOFTY_LAP_MOTION_ORIENT 8u
#define SWOFTY_LAP_MOTION_SPIN 16u
#define SWOFTY_LAP_MOTION_SLIP 32u
#define SWOFTY_LAP_MOTION_DRS 64u

/* Session host, from the core export SwoftyCoreSessionHost; not a SwoftyApi field. Table version 1. It serves
   one plugin that sets up an online session from outside the menus; each listener has one slot and a later call
   replaces it. root: the game's swofty folder, with a trailing backslash. on_ready: fn runs once the core's
   runtime setup is done, at once when it already is; the game's code is readable from then on. on_flow: every
   flow state change (the state's name hash and its new status), on the flow thread. on_screen: every screen
   the UI opens, by name. on_message: every game message before it is queued, with its reply callback and that
   callback's context; a nonzero answer takes the message, which is then not shown. on_home: the home view
   model on every home command. on_exit: the process is ending.
   pad_claim: takes (1) or drops (0) a claim on the core's virtual pad, installing it first; 0 when there is
   none, and pad_error says why. pad_set holds buttons and triggers, pad_release lets them go.
   direct_start_setup: the setup of the direct start's external target, polled on the flow thread until it
   answers 1 (done) or 2 (failed, the reason in why); 0 is pending. cover_step names the launch cover's step
   and its progress (ceiling below 0: none); cover_state copies the step's label and percent and answers 1
   while the cover is armed; cover_signal sets what the cover of an external target waits for: -1 failed,
   0 waiting, 1 shown. ui_set writes one UI view-model path (type 0: a string; 1: a bool and 2: an integer, both from
   an int32); car_category chooses a Car Category box by value as the screen would (empty: a stock one), 0 with the reason in why. */
#define SWOFTY_SESSION_HOST_VERSION 1
typedef int (*SwoftySessionSetupFn)(void *user, char *why, size_t cap);
typedef int (*SwoftySessionSignalFn)(void *user);
typedef struct SwoftySessionHost {
    uint32_t size;
    uint32_t version;
    const wchar_t *(*root)(void);
    void (*on_ready)(void (*fn)(void *user), void *user);
    void (*on_flow)(void (*fn)(uint32_t state, uint32_t status, void *user), void *user);
    void (*on_screen)(void (*fn)(const char *name, void *user), void *user);
    void (*on_message)(int (*fn)(const char *id, void *reply, void *ctx, void *user), void *user);
    void (*on_home)(void (*fn)(void *vm, void *user), void *user);
    void (*on_exit)(void (*fn)(void *user), void *user);
    int (*pad_claim)(int on);
    const char *(*pad_error)(void);
    void (*pad_set)(uint16_t buttons, uint8_t lt, uint8_t rt);
    void (*pad_release)(void);
    void (*direct_start_setup)(SwoftySessionSetupFn fn, void *user);
    void (*cover_step)(const char *what, float fraction, float ceiling);
    int (*cover_state)(char *label, size_t cap, int *percent);
    void (*cover_signal)(SwoftySessionSignalFn fn, void *user);
    int (*ui_set)(const char *path, int type, const void *value);
    int (*car_category)(const char *value, char *why, size_t cap);
} SwoftySessionHost;

typedef int (*SwoftyCoreAttachFn)(SwoftyApi *api);
typedef int (*SwoftyCoreInitFn)(const wchar_t *name, SwoftyAddonInitFn init);
typedef int (*SwoftyCoreCommitFn)(void);
typedef int (*SwoftyCoreStageFn)(int stage);
typedef int (*SwoftyAddonRegisterFn)(const SwoftyApi *api);
typedef const SwoftyRegistry *(*SwoftyCoreRegistryFn)(void);
typedef const SwoftyHooks *(*SwoftyCoreHooksFn)(void);
typedef const SwoftyServices *(*SwoftyCoreServicesFn)(void);
typedef const SwoftyCareerOps *(*SwoftyCoreCareerOpsFn)(void);
typedef const SwoftyCareerSaves *(*SwoftyCoreCareerSavesFn)(void);
typedef const SwoftySessionOps *(*SwoftyCoreSessionOpsFn)(void);
typedef const SwoftyCarOps *(*SwoftyCoreCarOpsFn)(void);
typedef const SwoftyContentOps *(*SwoftyCoreContentOpsFn)(void);
typedef const SwoftyLive *(*SwoftyCoreLiveFn)(void);
typedef const SwoftyMem *(*SwoftyCoreMemFn)(void);
typedef const SwoftyLapHost *(*SwoftyCoreLapHostFn)(void);
typedef const SwoftySessionHost *(*SwoftyCoreSessionHostFn)(void);

#define SWOFTY_STAGE_WINDOW 1

#if defined(_WIN32)
#define SWOFTY_EXPORT __declspec(dllexport)
#else
#define SWOFTY_EXPORT __attribute__((visibility("default")))
#endif

/* Core exports for feature detection (API v9), and the addon export SWOFTY_ADDON() emits when the file
   that expands it includes swofty/registry.hpp: the loader calls SwoftyAddonRegister at the early stage,
   in load order, and SwoftyAddonInit at the stage the manifest asks for. */
SWOFTY_EXPORT const SwoftyRegistry *SwoftyCoreRegistry(void);
SWOFTY_EXPORT const SwoftyHooks *SwoftyCoreHooks(void);
SWOFTY_EXPORT const SwoftyServices *SwoftyCoreServices(void);
SWOFTY_EXPORT const SwoftyCareerOps *SwoftyCoreCareerOps(void);
SWOFTY_EXPORT const SwoftyCareerSaves *SwoftyCoreCareerSaves(void);
SWOFTY_EXPORT const SwoftySessionOps *SwoftyCoreSessionOps(void);
SWOFTY_EXPORT const SwoftyCarOps *SwoftyCoreCarOps(void);
SWOFTY_EXPORT const SwoftyContentOps *SwoftyCoreContentOps(void);
SWOFTY_EXPORT const SwoftyLive *SwoftyCoreLive(void);
SWOFTY_EXPORT const SwoftyMem *SwoftyCoreMem(void);
SWOFTY_EXPORT const SwoftyLapHost *SwoftyCoreLapHost(void);
SWOFTY_EXPORT const SwoftySessionHost *SwoftyCoreSessionHost(void);

#ifdef __cplusplus
}
#endif
