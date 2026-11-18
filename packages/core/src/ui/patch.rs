use swofty_formats::pkg::{Node, Obj, Val};

use super::node::*;
use super::pages::{HOME, HomePage, NEXT_PAGE, PREV_PAGE, STATE};
use crate::Result;

pub const ACTION: &str = "Menu Right Stick Click";
const ACTION_PREFIX: &str = "#input_data.action_sets.menu_actions.actions.";
pub const MOD_MENU: &str = "mod_menu";
pub const MOD_MENU_OFFLINE: &str = "mod_menu_offline";
pub const OFFLINE_TEXT: &str = "Launched with Swofty Launcher";
pub const OFFLINE_TEXT_PATH: &str = "#ui_menu_data.swofty_offline_text";
pub const OFFLINE_PROMPT: &str = "to open settings";
pub const PROMPT_LABEL: &str = "Addon Settings";
const PANEL: &str = "fe_home.contextual_panels.offline_contextual_panel";
const UIDM: &str = "persistent.consume_input_wrapper.persistent_multichoice_message";
const ONCE_ATTR: &str = "swofty_once";
const BUTTON: &str = "lng_continue";
pub const POPUP_TITLE: &str = "lng_swofty_popup_title";
pub const POPUP_BODY: &str = "lng_swofty_popup_body";
const ONLINE_MESSAGES: &[&str] = &[
    "online_services_generic_exception_online",
    "online_services_generic_exception_online_with_code",
    "online_services_generic_exception_offline",
    "online_services_generic_exception_offline_with_code",
];

pub fn popup_seen() -> String {
    format!("{STATE}.online_popup_seen")
}

pub struct Popup {
    pub title: String,
    pub body: String,
}

impl Popup {
    pub fn for_version(version: &str) -> Self {
        Self {
            title: format!("Swofty Mod v{version}"),
            body: format!(
                "Online services are off while mods are loaded, so multiplayer is unavailable.\n\n\
                 New in v{version}:\n\
                 - ADDON SETTINGS on the home screen (F9 / R3)\n\
                 - Addon settings page listing installed addons\n\
                 - Launcher panel on the home screen"
            ),
        }
    }

    pub fn strings(&self) -> Vec<(String, String)> {
        vec![
            (POPUP_TITLE.into(), self.title.clone()),
            (POPUP_BODY.into(), self.body.clone()),
        ]
    }
}

fn signal(to: &str, pin: &str, from: &str, attrs: &[(&str, &str)]) -> Node {
    let attrs: Vec<(&str, Attr)> = attrs.iter().map(|(k, v)| (*k, Attr::from(*v))).collect();
    sig_with(to, pin, from, &attrs)
}

fn sig_to(s: &Node) -> (&str, &str) {
    let o = s.as_obj();
    let get = |k| {
        o.and_then(|o| o.get(k))
            .and_then(Node::as_val)
            .and_then(Val::as_str)
            .unwrap_or_default()
    };
    (get("to_object_name"), get("to_pin_name"))
}

fn event_mut<'n>(n: &'n mut Node, name: &str) -> Result<&'n mut Vec<Node>> {
    let what = n.name().to_owned();
    n.slot_mut(EVENTS)?
        .iter_mut()
        .find(|e| e.o().ok().and_then(|o| o.str_at("eventName").ok()) == Some(name))
        .ok_or_else(|| err(format!("{what} has no {name} event")))?
        .o_mut()?
        .at_mut("signals")?
        .a_mut()
}

pub fn fe_home(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        if screen.name() != "fe_home" {
            return Err(err("the home screen file does not hold fe_home"));
        }
        let tooltips = screen.child_mut("tooltips")?;
        let mut tip = clone_node(tooltips.child("settings")?, "fe_home.tooltips", MOD_MENU)?;
        set_prop_value(&mut tip, "tooltipsName", Val::str(MOD_MENU))?;
        set_prop_value(tip.child_mut("text")?, "localise", Val::bool(false))?;
        tooltips.slot_mut(CHILDREN)?.push(tip);
        let mut listener = clone_node(screen.child("settings")?, "fe_home", "mod_menu_listener")?;
        set_prop_value(&mut listener, "action_name", Val::str(ACTION))?;
        let ev = listener
            .slot_mut(EVENTS)?
            .first_mut()
            .ok_or_else(|| err("the settings listener has no events"))?
            .o_mut()?;
        if ev.str_at("eventName")? != "pressed" {
            return Err(err("the settings listener does not start with pressed"));
        }
        *ev.at_mut("signals")?.a_mut()? = vec![
            signal("fe_home.Action", "route", "pressed", &[("key", MOD_MENU)]),
            signal("fe_home.audio_theme_trigger", "confirm", "pressed", &[]),
        ];
        fix_ids(&mut listener, Some("fe_home"));
        screen.slot_mut(CHILDREN)?.push(listener);
        Ok(())
    })
}

pub fn offline_panel(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        let cp = screen.child_mut("contextual_panels")?;
        let panel = cp.child_mut("offline_contextual_panel")?;
        let subtitle = panel.child_mut("subtitle")?;
        literal_text(subtitle, OFFLINE_TEXT)?;
        set_prop(subtitle, "text", prop(s(OFFLINE_TEXT), Some(OFFLINE_TEXT_PATH)))?;
        literal_text(panel.child_mut("tooltip_label")?, OFFLINE_PROMPT)?;
        set_prop_value(
            panel.child_mut("tooltip_extra_2")?,
            "tooltipsName",
            Val::str(MOD_MENU_OFFLINE),
        )?;
        let decision = cp.child_mut("contextual_decision")?;
        event_mut(decision, "offline")?
            .retain(|s| sig_to(s).0 != "fe_home.contextual_panels.extra_2_listener");
        event_mut(decision, "default")?.push(signal(PANEL, "show", "default", &[]));
        fix_ids(screen, None);
        Ok(())
    })
}

pub fn hide_panel(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        let dec = screen
            .child_mut("contextual_panels")?
            .child_mut("contextual_decision")?;
        for e in dec.slot_mut(EVENTS)? {
            e.o_mut()?.at_mut("signals")?.a_mut()?.retain(|s| {
                let (to, pin) = sig_to(s);
                !(to.ends_with("offline_contextual_panel") && pin == "show")
            });
        }
        Ok(())
    })
}

fn tooltip_dict(p: &mut swofty_formats::pkg::Pkg) -> Result<&mut Obj> {
    root_obj(p)?
        .at_mut("tool_tips")?
        .o_mut()?
        .at_mut("Contents")?
        .o_mut()?
        .at_mut("tooltips_dictionary")?
        .o_mut()?
        .at_mut("Contents")?
        .o_mut()
}

fn add_tooltip(
    data: &[u8],
    name: &str,
    src: &str,
    label: &str,
    action: Option<&str>,
    priority: Option<&str>,
) -> Result<Vec<u8>> {
    edit(data, |p| {
        let d = tooltip_dict(p)?;
        if d.get(name).is_some() {
            return Err(err(format!("the tooltips already have {name}")));
        }
        let mut e = d.at(src)?.clone();
        let eo = e.o_mut()?;
        set_str(eo, "Name", name)?;
        let c = eo.at_mut("Contents")?.o_mut()?;
        let mut set = |key: &str, v: &str| set_str(c.at_mut(key)?.o_mut()?, "Value", v);
        if let Some(action) = action {
            set("action_path", &format!("{ACTION_PREFIX}{action}"))?;
        }
        set("text", label)?;
        if let Some(priority) = priority {
            set("priority", priority)?;
        }
        d.set(name, e);
        Ok(())
    })
}

pub fn tooltips(data: &[u8], adjust: &str) -> Result<Vec<u8>> {
    let d = add_tooltip(
        data,
        MOD_MENU,
        "settings_fe_home",
        PROMPT_LABEL,
        Some(ACTION),
        Some("5"),
    )?;
    let d = add_tooltip(&d, MOD_MENU_OFFLINE, "delete", OFFLINE_PROMPT, Some(ACTION), None)?;
    add_tooltip(&d, adjust, "change_driver", "Adjust", None, None)
}

fn route_cmd(kind: &str, id: u32, attrs: &[(&str, &str)], deps: Option<&str>) -> Node {
    let mut a = vec![obj(vec![("Id", s("Id")), ("Value", s(&id.to_string()))])];
    a.extend(
        attrs
            .iter()
            .map(|(k, v)| obj(vec![("Id", s(k)), ("Value", s(v))])),
    );
    let mut items = vec![("Attributes", arr(a)), ("Type", s(kind))];
    if let Some(d) = deps {
        items.push(("Dependencies", s(d)));
    }
    obj(items)
}

fn route(id: &str, cmds: Vec<Node>, deps: &str, kind: i32, destination: Option<&str>) -> Node {
    let mut items = vec![("Id", s(id)), ("Commands", arr(cmds))];
    if let Some(dst) = destination {
        items.push((
            "Attributes",
            arr(vec![obj(vec![("Id", s("Destination")), ("Value", s(dst))])]),
        ));
    }
    items.push(("Type", i32v(kind)));
    items.push(("Dependencies", s(deps)));
    obj(items)
}

fn screen_routes(name: &str) -> Vec<Node> {
    let open = |n| route_cmd("OpenScreen", n, &[("Target", name)], None);
    vec![
        route(
            "open",
            vec![
                open(1),
                route_cmd("InputState", 2, &[], Some("1")),
                route_cmd(
                    "TransitionScreen",
                    3,
                    &[("Direction", "in"), ("Target", name)],
                    Some("1"),
                ),
            ],
            "2,3",
            0,
            None,
        ),
        route(
            "open_no_input",
            vec![
                open(1),
                route_cmd(
                    "TransitionScreen",
                    2,
                    &[("Direction", "in"), ("Target", name)],
                    Some("1"),
                ),
            ],
            "2",
            0,
            None,
        ),
        route(
            "close",
            vec![
                route_cmd(
                    "TransitionScreen",
                    1,
                    &[("Direction", "out"), ("Target", name)],
                    None,
                ),
                route_cmd("CloseScreen", 2, &[("Target", name)], Some("1")),
            ],
            "2",
            0,
            None,
        ),
    ]
}

fn subpage_route(id: &str, src: &str, dst: &str, kind: i32) -> Node {
    route(
        id,
        vec![
            route_cmd(
                "TriggerSubRoute",
                1,
                &[("SubRoute", "close"), ("Target", src), ("To", dst)],
                None,
            ),
            route_cmd(
                "TriggerSubRoute",
                2,
                &[
                    ("SubRoute", "open"),
                    ("Target", dst),
                    ("From", src),
                    ("Message", ""),
                ],
                Some("1"),
            ),
        ],
        "2",
        kind,
        Some(dst),
    )
}

fn migrate_route(id: &str, src: &str, flow: &str, dst: &str) -> Node {
    migrate_route_with(id, src, flow, dst, ("open_no_input", ""), 2)
}

fn migrate_route_with(id: &str, src: &str, flow: &str, dst: &str, open: (&str, &str), kind: i32) -> Node {
    let (sub, message) = open;
    route(
        id,
        vec![
            route_cmd(
                "TriggerSubRoute",
                1,
                &[("SubRoute", "close"), ("Target", src), ("To", dst)],
                None,
            ),
            route_cmd(
                "Migrate",
                2,
                &[("Flow", flow), ("SkipActivator", "true")],
                Some("1"),
            ),
            route_cmd(
                "TriggerSubRoute",
                3,
                &[
                    ("SubRoute", sub),
                    ("Target", dst),
                    ("From", src),
                    ("Message", message),
                ],
                Some("2"),
            ),
        ],
        "3",
        kind,
        Some(dst),
    )
}

fn route_group(name: &str, routes: Vec<Node>) -> Node {
    obj(vec![
        ("Id", s(name)),
        ("Routes", arr(routes)),
        (
            "Attributes",
            arr(vec![obj(vec![("Id", s("Source")), ("Value", s(name))])]),
        ),
    ])
}

