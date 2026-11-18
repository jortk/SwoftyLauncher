//! Shader configs and effects in the store (DESIGN.md §1, §5, §6): personal configs in
//! `<data>/shaders/`, addon configs and effects from installed addons, the active config per profile,
//! and the files the game reads.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use sha2::{Digest, Sha256};
pub use swofty_shaders::{Active, ActiveInstance, Catalog, CompiledEffect, Effect, Target};
use swofty_shaders::{Origin, Severity};

use crate::builtin::CORE_ID;
use crate::profile::Profile;
use crate::store::Store;
use crate::{Error, Result, io_err};

mod table {
    include!(concat!(env!("OUT_DIR"), "/shader_library.rs"));
}

const LIBRARY_EFFECTS: &str = "effects/";
const STARTER: &str = "starter.swfx";
const SETTINGS: &str = "settings.json";
const DEFAULT_KEY: &str = "B";
const NO_KEY: &str = "None";
const EXTENSION: &str = "swfx";

/// The files of the core's effect library, by path under `addons/swoftymod/shaders/`: the
/// `effects/<id>.effect.json` declarations with their HLSL and images, and `starter.swfx`.
pub fn library() -> &'static [(&'static str, &'static [u8])] {
    table::FILES
}

/// The text "New config" starts from.
pub fn starter() -> Option<&'static str> {
    library()
        .iter()
        .find(|(path, _)| *path == STARTER)
        .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConfigRef {
    /// `personal:<slug>`: `<data>/shaders/<slug>.swfx`.
    Personal(String),
    /// `addon:<id>/<file stem>`.
    Addon { addon: String, file: String },
}

impl fmt::Display for ConfigRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Personal(slug) => write!(f, "personal:{slug}"),
            Self::Addon { addon, file } => write!(f, "addon:{addon}/{file}"),
        }
    }
}

impl FromStr for ConfigRef {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(slug) = s.strip_prefix("personal:") {
            return Ok(Self::Personal(slug.into()));
        }
        if let Some((addon, file)) = s.strip_prefix("addon:").and_then(|r| r.split_once('/')) {
            return Ok(Self::Addon {
                addon: addon.into(),
                file: file.into(),
            });
        }
        Err(Error::Other(format!("{s:?} is not a shader config")))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    pub r#ref: ConfigRef,
    pub name: String,
    /// "Personal", or the addon's name.
    pub source: String,
    pub editable: bool,
    pub path: PathBuf,
}

/// Where a catalog effect's HLSL and images live.
#[derive(Debug, Clone)]
enum Home {
    Library,
    Addon(PathBuf),
}

struct Loaded {
    catalog: Catalog,
    homes: BTreeMap<String, Home>,
    warnings: Vec<String>,
}

/// Installed addons that ship shader configs or effects, in load order: the profile's order, then
/// the rest by id.
fn shader_addons(store: &Store, profile: Option<&Profile>) -> Vec<swofty_addon::manifest::AddonManifest> {
    let Ok(mut all) = store.addons() else {
        return Vec::new();
    };
    all.remove(CORE_ID);
    all.retain(|id, _| !store.is_builtin(id));
    let mut out = Vec::new();
    if let Some(p) = profile {
        for a in &p.addons {
            if let Some(m) = all.remove(&a.id) {
                out.push(m);
            }
        }
    }
    out.extend(all.into_values());
    out.retain(|m| !m.shaders.is_empty() || !m.effects.is_empty());
    out
}

fn active_profile(store: &Store) -> Option<Profile> {
    store.profile(&store.active_profile()).ok()
}

fn load(store: &Store) -> Loaded {
    let mut out = Loaded {
        catalog: Catalog::default(),
        homes: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let add = |out: &mut Loaded, what: String, json: &str, origin: Origin, home: Home| {
        match swofty_shaders::parse_effect(json, origin) {
            Ok(effect) => {
                if let Some(first) = out.catalog.get(&effect.id) {
                    out.warnings.push(format!(
                        "{what}: the effect {} is ignored; {} already provides it",
                        effect.id,
                        match &first.origin {
                            Origin::Library => "the library".to_owned(),
                            Origin::Addon { name, .. } => name.clone(),
                        }
                    ));
                    return;
                }
                out.warnings
                    .extend(effect.warnings.iter().map(|w| format!("{what}: {w}")));
                out.homes.insert(effect.id.clone(), home);
                out.catalog.effects.push(effect);
            }
            Err(e) => out.warnings.push(format!("{what}: {e}")),
        }
    };
    for (path, bytes) in library() {
        if path.starts_with(LIBRARY_EFFECTS) && path.ends_with(".effect.json") {
            match std::str::from_utf8(bytes) {
                Ok(json) => add(
                    &mut out,
                    format!("library {path}"),
                    json,
                    Origin::Library,
                    Home::Library,
                ),
                Err(_) => out.warnings.push(format!("library {path}: not UTF-8")),
            }
        }
    }
    let profile = active_profile(store);
    for m in shader_addons(store, profile.as_ref()) {
        let root = store.addons_dir().join(&m.id);
        for rel in &m.effects {
            let path = root.join(rel);
            let what = format!("{}: {rel}", m.name);
            match std::fs::read_to_string(&path) {
                Ok(json) => {
                    let origin = Origin::Addon {
                        id: m.id.clone(),
                        name: m.name.clone(),
                    };
                    let dir = path.parent().unwrap_or(&root).to_path_buf();
                    add(&mut out, what, &json, origin, Home::Addon(dir));
                }
                Err(e) => out.warnings.push(format!("{what}: {e}")),
            }
        }
    }
    out
}

/// The library's effects and every installed addon's, whether the addon is on or off.
pub fn catalog(store: &Store) -> Catalog {
    load(store).catalog
}

fn escape(name: &str) -> String {
    let mut out = String::from('"');
    for c in name.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn unescape(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('\'') {
        return Some(rest[..rest.find('\'')?].to_owned());
    }
    let mut chars = value.strip_prefix('"')?.chars();
    let mut out = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => out.push(match chars.next()? {
                'n' => '\n',
                't' => '\t',
                c => c,
            }),
            c => out.push(c),
        }
    }
}

