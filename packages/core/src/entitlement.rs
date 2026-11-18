//! Whether a copy of F1 25 came from a store that checks who owns it.
//!
//! Steam and the EA app are what make sure the player owns the game: Steam's
//! client runs the game's licence check through `steam_api64.dll`, and the EA
//! app only launches a game its account owns. Modded sessions therefore run
//! only on a copy one of them installed, and an EA app copy starts through
//! the EA app. A copy from anywhere else, one carrying the files of a Steam
//! emulator or crack, and a Steam copy whose game or Steam API files are not
//! the ones Steam installed are refused before anything is built or staged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::install::Source;
use crate::stock::{Depots, sha1_file};
use crate::{Error, GAME_EXE, Result, io_err, read_json, write_json};

pub const STEAM_API: &str = "steam_api64.dll";

/// The files the game's Steam licence check runs through. Each is compared
/// with Steam's own copy on every modded launch.
pub const OWNERSHIP_FILES: &[&str] = &[GAME_EXE, STEAM_API];

/// What Steam emulators and cracks leave in the game folder, matched without
/// case. Steam and the EA app install none of them with F1 25.
pub const CRACK_FILES: &[&str] = &[
    "steam_settings",
    "steam_emu.ini",
    "steam_api64_o.dll",
    "steam_interfaces.txt",
    "ColdClientLoader.ini",
    "steamclient_loader_x64.exe",
    "steamclient64.dll",
    "OnlineFix64.dll",
    "OnlineFix.ini",
    "cream_api.ini",
    "CODEX64.dll",
    "SmartSteamEmu.ini",
];

const OWNED_BY: &str = "Swofty runs only on a copy you own on Steam or the EA app";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Neither Steam nor the EA app installed the copy.
    UnknownCopy,
    /// An EA app copy was asked to start without the EA app.
    EaWithoutApp,
    /// Files of a Steam emulator or crack are in the game folder.
    CrackFiles(Vec<String>),
}

impl Refusal {
    pub fn text(&self) -> String {
        match self {
            Self::UnknownCopy => format!(
                "this copy of F1 25 wasn't installed by Steam or the EA app, so it can't be played modded; {OWNED_BY}"
            ),
            Self::EaWithoutApp => {
                "the EA app's copy of F1 25 starts through the EA app, which checks that you own \
                                   it; press Play in Swofty, or run swofty run without --direct"
                    .into()
            }
            Self::CrackFiles(files) => format!(
                "the game folder has files from a Steam emulator or crack ({}), so F1 25 can't be played modded; \
                 {OWNED_BY}",
                files.join(", ")
            ),
        }
    }
}

/// The emulator or crack files in the root of the game folder. A name that
/// Steam's list for the install has is Steam's own and is left out.
pub fn crack_files(dir: &Path, depots: Option<&Depots>) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io_err(dir))? {
        let name = entry
            .map_err(io_err(dir))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if CRACK_FILES.iter().any(|c| c.eq_ignore_ascii_case(&name))
            && depots.is_none_or(|d| d.get(&name).is_none())
        {
            found.push(name);
        }
    }
    found.sort();
    Ok(found)
}

/// Refuse a modded session on a copy neither store installed, or one with
/// the files of a Steam emulator or crack in it.
pub fn check(dir: &Path, depots: Option<&Depots>) -> Result<()> {
    if Source::detect(dir) == Source::Unknown {
        return Err(Error::NotEntitled(Refusal::UnknownCopy));
    }
    let found = crack_files(dir, depots)?;
    if !found.is_empty() {
        return Err(Error::NotEntitled(Refusal::CrackFiles(found)));
    }
    Ok(())
}

/// Refuse an EA app copy that would start without the EA app.
pub fn check_store_start(dir: &Path, via_ea_app: bool) -> Result<()> {
    if Source::detect(dir) == Source::EaApp && !via_ea_app {
        return Err(Error::NotEntitled(Refusal::EaWithoutApp));
    }
    Ok(())
}

fn hashed_path(data: &Path) -> PathBuf {
    data.join("state/ownership-files.json")
}

fn hashed_key(path: &Path, sha1: &[u8; 20]) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let steam: String = sha1.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!("{}|{modified}|{steam}", meta.len()))
}