/// ADDON SETTINGS' route groups: each home screen's, with a route to each of its `pages` (route id,
/// screen, home screen) and to the home screens before and after it, and each page's, whose `back`
/// opens its home screen again. `back` on the first home screen leaves for the game's home menu, and on
/// a later one opens the home screen before it.
pub fn route_groups(homes: &[HomePage], pages: &[(String, String, String)]) -> Vec<Node> {
    let mut groups = Vec::new();
    for h in homes {
        let mut r = screen_routes(&h.screen);
        r.push(match &h.prev {
            Some(prev) => subpage_route("back", &h.screen, prev, 2),
            None => migrate_route("back", &h.screen, "frontend_flow", "fe_home"),
        });
        if let Some(next) = &h.next {
            r.push(subpage_route(NEXT_PAGE, &h.screen, next, 1));
        }
        if let Some(prev) = &h.prev {
            r.push(subpage_route(PREV_PAGE, &h.screen, prev, 2));
        }
        r.extend(
            pages
                .iter()
                .filter(|(_, _, home)| home == &h.screen)
                .map(|(rid, s, _)| subpage_route(rid, &h.screen, s, 1)),
        );
        groups.push(route_group(&h.screen, r));
    }
    for (_, s, home) in pages {
        let mut r = screen_routes(s);
        r.push(subpage_route("back", s, home, 2));
        groups.push(route_group(s, r));
    }
    groups
}

/// Route groups for addon screens, as ADDON SETTINGS pages have: open, open_no_input and close,
/// and back, which closes the screen and opens its anchor again.
pub fn screen_route_groups(screens: &[(String, String)]) -> Vec<Node> {
    screens
        .iter()
        .map(|(s, anchor)| {
            let mut r = screen_routes(s);
            r.push(subpage_route("back", s, anchor, 2));
            route_group(s, r)
        })
        .collect()
}

/// Adds open_<name> to each anchor's route group in these routes, when they hold it: the route
/// closes the anchor and opens the addon screen, as ADDON SETTINGS opens its pages. Each entry is
/// (name, anchor, screen).
pub fn anchor_routes(data: &[u8], screens: &[(String, String, String)]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let gs = root_obj(p)?.at_mut("RouteGroupings")?.a_mut()?;
        for (name, anchor, s) in screens {
            let Some(g) = gs.iter_mut().find(|g| id_of(g) == Some(anchor)) else {
                continue;
            };
            let id = format!("open_{name}");
            let routes = g.o_mut()?.at_mut("Routes")?.a_mut()?;
            if !routes.iter().any(|r| id_of(r) == Some(&id)) {
                routes.push(subpage_route(&id, anchor, s, 1));
            }
        }
        Ok(())
    })
}

/// The Driver Career tile's route to the season length step, which a career setup's entry route copies.
pub const SETUP_ENTRY_SOURCE: &str = "career_wizard_driver_career_to_f1";
/// The message the career menu opens the career content select with for Driver Career.
pub const DRIVER_CAREER: &str = "driver_career";

pub struct SetupRoutes {
    pub route: String,
    pub home: String,
    pub rows: String,
    pub next: String,
    pub next_flow: String,
    pub back: String,
    pub back_flow: String,
    pub back_route: String,
}

/// The route groups of a career setup's home and rows page. The home's `confirm` closes it, moves to the
/// wizard's flow and opens `next`; its `back` moves to the content select's flow and opens it for Driver Career.
pub fn setup_route_groups(s: &SetupRoutes) -> Vec<Node> {
    let mut home = screen_routes(&s.home);
    let mut back = migrate_route_with("back", &s.home, &s.back_flow, &s.back, ("open", DRIVER_CAREER), 2);
    no_auto_focus(&mut back);
    home.push(back);
    home.push(migrate_route_with(
        super::pages::SETUP_CONFIRM,
        &s.home,
        &s.next_flow,
        &s.next,
        ("open", ""),
        1,
    ));
    home.push(subpage_route(super::pages::SETUP_OPTIONS, &s.home, &s.rows, 1));
    let mut rows = screen_routes(&s.rows);
    rows.push(subpage_route("back", &s.rows, &s.home, 2));
    vec![route_group(&s.home, home), route_group(&s.rows, rows)]
}

fn entry_route(group: &Node, s: &SetupRoutes) -> Result<Node> {
    let mut r = group
        .o()?
        .at("Routes")?
        .a()?
        .iter()
        .find(|r| id_of(r) == Some(SETUP_ENTRY_SOURCE))
        .cloned()
        .ok_or_else(|| err(format!("{} has no {SETUP_ENTRY_SOURCE} route", s.back)))?;
    let from = r
        .o()?
        .at("Attributes")?
        .a()?
        .iter()
        .filter_map(Node::as_obj)
        .find(|a| a.str_at("Id").ok() == Some("Destination"))
        .and_then(|a| a.str_at("Value").ok())
        .map(str::to_owned)
        .ok_or_else(|| err(format!("{SETUP_ENTRY_SOURCE} has no destination")))?;
    replace_strings(&mut r, &|v| match v {
        v if v == from => Some(s.home.clone()),
        "career_season_setup_flow" => Some("options_flow".into()),
        "open_no_input" => Some("open".into()),
        _ => None,
    });
    set_str(r.o_mut()?, "Id", &s.route)?;
    let cmds = r.o_mut()?.at_mut("Commands")?.a_mut()?;
    let id = cmds
        .iter()
        .filter_map(|c| cmd_attr(c, "Id")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let wiped = cmds
        .iter()
        .find(|c| cmd_type(c) == Some("TriggerSubRoute") && cmd_attr(c, "SubRoute") == Some("close"))
        .and_then(|c| c.as_obj()?.get("Dependencies")?.as_val()?.as_str())
        .map(str::to_owned);
    cmds.push(route_cmd(
        "FrontendSceneSwitch",
        id,
        &SETUP_SCENE,
        wiped.as_deref(),
    ));
    Ok(r)
}

/// The stock CUSTOM settings page's backdrop, which every route into a career setup page switches to.
const SETUP_SCENE: [(&str, &str); 4] = [
    ("Target", ""),
    ("SceneOverride", "Customisation"),
    ("SubSceneOverride", "Generic"),
    ("CameraOverride", "Main"),
];

fn cmd_type(c: &Node) -> Option<&str> {
    c.as_obj()?.get("Type")?.as_val()?.as_str()
}

fn cmd_attr<'a>(c: &'a Node, key: &str) -> Option<&'a str> {
    c.as_obj()?
        .get("Attributes")?
        .as_arr()?
        .items
        .iter()
        .filter_map(Node::as_obj)
        .find(|a| a.str_at("Id").ok() == Some(key))?
        .str_at("Value")
        .ok()
}

fn no_auto_focus(r: &mut Node) {
    if let Some(a) = r
        .as_obj_mut()
        .and_then(|o| o.get_mut("Attributes"))
        .and_then(Node::as_arr_mut)
    {
        a.items
            .push(obj(vec![("Id", s("AutoFocus")), ("Value", s("false"))]));
    }
}

/// The way back from the wizard step `next` to the home, as the wizard's own routes leave its flow: `next` and the
/// slices its group opens transition out, the slices close, and the flow moves to the options flow.
fn wizard_exit_route(s: &SetupRoutes, slices: &[&str]) -> Node {
    let out = |id: u32, target: &str| {
        route_cmd(
            "TransitionScreen",
            id,
            &[("Direction", "out"), ("Target", target)],
            None,
        )
    };
    let mut cmds = vec![out(1, &s.next)];
    let mut done = vec!["1".to_owned()];
    let mut id = 1;
    for slice in slices {
        cmds.push(out(id + 1, slice));
        cmds.push(route_cmd(
            "CloseScreen",
            id + 2,
            &[("Target", slice)],
            Some(&(id + 1).to_string()),
        ));
        done.push((id + 2).to_string());
        id += 2;
    }
    let migrate = (id + 1).to_string();
    cmds.push(route_cmd(
        "Migrate",
        id + 1,
        &[("Flow", "options_flow"), ("SkipActivator", "true")],
        Some(&done.join(",")),
    ));
    let open = (id + 2).to_string();
    cmds.push(route_cmd(
        "TriggerSubRoute",
        id + 2,
        &[
            ("SubRoute", "open"),
            ("Target", &s.home),
            ("From", &s.next),
            ("Message", ""),
        ],
        Some(&migrate),
    ));
    cmds.push(route_cmd(
        "FrontendSceneSwitch",
        id + 3,
        &SETUP_SCENE,
        Some(&migrate),
    ));
    let mut r = route(&s.back_route, cmds, &open, 2, Some(&s.home));
    no_auto_focus(&mut r);
    r
}

fn slices_of(g: &Node) -> Vec<String> {
    g.as_obj()
        .and_then(|o| o.get("Attributes"))
        .and_then(Node::as_arr)
        .into_iter()
        .flat_map(|a| &a.items)
        .filter_map(Node::as_obj)
        .filter(|a| a.str_at("Id").ok() == Some("Slices"))
        .filter_map(|a| a.str_at("Value").ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}

fn add_route(g: &mut Node, r: Node) -> Result<()> {
    let gid = id_of(g).unwrap_or_default().to_owned();
    let rid = id_of(&r).unwrap_or_default().to_owned();
    let routes = g.o_mut()?.at_mut("Routes")?.a_mut()?;
    if routes.iter().any(|x| id_of(x) == Some(&rid)) {
        return Err(err(format!("{gid} has a route {rid} already")));
    }
    routes.push(r);
    Ok(())
}

#[derive(Debug, Default)]
pub struct Placed {
    /// Each route added, as `<group>.<route>`.
    pub routes: Vec<String>,
    /// The game screens the added routes name: the slices of each `next` screen's group.
    pub screens: Vec<String>,
}

/// Adds each career setup's entry route to the content select's route group and its back route to the group of
/// its `next` screen, for the groups these routes hold.
pub fn setup_routes(data: &[u8], setups: &[SetupRoutes], placed: &mut Placed) -> Result<Vec<u8>> {
    edit(data, |p| {
        let gs = root_obj(p)?.at_mut("RouteGroupings")?.a_mut()?;
        for s in setups {
            if let Some(g) = gs.iter_mut().find(|g| id_of(g) == Some(&s.back)) {
                let r = entry_route(g, s)?;
                add_route(g, r)?;
                placed.routes.push(format!("{}.{}", s.back, s.route));
            }
            if let Some(g) = gs.iter_mut().find(|g| id_of(g) == Some(&s.next)) {
                let slices = slices_of(g);
                let names: Vec<&str> = slices.iter().map(String::as_str).collect();
                add_route(g, wizard_exit_route(s, &names))?;
                placed.routes.push(format!("{}.{}", s.next, s.back_route));
                placed.screens.extend(slices);
            }
        }
        Ok(())
    })
}

fn id_of(n: &Node) -> Option<&str> {
    n.as_obj()?.get("Id")?.as_val()?.as_str()
}

fn mod_menu_route(groupings: &[Node]) -> Result<Node> {
    let home = groupings
        .iter()
        .find(|g| id_of(g) == Some("fe_home"))
        .ok_or_else(|| err("the routes have no fe_home grouping"))?;
    let mut r = home
        .o()?
        .at("Routes")?
        .a()?
        .iter()
        .find(|r| id_of(r) == Some("options"))
        .cloned()
        .ok_or_else(|| err("fe_home has no options route"))?;
    set_str(r.o_mut()?, "Id", MOD_MENU)?;
    replace_strings(&mut r, &|v| match v {
        "settings_container" => Some(HOME.into()),
        "open_no_input" => Some("open".into()),
        _ => None,
    });
    Ok(r)
}

pub const SHOWROOM_ROUTE: &str = "swofty_showroom";
pub const CUSTOMISATION_ROUTE: &str = "swofty_customisation";
pub const CUSTOMISATION_GROUP: &str = "customisation_f1_life_entry";
const CUSTOMISATION_FLOW: &str = "customisation_flow";
const CUSTOMISATION_ENTRY: &str = "enter_f1_life_customisation";
pub const HOME_BACK_ROUTE: &str = "swofty_home";
pub const BACK_HOME_FROM_CONTAINER: &str = "swofty_back_home";
pub const TIME_TRIAL_ROUTE: &str = "swofty_time_trial";
pub const GRAND_PRIX_ROUTE: &str = "swofty_grand_prix";
pub const GRAND_PRIX_SAVES_ROUTE: &str = "swofty_grand_prix_saves";
pub const TIME_TRIAL_ENTRY: &str = "swofty_enter_time_trial";
pub const GRAND_PRIX_ENTRY: &str = "swofty_enter_grand_prix";
pub const RETURN_ENTRY: &str = "swofty_enter_return";
const ACTIVITY_ENTRY: &str = "enter_f1_world_activity";
const SPLITSCREEN_ENTRY: &str = "enter_splitscreen_era_select";
const F1_WORLD_FLOW: &str = "f1_world_flow";
const THEMING_OVERLAY: &str = "theming_overlay_frontend";
const TINY_WIPE: &str = "ui_wipes/ui_wipe_tiny_right.bk2";
const TINY_WIPE_CURVE: &str = "tran_wipe_tiny_right_dly_c";
const STANDARD_WIPE: &str = "ui_wipes/ui_wipe_standard_out.bk2";
const STANDARD_WIPE_CURVE: &str = "tran_wipe_stndrd_out_dly_c";
const TRANSITION_ENTRY_NODE: &str = "migrate_to_f1_world";
const F1_WORLD_HUB: &str = "fe_f1_world";
const SHOWROOM_SCREEN: &str = "fe_showroom";

/// routes_f1_world.uir with the routes that leave the pickers for the main menu without the F1 World hub:
/// `swofty_back_home` in the activity container's group, a copy of the container's own `home` route (a wipe,
/// `Migrate frontend_flow enter_fe_home`, then fe_home with its slices and scene) that runs the standard
/// small wipe of its own and opens the theming overlay only after the migration, so nothing of the old screen or the hub
/// shows before the wipe covers it; `swofty_home` in the fe_showroom and grand_prix_continue groups, a copy of the
/// hub's `back_1` (a wipe, then close the screen and open fe_home; both screens are in the front-end flow, so no
/// migration is needed). The core swaps them in where the game's own handlers fire their Back route.
pub fn f1_world_routes(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let gs = root_obj(p)?.at_mut("RouteGroupings")?.a_mut()?;
        let back = group_route(gs, F1_WORLD_HUB, "back_1")?;
        let home = container_home_route(&group_route(gs, "f1_world_activity_container", "home")?)?;
        let mut added = vec![("f1_world_activity_container", home)];
        for group in ["fe_showroom", "grand_prix_continue"] {
            let mut r = back.clone();
            set_str(r.o_mut()?, "Id", HOME_BACK_ROUTE)?;
            replace_strings(&mut r, &|v| match v {
                "fe_f1_world" => Some(group.to_owned()),
                STANDARD_WIPE => Some(TINY_WIPE.to_owned()),
                STANDARD_WIPE_CURVE => Some(TINY_WIPE_CURVE.to_owned()),
                _ => None,
            });
            added.push((group, r));
        }
        for (group, r) in added {
            let id = id_of(&r).unwrap_or_default().to_owned();
            let target = gs
                .iter_mut()
                .find(|g| id_of(g) == Some(group))
                .ok_or_else(|| err(format!("the routes have no {group} grouping")))?;
            let routes = target.o_mut()?.at_mut("Routes")?.a_mut()?;
            routes.retain(|x| id_of(x) != Some(&id));
            routes.push(r);
        }
        Ok(())
    })
}

