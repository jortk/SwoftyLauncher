use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText};
use swofty_core::process::same_folder;
use swofty_core::update::{self, Install};

use crate::{FOLDER_NAME, MUTED, shortcut};

const RELEASE: [&str; 8] = [
    "bin",
    "loader",
    "dxc",
    "share",
    "addons",
    "LICENSE",
    "README.md",
    "THIRD_PARTY.md",
];

const DATA_ENTRIES: [&str; 7] = [
    "addons",
    "config.json",
    "profiles",
    "saves",
    "state",
    "cache",
    "baseline",
];

pub fn is_install(dir: &Path) -> bool {
    dir.join("bin").join(update::exe("swofty-launcher")).is_file() && dir.join("loader").is_dir()
}

pub fn default_root() -> PathBuf {
    let parent = if cfg!(windows) {
        dirs::data_local_dir().map(|d| d.join("Programs"))
    } else {
        dirs::home_dir()
    };
    parent.unwrap_or_default().join(FOLDER_NAME)
}

pub struct Existing {
    found: Vec<PathBuf>,
    picked: usize,
    keep: bool,
}

impl Existing {
    pub fn find() -> Self {
        Self {
            found: find(),
            picked: 0,
            keep: false,
        }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.found
    }

    pub fn root(&self) -> Option<&Path> {
        self.found.get(self.picked).map(PathBuf::as_path)
    }

    pub fn list(&mut self, ui: &mut egui::Ui, folder: &mut String) {
        if self.found.len() < 2 {
            return;
        }
        ui.label(RichText::new("Installed in").color(MUTED));
        for (i, root) in self.found.iter().enumerate() {
            if ui
                .radio_value(&mut self.picked, i, root.display().to_string())
                .clicked()
            {
                *folder = root.display().to_string();
            }
        }
        ui.add_space(12.0);
    }

    pub fn status(&mut self, ui: &mut egui::Ui, folder: &str) {
        let Ok(target) = crate::resolve(folder) else {
            return;
        };
        match self.found.iter().position(|r| same_folder(r, &target)) {
            Some(i) => self.picked = i,
            None if is_install(&target) => {
                self.found.push(target.clone());
                self.picked = self.found.len() - 1;
            }
            None => {}
        }
        let Some(from) = self.root().map(Path::to_path_buf) else {
            return;
        };
        ui.add_space(6.0);
        if same_folder(&from, &target) {
            ui.label(RichText::new("Updates your install in place.").color(MUTED));
            return;
        }
        ui.label(
            RichText::new(if self.keep {
                format!("Installs a second copy and keeps the one in {}.", from.display())
            } else {
                format!(
                    "Moves your install from {} to {}.",
                    from.display(),
                    target.display()
                )
            })
            .color(MUTED),
        );
    }

    pub fn differs_from(&self, target: &Path) -> bool {
        self.root().is_some_and(|from| !same_folder(from, target))
    }

    pub fn advanced(&mut self, ui: &mut egui::Ui, folder: &str) {
        let Ok(target) = crate::resolve(folder) else {
            return;
        };
        if self.differs_from(&target) {
            ui.checkbox(&mut self.keep, "Keep a second copy of the launcher");
        }
    }

    pub fn planned(&self, target: &Path) -> Result<Option<Move>, String> {
        match self.root() {
            Some(from) if !self.keep && is_install(from) && !same_folder(from, target) => {
                plan(from, target, &swofty_core::paths::data_dir()).map(Some)
            }
            _ => Ok(None),
        }
    }
}

fn find() -> Vec<PathBuf> {
    let linked: Vec<PathBuf> = shortcut::files().iter().flat_map(|f| shortcut_roots(f)).collect();
    collect(running_roots().into_iter().chain(linked).chain([default_root()]))
}

fn collect(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for root in candidates {
        if is_install(&root) && !found.iter().any(|f| same_folder(f, &root)) {
            found.push(root);
        }
    }
    found
}

fn root_of(exe: &Path) -> Option<PathBuf> {
    let bin = exe
        .parent()
        .filter(|b| b.file_name().is_some_and(|n| n.eq_ignore_ascii_case("bin")))?;
    bin.parent().map(Path::to_path_buf)
}

