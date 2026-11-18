//! Whether the game folder is the copy Steam installed. Steam keeps a depot
//! manifest for every installed depot in `depotcache`, with the size and the
//! SHA-1 of each file. The baseline's originals are checked against it before
//! a modded session builds anything, and the first time a game build is seen
//! every file Steam installed is checked by size. A file that differs stops
//! the session untouched: the launcher can only undo its own changes on top
//! of an unmodified game, so the player verifies the game files in Steam
//! first. A DLL of Steam's that is present but differs is a third-party
//! replacement (an upscaler build, a VR runtime, a graphics hook): it is
//! logged and never stops the session. A missing one still does.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::STEAM_APP_ID;
use crate::baseline::{Baseline, DepotHashes, Objects, Source, sha256_file};
use crate::install::Game;
use crate::placed::Placed;
use crate::steam::GameInstall;
use crate::vdf::Vdf;
use crate::{Result, io_err, read_json, write_json};

const PAYLOAD: u32 = 0x71F6_17D0;
const METADATA: u32 = 0x1F48_12BE;
const END: u32 = 0x32C4_15AB;
const DIRECTORY: u32 = 0x40;

/// Opens Steam's Verify integrity of game files for F1 25.
pub const STEAM_VERIFY_URL: &str = "steam://validate/3059520";

/// EA AntiCheat's launcher files sit in the game root as `EAAntiCheat.*`. EA
/// rewrites the launcher exe and its dll on a normal start, so their sizes
/// drift from Steam's. Modded sessions never use them, so a drifted one is
/// logged and never blocks; offline play parks and restores the exe itself.
pub const EAC_LAUNCHER: &str = "EAAntiCheat.GameServiceLauncher.exe";

fn is_eac(rel: &str) -> bool {
    !rel.contains('/') && rel.to_ascii_lowercase().starts_with("eaanticheat.")
}

/// Whether `rel` is a DLL, by its extension in any case. A DLL of Steam's
/// that differs from Steam's copy is a third-party replacement, such as
/// OpenComposite's `openvr_api.dll`, an FSR 4 build of
/// `amd_fidelityfx_dx12.dll` or a DLSS swap.
fn is_dll(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
}

/// Upscaler drop-ins (FSR 4 builds such as HelixSR) replace the game's
/// `amd_fidelityfx_dx12.dll` in the game folder. The launcher never writes or
/// restores it, and no addon or profile can replace it.
pub const UPSCALER_DROP_INS: &[&str] = &["amd_fidelityfx_dx12.dll"];

pub(crate) fn is_upscaler_drop_in(rel: &str) -> bool {
    !rel.contains('/')
        && !rel.contains('\\')
        && UPSCALER_DROP_INS
            .iter()
            .any(|name| name.eq_ignore_ascii_case(rel))
}

fn modified_secs(path: &Path) -> Option<f64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub size: u64,
    pub sha1: [u8; 20],
}

/// Every file of the installed depots, by its path in the game folder with
/// `/` separators.
#[derive(Debug, Default)]
pub struct Depots {
    files: HashMap<String, Entry>,
    lower: HashMap<String, String>,
}

impl Depots {
    pub fn from_entries(entries: impl IntoIterator<Item = (String, Entry)>) -> Self {
        let mut d = Self::default();
        for (rel, e) in entries {
            d.insert(rel, e);
        }
        d
    }

