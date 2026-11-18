use std::path::{Path, PathBuf};

use crate::vdf::{self, Vdf};
use crate::{GAME_EXE, STEAM_APP_ID};

#[derive(Debug, Clone, PartialEq)]
pub struct GameInstall {
    pub steam_root: PathBuf,
    pub library: PathBuf,
    pub dir: PathBuf,
    pub build_id: Option<u64>,
    pub compatdata: Option<PathBuf>,
}

impl GameInstall {
    pub fn exe(&self) -> PathBuf {
        self.dir.join(GAME_EXE)
    }

    pub fn prefix(&self) -> Option<PathBuf> {
        self.compatdata.as_ref().map(|c| c.join("pfx"))
    }
}

/// An explicit isolation root suppresses registry, default-location and drive scans.
/// This is useful for sandboxed tools and tests that must never inspect host saves.
fn isolated_root() -> Option<PathBuf> {
    std::env::var_os("SWOFTY_STEAM_ROOT_ONLY").map(PathBuf::from)
}

pub fn steam_roots() -> Vec<PathBuf> {
    if let Some(root) = isolated_root() {
        return if root.join("steamapps").is_dir() {
            vec![root]
        } else {
            Vec::new()
        };
    }
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("SWOFTY_STEAM_ROOT") {
        out.push(dir.into());
    }
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(p) = std::env::var_os(var) {
                out.push(PathBuf::from(p).join("Steam"));
            }
        }
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        out.push(home.join(".steam/steam"));
        out.push(home.join(".local/share/Steam"));
        out.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        out.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
    }
    out.extend(registered_roots_cached());
    let mut seen = Vec::new();
    out.retain(|p| {
        let real = p.canonicalize().unwrap_or_else(|_| p.clone());
        let keep = p.join("steamapps").is_dir() && !seen.contains(&real);
        seen.push(real);
        keep
    });
    out
}

const REGISTERED_KEYS: [(&str, &str); 4] = [
    (r"HKCU\Software\Valve\Steam", "SteamPath"),
    (r"HKCU\Software\Valve\Steam", "SteamExe"),
    (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
    (r"HKLM\SOFTWARE\Valve\Steam", "InstallPath"),
];

/// The Steam folders Windows records: the per-user `SteamPath` and `SteamExe`, and the machine-wide
/// `InstallPath`. Steam registers where it is installed, not where the game library is.
pub fn registered_roots(reg: &dyn crate::steam_client::Registry) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut last_key = "";
    let mut values: Vec<(String, String)> = Vec::new();
    for (key, name) in REGISTERED_KEYS {
        if key != last_key {
            values = reg
                .query(key)
                .map(|l| crate::compat::parse_reg_query(&l))
                .unwrap_or_default();
            last_key = key;
        }
        let Some((_, data)) = values
            .iter()
            .find(|(n, d)| n.eq_ignore_ascii_case(name) && !d.trim().is_empty())
        else {
            continue;
        };
        let path = PathBuf::from(data.trim());
        let root = match name {
            "SteamExe" => path.parent().map(Path::to_path_buf),
            _ => Some(path),
        };
        out.extend(root.filter(|r| !out.contains(r)));
    }
    out
}

#[cfg(windows)]
fn registered_roots_cached() -> Vec<PathBuf> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, Vec<PathBuf>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, roots)) = cache.as_ref()
        && at.elapsed() < Duration::from_secs(30)
    {
        return roots.clone();
    }
    let mut roots = registered_roots(&crate::steam_client::RegExe);
    roots.extend(
        crate::process::named_paths("steam.exe")
            .iter()
            .filter_map(|p| p.parent().map(Path::to_path_buf)),
    );
    *cache = Some((Instant::now(), roots.clone()));
    roots
}

#[cfg(not(windows))]
fn registered_roots_cached() -> Vec<PathBuf> {
    Vec::new()
}

/// Where Steam is usually installed on each drive in `drives` (`C:\`, `D:\`, ...), for a Steam that
/// neither the registry nor a running steam.exe revealed.
pub fn drive_roots(drives: &[PathBuf]) -> Vec<PathBuf> {
    drives
        .iter()
        .flat_map(|d| {
            [
                d.join("Program Files (x86)").join("Steam"),
                d.join("Program Files").join("Steam"),
                d.join("Steam"),
            ]
        })
        .filter(|r| r.join("steamapps").is_dir() || r.join("userdata").is_dir())
        .collect()
}

#[cfg(windows)]
fn drives() -> Vec<PathBuf> {
    ('C'..='Z').map(|c| PathBuf::from(format!("{c}:\\"))).collect()
}

#[cfg(not(windows))]
fn drives() -> Vec<PathBuf> {
    Vec::new()
}

fn read_vdf(path: &Path) -> Option<Vdf> {
    vdf::parse(&std::fs::read_to_string(path).ok()?)
}