fn running_roots() -> Vec<PathBuf> {
    running_launchers()
        .iter()
        .filter_map(|exe| root_of(exe))
        .collect()
}

#[cfg(target_os = "linux")]
fn running_launchers() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let name = update::exe("swofty-launcher");
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
        })
        .filter_map(|e| std::fs::read_link(e.path().join("exe")).ok())
        .map(
            |exe| match exe.to_str().and_then(|s| s.strip_suffix(" (deleted)")) {
                Some(live) => PathBuf::from(live),
                None => exe,
            },
        )
        .filter(|exe| exe.file_name().is_some_and(|n| *n == *name))
        .collect()
}

#[cfg(windows)]
fn running_launchers() -> Vec<PathBuf> {
    crate::win32::images(&update::exe("swofty-launcher"))
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn running_launchers() -> Vec<PathBuf> {
    Vec::new()
}

fn shortcut_roots(file: &Path) -> Vec<PathBuf> {
    let Ok(data) = std::fs::read(file) else {
        return Vec::new();
    };
    if cfg!(windows) {
        lnk_roots(&data)
    } else {
        desktop_roots(&String::from_utf8_lossy(&data))
    }
}

fn remove_stale(old: &Path) {
    for file in shortcut::files() {
        if shortcut_roots(&file).iter().any(|r| same_folder(r, old)) {
            std::fs::remove_file(&file).ok();
        }
    }
}

fn desktop_roots(text: &str) -> Vec<PathBuf> {
    let value = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('='));
    let exe = value("Exec")
        .and_then(first_arg)
        .and_then(|exe| root_of(Path::new(&exe)));
    let dir = value("Path").filter(|p| !p.is_empty()).map(PathBuf::from);
    exe.into_iter().chain(dir).collect()
}

fn first_arg(exec: &str) -> Option<String> {
    let exec = exec.trim_start();
    let Some(quoted) = exec.strip_prefix('"') else {
        return exec.split_whitespace().next().map(str::to_owned);
    };
    let mut out = String::new();
    let mut chars = quoted.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            '%' => {
                chars.next();
                out.push('%');
            }
            c => out.push(c),
        }
    }
    None
}

#[derive(Debug, Default, PartialEq)]
struct Link {
    target: Option<String>,
    working_dir: Option<String>,
}

const HAS_ID_LIST: usize = 0x1;
const HAS_LINK_INFO: usize = 0x2;
const HAS_WORKING_DIR: usize = 0x10;
const IS_UNICODE: usize = 0x80;

fn lnk_roots(data: &[u8]) -> Vec<PathBuf> {
    let Some(link) = read_lnk(data) else {
        return Vec::new();
    };
    let target = link.target.and_then(|t| root_of(Path::new(&t)));
    target
        .into_iter()
        .chain(link.working_dir.map(PathBuf::from))
        .collect()
}

fn read_lnk(data: &[u8]) -> Option<Link> {
    if le32(data, 0)? != 0x4C {
        return None;
    }
    let flags = le32(data, 0x14)?;
    let mut at = 0x4C;
    let mut link = Link::default();
    if flags & HAS_ID_LIST != 0 {
        let size = le16(data, at)?;
        link.target = id_list_path(data.get(at + 2..at + 2 + size)?);
        at += 2 + size;
    }
    if flags & HAS_LINK_INFO != 0 {
        let size = le32(data, at)?;
        link.target = data.get(at..at + size).and_then(link_info_path).or(link.target);
        at += size;
    }
    let unit = if flags & IS_UNICODE != 0 { 2 } else { 1 };
    for bit in [0x4, 0x8, HAS_WORKING_DIR] {
        if flags & bit == 0 {
            continue;
        }
        let len = le16(data, at)? * unit;
        let raw = data.get(at + 2..at + 2 + len)?;
        if bit == HAS_WORKING_DIR {
            link.working_dir = Some(if unit == 2 {
                wide(raw, 0)?
            } else {
                String::from_utf8_lossy(raw).into_owned()
            });
        }
        at += 2 + len;
    }
    Some(link)
}

