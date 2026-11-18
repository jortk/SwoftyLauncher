use std::collections::HashMap;

use swofty_formats::pkg::{Node, Pkg, Val, Value};

use super::node::*;
use super::rows;
use crate::Result;
use crate::manifest::{Binds, CareerSetup, OptionType};
use crate::settings::{SessionAddon, SessionMap, SessionMod, SessionOption};

pub const HOME: &str = "swofty_settings";
pub const STATE: &str = "#ui_menu_data.swofty";
pub const HOME_LAYOUT: &str = "eaid://ui_package/art/screens/options/options_screen";
pub const PAGE_LAYOUT: &str = "eaid://ui_package/art/screens/options/options_edit_screen";
pub const ADJUST_TOOLTIP: &str = "swofty_adjust";
const TAG_STYLE: &str = "18_regular_upper_part_neg";
const LIGHT_TAG: &str = "17_regular_uber";
const SETTINGS_ICON: &str = "ui_package/art/textures/options/o_category/";
/// Columns of tiles ADDON SETTINGS keeps on screen; past them the row
/// scrolls by hiding the column at the other edge, which the stacker closes up.
const VISIBLE_COLUMNS: usize = 3;
/// Rows ADDON SETTINGS shows whole, each under its label, with the next one in part under them; past
/// them the rows scroll by hiding the row and label at the other edge, which the block's stacker closes
/// up.
const HOME_ROWS: usize = 2;
/// The row the page tiles go in.
const PAGES_ROW: &str = "More";
/// The home's one block, which stacks its rows.
const HOME_BLOCK: &str = "col_0";
/// The screen title's element, which the title's parts name first.
const TITLE: &str = "screen_title";
/// Rows an addon's page keeps on screen: what fits the settings page's clipping window. Past
/// them the page scrolls by hiding the row at the other edge, which the list's stacker closes up.
const VISIBLE_ROWS: usize = 18;
/// Tiles one ADDON SETTINGS home screen holds; more go on further home screens, which a NEXT PAGE
/// tile opens. No addon page has opened from a home of 60 tiles or more.
pub const HOME_PAGE_TILES: usize = 24;
pub const NEXT_PAGE: &str = "page_next";
pub const PREV_PAGE: &str = "page_prev";
const PAGE_FIRST: &str = "page_first";

pub struct TileStyle {
    pub rows: usize,
    pub slot: fn(usize) -> &'static str,
    /// A tile's icon, from its kind, its id and whether it is a track.
    pub icon: fn(Kind, &str, bool) -> Icon,
    pub attach: &'static [&'static str],
    pub hide_divider: bool,
    pub by_gap: bool,
    pub wrap: Option<(usize, usize)>,
}

pub enum Icon {
    Texture(&'static str),
    Ref(&'static str),
    Path(&'static str),
}

pub const TILES: TileStyle = TileStyle {
    rows: 3,
    slot: |_| "1a",
    icon: |k, id, track| Icon::Ref(super::icons::of(k, id, track)),
    attach: &["title_1", "stacker_title"],
    hide_divider: true,
    by_gap: true,
    wrap: Some((24, 32)),
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Addon,
    Map,
    Mod,
}

impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::Addon => "addon",
            Kind::Map => "map",
            Kind::Mod => "mod",
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Kind::Addon => "ADDON",
            Kind::Map | Kind::Mod => "MOD",
        }
    }
}

pub struct Entry<'a> {
    pub kind: Kind,
    pub index: usize,
    pub screen: String,
    pub name: &'a str,
    pub author: &'a str,
    pub version: &'a str,
    pub description: &'a str,
    pub id: &'a str,
    /// A track addon or a track package's page.
    pub track: bool,
    /// The track packages an addon brings, shown on its page.
    pub maps: Vec<&'a SessionMap>,
    /// The row it is listed in: its launcher folder, or Addons, Tracks or Mods.
    pub folder: &'a str,
    pub options: Vec<Opt<'a>>,
    /// Every option of the addon, hidden ones too, for its career setup page.
    pub all: Vec<Opt<'a>>,
    pub setup: Option<&'a CareerSetup>,
}

pub struct Opt<'a> {
    pub k: usize,
    pub local: usize,
    pub o: &'a SessionOption,
}

impl Entry<'_> {
    fn details(&self) -> String {
        let author = if self.author.is_empty() {
            "unknown"
        } else {
            self.author
        };
        let version = if self.version.is_empty() {
            "?"
        } else {
            self.version
        };
        format!("by {author}, v{version}")
    }

    fn upper(&self) -> String {
        self.name.to_uppercase()
    }
}

/// The game's strings hold at most 255 bytes: a longer one is cut at a character and ends with "…".
fn fit(s: String) -> String {
    fit_to(s, 255)
}

fn fit_to(s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut end = max - '…'.len_utf8();
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", s[..end].trim_end())
}

const RESTART_NOTE: &str = "Applies after a restart.";

fn with_note(text: &str, note: &str) -> String {
    let text = text.trim();
    if note.is_empty() {
        return fit(text.to_owned());
    }
    if text.is_empty() {
        return fit(note.to_owned());
    }
    let room = 255usize.saturating_sub(note.len() + 1);
    format!("{} {note}", fit_to(text.to_owned(), room))
}

pub fn home_screen(page: usize) -> String {
    match page {
        0 => HOME.to_owned(),
        p => format!("{HOME}_{}", p + 1),
    }
}

pub fn home_of(index: usize) -> String {
    home_screen(index / HOME_PAGE_TILES)
}

#[derive(Debug, Clone, PartialEq)]
pub struct HomePage {
    pub page: usize,
    pub pages: usize,
    pub screen: String,
    pub tiles: std::ops::Range<usize>,
    pub prev: Option<String>,
    pub next: Option<String>,
}

