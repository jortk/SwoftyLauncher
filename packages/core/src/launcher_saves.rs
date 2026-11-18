//! Launcher saves: each launcher profile keeps its own copy of F1 25's profile save at
//! `$DATA/saves/<profile>/remote/`, and a session with "Use launcher saves" on has the core serve the
//! game from there instead of from Steam's `userdata/<id3>/3059520/remote/`.
//!
//! ```text
//! saves/
//!   imported.json                    written once the first-start import has run
//!   default/remote/savegame@profile#NXDSMWW.BWW
//!   default/remote/savegame@profile#QKRHMYXE
//!   progressive/remote/...
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::baseline::sha256_file;
use crate::vault::restore::Running;
use crate::vault::{Layout, Location, Meta, Reason, Taken, Vault};
use crate::{Error, Result, io_err};

pub const DIR: &str = "saves";
pub const REMOTE: &str = "remote";
pub const MARKER: &str = "imported.json";
/// The exit code the core ends F1 25 with when launcher saves cannot be armed
/// (native/src/core/save_sandbox_logic.hpp `EXIT_LAUNCHER`).
pub const EXIT: i32 = 93;
/// Status the core writes next to settings.json when it ends the game with [`EXIT`].
pub const STATUS_FILE: &str = "swofty/launcher-saves.json";
/// Set to `off` in the game's environment to run one session on Steam's saves.
pub const OFF_ENV: &str = "SWOFTY_LAUNCHER_SAVES";
pub const EA_NOTE: &str =
    "launcher saves are not available on the EA copy yet; this session uses the EA copy's own saves";
pub const EA_DIR: &str = "ea/profile";
/// The EA copy's launcher store, import and export; the core serves the EA save device from it.
pub const EA_ENABLED: bool = true;

pub fn root(data: &Path) -> PathBuf {
    data.join(DIR)
}

pub fn remote_dir(data: &Path, profile: &str) -> PathBuf {
    root(data).join(profile).join(REMOTE)
}

pub fn ea_dir(data: &Path, profile: &str) -> PathBuf {
    root(data).join(profile).join(EA_DIR)
}

pub fn location(data: &Path, profile: &str) -> Location {
    Location::launcher(data, profile)
}

pub fn location_ea(data: &Path, profile: &str) -> Location {
    Location::launcher_ea(data, profile)
}

/// The launcher store that goes with a game save: the Steam layout's for a Steam save, the EA layout's for
/// an EA app save.
pub fn store_for(data: &Path, profile: &str, game_save: &Location) -> Option<Location> {
    match game_save.layout {
        Layout::Steam | Layout::Launcher => Some(location(data, profile)),
        Layout::Ea | Layout::LauncherEa => Some(location_ea(data, profile)),
    }
}

/// Why a Play uses the game copy's own save instead of the profile's launcher save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    UnsupportedBuild(String),
    Vanilla,
    LauncherSavesOff,
    EaCopy,
    SteamOnce,
}

/// The save a Play runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Area {
    Launcher,
    Own(Why),
}

/// What decides the save area of one Play.
#[derive(Debug, Clone, Copy)]
pub struct Choice<'a> {
    pub build_blocked: Option<&'a str>,
    pub vanilla: bool,
    pub profile_uses_launcher_saves: bool,
    pub steam_saves_once: bool,
    pub layout: Layout,
}

/// The one decision between the profile's launcher save and the copy's own save. `swofty run` acts on
/// it and the launcher's play bar shows it.
pub fn area(c: Choice<'_>) -> Area {
    if let Some(version) = c.build_blocked {
        return Area::Own(Why::UnsupportedBuild(version.to_owned()));
    }
    if c.vanilla {
        return Area::Own(Why::Vanilla);
    }
    if !c.profile_uses_launcher_saves {
        return Area::Own(Why::LauncherSavesOff);
    }
    if !supported(c.layout) {
        return Area::Own(Why::EaCopy);
    }
    if c.steam_saves_once {
        return Area::Own(Why::SteamOnce);
    }
    Area::Launcher
}

/// Whether launcher saves can be used with a copy whose save is `layout`.
pub fn supported(layout: Layout) -> bool {
    match layout {
        Layout::Steam | Layout::Launcher => true,
        Layout::Ea | Layout::LauncherEa => EA_ENABLED,
    }
}

pub fn has_files(loc: &Location) -> bool {
    loc.files().is_ok_and(|f| !f.is_empty())
}

