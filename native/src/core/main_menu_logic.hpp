#pragma once

#include <cstdint>
#include <string>
#include <vector>

#include "json.hpp"

namespace runtime::main_menu {

constexpr const char *ID_STORY = "story";
constexpr const char *ID_CAREER = "career";
constexpr const char *ID_MOVIE = "zenith_button";
constexpr const char *ID_F1_WORLD = "f1_world";
constexpr const char *ID_CHALLENGE = "home_navigate_to_challenge_event";
constexpr const char *ID_LOCAL = "local_multiplayer";
constexpr const char *ID_LEAGUE = "pro_leagues";
constexpr const char *ID_HUB = "player_hub";
constexpr const char *ID_THEATRE = "theatre";
constexpr const char *ID_SETTINGS = "mod_menu";
constexpr const char *ID_GRAND_PRIX = "swofty_grand_prix";
constexpr const char *ID_TIME_TRIAL = "swofty_time_trial";
constexpr const char *ID_SHOWROOM = "swofty_showroom";
constexpr const char *ID_CUSTOMISATION = "swofty_customisation";
constexpr const char *ID_DIVIDER = "swofty_divider";
constexpr const char *STOCK_ROUTE_F1_WORLD = "f1_world";
constexpr const char *TEMPLATE_DEFAULT = "fe_tile_button_default";
constexpr const char *TEMPLATE_DIVIDER = "swofty_home_divider";

constexpr const char *SPLIT_SCREEN_TITLE = "SPLIT SCREEN";
constexpr const char *SPLIT_SCREEN_DESCRIPTION = "Race a friend on one screen";

constexpr const char *TT_TITLE = "Not signed in";
constexpr const char *TT_BODY = "Sign in with Discord in Swofty Launcher to save your laps to the Time Trial boards. "
                                "Laps from this session will not be saved. Press OK to continue to Time Trial.";

constexpr const char *TEMPLATE_TOP_LEFT = "swofty_home_top_left";
constexpr const char *TEMPLATE_TOP_RIGHT = "swofty_home_top_right";
constexpr const char *TEMPLATE_TOP_THIRD = "swofty_home_top_third";
constexpr const char *TEMPLATE_MODES_LEFT = "swofty_home_modes_left";
constexpr const char *TEMPLATE_MODES_RIGHT = "swofty_home_modes_right";
constexpr const char *TEMPLATE_MODES_THIRD = "swofty_home_modes_third";
constexpr const char *TEMPLATE_WIDE_A = "swofty_home_wide_a";
constexpr const char *TEMPLATE_WIDE_B = "swofty_home_wide_b";
constexpr const char *TEMPLATE_WIDE_C = "swofty_home_wide_c";

inline const char *template_for(const std::string &id, bool rows = true)
{
    if (!rows)
        return id == ID_THEATRE || id == ID_SHOWROOM || id == ID_CUSTOMISATION || id == ID_LOCAL ? TEMPLATE_DEFAULT : "";
    if (id == ID_DIVIDER)
        return TEMPLATE_DIVIDER;
    if (id == ID_STORY)
        return TEMPLATE_TOP_LEFT;
    if (id == ID_CAREER)
        return TEMPLATE_TOP_RIGHT;
    if (id == ID_CHALLENGE)
        return TEMPLATE_TOP_THIRD;
    if (id == ID_GRAND_PRIX)
        return TEMPLATE_MODES_LEFT;
    if (id == ID_TIME_TRIAL)
        return TEMPLATE_MODES_RIGHT;
    if (id == ID_LOCAL)
        return TEMPLATE_MODES_THIRD;
    if (id == ID_THEATRE)
        return TEMPLATE_WIDE_A;
    if (id == ID_SHOWROOM)
        return TEMPLATE_WIDE_B;
    if (id == ID_SETTINGS)
        return TEMPLATE_WIDE_C;
    return "";
}

constexpr uint16_t WATCH_PAD_MASK = 0x0200;
constexpr const char *SHOWROOM_ROUTE = "swofty_showroom";
constexpr const char *CUSTOMISATION_ROUTE = "swofty_customisation";
constexpr const char *CUSTOMISATION_TITLE = "CUSTOMISATION";
constexpr const char *CUSTOMISATION_DESCRIPTION = "Your helmet, race suit, gloves, car and badges";
constexpr const char *HOME_BACK_ROUTE = "swofty_home";
constexpr const char *BACK_HOME_FROM_CONTAINER = "swofty_back_home";
constexpr const char *TIME_TRIAL_ROUTE = "swofty_time_trial";
constexpr const char *GRAND_PRIX_ROUTE = "swofty_grand_prix";
constexpr const char *GRAND_PRIX_SAVES_ROUTE = "swofty_grand_prix_saves";
constexpr const char *STOCK_ACTIVITY_ROUTE = "home_to_activity";
constexpr const char *GRAND_PRIX_COMMAND = "solo_gp";
constexpr const char *FRONTEND_FLOW = "frontend_flow";
constexpr const char *F1_WORLD_FLOW = "f1_world_flow";
constexpr const char *CUSTOMISATION_FLOW = "customisation_flow";
constexpr const char *CUSTOMISATION_GROUP = "customisation_f1_life_entry";

constexpr uint32_t WATCH_KEY_VK = 0x56;
constexpr double PANEL_BUDGET_MS = 4.0;
constexpr double ACCOUNT_AT_X = 4.06, ACCOUNT_AT_Y = 3.91;
constexpr double ACCOUNT_AVATAR_DX = -0.674, ACCOUNT_AVATAR_DY = 0.834, ACCOUNT_AVATAR_SIZE = 0.68;
// The video card's right edge (x 9.21, 1800 px at 1908x1028) and bottom edge (y -4.82, 956 px), scene units.
constexpr double CARD_RIGHT_X = 9.21, CARD_BOTTOM_Y = -4.82;
constexpr double SCENE_UNITS_TALL = 11.0, SCENE_CENTRE_X_SHIFT = -0.0148, SCENE_CENTRE_Y = 0.4918;

enum class Screen { overlay, home, other };

inline Screen classify_screen(const std::string &name)
{
    static const char *const overlays[] = {"persistent", "playercard", "render_badge", "theming_overlay_frontend",
                                           "standard_wipe", "frontend_transition", "slice_bg_content_underlay",
                                           "f1_life_driver_tags", "audio_themes", "fe_planes", "wipes",
                                           "tutorials_masked"};
    if (name == "fe_home")
        return Screen::home;
    for (const char *o : overlays)
        if (name == o)
            return Screen::overlay;
    return Screen::other;
}

struct HomeView {
    bool home = false;
    void screen(const std::string &name)
    {
        switch (classify_screen(name)) {
        case Screen::home:
            home = true;
            break;
        case Screen::other:
            home = false;
            break;
        case Screen::overlay:
            break;
        }
    }
    void left() { home = false; }
    bool visible(bool in_session, bool hub_updating) const { return home && !in_session && !hub_updating; }
};

struct Point {
    float x = 0, y = 0;
};

inline float scene_scale(float height) { return height / float(SCENE_UNITS_TALL); }

struct VideoCard {
    float pad = 0, gap = 0, thumb_w = 0, thumb_h = 0, text_max = 0, width = 0, height = 0;
};

// The video card for a screen of height k * 1028: a 16:9 thumbnail with an even inset, a text block of the measured
// width (at most 420 px at 1028) and a minimum width (480 px at 1028). It grows to the left of its right edge.
inline VideoCard plan_card(float k, float text_w)
{
    VideoCard c;
    c.pad = 12 * k;
    c.gap = 28 * k;
    c.thumb_h = 150 * k;
    c.thumb_w = c.thumb_h * 16.0f / 9.0f;
    c.text_max = 420 * k;
    float tw = text_w < c.text_max ? text_w : c.text_max;
    float want = c.pad + tw + c.gap + c.thumb_w + c.pad;
    c.width = want > 480 * k ? want : 480 * k;
    c.height = c.thumb_h + 2 * c.pad;
    return c;
}

// The 16:9 band the front end is laid out in, centred in the frame: the full height of a 16:9 or wider frame and
// the full width of a taller one (16:10 on a Steam Deck), with the game's backdrop above and below it.
struct Band {
    float left = 0, top = 0, width = 0, height = 0;
};

inline Band ui_band(float width, float height)
{
    Band b;
    b.height = width * 9.0f / 16.0f < height ? width * 9.0f / 16.0f : height;
    b.width = b.height * 16.0f / 9.0f;
    b.left = (width - b.width) * 0.5f;
    b.top = (height - b.height) * 0.5f;
    return b;
}

// The panel's size unit: 1 at a band 1028 px tall.
inline float panel_scale(float width, float height) { return ui_band(width, height).height / 1028.0f; }

inline Point scene_to_px(double x, double y, float width, float height)
{
    Band b = ui_band(width, height);
    float s = scene_scale(b.height);
    return {width * 0.5f + float(SCENE_CENTRE_X_SHIFT) * b.height + s * float(x),
            b.top + float(SCENE_CENTRE_Y) * b.height - s * float(y)};
}

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    for (; *s; s++)
        h = h * 33 + uint8_t(*s);
    return h;
}