    fn insert(&mut self, rel: String, e: Entry) {
        self.lower.insert(rel.to_lowercase(), rel.clone());
        self.files.insert(rel, e);
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get(&self, rel: &str) -> Option<&Entry> {
        self.files.get(rel).or_else(|| {
            self.lower
                .get(&rel.to_lowercase())
                .and_then(|r| self.files.get(r))
        })
    }

    /// The manifests of the depots Steam lists as installed for the game, or
    /// why they can't be read. Any depot without a readable manifest makes the
    /// whole list unknown, so a partial list never reports a file as foreign.
    pub fn load(install: &GameInstall) -> std::result::Result<Self, String> {
        let acf = install
            .library
            .join("steamapps")
            .join(format!("appmanifest_{STEAM_APP_ID}.acf"));
        let text = std::fs::read_to_string(&acf).map_err(|e| format!("{}: {e}", acf.display()))?;
        let doc = crate::vdf::parse(&text).ok_or_else(|| format!("{} does not parse", acf.display()))?;
        let depots = doc
            .path(&["AppState", "InstalledDepots"])
            .ok_or_else(|| format!("{} lists no installed depots", acf.display()))?;
        let mut out = Self::default();
        let mut any = false;
        for (depot, v) in depots.entries() {
            let gid = v
                .get("manifest")
                .and_then(Vdf::str)
                .ok_or_else(|| format!("depot {depot} has no manifest id"))?;
            let name = format!("{depot}_{gid}.manifest");
            let path = [&install.steam_root, &install.library]
                .iter()
                .map(|r| r.join("depotcache").join(&name))
                .find(|p| p.is_file())
                .ok_or_else(|| format!("Steam's manifest {name} is not in depotcache"))?;
            let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            for (rel, e) in parse_manifest(&data).map_err(|e| format!("{name}: {e}"))? {
                out.insert(rel, e);
            }
            any = true;
        }
        if !any {
            return Err(format!("{} lists no installed depots", acf.display()));
        }
        Ok(out)
    }

    fn matches(&self, rel: &str, path: &Path) -> Result<Option<bool>> {
        let Some(e) = self.get(rel) else {
            return Ok(None);
        };
        let size = std::fs::metadata(path).map_err(io_err(path))?.len();
        Ok(Some(size == e.size && sha1_file(path)? == e.sha1))
    }
}

impl DepotHashes for Depots {
    fn stock(&self, rel: &str, path: &Path) -> Result<Option<bool>> {
        self.matches(rel, path)
    }
}

fn varint(b: &[u8], i: &mut usize) -> std::result::Result<u64, String> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let c = *b.get(*i).ok_or("truncated varint")?;
        *i += 1;
        v |= u64::from(c & 0x7f) << shift;
        if c < 0x80 {
            return Ok(v);
        }
    }
    Err("varint is too long".into())
}

enum Field<'a> {
    Int(u64),
    Bytes(&'a [u8]),
}

fn fields(b: &[u8]) -> std::result::Result<Vec<(u64, Field<'_>)>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let key = varint(b, &mut i)?;
        let value = match key & 7 {
            0 => Field::Int(varint(b, &mut i)?),
            1 | 5 => {
                let n = if key & 7 == 1 { 8 } else { 4 };
                let s = b.get(i..i + n).ok_or("truncated fixed field")?;
                i += n;
                Field::Bytes(s)
            }
            2 => {
                let n = varint(b, &mut i)? as usize;
                let s = b.get(i..i + n).ok_or("truncated field")?;
                i += n;
                Field::Bytes(s)
            }
            t => return Err(format!("wire type {t}")),
        };
        out.push((key >> 3, value));
    }
    Ok(out)
}

/// A depot manifest as Steam stores it in `depotcache`: tagged sections, the
/// payload a protobuf list of files (name 1, size 2, flags 3, content SHA-1
/// 5). Directories are left out.
pub fn parse_manifest(data: &[u8]) -> std::result::Result<Vec<(String, Entry)>, String> {
    let mut out = Vec::new();
    let mut encrypted = false;
    let mut i = 0;
    let mut payload = false;
    while i + 8 <= data.len() {
        let magic = u32::from_le_bytes(data[i..i + 4].try_into().expect("4 bytes"));
        let len = u32::from_le_bytes(data[i + 4..i + 8].try_into().expect("4 bytes")) as usize;
        let body = data.get(i + 8..i + 8 + len).ok_or("truncated section")?;
        i += 8 + len;
        match magic {
            PAYLOAD => {
                payload = true;
                for (f, v) in fields(body)? {
                    let (1, Field::Bytes(m)) = (f, v) else { continue };
                    let (mut name, mut size, mut flags, mut sha) = (None, 0, 0, None);
                    for (g, w) in fields(m)? {
                        match (g, w) {
                            (1, Field::Bytes(s)) => {
                                name = Some(String::from_utf8_lossy(s).replace('\\', "/"))
                            }
                            (2, Field::Int(n)) => size = n,
                            (3, Field::Int(n)) => flags = n as u32,
                            (5, Field::Bytes(s)) if s.len() == 20 => {
                                sha = Some(s.try_into().expect("20 bytes"))
                            }
                            _ => {}
                        }
                    }
                    if flags & DIRECTORY != 0 {
                        continue;
                    }
                    if let (Some(name), Some(sha1)) = (name, sha) {
                        out.push((name, Entry { size, sha1 }));
                    }
                }
            }
            METADATA => {
                for (f, v) in fields(body)? {
                    if let (4, Field::Int(n)) = (f, v) {
                        encrypted = n != 0;
                    }
                }
            }
            END => break,
            _ => {}
        }
    }
    if encrypted {
        return Err("its file names are encrypted".into());
    }
    if !payload {
        return Err("no file list".into());
    }
    Ok(out)
}

