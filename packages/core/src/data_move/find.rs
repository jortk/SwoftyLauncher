//! Looking for an addons-and-saves folder the launcher is not using: left
//! behind by a move, an install that was moved, or a folder on another drive.
//! A fixed list of places is read, one level deep, with no search of a drive.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::{Location, history, within};
use crate::process::{same_folder, same_path};

const ADDONS: [&str; 2] = ["addons", "mods"];
const MANIFEST: &str = "swofty.json";
const MEASURE_UP_TO: usize = 40_000;
const NAMES_SHOWN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Default,
    Aside,
    PreviousLocation,
    OldInstall,
    DriveRoot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    pub kind: Kind,
    pub addons: usize,
    pub names: Vec<String>,
    pub profiles: usize,
    pub bytes: u64,
    pub bytes_partial: bool,
    pub newest: SystemTime,
}

pub fn candidates(loc: &Location, installs: &[PathBuf], bundled: &[String]) -> Vec<Found> {
    candidates_on(loc, installs, bundled, &drive_roots())
}

pub fn candidates_on(
    loc: &Location,
    installs: &[PathBuf],
    bundled: &[String],
    drives: &[PathBuf],
) -> Vec<Found> {
    let current = loc.current();
    let have = looks_like_data(&current, bundled).map_or(0, |f| f.addons);
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out: Vec<Found> = Vec::new();
    for (path, kind) in places(loc, installs, drives) {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.iter().any(|s| same_path(s, &key)) {
            continue;
        }
        seen.push(key);
        if same_folder(&path, &current)
            || (current.exists() && (within(&path, &current) || within(&current, &path)))
            || (loc.default.exists() && within(&loc.default, &path) && !same_path(&path, &loc.default))
        {
            continue;
        }
        let Some(mut found) = looks_like_data(&path, bundled) else {
            continue;
        };
        if found.addons < have {
            continue;
        }
        found.kind = kind;
        measure(&mut found);
        out.push(found);
    }
    out.sort_by_key(|f| (std::cmp::Reverse(f.newest), std::cmp::Reverse(f.addons)));
    out
}

fn places(loc: &Location, installs: &[PathBuf], drives: &[PathBuf]) -> Vec<(PathBuf, Kind)> {
    let mut out: Vec<(PathBuf, Kind)> = Vec::new();
    if loc.env.is_some() {
        out.push((loc.default.clone(), Kind::Default));
    }
    let name = loc
        .default
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_lowercase());
    if let Some(parent) = loc.default.parent() {
        for child in children(parent) {
            let lower = file_name(&child);
            if lower.starts_with(&format!("{name}.old")) || lower.starts_with(&format!("{name}.fresh")) {
                out.push((child, Kind::Aside));
            }
        }
        if let Some(journal) = std::fs::read_to_string(super::Journal::path(&loc.default))
            .ok()
            .and_then(|t| serde_json::from_str::<super::Journal>(&t).ok())
        {
            out.push((journal.from, Kind::PreviousLocation));
            out.push((journal.to, Kind::PreviousLocation));
        }
    }
    for entry in history::entries(&loc.default).into_iter().rev() {
        out.push((entry.path, Kind::PreviousLocation));
    }
    for root in installs.iter().chain(loc.install.iter()) {
        out.push((root.clone(), Kind::OldInstall));
        for child in children(root) {
            if file_name(&child).starts_with("swofty") {
                out.push((child, Kind::OldInstall));
            }
        }
    }
    for drive in drives {
        for child in children(drive) {
            let lower = file_name(&child);
            if lower.starts_with("swofty") {
                out.push((child, Kind::DriveRoot));
            } else if matches!(lower.as_str(), "games" | "program files" | "program files (x86)") {
                for inner in children(&child) {
                    if file_name(&inner).starts_with("swofty") {
                        out.push((inner, Kind::DriveRoot));
                    }
                }
            }
        }
    }
    out
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_lowercase())
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir() || t.is_symlink()))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

#[cfg(windows)]
fn drive_roots() -> Vec<PathBuf> {
    ('C'..='Z')
        .map(|c| PathBuf::from(format!("{c}:\\")))
        .filter(|d| d.is_dir())
        .collect()
}

#[cfg(not(windows))]
fn drive_roots() -> Vec<PathBuf> {
    Vec::new()
}

fn manifest_name(dir: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(&folder)
        .to_owned();
    let name = doc.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_owned();
    Some((id, name))
}

/// What `dir` holds when it is an addons-and-saves folder: at least one addon
/// that does not ship with the launcher, and a config, profiles or saves.
pub fn looks_like_data(dir: &Path, bundled: &[String]) -> Option<Found> {
    let mut names: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut newest = SystemTime::UNIX_EPOCH;
    let touch = |newest: &mut SystemTime, path: &Path| {
        if let Some(m) = path.metadata().ok().and_then(|m| m.modified().ok()) {
            *newest = (*newest).max(m);
        }
    };
    for folder in ADDONS {
        let addons = dir.join(folder);
        for addon in children(&addons) {
            let Some((id, name)) = manifest_name(&addon) else {
                continue;
            };
            let folder_name = file_name(&addon);
            let shipped = bundled
                .iter()
                .any(|b| b.eq_ignore_ascii_case(&id) || b.to_lowercase() == folder_name);
            if !shipped && seen.insert(id) {
                names.push(name);
            }
        }
        touch(&mut newest, &addons);
    }
    if names.is_empty() {
        return None;
    }
    let profiles = std::fs::read_dir(dir.join("profiles")).map_or(0, |r| r.flatten().count());
    let config = dir.join("config.json");
    if !config.is_file() && profiles == 0 && !dir.join("saves").is_dir() {
        return None;
    }
    touch(&mut newest, &config);
    touch(&mut newest, &dir.join("profiles"));
    let addons = names.len();
    names.sort();
    names.truncate(NAMES_SHOWN);
    Some(Found {
        path: dir.to_path_buf(),
        kind: Kind::DriveRoot,
        addons,
        names,
        profiles,
        bytes: 0,
        bytes_partial: false,
        newest,
    })
}

fn measure(found: &mut Found) {
    let mut left = MEASURE_UP_TO;
    let mut stack = vec![found.path.clone()];
    let mut bytes = 0;
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            if left == 0 {
                found.bytes = bytes;
                found.bytes_partial = true;
                return;
            }
            left -= 1;
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                bytes += meta.len();
            }
        }
    }
    found.bytes = bytes;
}