inline std::string button_name(bool pad) { return pad ? "RB" : "V"; }

inline std::string open_request_json(const std::string &url)
{
    std::string out = "{\"url\":\"";
    for (char c : url) {
        if (c == '"' || c == '\\')
            out += '\\';
        if (uint8_t(c) >= 0x20)
            out += c;
    }
    return out + "\"}";
}

struct Card {
    std::string id;
    int row = 0;
    int column = 0;
    bool wide = false;
};

struct Plan {
    std::vector<Card> cards;
    std::vector<std::string> dropped;
};

inline bool removed(const std::string &id)
{
    return id == ID_F1_WORLD || id == ID_LEAGUE || id == ID_HUB || id == ID_MOVIE || id == "divider";
}

inline Plan layout(const std::vector<std::string> &stock, bool showroom_available, bool customisation_available)
{
    Plan plan;
    auto has = [&](const char *id) {
        for (auto &s : stock)
            if (s == id)
                return true;
        return false;
    };
    int col = 0;
    auto top = [&](const std::string &id) { plan.cards.push_back({id, 0, col++, false}); };
    if (has(ID_STORY))
        top(ID_STORY);
    if (has(ID_CAREER))
        top(ID_CAREER);
    if (has(ID_CHALLENGE))
        top(ID_CHALLENGE);
    for (auto &s : stock) {
        if (s == ID_CAREER || s == ID_STORY || s == ID_CHALLENGE || s == ID_LOCAL || s == ID_THEATRE || s == ID_SETTINGS)
            continue;
        if (removed(s))
            plan.dropped.push_back(s);
        else
            top(s);
    }
    if (has(ID_F1_WORLD)) {
        plan.cards.push_back({ID_GRAND_PRIX, 1, 0, false});
        plan.cards.push_back({ID_TIME_TRIAL, 1, 1, false});
    }
    if (has(ID_LOCAL))
        plan.cards.push_back({ID_LOCAL, 1, 2, false});
    int row = 2;
    if (has(ID_THEATRE))
        plan.cards.push_back({ID_THEATRE, row++, 0, true});
    if (showroom_available)
        plan.cards.push_back({ID_SHOWROOM, row++, 0, true});
    if (customisation_available)
        plan.cards.push_back({ID_CUSTOMISATION, row++, 0, true});
    if (has(ID_SETTINGS)) {
        plan.cards.push_back({ID_DIVIDER, row++, 0, true});
        plan.cards.push_back({ID_SETTINGS, row++, 0, true});
    }
    return plan;
}

