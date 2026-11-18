//! Crash causes the Send Logs triage keeps finding, recognised from a crashed
//! session's own files: the `[crash]`, `[overlay]` and `[offline] module:`
//! lines of its loader.log, the crash report the core wrote, and the files in
//! the game folder. The launcher shows the cause and its fix when one of the
//! rules here matches, and Send Logs and the support Discord when none does.
//! An addon whose changes don't apply to an F1 25 older than the newest
//! update is recognised from the build's error, for the launch's failure.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use crate::conflicts::Pair;
use crate::install::LOADER_DIR;
use crate::launch::LOW_DISK;
use crate::manifest::{AddonManifest, Category};
use crate::overlay_guard::{Removal, SessionOverlay};
use crate::session::SessionRecord;
use crate::store::Store;

/// The most of a loader.log or crash report that is read.
const READ_CAP: u64 = 32 << 20;
/// The stack frames, nearest first, that count as the crash's own.
const NEAR_FRAMES: usize = 8;
/// The files a step names before it says how many more there are.
const NAMED_FILES: usize = 6;

/// The newest F1 25 update the launcher knows, as `F1_25.exe`'s file
/// version: Steam build 26566308, and the EA app's copy of the same update.
pub const LATEST_GAME_VERSION: &str = "1.0.145.2472";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    /// A stable name for the rule that matched, for logs and Send Logs.
    pub id: &'static str,
    pub title: String,
    pub what: String,
    pub steps: Vec<String>,
}

impl Diagnosis {
    /// What happened, then its steps, as one paragraph.
    pub fn text(&self) -> String {
        std::iter::once(&self.what)
            .chain(&self.steps)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A game version's four numbers, as in `1.0.141.7865`.
fn version_numbers(version: &str) -> Option<[u32; 4]> {
    let mut parts = version.trim().split('.');
    let mut numbers = [0; 4];
    for n in &mut numbers {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *n = part.parse().ok()?;
    }
    parts.next().is_none().then_some(numbers)
}

/// Whether game version `version` is older than `than`, number by number.
/// None when either isn't a version.
pub fn older_game(version: &str, than: &str) -> Option<bool> {
    Some(version_numbers(version)? < version_numbers(than)?)
}

/// The addon whose changes failed to apply, from a build error such as
/// `applying swofty.classic-seasons: eaid://…: no entry [Name=…]`, on its
/// own or after what was being done when it failed.
fn failed_addon(error: &str) -> Option<&str> {
    let rest = error
        .strip_prefix("applying ")
        .or_else(|| error.split_once(": applying ").map(|(_, rest)| rest))?;
    let id = rest.split([':', ' ', '\n']).next()?;
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    valid.then_some(id)
}

/// An addon whose changes failed to apply to an F1 25 older than
/// [`LATEST_GAME_VERSION`]: it needs that update. None when `error` isn't an
/// addon's changes failing, or when `version`, the game's own, is unknown or
/// not older.
pub fn outdated_game(
    error: &str,
    version: Option<&str>,
    names: &BTreeMap<String, String>,
) -> Option<Diagnosis> {
    let version = version?.trim();
    if older_game(version, LATEST_GAME_VERSION) != Some(true) {
        return None;
    }
    let id = failed_addon(error)?;
    let name = names.get(id).map_or(id, String::as_str);
    Some(Diagnosis {
        id: "outdated_game",
        title: format!("{name} needs the latest F1 25"),
        what: format!(
            "{name} needs F1 25 {LATEST_GAME_VERSION}, the latest update. Your F1 25 is {version}."
        ),
        steps: vec!["Update F1 25 in Steam or the EA app, then press Play again.".to_owned()],
    })
}

/// An F1 25 older than [`LATEST_GAME_VERSION`], before a modded launch: the
/// mod loader can't hook into it, so addons may not load. None when the
/// version is unknown or not older.
pub fn outdated_exe(version: Option<&str>) -> Option<Diagnosis> {
    let version = version?.trim();
    if older_game(version, LATEST_GAME_VERSION) != Some(true) {
        return None;
    }
    Some(Diagnosis {
        id: "outdated_exe",
        title: "F1 25 needs the latest update".to_owned(),
        what: format!(
            "Your F1 25 is {version}. Mods are made for {LATEST_GAME_VERSION}, the latest update, and may not \
             load on an older one."
        ),
        steps: vec![format!(
            "Update F1 25 in Steam or the EA app. If Steam says it is up to date, verify the game files \
             ({}), then press Play again.",
            crate::stock::STEAM_VERIFY_URL
        )],
    })
}

/// What a crashed session left behind, as the rules read it.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    /// The session's loader.log.
    pub loader: String,
    /// The crash report the core wrote for the session, when it can be found.
    pub report: Option<String>,
    /// The session's perf.log.
    pub perf: Option<String>,
    /// The names of the files and folders in the game folder.
    pub game_files: Vec<String>,
    /// Addon names by id, for addons the rules name.
    pub names: BTreeMap<String, String>,
    /// The installed version of each addon, by id.
    pub versions: BTreeMap<String, String>,
    /// The profile's enabled addons in load order, from its last build.
    pub enabled: Vec<String>,
    /// The enabled addons that are tracks.
    pub tracks: Vec<String>,
    /// The installed track and season addons the profile has off.
    pub off: Vec<String>,
    /// Which addon overrode which in the profile's last build.
    pub conflicts: Vec<Pair>,
    /// Per addon id, the game's entrant lists it renamed or removed that the last build kept.
    pub kept_lists: BTreeMap<String, Vec<String>>,
    /// The installed addons that are built in or managed by the launcher, which a profile can't turn off.
    pub builtin: BTreeSet<String>,
    /// The game's exit code, or the crash code the session recorded.
    pub code: Option<u32>,
    /// The game exe's version, when it can be read.
    pub game_version: Option<String>,
    /// The game exe's size and PE link timestamp, the pair that identifies a build.
    pub game_build: Option<(u64, u32)>,
}

impl Evidence {
    pub fn read(
        session: &SessionRecord,
        game: Option<&Path>,
        names: BTreeMap<String, String>,
        data: Option<&Path>,
    ) -> Self {
        let loader = ["loader.log", "game-logs/loader.log"]
            .iter()
            .map(|rel| session.dir.join(rel))
            .find(|p| p.is_file())
            .and_then(|p| read_capped(&p))
            .unwrap_or_default();
        let perf = read_perf(&session.dir);
        let report = report_name(&loader).and_then(|name| {
            let mut dirs = vec![session.dir.clone(), session.dir.join("game-logs")];
            if let Some(game) = game {
                dirs.insert(0, game.join(LOADER_DIR));
            }
            if let Some(data) = data {
                dirs.push(crate::crash_report::fallback_dir(data));
            }
            dirs.iter()
                .map(|d| d.join(&name))
                .find(|p| p.is_file())
                .and_then(|p| read_capped(&p))
        });
        let game_files = game
            .and_then(|g| std::fs::read_dir(g).ok())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let code = session
            .end
            .as_ref()
            .and_then(|e| e.crash_code.or(e.exit_code.map(|c| c as u32)))
            .or(session.exit_code.map(|c| c as u32));
        let game_version = game.and_then(|g| crate::exe_version::read(&g.join(crate::GAME_EXE)));
        let game_build = game.and_then(|g| crate::build_support::identity(&g.join(crate::GAME_EXE)));
        Self {
            loader,
            report,
            perf,
            game_files,
            names,
            code,
            game_version,
            game_build,
            ..Default::default()
        }
    }

    /// The profile's addons from its last build in `store`.
    pub fn with_profile(mut self, store: &Store, addons: &BTreeMap<String, AddonManifest>) -> Self {
        self.builtin = addons
            .keys()
            .filter(|id| store.is_builtin(id) || crate::order::is_pinned(id))
            .cloned()
            .collect();
        self.versions = addons
            .iter()
            .map(|(id, m)| (id.clone(), m.version.to_string()))
            .collect();
        let profile = store.active_profile();
        if let Ok(p) = store.profile(&profile) {
            self.off = p
                .addons
                .into_iter()
                .filter(|a| !a.enabled)
                .filter(|a| {
                    addons
                        .get(&a.id)
                        .is_some_and(|m| matches!(m.category, Some(Category::Tracks | Category::Seasons)))
                })
                .map(|a| a.id)
                .collect();
        }
        if let Some(report) = crate::conflicts::last(store, &profile) {
            self.tracks = report
                .order
                .iter()
                .filter(|id| {
                    addons
                        .get(*id)
                        .is_some_and(|m| m.category == Some(Category::Tracks))
                })
                .cloned()
                .collect();
            self.enabled = report.order;
            self.conflicts = report.pairs;
            self.kept_lists = report.kept_lists;
        }
        self
    }
}

/// The cause of the crash `session` ended with, when a rule recognises it.
pub fn crash(session: &SessionRecord, game: Option<&Path>, store: &Store) -> Option<Diagnosis> {
    diagnose(&evidence(session, game, store))
}

/// Why `session`, which ended without a crash, never got going, when a rule
/// recognises it.
pub fn stall(session: &SessionRecord, game: Option<&Path>, store: &Store) -> Option<Diagnosis> {
    perf_start_screen(&read_perf(&session.dir)?).filter(|s| *s >= BOOT_STALL_SECS)?;
    stalled(&evidence(session, game, store))
}

/// Why `session`, which ended without a crash, ran with cars that had no
/// tyres, when its log names them.
pub fn tyres(session: &SessionRecord, game: Option<&Path>, store: &Store) -> Option<Diagnosis> {
    unplaced_tyres(&evidence(session, game, store))
}

fn evidence(session: &SessionRecord, game: Option<&Path>, store: &Store) -> Evidence {
    let addons = store.addons().unwrap_or_default();
    let names = addons
        .iter()
        .map(|(id, m)| (id.clone(), m.name.clone()))
        .collect();
    Evidence::read(session, game, names, Some(&store.root)).with_profile(store, &addons)
}

pub(crate) fn read_perf(dir: &Path) -> Option<String> {
    ["perf.log", "game-logs/perf.log"]
        .iter()
        .find_map(|rel| read_capped(&dir.join(rel)))
}

fn read_capped(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(READ_CAP)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn report_name(loader: &str) -> Option<String> {
    crate::crash_report::logged_name(loader)
}

/// The fatal exception as the loader log and the crash report describe it.
#[derive(Debug, Clone, Default, PartialEq)]
struct Fatal {
    /// The module the exception happened in, lower case.
    module: Option<String>,
    /// The modules on the crashing thread's stack, nearest first, lower case.
    stack: Vec<String>,
    /// The loader's record of the exception: registers, code and stack.
    record: Vec<String>,
    /// The crashing thread's stack frames as written, nearest first.
    frames: Vec<String>,
    /// Where the record starts among the loader's crash lines.
    at: Option<usize>,
}

impl Fatal {
    fn near(&self) -> impl Iterator<Item = &str> {
        self.module
            .iter()
            .chain(self.stack.iter().take(NEAR_FRAMES))
            .map(String::as_str)
    }
}

/// The module in `text` before its first `+0x`, as in `F1_25.exe+0x39182a4`.
fn module_at(text: &str) -> Option<String> {
    let at = text.find("+0x")?;
    let name = text[..at].rsplit([' ', '\t']).next()?;
    (!name.is_empty() && !name.starts_with("0x")).then(|| name.to_ascii_lowercase())
}

/// The loader's crash lines, without their time and thread.
fn crash_lines(loader: &str) -> Vec<&str> {
    loader
        .lines()
        .filter_map(|l| Some(&l[l.find("[crash] ")? + "[crash] ".len()..]))
        .collect()
}

/// The exception code in a first-chance line, as in `ACCESS_VIOLATION (0xc0000005)`.
fn exception_code(line: &str) -> Option<u32> {
    line.match_indices("(0x").find_map(|(i, _)| {
        let hex = &line[i + 3..];
        u32::from_str_radix(&hex[..hex.find(')')?], 16).ok()
    })
}

fn record_at<'a>(crash: &[&'a str], i: usize) -> impl Iterator<Item = &'a str> {
    crash[i + 1..].iter().copied().take_while(|l| l.starts_with("  "))
}

fn fatal(e: &Evidence) -> Fatal {
    let mut f = Fatal::default();
    let crash = crash_lines(&e.loader);
    let unhandled = crash
        .iter()
        .find(|l| l.starts_with("unhandled ") && l.contains("reached the top-level filter"));
    f.module = unhandled.and_then(|l| module_at(&l[l.find(" at ")? + 4..]));
    let address = unhandled.and_then(|l| {
        let at = &l[l.find(" at ")? + 4..];
        Some(at.split_whitespace().next()?.to_owned())
    });
    f.at = match &address {
        Some(address) => crash
            .iter()
            .position(|l| l.contains("first-chance") && l.contains(address.as_str())),
        None if e.report.is_none() => crash
            .iter()
            .rposition(|l| l.contains("first-chance"))
            .filter(|&i| e.code.is_some() && exception_code(crash[i]) == e.code),
        None => None,
    };
    if let Some(i) = f.at {
        if f.module.is_none() {
            f.module = crash[i]
                .find(" at ")
                .and_then(|at| module_at(&crash[i][at + 4..]));
        }
        f.record = record_at(&crash, i).map(str::to_owned).collect();
        f.frames = f
            .record
            .iter()
            .filter(|l| l.trim_start().starts_with('#'))
            .cloned()
            .collect();
        f.stack = f.frames.iter().filter_map(|l| module_at(l)).collect();
    }
    if let Some(report) = &e.report {
        let mut section = "";
        let mut stack = Vec::new();
        let mut frames = Vec::new();
        for line in report.lines() {
            if let Some(name) = line.strip_prefix("== ") {
                section = name;
                continue;
            }
            if section == "Exception" && f.module.is_none() {
                if let Some(at) = line.find(" at ") {
                    f.module = module_at(&line[at + 4..]);
                }
            } else if section.starts_with("Stack (") {
                stack.extend(line.split_whitespace().nth(3).and_then(module_at));
                if line.trim_start().starts_with('#') {
                    frames.push(line.to_owned());
                }
            }
        }
        if f.stack.is_empty() {
            f.stack = stack;
        }
        if f.frames.is_empty() {
            f.frames = frames;
        }
    }
    f
}

/// The modules the loader and the offline play hook saw in the game process.
fn loaded_modules(loader: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in loader.lines() {
        let Some(rest) = ["[offline] module: ", "[loader] module: "]
            .iter()
            .find_map(|tag| Some(&line[line.find(tag)? + tag.len()..]))
        else {
            continue;
        };
        let name = rest.split(" @").next().unwrap_or(rest).trim();
        if !name.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(name)) {
            out.push(name.to_owned());
        }
    }
    out
}

/// The loaded modules with the address they are loaded at, as in
/// `[offline] module: RTSSHooks64.dll @ 0x180000000`.
fn module_bases(loader: &str) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = Vec::new();
    for line in loader.lines() {
        let Some(rest) = ["[offline] module: ", "[loader] module: "]
            .iter()
            .find_map(|tag| Some(&line[line.find(tag)? + tag.len()..]))
        else {
            continue;
        };
        let Some((name, base)) = rest.split_once(" @ 0x") else {
            continue;
        };
        let hex = base
            .split(|c: char| !c.is_ascii_hexdigit())
            .next()
            .unwrap_or_default();
        if let Ok(base) = u64::from_str_radix(hex, 16) {
            out.push((name.trim().to_ascii_lowercase(), base));
        }
    }
    out
}

/// The register values in the loader's record of the exception, as in
/// `rsi=000000018024056d rdi=00007ffd675e7776` and `r8 =7ffffffffffffffc`.
fn record_registers(record: &[String]) -> Vec<u64> {
    record
        .iter()
        .filter(|l| !l.trim_start().starts_with('#'))
        .flat_map(|l| l.split_whitespace())
        .filter_map(|t| {
            let (_, hex) = t.split_once('=')?;
            (hex.len() == 16).then(|| u64::from_str_radix(hex, 16).ok())?
        })
        .collect()
}

/// How far past a module's load address a register still counts as pointing into it.
const MODULE_REACH: u64 = 64 << 20;

/// The hook a jump to an address outside every module came from: the game ran
/// an address that is not code, and a register points into the hook's module.
/// A hook that patches the game's graphics calls leaves the game on a dead
/// trampoline this way, so no stack frame names it.
fn stray_jump_hook(f: &Fatal, loader: &str) -> Option<(&'static Hook, String)> {
    if f.module.is_some() || !f.record.iter().any(|l| l.contains("executing (DEP)")) {
        return None;
    }
    let bases = module_bases(loader);
    record_registers(&f.record).into_iter().find_map(|value| {
        let (name, base) = bases
            .iter()
            .filter(|(_, base)| *base <= value)
            .max_by_key(|(_, base)| *base)?;
        if value - base >= MODULE_REACH {
            return None;
        }
        let hook = HOOKS.iter().find(|h| !h.own_only && name.starts_with(h.prefix))?;
        Some((hook, name.clone()))
    })
}

/// Whether the crash's lines (and the crash report) hold any of `codes`, the
/// HRESULTs the game hands its fatal abort, as in `rdi=00000000887a0005`.
fn crash_holds(e: &Evidence, codes: &[&str]) -> bool {
    let hit = |l: &str| {
        let l = l.to_ascii_lowercase();
        codes.iter().any(|c| l.contains(c))
    };
    e.loader
        .lines()
        .filter(|l| l.contains("[crash]") || l.contains("[overlay] the GPU was removed"))
        .any(hit)
        || e.report.as_deref().is_some_and(|r| r.lines().any(hit))
}

fn gpu_removed(e: &Evidence) -> bool {
    crash_holds(
        e,
        &[
            "887a0005",
            "887a0006",
            "887a0007",
            "dxgi_error_device_removed",
            "dxgi_error_device_hung",
            "dxgi_error_device_reset",
        ],
    )
}

/// Whether the game ran under Wine or Proton, where vkd3d reports its own
/// failures (a pipeline it can't build) with the same codes as a Windows
/// driver out of memory or lost.
fn wine(e: &Evidence, loaded: &[String]) -> bool {
    e.report
        .as_deref()
        .is_some_and(|r| r.lines().any(|l| l.starts_with("runtime") && l.contains("Wine")))
        || loaded.iter().any(|m| {
            let m = m.to_ascii_lowercase();
            m.starts_with("winex11") || m.starts_with("winewayland") || m.starts_with("winevulkan")
        })
}

fn out_of_memory(e: &Evidence) -> bool {
    crash_holds(e, &["8007000e", "e_outofmemory"])
}

/// The names of the addons that drew with Swofty's overlay in the session.
fn painters(e: &Evidence, overlay: &SessionOverlay) -> Vec<String> {
    overlay
        .painters
        .iter()
        .filter(|id| !e.builtin.contains(*id))
        .map(|id| e.names.get(id).cloned().unwrap_or_else(|| id.clone()))
        .collect()
}

/// Programs that hook into games as they run, by the start of their DLL's name.
struct Hook {
    prefix: &'static str,
    product: &'static str,
    step: &'static str,
    /// Only when the exception happened in it: the DLL also sits harmlessly
    /// on stacks of crashes it had no part in.
    own_only: bool,
}

const HOOKS: &[Hook] = &[
    Hook {
        prefix: "rtsshooks",
        product: "RivaTuner Statistics Server (RTSS), which comes with MSI Afterburner",
        step: "Close RivaTuner Statistics Server and MSI Afterburner, including from the system tray, before you \
               press Play. Or, in RTSS, add F1_25.exe and set its Application detection level to None.",
        own_only: false,
    },
    Hook {
        prefix: "windhawk",
        product: "Windhawk",
        step: "Quit Windhawk from its tray icon before you press Play, or add F1_25.exe to the processes \
               Windhawk doesn't inject into in its settings.",
        own_only: false,
    },
    Hook {
        prefix: "medal-hook",
        product: "Medal",
        step: "Close Medal, including from the system tray, before you press Play.",
        own_only: false,
    },
    Hook {
        prefix: "discordhook",
        product: "Discord's in-game overlay",
        step: "Turn off Discord's in-game overlay in Discord's settings (Game Overlay).",
        own_only: false,
    },
    Hook {
        prefix: "graphics-hook",
        product: "OBS Studio's game capture",
        step: "In OBS Studio, capture F1 25 with Window Capture or Display Capture instead of Game Capture, or \
               close OBS while you play.",
        own_only: false,
    },
    Hook {
        prefix: "nvpresent",
        product: "the NVIDIA app's overlay and frame tools",
        step: "Turn off the in-game overlay and Smooth Motion for F1 25 in the NVIDIA app.",
        own_only: true,
    },
    Hook {
        prefix: "nvspcap",
        product: "the NVIDIA app's overlay",
        step: "Turn off the in-game overlay in the NVIDIA app's settings.",
        own_only: true,
    },
    Hook {
        prefix: "gameoverlayrenderer",
        product: "the Steam overlay",
        step: "Turn off the Steam overlay for F1 25: right-click F1 25 in Steam, Properties, General, and untick \
               Enable the Steam Overlay while in-game.",
        own_only: true,
    },
    Hook {
        prefix: "specialk",
        product: "Special K",
        step: "Turn Special K off for F1 25, or remove it from the game folder.",
        own_only: false,
    },
    Hook {
        prefix: "pid.dll",
        product: "Windows' force feedback driver for your wheel",
        step: "Update your wheel's driver and software (for example Logitech G HUB, Fanatec, Thrustmaster or \
               Moza), plug the wheel straight into the PC, and make sure only one wheel program is running.",
        own_only: true,
    },
];

/// Graphics drivers' user-mode DLLs, by the start of their name.
const DRIVERS: &[(&str, &str)] = &[
    ("nvwgf2um", "NVIDIA"),
    ("nvd3dum", "NVIDIA"),
    ("nvoglv", "NVIDIA"),
    ("nvlddmkm", "NVIDIA"),
    ("amdxc64", "AMD"),
    ("amdxx64", "AMD"),
    ("atidxx64", "AMD"),
    ("igd12umd", "Intel"),
    ("igd10iumd", "Intel"),
    ("igdumdim", "Intel"),
    ("igc64", "Intel"),
];

/// The game's frame generation DLLs.
const FRAME_GEN: &[(&str, &str)] = &[
    ("nvngx_dlssg", "DLSS Frame Generation"),
    ("sl.dlss_g", "DLSS Frame Generation"),
    ("libxess_fg", "XeSS frame generation"),
];

/// Modules that mean the crash happened in graphics code.
fn graphics(module: &str) -> bool {
    module.starts_with("sl.")
        || module.starts_with("nvngx")
        || module.starts_with("dxgi")
        || module.starts_with("d3d1")
        || module.starts_with("dxcore")
        || module.starts_with("libxess")
        || module.starts_with("amd_fidelityfx")
        || DRIVERS.iter().any(|(p, _)| module.starts_with(p))
}

/// Names in `loaded` that only graphics mods use; Windows and the drivers
/// ship DLLs named like the others (dxgi.dll, nvapi64.dll).
fn distinctive(name: &str) -> bool {
    let f = name.to_ascii_lowercase();
    f.starts_with("reshade")
        || f.starts_with("optiscaler")
        || f.starts_with("specialk")
        || f.starts_with("dlssg_to_fsr3")
        || f.ends_with(".asi")
        || f.ends_with(".addon64")
        || f.ends_with(".addon")
}

/// Files in the game folder that F1 25 doesn't ship and that hook its
/// graphics, plus the distinctive ones among the game's loaded modules,
/// grouped by the tool they come from.
fn graphics_mods(game_files: &[String], loaded: &[String]) -> Vec<(&'static str, Vec<String>)> {
    let mut files = game_files.to_vec();
    for m in loaded.iter().filter(|m| distinctive(m)) {
        if !files.iter().any(|f| f.eq_ignore_ascii_case(m)) {
            files.push(m.clone());
        }
    }
    let files = &files;
    let mut groups: Vec<(&'static str, Vec<String>)> = Vec::new();
    let mut add = |tool: &'static str, name: &str| match groups.iter_mut().find(|(t, _)| *t == tool) {
        Some((_, names)) => names.push(name.to_owned()),
        None => groups.push((tool, vec![name.to_owned()])),
    };
    let reshade = files.iter().any(|f| {
        let f = f.to_ascii_lowercase();
        f == "reshade.ini" || f.starts_with("reshade64") || f.ends_with(".addon64") || f.ends_with(".addon")
    });
    let optiscaler = files
        .iter()
        .any(|f| f.to_ascii_lowercase().starts_with("optiscaler"));
    for name in files {
        let f = name.to_ascii_lowercase();
        let tool = if f.starts_with("optiscaler")
            || f == "nvngx.dll"
            || f == "fakenvapi.ini"
            || f == "nvapi64.dll"
            || f.starts_with("dlssg_to_fsr3")
            || f == "amdxc64.dll"
            || f == "libxess_dx11.dll"
        {
            "OptiScaler"
        } else if f == "reshade.ini"
            || f.starts_with("reshade64")
            || f.ends_with(".addon64")
            || f.ends_with(".addon")
            || f == "reshade-shaders"
        {
            "ReShade"
        } else if f == "dxgi.dll" || f == "d3d12.dll" || f == "d3d11.dll" {
            if optiscaler {
                "OptiScaler"
            } else if reshade {
                "ReShade"
            } else {
                "a graphics mod"
            }
        } else if f.starts_with("specialk") {
            "Special K"
        } else if f.ends_with(".asi") || f == "winmm.dll" || f == "dinput8.dll" {
            "an ASI loader"
        } else {
            continue;
        };
        add(tool, name);
    }
    groups
}

fn list(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(NAMED_FILES).map(String::as_str).collect();
    let more = names.len().saturating_sub(NAMED_FILES);
    match (shown.as_slice(), more) {
        ([one], 0) => (*one).to_owned(),
        (_, 0) => {
            let (last, rest) = shown.split_last().expect("at least two names");
            format!("{} and {last}", rest.join(", "))
        }
        _ => format!("{} and {more} more", shown.join(", ")),
    }
}

