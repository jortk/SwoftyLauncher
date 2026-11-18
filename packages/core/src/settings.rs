use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{
    AddonManifest, AddonNeed, AddonOption, Binds, CareerSetup, HubTab, OptionType, ScreenOp,
};
use crate::map::TrackPackage;
use crate::owned::OwnedItem;
use crate::profile::Profile;
use crate::{Error, Result};

pub use swofty_addon::option::{MAX_CHOICES, MAX_ID_LEN, MAX_SLIDER_STEPS, State, valid_option_id};

pub const SCHEMA: u32 = 1;
pub const SETTINGS_FILE: &str = "swofty/settings.json";
pub const RESTART_COMMAND: &str = "swofty_restart";
pub const UI_DIR: &str = "swofty/ui";

/// The addons on in a build, each with its version: what an op's `when.addon` and an option's `needs`
/// are checked against.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OnAddons {
    on: BTreeMap<String, semver::Version>,
    off: BTreeSet<(String, String)>,
}

impl OnAddons {
    pub fn new<'a>(on: impl IntoIterator<Item = &'a AddonManifest>) -> Self {
        Self {
            on: on
                .into_iter()
                .map(|m| (m.id.clone(), m.version.clone()))
                .collect(),
            off: BTreeSet::new(),
        }
    }

    /// Turns toggle `option` of `addon` off for this build, whatever the profile holds.
    pub fn force_off(&mut self, addon: &str, option: &str) {
        self.off.insert((addon.to_owned(), option.to_owned()));
    }

    /// The installed addons the profile has on.
    pub fn of(profile: &Profile, installed: &BTreeMap<String, AddonManifest>) -> Self {
        Self::new(profile.enabled().filter_map(|id| installed.get(id)))
    }

    pub fn has(&self, need: &AddonNeed) -> bool {
        self.on
            .get(&need.id)
            .is_some_and(|v| crate::version::satisfies(&need.version, v))
    }

    /// The first addon the option needs that is off, or on at a version out of its range.
    pub fn missing<'a>(&self, o: &'a AddonOption) -> Option<&'a AddonNeed> {
        o.needs.iter().find(|n| !self.has(n))
    }

    /// The option's value in a build: the profile's, or off while an addon it needs is off or the build
    /// turned it off.
    pub fn value(&self, profile: &Profile, m: &AddonManifest, o: &AddonOption) -> Value {
        match self.missing(o).is_some() || self.off.contains(&(m.id.clone(), o.id.clone())) {
            true => Value::Bool(false),
            false => profile.option(m, o),
        }
    }
}

pub fn djb2(s: &str) -> u32 {
    s.bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33).wrapping_add(u32::from(b)))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSettings {
    pub schema: u32,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests: Option<String>,
    pub restart_command: String,
    pub addons: Vec<SessionAddon>,
    #[serde(default)]
    pub maps: Vec<SessionMap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<SessionMod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<SessionUi>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_items: Vec<OwnedItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub early_plugins: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<String>,
    /// Present when the profile uses launcher saves: the core serves the game's profile save from `remote`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher_saves: Option<SessionSaves>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSaves {
    pub profile: String,
    pub remote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUi {
    pub register: bool,
    pub strict: bool,
    pub recover: bool,
    pub screens: Vec<SessionScreen>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionScreen {
    pub name: String,
    /// `options` for a screen the launcher generates from its addon's options page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// The addon a generated page shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon: Option<String>,
    pub file: String,
    pub flow: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default)]
    pub view_model: bool,
}

pub fn screen_file(name: &str) -> String {
    format!("ui/{name}.ui")
}