/// The [`OWNERSHIP_FILES`] in the game folder that are not the copy Steam
/// installed, by size and SHA-1. A file that matched before and has kept its
/// size and modified time is not hashed again.
pub fn changed_files(data: &Path, dir: &Path, depots: &Depots) -> Result<Vec<String>> {
    let path = hashed_path(data);
    let mut hashed: BTreeMap<String, String> = read_json(&path).unwrap_or_default();
    let mut changed = Vec::new();
    let mut learned = false;
    for rel in OWNERSHIP_FILES {
        let Some(e) = depots.get(rel) else { continue };
        let live = dir.join(rel);
        let key = hashed_key(&live, &e.sha1);
        if key.is_some() && hashed.get(*rel) == key.as_ref() {
            continue;
        }
        let size = std::fs::metadata(&live).ok().map(|m| m.len());
        if size == Some(e.size) && sha1_file(&live)? == e.sha1 {
            if let Some(key) = key {
                hashed.insert((*rel).to_owned(), key);
                learned = true;
            }
            continue;
        }
        hashed.remove(*rel);
        changed.push((*rel).to_owned());
    }
    if learned || !changed.is_empty() {
        write_json(&path, &hashed)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stock::Entry;
    use sha1::{Digest, Sha1};

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-entitlement-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn steam_copy(root: &Path) -> PathBuf {
        let dir = root.join("F1 25");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(GAME_EXE), b"MZ game").unwrap();
        std::fs::write(dir.join(STEAM_API), b"MZ steam api").unwrap();
        std::fs::write(dir.join("EAJavelinInstaller_installscript.vdf"), b"").unwrap();
        dir
    }

    fn entry(bytes: &[u8]) -> Entry {
        Entry {
            size: bytes.len() as u64,
            sha1: Sha1::digest(bytes).into(),
        }
    }

    fn depots() -> Depots {
        Depots::from_entries([
            (GAME_EXE.to_owned(), entry(b"MZ game")),
            (STEAM_API.to_owned(), entry(b"MZ steam api")),
        ])
    }

    #[test]
    fn refuses_a_copy_neither_store_installed() {
        let root = scratch("unknown");
        let dir = root.join("F1 25");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(GAME_EXE), b"MZ").unwrap();
        assert!(matches!(
            check(&dir, None),
            Err(Error::NotEntitled(Refusal::UnknownCopy))
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn accepts_store_copies_without_crack_files() {
        let root = scratch("stores");
        let steam = steam_copy(&root);
        check(&steam, Some(&depots())).unwrap();
        let ea = root.join("EA/F1 25");
        std::fs::create_dir_all(ea.join("__Installer")).unwrap();
        std::fs::write(ea.join(GAME_EXE), b"MZ").unwrap();
        std::fs::write(ea.join("__Installer/installerdata.xml"), b"<game/>").unwrap();
        check(&ea, None).unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn refuses_emulator_and_crack_files_whatever_their_case() {
        let root = scratch("crack");
        let dir = steam_copy(&root);
        std::fs::create_dir_all(dir.join("steam_settings")).unwrap();
        std::fs::write(dir.join("onlinefix64.dll"), b"").unwrap();
        match check(&dir, Some(&depots())) {
            Err(Error::NotEntitled(Refusal::CrackFiles(files))) => {
                assert_eq!(files, ["onlinefix64.dll", "steam_settings"])
            }
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_file_steam_installed_is_never_a_crack_file() {
        let root = scratch("listed");
        let dir = steam_copy(&root);
        std::fs::write(dir.join("steamclient64.dll"), b"steam's").unwrap();
        let listed = Depots::from_entries([("steamclient64.dll".to_owned(), entry(b"steam's"))]);
        assert!(crack_files(&dir, Some(&listed)).unwrap().is_empty());
        assert_eq!(crack_files(&dir, None).unwrap(), ["steamclient64.dll"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_ea_app_copy_starts_only_through_the_ea_app() {
        let root = scratch("ea");
        let ea = root.join("F1 25");
        std::fs::create_dir_all(ea.join("__Installer")).unwrap();
        std::fs::write(ea.join(GAME_EXE), b"MZ").unwrap();
        std::fs::write(ea.join("__Installer/installerdata.xml"), b"<game/>").unwrap();
        check_store_start(&ea, true).unwrap();
        assert!(matches!(
            check_store_start(&ea, false),
            Err(Error::NotEntitled(Refusal::EaWithoutApp))
        ));
        check_store_start(&steam_copy(&root.join("steam")), false).unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn finds_game_and_steam_api_files_that_are_not_steams() {
        let root = scratch("changed");
        let data = root.join("data");
        let dir = steam_copy(&root);
        assert!(changed_files(&data, &dir, &depots()).unwrap().is_empty());
        std::fs::write(dir.join(STEAM_API), b"MZ emulator!").unwrap();
        assert_eq!(changed_files(&data, &dir, &depots()).unwrap(), [STEAM_API]);
        std::fs::remove_file(dir.join(GAME_EXE)).unwrap();
        assert_eq!(
            changed_files(&data, &dir, &depots()).unwrap(),
            [GAME_EXE, STEAM_API]
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_upscaler_drop_in_changes_nothing_here_and_a_swapped_steam_api_still_shows() {
        let root = scratch("upscaler");
        let data = root.join("data");
        let dir = steam_copy(&root);
        std::fs::write(dir.join("amd_fidelityfx_dx12.dll"), b"a drop-in fsr 4 loader").unwrap();
        assert!(changed_files(&data, &dir, &depots()).unwrap().is_empty());
        assert!(crack_files(&dir, None).unwrap().is_empty());
        std::fs::write(dir.join(STEAM_API), b"MZ emulator!").unwrap();
        assert_eq!(changed_files(&data, &dir, &depots()).unwrap(), [STEAM_API]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_same_size_swap_is_caught_after_a_match_was_remembered() {
        let root = scratch("cache");
        let data = root.join("data");
        let dir = steam_copy(&root);
        assert!(changed_files(&data, &dir, &depots()).unwrap().is_empty());
        let api = dir.join(STEAM_API);
        let modified = std::fs::metadata(&api).unwrap().modified().unwrap();
        std::fs::write(&api, b"MZ steam ap!").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&api)
            .unwrap()
            .set_modified(modified + std::time::Duration::from_secs(1))
            .unwrap();
        assert_eq!(changed_files(&data, &dir, &depots()).unwrap(), [STEAM_API]);
        std::fs::remove_dir_all(&root).ok();
    }
}