pub fn home_plan(tiles: usize) -> Vec<HomePage> {
    let pages = tiles.div_ceil(HOME_PAGE_TILES).max(1);
    (0..pages)
        .map(|page| HomePage {
            page,
            pages,
            screen: home_screen(page),
            tiles: page * HOME_PAGE_TILES..((page + 1) * HOME_PAGE_TILES).min(tiles),
            prev: (page > 0).then(|| home_screen(page - 1)),
            next: (page + 1 < pages).then(|| home_screen(page + 1)),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Slot {
    Entry(usize),
    Next,
    Prev,
}

/// A home page's rows: each run of entries of one folder, then the page tiles under More.
fn home_rows(flow: &[Entry], page: &HomePage) -> Vec<(String, Vec<Slot>)> {
    let mut rows: Vec<(String, Vec<Slot>)> = Vec::new();
    for i in page.tiles.clone() {
        let folder = flow.get(i).map_or(LOOSE_ADDONS, |e| e.folder);
        match rows.last_mut() {
            Some((name, slots)) if name == folder => slots.push(Slot::Entry(i)),
            _ => rows.push((folder.to_owned(), vec![Slot::Entry(i)])),
        }
    }
    let nav: Vec<Slot> = [
        (page.next.is_some(), Slot::Next),
        (page.prev.is_some(), Slot::Prev),
    ]
    .into_iter()
    .filter_map(|(on, s)| on.then_some(s))
    .collect();
    if !nav.is_empty() {
        rows.push((PAGES_ROW.to_owned(), nav));
    }
    rows
}

/// Signals that scroll `items`, in stack order, so `to` shows with as many before it as `visible`
/// leaves room for: the ones before those hide, the rest that scrolling ever hides show.
fn scroll_to(items: &[String], to: usize, visible: usize, from: &str) -> Vec<Node> {
    let hidden = (to + 1).saturating_sub(visible);
    items[..items.len().saturating_sub(visible)]
        .iter()
        .enumerate()
        .map(|(i, p)| sig(p, if i < hidden { "hide" } else { "show" }, from))
        .collect()
}

pub fn entries<'a>(
    addons: &'a [SessionAddon],
    maps: &'a [SessionMap],
    mods: &'a [SessionMod],
) -> Vec<Entry<'a>> {
    let mut k = 0;
    let mut out = Vec::new();
    for (i, (full, m)) in addons.iter().enumerate().filter(|(_, m)| m.enabled).enumerate() {
        let all: Vec<Opt> = m
            .options
            .iter()
            .enumerate()
            .map(|(local, o)| {
                k += 1;
                Opt { k: k - 1, local, o }
            })
            .collect();
        let options = all
            .iter()
            .filter(|o| !o.o.hidden)
            .map(|o| Opt {
                k: o.k,
                local: o.local,
                o: o.o,
            })
            .collect();
        out.push(Entry {
            kind: Kind::Addon,
            index: i,
            screen: format!("swofty_addon_{full}"),
            name: &m.name,
            author: &m.author,
            version: &m.version,
            description: &m.description,
            id: &m.id,
            track: m.track,
            maps: maps.iter().filter(|mp| mp.addon == m.id).collect(),
            folder: m.folder.as_deref().unwrap_or(LOOSE_ADDONS),
            options,
            all,
            setup: m.career_setup.as_ref(),
        });
    }
    // A track package an enabled addon brings is on that addon's tile; the rest get their own.
    let brought = |mp: &SessionMap| addons.iter().any(|a| a.enabled && a.id == mp.addon);
    for (j, mp) in maps.iter().enumerate().filter(|(_, mp)| !brought(mp)) {
        out.push(Entry {
            kind: Kind::Map,
            index: j,
            screen: format!("swofty_map_{j}"),
            name: &mp.name,
            author: &mp.author,
            version: &mp.version,
            description: &mp.description,
            id: &mp.code,
            track: true,
            maps: Vec::new(),
            folder: LOOSE_TRACKS,
            options: Vec::new(),
            all: Vec::new(),
            setup: None,
        });
    }
    for (j, md) in mods.iter().enumerate() {
        out.push(Entry {
            kind: Kind::Mod,
            index: j,
            screen: format!("swofty_mod_{j}"),
            name: &md.name,
            author: &md.author,
            version: &md.version,
            description: &md.description,
            id: &md.id,
            track: false,
            maps: Vec::new(),
            folder: LOOSE_MODS,
            options: Vec::new(),
            all: Vec::new(),
            setup: None,
        });
    }
    by_folder(out)
}

/// The rows of entries that are in no folder: addons, track packages and mods. A launcher folder of
/// the same name, as the Tracks category's, takes them in.
const LOOSE_ADDONS: &str = "Addons";
const LOOSE_TRACKS: &str = "Tracks";
const LOOSE_MODS: &str = "Mods";

/// Entries grouped by folder, the folders in the order their first entry comes, each folder's
/// entries in the order they come.
fn by_folder(entries: Vec<Entry>) -> Vec<Entry> {
    let mut folders: Vec<&str> = Vec::new();
    for e in &entries {
        if !folders.contains(&e.folder) {
            folders.push(e.folder);
        }
    }
    let mut out = entries;
    out.sort_by_key(|e| folders.iter().position(|f| *f == e.folder));
    out
}

fn tagged(text: &str, tag: &str, style: &str) -> String {
    format!("{text}  {{s:{style}}}{tag}{{/s}}")
}

fn wrap_text(text: &str, width: usize) -> String {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                out.push(std::mem::replace(&mut line, word.to_owned()));
            } else if line.is_empty() {
                line = word.to_owned();
            } else {
                line = format!("{line} {word}");
            }
        }
        out.push(line);
    }
    out.join("\n")
}

/// Lines a tile has for its title, details and description.
const TILE_LINES: usize = 6;
/// Characters of a tile's title, tag included, to a line.
const TITLE_WIDTH: usize = 30;

/// The lines a tile's title takes, as `tagged` writes it: its text and tag, the tag's markup left out.
fn title_lines(title: &str) -> usize {
    let plain = match title.split_once("  {s:") {
        Some((text, rest)) => format!(
            "{text}  {}",
            rest.split_once('}')
                .map_or(rest, |(_, t)| t)
                .trim_end_matches("{/s}")
        ),
        None => title.to_owned(),
    };
    wrap_text(&plain, TITLE_WIDTH).lines().count().max(1)
}

/// The first `max` lines of `wrapped`, the last ending with "..." in `width` when lines were left
/// out, a word at a time.
fn clip_lines(wrapped: &str, max: usize, width: usize) -> String {
    const MORE: &str = "...";
    let lines: Vec<&str> = wrapped.lines().collect();
    if lines.len() <= max {
        return wrapped.to_owned();
    }
    let mut kept: Vec<String> = lines[..max].iter().map(|l| (*l).to_owned()).collect();
    let last = kept.last_mut().expect("max is at least one");
    let mut words: Vec<&str> = last.split(' ').collect();
    while words.len() > 1 && words.join(" ").chars().count() + MORE.len() > width {
        words.pop();
    }
    *last = format!(
        "{}{MORE}",
        words.join(" ").trim_end_matches([',', '.', ';', ':', ' '])
    );
    kept.join("\n")
}

fn label_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct Sources {
    pub templates: HashMap<String, Node>,
    pub career: Node,
    pub home: Node,
    pub container: Node,
}

impl Sources {
    pub fn parse(syst: &[u8], career: &[u8], home: &[u8], container: &[u8]) -> Result<Self> {
        let mut templates = HashMap::new();
        let p = Pkg::parse(syst)?;
        for t in p.root.o()?.at("Templates")?.a()? {
            templates.insert(t.name().to_owned(), t.clone());
        }
        Ok(Self {
            templates,
            career: first_screen_of(career)?,
            home: first_screen_of(home)?,
            container: first_screen_of(container)?,
        })
    }

    fn t(&self, name: &str) -> Result<&Node> {
        self.templates
            .get(name)
            .ok_or_else(|| err(format!("the settings templates have no {name}")))
    }

    fn frame(&self, sname: &str) -> Result<Frame> {
        frame(
            sname,
            &FrameSources {
                action: self.career.child("Action")?,
                audio: self.home.child("audio_theme_trigger")?,
                back: self.home.child("back_listener")?,
                tooltips: self.career.child("tooltips")?,
            },
            &["select", "back"],
        )
    }

    fn screen(&self, name: &str, layout: &str) -> Result<Node> {
        screen(name, layout, self.career.props()?)
    }

    fn title(&self, sname: &str, panel: &str, text: &str) -> Result<Node> {
        let mut t = self.t(panel)?.child("screen_title")?.clone();
        rename_refs(
            &mut t,
            &format!("{panel}.screen_title"),
            &format!("{sname}.screen_title"),
        );
        strip_data(&mut t)?;
        literal_label(t.child_mut("title")?, text)?;
        literal_label(t.child_mut("hidden_settings_text")?, "")?;
        for c in t.slot_mut(CHILDREN)? {
            if matches!(c.name(), "hidden_settings_bg" | "hidden_settings_text") {
                set_bool(c, "visible", false)?;
            }
        }
        Ok(t)
    }
}

pub(super) fn as_group(n: &mut Node) -> Result<()> {
    let keep = [
        "id", "version", "type", "nameId", "name", "animData", "enabled", "visible",
    ];
    let p = n.props_mut()?;
    p.items
        .retain(|(k, _)| k.as_str().is_some_and(|k| keep.contains(&k)));
    p.at_mut("type")?.v_mut()?.set_u32(fourcc("UIGP"));
    n.a_mut()?[BEHAVIOURS] = behaviours(GROUP_BEHAVIOURS);
    Ok(())
}

fn adjust_tip(fr: &mut Frame) -> Result<()> {
    let old = fr.tooltips_old.clone();
    let tips = fr.tooltips.slot_mut(CHILDREN)?;
    let mut c = tips
        .iter()
        .find(|c| c.name() == "select")
        .cloned()
        .ok_or_else(|| err("the tooltips have no select"))?;
    rename_refs(&mut c, &format!("{old}.select"), &format!("{old}.adjust"));
    rename(&mut c, "adjust")?;
    set_prop_value(&mut c, "tooltipsName", Val::str(ADJUST_TOOLTIP))?;
    tips.retain(|x| x.name() != "select");
    tips.push(c);
    Ok(())
}

fn panel(src: &Sources, sname: &str, e: &Entry, first_title: &str, first_body: &str) -> Result<[Node; 2]> {
    let body = fit([e.details(), e.description.to_owned()]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join("\n"));
    let upper = fit(e.upper());
    let mut cat = instance(
        src.t("settings_edit_description_category")?,
        sname,
        "about",
        &[("title", &upper), ("body", &body)],
        true,
    )?;
    for c in cat.slot_mut(CHILDREN)? {
        if c.name() == "icon" {
            set_bool(c, "visible", false)?;
        }
    }
    let mut panel = instance(
        src.t("settings_edit_description_panel")?,
        sname,
        "description",
        &[],
        true,
    )?;
    let set = instance(
        src.t("settings_edit_description_set")?,
        &format!("{sname}.description"),
        "set",
        &[("title", first_title), ("body", first_body)],
        true,
    )?;
    panel.slot_mut(CHILDREN)?.push(set);
    Ok([cat, panel])
}