struct State {
    bool loaded = false;
    std::string version;
    bool signed_in = false;
    std::string name;
    std::string avatar;
    std::string video_title;
    std::string video_url;
    std::string video_thumbnail;
    long long online = 0;
};

inline std::string with_separators(long long n)
{
    std::string digits = std::to_string(n < 0 ? -n : n), out;
    for (size_t i = 0; i < digits.size(); i++) {
        if (i && (digits.size() - i) % 3 == 0)
            out += ',';
        out += digits[i];
    }
    return (n < 0 ? "-" : "") + out;
}

constexpr const char *OFFLINE_TEXT = "Launched with Swofty Launcher";

inline std::string online_line(long long n)
{
    return n <= 0 ? "" : with_separators(n) + " currently playing";
}

inline std::string offline_text(long long n)
{
    std::string line = online_line(n);
    return line.empty() ? std::string(OFFLINE_TEXT) : std::string(OFFLINE_TEXT) + "\n" + line;
}

inline State parse_state(const std::string &text)
{
    State s;
    json::Value doc;
    if (!json::parse(text, doc) || doc["schema"].num() != 1)
        return s;
    s.loaded = true;
    s.version = doc["launcher_version"].str();
    auto &d = doc["discord"];
    s.signed_in = d["signed_in"].type == json::Value::Bool && d["signed_in"].b;
    s.name = d["name"].str();
    s.avatar = d["avatar"].str();
    auto &v = doc["video"];
    s.video_title = v["title"].str();
    s.video_url = v["url"].str();
    s.video_thumbnail = v["thumbnail"].str();
    s.online = doc["online"].num(0);
    return s;
}

