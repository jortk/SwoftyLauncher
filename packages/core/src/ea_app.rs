//! The EA app's copy of F1 25: where Windows has it installed, the link that
//! asks the EA app to launch it, and the command that hands the link over.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::GAME_EXE;
use crate::compat::parse_reg_query;
use crate::session::COMPAT_LAYER;

pub const MARKER: &str = "__Installer/installerdata.xml";
pub const PROCESS: &str = "EADesktop.exe";
pub const LAUNCHER_EXE: &str = "EALauncher.exe";

/// The game's own installerdata.xml names the first key as its install check.
pub const INSTALL_KEYS: &[&str] = &[
    r"HKLM\SOFTWARE\Codemasters\F1_25",
    r"HKLM\SOFTWARE\WOW6432Node\Codemasters\F1_25",
    r"HKLM\SOFTWARE\EA Games\F1 25",
    r"HKLM\SOFTWARE\WOW6432Node\EA Games\F1 25",
    r"HKLM\SOFTWARE\EA Sports\F1 25",
    r"HKLM\SOFTWARE\WOW6432Node\EA Sports\F1 25",
];
pub const UNINSTALL_KEYS: &[&str] = &[
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];
pub const PROTOCOL_KEY: &str = r"HKCR\origin2\shell\open\command";
pub const DESKTOP_KEYS: &[&str] = &[
    r"HKLM\SOFTWARE\Electronic Arts\EA Desktop",
    r"HKLM\SOFTWARE\WOW6432Node\Electronic Arts\EA Desktop",
];

pub fn is_install(dir: &Path) -> bool {
    dir.join(GAME_EXE).is_file() && dir.join(MARKER).is_file()
}

/// The install folders a `reg query` lists, by any of the value names EA
/// and Windows use for one.
pub fn install_dirs(reg_out: &str) -> Vec<PathBuf> {
    parse_reg_query(reg_out)
        .into_iter()
        .filter(|(name, _)| {
            ["Install Dir", "InstallDir", "InstallLocation"]
                .iter()
                .any(|n| name.eq_ignore_ascii_case(n))
        })
        .map(|(_, data)| data.trim().trim_matches('"').to_owned())
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// `reg query <key> /s` output split into each subkey's values.
pub fn reg_tree(out: &str) -> Vec<(String, String)> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    for line in out.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with("HKEY_") {
            blocks.push((line.to_owned(), String::new()));
        } else if let Some((_, values)) = blocks.last_mut() {
            values.push_str(line);
            values.push('\n');
        }
    }
    blocks
}

/// Every F1 25 install folder Windows records for the EA app, checked with
/// `is_install`. `reg` runs `reg query` with the given arguments.
pub fn find_installs(
    reg: impl Fn(&[&str]) -> Option<String>,
    defaults: &[PathBuf],
    is_install: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let keyed = INSTALL_KEYS
        .iter()
        .filter_map(|k| reg(&[k]))
        .flat_map(|out| install_dirs(&out));
    let uninstall = UNINSTALL_KEYS
        .iter()
        .filter_map(|k| reg(&[k, "/s"]))
        .flat_map(|out| reg_tree(&out))
        .flat_map(|(_, values)| install_dirs(&values));
    let folders = defaults.iter().flat_map(|d| {
        std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
    });
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in keyed.chain(uninstall).chain(folders) {
        if is_install(&dir) && !out.iter().any(|o| crate::copies::same_dir(o, &dir)) {
            out.push(dir);
        }
    }
    out
}

fn names_f1_25(values: &str) -> bool {
    parse_reg_query(values).iter().any(|(name, data)| {
        let plain: String = data
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
            .collect();
        let plain = plain
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        name.eq_ignore_ascii_case("DisplayName") && (plain == "f1 25" || plain.ends_with(" f1 25"))
    })
}

/// The folders the EA app records F1 25 in, whether or not the game is
/// still there. `reg` runs `reg query` with the given arguments.
pub fn recorded(reg: impl Fn(&[&str]) -> Option<String>) -> Vec<PathBuf> {
    let keyed = INSTALL_KEYS
        .iter()
        .filter_map(|k| reg(&[k]))
        .flat_map(|out| install_dirs(&out));
    let listed = UNINSTALL_KEYS
        .iter()
        .filter_map(|k| reg(&[k, "/s"]))
        .flat_map(|out| reg_tree(&out))
        .filter(|(_, values)| names_f1_25(values))
        .flat_map(|(_, values)| install_dirs(&values));
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in keyed.chain(listed) {
        if !out.iter().any(|o| same_folder(o, &dir)) {
            out.push(dir);
        }
    }
    out
}