pub fn libraries(steam_root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_root.to_path_buf()];
    if let Some(v) = read_vdf(&steam_root.join("steamapps/libraryfolders.vdf")) {
        let entries = v.get("libraryfolders").into_iter().flat_map(Vdf::entries);
        for (_, e) in entries {
            if let Some(p) = e.get("path").and_then(Vdf::str) {
                let p = PathBuf::from(p);
                let real = |x: &Path| x.canonicalize().unwrap_or_else(|_| x.to_path_buf());
                if !libs.iter().any(|l| real(l) == real(&p)) {
                    libs.push(p);
                }
            }
        }
    }
    libs
}

pub fn find_game() -> Option<GameInstall> {
    steam_roots().into_iter().find_map(|root| find_game_in(&root))
}

pub fn find_game_in(steam_root: &Path) -> Option<GameInstall> {
    libraries(steam_root).into_iter().find_map(|lib| {
        let steamapps = lib.join("steamapps");
        let acf = read_vdf(&steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf")))?;
        let state = acf.get("AppState")?;
        let dir = steamapps.join("common").join(state.get("installdir")?.str()?);
        let build_id = state
            .get("buildid")
            .and_then(Vdf::str)
            .and_then(|b| b.parse().ok());
        let compat = steamapps.join("compatdata").join(STEAM_APP_ID.to_string());
        Some(GameInstall {
            steam_root: steam_root.to_path_buf(),
            library: lib.clone(),
            dir,
            build_id,
            compatdata: compat.is_dir().then_some(compat),
        })
    })
}

const VALIDATING: u64 = 0x2_0000;
// EAppState bits for a running update, uninstall or backup, and the stages of
// a download: Steam is writing the game's files.
const CHANGING: u64 = 0x100
    | 0x400
    | 0x800
    | 0x1000
    | 0x1_0000
    | 0x4_0000
    | 0x8_0000
    | 0x10_0000
    | 0x20_0000
    | 0x40_0000
    | 0x80_0000;

/// What Steam is doing to the game's files: "verifying" while it validates
/// them, "updating" while it changes them any other way, None when it leaves
/// them alone. Steam keeps the `StateFlags` of the app manifest as they were
/// while it verifies, so a running Steam's content log, which records every
/// state change, is read first.
pub fn busy(install: &GameInstall) -> Option<&'static str> {
    let running = crate::steam_client::status().state != crate::steam_client::State::Stopped;
    running
        .then(|| log_busy(&install.steam_root.join("logs/content_log.txt")))
        .flatten()
        .or_else(|| manifest_busy(install))
}

/// The game's last state in Steam's content log: busy while an update runs,
/// and verifying when the update is a check of the installed files.
fn log_busy(log: &Path) -> Option<&'static str> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(log).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(256 * 1024))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let last = |what: &str| {
        let key = format!("AppID {STEAM_APP_ID} {what} : ");
        text.lines()
            .rev()
            .find_map(|l| l.split_once(key.as_str()).map(|(_, state)| state.to_owned()))
    };
    if !last("state changed")?.contains("Update Running") {
        return None;
    }
    Some(
        if last("App update changed").is_some_and(|u| u.contains("Verifying")) {
            "verifying"
        } else {
            "updating"
        },
    )
}

fn manifest_busy(install: &GameInstall) -> Option<&'static str> {
    let acf = read_vdf(
        &install
            .library
            .join("steamapps")
            .join(format!("appmanifest_{STEAM_APP_ID}.acf")),
    )?;
    let flags: u64 = acf.path(&["AppState", "StateFlags"])?.str()?.parse().ok()?;
    if flags & VALIDATING != 0 {
        Some("verifying")
    } else if flags & CHANGING != 0 {
        Some("updating")
    } else {
        None
    }
}

/// True when `dir` is a Steam copy of the game: it sits in a library's
/// `steamapps/common` next to the app's manifest.
pub fn in_steam_library(dir: &Path) -> bool {
    let shaped = |dir: &Path| {
        let named = |p: Option<&Path>, name: &str| {
            p.and_then(Path::file_name)
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
        };
        let common = dir.parent();
        let steamapps = common.and_then(Path::parent);
        named(common, "common")
            && named(steamapps, "steamapps")
            && steamapps.is_some_and(|s| s.join(format!("appmanifest_{STEAM_APP_ID}.acf")).is_file())
    };
    shaped(dir) || dir.canonicalize().is_ok_and(|real| shaped(&real))
}

pub const STEAMVR_APP_ID: u32 = 250_820;

pub fn steamvr_installed(steam_root: &Path) -> bool {
    libraries(steam_root).iter().any(|lib| {
        lib.join(format!("steamapps/appmanifest_{STEAMVR_APP_ID}.acf"))
            .is_file()
    })
}

pub fn launch_options(steam_root: &Path) -> Vec<(String, String)> {
    let Ok(users) = std::fs::read_dir(steam_root.join("userdata")) else {
        return Vec::new();
    };
    users
        .flatten()
        .filter_map(|u| {
            let cfg = read_vdf(&u.path().join("config/localconfig.vdf"))?;
            let apps = cfg.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])?;
            let opts = apps.get(&STEAM_APP_ID.to_string())?.get("LaunchOptions")?.str()?;
            Some((u.file_name().to_string_lossy().into_owned(), opts.to_owned()))
        })
        .collect()
}