fn attr_mut<'a>(cmd: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    cmd.as_obj_mut()?
        .get_mut("Attributes")?
        .as_arr_mut()?
        .items
        .iter_mut()
        .find(|a| a.as_obj().and_then(|o| o.str_at("Id").ok()) == Some(id))
}

fn attr_value(cmd: &Node, id: &str) -> Option<String> {
    cmd.as_obj()?
        .get("Attributes")?
        .as_arr()?
        .items
        .iter()
        .filter_map(Node::as_obj)
        .find(|a| a.str_at("Id").ok() == Some(id))?
        .str_at("Value")
        .ok()
        .map(str::to_owned)
}

fn cmd_id(cmd: &Node) -> Option<String> {
    attr_value(cmd, "Id")
}

fn container_home_route(stock: &Node) -> Result<Node> {
    let mut r = stock.clone();
    set_str(r.o_mut()?, "Id", BACK_HOME_FROM_CONTAINER)?;
    let cmds = r.o_mut()?.at_mut("Commands")?.a_mut()?;
    let migrate = cmds
        .iter()
        .find(|c| cmd_type(c) == Some("Migrate") && attr_value(c, "Flow").as_deref() == Some("frontend_flow"))
        .and_then(cmd_id)
        .ok_or_else(|| err("the container's home route has no Migrate to frontend_flow"))?;
    let mut wiped = 0;
    for c in cmds.iter_mut() {
        if attr_mut(c, "Video").is_some() {
            wiped += 1;
        }
        if let Some(v) = attr_mut(c, "DilationCurve") {
            let curve = v.o()?.str_at("Value")?.to_owned();
            if curve.starts_with("tran_wipe_") && curve.ends_with("_dly_c") {
                wiped += 1;
            }
        }
        if cmd_type(c) == Some("TriggerSubRoute")
            && attr_value(c, "Target").as_deref() == Some(THEMING_OVERLAY)
            && c.as_obj().and_then(|o| o.get("Dependencies")).is_none()
        {
            c.o_mut()?.set("Dependencies", s(&migrate));
        }
    }
    if wiped != 2 {
        return Err(err(format!(
            "the container's home route has {wiped} wipe settings, not the stock 2"
        )));
    }
    Ok(r)
}

/// The main menu's Showroom route: a copy of the F1 World hub's own `showroom` (routes_f1_world.uir) with the hub
/// swapped for the home screen. It closes only the home screen, so `slice_bg_content_underlay` and the player card
/// stay up for the series picks that close them, runs fe_showroom's view model callback before input, and closes the
/// theming overlay itself.
fn showroom_route(f1_world: &[Node]) -> Result<Node> {
    let mut r = group_route(f1_world, F1_WORLD_HUB, "showroom")?;
    set_str(r.o_mut()?, "Id", SHOWROOM_ROUTE)?;
    replace_strings(&mut r, &|v| (v == F1_WORLD_HUB).then(|| "fe_home".to_owned()));
    let cmds = r.o()?.at("Commands")?.a()?;
    let has = |kind: &str, target: &str| {
        cmds.iter()
            .any(|c| cmd_type(c) == Some(kind) && attr_value(c, "Target").as_deref() == Some(target))
    };
    if !has("CloseScreen", "fe_home")
        || !has("OpenScreen", SHOWROOM_SCREEN)
        || !has("ViewModelCallback", SHOWROOM_SCREEN)
    {
        return Err(err(format!(
            "{F1_WORLD_HUB}'s showroom route does not close the hub and open {SHOWROOM_SCREEN} with its view model callback"
        )));
    }
    Ok(r)
}

fn group_route(groupings: &[Node], group: &str, id: &str) -> Result<Node> {
    groupings
        .iter()
        .find(|g| id_of(g) == Some(group))
        .ok_or_else(|| err(format!("the routes have no {group} grouping")))?
        .o()?
        .at("Routes")?
        .a()?
        .iter()
        .find(|r| id_of(r) == Some(id))
        .cloned()
        .ok_or_else(|| err(format!("{group} has no {id} route")))
}

/// The main menu's Customisation route: a copy of fe_home's own `customisation` (a wipe, close the home screen and
/// its slices, then `Migrate customisation_flow enter_f1_life_customisation`). F1 World's own `customisation`
/// closes the hub instead.
fn customisation_route(groupings: &[Node]) -> Result<Node> {
    let mut r = home_route(groupings, "customisation")?;
    let cmds = r.o()?.at("Commands")?.a()?;
    let enters = cmds.iter().any(|c| {
        cmd_type(c) == Some("Migrate")
            && attr_value(c, "Flow").as_deref() == Some(CUSTOMISATION_FLOW)
            && attr_value(c, "Activator").as_deref() == Some(CUSTOMISATION_ENTRY)
    });
    let closes = cmds
        .iter()
        .any(|c| cmd_type(c) == Some("CloseScreen") && attr_value(c, "Target").as_deref() == Some("fe_home"));
    if !enters || !closes {
        return Err(err(format!(
            "fe_home's customisation route does not close fe_home and enter {CUSTOMISATION_FLOW} at {CUSTOMISATION_ENTRY}"
        )));
    }
    set_str(r.o_mut()?, "Id", CUSTOMISATION_ROUTE)?;
    Ok(r)
}

/// routes_customisation_and_store.uir with `swofty_back_home` in the Customisation screen's group, the activity
/// container's `home` route from `f1_world` (routes_f1_world.uir) as `f1_world_routes` adds it: a wipe, `Migrate
/// frontend_flow enter_fe_home`, then fe_home with its slices and scene. The game's `back_1` there migrates to
/// the F1 World hub; the core swaps it for this route after a Customisation card press.
pub fn customisation_routes(data: &[u8], f1_world: &[u8]) -> Result<Vec<u8>> {
    let stock = swofty_formats::pkg::Pkg::parse(f1_world)?;
    let home = stock
        .root
        .o()?
        .at("RouteGroupings")?
        .a()?
        .iter()
        .find(|g| id_of(g) == Some("f1_world_activity_container"))
        .ok_or_else(|| err("the routes have no f1_world_activity_container grouping"))?
        .o()?
        .at("Routes")?
        .a()?
        .iter()
        .find(|r| id_of(r) == Some("home"))
        .cloned()
        .ok_or_else(|| err("f1_world_activity_container has no home route"))?;
    let home = container_home_route(&home)?;
    edit(data, |p| {
        let routes = root_obj(p)?
            .at_mut("RouteGroupings")?
            .a_mut()?
            .iter_mut()
            .find(|g| id_of(g) == Some(CUSTOMISATION_GROUP))
            .ok_or_else(|| err(format!("the routes have no {CUSTOMISATION_GROUP} grouping")))?
            .o_mut()?
            .at_mut("Routes")?
            .a_mut()?;
        if !routes.iter().any(|r| id_of(r) == Some("back_1")) {
            return Err(err(format!("{CUSTOMISATION_GROUP} has no back_1 route")));
        }
        routes.retain(|r| id_of(r) != Some(BACK_HOME_FROM_CONTAINER));
        routes.push(home);
        Ok(())
    })
}

pub fn has_route(data: &[u8], group: &str, id: &str) -> bool {
    let Ok(p) = swofty_formats::pkg::Pkg::parse(data) else {
        return false;
    };
    p.root
        .o()
        .and_then(|o| o.at("RouteGroupings"))
        .and_then(|g| g.a())
        .is_ok_and(|gs| {
            gs.iter().filter(|g| id_of(g) == Some(group)).any(|g| {
                g.o()
                    .and_then(|o| o.at("Routes"))
                    .and_then(|r| r.a())
                    .is_ok_and(|rs| rs.iter().any(|r| id_of(r) == Some(id)))
            })
        })
}

fn home_route(groupings: &[Node], id: &str) -> Result<Node> {
    group_route(groupings, "fe_home", id)
}

/// The main menu's Time Trial and Grand Prix routes: the game's own `home_to_activity` (wipe, close the home
/// screen and its slices, `Migrate f1_world_flow enter_f1_world_activity`) with the entry swapped for a launcher
/// entry that carries the picker's command, and a copy of `theatre` that opens the Grand Prix saves list in the
/// front-end flow itself.
fn nav_routes(groupings: &[Node]) -> Result<Vec<Node>> {
    let activity = home_route(groupings, "home_to_activity")?;
    let mut out = Vec::new();
    for (id, entry) in [
        (TIME_TRIAL_ROUTE, TIME_TRIAL_ENTRY),
        (GRAND_PRIX_ROUTE, GRAND_PRIX_ENTRY),
    ] {
        let mut r = activity.clone();
        set_str(r.o_mut()?, "Id", id)?;
        let swapped = std::cell::Cell::new(0);
        replace_strings(&mut r, &|v| {
            (v == ACTIVITY_ENTRY).then(|| {
                swapped.set(swapped.get() + 1);
                entry.to_owned()
            })
        });
        if swapped.get() == 0 {
            return Err(err("home_to_activity does not name enter_f1_world_activity"));
        }
        out.push(r);
    }
    let mut saves = home_route(groupings, "theatre")?;
    set_str(saves.o_mut()?, "Id", GRAND_PRIX_SAVES_ROUTE)?;
    replace_strings(&mut saves, &|v| {
        (v == "theatre_container").then(|| "grand_prix_continue".to_owned())
    });
    out.push(saves);
    Ok(out)
}