struct Row<'a> {
    key: String,
    kind: RowKind<'a>,
    label: String,
    title: String,
    body: String,
}

enum RowKind<'a> {
    Info(String),
    Slider(&'a Opt<'a>),
    Toggle(&'a Opt<'a>),
}

fn addon_rows<'a>(e: &'a Entry<'a>) -> Vec<Row<'a>> {
    let mut rows = track_rows(e);
    rows.extend(option_rows(e));
    rows
}

// A track addon's track packages: their codes, to pick in any mode.
fn track_rows<'a>(e: &Entry) -> Vec<Row<'a>> {
    let several = e.maps.len() > 1;
    e.maps
        .iter()
        .enumerate()
        .map(|(j, mp)| {
            let label = match several {
                true => format!("Track Code: {}", mp.name),
                false => "Track Code".to_owned(),
            };
            Row {
                key: format!("row_code_{j}"),
                kind: RowKind::Info(mp.code.clone()),
                title: fit(label.to_uppercase()),
                label: fit(label),
                body: "Pick this track in any mode to race on it.".into(),
            }
        })
        .collect()
}

fn option_rows<'a>(e: &'a Entry<'a>) -> Vec<Row<'a>> {
    if e.options.is_empty() {
        let none = match e.maps.is_empty() {
            true => "This addon has no options.",
            false => "",
        };
        return vec![
            Row {
                key: "row_id".into(),
                kind: RowKind::Info(e.id.into()),
                label: "Addon ID".into(),
                title: "ADDON ID".into(),
                body: none.into(),
            },
            Row {
                key: "row_version".into(),
                kind: RowKind::Info(e.version.into()),
                label: "Version".into(),
                title: "VERSION".into(),
                body: String::new(),
            },
        ];
    }
    e.options
        .iter()
        .map(|opt| {
            let o = opt.o;
            let restart = if o.binds == Binds::Db { RESTART_NOTE } else { "" };
            Row {
                key: format!("row_{}", opt.k),
                kind: if o.kind == OptionType::Slider {
                    RowKind::Slider(opt)
                } else {
                    RowKind::Toggle(opt)
                },
                label: fit(label_case(&o.label)),
                title: fit(o.label.to_uppercase()),
                body: with_note(&o.description, restart),
            }
        })
        .collect()
}

fn map_rows<'a>(e: &Entry) -> Vec<Row<'a>> {
    let info = |key: &str, label: &str, value: &str, title: &str, body: &str| Row {
        key: key.into(),
        kind: RowKind::Info(value.into()),
        label: label.into(),
        title: title.into(),
        body: body.into(),
    };
    vec![
        info(
            "row_code",
            "Track Code",
            e.id,
            "TRACK CODE",
            "Pick this track in any mode to race on it.",
        ),
        info("row_author", "Author", e.author, "AUTHOR", ""),
        info("row_version", "Version", e.version, "VERSION", ""),
    ]
}

fn mod_rows<'a>(e: &Entry) -> Vec<Row<'a>> {
    let info = |key: &str, label: &str, value: &str, title: &str, body: &str| Row {
        key: key.into(),
        kind: RowKind::Info(value.into()),
        label: label.into(),
        title: title.into(),
        body: body.into(),
    };
    vec![
        info(
            "row_id",
            "Mod ID",
            e.id,
            "MOD ID",
            "Enabled by its addon. Turn it on or off from the addon's page in the launcher.",
        ),
        info("row_author", "Author", e.author, "AUTHOR", ""),
        info("row_version", "Version", e.version, "VERSION", ""),
    ]
}

fn state_value(o: &SessionOption) -> Result<&crate::settings::SessionState> {
    o.states
        .get(o.current)
        .ok_or_else(|| err(format!("option {} has no state {}", o.id, o.current)))
}

fn number(v: &serde_json::Value) -> Result<f32> {
    v.as_f64()
        .map(|x| x as f32)
        .ok_or_else(|| err(format!("slider value {v} is not a number")))
}

fn build_page(src: &Sources, e: &Entry, rows: &[Row], adjustable: bool) -> Result<Node> {
    let sname = e.screen.as_str();
    let scr = src.screen(sname, PAGE_LAYOUT)?;
    let mut fr = src.frame(sname)?;
    if adjustable {
        adjust_tip(&mut fr)?;
    } else {
        fr.tooltips.slot_mut(CHILDREN)?.retain(|x| x.name() != "select");
    }
    let mut kids = vec![
        src.title(sname, "settings_edit_screen", &fit(e.upper()))?,
        hidden("hide_filter", &["filter"]),
        hidden("hide_scroll_bar", &["scroll_bar"]),
    ];
    let first = rows.first().ok_or_else(|| err(format!("{sname} has no rows")))?;
    kids.extend(panel(src, sname, e, &first.title, &first.body)?);
    let mut rows_at = Vec::new();
    for r in rows {
        let key = r.key.as_str();
        let mut t = match &r.kind {
            RowKind::Info(value) => {
                let mut t = instance(
                    src.t("settings_edit_toggle")?,
                    sname,
                    key,
                    &[("toggle.label", &r.label), ("toggle.value", &fit(value.clone()))],
                    true,
                )?;
                let tg = t.child_mut("toggle")?;
                as_group(tg)?;
                for c in tg.slot_mut(CHILDREN)? {
                    if matches!(c.name(), "indicator" | "left_arrow" | "right_arrow") {
                        set_bool(c, "visible", false)?;
                    }
                }
                t
            }
            RowKind::Slider(opt) => {
                let o = opt.o;
                let cur = state_value(o)?;
                let mut t = instance(
                    src.t("settings_edit_slider")?,
                    sname,
                    key,
                    &[
                        ("slider.label", &r.label),
                        ("slider.value", &fit(cur.text.clone())),
                    ],
                    true,
                )?;
                let vals = o
                    .states
                    .iter()
                    .map(|s| number(&s.value))
                    .collect::<Result<Vec<_>>>()?;
                let sl = t.child_mut("slider")?;
                let p = sl.props_mut()?;
                p.drop_key("dataContext");
                p.set(
                    "value",
                    prop(f32v(number(&cur.value)?), Some(&format!("{STATE}.s{}", opt.k))),
                );
                p.set("min", prop(f32v(vals[0]), None));
                p.set(
                    "max",
                    prop(
                        f32v(
                            *vals
                                .last()
                                .ok_or_else(|| err(format!("option {} has no states", o.id)))?,
                        ),
                        None,
                    ),
                );
                p.set("step", prop(f32v(1.0), None));
                let c = o.current;
                set_bool(sl.child_mut("left_toggle")?, "enabled", c > 0)?;
                set_bool(sl.child_mut("right_toggle")?, "enabled", c + 1 < vals.len())?;
                set_bool(sl.child_mut("indicator")?, "visible", false)?;
                t
            }
            RowKind::Toggle(opt) => {
                let o = opt.o;
                let c = o.current;
                let cur = state_value(o)?;
                let mut t = instance(
                    src.t("settings_edit_toggle")?,
                    sname,
                    key,
                    &[
                        ("toggle.label", &r.label),
                        ("toggle.value", &fit(cur.text.clone())),
                    ],
                    true,
                )?;
                let tg = t.child_mut("toggle")?;
                as_group(tg)?;
                set_bool(tg.child_mut("indicator")?, "visible", false)?;
                set_bool(tg.child_mut("left_arrow")?, "enabled", c > 0)?;
                set_bool(tg.child_mut("right_arrow")?, "enabled", c + 1 < o.states.len())?;
                t
            }
        };
        let mut evs = vec![event(
            "focused",
            vec![
                sig_with(
                    &format!("{sname}.description.set.title"),
                    "setText",
                    "focused",
                    &[("text", r.title.as_str().into())],
                ),
                sig_with(
                    &format!("{sname}.description.set.body"),
                    "setText",
                    "focused",
                    &[("text", r.body.as_str().into())],
                ),
            ],
        )];
        if let RowKind::Slider(opt) | RowKind::Toggle(opt) = &r.kind {
            let k = opt.k;
            evs.push(event(
                "navRight",
                vec![sig(&format!("{sname}.o{k}_next"), "query", "navRight")],
            ));
            evs.push(event(
                "navLeft",
                vec![sig(&format!("{sname}.o{k}_prev"), "query", "navLeft")],
            ));
        }
        add_events(&mut t, evs)?;
        rows_at.push(kids.len());
        kids.push(t);
    }
    chain_focus(sname, &mut kids, &rows_at)?;
    scroll_rows(sname, &mut kids, &rows_at)?;
    let first = kids[rows_at[0]].name().to_owned();
    finish(scr, fr, kids, &first)
}

