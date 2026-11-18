use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::STEAM_APP_ID;
use crate::asset::Kind;
use crate::update::UpdateConfig;

pub const DEFAULT_WINEDEBUG: &str = "+timestamp,+pid,+tid,+loaddll,fixme-all";
pub const LOADER_LOG: &str = "swofty/loader.log";
/// Where in the data folder the loader logs when the game folder refuses `LOADER_LOG`.
pub const LOADER_FALLBACK: &str = "state/loader-fallback.log";
pub const LOADER_MARK: &str = "loader_mark.json";
pub const PERF_LOG: &str = "swofty/perf.log";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub schema: u32,
    pub ticket: String,
    pub expires: String,
    pub mode: String,
    pub game: GameRef,
    pub profile: String,
    pub overlay: Overlay,
    #[serde(default)]
    pub plugins: Vec<PluginPin>,
    #[serde(default)]
    pub options: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    pub ui_bridge: UiBridge,
    pub policy: Policy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameRef {
    pub app_id: u32,
    pub build: String,
    pub exe_sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Overlay {
    pub root: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginPin {
    pub id: String,
    pub dll: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UiBridge {
    #[serde(default)]
    pub notify_map: BTreeMap<String, (String, String, serde_json::Value)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    pub offline: String,
    pub save_namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub launch: LaunchConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    /// The folder Locate F1 25 saved before 1.3.0; [`Config::migrate_games`]
    /// moves it into `games`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Games::is_empty")]
    pub games: Games,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub console: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub developer: bool,
    /// Opt-in: clear old game session logs each time the launcher starts (see
    /// `data_usage::prune_sessions`). Off unless the player turns it on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_old_session_logs: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sidebar_list: bool,
    /// The layout the player chose on first run; `None` until they choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_mode: Option<UiMode>,
    /// The interface scale in percent, 1 to 400; `None` is Auto, the scale the display mode gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_scale: Option<u16>,
    /// The code of the language the launcher shows; `None` is English.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The version of the welcome wizard the player last finished or skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboarding: Option<u32>,
    /// Set by "Don't ask again" on the prompt that offers addons and saves found in another folder.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_find_prompt: bool,
    #[serde(default)]
    pub data_version: u32,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub runtime_assets: BTreeSet<Kind>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub build_assets: BTreeSet<Kind>,
    #[serde(default, skip_serializing_if = "UpdateConfig::is_default")]
    pub updates: UpdateConfig,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub vr_profiles: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord_username: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub news_seen: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub dismissed_cards: BTreeSet<String>,
    /// The career vault's size cap in MB; 500 when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_cap_mb: Option<u64>,
    /// Maximum simultaneous Addon Browser install jobs; two when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_jobs: Option<usize>,
    /// Upload completed Time Trial laps to the Swofty leaderboards while signed in; on unless turned off.
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub time_trial_boards: bool,
    /// Show the Swofty boards and rival ghosts inside Time Trial; on unless turned off.
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub time_trial_ingame: bool,
    /// Show the latest Swofty video card on the F1 25 home screen; on unless turned off.
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub home_video: bool,
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub use_profiles: bool,
    /// Offer Addon Browser updates for installed addons that are off as well; off unless turned on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub update_off_addons: bool,
}

fn on() -> bool {
    true
}

fn is_on(v: &bool) -> bool {
    *v
}

impl Default for Config {
    fn default() -> Self {
        serde_json::from_str("{}").expect("every field of Config has a default")
    }
}

/// The launcher's two layouts: the desktop one, and TV mode for handhelds and
/// couch play, which scales the interface up and widens the scrollbars.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    #[default]
    Pc,
    Tv,
}

impl UiMode {
    pub fn is_tv(self) -> bool {
        self == Self::Tv
    }
}

/// The smallest interface scale in percent.
pub const UI_SCALE_MIN: u16 = 1;

/// The largest interface scale in percent.
pub const UI_SCALE_MAX: u16 = 400;

/// `percent` held to the range the launcher supports, so a hand-edited value stays usable.
pub fn clamp_ui_scale(percent: u16) -> u16 {
    percent.clamp(UI_SCALE_MIN, UI_SCALE_MAX)
}

/// The version of the welcome wizard. Raise it to show the wizard once more to everyone.
pub const ONBOARDING_VERSION: u32 = 2;

impl Config {
    /// Whether the welcome wizard is due: never seen, or an older version.
    pub fn needs_onboarding(&self) -> bool {
        self.onboarding.is_none_or(|seen| seen < ONBOARDING_VERSION)
    }

    /// The chosen interface scale in percent, held to 1 to 400; `None` is Auto.
    pub fn ui_scale_percent(&self) -> Option<u16> {
        self.ui_scale.map(clamp_ui_scale)
    }

    /// The layout in use: what the player chose, PC until they do.
    pub fn ui_mode_or_default(&self) -> UiMode {
        self.ui_mode.unwrap_or_default()
    }
}

/// The copies of F1 25 the player added by folder, and the one Play uses.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Games {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<PathBuf>,
}

impl Games {
    pub fn is_empty(&self) -> bool {
        self.active.is_none() && self.added.is_empty()
    }

    pub fn add(&mut self, dir: PathBuf) {
        if !self.added.contains(&dir) {
            self.added.push(dir);
        }
    }
}

impl Config {
    /// Return a valid limit even if a config was edited by hand.
    pub fn download_jobs(&self) -> usize {
        self.download_jobs.unwrap_or(2).clamp(1, 10)
    }

    pub fn set_download_jobs(&mut self, limit: usize) {
        self.download_jobs = Some(limit.clamp(1, 10));
    }

    pub fn migrate_games(&mut self) {
        if let Some(dir) = self.game_dir.take() {
            self.games.active.get_or_insert_with(|| dir.clone());
            self.games.add(dir);
        }
    }

    pub fn launch_mode(&self, profile: &str) -> LaunchMode {
        if self.vr_profiles.contains(profile) {
            LaunchMode::Vr
        } else {
            LaunchMode::Normal
        }
    }

    pub fn set_launch_mode(&mut self, profile: &str, mode: LaunchMode) {
        match mode {
            LaunchMode::Vr => self.vr_profiles.insert(profile.to_owned()),
            LaunchMode::Normal => self.vr_profiles.remove(profile),
        };
    }
}

/// Steam's own launch options for app 3059520 (`appinfo.vdf`, `config` →
/// `launch`): option 0 runs `EAAntiCheat.GameServiceLauncher.exe` with no
/// arguments, and option 1, of type `vr`, runs the same launcher with `-vr`.
pub const VR_ARG: &str = "-vr";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchMode {
    #[default]
    Normal,
    Vr,
}

impl LaunchMode {
    pub fn of(game_args: &[String]) -> Self {
        if game_args.iter().any(|a| a.eq_ignore_ascii_case(VR_ARG)) {
            Self::Vr
        } else {
            Self::Normal
        }
    }

    pub fn args(self) -> &'static [&'static str] {
        match self {
            Self::Normal => &[],
            Self::Vr => &[VR_ARG],
        }
    }

    pub fn is_vr(self) -> bool {
        self == Self::Vr
    }
}