pub fn sha1_file(path: &Path) -> Result<[u8; 20]> {
    crate::worked();
    let mut f = std::fs::File::open(path).map_err(io_err(path))?;
    let mut h = Sha1::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io_err(path)(e)),
        }
    }
    Ok(h.finalize().into())
}

/// A game file that is not Steam's copy, with what was measured so a bundle
/// can tell one blocked player's case from another's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Foreign {
    pub rel: String,
    pub measured: String,
    pub size: Option<u64>,
    pub steam: u64,
    pub modified: Option<f64>,
}

/// A file of Steam's that differs from Steam's copy without blocking: an
/// `EAAntiCheat.*` file in the game root, or a third-party DLL.
#[derive(Debug, Clone, PartialEq)]
pub struct Drifted {
    pub rel: String,
    pub size: Option<u64>,
    pub steam: u64,
}

/// What `check` found: the files that are not Steam's, the EA AntiCheat files
/// and the third-party DLLs that differ without blocking, and what it did to
/// the baseline on the way.
#[derive(Debug, Default, PartialEq)]
pub struct Checked {
    pub foreign: Vec<String>,
    pub detail: Vec<Foreign>,
    pub eac: Vec<Drifted>,
    pub third_party: Vec<Drifted>,
    pub missing: Vec<String>,
    pub proven: usize,
    pub rerecorded: Vec<String>,
    pub swept: Option<usize>,
}

/// Check the game against Steam's manifests before a session builds from
/// it. Each original the baseline took at first sight is compared with
/// Steam's copy: a match is marked as proven, and a mismatch whose game
/// folder file now matches Steam (the player verified the game files) is
/// backed up again from the game folder. Until a sweep has passed for this
/// game build, every file Steam installed is also checked for its size; a
/// file the launcher itself left behind is left to the install, which clears
/// it. A DLL that is present and differs is third-party and never foreign; a
/// missing one is. The game folder is only read.
pub fn check(data: &Path, game: &Game, depots: &Depots) -> Result<Checked> {
    let mut out = Checked::default();
    let launcher = game.dir.join(EAC_LAUNCHER);
    if depots.get(EAC_LAUNCHER).is_some()
        && std::fs::symlink_metadata(&launcher).is_err()
        && std::fs::symlink_metadata(game.dir.join(format!("{EAC_LAUNCHER}.bak"))).is_err()
    {
        out.missing.push(EAC_LAUNCHER.to_owned());
    }
    let loaded = Baseline::load_or_new(data, &game.identity)?;
    let mut baseline = loaded.clone();
    let objects = Objects {
        root: data.join("baseline/objects"),
    };
    let rels: Vec<String> = baseline
        .files
        .iter()
        .filter(|(_, r)| r.source == Source::FirstSight)
        .map(|(rel, _)| rel.clone())
        .collect();
    for rel in rels {
        let Some(e) = depots.get(&rel) else { continue };
        let rec = &baseline.files[&rel];
        let backup = objects.path(&rec.sha256);
        if rec.size == e.size && backup.is_file() && sha1_file(&backup)? == e.sha1 {
            baseline.files.get_mut(&rel).expect("listed").source = Source::Depot;
            out.proven += 1;
            continue;
        }
        let live = game.dir.join(&rel);
        if depots.matches(&rel, &live)? == Some(true) {
            let sha256 = objects.put_file(&live)?;
            let r = baseline.files.get_mut(&rel).expect("listed");
            r.size = e.size;
            r.sha256 = sha256;
            r.source = Source::Depot;
            out.rerecorded.push(rel);
        } else if is_dll(&rel) {
            out.third_party.push(Drifted {
                size: std::fs::metadata(&live).ok().map(|m| m.len()),
                steam: e.size,
                rel,
            });
        } else {
            let size = std::fs::metadata(&live).ok().map(|m| m.len());
            out.detail.push(Foreign {
                rel: rel.clone(),
                measured: rel.clone(),
                size,
                steam: e.size,
                modified: modified_secs(&live),
            });
            out.foreign.push(rel);
        }
    }
    if !baseline.stock_checked {
        let placed = Placed::load(data)?;
        let mut swept = 0;
        for (rel, e) in &depots.files {
            if baseline.files.contains_key(rel) {
                continue;
            }
            swept += 1;
            let live = game.dir.join(rel);
            let size = std::fs::metadata(&live).ok().map(|m| m.len());
            if size == Some(e.size)
                || (size.is_some() && placed.get(rel).is_some() && placed.wrote(rel, &sha256_file(&live)?))
            {
                continue;
            }
            if is_eac(rel) {
                out.eac.push(Drifted {
                    rel: rel.clone(),
                    size,
                    steam: e.size,
                });
                continue;
            }
            if size.is_some() && is_dll(rel) {
                out.third_party.push(Drifted {
                    rel: rel.clone(),
                    size,
                    steam: e.size,
                });
                continue;
            }
            out.detail.push(Foreign {
                rel: rel.clone(),
                measured: rel.clone(),
                size,
                steam: e.size,
                modified: modified_secs(&live),
            });
            out.foreign.push(rel.clone());
        }
        out.swept = Some(swept);
        baseline.stock_checked = out.foreign.is_empty();
    }
    out.foreign.sort();
    out.foreign.dedup();
    out.detail.sort_by(|a, b| a.rel.cmp(&b.rel));
    out.eac.sort_by(|a, b| a.rel.cmp(&b.rel));
    out.third_party.sort_by(|a, b| a.rel.cmp(&b.rel));
    if baseline != loaded {
        baseline.save(data)?;
    }
    Ok(out)
}

