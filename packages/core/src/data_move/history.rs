//! Where the data folder has been, kept beside the default folder (never inside
//! it, because that is the folder a move replaces) so the places an old copy
//! may sit can be found again.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const FILE: &str = "swofty-data-history.json";
const KEEP: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub at: i64,
    pub why: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct History {
    #[serde(default)]
    locations: Vec<Entry>,
}

pub fn path(default: &Path) -> PathBuf {
    default.with_file_name(FILE)
}

pub fn entries(default: &Path) -> Vec<Entry> {
    std::fs::read_to_string(path(default))
        .ok()
        .and_then(|t| serde_json::from_str::<History>(&t).ok())
        .map(|h| h.locations)
        .unwrap_or_default()
}

pub fn record(default: &Path, place: &Path, why: &str) {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let mut history = History {
        locations: entries(default),
    };
    history
        .locations
        .retain(|e| !crate::process::same_path(&e.path, place));
    history.locations.push(Entry {
        path: place.to_path_buf(),
        at,
        why: why.to_owned(),
    });
    let extra = history.locations.len().saturating_sub(KEEP);
    history.locations.drain(..extra);
    crate::write_json(&path(default), &history).ok();
}

/// Records the place the launcher's data folder is `default` or links to, for a caller that has no move of its own.
pub fn record_current(default: &Path, why: &str) {
    let place = super::link::target(default).unwrap_or_else(|| default.to_path_buf());
    record(default, &place, why);
}