fn scroll_rows(sname: &str, kids: &mut [Node], rows_at: &[usize]) -> Result<()> {
    if rows_at.len() <= VISIBLE_ROWS {
        return Ok(());
    }
    let paths: Vec<String> = rows_at
        .iter()
        .map(|&k| format!("{sname}.{}", kids[k].name()))
        .collect();
    for (i, &k) in rows_at.iter().enumerate() {
        let mut evs = Vec::new();
        if i + 1 < paths.len() && i + 1 >= VISIBLE_ROWS {
            evs.push(event(
                "navDown",
                vec![sig(&paths[i + 1 - VISIBLE_ROWS], "hide", "navDown")],
            ));
        }
        if i > 0 {
            evs.push(event("navUp", vec![sig(&paths[i - 1], "show", "navUp")]));
        }
        add_events(&mut kids[k], evs)?;
    }
    Ok(())
}

fn retitle(scr: &mut Node, e: &Entry) -> Result<()> {
    let upper = e.upper();
    let tag = e.kind.tag();
    set_text_value(
        scr.child_mut("screen_title")?.child_mut("title")?,
        &tagged(&upper, tag, LIGHT_TAG),
    )?;
    set_text_value(
        scr.child_mut("about")?.child_mut("title")?,
        &tagged(&upper, tag, TAG_STYLE),
    )
}

fn bind_label(n: &mut Node, path: &str) -> Result<()> {
    let p = n.props_mut()?;
    p.set("text", obj(vec![("value", s("")), ("dbentry", s(path))]));
    p.set("localise", prop(boolv(false), None));
    Ok(())
}

fn vm_page(scr: &mut Node, e: &Entry) -> Result<()> {
    let sname = scr.name().to_owned();
    set_prop(scr, "allowVMBinding", prop(boolv(true), None))?;
    for opt in &e.options {
        let o = opt.o;
        let row = scr.child_mut(&format!("row_{}", opt.k))?;
        let loc = format!("o{}", opt.local);
        let cmd = format!("{}:{}", e.id, o.id);
        let slider = o.kind == OptionType::Slider;
        let sound = if slider { "slider" } else { "toggle" };
        let (ctl, arrows) = if slider {
            let ctl = row.child_mut("slider")?;
            let p = ctl.props_mut()?;
            p.set("dataContext", obj(vec![("value", s("")), ("dbentry", s(&loc))]));
            p.set("value", prop(i32v(0), Some(&format!("{loc}.value"))));
            p.set("min", prop(i32v(0), Some(&format!("{loc}.min"))));
            p.set("max", prop(i32v(1), Some(&format!("{loc}.max"))));
            p.set("step", prop(i32v(1), None));
            (ctl, ["left_toggle", "right_toggle"])
        } else {
            (row.child_mut("toggle")?, ["left_arrow", "right_arrow"])
        };
        let (label, text) = if slider {
            ("label".to_owned(), "text".to_owned())
        } else {
            (format!("{loc}.label"), format!("{loc}.text"))
        };
        bind_label(ctl.child_mut("label")?, &label)?;
        bind_label(ctl.child_mut("value")?, &text)?;
        for a in arrows {
            set_bool(ctl.child_mut(a)?, "enabled", true)?;
        }
        for ev in scr.child_mut(&format!("row_{}", opt.k))?.slot_mut(EVENTS)? {
            let o = ev.o_mut()?;
            let nm = o.str_at("eventName")?.to_owned();
            let verb = match nm.as_str() {
                "navRight" => "next",
                "navLeft" => "prev",
                _ => continue,
            };
            let c = format!("swofty_{verb}:{cmd}");
            *o.at_mut("signals")?.a_mut()? = vec![
                sig(&format!("{sname}.audio_theme_trigger"), sound, &nm),
                raise_cmd(&sname, &nm, &c, &c),
            ];
        }
    }
    fix_ids(scr, None);
    Ok(())
}

fn tile(src: &Sources, style: &TileStyle, parent: &str, name: &str, row: usize, e: &Entry) -> Result<Node> {
    let title = tagged(&fit_to(e.upper(), 200), e.kind.tag(), TAG_STYLE);
    let description = fit(e.description.to_owned());
    let text = TileText {
        title: &title,
        by: &fit(e.details()),
        description: &description,
    };
    tile_with(
        src,
        style,
        parent,
        name,
        row,
        (style.icon)(e.kind, e.id, e.track),
        &text,
    )
}

struct TileText<'a> {
    title: &'a str,
    by: &'a str,
    description: &'a str,
}

fn tile_with(
    src: &Sources,
    style: &TileStyle,
    parent: &str,
    name: &str,
    row: usize,
    icon: Icon,
    text: &TileText,
) -> Result<Node> {
    let mut t = instance(
        src.t("settings_home_category")?,
        parent,
        name,
        &[("title", text.title), ("subtitle", "")],
        true,
    )?;
    t.a_mut()?[LAYOUT] = layout_path(&[(style.slot)(row)]);
    for c in t.slot_mut(CHILDREN)? {
        if c.name() == "icon" {
            let p = c.props_mut()?;
            match icon {
                Icon::Ref(r) => p.set("textureOverride", prop(s(""), Some(r))),
                Icon::Path(path) => p.set("textureOverride", prop(s(path), None)),
                Icon::Texture(name) => p.set(
                    "textureOverride",
                    prop(s(&format!("{SETTINGS_ICON}{name}.tif")), None),
                ),
            }
            p.set("visible", prop(boolv(true), None));
        }
    }
    let (by, description) = match style.wrap {
        Some((by_width, desc_width)) => {
            let by = wrap_text(text.by, by_width);
            let room = TILE_LINES
                .saturating_sub(title_lines(text.title))
                .saturating_sub(by.lines().count())
                .max(1);
            (
                by,
                clip_lines(&wrap_text(text.description, desc_width), room, desc_width),
            )
        }
        None => (text.by.to_owned(), text.description.to_owned()),
    };
    let gap = if style.by_gap && !by.is_empty() && !description.is_empty() {
        "\n"
    } else {
        ""
    };
    let mut info = instance(
        src.t("settings_edit_description_set")?,
        &format!("{parent}.{name}"),
        "info",
        &[("title", &format!("{by}{gap}")), ("body", &description)],
        true,
    )?;
    info.a_mut()?[LAYOUT] = layout_path(style.attach);
    // Like the stock SETTINGS tiles, the details show only on the focused tile.
    set_bool(&mut info, "visible", false)?;
    let info_path = format!("{parent}.{name}.info");
    add_events(
        &mut t,
        vec![
            event("focused", vec![sig(&info_path, "show", "focused")]),
            event("unfocused", vec![sig(&info_path, "hide", "unfocused")]),
        ],
    )?;
    for c in info.slot_mut(CHILDREN)? {
        match c.name() {
            "title" => rename(c, "by")?,
            "body" => rename(c, "desc")?,
            _ => {}
        }
        set_bool(c, "visible", true)?;
    }
    if style.hide_divider {
        info.slot_mut(CHILDREN)?
            .push(hidden("hide_divider", &["divider"]));
    }
    t.slot_mut(CHILDREN)?.push(info);
    Ok(t)
}

struct HomeTile {
    path: String,
    name: String,
    row: usize,
    col: usize,
    writes: Option<String>,
}

/// Drops the first step of the layout of `n`'s children and their children when it is `step`: a node
/// attached to a scene element by its own component names that scene's elements alone.
fn drop_first_step(n: &mut Node, step: &str) -> Result<()> {
    let Some(path) = n.a_mut()?[LAYOUT].a_mut()?.first_mut() else {
        return Ok(());
    };
    let steps = path.a_mut()?;
    let first = steps
        .first()
        .and_then(|s| s.as_obj())
        .and_then(|o| o.str_at("debugName").ok());
    if first == Some(step) && steps.len() > 1 {
        steps.remove(0);
    }
    Ok(())
}

/// Gives `n` the component scene at `path`, as a block's o_block or a tile's o_category.
fn set_component(n: &mut Node, path: &str) -> Result<()> {
    let path = path.strip_suffix(".idf").unwrap_or(path);
    let name = path.rsplit('/').next().unwrap_or(path);
    n.a_mut()?[COMPONENT] = arr(vec![obj(vec![
        (
            "componentId",
            Node::Val(Val::raw(*b"ui64", component_id(path).to_le_bytes())),
        ),
        ("debugName", s(path)),
        ("name", s(name)),
    ])]);
    Ok(())
}