#[cfg(windows)]
pub fn recorded_here() -> Vec<PathBuf> {
    recorded(reg_exe)
}

#[cfg(not(windows))]
pub fn recorded_here() -> Vec<PathBuf> {
    Vec::new()
}

fn same_folder(a: &Path, b: &Path) -> bool {
    let plain = |p: &Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    crate::copies::same_dir(a, b) || (cfg!(windows) && plain(a) == plain(b))
}

/// An EA app copy in a folder the EA app has no record of: moved by hand,
/// so the EA app's launch link starts the recorded copy, or none.
#[derive(Debug, Clone, PartialEq)]
pub struct Moved {
    pub dir: PathBuf,
    pub recorded: PathBuf,
    /// Whether the game is in the recorded folder.
    pub there: bool,
}

impl Moved {
    pub fn text(&self) -> String {
        let fix = if self.there {
            format!("Play the copy in {} instead", self.recorded.display())
        } else {
            format!("Move the folder back to {}", self.recorded.display())
        };
        format!(
            "The EA app has F1 25 in {}, so it can't start the copy in {}. {fix}. To keep F1 25 somewhere \
             else, move it with EA app > F1 25 > Manage > Move, not File Explorer",
            self.recorded.display(),
            self.dir.display()
        )
    }
}

/// `dir` against the folders the EA app records F1 25 in. Nothing recorded
/// says nothing either way, and neither does a record with a `?` or U+FFFD:
/// reg.exe writes a letter its code page has no room for that way, and no
/// Windows folder name holds a `?`.
pub fn moved(dir: &Path, recorded: &[PathBuf], is_install: impl Fn(&Path) -> bool) -> Option<Moved> {
    let readable = |r: &&PathBuf| !r.to_string_lossy().contains(['?', '\u{fffd}']);
    let recorded: Vec<&PathBuf> = recorded.iter().filter(readable).collect();
    if recorded.is_empty() || recorded.iter().any(|r| same_folder(r, dir)) {
        return None;
    }
    let there = recorded.iter().find(|r| is_install(r));
    Some(Moved {
        dir: dir.to_path_buf(),
        recorded: (*there.unwrap_or(&recorded[0])).clone(),
        there: there.is_some(),
    })
}

#[cfg(windows)]
fn reg_exe(args: &[&str]) -> Option<String> {
    let mut all = vec!["query"];
    all.extend_from_slice(args);
    crate::compat::windows_tool("reg", &all).filter(|o| !o.trim().is_empty())
}

#[cfg(windows)]
fn program_files() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect()
}

/// The folders the EA app installs games into, from the `downloadinplacedir`
/// lines of its `machine.ini` and `user_<id>.ini` settings.
pub fn library_dirs(ini: &str) -> Vec<PathBuf> {
    ini.lines()
        .filter_map(|l| l.trim_end_matches('\r').split_once('='))
        .filter(|(k, _)| k.trim().to_ascii_lowercase().ends_with(".downloadinplacedir"))
        .map(|(_, v)| v.trim().trim_matches('"'))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(windows)]
fn settings_files() -> Vec<PathBuf> {
    let machine = std::env::var_os("ProgramData").map(|d| PathBuf::from(d).join(r"EA Desktop\machine.ini"));
    let users = std::env::var_os("LOCALAPPDATA")
        .map(|d| PathBuf::from(d).join(r"Electronic Arts\EA Desktop"))
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("user_") && n.ends_with(".ini"))
        });
    machine.into_iter().chain(users).collect()
}

#[cfg(windows)]
pub fn installs() -> Vec<PathBuf> {
    let defaults: Vec<PathBuf> = settings_files()
        .into_iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .flat_map(|ini| library_dirs(&ini))
        .chain(program_files().into_iter().map(|p| p.join("EA Games")))
        .collect();
    find_installs(reg_exe, &defaults, is_install)
}

/// Linux has no standard place for the EA app's games; players add the
/// folder in the launcher.
#[cfg(not(windows))]
pub fn installs() -> Vec<PathBuf> {
    Vec::new()
}

/// The `<contentID>` values in the install's installerdata.xml.
pub fn content_ids(installerdata: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = installerdata;
    while let Some(start) = rest.find("<contentID>") {
        rest = &rest[start + "<contentID>".len()..];
        let Some(end) = rest.find("</contentID>") else {
            break;
        };
        let id = rest[..end].trim();
        if !id.is_empty() && !out.iter().any(|o| o == id) {
            out.push(id.to_owned());
        }
        rest = &rest[end..];
    }
    out
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The link that asks the EA app to launch the game, with `args` for it.
pub fn launch_url(ids: &[String], args: &[String]) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let offers: Vec<String> = ids.iter().map(|i| encode(i)).collect();
    let mut url = format!(
        "origin2://game/launch?offerIds={}&autoDownload=1",
        offers.join(",")
    );
    if !args.is_empty() {
        url.push_str("&cmdParams=");
        url.push_str(&encode(&args.join(" ")));
    }
    Some(url)
}