pub fn compat_tool(steam_root: &Path) -> Option<String> {
    let cfg = read_vdf(&steam_root.join("config/config.vdf"))?;
    let map = cfg.path(&[
        "InstallConfigStore",
        "Software",
        "Valve",
        "Steam",
        "CompatToolMapping",
    ])?;
    let name = |key: &str| {
        map.get(key)?
            .get("name")?
            .str()
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
    };
    name(&STEAM_APP_ID.to_string()).or_else(|| name("0"))
}

fn valve_folder_name(tool: &str) -> Option<String> {
    let rest = tool.strip_prefix("proton_")?;
    match rest {
        "experimental" => Some("Proton - Experimental".to_owned()),
        "hotfix" => Some("Proton Hotfix".to_owned()),
        v if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) => Some(format!("Proton {v}.0")),
        _ => None,
    }
}

fn manifest_names(dir: &Path) -> Vec<String> {
    let Some(v) = read_vdf(&dir.join("compatibilitytool.vdf")) else {
        return Vec::new();
    };
    let Some(tools) = v.path(&["compatibilitytools", "compat_tools"]) else {
        return Vec::new();
    };
    tools
        .entries()
        .flat_map(|(k, e)| {
            [
                Some(k.clone()),
                e.get("display_name").and_then(Vdf::str).map(str::to_owned),
            ]
        })
        .flatten()
        .collect()
}

pub struct ProtonSearch {
    pub found: Option<PathBuf>,
    pub searched: Vec<PathBuf>,
}

/// The Proton directory (the one holding `proton`) for the compat tool name
/// Steam records: a folder of that name, a tool whose compatibilitytool.vdf
/// declares the name, or Valve's own folder for an internal name such as
/// `proton_9`. Searches compatibilitytools.d and every library's
/// steamapps/common, with `first` searched ahead of the rest.
pub fn find_proton(steam_root: &Path, first: Option<&Path>, tool: &str) -> ProtonSearch {
    find_proton_in(steam_root, first, tool, &system_compat_dirs())
}

pub fn system_compat_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("STEAM_EXTRA_COMPAT_TOOLS_PATHS")
        .map(|v| {
            std::env::split_paths(&v)
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default();
    for fixed in [
        "/usr/share/steam/compatibilitytools.d",
        "/usr/local/share/steam/compatibilitytools.d",
    ] {
        dirs.push(PathBuf::from(fixed));
    }
    dirs
}

pub fn find_proton_in(
    steam_root: &Path,
    first: Option<&Path>,
    tool: &str,
    system: &[PathBuf],
) -> ProtonSearch {
    let mut homes = vec![steam_root.join("compatibilitytools.d")];
    for dir in system {
        if !homes.contains(dir) {
            homes.push(dir.clone());
        }
    }
    let mut libs: Vec<PathBuf> = first.into_iter().map(Path::to_path_buf).collect();
    libs.extend(libraries(steam_root));
    for lib in libs {
        let common = lib.join("steamapps/common");
        if !homes.contains(&common) {
            homes.push(common);
        }
    }
    let valve = valve_folder_name(tool);
    let matches = |dir: &Path| {
        let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned());
        let named = |want: &str| folder.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(want));
        named(tool) || valve.as_deref().is_some_and(named) || manifest_names(dir).iter().any(|n| n == tool)
    };
    let mut found = None;
    for home in &homes {
        let Ok(rd) = std::fs::read_dir(home) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        dirs.sort();
        if let Some(hit) = dirs
            .into_iter()
            .find(|d| d.join("proton").is_file() && matches(d))
        {
            found = Some(hit);
            break;
        }
    }
    ProtonSearch {
        found,
        searched: homes,
    }
}

const STEAMID64_BASE: u64 = 76_561_197_960_265_728;

fn has_files(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|d| d.flatten().any(|e| e.file_type().is_ok_and(|t| t.is_file())))
}

/// The game's Steam Cloud save folder, `userdata/<steamid3>/<app>/remote`: for the
/// most recent user in loginusers.vdf, else the user whose folder changed last.
pub fn save_dir(steam_root: &Path) -> Option<PathBuf> {
    save_dir_with(steam_root, None, &mut Vec::new())
}