fn page_tile(
    src: &Sources,
    style: &TileStyle,
    parent: &str,
    row: usize,
    flow: &[Entry],
    page: &HomePage,
    slot: Slot,
) -> Result<Node> {
    let (label, to) = match slot {
        Slot::Next => ("NEXT PAGE", page.page + 1),
        _ => ("PREVIOUS PAGE", page.page.saturating_sub(1)),
    };
    let name = if slot == Slot::Next { NEXT_PAGE } else { PREV_PAGE };
    let title = tagged(label, &format!("{}/{}", to + 1, page.pages), TAG_STYLE);
    let on = &flow[(to * HOME_PAGE_TILES).min(flow.len())..((to + 1) * HOME_PAGE_TILES).min(flow.len())];
    let names: Vec<&str> = on.iter().take(3).map(|e| e.name).collect();
    let description = match on.len().saturating_sub(names.len()) {
        0 => names.join(", "),
        more => format!("{} and {more} more", names.join(", ")),
    };
    let text = TileText {
        title: &title,
        by: &format!("Page {} of {}", to + 1, page.pages),
        description: &description,
    };
    tile_with(
        src,
        style,
        parent,
        name,
        row,
        (style.icon)(Kind::Addon, "", false),
        &text,
    )
}

fn home_title(page: &HomePage) -> String {
    match page.pages {
        1 => "ADDON SETTINGS".to_owned(),
        n => tagged(
            "ADDON SETTINGS",
            &format!("PAGE {}/{n}", page.page + 1),
            LIGHT_TAG,
        ),
    }
}

fn build_home(src: &Sources, style: &TileStyle, flow: &[Entry], page: &HomePage) -> Result<Node> {
    let home = page.screen.as_str();
    let scr = src.screen(home, HOME_LAYOUT)?;
    let mut fr = src.frame(home)?;
    if page.prev.is_some() {
        for ev in fr.back.slot_mut(EVENTS)? {
            let o = ev.o_mut()?;
            if o.str_at("eventName")? == "pressed" {
                o.at_mut("signals")?.a_mut()?.insert(
                    1,
                    sig(&format!("{home}.remember_{PREV_PAGE}"), "trigger", "pressed"),
                );
            }
        }
    }
    let mut kids = vec![src.title(home, "settings_home_screen", &home_title(page))?];
    for n in ["filter", "side_panel", "scroll_bar", "scroll_bar_panel"] {
        kids.push(hidden(&format!("hide_{n}"), &[n]));
    }
    let rows = home_rows(flow, page);
    let block_path = format!("{home}.{HOME_BLOCK}");
    let mut block = instance(src.t("settings_home_block")?, home, HOME_BLOCK, &[], true)?;
    block.slot_mut(CHILDREN)?.clear();
    let mut row_nodes = Vec::new();
    let mut label_nodes = Vec::new();
    let mut tiles: Vec<Vec<String>> = Vec::new();
    let mut keyed = Vec::new();
    for (r, (label, slots)) in rows.iter().enumerate() {
        let rname = format!("row_{r}");
        let rpath = format!("{block_path}.{rname}");
        let mut row = instance(src.t("settings_home_block")?, &block_path, &rname, &[], true)?;
        row.slot_mut(CHILDREN)?.clear();
        row.a_mut()?[LAYOUT] = layout_path(&[rows::STACKER]);
        set_component(&mut row, rows::O_ROW)?;
        // The folder's name over its row: a copy of the screen title that the block stacks as an
        // item of its own, as the career content select stacks its Built In and Addons labels.
        let lname = format!("label_{r}");
        let mut name = src.title(home, "settings_home_screen", &fit(label.to_uppercase()))?;
        rename_refs(
            &mut name,
            &format!("{home}.screen_title"),
            &format!("{block_path}.{lname}"),
        );
        rename(&mut name, &lname)?;
        name.a_mut()?[LAYOUT] = layout_path(&[rows::STACKER]);
        set_component(&mut name, rows::TITLE_SCENE)?;
        for c in name.slot_mut(CHILDREN)? {
            if c.is_node() {
                drop_first_step(c, TITLE)?;
            }
        }
        label_nodes.push(name);
        let mut paths = Vec::new();
        for (c, slot) in slots.iter().enumerate() {
            let (mut t, tname, route, writes) = match *slot {
                Slot::Entry(i) => {
                    let e = flow.get(i).ok_or_else(|| {
                        err(format!(
                            "{home} lists tile {i}, past the {} there are",
                            flow.len()
                        ))
                    })?;
                    let tname = format!("t{i}");
                    let t = tile(src, style, &rpath, &tname, c, e)?;
                    (
                        t,
                        tname.clone(),
                        format!("open_{}_{}", e.kind.key(), e.index),
                        tname,
                    )
                }
                Slot::Next => (
                    page_tile(src, style, &rpath, c, flow, page, *slot)?,
                    NEXT_PAGE.to_owned(),
                    NEXT_PAGE.to_owned(),
                    PAGE_FIRST.to_owned(),
                ),
                Slot::Prev => (
                    page_tile(src, style, &rpath, c, flow, page, *slot)?,
                    PREV_PAGE.to_owned(),
                    PREV_PAGE.to_owned(),
                    NEXT_PAGE.to_owned(),
                ),
            };
            let path = format!("{rpath}.{tname}");
            add_events(
                &mut t,
                vec![event(
                    "pressed",
                    vec![
                        sig(&format!("{home}.audio_theme_trigger"), "confirm", "pressed"),
                        sig(&format!("{home}.remember_{tname}"), "trigger", "pressed"),
                        sig_with(
                            &format!("{home}.Action"),
                            "route",
                            "pressed",
                            &[("key", route.into())],
                        ),
                    ],
                )],
            )?;
            row.slot_mut(CHILDREN)?.push(t);
            paths.push(path.clone());
            keyed.push(HomeTile {
                path,
                name: tname,
                row: r,
                col: c,
                writes: Some(writes),
            });
        }
        tiles.push(paths);
        row_nodes.push(row);
    }
    let row_paths: Vec<String> = (0..rows.len()).map(|r| format!("{block_path}.row_{r}")).collect();
    let label_paths: Vec<String> = (0..rows.len())
        .map(|r| format!("{block_path}.label_{r}"))
        .collect();
    for (r, row_tiles) in tiles.iter().enumerate() {
        for (c, path) in row_tiles.iter().enumerate() {
            let mut evs = Vec::new();
            if c > 0 {
                let mut sigs = vec![sig(&row_tiles[c - 1], "focus", "navLeft")];
                if row_tiles.len() > VISIBLE_COLUMNS {
                    sigs.push(sig(&row_tiles[c - 1], "show", "navLeft"));
                }
                evs.push(event("navLeft", sigs));
            }
            if c + 1 < row_tiles.len() {
                let mut sigs = vec![sig(&row_tiles[c + 1], "focus", "navRight")];
                if row_tiles.len() > VISIBLE_COLUMNS && c + 1 >= VISIBLE_COLUMNS {
                    sigs.push(sig(&row_tiles[c + 1 - VISIBLE_COLUMNS], "hide", "navRight"));
                }
                evs.push(event("navRight", sigs));
            }
            // Up and down land on the same column, or the row's last tile, with that row
            // scrolled to show it.
            if r > 0 {
                let above = &tiles[r - 1];
                let to = c.min(above.len() - 1);
                let mut sigs = scroll_to(above, to, VISIBLE_COLUMNS, "navUp");
                if rows.len() > HOME_ROWS {
                    sigs.push(sig(&label_paths[r - 1], "show", "navUp"));
                    sigs.push(sig(&row_paths[r - 1], "show", "navUp"));
                }
                sigs.push(sig(&above[to], "focus", "navUp"));
                evs.push(event("navUp", sigs));
            }
            if r + 1 < tiles.len() {
                let below = &tiles[r + 1];
                let to = c.min(below.len() - 1);
                let mut sigs = scroll_to(below, to, VISIBLE_COLUMNS, "navDown");
                if rows.len() > HOME_ROWS && r + 1 >= HOME_ROWS {
                    sigs.push(sig(&label_paths[r + 1 - HOME_ROWS], "hide", "navDown"));
                    sigs.push(sig(&row_paths[r + 1 - HOME_ROWS], "hide", "navDown"));
                }
                sigs.push(sig(&below[to], "focus", "navDown"));
                evs.push(event("navDown", sigs));
            }
            let tname = &path[path.rfind('.').map_or(0, |d| d + 1)..];
            let tile = row_nodes[r]
                .slot_mut(CHILDREN)?
                .iter_mut()
                .find(|x| x.is_node() && x.name() == tname)
                .ok_or_else(|| err(format!("tile {tname} went missing")))?;
            add_events(tile, evs)?;
        }
    }
    for (label, row) in label_nodes.into_iter().zip(row_nodes) {
        block.slot_mut(CHILDREN)?.extend([label, row]);
    }
    kids.push(block);
    let first_path = keyed
        .first()
        .map(|k| k.path.clone())
        .ok_or_else(|| err("ADDON SETTINGS has nothing to list"))?;
    if page.prev.is_some() {
        keyed.push(HomeTile {
            path: first_path.clone(),
            name: PAGE_FIRST.to_owned(),
            row: 0,
            col: 0,
            writes: None,
        });
    }
    let first = first_path[home.len() + 1..].to_owned();
    finish_home(scr, fr, kids, &keyed, &[row_paths, label_paths], &tiles, &first)
}