/// The game's arguments for a launch: what Steam passed after the target
/// (its VR launch option adds `-vr` there), plus `-vr` for a VR launch when
/// Steam's command lacks it.
pub fn game_args(steam_args: &[String], mode: LaunchMode) -> Vec<String> {
    let mut out = steam_args.to_vec();
    if mode.is_vr() && !LaunchMode::of(steam_args).is_vr() {
        out.push(VR_ARG.to_owned());
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_affinity: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_winedebug")]
    pub winedebug: String,
    /// Start the store the active copy came from (Steam or the EA app) when
    /// Play finds it closed.
    #[serde(default = "yes", skip_serializing_if = "is_yes", alias = "start_steam")]
    pub start_store: bool,
    /// The command that hands an EA app launch link to the EA app, with
    /// `{url}` where the link goes. Needed on Linux, optional on Windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ea_launch_command: Option<String>,
    /// The Wine prefix the EA app runs in on Linux (the folder with
    /// `user.reg`, or the Proton compatdata folder holding `pfx`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ea_prefix: Option<PathBuf>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            cpu_affinity: None,
            env: BTreeMap::new(),
            winedebug: default_winedebug(),
            start_store: true,
            ea_launch_command: None,
            ea_prefix: None,
        }
    }
}

fn yes() -> bool {
    true
}

fn is_yes(on: &bool) -> bool {
    *on
}

/// The Windows environment block cannot exceed 32767 characters, and
/// Proton's steam helper packs the game's environment into one. A `nix
/// develop` (or CI) shell more than doubles that with build variables, and
/// the helper then creates a game process that dies during startup before
/// loading a single DLL. Build-shell variables are not part of running a
/// game, so they are dropped, and whatever remains is trimmed to fit.
///
/// The launcher's own [`COMPAT_LAYER`] is dropped too: Windows applies the
/// layers it names to every program started with it, and with one set (such
/// as `DetectorsAppHealth`) F1 25 has held System32's version.dll instead of
/// the loader. `overrides` can still set one.
pub fn game_env(
    parent: impl IntoIterator<Item = (String, String)>,
    overrides: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    const LIMIT: usize = 30_000; // under the 32767 block limit, names included

    fn is_build_shell_var(name: &str) -> bool {
        const EXACT: &[&str] = &[
            "AR",
            "AS",
            "CC",
            "CXX",
            "LD",
            "NM",
            "OBJCOPY",
            "OBJDUMP",
            "RANLIB",
            "READELF",
            "SIZE",
            "STRIP",
            "STRINGS",
            "PKG_CONFIG",
            "LIBCLANG_PATH",
            "CONFIG_SHELL",
            "HOST_PATH",
            "SOURCE_DATE_EPOCH",
            "DETERMINISTIC_BUILD",
            "IN_NIX_SHELL",
            "NIXPKGS_CMAKE_PREFIX_PATH",
            "CMAKE_INCLUDE_PATH",
            "CMAKE_LIBRARY_PATH",
            "PYTHONPATH",
            "PYTHONHASHSEED",
            "PYTHONNOUSERSITE",
            "_PYTHON_HOST_PLATFORM",
            "_PYTHON_SYSCONFIGDATA_NAME",
            "buildInputs",
            "nativeBuildInputs",
            "buildPhase",
            "builder",
            "shell",
            "shellHook",
            "stdenv",
            "out",
            "outputs",
            "name",
            "system",
            "phases",
            "patches",
            "strictDeps",
            "doCheck",
            "doInstallCheck",
            "preferLocalBuild",
            "allowSubstitutes",
            "dontAddDisableDepTrack",
            "__structuredAttrs",
            "cmakeFlags",
            "configureFlags",
            "mesonFlags",
        ];
        const PREFIXES: &[&str] = &["NIX_", "NIXPKGS_", "CMAKE_", "deps", "propagated"];
        const SUFFIX: &str = "FLAGS";
        EXACT.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p)) || name.ends_with(SUFFIX)
    }

    fn block_size(env: &BTreeMap<String, String>) -> usize {
        env.iter().map(|(k, v)| k.len() + v.len() + 2).sum()
    }

    let mut env: BTreeMap<String, String> = parent
        .into_iter()
        .filter(|(k, _)| !is_build_shell_var(k) && !is_compat_layer(k))
        .collect();
    for (k, v) in overrides {
        env.insert(k.clone(), v.clone());
    }
    // Trim whatever is left to fit: the largest values go first, and the
    // variables a game launch cannot do without are kept.
    const KEEP: &[&str] = &["PATH", "HOME", "USER", "TEMP", "TMP", "TMPDIR", "LANG"];
    while block_size(&env) > LIMIT {
        let Some(largest) = env
            .iter()
            .filter(|(k, _)| !KEEP.contains(&k.as_str()) && !overrides.contains_key(*k))
            .max_by_key(|(k, v)| k.len() + v.len())
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        env.remove(&largest);
    }
    env
}

/// The variable Windows reads a process's compatibility layers from, and
/// hands on to the programs that process starts.
pub const COMPAT_LAYER: &str = "__COMPAT_LAYER";

pub fn is_compat_layer(name: &str) -> bool {
    name.eq_ignore_ascii_case(COMPAT_LAYER)
}

/// `WINEDLLOVERRIDES` with Wine loading a native version.dll first, so the
/// loader in the game folder attaches whatever the prefix's registry holds:
/// a prefix the launcher didn't find, a version value of its own, or a
/// registry a wineserver wrote back over the launcher's. None when the value
/// already names version.dll, which is left as the player set it.
pub fn with_version_override(value: Option<&str>) -> Option<String> {
    const OVERRIDE: &str = "version=n,b";
    let names_version = |v: &str| {
        v.split(';').any(|entry| {
            let names = entry.split_once('=').map_or(entry, |(names, _)| names);
            names.split(',').any(|n| {
                let n = n.trim().trim_start_matches('*').to_ascii_lowercase();
                n == "version" || n == "version.dll"
            })
        })
    };
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) if names_version(v) => None,
        Some(v) => Some(format!("{};{OVERRIDE}", v.trim_end_matches(';'))),
        None => Some(OVERRIDE.to_owned()),
    }
}

pub const SDL2_SONAME: &str = "libSDL2-2.0.so.0";
pub const RUNTIME_LIBS_DIR: &str = "state/runtime-libs";

const HOST_LIB_DIRS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu",
    "/lib/x86_64-linux-gnu",
    "/usr/lib64",
    "/lib64",
    "/usr/lib",
    "/lib",
    "/usr/local/lib",
];

const SDL3_SONAME: &str = "libSDL3.so.0";

const STEAM_RUNTIME_LIB_DIRS: &[&str] = &[
    "ubuntu12_32/steam-runtime/usr/lib/x86_64-linux-gnu",
    "ubuntu12_32/steam-runtime/amd64/usr/lib/x86_64-linux-gnu",
];

/// The 64-bit Steam client's own SDL3, which loads X11, Wayland, udev and
/// audio at run time and links nothing past libc.
const STEAM_CLIENT_SDL3: &str = "steamrt64/libSDL3.so.0";

/// The client runtime's platform folders, one per runtime version. Their SDL2
/// is sdl2-compat, which forwards to whichever `libSDL3.so.0` loads.
const STEAM_CLIENT_PLATFORMS: &str = "steamrt64/pv-runtime/steam-runtime-steamrt";
const PLATFORM_LIB_DIR: &str = "files/lib/x86_64-linux-gnu";