/// [`save_dir`] that prefers `active_user` (Steam's `ActiveUser`, 0 for nobody) and then the
/// `MostRecent` account in loginusers.vdf, then the other accounts by sign-in time and folder
/// change time, and takes the first whose folder holds files, else the first that exists. `tried`
/// gets one line per folder looked at and why it was rejected.
pub fn save_dir_with(
    steam_root: &Path,
    active_user: Option<u32>,
    tried: &mut Vec<String>,
) -> Option<PathBuf> {
    let userdata = steam_root.join("userdata");
    let mut order: Vec<(String, &'static str)> = Vec::new();
    let mut push = |id3: String, why: &'static str| {
        if !order.iter().any(|(i, _)| *i == id3) {
            order.push((id3, why));
        }
    };
    if let Some(user) = active_user.filter(|u| *u != 0) {
        push(user.to_string(), "ActiveUser");
    }
    if let Some(v) = read_vdf(&steam_root.join("config/loginusers.vdf"))
        && let Some(users) = v.get("users")
    {
        let stamp = |e: &Vdf| {
            e.get("Timestamp")
                .and_then(Vdf::str)
                .and_then(|t| t.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let mut all: Vec<_> = users
            .entries()
            .filter_map(|(id, e)| {
                let id3 = id.parse::<u64>().ok()?.checked_sub(STEAMID64_BASE)?;
                let most_recent = e.get("MostRecent").and_then(Vdf::str) == Some("1");
                Some((most_recent, stamp(e), id3.to_string()))
            })
            .collect();
        all.sort_by_key(|u| std::cmp::Reverse((u.0, u.1)));
        for (most_recent, _, id3) in all {
            push(
                id3,
                match most_recent {
                    true => "loginusers.vdf MostRecent",
                    false => "loginusers.vdf",
                },
            );
        }
    } else {
        tried.push(format!(
            "{}: no readable loginusers.vdf",
            steam_root.join("config/loginusers.vdf").display()
        ));
    }
    let mut folders: Vec<(Option<std::time::SystemTime>, String)> = std::fs::read_dir(&userdata)
        .map(|d| {
            d.flatten()
                .filter_map(|u| {
                    let name = u.file_name().to_str()?.to_owned();
                    let remote = u.path().join(STEAM_APP_ID.to_string()).join("remote");
                    Some((std::fs::metadata(remote).and_then(|m| m.modified()).ok(), name))
                })
                .collect()
        })
        .unwrap_or_default();
    if folders.is_empty() {
        tried.push(format!("{}: no userdata accounts", userdata.display()));
    }
    folders.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, id3) in folders {
        push(id3, "newest userdata folder");
    }
    let mut first_existing = None;
    for (id3, why) in order {
        let dir = userdata.join(&id3).join(STEAM_APP_ID.to_string()).join("remote");
        if !dir.is_dir() {
            tried.push(format!("{} ({why}): rejected, no such folder", dir.display()));
        } else if has_files(&dir) {
            tried.push(format!("{} ({why}): chosen, it holds save files", dir.display()));
            return Some(dir);
        } else {
            tried.push(format!(
                "{} ({why}): rejected, the folder is empty",
                dir.display()
            ));
            first_existing.get_or_insert(dir);
        }
    }
    first_existing
}

/// The Steam install whose `userdata` holds the saves of the game at `game`
/// (`<library>/steamapps/common/<game>`): the library itself when it is a Steam root,
/// else the known root whose `libraryfolders.vdf` lists the library.
pub fn root_of_game_in(game: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let common = game.parent()?;
    let steamapps = common.parent()?;
    let named = |p: &Path, name: &str| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
    };
    if !named(common, "common") || !named(steamapps, "steamapps") {
        return None;
    }
    let library = steamapps.parent()?;
    let real = |x: &Path| x.canonicalize().unwrap_or_else(|_| x.to_path_buf());
    let is_root = |r: &Path| r.join("userdata").is_dir() || r.join("config/loginusers.vdf").is_file();
    if is_root(library) {
        return Some(library.to_path_buf());
    }
    roots
        .iter()
        .find(|r| libraries(r).iter().any(|l| real(l) == real(library)))
        .cloned()
}

pub fn root_of_game(game: &Path) -> Option<PathBuf> {
    root_of_game_in(game, &steam_roots())
}

/// save_dir for the Steam install that holds the game, then for every other one.
pub fn find_save_dir() -> Option<PathBuf> {
    search_save(find_game().map(|g| g.dir).as_deref()).dir
}

/// The Steam Cloud save folder found, and what was looked at on the way.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SaveSearch {
    pub dir: Option<PathBuf>,
    pub tried: Vec<String>,
}

/// The save folder for the game in `game`. The Steam root comes from the library's own Steam
/// install, never from the game's path: `userdata` is always under the root, and a library on
/// another drive has none. Roots come from the library's `libraryfolders.vdf` owner, then the
/// default folders, the registry and a running steam.exe, and last from the usual folders on every
/// drive.
pub fn search_save(game: Option<&Path>) -> SaveSearch {
    if isolated_root().is_some() {
        return search_save_in(game, &steam_roots(), &[], None);
    }
    search_save_in(game, &steam_roots(), &drive_roots(&drives()), active_user())
}