/// Adds ADDON SETTINGS' route groups, which must be new, and each of `optional` whose group the
/// routes don't have yet. `f1_world` is the stock routes_f1_world.uir, the source of the main menu's Showroom route.
pub fn routes(data: &[u8], groups: Vec<Node>, optional: Vec<Node>, f1_world: &[u8]) -> Result<Vec<u8>> {
    let showroom = swofty_formats::pkg::Pkg::parse(f1_world)
        .ok()
        .and_then(|p| showroom_route(p.root.o().ok()?.at("RouteGroupings").ok()?.a().ok()?).ok());
    edit(data, |p| {
        let gs = root_obj(p)?.at_mut("RouteGroupings")?.a_mut()?;
        let replace = mod_menu_route(gs)?;
        let customisation = customisation_route(gs).ok();
        let nav = nav_routes(gs).ok();
        for g in optional {
            if !gs.iter().any(|x| id_of(x) == id_of(&g)) {
                gs.push(g);
            }
        }
        for g in groups {
            if gs.iter().any(|x| id_of(x) == id_of(&g)) {
                return Err(err(format!(
                    "route group {} exists already",
                    id_of(&g).unwrap_or_default()
                )));
            }
            gs.push(g);
        }
        let rid = id_of(&replace).unwrap_or_default().to_owned();
        let grp = gs
            .iter_mut()
            .find(|x| id_of(x) == Some("fe_home"))
            .ok_or_else(|| err("the routes have no fe_home grouping"))?;
        let routes = grp.o_mut()?.at_mut("Routes")?.a_mut()?;
        routes.retain(|r| id_of(r) != Some(&rid));
        routes.push(replace);
        if let Some(showroom) = showroom {
            routes.retain(|r| id_of(r) != Some(SHOWROOM_ROUTE));
            routes.push(showroom);
        }
        for r in nav.into_iter().flatten().chain(customisation) {
            let id = id_of(&r).unwrap_or_default().to_owned();
            routes.retain(|x| id_of(x) != Some(&id));
            routes.push(r);
        }
        Ok(())
    })
}

pub fn flow_entries(data: &[u8], flow: &str, screens: &[String]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let flows = root_obj(p)?.at_mut("Flows")?.a_mut()?;
        let f = flows
            .iter_mut()
            .find(|f| f.name() == flow)
            .ok_or_else(|| err(format!("flows.ui has no {flow}")))?;
        let ents = f
            .slot_mut(CHILDREN)?
            .first_mut()
            .ok_or_else(|| err(format!("{flow} has no scenes")))?
            .slot_mut(CHILDREN)?;
        let tmpl = ents
            .first()
            .cloned()
            .ok_or_else(|| err(format!("{flow} has no entries")))?;
        for screen in screens {
            let entry = format!("enter_{screen}");
            if ents.iter().any(|e| e.name() == entry) {
                return Err(err(format!("{flow} has {entry} already")));
            }
            let mut e = tmpl.clone();
            let p0 = e.props_mut()?;
            set_str(p0, "name", &entry)?;
            set_str(p0.at_mut("screen")?.o_mut()?, "value", screen)?;
            p0.drop_key("command");
            ents.push(e);
        }
        fix_ids(f, None);
        Ok(())
    })
}

/// The f1_world flow's launcher entries: the activity container opened with the Time Trial picker's command
/// (`time_trial`) or the new Grand Prix picker's (`grand_prix_new`) already raised, the way the game's own
/// `enter_splitscreen_era_select` opens it with `ss_game_mode_select`, and the bare activity container
/// (`swofty_enter_return`) the session-end hand-over lands on.
pub fn f1_world_entries(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let flows = root_obj(p)?.at_mut("Flows")?.a_mut()?;
        let f = flows
            .iter_mut()
            .find(|f| f.name() == F1_WORLD_FLOW)
            .ok_or_else(|| err(format!("flows.ui has no {F1_WORLD_FLOW}")))?;
        let ents = f
            .slot_mut(CHILDREN)?
            .first_mut()
            .ok_or_else(|| err(format!("{F1_WORLD_FLOW} has no scenes")))?
            .slot_mut(CHILDREN)?;
        let stock = |ents: &[Node], name: &str| {
            ents.iter()
                .find(|e| e.name() == name)
                .cloned()
                .ok_or_else(|| err(format!("{F1_WORLD_FLOW} has no {name}")))
        };
        let plain = stock(ents, ACTIVITY_ENTRY)?;
        let with_command = stock(ents, SPLITSCREEN_ENTRY)?;
        for (name, command) in [
            (TIME_TRIAL_ENTRY, Some("time_trial")),
            (GRAND_PRIX_ENTRY, Some("grand_prix_new")),
            (RETURN_ENTRY, None),
        ] {
            if ents.iter().any(|e| e.name() == name) {
                return Err(err(format!("{F1_WORLD_FLOW} has {name} already")));
            }
            let mut e = if command.is_some() {
                with_command.clone()
            } else {
                plain.clone()
            };
            let p0 = e.props_mut()?;
            set_str(p0, "name", name)?;
            if let Some(c) = command {
                set_str(p0.at_mut("command")?.o_mut()?, "value", c)?;
            }
            ents.push(e);
        }
        fix_ids(f, None);
        Ok(())
    })
}

fn named_mut<'a>(n: &'a mut Node, name: &str) -> Option<&'a mut Node> {
    if n.is_node() && n.name() == name {
        return Some(n);
    }
    n.as_arr_mut()?
        .items
        .iter_mut()
        .filter_map(Node::as_arr_mut)
        .flat_map(|a| a.items.iter_mut())
        .find_map(|c| named_mut(c, name))
}

/// frontend_transition.ui with the hand-over for a session that ended in F1 World landing on the bare activity
/// container (`swofty_enter_return`) instead of the F1 World hub (`enter_f1_world_container`).
pub fn frontend_transition(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        let n = named_mut(screen, TRANSITION_ENTRY_NODE)
            .ok_or_else(|| err(format!("frontend_transition has no {TRANSITION_ENTRY_NODE} node")))?;
        let entry = n.props_mut()?.at_mut("migrate_entry")?.o_mut()?;
        if entry.str_at("value")? != "enter_f1_world_container" {
            return Err(err(format!(
                "{TRANSITION_ENTRY_NODE} does not enter enter_f1_world_container"
            )));
        }
        set_str(entry, "value", RETURN_ENTRY)
    })
}

pub fn regions(data: &[u8], screens: &[String]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let regs = root_obj(p)?
            .at_mut("regions")?
            .o_mut()?
            .at_mut("Contents")?
            .a_mut()?;
        let reg = regs
            .iter_mut()
            .find(|r| r.o().ok().and_then(|o| o.str_at("Name").ok()) == Some("region_undefined"))
            .ok_or_else(|| err("region_loading has no region_undefined"))?;
        let c = reg.o_mut()?.at_mut("Contents")?.o_mut()?;
        for screen in screens {
            if c.get(screen).is_some() {
                return Err(err(format!("region_undefined has {screen} already")));
            }
            let mut e = c.at("fe_home")?.clone();
            let eo = e.o_mut()?;
            set_str(eo, "Name", screen)?;
            set_str(
                eo,
                "Value",
                &format!("asset_groups/ui_package/screens/{screen}.assetgroup"),
            )?;
            c.set(screen, e);
        }
        Ok(())
    })
}

pub fn ui_data(data: &[u8], entries: Vec<(String, Node)>) -> Result<Vec<u8>> {
    edit(data, |p| {
        let c = root_obj(p)?
            .at_mut("ui_menu_data")?
            .o_mut()?
            .at_mut("Contents")?
            .o_mut()?;
        if c.get("swofty").is_some() {
            return Err(err("ui_menu_data has swofty already"));
        }
        c.set("swofty", uidb_dict("swofty", entries));
        Ok(())
    })
}

pub const FRIENDS_TAB_LABEL: &str = "lng_friends_leaderboard";
pub const FRIENDS_TAB_TEXT: &str = " ";

/// leaderboard_container.ui without its two shoulder-button glyphs, and with the tab controller's input off so the
/// bumpers (and the stick) no longer switch tabs. The tab buttons themselves are built by the game into the shared nav
/// scene from the screen model's view type list, which no data edit reaches.
pub fn leaderboard_glyphs(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        for name in ["tooltip_tab_left", "tooltip_tab_right"] {
            let n = named_mut(screen, name)
                .ok_or_else(|| err(format!("leaderboard_container has no {name} node")))?;
            if n.type_code() != "UITL" {
                return Err(err(format!("{name} is not a tooltip element")));
            }
            set_bool(n, "visible", false)?;
        }
        let tc = named_mut(screen, "tab_controller")
            .ok_or_else(|| err("leaderboard_container has no tab_controller node"))?;
        if tc.type_code() != "UITC" {
            return Err(err("tab_controller is not a tab controller"));
        }
        set_bool(tc, "inputAllowed", false)?;
        Ok(())
    })
}

pub const MESSAGE_ID: &str = "swofty_message";
const MESSAGE_TEMPLATE: &str = "online_services_EA_not_logged_in";
const ATTRIBUTE_ROW: &str = "#ui_messages.default_queue.multichoice_message.user_attributes";

fn attribute_token(attribute: &str) -> Node {
    obj(vec![(
        "VALUE",
        obj(vec![
            ("localise", boolv(false)),
            ("value", s("")),
            ("name", s("VALUE")),
            ("dbEntry", s(&format!("{ATTRIBUTE_ROW}[{attribute}].Value"))),
        ]),
    )])
}

fn attribute(name: &str) -> Node {
    obj(vec![
        ("localise", boolv(false)),
        ("value", s("")),
        ("name", s(name)),
        ("dbEntry", s("")),
    ])
}

fn by_message_id<'n>(list: &'n [Node], id: &str) -> Option<&'n Node> {
    list.iter()
        .find(|m| m.as_obj().and_then(|o| o.str_at("id").ok()) == Some(id))
}

fn runtime_message(list: &[Node]) -> Result<Option<Node>> {
    if by_message_id(list, MESSAGE_ID).is_some() {
        return Ok(None);
    }
    let Some(template) = by_message_id(list, MESSAGE_TEMPLATE) else {
        return Ok(None);
    };
    let mut m = template.clone();
    let o = m.o_mut()?;
    set_str(o, "id", MESSAGE_ID)?;
    set_str(o, "title", "lng_value_tag")?;
    set_str(o, "messageBody", "lng_value_tag")?;
    o.set("titleTokens", attribute_token("title"));
    o.set("bodyTokens", attribute_token("message"));
    *o.at_mut("attributes")?.a_mut()? = vec![attribute("title"), attribute("message")];
    *o.at_mut("options")?.a_mut()? = vec![obj(vec![
        ("value", s("lng_msg_ok")),
        ("response", s("ok")),
        ("defaultSelection", boolv(true)),
    ])];
    Ok(Some(m))
}

pub fn messages(data: &[u8], text: bool) -> Result<Vec<u8>> {
    edit(data, |p| {
        let list = root_obj(p)?.at_mut("messages")?.a_mut()?;
        if let Some(added) = runtime_message(list)? {
            list.push(added);
        }
        let mut done = Vec::new();
        for m in root_obj(p)?.at_mut("messages")?.a_mut()? {
            let m = m.o_mut()?;
            let id = m.str_at("id")?.to_owned();
            if !ONLINE_MESSAGES.contains(&id.as_str()) {
                continue;
            }
            if text {
                set_str(m, "title", POPUP_TITLE)?;
                set_str(m, "messageBody", POPUP_BODY)?;
                m.drop_key("titleTokens");
                m.drop_key("bodyTokens");
            }
            *m.at_mut("options")?.a_mut()? = vec![obj(vec![
                ("value", s(BUTTON)),
                ("response", s("cancel")),
                ("defaultSelection", boolv(true)),
            ])];
            m.at_mut("attributes")?.a_mut()?.push(obj(vec![
                ("localise", boolv(false)),
                ("value", s("true")),
                ("name", s(ONCE_ATTR)),
                ("dbEntry", s("")),
            ]));
            done.push(id);
        }
        done.sort();
        let mut want: Vec<&str> = ONLINE_MESSAGES.to_vec();
        want.sort();
        if done != want {
            return Err(err(format!(
                "the online services messages changed: found {done:?}"
            )));
        }
        Ok(())
    })
}

pub const TOAST_PREFIX: &str = "swofty_toast_";
pub const TOAST_SECONDS: &[u32] = &[2, 3, 4, 5, 6, 8, 10, 12, 15, 20, 30];
const TOAST_TEMPLATE: &str = "teammate_entering_pit";
const TOAST_FLAG: &str = "#overrides.osd.race_flags.generic";