/// The top-level `name = "…"` line: its byte range without the line break, and the name.
fn name_line(text: &str) -> Option<(Range<usize>, String)> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let t = body.trim_start();
        if t.starts_with('[') {
            return None;
        }
        if let Some(rest) = t.strip_prefix("name")
            && let Some(value) = rest.trim_start().strip_prefix('=')
            && let Some(name) = unescape(value)
        {
            return Some((at..at + body.len(), name));
        }
        at += line.len();
    }
    None
}

fn name_of(text: &str, fallback: &str) -> String {
    name_line(text)
        .map(|(_, n)| n)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

/// `text` with its `name` set; only that line changes.
fn with_name(text: &str, name: &str) -> String {
    let line = format!("name = {}", escape(name));
    match name_line(text) {
        Some((range, _)) => format!("{}{line}{}", &text[..range.start], &text[range.end..]),
        None => format!("{line}\n{text}"),
    }
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "config".into()
    } else {
        out.chars().take(48).collect()
    }
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn personal_path(store: &Store, slug: &str) -> PathBuf {
    store.shaders_dir().join(format!("{slug}.{EXTENSION}"))
}

fn unique_slug(store: &Store, base: &str) -> String {
    let mut candidate = base.to_owned();
    let mut n = 2;
    while personal_path(store, &candidate).exists() {
        candidate = format!("{base}-{n}");
        n += 1;
    }
    candidate
}

struct Personal {
    slug: String,
    name: String,
    path: PathBuf,
}

fn personal(store: &Store) -> Vec<Personal> {
    let mut out: Vec<Personal> = std::fs::read_dir(store.shaders_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let slug = path.file_name()?.to_str()?.strip_suffix(".swfx")?.to_owned();
            valid_slug(&slug).then(|| {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                Personal {
                    name: name_of(&text, &slug),
                    slug,
                    path,
                }
            })
        })
        .collect();
    out.sort_by_key(|p| (p.name.to_lowercase(), p.slug.clone()));
    out
}

fn name_taken(store: &Store, name: &str) -> bool {
    personal(store).iter().any(|p| p.name.eq_ignore_ascii_case(name))
}

fn free_name(store: &Store, name: &str) -> String {
    let mut candidate = name.to_owned();
    let mut n = 2;
    while name_taken(store, &candidate) {
        candidate = format!("{name} ({n})");
        n += 1;
    }
    candidate
}

/// Personal configs first (by name), then addon configs in load order.
pub fn list(store: &Store) -> Vec<ConfigEntry> {
    let mut out: Vec<ConfigEntry> = personal(store)
        .into_iter()
        .map(|p| ConfigEntry {
            r#ref: ConfigRef::Personal(p.slug),
            name: p.name,
            source: "Personal".into(),
            editable: true,
            path: p.path,
        })
        .collect();
    let profile = active_profile(store);
    for m in shader_addons(store, profile.as_ref()) {
        for decl in &m.shaders {
            let path = store.addons_dir().join(&m.id).join(&decl.config);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            out.push(ConfigEntry {
                r#ref: ConfigRef::Addon {
                    addon: m.id.clone(),
                    file: decl.stem().to_owned(),
                },
                name: name_of(&text, decl.stem()),
                source: m.name.clone(),
                editable: false,
                path,
            });
        }
    }
    out
}

fn entry(store: &Store, r: &ConfigRef) -> Result<ConfigEntry> {
    match r {
        ConfigRef::Personal(slug) if valid_slug(slug) => {
            let path = personal_path(store, slug);
            if !path.is_file() {
                return Err(Error::Other(format!("no personal shader config {slug}")));
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            Ok(ConfigEntry {
                r#ref: r.clone(),
                name: name_of(&text, slug),
                source: "Personal".into(),
                editable: true,
                path,
            })
        }
        _ => list(store)
            .into_iter()
            .find(|e| &e.r#ref == r)
            .ok_or_else(|| Error::Other(format!("no shader config {r}"))),
    }
}

pub fn read(store: &Store, r: &ConfigRef) -> Result<String> {
    let e = entry(store, r)?;
    std::fs::read_to_string(&e.path).map_err(io_err(&e.path))
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let tmp = path.with_extension("swfx.tmp");
    std::fs::write(&tmp, text).map_err(io_err(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io_err(path))
}

/// Personal configs only.
pub fn save(store: &Store, r: &ConfigRef, text: &str) -> Result<()> {
    match r {
        ConfigRef::Personal(_) => write_text(&entry(store, r)?.path, text),
        ConfigRef::Addon { .. } => Err(Error::Other(
            "an addon's shader config is read-only; copy it to your own configs to edit it".into(),
        )),
    }
}

fn write_new(store: &Store, name: &str, text: &str) -> Result<ConfigRef> {
    let slug = unique_slug(store, &slug(name));
    write_text(&personal_path(store, &slug), &with_name(text, name))?;
    Ok(ConfigRef::Personal(slug))
}

/// `None` starts from the starter template.
pub fn create(store: &Store, name: &str, text: Option<&str>) -> Result<ConfigRef> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("a shader config needs a name".into()));
    }
    if name_taken(store, name) {
        return Err(Error::Other(format!(
            "you already have a shader config named {name}"
        )));
    }
    let text = match text {
        Some(t) => t,
        None => starter().unwrap_or("name = \"\"\n"),
    };
    write_new(store, name, text)
}