/// The save the launcher copies in: the active copy's own, when launcher saves work with that copy.
pub fn steam_source(data: &Path) -> (Option<Location>, Vec<String>) {
    let (loc, tried) = Location::active_search(data);
    (loc.filter(|l| supported(l.layout)), tried)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seeded {
    Copied {
        files: usize,
        bytes: u64,
        snapshot: Option<String>,
    },
    NoSource,
    NotEmpty,
}

impl Seeded {
    pub fn line(&self, profile: &str, from: &Location, to: &Location) -> String {
        match self {
            Self::Copied {
                files,
                bytes,
                snapshot,
            } => format!(
                "saves: copied the Steam save ({files} file(s), {bytes} bytes, {}) into the launcher saves of profile {profile} ({}){}",
                from.dir.display(),
                to.dir.display(),
                snapshot
                    .as_ref()
                    .map(|s| format!("; the vault holds it as snapshot {s}"))
                    .unwrap_or_default()
            ),
            Self::NoSource => format!(
                "saves: no Steam save to copy into the launcher saves of profile {profile}; it starts empty"
            ),
            Self::NotEmpty => format!(
                "saves: the launcher saves of profile {profile} already hold files; nothing was copied"
            ),
        }
    }
}

fn stage_copy(from: &Path, to: &Path) -> Result<()> {
    let name = to.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let tmp = to.with_file_name(format!(".{name}.swofty-import"));
    let want = sha256_file(from)?;
    let done = std::fs::copy(from, &tmp)
        .map_err(io_err(&tmp))
        .and_then(|_| sha256_file(&tmp))
        .and_then(|h| match h == want {
            true => Ok(()),
            false => Err(Error::Other(format!("{} did not copy cleanly", tmp.display()))),
        })
        .and_then(|()| crate::sync_file(&tmp))
        .and_then(|()| std::fs::rename(&tmp, to).map_err(io_err(to)));
    if done.is_err() {
        std::fs::remove_file(&tmp).ok();
    } else {
        crate::sync_dir(to.parent());
    }
    done
}

/// Copy the profile files of `from` into `to` without touching `from`; returns (files, bytes).
pub fn copy_files(from: &Location, to: &Location) -> Result<(usize, u64)> {
    let files = from.files()?;
    std::fs::create_dir_all(&to.dir).map_err(io_err(&to.dir))?;
    let mut bytes = 0;
    for f in &files {
        let dest = to
            .path_of(&f.key)
            .ok_or_else(|| Error::Other(format!("{} has an odd name", f.key)))?;
        stage_copy(&f.path, &dest)?;
        bytes += std::fs::metadata(&dest).map_err(io_err(&dest))?.len();
    }
    Ok((files.len(), bytes))
}

/// Copy the Steam save into an empty store and snapshot the result in the vault.
pub fn seed(data: &Path, profile: &str, from: &Location, reason: Reason, now: i64) -> Result<Seeded> {
    let to = store_for(data, profile, from).expect("every layout has a store");
    if has_files(&to) {
        return Ok(Seeded::NotEmpty);
    }
    if !has_files(from) {
        return Ok(Seeded::NoSource);
    }
    crate::save_stall::refuse(data, from)?;
    let (files, bytes) = copy_files(from, &to)?;
    let meta = Meta::new(reason)
        .profile(profile)
        .note(format!("copied from {}", from.dir.display()));
    let snapshot = match Vault::of(data, &to)?.snapshot(&to, meta, now)? {
        Taken::New(s) | Taken::Unchanged(s) => Some(s.id),
        Taken::NoSave => None,
    };
    Ok(Seeded::Copied {
        files,
        bytes,
        snapshot,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Marker {
    pub at: i64,
    pub launcher: String,
    pub profiles: Vec<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub seen: bool,
}

pub fn marker_path(data: &Path) -> PathBuf {
    root(data).join(MARKER)
}

pub fn marker(data: &Path) -> Option<Marker> {
    serde_json::from_slice(&std::fs::read(marker_path(data)).ok()?).ok()
}

/// Record in `imported.json` that profile `from` is now called `to`.
pub fn rename_in_marker(data: &Path, from: &str, to: &str) -> Result<()> {
    let Some(mut m) = marker(data) else {
        return Ok(());
    };
    if !m.profiles.iter().any(|p| p == from) {
        return Ok(());
    }
    for p in m.profiles.iter_mut().filter(|p| *p == from) {
        *p = to.to_owned();
    }
    crate::write_json(&marker_path(data), &m)
}

/// Move the vaults of launcher profile `from` to the names for `to`. A vault already under `to`'s name
/// is a deleted profile's (deleting a profile keeps its vault): it moves aside to
/// `<name>.deleted.<unix time>` first, so the renamed profile shows its own history and the deleted
/// profile's stays in the vault under a name no profile can have.
pub fn rename_vaults(data: &Path, from: &str, to: &str) -> Result<()> {
    for (old, new) in [
        (Location::launcher(data, from), Location::launcher(data, to)),
        (Location::launcher_ea(data, from), Location::launcher_ea(data, to)),
    ] {
        let old = Vault::root(data).join(&old.source);
        let new = Vault::root(data).join(&new.source);
        if !old.is_dir() || old == new {
            continue;
        }
        let aside = new.symlink_metadata().is_ok().then(|| deleted_vault_name(&new));
        if let Some(aside) = &aside {
            crate::with_retry(&new, || std::fs::rename(&new, aside))?;
        }
        if let Err(e) = crate::with_retry(&old, || std::fs::rename(&old, &new)) {
            if let Some(aside) = &aside {
                std::fs::rename(aside, &new).ok();
            }
            return Err(e);
        }
    }
    Ok(())
}

/// A free name beside `vault` for the vault of a deleted profile: `<name>.deleted.<unix time>`.
fn deleted_vault_name(vault: &Path) -> PathBuf {
    let name = vault
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let stamp = crate::vault::now();
    let mut aside = vault.with_file_name(format!("{name}.deleted.{stamp}"));
    let mut n = 1;
    while aside.symlink_metadata().is_ok() {
        aside = vault.with_file_name(format!("{name}.deleted.{stamp}-{n}"));
        n += 1;
    }
    aside
}

/// Whether the first-start import has not yet found a Steam save to read.
pub fn import_pending(data: &Path) -> bool {
    !marker_path(data).exists()
}

/// Whether the launcher still has to tell the player about the first-start import: it copied the Steam
/// save into at least one profile and the player hasn't seen the notice.
pub fn notice_due(data: &Path) -> bool {
    marker(data).is_some_and(|m| !m.seen && !m.profiles.is_empty())
}

/// Record in `imported.json` that the player has seen the first-start notice.
pub fn mark_seen(data: &Path) -> Result<()> {
    let Some(mut m) = marker(data) else {
        return Ok(());
    };
    if m.seen {
        return Ok(());
    }
    m.seen = true;
    crate::write_json(&marker_path(data), &m)
}

/// The first launcher start with launcher saves: copy the Steam save into every launcher profile's
/// empty store, snapshot each in the vault, and record that it ran so it never repeats on its own. A
/// running game defers it to the next start, and a start that finds no Steam save to read records
/// nothing, so the next start tries again; the Steam save is only read.
pub fn first_start_import(
    data: &Path,
    profiles: &[String],
    running: Running,
    now: i64,
    log_miss: bool,
) -> Vec<String> {
    let config = crate::store::Store::new(data.to_path_buf()).config();
    if crate::copies::find_active(&config).is_some_and(|c| c.source == crate::install::Source::EaApp) {
        return vec![
            "saves: the import into the launcher saves is only for Steam copies; the active copy is from the EA app, so nothing is imported and it is not retried".into(),
        ];
    }
    first_start_import_from(data, profiles, || steam_source(data), running, now, log_miss)
}

/// Whether profile `profile` has to play on Steam's saves for now: the first-start import has not found
/// a Steam save yet and the profile's launcher save is empty, so redirecting the game to it would start
/// a new profile beside a Steam save the launcher could not find. EA app copies have no Steam save.
pub fn steam_save_pending(data: &Path, profile: &str) -> bool {
    if !import_pending(data) || profile == crate::profile::VANILLA {
        return false;
    }
    let config = crate::store::Store::new(data.to_path_buf()).config();
    if crate::copies::find_active(&config).is_some_and(|c| c.source == crate::install::Source::EaApp) {
        return false;
    }
    !has_files(&Location::launcher(data, profile))
}

pub fn first_start_import_from(
    data: &Path,
    profiles: &[String],
    source: impl FnOnce() -> (Option<Location>, Vec<String>),
    running: Running,
    now: i64,
    log_miss: bool,
) -> Vec<String> {
    if marker_path(data).exists() {
        return Vec::new();
    }
    if running.game {
        return vec!["saves: F1 25 is running; the import of the Steam save into the launcher saves waits for the next launcher start".into()];
    }
    let mut out = Vec::new();
    let mut done = Vec::new();
    let note;
    let mut found = false;
    let (from, tried) = source();
    match from {
        None => {
            note = "no Steam save of the active copy".to_owned();
            out.push(format!(
                "saves: no Steam save found to import into the launcher saves ({note}); these profiles play on Steam's saves until an import finds one, and the import is tried again quietly"
            ));
            out.extend(
                tried
                    .iter()
                    .map(|t| format!("saves: looked for the Steam save: {t}")),
            );
        }
        Some(from) => {
            note = format!("imported from {}", from.dir.display());
            for p in profiles {
                if p == crate::profile::VANILLA {
                    continue;
                }
                let to = store_for(data, p, &from).expect("every layout has a store");
                match seed(data, p, &from, Reason::Imported, now) {
                    Ok(s) => {
                        found |= !matches!(s, Seeded::NoSource);
                        if matches!(s, Seeded::Copied { .. }) {
                            done.push(p.clone());
                        }
                        out.push(s.line(p, &from, &to));
                    }
                    Err(e) => {
                        out.push(format!(
                            "saves: could not import the Steam save into profile {p}: {e}"
                        ));
                        return out;
                    }
                }
            }
        }
    }
    if !found {
        return match log_miss {
            true => out,
            false => Vec::new(),
        };
    }
    let m = Marker {
        at: now,
        launcher: env!("CARGO_PKG_VERSION").to_owned(),
        profiles: done,
        note,
        seen: false,
    };
    if let Err(e) = crate::write_json(&marker_path(data), &m) {
        out.push(format!("saves: could not record the import: {e}"));
    }
    out
}

/// Explicit import into `profile`'s store, replacing what it holds after the vault has kept it.
pub fn import(
    data: &Path,
    profile: &str,
    from: &Location,
    reason: Reason,
    running: Running,
    now: i64,
) -> Result<(usize, u64, Option<String>)> {
    if !supported(from.layout) {
        return Err(Error::Other(EA_NOTE.to_owned()));
    }
    if let Some(why) = crate::vault::restore::blocker(Layout::Launcher, running) {
        return Err(Error::Other(why));
    }
    let to = store_for(data, profile, from).expect("every layout has a store");
    if !has_files(from) {
        return Err(Error::Other(format!(
            "no profile save in {} to import",
            from.dir.display()
        )));
    }
    crate::save_stall::refuse(data, from)?;
    let vault = Vault::of(data, &to)?;
    if has_files(&to) {
        vault.snapshot(
            &to,
            Meta::new(Reason::BeforeRestore)
                .profile(profile)
                .pin("before-import")
                .note("before an import"),
            now,
        )?;
    }
    let (files, bytes) = copy_files(from, &to)?;
    let meta = Meta::new(reason)
        .profile(profile)
        .note(format!("copied from {}", from.dir.display()));
    let snapshot = match vault.snapshot(&to, meta, now)? {
        Taken::New(s) | Taken::Unchanged(s) => Some(s.id),
        Taken::NoSave => None,
    };
    Ok((files, bytes, snapshot))
}

/// Copy the save at `from` over the store of `to_profile`: the Steam save, or another profile's store. The
/// store is kept in the vault first, and the copy is snapshotted as `Imported` for a Steam save and
/// `ImportedProfile` for another profile.
pub fn copy_to(
    data: &Path,
    from: &Location,
    to_profile: &str,
    running: Running,
    now: i64,
) -> Result<(usize, u64, Option<String>)> {
    let to = store_for(data, to_profile, from).expect("every layout has a store");
    if to.source == from.source {
        return Err(Error::Other(format!(
            "{to_profile} is the profile the save comes from"
        )));
    }
    let reason = match from.layout {
        Layout::Launcher | Layout::LauncherEa => Reason::ImportedProfile,
        Layout::Steam | Layout::Ea => Reason::Imported,
    };
    import(data, to_profile, from, reason, running, now)
}

/// Copy a launcher profile's store over the Steam save, through the vault's restore path: the store is
/// snapshotted, the Steam save is snapshotted as `before-restore`, and `remotecache.vdf` stays as it is.
pub fn export(
    data: &Path,
    profile: &str,
    steam: &Location,
    running: Running,
    now: i64,
) -> Result<crate::vault::restore::Restored> {
    let from = store_for(data, profile, steam).expect("every layout has a store");
    if !supported(steam.layout) {
        return Err(Error::Other(EA_NOTE.to_owned()));
    }
    if !has_files(&from) {
        return Err(Error::Other(format!(
            "the launcher saves of profile {profile} are empty; there is nothing to export"
        )));
    }
    crate::save_stall::refuse(data, &from)?;
    let lv = Vault::of(data, &from)?;
    let id = match lv.snapshot(
        &from,
        Meta::new(Reason::Manual)
            .profile(profile)
            .pin("export")
            .note("before an export to Steam"),
        now,
    )? {
        Taken::New(s) | Taken::Unchanged(s) => s.id,
        Taken::NoSave => return Err(Error::Other("the launcher saves hold no profile save".into())),
    };
    let sv = Vault::of(data, steam)?;
    crate::vault::restore::restore_across(&lv, &id, steam, &sv, running, now)
}

#[cfg(test)]
mod tests;