fn id_list_path(list: &[u8]) -> Option<String> {
    let mut path = String::new();
    let mut at = 0;
    while let Some(size) = le16(list, at)
        && size > 2
    {
        let item = list.get(at..at + size)?;
        match item[2] & 0x70 {
            0x20 => path = ansi(item, 3)?,
            0x30 => {
                let name = if item[2] & 0x04 != 0 {
                    wide(item, 14)?
                } else {
                    ansi(item, 14)?
                };
                if !path.ends_with('\\') {
                    path.push('\\');
                }
                path.push_str(&name);
            }
            _ => {}
        }
        at += size;
    }
    (path.as_bytes().get(1) == Some(&b':')).then_some(path)
}

fn link_info_path(info: &[u8]) -> Option<String> {
    if le32(info, 8)? & 0x1 == 0 {
        return None;
    }
    let (base, suffix) = if le32(info, 4)? >= 0x24 {
        (wide(info, le32(info, 28)?)?, wide(info, le32(info, 32)?))
    } else {
        (ansi(info, le32(info, 16)?)?, ansi(info, le32(info, 24)?))
    };
    Some(base + &suffix.unwrap_or_default())
}

fn le16(data: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?) as usize)
}

fn le32(data: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn ansi(data: &[u8], at: usize) -> Option<String> {
    let rest = data.get(at..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn wide(data: &[u8], at: usize) -> Option<String> {
    let units: Vec<u16> = data
        .get(at..)?
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    from: PathBuf,
    to: PathBuf,
    entries: Vec<PathBuf>,
}

pub fn plan(from: &Path, to: &Path, data: &Path) -> Result<Move, String> {
    if !is_install(from) {
        return Err(format!("There is no install in {} to move.", from.display()));
    }
    if is_install(to) {
        return Err(format!("{} already holds an install.", to.display()));
    }
    let owned = data_inside(from, data);
    let entries: Vec<PathBuf> = RELEASE
        .iter()
        .filter(|e| !(owned && DATA_ENTRIES.contains(e)))
        .map(PathBuf::from)
        .filter(|e| from.join(e).symlink_metadata().is_ok())
        .collect();
    for rel in &entries {
        let (src, dst) = (from.join(rel), to.join(rel));
        if to.starts_with(&src) {
            return Err(format!("Choose a folder outside {}.", src.display()));
        }
        if dst.symlink_metadata().is_ok() {
            return Err(format!(
                "{} is already there. Choose an empty folder.",
                dst.display()
            ));
        }
    }
    Ok(Move {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        entries,
    })
}

fn data_inside(from: &Path, data: &Path) -> bool {
    same_folder(from, data) || {
        let resolved = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        resolved(data).starts_with(resolved(from))
    }
}

impl Move {
    pub fn run(&self, desktop: bool) -> Result<(), String> {
        if running_roots().iter().any(|r| same_folder(r, &self.from)) {
            return Err("Close the Swofty Launcher before moving it.".into());
        }
        if swofty_core::process::game_running() {
            return Err("Close F1 25 before moving the launcher.".into());
        }
        Install {
            root: self.from.clone(),
        }
        .cleanup();
        self.relocate(|from, to| std::fs::rename(from, to))?;
        let exe = Install {
            root: self.to.clone(),
        }
        .launcher();
        shortcut::create(&self.to, &exe, desktop);
        remove_stale(&self.from);
        Ok(())
    }

    fn relocate(&self, rename: impl Fn(&Path, &Path) -> std::io::Result<()>) -> Result<(), String> {
        let created = !self.to.exists();
        std::fs::create_dir_all(&self.to)
            .map_err(|e| format!("Could not create {}: {e}", self.to.display()))?;
        let mut done: Vec<(&Path, bool)> = Vec::new();
        for rel in &self.entries {
            let (src, dst) = (self.from.join(rel), self.to.join(rel));
            let moved = dst
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|_| match rename(&src, &dst) {
                    Ok(()) => Ok(false),
                    Err(e) if e.kind() == ErrorKind::CrossesDevices => copy(&src, &dst).map(|_| true),
                    Err(e) => Err(e),
                });
            match moved {
                Ok(copied) => done.push((rel.as_path(), copied)),
                Err(e) => {
                    self.undo(&done, created);
                    return Err(format!("Could not move {}: {e}", src.display()));
                }
            }
        }
        if let Err(e) = self.verify(&done) {
            self.undo(&done, created);
            return Err(e);
        }
        for (rel, copied) in &done {
            if *copied {
                remove(&self.from.join(rel)).ok();
            }
        }
        prune(&self.from, &self.entries);
        std::fs::remove_dir(&self.from).ok();
        Ok(())
    }

    fn verify(&self, done: &[(&Path, bool)]) -> Result<(), String> {
        for (rel, copied) in done {
            let (src, dst) = (self.from.join(rel), self.to.join(rel));
            let whole = if *copied {
                sizes(&src).is_ok_and(|a| sizes(&dst).is_ok_and(|b| a == b))
            } else {
                dst.symlink_metadata().is_ok() && src.symlink_metadata().is_err()
            };
            if !whole {
                return Err(format!(
                    "{} did not move whole to {}.",
                    src.display(),
                    dst.display()
                ));
            }
        }
        if !is_install(&self.to) {
            return Err(format!(
                "The moved install in {} is not complete.",
                self.to.display()
            ));
        }
        Ok(())
    }

    fn undo(&self, done: &[(&Path, bool)], created: bool) {
        for (rel, copied) in done.iter().rev() {
            let (src, dst) = (self.from.join(rel), self.to.join(rel));
            if *copied {
                remove(&dst).ok();
            } else {
                std::fs::rename(&dst, &src).ok();
            }
        }
        prune(&self.to, &self.entries);
        if created {
            std::fs::remove_dir(&self.to).ok();
        }
    }
}

fn prune(base: &Path, entries: &[PathBuf]) {
    for rel in entries {
        let path = base.join(rel);
        for dir in path.ancestors().skip(1).take_while(|d| *d != base) {
            std::fs::remove_dir(dir).ok();
        }
    }
}

fn copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    fn tree(src: &Path, dst: &Path) -> std::io::Result<()> {
        if std::fs::metadata(src)?.is_dir() {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                tree(&entry.path(), &dst.join(entry.file_name()))?;
            }
            Ok(())
        } else {
            std::fs::copy(src, dst).map(drop)
        }
    }
    let copied = tree(src, dst);
    if copied.is_err() {
        remove(dst).ok();
    }
    copied
}