fn tools(groups: &[(&'static str, Vec<String>)]) -> String {
    let names: Vec<String> = groups.iter().map(|(t, _)| (*t).to_owned()).collect();
    list(&names)
}

fn move_out(groups: &[(&'static str, Vec<String>)]) -> String {
    let files: Vec<String> = groups.iter().flat_map(|(_, f)| f.iter().cloned()).collect();
    format!(
        "Close Swofty Launcher, then move {} out of the F1 25 folder into a folder of their own, so you can put \
         them back later.",
        list(&files)
    )
}

const VERIFY: &str = "Verify F1 25's files: in Steam, right-click F1 25, Properties, Installed Files, Verify integrity \
                      of game files. For the EA app copy, use Repair in the EA app.";

fn driver_steps() -> Vec<String> {
    vec![
        "Update your graphics driver, or reinstall it cleanly. If you installed a new driver in the last few days, \
         try the one before it."
            .to_owned(),
        "Set any graphics card overclock or undervolt (MSI Afterburner, the NVIDIA app's tuning, AMD Adrenalin) \
         back to stock."
            .to_owned(),
        "Turn off frame generation (DLSS, FSR or XeSS frame generation) in F1 25's graphics settings.".to_owned(),
        "On a graphics card with 8 GB of memory or less, lower Texture Quality, or play with fewer car, livery \
         and track addons."
            .to_owned(),
    ]
}

/// The exit code of a game Windows stopped from loading a file it doesn't
/// trust (STATUS_SYSTEM_INTEGRITY_POLICY_VIOLATION).
const APP_CONTROL: u32 = 0xc0e9_0002;
/// F1 25's hang watchdog limit in milliseconds, which it holds in r15 when it
/// closes the game.
const WATCHDOG_MS: &str = "r15=0000000000007530";
/// How long before the watchdog fired the game had surely stopped moving on.
const STALL_SECS: f64 = 28.0;
/// The earliest before the watchdog fired that the freeze can have started.
const FREEZE_SECS: f64 = 36.0;
/// The least time between two log lines of one addon on one thread, across
/// the freeze, that counts as the addon keeping that thread busy.
const BUSY_GAP_SECS: f64 = 20.0;
/// The lines an addon logs in the stalled time to count as the one busy.
const BUSY_LINES: usize = 5;
/// The archive resources two car packs must both replace to count as full ones.
const CAR_PACK_RESOURCES: usize = 1000;
/// Imported Sepang, the heaviest imported track: the track code of its package.
const SEPANG: &str = "bahrain24";
/// Manual Pit Lane, and where F1 25 crashes when it takes the car as the
/// game returns it to the garage.
const PITLANE: &str = "kizi.pitlane";
const PITLANE_CRASH: &str = "F1_25.exe+0x389a1d7 ";
/// Aggressive AI (Alternate), and the version of it whose track files crash
/// F1 25 as it loads a session.
const AGGRESSIVE_AI: &str = "kizi.f1-25-more-aggresive-ai-2";
const AGGRESSIVE_AI_BROKEN: &str = "3.0.0";
/// The three game functions that load those track files, as a stack frame
/// writes them: on the Steam build, then on the EA app's. The crash is a bad
/// free in the game's `RACE` heap or an access violation, in or called from
/// one of them.
const AGGRESSIVE_AI_CRASH: [&str; 6] = [
    " fn base+0x67b3f0+",
    " fn base+0x67bf30+",
    " fn base+0x67c680+",
    " fn base+0x67be00+",
    " fn base+0x67c940+",
    " fn base+0x67d090+",
];
/// Skip Cutscenes, whose versions up to [`SKIP_CUTSCENES_LAST_BROKEN`] fail to
/// find a vtable on F1 25 1.0.145 and crash within a minute of the core loading.
const SKIP_CUTSCENES: &str = "dromodf.skip-cutscenes";
const SKIP_CUTSCENES_LAST_BROKEN: &str = "1.9.4";
const SKIP_CUTSCENES_MODULE: &str = "skip_cutscenes.dll";
const SKIP_CUTSCENES_VTABLE: &str = "vtable not found";
/// The entrant list search by name (fn 0x141738eb0) that runs off the end of the
/// game's loaded lists when the name is missing.
const ENTRANT_LIST_SEARCH: &str = "F1_25.exe+0x1738f30 ";
/// Where F1 25 sets up the track for a session it loads.
const SETUP_TRACK: &str = "Sess_Loader_SetupTrackAssets";
/// The fault in the career hub's set-up: the read of the first record of a calendar event that matched nothing.
const SEQUENCER_RECORD: &str = "Game_CareerEventsSequencer_Initialise+0x1020";
/// The fault in the career hub's set-up when a driver of the player's team is not in the game's driver list: the
/// driver morph data read of the driver the lookup did not find.
const MORPH_DRIVER: &str = "Game_CareerEventResourceManager_AddDriverMorphData+0x82";
/// The frame that called the game's driver name fill for a race's retirement message: the return address of the call
/// whose driver the lookup did not find.
const RETIREMENT_NAME: &str = "Sess_InGameRace_Update+0x77e";
/// What the driver name fill reads for a missing driver: the composed name pointer of a null driver's name holder.
const RETIREMENT_READ: &str = "reading 0x1f0";
/// The fault in the weekend list builder of the Team HQ screens: the read of the node of a round's track record.
const WEEKENDS_TRACK_NODE: &str = "Career_Weekends_BuildResultNodes+0x4da";
/// The fault in the session loader's weekend set-up when the current season of the career's current championship has
/// no weekend at its current round: the read of the null weekend's session list (`mov r12, [r15+0x250]`), by name and
/// as the Steam 1.0.145 frame writes it unnamed.
const NO_WEEKEND_LOAD: [&str; 2] = [
    "Sess_WeekendManager_PrepareTrackLoad+0x11b",
    " fn base+0x240b9b0+0x11b",
];
const NO_WEEKEND_READ: &str = "reading 0x250";
/// Custom Calendar, whose picker (F9) replaces the career's calendar when it imports its rounds into the game's track
/// select screen, and the line it logs for that import.
const CUSTOM_CALENDAR: &str = "majorpayne8779.progressive-season-screen";
const CALENDAR_IMPORT: &str = "[Progressive season] Imported ";
/// F2 Revamp, whose career starts in the F2 championship.
const F2_REVAMP: &str = "miguelvf3.f2-revamp";
/// The game function that reads a car's racing line, which a car with no line of its own faults in.
const LINE_CACHE: &str = "Ai_LineCache_UpdateIfChanged+";
/// What the core logs for a car that is the third of its team.
const THIRD_CAR: &str = "is the third car of its garage";
/// How long after a career loaded a crash counts as loading its next session.
const CAREER_LOAD_SECS: f64 = 10.0;
/// How long before the crash Manual Pit Lane's lines count, and after it.
const PITLANE_SECS: f64 = 1.5;
const PITLANE_AFTER_SECS: f64 = 0.5;
/// The highest control mode a car really has; Manual Pit Lane prints a
/// garbage one when it read the car as the game moved it.
const PITLANE_MODES: u64 = 0xffff;
/// Addons known to keep F1 25 on Loading... after Press any button, so it never
/// reaches the main menu.
const BOOT_STALLS: &[&str] = &["crew8006.bp-ultimate-livery"];
/// The screen F1 25 shows after Press any button while it loads the main menu.
const START_SCREEN: &str = "splash_transition";
/// How long a session that ended on that screen had been on it to count as stuck.
const BOOT_STALL_SECS: f64 = 45.0;

const SEND_LOGS: &str = "Press Send Logs in the launcher so this reaches the developers.";

fn name(e: &Evidence, id: &str) -> String {
    e.names.get(id).cloned().unwrap_or_else(|| id.to_owned())
}

/// When a loader line was written, in seconds, from its `2026-10-27 15:00:08.518` start.
fn stamp(line: &str) -> Option<f64> {
    let mut date = line.get(..10)?.split('-').map(|n| n.parse::<u32>().ok());
    let (y, m, d) = (date.next()??, date.next()??, date.next()??);
    let mut time = line.get(11..23)?.split([':', '.']).map(|n| n.parse::<u32>().ok());
    let (h, min, sec, ms) = (time.next()??, time.next()??, time.next()??, time.next()??);
    let day = f64::from((y * 12 + m) * 31 + d);
    Some(day * 86400.0 + f64::from(h * 3600 + min * 60 + sec) + f64::from(ms) / 1000.0)
}

/// The fatal engine heap error the game ended with: its code and message.
fn heap_error(e: &Evidence) -> Option<(u32, String)> {
    let parse = |l: &str| {
        let rest = &l[l.find("Ne heap error 0x")? + "Ne heap error 0x".len()..];
        let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        let fatal =
            l.contains("fatal Ne heap error") || rest[hex.len()..].trim_start().starts_with("(fatal)");
        fatal.then(|| Some((u32::from_str_radix(&hex, 16).ok()?, rest.to_owned())))?
    };
    e.loader
        .lines()
        .rev()
        .filter(|l| l.contains("[crash] "))
        .find_map(parse)
        .or_else(|| {
            e.report
                .as_deref()?
                .lines()
                .find(|l| l.starts_with("reason"))
                .and_then(parse)
        })
}

fn windows_out_of_memory(code: u32, message: &str) -> bool {
    matches!(code, 0x5 | 0x58)
        || message.contains("useAddressSpace")
        || message.contains("Out of physical memory")
}

/// The enabled Classic Seasons year packs.
fn classic_years(e: &Evidence) -> usize {
    e.enabled
        .iter()
        .filter(|id| {
            id.strip_prefix("swofty.classic-seasons-")
                .is_some_and(|y| !y.is_empty() && y.bytes().all(|b| b.is_ascii_digit()))
        })
        .count()
}

const CAREER_ADDONS: [&str; 2] = ["swofty.progressive", "career.classic-career"];

fn needs_all_years(e: &Evidence) -> bool {
    e.enabled.iter().any(|id| CAREER_ADDONS.contains(&id.as_str()))
}

const KEEP_YEARS: &str = "Keep every Classic Seasons year on (Progressive needs them for the next seasons); turn off \
     imported tracks that aren't in your calendar and other content addons you don't need.";

fn years(n: usize) -> String {
    format!("{n} Classic Seasons year{}", if n == 1 { "" } else { "s" })
}

/// Whether the game's own hang watchdog closed it: it writes to address 1
/// with its 30 s limit in r15.
fn hang_watchdog(e: &Evidence, f: &Fatal) -> bool {
    let trap = |lines: &mut dyn Iterator<Item = &str>| {
        let (mut write, mut rax, mut limit) = (false, false, false);
        for l in lines {
            let t = l.trim_start();
            write |= t == "writing 0x1" || t.starts_with("writing 0x1:");
            rax |= t.starts_with("rax=0000000000000001 ");
            limit |= l.contains(WATCHDOG_MS);
        }
        write && rax && limit
    };
    let report = e.report.as_deref().is_some_and(|r| {
        let mut section = "";
        trap(&mut r.lines().filter(|l| {
            if let Some(name) = l.strip_prefix("== ") {
                section = name;
            }
            section == "Exception" || section == "Registers"
        }))
    });
    trap(&mut f.record.iter().map(String::as_str)) || report
}

/// The loader's crash lines as written, with their time.
fn raw_crash_lines(loader: &str) -> Vec<&str> {
    loader.lines().filter(|l| l.contains("[crash] ")).collect()
}

/// The addon that kept a game thread busy while the game froze: one whose
/// consecutive lines on a thread are far apart across the stall, or the one
/// that logged the most in it.
fn busy_addon(e: &Evidence, f: &Fatal, built_in: bool) -> Option<String> {
    let end = stamp(raw_crash_lines(&e.loader).get(f.at?)?)?;
    let ids: Vec<&String> = if e.enabled.is_empty() {
        e.names.keys().collect()
    } else {
        e.enabled.iter().collect()
    };
    let ids: Vec<&String> = ids
        .into_iter()
        .filter(|id| e.builtin.contains(*id) == built_in)
        .collect();
    let addon_of = |rest: &str| -> Option<&String> {
        let tag = rest.strip_prefix('[')?.split(']').next()?;
        if let Some((_, api)) = rest.split_once("] api: ") {
            let id = api.split(' ').next()?;
            return ids.iter().find(|i| i.as_str() == id).copied();
        }
        ids.iter()
            .find(|id| id.as_str() == tag || id.rsplit('.').next() == Some(tag))
            .copied()
    };
    let mut counts: BTreeMap<&String, usize> = BTreeMap::new();
    let mut last: BTreeMap<&str, (f64, &String)> = BTreeMap::new();
    let mut gap = None;
    for line in e.loader.lines() {
        let Some(t) = stamp(line) else { continue };
        if t > end {
            break;
        }
        let Some((thread, rest)) = line.get(24..).and_then(|r| r.split_once("] ")) else {
            continue;
        };
        match addon_of(rest) {
            Some(a) => {
                if t >= end - STALL_SECS {
                    *counts.entry(a).or_default() += 1;
                    if last
                        .get(thread)
                        .is_some_and(|(at, b)| *b == a && t - at > BUSY_GAP_SECS && end - at <= FREEZE_SECS)
                    {
                        gap = Some(a);
                    }
                }
                last.insert(thread, (t, a));
            }
            None => {
                last.remove(thread);
            }
        }
    }
    gap.or_else(|| {
        counts
            .into_iter()
            .filter(|(_, n)| *n >= BUSY_LINES)
            .max_by_key(|(_, n)| *n)
            .map(|(a, _)| a)
    })
    .cloned()
}

/// A graphics driver error the game hit in the minute before it froze: the
/// module, its vendor and the frame generation on its stack.
fn driver_error_before(e: &Evidence, f: &Fatal) -> Option<(String, &'static str, Option<&'static str>)> {
    let at = f.at?;
    let crash = crash_lines(&e.loader);
    let raw = raw_crash_lines(&e.loader);
    let end = stamp(raw.get(at)?)?;
    (0..at).rev().find_map(|i| {
        let l = crash[i];
        if !l.contains("first-chance") || stamp(raw[i]).is_none_or(|t| end - t > 2.0 * STALL_SECS) {
            return None;
        }
        let module = module_at(&l[l.find(" at ")? + 4..])?;
        let vendor = DRIVERS.iter().find(|(p, _)| module.starts_with(p))?.1;
        let frame_gen = record_at(&crash, i)
            .filter_map(module_at)
            .find_map(|m| FRAME_GEN.iter().find(|(p, _)| m.starts_with(p)).map(|(_, w)| *w));
        Some((module, vendor, frame_gen))
    })
}

/// Whether the game's graphics settings had frame generation on, as Swofty's
/// overlay last read them.
fn frame_gen_on(loader: &str) -> bool {
    loader
        .lines()
        .rev()
        .find(|l| l.contains("[overlay] the game's hardware settings"))
        .is_some_and(|l| l.contains("have frame generation on"))
}

/// Two or more full car packs that replace each other's cars in the build, by name.
fn car_packs(e: &Evidence) -> Vec<String> {
    let mut packs: Vec<&String> = Vec::new();
    for p in e.conflicts.iter().filter(|p| {
        p.resources >= CAR_PACK_RESOURCES && p.items.iter().any(|i| i.contains("vehicle_package/teams/"))
    }) {
        for id in [&p.winner, &p.loser] {
            if !packs.contains(&id) {
                packs.push(id);
            }
        }
    }
    if packs.len() < 2 {
        return Vec::new();
    }
    packs.sort_by_key(|id| e.enabled.iter().position(|x| x == *id));
    packs.iter().map(|id| name(e, id)).collect()
}

/// What the addons in the profile and the game's settings add to a graphics
/// card crash: sentences for what happened, and their steps.
fn graphics_causes(e: &Evidence) -> (String, Vec<String>) {
    let mut what = String::new();
    let mut steps = Vec::new();
    let packs = car_packs(e);
    if !packs.is_empty() {
        what.push_str(&format!(
            " {} are all full car packs, and they replace each other's cars. Several on at once can load more \
             than the graphics card can take.",
            list(&packs)
        ));
        steps.push(format!("Keep only one of {} on in your profile.", list(&packs)));
    }
    if frame_gen_on(&e.loader) {
        what.push_str(" Frame generation was on in F1 25's graphics settings.");
        steps.push(FRAME_GEN_OFF.to_owned());
    }
    (what, steps)
}

const FRAME_GEN_OFF: &str =
    "Turn off frame generation (DLSS, FSR or XeSS frame generation) in F1 25's graphics settings.";

fn app_control() -> Diagnosis {
    Diagnosis {
        id: "app_control",
        title: "Windows blocked Swofty's files from loading".to_owned(),
        what: "Windows stopped F1 25 from loading Swofty's version.dll (error 0xc0e90002, Bad Image), so the game \
               closed before it started. Smart App Control or an app control policy does this to files it doesn't \
               know."
            .to_owned(),
        steps: vec![
            "Turn off Smart App Control: open Windows Security, App & browser control, Smart App Control settings, \
             and choose Off."
                .to_owned(),
            "On a work or school PC, an app control policy from your organisation does this instead. Ask whoever \
             manages the PC to allow F1 25's folder."
                .to_owned(),
            "Press Play again.".to_owned(),
        ],
    }
}

fn saving(e: &Evidence) -> bool {
    e.report
        .as_deref()
        .is_some_and(|r| r.contains("Game_SaveData_WriteBag"))
}

fn absurd_request(message: &str) -> bool {
    message
        .split("Requested size:")
        .nth(1)
        .map(|rest| rest.chars().take_while(char::is_ascii_digit).collect::<String>())
        .and_then(|n| n.parse::<u64>().ok())
        .is_some_and(|n| n > 1_000_000_000_000_000)
}

fn replay_state_dropped(e: &Evidence) -> bool {
    e.loader
        .lines()
        .any(|l| l.contains("[replay] ") && l.contains("is not recorded"))
}

fn replay_state() -> Diagnosis {
    Diagnosis {
        id: "replay_state",
        title: "F1 25 read back a replay it could not record on a grid this big".to_owned(),
        what: "A season with 23 or more cars outgrew part of the game's replay recorder, so that part was left out \
               of the recording. Going back through the replay (a flashback, an instant replay) then read it as \
               garbage and the game closed. It is not a memory problem, and fewer mods or more memory in your PC \
               won't change it."
            .to_owned(),
        steps: vec![
            "Turn Flashbacks off in the assists for seasons with 23 or more cars.".to_owned(),
            "Avoid instant replays in those sessions until the next Swofty update.".to_owned(),
            "Press Play again.".to_owned(),
        ],
    }
}

/// Whether `id` is an import of Sepang, under any id the launcher installs one as.
fn is_sepang(id: &str) -> bool {
    id.split_once('.').is_some_and(|(_, name)| name == SEPANG)
}

fn game_heap(e: &Evidence) -> Diagnosis {
    let mut tracks: Vec<&String> = e.tracks.iter().collect();
    tracks.sort_by_key(|id| !is_sepang(id));
    let tracks: Vec<String> = tracks.iter().map(|id| name(e, id)).collect();
    let mut heavy = tracks.clone();
    let years_on = classic_years(e);
    let all_years = needs_all_years(e);
    if years_on > 0 {
        heavy.push(years(years_on));
    }
    let mut what = if saving(e) {
        "F1 25 keeps a fixed amount of memory for game data, and it filled up while the game was saving your \
         career, so the game closed. More memory in your PC doesn't change that."
    } else {
        "F1 25 keeps a fixed amount of memory for game data, and it filled up, so the game closed. More \
         memory in your PC doesn't change that."
    }
    .to_owned();
    if !heavy.is_empty() {
        what.push_str(&format!(
            " Imported tracks and season packs use a lot of it, and your profile has {} on.",
            list(&heavy)
        ));
    }
    let mut steps = Vec::new();
    if let Some(first) = tracks.first() {
        let one = tracks.len() == 1 || e.tracks.iter().any(|t| is_sepang(t));
        let (which, at) = match one {
            true => (first.clone(), first.clone()),
            false => (list(&tracks), "one of them".to_owned()),
        };
        steps.push(format!(
            "Turn off {which} in your profile, or make a lighter profile without it for the sessions it crashes in."
        ));
        if !all_years {
            steps[0].push_str(&format!(
                " In a My Team career whose next round is at {at}, change that round with the Career Mode Editor \
                 first: the career won't load while that track is off."
            ));
        }
    }
    steps.push(if all_years {
        KEEP_YEARS.to_owned()
    } else if years_on > 0 {
        "Turn on fewer imported tracks and only the Classic Seasons years you race.".to_owned()
    } else {
        "Play with fewer imported tracks and season addons on at once.".to_owned()
    });
    Diagnosis {
        id: "game_heap",
        title: "F1 25's game memory filled up".to_owned(),
        what,
        steps,
    }
}

fn windows_memory(e: &Evidence) -> Diagnosis {
    let years_on = classic_years(e);
    let fewer = if needs_all_years(e) {
        KEEP_YEARS.to_owned()
    } else if years_on > 0 {
        format!(
            "Turn on only the Classic Seasons years you race; your profile has {} on.",
            years(years_on)
        )
    } else {
        "Play with fewer car, livery and track addons on at once.".to_owned()
    };
    Diagnosis {
        id: "windows_out_of_memory",
        title: "Windows ran out of memory".to_owned(),
        what: "Windows had no memory left to give F1 25, so the game closed. This is your PC's memory (RAM and the \
               page file), not the graphics card's."
            .to_owned(),
        steps: vec![
            "Let Windows manage the page file: search Windows for View advanced system settings, open Performance \
             Settings, Advanced, Virtual memory, Change, and tick Automatically manage paging file size for all \
             drives. Keep 20 GB or more free on drive C."
                .to_owned(),
            "Close other programs, especially web browsers, while you play.".to_owned(),
            fewer,
        ],
    }
}

fn hang(e: &Evidence, f: &Fatal, mods: &[(&'static str, Vec<String>)]) -> Diagnosis {
    let mut what =
        "F1 25 stopped responding for 30 seconds, so its own watchdog closed it. It froze rather than \
                    crashed."
            .to_owned();
    let mut steps = Vec::new();
    if let Some(addon) = busy_addon(e, f, false) {
        let addon = name(e, &addon);
        what.push_str(&format!(" {addon} kept the game busy while it froze."));
        steps.push(format!("Turn off {addon} in your profile, then play again."));
    } else if let Some(addon) = busy_addon(e, f, true) {
        let addon = name(e, &addon);
        what.push_str(&format!(
            " The freeze happened inside the launcher's own code ({addon}, which is built in and can't be turned off)."
        ));
        steps.push(SEND_LOGS.to_owned());
    } else if let Some((addon, _)) = boot_stall(e) {
        what.push_str(&format!(
            " It never got past Loading... after Press any button, which {addon} is known to cause."
        ));
        steps.push(format!("Turn off {addon} in your profile, then play again."));
    } else if let Some((module, vendor, frame_gen)) = driver_error_before(e, f) {
        let during = frame_gen.map(|w| format!(" during {w}")).unwrap_or_default();
        what.push_str(&format!(
            " Just before, F1 25 hit an error in {module}, part of the {vendor} graphics driver{during}."
        ));
        if frame_gen.is_some() {
            steps.push(FRAME_GEN_OFF.to_owned());
        }
        steps.extend(
            driver_steps()
                .into_iter()
                .filter(|s| frame_gen.is_none() || !s.starts_with("Turn off frame generation")),
        );
    }
    if !mods.is_empty() {
        what.push_str(&format!(
            " Your game folder has {} installed, which hooks F1 25's graphics.",
            tools(mods)
        ));
        steps.insert(0, move_out(mods));
    }
    if steps.is_empty() {
        steps.push(
            "Press Play again. If F1 25 freezes at the same place again, turn off the addons you turned on last."
                .to_owned(),
        );
        steps.push(
            "Close other programs while you play, especially ones that record or draw over the screen."
                .to_owned(),
        );
    }
    Diagnosis {
        id: "hang_watchdog",
        title: "F1 25 froze and closed itself".to_owned(),
        what,
        steps,
    }
}

/// A row of perf.log's timeline: when it started, in seconds after the core
/// attached, and what the game was doing.
fn timeline_row(line: &str) -> Option<(f64, &str)> {
    let mut rest = line;
    let mut at = None;
    for i in 0..6 {
        let (field, more) = rest.trim_start().split_once(' ')?;
        match i {
            0 => {
                let mut hms = field.split(':').map(|n| n.parse::<u32>().ok());
                let (h, m, s) = (hms.next()??, hms.next()??, hms.next()??);
                at = Some(f64::from(h * 3600 + m * 60 + s));
            }
            _ if field != "-" && field.parse::<f64>().is_err() => return None,
            _ => {}
        }
        rest = more;
    }
    let what = rest.trim_start();
    let what = what.split("  [").next().unwrap_or(what);
    Some((at?, what.trim_end().trim_end_matches(" (current)")))
}

/// How long the game had been on the start screen when perf.log was last
/// written, when that is the last screen its timeline shows.
pub(crate) fn perf_start_screen(perf: &str) -> Option<f64> {
    let uptime: f64 = perf
        .lines()
        .find_map(|l| l.strip_prefix("uptime: "))?
        .split(' ')
        .next()?
        .parse()
        .ok()?;
    let mut timeline = false;
    let mut last = None;
    for line in perf.lines() {
        if let Some(section) = line.strip_prefix("== ") {
            timeline = section.starts_with("Timeline");
            continue;
        }
        let Some((at, what)) = timeline.then(|| timeline_row(line)).flatten() else {
            continue;
        };
        if let Some(screen) = what.split("; ").filter_map(|w| w.strip_prefix("screen ")).last() {
            last = Some((at, screen));
        }
    }
    let (at, screen) = last?;
    (screen == START_SCREEN).then_some(uptime - at)
}

/// The last screen the perf timeline names.
pub(crate) fn last_screen(perf: &str) -> Option<&str> {
    let mut timeline = false;
    let mut last = None;
    for line in perf.lines() {
        if let Some(section) = line.strip_prefix("== ") {
            timeline = section.starts_with("Timeline");
            continue;
        }
        let Some((_, what)) = timeline.then(|| timeline_row(line)).flatten() else {
            continue;
        };
        if let Some(screen) = what.split("; ").filter_map(|w| w.strip_prefix("screen ")).last() {
            last = Some(screen.split(' ').next().unwrap_or(screen));
        }
    }
    last
}

/// Whether an addon known to stall the boot explains the session's stall.
pub(crate) fn addon_stall(session: &SessionRecord, store: &Store) -> bool {
    boot_stall(&evidence(session, None, store)).is_some()
}

/// How long the game had been on the start screen when it ended: from the
/// crash report's black box, or else perf.log.
fn start_screen_secs(e: &Evidence) -> Option<f64> {
    let screen = e.report.as_deref().and_then(|r| {
        r.lines()
            .find_map(|l| l.trim_start().strip_prefix("last screen"))
            .map(str::trim_start)
    });
    match screen {
        Some(screen) => {
            let rest = screen
                .strip_prefix(START_SCREEN)?
                .trim_start()
                .strip_prefix('(')?;
            rest.split(' ').next()?.parse().ok()
        }
        None => perf_start_screen(e.perf.as_deref()?),
    }
}

/// An addon known to stall the boot, when the game sat on the start screen for
/// [`BOOT_STALL_SECS`] or more and never left it: its name and the seconds.
fn boot_stall(e: &Evidence) -> Option<(String, f64)> {
    let secs = start_screen_secs(e).filter(|s| *s >= BOOT_STALL_SECS)?;
    let on = |id: &str| match e.enabled.is_empty() {
        true => e.names.contains_key(id),
        false => e.enabled.iter().any(|on| on == id),
    };
    let id = BOOT_STALLS
        .iter()
        .find(|id| on(id) && !e.builtin.contains(**id))?;
    Some((name(e, id), secs))
}

fn minutes(secs: f64) -> String {
    match (secs / 60.0).round() as u64 {
        0 | 1 => format!("{} seconds", secs.round() as u64),
        n => format!("{n} minutes"),
    }
}

/// The cause of a session that ended without a crash, from the first rule that
/// recognises it.
pub fn stalled(e: &Evidence) -> Option<Diagnosis> {
    let Some((addon, secs)) = boot_stall(e) else {
        return stalled_save(e);
    };
    Some(Diagnosis {
        id: "boot_stall",
        title: format!("{addon} kept F1 25 on the loading screen"),
        what: format!(
            "F1 25 stayed on Loading... after Press any button for {} and never reached the main menu. \
             {addon} is known to cause this.",
            minutes(secs)
        ),
        steps: vec![format!(
            "Turn off {addon} in your profile, then press Play again."
        )],
    })
}

/// What the core logs for each profile load.
const PROFILE_LOAD: &str = "[saves] profile load ";

/// A start-screen stall right after the game read the profile save, with no addon known to cause one.
fn stalled_save(e: &Evidence) -> Option<Diagnosis> {
    let secs = start_screen_secs(e).filter(|s| *s >= crate::save_stall::STALL_SECS)?;
    if !e.loader.contains(PROFILE_LOAD) {
        return None;
    }
    Some(Diagnosis {
        id: "stalled_save",
        title: "F1 25 stayed on the loading screen after reading your save".to_owned(),
        what: format!(
            "F1 25 read your profile save, then stayed on Loading... after Press any button for {} and never \
             reached the main menu. A save last played with addon content that is off now (a career, livery or \
             item from an addon) can do this, also from Steam or the EA app without the launcher. When the same \
             save stalls again, the launcher moves it into the career vault before the next Play and puts back \
             the newest version that isn't stuck.",
            minutes(secs)
        ),
        steps: vec![
            "Press Play again. If it stalls a second time, the launcher puts back an earlier version of your save; \
             the stuck one stays on the Saves page."
                .to_owned(),
            "To get the stuck save back as it is, turn on the addons you last played it with, then restore it from \
             the Saves page."
                .to_owned(),
        ],
    })
}

/// The log line the core writes for a car whose fitted tyre models were never
/// placed on its wheels.
const TYRES_UNPLACED: &str = "fitted tyres were never placed on the wheels";

/// The car numbers, humans first, the core's tyre check found without placed
/// tyres: from `[tyres] car 24 (team 7, AI): fitted tyres were never placed ...`.
fn unplaced_cars(loader: &str) -> (BTreeSet<u32>, BTreeSet<u32>) {
    let (mut human, mut ai) = (BTreeSet::new(), BTreeSet::new());
    for line in loader.lines().filter(|l| l.contains(TYRES_UNPLACED)) {
        let Some(rest) = line.split_once("[tyres] car ").map(|(_, r)| r) else {
            continue;
        };
        let Some((number, rest)) = rest.split_once(' ') else {
            continue;
        };
        let Ok(number) = number.parse::<u32>() else {
            continue;
        };
        if rest
            .split_once("):")
            .is_some_and(|(who, _)| who.ends_with(", human"))
        {
            human.insert(number);
        } else {
            ai.insert(number);
        }
    }
    (human, ai)
}

fn car_numbers(cars: &BTreeSet<u32>) -> String {
    cars.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
}

/// A session whose cars drove without tyres: the core's tyre check logged the
/// cars whose fitted tyre models were never placed on their wheels.
pub fn unplaced_tyres(e: &Evidence) -> Option<Diagnosis> {
    let (human, ai) = unplaced_cars(&e.loader);
    if human.is_empty() && ai.is_empty() {
        return None;
    }
    let who = match (human.is_empty(), ai.is_empty()) {
        (false, true) => format!("your car (car {})", car_numbers(&human)),
        (false, false) => format!(
            "your car (car {}) and {} other car(s)",
            car_numbers(&human),
            ai.len()
        ),
        _ => format!("{} car(s) (cars {})", ai.len(), car_numbers(&ai)),
    };
    Some(Diagnosis {
        id: "tyres_not_placed",
        title: "Cars drove without any tyres".to_owned(),
        what: format!(
            "In this session the tyres of {who} were never put on the wheels, so they were invisible. The \
             car is not damaged and drives normally. This happens when an addon's car or handling data moves \
             the wheels away from where the car's tyre models sit, or when a grid has more cars than F1 25 \
             sets tyres up for."
        ),
        steps: vec![
            "If you installed or updated a car or handling addon just before this, turn it off in your \
             profile, then press Play again."
                .to_owned(),
            "If it keeps happening with every addon off, press Send Logs so the tyre lines in this \
             session's log reach support."
                .to_owned(),
        ],
    })
}

/// F1 25's Insufficient Storage dialog: the drive it names, and the MiB free
/// and needed there.
fn low_disk(loader: &str) -> Option<(String, u64, u64)> {
    loader.lines().rev().find_map(|l| {
        let rest = &l[l.find(LOW_DISK)? + LOW_DISK.len()..];
        let (free, rest) = rest.split_once('/')?;
        let (need, rest) = rest.split_once(" MiB available on ")?;
        let drive = rest
            .trim_start_matches(['\'', '"'])
            .get(..2)
            .filter(|d| d.ends_with(':') && d.starts_with(|c: char| c.is_ascii_alphabetic()))
            .map_or_else(
                || "the drive F1 25 saves to".to_owned(),
                |d| format!("drive {}", d[..1].to_ascii_uppercase()),
            );
        Some((drive, free.trim().parse().ok()?, need.trim().parse().ok()?))
    })
}

fn gb(mib: u64, round: fn(f64) -> f64) -> String {
    let tenths = round(mib as f64 / 102.4);
    if tenths % 10.0 == 0.0 {
        format!("{:.0} GB", tenths / 10.0)
    } else {
        format!("{:.1} GB", tenths / 10.0)
    }
}

fn low_disk_space(drive: &str, free: u64, need: u64) -> Diagnosis {
    let mut what = format!(
        "F1 25 needs {} free on {drive} and found {}, so it showed Insufficient Storage and closed.",
        gb(need, f64::round),
        gb(free, f64::floor)
    );
    if drive == "drive C" {
        what.push_str(" It checks drive C even when the game is installed on another drive.");
    }
    Diagnosis {
        id: "low_disk",
        title: format!("F1 25 needs more free space on {drive}"),
        what,
        steps: vec![
            format!(
                "Free up at least {} on {drive}: empty the Recycle Bin, and delete temporary files in Windows \
                 Settings, System, Storage, Temporary files.",
                gb(need.saturating_sub(free).max(1), f64::ceil)
            ),
            "Press Play again.".to_owned(),
        ],
    }
}

/// When the game's fatal exception happened, from its first-chance line or
/// else the line where it reached the top-level filter.
fn crash_time(e: &Evidence, f: &Fatal) -> Option<f64> {
    f.at.and_then(|at| raw_crash_lines(&e.loader).get(at).copied())
        .or_else(|| {
            e.loader
                .lines()
                .find(|l| l.contains("[crash] unhandled ") && l.contains("reached the top-level filter"))
        })
        .and_then(stamp)
}

/// Whether the game crashed as Manual Pit Lane took the car in the pit lane:
/// it decoded the car's control mode to garbage around the crash, or kept
/// control just before the game crashed where this crash happens.
fn pitlane_crash(e: &Evidence, f: &Fatal) -> bool {
    if f.module.as_deref() != Some("f1_25.exe") {
        return false;
    }
    let Some(end) = crash_time(e, f) else {
        return false;
    };
    let within = |l: &str, after: f64| stamp(l).is_some_and(|t| t >= end - PITLANE_SECS && t <= end + after);
    let (mut kept, mut garbage) = (false, false);
    for l in e.loader.lines().filter(|l| l.contains("] [pitlane] ")) {
        kept |= l.contains("] [pitlane] pit lane: control kept")
            && !l.contains("the AI keeps the car")
            && within(l, 0.0);
        garbage |= l
            .split_once(", mode ")
            .and_then(|(_, m)| m.split_once(" -> "))
            .and_then(|(_, to)| to.trim().parse::<u64>().ok())
            .is_some_and(|mode| mode > PITLANE_MODES)
            && within(l, PITLANE_AFTER_SECS);
    }
    let at = e
        .loader
        .lines()
        .any(|l| l.contains("[crash] unhandled ") && l.contains(PITLANE_CRASH));
    garbage || (kept && at)
}

/// Whether F1 25 crashed loading the track files of the Aggressive AI
/// (Alternate) version that ships broken ones: that version is on in the
/// profile, and the unhandled exception's address or one of the crash's
/// nearest frames is in [`AGGRESSIVE_AI_CRASH`].
fn aggressive_ai_crash(e: &Evidence, f: &Fatal) -> bool {
    let unhandled = crash_lines(&e.loader)
        .into_iter()
        .filter(|l| l.starts_with("unhandled ") && l.contains("reached the top-level filter"));
    e.enabled.iter().any(|id| id == AGGRESSIVE_AI)
        && e.versions.get(AGGRESSIVE_AI).map(String::as_str) == Some(AGGRESSIVE_AI_BROKEN)
        && f.frames
            .iter()
            .take(NEAR_FRAMES)
            .map(String::as_str)
            .chain(unhandled)
            .any(|l| l.contains("F1_25.exe+0x") && AGGRESSIVE_AI_CRASH.iter().any(|at| l.contains(at)))
}

/// Whether F1 25 1.0.145 crashed with a Skip Cutscenes version up to
/// [`SKIP_CUTSCENES_LAST_BROKEN`] on in the profile, in its own module or after
/// it logged that it could not find its vtable.
fn skip_cutscenes_crash(e: &Evidence, f: &Fatal) -> bool {
    let broken = e
        .versions
        .get(SKIP_CUTSCENES)
        .is_some_and(|v| semver::Version::parse(v.trim()).is_ok_and(|v| v <= semver::Version::new(1, 9, 4)));
    e.game_build == Some(crate::build_support::BUILD_1_0_145)
        && e.enabled.iter().any(|id| id == SKIP_CUTSCENES)
        && broken
        && (f.module.as_deref() == Some(SKIP_CUTSCENES_MODULE)
            || e.loader.lines().any(|l| l.contains(SKIP_CUTSCENES_VTABLE)))
}

fn skip_cutscenes(e: &Evidence) -> Diagnosis {
    let addon = name(e, SKIP_CUTSCENES);
    Diagnosis {
        id: "skip_cutscenes_1_0_145",
        title: format!("{addon} (up to {SKIP_CUTSCENES_LAST_BROKEN}) crashed F1 25 version 1.0.145"),
        what: format!(
            "{addon} (up to {SKIP_CUTSCENES_LAST_BROKEN}) crashes the game on F1 25 version 1.0.145 within a \
             minute of loading."
        ),
        steps: vec!["Turn it off on the Addons page until its author releases an update.".to_owned()],
    }
}

fn aggressive_ai(e: &Evidence) -> Diagnosis {
    let addon = name(e, AGGRESSIVE_AI);
    Diagnosis {
        id: "aggressive_ai_alternate",
        title: format!("{addon} {AGGRESSIVE_AI_BROKEN} crashed F1 25 while a session loaded"),
        what: format!(
            "Version {AGGRESSIVE_AI_BROKEN} of the {addon} addon crashes F1 25 while it loads a session, on most \
             tracks. That version has been taken down."
        ),
        steps: vec![
            format!(
                "Uninstall {addon} in the launcher, then install it again from the Addon Browser. That gives you \
                 the version the Addon Browser has now, which doesn't have this crash."
            ),
            format!("Or turn off {addon} in your profile."),
        ],
    }
}

fn entrant_list_crash(e: &Evidence, f: &Fatal) -> bool {
    let at = |l: &str| l.contains("unhandled ") && l.contains(ENTRANT_LIST_SEARCH);
    f.module.as_deref() == Some("f1_25.exe")
        && (crash_lines(&e.loader).into_iter().any(at) || e.report.iter().flat_map(|r| r.lines()).any(at))
}

/// The game function a stack frame names, as `Sess_Loader_SetupTrackAssets`
/// in `#5  F1_25.exe+0x2d48bb7 .xtls Sess_Loader_SetupTrackAssets+0x47`. None
/// for a frame the symbols don't name (`fn base+…`) and for other modules.
fn game_function(frame: &str) -> Option<&str> {
    let mut words = frame
        .split_whitespace()
        .skip_while(|w| !w.to_ascii_lowercase().starts_with("f1_25.exe+0x"))
        .skip(2);
    let name = words.next()?.split("+0x").next()?;
    (!name.is_empty() && name != "fn").then_some(name)
}

/// Whether F1 25 crashed setting up the track for a career's session: the
/// nearest game function the stack names is [`SETUP_TRACK`], and the career
/// loaded at most [`CAREER_LOAD_SECS`] before.
fn career_track_crash(e: &Evidence, f: &Fatal) -> bool {
    let setup = f.frames.iter().take(NEAR_FRAMES).find_map(|l| game_function(l)) == Some(SETUP_TRACK);
    if !setup || f.module.as_deref() != Some("f1_25.exe") {
        return false;
    }
    let Some(end) = crash_time(e, f) else {
        return false;
    };
    e.loader
        .lines()
        .rev()
        .filter(|l| l.contains("] [career] career.loaded: ") || l.contains("] [career] career.unloaded "))
        .filter_map(|l| Some((stamp(l)?, l)))
        .find(|(t, _)| *t <= end)
        .is_some_and(|(t, l)| l.contains("career.loaded: ") && end - t <= CAREER_LOAD_SECS)
}

/// Whether F1 25 faulted reading the first calendar event record while a career session was entered: the
/// nearest frame is [`SEQUENCER_RECORD`].
fn sequencer_record_crash(f: &Fatal) -> bool {
    f.module.as_deref() == Some("f1_25.exe") && f.frames.first().is_some_and(|l| l.contains(SEQUENCER_RECORD))
}

/// Whether F1 25 faulted in the AI's racing line update, the crash of a third car in a team that has no line.
fn third_car_line_crash(e: &Evidence, f: &Fatal) -> bool {
    f.module.as_deref() == Some("f1_25.exe")
        && f.frames.first().is_some_and(|l| l.contains(LINE_CACHE))
        && e.loader.contains(THIRD_CAR)
}

/// The enabled addons that add drivers or cars to a team.
fn third_car_addons(e: &Evidence) -> Vec<String> {
    e.enabled
        .iter()
        .filter(|id| {
            let id = id.to_ascii_lowercase();
            id.contains("more-driver") || id.contains("third") || id.contains("extra-car")
        })
        .map(|id| name(e, id))
        .collect()
}

fn third_car_line(e: &Evidence) -> Diagnosis {
    let addons = third_car_addons(e);
    let by = match addons.as_slice() {
        [] => "a team with three cars".to_owned(),
        _ => format!("{} giving a team a third car", list(&addons)),
    };
    let off = match addons.as_slice() {
        [] => "Go back to two cars in every team of My Team, or turn off the addon that adds the third car."
            .to_owned(),
        _ => format!(
            "Turn off {}, or go back to two cars in every team of My Team.",
            list(&addons)
        ),
    };
    Diagnosis {
        id: "third_car_line",
        title: "A third car in a team crashed F1 25".to_owned(),
        what: format!(
            "F1 25 crashed a moment after the session started, while the AI steered the cars. The cause is {by}: \
             the game gives the AI's racing line and tables to two cars per team, and a third car has none to read."
        ),
        steps: vec![off, "Press Play again.".to_owned()],
    }
}

/// The enabled addons that change a career's calendar: calendar and schedule packages, and DB Editor packages.
fn calendar_addons(e: &Evidence) -> Vec<String> {
    e.enabled
        .iter()
        .filter(|id| {
            let id = id.to_ascii_lowercase();
            id.contains("calend") || id.contains("schedule") || id.starts_with("dbedit.")
        })
        .map(|id| name(e, id))
        .collect()
}

fn calendar_event_missing(e: &Evidence) -> Diagnosis {
    let calendar = calendar_addons(e);
    let (title, cause) = match calendar.as_slice() {
        [] => (
            "Your career's calendar has a round F1 25 can't find".to_owned(),
            "A round of your career's calendar is not in the game's calendar, usually because an addon changed the \
             calendar or a track it uses is missing."
                .to_owned(),
        ),
        _ => (
            format!("{} changed your career's calendar", list(&calendar)),
            format!(
                "{} changed the rounds of the calendar, and a round of your career now points at a track or event \
                 the game can't find.",
                list(&calendar)
            ),
        ),
    };
    let off = match calendar.as_slice() {
        [] => "Turn off the addon that changes your calendar (a calendar or DB Editor package), or one that adds a track the calendar uses."
            .to_owned(),
        _ => format!(
            "Turn off {}, or make sure every track it points the calendar at is installed and on.",
            list(&calendar)
        ),
    };
    Diagnosis {
        id: "calendar_event_missing",
        title,
        what: format!(
            "F1 25 crashed as your career's first session loaded, while it set up the career hub for that round. {cause}"
        ),
        steps: vec![off, "Press Play again.".to_owned()],
    }
}

/// Whether F1 25 faulted reading the driver it passes to the career hub's driver morph data while a career session
/// was entered: the nearest frame is [`MORPH_DRIVER`].
fn morph_driver_crash(f: &Fatal) -> bool {
    f.module.as_deref() == Some("f1_25.exe") && f.frames.first().is_some_and(|l| l.contains(MORPH_DRIVER))
}

/// Whether F1 25 faulted naming the driver of a car that retired from a race: the frame after the nearest is
/// [`RETIREMENT_NAME`] and the fault read [`RETIREMENT_READ`].
fn retirement_driver_crash(f: &Fatal) -> bool {
    f.module.as_deref() == Some("f1_25.exe")
        && f.frames.get(1).is_some_and(|l| l.contains(RETIREMENT_NAME))
        && f.record.iter().any(|l| l.trim_end().ends_with(RETIREMENT_READ))
}

/// Whether F1 25 faulted reading the track node of a round while Team HQ listed the season's weekends: the nearest
/// frame is [`WEEKENDS_TRACK_NODE`].
fn weekends_track_crash(f: &Fatal) -> bool {
    f.module.as_deref() == Some("f1_25.exe")
        && f.frames.first().is_some_and(|l| l.contains(WEEKENDS_TRACK_NODE))
}

/// Whether F1 25 faulted loading the first session of a career whose current season has no weekend, after Custom
/// Calendar's picker imported the career's rounds: the nearest frame is in [`NO_WEEKEND_LOAD`], the fault read
/// [`NO_WEEKEND_READ`] and the loader logged [`CALENDAR_IMPORT`] before the crash.
fn imported_calendar_crash(e: &Evidence, f: &Fatal) -> bool {
    let at = f.module.as_deref() == Some("f1_25.exe")
        && f.frames
            .first()
            .is_some_and(|l| NO_WEEKEND_LOAD.iter().any(|site| l.contains(site)))
        && f.record.iter().any(|l| l.trim_end().ends_with(NO_WEEKEND_READ));
    let end = crash_time(e, f);
    at && e
        .loader
        .lines()
        .any(|l| l.contains(CALENDAR_IMPORT) && end.is_none_or(|end| stamp(l).is_none_or(|t| t <= end)))
}

fn imported_calendar_empty_season(e: &Evidence) -> Diagnosis {
    let picker = name(e, CUSTOM_CALENDAR);
    let career = match e.enabled.iter().any(|id| id == F2_REVAMP) {
        true => format!(" into {}'s career, which starts in F2", name(e, F2_REVAMP)),
        false => String::new(),
    };
    Diagnosis {
        id: "imported_calendar_empty_season",
        title: format!("{picker}'s imported calendar left your career without a first round"),
        what: format!(
            "F1 25 crashed loading the first session of your new career: the career's first season has no rounds. \
             Its calendar was imported with the {picker} picker (F9){career}, and that import left the first season \
             empty."
        ),
        steps: vec![
            format!(
                "Start the career again and choose its season on the game's own screens, without the {picker} picker \
                 (F9). The career you just started has no first round, so it can't be played."
            ),
            format!("Or turn off {picker} in your profile before you start the career."),
            "Press Play again.".to_owned(),
        ],
    }
}

fn weekends_track_record_bad_index() -> Diagnosis {
    Diagnosis {
        id: "weekends_track_record_bad_index",
        title: "A round of your career has a track F1 25 can't find".to_owned(),
        what: "F1 25 crashed as Team HQ listed the weekends of a season. A round of that season is held at a track \
               that is not in the game's track list."
            .to_owned(),
        steps: vec![
            "Turn on the track addons the career's earlier seasons use, or turn off the addon that changed the \
             tracks of the calendar."
                .to_owned(),
            "Press Play again.".to_owned(),
        ],
    }
}

fn race_driver_missing() -> Diagnosis {
    Diagnosis {
        id: "race_driver_missing",
        title: "A car in your race has a driver F1 25 can't find".to_owned(),
        what: "F1 25 crashed as a car retired from the race, while it named that car's driver in the retirement \
               message. That car's driver is not in the game's driver list, usually because an addon changed the \
               career's drivers or line-ups."
            .to_owned(),
        steps: vec![
            "Turn off the addon that changes your career's drivers or line-ups (a driver market, team and driver \
             editor or DB Editor package)."
                .to_owned(),
            "Press Play again.".to_owned(),
        ],
    }
}

fn career_driver_missing() -> Diagnosis {
    Diagnosis {
        id: "career_driver_missing",
        title: "Your career has a driver F1 25 can't find".to_owned(),
        what: "F1 25 crashed as your career's session loaded, while it set up the driver models of your team for \
               the career hub. A driver of your team in this career is not in the game's driver list, usually \
               because an addon changed the career's drivers or line-ups."
            .to_owned(),
        steps: vec![
            "Turn off the addon that changes your career's drivers or line-ups (a driver market, team and driver \
             editor or DB Editor package)."
                .to_owned(),
            "Press Play again.".to_owned(),
        ],
    }
}

/// The text `rdi` points at in the crash report's memory dump, the name the game looked up.
fn searched_name(report: &str) -> Option<String> {
    let mut lines = report
        .lines()
        .skip_while(|l| !l.starts_with("== Memory at the registers"))
        .skip_while(|l| !l.trim_start().starts_with("rdi="));
    let rdi = u64::from_str_radix(lines.next()?.trim_start().get(4..20)?, 16).ok()?;
    let mut dump = BTreeMap::new();
    for l in lines.take_while(|l| l.starts_with("    ")) {
        let t = l.trim_start();
        let Some(addr) = t.get(..16).and_then(|a| u64::from_str_radix(a, 16).ok()) else {
            continue;
        };
        let bytes: Vec<u8> = t
            .get(17..)
            .unwrap_or_default()
            .split_whitespace()
            .take(16)
            .map_while(|b| u8::from_str_radix(b, 16).ok())
            .collect();
        dump.insert(addr, bytes);
    }
    let mut name = Vec::new();
    let mut addr = rdi;
    loop {
        let byte = *dump.get(&(addr & !0xf))?.get((addr & 0xf) as usize)?;
        if byte == 0 {
            break;
        }
        name.push(byte);
        if name.len() > 64 {
            return None;
        }
        addr += 1;
    }
    let ok = !name.is_empty()
        && name
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_');
    ok.then(|| String::from_utf8_lossy(&name).into_owned())
}

fn missing_entrant_list(e: &Evidence) -> Diagnosis {
    let list = e.report.as_deref().and_then(searched_name);
    let addon = e
        .kept_lists
        .iter()
        .filter(|(id, _)| !e.builtin.contains(*id))
        .find(|(_, lists)| list.as_ref().is_none_or(|l| lists.contains(l)))
        .map(|(id, _)| name(e, id));
    let which = list
        .as_ref()
        .map_or("an entrant list".to_owned(), |l| format!("the entrant list {l}"));
    let by = addon.as_ref().map_or(String::new(), |a| {
        format!(" The last build found {a} renaming or removing it.")
    });
    let mut steps = vec![match &addon {
        Some(a) => format!("Turn off {a}, or ask its author for a version made for this F1 25 update."),
        None => "Turn off the database addons you added most recently, such as imported team packs, one at a time \
                 until the crash stops."
            .to_owned(),
    }];
    steps.push("Press Play again.".to_owned());
    Diagnosis {
        id: "missing_entrant_list",
        title: format!("F1 25 crashed looking for {which}"),
        what: format!(
            "F1 25 looks up {which} by name when it builds a menu such as the F1 World Showroom, and the game's \
             database has no such list. A database addon renames or removes it, often one made for an older F1 25 \
             update.{by}"
        ),
        steps,
    }
}

/// The track stat id the core logged as having no tracks row or track record at the session load.
fn missing_track_stat(loader: &str) -> Option<i64> {
    loader.lines().rev().find_map(|l| {
        let rest = l.split("track stat id ").nth(1)?;
        let (id, tail) = rest.split_once(' ')?;
        tail.starts_with("has no tracks row or track record")
            .then(|| id.parse().ok())
            .flatten()
    })
}

fn career_content_off(e: &Evidence) -> Diagnosis {
    let off: Vec<String> = e.off.iter().map(|id| name(e, id)).collect();
    let what =
        "F1 25 crashed setting up the track for your career's next session, just after the career loaded.";
    let missing = missing_track_stat(&e.loader);
    let (title, what) = match off.as_slice() {
        [] => (
            "Your career uses a track that is not in this profile".to_owned(),
            format!(
                "{what} A race in your career uses a track (track stat id {}) that no addon in this profile provides.",
                missing.unwrap_or_default()
            ),
        ),
        [one] => (
            format!("Your career uses {one}, which is off"),
            format!("{what} A race in your career uses {one}, which is off in this profile."),
        ),
        _ => (
            "Your career uses a track or season that is off".to_owned(),
            format!(
                "{what} A race in your career uses a track or season that is off in this profile, which has {} off.",
                list(&off)
            ),
        ),
    };
    Diagnosis {
        id: "career_content_off",
        title,
        what,
        steps: vec![
            match off.is_empty() {
                true => "Turn back on the track addon your career's calendar uses. In a My Team career you can also change that round with the Career Mode Editor."
                    .to_owned(),
                false => format!(
                    "Turn {} back on. In a My Team career you can also change that round with the Career Mode Editor.",
                    list(&off)
                ),
            },
            "Press Play again.".to_owned(),
        ],
    }
}

/// The core's `[hwsettings]` line about the player's hardware_settings_config.xml, from the loader.log.
fn hardware_settings_line(loader: &str) -> Option<&str> {
    loader
        .lines()
        .find_map(|l| l.find("[hwsettings] ").map(|at| &l[at..]))
}

/// Whether the game aborted in its startup sequence, right after reading its hardware settings file, before
/// any window, with the core's line saying that file is a OneDrive file or cloud-only.
fn hardware_settings_crash(e: &Evidence, f: &Fatal) -> bool {
    let at_abort = f
        .frames
        .iter()
        .any(|l| l.contains("Game_Startup_InitSequence+0x291"));
    let line = hardware_settings_line(&e.loader);
    at_abort
        && e.loader.contains("before the game's window")
        && line.is_some_and(|l| l.contains("cloud-only=yes") || l.contains("onedrive=yes"))
}

fn hardware_settings(e: &Evidence) -> Diagnosis {
    let cloud = hardware_settings_line(&e.loader).is_some_and(|l| l.contains("cloud-only=yes"));
    let what = if cloud {
        "F1 25 closed while it started up because it could not read its graphics settings file, \
         hardware_settings_config.xml in Documents\\My Games\\F1 25\\hardwaresettings. OneDrive has that file in \
         the cloud only, not on your PC, and the game can't download it."
    } else {
        "F1 25 closed while it started up because it could not read its graphics settings file, \
         hardware_settings_config.xml in Documents\\My Games\\F1 25\\hardwaresettings. Your Documents folder \
         is in OneDrive, which can leave that file unreadable while it syncs or after it is moved to the cloud."
    };
    Diagnosis {
        id: "hardware_settings_unreadable",
        title: "F1 25 couldn't read its graphics settings file".to_owned(),
        what: what.to_owned(),
        steps: vec![
            "In File Explorer, right-click Documents\\My Games\\F1 25\\hardwaresettings, choose \"Always keep on this device\", and wait for OneDrive to finish syncing, then press Play again."
                .to_owned(),
            "If it still closes, quit OneDrive, rename hardware_settings_config.xml in that folder to hardware_settings_config.xml.bak (F1 25 makes a new one, with its graphics settings back at their defaults), and press Play again."
                .to_owned(),
            "If it still closes, move your Documents folder out of OneDrive: OneDrive settings, Sync and backup, Manage backup, turn Documents off."
                .to_owned(),
        ],
    }
}

fn pitlane(e: &Evidence) -> Diagnosis {
    let addon = e
        .names
        .get(PITLANE)
        .cloned()
        .unwrap_or_else(|| "Manual Pit Lane".to_owned());
    Diagnosis {
        id: "pitlane_garage",
        title: format!("{addon} crashed F1 25 in the pit lane"),
        what: format!(
            "F1 25 crashed as {addon} took over your car in the pit lane while the game was moving the car \
             itself, for example for Return to Garage or a flashback."
        ),
        steps: vec![
            format!("Turn off {addon} in your profile until its author fixes this crash."),
            "If you keep it on, don't use Return to Garage or flashbacks while you are in the pit lane."
                .to_owned(),
        ],
    }
}

const QUIT_SECS: f64 = 10.0;

/// The flow graph's root state, StateApp, as the black box prints it: by its name hash, or by name. It exits only
/// when the game itself closes; every other state (a session, a screen's flow) exits while the game keeps running.
const APP_FLOW: [&str; 2] = ["#e11edb87", "StateApp"];

/// Whether the crash report's black box shows the game itself was closing: its root flow state had just exited,
/// with a first-chance access violation in the game's own code just before the process ended.
fn quit_fault(e: &Evidence) -> bool {
    let Some(report) = &e.report else {
        return false;
    };
    let secs_before = |line: &str| -> Option<f64> {
        let rest = &line[line.find('(')? + 1..];
        rest.split(" s before").next()?.parse().ok()
    };
    let state = |key: &str| report.lines().find(|l| l.trim_start().starts_with(key));
    let exiting = state("last flow").is_some_and(|l| {
        let mut words = l.split_whitespace().skip(2);
        words.next().is_some_and(|flow| APP_FLOW.contains(&flow))
            && words.next() == Some("Exiting")
            && secs_before(l).is_some_and(|s| s <= QUIT_SECS)
    });
    let violation = state("last exception")
        .is_some_and(|l| l.contains("ACCESS_VIOLATION") && l.to_ascii_lowercase().contains(" at f1_25.exe+"));
    exiting && violation
}

fn quit_fault_diagnosis() -> Diagnosis {
    Diagnosis {
        id: "quit_fault",
        title: "F1 25 hit an error as it closed".to_owned(),
        what: "F1 25 was already closing when it hit an error and ended itself. This is a fault in the game's own \
               shutdown, not a crash during play, and nothing you were doing is lost."
            .to_owned(),
        steps: vec!["Nothing to fix. Press Play when you want to play again.".to_owned()],
    }
}

/// The stat ids of `vehicle_packages` rows, as `ids.json` and `row-ids.json` in the data folder hold them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageIds {
    /// Every id held, by the addon that holds it.
    pub held: BTreeMap<i64, String>,
}

pub const PACKAGE_ID_MAX: i64 = 254;
const PACKAGE_TABLE: &str = "entrants.vehicles.vehicle_packages.vehicle_package";
const PACKAGE_LOW: i64 = 8;

impl PackageIds {
    pub fn read(data: &Path) -> Option<Self> {
        let load = |name: &str| -> Option<serde_json::Value> {
            let text = std::fs::read_to_string(crate::stdlib::reports::path(data, name)).ok()?;
            serde_json::from_str(&text).ok()
        };
        let ids = load(crate::stdlib::reports::IDS);
        let rows = load(crate::row_ids::FILE);
        if ids.is_none() && rows.is_none() {
            return None;
        }
        let mut held = BTreeMap::new();
        let mut records = |doc: &serde_json::Value| {
            if let Some(map) = doc["vehicle.package"].as_object() {
                for (record, e) in map {
                    if let Some(id) = e.as_i64().or_else(|| e["id"].as_i64()) {
                        let addon = record.split_once('/').map_or(record.as_str(), |(a, _)| a);
                        held.insert(id, addon.to_owned());
                    }
                }
            }
        };
        if let Some(ids) = &ids {
            records(&ids["ids"]);
            if let Some(careers) = ids["careers"].as_object() {
                careers.values().for_each(&mut records);
            }
        }
        if let Some(rows) = &rows {
            let mut take = |map: &serde_json::Value| {
                for (key, e) in map.as_object().into_iter().flatten() {
                    let mut parts = key.splitn(3, '/');
                    let (Some(addon), Some(table)) = (parts.next(), parts.next()) else {
                        continue;
                    };
                    if table.starts_with(PACKAGE_TABLE)
                        && let Some(id) = e["id"].as_i64()
                    {
                        held.insert(id, addon.to_owned());
                    }
                }
            };
            take(&rows["rows"]);
            for p in rows["profiles"].as_object().into_iter().flat_map(|m| m.values()) {
                take(&p["rows"]);
            }
        }
        held.retain(|id, _| (1..=PACKAGE_ID_MAX).contains(id));
        Some(Self { held })
    }

    pub fn used(&self) -> usize {
        self.held.len()
    }

    pub fn highest(&self) -> Option<i64> {
        self.held.keys().next_back().copied()
    }

    pub fn free(&self) -> usize {
        PACKAGE_ID_MAX as usize - self.used()
    }

    /// The ids held by addons that aren't in `installed`, by addon.
    pub fn orphaned(&self, installed: &BTreeSet<String>) -> BTreeMap<&str, usize> {
        let mut out = BTreeMap::new();
        for addon in self.held.values().filter(|a| !installed.contains(*a)) {
            *out.entry(addon.as_str()).or_insert(0) += 1;
        }
        out
    }

    pub fn summary(&self, installed: &BTreeSet<String>) -> String {
        let orphans: usize = self.orphaned(installed).values().sum();
        format!(
            "{} of {PACKAGE_ID_MAX} used, highest {}, {orphans} held by addons not installed",
            self.used(),
            self.highest().unwrap_or(0)
        )
    }

    /// A diagnosis when few ids are left: the addons not installed whose ids could be freed come first.
    pub fn diagnosis(
        &self,
        installed: &BTreeSet<String>,
        names: &BTreeMap<String, String>,
    ) -> Option<Diagnosis> {
        if self.free() as i64 > PACKAGE_LOW {
            return None;
        }
        let orphans = self.orphaned(installed);
        let mut what = format!(
            "The car package ids (vehicle_packages stat_id) are nearly used up: {}.",
            self.summary(installed)
        );
        let mut steps = Vec::new();
        if orphans.is_empty() {
            steps.push(
                "Every held id belongs to an installed addon: turn off or remove car addons you don't use, then press Play again."
                    .to_owned(),
            );
        } else {
            let list: Vec<String> = orphans
                .iter()
                .map(|(a, n)| format!("{} ({n})", names.get(*a).map_or(*a, String::as_str)))
                .collect();
            what.push_str(&format!(
                " Ids are held by addons that are not installed: {}.",
                list.join(", ")
            ));
            steps.push("Their ids are reused once a build needs them; if a build still fails, remove car addons you don't use.".to_owned());
        }
        Some(Diagnosis {
            id: "package_ids_full",
            title: "Car package ids are nearly used up".to_owned(),
            what,
            steps,
        })
    }
}

/// The cause of a crash, from the first rule that recognises it. A crash no
/// rule recognises, on an F1 25 older than [`LATEST_GAME_VERSION`], is put down
/// to the old build.
pub fn diagnose(e: &Evidence) -> Option<Diagnosis> {
    matched(e).or_else(|| {
        let mut d = outdated_exe(e.game_version.as_deref())?;
        d.what = format!(
            "Your F1 25 is {}. Mods are made for {LATEST_GAME_VERSION}, the latest update, and F1 25 crashed on \
             the older one.",
            e.game_version.as_deref().unwrap_or_default().trim()
        );
        Some(d)
    })
}

fn matched(e: &Evidence) -> Option<Diagnosis> {
    if let Some((drive, free, need)) = low_disk(&e.loader) {
        return Some(low_disk_space(&drive, free, need));
    }
    if e.code == Some(APP_CONTROL) {
        return Some(app_control());
    }
    if quit_fault(e) {
        return Some(quit_fault_diagnosis());
    }
    let f = fatal(e);
    let loaded = loaded_modules(&e.loader);
    let mods = graphics_mods(&e.game_files, &loaded);
    for hook in HOOKS {
        let own = f.module.as_deref().is_some_and(|m| m.starts_with(hook.prefix));
        let near = !hook.own_only && f.near().any(|m| m.starts_with(hook.prefix));
        if !own && !near {
            continue;
        }
        let module = f
            .near()
            .find(|m| m.starts_with(hook.prefix))
            .unwrap_or(hook.prefix)
            .to_owned();
        return Some(Diagnosis {
            id: "third_party_hook",
            title: format!("F1 25 crashed inside {}", hook.product),
            what: format!(
                "The crash happened in {module}, which belongs to {}. It hooks into games while they run, and here \
                 it took F1 25 down with it.",
                hook.product
            ),
            steps: vec![hook.step.to_owned(), "Press Play again.".to_owned()],
        });
    }
    if aggressive_ai_crash(e, &f) {
        return Some(aggressive_ai(e));
    }
    if skip_cutscenes_crash(e, &f) {
        return Some(skip_cutscenes(e));
    }
    if let Some((hook, module)) = stray_jump_hook(&f, &e.loader) {
        return Some(Diagnosis {
            id: "third_party_hook",
            title: format!("F1 25 crashed inside {}", hook.product),
            what: format!(
                "F1 25 jumped to an address that is not code while {module} was at work, which belongs to {}. It \
                 hooks into games while they run, and here it took F1 25 down with it.",
                hook.product
            ),
            steps: vec![hook.step.to_owned(), "Press Play again.".to_owned()],
        });
    }
    let windows = !wine(e, &loaded);
    if let Some((code, message)) = heap_error(e) {
        if code == 0xc && message.contains("'GAME_DEFAULT'") {
            if absurd_request(&message) && replay_state_dropped(e) {
                return Some(replay_state());
            }
            return Some(game_heap(e));
        }
        if windows && windows_out_of_memory(code, &message) {
            return Some(windows_memory(e));
        }
    }
    if hang_watchdog(e, &f) {
        return Some(hang(e, &f, &mods));
    }
    if pitlane_crash(e, &f) {
        return Some(pitlane(e));
    }
    if hardware_settings_crash(e, &f) {
        return Some(hardware_settings(e));
    }
    if entrant_list_crash(e, &f) {
        return Some(missing_entrant_list(e));
    }
    if (!e.off.is_empty() || missing_track_stat(&e.loader).is_some()) && career_track_crash(e, &f) {
        return Some(career_content_off(e));
    }
    if sequencer_record_crash(&f) {
        return Some(calendar_event_missing(e));
    }
    if morph_driver_crash(&f) {
        return Some(career_driver_missing());
    }
    if retirement_driver_crash(&f) {
        return Some(race_driver_missing());
    }
    if weekends_track_crash(&f) {
        return Some(weekends_track_record_bad_index());
    }
    if imported_calendar_crash(e, &f) {
        return Some(imported_calendar_empty_season(e));
    }
    if third_car_line_crash(e, &f) {
        return Some(third_car_line(e));
    }
    if windows && gpu_removed(e) {
        let overlay = SessionOverlay::scan(&e.loader).unwrap_or_default();
        let addons = match overlay.removal {
            Some(Removal::AfterDraw { .. }) if overlay.drew => painters(e, &overlay),
            _ => Vec::new(),
        };
        if overlay.removal == Some(Removal::AfterDraw { at_present: true }) && !addons.is_empty() {
            let mut steps = vec![format!(
                "Turn off {} in your profile, then play again.",
                list(&addons)
            )];
            steps.extend(driver_steps().into_iter().take(2));
            return Some(Diagnosis {
                id: "gpu_removed_overlay",
                title: "Your graphics card stopped responding while Swofty's overlay was drawing".to_owned(),
                what: format!(
                    "Windows reset the graphics driver (DXGI_ERROR_DEVICE_REMOVED) shortly after Swofty's in-game \
                     overlay drew for {}, and F1 25 can't recover from that, so it closed. The overlay is \
                     experimental.",
                    list(&addons)
                ),
                steps,
            });
        }
        let (causes, mut steps) = graphics_causes(e);
        if !mods.is_empty() {
            steps.insert(0, move_out(&mods));
        }
        let frame_gen = steps.iter().any(|s| s == FRAME_GEN_OFF);
        steps.extend(
            driver_steps()
                .into_iter()
                .filter(|s| !frame_gen || !s.starts_with("Turn off frame generation")),
        );
        if !addons.is_empty() {
            steps.push(format!(
                "If it keeps happening, turn off {} in your profile for one session.",
                list(&addons)
            ));
        }
        let also = if mods.is_empty() {
            String::new()
        } else {
            format!(
                " Your game folder also has {} installed, which hooks F1 25's graphics.",
                tools(&mods)
            )
        };
        let cause = if causes.is_empty() {
            " This comes from the graphics card or its driver, not from Swofty's code."
        } else {
            causes.as_str()
        };
        return Some(Diagnosis {
            id: "gpu_removed",
            title: "Your graphics driver stopped responding".to_owned(),
            what: format!(
                "Windows reset the graphics driver while F1 25 was running (DXGI_ERROR_DEVICE_REMOVED). F1 25 can't \
                 recover from that, so it closed.{cause}{also}"
            ),
            steps,
        });
    }
    if windows && out_of_memory(e) {
        let (causes, mut steps) = graphics_causes(e);
        steps.extend([
            "Lower Texture Quality, and the resolution if it's above your screen's, in F1 25's graphics settings."
                .to_owned(),
            if needs_all_years(e) {
                KEEP_YEARS.to_owned()
            } else {
                "Play with fewer car, livery and track addons on at once, for example only the Classic Seasons years \
                 you race."
                    .to_owned()
            },
            "Close other programs, especially web browsers, while you play.".to_owned(),
            "On a laptop with integrated graphics, give the graphics more memory if its settings allow it (often \
             called UMA frame buffer size)."
                .to_owned(),
        ]);
        let what = if causes.is_empty() {
            " Car, livery and track addons use a lot of graphics memory, and several at once can use more than the \
             graphics card has."
        } else {
            causes.as_str()
        };
        return Some(Diagnosis {
            id: "out_of_memory",
            title: "F1 25 ran out of graphics memory".to_owned(),
            what: format!(
                "The game couldn't get the memory it asked for (E_OUTOFMEMORY), so it closed.{what}"
            ),
            steps,
        });
    }
    let started = crate::launch::LoaderSeen::scan(&e.loader).window;
    if !mods.is_empty() && (!started || f.near().any(graphics)) {
        let when = if started {
            "in its graphics code"
        } else {
            "while it was setting up its graphics, before its window opened"
        };
        return Some(Diagnosis {
            id: "graphics_mods",
            title: format!("{} in your game folder crashed F1 25", tools(&mods)),
            what: format!(
                "Your F1 25 folder has {} installed. It changes how F1 25 draws, and F1 25 crashed {when}.",
                tools(&mods)
            ),
            steps: vec![
                move_out(&mods),
                VERIFY.to_owned(),
                "Press Play. If F1 25 starts, you can put the files back one at a time to find the one that breaks \
                 it."
                    .to_owned(),
            ],
        });
    }
    if let Some((module, vendor)) = f.module.as_deref().filter(|_| windows).and_then(|m| {
        DRIVERS
            .iter()
            .find(|(p, _)| m.starts_with(p))
            .map(|(_, v)| (m, *v))
    }) {
        return Some(Diagnosis {
            id: "graphics_driver",
            title: format!("F1 25 crashed inside your {vendor} graphics driver"),
            what: format!("The crash happened in {module}, part of the {vendor} graphics driver."),
            steps: driver_steps(),
        });
    }
    if let Some((module, what)) = f.module.as_deref().and_then(|m| {
        FRAME_GEN
            .iter()
            .find(|(p, _)| m.starts_with(p))
            .map(|(_, w)| (m, *w))
    }) {
        let overlay = SessionOverlay::scan(&e.loader).unwrap_or_default();
        let mut steps = vec![format!(
            "Turn off {what} in F1 25's graphics settings, then play again."
        )];
        let mut also = String::new();
        if overlay.drew && !painters(e, &overlay).is_empty() {
            let addons = list(&painters(e, &overlay));
            also = format!(
                " Swofty's in-game overlay was drawing for {addons} at the time, and it is experimental."
            );
            steps.insert(0, format!("Turn off {addons} in your profile."));
        }
        steps.push("Update your graphics driver.".to_owned());
        return Some(Diagnosis {
            id: "frame_generation",
            title: format!("F1 25 crashed in {what}"),
            what: format!("The crash happened in {module}, the game's {what} code.{also}"),
            steps,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYRES_LOG: &str = "\
2026-11-03 10:00:10.000 [1] [tyres] car 23 (team 4, AI): fitted tyres were never placed on the wheels (3 of 3 fitted tyre model(s) still at the spawn position -1000 and size 60/30, session 12, 10 s in); the car has no visible tyres
2026-11-03 10:00:10.000 [1] [tyres] car 24 (team 4, human): fitted tyres were never placed on the wheels (3 of 3 fitted tyre model(s) still at the spawn position -1000 and size 60/30, session 12, 10 s in); the car has no visible tyres
2026-11-03 10:00:10.000 [1] [tyres] fitted tyre check (10 s in), session 12: 2 of 26 car(s) have unplaced tyres (cars 23, 24)
2026-11-03 10:01:00.000 [1] [tyres] car 23 (team 4, AI): fitted tyres were never placed on the wheels (3 of 3 fitted tyre model(s) still at the spawn position -1000 and size 60/30, session 12, 60 s in); the car has no visible tyres
";

    #[test]
    fn unplaced_tyres_name_the_cars() {
        let d = unplaced_tyres(&ev(TYRES_LOG, &[])).expect("the tyre lines are recognised");
        assert_eq!(d.id, "tyres_not_placed");
        assert!(
            d.what.contains("your car (car 24) and 1 other car(s)"),
            "{}",
            d.what
        );
        let (human, ai) = unplaced_cars(TYRES_LOG);
        assert_eq!(human, BTreeSet::from([24]));
        assert_eq!(ai, BTreeSet::from([23]));
    }

    #[test]
    fn unplaced_ai_cars_are_listed() {
        let log = "2026-11-03 10:00:10.000 [1] [tyres] car 25 (team 9, AI): fitted tyres were never placed on the wheels (3 of 3 x)\n";
        let d = unplaced_tyres(&ev(log, &[])).unwrap();
        assert!(d.what.contains("1 car(s) (cars 25)"), "{}", d.what);
    }

    #[test]
    fn a_healthy_tyre_check_is_not_a_problem() {
        let log = "2026-11-03 10:00:10.000 [1] [tyres] fitted tyre check (10 s in), session 12: all 26 car(s) have their fitted tyres placed\n";
        assert!(unplaced_tyres(&ev(log, &[])).is_none());
        assert!(unplaced_tyres(&ev("", &[])).is_none());
    }

    const GPU: &str = "\
2026-10-24 02:52:26.633 [33008] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 33008, 463 s after load, at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-10-24 02:52:26.633 [33008] [crash]   writing 0x1
2026-10-24 02:52:26.633 [33008] [crash]   rsi=00000001c3603440 rdi=00000000887a0005 rbp=00000000887a002b rsp=000000000015be50
2026-10-24 02:52:26.633 [33008] [crash]   stack:
2026-10-24 02:52:26.633 [33008] [crash]   #0  F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-10-24 02:52:26.633 [33008] [crash]   #1  F1_25.exe+0x39b8802 .xtls fn base+0x39b8510+0x2f2
2026-10-24 02:52:27.289 [20344] [crash] crash report swofty\\crash-20261024-025226.txt: the game aborted itself
2026-10-24 02:52:27.289 [33008] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4 reached the top-level filter; passing it to the game's filter
";

    fn crash_in(module: &str) -> String {
        format!(
            "2026-10-26 14:01:42.000 [1] [crash] #1 first-chance (the game may still handle it) BREAKPOINT (0x80000003) on thread 1, 21 s after load, at {module}+0x178fd
2026-10-26 14:01:42.000 [1] [crash]   stack:
2026-10-26 14:01:42.000 [1] [crash]   #0  {module}+0x178fd
2026-10-26 14:01:42.000 [1] [crash]   #1  F1_25.exe+0x1234 .xtls fn base+0x1200+0x34
2026-10-26 14:01:42.100 [1] [crash] unhandled BREAKPOINT (0x80000003) at {module}+0x178fd reached the top-level filter; passing it to the game's filter
"
        )
    }

    fn ev(loader: &str, files: &[&str]) -> Evidence {
        Evidence {
            loader: loader.to_owned(),
            game_files: files.iter().map(|f| (*f).to_owned()).collect(),
            ..Default::default()
        }
    }

    const INIT_ABORT: &str = "\
2026-11-01 15:30:27.529 [21084] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 21084, 5 s after load, at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-11-01 15:30:27.529 [21084] [crash]   writing 0x1
2026-11-01 15:30:27.529 [21084] [crash]   stack:
2026-11-01 15:30:27.529 [21084] [crash]   #0  F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-11-01 15:30:27.529 [21084] [crash]   #1  F1_25.exe+0x43c07f1 .xtls Game_Startup_InitSequence+0x291
2026-11-01 15:30:27.529 [21084] [crash]   #2  F1_25.exe+0x43e0af1 .xtls fn base+0x43e0ad0+0x21
2026-11-01 15:30:27.958 [21084] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4 reached the top-level filter; passing it to the game's filter
2026-11-01 15:30:29.407 [21084] [crash] TerminateProcess(4294967295) on thread 21084, 6 s after load, before the game's window; caller stack:
";

    fn init_abort(line: &str) -> Evidence {
        ev(
            &format!("2026-11-01 15:30:22.500 [21084] {line}\n{INIT_ABORT}"),
            &[],
        )
    }

    #[test]
    fn a_startup_abort_on_a_cloud_only_settings_file_is_named() {
        let d = diagnose(&init_abort(
            "[hwsettings] C:\\Users\\m\\OneDrive\\Dokumente\\My Games\\F1 25\\hardwaresettings\\hardware_settings_config.xml: onedrive=yes exists=yes size=5120 attributes=0x00400020 cloud-only=yes (recall_on_data_access) read test: skipped",
        ))
        .expect("a diagnosis");
        assert_eq!(d.id, "hardware_settings_unreadable");
        assert!(d.what.contains("in the cloud only"));
        assert_eq!(d.steps.len(), 3);
    }

    #[test]
    fn a_startup_abort_with_documents_in_onedrive_is_named_softly() {
        let d = diagnose(&init_abort(
            "[hwsettings] C:\\Users\\m\\OneDrive\\Documents\\x.xml: onedrive=yes exists=yes size=5120 attributes=0x00000020 cloud-only=no read test: ok (4096 bytes)",
        ))
        .expect("a diagnosis");
        assert_eq!(d.id, "hardware_settings_unreadable");
        assert!(d.what.contains("is in OneDrive"));
    }

    #[test]
    fn a_startup_abort_with_local_documents_is_not_this_rule() {
        let d = diagnose(&init_abort(
            "[hwsettings] C:\\Users\\v\\Documents\\x.xml: onedrive=no exists=yes size=5120 attributes=0x00000020 cloud-only=no read test: ok (4096 bytes)",
        ));
        assert!(d.is_none_or(|d| d.id != "hardware_settings_unreadable"));
        let silent = ev(INIT_ABORT, &[]);
        assert!(diagnose(&silent).is_none_or(|d| d.id != "hardware_settings_unreadable"));
    }

    #[test]
    fn another_abort_with_a_cloud_file_is_not_this_rule() {
        let other = init_abort("[hwsettings] p: onedrive=yes exists=yes size=1 attributes=0x00400020 cloud-only=yes (recall_on_data_access)")
            .loader
            .replace("InitSequence+0x291", "InitSequence+0x148");
        assert!(diagnose(&ev(&other, &[])).is_none_or(|d| d.id != "hardware_settings_unreadable"));
    }

    #[test]
    fn reads_the_fatal_exception_and_its_stack() {
        let f = fatal(&ev(GPU, &[]));
        assert_eq!(f.module.as_deref(), Some("f1_25.exe"));
        assert_eq!(f.stack, ["f1_25.exe", "f1_25.exe"]);
        assert_eq!(report_name(GPU).as_deref(), Some("crash-20261024-025226.txt"));
        let fallback = "2026-11-01 12:53:50.100 [5] [crash] crash report C:\\Users\\id\\AppData\\Local\\Swofty\\state\\crash-20261101-125350.txt: boom\n";
        assert_eq!(
            report_name(fallback).as_deref(),
            Some("crash-20261101-125350.txt")
        );
    }

    #[test]
    fn a_removed_gpu_is_the_driver() {
        let d = diagnose(&ev(GPU, &["F1_25.exe", "sl.common.dll"])).unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert!(d.what.contains("not from Swofty's code"), "{}", d.what);
        assert!(d.steps[0].starts_with("Update your graphics driver"));
    }

    #[test]
    fn a_removed_gpu_with_graphics_mods_names_them_first() {
        let d = diagnose(&ev(
            GPU,
            &["F1_25.exe", "dxgi.dll", "ReShade.ini", "dlss5-feed.addon64"],
        ))
        .unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert!(d.what.contains("ReShade"), "{}", d.what);
        assert!(
            d.steps[0].contains("dxgi.dll, ReShade.ini and dlss5-feed.addon64"),
            "{}",
            d.steps[0]
        );
    }

    /// A report's session: the overlay drew on the main menu only, the game aborted in its own
    /// resource creation half a minute later, and Present had returned 0.
    const GPU_LONG_AFTER_OVERLAY: &str = "\
2026-11-14 10:35:11.303 [12808] [overlay] at start the game's hardware settings (C:\\Users\\id\\Documents\\My Games\\F1 25\\hardwaresettings\\hardware_settings_config.xml) have frame generation on (frame_gen mode 1, multi_frame_gen 3); drawing before frame generation is on
2026-11-14 10:35:23.897 [12808] [overlay] api: alexs.silly-season on_paint -> ok (token 1)
2026-11-14 10:35:23.917 [12808] [overlay] api: kizi.ai-tweaks on_paint -> ok (token 2)
2026-11-14 10:35:38.567 [12808] [overlay] the game's hardware settings (C:\\Users\\id\\Documents\\My Games\\F1 25\\hardwaresettings\\hardware_settings_config.xml) have frame generation on (frame_gen mode 1, multi_frame_gen 3)
2026-11-14 10:37:02.225 [17276] [overlay] first overlay frame submitted: 2820 vertices
2026-11-14 10:37:38.712 [12808] [overlay] last 60 s: 281 frame(s) drawn, 0 skipped while the GPU still held the slot; GPU 0.031 ms per drawn frame (278 timed)
2026-11-14 10:38:11.319 [17276] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 17276, 183 s after load, at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-11-14 10:38:11.319 [17276] [crash]   writing 0x1
2026-11-14 10:38:11.319 [17276] [crash]   rsi=00000001c3603440 rdi=00000000887a0005 rbp=00000000887a0001 rsp=000000007d08f470
2026-11-14 10:38:11.320 [17276] [crash]   stack:
2026-11-14 10:38:11.320 [17276] [crash]   #0  F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-11-14 10:38:11.320 [17276] [crash]   #1  F1_25.exe+0x39b8802 .xtls Render_Check_GfxResult+0x2f2
2026-11-14 10:38:11.320 [17276] [crash]   #2  F1_25.exe+0x39b84b5 .xtls Render_Gfx2_PlacedResourceForward_C+0x25
2026-11-14 10:38:11.320 [17276] [crash]   #3  F1_25.exe+0x39b9c46 .xtls Render_Gfx2Heap_BumpAllocPlacedResource+0x1b6
2026-11-14 10:38:11.320 [17276] [overlay] the GPU was removed (present 0x00000000, reason 0x887a0001) more than 10 s after the overlay last drew; the overlay stops drawing
2026-11-14 10:38:13.897 [17276] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4 reached the top-level filter; passing it to the game's filter
";

    /// A report's session: the device went 0.1 s after an overlay frame at a session's start, found
    /// by the game's own resource creation and not by a Present.
    const GPU_JUST_AFTER_OVERLAY: &str = "\
2026-11-14 11:10:28.332 [ 4324] [overlay] api: kizi.ai-tweaks on_paint -> ok (token 1)
2026-11-14 11:10:28.337 [ 4324] [overlay] api: dromodf.skip-cutscenes on_paint -> ok (token 2)
2026-11-14 11:10:28.372 [ 4324] [overlay] api: alexs.silly-season on_paint -> ok (token 3)
2026-11-14 11:10:35.253 [ 4324] [overlay] drawing on swapchain 000000007A32A040 with the direct queue 000000007BA5D780 from CreateSwapChainForHwnd (2560x1440, format 28, 3 buffer(s), created by sl.interposer.dll <- amd_fidelityfx_dx12.dll <- F1_25.exe <- KERNEL32.DLL <- ntdll.dll)
2026-11-14 11:23:28.091 [ 4324] [overlay] first overlay frame submitted: 3330 vertices
2026-11-14 11:24:34.769 [ 4324] [overlay] last 60 s: 227 frame(s) drawn, 0 skipped while the GPU still held the slot; GPU 0.038 ms per drawn frame (227 timed)
2026-11-14 11:25:46.381 [ 4324] [overlay] the GPU was removed (present 0x00000000, reason 0x887a0006) 0.1 s after the overlay last drew (swapchain by sl.interposer.dll <- amd_fidelityfx_dx12.dll <- F1_25.exe <- KERNEL32.DLL <- ntdll.dll); the overlay stops drawing
2026-11-14 11:25:46.384 [ 4324] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 4324, 949 s after load, at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
2026-11-14 11:25:46.384 [ 4324] [crash]   rsi=00000001c3603440 rdi=00000000887a0005 rbp=00000000887a0006 rsp=000000000015bdf0
2026-11-14 11:25:46.384 [ 4324] [crash]   stack:
2026-11-14 11:25:46.384 [ 4324] [crash]   #0  F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4
";

    fn overlay_addons(log: &str) -> Evidence {
        with_names(
            ev(log, &["F1_25.exe"]),
            &[
                ("alexs.silly-season", "Silly Season"),
                ("kizi.ai-tweaks", "Contact Physics"),
                ("dromodf.skip-cutscenes", "Skip Cutscenes"),
            ],
        )
    }

    #[test]
    fn a_gpu_removed_long_after_the_overlay_drew_is_not_the_overlay() {
        let d = diagnose(&overlay_addons(GPU_LONG_AFTER_OVERLAY)).unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert_eq!(d.title, "Your graphics driver stopped responding");
        assert!(d.what.contains("Frame generation was on"), "{}", d.what);
        assert_eq!(d.steps[0], FRAME_GEN_OFF);
        let all = format!("{} {} {}", d.title, d.what, d.steps.join(" "));
        for word in ["overlay", "Silly Season", "Contact Physics"] {
            assert!(!all.contains(word), "{word}: {all}");
        }
    }

    #[test]
    fn a_gpu_removed_before_the_overlay_drew_is_not_the_overlay() {
        let log = GPU_LONG_AFTER_OVERLAY.replace(
            "more than 10 s after the overlay last drew",
            "and the overlay had not drawn",
        );
        assert!(log.contains("and the overlay had not drawn"));
        assert_eq!(diagnose(&overlay_addons(&log)).unwrap().id, "gpu_removed");
        let unknown =
            GPU_LONG_AFTER_OVERLAY.replace("more than 10 s after the overlay last drew", "at some point");
        assert_eq!(diagnose(&overlay_addons(&unknown)).unwrap().id, "gpu_removed");
    }

    #[test]
    fn a_gpu_the_game_found_removed_just_after_an_overlay_frame_is_the_driver() {
        let d = diagnose(&overlay_addons(GPU_JUST_AFTER_OVERLAY)).unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert_eq!(d.title, "Your graphics driver stopped responding");
        assert!(!d.what.contains("overlay"), "{}", d.what);
        assert!(
            d.steps[0].starts_with("Update your graphics driver"),
            "{:?}",
            d.steps
        );
        let named: Vec<&String> = d.steps.iter().filter(|s| s.contains("Silly Season")).collect();
        assert_eq!(
            named,
            [
                "If it keeps happening, turn off Contact Physics, Skip Cutscenes and Silly Season in your profile \
                 for one session."
            ]
        );
        assert_eq!(d.steps.last(), Some(named[0]));
    }

    #[test]
    fn a_present_that_returned_the_removal_just_after_an_overlay_frame_names_the_addons() {
        let log = GPU_JUST_AFTER_OVERLAY.replace("present 0x00000000", "present 0x887a0005");
        let d = diagnose(&overlay_addons(&log)).unwrap();
        assert_eq!(d.id, "gpu_removed_overlay");
        assert_eq!(
            d.steps[0],
            "Turn off Contact Physics, Skip Cutscenes and Silly Season in your profile, then play again."
        );
    }

    #[test]
    fn a_removed_gpu_after_the_overlay_drew_names_the_addons() {
        let log = format!(
            "{GPU}2026-10-24 02:52:26.000 [1] [overlay] api: kizi.pitlane on_paint -> ok\n\
             2026-10-24 02:52:26.000 [1] [overlay] renderer up: d3d12 swapchain 1920x1080\n\
             2026-10-24 02:52:26.500 [1] [overlay] the GPU was removed (present 0x887a0005, reason 0x887a0006) 4.8 s after the overlay last drew; the overlay stops drawing\n"
        );
        let mut e = ev(&log, &[]);
        e.names.insert("kizi.pitlane".into(), "Manual Pit Lane".into());
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "gpu_removed_overlay");
        assert_eq!(
            d.steps[0],
            "Turn off Manual Pit Lane in your profile, then play again."
        );
    }

    #[test]
    fn a_crash_inside_a_hook_names_its_program() {
        let d = diagnose(&ev(&crash_in("windhawk.dll"), &[])).unwrap();
        assert_eq!(d.id, "third_party_hook");
        assert_eq!(d.title, "F1 25 crashed inside Windhawk");
        let d = diagnose(&ev(&crash_in("RTSSHooks64.dll"), &[])).unwrap();
        assert!(d.title.contains("RivaTuner"), "{}", d.title);
        let d = diagnose(&ev(&crash_in("pid.dll"), &[])).unwrap();
        assert!(d.title.contains("force feedback"), "{}", d.title);
    }

    #[test]
    fn the_steam_overlay_counts_only_when_the_crash_is_in_it() {
        let log = crash_in("F1_25.exe").replace(
            "#1  F1_25.exe+0x1234 .xtls fn base+0x1200+0x34",
            "#1  GameOverlayRenderer64.dll+0x1234",
        );
        assert_eq!(diagnose(&ev(&log, &[])), None);
        let d = diagnose(&ev(&crash_in("GameOverlayRenderer64.dll"), &[])).unwrap();
        assert!(d.title.contains("Steam overlay"));
    }

    #[test]
    fn graphics_mods_before_the_window_are_named() {
        let d = diagnose(&ev(
            &crash_in("sl.common.dll"),
            &[
                "F1_25.exe",
                "OptiScaler.ini",
                "nvngx.dll",
                "dxgi.dll",
                "OptiPatcher.asi",
            ],
        ))
        .unwrap();
        assert_eq!(d.id, "graphics_mods");
        assert!(d.title.starts_with("OptiScaler and an ASI loader"), "{}", d.title);
        assert!(d.what.contains("before its window opened"), "{}", d.what);
        assert!(d.steps[0].contains("OptiScaler.ini, nvngx.dll, dxgi.dll and OptiPatcher.asi"));
    }

    #[test]
    fn graphics_mods_play_no_part_in_a_game_code_crash_after_the_window() {
        let log = format!(
            "2026-10-26 14:00:00.000 [1] trigger: main window visible\n{}",
            crash_in("F1_25.exe")
        );
        assert_eq!(
            diagnose(&ev(&log, &["F1_25.exe", "dxgi.dll", "ReShade.ini"])),
            None
        );
    }

    const REPLAY_LOADER: &str = "\
2026-11-12 17:57:27.502 [ 5736] stage: window (USER32!RegisterClassExW from 00000001439f00a0 (.xtls)), addons were loaded at the early stage
2026-11-12 18:23:14.352 [12208] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 12208, 1558 s after load, at F1_25.exe+0x1fbd2c .xtls fn base+0x1fbd00+0x2c
2026-11-12 18:23:14.352 [12208] [crash]   reading 0xffffffffffffffff
2026-11-12 18:23:14.352 [12208] [crash]   rax=40000003c3bc3180 rbx=0000000000000000 rcx=00000003c3bc31e8 rdx=00000000693af410
2026-11-12 18:23:14.353 [12208] [crash]   stack:
2026-11-12 18:23:14.353 [12208] [crash]   #0  F1_25.exe+0x1fbd2c .xtls fn base+0x1fbd00+0x2c
2026-11-12 18:23:14.353 [12208] [crash]   #1  F1_25.exe+0x3b8250 .xtls Game_Garage_ClearOverlayAllModels+0xd0
2026-11-12 18:23:14.353 [12208] [crash]   #2  F1_25.exe+0x3b80ac .xtls Car_PitStop_ApplyTyreAndWingVisuals+0x8ec
2026-11-12 18:23:14.353 [12208] [crash]   #3  F1_25.exe+0x3b7579 .xtls Car_Tyres_ApplyStateToCarPair+0x69
2026-11-12 18:23:14.353 [12208] [crash]   #4  F1_25.exe+0x7e3c9b4 .data1 fn base+0x7e3c870+0x144
2026-11-12 18:23:14.353 [12208] [crash]   #5  F1_25.exe+0x45ceeee .xtls Ne_ReplayRecordable_ReadFrom+0x151e
2026-11-12 18:23:14.353 [12208] [crash]   #6  F1_25.exe+0x45d3aee .xtls Game_ReplayStream_InterpolateBackwards+0x4ae
2026-11-12 18:23:14.353 [12208] [crash]   #7  F1_25.exe+0x45d71e4 .xtls Game_ReplaySystem_TargetTime+0x504
2026-11-12 18:23:14.353 [12208] [crash]   #8  F1_25.exe+0x45d6c56 .xtls Game_ReplaySystem_UpdateFromModule+0xc16
2026-11-12 18:23:14.353 [12208] [crash]   #9  F1_25.exe+0x2a37f0 .xtls fn base+0x2a37e0+0x10
2026-11-12 18:23:14.353 [12208] [crash]   #10 F1_25.exe+0x3ac0c33 .xtls Ne_JobQueue_TryAcquireJob+0x483
2026-11-12 18:23:14.353 [12208] [crash]   #11 F1_25.exe+0x3ac4fb3 .xtls fn base+0x3ac4a50+0x563
2026-11-12 18:23:14.353 [12208] [crash]   #12 F1_25.exe+0x391e16a .xtls fn base+0x391def0+0x27a
2026-11-12 18:23:14.353 [12208] [crash]   #13 KERNEL32.DLL+0x17374
2026-11-12 18:23:14.353 [12208] [crash]   #14 ntdll.dll+0x4cc91
2026-11-12 18:23:14.353 [12208] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x1fbd2c .xtls fn base+0x1fbd00+0x2c reached the top-level filter; passing it to the game's filter
";

    const REPLAY_REPORT: &str = "\
Swofty crash report

reason       unhandled ACCESS_VIOLATION (0xc0000005) reading 0xffffffffffffffff at F1_25.exe+0x1fbd2c .xtls fn base+0x1fbd00+0x2c
runtime      Windows

== Exception
  ACCESS_VIOLATION (0xc0000005), flags 0x0, at F1_25.exe+0x1fbd2c .xtls fn base+0x1fbd00+0x2c

== Stack (unwound with the modules' exception data)
  #0  00000001401fbd2c rsp=00000000693af350  F1_25.exe+0x1fbd2c .xtls fn base+0x1fbd00+0x2c
  #1  00000001403b8250 rsp=00000000693af3e0  F1_25.exe+0x3b8250 .xtls Game_Garage_ClearOverlayAllModels+0xd0
  #2  00000001403b80ac rsp=00000000693af460  F1_25.exe+0x3b80ac .xtls Car_PitStop_ApplyTyreAndWingVisuals+0x8ec
  #3  00000001403b7579 rsp=00000000693af5f0  F1_25.exe+0x3b7579 .xtls Car_Tyres_ApplyStateToCarPair+0x69
  #4  0000000147e3c9b4 rsp=00000000693af640  F1_25.exe+0x7e3c9b4 .data1 fn base+0x7e3c870+0x144
  #5  00000001445ceeee rsp=00000000693af700  F1_25.exe+0x45ceeee .xtls Ne_ReplayRecordable_ReadFrom+0x151e
  #6  00000001445d3aee rsp=00000000693af870  F1_25.exe+0x45d3aee .xtls Game_ReplayStream_InterpolateBackwards+0x4ae
  #7  00000001445d71e4 rsp=00000000693af950  F1_25.exe+0x45d71e4 .xtls Game_ReplaySystem_TargetTime+0x504
  #8  00000001445d6c56 rsp=00000000693afa80  F1_25.exe+0x45d6c56 .xtls Game_ReplaySystem_UpdateFromModule+0xc16

== Return addresses found on the stack (rsp to +0x8000)
  rsp+0x88    00000001403b8250  F1_25.exe+0x3b8250 .xtls Game_Garage_ClearOverlayAllModels+0xd0
  rsp+0x3a8   00000001445ceeee  F1_25.exe+0x45ceeee .xtls Ne_ReplayRecordable_ReadFrom+0x151e
  rsp+0x3b8   00007ff8d501b6ae  KERNELBASE.dll+0x1b6ae .text fn +0x1b620+0x8e
  rsp+0x518   00000001445d3aee  F1_25.exe+0x45d3aee .xtls Game_ReplayStream_InterpolateBackwards+0x4ae
  rsp+0x528   00007ff87ef46586  d3d12.dll+0x6586 .text fn +0x64f0+0x96
  rsp+0x5f8   00000001445d71e4  F1_25.exe+0x45d71e4 .xtls Game_ReplaySystem_TargetTime+0x504
";

    #[test]
    fn a_graphics_module_only_in_the_stack_scan_does_not_blame_graphics_mods() {
        let mut e = ev(REPLAY_LOADER, &["F1_25.exe", "DINPUT8.dll"]);
        e.report = Some(REPLAY_REPORT.to_owned());
        assert!(diagnose(&e).is_none_or(|d| d.id != "graphics_mods"));
        let mut through_d3d12 = ev(
            &REPLAY_LOADER.replace(
                "#3  F1_25.exe+0x3b7579 .xtls Car_Tyres_ApplyStateToCarPair+0x69",
                "#3  d3d12.dll+0x6586",
            ),
            &["F1_25.exe", "DINPUT8.dll"],
        );
        through_d3d12.report = Some(REPLAY_REPORT.to_owned());
        let d = diagnose(&through_d3d12).expect("d3d12.dll is among the near frames");
        assert_eq!(d.id, "graphics_mods");
        assert_eq!(d.title, "an ASI loader in your game folder crashed F1 25");
    }

    #[test]
    fn a_stale_driver_return_does_not_blame_optiscaler_for_a_core_crash() {
        let loader = "\
2026-11-12 10:26:30.000 [11396] stage: window (USER32!RegisterClassExW from 00000001439f00a0 (.xtls)), addons were loaded at the early stage
2026-11-12 10:27:57.733 [11396] [crash] unhandled exception (0xc0000028) at ntdll.dll+0x1d3df reached the top-level filter; passing it to the game's filter
";
        let report = "\
== Exception
  exception (0xc0000028), flags 0x81, at ntdll.dll+0x1d3df .text fn +0x1d390+0x4f

== Stack (unwound with the modules' exception data)
  #0  00007ffa1a2ed3df rsp=0000000067ffd1f0  ntdll.dll+0x1d3df .text fn +0x1d390+0x4f
  #1  00007ffa1a2edaee rsp=0000000067ffd790  ntdll.dll+0x1daee .text fn +0x1da30+0xbe
  #2  00007ffa17731da2 rsp=0000000067ffdec0  ucrtbase.dll+0xb1da2 .text fn +0xb1c80+0x122
  #3  00007ffa1a43ab3f rsp=0000000067ffdf30  ntdll.dll+0x16ab3f .text fn +0x16ab30+0xf
  #4  00007ffa1a396985 rsp=0000000067ffdf60  ntdll.dll+0xc6985 .text fn +0xc6390+0x5f5
  #5  00007ffa1a43a47e rsp=0000000067ffe6c0  ntdll.dll+0x16a47e .text fn +0x16a450+0x2e
  #6  00007ff92fecfda9 rsp=0000000067ffee78  swofty_core.dll+0x64fda9 .text
  #7  3f80000000000007 rsp=0000000067ffee80  0x3f80000000000007 (no module)

== Return addresses found on the stack (rsp to +0x8000)
  rsp+0x8     00007ff9fed046d3  nvwgf2umx.dll+0x4846d3 .text fn +0x483ce0+0x9f3
  rsp+0x30    00007ffa1a2ed3df  ntdll.dll+0x1d3df .text fn +0x1d390+0x4f
  rsp+0x908   00007ff9a057f54b  D3D12Core.dll+0x3f54b .text fn +0x3f1f0+0x35b
";
        let mut e = ev(loader, &["F1_25.exe", "OptiScaler.ini", "nvngx.dll"]);
        e.report = Some(report.to_owned());
        assert!(diagnose(&e).is_none_or(|d| d.id != "graphics_mods"));
    }

    #[test]
    fn a_crash_in_a_graphics_driver_names_the_vendor() {
        let d = diagnose(&ev(&crash_in("amdxc64.dll"), &["F1_25.exe"])).unwrap();
        assert_eq!(d.id, "graphics_driver");
        assert_eq!(d.title, "F1 25 crashed inside your AMD graphics driver");
        let d = diagnose(&ev(
            &crash_in("amdxc64.dll"),
            &["F1_25.exe", "amdxc64.dll", "OptiScaler.ini"],
        ))
        .unwrap();
        assert_eq!(d.id, "graphics_mods", "OptiScaler's own amdxc64.dll");
    }

    #[test]
    fn a_crash_in_frame_generation_says_turn_it_off() {
        let log = format!(
            "2026-10-26 14:00:00.000 [1] trigger: main window visible\n{}",
            crash_in("libxess_fg.dll")
        );
        let d = diagnose(&ev(&log, &["F1_25.exe"])).unwrap();
        assert_eq!(d.id, "frame_generation");
        assert_eq!(
            d.steps[0],
            "Turn off XeSS frame generation in F1 25's graphics settings, then play again."
        );
    }

    #[test]
    fn running_out_of_memory_says_so() {
        let d = diagnose(&ev(&GPU.replace("887a0005", "8007000e"), &[])).unwrap();
        assert_eq!(d.id, "out_of_memory");
    }

    #[test]
    fn under_wine_the_windows_driver_rules_stay_out() {
        let log = format!(
            "{}2026-10-21 07:05:50.000 [1] [offline] module: winex11.drv @ 0x6ffffde80000\n",
            GPU.replace("887a0005", "8007000e")
        );
        assert_eq!(diagnose(&ev(&log, &[])), None);
    }

    #[test]
    fn a_plain_game_crash_has_no_diagnosis() {
        assert_eq!(
            diagnose(&ev(&crash_in("F1_25.exe"), &["F1_25.exe", "sl.common.dll"])),
            None
        );
        assert_eq!(diagnose(&ev("", &[])), None);
    }

    const WATCHDOG: &str = "\
2026-10-27 15:00:08.518 [3376] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 3376, 1083 s after load, at F1_25.exe+0x43e1113 .xtls fn base+0x43e10d0+0x43
2026-10-27 15:00:08.518 [3376] [crash]   writing 0x1
2026-10-27 15:00:08.518 [3376] [crash]   rax=0000000000000001 rbx=0000000000222200 rcx=00000000000077ae rdx=0000000000002065
2026-10-27 15:00:08.518 [3376] [crash]   r12=000000014604b4f0 r13=000000000351fe88 r14=00000000000077ae r15=0000000000007530
2026-10-27 15:00:08.519 [3376] [crash]   code: C7 00 BE CC 4B 28 48 83 C4 38 C3 CC CC 50 B8 01
2026-10-27 15:00:08.519 [3376] [crash]   stack:
2026-10-27 15:00:08.519 [3376] [crash]   #0  F1_25.exe+0x43e1113 .xtls fn base+0x43e10d0+0x43
2026-10-27 15:00:08.519 [3376] [crash]   #1  KERNEL32.DLL+0x2dda7
2026-10-27 15:00:08.520 [3376] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x43e1113 .xtls fn base+0x43e10d0+0x43 reached the top-level filter; passing it to the game's filter
";

    fn with_names(mut e: Evidence, names: &[(&str, &str)]) -> Evidence {
        e.names = names
            .iter()
            .map(|(id, n)| ((*id).to_owned(), (*n).to_owned()))
            .collect();
        e.enabled = e.names.keys().cloned().collect();
        e
    }

    #[test]
    fn the_hang_watchdog_is_a_freeze() {
        let d = diagnose(&ev(WATCHDOG, &["F1_25.exe"])).unwrap();
        assert_eq!(d.id, "hang_watchdog");
        assert_eq!(d.title, "F1 25 froze and closed itself");
        assert!(d.what.contains("stopped responding for 30 seconds"), "{}", d.what);
        assert!(d.steps[0].starts_with("Press Play again"), "{:?}", d.steps);
        let other = WATCHDOG.replace("r15=0000000000007530", "r15=0000000000000000");
        assert_eq!(diagnose(&ev(&other, &["F1_25.exe"])), None);
    }

    fn busy_log(tag: &str) -> String {
        let mut log = String::new();
        for s in 0..8 {
            log.push_str(&format!(
                "2026-10-27 14:59:{:02}.807 [21888] [{tag}] working on item {s}\n",
                45 + s
            ));
        }
        log.push_str(WATCHDOG);
        log
    }

    #[test]
    fn a_freeze_inside_the_core_offers_no_turn_off_step() {
        let mut e = with_names(
            ev(&busy_log("core"), &["F1_25.exe"]),
            &[("swofty.core", "Swofty Core")],
        );
        e.builtin = BTreeSet::from(["swofty.core".to_owned()]);
        let d = diagnose(&e).unwrap();
        assert!(!d.text().contains("Turn off"), "{}", d.text());
        assert!(d.steps.iter().any(|s| s.contains("Send Logs")), "{:?}", d.steps);
    }

    #[test]
    fn a_non_builtin_suspect_keeps_its_turn_off_step() {
        let e = with_names(
            ev(&busy_log("helmet-exchange"), &["F1_25.exe"]),
            &[
                ("giggles.helmet-exchange", "Helmet Exchange"),
                ("swofty.core", "Swofty Core"),
            ],
        );
        let mut e = e;
        e.builtin = BTreeSet::from(["swofty.core".to_owned()]);
        let d = diagnose(&e).unwrap();
        assert_eq!(
            d.steps[0],
            "Turn off Helmet Exchange in your profile, then play again."
        );
    }

    #[test]
    fn the_hang_watchdog_names_the_addon_that_kept_the_game_busy() {
        let mut log = String::new();
        for s in 0..8 {
            log.push_str(&format!(
                "2026-10-27 14:59:{:02}.807 [21888] [helmet-exchange] thumbnail requested for helmet {s}\n",
                45 + s
            ));
        }
        log.push_str("2026-10-27 14:59:58.000 [5672] [progressive] standings read\n");
        log.push_str(WATCHDOG);
        let e = with_names(
            ev(&log, &["F1_25.exe"]),
            &[
                ("giggles.helmet-exchange", "Helmet Exchange"),
                ("swofty.progressive", "Progressive Career"),
            ],
        );
        let d = diagnose(&e).unwrap();
        assert!(
            d.what.contains("Helmet Exchange kept the game busy"),
            "{}",
            d.what
        );
        assert_eq!(
            d.steps[0],
            "Turn off Helmet Exchange in your profile, then play again."
        );

        let gap = format!(
            "2026-10-27 14:59:37.896 [14804] [storage] api: swofty.progressive (progressive_cards.dll) read data/years.json -> ok\n\
             2026-10-27 14:59:53.095 [28800] [game] WARN (warn): The signal timed out\n\
             2026-10-27 15:00:06.111 [14804] [progressive] gamedb labels loaded\n{WATCHDOG}"
        );
        let e = with_names(ev(&gap, &[]), &[("swofty.progressive", "Progressive Career")]);
        assert!(
            diagnose(&e)
                .unwrap()
                .what
                .contains("Progressive Career kept the game busy")
        );

        let idle = gap.replace("14:59:37.896", "14:58:20.000");
        let e = with_names(ev(&idle, &[]), &[("swofty.progressive", "Progressive Career")]);
        assert!(!diagnose(&e).unwrap().what.contains("kept the game busy"));
    }

    #[test]
    fn the_hang_watchdog_names_a_driver_error_and_graphics_mods() {
        let log = format!(
            "2026-10-27 14:59:36.800 [9000] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 9000, 1050 s after load, at nvwgf2umx.dll+0x93d4c2\n\
             2026-10-27 14:59:36.800 [9000] [crash]   stack:\n\
             2026-10-27 14:59:36.800 [9000] [crash]   #0  nvwgf2umx.dll+0x93d4c2\n\
             2026-10-27 14:59:36.800 [9000] [crash]   #1  sl.dlss_g.dll+0x1234\n{}",
            WATCHDOG.replace("#1 first-chance", "#2 first-chance")
        );
        let d = diagnose(&ev(&log, &["F1_25.exe"])).unwrap();
        assert_eq!(d.id, "hang_watchdog");
        assert!(
            d.what
                .contains("nvwgf2umx.dll, part of the NVIDIA graphics driver during DLSS Frame Generation"),
            "{}",
            d.what
        );
        assert_eq!(d.steps[0], FRAME_GEN_OFF);
        let d = diagnose(&ev(WATCHDOG, &["F1_25.exe", "dxgi.dll", "OptiScaler.ini"])).unwrap();
        assert!(d.what.contains("OptiScaler"), "{}", d.what);
        assert!(
            d.steps[0].contains("dxgi.dll and OptiScaler.ini"),
            "{}",
            d.steps[0]
        );
    }

    fn perf(rows: &[&str], uptime: u32) -> String {
        format!(
            "Swofty Launcher performance report (format 1)\n\
             == Summary\n\
             uptime: {uptime} s\n\
             \n\
             == Timeline (last 15; each row runs until the next one)\n\
             at          took ms   frames   slow   worst ms  swofty ms  what\n{}\n",
            rows.join("\n")
        )
    }

    const SPLASH: &[&str] = &[
        "00:00:37      44811     2157      4        624      33.71  screen splash  [core flow tick 20.39 ms]",
        "00:01:27       1772      105      0         18      57.27  flow #a4d3a4b2 Exiting; flow #04fc8ffa Entering  [core flow tick 50.48 ms]",
        "00:01:29     232600    13654      7       1866     331.45  screen splash_transition  [core flow tick 306.18 ms]",
        "00:05:21      86418     5061      3       1219          -  flow #04fc8ffa Exiting (current)",
    ];

    const LIVERY: (&str, &str) = ("crew8006.bp-ultimate-livery", "BP Ultimate Livery");

    fn stuck(rows: &[&str], uptime: u32, names: &[(&str, &str)]) -> Evidence {
        with_names(
            Evidence {
                perf: Some(perf(rows, uptime)),
                ..Default::default()
            },
            names,
        )
    }

    #[test]
    fn reads_the_last_screen_from_the_perf_timeline() {
        assert_eq!(perf_start_screen(&perf(SPLASH, 408)), Some(319.0));
        let menu = [
            SPLASH[2],
            "00:01:45      27019     2404     15        271     189.46  flow #04fc8ffa Exiting; screen playercard  [x]",
            "00:02:12        653       31      0         50      11.96  screen fe_home (current)",
        ];
        assert_eq!(perf_start_screen(&perf(&menu, 408)), None);
        assert_eq!(
            timeline_row(
                "00:04:44    2585578   469557      7       1420          -  screen splash_transition (current)"
            ),
            Some((284.0, "screen splash_transition"))
        );
        assert_eq!(
            timeline_row("at          took ms   frames   slow   worst ms  swofty ms  what"),
            None
        );
    }

    #[test]
    fn a_stall_right_after_the_profile_load_points_at_the_save() {
        let load = "2026-11-04 03:32:34.875 [ 5256] [saves] profile load 1: manager 5 slot 2: gm_my_team id=16 \
                    key=66f051ee751e04d1 career=my_team:66f051ee751e04d1 flags=0x20 name=\"My Team Save 3\"\n";
        let mut e = stuck(SPLASH, 408, &[]);
        e.loader = load.into();
        let d = stalled(&e).unwrap();
        assert_eq!(d.id, "stalled_save");
        assert_eq!(
            d.title,
            "F1 25 stayed on the loading screen after reading your save"
        );
        assert!(d.what.contains("for 5 minutes"), "{}", d.what);
        assert_eq!(d.steps.len(), 2);
        let mut quick = stuck(SPLASH, 120, &[]);
        quick.loader = load.into();
        assert_eq!(stalled(&quick), None, "30 s is a normal load");
        assert_eq!(
            stalled(&stuck(SPLASH, 408, &[])),
            None,
            "no profile load, no save to blame"
        );
        let mut addon = stuck(SPLASH, 408, &[LIVERY]);
        addon.loader = load.into();
        assert_eq!(
            stalled(&addon).unwrap().id,
            "boot_stall",
            "the known addon comes first"
        );
    }

    #[test]
    fn a_boot_stuck_on_loading_names_the_addon_known_to_cause_it() {
        let d = stalled(&stuck(
            SPLASH,
            408,
            &[LIVERY, ("giggles.helmet-exchange", "Helmet Exchange")],
        ))
        .unwrap();
        assert_eq!(d.id, "boot_stall");
        assert_eq!(d.title, "BP Ultimate Livery kept F1 25 on the loading screen");
        assert!(d.what.contains("for 5 minutes"), "{}", d.what);
        assert_eq!(
            d.steps,
            ["Turn off BP Ultimate Livery in your profile, then press Play again."]
        );
        assert_eq!(
            stalled(&stuck(
                SPLASH,
                408,
                &[("giggles.helmet-exchange", "Helmet Exchange")]
            )),
            None
        );
        assert_eq!(
            stalled(&stuck(SPLASH, 120, &[LIVERY])),
            None,
            "30 s is a normal load"
        );
        let mut off = stuck(SPLASH, 408, &[LIVERY]);
        off.enabled = vec!["giggles.helmet-exchange".into()];
        assert_eq!(stalled(&off), None, "installed but off");
        let menu = [
            SPLASH[2],
            "00:05:21        653       31      0         50      11.96  screen fe_home (current)",
        ];
        assert_eq!(stalled(&stuck(&menu, 408, &[LIVERY])), None);
    }

    #[test]
    fn the_hang_watchdog_on_the_loading_screen_names_the_addon_known_to_stall_it() {
        let report = "== Game state (from the black box)\n  last flow      #04fc8ffa Entering (2584.6 s before this report)\n  \
                      last screen    splash_transition (2581.5 s before this report)\n";
        let mut e = with_names(ev(WATCHDOG, &["F1_25.exe"]), &[LIVERY]);
        e.report = Some(report.to_owned());
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "hang_watchdog");
        assert!(
            d.what
                .contains("never got past Loading... after Press any button, which BP Ultimate Livery"),
            "{}",
            d.what
        );
        assert_eq!(
            d.steps[0],
            "Turn off BP Ultimate Livery in your profile, then play again."
        );
        e.report = Some(report.replace("splash_transition", "fe_home"));
        e.perf = Some(perf(SPLASH, 408));
        assert!(!diagnose(&e).unwrap().what.contains("BP Ultimate Livery"));
    }

    const GAME_HEAP: &str = "\
2026-10-27 14:21:24.880 [17796] [crash] Ne heap error 0xc (fatal) on thread 17796: Allocation error. Requested size:3528, Free Memory:829408, Largest Free Block:1392, HeapSize:204472320, Alignment: 8; heap 00000000636F4590 'GAME_DEFAULT', capacity 204472320
2026-10-27 14:21:24.880 [16100] [crash] writing a crash report: fatal Ne heap error 0xc in heap 'GAME_DEFAULT': Allocation error. Requested size:3528
2026-10-27 14:21:25.000 [17796] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4 reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_full_game_heap_while_saving_says_so() {
        let mut e = ev(GAME_HEAP, &[]);
        assert!(!diagnose(&e).unwrap().what.contains("saving"));
        e.report = Some(
            "== Stack\n  #9  00000001439340f9 F1_25.exe+0x39340f9 .xtls Game_SaveData_WriteBag+0x39\n"
                .to_owned(),
        );
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "game_heap");
        assert!(
            d.what.contains("while the game was saving your career"),
            "{}",
            d.what
        );
    }

    const REPLAY_STATE: &str = "\
2026-11-02 05:10:01.100 [4000] [replay] session has 24 cars, the recorder holds 22; CannedEventsSystem needs 1032 bytes of replay state and its holder has 1024, so it is not recorded (replay and flashback lose its state; the session keeps all its cars)
2026-11-02 05:21:24.880 [17796] [crash] Ne heap error 0xc (fatal) on thread 17796: Allocation error. Requested size:3747052065449325568, Free Memory:829408, Largest Free Block:1392, HeapSize:204472320, Alignment: 8; heap 00000000636F4590 'GAME_DEFAULT', capacity 204472320
";

    #[test]
    fn an_absurd_heap_request_after_a_dropped_replay_state_is_the_replay_crash() {
        let d = diagnose(&ev(REPLAY_STATE, &[])).unwrap();
        assert_eq!(d.id, "replay_state");
        assert!(d.steps[0].contains("Flashbacks off"), "{:?}", d.steps);
        assert!(d.what.contains("not a memory problem"), "{}", d.what);
        let without = REPLAY_STATE.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert_eq!(diagnose(&ev(&without, &[])).unwrap().id, "game_heap");
        let small = REPLAY_STATE.replace("3747052065449325568", "3528");
        assert_eq!(diagnose(&ev(&small, &[])).unwrap().id, "game_heap");
    }

    const QUIT_REPORT: &str = "== Game state (from the black box)\n  last flow      #e11edb87 Exiting (1.8 s before this report)\n  \
        last exception first-chance ACCESS_VIOLATION at F1_25.exe+0x537db28 .xtls fn base+0x537db18+0x10 (0.0 s before this report)\n";

    #[test]
    fn an_access_violation_in_the_exiting_flow_is_a_quit_fault() {
        let mut e = ev("", &[]);
        e.report = Some(QUIT_REPORT.to_owned());
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "quit_fault");
        assert!(!d.what.contains("crashed"), "{}", d.what);
        e.report = Some(QUIT_REPORT.replace("Exiting (1.8", "Exiting (400.0"));
        assert_eq!(diagnose(&e), None, "an exit long before the fault");
        e.report = Some(QUIT_REPORT.replace("Exiting", "Entering"));
        assert_eq!(diagnose(&e), None, "not exiting");
        e.report = Some(QUIT_REPORT.replace("F1_25.exe+", "other.dll+"));
        assert_eq!(diagnose(&e), None, "not in the game's code");
        e.report = Some(QUIT_REPORT.replace("#e11edb87", "StateApp"));
        assert_eq!(diagnose(&e).unwrap().id, "quit_fault", "the root state by name");
    }

    const SESSION_EXIT_REPORT: &str = "== Game state (from the black box)\n  \
        last flow      StateSession Exiting (1.2 s before this report)\n  \
        last screen    garage_monitor_container (0.1 s before this report)\n  \
        last session   started (Game_GameModule_OnSessionEnter); frame 19364 (1.9 s before this report)\n  \
        last race      none recorded\n  \
        last career    hub left (117.5 s before this report)\n  \
        last heap      none recorded\n  \
        last exception first-chance ACCESS_VIOLATION at F1_25.exe+0x1f8a25c .xtls \
        Game_Read_OverridesCarMonitorMaximisedConfig_2+0xe6c (0.0 s before this report)\n";

    const SESSION_EXIT_LOADER: &str = "\
2026-11-14 19:35:37.360 [ 9720] [career] flow \"StateSession\" Updating -> Exiting
2026-11-14 19:35:37.362 [ 9720] [events] session.end (StateSession exited)
2026-11-14 19:35:37.769 [ 5108] [events] career.saved (driver_career): 0 handlers
2026-11-14 19:35:38.524 [17320] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 17320, 419 s after load, at F1_25.exe+0x1f8a25c .xtls Game_Read_OverridesCarMonitorMaximisedConfig_2+0xe6c
2026-11-14 19:35:38.524 [17320] [crash]   reading 0x9c58
2026-11-14 19:35:38.525 [17320] [crash]   stack:
2026-11-14 19:35:38.525 [17320] [crash]   #0  F1_25.exe+0x1f8a25c .xtls Game_Read_OverridesCarMonitorMaximisedConfig_2+0xe6c
2026-11-14 19:35:38.525 [17320] [crash]   #1  F1_25.exe+0x42c02eb .xtls Vm_Dispatch_BindingMessage+0x20b
2026-11-14 19:35:38.525 [17320] [crash]   #2  F1_25.exe+0x398c32c .xtls Ne_EventQueue_DispatchPending+0xbc
2026-11-14 19:35:38.526 [17320] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x1f8a25c .xtls Game_Read_OverridesCarMonitorMaximisedConfig_2+0xe6c reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_session_that_exits_while_the_game_runs_is_not_a_quit_fault() {
        let mut e = ev(SESSION_EXIT_LOADER, &[]);
        e.report = Some(SESSION_EXIT_REPORT.to_owned());
        assert_eq!(diagnose(&e), None, "the session was exiting, not the game");
        for flow in ["StateUnloadSession", "SessionStateCareerHubUpdate", "#04fc8ffa"] {
            e.report = Some(SESSION_EXIT_REPORT.replace("StateSession", flow));
            assert_eq!(diagnose(&e), None, "{flow}");
        }
        e.report = Some(SESSION_EXIT_REPORT.replace("StateSession", "#e11edb87"));
        assert_eq!(diagnose(&e).unwrap().id, "quit_fault");
    }

    #[test]
    fn a_full_game_heap_names_sepang_first() {
        let mut e = with_names(
            ev(GAME_HEAP, &[]),
            &[
                ("mod.hockenheim", "Hockenheim"),
                ("mod.portimao", "Portimao"),
                ("mod.bahrain24", "Sepang"),
                ("swofty.classic-seasons", "Classic Seasons"),
                ("swofty.classic-seasons-2016", "Classic Seasons 2016"),
                ("swofty.classic-seasons-2017", "Classic Seasons 2017"),
            ],
        );
        e.tracks = vec!["mod.hockenheim".into(), "mod.bahrain24".into()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "game_heap");
        assert!(
            d.what
                .contains("your profile has Sepang, Hockenheim and 2 Classic Seasons years on"),
            "{}",
            d.what
        );
        assert!(
            d.steps[0].starts_with("Turn off Sepang in your profile"),
            "{}",
            d.steps[0]
        );
        assert!(
            d.steps[0].ends_with(
                "In a My Team career whose next round is at Sepang, change that round with the Career Mode Editor first: \
                 the career won't load while that track is off."
            ),
            "{}",
            d.steps[0]
        );
        e.tracks = vec!["mod.hockenheim".into(), "mod.portimao".into()];
        let d = diagnose(&e).unwrap();
        assert!(
            d.steps[0].contains("Turn off Hockenheim and Portimao in your profile")
                && d.steps[0].contains("next round is at one of them,"),
            "{}",
            d.steps[0]
        );
        let d = diagnose(&ev(GAME_HEAP, &[])).unwrap();
        assert_eq!(d.id, "game_heap");
        assert_eq!(d.steps.len(), 1);
    }

    #[test]
    fn a_full_game_heap_in_progressive_keeps_every_year_and_never_moves_a_round() {
        let mut e = with_names(
            ev(GAME_HEAP, &[]),
            &[
                ("mod.bahrain24", "Sepang"),
                ("swofty.progressive", "Progressive Career"),
                ("swofty.classic-seasons-2016", "Classic Seasons 2016"),
            ],
        );
        e.tracks = vec!["mod.bahrain24".into()];
        let d = diagnose(&e).unwrap();
        let all = d.steps.join(" ");
        assert!(
            !all.contains("Career Mode Editor") && !all.contains("you race"),
            "{all}"
        );
        assert!(all.contains("Keep every Classic Seasons year on"), "{all}");
        assert!(d.steps[0].starts_with("Turn off Sepang in your profile"), "{all}");
    }

    const COMMIT: &str = "\
2026-10-27 13:35:41.000 [15812] [crash] Ne heap error 0x58 (fatal) on thread 15812: FATAL ERROR : allocateAligned / useAddressSpace(block = 0xcdfe3a80, allocSize = 2088976) failed.Out of physical memory or Virtual memory problem; heap 0000000064243050 'RENDERER_INTERNAL'
2026-10-27 13:35:41.100 [15812] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4 reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn windows_running_out_of_memory_is_not_the_graphics_card() {
        let mut e = with_names(
            ev(COMMIT, &[]),
            &[("swofty.classic-seasons-2019", "Classic Seasons 2019")],
        );
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "windows_out_of_memory");
        assert!(d.what.contains("not the graphics card's"), "{}", d.what);
        assert!(d.steps[0].contains("page file"), "{}", d.steps[0]);
        assert!(d.steps[2].contains("1 Classic Seasons year on"), "{}", d.steps[2]);
        let warned = COMMIT
            .replace("0x58 (fatal)", "0x58 (not fatal)")
            .replace("Ne_FatalAbort", "Other");
        e.loader = warned;
        assert_eq!(diagnose(&e), None);
        e.loader =
            format!("{COMMIT}2026-10-21 07:05:50.000 [1] [offline] module: winex11.drv @ 0x6ffffde80000\n");
        assert_eq!(diagnose(&e), None);
    }

    #[test]
    fn exit_code_0xc0e90002_is_app_control() {
        let mut e = ev("", &[]);
        e.code = Some(APP_CONTROL);
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "app_control");
        assert!(
            d.steps[0].contains("Smart App Control settings, and choose Off"),
            "{}",
            d.steps[0]
        );
    }

    const RTSS: &str = "\
2026-10-27 17:37:40.100 [1] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 1, 3 s after load, at ntdll.dll+0x4a612
2026-10-27 17:37:40.100 [1] [crash]   reading 0xffffffffffffffff
2026-10-27 17:37:40.100 [1] [crash]   stack:
2026-10-27 17:37:40.100 [1] [crash]   #0  ntdll.dll+0x4a612
2026-10-27 17:37:40.100 [1] [crash]   #1  ntdll.dll+0x1d3b2
2026-10-27 17:37:40.100 [1] [crash]   #2  RTSSHooks64.dll+0x2a1b0
2026-10-27 17:37:40.100 [1] [crash]   #3  RTSSHooks64.dll+0x2a0f0
";

    const RTSS_STRAY_JUMP: &str = "\
[offline] module: RTSSHooks64.dll @ 0x180000000
[offline] module: dxgi.dll @ 0x7fffa3b20000
2026-11-15 07:12:24.289 [ 1212] [d3d12] swapchain 000000006B9A5530 created through CreateSwapChainForHwnd on queue 00000000812DD3C0 (type 0, 1600x1024, format 28, 3 buffer(s)); created by sl.interposer.dll <- F1_25.exe <- KERNEL32.DLL <- ntdll.dll
2026-11-15 07:12:26.735 [ 1212] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 1212, 20 s after load, at 0x7c3615b0 (no module)
2026-11-15 07:12:26.735 [ 1212] [crash]   executing (DEP) 0x7c3615b0
2026-11-15 07:12:26.735 [ 1212] [crash]   rax=0000000080070057 rbx=00000000865546b0 rcx=0000000000000000 rdx=0000000000010000
2026-11-15 07:12:26.735 [ 1212] [crash]   rsi=000000018024056d rdi=00007fff2a76b566 rbp=0000000180073c51 rsp=000000000015c590
2026-11-15 07:12:26.735 [ 1212] [crash]   r8 =7ffffffffffffffc r9 =0000000000000000 r10=00007fffa78fb000 r11=000000000015c320
2026-11-15 07:12:26.735 [ 1212] [crash]   r12=0000000000000003 r13=000000006b9a5530 r14=0000000000000400 r15=0000000000000640
2026-11-15 07:12:26.735 [ 1212] [crash]   code: FF 00 00 00 FF 00 00 00 B4 13 6E 46 FF 17 00 88
2026-11-15 07:12:26.735 [ 1212] [crash]   stack:
2026-11-15 07:12:26.735 [ 1212] [crash]   #0  0x7c3615b0 (no module)
2026-11-15 07:12:26.735 [ 1212] [crash]   #1  0x64b82cb0 (no module)
";

    #[test]
    fn a_jump_to_no_module_with_registers_in_a_hook_names_its_program() {
        let mut e = ev(RTSS_STRAY_JUMP, &[]);
        e.code = Some(0xc000_0005);
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "third_party_hook");
        assert!(d.title.contains("RivaTuner"), "{}", d.title);
        assert!(d.what.contains("rtsshooks64.dll"), "{}", d.what);
        assert!(d.steps[0].contains("MSI Afterburner"), "{}", d.steps[0]);
        let reading = RTSS_STRAY_JUMP.replace("executing (DEP)", "reading");
        e.loader = reading;
        assert_eq!(
            diagnose(&e),
            None,
            "only a jump to an address that is not code counts"
        );
        e.loader = RTSS_STRAY_JUMP.replace("RTSSHooks64.dll @ 0x180000000", "RTSSHooks64.dll @ 0x809f0000");
        assert_eq!(diagnose(&e), None, "no register points into the hook's module");
        e.loader = RTSS_STRAY_JUMP.replace("module: RTSSHooks64.dll", "module: sl.interposer.dll");
        assert_eq!(
            diagnose(&e),
            None,
            "the registers point into a module that is no hook"
        );
        e.loader = RTSS_STRAY_JUMP.replace("[offline] module: RTSSHooks64.dll @ 0x180000000\n", "");
        assert_eq!(diagnose(&e), None, "the hook is not among the loaded modules");
    }

    #[test]
    fn module_bases_and_record_registers_are_read() {
        assert_eq!(
            module_bases(RTSS_STRAY_JUMP),
            [
                ("rtsshooks64.dll".to_owned(), 0x1_8000_0000),
                ("dxgi.dll".to_owned(), 0x7fff_a3b2_0000)
            ]
        );
        let f = fatal(&Evidence {
            code: Some(0xc000_0005),
            ..ev(RTSS_STRAY_JUMP, &[])
        });
        assert_eq!(f.module, None);
        let regs = record_registers(&f.record);
        assert_eq!(regs.len(), 16, "{regs:x?}");
        assert!(
            regs.contains(&0x1_8024_056d) && regs.contains(&0x7fff_ffff_ffff_fffc),
            "{regs:x?}"
        );
    }

    #[test]
    fn a_first_chance_crash_without_an_unhandled_line_still_counts() {
        let mut e = ev(RTSS, &[]);
        e.code = Some(0xc000_0005);
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "third_party_hook");
        assert!(d.title.contains("RivaTuner"), "{}", d.title);
        e.code = Some(0xc000_0409);
        assert_eq!(diagnose(&e), None);
        e.code = None;
        assert_eq!(diagnose(&e), None);
    }

    fn car_pack(winner: &str, loser: &str, resources: usize) -> Pair {
        Pair {
            winner: winner.into(),
            loser: loser.into(),
            resources,
            items: vec![
                "2025_asset_groups/f1_2025_vehicle_package/teams/ferrari/wep/ferrari.erp:eaid://ferrari.idf"
                    .into(),
            ],
            ..Pair::default()
        }
    }

    #[test]
    fn a_removed_gpu_names_car_packs_and_frame_generation() {
        let mut e = with_names(
            ev(GPU, &["F1_25.exe"]),
            &[
                ("mod.f1-2016", "F1 2016 Cars"),
                ("mod.f1-2018", "F1 2018 Cars"),
                ("mod.livery", "Livery"),
            ],
        );
        e.conflicts = vec![
            car_pack("mod.f1-2018", "mod.f1-2016", 8902),
            car_pack("mod.livery", "mod.f1-2018", 12),
        ];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "gpu_removed");
        assert!(!d.what.contains("not from Swofty's code"), "{}", d.what);
        assert!(
            d.what
                .contains("F1 2016 Cars and F1 2018 Cars are all full car packs"),
            "{}",
            d.what
        );
        assert_eq!(
            d.steps[0],
            "Keep only one of F1 2016 Cars and F1 2018 Cars on in your profile."
        );
        e.conflicts.clear();
        e.loader = format!(
            "2026-10-24 02:50:00.000 [1] [overlay] the game's hardware settings (x.xml) have frame generation on (frame_gen mode 1, multi_frame_gen 1)\n{GPU}"
        );
        let d = diagnose(&e).unwrap();
        assert!(d.what.contains("Frame generation was on"), "{}", d.what);
        assert_eq!(d.steps[0], FRAME_GEN_OFF);
        assert_eq!(
            d.steps.iter().filter(|s| s.contains("frame generation")).count(),
            1
        );
    }

    #[test]
    fn the_games_low_disk_dialog_names_the_drive_and_the_space() {
        let log = "2026-10-27 22:08:24.555 [ 3820] dialog shown: Insufficient Storage: 652/2048 MiB available on \
                   'C:\\', please free some space.\n22:08:32.240 [stub] [offline] game exited with code 4294967295\n";
        let d = diagnose(&ev(log, &[])).unwrap();
        assert_eq!(d.id, "low_disk");
        assert_eq!(d.title, "F1 25 needs more free space on drive C");
        assert_eq!(
            d.what,
            "F1 25 needs 2 GB free on drive C and found 0.6 GB, so it showed Insufficient Storage and closed. It \
             checks drive C even when the game is installed on another drive."
        );
        assert!(
            d.steps[0].starts_with("Free up at least 1.4 GB on drive C: empty"),
            "{}",
            d.steps[0]
        );
        let other = log.replace("652/2048", "2000/2048").replace("'C:", "'d:");
        let d = diagnose(&ev(&other, &[])).unwrap();
        assert_eq!(
            d.what,
            "F1 25 needs 2 GB free on drive D and found 1.9 GB, so it showed Insufficient Storage and closed."
        );
        assert!(
            d.steps[0].starts_with("Free up at least 0.1 GB on drive D: empty"),
            "{}",
            d.steps[0]
        );
        let path = log.replace("'C:\\'", "'\\\\server\\share'");
        assert!(
            diagnose(&ev(&path, &[]))
                .unwrap()
                .what
                .contains("on the drive F1 25 saves to")
        );
        assert_eq!(diagnose(&ev(&log.replace("652/2048", "lots"), &[])), None);
    }

    const AGGRESSIVE_AI_HEAP: [&str; 7] = [
        "F1_25.exe+0x39182a4 .xtls Ne_FatalAbort+0x4",
        "F1_25.exe+0x395fa23 .xtls Ne_ReportMemoryError+0x343",
        "F1_25.exe+0x39658d8 .xtls fn base+0x39657d0+0x108",
        "F1_25.exe+0x39640e3 .xtls Ne_Heap_DeallocateNonSmall+0x163",
        "F1_25.exe+0x3963f5f .xtls Ne_Heap_Free+0xdf",
        "F1_25.exe+0x67c1f2 .xtls fn base+0x67bf30+0x2c2",
        "F1_25.exe+0x8303f43 .data1 fn base+0x8303f00+0x43",
    ];
    const AGGRESSIVE_AI_HEAP_EA: [&str; 7] = [
        "F1_25.exe+0x3905334 .rdata fn base+0x3905330+0x4",
        "F1_25.exe+0x394cab3 .rdata fn base+0x394c770+0x343",
        "F1_25.exe+0x3952968 .rdata fn base+0x3952860+0x108",
        "F1_25.exe+0x3951173 .rdata fn base+0x3951010+0x163",
        "F1_25.exe+0x3950fef .rdata fn base+0x3950f10+0xdf",
        "F1_25.exe+0x67cc02 .rdata fn base+0x67c940+0x2c2",
        "F1_25.exe+0x80a5f03 .xcode fn base+0x80a5ec0+0x43",
    ];

    fn frames_log(frames: &[&str]) -> String {
        let at = frames[0];
        let mut log = format!(
            "2026-11-15 13:13:10.664 [2668] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 2668, 2607 s after load, at {at}\n\
             2026-11-15 13:13:10.664 [2668] [crash]   reading 0x8\n\
             2026-11-15 13:13:10.666 [2668] [crash]   stack:\n"
        );
        for (i, frame) in frames.iter().enumerate() {
            log.push_str(&format!(
                "2026-11-15 13:13:10.666 [2668] [crash]   #{i:<2} {frame}\n"
            ));
        }
        log.push_str(&format!(
            "2026-11-15 13:13:10.667 [2668] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at {at} reached the top-level filter; passing it to the game's filter\n"
        ));
        log
    }

    fn aggressive_ai_on(loader: &str, version: &str) -> Evidence {
        let mut e = with_names(
            ev(loader, &[]),
            &[(AGGRESSIVE_AI, "F1 25 + 26 Aggressive AI (Alternate)")],
        );
        e.versions.insert(AGGRESSIVE_AI.to_owned(), version.to_owned());
        e
    }

    #[test]
    fn a_session_load_crash_on_aggressive_ai_alternate_3_0_0_names_it() {
        let faces: [&[&str]; 10] = [
            &AGGRESSIVE_AI_HEAP,
            &[
                "F1_25.exe+0x67c959 .xtls fn base+0x67c680+0x2d9",
                "F1_25.exe+0x8303f69 .data1 fn base+0x8303f00+0x69",
            ],
            &[
                "F1_25.exe+0x67b49b .xtls fn base+0x67b3f0+0xab",
                "F1_25.exe+0x8308ef7 .data1 fn base+0x8308d50+0x1a7",
            ],
            &[
                "F1_25.exe+0x67b952 .xtls fn base+0x67b3f0+0x562",
                "F1_25.exe+0x8014c5c .data1 fn base+0x8014a90+0x1cc",
            ],
            &[
                "F1_25.exe+0x67b9aa .xtls fn base+0x67b3f0+0x5ba",
                "F1_25.exe+0x8014c5c .data1 fn base+0x8014a90+0x1cc",
            ],
            &[
                "0x1c4096c0 (no module)",
                "F1_25.exe+0x67b4a1 .xtls fn base+0x67b3f0+0xb1",
                "F1_25.exe+0x8014c5c .data1 fn base+0x8014a90+0x1cc",
            ],
            &AGGRESSIVE_AI_HEAP_EA,
            &[
                "F1_25.exe+0x67d369 .rdata fn base+0x67d090+0x2d9",
                "F1_25.exe+0x80a5f29 .xcode fn base+0x80a5ec0+0x69",
            ],
            &[
                "F1_25.exe+0x67beab .rdata fn base+0x67be00+0xab",
                "F1_25.exe+0x7e84a49 .xcode fn base+0x7e84870+0x1d9",
            ],
            &[
                "F1_25.exe+0x67c3ba .rdata fn base+0x67be00+0x5ba",
                "F1_25.exe+0x7e84a49 .xcode fn base+0x7e84870+0x1d9",
            ],
        ];
        for face in faces {
            let d = diagnose(&aggressive_ai_on(&frames_log(face), "3.0.0"));
            assert_eq!(d.map(|d| d.id), Some("aggressive_ai_alternate"), "{face:?}");
        }
        let split = frames_log(faces[4]).replace(
            "2026-11-15 13:13:10.666 [2668] [crash]   stack:\n",
            "2026-11-15 13:13:10.666 [9540] [crash] writing a crash report: unhandled ACCESS_VIOLATION (0xc0000005)\n",
        );
        let d = diagnose(&aggressive_ai_on(&split, "3.0.0"));
        assert_eq!(d.map(|d| d.id), Some("aggressive_ai_alternate"));
        let d = diagnose(&aggressive_ai_on(&frames_log(&AGGRESSIVE_AI_HEAP), "3.0.0")).unwrap();
        assert_eq!(
            d.title,
            "F1 25 + 26 Aggressive AI (Alternate) 3.0.0 crashed F1 25 while a session loaded"
        );
        assert!(d.what.contains("has been taken down"), "{}", d.what);
        assert!(
            d.steps[0].contains("install it again from the Addon Browser"),
            "{}",
            d.steps[0]
        );
        assert_eq!(
            d.steps[1],
            "Or turn off F1 25 + 26 Aggressive AI (Alternate) in your profile."
        );
    }

    #[test]
    fn the_aggressive_ai_alternate_crash_is_read_from_the_crash_report() {
        let mut e = aggressive_ai_on(
            "2026-11-15 20:38:35.219 [9112] [crash] Ne heap error 0x16 (fatal) on thread 9112: Failed to find address 0000000399450C30 in the allocated list; heap 000000006CB5F0B0 'RACE', capacity 10485760, used 2440304\n",
            "3.0.0",
        );
        e.report = Some(
            "reason       fatal Ne heap error 0x16 in heap 'RACE': Failed to find address 0000000399450C30 in the allocated list\n\
             == Stack (unwound with the modules' exception data)\n  \
             #0  00007ffe3e9d6f94 rsp=000000006f2ff290  swofty_core.dll+0xd6f94 .text fn +0xd6990+0x604\n  \
             #1  00007ffe3e9d68db rsp=000000006f2ff990  swofty_core.dll+0xd68db .text fn +0xd68b0+0x2b\n  \
             #2  00000001439658d8 rsp=000000006f2ff9e0  F1_25.exe+0x39658d8 .xtls fn base+0x39657d0+0x108\n  \
             #3  00000001439640e3 rsp=000000006f2ffad0  F1_25.exe+0x39640e3 .xtls Ne_Heap_DeallocateNonSmall+0x163\n  \
             #4  0000000143963f5f rsp=000000006f2ffb70  F1_25.exe+0x3963f5f .xtls Ne_Heap_Free+0xdf\n  \
             #5  000000014067c1f2 rsp=000000006f2ffbc0  F1_25.exe+0x67c1f2 .xtls fn base+0x67bf30+0x2c2\n  \
             #6  0000000148303f43 rsp=000000006f2ffc60  F1_25.exe+0x8303f43 .data1 fn base+0x8303f00+0x43\n"
                .to_owned(),
        );
        assert_eq!(diagnose(&e).map(|d| d.id), Some("aggressive_ai_alternate"));
    }

    fn skip_cutscenes_on(loader: &str, version: &str, build: (u64, u32)) -> Evidence {
        let mut e = with_names(ev(loader, &[]), &[(SKIP_CUTSCENES, "Skip Cutscenes")]);
        e.versions.insert(SKIP_CUTSCENES.to_owned(), version.to_owned());
        e.enabled = vec![SKIP_CUTSCENES.to_owned()];
        e.game_build = Some(build);
        e
    }

    #[test]
    fn skip_cutscenes_up_to_1_9_4_on_1_0_145_is_named() {
        let log = "2026-11-15 13:13:00.000 [1] [loader] vtable not found (Sess_SessionStateIntroCameras_OnEnter 142D6DE80)\n\
                   2026-11-15 13:13:40.000 [2] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at skip_cutscenes.dll+0x23f4 reached the top-level filter; passing it to the game's filter\n";
        for version in ["1.9.4", "1.0.0", "1.9.3"] {
            let d = diagnose(&skip_cutscenes_on(
                log,
                version,
                crate::build_support::BUILD_1_0_145,
            ))
            .unwrap();
            assert_eq!(d.id, "skip_cutscenes_1_0_145", "{version}");
        }
        let d = diagnose(&skip_cutscenes_on(
            log,
            "1.9.4",
            crate::build_support::BUILD_1_0_145,
        ))
        .unwrap();
        assert_eq!(
            d.what,
            "Skip Cutscenes (up to 1.9.4) crashes the game on F1 25 version 1.0.145 within a minute of loading."
        );
        assert_eq!(
            d.steps,
            ["Turn it off on the Addons page until its author releases an update."]
        );
    }

    #[test]
    fn skip_cutscenes_is_not_named_on_other_versions_builds_or_when_off() {
        let log = "2026-11-15 13:13:00.000 [1] [loader] vtable not found (Sess_SessionStateIntroCameras_OnEnter 142D6DE80)\n";
        let on = crate::build_support::BUILD_1_0_145;
        assert_eq!(diagnose(&skip_cutscenes_on(log, "1.9.5", on)), None);
        assert_eq!(
            diagnose(&skip_cutscenes_on(log, "1.9.4", (408_063_872, 1_789_118_616))),
            None
        );
        let mut off = skip_cutscenes_on(log, "1.9.4", on);
        off.enabled.clear();
        assert_eq!(diagnose(&off), None);
    }

    #[test]
    fn other_aggressive_ai_alternate_versions_and_crashes_are_not_named() {
        let heap = frames_log(&AGGRESSIVE_AI_HEAP);
        for version in ["2.0.1", "3.0.1", "3.1.0", "13.0.0"] {
            assert_eq!(diagnose(&aggressive_ai_on(&heap, version)), None, "{version}");
        }
        let mut off = aggressive_ai_on(&heap, "3.0.0");
        off.enabled.clear();
        assert_eq!(diagnose(&off), None);
        assert_eq!(diagnose(&ev(&heap, &[])), None);
        let unrelated = frames_log(&[
            "F1_25.exe+0x2e57efb .xtls fn base+0x2e57e90+0x6b",
            "F1_25.exe+0x445b18c .xtls fn base+0x445aea0+0x2ec",
        ]);
        assert_eq!(diagnose(&aggressive_ai_on(&unrelated, "3.0.0")), None);
        let deep: Vec<&str> =
            std::iter::repeat_n("F1_25.exe+0x445b18c .xtls fn base+0x445aea0+0x2ec", NEAR_FRAMES)
                .chain(["F1_25.exe+0x67c1f2 .xtls fn base+0x67bf30+0x2c2"])
                .collect();
        assert_eq!(diagnose(&aggressive_ai_on(&frames_log(&deep), "3.0.0")), None);
    }

    fn pitlane_log(kept: &str, mode: &str, address: &str) -> String {
        format!(
            "2026-10-27 19:26:15.735 [30872] [pitlane] your car: flags 0 -> 0x1, mode 0 -> 1
2026-10-27 19:26:15.741 [30872] [pitlane] pit lane: control kept (7 this session){kept}
2026-10-27 19:26:15.761 [30872] [pitlane] your car: flags 0x1 -> 0, mode 1 -> {mode}
2026-10-27 19:26:15.767 [12860] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 12860, 2079 s after load, at F1_25.exe+{address} .xtls fn base+0x389a120+0xb7
2026-10-27 19:26:15.767 [12860] [crash]   reading 0x2d3bf4f40
2026-10-27 19:26:15.767 [12860] [crash]   stack:
2026-10-27 19:26:15.767 [12860] [crash]   #0  F1_25.exe+{address} .xtls fn base+0x389a120+0xb7
2026-10-27 19:26:15.767 [12860] [crash]   #1  F1_25.exe+0x445b18c .xtls fn base+0x445aea0+0x2ec
2026-10-27 19:26:15.768 [12860] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+{address} .xtls fn base+0x389a120+0xb7 reached the top-level filter; passing it to the game's filter
"
        )
    }

    #[test]
    fn a_crash_as_manual_pit_lane_takes_the_car_names_it() {
        let e = with_names(
            ev(&pitlane_log("", "1474065524", "0x389a1d7"), &[]),
            &[("kizi.pitlane", "Manual Pit Lane")],
        );
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "pitlane_garage");
        assert_eq!(d.title, "Manual Pit Lane crashed F1 25 in the pit lane");
        assert!(d.what.contains("Return to Garage or a flashback"), "{}", d.what);
        assert_eq!(
            d.steps[0],
            "Turn off Manual Pit Lane in your profile until its author fixes this crash."
        );
        let older_game = pitlane_log("", "1159247034", "0x38873a7");
        assert_eq!(diagnose(&ev(&older_game, &[])).unwrap().id, "pitlane_garage");
        let kept_only = pitlane_log("", "0", "0x389a1d7");
        assert_eq!(diagnose(&ev(&kept_only, &[])).unwrap().id, "pitlane_garage");
        let ai_kept = pitlane_log(
            " - another flag is set too, so the AI keeps the car for now",
            "0",
            "0x389a1d7",
        );
        assert_eq!(diagnose(&ev(&ai_kept, &[])), None);
        assert_eq!(diagnose(&ev(&pitlane_log("", "0", "0x1234567"), &[])), None);
        let later = pitlane_log("", "1474065524", "0x389a1d7").replace("19:26:15.7", "19:26:19.7");
        let later = later.replacen("19:26:19.7", "19:26:15.7", 3);
        assert_eq!(diagnose(&ev(&later, &[])), None);
    }

    /// The game folder's file names among the crash report's loaded modules.
    fn game_folder_modules(report: &str) -> Vec<String> {
        let exe = report
            .lines()
            .find_map(|l| l.strip_prefix("exe")?.trim().split(", ").next())
            .unwrap_or_default();
        let Some((folder, _)) = exe.rsplit_once('\\') else {
            return Vec::new();
        };
        report
            .lines()
            .filter_map(|l| l.trim().split_once("  ").and(l.find(folder)).map(|i| &l[i..]))
            .filter_map(|p| p.strip_prefix(folder)?.strip_prefix('\\'))
            .filter(|rest| !rest.contains('\\'))
            .map(str::to_owned)
            .collect()
    }

    const LIST_SEARCH: &str = "\
2026-10-28 01:49:51.119 [15588] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 15588, 237 s after load, at F1_25.exe+0x1738f30 .xtls fn base+0x1738eb0+0x80
2026-10-28 01:49:51.119 [15588] [crash]   rsi=000000006b18b9ca rdi=0000000021500d09 rbp=0000000000000000 rsp=000000006b18b940
2026-10-28 01:49:51.119 [15588] [crash]   #0  F1_25.exe+0x1738f30 .xtls fn base+0x1738eb0+0x80
2026-10-28 01:49:51.119 [15588] [crash]   #1  F1_25.exe+0x17384ec .xtls fn base+0x17382b0+0x23c
2026-10-28 01:49:51.120 [15588] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x1738f30 .xtls fn base+0x1738eb0+0x80 reached the top-level filter; passing it to the game's filter
";

    const LIST_REPORT: &str = "\
== Exception
  ACCESS_VIOLATION (0xc0000005), flags 0x0, at F1_25.exe+0x1738f30 .xtls fn base+0x1738eb0+0x80

== Memory at the registers
  rcx=000000000715a9d1: committed private region 0x715a000 +0x238a6000 (allocation 0x6c90000), read/write
    000000000715a9d0> 66 31 5f 32 30 32 36 5f 64 72 69 76 65 72 5f 70  f1_2026_driver_p
  rdi=0000000021500d09: committed private region 0x21500000 +0x9500000 (allocation 0x6c90000), read/write
    0000000021500cf0  64 00 95 9e 01 00 00 00 f0 40 6a 21 00 00 00 00  d........@j!....
    0000000021500d00> 66 17 e9 2a 01 00 07 00 c0 66 32 5f 32 30 32 36  f..*.....f2_2026
    0000000021500d10  00 6e 5f 70 61 63 6b 61 67 65 00 17 00 00 00 00  .n_package......
  r14=cb42b7f752c18f53: not a valid address
";

    #[test]
    fn a_missing_entrant_list_names_the_list_and_the_addon_the_build_saw() {
        let mut e = ev(LIST_SEARCH, &["F1_25.exe"]);
        e.report = Some(LIST_REPORT.to_owned());
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "missing_entrant_list");
        assert_eq!(d.title, "F1 25 crashed looking for the entrant list f2_2026");
        assert!(
            d.steps[0].starts_with("Turn off the database addons"),
            "{:?}",
            d.steps
        );

        e = with_names(e, &[("mod.old-team-pack", "Old Team Pack")]);
        e.kept_lists = BTreeMap::from([("mod.old-team-pack".to_owned(), vec!["f2_2026".to_owned()])]);
        let d = diagnose(&e).unwrap();
        assert!(
            d.what
                .ends_with("The last build found Old Team Pack renaming or removing it."),
            "{}",
            d.what
        );
        assert_eq!(
            d.steps[0],
            "Turn off Old Team Pack, or ask its author for a version made for this F1 25 update."
        );

        e.report = None;
        let d = diagnose(&e).unwrap();
        assert_eq!(d.title, "F1 25 crashed looking for an entrant list");
        let other = LIST_SEARCH.replace("+0x1738f30", "+0x1738f31");
        assert_ne!(
            diagnose(&ev(&other, &["F1_25.exe"])).map(|d| d.id),
            Some("missing_entrant_list")
        );
    }

    const CAREER_TRACK: &str = "\
2026-10-28 20:58:46.394 [4936] [career] career.loaded: mode driver_career, key 'dc-00000003' (components live, component mode driver_career, Team HQ answer none)
2026-10-28 20:58:47.933 [5060] [career] flow \"StateLoadSession\" None -> Entering
2026-10-28 20:58:47.954 [7472] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 7472, 169 s after load, at F1_25.exe+0x2e57efb .xtls fn base+0x2e57e90+0x6b
2026-10-28 20:58:47.954 [7472] [crash]   reading 0x78
2026-10-28 20:58:47.954 [7472] [crash]   stack:
2026-10-28 20:58:47.954 [7472] [crash]   #0  F1_25.exe+0x2e57efb .xtls fn base+0x2e57e90+0x6b
2026-10-28 20:58:47.954 [7472] [crash]   #1  F1_25.exe+0x7a13ce .xtls fn base+0x7a1310+0xbe
2026-10-28 20:58:47.954 [7472] [crash]   #2  F1_25.exe+0x8706316 .data1 fn base+0x87062f0+0x26
2026-10-28 20:58:47.954 [7472] [crash]   #3  F1_25.exe+0x2d48bb7 .xtls Sess_Loader_SetupTrackAssets+0x47
2026-10-28 20:58:47.954 [7472] [crash]   #4  F1_25.exe+0x2d471af .xtls Sess_Loader_RequestSessionLoad+0xef
2026-10-28 20:58:47.955 [7472] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x2e57efb .xtls fn base+0x2e57e90+0x6b reached the top-level filter; passing it to the game's filter
";

    fn career_off(loader: &str, off: &[&str]) -> Evidence {
        let mut e = with_names(
            ev(loader, &[]),
            &[
                ("mod.bahrain24", "Sepang"),
                ("swofty.classic-seasons-2018", "F1 2018 GP/TT Addon"),
            ],
        );
        e.off = off.iter().map(|id| (*id).to_owned()).collect();
        e
    }

    #[test]
    fn a_career_crash_setting_up_the_track_names_the_addon_that_is_off() {
        let d = diagnose(&career_off(CAREER_TRACK, &["mod.bahrain24"])).unwrap();
        assert_eq!(d.id, "career_content_off");
        assert_eq!(d.title, "Your career uses Sepang, which is off");
        assert!(
            d.what
                .ends_with("A race in your career uses Sepang, which is off in this profile."),
            "{}",
            d.what
        );
        assert_eq!(
            d.steps[0],
            "Turn Sepang back on. In a My Team career you can also change that round with the Career Mode Editor."
        );

        let d = diagnose(&career_off(
            CAREER_TRACK,
            &["mod.bahrain24", "swofty.classic-seasons-2018"],
        ))
        .unwrap();
        assert_eq!(d.id, "career_content_off");
        assert_eq!(d.title, "Your career uses a track or season that is off");
        assert!(
            d.what.ends_with("which has Sepang and F1 2018 GP/TT Addon off."),
            "{}",
            d.what
        );
        assert!(!d.what.contains("uses Sepang"), "{}", d.what);
        assert_eq!(
            d.steps[0],
            "Turn Sepang and F1 2018 GP/TT Addon back on. In a My Team career you can also change that round with the Career Mode Editor."
        );
    }

    #[test]
    fn a_career_track_crash_needs_an_addon_off_a_fresh_career_load_and_the_track_setup() {
        assert_eq!(diagnose(&career_off(CAREER_TRACK, &[])), None, "nothing off");
        let late = CAREER_TRACK.replace("20:58:46.394", "20:58:30.000");
        assert_eq!(
            diagnose(&career_off(&late, &["mod.bahrain24"])),
            None,
            "long after the career loaded"
        );
        let unloaded = CAREER_TRACK.replace(
            "2026-10-28 20:58:47.933",
            "2026-10-28 20:58:47.000 [4936] [career] career.unloaded (mode driver_career)\n2026-10-28 20:58:47.933",
        );
        assert_eq!(
            diagnose(&career_off(&unloaded, &["mod.bahrain24"])),
            None,
            "the career unloaded"
        );
        let grid = CAREER_TRACK.replace(
            ".xtls fn base+0x7a1310+0xbe",
            ".xtls Game_GridSetup_DriverBasedGrid+0x847",
        );
        assert_eq!(
            diagnose(&career_off(&grid, &["mod.bahrain24"])),
            None,
            "a nearer named frame"
        );
        let no_career = CAREER_TRACK.replace("career.loaded: ", "career.starting: ");
        assert_eq!(
            diagnose(&career_off(&no_career, &["mod.bahrain24"])),
            None,
            "no career"
        );
    }

    const SEQUENCER: &str = "\
2026-11-03 09:03:51.484 [12700] [era] session trace Sess_Setup_VehicleComponents (driver_career_progressive): session vehicles 22
2026-11-03 09:03:51.486 [12700] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 12700, 119 s after load, at F1_25.exe+0xc72ea0 .xtls Game_CareerEventsSequencer_Initialise+0x1020
2026-11-03 09:03:51.486 [12700] [crash]   reading 0x0
2026-11-03 09:03:51.486 [12700] [crash]   stack:
2026-11-03 09:03:51.486 [12700] [crash]   #0  F1_25.exe+0xc72ea0 .xtls Game_CareerEventsSequencer_Initialise+0x1020
2026-11-03 09:03:51.486 [12700] [crash]   #1  F1_25.exe+0x7bef38 .xtls fn base+0x7bec10+0x328
2026-11-03 09:03:51.487 [12700] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0xc72ea0 .xtls Game_CareerEventsSequencer_Initialise+0x1020 reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_crash_reading_the_first_calendar_event_record_names_the_calendar_addon() {
        let mut e = with_names(
            ev(SEQUENCER, &[]),
            &[
                ("dbedit.new-calender", "New calender"),
                ("swofty.progressive", "Progressive Career"),
            ],
        );
        e.enabled = vec!["swofty.progressive".to_owned(), "dbedit.new-calender".to_owned()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "calendar_event_missing");
        assert_eq!(d.title, "New calender changed your career's calendar");
        assert!(d.steps[0].starts_with("Turn off New calender"), "{}", d.steps[0]);

        e.enabled = vec!["swofty.progressive".to_owned()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "calendar_event_missing");
        assert_eq!(d.title, "Your career's calendar has a round F1 25 can't find");

        let other = SEQUENCER.replace("Initialise+0x1020", "Initialise+0x44");
        assert_eq!(
            diagnose(&ev(&other, &[])).map(|d| d.id),
            None,
            "another offset of the function"
        );
    }

    const MORPH: &str = "\
2026-11-12 19:34:23.598 [ 3048] [silly-season] session start: race session, weekend round 4
2026-11-12 19:34:23.598 [ 3048] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 3048, 111 s after load, at F1_25.exe+0xc80962 .xtls Game_CareerEventResourceManager_AddDriverMorphData+0x82
2026-11-12 19:34:23.598 [ 3048] [crash]   reading 0x8
2026-11-12 19:34:23.599 [ 3048] [crash]   stack:
2026-11-12 19:34:23.599 [ 3048] [crash]   #0  F1_25.exe+0xc80962 .xtls Game_CareerEventResourceManager_AddDriverMorphData+0x82
2026-11-12 19:34:23.599 [ 3048] [crash]   #1  F1_25.exe+0xc72ebf .xtls Game_CareerEventsSequencer_Initialise+0x103f
2026-11-12 19:34:23.599 [ 3048] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0xc80962 .xtls Game_CareerEventResourceManager_AddDriverMorphData+0x82 reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_crash_reading_a_missing_team_driver_names_no_addon() {
        let mut e = with_names(
            ev(MORPH, &[]),
            &[
                ("alexs.silly-season", "Silly Season"),
                ("custom.team-names", "Team & Driver Editor"),
                ("mildtdesign.more-driver-models", "More Driver Models"),
            ],
        );
        e.enabled = vec![
            "mildtdesign.more-driver-models".to_owned(),
            "alexs.silly-season".to_owned(),
            "custom.team-names".to_owned(),
        ];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "career_driver_missing");
        assert_eq!(d.title, "Your career has a driver F1 25 can't find");
        for addon in ["Silly Season", "Team & Driver Editor", "More Driver Models"] {
            assert!(!d.text().contains(addon), "{addon} is named: {}", d.text());
        }

        e.enabled = vec![];
        assert_eq!(diagnose(&e).unwrap(), d, "the same diagnosis with no addon on");

        let other = MORPH.replace("AddDriverMorphData+0x82", "AddDriverMorphData+0x44");
        assert_eq!(
            diagnose(&ev(&other, &[])).map(|d| d.id),
            None,
            "another offset of the function"
        );
    }

    const RETIREMENT: &str = "\
2026-11-12 20:54:48.502 [29072] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 29072, 612 s after load, at F1_25.exe+0xcb8a5e1 .data1 fn base+0xcb8a5d0+0x11
2026-11-12 20:54:48.502 [29072] [crash]   reading 0x1f0
2026-11-12 20:54:48.502 [29072] [crash]   rax=0000000000000000 rbx=0000000000000018 rcx=0000000000000018 rdx=0000000067dce2c0
2026-11-12 20:54:48.502 [29072] [crash]   stack:
2026-11-12 20:54:48.502 [29072] [crash]   #0  F1_25.exe+0xcb8a5e1 .data1 fn base+0xcb8a5d0+0x11
2026-11-12 20:54:48.503 [29072] [crash]   #1  F1_25.exe+0x2d624ee .xtls Sess_InGameRace_Update+0x77e
2026-11-12 20:54:48.503 [29072] [crash]   #2  F1_25.exe+0x2d56e08 .xtls Sess_InGame_UpdateBase+0x768
2026-11-12 20:54:48.511 [29072] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0xcb8a5e1 .data1 fn base+0xcb8a5d0+0x11 reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_crash_naming_a_retired_car_without_a_driver_names_no_addon() {
        let mut e = with_names(
            ev(RETIREMENT, &[]),
            &[
                ("kizi.canned", "Canned DNFs"),
                ("progressive.career", "Progressive Career"),
                ("custom.team-names", "Team & Driver Editor"),
            ],
        );
        e.enabled = vec![
            "kizi.canned".to_owned(),
            "progressive.career".to_owned(),
            "custom.team-names".to_owned(),
        ];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "race_driver_missing");
        assert_eq!(d.title, "A car in your race has a driver F1 25 can't find");
        for addon in ["Canned DNFs", "Progressive Career", "Team & Driver Editor"] {
            assert!(!d.text().contains(addon), "{addon} is named: {}", d.text());
        }

        e.enabled = vec![];
        assert_eq!(diagnose(&e).unwrap(), d, "the same diagnosis with no addon on");

        let other = RETIREMENT.replace("Sess_InGameRace_Update+0x77e", "Sess_InGameRace_Update+0x6a1");
        assert_eq!(
            diagnose(&ev(&other, &[])).map(|d| d.id),
            None,
            "another call in the race update"
        );
        let read = RETIREMENT.replace("reading 0x1f0", "reading 0x8");
        assert_eq!(
            diagnose(&ev(&read, &[])).map(|d| d.id),
            None,
            "another read under the same call"
        );
    }

    const WEEKENDS: &str = "\
2026-11-15 16:15:41.332 [ 9736] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 9736, 5051 s after load, at F1_25.exe+0x181b87a .xtls Career_Weekends_BuildResultNodes+0x4da
2026-11-15 16:15:41.332 [ 9736] [crash]   reading 0x1110000008
2026-11-15 16:15:41.332 [ 9736] [crash]   rax=0000001110000000 rbx=0000000067cff500 rcx=0000001110000008 rdx=0000000000000005
2026-11-15 16:15:41.332 [ 9736] [crash]   stack:
2026-11-15 16:15:41.332 [ 9736] [crash]   #0  F1_25.exe+0x181b87a .xtls Career_Weekends_BuildResultNodes+0x4da
2026-11-15 16:15:41.332 [ 9736] [crash]   #1  F1_25.exe+0x181b37b .xtls fn base+0x181b280+0xfb
2026-11-15 16:15:41.341 [ 9736] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x181b87a .xtls Career_Weekends_BuildResultNodes+0x4da reached the top-level filter; passing it to the game's filter
";

    const NO_WEEKEND: &str = "\
2026-11-17 10:25:53.419 [ 2100] [Progressive season] Imported 12 rounds into the Career picker.
2026-11-17 10:25:53.773 [ 2100] [Progressive season] Advancing imported calendar.
2026-11-17 10:25:53.773 [ 2100] [Free Calendar] Confirmed 12 selected rounds.
2026-11-17 10:31:07.402 [ 1500] [view] pause reason bit 0 cleared, mask 00000000
2026-11-17 10:31:07.453 [ 1500] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 1500, 412 s after load, at F1_25.exe+0x240bacb .tls$ fn base+0x240b9b0+0x11b
2026-11-17 10:31:07.453 [ 1500] [crash]   reading 0x250
2026-11-17 10:31:07.453 [ 1500] [crash]   r12=0000000001000000 r13=0000000002000000 r14=0000000000000000 r15=0000000000000000
2026-11-17 10:31:07.454 [ 1500] [crash]   stack:
2026-11-17 10:31:07.454 [ 1500] [crash]   #0  F1_25.exe+0x240bacb .tls$ fn base+0x240b9b0+0x11b
2026-11-17 10:31:07.454 [ 1500] [crash]   #1  F1_25.exe+0x2da9fb1 .tls$ fn base+0x2da9e00+0x1b1
2026-11-17 10:31:07.456 [ 1500] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x240bacb .tls$ fn base+0x240b9b0+0x11b reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_first_session_without_a_weekend_after_a_calendar_import_names_the_picker() {
        let mut e = with_names(
            ev(NO_WEEKEND, &[]),
            &[(CUSTOM_CALENDAR, "Custom Calendar"), (F2_REVAMP, "F2 Revamp")],
        );
        e.enabled = vec![CUSTOM_CALENDAR.to_owned(), F2_REVAMP.to_owned()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "imported_calendar_empty_season");
        assert_eq!(
            d.title,
            "Custom Calendar's imported calendar left your career without a first round"
        );
        assert!(
            d.what.contains("F2 Revamp's career, which starts in F2"),
            "{}",
            d.what
        );
        assert!(
            d.steps[0].contains("without the Custom Calendar picker (F9)"),
            "{}",
            d.steps[0]
        );

        e.enabled = vec![CUSTOM_CALENDAR.to_owned()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "imported_calendar_empty_season");
        assert!(!d.what.contains("F2 Revamp"), "{}", d.what);

        let named = NO_WEEKEND.replace(
            " fn base+0x240b9b0+0x11b",
            " Sess_WeekendManager_PrepareTrackLoad+0x11b",
        );
        assert_eq!(
            diagnose(&ev(&named, &[])).map(|d| d.id),
            Some("imported_calendar_empty_season"),
            "the frame by name"
        );
        let no_import = NO_WEEKEND.replace(
            "[Progressive season] Imported 12",
            "[Progressive season] Accepted 12",
        );
        assert_eq!(diagnose(&ev(&no_import, &[])).map(|d| d.id), None, "no import");
        let late = format!(
            "{NO_WEEKEND}2026-11-17 10:40:00.000 [ 2100] [Progressive season] Imported 3 rounds into the Career picker.\n"
        );
        let late = late.replacen(
            "[Progressive season] Imported 12",
            "[Progressive season] Accepted 12",
            1,
        );
        assert_eq!(
            diagnose(&ev(&late, &[])).map(|d| d.id),
            None,
            "an import after the crash"
        );
        let other = NO_WEEKEND.replace("+0x240b9b0+0x11b", "+0x240b9b0+0x1d2");
        assert_eq!(
            diagnose(&ev(&other, &[])).map(|d| d.id),
            None,
            "another offset of the function"
        );
        let read = NO_WEEKEND.replace("reading 0x250", "reading 0x268");
        assert_eq!(diagnose(&ev(&read, &[])).map(|d| d.id), None, "another read");
    }

    #[test]
    fn a_crash_reading_a_round_s_track_node_names_no_addon() {
        let mut e = with_names(ev(WEEKENDS, &[]), &[("progressive.career", "Progressive Career")]);
        e.enabled = vec!["progressive.career".to_owned()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "weekends_track_record_bad_index");
        assert_eq!(d.title, "A round of your career has a track F1 25 can't find");
        assert!(
            !d.text().contains("Progressive Career"),
            "an addon is named: {}",
            d.text()
        );

        e.enabled = vec![];
        assert_eq!(diagnose(&e).unwrap(), d, "the same diagnosis with no addon on");

        let other = WEEKENDS.replace(
            "Career_Weekends_BuildResultNodes+0x4da",
            "Career_Weekends_BuildResultNodes+0x4c1",
        );
        assert_eq!(
            diagnose(&ev(&other, &[])).map(|d| d.id),
            None,
            "another offset of the function"
        );
    }

    const THIRD_CAR_LINE: &str = "\
2026-11-03 10:27:30.114 [3516] [garage] garage 8: team 8 car #3 (driver 41) is the third car of its garage; its crew is not spawned, the car still races
2026-11-03 10:27:56.399 [3516] [session] vehicle slot 22 (hash 5c0e1d2a) is in no AI asset group; using 3b1f0a77 for its AI record
2026-11-03 10:27:56.420 [3516] [crash] #1 first-chance (the game may still handle it) ACCESS_VIOLATION (0xc0000005) on thread 3516, 217 s after load, at F1_25.exe+0x686a9e .xtls Ai_LineCache_UpdateIfChanged+0x5e
2026-11-03 10:27:56.420 [3516] [crash]   reading 0x178
2026-11-03 10:27:56.420 [3516] [crash]   stack:
2026-11-03 10:27:56.420 [3516] [crash]   #0  F1_25.exe+0x686a9e .xtls Ai_LineCache_UpdateIfChanged+0x5e
2026-11-03 10:27:56.420 [3516] [crash]   #1  F1_25.exe+0x63ece8 .xtls Ai_Job_UpdateCarsParallel+0x978
2026-11-03 10:27:56.421 [3516] [crash] unhandled ACCESS_VIOLATION (0xc0000005) at F1_25.exe+0x686a9e .xtls Ai_LineCache_UpdateIfChanged+0x5e reached the top-level filter; passing it to the game's filter
";

    #[test]
    fn a_line_cache_crash_with_a_third_car_names_the_addon_that_adds_it() {
        let mut e = with_names(
            ev(THIRD_CAR_LINE, &[]),
            &[
                ("mildtdesign.more-driver-models", "More Driver Models"),
                ("swofty.progressive", "Progressive Career"),
            ],
        );
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "third_car_line");
        assert!(
            d.what.contains("More Driver Models giving a team a third car"),
            "{}",
            d.what
        );
        assert!(
            d.steps[0].starts_with("Turn off More Driver Models"),
            "{}",
            d.steps[0]
        );

        e.enabled = vec!["swofty.progressive".to_owned()];
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "third_car_line");
        assert!(d.what.contains("a team with three cars"), "{}", d.what);

        let no_third = THIRD_CAR_LINE.replace("is the third car of its garage", "is a car of its garage");
        assert_ne!(
            diagnose(&ev(&no_third, &[])).map(|d| d.id),
            Some("third_car_line")
        );
        let other = THIRD_CAR_LINE.replace("Ai_LineCache_UpdateIfChanged+0x5e", "Ai_LineCache_Other+0x5e");
        assert_ne!(diagnose(&ev(&other, &[])).map(|d| d.id), Some("third_car_line"));
    }

    #[test]
    fn a_track_stat_id_with_no_row_is_a_missing_track_even_with_nothing_off() {
        let line =
            "2026-10-28 20:58:47.900 [5060] [moments] track stat id 48 has no tracks row or track record\n";
        let loader = CAREER_TRACK.replace(
            "2026-10-28 20:58:47.933",
            &format!("{line}2026-10-28 20:58:47.933"),
        );
        assert_eq!(missing_track_stat(&loader), Some(48));
        let d = diagnose(&career_off(&loader, &[])).unwrap();
        assert_eq!(d.id, "career_content_off");
        assert_eq!(d.title, "Your career uses a track that is not in this profile");
        assert!(d.what.contains("track stat id 48"), "{}", d.what);
        assert_eq!(missing_track_stat(CAREER_TRACK), None);
    }

    #[test]
    fn a_career_track_crash_reads_the_crash_reports_stack() {
        let loader = CAREER_TRACK
            .lines()
            .filter(|l| !l.contains("[crash]   ") && !l.contains("first-chance"))
            .map(|l| format!("{l}\n"))
            .collect::<String>();
        let mut e = career_off(&loader, &["mod.bahrain24"]);
        e.report = Some(
            "== Exception\n  ACCESS_VIOLATION (0xc0000005), flags 0x0, at F1_25.exe+0x2e57efb .xtls fn base+0x2e57e90+0x6b\n\
             == Stack (unwound with the modules' exception data)\n  \
             #0  0000000142e57efb rsp=000000006cfa7020  F1_25.exe+0x2e57efb .xtls fn base+0x2e57e90+0x6b\n  \
             #1  0000000142d48bb7 rsp=000000006cfae970  F1_25.exe+0x2d48bb7 .xtls Sess_Loader_SetupTrackAssets+0x47\n"
                .to_owned(),
        );
        assert_eq!(diagnose(&e).map(|d| d.id), Some("career_content_off"));
    }

    #[test]
    fn the_profiles_track_and_season_addons_that_are_off_are_read() {
        let root = std::env::temp_dir().join(format!("swofty-kp-off-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let manifest = |id: &str, category: &str| {
            let m = AddonManifest::parse(&format!(
                r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t",
                "compat":{{"game":"f1_25"}},"category":"{category}"}}"#
            ))
            .unwrap();
            (id.to_owned(), m)
        };
        let addons = BTreeMap::from([
            manifest("t.track-on", "tracks"),
            manifest("t.track-off", "tracks"),
            manifest("t.season-off", "seasons"),
            manifest("t.cars-off", "cars"),
        ]);
        let mut p = crate::profile::Profile::new("main");
        for (id, enabled) in [
            ("t.track-on", true),
            ("t.track-off", false),
            ("t.cars-off", false),
            ("t.season-off", false),
            ("t.gone", false),
        ] {
            p.addons.push(crate::profile::ProfileAddon {
                id: id.to_owned(),
                enabled,
                unknown: BTreeMap::new(),
            });
        }
        store.save_profile(&p).unwrap();
        store.set_active_profile("main").unwrap();
        let e = Evidence::default().with_profile(&store, &addons);
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(e.off, ["t.track-off", "t.season-off"]);
    }

    /// `SWOFTY_KP_CASES=<folder of extracted Send Logs bundles> cargo test -p swofty-core --lib
    /// diagnoses_real_bundles -- --ignored --nocapture` prints each crashed session's diagnosis,
    /// and each other session a rule recognises.
    #[test]
    fn unmatched_crash_on_old_build_names_the_update() {
        let mut e = ev("", &[]);
        assert_eq!(diagnose(&e), None);
        e.game_version = Some("1.0.136.6820".to_owned());
        let d = diagnose(&e).unwrap();
        assert_eq!(d.id, "outdated_exe");
        assert!(d.what.contains("1.0.136.6820"));
        assert!(d.text().contains("Steam or the EA app"));
        e.game_version = Some(LATEST_GAME_VERSION.to_owned());
        assert_eq!(diagnose(&e), None);
    }

    #[test]
    #[ignore]
    fn diagnoses_real_bundles() {
        let Some(root) = std::env::var_os("SWOFTY_KP_CASES") else {
            return;
        };
        let dirs = |p: &Path| -> Vec<std::path::PathBuf> {
            let mut v: Vec<_> = std::fs::read_dir(p)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            v.sort();
            v
        };
        let json = |p: &Path| -> serde_json::Value {
            read_capped(p)
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default()
        };
        for case in dirs(Path::new(&root)) {
            let logs = case.join("logs");
            let addons = json(&logs.join("addons.json"));
            let installed = addons["installed"].as_array().cloned().unwrap_or_default();
            let names = installed
                .iter()
                .filter_map(|a| Some((a["id"].as_str()?.to_owned(), a["name"].as_str()?.to_owned())))
                .collect();
            let versions: BTreeMap<String, String> = installed
                .iter()
                .filter_map(|a| Some((a["id"].as_str()?.to_owned(), a["version"].as_str()?.to_owned())))
                .collect();
            let conflicts: crate::conflicts::Report =
                serde_json::from_value(json(&logs.join("conflicts.json"))).unwrap_or_default();
            let tracks = conflicts
                .order
                .iter()
                .filter(|id| {
                    installed
                        .iter()
                        .any(|a| a["id"].as_str() == Some(id.as_str()) && a["category"] == "tracks")
                })
                .cloned()
                .collect();
            let off: Vec<String> = addons["profiles"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| p["active"] == true)
                .flat_map(|p| p["addons"].as_array().cloned().unwrap_or_default())
                .filter(|a| a["enabled"] == false)
                .filter_map(|a| a["id"].as_str().map(str::to_owned))
                .filter(|id| {
                    installed.iter().any(|a| {
                        a["id"].as_str() == Some(id.as_str())
                            && (a["category"] == "tracks" || a["category"] == "seasons")
                    })
                })
                .collect();
            for session in dirs(&logs.join("sessions")) {
                let record = SessionRecord::read(&session);
                let loader = ["loader.log", "game-logs/loader.log"]
                    .iter()
                    .find_map(|rel| read_capped(&session.join(rel)))
                    .unwrap_or_default();
                let crashed = record.crashed()
                    || loader.contains("reached the top-level filter")
                    || loader.contains("[crash] crash report ")
                    || loader.contains(LOW_DISK);
                let report = report_name(&loader).and_then(|n| read_capped(&logs.join("crash").join(n)));
                let game_files = report.as_deref().map(game_folder_modules).unwrap_or_default();
                let mut e = Evidence::read(&record, None, BTreeMap::new(), None);
                let game_version = read_capped(&logs.join("summary.txt")).and_then(|t| {
                    t.lines()
                        .find_map(|l| l.strip_prefix("F1 25 "))
                        .map(|v| v.trim().to_owned())
                });
                e = Evidence {
                    game_version,
                    loader,
                    report,
                    game_files,
                    names: BTreeMap::clone(&names),
                    versions: versions.clone(),
                    enabled: conflicts.order.clone(),
                    tracks: Vec::clone(&tracks),
                    off: off.clone(),
                    conflicts: conflicts.pairs.clone(),
                    kept_lists: conflicts.kept_lists.clone(),
                    ..e
                };
                let found = match crashed {
                    true => diagnose(&e),
                    false => stalled(&e),
                };
                if !crashed && found.is_none() {
                    continue;
                }
                let found = found.map_or_else(
                    || "-".to_owned(),
                    |d| format!("{}\t{}\t{}", d.id, d.title, d.text()),
                );
                println!(
                    "KP\t{}\t{}\t{found}",
                    case.file_name().unwrap().to_string_lossy(),
                    session.file_name().unwrap().to_string_lossy()
                );
            }
        }
    }

    const APPLY_FAILED: &str = "applying swofty.classic-seasons: eaid://ui_package/databases/overrides.uidb: \
                                overrides.Contents.teams.Contents.mp_team_select_logos.Contents: no entry \
                                [Name=vanamersfoort_26]";

    fn classic_seasons() -> BTreeMap<String, String> {
        BTreeMap::from([("swofty.classic-seasons".to_owned(), "Classic Seasons".to_owned())])
    }

    #[test]
    fn game_versions_compare_by_number() {
        assert_eq!(older_game("1.0.138.5601", LATEST_GAME_VERSION), Some(true));
        assert_eq!(older_game("1.0.136.6820", "1.0.141.7865"), Some(true));
        assert_eq!(older_game("1.0.141.7865", LATEST_GAME_VERSION), Some(true));
        assert_eq!(older_game("1.0.145.2472", LATEST_GAME_VERSION), Some(false));
        assert_eq!(older_game("1.0.146.1", LATEST_GAME_VERSION), Some(false));
        assert_eq!(older_game("1.0.141.10000", "1.0.141.7865"), Some(false));
        assert_eq!(older_game("1.0.99.9999", "1.0.141.7865"), Some(true));
        assert_eq!(older_game(" 1.0.137.2811 ", LATEST_GAME_VERSION), Some(true));
        for unknown in [
            "",
            "1.0.138",
            "1.0.138.5601.2",
            "1.0.x.5601",
            "25251167",
            "1..138.5601",
            "1.0.+8.5601",
        ] {
            assert_eq!(older_game(unknown, LATEST_GAME_VERSION), None, "{unknown}");
        }
    }

    #[test]
    fn an_addon_that_does_not_apply_to_an_old_game_names_the_update() {
        let d = outdated_game(APPLY_FAILED, Some("1.0.136.6820"), &classic_seasons()).unwrap();
        assert_eq!(d.id, "outdated_game");
        assert_eq!(d.title, "Classic Seasons needs the latest F1 25");
        assert_eq!(
            d.text(),
            "Classic Seasons needs F1 25 1.0.145.2472, the latest update. Your F1 25 is 1.0.136.6820. Update F1 \
             25 in Steam or the EA app, then press Play again."
        );
        let wrapped = format!("the wrapper exited (exit code: 1) before the game started: {APPLY_FAILED}");
        let d = outdated_game(&wrapped, Some("1.0.138.5601"), &BTreeMap::new()).unwrap();
        assert!(
            d.what
                .starts_with("swofty.classic-seasons needs F1 25 1.0.145.2472"),
            "{}",
            d.what
        );
        let item = APPLY_FAILED.replace("classic-seasons:", "classic-seasons (2018):");
        let d = outdated_game(&item, Some("1.0.137.2811"), &classic_seasons()).unwrap();
        assert!(d.what.starts_with("Classic Seasons needs"), "{}", d.what);
    }

    #[test]
    fn other_games_and_other_errors_keep_their_own_message() {
        let names = classic_seasons();
        assert_eq!(
            outdated_game(APPLY_FAILED, Some(LATEST_GAME_VERSION), &names),
            None
        );
        assert_eq!(outdated_game(APPLY_FAILED, Some("1.0.146.1"), &names), None);
        assert_eq!(outdated_game(APPLY_FAILED, None, &names), None);
        assert_eq!(outdated_game(APPLY_FAILED, Some("unknown"), &names), None);
        for other in [
            "version.dll already exists in the game folder; run swofty restore",
            "building profile default: reading x: missing",
            "downloading the offline play extension: offline play 0.3.11: the download was blocked",
            "applying : no entry",
        ] {
            assert_eq!(
                outdated_game(other, Some("1.0.136.6820"), &names),
                None,
                "{other}"
            );
        }
    }

    #[test]
    fn an_older_game_is_told_to_update_before_play() {
        let d = outdated_exe(Some("1.0.138.5601")).unwrap();
        assert_eq!(d.id, "outdated_exe");
        assert!(d.what.starts_with("Your F1 25 is 1.0.138.5601."), "{}", d.what);
        assert!(
            d.text().contains("Update F1 25 in Steam or the EA app"),
            "{}",
            d.text()
        );
        assert!(d.text().contains("steam://validate/3059520"), "{}", d.text());
        let d = outdated_exe(Some("1.0.141.7865")).unwrap();
        assert!(
            d.what
                .contains("Mods are made for 1.0.145.2472, the latest update"),
            "{}",
            d.what
        );
        assert_eq!(outdated_exe(Some(LATEST_GAME_VERSION)), None);
        assert_eq!(outdated_exe(Some("1.0.146.1")), None);
        assert_eq!(outdated_exe(Some("unknown")), None);
        assert_eq!(outdated_exe(None), None);
    }

    #[test]
    fn modules_the_offline_hook_lists_are_read() {
        let log = "2026-10-25 19:18:28.000 [1] [offline] module: RTSSHooks64.dll @ 0x7dd50000\n";
        assert_eq!(loaded_modules(log), ["RTSSHooks64.dll"]);
    }
}