pub fn copy_to_personal(store: &Store, r: &ConfigRef) -> Result<ConfigRef> {
    let text = read(store, r)?;
    let name = name_of(&text, "Shader config");
    let name = if name_taken(store, &name) {
        let mut candidate = format!("{name} (copy)");
        let mut n = 2;
        while name_taken(store, &candidate) {
            candidate = format!("{name} (copy {n})");
            n += 1;
        }
        candidate
    } else {
        name
    };
    write_new(store, &name, &text)
}

fn repoint(store: &Store, from: &ConfigRef, to: Option<&ConfigRef>) -> Result<()> {
    let from = from.to_string();
    for name in store.profile_names()? {
        if name == crate::profile::VANILLA {
            continue;
        }
        let Ok(p) = store.profile(&name) else { continue };
        if p.shader.as_deref() == Some(from.as_str()) {
            store.update_profile(&name, |p| {
                p.shader = to.map(ToString::to_string);
                Ok(())
            })?;
        }
    }
    Ok(())
}

pub fn rename(store: &Store, r: &ConfigRef, name: &str) -> Result<ConfigRef> {
    let ConfigRef::Personal(old) = r else {
        return Err(Error::Other("an addon's shader config cannot be renamed".into()));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("a shader config needs a name".into()));
    }
    let e = entry(store, r)?;
    if !e.name.eq_ignore_ascii_case(name) && name_taken(store, name) {
        return Err(Error::Other(format!(
            "you already have a shader config named {name}"
        )));
    }
    let text = with_name(&read(store, r)?, name);
    let base = slug(name);
    let new = if &base == old || personal_path(store, &base) == e.path {
        old.clone()
    } else {
        unique_slug(store, &base)
    };
    write_text(&personal_path(store, &new), &text)?;
    let renamed = ConfigRef::Personal(new);
    if &renamed != r {
        std::fs::remove_file(&e.path).map_err(io_err(&e.path))?;
        repoint(store, r, Some(&renamed))?;
    }
    Ok(renamed)
}

pub fn delete(store: &Store, r: &ConfigRef) -> Result<()> {
    if !matches!(r, ConfigRef::Personal(_)) {
        return Err(Error::Other(
            "an addon's shader config cannot be deleted; remove the addon".into(),
        ));
    }
    let e = entry(store, r)?;
    std::fs::remove_file(&e.path).map_err(io_err(&e.path))?;
    repoint(store, r, None)
}

pub fn import(store: &Store, path: &Path) -> Result<ConfigRef> {
    let text = std::fs::read_to_string(path).map_err(io_err(path))?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported config");
    let name = free_name(store, &name_of(&text, stem));
    write_new(store, &name, &text)
}

pub fn export(store: &Store, r: &ConfigRef, path: &Path) -> Result<()> {
    let text = read(store, r)?;
    std::fs::write(path, text).map_err(io_err(path))
}

pub fn active(store: &Store, profile: &Profile) -> Option<ConfigRef> {
    let r: ConfigRef = profile.shader.as_deref()?.parse().ok()?;
    entry(store, &r).ok().map(|_| r)
}

pub fn set_active(store: &Store, profile: &mut Profile, r: Option<ConfigRef>) -> Result<()> {
    profile.shader = r.map(|r| r.to_string());
    store.save_profile(profile)
}

/// The config an enabled addon forces, with the addon's name. A shader-only addon is enabled
/// whenever it is installed; of several forcing addons the last in load order wins.
pub fn forced(store: &Store, profile: &Profile) -> Option<(ConfigRef, String)> {
    if profile.is_vanilla() {
        return None;
    }
    let enabled: Vec<&str> = profile.enabled().collect();
    shader_addons(store, Some(profile))
        .into_iter()
        .filter(|m| m.is_shader_only() || enabled.contains(&m.id.as_str()))
        .filter_map(|m| {
            let decl = m.shaders.iter().find(|d| d.force)?;
            Some((
                ConfigRef::Addon {
                    addon: m.id.clone(),
                    file: decl.stem().to_owned(),
                },
                m.name.clone(),
            ))
        })
        .next_back()
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toggle_key: Option<String>,
}

/// The in-game on/off key, `"B"` by default; `"None"` is no key.
pub fn toggle_key(store: &Store) -> String {
    std::fs::read(store.shaders_dir().join(SETTINGS))
        .ok()
        .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
        .and_then(|s| s.toggle_key)
        .filter(|k| key_ok(k))
        .unwrap_or_else(|| DEFAULT_KEY.into())
}

fn key_ok(key: &str) -> bool {
    key == NO_KEY || swofty_addon::manifest::postfx_key_ok(key)
}