fn toast_attribute(name: &str, value: Node) -> Node {
    obj(vec![
        ("localise", boolv(false)),
        ("value", value),
        ("name", s(name)),
        ("dbEntry", s("")),
    ])
}

pub fn hud_messages(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let list = root_obj(p)?.at_mut("messages")?.a_mut()?;
        let Some(template) = by_message_id(list, TOAST_TEMPLATE).cloned() else {
            return Err(err(format!("the HUD messages have no {TOAST_TEMPLATE}")));
        };
        for &n in TOAST_SECONDS {
            let id = format!("{TOAST_PREFIX}{n}");
            if by_message_id(list, &id).is_some() {
                continue;
            }
            let mut m = template.clone();
            let o = m.o_mut()?;
            set_str(o, "id", &id)?;
            *o.at_mut("attributes")?.a_mut()? = vec![
                toast_attribute("message", s("")),
                toast_attribute("sub_message", s("")),
                toast_attribute("msg_duration", f32v(n as f32)),
                toast_attribute("flag", s(TOAST_FLAG)),
                toast_attribute("enabled_in_spectator", boolv(true)),
            ];
            list.push(m);
        }
        Ok(())
    })
}

fn multichoice(p: &mut swofty_formats::pkg::Pkg) -> Result<&mut Node> {
    let scr = first_screen(p)?;
    if scr.name() != "persistent" {
        return Err(err("the persistent screen file does not hold persistent"));
    }
    scr.child_mut("consume_input_wrapper")?
        .child_mut("persistent_multichoice_message")
}

fn relabel(sigs: &[Node], from: &str) -> Result<Vec<Node>> {
    let mut out = sigs.to_vec();
    for s in &mut out {
        set_str(s.o_mut()?, "from_pin_name", from)?;
    }
    Ok(out)
}