/// Proton's HID bus reads wheels and pads through SDL2, which it loads by
/// soname. Under Steam it comes from Steam's runtime; a direct launch (from
/// `steam-run` on NixOS, or a host without SDL2) has none, and every
/// joystick Proton does not drive over hidraw (a G920, most wheels) is then
/// left unclaimed: the evdev side defers it to an SDL side that never
/// started, so the game sees no wheel. When the host has no 64-bit SDL2,
/// this links Steam's copy into `link_dir` and returns `library_path` with
/// that directory appended; `None` leaves the launch as it was. Steam's copy
/// is the old runtime's SDL2 where the client still has it, else the 64-bit
/// client's SDL3 under its runtime's sdl2-compat.
pub fn sdl_library_path(library_path: Option<&str>, steam_root: &Path, link_dir: &Path) -> Option<String> {
    sdl_library_path_in(library_path, HOST_LIB_DIRS, steam_root, link_dir)
}

fn sdl_library_path_in(
    library_path: Option<&str>,
    host: &[&str],
    steam_root: &Path,
    link_dir: &Path,
) -> Option<String> {
    let current = library_path.unwrap_or_default();
    let mut searched = current
        .split(':')
        .filter(|d| !d.is_empty())
        .chain(host.iter().copied());
    if searched.any(|d| is_elf64(&Path::new(d).join(SDL2_SONAME))) {
        return None;
    }
    for (source, soname) in steam_sdl(steam_root)? {
        link_library(&source, &link_dir.join(soname)).ok()?;
    }
    let dir = link_dir.display().to_string();
    Some(if current.is_empty() {
        dir
    } else {
        format!("{current}:{dir}")
    })
}

fn steam_sdl(steam_root: &Path) -> Option<Vec<(PathBuf, &'static str)>> {
    if let Some(sdl2) = STEAM_RUNTIME_LIB_DIRS
        .iter()
        .map(|d| steam_root.join(d).join(SDL2_SONAME))
        .find(|p| is_elf64(p))
    {
        return Some(vec![(sdl2, SDL2_SONAME)]);
    }
    let sdl3 = steam_root.join(STEAM_CLIENT_SDL3);
    if !is_elf64(&sdl3) {
        return None;
    }
    let mut platforms: Vec<PathBuf> = std::fs::read_dir(steam_root.join(STEAM_CLIENT_PLATFORMS))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_platform_"))
        })
        .collect();
    platforms.sort();
    let sdl2 = platforms.iter().rev().find_map(|p| {
        let mut libs: Vec<PathBuf> = std::fs::read_dir(p.join(PLATFORM_LIB_DIR))
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|f| {
                f.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(SDL2_SONAME))
            })
            .collect();
        libs.sort();
        libs.into_iter().find(|f| is_elf64(f))
    })?;
    Some(vec![(sdl2, SDL2_SONAME), (sdl3, SDL3_SONAME)])
}

fn is_elf64(path: &Path) -> bool {
    let mut head = [0u8; 5];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && head == *b"\x7fELF\x02"
}

#[cfg(unix)]
fn link_library(source: &Path, link: &Path) -> std::io::Result<()> {
    if std::fs::read_link(link).ok().as_deref() == Some(source) {
        return Ok(());
    }
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::remove_file(link) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    std::os::unix::fs::symlink(source, link)
}

#[cfg(not(unix))]
fn link_library(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

fn default_winedebug() -> String {
    DEFAULT_WINEDEBUG.into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LaunchRequest {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winedebug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub apply: bool,
    /// `SWOFTY_NETPLAY` for the game, for a wrapper Steam starts without the caller's environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netplay: Option<String>,
    /// A file of the host's row ids ([`crate::row_ids::Overrides`]) this one session's builds use, for a
    /// room launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_ids: Option<PathBuf>,
    /// Run this one session on Steam's saves, though the profile uses launcher saves.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub steam_saves_once: bool,
}

pub const END_FILE: &str = "end.json";
pub const STOP_FILE: &str = "stop_requested";

/// How a session's game ended. `Crash` needs evidence: a crash exit code,
/// or an unhandled exception the mod loader saw. F1 25's own crash handler
/// ends the game with -1, a code it also leaves when it fails while
/// closing, so -1 alone is `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ending {
    Quit,
    Crash,
    Stopped,
    Unknown,
}

impl Ending {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::Crash => "crash",
            Self::Stopped => "stopped",
            Self::Unknown => "unknown",
        }
    }
}

/// An exit code a crash leaves: an error NTSTATUS (0xC0000000 and up) other
/// than -1, or an unhandled breakpoint.
/// The exit code the core closes F1 25 with when it stops a Grand Prix or Driver Career load that would crash
/// (native/src/core/era_logic.hpp `CONTINUE_STOP_EXIT`).
pub const CORE_STOPPED_LOAD: i32 = 92;

pub fn crash_code(code: u32) -> bool {
    (0xc000_0000..0xffff_ffff).contains(&code) || code == 0x8000_0003
}

/// The session log's `end.json`: how the game ended, its exit code when the
/// session could read it, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEnd {
    pub end: Ending,
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crash_code: Option<u32>,
    pub reason: String,
}

impl SessionEnd {
    /// `code` is the exit code the session read, or None with `unread`
    /// saying why it could not. The session launcher's line in the loader
    /// log gives the game's own code, which the session lacks when the EA
    /// app started the game and gets only the low byte of under Proton.
    /// `stopped` says who closed the game on purpose.
    pub fn resolve(
        code: Option<i32>,
        unread: &str,
        stopped: Option<&str>,
        loader: Option<&crate::launch::LoaderSeen>,
    ) -> Self {
        let loader = loader.copied().unwrap_or_default();
        let code = loader.stub_exit.map(|c| c as i32).or(code);
        let end = |end, crash_code, reason: String| Self {
            end,
            exit_code: code,
            crash_code,
            reason,
        };
        let named = |x: u32| {
            crate::crash::fatal_name(x)
                .map(|n| format!("{n}, "))
                .unwrap_or_default()
        };
        let Some(c) = code else {
            return match (stopped, loader.unhandled) {
                (Some(who), _) => end(Ending::Stopped, None, format!("{who}; {unread}")),
                (None, Some(x)) => end(
                    Ending::Crash,
                    Some(x),
                    format!(
                        "the mod loader saw an unhandled exception ({}{x:#010x}); {unread}",
                        named(x)
                    ),
                ),
                (None, None) if loader.low_disk => end(
                    Ending::Crash,
                    None,
                    format!("F1 25 closed after its Insufficient Storage dialog; {unread}"),
                ),
                (None, None) => end(Ending::Unknown, None, unread.to_owned()),
            };
        };
        if let Some(who) = stopped {
            return end(
                Ending::Stopped,
                None,
                format!("{who}; F1 25 exited with code {c}"),
            );
        }
        if c == 0 {
            return end(Ending::Quit, None, "F1 25 exited with code 0".into());
        }
        if c == CORE_STOPPED_LOAD && loader.attached && loader.unhandled.is_none() {
            return end(
                Ending::Stopped,
                None,
                format!(
                    "the core closed F1 25 at the session load (exit code {c}) instead of letting it crash: the save \
                     names a grid, a list or a track this build doesn't have, and loader.log's [era] line says which"
                ),
            );
        }
        if c == crate::launcher_saves::EXIT && loader.attached && loader.unhandled.is_none() {
            return end(
                Ending::Stopped,
                None,
                format!(
                    "the core closed F1 25 before the title screen because launcher saves could not be used (exit \
                     code {c}); no save was read or written. Play once with your Steam saves, or turn off Use \
                     launcher saves for the profile; loader.log's [saves] lines say why"
                ),
            );
        }
        if let Some(x) = loader.unhandled {
            return end(
                Ending::Crash,
                Some(x),
                format!(
                    "the mod loader saw an unhandled exception ({}{x:#010x}), then F1 25 exited with code {c}",
                    named(x)
                ),
            );
        }
        let u = c as u32;
        if loader.low_disk {
            return end(
                Ending::Crash,
                None,
                format!("F1 25 closed with code {c} after its Insufficient Storage dialog"),
            );
        }
        if let Some(sentence) = crate::fast_fail::sentence(u) {
            return end(Ending::Crash, Some(u), sentence);
        }
        if crash_code(u) {
            return end(
                Ending::Crash,
                Some(u),
                format!("F1 25 exited with a crash code ({}{u:#010x})", named(u)),
            );
        }
        let seen = if loader.attached {
            "the mod loader saw no unhandled exception"
        } else {
            "the mod loader was not attached to see why"
        };
        let reason = if c == -1 {
            format!(
                "F1 25 ended itself with code -1, which its crash handler uses and which it also leaves when \
                 it fails while closing; {seen}"
            )
        } else {
            format!("F1 25 exited with code {c} ({u:#x}); {seen}")
        };
        end(Ending::Unknown, None, reason)
    }