pub fn set_toggle_key(store: &Store, key: &str) -> Result<()> {
    let key = key.trim();
    let key = if key.eq_ignore_ascii_case(NO_KEY) {
        NO_KEY.to_owned()
    } else {
        key.to_ascii_uppercase()
    };
    if !key_ok(&key) {
        return Err(Error::Other(format!(
            "{key} is not a key: use A-Z, 0-9, F1-F24 or {NO_KEY}"
        )));
    }
    crate::write_json(
        &store.shaders_dir().join(SETTINGS),
        &Settings {
            toggle_key: Some(key),
        },
    )
}

/// The active config resolved for the game: `active` is what `active.json` holds, `problem` says
/// why a chosen config is not running.
pub struct Resolved {
    pub active: Active,
    pub config: Option<ConfigRef>,
    pub problem: Option<String>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn resolve_active(store: &Store, profile: &Profile, catalog: &Catalog) -> Resolved {
    let forced = forced(store, profile);
    let (config, forced_by) = match forced {
        Some((r, addon)) => (Some(r), addon),
        None => (active(store, profile), String::new()),
    };
    let mut out = Resolved {
        active: Active {
            version: 1,
            serial: now_ms(),
            name: String::new(),
            source: String::new(),
            forced_by,
            toggle_key: toggle_key(store),
            instances: Vec::new(),
            rules: Vec::new(),
        },
        config: config.clone(),
        problem: None,
    };
    let Some(r) = config else { return out };
    out.active.source = r.to_string();
    let text = match read(store, &r) {
        Ok(t) => t,
        Err(e) => {
            out.problem = Some(e.to_string());
            return out;
        }
    };
    out.active.name = name_of(&text, &r.to_string());
    let lint = swofty_shaders::lint(&text, catalog);
    match &lint.config {
        Some(config) => {
            out.active.instances = swofty_shaders::resolve(config, catalog);
            out.active.rules = swofty_shaders::resolve_rules(config, catalog);
        }
        None => {
            let first = lint.diagnostics.iter().find(|d| d.severity == Severity::Error);
            out.problem = Some(match first {
                Some(d) => format!(
                    "{} line {}: {} ({} error(s))",
                    out.active.name,
                    d.start.line + 1,
                    d.message,
                    lint.errors()
                ),
                None => format!("{} does not pass the checks", out.active.name),
            });
        }
    }
    out
}

/// What the build and `push_live` write as `active.json`.
pub fn active_file(store: &Store, profile: &Profile) -> Active {
    resolve_active(store, profile, &catalog(store)).active
}

/// Writes `<game>/swofty/shaders/active.json` atomically, for the running game to pick up.
pub fn push_live(game_dir: &Path, active: &Active) -> Result<()> {
    crate::write_json_shared(&game_dir.join("swofty").join(ACTIVE_FILE), active)
}

const ACTIVE_FILE: &str = "shaders/active.json";

/// The game path of the live config file. The launcher rewrites it while a session runs, so its bytes
/// differ from what the build placed; it is the launcher's own whatever it holds.
pub const LIVE_FILE: &str = "swofty/shaders/active.json";

/// The library extracted to the cache, named by its content.
fn extract_library(store: &Store) -> Result<PathBuf> {
    let mut h = Sha256::new();
    for (path, bytes) in library() {
        h.update(path.as_bytes());
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    let key = crate::baseline::hex(&h.finalize());
    let root = store.shaders_cache_dir().join("library");
    let dir = root.join(&key[..16]);
    if dir.join(".done").is_file() {
        return Ok(dir);
    }
    let tmp = root.join(format!("{}.{}.tmp", &key[..16], std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    for (path, bytes) in library() {
        let to = tmp.join(path);
        let parent = to.parent().expect("has parent");
        std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        std::fs::write(&to, bytes).map_err(io_err(&to))?;
    }
    std::fs::write(tmp.join(".done"), b"").map_err(io_err(&tmp))?;
    std::fs::remove_dir_all(&dir).ok();
    if std::fs::rename(&tmp, &dir).is_err() {
        std::fs::remove_dir_all(&tmp).ok();
        if !dir.join(".done").is_file() {
            return Err(Error::Other(format!(
                "{}: could not extract the shader library",
                dir.display()
            )));
        }
    }
    Ok(dir)
}

fn decode_image(png: &[u8]) -> std::result::Result<(u32, u32, Vec<u8>), String> {
    let tex = crate::postfx::png_to_tex(png)?;
    let word = |at: usize| u32::from_le_bytes(tex[at..at + 4].try_into().expect("four bytes"));
    Ok((word(4), word(8), tex[16..].to_vec()))
}

fn tex_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + rgba.len());
    out.extend_from_slice(b"SWTX");
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(rgba);
    out
}

/// Compiles every catalog effect for `target` (DXIL for the game, SPIR-V for the preview). An
/// effect that fails is left out and named in the second list.
pub fn compile_all(store: &Store, target: Target) -> (Vec<CompiledEffect>, Vec<String>) {
    compile_loaded(store, &load(store), target)
}

fn compile_loaded(store: &Store, loaded: &Loaded, target: Target) -> (Vec<CompiledEffect>, Vec<String>) {
    let mut warnings = loaded.warnings.clone();
    let Some(dxc) = crate::postfx::dxc() else {
        warnings.push("dxc was not found (set SWOFTY_DXC, or put dxc on PATH)".into());
        return (Vec::new(), warnings);
    };
    let library_dir = if loaded.homes.values().any(|h| matches!(h, Home::Library)) {
        match extract_library(store) {
            Ok(d) => Some(d.join("effects")),
            Err(e) => {
                warnings.push(e.to_string());
                None
            }
        }
    } else {
        None
    };
    let cache = store.shaders_cache_dir().join(match target {
        Target::Dxil => "dxil",
        Target::Spirv => "spirv",
    });
    let mut out = Vec::new();
    for effect in &loaded.catalog.effects {
        let dir = match loaded.homes.get(&effect.id) {
            Some(Home::Addon(dir)) => dir.clone(),
            Some(Home::Library) => match &library_dir {
                Some(d) => d.clone(),
                None => {
                    warnings.push(format!(
                        "the effect {} was skipped: the effect library did not unpack",
                        effect.id
                    ));
                    continue;
                }
            },
            None => {
                warnings.push(format!("the effect {} has no source to compile", effect.id));
                continue;
            }
        };
        let blobs = match swofty_shaders::compile(effect, &dir, target, &dxc, crate::postfx::HEADER, &cache) {
            Ok(b) => b,
            Err(e) if target == Target::Spirv && e.contains("SPIR-V CodeGen not available") => {
                // Every effect fails the same way: say it once.
                warnings.clear();
                warnings.push(
                    "This launcher's shader compiler can't build effects for the preview (it has no SPIR-V). \
                     Shaders still work in game; updating Swofty Launcher brings a compiler that can."
                        .to_owned(),
                );
                return (Vec::new(), warnings);
            }
            Err(e) => {
                warnings.push(format!("the effect {} did not compile: {e}", effect.id));
                continue;
            }
        };
        let mut images = Vec::new();
        let mut failed = None;
        for t in &effect.textures {
            let Some(file) = &t.file else { continue };
            let image = std::fs::read(dir.join(file))
                .map_err(|e| e.to_string())
                .and_then(|png| decode_image(&png));
            match image {
                Ok((w, h, rgba)) => images.push((t.name.clone(), w, h, rgba)),
                Err(e) => failed = Some(format!("the effect {}: {file}: {e}", effect.id)),
            }
        }
        if let Some(e) = failed {
            warnings.push(e);
            continue;
        }
        out.push(CompiledEffect {
            effect: effect.clone(),
            blobs,
            images,
        });
    }
    (out, warnings)
}

/// What a build writes under `swofty/` for shaders.
#[derive(Debug, Default)]
pub struct BuildFiles {
    /// Paths relative to `swofty/`.
    pub files: Vec<(String, Vec<u8>)>,
    /// Identifies the content without the live serial, for the build hash.
    pub digest: String,
    pub line: Option<String>,
    pub warnings: Vec<String>,
}

/// `shaders/effects.json`, the compiled passes and textures, and `shaders/active.json` for
/// `profile`. A failed effect or a config with errors is a warning, never a failed build.
pub fn build_files(store: &Store, profile: &Profile) -> BuildFiles {
    let mut out = BuildFiles::default();
    if profile.is_vanilla() {
        return out;
    }
    let loaded = load(store);
    if loaded.catalog.effects.is_empty() {
        out.warnings = loaded.warnings;
        return out;
    }
    let resolved = resolve_active(store, profile, &loaded.catalog);
    if resolved.active.instances.is_empty() {
        // Nothing runs, so the game gets no shader files and the core arms none of its hooks.
        out.warnings = loaded.warnings;
        if let Some(problem) = &resolved.problem {
            out.warnings.push(format!("shaders: {problem}; shaders are off"));
        }
        out.line = Some("shaders: off".into());
        return out;
    }
    let (compiled, mut warnings) = compile_loaded(store, &loaded, Target::Dxil);
    if let Some(problem) = &resolved.problem {
        warnings.push(format!("shaders: {problem}; shaders are off"));
    }
    out.warnings = warnings;
    if compiled.is_empty() {
        out.line = Some("shaders: off".into());
        out.warnings.push(format!(
            "shaders: {} is off; no effect compiled",
            resolved.active.name
        ));
        return out;
    }
    let mut active = resolved.active;
    let known: Vec<&str> = compiled.iter().map(|c| c.effect.id.as_str()).collect();
    active.instances.retain(|i| {
        let ok = known.contains(&i.effect.as_str());
        if !ok {
            out.warnings.push(format!(
                "shaders: {} skips {}, which did not compile",
                active.name, i.effect
            ));
        }
        ok
    });
    let mut effects = Vec::new();
    for c in &compiled {
        let id = &c.effect.id;
        let mut textures = Vec::new();
        for t in &c.effect.textures {
            let image = c.images.iter().find(|i| i.0 == t.name);
            let file = image.map(|(name, w, h, rgba)| {
                let rel = format!("shaders/tex/{id}-{name}.tex");
                out.files.push((rel.clone(), tex_bytes(*w, *h, rgba)));
                rel
            });
            let mut v = serde_json::json!({ "name": t.name });
            let o = v.as_object_mut().expect("object");
            if file.is_some() {
                o.insert("format".into(), "rgba8".into());
            } else {
                o.insert(
                    "format".into(),
                    t.format.clone().unwrap_or_else(|| "rgba16f".into()).into(),
                );
                if let Some(s) = t.scale {
                    o.insert("scale".into(), s.into());
                }
                if let Some(w) = t.width {
                    o.insert("width".into(), w.into());
                }
                if let Some(h) = t.height {
                    o.insert("height".into(), h.into());
                }
                if t.persistent {
                    o.insert("persistent".into(), true.into());
                }
                if let Some(m) = t.mips.filter(|&m| m > 1) {
                    o.insert("mips".into(), m.into());
                }
            }
            if let Some(f) = file {
                o.insert("file".into(), f.into());
            }
            textures.push(v);
        }
        let mut passes = Vec::new();
        for (n, (pass, dxil)) in c.effect.passes.iter().zip(&c.blobs).enumerate() {
            let rel = format!("shaders/fx/{id}-{n:02}.dxil");
            out.files.push((rel.clone(), dxil.clone()));
            let mut v = serde_json::json!({
                "name": pass.name,
                "dxil": rel,
                "inputs": pass.inputs,
                "output": pass.output.clone().unwrap_or_default(),
                "threads": pass.threads.unwrap_or([8, 8]),
                "raw": pass.raw,
                "budget_ms": pass.budget_ms.unwrap_or(1.5),
            });
            if let Some(when) = &pass.when {
                v["when"] = when.clone();
            }
            passes.push(v);
        }
        let addon = match &c.effect.origin {
            Origin::Library => String::new(),
            Origin::Addon { id, .. } => id.clone(),
        };
        effects.push(serde_json::json!({
            "id": id, "name": c.effect.name, "addon": addon, "slots": c.effect.slots,
            "textures": textures, "passes": passes,
        }));
    }
    let effects_json = serde_json::to_vec_pretty(&serde_json::json!({ "version": 1, "effects": effects }))
        .expect("serialisable");
    out.files.push(("shaders/effects.json".into(), effects_json));
    let mut h = Sha256::new();
    for (rel, bytes) in &out.files {
        h.update(rel.as_bytes());
        h.update(bytes);
    }
    let serial = active.serial;
    active.serial = 0;
    h.update(serde_json::to_vec(&active).expect("serialisable"));
    active.serial = serial;
    out.digest = crate::baseline::hex(&h.finalize());
    let passes: usize = active
        .instances
        .iter()
        .filter_map(|i| compiled.iter().find(|c| c.effect.id == i.effect))
        .map(|c| c.effect.passes.len())
        .sum();
    out.line = Some(if active.instances.is_empty() {
        "shaders: off".into()
    } else {
        format!(
            "shaders: config \"{}\" runs {} effect(s) ({passes} passes)",
            active.name,
            active.instances.len()
        )
    });
    let mut json = serde_json::to_vec_pretty(&active).expect("serialisable");
    json.push(b'\n');
    out.files.push((ACTIVE_FILE.into(), json));
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const SPARK: &str = r#"{
        "id": "spark", "name": "Spark", "description": "A test effect.",
        "params": [{ "id": "intensity", "type": "float", "default": 1.0, "min": 0, "max": 4,
                     "label": "Intensity", "doc": "How much." }],
        "passes": [{ "name": "apply", "shader": "spark.hlsl" }]
    }"#;

    pub(crate) const SPARK_HLSL: &str = "#include \"swofty/params.hlsli\"\n\
        float4 effect(float2 uv, uint2 p) { return scene(uv) * param_intensity(); }\n";

    pub(crate) fn temp(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("swofty-shaders-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        root
    }

    pub(crate) fn addon(store: &Store, id: &str, extra: &str, configs: &[(&str, &str)], effect: bool) {
        let dir = store.addons_dir().join(id);
        std::fs::create_dir_all(dir.join("shaders")).unwrap();
        std::fs::create_dir_all(dir.join("fx")).unwrap();
        let mut shaders = Vec::new();
        for (file, text) in configs {
            std::fs::write(dir.join("shaders").join(file), text).unwrap();
            let force = *file == "forced.swfx";
            shaders.push(format!(r#"{{ "config": "shaders/{file}", "force": {force} }}"#));
        }
        if effect {
            std::fs::write(dir.join("fx/spark.effect.json"), SPARK).unwrap();
            std::fs::write(dir.join("fx/spark.hlsl"), SPARK_HLSL).unwrap();
        }
        let effects = if effect {
            r#""effects": ["fx/spark.effect.json"],"#
        } else {
            ""
        };
        std::fs::write(
            dir.join("swofty.json"),
            format!(
                r#"{{ "schema_version": 1, "id": "{id}", "name": "{id} name", "version": "1.0.0",
                    "author": "t", {extra} {effects} "shaders": [{}] }}"#,
                shaders.join(",")
            ),
        )
        .unwrap();
    }

    const PLAIN: &str = "# keep me\nname = \"Plain\"\n\n[spark]\nintensity = 2.0\n";

    #[test]
    fn personal_configs_are_created_renamed_copied_and_deleted() {
        let root = temp("personal");
        let store = Store::new(&root);
        let r = create(&store, "My Look", Some(PLAIN)).unwrap();
        assert_eq!(r, ConfigRef::Personal("my-look".into()));
        assert_eq!(
            create(&store, "my look", None).unwrap_err().to_string(),
            "you already have a shader config named my look"
        );
        let text = read(&store, &r).unwrap();
        assert!(
            text.contains("# keep me") && text.contains("name = \"My Look\""),
            "{text}"
        );
        save(&store, &r, &format!("{text}# more\n")).unwrap();

        let mut p = store.profile("default").unwrap();
        set_active(&store, &mut p, Some(r.clone())).unwrap();
        assert_eq!(active(&store, &p), Some(r.clone()));

        let renamed = rename(&store, &r, "Bright").unwrap();
        assert_eq!(renamed, ConfigRef::Personal("bright".into()));
        let text = read(&store, &renamed).unwrap();
        assert!(text.contains("# keep me") && text.contains("# more") && text.contains("name = \"Bright\""));
        assert!(read(&store, &r).is_err(), "the old file is gone");
        let p = store.profile("default").unwrap();
        assert_eq!(
            p.shader.as_deref(),
            Some("personal:bright"),
            "the profile follows the rename"
        );

        let copy = copy_to_personal(&store, &renamed).unwrap();
        assert_eq!(copy, ConfigRef::Personal("bright-copy".into()));
        let names: Vec<_> = list(&store).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["Bright", "Bright (copy)"]);

        let out = root.join("export.swfx");
        export(&store, &copy, &out).unwrap();
        let imported = import(&store, &out).unwrap();
        assert_eq!(
            read(&store, &imported).unwrap(),
            read(&store, &copy).unwrap().replace("(copy)", "(copy) (2)")
        );

        delete(&store, &renamed).unwrap();
        assert_eq!(store.profile("default").unwrap().shader, None);
        assert_eq!(active(&store, &store.profile("default").unwrap()), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn with_name_changes_only_the_name_line() {
        assert_eq!(with_name("a = 1\n", "X"), "name = \"X\"\na = 1\n");
        assert_eq!(
            with_name("# c\nname = 'Old'  # note\n[t]\nname = \"inner\"\n", "New \"q\""),
            "# c\nname = \"New \\\"q\\\"\"\n[t]\nname = \"inner\"\n"
        );
        assert_eq!(name_of("[t]\nname = \"inner\"\n", "stem"), "stem");
        assert_eq!(slug("  Hello, World!! "), "hello-world");
        assert_eq!(slug("!!!"), "config");
    }

    #[test]
    fn addon_configs_are_listed_read_only_whether_the_addon_is_on_or_off() {
        let root = temp("addons");
        let store = Store::new(&root);
        addon(
            &store,
            "t.pack",
            r#""multiplayer": "cosmetic","#,
            &[("a.swfx", "name = \"Alpha\"\n"), ("b.swfx", "[spark]\n")],
            true,
        );
        addon(&store, "t.only", "", &[("o.swfx", "name = \"Only\"\n")], false);
        create(&store, "Mine", Some(PLAIN)).unwrap();

        let all = list(&store);
        let seen: Vec<_> = all
            .iter()
            .map(|e| (e.r#ref.to_string(), e.name.as_str(), e.editable))
            .collect();
        assert_eq!(
            seen,
            [
                ("personal:mine".to_owned(), "Mine", true),
                ("addon:t.only/o".to_owned(), "Only", false),
                ("addon:t.pack/a".to_owned(), "Alpha", false),
                ("addon:t.pack/b".to_owned(), "b", false),
            ]
        );
        assert_eq!(all[2].source, "t.pack name");
        let alpha: ConfigRef = "addon:t.pack/a".parse().unwrap();
        assert!(save(&store, &alpha, "x").is_err());
        assert!(delete(&store, &alpha).is_err() && rename(&store, &alpha, "x").is_err());
        let copy = copy_to_personal(&store, &alpha).unwrap();
        assert_eq!(copy, ConfigRef::Personal("alpha".into()));
        assert!(
            !store.profile("default").unwrap().has("t.pack"),
            "the addon was never turned on"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_forced_config_needs_its_addon_on_unless_the_addon_is_shader_only() {
        let root = temp("forced");
        let store = Store::new(&root);
        addon(
            &store,
            "t.mixed",
            r#""multiplayer": "cosmetic","#,
            &[("forced.swfx", "name = \"Forced\"\n")],
            false,
        );
        let mut p = store.profile("default").unwrap();
        assert_eq!(forced(&store, &p), None);
        p.set_enabled("t.mixed", true).unwrap();
        assert_eq!(
            forced(&store, &p),
            Some((
                ConfigRef::Addon {
                    addon: "t.mixed".into(),
                    file: "forced".into()
                },
                "t.mixed name".into()
            ))
        );
        p.set_enabled("t.mixed", false).unwrap();
        assert_eq!(forced(&store, &p), None);
        assert_eq!(forced(&store, &Profile::vanilla()), None);

        addon(
            &store,
            "t.only",
            "",
            &[("forced.swfx", "name = \"Only forced\"\n")],
            false,
        );
        let (r, name) = forced(&store, &p).unwrap();
        assert_eq!(
            (r.to_string(), name.as_str()),
            ("addon:t.only/forced".to_owned(), "t.only name")
        );
        assert_eq!(
            forced(&store, &Profile::vanilla()),
            None,
            "the vanilla profile has no shaders"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_toggle_key_defaults_to_b_and_is_checked() {
        let root = temp("key");
        let store = Store::new(&root);
        assert_eq!(toggle_key(&store), "B");
        set_toggle_key(&store, "f9").unwrap();
        assert_eq!(toggle_key(&store), "F9");
        set_toggle_key(&store, "none").unwrap();
        assert_eq!(toggle_key(&store), "None");
        assert!(set_toggle_key(&store, "ctrl").is_err());
        assert_eq!(toggle_key(&store), "None");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn push_live_replaces_active_json_atomically() {
        let root = temp("live");
        let active = Active {
            version: 1,
            serial: 7,
            name: "Realistic".into(),
            source: "personal:realistic".into(),
            forced_by: String::new(),
            toggle_key: "B".into(),
            instances: vec![ActiveInstance {
                effect: "spark".into(),
                label: String::new(),
                params: vec![2.0],
            }],
            rules: Vec::new(),
        };
        push_live(&root, &active).unwrap();
        push_live(
            &root,
            &Active {
                serial: 8,
                ..active.clone()
            },
        )
        .unwrap();
        let path = root.join("swofty/shaders/active.json");
        let back: Active = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back.serial, 8);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1,
            "no temp file is left"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_catalog_holds_every_installed_addons_effects_and_the_first_clash_wins() {
        let root = temp("catalog");
        let store = Store::new(&root);
        addon(&store, "t.one", "", &[], true);
        addon(&store, "t.two", r#""multiplayer": "cosmetic","#, &[], true);
        let loaded = load(&store);
        let ids: Vec<_> = loaded.catalog.effects.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"spark"), "{ids:?} {:?}", loaded.warnings);
        assert_eq!(ids.iter().filter(|i| **i == "spark").count(), 1);
        assert!(
            loaded
                .warnings
                .iter()
                .any(|w| w.contains("spark") && w.contains("already provides")),
            "{:?}",
            loaded.warnings
        );
        assert!(
            matches!(&loaded.catalog.get("spark").unwrap().origin, Origin::Addon { id, .. } if id == "t.one")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_active_file_resolves_the_chosen_config() {
        let root = temp("activefile");
        let store = Store::new(&root);
        addon(
            &store,
            "t.one",
            "",
            &[("a.swfx", "name = \"Alpha\"\n[spark]\nintensity = 2.5\n")],
            true,
        );
        let mut p = store.profile("default").unwrap();
        let none = active_file(&store, &p);
        assert_eq!(
            (none.name.as_str(), none.instances.len(), none.toggle_key.as_str()),
            ("", 0, "B")
        );
        let r = ConfigRef::Addon {
            addon: "t.one".into(),
            file: "a".into(),
        };
        set_active(&store, &mut p, Some(r.clone())).unwrap();
        let a = active_file(&store, &p);
        assert_eq!((a.name.as_str(), a.source.as_str()), ("Alpha", "addon:t.one/a"));
        assert_eq!(a.instances.len(), 1);
        assert_eq!(a.instances[0].params, [2.5]);
        assert!(a.serial > 0);
        assert!(a.rules.is_empty());

        let ruled = create(
            &store,
            "Ruled",
            Some("name = \"Ruled\"\n[spark]\nintensity = 2.5\n[[when]]\nname = \"lap\"\non = \"session.lap\"\n[when.set.spark]\nintensity = 4\n"),
        )
        .unwrap();
        set_active(&store, &mut p, Some(ruled)).unwrap();
        let a = active_file(&store, &p);
        assert_eq!(a.rules.len(), 1);
        assert_eq!(a.rules[0].name, "lap");
        assert!(a.rules[0].event);
        assert_eq!(a.rules[0].set[0].slot, Some(0));
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["rules"][0]["when"][0]["op"], "event");
        assert_eq!(json["rules"][0]["set"][0]["mode"], "blend");

        let bad = create(&store, "Bad", Some("[nothing]\n")).unwrap();
        set_active(&store, &mut p, Some(bad)).unwrap();
        assert!(
            active_file(&store, &p).instances.is_empty(),
            "a config with errors runs nothing"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_shader_only_addon_installs_from_a_folder_and_a_zip_and_removing_it_clears_the_profiles() {
        let root = temp("install");
        let store = Store::new(root.join("data"));
        let manifest = r#"{"schema_version":1,"id":"t.look","name":"Look","version":"1.0.0","author":"t",
            "category":"shaders","compat":{"game":"f1_25"},"shaders":[{"config":"look.swfx"}]}"#;
        let config = "name = \"Look\"\n[spark]\nintensity = 2.0\n";
        let folder = root.join("src/t.look");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("swofty.json"), manifest).unwrap();
        std::fs::write(folder.join("look.swfx"), config).unwrap();
        let zip = root.join("look.swoftyaddon");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("swofty.json", opts).unwrap();
        std::io::Write::write_all(&mut z, manifest.as_bytes()).unwrap();
        z.start_file("look.swfx", opts).unwrap();
        std::io::Write::write_all(&mut z, config.as_bytes()).unwrap();
        z.finish().unwrap();

        let r = ConfigRef::Addon {
            addon: "t.look".into(),
            file: "look".into(),
        };
        for src in [&folder, &zip] {
            assert!(store.install_addon(src).unwrap().is_shader_only());
            let listed: Vec<_> = list(&store).into_iter().map(|e| e.r#ref).collect();
            assert_eq!(listed, std::slice::from_ref(&r));
            let mut p = store.profile("default").unwrap();
            set_active(&store, &mut p, Some(r.clone())).unwrap();
            assert_eq!(active(&store, &p), Some(r.clone()));
            assert_eq!(read(&store, &r).unwrap(), config);
            store.remove_addon("t.look").unwrap();
            assert!(list(&store).is_empty());
            let p = store.profile("default").unwrap();
            assert_eq!(p.shader, None);
            assert_eq!(active(&store, &p), None);
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_catalog_effects_compile_to_dxil_when_dxc_is_at_hand() {
        if crate::postfx::dxc().is_none() {
            return;
        }
        let root = temp("compile");
        let store = Store::new(&root);
        addon(&store, "t.one", "", &[], true);
        let (compiled, warnings) = compile_all(&store, Target::Dxil);
        assert!(warnings.is_empty(), "{warnings:?}");
        let spark = compiled.iter().find(|c| c.effect.id == "spark").unwrap();
        assert!(spark.blobs.len() == 1 && spark.blobs[0].starts_with(b"DXBC"));
        std::fs::remove_dir_all(&root).ok();
    }
}