/// The home screen's tiles are `keyed`, in the rows and labels at `row_paths` whose tiles are `tiles`. A tile
/// press stores the tile's `writes` in the state's `last`, and an open of the screen focuses the tile
/// whose name `last` holds, with the rows and its row scrolled to show it.
fn finish_home(
    scr: Node,
    fr: Frame,
    mut kids: Vec<Node>,
    keyed: &[HomeTile],
    row_paths: &[Vec<String>; 2],
    tiles: &[Vec<String>],
    first: &str,
) -> Result<Node> {
    let home = scr.name().to_owned();
    let last = format!("{STATE}.last");
    for k in keyed {
        let Some(writes) = &k.writes else { continue };
        kids.push(node(
            &format!("remember_{}", k.name),
            "UIDS",
            vec![
                ("source", prop(s(writes), None)),
                ("target", prop(s(""), Some(&last))),
            ],
            vec![],
            &[],
            &[],
        ));
    }
    kids.push(node(
        "restore_focus",
        "MVCD",
        vec![("dbEntry", prop(s(&last), None))],
        keyed
            .iter()
            .map(|k| {
                // The screen opens with every row and tile showing: only the rows and the
                // tile's own row scroll.
                let mut sigs: Vec<Node> = row_paths
                    .iter()
                    .flat_map(|paths| scroll_to(paths, k.row, HOME_ROWS, &k.name))
                    .collect();
                sigs.extend(scroll_to(&tiles[k.row], k.col, VISIBLE_COLUMNS, &k.name));
                sigs.push(sig(&k.path, "focus", &k.name));
                event(&k.name, sigs)
            })
            .collect(),
        &[],
        &[],
    ));
    let mut scr = finish(scr, fr, kids, first)?;
    for e in scr.child_mut("Action")?.slot_mut(EVENTS)? {
        let o = e.o_mut()?;
        let nm = o.str_at("eventName")?.to_owned();
        if matches!(nm.as_str(), "opened_screen" | "finishedTranIn") {
            o.at_mut("signals")?
                .a_mut()?
                .push(sig(&format!("{home}.restore_focus"), "query", &nm));
        }
    }
    fix_ids(&mut scr, None);
    Ok(scr)
}

fn settings_state_matrix(src: &Sources, scr: &mut Node) -> Result<()> {
    let want: Value = src
        .container
        .props()?
        .at("animData")?
        .o()?
        .at("stateMatrix")?
        .o()?
        .at("value")?
        .v()?
        .value
        .clone();
    scr.props_mut()?
        .at_mut("animData")?
        .o_mut()?
        .at_mut("stateMatrix")?
        .o_mut()?
        .at_mut("value")?
        .v_mut()?
        .value = want;
    Ok(())
}

fn literal_tooltips(scr: &mut Node) -> Result<()> {
    for t in scr.child_mut("tooltips")?.slot_mut(CHILDREN)? {
        if t.name() == "adjust" {
            set_prop_value(t.child_mut("text")?, "localise", Val::bool(false))?;
        }
    }
    Ok(())
}

pub struct Screens {
    pub order: Vec<(String, Vec<u8>)>,
    pub view_models: Vec<String>,
}

/// An addon's options page under another screen name, for an addon screen with `page: options`.
pub fn build_options_page(src: &Sources, e: &Entry, screen: &str) -> Result<Vec<u8>> {
    let e = Entry {
        kind: e.kind,
        index: e.index,
        screen: screen.to_owned(),
        name: e.name,
        author: e.author,
        version: e.version,
        description: e.description,
        id: e.id,
        track: e.track,
        maps: e.maps.clone(),
        folder: e.folder,
        options: e
            .options
            .iter()
            .map(|o| Opt {
                k: o.k,
                local: o.local,
                o: o.o,
            })
            .collect(),
        all: Vec::new(),
        setup: e.setup,
    };
    let rows = addon_rows(&e);
    let mut scr = build_page(src, &e, &rows, !e.options.is_empty())?;
    retitle(&mut scr, &e)?;
    if !e.options.is_empty() {
        vm_page(&mut scr, &e)?;
    }
    settings_state_matrix(src, &mut scr)?;
    literal_tooltips(&mut scr)?;
    ui_bytes(vec![scr])
}

pub const SETUP_CONFIRM: &str = "confirm";
pub const SETUP_OPTIONS: &str = "open_options";

pub fn setup_title<'a>(e: &Entry<'a>, setup: &'a CareerSetup) -> &'a str {
    setup.title.as_deref().map(str::trim).unwrap_or(e.name)
}

pub fn setup_rows_screen(e: &Entry, setup: &CareerSetup) -> String {
    let full = e.screen.trim_start_matches("swofty_addon_");
    format!("swofty_addon_{}_{full}", setup.route)
}

fn setup_entry<'a>(e: &Entry<'a>, setup: &'a CareerSetup) -> Entry<'a> {
    Entry {
        kind: e.kind,
        index: e.index,
        screen: setup_rows_screen(e, setup),
        name: setup_title(e, setup),
        author: e.author,
        version: e.version,
        description: e.description,
        id: e.id,
        track: e.track,
        maps: e.maps.clone(),
        folder: e.folder,
        options: setup
            .options
            .iter()
            .filter_map(|id| e.all.iter().find(|o| &o.o.id == id))
            .map(|o| Opt {
                k: o.k,
                local: o.local,
                o: o.o,
            })
            .collect(),
        all: Vec::new(),
        setup: Some(setup),
    }
}

/// A career setup's rows page: the setup's options, as ADDON SETTINGS shows them, under the setup's title.
pub fn build_setup_page(src: &Sources, e: &Entry, setup: &CareerSetup) -> Result<(String, Vec<u8>)> {
    let e = setup_entry(e, setup);
    if e.options.len() != setup.options.len() {
        return Err(err(format!(
            "career setup {}: {} names options its addon does not have",
            setup.route, e.id
        )));
    }
    let rows = addon_rows(&e);
    let mut scr = build_page(src, &e, &rows, true)?;
    vm_page(&mut scr, &e)?;
    settings_state_matrix(src, &mut scr)?;
    literal_tooltips(&mut scr)?;
    Ok((e.screen.clone(), ui_bytes(vec![scr])?))
}