pub fn persistent_gate(data: &[u8]) -> Result<Vec<u8>> {
    let seen = popup_seen();
    edit(data, |p| {
        let m = multichoice(p)?;
        let act = event_mut(m, "activate_message")?;
        let names: Vec<(String, String)> = act
            .iter()
            .map(|s| {
                let (a, b) = sig_to(s);
                (a.to_owned(), b.to_owned())
            })
            .collect();
        let want = [
            (
                "persistent.consume_input_wrapper".to_owned(),
                "consumeStart".to_owned(),
            ),
            (UIDM.to_owned(), "show".to_owned()),
        ];
        if names != want {
            return Err(err(format!("activate_message changed: {names:?}")));
        }
        let show = act.clone();
        *act = vec![sig(
            &format!("{UIDM}.swofty_once_check"),
            "is_valid",
            "activate_message",
        )];
        let hidden_props = || {
            vec![
                ("enabled", prop(boolv(false), None)),
                ("visible", prop(boolv(false), None)),
            ]
        };
        let mut once = hidden_props();
        once.push((
            "dbEntry",
            prop(s(""), Some(&format!("user_attributes.{ONCE_ATTR}"))),
        ));
        let mut check = hidden_props();
        check.push(("value", prop(boolv(true), None)));
        check.push(("dbEntry", prop(s(&seen), None)));
        let mut mark = hidden_props();
        mark.push(("source", prop(boolv(true), None)));
        mark.push(("target", prop(boolv(false), Some(&seen))));
        let mut no_seen = vec![sig(&format!("{UIDM}.swofty_mark_seen"), "trigger", "no")];
        no_seen.extend(relabel(&show, "no")?);
        let kids = vec![
            node(
                "swofty_once_check",
                "DBVV",
                once,
                vec![
                    event(
                        "yes",
                        vec![sig(&format!("{UIDM}.swofty_seen_check"), "query", "yes")],
                    ),
                    event("no", relabel(&show, "no")?),
                ],
                &[],
                &[],
            ),
            node(
                "swofty_seen_check",
                "SVCD",
                check,
                vec![
                    event("no", no_seen),
                    event(
                        "yes",
                        vec![
                            sig(UIDM, "deactivate_message", "yes"),
                            sig(&format!("{UIDM}.disable_input"), "trigger", "yes"),
                            signal(
                                &format!("{UIDM}.user_response_setter"),
                                "setValue",
                                "yes",
                                &[("value", "cancel")],
                            ),
                        ],
                    ),
                ],
                &[],
                &[],
            ),
            node("swofty_mark_seen", "UIDS", mark, vec![], &[], &[]),
        ];
        m.slot_mut(CHILDREN)?.extend(kids);
        fix_ids(first_screen(p)?, None);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use swofty_formats::pkg::Pkg;

    use super::*;
    use crate::ui::check::stock_kept;

    fn label(name: &str) -> Node {
        node(
            name,
            "UILB",
            vec![(
                "text",
                obj(vec![
                    ("value", s("")),
                    ("dbentry", s(name)),
                    ("isloctag", boolv(true)),
                ]),
            )],
            vec![],
            &[],
            &[],
        )
    }

    fn with_kids(mut n: Node, kids: Vec<Node>) -> Node {
        n.slot_mut(CHILDREN).unwrap().extend(kids);
        n
    }

    fn stock_popup() -> Vec<u8> {
        let activate = vec![
            sig(
                "persistent.consume_input_wrapper",
                "consumeStart",
                "activate_message",
            ),
            sig(UIDM, "show", "activate_message"),
        ];
        let pressed = vec![
            sig(UIDM, "deactivate_message", "pressed"),
            sig(&format!("{UIDM}.disable_input"), "trigger", "pressed"),
        ];
        let button = with_kids(
            node(
                "button1",
                "UIBT",
                vec![],
                vec![event("pressed", pressed)],
                &[],
                &[],
            ),
            vec![label("title")],
        );
        let back = node(
            "back_listener",
            "UIBT",
            vec![],
            vec![event(
                "pressed",
                vec![
                    sig(UIDM, "deactivate_message", "pressed"),
                    sig(&format!("{UIDM}.disable_input"), "trigger", "pressed"),
                    signal(
                        &format!("{UIDM}.user_response_setter"),
                        "setValue",
                        "pressed",
                        &[("value", "cancel")],
                    ),
                ],
            )],
            &[],
            &[],
        );
        let message = with_kids(
            node(
                "persistent_multichoice_message",
                "UIDM",
                vec![],
                vec![event("activate_message", activate)],
                &[],
                &[],
            ),
            vec![
                label("title"),
                label("body"),
                button,
                back,
                node("disable_input", "UIGP", vec![], vec![], &[], &[]),
                node("user_response_setter", "UIDS", vec![], vec![], &[], &[]),
            ],
        );
        let wrapper = with_kids(
            node("consume_input_wrapper", "UIGP", vec![], vec![], &[], &[]),
            vec![message],
        );
        let mut screen = with_kids(
            node("persistent", "UISC", vec![], vec![], &[], &[]),
            vec![wrapper],
        );
        fix_ids(&mut screen, None);
        ui_bytes(vec![screen]).unwrap()
    }

    fn popup_node(data: &[u8]) -> Node {
        first_screen_of(data)
            .unwrap()
            .find_mut("consume_input_wrapper.persistent_multichoice_message")
            .unwrap()
            .clone()
    }

    fn event_signals(n: &Node, name: &str) -> Vec<Node> {
        n.slot(EVENTS)
            .unwrap()
            .iter()
            .find(|e| e.o().unwrap().str_at("eventName").unwrap() == name)
            .unwrap()
            .o()
            .unwrap()
            .at("signals")
            .unwrap()
            .a()
            .unwrap()
            .clone()
    }

    #[test]
    fn the_popup_gate_leaves_stock_popups_alone() {
        let stock = stock_popup();
        let gated = persistent_gate(&stock).unwrap();
        let changed = stock_kept(&stock, &gated, "swofty_").unwrap();
        assert!(changed.is_empty(), "{changed:?}");

        let popup = popup_node(&gated);
        let shown = relabel(&event_signals(&popup_node(&stock), "activate_message"), "no").unwrap();
        assert_eq!(
            canonical(&arr(event_signals(
                popup.child("swofty_once_check").unwrap(),
                "no"
            ))),
            canonical(&arr(shown))
        );
    }

    #[test]
    fn a_signal_to_the_popup_labels_fails_the_check() {
        let stock = stock_popup();
        let gated = edit(&persistent_gate(&stock).unwrap(), |p| {
            let m = multichoice(p)?;
            event_mut(m.child_mut("swofty_once_check")?, "no")?.push(sig_with(
                &format!("{UIDM}.title"),
                "setLocalise",
                "no",
                &[("localise", Attr::Bool(true))],
            ));
            Ok(())
        })
        .unwrap();
        let changed = stock_kept(&stock, &gated, "swofty_").unwrap();
        assert_eq!(changed.len(), 1, "{changed:?}");
        assert!(changed[0].contains(".title.setLocalise"), "{changed:?}");
    }

    fn message(id: &str, tokens: bool) -> Node {
        let mut items = vec![
            ("id", s(id)),
            ("title", s(&format!("lng_{id}_title"))),
            ("messageBody", s(&format!("lng_{id}_body"))),
            ("attributes", arr(vec![])),
            (
                "options",
                arr(vec![obj(vec![
                    ("value", s("lng_msg_cancel")),
                    ("response", s("cancel")),
                    ("defaultSelection", boolv(false)),
                ])]),
            ),
        ];
        if tokens {
            items.push((
                "bodyTokens",
                obj(vec![(
                    "ERRORCODE",
                    obj(vec![("localise", boolv(false)), ("value", s(""))]),
                )]),
            ));
        }
        obj(items)
    }

    fn messages_file() -> Vec<u8> {
        let mut all: Vec<Node> = ONLINE_MESSAGES
            .iter()
            .map(|id| message(id, id.ends_with("_with_code")))
            .collect();
        all.push(message("quit_session", false));
        all.push(message(MESSAGE_TEMPLATE, true));
        Pkg {
            name: [0; 4],
            root: obj(vec![("messages", arr(all))]),
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn by_id(data: &[u8]) -> Vec<(String, Node)> {
        Pkg::parse(data)
            .unwrap()
            .root
            .o()
            .unwrap()
            .at("messages")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|m| (m.o().unwrap().str_at("id").unwrap().to_owned(), m.clone()))
            .collect()
    }

    #[test]
    fn online_messages_show_language_strings_and_others_stay_stock() {
        let stock = by_id(&messages_file());
        let ours = by_id(&messages(&messages_file(), true).unwrap());
        assert_eq!(ours.len(), stock.len() + 1);
        assert_eq!(ours.last().unwrap().0, MESSAGE_ID);
        for ((id, m), (_, before)) in ours.iter().zip(&stock) {
            let o = m.o().unwrap();
            if !ONLINE_MESSAGES.contains(&id.as_str()) {
                assert_eq!(canonical(m), canonical(before), "{id} changed");
                continue;
            }
            assert_eq!(o.str_at("title").unwrap(), POPUP_TITLE);
            assert_eq!(o.str_at("messageBody").unwrap(), POPUP_BODY);
            assert!(o.get("titleTokens").is_none() && o.get("bodyTokens").is_none());
            assert_eq!(o.at("options").unwrap().a().unwrap().len(), 1);
        }
        let plain = by_id(&messages(&messages_file(), false).unwrap());
        assert_eq!(plain.len(), stock.len() + 1);
        for ((id, m), (_, before)) in plain.iter().zip(&stock) {
            let (o, b) = (m.o().unwrap(), before.o().unwrap());
            assert_eq!(o.str_at("title").unwrap(), b.str_at("title").unwrap(), "{id}");
            assert_eq!(
                o.str_at("messageBody").unwrap(),
                b.str_at("messageBody").unwrap(),
                "{id}"
            );
        }
        let strings = Popup::for_version("1.2.3").strings();
        assert_eq!(
            strings[0],
            (POPUP_TITLE.to_owned(), "Swofty Mod v1.2.3".to_owned())
        );
        assert_eq!(strings[1].0, POPUP_BODY);
        assert!(strings[1].1.contains("New in v1.2.3"));
    }

    #[test]
    fn the_runtime_message_reads_its_text_from_two_attributes() {
        let ours = by_id(&messages(&messages_file(), false).unwrap());
        let (_, m) = ours.iter().find(|(id, _)| id == MESSAGE_ID).unwrap();
        let o = m.o().unwrap();
        assert_eq!(o.str_at("title").unwrap(), "lng_value_tag");
        assert_eq!(o.str_at("messageBody").unwrap(), "lng_value_tag");
        for (tokens, attr) in [("titleTokens", "title"), ("bodyTokens", "message")] {
            let t = o
                .at(tokens)
                .unwrap()
                .o()
                .unwrap()
                .at("VALUE")
                .unwrap()
                .o()
                .unwrap();
            assert_eq!(t.str_at("name").unwrap(), "VALUE");
            assert_eq!(
                t.str_at("dbEntry").unwrap(),
                format!("{ATTRIBUTE_ROW}[{attr}].Value")
            );
            assert_eq!(canonical(t.get("localise").unwrap()), canonical(&boolv(false)));
        }
        let names: Vec<&str> = o
            .at("attributes")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|a| a.o().unwrap().str_at("name").unwrap())
            .collect();
        assert_eq!(names, ["title", "message"]);
        let options = o.at("options").unwrap().a().unwrap();
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].o().unwrap().str_at("response").unwrap(), "ok");
        let twice = by_id(&messages(&messages(&messages_file(), false).unwrap(), false).unwrap());
        assert_eq!(twice.iter().filter(|(id, _)| id == MESSAGE_ID).count(), 1);
    }

    fn hud_message(id: &str) -> Node {
        obj(vec![
            ("id", s(id)),
            ("type", s("hud_main")),
            ("playerFilter", s("exclusive")),
            ("priority", u32v(2)),
            (
                "attributes",
                arr(vec![toast_attribute("message", s(&format!("lng_{id}")))]),
            ),
            ("options", arr(vec![])),
            ("queue", s("hud_main_queue")),
        ])
    }

    #[test]
    fn toasts_are_hud_main_messages_with_literal_text_and_a_fixed_duration() {
        let stock = Pkg {
            name: [0; 4],
            root: obj(vec![(
                "messages",
                arr(vec![hud_message("drs_enabled"), hud_message(TOAST_TEMPLATE)]),
            )]),
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap();
        let ours = by_id(&hud_messages(&stock).unwrap());
        assert_eq!(ours.len(), 2 + TOAST_SECONDS.len());
        for (i, &n) in TOAST_SECONDS.iter().enumerate() {
            let (id, m) = &ours[2 + i];
            assert_eq!(id, &format!("{TOAST_PREFIX}{n}"));
            let o = m.o().unwrap();
            assert_eq!(o.str_at("type").unwrap(), "hud_main");
            assert_eq!(o.str_at("queue").unwrap(), "hud_main_queue");
            let attrs = o.at("attributes").unwrap().a().unwrap();
            let names: Vec<&str> = attrs
                .iter()
                .map(|a| a.o().unwrap().str_at("name").unwrap())
                .collect();
            assert_eq!(
                names,
                [
                    "message",
                    "sub_message",
                    "msg_duration",
                    "flag",
                    "enabled_in_spectator"
                ]
            );
            for a in attrs {
                let a = a.o().unwrap();
                assert_eq!(canonical(a.get("localise").unwrap()), canonical(&boolv(false)));
            }
            let duration = attrs[2].o().unwrap().get("value").unwrap();
            assert_eq!(canonical(duration), canonical(&f32v(n as f32)));
        }
        assert_eq!(canonical(&ours[0].1), canonical(&hud_message("drs_enabled")));
        let twice = by_id(&hud_messages(&hud_messages(&stock).unwrap()).unwrap());
        assert_eq!(twice.len(), ours.len());
    }

    fn routes_file() -> Vec<u8> {
        let options = route("options", vec![], "", 0, None);
        Pkg {
            name: [0; 4],
            root: obj(vec![(
                "RouteGroupings",
                arr(vec![
                    route_group("fe_home", vec![options]),
                    route_group("settings_container", vec![]),
                ]),
            )]),
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn pkg_of(groups: Vec<Node>) -> Vec<u8> {
        Pkg {
            name: [0; 4],
            root: obj(vec![("RouteGroupings", arr(groups))]),
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn route_ids_of(p: &Pkg, g: &str) -> Vec<String> {
        group(p, g)
            .unwrap()
            .o()
            .unwrap()
            .at("Routes")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .filter_map(|r| id_of(r).map(str::to_owned))
            .collect()
    }

    fn route_in<'a>(p: &'a Pkg, g: &str, id: &str) -> &'a Node {
        group(p, g)
            .unwrap()
            .o()
            .unwrap()
            .at("Routes")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .find(|r| id_of(r) == Some(id))
            .unwrap()
    }

    fn migrate_cmd(id: u32, flow: &str, entry: &str) -> Node {
        route_cmd("Migrate", id, &[("Flow", flow), ("Activator", entry)], None)
    }

    fn hub_showroom_route(callback: bool) -> Node {
        let mut cmds = vec![
            route_cmd("PlayerCard", 1, &[("PlayerCardMode", "0")], None),
            route_cmd(
                "TransitionScreen",
                2,
                &[("Direction", "out"), ("Target", F1_WORLD_HUB)],
                None,
            ),
            route_cmd("CloseScreen", 3, &[("Target", F1_WORLD_HUB)], Some("2")),
            route_cmd("OpenScreen", 4, &[("Target", SHOWROOM_SCREEN)], Some("3")),
        ];
        if callback {
            cmds.push(route_cmd(
                "ViewModelCallback",
                5,
                &[("Message", ""), ("Target", SHOWROOM_SCREEN)],
                Some("4"),
            ));
        }
        cmds.extend([
            route_cmd("InputState", 6, &[], Some("5")),
            route_cmd(
                "TransitionScreen",
                7,
                &[("Direction", "in"), ("Target", SHOWROOM_SCREEN)],
                Some("5"),
            ),
            route_cmd(
                "TriggerSubRoute",
                8,
                &[("SubRoute", "close"), ("Target", THEMING_OVERLAY)],
                None,
            ),
        ]);
        route("showroom", cmds, "6,7,8", 1, Some(SHOWROOM_SCREEN))
    }

    fn f1_world_with(showroom: Node) -> Vec<u8> {
        pkg_of(vec![
            route_group(
                F1_WORLD_HUB,
                vec![route("back_1", vec![], "fe_home", 2, None), showroom],
            ),
            route_group(SHOWROOM_SCREEN, vec![route("back_1", vec![], "", 2, None)]),
        ])
    }

    fn steps(r: &Node) -> Vec<String> {
        r.o()
            .unwrap()
            .at("Commands")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|c| {
                [
                    cmd_type(c).map(str::to_owned),
                    attr_value(c, "SubRoute"),
                    attr_value(c, "Direction"),
                    attr_value(c, "Target"),
                    attr_value(c, "PlayerCardMode"),
                    c.as_obj()
                        .and_then(|o| o.str_at("Dependencies").ok())
                        .map(str::to_owned),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ")
            })
            .collect()
    }

    fn home_with_theatre() -> Vec<u8> {
        pkg_of(vec![
            route_group(
                "fe_home",
                vec![
                    route("options", vec![], "", 0, None),
                    route(
                        "theatre",
                        vec![
                            route_cmd(
                                "TriggerSubRoute",
                                5,
                                &[
                                    ("SubRoute", "close"),
                                    ("Target", "fe_home"),
                                    ("To", "theatre_container"),
                                ],
                                None,
                            ),
                            route_cmd(
                                "TriggerSubRoute",
                                6,
                                &[("SubRoute", "open_no_input"), ("Target", "theatre_container")],
                                Some("5"),
                            ),
                        ],
                        "6",
                        1,
                        Some("theatre_container"),
                    ),
                ],
            ),
            route_group("settings_container", vec![]),
        ])
    }

    #[test]
    fn the_main_menu_showroom_route_is_the_hubs_own_showroom_route_from_the_home_screen() {
        let out = routes(
            &home_with_theatre(),
            Vec::new(),
            Vec::new(),
            &f1_world_with(hub_showroom_route(true)),
        )
        .unwrap();
        let p = Pkg::parse(&out).unwrap();
        let ours = route_in(&p, "fe_home", SHOWROOM_ROUTE);
        assert_eq!(
            steps(ours),
            [
                "PlayerCard 0",
                "TransitionScreen out fe_home",
                "CloseScreen fe_home 2",
                "OpenScreen fe_showroom 3",
                "ViewModelCallback fe_showroom 4",
                "InputState 5",
                "TransitionScreen in fe_showroom 5",
                "TriggerSubRoute close theming_overlay_frontend",
            ],
            "fe_home's slices stay open, fe_showroom gets its view model callback before input, and the theming overlay closes on its own"
        );
        let o = ours.o().unwrap();
        assert_eq!(o.str_at("Dependencies").unwrap(), "6,7,8");
        assert_eq!(o.at("Type").unwrap().as_val().unwrap().as_u32(), Some(1));
        assert_eq!(attr_value(ours, "Destination").as_deref(), Some(SHOWROOM_SCREEN));
        let text = format!("{ours:?}");
        for gone in [
            "theatre_container",
            F1_WORLD_HUB,
            "slice_bg_content_underlay",
            "CloseSlices",
            "open_no_input",
        ] {
            assert!(!text.contains(gone), "{gone} in {text}");
        }
        assert_eq!(
            steps(route_in(&p, "fe_home", "theatre")),
            [
                "TriggerSubRoute close fe_home",
                "TriggerSubRoute open_no_input theatre_container 5"
            ],
            "the game's own Theatre route stays"
        );
        let twice = routes(
            &out,
            Vec::new(),
            Vec::new(),
            &f1_world_with(hub_showroom_route(true)),
        )
        .unwrap();
        let ids = route_ids_of(&Pkg::parse(&twice).unwrap(), "fe_home");
        assert_eq!(ids.iter().filter(|i| *i == SHOWROOM_ROUTE).count(), 1);
    }

    #[test]
    fn no_showroom_route_is_built_from_the_theatre_when_the_hubs_showroom_route_is_missing_or_changed() {
        let without_hub = pkg_of(vec![route_group(SHOWROOM_SCREEN, vec![])]);
        for f1_world in [Vec::new(), without_hub, f1_world_with(hub_showroom_route(false))] {
            let out = routes(&home_with_theatre(), Vec::new(), Vec::new(), &f1_world).unwrap();
            assert!(!has_route(&out, "fe_home", SHOWROOM_ROUTE));
            assert!(has_route(&out, "fe_home", "theatre"));
        }
    }

    #[test]
    fn the_home_gets_showroom_time_trial_and_grand_prix_routes_copied_from_its_own() {
        let activity = route(
            "home_to_activity",
            vec![migrate_cmd(1, "f1_world_flow", ACTIVITY_ENTRY)],
            "1",
            0,
            Some("f1_world_activity_container"),
        );
        let data = pkg_of(vec![
            route_group(
                "fe_home",
                vec![
                    route("options", vec![], "", 0, None),
                    route("theatre", vec![], "theatre_container", 0, None),
                    activity,
                ],
            ),
            route_group("settings_container", vec![]),
        ]);
        let out = routes(
            &data,
            Vec::new(),
            Vec::new(),
            &f1_world_with(hub_showroom_route(true)),
        )
        .unwrap();
        let p = Pkg::parse(&out).unwrap();
        let ids = route_ids_of(&p, "fe_home");
        for id in [
            SHOWROOM_ROUTE,
            TIME_TRIAL_ROUTE,
            GRAND_PRIX_ROUTE,
            GRAND_PRIX_SAVES_ROUTE,
            "theatre",
            "home_to_activity",
        ] {
            assert!(ids.iter().any(|i| i == id), "{id} in {ids:?}");
        }
        let entry_of = |id: &str| {
            let r = route_in(&p, "fe_home", id);
            let cmds = r.o().unwrap().at("Commands").unwrap().a().unwrap();
            attr_value(&cmds[0], "Activator").unwrap()
        };
        assert_eq!(entry_of("home_to_activity"), ACTIVITY_ENTRY);
        assert_eq!(entry_of(TIME_TRIAL_ROUTE), TIME_TRIAL_ENTRY);
        assert_eq!(entry_of(GRAND_PRIX_ROUTE), GRAND_PRIX_ENTRY);
        let saves = route_in(&p, "fe_home", GRAND_PRIX_SAVES_ROUTE);
        assert_eq!(
            saves.o().unwrap().str_at("Dependencies").unwrap(),
            "grand_prix_continue"
        );
    }

    fn home_route_with(video: &str, curve: &str) -> Node {
        route(
            "home",
            vec![
                route_cmd(
                    "Wait",
                    1,
                    &[("Time", "0"), ("DilationCurve", "tran_pre_wipe_dly_gen")],
                    None,
                ),
                route_cmd("StartStandardWipe", 2, &[("Video", video)], Some("1")),
                route_cmd("Wait", 3, &[("Time", "0"), ("DilationCurve", curve)], None),
                route_cmd(
                    "TriggerSubRoute",
                    4,
                    &[("SubRoute", "open"), ("Target", THEMING_OVERLAY)],
                    None,
                ),
                migrate_cmd(5, "frontend_flow", "enter_fe_home"),
            ],
            "2,4,5",
            0,
            Some("fe_home"),
        )
    }

    #[test]
    fn the_container_the_showroom_and_the_saves_get_a_route_that_leaves_for_the_main_menu() {
        let data = pkg_of(vec![
            route_group(
                "f1_world_activity_container",
                vec![
                    route("f1_world", vec![], "fe_f1_world", 0, None),
                    home_route_with("ui_wipes/ui_wipe_tiny_right.bk2", "tran_wipe_tiny_right_dly_c"),
                ],
            ),
            route_group("fe_showroom", vec![route("back_1", vec![], "", 0, None)]),
            route_group("grand_prix_continue", vec![route("back_1", vec![], "", 0, None)]),
            route_group("fe_f1_world", vec![route("back_1", vec![], "fe_home", 0, None)]),
        ]);
        let out = f1_world_routes(&data).unwrap();
        let p = Pkg::parse(&out).unwrap();
        assert_eq!(route_ids_of(&p, "fe_showroom"), ["back_1", HOME_BACK_ROUTE]);
        assert_eq!(
            route_ids_of(&p, "grand_prix_continue"),
            ["back_1", HOME_BACK_ROUTE]
        );
        assert_eq!(
            route_ids_of(&p, "f1_world_activity_container"),
            ["f1_world", "home", BACK_HOME_FROM_CONTAINER]
        );
        assert_eq!(route_ids_of(&p, "fe_f1_world"), ["back_1"]);
        let home = route_in(&p, "f1_world_activity_container", BACK_HOME_FROM_CONTAINER);
        let cmds = home.o().unwrap().at("Commands").unwrap().a().unwrap();
        assert_eq!(
            attr_value(&cmds[1], "Video").as_deref(),
            Some(TINY_WIPE),
            "the stock small wipe stays"
        );
        assert_eq!(
            attr_value(&cmds[2], "DilationCurve").as_deref(),
            Some(TINY_WIPE_CURVE)
        );
        assert_eq!(
            attr_value(&cmds[0], "DilationCurve").as_deref(),
            Some("tran_pre_wipe_dly_gen")
        );
        assert_eq!(
            cmds[3].o().unwrap().str_at("Dependencies").unwrap(),
            "5",
            "the overlay opens after the migration"
        );
        let stock = route_in(&p, "f1_world_activity_container", "home");
        let stock_cmds = stock.o().unwrap().at("Commands").unwrap().a().unwrap();
        assert_eq!(
            attr_value(&stock_cmds[1], "Video").as_deref(),
            Some("ui_wipes/ui_wipe_tiny_right.bk2")
        );
    }

    fn home_customisation_route(flow: &str) -> Node {
        route(
            "customisation",
            vec![
                route_cmd(
                    "TransitionScreen",
                    1,
                    &[("Direction", "out"), ("Target", "fe_home")],
                    None,
                ),
                route_cmd("CloseScreen", 2, &[("Target", "fe_home")], Some("1")),
                migrate_cmd(3, flow, CUSTOMISATION_ENTRY),
            ],
            "2,3",
            1,
            Some(CUSTOMISATION_GROUP),
        )
    }

    #[test]
    fn the_home_gets_a_customisation_route_copied_from_its_own() {
        let data = pkg_of(vec![
            route_group(
                "fe_home",
                vec![
                    route("options", vec![], "", 0, None),
                    home_customisation_route(CUSTOMISATION_FLOW),
                ],
            ),
            route_group("settings_container", vec![]),
        ]);
        let out = routes(&data, Vec::new(), Vec::new(), &[]).unwrap();
        assert!(has_route(&out, "fe_home", CUSTOMISATION_ROUTE));
        assert!(has_route(&out, "fe_home", "customisation"));
        let p = Pkg::parse(&out).unwrap();
        let ours = route_in(&p, "fe_home", CUSTOMISATION_ROUTE);
        let cmds = ours.o().unwrap().at("Commands").unwrap().a().unwrap();
        assert_eq!(attr_value(&cmds[1], "Target").as_deref(), Some("fe_home"));
        assert_eq!(attr_value(&cmds[2], "Flow").as_deref(), Some(CUSTOMISATION_FLOW));
        assert_eq!(
            attr_value(&cmds[2], "Activator").as_deref(),
            Some(CUSTOMISATION_ENTRY)
        );
        let twice = routes(&out, Vec::new(), Vec::new(), &[]).unwrap();
        let ids = route_ids_of(&Pkg::parse(&twice).unwrap(), "fe_home");
        assert_eq!(ids.iter().filter(|i| *i == CUSTOMISATION_ROUTE).count(), 1);
    }

    #[test]
    fn a_home_customisation_route_into_another_flow_is_not_copied() {
        for groups in [
            vec![route_group(
                "fe_home",
                vec![route("options", vec![], "", 0, None)],
            )],
            vec![route_group(
                "fe_home",
                vec![
                    route("options", vec![], "", 0, None),
                    home_customisation_route("frontend_flow"),
                ],
            )],
        ] {
            let mut groups = groups;
            groups.push(route_group("settings_container", vec![]));
            let out = routes(&pkg_of(groups), Vec::new(), Vec::new(), &[]).unwrap();
            assert!(!has_route(&out, "fe_home", CUSTOMISATION_ROUTE));
        }
    }

    #[test]
    fn the_customisation_screen_gets_the_containers_route_to_the_main_menu() {
        let f1_world = pkg_of(vec![route_group(
            "f1_world_activity_container",
            vec![home_route_with(
                "ui_wipes/ui_wipe_tiny_right.bk2",
                "tran_wipe_tiny_right_dly_c",
            )],
        )]);
        let back = migrate_cmd(1, "frontend_flow", "enter_fe_f1_world");
        let data = pkg_of(vec![
            route_group(
                CUSTOMISATION_GROUP,
                vec![
                    route("back_1", vec![back], "1", 2, Some("fe_f1_world")),
                    route("back_2", vec![], "", 2, Some("fe_news")),
                ],
            ),
            route_group(
                "customisation_driver_suit",
                vec![route("back_1", vec![], "", 2, None)],
            ),
        ]);
        let out = customisation_routes(&data, &f1_world).unwrap();
        let p = Pkg::parse(&out).unwrap();
        assert_eq!(
            route_ids_of(&p, CUSTOMISATION_GROUP),
            ["back_1", "back_2", BACK_HOME_FROM_CONTAINER]
        );
        assert_eq!(route_ids_of(&p, "customisation_driver_suit"), ["back_1"]);
        assert!(has_route(&out, CUSTOMISATION_GROUP, BACK_HOME_FROM_CONTAINER));
        let ours = route_in(&p, CUSTOMISATION_GROUP, BACK_HOME_FROM_CONTAINER);
        let cmds = ours.o().unwrap().at("Commands").unwrap().a().unwrap();
        assert_eq!(attr_value(&cmds[4], "Flow").as_deref(), Some("frontend_flow"));
        assert_eq!(
            attr_value(&cmds[4], "Activator").as_deref(),
            Some("enter_fe_home")
        );
        assert_eq!(
            cmds[3].o().unwrap().str_at("Dependencies").unwrap(),
            "5",
            "the overlay opens after the migration"
        );
        let stock_back = route_in(&p, CUSTOMISATION_GROUP, "back_1");
        let stock_cmds = stock_back.o().unwrap().at("Commands").unwrap().a().unwrap();
        assert_eq!(
            attr_value(&stock_cmds[0], "Activator").as_deref(),
            Some("enter_fe_f1_world"),
            "the game's own Back stays"
        );
        let twice = customisation_routes(&out, &f1_world).unwrap();
        let ids = route_ids_of(&Pkg::parse(&twice).unwrap(), CUSTOMISATION_GROUP);
        assert_eq!(ids.iter().filter(|i| *i == BACK_HOME_FROM_CONTAINER).count(), 1);
    }

    #[test]
    fn the_customisation_back_route_needs_the_stock_screen_and_container() {
        let f1_world = pkg_of(vec![route_group(
            "f1_world_activity_container",
            vec![home_route_with(
                "ui_wipes/ui_wipe_tiny_right.bk2",
                "tran_wipe_tiny_right_dly_c",
            )],
        )]);
        let no_back = pkg_of(vec![route_group(
            CUSTOMISATION_GROUP,
            vec![route("open", vec![], "", 0, None)],
        )]);
        assert!(customisation_routes(&no_back, &f1_world).is_err());
        let no_group = pkg_of(vec![route_group(
            "customisation_car",
            vec![route("back_1", vec![], "", 2, None)],
        )]);
        assert!(customisation_routes(&no_group, &f1_world).is_err());
        let no_home = pkg_of(vec![route_group("f1_world_activity_container", vec![])]);
        let data = pkg_of(vec![route_group(
            CUSTOMISATION_GROUP,
            vec![route("back_1", vec![], "", 2, None)],
        )]);
        assert!(customisation_routes(&data, &no_home).is_err());
        assert!(!has_route(&data, CUSTOMISATION_GROUP, BACK_HOME_FROM_CONTAINER));
        assert!(!has_route(b"not a pkg", CUSTOMISATION_GROUP, "back_1"));
    }

    fn group<'a>(p: &'a Pkg, id: &str) -> Option<&'a Node> {
        p.root
            .o()
            .unwrap()
            .at("RouteGroupings")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .find(|g| id_of(g) == Some(id))
    }

    fn route_ids(g: &Node) -> Vec<String> {
        g.o()
            .unwrap()
            .at("Routes")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|r| id_of(r).unwrap().to_owned())
            .collect()
    }

    #[test]
    fn addon_screens_get_route_groups_and_an_open_route_on_their_anchor() {
        let screens = [("ai".to_owned(), "settings_container".to_owned())];
        let d = routes(
            &routes_file(),
            route_groups(&[], &[]),
            screen_route_groups(&screens),
            &[],
        )
        .unwrap();
        // a group already there is kept, not added twice
        let d = routes(&d, Vec::new(), screen_route_groups(&screens), &[]).unwrap();
        let opened = [
            (
                "ai".to_owned(),
                "settings_container".to_owned(),
                "swofty_addon_ai_3".to_owned(),
            ),
            ("x".to_owned(), "no_such_anchor".to_owned(), "x".to_owned()),
        ];
        let d = anchor_routes(&anchor_routes(&d, &opened).unwrap(), &opened).unwrap();
        let p = Pkg::parse(&d).unwrap();
        let ai: Vec<&Node> = p
            .root
            .o()
            .unwrap()
            .at("RouteGroupings")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .filter(|g| id_of(g) == Some("ai"))
            .collect();
        assert_eq!(ai.len(), 1);
        assert_eq!(route_ids(ai[0]), ["open", "open_no_input", "close", "back"]);
        let back = canonical(&ai[0].o().unwrap().at("Routes").unwrap().a().unwrap()[3]);
        assert_eq!(
            back,
            canonical(&subpage_route("back", "ai", "settings_container", 2))
        );
        assert_eq!(route_ids(group(&p, "settings_container").unwrap()), ["open_ai"]);
        assert!(group(&p, "no_such_anchor").is_none());
    }

    #[test]
    fn a_long_addon_settings_list_gets_a_route_group_per_home_screen() {
        let homes = super::super::pages::home_plan(70);
        let pages: Vec<(String, String, String)> = (0..70)
            .map(|i| {
                (
                    format!("open_addon_{i}"),
                    format!("swofty_addon_{i}"),
                    super::super::pages::home_of(i),
                )
            })
            .collect();
        let groups = route_groups(&homes, &pages);
        assert_eq!(groups.len(), 3 + 70);
        let ids: Vec<&str> = groups.iter().take(3).map(|g| id_of(g).unwrap()).collect();
        assert_eq!(ids, ["swofty_settings", "swofty_settings_2", "swofty_settings_3"]);
        let routes = |g: &Node| route_ids(g);
        let first = routes(&groups[0]);
        assert_eq!(&first[..5], ["open", "open_no_input", "close", "back", NEXT_PAGE]);
        assert_eq!(first.len(), 5 + 24);
        let second = routes(&groups[1]);
        assert_eq!(
            &second[..6],
            ["open", "open_no_input", "close", "back", NEXT_PAGE, PREV_PAGE]
        );
        assert_eq!(second.len(), 6 + 24);
        let third = routes(&groups[2]);
        assert_eq!(&third[..5], ["open", "open_no_input", "close", "back", PREV_PAGE]);
        assert_eq!(third.len(), 5 + 22);
        for (g, opens) in [(&groups[0], 0..24), (&groups[1], 24..48), (&groups[2], 48..70)] {
            let ids = routes(g);
            for i in 0..70 {
                assert_eq!(
                    ids.contains(&format!("open_addon_{i}")),
                    opens.contains(&i),
                    "{i}"
                );
            }
        }
        let at = |g: &Node, id: &str| {
            canonical(
                g.o()
                    .unwrap()
                    .at("Routes")
                    .unwrap()
                    .a()
                    .unwrap()
                    .iter()
                    .find(|r| id_of(r) == Some(id))
                    .unwrap(),
            )
        };
        assert_eq!(
            at(&groups[0], "back"),
            canonical(&migrate_route(
                "back",
                "swofty_settings",
                "frontend_flow",
                "fe_home"
            ))
        );
        assert_eq!(
            at(&groups[1], "back"),
            canonical(&subpage_route("back", "swofty_settings_2", "swofty_settings", 2))
        );
        assert_eq!(
            at(&groups[1], NEXT_PAGE),
            canonical(&subpage_route(
                NEXT_PAGE,
                "swofty_settings_2",
                "swofty_settings_3",
                1
            ))
        );
        assert_eq!(
            at(&groups[2], PREV_PAGE),
            canonical(&subpage_route(
                PREV_PAGE,
                "swofty_settings_3",
                "swofty_settings_2",
                2
            ))
        );
        let page = |i: usize| {
            groups
                .iter()
                .find(|g| id_of(g) == Some(&format!("swofty_addon_{i}")))
                .unwrap()
        };
        for (i, home) in [
            (0, "swofty_settings"),
            (30, "swofty_settings_2"),
            (69, "swofty_settings_3"),
        ] {
            let name = format!("swofty_addon_{i}");
            assert_eq!(
                at(page(i), "back"),
                canonical(&subpage_route("back", &name, home, 2))
            );
        }
    }

    #[test]
    fn a_short_addon_settings_list_keeps_one_home_screen() {
        let homes = super::super::pages::home_plan(5);
        let pages: Vec<(String, String, String)> = (0..5)
            .map(|i| {
                (
                    format!("open_addon_{i}"),
                    format!("swofty_addon_{i}"),
                    HOME.to_owned(),
                )
            })
            .collect();
        let groups = route_groups(&homes, &pages);
        assert_eq!(groups.len(), 6);
        let ids = route_ids(&groups[0]);
        assert_eq!(
            ids,
            [
                "open",
                "open_no_input",
                "close",
                "back",
                "open_addon_0",
                "open_addon_1",
                "open_addon_2",
                "open_addon_3",
                "open_addon_4"
            ]
        );
    }

    fn setup() -> SetupRoutes {
        SetupRoutes {
            route: "career_wizard_driver_career_pg".into(),
            home: "swofty_setup_career_wizard_driver_career_pg".into(),
            rows: "swofty_addon_career_wizard_driver_career_pg_2".into(),
            next: "career_wizard_season_length_select".into(),
            next_flow: "career_season_setup_flow".into(),
            back: "fe_career_dlc_select".into(),
            back_flow: "frontend_flow".into(),
            back_route: "back_career_wizard_driver_career_pg".into(),
        }
    }

    fn attr_pairs(list: &Node) -> Vec<(String, String)> {
        list.a()
            .unwrap()
            .iter()
            .map(|a| {
                let a = a.o().unwrap();
                (
                    a.str_at("Id").unwrap().to_owned(),
                    a.str_at("Value").unwrap().to_owned(),
                )
            })
            .filter(|(k, _)| k != "Id")
            .collect()
    }

    fn command_attrs(r: &Node) -> Vec<(String, Vec<(String, String)>)> {
        r.o()
            .unwrap()
            .at("Commands")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|c| {
                let o = c.o().unwrap();
                (
                    o.str_at("Type").unwrap().to_owned(),
                    attr_pairs(o.at("Attributes").unwrap()),
                )
            })
            .collect()
    }

    fn route_by_id<'a>(g: &'a Node, id: &str) -> &'a Node {
        g.o()
            .unwrap()
            .at("Routes")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .find(|r| id_of(r) == Some(id))
            .unwrap()
    }

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    fn file(groups: Vec<Node>) -> Vec<u8> {
        Pkg {
            name: [0; 4],
            root: obj(vec![("RouteGroupings", arr(groups))]),
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    #[test]
    fn a_career_setup_gets_an_entry_route_a_page_pair_and_a_way_back_from_the_wizard() {
        let s = setup();
        let to_f1 = route(
            SETUP_ENTRY_SOURCE,
            vec![
                route_cmd("InputState", 1, &[], None),
                route_cmd(
                    "TriggerSubRoute",
                    2,
                    &[("SubRoute", "close"), ("Target", &s.back), ("To", &s.next)],
                    Some("1"),
                ),
                route_cmd(
                    "Migrate",
                    3,
                    &[("Flow", "career_season_setup_flow"), ("SkipActivator", "true")],
                    Some("2"),
                ),
                route_cmd(
                    "TriggerSubRoute",
                    4,
                    &[
                        ("SubRoute", "open_no_input"),
                        ("Target", &s.next),
                        ("From", &s.back),
                    ],
                    Some("3"),
                ),
            ],
            "1,4",
            1,
            Some(&s.next),
        );
        let frontend = file(vec![route_group(&s.back, vec![to_f1.clone()])]);
        let mut next_group = route_group(&s.next, screen_routes(&s.next));
        next_group
            .o_mut()
            .unwrap()
            .at_mut("Attributes")
            .unwrap()
            .a_mut()
            .unwrap()
            .push(obj(vec![
                ("Id", super::s("Slices")),
                ("Value", super::s("career_wizard_summary,slice_gen_screen_title")),
            ]));
        let wizard = file(vec![next_group]);
        let mut placed = Placed::default();
        let frontend = setup_routes(&frontend, &[setup()], &mut placed).unwrap();
        let wizard = setup_routes(&wizard, &[setup()], &mut placed).unwrap();
        assert_eq!(
            placed.routes,
            [
                format!("{}.{}", s.back, s.route),
                format!("{}.{}", s.next, s.back_route)
            ]
        );
        assert_eq!(
            placed.screens,
            ["career_wizard_summary", "slice_gen_screen_title"]
        );
        let scene = pairs(&SETUP_SCENE);

        let p = Pkg::parse(&frontend).unwrap();
        let dlc = group(&p, &s.back).unwrap();
        assert_eq!(route_ids(dlc), [SETUP_ENTRY_SOURCE, s.route.as_str()]);
        assert_eq!(canonical(route_by_id(dlc, SETUP_ENTRY_SOURCE)), canonical(&to_f1));
        let entry = route_by_id(dlc, &s.route);
        assert_eq!(
            command_attrs(entry),
            [
                ("InputState".to_owned(), vec![]),
                (
                    "TriggerSubRoute".to_owned(),
                    pairs(&[("SubRoute", "close"), ("Target", &s.back), ("To", &s.home)])
                ),
                (
                    "Migrate".to_owned(),
                    pairs(&[("Flow", "options_flow"), ("SkipActivator", "true")])
                ),
                (
                    "TriggerSubRoute".to_owned(),
                    pairs(&[("SubRoute", "open"), ("Target", &s.home), ("From", &s.back)])
                ),
                ("FrontendSceneSwitch".to_owned(), scene.clone()),
            ]
        );
        let switch = &entry.o().unwrap().at("Commands").unwrap().a().unwrap()[4];
        assert_eq!(switch.o().unwrap().str_at("Dependencies").unwrap(), "1");
        assert_eq!(
            attr_pairs(entry.o().unwrap().at("Attributes").unwrap()),
            pairs(&[("Destination", &s.home)])
        );

        let p = Pkg::parse(&wizard).unwrap();
        let back = route_by_id(group(&p, &s.next).unwrap(), &s.back_route);
        let out = |t: &str| {
            (
                "TransitionScreen".to_owned(),
                pairs(&[("Direction", "out"), ("Target", t)]),
            )
        };
        let close = |t: &str| ("CloseScreen".to_owned(), pairs(&[("Target", t)]));
        assert_eq!(
            command_attrs(back),
            [
                out(&s.next),
                out("career_wizard_summary"),
                close("career_wizard_summary"),
                out("slice_gen_screen_title"),
                close("slice_gen_screen_title"),
                (
                    "Migrate".to_owned(),
                    pairs(&[("Flow", "options_flow"), ("SkipActivator", "true")])
                ),
                (
                    "TriggerSubRoute".to_owned(),
                    pairs(&[
                        ("SubRoute", "open"),
                        ("Target", &s.home),
                        ("From", &s.next),
                        ("Message", "")
                    ])
                ),
                ("FrontendSceneSwitch".to_owned(), scene.clone()),
            ]
        );
        let deps: Vec<&str> = back
            .o()
            .unwrap()
            .at("Commands")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|c| c.o().unwrap().str_at("Dependencies").unwrap_or(""))
            .collect();
        assert_eq!(deps, ["", "", "2", "", "4", "1,3,5", "6", "6"]);
        let back = back.o().unwrap();
        assert_eq!(back.str_at("Dependencies").unwrap(), "7");
        assert_eq!(
            attr_pairs(back.at("Attributes").unwrap()),
            pairs(&[("Destination", &s.home), ("AutoFocus", "false")])
        );

        let groups = setup_route_groups(&s);
        assert_eq!(
            groups.iter().map(|g| id_of(g).unwrap()).collect::<Vec<_>>(),
            [s.home.as_str(), s.rows.as_str()]
        );
        assert_eq!(
            route_ids(&groups[0]),
            [
                "open",
                "open_no_input",
                "close",
                "back",
                "confirm",
                "open_options"
            ]
        );
        assert_eq!(route_ids(&groups[1]), ["open", "open_no_input", "close", "back"]);
        assert_eq!(
            attr_pairs(
                route_by_id(&groups[0], "back")
                    .o()
                    .unwrap()
                    .at("Attributes")
                    .unwrap()
            ),
            pairs(&[("Destination", &s.back), ("AutoFocus", "false")])
        );
        let back = command_attrs(route_by_id(&groups[0], "back"));
        assert_eq!(
            back[1],
            (
                "Migrate".to_owned(),
                pairs(&[("Flow", "frontend_flow"), ("SkipActivator", "true")])
            )
        );
        assert_eq!(
            back[2].1,
            pairs(&[
                ("SubRoute", "open"),
                ("Target", &s.back),
                ("From", &s.home),
                ("Message", DRIVER_CAREER)
            ])
        );
        let confirm = command_attrs(route_by_id(&groups[0], "confirm"));
        assert_eq!(
            confirm[1].1,
            pairs(&[("Flow", "career_season_setup_flow"), ("SkipActivator", "true")])
        );
        assert_eq!(confirm[2].1[1], ("Target".to_owned(), s.next.clone()));
        assert_eq!(
            canonical(route_by_id(&groups[1], "back")),
            canonical(&subpage_route("back", &s.rows, &s.home, 2))
        );

        let e = setup_routes(&frontend, &[setup()], &mut Placed::default()).unwrap_err();
        assert!(
            e.to_string()
                .contains(&format!("{} has a route {} already", s.back, s.route)),
            "{e}"
        );
        let bare = file(vec![route_group(&s.back, vec![])]);
        let e = setup_routes(&bare, &[setup()], &mut Placed::default()).unwrap_err();
        assert!(e.to_string().contains(SETUP_ENTRY_SOURCE), "{e}");
    }
}