/// The game files a session found not to be Steam's, for the launcher to show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub at: f64,
    pub files: Vec<String>,
    pub game: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detail: Vec<Foreign>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub missing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Report {
    pub fn new(files: Vec<String>, game: &Path) -> Self {
        Self {
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64()),
            files,
            game: game.to_path_buf(),
            detail: Vec::new(),
            missing: false,
            version: None,
        }
    }

    pub fn missing(files: Vec<String>, game: &Path, version: Option<String>) -> Self {
        Self {
            missing: true,
            version,
            ..Self::new(files, game)
        }
    }

    pub fn mixed(&self) -> bool {
        if self.missing {
            return false;
        }
        let (changed, missing) = split_absent(&self.detail);
        !changed.is_empty() && !missing.is_empty()
    }

    pub fn fix(&self) -> Fix {
        fix_for(self.version.as_deref())
    }

    pub fn with_detail(mut self, detail: Vec<Foreign>) -> Self {
        self.detail = detail;
        self
    }

    pub fn path(data: &Path) -> PathBuf {
        data.join("state/not-stock.json")
    }

    pub fn write(&self, data: &Path) -> Result<()> {
        write_json(&Self::path(data), self)
    }

    pub fn read(data: &Path) -> Option<Self> {
        read_json(&Self::path(data)).ok()
    }

    pub fn clear(data: &Path) -> Result<()> {
        let p = Self::path(data);
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_err(&p)(e)),
            _ => Ok(()),
        }
    }

    pub fn title(&self) -> &'static str {
        "Your game files aren't the ones Steam installed"
    }

    pub fn text(&self) -> String {
        let (changed, missing) = split_absent(&self.detail);
        if self.missing || (!missing.is_empty() && changed.is_empty() && missing.len() == self.files.len()) {
            return format!(
                "F1 25 was not started. {}. {}.",
                missing_subject(&self.files),
                capitalise(&missing_fix(self.files.len(), self.version.as_deref()))
            );
        }
        if !missing.is_empty() {
            return format!(
                "F1 25 was not started. {} changed outside the launcher and {}. Verifying the game files in Steam puts Steam's copies back: open {STEAM_VERIFY_URL}, or right-click F1 25 in Steam > Properties > Installed Files > Verify integrity of game files. Then press Play again.",
                changed_count(changed.len()),
                missing_subject(&missing)
            );
        }
        format!(
            "F1 25 was not started. {} {} changed outside the launcher, usually by a mod copied \
             into the game folder by hand. Verifying the game files in Steam puts Steam's copies \
             back: open {STEAM_VERIFY_URL}, or right-click F1 25 in Steam > Properties > Installed \
             Files > Verify integrity of game files. Then press Play again.",
            names(&self.files),
            if self.files.len() == 1 { "was" } else { "were" }
        )
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn changed_count(n: usize) -> String {
    match n {
        1 => "1 game file was".to_owned(),
        n => format!("{n} game files were"),
    }
}

/// What puts missing game files back: the latest update when the game is
/// older than it, a verify when it is that update, and both when the
/// version can't be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    Update,
    Verify,
    Both,
}

pub fn fix_for(version: Option<&str>) -> Fix {
    match version
        .and_then(|v| crate::known_problems::older_game(v, crate::known_problems::LATEST_GAME_VERSION))
    {
        Some(true) => Fix::Update,
        Some(false) => Fix::Verify,
        None => Fix::Both,
    }
}