#[cfg(test)]
mod package_ids_tests {
    use super::*;

    fn data(name: &str, ids: &str, rows: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-pkgids-{}-{name}", std::process::id()));
        std::fs::create_dir_all(dir.join("registry")).unwrap();
        std::fs::write(dir.join("registry/ids.json"), ids).unwrap();
        std::fs::write(dir.join("registry/row-ids.json"), rows).unwrap();
        dir
    }

    #[test]
    fn a_nearly_full_table_names_the_ids_addons_that_are_gone_hold() {
        let records: Vec<String> = (1..=250).map(|i| format!(r#""gone.x/p{i}":{i}"#)).collect();
        let ids = format!(r#"{{"ids":{{"vehicle.package":{{{}}}}}}}"#, records.join(","));
        let rows = r#"{"rows":{"here/entrants.vehicles.vehicle_packages.vehicle_package/a":{"id":254,"from":"new"}}}"#;
        let dir = data("full", &ids, rows);
        let held = PackageIds::read(&dir).unwrap();
        let installed = BTreeSet::from(["here".to_owned()]);
        assert_eq!(held.used(), 251);
        assert_eq!(held.highest(), Some(254));
        assert_eq!(held.orphaned(&installed).get("gone.x"), Some(&250));
        let d = held.diagnosis(&installed, &BTreeMap::new()).unwrap();
        assert_eq!(d.id, "package_ids_full");
        assert!(
            d.what
                .contains("251 of 254 used, highest 254, 250 held by addons not installed")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_table_with_room_has_no_diagnosis() {
        let dir = data("room", r#"{"ids":{"vehicle.package":{"a/p":{"id":5}}}}"#, "{}");
        let held = PackageIds::read(&dir).unwrap();
        assert!(held.diagnosis(&BTreeSet::new(), &BTreeMap::new()).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