pub fn screens<'a>(
    order: impl IntoIterator<Item = &'a AddonManifest>,
) -> Result<Vec<(&'a AddonManifest, &'a ScreenOp)>> {
    let mut out: Vec<(&AddonManifest, &ScreenOp)> = Vec::new();
    for m in order {
        for s in &m.operations.screens {
            if let Some((other, _)) = out.iter().find(|(_, o)| o.name == s.name) {
                return Err(Error::Other(format!(
                    "screen {} is declared by {} and {}",
                    s.name, other.id, m.id
                )));
            }
            out.push((m, s));
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMap {
    pub code: String,
    pub name: String,
    /// The addon that brings the track package: its ADDON SETTINGS tile shows the track.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub addon: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMod {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionAddon {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub enabled: bool,
    pub locked: bool,
    /// A track addon: its category is Tracks or it brings a track package, as the launcher shows it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub track: bool,
    /// The launcher folder the addon is in, by name; ADDON SETTINGS lists a row per folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default)]
    pub dlls: Vec<String>,
    pub enable_commands: [String; 2],
    #[serde(default)]
    pub options: Vec<SessionOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub career_setup: Option<CareerSetup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hub_tab: Option<HubTab>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mfd: Option<SessionMfd>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMfd {
    pub title: String,
    pub rows: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionOption {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: OptionType,
    pub binds: Binds,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub current: usize,
    pub states: Vec<SessionState>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    pub value: Value,
    pub text: String,
    pub command: String,
}

impl SessionSettings {
    /// The build's track packages, each with the addon that brings it, which is then a track.
    pub fn set_maps<'a>(&mut self, packages: impl Iterator<Item = (&'a TrackPackage, &'a str)>) {
        self.maps = packages
            .map(|(p, addon)| SessionMap {
                code: p.track_code.clone(),
                name: p.track_name.clone().unwrap_or_else(|| p.track_code.clone()),
                addon: addon.to_owned(),
                version: p.version.clone().unwrap_or_default(),
                author: p.author_name.clone().unwrap_or_default(),
                description: p
                    .extra
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
            .collect();
        for a in &mut self.addons {
            a.track |= self.maps.iter().any(|m| m.addon == a.id);
        }
    }

    pub fn set_mods(&mut self, items: impl Iterator<Item = SessionMod>) {
        self.mods = items.collect();
    }

    pub fn build(
        profile: &Profile,
        installed: &BTreeMap<String, AddonManifest>,
        requests: Option<String>,
    ) -> Result<Self> {
        if profile.is_vanilla() {
            return Err(Error::Other(format!(
                "{} plays without addons and has no addon settings",
                profile.name
            )));
        }
        let mut order: Vec<&AddonManifest> = profile
            .addons
            .iter()
            .filter_map(|m| installed.get(&m.id))
            .collect();
        let mut rest: Vec<&AddonManifest> = installed
            .values()
            .filter(|m| !order.iter().any(|o| o.id == m.id))
            .collect();
        rest.sort_by_key(|m| m.name.to_lowercase());
        order.extend(rest);
        let enabled: Vec<&str> = profile.enabled().collect();
        let on = OnAddons::of(profile, installed);
        let screens: Vec<SessionScreen> =
            screens(order.iter().copied().filter(|m| enabled.contains(&m.id.as_str())))?
                .into_iter()
                .map(|(m, s)| SessionScreen {
                    name: s.name.clone(),
                    page: s.page.clone(),
                    addon: s.page.as_ref().map(|_| m.id.clone()),
                    file: screen_file(&s.name),
                    flow: s.flow.clone(),
                    anchor: s.anchor.clone(),
                    view_model: s.view_model,
                })
                .collect();
        let ui = (!screens.is_empty()).then_some(SessionUi {
            register: true,
            strict: false,
            recover: true,
            screens,
        });
        let early_plugins = order
            .iter()
            .filter(|m| enabled.contains(&m.id.as_str()))
            .flat_map(|m| &m.operations.native)
            .filter(|n| n.early)
            .map(|n| n.dll.rsplit('/').next().unwrap_or(&n.dll).to_owned())
            .collect();
        let addons = order
            .into_iter()
            .map(|m| SessionAddon {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.to_string(),
                author: m.author.clone(),
                description: m.description.clone().unwrap_or_default(),
                enabled: enabled.contains(&m.id.as_str()),
                locked: m.is_builtin() || crate::managed::is_managed(&m.id),
                track: m.category == Some(crate::manifest::Category::Tracks),
                folder: None,
                dlls: m
                    .operations
                    .native
                    .iter()
                    .map(|n| n.dll.rsplit('/').next().unwrap_or(&n.dll).to_owned())
                    .collect(),
                enable_commands: [0, 1].map(|b| format!("swofty_enable:{}:{b}", m.id)),
                options: m
                    .options
                    .iter()
                    .map(|o| SessionOption {
                        id: o.id.clone(),
                        kind: o.kind,
                        binds: o.binds,
                        label: o.title().to_owned(),
                        description: o.description.clone().unwrap_or_default(),
                        current: o.index(&on.value(profile, m, o)).unwrap_or(0),
                        states: o
                            .states()
                            .into_iter()
                            .enumerate()
                            .map(|(i, s)| SessionState {
                                value: s.value,
                                text: s.text,
                                command: format!("swofty_set:{}:{}:{i}", m.id, o.id),
                            })
                            .collect(),
                        hidden: o.hidden || on.missing(o).is_some(),
                    })
                    .collect(),
                career_setup: m.career_setup.clone(),
                hub_tab: m.hub_tab.clone(),
                mfd: m.hud.as_ref().and_then(|h| h.mfd.as_ref()).map(|f| SessionMfd {
                    title: f.title.clone().unwrap_or_else(|| m.name.clone()),
                    rows: f.rows.clone(),
                }),
            })
            .collect();
        let out = Self {
            schema: SCHEMA,
            profile: profile.name.clone(),
            requests,
            restart_command: RESTART_COMMAND.into(),
            addons,
            maps: Vec::new(),
            mods: Vec::new(),
            ui,
            owned_items: Vec::new(),
            early_plugins,
            plugins: Vec::new(),
            launcher_saves: None,
        };
        out.check_commands()?;
        Ok(out)
    }

    pub fn commands(&self) -> impl Iterator<Item = &str> {
        let addons = self.addons.iter().flat_map(|m| {
            let options = m
                .options
                .iter()
                .flat_map(|o| o.states.iter().map(|s| s.command.as_str()));
            m.enable_commands.iter().map(String::as_str).chain(options)
        });
        std::iter::once(self.restart_command.as_str()).chain(addons)
    }

    fn check_commands(&self) -> Result<()> {
        let mut seen: HashMap<u32, &str> = HashMap::new();
        for c in self.commands() {
            if let Some(other) = seen.insert(djb2(c), c) {
                return Err(Error::Other(format!(
                    "commands {other} and {c} share the djb2 hash {:08x}; rename an option",
                    djb2(c)
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Requests {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub seq: u64,
    #[serde(default)]
    pub options: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub enabled: BTreeMap<String, bool>,
    #[serde(default)]
    pub relaunch: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Merge {
    pub changed: Vec<String>,
    pub skipped: Vec<String>,
    pub relaunch: bool,
}

pub fn merge(profile: &mut Profile, installed: &BTreeMap<String, AddonManifest>, req: &Requests) -> Merge {
    let mut out = Merge {
        relaunch: req.relaunch,
        ..Merge::default()
    };
    for (addon_id, values) in &req.options {
        let Some(m) = installed.get(addon_id) else {
            out.skipped.push(format!("{addon_id} is not installed"));
            continue;
        };
        for (id, v) in values {
            match profile.set_option(m, id, v.clone()) {
                Ok(true) => {
                    let text = m.options.iter().find(|o| &o.id == id).map(|o| o.text(v));
                    out.changed
                        .push(format!("{addon_id}.{id} = {}", text.unwrap_or_default()));
                }
                Ok(false) => {}
                Err(e) => out.skipped.push(e.to_string()),
            }
        }
    }
    for (addon_id, &on) in &req.enabled {
        if !installed.contains_key(addon_id) {
            out.skipped.push(format!("{addon_id} is not installed"));
            continue;
        }
        if profile.enabled().any(|e| e == addon_id) == on {
            continue;
        }
        match profile.set_enabled(addon_id, on) {
            Ok(()) => out
                .changed
                .push(format!("{addon_id} {}", if on { "enabled" } else { "disabled" })),
            Err(e) => out.skipped.push(e.to_string()),
        }
    }
    out
}

pub fn render_ops(doc: &mut Value, m: &AddonManifest, profile: &Profile, on: &OnAddons) -> Result<()> {
    let fail = |msg: String| Error::Resolve(format!("{}: {msg}", m.id));
    let option = |id: &str| {
        m.options
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| fail(format!("unknown option {id}")))
    };
    let holds = |when: Value| -> Result<bool> {
        let Value::Object(when) = when else {
            return Err(fail("when must be an object of option values and addon".into()));
        };
        let mut hit = true;
        for (id, want) in &when {
            if id == "addon" {
                let needs = AddonNeed::parse_list(want).map_err(|e| fail(format!("when.addon: {e}")))?;
                hit &= needs.iter().all(|n| on.has(n));
                continue;
            }
            let o = option(id)?;
            let want = o
                .normalise(want)
                .ok_or_else(|| fail(format!("when: {want} is not a value of {id}")))?;
            hit &= on.value(profile, m, o) == want;
        }
        Ok(hit)
    };
    let Some(doc) = doc.as_object_mut() else {
        return Ok(());
    };
    let file = doc.remove("when").map(holds).transpose()?.unwrap_or(true);
    let Some(ops) = doc.get_mut("ops").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    if !file {
        ops.clear();
        return Ok(());
    }
    let mut keep = Vec::with_capacity(ops.len());
    for mut op in ops.drain(..) {
        if let Some(when) = op.as_object_mut().and_then(|o| o.remove("when"))
            && !holds(when)?
        {
            continue;
        }
        substitute(&mut op, &|id| {
            let o = option(id)?;
            Ok(o.placeholder(&on.value(profile, m, o)))
        })?;
        keep.push(op);
    }
    *ops = keep;
    Ok(())
}

/// One manifest string with its `{{id}}` placeholders filled from the profile's option values.
pub fn render_text(text: &str, m: &AddonManifest, profile: &Profile, on: &OnAddons) -> Result<String> {
    let mut v = Value::String(text.to_owned());
    substitute(&mut v, &|id| {
        let o = m
            .options
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| Error::Resolve(format!("{}: unknown option {id}", m.id)))?;
        Ok(o.placeholder(&on.value(profile, m, o)))
    })?;
    Ok(v.as_str().expect("a string stays a string").to_owned())
}

fn substitute(v: &mut Value, lookup: &dyn Fn(&str) -> Result<String>) -> Result<()> {
    match v {
        Value::String(s) if s.contains("{{") => {
            let mut out = String::with_capacity(s.len());
            let mut rest = s.as_str();
            while let Some(start) = rest.find("{{") {
                let end = rest[start..]
                    .find("}}")
                    .ok_or_else(|| Error::Resolve(format!("unclosed {{{{ in {s}")))?;
                out.push_str(&rest[..start]);
                out.push_str(&lookup(rest[start + 2..start + end].trim())?);
                rest = &rest[start + end + 2..];
            }
            out.push_str(rest);
            *s = out;
        }
        Value::Array(a) => a.iter_mut().try_for_each(|x| substitute(x, lookup))?,
        Value::Object(o) => o.values_mut().try_for_each(|x| substitute(x, lookup))?,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AddonManifest;

    fn manifest() -> AddonManifest {
        AddonManifest::parse(
            r#"{
              "schema_version": 1, "id": "a.grid", "name": "Grid", "version": "1.0.0", "author": "a",
              "compat": { "game": "f1_25" },
              "options": [
                { "id": "livery", "type": "choice", "default": "title", "binds": "db",
                  "values": ["title", { "value": "tier01", "label": "Tier 1" }, "replica"] },
                { "id": "audi", "type": "toggle", "default": false, "binds": "db" },
                { "id": "offset", "type": "slider", "min": -1, "max": 1, "step": 0.25, "default": 0, "binds": "native" },
                { "id": "count", "type": "slider", "min": 0, "max": 10, "step": 2, "default": 4, "binds": "native" }
              ],
              "operations": { "native": [{ "dll": "native/grid.dll" }] }
            }"#,
        )
        .unwrap()
    }

    fn installed() -> BTreeMap<String, AddonManifest> {
        let mut m = BTreeMap::from([(
            crate::builtin::CORE_ID.to_owned(),
            crate::builtin::core_manifest(),
        )]);
        m.insert("a.grid".into(), manifest());
        m
    }

    #[test]
    fn lists_the_early_plugins_of_enabled_addons() {
        let mut installed = installed();
        let early = AddonManifest::parse(
            r#"{
              "schema_version": 1, "id": "a.early", "name": "Early", "version": "1.0.0", "author": "a",
              "compat": { "game": "f1_25" },
              "operations": { "native": [
                { "dll": "native/early.dll", "capabilities": ["assets"], "early": true },
                { "dll": "native/late.dll", "capabilities": ["log"] }
              ] }
            }"#,
        )
        .unwrap();
        installed.insert("a.early".into(), early);
        let mut p = Profile::new("x");
        let s = SessionSettings::build(&p, &installed, None).unwrap();
        assert!(s.early_plugins.is_empty());
        assert!(serde_json::to_value(&s).unwrap().get("early_plugins").is_none());
        p.set_enabled("a.early", true).unwrap();
        let s = SessionSettings::build(&p, &installed, None).unwrap();
        assert_eq!(s.early_plugins, ["early.dll"]);
        assert_eq!(
            serde_json::to_value(&s).unwrap()["early_plugins"],
            serde_json::json!(["early.dll"])
        );
    }

    #[test]
    fn djb2_matches_the_game() {
        assert_eq!(djb2("swofty_set"), 0x5fdf279c);
        assert_eq!(djb2("swofty_settings"), 0x90c2d901);
        assert_eq!(djb2(""), 0x1505);
    }

    #[test]
    fn slider_grids_and_text() {
        let m = manifest();
        let offset = &m.options[2];
        let states = offset.states();
        assert_eq!(states.len(), 9);
        assert_eq!(states[0].text, "-1.00");
        assert_eq!(states[5].value, serde_json::json!(0.25));
        assert_eq!(offset.normalise(&serde_json::json!(0.26)), None);
        assert_eq!(
            offset.normalise(&serde_json::json!(0.5000000001)),
            Some(serde_json::json!(0.5))
        );
        let count = &m.options[3];
        assert_eq!(
            count.states().iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
            ["0", "2", "4", "6", "8", "10"]
        );
        assert_eq!(
            count.normalise(&serde_json::json!(6.0)),
            Some(serde_json::json!(6))
        );
        assert_eq!(count.text(&serde_json::json!(8)), "8");
        assert_eq!(m.options[0].text(&serde_json::json!("tier01")), "Tier 1");
    }

    #[test]
    fn rejects_bad_options() {
        let base = |opt: &str| {
            format!(
                r#"{{"schema_version":1,"id":"a.b","name":"B","version":"1.0.0","author":"a",
                "compat":{{"game":"f1_25"}},"options":[{opt}]}}"#
            )
        };
        for bad in [
            r#"{"id":"Bad","type":"toggle","default":true,"binds":"native"}"#,
            r#"{"id":"t","type":"toggle","default":1,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":10,"default":0,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":10,"step":3,"default":0,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":1000,"step":1,"default":0,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":10,"step":1,"default":0.5,"binds":"native"}"#,
            r#"{"id":"c","type":"choice","values":[],"default":"a","binds":"db"}"#,
            r#"{"id":"c","type":"choice","values":["a","a"],"default":"a","binds":"db"}"#,
            r#"{"id":"c","type":"choice","values":["a"],"default":"b","binds":"db"}"#,
            r#"{"id":"c","type":"string","default":"b","binds":"db"}"#,
        ] {
            assert!(
                AddonManifest::parse(&base(bad)).is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn session_file_lists_built_maps_in_order() {
        let p = Profile::new("x");
        let pkg = |code: &str, name: &str| {
            TrackPackage::parse(
                format!(
                    r#"{{"track_code":"{code}","track_name":"{name}","version":"1.0.0","author_name":"a","erp":"{code}.erp","description":"d","activation":{{"copy_from_track":"melbourne"}}}}"#
                )
                .as_bytes(),
            )
            .unwrap()
        };
        let packages = [pkg("okayama", "Okayama"), pkg("bahrain24", "Sepang")];
        let mut s = SessionSettings::build(&p, &installed(), None).unwrap();
        s.set_maps(packages.iter().map(|p| (p, "")));
        let codes: Vec<&str> = s.maps.iter().map(|m| m.code.as_str()).collect();
        assert_eq!(codes, ["okayama", "bahrain24"]);
        assert_eq!(s.maps[1].name, "Sepang");
        assert_eq!(
            (s.maps[0].author.as_str(), s.maps[0].description.as_str()),
            ("a", "d")
        );
    }

    #[test]
    fn session_file_lists_every_addon_with_states_and_commands() {
        let mut p = Profile::new("x");
        let m = manifest();
        p.set_option(&m, "livery", serde_json::json!("replica")).unwrap();
        let s = SessionSettings::build(&p, &installed(), Some("Z:\\r.json".into())).unwrap();
        assert_eq!(s.addons[0].id, crate::builtin::CORE_ID);
        assert!(s.addons[0].locked && s.addons[0].enabled);
        let grid = &s.addons[1];
        assert!(!grid.enabled && !grid.locked);
        assert_eq!(grid.author, "a");
        assert_eq!(grid.dlls, ["grid.dll"]);
        assert_eq!(grid.enable_commands[1], "swofty_enable:a.grid:1");
        assert_eq!(grid.options[0].current, 2);
        assert_eq!(grid.options[0].states[2].command, "swofty_set:a.grid:livery:2");
        assert_eq!(grid.options[2].current, 4);
        assert!(s.commands().any(|c| c == RESTART_COMMAND));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["addons"][1]["options"][0]["type"], "choice");
        assert!(json.get("mods").is_none());
        assert_eq!(serde_json::from_value::<SessionSettings>(json).unwrap(), s);
        assert!(SessionSettings::build(&Profile::vanilla(), &installed(), None).is_err());
    }

    #[test]
    fn mods_list_serialises_under_its_own_key_and_round_trips() {
        let mut s = SessionSettings::build(&Profile::new("x"), &installed(), None).unwrap();
        assert!(serde_json::to_value(&s).unwrap().get("mods").is_none());
        s.set_mods(
            [SessionMod {
                id: "ts.sauber".into(),
                name: "Sauber 2026".into(),
                version: "1.2.0".into(),
                author: "Team Simplified".into(),
                description: String::new(),
            }]
            .into_iter(),
        );
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["mods"][0]["id"], "ts.sauber");
        assert_eq!(json["mods"][0]["name"], "Sauber 2026");
        assert_eq!(serde_json::from_value::<SessionSettings>(json).unwrap(), s);
    }

    #[test]
    fn managed_addon_is_locked_in_game() {
        let mut installed = installed();
        let managed = AddonManifest::parse(
            r#"{"schema_version":1,"id":"teamsimplified.mods","name":"Simplified for Swofty",
                "version":"0.5.0","author":"Team Simplified","compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        installed.insert("teamsimplified.mods".into(), managed);
        let mut p = Profile::new("x");
        p.set_enabled("teamsimplified.mods", true).unwrap();
        let s = SessionSettings::build(&p, &installed, None).unwrap();
        let a = s.addons.iter().find(|a| a.id == "teamsimplified.mods").unwrap();
        assert!(a.locked && a.enabled);
        let grid = s.addons.iter().find(|a| a.id == "a.grid").unwrap();
        assert!(!grid.locked);
    }

    fn screens_manifest(id: &str, screens: &str) -> AddonManifest {
        AddonManifest::parse(&format!(
            r#"{{ "schema_version": 1, "id": "{id}", "name": "S", "version": "1.0.0", "author": "a",
                 "compat": {{ "game": "f1_25" }}, "operations": {{ "screens": [{screens}] }} }}"#
        ))
        .unwrap()
    }

    #[test]
    fn session_file_lists_enabled_screens_and_omits_an_empty_ui_block() {
        let mut addons = installed();
        addons.insert(
            "a.pages".into(),
            screens_manifest(
                "a.pages",
                r#"{ "name": "swofty_addon_0", "file": "ui/page0.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": true },
                   { "name": "swofty_addon_1", "file": "ui/page1.ui", "flow": "flow_options" }"#,
            ),
        );
        addons.insert(
            "b.off".into(),
            screens_manifest("b.off", r#"{ "name": "off_page", "file": "ui/off.ui" }"#),
        );
        let mut p = Profile::new("x");
        let none = SessionSettings::build(&p, &addons, None).unwrap();
        assert_eq!(none.ui, None);
        assert!(serde_json::to_value(&none).unwrap().get("ui").is_none());
        p.set_enabled("a.pages", true).unwrap();
        let s = SessionSettings::build(&p, &addons, None).unwrap();
        let ui = s.ui.as_ref().unwrap();
        assert!(ui.register && !ui.strict && ui.recover);
        let names: Vec<&str> = ui.screens.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["swofty_addon_0", "swofty_addon_1"]);
        assert_eq!(ui.screens[0].file, "ui/swofty_addon_0.ui");
        assert_eq!(ui.screens[0].anchor.as_deref(), Some("settings_container"));
        assert!(ui.screens[0].view_model && !ui.screens[1].view_model);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["ui"]["screens"][1]["flow"], "flow_options");
        assert!(json["ui"]["screens"][1].get("anchor").is_none());
        let back: SessionSettings = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
        addons.insert(
            "c.clash".into(),
            screens_manifest("c.clash", r#"{ "name": "swofty_addon_0", "file": "ui/x.ui" }"#),
        );
        p.set_enabled("c.clash", true).unwrap();
        assert!(SessionSettings::build(&p, &addons, None).is_err());
    }

    #[test]
    fn merges_requests_and_tracks_rebuilds() {
        let addons = installed();
        let mut p = Profile::new("x");
        p.set_enabled("a.grid", true).unwrap();
        assert!(p.rebuild_pending(&addons).is_empty());
        let req: Requests = serde_json::from_str(
            r#"{"schema":1,"profile":"x","seq":2,
                "options":{"a.grid":{"livery":"tier01","offset":0.5,"nope":1},"z.missing":{"a":1}},
                "enabled":{"a.grid":false,"swofty.core":false},"relaunch":true}"#,
        )
        .unwrap();
        let out = merge(&mut p, &addons, &req);
        assert_eq!(
            out.changed,
            [
                "a.grid.livery = Tier 1",
                "a.grid.offset = 0.50",
                "a.grid disabled"
            ]
        );
        assert_eq!(out.skipped.len(), 3);
        assert!(out.relaunch);
        assert_eq!(merge(&mut p, &addons, &req).changed, Vec::<String>::new());
        assert!(p.rebuild_pending(&addons).is_empty());
        p.set_enabled("a.grid", true).unwrap();
        assert_eq!(p.rebuild_pending(&addons), ["a.grid.livery"]);
        p.mark_built(&addons);
        assert!(p.rebuild_pending(&addons).is_empty());
        p.set_option(&addons["a.grid"], "livery", serde_json::json!("title"))
            .unwrap();
        assert_eq!(p.rebuild_pending(&addons), ["a.grid.livery"]);
        assert!(!p.options.contains_key("a.grid") || !p.options["a.grid"].contains_key("livery"));
    }

    #[test]
    fn renders_database_ops_from_options() {
        let m = manifest();
        let mut p = Profile::new("x");
        p.set_option(&m, "count", serde_json::json!(8)).unwrap();
        let mut doc = serde_json::json!({"schema_version": 1, "ops": [
            {"op": "set", "select": "t.r", "field": "n", "value": "{{count}}"},
            {"op": "set", "select": "t.r", "field": "l", "value": "{{ livery }}-{{audi}}", "when": {"livery": "title"}},
            {"op": "set", "select": "t.r", "field": "a", "value": "1", "when": {"audi": true}}
        ]});
        render_ops(&mut doc, &m, &p, &OnAddons::default()).unwrap();
        let ops = doc["ops"].as_array().unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0]["value"], "8");
        assert_eq!(ops[1]["value"], "title-false");
        assert!(ops[1].get("when").is_none());
        let mut bad = serde_json::json!({"ops": [{"op": "set", "value": "{{missing}}"}]});
        assert!(render_ops(&mut bad, &m, &p, &OnAddons::default()).is_err());
        let mut bad = serde_json::json!({"ops": [{"op": "set", "value": "x", "when": {"livery": "nope"}}]});
        assert!(render_ops(&mut bad, &m, &p, &OnAddons::default()).is_err());
    }

    #[test]
    fn renders_material_values_from_options() {
        use crate::asset::{AssetOp, FileKind, OpFile};
        use crate::ops::ErpOp;
        let m = AddonManifest::parse(
            r#"{
              "schema_version": 1, "id": "a.paint", "name": "Paint", "version": "1.0.0", "author": "a",
              "compat": { "game": "f1_25" },
              "options": [
                { "id": "coat", "type": "choice", "default": "1 1 1", "binds": "db",
                  "values": [{ "value": "1 1 1", "label": "Clear" }, { "value": "0, 1, 0", "label": "Green" }] },
                { "id": "flakes", "type": "slider", "min": 0, "max": 1, "step": 0.25, "default": 0, "binds": "db" },
                { "id": "matte", "type": "toggle", "default": false, "binds": "db" }
              ]
            }"#,
        )
        .unwrap();
        let mut p = Profile::new("x");
        p.set_option(&m, "coat", serde_json::json!("0, 1, 0")).unwrap();
        p.set_option(&m, "flakes", serde_json::json!(0.75)).unwrap();
        p.set_option(&m, "matte", serde_json::json!(true)).unwrap();
        let mut doc = serde_json::json!({"schema_version": 1, "ops": [{
            "op": "set-material", "erp": "a.erp", "resource": "eaid://a/paint.material",
            "params": {
                "me_lacquerColour": "{{coat}}",
                "me_flakesStrength": "{{flakes}}",
                "me_flakesColourInfluence": ["{{flakes}}", 0, "{{matte}}"],
                "me_lacquerRoughness": 0.1
            }
        }]});
        render_ops(&mut doc, &m, &p, &OnAddons::default()).unwrap();
        let file = OpFile::from_value(doc, FileKind::Erp).unwrap();
        let AssetOp::Erp(ErpOp::SetMaterial { params, .. }) = &file.ops[0] else {
            panic!("{:?}", file.ops[0]);
        };
        let values = |k: &str| params[k].values().unwrap();
        assert_eq!(values("me_lacquerColour"), [0.0, 1.0, 0.0]);
        assert_eq!(values("me_flakesStrength"), [0.75]);
        assert_eq!(values("me_flakesColourInfluence"), [0.75, 0.0, 1.0]);
        assert_eq!(values("me_lacquerRoughness"), [0.1]);
        p.set_option(&m, "coat", serde_json::json!("1 1 1")).unwrap();
        let mut doc = serde_json::json!({"schema_version": 1, "ops": [{"op": "set-material", "erp": "a.erp", "resource": "r",
            "params": {"me_x": "{{coat}} x"}}]});
        render_ops(&mut doc, &m, &p, &OnAddons::default()).unwrap();
        let file = OpFile::from_value(doc, FileKind::Erp).unwrap();
        let AssetOp::Erp(ErpOp::SetMaterial { params, .. }) = &file.ops[0] else {
            panic!();
        };
        assert_eq!(params["me_x"].values().unwrap_err(), "\"x\" is not a number");
    }

    fn addon(id: &str, version: &str) -> AddonManifest {
        AddonManifest::parse(&format!(
            r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "{version}", "author": "a",
                 "compat": {{ "game": "f1_25" }} }}"#
        ))
        .unwrap()
    }

    fn rounds() -> AddonManifest {
        AddonManifest::parse(
            r#"{
              "schema_version": 1, "id": "miguel.f2-revamp", "name": "F2 Revamp", "version": "1.0.0", "author": "a",
              "compat": { "game": "f1_25" },
              "options": [
                { "id": "sprint", "type": "toggle", "default": false, "binds": "db" },
                { "id": "istanbul_park", "type": "toggle", "default": false, "binds": "db",
                  "needs": ["track.istanbul_park"] },
                { "id": "imola", "type": "toggle", "default": true, "binds": "native",
                  "needs": [{ "id": "track.imola", "version": ">=2.0.0" }] }
              ],
              "operations": { "database": ["db/rounds.json"] }
            }"#,
        )
        .unwrap()
    }

    fn kept(doc: &Value) -> Vec<&str> {
        doc["ops"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["field"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn an_op_with_when_addon_applies_only_while_that_addon_is_on() {
        let m = rounds();
        let mut p = Profile::new("x");
        let doc = serde_json::json!({"schema_version": 1, "ops": [
            {"op": "set", "select": "t.r", "field": "always", "value": "1"},
            {"op": "set", "select": "t.r", "field": "round", "value": "1", "when": {"addon": "track.istanbul_park"}},
            {"op": "set", "select": "t.r", "field": "sprint", "value": "1",
             "when": {"addon": "track.istanbul_park", "sprint": true}},
            {"op": "set", "select": "t.r", "field": "v2", "value": "1",
             "when": {"addon": {"id": "track.istanbul_park", "version": ">=2.0.0"}}},
            {"op": "set", "select": "t.r", "field": "both", "value": "1",
             "when": {"addon": ["track.istanbul_park", "track.imola"]}}
        ]});
        let render = |p: &Profile, on: &[AddonManifest]| {
            let mut doc = doc.clone();
            render_ops(&mut doc, &m, p, &OnAddons::new(on)).unwrap();
            doc
        };
        assert_eq!(kept(&render(&p, &[])), ["always"]);
        let istanbul = addon("track.istanbul_park", "1.4.0");
        let out = render(&p, std::slice::from_ref(&istanbul));
        assert_eq!(kept(&out), ["always", "round"]);
        assert!(out["ops"][1].get("when").is_none());
        p.set_option(&m, "sprint", serde_json::json!(true)).unwrap();
        assert_eq!(kept(&render(&p, &[])), ["always"]);
        assert_eq!(
            kept(&render(&p, std::slice::from_ref(&istanbul))),
            ["always", "round", "sprint"]
        );
        let on = [
            addon("track.istanbul_park", "2.1.0"),
            addon("track.imola", "1.0.0"),
        ];
        assert_eq!(
            kept(&render(&p, &on)),
            ["always", "round", "sprint", "v2", "both"]
        );
    }

    #[test]
    fn a_whole_op_file_applies_only_while_its_when_holds() {
        let m = rounds();
        let mut p = Profile::new("x");
        let doc = serde_json::json!({"schema_version": 1, "when": {"addon": "track.istanbul_park", "sprint": true},
        "ops": [
            {"op": "set", "select": "t.r", "field": "round", "value": "1"},
            {"op": "set", "select": "t.r", "field": "laps", "value": "1", "when": {"addon": "track.imola"}}
        ]});
        let istanbul = [addon("track.istanbul_park", "1.0.0")];
        let mut off = doc.clone();
        render_ops(&mut off, &m, &p, &OnAddons::new(&istanbul)).unwrap();
        assert!(kept(&off).is_empty());
        assert!(off.get("when").is_none());
        p.set_option(&m, "sprint", serde_json::json!(true)).unwrap();
        let mut on = doc.clone();
        render_ops(&mut on, &m, &p, &OnAddons::new(&istanbul)).unwrap();
        assert_eq!(kept(&on), ["round"]);
        let file = crate::asset::OpFile::from_value(on, crate::asset::FileKind::Database).unwrap();
        assert_eq!(file.ops.len(), 1);
    }

    #[test]
    fn a_bad_when_addon_names_the_addon_and_the_problem() {
        let m = rounds();
        let p = Profile::new("x");
        let err = |when: Value| {
            let mut doc = serde_json::json!({"ops": [{"op": "set", "value": "1", "when": when}]});
            render_ops(&mut doc, &m, &p, &OnAddons::default())
                .unwrap_err()
                .to_string()
        };
        assert!(
            err(serde_json::json!({"addon": "Istanbul Park"}))
                .contains(r#"miguel.f2-revamp: when.addon: "Istanbul Park" is not an addon id"#)
        );
        assert!(
            err(serde_json::json!({"addon": {"id": "track.istanbul_park", "versoin": "2"}}))
                .contains(r#"when.addon: "versoin" is not a key of an addon"#)
        );
        assert!(err(serde_json::json!({"addon": []})).contains("when.addon: lists no addon"));
        assert!(err(serde_json::json!({"adon": "track.istanbul_park"})).contains("unknown option adon"));
        let mut file = serde_json::json!({"when": {"addon": 4}, "ops": []});
        let e = render_ops(&mut file, &m, &p, &OnAddons::default())
            .unwrap_err()
            .to_string();
        assert!(e.contains("when.addon: 4 is not an addon id"), "{e}");
    }

    #[test]
    fn an_option_that_needs_an_addon_is_off_while_it_is_off_and_keeps_its_value() {
        let m = rounds();
        let istanbul = &m.options[1];
        let mut p = Profile::new("x");
        p.set_option(&m, "istanbul_park", serde_json::json!(true))
            .unwrap();
        let none = OnAddons::default();
        assert_eq!(none.missing(istanbul).unwrap().to_string(), "track.istanbul_park");
        assert_eq!(none.value(&p, &m, istanbul), false);
        assert_eq!(p.option(&m, istanbul), true);
        let on = OnAddons::new(&[addon("track.istanbul_park", "1.0.0")]);
        assert!(on.missing(istanbul).is_none());
        assert_eq!(on.value(&p, &m, istanbul), true);
        let doc = serde_json::json!({"ops": [
            {"op": "set", "select": "t.r", "field": "round", "value": "{{istanbul_park}}"},
            {"op": "set", "select": "t.r", "field": "only", "value": "1", "when": {"istanbul_park": true}}
        ]});
        let mut off = doc.clone();
        render_ops(&mut off, &m, &p, &none).unwrap();
        assert_eq!(kept(&off), ["round"]);
        assert_eq!(off["ops"][0]["value"], "false");
        let mut back = doc.clone();
        render_ops(&mut back, &m, &p, &on).unwrap();
        assert_eq!(kept(&back), ["round", "only"]);
        assert_eq!(back["ops"][0]["value"], "true");
        assert_eq!(render_text("{{istanbul_park}}", &m, &p, &none).unwrap(), "false");
    }

    #[test]
    fn a_toggle_on_by_default_that_needs_an_addon_follows_it() {
        let m = rounds();
        let imola = &m.options[2];
        let p = Profile::new("x");
        assert_eq!(OnAddons::default().value(&p, &m, imola), false);
        assert_eq!(
            OnAddons::new(&[addon("track.imola", "1.9.0")]).value(&p, &m, imola),
            false
        );
        assert_eq!(
            OnAddons::new(&[addon("track.imola", "1.9.0")])
                .missing(imola)
                .unwrap()
                .to_string(),
            "track.imola >=2.0.0"
        );
        assert_eq!(
            OnAddons::new(&[addon("track.imola", "2.0.0")]).value(&p, &m, imola),
            true
        );
    }

    #[test]
    fn the_session_file_shows_an_option_off_and_hidden_while_an_addon_it_needs_is_off() {
        let mut installed = installed();
        installed.insert("miguel.f2-revamp".into(), rounds());
        installed.insert(
            "track.istanbul_park".into(),
            addon("track.istanbul_park", "1.0.0"),
        );
        let m = &installed["miguel.f2-revamp"];
        let mut p = Profile::new("x");
        p.set_enabled("miguel.f2-revamp", true).unwrap();
        p.set_option(m, "istanbul_park", serde_json::json!(true)).unwrap();
        let option = |s: &SessionSettings| {
            let a = s.addons.iter().find(|a| a.id == "miguel.f2-revamp").unwrap();
            a.options
                .iter()
                .find(|o| o.id == "istanbul_park")
                .unwrap()
                .clone()
        };
        let off = option(&SessionSettings::build(&p, &installed, None).unwrap());
        assert_eq!((off.current, off.hidden), (0, true));
        p.set_enabled("track.istanbul_park", true).unwrap();
        let on = option(&SessionSettings::build(&p, &installed, None).unwrap());
        assert_eq!((on.current, on.hidden), (1, false));
    }
}