inline std::string version_line(const State &s)
{
    return s.version.empty() ? "Swofty Launcher" : "Swofty Launcher " + s.version;
}

inline std::string discord_line(const State &s)
{
    if (!s.signed_in)
        return "Not signed in to Discord";
    return s.name.empty() ? "Signed in to Discord" : "Signed in to Discord as " + s.name;
}

inline bool has_avatar(const State &s) { return s.signed_in && !s.avatar.empty(); }

inline bool has_video(const State &s) { return !s.video_url.empty() && !s.video_thumbnail.empty(); }

inline std::string watch_line(const std::string &button)
{
    return "Press " + button + " to watch";
}

inline bool panel_due(bool patched, bool home_open, bool loaded, bool disabled, bool prepared)
{
    return patched && home_open && loaded && prepared && !disabled;
}

inline bool panel_too_slow(double draw_ms) { return draw_ms > PANEL_BUDGET_MS; }

inline bool time_trial_warning_due(const State &s, bool is_time_trial, bool already_shown)
{
    return s.loaded && is_time_trial && !s.signed_in && !already_shown;
}


// A route fired from the home screen hides the panel, except the launcher's own that keep the screen (the Showroom's, Addon Settings').
inline bool hides_panel(const std::string &route)
{
    return route.rfind("swofty_", 0) != 0 || route == TIME_TRIAL_ROUTE || route == GRAND_PRIX_ROUTE || route == GRAND_PRIX_SAVES_ROUTE ||
           route == CUSTOMISATION_ROUTE;
}

enum class Mode { none, time_trial, grand_prix };

inline const char *press_route(Mode m)
{
    switch (m) {
    case Mode::time_trial:
        return TIME_TRIAL_ROUTE;
    case Mode::grand_prix:
        return STOCK_ACTIVITY_ROUTE;
    default:
        return "";
    }
}

// The Grand Prix card opens the activity container through the game's own home_to_activity (an entry with no command)
// and raises solo_gp on it, the command the container's own Grand Prix card raises. The game answers with
// grand_prix_resume, whose route migrates to the front-end flow's enter_gp_resume and opens grand_prix_continue: the
// saved seasons, Continue and New Grand Prix, as F1 World > Play > Grand Prix shows them; with no saved season the game
// opens the new Grand Prix picker itself.

inline const char *return_command(Mode m)
{
    switch (m) {
    case Mode::time_trial:
        return "time_trial";
    case Mode::grand_prix:
        return GRAND_PRIX_COMMAND;
    default:
        return "";
    }
}

// The card mode a session counts for, from the game mode of the session that loaded (apply::facts::mode_of names). The
// container a card opens keeps its own tabs, so a Grand Prix card press can end in a Time Trial (or the other way round):
// the session's mode then wins. Any other known mode has no card command; an unread or unknown mode keeps the press.
inline Mode driven_mode(Mode pressed, const std::string &session_mode)
{
    if (pressed == Mode::none || session_mode.empty() || session_mode == "unknown")
        return pressed;
    if (session_mode == "time_trial")
        return Mode::time_trial;
    if (session_mode == "grand_prix")
        return Mode::grand_prix;
    return Mode::none;
}

enum class Back { stock, route, home, twostep };

inline Back back_variant_of(const std::string &v, bool data_in_build)
{
    if (v == "stock")
        return Back::stock;
    if (v == "twostep")
        return Back::twostep;
    if (v == "home")
        return Back::home;
    return data_in_build ? Back::route : Back::stock;
}

// The route the launcher fires in place of the game's own Back route (nullptr: the game's stays). The container's
// Back fires f1_world (to the hub) or home; the Showroom's fires back_1.
inline const char *back_replacement(Back variant, uint32_t group, const std::string &route)
{
    if (variant == Back::route || variant == Back::home) {
        if (group == djb2("f1_world_activity_container") && (route == "f1_world" || route == "home"))
            return variant == Back::home ? "home" : BACK_HOME_FROM_CONTAINER;
    }
    if (variant == Back::route && route == "back_1" && group == djb2("fe_showroom"))
        return HOME_BACK_ROUTE;
    return nullptr;
}