pub fn install_url(dir: &Path, args: &[String]) -> Result<String, String> {
    let xml = std::fs::read_to_string(dir.join(MARKER))
        .map_err(|e| format!("could not read {}: {e}", dir.join(MARKER).display()))?;
    launch_url(&content_ids(&xml), args)
        .ok_or_else(|| format!("{} names no content ID to launch", dir.join(MARKER).display()))
}

/// A command line split into words, honouring double quotes.
pub fn split_command(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut any) = (false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            c => word.push(c),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

/// A registered handler's command line: the EA app writes its program path
/// unquoted, spaces and all, so the program runs up to the first `.exe`.
pub fn split_handler(line: &str) -> Vec<String> {
    let line = line.trim();
    if line.starts_with('"') {
        return split_command(line);
    }
    let lower = line.to_ascii_lowercase();
    let end = lower.match_indices(".exe").map(|(i, _)| i + 4).find(|&i| {
        line[i..]
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace() || c == '"')
    });
    match end {
        Some(end) => std::iter::once(line[..end].to_owned())
            .chain(split_command(&line[end..]))
            .collect(),
        None => split_command(line),
    }
}

/// How a launch link reaches the EA app.
#[derive(Debug, Clone, PartialEq)]
pub enum Handoff {
    /// A program and its arguments, with `%1` or `{url}` where the link goes.
    Program(Vec<String>),
    /// A shell command line with `{url}` where the link goes.
    Shell(String),
}

impl Handoff {
    /// The `origin2` handler Windows registered, from `reg query <key> /ve`.
    pub fn from_protocol(reg_out: &str) -> Option<Self> {
        let (_, command) = parse_reg_query(reg_out).into_iter().next()?;
        let words = split_handler(&command);
        (!words.is_empty()).then_some(Self::Program(words))
    }

    pub fn from_setting(command: &str) -> Option<Self> {
        let command = command.trim();
        if command.is_empty() {
            None
        } else if cfg!(windows) {
            Some(Self::Program(split_handler(command)))
        } else {
            Some(Self::Shell(command.to_owned()))
        }
    }

    /// The words to run for `url`; the link is appended when the command
    /// has no placeholder.
    pub fn argv(&self, url: &str) -> Vec<String> {
        match self {
            Self::Program(words) => {
                let mut placed = false;
                let mut out: Vec<String> = words
                    .iter()
                    .map(|w| {
                        if w.contains("%1") || w.contains("{url}") {
                            placed = true;
                            w.replace("%1", url).replace("{url}", url)
                        } else {
                            w.clone()
                        }
                    })
                    .collect();
                if !placed {
                    out.push(url.to_owned());
                }
                out
            }
            Self::Shell(line) => {
                let quoted = format!("'{}'", url.replace('\'', r"'\''"));
                let line = if line.contains("{url}") {
                    line.replace("{url}", &quoted)
                } else {
                    format!("{line} {quoted}")
                };
                vec!["sh".into(), "-c".into(), line]
            }
        }
    }

    /// The program the hand-off runs, when it is an exe rather than a shell
    /// command line.
    pub fn program(&self) -> Option<PathBuf> {
        match self {
            Self::Program(words) if words[0].to_ascii_lowercase().ends_with(".exe") => {
                Some(PathBuf::from(&words[0]))
            }
            _ => None,
        }
    }

    /// The command that hands over `url`. The EA app it may start is the
    /// one that starts F1 25, so the launcher's [`COMPAT_LAYER`] stays out.
    pub fn command(&self, url: &str) -> Command {
        let argv = self.argv(url);
        let mut c = Command::new(&argv[0]);
        c.args(&argv[1..]).env_remove(COMPAT_LAYER);
        c
    }
}

impl std::fmt::Display for Handoff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Program(words) => f.write_str(&words.join(" ")),
            Self::Shell(line) => f.write_str(line),
        }
    }
}