pub fn missing_text(files: &[String], version: Option<&str>) -> String {
    format!(
        "{} from the game folder; nothing in the game folder was changed; {}, then try again",
        missing_subject(files),
        missing_fix(files.len(), version)
    )
}

fn missing_subject(files: &[String]) -> String {
    match files.len() {
        1 => format!("{} is missing", names(files)),
        2 | 3 => format!("{} are missing", names(files)),
        n => format!("{n} game files are missing"),
    }
}

/// Split the files a check flagged into those whose bytes differ from Steam's
/// and those that are not in the game folder at all.
pub fn split_absent(detail: &[Foreign]) -> (Vec<String>, Vec<String>) {
    let (absent, changed): (Vec<&Foreign>, Vec<&Foreign>) = detail.iter().partition(|f| f.size.is_none());
    let rels = |v: Vec<&Foreign>| v.into_iter().map(|f| f.rel.clone()).collect();
    (rels(changed), rels(absent))
}

pub fn changed_and_missing_text(changed: &[String], missing: &[String], version: Option<&str>) -> String {
    format!(
        "{} not the copy Steam installed, usually after a mod was copied into the game folder by hand, and {}; nothing in the game folder was changed; {}, then try again",
        changed_subject(changed),
        missing_subject(missing),
        missing_fix(changed.len() + missing.len(), version)
    )
}

fn changed_subject(files: &[String]) -> String {
    match files.len() {
        1 => format!("{} is", names(files)),
        2 | 3 => format!("{} are", names(files)),
        n => format!("{n} game files are"),
    }
}

/// How to put `n` missing game files back on a game of `version`.
pub fn missing_fix(n: usize, version: Option<&str>) -> String {
    let them = if n == 1 { "it" } else { "them" };
    let verify = format!(
        "verify the game files in Steam (open {STEAM_VERIFY_URL}, or F1 25 > Properties > Installed Files > \
         Verify integrity of game files) or repair the game in the EA app"
    );
    let latest = crate::known_problems::LATEST_GAME_VERSION;
    match (fix_for(version), version) {
        (Fix::Update, Some(v)) => {
            format!(
                "F1 25 {v} is older than {latest}; update F1 25 in Steam or the EA app to put {them} back"
            )
        }
        (Fix::Verify, _) => format!("{verify} to put {them} back"),
        _ => format!("update F1 25 in Steam or the EA app, then {verify} to put {them} back"),
    }
}

pub fn names(files: &[String]) -> String {
    let short: Vec<&str> = files.iter().map(|f| f.rsplit('/').next().unwrap_or(f)).collect();
    match short.as_slice() {
        [] => "Game files".to_owned(),
        [a] => (*a).to_owned(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, c, rest @ ..] => format!("{a}, {b}, {c} and {} more files", rest.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlls_are_matched_by_extension_in_any_case_and_folder() {
        for rel in [
            "openvr_api.dll",
            "AMD_FidelityFX_DX12.DLL",
            "nvngx_dlss.Dll",
            "bin/vendor.dll",
            "bin\\vendor.dll",
        ] {
            assert!(is_dll(rel), "{rel}");
        }
        for rel in [
            crate::GAME_EXE,
            "amd_fidelityfx_dx12.dll.bak",
            "readme.dll.txt",
            "dll",
            "2025_asset_groups/ui_package/databases.erp",
        ] {
            assert!(!is_dll(rel), "{rel}");
        }
    }

    #[test]
    fn the_drop_in_names_are_root_level_and_case_insensitive() {
        assert!(is_upscaler_drop_in("amd_fidelityfx_dx12.dll"));
        assert!(is_upscaler_drop_in("AMD_FidelityFX_DX12.DLL"));
        assert!(!is_upscaler_drop_in("bin/amd_fidelityfx_dx12.dll"));
        assert!(!is_upscaler_drop_in("bin\\amd_fidelityfx_dx12.dll"));
        assert!(!is_upscaler_drop_in("amd_fidelityfx_dx12.dll.bak"));
        assert!(!is_upscaler_drop_in("amd_fidelityfx_upscaler_dx12.dll"));
    }

    #[test]
    fn the_protected_files_are_never_drop_ins() {
        for name in [
            crate::GAME_EXE,
            crate::entitlement::STEAM_API,
            crate::install::PROXY,
        ] {
            assert!(!is_upscaler_drop_in(name), "{name}");
            assert!(
                !UPSCALER_DROP_INS.iter().any(|n| n.eq_ignore_ascii_case(name)),
                "{name}"
            );
        }
        assert!(!is_upscaler_drop_in("swofty/addons/core/swofty_core.dll"));
    }
}