/// A career setup's home: a CONFIRM tile in the first column and one category tile, which opens the rows page.
pub fn build_setup_home(src: &Sources, e: &Entry, setup: &CareerSetup) -> Result<(String, Vec<u8>)> {
    let home = setup.screen();
    let title = setup_title(e, setup).to_uppercase();
    let scr = src.screen(&home, HOME_LAYOUT)?;
    let fr = src.frame(&home)?;
    let mut kids = vec![src.title(&home, "settings_home_screen", &title)?];
    for n in ["filter", "side_panel", "scroll_bar", "scroll_bar_panel"] {
        kids.push(hidden(&format!("hide_{n}"), &[n]));
    }
    let left = format!("{home}.col_0");
    let right = format!("{home}.col_1");
    let confirm_at = format!("{left}.{SETUP_CONFIRM}");
    let options_at = format!("{right}.options");
    let press = |key: &str| {
        event(
            "pressed",
            vec![
                sig(&format!("{home}.audio_theme_trigger"), "confirm", "pressed"),
                sig_with(
                    &format!("{home}.Action"),
                    "route",
                    "pressed",
                    &[("key", key.into())],
                ),
            ],
        )
    };
    let mut confirm = instance(
        src.t("settings_home_confirm")?,
        &left,
        SETUP_CONFIRM,
        &[
            ("title", "CONFIRM"),
            ("subtitle", ""),
            ("info_title", ""),
            ("info_value", ""),
            ("info_body", ""),
        ],
        true,
    )?;
    confirm.a_mut()?[LAYOUT] = layout_path(&[(TILES.slot)(0)]);
    add_events(
        &mut confirm,
        vec![
            press(SETUP_CONFIRM),
            event("navRight", vec![sig(&options_at, "focus", "navRight")]),
        ],
    )?;
    let labels: Vec<String> = setup_entry(e, setup)
        .options
        .iter()
        .map(|o| label_case(&o.o.label))
        .collect();
    let text = TileText {
        title: &title,
        by: "",
        description: &labels.join(", "),
    };
    let mut category = tile_with(
        src,
        &TILES,
        &right,
        "options",
        0,
        (TILES.icon)(Kind::Addon, e.id, e.track),
        &text,
    )?;
    add_events(
        &mut category,
        vec![
            press(SETUP_OPTIONS),
            event("navLeft", vec![sig(&confirm_at, "focus", "navLeft")]),
        ],
    )?;
    for (name, tile) in [("col_0", confirm), ("col_1", category)] {
        let mut block = instance(src.t("settings_home_block")?, &home, name, &[], true)?;
        block.slot_mut(CHILDREN)?.push(tile);
        kids.push(block);
    }
    let mut scr = finish(scr, fr, kids, "col_1.options")?;
    settings_state_matrix(src, &mut scr)?;
    literal_tooltips(&mut scr)?;
    Ok((home, ui_bytes(vec![scr])?))
}

pub fn build_all(src: &Sources, style: &TileStyle, flow: &[Entry]) -> Result<Screens> {
    let mut nodes = Vec::new();
    for page in home_plan(flow.len()) {
        nodes.push((page.screen.clone(), build_home(src, style, flow, &page)?));
    }
    let mut view_models = Vec::new();
    for e in flow.iter().filter(|e| e.kind == Kind::Addon) {
        let rows = addon_rows(e);
        let mut scr = build_page(src, e, &rows, !e.options.is_empty())?;
        retitle(&mut scr, e)?;
        if !e.options.is_empty() {
            vm_page(&mut scr, e)?;
            view_models.push(e.screen.clone());
        }
        nodes.push((e.screen.clone(), scr));
    }
    for e in flow.iter().filter(|e| e.kind == Kind::Map) {
        let mut scr = build_page(src, e, &map_rows(e), false)?;
        retitle(&mut scr, e)?;
        nodes.push((e.screen.clone(), scr));
    }
    for e in flow.iter().filter(|e| e.kind == Kind::Mod) {
        let mut scr = build_page(src, e, &mod_rows(e), false)?;
        retitle(&mut scr, e)?;
        nodes.push((e.screen.clone(), scr));
    }
    let mut order = Vec::new();
    for (name, mut scr) in nodes {
        settings_state_matrix(src, &mut scr)?;
        literal_tooltips(&mut scr)?;
        order.push((name, ui_bytes(vec![scr])?));
    }
    Ok(Screens { order, view_models })
}