fn remove(path: &Path) -> std::io::Result<()> {
    if path.symlink_metadata()?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn sizes(path: &Path) -> std::io::Result<BTreeMap<PathBuf, u64>> {
    fn walk(path: &Path, rel: PathBuf, out: &mut BTreeMap<PathBuf, u64>) -> std::io::Result<()> {
        let meta = std::fs::metadata(path)?;
        if meta.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                walk(&entry.path(), rel.join(entry.file_name()), out)?;
            }
        } else {
            out.insert(rel, meta.len());
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(path, PathBuf::new(), &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-installer-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn release(root: &Path) {
        write(&root.join("bin").join(update::exe("swofty-launcher")), "launcher");
        write(&root.join("bin").join(update::exe("swofty")), "cli");
        write(&root.join("loader/version.dll"), "proxy");
        write(&root.join("loader/swofty_core.dll"), "core");
        write(&root.join("dxc/bin").join(update::exe("dxc")), "dxc");
        write(&root.join("share/icons/hicolor/256x256/apps/icon.png"), "icon");
        write(&root.join("LICENSE"), "license");
        write(&root.join("README.md"), "readme");
    }

    #[test]
    fn recognises_an_install_by_its_launcher_and_loader() {
        let dir = temp("recognise");
        let full = dir.join("full");
        release(&full);
        assert!(is_install(&full));
        let no_loader = dir.join("no-loader");
        write(
            &no_loader.join("bin").join(update::exe("swofty-launcher")),
            "launcher",
        );
        assert!(!is_install(&no_loader));
        let no_launcher = dir.join("no-launcher");
        write(&no_launcher.join("loader/version.dll"), "proxy");
        assert!(!is_install(&no_launcher));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lists_each_install_once_in_the_order_found() {
        let dir = temp("collect");
        let (a, b) = (dir.join("a"), dir.join("b"));
        release(&a);
        release(&b);
        let again = PathBuf::from(format!("{}/", a.display()));
        let found = collect([dir.join("missing"), b.clone(), a.clone(), again, dir.clone()]);
        assert_eq!(found, [b, a]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finds_the_install_a_launcher_runs_from() {
        let exe = Path::new("/games/SwoftyLauncher/bin").join(update::exe("swofty-launcher"));
        assert_eq!(root_of(&exe), Some(PathBuf::from("/games/SwoftyLauncher")));
        assert_eq!(root_of(Path::new("/games/swofty-launcher")), None);
    }

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn string_data(s: &str) -> Vec<u8> {
        let mut out = (s.encode_utf16().count() as u16).to_le_bytes().to_vec();
        out.extend(utf16(s));
        out
    }

    fn file_item(name: &str, last: bool) -> Vec<u8> {
        let ascii = name.is_ascii();
        let kind = match (last, ascii) {
            (false, true) => 0x31,
            (false, false) => 0x35,
            (true, true) => 0x32,
            (true, false) => 0x36,
        };
        let mut item = vec![0, 0, kind, 0];
        item.extend([0u8; 10]);
        if ascii {
            item.extend(name.as_bytes());
            item.push(0);
        } else {
            item.extend(utf16(name));
            item.extend([0, 0]);
        }
        item.extend([0x1a, 0, 8, 0, 0x04, 0, 0xef, 0xbe]);
        item.extend([0u8; 14]);
        item.extend(utf16(name));
        item.extend([0, 0, 0x14, 0]);
        let size = item.len() as u16;
        item[..2].copy_from_slice(&size.to_le_bytes());
        item
    }

    fn mslnk(target: &str, working_dir: &str) -> Vec<u8> {
        let mut data = vec![0u8; 0x4C];
        data[0] = 0x4C;
        let flags = (HAS_ID_LIST | 0x8 | HAS_WORKING_DIR | IS_UNICODE) as u32;
        data[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
        let mut list = vec![
            0x14, 0x00, 0x1F, 0x50, 0xE0, 0x4F, 0xD0, 0x20, 0xEA, 0x3A, 0x69, 0x10, 0xA2, 0xD8, 0x08, 0x00,
            0x2B, 0x30, 0x30, 0x9D,
        ];
        let (drive, rest) = target.split_at(3);
        let mut drive_item = vec![0x19, 0x00, 0x2f];
        drive_item.extend(drive.as_bytes());
        drive_item.resize(25, 0);
        list.extend(drive_item);
        let parts: Vec<&str> = rest.split('\\').collect();
        for (i, part) in parts.iter().enumerate() {
            list.extend(file_item(part, i + 1 == parts.len()));
        }
        list.extend([0, 0]);
        data.extend((list.len() as u16).to_le_bytes());
        data.extend(list);
        data.extend(string_data("./swofty-launcher.exe"));
        data.extend(string_data(working_dir));
        data
    }

    #[test]
    fn reads_where_an_installer_shortcut_points() {
        let target = r"C:\Users\Jöns\Games\SwoftyLauncher\bin\swofty-launcher.exe";
        let link = read_lnk(&mslnk(target, r"C:\Users\Jöns\Games\SwoftyLauncher")).unwrap();
        assert_eq!(link.target.as_deref(), Some(target));
        assert_eq!(
            link.working_dir.as_deref(),
            Some(r"C:\Users\Jöns\Games\SwoftyLauncher")
        );
    }

    #[test]
    fn prefers_the_path_windows_records_in_link_info() {
        let mut data = vec![0u8; 0x4C];
        data[0] = 0x4C;
        data[0x14..0x18].copy_from_slice(&((HAS_LINK_INFO | IS_UNICODE) as u32).to_le_bytes());
        let base = r"D:\Games\SwoftyLauncher\bin\swofty-launcher.exe";
        let mut info = vec![0u8; 0x24];
        info[4..8].copy_from_slice(&0x24u32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        info[16..20].copy_from_slice(&0x24u32.to_le_bytes());
        info[24..28].copy_from_slice(&0x24u32.to_le_bytes());
        info.extend([b'X', 0]);
        let unicode = info.len() as u32;
        info[28..32].copy_from_slice(&unicode.to_le_bytes());
        info.extend(utf16(base));
        info.extend([0, 0]);
        let suffix = info.len() as u32;
        info[32..36].copy_from_slice(&suffix.to_le_bytes());
        info.extend([0, 0]);
        let size = info.len() as u32;
        info[..4].copy_from_slice(&size.to_le_bytes());
        data.extend(info);
        let link = read_lnk(&data).unwrap();
        assert_eq!(link.target.as_deref(), Some(base));
        assert_eq!(link.working_dir, None);
    }

    #[test]
    fn ignores_files_that_are_not_shortcuts() {
        assert_eq!(read_lnk(b"[Desktop Entry]\nExec=x\n"), None);
        let whole = mslnk(r"C:\SwoftyLauncher\bin\swofty-launcher.exe", r"C:\SwoftyLauncher");
        for cut in [0x10, 0x4C, 0x60, whole.len() - 4] {
            assert_eq!(read_lnk(&whole[..cut]), None);
        }
        assert!(lnk_roots(&whole[..0x60]).is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn reads_back_the_menu_entry_it_writes() {
        let root = PathBuf::from("/home/a b/$HOME \"q\" `x` 100%/SwoftyLauncher");
        let exe = root.join("bin/swofty-launcher");
        let entry = shortcut::entry(&root, &exe);
        assert_eq!(desktop_roots(&entry), [root.clone(), root]);
        assert_eq!(
            desktop_roots("[Desktop Entry]\nExec=/opt/SL/bin/swofty-launcher %u\nPath=\n"),
            [PathBuf::from("/opt/SL")]
        );
    }

    #[test]
    fn plans_the_release_files_and_never_carries_a_data_folder() {
        let dir = temp("plan");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        write(&from.join("notes.txt"), "mine");
        write(&from.join("Swofty/profiles/default.json"), "{}");
        let names = |m: &Move| -> Vec<String> { m.entries.iter().map(|e| e.display().to_string()).collect() };
        let release_only = ["bin", "loader", "dxc", "share", "LICENSE", "README.md"];
        let absolute = plan(&from, &to, &from.join("Swofty")).unwrap();
        assert_eq!(names(&absolute), release_only);
        assert_eq!(
            names(&plan(&from, &to, Path::new("../Swofty")).unwrap()),
            release_only
        );
        let relative = plan(&from, &to, Path::new("./Swofty")).unwrap();
        assert_eq!(names(&relative), release_only);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refuses_a_move_onto_an_install_into_itself_or_over_files() {
        let dir = temp("refuse");
        let from = dir.join("old");
        release(&from);
        let data = Path::new("/elsewhere/swofty");
        assert!(plan(&dir.join("none"), &dir.join("new"), data).is_err());
        let other = dir.join("other");
        release(&other);
        assert!(plan(&from, &other, data).is_err());
        assert!(plan(&from, &from.join("bin/deeper"), data).is_err());
        let busy = dir.join("busy");
        write(&busy.join("loader/old.dll"), "x");
        assert!(plan(&from, &busy, data).is_err());
        assert!(plan(&from, &dir.join("new"), data).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn moves_an_install_and_removes_the_old_folder() {
        let dir = temp("move");
        let (from, to) = (dir.join("old"), dir.join("deeper/new"));
        release(&from);
        let m = plan(&from, &to, Path::new("/elsewhere/swofty")).unwrap();
        m.relocate(|a, b| std::fs::rename(a, b)).unwrap();
        assert!(is_install(&to));
        assert_eq!(read(&to.join("loader/swofty_core.dll")), "core");
        assert!(!from.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn keeps_the_old_folder_while_it_holds_other_files() {
        let dir = temp("keep");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        write(&from.join("notes.txt"), "mine");
        plan(&from, &to, Path::new("/elsewhere/swofty"))
            .unwrap()
            .relocate(|a, b| std::fs::rename(a, b))
            .unwrap();
        assert!(is_install(&to));
        assert!(!to.join("notes.txt").exists());
        assert_eq!(read(&from.join("notes.txt")), "mine");
        assert_eq!(std::fs::read_dir(&from).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copies_to_another_drive_and_removes_the_old_files_after_checking() {
        let dir = temp("copy");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        let other_drive = |_: &Path, _: &Path| Err(std::io::Error::from(ErrorKind::CrossesDevices));
        plan(&from, &to, Path::new("/elsewhere/swofty"))
            .unwrap()
            .relocate(other_drive)
            .unwrap();
        assert!(is_install(&to));
        assert_eq!(read(&to.join("bin").join(update::exe("swofty"))), "cli");
        assert_eq!(
            read(&to.join("share/icons/hicolor/256x256/apps/icon.png")),
            "icon"
        );
        assert!(!from.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn puts_everything_back_when_a_step_fails() {
        let dir = temp("undo");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        let locked = |a: &Path, b: &Path| {
            if a.ends_with("share") {
                Err(std::io::Error::from(ErrorKind::PermissionDenied))
            } else {
                std::fs::rename(a, b)
            }
        };
        let err = plan(&from, &to, Path::new("/elsewhere/swofty"))
            .unwrap()
            .relocate(locked)
            .unwrap_err();
        assert!(err.contains("share"), "{err}");
        assert!(is_install(&from));
        assert_eq!(read(&from.join("loader/version.dll")), "proxy");
        assert!(!to.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn with_user_data(root: &Path) {
        release(root);
        write(&root.join("addons/swofty.progressive/swofty.json"), "{}");
        write(&root.join("profiles/default.json"), "{}");
        write(&root.join("saves/a.sav"), "save");
        write(&root.join("config.json"), "{}");
    }

    fn kept(root: &Path) {
        for rel in [
            "addons/swofty.progressive/swofty.json",
            "profiles/default.json",
            "saves/a.sav",
            "config.json",
        ] {
            assert!(root.join(rel).is_file(), "{rel} left {}", root.display());
        }
    }

    #[test]
    fn moving_the_install_leaves_addons_when_the_data_folder_is_the_install_root() {
        let dir = temp("same-root");
        let (from, to) = (dir.join("shared"), dir.join("new"));
        with_user_data(&from);
        for data in [from.clone(), from.join("."), PathBuf::from(&from)] {
            let m = plan(&from, &to, &data).unwrap();
            assert!(
                !m.entries.iter().any(|e| e == Path::new("addons")),
                "{:?}",
                m.entries
            );
        }
        plan(&from, &to, &from)
            .unwrap()
            .relocate(|a, b| std::fs::rename(a, b))
            .unwrap();
        assert!(is_install(&to));
        assert!(!to.join("addons").exists());
        kept(&from);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn moving_the_install_with_data_in_a_subfolder_leaves_it_in_place() {
        let dir = temp("subfolder");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        write(&from.join("Swofty/addons/swofty.progressive/swofty.json"), "{}");
        write(&from.join("Swofty/profiles/default.json"), "{}");
        plan(&from, &to, &from.join("Swofty"))
            .unwrap()
            .relocate(|a, b| std::fs::rename(a, b))
            .unwrap();
        assert!(is_install(&to));
        assert!(
            from.join("Swofty/addons/swofty.progressive/swofty.json")
                .is_file()
        );
        assert!(from.join("Swofty/profiles/default.json").is_file());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_absolute_data_path_is_not_carried() {
        let dir = temp("absolute");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        let m = plan(&from, &to, &from.join("Swofty")).unwrap();
        assert!(!m.entries.iter().any(|e| e == Path::new("Swofty")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn data_elsewhere_still_moves_the_bundled_addons_with_the_install() {
        let dir = temp("bundled");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        write(&from.join("addons/swofty.bundled/swofty.json"), "{}");
        let m = plan(&from, &to, &dir.join("data")).unwrap();
        assert!(m.entries.iter().any(|e| e == Path::new("addons")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_existing_install_is_always_moved_unless_a_second_copy_is_asked_for() {
        let dir = temp("always-moves");
        let (from, to) = (dir.join("old"), dir.join("new"));
        release(&from);
        let mut existing = Existing {
            found: vec![from.clone()],
            picked: 0,
            keep: false,
        };
        assert!(existing.planned(&to).unwrap().is_some());
        assert!(existing.planned(&from).unwrap().is_none());
        assert!(existing.differs_from(&to));
        existing.keep = true;
        assert!(existing.planned(&to).unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