pub fn search_save_in(
    game: Option<&Path>,
    roots: &[PathBuf],
    scanned: &[PathBuf],
    active_user: Option<u32>,
) -> SaveSearch {
    let mut tried = Vec::new();
    let real = |x: &Path| x.canonicalize().unwrap_or_else(|_| x.to_path_buf());
    let mut order: Vec<(PathBuf, &str)> = Vec::new();
    let add = |order: &mut Vec<(PathBuf, &str)>, root: PathBuf, why: &'static str| {
        if !order.iter().any(|(r, _)| real(r) == real(&root)) {
            order.push((root, why));
        }
    };
    match game {
        Some(game) => match root_of_game_in(game, roots) {
            Some(root) => add(
                &mut order,
                root,
                "the Steam install that lists the game's library",
            ),
            None => tried.push(format!(
                "{}: no known Steam root lists this library in libraryfolders.vdf",
                game.display()
            )),
        },
        None => tried.push("no game folder to find the library of".to_owned()),
    }
    for root in roots {
        add(&mut order, root.clone(), "a known Steam install");
    }
    let mut empty: Option<PathBuf> = None;
    let mut look = |order: &[(PathBuf, &str)], tried: &mut Vec<String>| {
        for (root, why) in order {
            tried.push(format!("Steam root {} ({why})", root.display()));
            if let Some(dir) = save_dir_with(root, active_user, tried) {
                if has_files(&dir) {
                    return Some(dir);
                }
                empty.get_or_insert(dir);
            }
        }
        None
    };
    let mut found = look(&order, &mut tried);
    if found.is_none() {
        let mut more = Vec::new();
        for root in scanned
            .iter()
            .filter(|r| !order.iter().any(|(o, _)| real(o) == real(r)))
        {
            add(&mut more, root.clone(), "found by looking on every drive");
        }
        if more.is_empty() && order.is_empty() {
            tried.push("no Steam install found in the registry, the default folders or on any drive".into());
        }
        found = look(&more, &mut tried);
    }
    SaveSearch {
        dir: found.or(empty),
        tried,
    }
}

#[cfg(windows)]
pub fn active_user() -> Option<u32> {
    use crate::steam_client::Registry;
    let out = crate::steam_client::RegExe.query(crate::steam_client::ACTIVE_PROCESS_KEY)?;
    crate::compat::reg_dword(&out, "ActiveUser").filter(|u| *u != 0)
}

#[cfg(not(windows))]
pub fn active_user() -> Option<u32> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    [".steam/registry.vdf", ".local/share/Steam/registry.vdf"]
        .iter()
        .find_map(|f| {
            read_vdf(&home.join(f))?
                .path(&[
                    "Registry",
                    "HKCU",
                    "Software",
                    "Valve",
                    "Steam",
                    "ActiveProcess",
                    "ActiveUser",
                ])?
                .str()?
                .trim()
                .parse::<u32>()
                .ok()
        })
        .filter(|u| *u != 0)
}