pub fn state_entries(flow: &[Entry]) -> Result<Vec<(String, Node)>> {
    let mut out = vec![
        ("last".to_owned(), uidb_var("last", "uistring", s(""))),
        (
            "online_popup_seen".to_owned(),
            uidb_var("online_popup_seen", "bool", boolv(false)),
        ),
    ];
    for e in flow.iter().filter(|e| e.kind == Kind::Addon) {
        for opt in &e.options {
            let k = opt.k;
            let o = opt.o;
            let name = format!("o{k}");
            out.push((
                name.clone(),
                uidb_var(&name, "uistring", s(&o.current.to_string())),
            ));
            if o.kind == OptionType::Slider {
                let name = format!("s{k}");
                let v = number(&state_value(o)?.value)?;
                out.push((name.clone(), uidb_var(&name, "float", f32v(v))));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_past_the_tile_s_lines_ends_with_dots() {
        let desc = "Istanbul Park (Turkish Grand Prix) as track istanbul, with its own names in all 11 languages. Track v2.72 by Apoc & SK.";
        let wrapped = wrap_text(desc, 32);
        assert_eq!(wrapped.lines().count(), 5);
        let cut = clip_lines(&wrapped, 4, 32);
        let lines: Vec<&str> = cut.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[3].ends_with("..."), "{cut}");
        assert!(lines.iter().all(|l| l.chars().count() <= 32), "{cut}");
        assert_eq!(clip_lines("short", 4, 32), "short");
        assert_eq!(title_lines(&tagged("ISTANBUL PARK", "ADDON", TAG_STYLE)), 1);
        assert_eq!(
            title_lines(&tagged("SIMPLIFIED FOR SWOFTY", "ADDON", TAG_STYLE)),
            1
        );
        assert_eq!(
            title_lines(&tagged(
                "AUTODROMO INTERNAZIONALE DEL MUGELLO",
                "ADDON",
                TAG_STYLE
            )),
            2
        );
    }

    #[test]
    fn a_long_description_is_cut_to_fit_the_games_strings() {
        let long = format!("by Swofty, v0.4.0\n{}", "é".repeat(200));
        let cut = fit(long);
        assert!(cut.len() <= 255, "{}", cut.len());
        assert!(cut.ends_with('…'));
        assert_eq!(fit("short".into()), "short");
    }

    #[test]
    fn a_restart_note_is_kept_when_the_description_is_long() {
        let note = RESTART_NOTE;
        let long = "x".repeat(248);
        let body = with_note(&long, note);
        assert!(body.len() <= 255, "{}", body.len());
        assert!(body.ends_with(note));
        assert!(body.contains('…'));
    }

    #[test]
    fn a_description_that_exactly_fits_is_not_cut() {
        let text = "y".repeat(255 - RESTART_NOTE.len() - 1);
        let body = with_note(&text, RESTART_NOTE);
        assert_eq!(body.len(), 255);
        assert_eq!(body, format!("{text} {RESTART_NOTE}"));
        assert_eq!(
            with_note("Short.", RESTART_NOTE),
            "Short. Applies after a restart."
        );
    }

    #[test]
    fn a_restart_note_survives_multibyte_text_at_the_boundary() {
        for n in 100..140 {
            let text = format!("{}{}", "a".repeat(n % 3), "é🏁".repeat(n));
            let body = with_note(&text, RESTART_NOTE);
            assert!(body.len() <= 255, "{n}: {}", body.len());
            assert!(body.ends_with(RESTART_NOTE));
        }
    }

    #[test]
    fn an_empty_description_keeps_only_the_note() {
        assert_eq!(with_note("", RESTART_NOTE), RESTART_NOTE);
        assert_eq!(with_note("  ", ""), "");
        assert!(with_note(&"z".repeat(400), "").len() <= 255);
    }

    #[test]
    fn a_long_folder_name_is_cut_to_fit_the_games_strings() {
        let name = format!("{}ü🏁", "é".repeat(300)).to_uppercase();
        let cut = fit(name.clone());
        assert!(name.len() > 255);
        assert!(cut.len() <= 255, "{}", cut.len());
        assert!(cut.ends_with('…'));
        let emoji = fit("🏁".repeat(100));
        assert!(emoji.len() <= 255 && emoji.ends_with('…'));
        let tagged_title = tagged(&fit_to(name, 200), "ADDON", TAG_STYLE);
        assert!(tagged_title.len() <= 255, "{}", tagged_title.len());
    }

    fn map(code: &str, name: &str) -> SessionMap {
        SessionMap {
            code: code.into(),
            name: name.into(),
            addon: String::new(),
            version: "1.0".into(),
            author: "a".into(),
            description: String::new(),
        }
    }

    fn a_mod(id: &str, name: &str) -> SessionMod {
        SessionMod {
            id: id.into(),
            name: name.into(),
            version: "2.0".into(),
            author: "b".into(),
            description: String::new(),
        }
    }

    #[test]
    fn a_long_list_splits_into_home_screens_of_at_most_24_tiles() {
        let plan = home_plan(70);
        let page =
            |s: &str, tiles: std::ops::Range<usize>, prev: Option<&str>, next: Option<&str>| HomePage {
                page: 0,
                pages: 3,
                screen: s.to_owned(),
                tiles,
                prev: prev.map(str::to_owned),
                next: next.map(str::to_owned),
            };
        let want = [
            page("swofty_settings", 0..24, None, Some("swofty_settings_2")),
            page(
                "swofty_settings_2",
                24..48,
                Some("swofty_settings"),
                Some("swofty_settings_3"),
            ),
            page("swofty_settings_3", 48..70, Some("swofty_settings_2"), None),
        ];
        for (i, (got, mut want)) in plan.iter().zip(want).enumerate() {
            want.page = i;
            assert_eq!(got, &want);
        }
        assert_eq!(plan.len(), 3);
        assert!(plan.iter().all(|h| h.pages == 3));
        for i in 0..70 {
            assert_eq!(home_of(i), plan[i / HOME_PAGE_TILES].screen);
            assert!(plan[i / HOME_PAGE_TILES].tiles.contains(&i));
        }
        assert_eq!(home_plan(24).len(), 1);
        assert_eq!(home_plan(25).len(), 2);
        assert_eq!(home_plan(0)[0].screen, HOME);
    }

    fn foldered(id: &str, folder: Option<&str>) -> SessionAddon {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "version": "1.0.0", "author": "t", "enabled": true,
            "locked": false, "enable_commands": ["", ""], "folder": folder
        }))
        .unwrap()
    }

    fn slot_ids(slots: &[Slot]) -> Vec<usize> {
        slots
            .iter()
            .map(|s| match s {
                Slot::Entry(i) => *i,
                _ => usize::MAX,
            })
            .collect()
    }

    #[test]
    fn the_home_lists_a_row_per_folder_in_the_order_the_folders_come() {
        let addons = [
            foldered("swofty.core", Some("Built In")),
            foldered("a", Some("Cars")),
            foldered("b", None),
            foldered("c", Some("Cars")),
            foldered("track.x", Some("Tracks")),
        ];
        let maps = [map("imola", "Imola")];
        let mods = [a_mod("ts.sauber", "Sauber")];
        let flow = entries(&addons, &maps, &mods);
        let ids: Vec<&str> = flow.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            ["swofty.core", "a", "c", "b", "track.x", "imola", "ts.sauber"],
            "a folder's addons sit together, loose track packages join Tracks"
        );
        let plan = home_plan(flow.len());
        let rows = home_rows(&flow, &plan[0]);
        let shape: Vec<(&str, Vec<usize>)> = rows.iter().map(|(f, s)| (f.as_str(), slot_ids(s))).collect();
        assert_eq!(
            shape,
            [
                ("Built In", vec![0]),
                ("Cars", vec![1, 2]),
                (LOOSE_ADDONS, vec![3]),
                ("Tracks", vec![4, 5]),
                (LOOSE_MODS, vec![6]),
            ]
        );
    }

    #[test]
    fn page_tiles_take_a_row_after_the_page_s_tiles() {
        let addons: Vec<SessionAddon> = (0..70).map(|i| foldered(&format!("a{i}"), None)).collect();
        let flow = entries(&addons, &[], &[]);
        let plan = home_plan(flow.len());
        let rows: Vec<Vec<(String, Vec<Slot>)>> = plan.iter().map(|h| home_rows(&flow, h)).collect();
        assert_eq!(rows[0].len(), 2);
        assert_eq!(slot_ids(&rows[0][0].1), (0..24).collect::<Vec<_>>());
        assert_eq!(rows[0][1], (PAGES_ROW.to_owned(), vec![Slot::Next]));
        assert_eq!(rows[1][1].1, [Slot::Next, Slot::Prev]);
        assert_eq!(rows[2][1].1, [Slot::Prev]);
        assert_eq!(slot_ids(&rows[2][0].1), (48..70).collect::<Vec<_>>());
    }

    #[test]
    fn only_what_scrolling_hides_gets_scroll_signals() {
        let items: Vec<String> = (0..5).map(|i| format!("t{i}")).collect();
        let pins = |to| -> Vec<(String, String)> {
            scroll_to(&items, to, 3, "e")
                .iter()
                .map(|n| {
                    let o = n.o().unwrap();
                    (
                        o.str_at("to_object_name").unwrap().to_owned(),
                        o.str_at("to_pin_name").unwrap().to_owned(),
                    )
                })
                .collect()
        };
        let pair = |t: &str, p: &str| (t.to_owned(), p.to_owned());
        assert_eq!(pins(1), [pair("t0", "show"), pair("t1", "show")]);
        assert_eq!(pins(3), [pair("t0", "hide"), pair("t1", "show")]);
        assert_eq!(pins(4), [pair("t0", "hide"), pair("t1", "hide")]);
        assert!(
            scroll_to(&items[..3], 2, 3, "e").is_empty(),
            "a row that fits never scrolls"
        );
    }

    #[test]
    fn no_home_screen_grows_past_its_page_whatever_the_addon_count_or_folders() {
        for n in [1, 23, 24, 25, 60, 64, 72, 150, 500] {
            for per_folder in [1, 2, 3, 4, 24] {
                let addons: Vec<SessionAddon> = (0..n)
                    .map(|i| foldered(&format!("a{i}"), Some(&format!("f{}", i / per_folder))))
                    .collect();
                let flow = entries(&addons, &[], &[]);
                let plan = home_plan(n);
                assert_eq!(plan.len(), n.div_ceil(HOME_PAGE_TILES).max(1), "{n}");
                assert_eq!(plan.iter().map(|h| h.tiles.len()).sum::<usize>(), n, "{n}");
                for h in &plan {
                    let rows = home_rows(&flow, h);
                    let lens: Vec<usize> = rows.iter().map(|(_, s)| s.len()).collect();
                    let tiles: usize = lens.iter().sum();
                    assert!(
                        tiles <= HOME_PAGE_TILES + 2,
                        "{n}: {} has {tiles} tiles",
                        h.screen
                    );
                    // Each restore event scrolls the rows and its tile's row, then focuses.
                    let scrolled = rows.len().saturating_sub(HOME_ROWS);
                    let restore_signals: usize = lens
                        .iter()
                        .map(|len| len * (scrolled + len.saturating_sub(VISIBLE_COLUMNS) + 1))
                        .sum();
                    assert!(restore_signals <= 27 * 24, "{n}/{per_folder}: {restore_signals}");
                }
            }
        }
    }

    #[test]
    fn entries_list_maps_then_mods_with_their_own_screens() {
        let maps = [map("imola", "Imola")];
        let mods = [a_mod("ts.sauber", "Sauber"), a_mod("ts.rb", "Red Bull")];
        let flow = entries(&[], &maps, &mods);
        let shape: Vec<(Kind, &str, usize)> = flow
            .iter()
            .map(|e| (e.kind, e.screen.as_str(), e.index))
            .collect();
        assert_eq!(
            shape,
            [
                (Kind::Map, "swofty_map_0", 0),
                (Kind::Mod, "swofty_mod_0", 0),
                (Kind::Mod, "swofty_mod_1", 1),
            ]
        );
        assert_eq!(Kind::Mod.key(), "mod");
        assert_eq!(Kind::Mod.tag(), "MOD");
        assert_eq!((flow[1].name, flow[1].id), ("Sauber", "ts.sauber"));
        assert!(flow[1].options.is_empty());
    }

    fn track_addon(id: &str, enabled: bool) -> SessionAddon {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": "Mugello", "version": "0.58.1", "author": "t", "enabled": enabled,
            "locked": false, "enable_commands": ["", ""]
        }))
        .unwrap()
    }

    #[test]
    fn a_track_an_addon_brings_is_on_that_addons_tile_only() {
        let mut brought = map("mugello", "Mugello");
        brought.addon = "track.mugello".into();
        let mut off = map("istanbul", "Istanbul Park");
        off.addon = "ts.istanbul".into();
        let loose = map("imola", "Imola");
        let maps = [brought, off, loose];
        let addons = [
            track_addon("track.mugello", true),
            track_addon("ts.istanbul", false),
        ];
        let flow = entries(&addons, &maps, &[]);
        let shape: Vec<(Kind, &str)> = flow.iter().map(|e| (e.kind, e.screen.as_str())).collect();
        assert_eq!(
            shape,
            [
                (Kind::Addon, "swofty_addon_0"),
                (Kind::Map, "swofty_map_1"),
                (Kind::Map, "swofty_map_2"),
            ],
            "the enabled addon's track has no tile of its own; a disabled addon's and a loose one keep theirs"
        );
        let codes: Vec<&str> = flow[0].maps.iter().map(|m| m.code.as_str()).collect();
        assert_eq!(codes, ["mugello"]);
        let rows = addon_rows(&flow[0]);
        let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["row_code_0", "row_id", "row_version"]);
        assert!(matches!(&rows[0].kind, RowKind::Info(code) if code == "mugello"));
        assert_eq!(rows[1].body, "", "a track addon's page has more than its id");
    }
}