    pub fn write(&self, dir: &Path) -> crate::Result<()> {
        crate::write_json(&dir.join(END_FILE), self)
    }
}

/// Mark the live session as stopped by the player before its game is
/// closed, so its end is reported as a stop and not a crash.
pub fn request_stop(data: &Path) {
    let Ok(dir) = std::fs::read_to_string(data.join("sessions/latest.txt")) else {
        return;
    };
    let session = SessionRecord::read(Path::new(dir.trim()));
    if session.started.is_some() && session.ended.is_none() {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64());
        std::fs::write(session.dir.join(STOP_FILE), format!("{now}\n")).ok();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub dir: PathBuf,
    pub request_id: Option<String>,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    pub exit_code: Option<i32>,
    pub end: Option<SessionEnd>,
}

impl SessionRecord {
    pub fn read(dir: &Path) -> Self {
        let read = |f: &str| {
            std::fs::read_to_string(dir.join(f))
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        Self {
            dir: dir.into(),
            request_id: read("request_id"),
            started: read("game_start").and_then(|s| s.parse().ok()),
            ended: read("game_end").and_then(|s| s.parse().ok()),
            exit_code: read("exit_code").and_then(|s| s.parse().ok()),
            end: crate::read_json(&dir.join(END_FILE)).ok(),
        }
    }

    /// Whether to show the session as a crash: only an end with evidence of
    /// one, a crash exit code, an exception the mod loader saw or the game's
    /// Insufficient Storage dialog. F1 25 also
    /// leaves -1 and other codes when it is closed, so an `Unknown` end shows
    /// as closed, not crashed.
    pub fn crashed(&self) -> bool {
        match &self.end {
            Some(e) => e.end == Ending::Crash,
            None => self.exit_code.is_some_and(|c| crash_code(c as u32)),
        }
    }

    pub fn wine_log(&self) -> PathBuf {
        self.dir.join(format!("steam-{STEAM_APP_ID}.log"))
    }

    pub fn owns_loader_log(&self, log: &Path) -> bool {
        let Some(before) = std::fs::read(self.dir.join(LOADER_MARK))
            .ok()
            .and_then(|d| serde_json::from_slice::<Option<LogMark>>(&d).ok())
        else {
            return false;
        };
        LogMark::replaced(log, before.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogMark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inode: Option<u64>,
    pub head: String,
}

impl LogMark {
    pub fn read(path: &Path) -> Option<Self> {
        let file = std::fs::File::open(path).ok()?;
        #[cfg(unix)]
        let inode = Some(std::os::unix::fs::MetadataExt::ino(&file.metadata().ok()?));
        #[cfg(not(unix))]
        let inode = None;
        let mut head = Vec::new();
        file.take(512).read_to_end(&mut head).ok()?;
        let end = head.iter().position(|&b| b == b'\n').unwrap_or(head.len());
        Some(Self {
            inode,
            head: String::from_utf8_lossy(&head[..end]).trim_end().to_owned(),
        })
    }

    pub fn replaced(path: &Path, before: Option<&Self>) -> bool {
        Self::read(path).is_some_and(|now| before != Some(&now))
    }
}

pub fn written_since(path: &Path, since: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t >= since)
}

pub fn parse_cpu_list(spec: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => out.extend(a.parse::<usize>().ok()?..=b.parse::<usize>().ok()?),
            None => out.push(part.parse().ok()?),
        }
    }
    (!out.is_empty()).then_some(out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SteamCommand {
    pub proton: Option<String>,
    pub verb: Option<String>,
    pub target: Option<usize>,
    pub argv: Vec<String>,
}

impl SteamCommand {
    pub fn game_args(&self) -> &[String] {
        self.target
            .and_then(|t| self.argv.get(t + 1..))
            .unwrap_or_default()
    }
}

pub fn parse_steam_command(argv: &[String]) -> SteamCommand {
    let proton = argv.iter().position(|a| a.ends_with("/proton"));
    let verb = proton.and_then(|i| argv.get(i + 1)).cloned();
    let target = proton.map(|i| i + 2).filter(|&i| i < argv.len()).or_else(|| {
        argv.iter()
            .rposition(|a| a.to_ascii_lowercase().ends_with(".exe"))
    });
    SteamCommand {
        proton: proton.map(|i| argv[i].clone()),
        verb,
        target,
        argv: argv.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wine_loads_a_native_version_dll_unless_the_player_set_one() {
        assert_eq!(with_version_override(None).as_deref(), Some("version=n,b"));
        assert_eq!(with_version_override(Some(" ")).as_deref(), Some("version=n,b"));
        assert_eq!(
            with_version_override(Some("dxgi=n;")).as_deref(),
            Some("dxgi=n;version=n,b")
        );
        for set in [
            "version=b",
            "d3d11,Version=n",
            "VERSION.DLL=n,b",
            "*version=b",
            "dinput8;version",
        ] {
            assert_eq!(with_version_override(Some(set)), None, "{set}");
        }
        assert_eq!(
            with_version_override(Some("versions=n")).as_deref(),
            Some("versions=n;version=n,b")
        );
    }

    #[test]
    fn config_remembers_and_clamps_download_job_limit() {
        let mut config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config.download_jobs(), 2);
        assert!(!serde_json::to_string(&config).unwrap().contains("download_jobs"));
        config.set_download_jobs(8);
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&json).unwrap().download_jobs(), 8);
        config.set_download_jobs(0);
        assert_eq!(config.download_jobs(), 1);
        config.set_download_jobs(100);
        assert_eq!(config.download_jobs(), 10);
        let edited: Config = serde_json::from_str(r#"{"download_jobs":100}"#).unwrap();
        assert_eq!(edited.download_jobs(), 10);
    }

    #[test]
    fn ui_mode_is_unchosen_until_the_player_picks() {
        let config = Config::default();
        assert_eq!(config.ui_mode, None);
        assert_eq!(config.ui_mode_or_default(), UiMode::Pc);
        assert!(!serde_json::to_string(&config).unwrap().contains("ui_mode"));
        let old: Config = serde_json::from_str(r#"{ "developer": true }"#).unwrap();
        assert_eq!(old.ui_mode, None);
    }

    #[test]
    fn onboarding_is_due_until_the_current_version_is_seen() {
        let mut config = Config::default();
        assert!(config.needs_onboarding());
        assert!(!serde_json::to_string(&config).unwrap().contains("onboarding"));
        config.onboarding = Some(ONBOARDING_VERSION - 1);
        assert!(config.needs_onboarding());
        config.onboarding = Some(ONBOARDING_VERSION);
        assert!(!config.needs_onboarding());
        let back: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.onboarding, Some(ONBOARDING_VERSION));
        let old: Config = serde_json::from_str(r#"{ "ui_mode": "tv" }"#).unwrap();
        assert!(old.needs_onboarding());
    }

    #[test]
    fn ui_mode_round_trips() {
        for mode in [UiMode::Pc, UiMode::Tv] {
            let config = Config {
                ui_mode: Some(mode),
                ..Config::default()
            };
            let json = serde_json::to_string(&config).unwrap();
            let back: Config = serde_json::from_str(&json).unwrap();
            assert_eq!(back.ui_mode, Some(mode));
            assert_eq!(back.ui_mode_or_default(), mode);
        }
        let tv: Config = serde_json::from_str(r#"{ "ui_mode": "tv" }"#).unwrap();
        assert!(tv.ui_mode_or_default().is_tv());
        assert!(!UiMode::Pc.is_tv());
    }

    #[test]
    fn ui_scale_is_auto_until_chosen_and_round_trips() {
        let config = Config::default();
        assert_eq!(config.ui_scale, None);
        assert_eq!(config.ui_scale_percent(), None);
        assert!(!serde_json::to_string(&config).unwrap().contains("ui_scale"));
        for percent in [1, 75, 90, 100, 110, 125, 150, 237, 400] {
            let config = Config {
                ui_scale: Some(percent),
                ..Config::default()
            };
            let back: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
            assert_eq!(back.ui_scale_percent(), Some(percent));
        }
        let old: Config = serde_json::from_str(r#"{ "ui_mode": "tv" }"#).unwrap();
        assert_eq!(old.ui_scale, None);
    }

    #[test]
    fn ui_scale_is_held_to_one_through_four_hundred() {
        assert_eq!(clamp_ui_scale(0), 1);
        assert_eq!(clamp_ui_scale(1), 1);
        assert_eq!(clamp_ui_scale(237), 237);
        assert_eq!(clamp_ui_scale(400), 400);
        assert_eq!(clamp_ui_scale(60000), 400);
        let low: Config = serde_json::from_str(r#"{ "ui_scale": 0 }"#).unwrap();
        assert_eq!(low.ui_scale_percent(), Some(1));
        let high: Config = serde_json::from_str(r#"{ "ui_scale": 9000 }"#).unwrap();
        assert_eq!(high.ui_scale_percent(), Some(400));
    }

    #[test]
    fn config_remembers_the_language() {
        let mut config = Config::default();
        assert!(!serde_json::to_string(&config).unwrap().contains("language"));
        config.language = Some("ru".into());
        let json = serde_json::to_string(&config).unwrap();
        let loaded = serde_json::from_str::<Config>(&json).unwrap();
        assert_eq!(loaded.language.as_deref(), Some("ru"));
    }

    #[test]
    fn config_keeps_console_flag() {
        let mut config = Config::default();
        assert!(!serde_json::to_string(&config).unwrap().contains("console"));
        config.console = true;
        let json = serde_json::to_string(&config).unwrap();
        assert!(serde_json::from_str::<Config>(&json).unwrap().console);
    }

    #[test]
    fn profiles_are_on_unless_turned_off() {
        assert!(Config::default().use_profiles);
        assert!(serde_json::from_str::<Config>("{}").unwrap().use_profiles);
        let json = serde_json::to_string(&Config {
            use_profiles: false,
            ..Config::default()
        })
        .unwrap();
        assert!(json.contains(r#""use_profiles":false"#));
        assert!(!serde_json::from_str::<Config>(&json).unwrap().use_profiles);
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("use_profiles")
        );
    }

    #[test]
    fn off_addons_are_left_out_of_updates_unless_turned_on() {
        assert!(!Config::default().update_off_addons);
        assert!(!serde_json::from_str::<Config>("{}").unwrap().update_off_addons);
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("update_off_addons")
        );
        let on = Config {
            update_off_addons: true,
            ..Config::default()
        };
        let json = serde_json::to_string(&on).unwrap();
        assert!(json.contains(r#""update_off_addons":true"#));
        assert!(serde_json::from_str::<Config>(&json).unwrap().update_off_addons);
    }

    #[test]
    fn time_trial_boards_is_on_unless_turned_off() {
        assert!(Config::default().time_trial_boards);
        assert!(serde_json::from_str::<Config>("{}").unwrap().time_trial_boards);
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("time_trial")
        );
        let off = Config {
            time_trial_boards: false,
            ..Config::default()
        };
        let json = serde_json::to_string(&off).unwrap();
        assert!(json.contains(r#""time_trial_boards":false"#));
        assert!(!serde_json::from_str::<Config>(&json).unwrap().time_trial_boards);
    }

    #[test]
    fn time_trial_ingame_is_on_unless_turned_off() {
        assert!(Config::default().time_trial_ingame);
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("time_trial_ingame")
        );
        let off = Config {
            time_trial_ingame: false,
            ..Config::default()
        };
        let json = serde_json::to_string(&off).unwrap();
        assert!(json.contains(r#""time_trial_ingame":false"#));
        assert!(!serde_json::from_str::<Config>(&json).unwrap().time_trial_ingame);
    }

    #[test]
    fn home_video_is_on_unless_turned_off() {
        assert!(Config::default().home_video);
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("home_video")
        );
        let off = Config {
            home_video: false,
            ..Config::default()
        };
        let json = serde_json::to_string(&off).unwrap();
        assert!(json.contains(r#""home_video":false"#));
        assert!(!serde_json::from_str::<Config>(&json).unwrap().home_video);
    }

    #[test]
    fn old_session_logs_are_kept_unless_clearing_is_turned_on() {
        assert!(!Config::default().clear_old_session_logs);
        let old: Config = serde_json::from_str(r#"{ "developer": true }"#).unwrap();
        assert!(!old.clear_old_session_logs);
        assert!(
            !serde_json::to_string(&old)
                .unwrap()
                .contains("clear_old_session_logs")
        );
        let on = Config {
            clear_old_session_logs: true,
            ..Config::default()
        };
        let json = serde_json::to_string(&on).unwrap();
        assert!(json.contains(r#""clear_old_session_logs":true"#), "{json}");
        assert!(
            serde_json::from_str::<Config>(&json)
                .unwrap()
                .clear_old_session_logs
        );
    }

    #[test]
    fn starting_the_store_is_on_unless_turned_off() {
        assert!(Config::default().launch.start_store);
        let old: Config = serde_json::from_str(r#"{ "launch": { "winedebug": "-all" } }"#).unwrap();
        assert!(old.launch.start_store);
        assert!(!serde_json::to_string(&old).unwrap().contains("start_"));
        let off: Config = serde_json::from_str(r#"{ "launch": { "start_steam": false } }"#).unwrap();
        assert!(!off.launch.start_store);
        let json = serde_json::to_string(&off).unwrap();
        assert!(json.contains(r#""start_store":false"#), "{json}");
        assert!(!serde_json::from_str::<Config>(&json).unwrap().launch.start_store);
    }

    #[test]
    fn a_located_folder_becomes_the_active_added_copy() {
        let mut old: Config = serde_json::from_str(r#"{ "game_dir": "/games/F1 25" }"#).unwrap();
        old.migrate_games();
        assert_eq!(old.games.active.as_deref(), Some(Path::new("/games/F1 25")));
        assert_eq!(old.games.added, [PathBuf::from("/games/F1 25")]);
        let json = serde_json::to_string(&old).unwrap();
        assert!(!json.contains("game_dir"), "{json}");
        let mut back: Config = serde_json::from_str(&json).unwrap();
        back.migrate_games();
        assert_eq!(back, old);
        let mut both: Config =
            serde_json::from_str(r#"{ "game_dir": "/b", "games": { "active": "/a", "added": ["/a"] } }"#)
                .unwrap();
        both.migrate_games();
        assert_eq!(both.games.active.as_deref(), Some(Path::new("/a")));
        assert_eq!(both.games.added, [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("games")
        );
    }

    #[test]
    fn config_remembers_vr_per_profile() {
        let mut config = Config::default();
        assert_eq!(config.launch_mode("Default"), LaunchMode::Normal);
        assert!(!serde_json::to_string(&config).unwrap().contains("vr_profiles"));
        config.set_launch_mode("Default", LaunchMode::Vr);
        let back: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.launch_mode("Default"), LaunchMode::Vr);
        assert_eq!(back.launch_mode("Other"), LaunchMode::Normal);
        config.set_launch_mode("Default", LaunchMode::Normal);
        assert!(config.vr_profiles.is_empty());
    }

    #[test]
    fn config_remembers_closed_news() {
        let mut config = Config::default();
        assert!(!serde_json::to_string(&config).unwrap().contains("news_seen"));
        config.news_seen.insert("discord".into());
        let back: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert!(back.news_seen.contains("discord"));
    }

    #[test]
    fn config_remembers_dismissed_cards() {
        let mut config = Config::default();
        assert!(
            !serde_json::to_string(&config)
                .unwrap()
                .contains("dismissed_cards")
        );
        config.dismissed_cards.insert("discord-banner".into());
        let back: Config = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert!(back.dismissed_cards.contains("discord-banner"));
    }

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn vr_adds_steams_vr_argument_once() {
        assert_eq!(game_args(&[], LaunchMode::Normal), Vec::<String>::new());
        assert_eq!(game_args(&[], LaunchMode::Vr), ["-vr"]);
        assert_eq!(game_args(&strings(&["-vr"]), LaunchMode::Vr), ["-vr"]);
        assert_eq!(game_args(&strings(&["-VR"]), LaunchMode::Vr), ["-VR"]);
        assert_eq!(game_args(&strings(&["-vr"]), LaunchMode::Normal), ["-vr"]);
        assert_eq!(
            game_args(&strings(&["-windowed"]), LaunchMode::Vr),
            ["-windowed", "-vr"]
        );
        assert_eq!(LaunchMode::of(&strings(&["-windowed", "-vr"])), LaunchMode::Vr);
        assert_eq!(LaunchMode::of(&strings(&["-vrmode"])), LaunchMode::Normal);
        assert_eq!(LaunchMode::Vr.args(), ["-vr"]);
        assert!(LaunchMode::Normal.args().is_empty());
    }

    #[test]
    fn steam_vr_launch_keeps_its_argument() {
        let vr = parse_steam_command(&strings(&[
            "/steam/ubuntu12_32/reaper",
            "SteamLaunch",
            "AppId=3059520",
            "--",
            "/steam/compatibilitytools.d/Proton/proton",
            "waitforexitandrun",
            "/games/F1 25/EAAntiCheat.GameServiceLauncher.exe",
            "-vr",
        ]));
        assert_eq!(vr.game_args(), ["-vr"]);
        assert_eq!(LaunchMode::of(vr.game_args()), LaunchMode::Vr);
        let direct = parse_steam_command(&strings(&[
            "/proton/proton",
            "waitforexitandrun",
            "/games/F1 25/F1_25.exe",
        ]));
        assert!(direct.game_args().is_empty());
        let windows = parse_steam_command(&strings(&["C:\\F1 25\\F1_25.exe", "-vr"]));
        assert_eq!(windows.game_args(), ["-vr"]);
        assert!(parse_steam_command(&[]).game_args().is_empty());
    }

    #[test]
    fn game_env_drops_build_shell_junk() {
        let parent = [
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("HOME".to_string(), "/home/u".to_string()),
            ("SteamAppId".to_string(), "3059520".to_string()),
            ("NIX_LDFLAGS".to_string(), "-L/nix/store/aaaa ".repeat(200)),
            ("CFLAGS".to_string(), "-O2".to_string()),
            ("buildInputs".to_string(), "x".to_string()),
            ("depsBuildBuild".to_string(), String::new()),
        ];
        let env = game_env(parent, &BTreeMap::new());
        assert!(env.contains_key("PATH") && env.contains_key("SteamAppId"));
        for dropped in ["NIX_LDFLAGS", "CFLAGS", "buildInputs", "depsBuildBuild"] {
            assert!(!env.contains_key(dropped), "{dropped} kept");
        }
    }

    #[test]
    fn game_env_drops_an_inherited_compat_layer() {
        let parent = || {
            [
                ("Path".to_string(), r"C:\WINDOWS\system32".to_string()),
                ("__COMPAT_LAYER".to_string(), "DetectorsAppHealth".to_string()),
                ("__compat_layer".to_string(), "RunAsInvoker".to_string()),
            ]
        };
        let env = game_env(parent(), &BTreeMap::new());
        assert_eq!(env.keys().collect::<Vec<_>>(), ["Path"]);
        let mut overrides = BTreeMap::new();
        overrides.insert(COMPAT_LAYER.to_string(), "HighDpiAware".to_string());
        let env = game_env(parent(), &overrides);
        assert_eq!(env.get(COMPAT_LAYER).map(String::as_str), Some("HighDpiAware"));
        assert_eq!(env.len(), 2);
    }

    #[test]
    fn game_env_fits_the_windows_block() {
        let parent = (0..400).map(|i| (format!("VAR{i:03}"), "x".repeat(200)));
        let mut overrides = BTreeMap::new();
        overrides.insert("WINEDEBUG".to_string(), "+all".to_string());
        let env = game_env(parent, &overrides);
        let size: usize = env.iter().map(|(k, v)| k.len() + v.len() + 2).sum();
        assert!(size <= 30_000, "{size}");
        assert_eq!(env.get("WINEDEBUG").map(String::as_str), Some("+all"));
    }

    #[cfg(unix)]
    #[test]
    fn sdl_comes_from_steam_only_when_the_host_has_none() {
        let root = std::env::temp_dir().join(format!("swofty-sdl-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let steam = root.join("steam");
        let runtime = steam.join("ubuntu12_32/steam-runtime/usr/lib/x86_64-linux-gnu");
        let host = root.join("host");
        let host32 = root.join("host32");
        let links = root.join("data").join(RUNTIME_LIBS_DIR);
        for d in [&runtime, &host, &host32] {
            std::fs::create_dir_all(d).unwrap();
        }
        let elf64 = b"\x7fELF\x02\x01\x01\0".as_slice();
        std::fs::write(runtime.join(SDL2_SONAME), elf64).unwrap();
        std::fs::write(host32.join(SDL2_SONAME), b"\x7fELF\x01\x01\x01\0").unwrap();
        let host32 = host32.to_str().unwrap();
        let links_s = links.display().to_string();

        let path = sdl_library_path_in(None, &[host32], &steam, &links);
        assert_eq!(path.as_deref(), Some(links_s.as_str()));
        assert_eq!(
            std::fs::read_link(links.join(SDL2_SONAME)).unwrap(),
            runtime.join(SDL2_SONAME)
        );
        let path = sdl_library_path_in(Some("/proton/lib"), &[host32], &steam, &links);
        assert_eq!(path, Some(format!("/proton/lib:{links_s}")));

        std::fs::write(host.join(SDL2_SONAME), elf64).unwrap();
        assert_eq!(
            sdl_library_path_in(None, &[host.to_str().unwrap()], &steam, &links),
            None
        );
        assert_eq!(
            sdl_library_path_in(Some(host.to_str().unwrap()), &[], &steam, &links),
            None
        );
        assert_eq!(
            sdl_library_path_in(None, &[], &root.join("no-steam"), &links),
            None
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn sdl_comes_from_the_64_bit_clients_sdl3_when_the_old_runtime_is_gone() {
        let root = std::env::temp_dir().join(format!("swofty-sdl3-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let steam = root.join("steam");
        let links = root.join("data").join(RUNTIME_LIBS_DIR);
        let elf64 = b"\x7fELF\x02\x01\x01\0".as_slice();
        let platform = |version: &str| {
            steam
                .join(STEAM_CLIENT_PLATFORMS)
                .join(format!("steamrt3c_platform_3c.0.{version}"))
                .join(PLATFORM_LIB_DIR)
        };
        for (version, file) in [
            ("20260119.1", "libSDL2-2.0.so.0.3200.10"),
            ("20261103.2", "libSDL2-2.0.so.0.3200.74"),
        ] {
            std::fs::create_dir_all(platform(version)).unwrap();
            std::fs::write(platform(version).join(file), elf64).unwrap();
        }
        std::fs::create_dir_all(
            steam
                .join(STEAM_CLIENT_PLATFORMS)
                .join("steamrt3c_platform_3c.0.20991231.9"),
        )
        .unwrap();

        assert_eq!(sdl_library_path_in(None, &[], &steam, &links), None);

        std::fs::write(steam.join(STEAM_CLIENT_SDL3), elf64).unwrap();
        let path = sdl_library_path_in(Some("/proton/lib"), &[], &steam, &links);
        assert_eq!(path, Some(format!("/proton/lib:{}", links.display())));
        assert_eq!(
            std::fs::read_link(links.join(SDL2_SONAME)).unwrap(),
            platform("20261103.2").join("libSDL2-2.0.so.0.3200.74")
        );
        assert_eq!(
            std::fs::read_link(links.join(SDL3_SONAME)).unwrap(),
            steam.join(STEAM_CLIENT_SDL3)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cpu_lists() {
        assert_eq!(parse_cpu_list("0-2,5"), Some(vec![0, 1, 2, 5]));
        assert_eq!(parse_cpu_list("x"), None);
    }

    #[test]
    fn reads_session_records() {
        let dir = std::env::temp_dir().join(format!("swofty-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("request_id"), "abc\n").unwrap();
        std::fs::write(dir.join("game_start"), "100.5\n").unwrap();
        let live = SessionRecord::read(&dir);
        assert_eq!(live.request_id.as_deref(), Some("abc"));
        assert_eq!((live.started, live.exit_code), (Some(100.5), None));
        assert!(!live.crashed());
        std::fs::write(dir.join("exit_code"), "3\n").unwrap();
        assert!(
            !SessionRecord::read(&dir).crashed(),
            "a code that is not a crash code"
        );
        std::fs::write(dir.join("exit_code"), "-1073741819\n").unwrap();
        assert!(SessionRecord::read(&dir).crashed(), "an access violation");
        std::fs::remove_dir_all(&dir).ok();
    }

    fn seen(log: &str) -> crate::launch::LoaderSeen {
        crate::launch::LoaderSeen::scan(log)
    }

    const ATTACHED: &str = "swofty loader (version.dll proxy) attached, pid 15228\n";
    const UNHANDLED: &str = "[22104] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0xb9ae23 \
                             reached the top-level filter; passing it to the game's filter\n";

    #[test]
    fn a_crash_needs_a_crash_code_or_an_unhandled_exception() {
        let crashed = format!("{ATTACHED}{UNHANDLED}[stub] [offline] game exited with code 4294967295\n");
        let end = SessionEnd::resolve(Some(-1), "", None, Some(&seen(&crashed)));
        assert_eq!(
            (end.end, end.exit_code, end.crash_code),
            (Ending::Crash, Some(-1), Some(0xc000_0005))
        );
        assert!(
            end.reason.contains("access violation, 0xc0000005"),
            "{}",
            end.reason
        );

        let end = SessionEnd::resolve(Some(-1073740791), "", None, None);
        assert_eq!((end.end, end.crash_code), (Ending::Crash, Some(0xc000_0409)));
        assert_eq!(
            end.reason,
            "F1 25 stopped itself after detecting a corrupted stack or an invalid call (0xc0000409)"
        );
        for code in [0xc0e9_0002u32, 0xcfff_ffff, 0xc000_001d, 0x8000_0003] {
            assert_eq!(
                SessionEnd::resolve(Some(code as i32), "", None, None).end,
                Ending::Crash
            );
        }

        let quit_late =
            format!("{ATTACHED}[crash] TerminateProcess(4294967295) on thread 39612, 2916 s after load\n");
        let end = SessionEnd::resolve(Some(-1), "", None, Some(&seen(&quit_late)));
        assert_eq!(
            (end.end, end.exit_code, end.crash_code),
            (Ending::Unknown, Some(-1), None)
        );
        assert!(
            end.reason.contains("saw no unhandled exception"),
            "{}",
            end.reason
        );
        let end = SessionEnd::resolve(Some(-1), "", None, None);
        assert!(end.reason.contains("not attached"), "{}", end.reason);
        let full = format!(
            "{ATTACHED}[3820] dialog shown: Insufficient Storage: 652/2048 MiB available on 'C:\\', please free \
             some space.\n[stub] [offline] game exited with code 4294967295\n"
        );
        let end = SessionEnd::resolve(Some(-1), "", None, Some(&seen(&full)));
        assert_eq!((end.end, end.crash_code), (Ending::Crash, None));
        assert!(end.reason.contains("Insufficient Storage"), "{}", end.reason);
        let end = SessionEnd::resolve(None, "no code", None, Some(&seen(&full)));
        assert_eq!(end.end, Ending::Crash);
        let stopped = SessionEnd::resolve(
            Some(-1),
            "",
            Some("Stop in the launcher closed F1 25"),
            Some(&seen(&full)),
        );
        assert_eq!(stopped.end, Ending::Stopped);
        for code in [1, 0x4001_0004, 255] {
            assert_eq!(
                SessionEnd::resolve(Some(code), "", None, None).end,
                Ending::Unknown
            );
        }
        assert_eq!(SessionEnd::resolve(Some(0), "", None, None).end, Ending::Quit);
    }

    #[test]
    fn a_load_the_core_stopped_is_named() {
        let end = SessionEnd::resolve(Some(CORE_STOPPED_LOAD), "", None, Some(&seen(ATTACHED)));
        assert_eq!(end.end, Ending::Stopped);
        assert!(
            end.reason.contains("the core closed F1 25 at the session load"),
            "{}",
            end.reason
        );
        let plain = SessionEnd::resolve(Some(CORE_STOPPED_LOAD), "", None, None);
        assert_eq!(plain.end, Ending::Unknown);
    }

    #[test]
    fn the_session_launchers_line_gives_the_code_the_session_could_not_read() {
        let ea = "[stub] [offline] game exited with code 1\n";
        let end = SessionEnd::resolve(None, "the EA app started F1 25", None, Some(&seen(ea)));
        assert_eq!((end.end, end.exit_code), (Ending::Unknown, Some(1)));
        let proton = "[stub] [offline] game exited with code 3221225477\n";
        let end = SessionEnd::resolve(Some(5), "", None, Some(&seen(proton)));
        assert_eq!((end.end, end.exit_code), (Ending::Crash, Some(-1073741819)));
        let end = SessionEnd::resolve(None, "the EA app started F1 25", None, Some(&seen(ATTACHED)));
        assert_eq!(
            end,
            SessionEnd {
                end: Ending::Unknown,
                exit_code: None,
                crash_code: None,
                reason: "the EA app started F1 25".into(),
            }
        );
    }

    #[test]
    fn an_unhandled_exception_is_a_crash_when_the_ea_app_started_the_game() {
        let log = format!("{ATTACHED}{UNHANDLED}");
        let end = SessionEnd::resolve(None, "the EA app started F1 25", None, Some(&seen(&log)));
        assert_eq!(
            (end.end, end.exit_code, end.crash_code),
            (Ending::Crash, None, Some(0xc000_0005))
        );
        assert!(end.reason.contains("access violation"), "{}", end.reason);
        let stopped = SessionEnd::resolve(
            None,
            "",
            Some("Stop in the launcher closed F1 25"),
            Some(&seen(&log)),
        );
        assert_eq!(stopped.end, Ending::Stopped);
    }

    #[test]
    fn a_stop_is_not_a_crash() {
        let crashed = format!("{ATTACHED}{UNHANDLED}");
        let end = SessionEnd::resolve(
            Some(-1),
            "",
            Some("Stop in the launcher closed F1 25"),
            Some(&seen(&crashed)),
        );
        assert_eq!(
            (end.end, end.exit_code, end.crash_code),
            (Ending::Stopped, Some(-1), None)
        );
        assert_eq!(
            end.reason,
            "Stop in the launcher closed F1 25; F1 25 exited with code -1"
        );
        let end = SessionEnd::resolve(None, "signal 9", Some("asked to stop"), None);
        assert_eq!((end.end, end.exit_code), (Ending::Stopped, None));
        assert_eq!(end.reason, "asked to stop; signal 9");
    }

    #[test]
    fn the_end_file_decides_what_shows_as_a_crash() {
        let root = std::env::temp_dir().join(format!("swofty-session-end-{}", std::process::id()));
        let dir = root.join("sessions/20261023-221530");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(root.join("sessions/latest.txt"), format!("{}\n", dir.display())).unwrap();
        std::fs::write(dir.join("game_start"), "100\n").unwrap();
        request_stop(&root);
        assert!(dir.join(STOP_FILE).is_file());
        std::fs::write(dir.join("exit_code"), "-1\n").unwrap();
        std::fs::write(dir.join("game_end"), "200\n").unwrap();
        assert!(
            !SessionRecord::read(&dir).crashed(),
            "no end file: -1 is not a crash code"
        );
        let mut end = SessionEnd::resolve(Some(-1), "", Some("Stop in the launcher closed F1 25"), None);
        end.write(&dir).unwrap();
        let record = SessionRecord::read(&dir);
        assert_eq!(record.end.as_ref(), Some(&end));
        assert!(!record.crashed());
        for (ending, code, shown) in [
            (Ending::Crash, Some(-1), true),
            (Ending::Unknown, Some(-1), false),
            (Ending::Unknown, None, false),
            (Ending::Quit, Some(0), false),
        ] {
            end.end = ending;
            end.exit_code = code;
            end.write(&dir).unwrap();
            assert_eq!(SessionRecord::read(&dir).crashed(), shown, "{ending:?} {code:?}");
        }
        std::fs::remove_file(dir.join(STOP_FILE)).unwrap();
        request_stop(&root);
        assert!(!dir.join(STOP_FILE).exists(), "the session has ended");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn tells_a_rotated_loader_log_from_a_stale_one() {
        let dir = std::env::temp_dir().join(format!("swofty-loader-mark-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("loader.log");
        assert!(!LogMark::replaced(&log, None));
        std::fs::write(
            &log,
            "2026-10-17 21:44:08.228 [  416] swofty loader (version.dll proxy) attached, pid 412\nold\n",
        )
        .unwrap();
        assert!(LogMark::replaced(&log, None));
        let before = LogMark::read(&log);
        let session = SessionRecord::read(&dir);
        assert!(!session.owns_loader_log(&log));
        std::fs::write(dir.join(LOADER_MARK), serde_json::to_vec(&before).unwrap()).unwrap();
        assert!(!LogMark::replaced(&log, before.as_ref()));
        assert!(!session.owns_loader_log(&log));
        std::fs::rename(&log, dir.join("loader.prev.log")).unwrap();
        std::fs::write(&log, "").unwrap();
        assert!(session.owns_loader_log(&log));
        std::fs::write(
            &log,
            "2026-10-17 22:12:08.004 [  420] swofty loader (version.dll proxy) attached, pid 416\n",
        )
        .unwrap();
        assert!(session.owns_loader_log(&log));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn steam_command_shape() {
        let argv: Vec<String> = [
            "/steam/ubuntu12_32/reaper",
            "SteamLaunch",
            "AppId=3059520",
            "--",
            "/steam/SteamLinuxRuntime_sniper/_v2-entry-point",
            "--verb=waitforexitandrun",
            "--",
            "/steam/compatibilitytools.d/Proton/proton",
            "waitforexitandrun",
            "/games/F1 25/EAAntiCheat.GameServiceLauncher.exe",
        ]
        .map(String::from)
        .to_vec();
        let c = parse_steam_command(&argv);
        assert_eq!(c.verb.as_deref(), Some("waitforexitandrun"));
        assert_eq!(c.target, Some(9));
        assert!(c.proton.unwrap().ends_with("/proton"));
    }
}