pub fn wrapper_installed(launch_options: &str) -> bool {
    launch_options.contains("swofty")
        && launch_options.contains("run")
        && launch_options.contains("%command%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vdf_path(p: &Path) -> String {
        p.display().to_string().replace('\\', "\\\\")
    }

    fn proton_tree(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("swofty-proton-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        root
    }

    fn put_proton(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("proton"), "").unwrap();
    }

    fn mapping(root: &Path, body: &str) {
        std::fs::write(
            root.join("config/config.vdf"),
            format!(
                "\"InstallConfigStore\" {{ \"Software\" {{ \"Valve\" {{ \"Steam\" {{ \"CompatToolMapping\" {{ {body} }} }} }} }} }}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn compat_tool_falls_back_to_global_default() {
        let root = proton_tree("global");
        mapping(&root, "\"0\" { \"name\" \"proton_9\" }");
        assert_eq!(compat_tool(&root).as_deref(), Some("proton_9"));
        mapping(
            &root,
            "\"0\" { \"name\" \"proton_9\" } \"3059520\" { \"name\" \"GE-Proton9-1\" }",
        );
        assert_eq!(compat_tool(&root).as_deref(), Some("GE-Proton9-1"));
        mapping(&root, "\"3059520\" { \"name\" \"\" }");
        assert_eq!(compat_tool(&root), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_valve_proton_by_internal_name_in_main_library() {
        let root = proton_tree("valve");
        put_proton(&root.join("steamapps/common/Proton 9.0"));
        put_proton(&root.join("steamapps/common/Proton - Experimental"));
        let other = root.join("drive2");
        std::fs::create_dir_all(other.join("steamapps/common/F1 25")).unwrap();
        let hit = find_proton(&root, Some(&other), "proton_9");
        assert_eq!(hit.found, Some(root.join("steamapps/common/Proton 9.0")));
        let hit = find_proton(&root, Some(&other), "proton_experimental");
        assert_eq!(
            hit.found,
            Some(root.join("steamapps/common/Proton - Experimental"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_proton_in_listed_library_and_by_manifest_name() {
        let root = proton_tree("libs");
        let lib = std::env::temp_dir().join(format!("swofty-proton-libs2-{}", std::process::id()));
        put_proton(&lib.join("steamapps/common/Proton 10.0"));
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{ \"1\" {{ \"path\" \"{}\" }} }}",
                vdf_path(&lib)
            ),
        )
        .unwrap();
        let custom = root.join("compatibilitytools.d/some-folder");
        put_proton(&custom);
        std::fs::write(
            custom.join("compatibilitytool.vdf"),
            "\"compatibilitytools\" { \"compat_tools\" { \"my-tool\" { \"install_path\" \".\" \"display_name\" \"My Tool\" } } }",
        )
        .unwrap();
        assert_eq!(
            find_proton(&root, None, "proton_10").found,
            Some(lib.join("steamapps/common/Proton 10.0"))
        );
        assert_eq!(find_proton(&root, None, "my-tool").found, Some(custom.clone()));
        assert_eq!(find_proton(&root, None, "My Tool").found, Some(custom));
        let miss = find_proton(&root, None, "nothing");
        assert!(miss.found.is_none());
        assert!(miss.searched.contains(&lib.join("steamapps/common")));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&lib);
    }

    #[test]
    fn finds_proton_in_system_compat_dirs_after_user_dir() {
        let root = proton_tree("system");
        let sys = std::env::temp_dir().join(format!("swofty-proton-sys-{}", std::process::id()));
        put_proton(&sys.join("proton-ge-custom"));
        put_proton(&root.join("steamapps/common/proton-ge-custom"));
        let sys_list = [sys.clone()];
        let hit = find_proton_in(&root, None, "proton-ge-custom", &sys_list);
        assert_eq!(hit.found, Some(sys.join("proton-ge-custom")));
        put_proton(&root.join("compatibilitytools.d/proton-ge-custom"));
        let hit = find_proton_in(&root, None, "proton-ge-custom", &sys_list);
        assert_eq!(
            hit.found,
            Some(root.join("compatibilitytools.d/proton-ge-custom"))
        );
        let miss = find_proton_in(&root, None, "nothing", &sys_list);
        assert!(miss.found.is_none());
        let pos = |p: &Path| miss.searched.iter().position(|s| s == p).unwrap();
        assert!(pos(&sys) < pos(&root.join("steamapps/common")));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&sys);
    }

    #[test]
    fn exact_folder_name_still_resolves() {
        let root = proton_tree("exact");
        put_proton(&root.join("compatibilitytools.d/proton-cachyos-10.0-20260425-slr-x86_64"));
        assert_eq!(
            find_proton(&root, None, "proton-cachyos-10.0-20260425-slr-x86_64").found,
            Some(root.join("compatibilitytools.d/proton-cachyos-10.0-20260425-slr-x86_64"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_game_in_secondary_library() {
        let root = std::env::temp_dir().join(format!("swofty-steam-{}", std::process::id()));
        let lib = root.join("lib2");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/common/F1 25")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/compatdata/3059520/pfx")).unwrap();
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "0" {{ "path" "{}" }} "1" {{ "path" "{}" }} }}"#,
                vdf_path(&root),
                vdf_path(&lib)
            ),
        )
        .unwrap();
        std::fs::write(
            lib.join("steamapps/appmanifest_3059520.acf"),
            r#""AppState" { "appid" "3059520" "installdir" "F1 25" "buildid" "25251167" }"#,
        )
        .unwrap();
        let g = find_game_in(&root).unwrap();
        assert_eq!(g.dir, lib.join("steamapps/common/F1 25"));
        assert_eq!(g.build_id, Some(25_251_167));
        assert_eq!(busy(&g), None);
        for (flags, state) in [
            ("4", None),
            ("6", None),
            ("131076", Some("verifying")),
            ("1030", Some("updating")),
        ] {
            std::fs::write(
                lib.join("steamapps/appmanifest_3059520.acf"),
                format!(r#""AppState" {{ "appid" "3059520" "installdir" "F1 25" "StateFlags" "{flags}" }}"#),
            )
            .unwrap();
            assert_eq!(busy(&g), state, "{flags}");
        }
        std::fs::write(
            lib.join("steamapps/appmanifest_3059520.acf"),
            r#""AppState" { "appid" "3059520" "installdir" "F1 25" "buildid" "25251167" }"#,
        )
        .unwrap();
        assert!(g.prefix().unwrap().ends_with("compatdata/3059520/pfx"));
        assert!(in_steam_library(&g.dir));
        assert!(!in_steam_library(&root.join("steamapps/common/F1 25")));
        assert!(!in_steam_library(&lib));
        assert!(!steamvr_installed(&root));
        std::fs::write(
            lib.join("steamapps/appmanifest_250820.acf"),
            r#""AppState" { "appid" "250820" "installdir" "SteamVR" }"#,
        )
        .unwrap();
        assert!(steamvr_installed(&root));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn finds_the_steam_root_of_a_game_on_another_drive_with_spaces() {
        let base = std::env::temp_dir().join(format!("swofty-root-{}", std::process::id()));
        let root = base.join("Bagas").join("New Folder").join("Apps").join("Steam");
        std::fs::create_dir_all(root.join("steamapps/common/F1 25")).unwrap();
        std::fs::create_dir_all(root.join("userdata/1163457660/3059520/remote")).unwrap();
        let game = root.join("steamapps").join("common").join("F1 25");
        assert_eq!(root_of_game_in(&game, &[]), Some(root.clone()));
        assert_eq!(
            save_dir(&root),
            Some(root.join("userdata/1163457660/3059520/remote"))
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn finds_the_steam_root_of_a_game_in_a_secondary_library() {
        let base = std::env::temp_dir().join(format!("swofty-root2-{}", std::process::id()));
        let root = base.join("Steam Main");
        let lib = base.join("Games Lib").join("SteamLibrary");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(root.join("userdata/5")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/common/F1 25")).unwrap();
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(r#""libraryfolders" {{ "1" {{ "path" "{}" }} }}"#, vdf_path(&lib)),
        )
        .unwrap();
        let game = lib.join("steamapps").join("common").join("F1 25");
        assert_eq!(
            root_of_game_in(&game, std::slice::from_ref(&root)),
            Some(root.clone())
        );
        assert_eq!(root_of_game_in(&game, &[]), None);
        assert_eq!(root_of_game_in(&base.join("loose").join("F1 25"), &[root]), None);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_windows_shaped_game_path_without_a_steam_root_finds_none() {
        let game = Path::new(r"D:\Bagas\New Folder\Apps\Steam\steamapps\common\F1 25");
        assert_eq!(root_of_game_in(game, &[]), None);
    }

    #[test]
    fn finds_the_most_recent_users_save_folder() {
        let root = std::env::temp_dir().join(format!("swofty-saves-{}", std::process::id()));
        for id3 in ["11", "22"] {
            std::fs::create_dir_all(root.join(format!("userdata/{id3}/3059520/remote"))).unwrap();
        }
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(
            root.join("config/loginusers.vdf"),
            format!(
                r#""users" {{ "{}" {{ "AccountName" "a" "MostRecent" "0" "Timestamp" "900" }}
                   "{}" {{ "AccountName" "b" "MostRecent" "1" "Timestamp" "100" }} }}"#,
                STEAMID64_BASE + 11,
                STEAMID64_BASE + 22
            ),
        )
        .unwrap();
        assert_eq!(save_dir(&root), Some(root.join("userdata/22/3059520/remote")));
        std::fs::remove_file(root.join("config/loginusers.vdf")).unwrap();
        assert!(save_dir(&root).is_some_and(|d| d.ends_with("3059520/remote")));
        std::fs::remove_dir_all(root.join("userdata")).unwrap();
        assert_eq!(save_dir(&root), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reads_what_steam_does_to_the_game_from_its_content_log() {
        let dir = std::env::temp_dir().join(format!("swofty-content-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("content_log.txt");
        let at = |lines: &[&str]| {
            std::fs::write(&log, lines.join("\n") + "\n").unwrap();
            log_busy(&log)
        };
        let shader = [
            "[2026-10-24 12:48:43] AppID 3059520 Shader update changed : Running Update,Reconfiguring,",
            "[2026-10-24 12:48:45] AppID 3059520 state changed : Fully Installed,Update Queued,",
            "[2026-10-24 12:48:45] AppID 3059520 state changed : Fully Installed,",
        ];
        assert_eq!(at(&shader), None);
        let verify = [
            "[2026-10-24 15:23:18] Start validating appID 3059520 on BuildID 25251167",
            "[2026-10-24 15:23:18] AppID 3059520 state changed : Fully Installed,Update Queued,Update Running,",
            "[2026-10-24 15:23:18] AppID 228980 state changed : Fully Installed,Update Running,",
            "[2026-10-24 15:23:18] AppID 3059520 App update changed : Running Update,Verifying Installed,",
        ];
        assert_eq!(at(&verify), Some("verifying"));
        let mut done = verify.to_vec();
        done.push("[2026-10-24 15:31:02] AppID 3059520 App update changed : None");
        done.push("[2026-10-24 15:31:02] AppID 3059520 state changed : Fully Installed,");
        assert_eq!(at(&done), None);
        let mut update = done.clone();
        update.push("[2026-10-24 16:00:00] AppID 3059520 state changed : Fully Installed,Update Running,");
        update.push("[2026-10-24 16:00:01] AppID 3059520 App update changed : Running Update,Downloading,");
        assert_eq!(at(&update), Some("updating"));
        assert_eq!(log_busy(&dir.join("missing.txt")), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    struct FakeReg(Vec<(&'static str, String)>);

    impl crate::steam_client::Registry for FakeReg {
        fn query(&self, key: &str) -> Option<String> {
            self.0.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
        }
    }

    fn fixture(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("swofty-lib-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        base
    }

    fn save(root: &Path, id3: &str, files: &[&str]) {
        let remote = root.join("userdata").join(id3).join("3059520/remote");
        std::fs::create_dir_all(&remote).unwrap();
        for f in files {
            std::fs::write(remote.join(f), b"x").unwrap();
        }
    }

    fn logins(root: &Path, users: &[(u64, &str, u64)]) {
        std::fs::create_dir_all(root.join("config")).unwrap();
        let body: String = users
            .iter()
            .map(|(id3, recent, stamp)| {
                format!(
                    r#""{}" {{ "MostRecent" "{recent}" "Timestamp" "{stamp}" }} "#,
                    STEAMID64_BASE + id3
                )
            })
            .collect();
        std::fs::write(
            root.join("config/loginusers.vdf"),
            format!(r#""users" {{ {body} }}"#),
        )
        .unwrap();
    }

    fn library_on_other_drive(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let root = base.join("C").join("Program Files (x86)").join("Steamé Ünïcode");
        let lib = base.join("D").join("SteamLibrary");
        let game = lib.join("steamapps/common/F1 25");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "0" {{ "path" "{}" }} "1" {{ "path" "{}" }} }}"#,
                root.display().to_string().replace('\\', "\\\\"),
                lib.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        (root, lib, game)
    }

    #[test]
    fn a_library_on_another_drive_finds_the_save_under_the_steam_root() {
        let base = fixture("drive");
        let (root, lib, game) = library_on_other_drive(&base);
        assert!(!lib.join("userdata").exists());
        logins(&root, &[(11, "0", 900), (22, "1", 100)]);
        save(&root, "11", &["a.sav"]);
        save(&root, "22", &["ProfileSave.dat"]);
        save(&root, "33", &["old.sav"]);
        let found = search_save_in(Some(&game), std::slice::from_ref(&root), &[], Some(0));
        assert_eq!(found.dir, Some(root.join("userdata/22/3059520/remote")));
        let log = found.tried.join("\n");
        assert!(
            log.contains("the Steam install that lists the game's library"),
            "{log}"
        );
        assert!(log.contains("loginusers.vdf MostRecent): chosen"), "{log}");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn an_empty_most_recent_folder_is_rejected_for_one_with_saves() {
        let base = fixture("empty");
        let root = base.join("Steam");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        logins(&root, &[(11, "1", 900), (22, "0", 100)]);
        save(&root, "11", &[]);
        save(&root, "22", &["ProfileSave.dat"]);
        let mut tried = Vec::new();
        assert_eq!(
            save_dir_with(&root, Some(0), &mut tried),
            Some(root.join("userdata/22/3059520/remote"))
        );
        let log = tried.join("\n");
        let empty = root.join("userdata").join("11").join("3059520").join("remote");
        assert!(
            log.contains(&empty.display().to_string()) && log.contains("rejected, the folder is empty"),
            "{log}"
        );
        assert_eq!(
            save_dir_with(&root, Some(11), &mut Vec::new()),
            Some(root.join("userdata/22/3059520/remote")),
            "ActiveUser first, but only when its folder holds saves"
        );
        save(&root, "22", &[]);
        std::fs::remove_file(root.join("userdata/22/3059520/remote/ProfileSave.dat")).unwrap();
        assert_eq!(
            save_dir_with(&root, Some(22), &mut Vec::new()),
            Some(root.join("userdata/22/3059520/remote")),
            "no folder holds saves: the first that exists"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn active_user_leads_over_loginusers() {
        let base = fixture("active");
        let root = base.join("Steam");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        logins(&root, &[(11, "1", 900)]);
        save(&root, "11", &["a.sav"]);
        save(&root, "22", &["b.sav"]);
        assert_eq!(
            save_dir_with(&root, Some(22), &mut Vec::new()),
            Some(root.join("userdata/22/3059520/remote"))
        );
        assert_eq!(
            save_dir_with(&root, Some(0), &mut Vec::new()),
            Some(root.join("userdata/11/3059520/remote")),
            "ActiveUser 0 means nobody is signed in"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn the_registry_names_the_steam_root_not_the_library() {
        let reg = FakeReg(vec![
            (
                r"HKCU\Software\Valve\Steam",
                "    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n    SteamExe    REG_SZ    c:/program files (x86)/steam/steam.exe\r\n".to_owned(),
            ),
            (
                r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam",
                "    InstallPath    REG_SZ    C:\\Program Files (x86)\\Steam\r\n".to_owned(),
            ),
        ]);
        assert_eq!(
            registered_roots(&reg),
            [
                PathBuf::from("c:/program files (x86)/steam"),
                PathBuf::from("C:\\Program Files (x86)\\Steam")
            ]
        );
        assert!(registered_roots(&FakeReg(Vec::new())).is_empty());
    }

    #[test]
    fn a_steam_nobody_registered_is_found_by_looking_on_the_drives() {
        let base = fixture("scan");
        let (root, _, game) = library_on_other_drive(&base);
        let odd = base.join("E").join("Steam");
        std::fs::create_dir_all(odd.join("steamapps")).unwrap();
        save(&odd, "44", &["ProfileSave.dat"]);
        assert_eq!(
            drive_roots(&[base.join("E"), base.join("F")]),
            std::slice::from_ref(&odd)
        );
        let found = search_save_in(Some(&game), &[], &drive_roots(&[base.join("E")]), None);
        assert_eq!(found.dir, Some(odd.join("userdata/44/3059520/remote")));
        assert!(
            found
                .tried
                .join("\n")
                .contains("no known Steam root lists this library")
        );
        let none = search_save_in(Some(&game), &[], &[], None);
        assert_eq!(none.dir, None);
        assert!(
            none.tried.join("\n").contains("no Steam install found"),
            "{:?}",
            none.tried
        );
        let _ = root;
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn detects_wrapper() {
        assert!(wrapper_installed(
            "~/.local/share/swofty/bin/swofty run -- %command%"
        ));
        assert!(!wrapper_installed("echo %command%"));
    }
}