// The Customisation screen's Back after a Customisation card press: back (what the screen fires when it was opened
// from the main menu; it leads nowhere there), back_1 (to the F1 World hub) or back_2 (to the news) becomes the
// launcher's swofty_back_home in that screen's group, the container's home route.
inline const char *customisation_back(Back variant, bool from_card, uint32_t group, const std::string &route)
{
    if ((variant == Back::route || variant == Back::home) && from_card && group == djb2(CUSTOMISATION_GROUP) &&
        (route == "back" || route == "back_1" || route == "back_2"))
        return BACK_HOME_FROM_CONTAINER;
    return nullptr;
}

// A Customisation card press stays armed until the front-end flow is entered again: the customisation screens' help
// page (team_hq_flow) migrates back into the customisation flow.
inline bool customisation_disarms(const std::string &flow)
{
    return flow == FRONTEND_FLOW;
}

// The saves list's Back (route back in grand_prix_continue) stays the game's: it migrates to the activity container,
// and swapping it for a front-end route home left a black screen. After a Grand Prix card press the core then leaves
// the container for the main menu once its tabs exist, so one Back from the saves list returns home.
constexpr uint64_t SAVES_BACK_DELAY_MS = 400, SAVES_BACK_TIMEOUT_MS = 6000;

inline bool saves_back_leaves_home(Back variant, uint32_t group, const std::string &route, Mode mode)
{
    return variant == Back::route && mode == Mode::grand_prix && group == djb2("grand_prix_continue") && route == "back";
}

enum class Expect { none, container, saves, home };

enum class Recovery { none, back_home_route, open_home };

// What to do when the screen a transition asked for did not open in time. flow_now is the flow the last migration
// the game accepted asked for; a main menu wanted while another flow is current needs the migration back.
inline Recovery recovery_for(Expect e, const std::string &flow_now, bool home_screen_open)
{
    if (e == Expect::none || (e == Expect::home && home_screen_open))
        return Recovery::none;
    if (flow_now == F1_WORLD_FLOW || flow_now == CUSTOMISATION_FLOW)
        return Recovery::back_home_route;
    return home_screen_open ? Recovery::none : Recovery::open_home;
}

// Whether the F1 World entry point (g_Game_F1WorldEntryPoint) may be cleared when a Back leaves for the main menu. The
// global is only written through the address the running build's binding set resolved: an unresolved binding (a
// build whose scan found no match) skips the write, as does an address that is not writable memory. 1.9.0 wrote the
// Steam offset on every build and faulted on the EA build, where that offset is a read-only .sbss page.
enum class EntryPointWrite { write, unresolved, unwritable };

inline EntryPointWrite entry_point_write(uintptr_t resolved, bool writable)
{
    if (!resolved)
        return EntryPointWrite::unresolved;
    return writable ? EntryPointWrite::write : EntryPointWrite::unwritable;
}

inline bool migrate_failed(const std::string &text)
{
    return text.find("failed to migrate") != std::string::npos || text.find("No screen called") != std::string::npos;
}

// A Migrate command is logged when it fails or moves the tracked flow, not on every migrate.
inline bool migrate_logged(bool failed, const std::string &flow, const std::string &flow_before)
{
    return failed || (!flow.empty() && flow != flow_before);
}

// One VirtualQuery answer (state committed or not, protection, region base and size) says whether n bytes at p can be written.
// The protection values are the Windows PAGE_* constants.
inline bool region_writable(bool committed, uint32_t protect, uintptr_t base, uint64_t size, uintptr_t p, size_t n)
{
    constexpr uint32_t NOACCESS = 0x01, GUARD = 0x100, WRITABLE = 0x04 | 0x08 | 0x40 | 0x80;
    if (!p || !committed || (protect & (NOACCESS | GUARD)) || !(protect & WRITABLE))
        return false;
    return p >= base && p + n <= base + size;
}

// A session that ended after a Time Trial or Grand Prix card press hands over to the f1_world flow's container; the
// card's command is raised there once it is up.
inline bool return_due(bool was_in_session, bool in_session, Mode last, bool feature_on)
{
    return feature_on && was_in_session && !in_session && last != Mode::none;
}

}