/// EALauncher.exe from the EA Desktop key's values, else its default folder.
/// `InstallLocation` is the EA Desktop folder, with the app one level down.
pub fn launcher_exe(desktop_values: &str, defaults: &[PathBuf]) -> Option<PathBuf> {
    let values = parse_reg_query(desktop_values);
    let value = |name: &str| {
        values
            .iter()
            .filter(|(n, d)| n.eq_ignore_ascii_case(name) && !d.trim().is_empty())
            .map(|(_, d)| PathBuf::from(d.trim().trim_matches('"')))
            .collect::<Vec<_>>()
    };
    let beside = |exe: PathBuf| exe.parent().map(|d| d.join(LAUNCHER_EXE));
    value("LauncherAppPath")
        .into_iter()
        .chain(
            value("InstallLocation")
                .into_iter()
                .flat_map(|d| [d.join("EA Desktop").join(LAUNCHER_EXE), d.join(LAUNCHER_EXE)]),
        )
        .chain(value("DesktopAppPath").into_iter().filter_map(beside))
        .chain(value("ClientPath").into_iter().filter_map(beside))
        .chain(defaults.iter().cloned())
        .find(|p| p.is_file())
}

/// How this computer hands a launch link to the EA app: the setting when
/// set, else on Windows the `origin2` handler the EA app registered. The
/// handler is what Windows runs for EA's own links, so it follows the EA
/// app wherever it is installed and shows up front whether it is; running
/// it directly instead of through `start` keeps the link's `&` away from
/// cmd.exe. EALauncher.exe from the EA Desktop key is the fallback.
pub fn handoff(setting: Option<&str>) -> Option<Handoff> {
    if let Some(h) = setting.and_then(Handoff::from_setting) {
        return Some(h);
    }
    windows_handoff()
}

#[cfg(windows)]
fn windows_handoff() -> Option<Handoff> {
    if let Some(h) = reg_exe(&[PROTOCOL_KEY, "/ve"]).and_then(|o| Handoff::from_protocol(&o)) {
        return Some(h);
    }
    let desktop: String = DESKTOP_KEYS.iter().filter_map(|k| reg_exe(&[k])).collect();
    let defaults: Vec<PathBuf> = program_files()
        .iter()
        .map(|p| {
            p.join(r"Electronic Arts\EA Desktop\EA Desktop")
                .join(LAUNCHER_EXE)
        })
        .collect();
    launcher_exe(&desktop, &defaults).map(|exe| Handoff::Program(vec![exe.display().to_string()]))
}

#[cfg(not(windows))]
fn windows_handoff() -> Option<Handoff> {
    None
}

/// What opens the EA app without a launch link: EALauncher.exe on its own,
/// or the setting's command with the link left out.
pub fn opener(handoff: &Handoff) -> Option<Command> {
    let mut c = match handoff {
        Handoff::Program(words) if words[0].to_ascii_lowercase().ends_with(".exe") => Command::new(&words[0]),
        Handoff::Shell(line) => {
            let mut c = Command::new("sh");
            c.arg("-c").arg(line.replace("{url}", ""));
            c
        }
        Handoff::Program(_) => return None,
    };
    c.env_remove(COMPAT_LAYER);
    Some(c)
}

/// Starts a program as the signed-in user from an administrator's process.
pub const DESKTOP_SHELL: &str = "explorer.exe";

/// What opens the EA app for a process that may run as administrator, and
/// whether the command's exit code says if it worked. A program started by an
/// administrator's process runs as administrator too, so from one the EA app
/// goes through Explorer and runs with the signed-in user's own rights, as
/// from its shortcut. Explorer's exit code says nothing about the EA app.
pub fn desktop_opener(handoff: &Handoff, elevated: bool) -> Option<(Command, bool)> {
    let open = opener(handoff)?;
    if !elevated || !matches!(handoff, Handoff::Program(_)) {
        return Some((open, true));
    }
    let mut c = Command::new(DESKTOP_SHELL);
    c.arg(open.get_program()).env_remove(COMPAT_LAYER);
    Some((c, false))
}

pub fn running() -> bool {
    !crate::process::named(PROCESS).is_empty()
}

/// The `user.reg` of the EA app's Wine prefix from the `launch.ea_prefix`
/// setting: the prefix itself, or Proton's compatdata folder around it.
pub fn prefix_user_reg(setting: &Path) -> Option<PathBuf> {
    [setting.join("user.reg"), setting.join("pfx/user.reg")]
        .into_iter()
        .find(|p| p.is_file())
}

/// The wineserver running the Wine prefix `pfx`. Wine names the server's
/// folder after the prefix folder's device and inode, and the server works
/// in it.
#[cfg(not(windows))]
pub fn prefix_server(pfx: &Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(pfx).ok()?;
    let folder = format!("server-{:x}-{:x}", meta.dev(), meta.ino());
    std::fs::read_dir("/proc").ok()?.flatten().find_map(|e| {
        let pid = e.file_name().to_str()?.parse().ok()?;
        let comm = std::fs::read_to_string(e.path().join("comm")).ok()?;
        let cwd = std::fs::read_link(e.path().join("cwd")).ok()?;
        (comm.trim_end() == "wineserver" && cwd.file_name()? == folder.as_str()).then_some(pid)
    })
}

pub const NO_HANDOFF_LINUX: &str = "this is the EA app's copy of F1 25, and on Linux Swofty starts it through the EA app with \
     the launch.ea_launch_command setting: a command that hands {url} to the EA app in its Wine prefix. Set it in \
     config.json (see Playing the EA app copy in the docs), or switch to another copy";
pub const NO_HANDOFF_WINDOWS: &str =
    "the EA app was not found on this computer; install or repair the EA app, then press Play again";

pub fn no_handoff() -> &'static str {
    if cfg!(windows) {
        NO_HANDOFF_WINDOWS
    } else {
        NO_HANDOFF_LINUX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALLERDATA: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<DiPManifest version="4.0">
  <buildMetaData><gameVersion version="1.0.141.7865"/></buildMetaData>
  <contentIDs>
    <contentID>16425845</contentID>
    <contentID>16425845_oa</contentID>
    <contentID>16425845</contentID>
  </contentIDs>
  <runtime>
    <launcher uid="launcher_1">
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\Codemasters\F1_25\Install Dir]EAAntiCheat.GameServiceLauncher.exe</filePath>
    </launcher>
  </runtime>
</DiPManifest>"#;

    #[test]
    fn reads_the_content_ids_once_each() {
        assert_eq!(content_ids(INSTALLERDATA), ["16425845", "16425845_oa"]);
        assert!(content_ids("<DiPManifest/>").is_empty());
        assert_eq!(content_ids("<contentID> 1 </contentID><contentID>"), ["1"]);
    }

    #[test]
    fn builds_the_launch_link() {
        let ids = content_ids(INSTALLERDATA);
        assert_eq!(
            launch_url(&ids, &[]).unwrap(),
            "origin2://game/launch?offerIds=16425845,16425845_oa&autoDownload=1"
        );
        assert_eq!(
            launch_url(&ids, &["-vr".into()]).unwrap(),
            "origin2://game/launch?offerIds=16425845,16425845_oa&autoDownload=1&cmdParams=-vr"
        );
        assert_eq!(
            launch_url(&ids, &["-vr".into(), "-x=a&b".into()]).unwrap(),
            "origin2://game/launch?offerIds=16425845,16425845_oa&autoDownload=1&cmdParams=-vr%20-x%3Da%26b"
        );
        assert_eq!(launch_url(&[], &[]), None);
    }

    #[test]
    fn reads_install_folders_from_the_registry() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Codemasters\\F1_25\r\n    Install Dir    REG_SZ    D:\\EA Games\\F1 25\\\r\n    Locale    REG_SZ    en_US\r\n\r\n";
        assert_eq!(install_dirs(out), [PathBuf::from("D:\\EA Games\\F1 25\\")]);
        let tree = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{A}\r\n    DisplayName    REG_SZ    Other\r\n    InstallLocation    REG_SZ    C:\\Other\r\n\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{B}\r\n    DisplayName    REG_SZ    F1® 25\r\n    InstallLocation    REG_SZ    \"E:\\Games\\F1 25\"\r\n";
        let blocks = reg_tree(tree);
        assert_eq!(blocks.len(), 2);
        let dirs: Vec<PathBuf> = blocks.iter().flat_map(|(_, v)| install_dirs(v)).collect();
        assert_eq!(
            dirs,
            [PathBuf::from("C:\\Other"), PathBuf::from("E:\\Games\\F1 25")]
        );
    }

    #[test]
    fn finds_installs_by_key_uninstall_entry_and_default_folder() {
        let root = std::env::temp_dir().join(format!("swofty-ea-find-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let make = |rel: &str, marker: bool| {
            let d = root.join(rel);
            std::fs::create_dir_all(d.join("__Installer")).unwrap();
            std::fs::write(d.join(GAME_EXE), b"MZ").unwrap();
            if marker {
                std::fs::write(d.join(MARKER), INSTALLERDATA).unwrap();
            }
            d
        };
        let keyed = make("keyed/F1 25", true);
        let listed = make("listed/F1 25", true);
        let default = make("EA Games/F1 25", true);
        let steamish = make("steamish/F1 25", false);
        let reg = |args: &[&str]| -> Option<String> {
            match args {
                [k] if *k == INSTALL_KEYS[0] => Some(format!(
                    "\r\n{k}\r\n    Install Dir    REG_SZ    {}\r\n",
                    keyed.display()
                )),
                [k] if *k == INSTALL_KEYS[1] => Some(format!(
                    "\r\n{k}\r\n    Install Dir    REG_SZ    {}\r\n",
                    keyed.display()
                )),
                [k, "/s"] if *k == UNINSTALL_KEYS[0] => Some(format!(
                    "\r\nHKEY_X\\{{1}}\r\n    InstallLocation    REG_SZ    {}\r\n\r\nHKEY_X\\{{2}}\r\n    InstallLocation    REG_SZ    {}\r\n",
                    listed.display(),
                    steamish.display()
                )),
                _ => None,
            }
        };
        let found = find_installs(reg, &[root.join("EA Games")], is_install);
        assert_eq!(found, [keyed, listed, default]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_copy_moved_by_hand_is_not_where_the_ea_app_has_it() {
        let root = std::env::temp_dir().join(format!("swofty-ea-moved-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let program_files = root.join("Program Files/EA Games/F1 25");
        let moved_to = root.join("EA Games/F1 25");
        for d in [&program_files, &moved_to] {
            std::fs::create_dir_all(d.join("__Installer")).unwrap();
            std::fs::write(d.join(GAME_EXE), b"MZ").unwrap();
            std::fs::write(d.join(MARKER), INSTALLERDATA).unwrap();
        }
        let record = program_files.display().to_string();
        let reg = |args: &[&str]| -> Option<String> {
            match args {
                [k] if *k == INSTALL_KEYS[1] => {
                    Some(format!("\r\n{k}\r\n    Install Dir    REG_SZ    {record}\r\n"))
                }
                [k, "/s"] if *k == UNINSTALL_KEYS[0] => Some(format!(
                    "\r\nHKEY_X\\{{A}}\r\n    DisplayName    REG_SZ    Other\r\n    InstallLocation    REG_SZ    C:\\Other\r\n\r\n\
                     HKEY_X\\{{B}}\r\n    DisplayName    REG_SZ    F1® 25\r\n    InstallLocation    REG_SZ    \"{record}\"\r\n"
                )),
                _ => None,
            }
        };
        let rec = recorded(reg);
        assert_eq!(rec.len(), 1, "{rec:?}");
        assert!(crate::copies::same_dir(&rec[0], &program_files));
        assert_eq!(moved(&program_files, &rec, is_install), None);
        let m = moved(&moved_to, &rec, is_install).unwrap();
        assert!(m.there);
        assert!(crate::copies::same_dir(&m.recorded, &program_files));
        let text = m.text();
        assert!(
            text.starts_with(&format!("The EA app has F1 25 in {}", m.recorded.display())),
            "{text}"
        );
        assert!(text.contains(&moved_to.display().to_string()), "{text}");
        assert!(text.contains("Play the copy in"), "{text}");
        assert!(
            text.ends_with("EA app > F1 25 > Manage > Move, not File Explorer"),
            "{text}"
        );

        std::fs::remove_dir_all(&program_files).unwrap();
        let gone = moved(&moved_to, &rec, is_install).unwrap();
        assert!(!gone.there);
        assert!(
            gone.text().contains(". Move the folder back to "),
            "{}",
            gone.text()
        );
        assert_eq!(moved(&moved_to, &[], is_install), None);
        let mangled = [
            PathBuf::from("D:\\Yeni klas?r\\F1 25"),
            PathBuf::from("D:\\Yeni klas\u{fffd}r\\F1 25"),
        ];
        assert_eq!(moved(&moved_to, &mangled, is_install), None);
        assert_eq!(moved(&moved_to, &mangled[..1], is_install), None);
        assert!(recorded(|_| None).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn only_f1_25_uninstall_entries_count_as_its_record() {
        assert!(names_f1_25("    DisplayName    REG_SZ    F1® 25\r\n"));
        assert!(names_f1_25("    DisplayName    REG_SZ    F1 25\r\n"));
        assert!(names_f1_25("    DisplayName    REG_SZ    EA SPORTS™ F1® 25\r\n"));
        assert!(!names_f1_25("    DisplayName    REG_SZ    F1® 24\r\n"));
        assert!(!names_f1_25(
            "    DisplayName    REG_SZ    F1 25 Champions Edition Content\r\n"
        ));
        assert!(!names_f1_25("    InstallLocation    REG_SZ    F1 25\r\n"));
    }

    #[test]
    fn hands_the_link_to_the_registered_handler() {
        let out = "\r\nHKEY_CLASSES_ROOT\\origin2\\shell\\open\\command\r\n    (Default)    REG_SZ    \"C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EALauncher.exe\" \"%1\"\r\n";
        let h = Handoff::from_protocol(out).unwrap();
        assert_eq!(
            h.argv("origin2://game/launch?offerIds=1&autoDownload=1"),
            [
                r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe",
                "origin2://game/launch?offerIds=1&autoDownload=1"
            ]
        );
        assert_eq!(
            opener(&h).map(|c| c.get_program().to_owned()),
            Some(r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe".into())
        );
        let bare = Handoff::Program(vec!["EALauncher.exe".into()]);
        assert_eq!(bare.argv("u"), ["EALauncher.exe", "u"]);
        let strips = |c: Command| c.get_envs().any(|(k, v)| k == COMPAT_LAYER && v.is_none());
        assert!(strips(h.command("u")) && strips(opener(&h).unwrap()));
        assert!(strips(opener(&Handoff::Shell("open {url}".into())).unwrap()));
    }

    #[test]
    fn a_shell_setting_gets_the_link_quoted() {
        let h = Handoff::Shell("/games/ea-app.sh wine start {url}".into());
        assert_eq!(
            h.argv("origin2://game/launch?offerIds=1&autoDownload=1"),
            [
                "sh",
                "-c",
                "/games/ea-app.sh wine start 'origin2://game/launch?offerIds=1&autoDownload=1'"
            ]
        );
        assert_eq!(Handoff::Shell("open".into()).argv("a'b")[2], r"open 'a'\''b'");
        assert_eq!(Handoff::from_setting("  "), None);
        let open = opener(&h).unwrap();
        let args: Vec<_> = open.get_args().collect();
        assert_eq!(open.get_program(), "sh");
        assert_eq!(args, ["-c", "/games/ea-app.sh wine start "]);
        assert!(opener(&Handoff::Program(vec!["open.bat".into()])).is_none());
    }

    #[test]
    fn an_administrator_opens_the_ea_app_through_explorer() {
        let h = Handoff::from_protocol(ORIGIN2_KEY).unwrap();
        let exe = r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe";
        let (open, exit_tells) = desktop_opener(&h, true).unwrap();
        let args: Vec<_> = open.get_args().collect();
        assert_eq!(open.get_program(), DESKTOP_SHELL);
        assert_eq!(args, [exe]);
        assert!(!exit_tells);
        assert!(open.get_envs().any(|(k, v)| k == COMPAT_LAYER && v.is_none()));
        let (open, exit_tells) = desktop_opener(&h, false).unwrap();
        assert_eq!(open.get_program(), exe);
        assert_eq!(open.get_args().count(), 0);
        assert!(exit_tells);
        let shell = Handoff::Shell("/games/ea-app.sh wine start {url}".into());
        let (open, exit_tells) = desktop_opener(&shell, true).unwrap();
        assert_eq!(open.get_program(), "sh");
        assert!(exit_tells);
        assert!(desktop_opener(&Handoff::Program(vec!["open.bat".into()]), true).is_none());
    }

    // The values the EA app 13.813 installer writes, as `reg query` lists them.
    const ORIGIN2_KEY: &str = "\r\nHKEY_CLASSES_ROOT\\origin2\\shell\\open\\command\r\n    (Default)    REG_SZ    C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EALauncher.exe \"%1\"\r\n\r\n";
    const DESKTOP_KEY: &str = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\Electronic Arts\\EA Desktop\r\n    ClientPath    REG_SZ    C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EADesktop.exe\r\n    DesktopAppPath    REG_SZ    C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EADesktop.exe\r\n    InstallLocation    REG_SZ    C:\\Program Files\\Electronic Arts\\EA Desktop\r\n";

    #[test]
    fn runs_the_unquoted_handler_the_ea_app_registers() {
        let h = Handoff::from_protocol(ORIGIN2_KEY).unwrap();
        let url = "origin2://game/launch?offerIds=16425845,16425845_oa&autoDownload=1&cmdParams=-vr";
        assert_eq!(
            h.argv(url),
            [
                r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe",
                url
            ]
        );
        assert_eq!(
            opener(&h).map(|c| c.get_program().to_owned()),
            Some(r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe".into())
        );
        assert_eq!(
            split_handler(r#"C:\EA app.EXE --x "%1""#),
            [r"C:\EA app.EXE", "--x", "%1"]
        );
        assert_eq!(split_handler(r"C:\a.exe.d\b.exe %1"), [r"C:\a.exe.d\b.exe", "%1"]);
        assert_eq!(split_handler("open %1"), ["open", "%1"]);
        assert_eq!(
            h.program(),
            Some(PathBuf::from(
                r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe"
            ))
        );
        assert_eq!(Handoff::Program(vec!["open.bat".into()]).program(), None);
        assert_eq!(Handoff::Shell("/games/ea-app.sh {url}".into()).program(), None);
    }

    #[test]
    fn finds_the_launcher_beside_the_desktop_app() {
        let root = std::env::temp_dir().join(format!("swofty-ea-desktop-{}", std::process::id()));
        let app = root.join("EA Desktop");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join(LAUNCHER_EXE), b"MZ").unwrap();
        let values = DESKTOP_KEY.replace(
            r"C:\Program Files\Electronic Arts\EA Desktop",
            &root.display().to_string(),
        );
        assert_eq!(launcher_exe(&values, &[]), Some(app.join(LAUNCHER_EXE)));
        let desktop_only = format!(
            "    DesktopAppPath    REG_SZ    {}\r\n",
            app.join("EADesktop.exe").display()
        );
        assert_eq!(launcher_exe(&desktop_only, &[]), Some(app.join(LAUNCHER_EXE)));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reads_the_ea_app_library_folders() {
        let machine =
            "machine.downloadinplacedir=C:\\Program Files\\EA Games\\\r\nmachine.updatebucket=6\r\n";
        let user = "user.locale=en_GB\nuser.downloadinplacedir=D:\\\nuser.downloadinplacedir=\n";
        assert_eq!(
            library_dirs(machine),
            [PathBuf::from("C:\\Program Files\\EA Games\\")]
        );
        assert_eq!(library_dirs(user), [PathBuf::from("D:\\")]);
        assert!(library_dirs("machine.razormodesplashscreenenabled=1").is_empty());
    }

    #[test]
    fn finds_an_install_in_a_library_folder_without_registry_keys() {
        let root = std::env::temp_dir().join(format!("swofty-ea-library-{}", std::process::id()));
        let game = root.join("F1 25");
        std::fs::create_dir_all(game.join("__Installer")).unwrap();
        std::fs::write(game.join(GAME_EXE), b"MZ").unwrap();
        std::fs::write(game.join(MARKER), INSTALLERDATA).unwrap();
        let ini = format!(
            "user.downloadinplacedir={}{}\n",
            root.display(),
            std::path::MAIN_SEPARATOR
        );
        let found = find_installs(|_| None, &library_dirs(&ini), is_install);
        assert_eq!(found.len(), 1);
        assert!(crate::copies::same_dir(&found[0], &game));
        assert!(find_installs(|_| None, &[root.join("missing")], is_install).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(windows)]
    #[test]
    fn windows_lookups_hold_on_a_computer_without_the_ea_app() {
        // CI's runner has no EA app: discovery and the handler lookup must
        // come back empty-handed, not fail, and a found handler must be one
        // that takes the link.
        for dir in installs() {
            assert!(is_install(&dir));
        }
        if let Some(h) = handoff(None) {
            assert!(h.argv("origin2://x").iter().any(|w| w == "origin2://x"));
        }
        assert_eq!(no_handoff(), NO_HANDOFF_WINDOWS);
        let set = handoff(Some(r#"C:\Tools\ea open.exe "%1""#)).unwrap();
        assert_eq!(set.argv("u"), [r"C:\Tools\ea open.exe", "u"]);
    }

    #[cfg(unix)]
    #[test]
    fn one_setting_both_opens_the_app_and_hands_over_the_link() {
        let root = std::env::temp_dir().join(format!("swofty-ea-branch-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let out = root.join("out");
        let h = Handoff::Shell(format!(
            r#"u={{url}}; if [ -z "$u" ]; then echo open >> '{0}'; else echo "link $u" >> '{0}'; fi"#,
            out.display()
        ));
        assert!(opener(&h).unwrap().status().unwrap().success());
        let url = "origin2://game/launch?offerIds=1&autoDownload=1&cmdParams=-vr";
        assert!(h.command(url).status().unwrap().success());
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            format!("open\nlink {url}\n")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn splits_quoted_commands() {
        assert_eq!(
            split_command(r#""C:\A B\x.exe" --flag "%1" """#),
            [r"C:\A B\x.exe", "--flag", "%1", ""]
        );
    }

    #[test]
    fn finds_the_launcher_from_the_desktop_key() {
        let root = std::env::temp_dir().join(format!("swofty-ea-launcher-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let exe = root.join(LAUNCHER_EXE);
        std::fs::write(&exe, b"MZ").unwrap();
        let values = format!(
            "\r\nHKEY_X\r\n    InstallLocation    REG_SZ    {}\r\n",
            root.display()
        );
        assert_eq!(launcher_exe(&values, &[]), Some(exe.clone()));
        assert_eq!(launcher_exe("", std::slice::from_ref(&exe)), Some(exe));
        assert_eq!(launcher_exe("", &[root.join("missing.exe")]), None);
        std::fs::remove_dir_all(&root).ok();
    }
}
