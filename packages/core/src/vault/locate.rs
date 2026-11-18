//! Where a profile save lives: Steam's `userdata/<id3>/3059520/remote/` with
//! `savegame@profile#<file>` names, or the EA app copy's
//! `%LOCALAPPDATA%\F1 25\savegame\ea\profile\<file>`. Both map onto the
//! vault's `profile/<file>` keys.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::copies::GameCopy;
use crate::install::{Game, Source};
use crate::session::Config;
use crate::{Result, io_err};

pub const STEAM_PREFIX: &str = "savegame@profile#";
pub const KEY_PREFIX: &str = "profile/";
const EA_PROFILE: &[&str] = &["F1 25", "savegame", "ea", "profile"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    Steam,
    Ea,
    Launcher,
    LauncherEa,
}

impl Layout {
    pub fn named_like_steam(self) -> bool {
        matches!(self, Self::Steam | Self::Launcher)
    }

    pub fn exchanges_with(self, other: Self) -> bool {
        self == other
            || (self.named_like_steam() && other.named_like_steam())
            || (matches!(self, Self::Ea | Self::LauncherEa) && matches!(other, Self::Ea | Self::LauncherEa))
    }

    pub fn is_launcher(self) -> bool {
        matches!(self, Self::Launcher | Self::LauncherEa)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub source: String,
    pub layout: Layout,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    pub key: String,
    pub path: PathBuf,
}

fn safe(name: &str) -> String {
    name.chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                true => c,
                false => '_',
            },
        )
        .collect()
}

impl Location {
    /// A Steam `remote` folder: the account is the `userdata/<id3>` above it.
    pub fn steam(remote: &Path) -> Option<Self> {
        let app = remote.parent()?;
        let id3 = app.parent()?.file_name()?.to_str()?;
        let userdata = app.parent()?.parent()?.file_name()?.to_str()?;
        (userdata.eq_ignore_ascii_case("userdata")
            && id3.chars().all(|c| c.is_ascii_digit())
            && !id3.is_empty())
        .then(|| Self {
            source: format!("steam-{id3}"),
            layout: Layout::Steam,
            dir: remote.to_path_buf(),
        })
    }

    /// A launcher profile's own store, `$DATA/saves/<profile>/remote`: the
    /// files carry the names Steam's remote storage gives them.
    pub fn launcher(data: &Path, profile: &str) -> Self {
        Self {
            source: format!("launcher-{}", safe(profile)),
            layout: Layout::Launcher,
            dir: crate::launcher_saves::remote_dir(data, profile),
        }
    }

    /// A launcher profile's store for an EA app copy, `$DATA/saves/<profile>/ea/profile`: the files
    /// carry the names the EA app copy's save folder gives them.
    pub fn launcher_ea(data: &Path, profile: &str) -> Self {
        Self {
            source: format!("launcher-ea.{}", safe(profile)),
            layout: Layout::LauncherEa,
            dir: crate::launcher_saves::ea_dir(data, profile),
        }
    }

    /// The launcher store a vault source name stands for (`launcher-<profile>`, `launcher-ea.<profile>`).
    pub fn of_source(data: &Path, source: &str) -> Option<Self> {
        if let Some(p) = source.strip_prefix("launcher-ea.") {
            return Some(Self::launcher_ea(data, p));
        }
        source.strip_prefix("launcher-").map(|p| Self::launcher(data, p))
    }

    pub fn ea(profile: &Path, user: &str) -> Self {
        Self {
            source: format!("ea-{}", safe(user)),
            layout: Layout::Ea,
            dir: profile.to_path_buf(),
        }
    }

    /// The EA app copy's profile folder under a Windows user's
    /// `AppData\Local`.
    pub fn ea_in_local_appdata(local: &Path, user: &str) -> Self {
        Self::ea(
            &EA_PROFILE.iter().fold(local.to_path_buf(), |p, s| p.join(s)),
            user,
        )
    }

    /// The EA app copy's profile folder inside a Wine prefix (`pfx`, the
    /// folder with `drive_c`): the user whose folder holds one.
    pub fn ea_in_prefix(pfx: &Path) -> Option<Self> {
        let users = pfx.join("drive_c/users");
        let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(&users)
            .ok()?
            .flatten()
            .filter_map(|u| {
                let name = u.file_name().to_str()?.to_owned();
                let local = u.path().join("AppData/Local");
                let at = EA_PROFILE.iter().fold(local, |p, s| p.join(s));
                at.is_dir().then_some((name, at))
            })
            .filter(|(name, _)| !name.eq_ignore_ascii_case("public"))
            .collect();
        found.sort();
        let (user, _) = found.into_iter().next()?;
        Some(Self::ea_in_local_appdata(
            &users.join(&user).join("AppData/Local"),
            &user,
        ))
    }

    #[cfg(windows)]
    pub fn ea_windows() -> Option<Self> {
        let local = std::env::var_os("LOCALAPPDATA")?;
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        let loc = Self::ea_in_local_appdata(Path::new(&local), &user);
        loc.dir.is_dir().then_some(loc)
    }

    #[cfg(not(windows))]
    pub fn ea_windows() -> Option<Self> {
        None
    }

    /// The save of a game copy: Steam's for a Steam copy, the EA app's local
    /// copy for an EA app copy (through its Wine prefix on Linux).
    pub fn for_game(g: &Game) -> Option<Self> {
        match Source::detect(&g.dir) {
            Source::EaApp => match cfg!(windows) {
                true => Self::ea_windows(),
                false => g
                    .user_reg
                    .as_deref()
                    .and_then(Path::parent)
                    .and_then(Self::ea_in_prefix),
            },
            _ => Self::steam_for(&g.dir),
        }
    }

    /// The Steam save folder for the Steam install that holds the game in
    /// `game`, else the first one found.
    pub fn steam_for(game: &Path) -> Option<Self> {
        Self::steam_search(Some(game)).0
    }

    /// [`Self::steam_for`] with a line for every folder looked at and why it was rejected.
    pub fn steam_search(game: Option<&Path>) -> (Option<Self>, Vec<String>) {
        Self::from_search(crate::steam::search_save(game))
    }

    pub fn steam_from(game: &Path, roots: &[PathBuf]) -> (Option<Self>, Vec<String>) {
        Self::from_search(crate::steam::search_save_in(Some(game), roots, &[], None))
    }

    fn from_search(found: crate::steam::SaveSearch) -> (Option<Self>, Vec<String>) {
        let mut tried = found.tried;
        let loc = found.dir.and_then(|d| match Self::steam(&d) {
            Some(l) => Some(l),
            None => {
                tried.push(format!(
                    "{}: not a userdata/<id>/<app>/remote folder",
                    d.display()
                ));
                None
            }
        });
        (loc, tried)
    }

    /// The save a profile boots: its own launcher save with Use launcher saves on, else the active
    /// copy's game save (Steam or EA app, also for Vanilla). An EA app copy has no launcher saves yet,
    /// so every profile boots its one save.
    pub fn of_profile(data: &Path, profile: &str) -> Option<Self> {
        let store = crate::store::Store::new(data.to_path_buf());
        let launcher =
            profile != crate::profile::VANILLA && store.profile(profile).map_or(true, |p| p.launcher_saves);
        let config = store.config();
        let copies = crate::copies::discover(&config);
        let copy = crate::copies::active(&copies, &config);
        Self::choose(data, profile, launcher, copy.map(|c| c.source), || {
            Self::of_copy(&config, copy).0
        })
    }

    fn choose(
        data: &Path,
        profile: &str,
        launcher: bool,
        copy: Option<Source>,
        active: impl FnOnce() -> Option<Self>,
    ) -> Option<Self> {
        match (launcher, copy) {
            (true, Some(Source::EaApp)) if crate::launcher_saves::supported(Layout::Ea) => {
                Some(Self::launcher_ea(data, profile))
            }
            (true, Some(Source::EaApp)) | (false, _) => active(),
            (true, _) => Some(Self::launcher(data, profile)),
        }
    }

    /// The vault source of [`Self::of_profile`].
    pub fn source_of_profile(data: &Path, profile: &str) -> Option<String> {
        Self::of_profile(data, profile).map(|l| l.source)
    }

    /// The save of the copy Play uses.
    pub fn active(data: &Path) -> Option<Self> {
        Self::active_search(data).0
    }

    /// [`Self::active`] with the folders a Steam copy's lookup tried.
    pub fn active_search(data: &Path) -> (Option<Self>, Vec<String>) {
        let config = crate::store::Store::new(data.to_path_buf()).config();
        let copies = crate::copies::discover(&config);
        Self::of_copy(&config, crate::copies::active(&copies, &config))
    }

    fn of_copy(config: &Config, copy: Option<&GameCopy>) -> (Option<Self>, Vec<String>) {
        let Some(copy) = copy else {
            return Self::steam_search(None);
        };
        let loc = match copy.source {
            Source::EaApp => match cfg!(windows) {
                true => Self::ea_windows(),
                false => config
                    .launch
                    .ea_prefix
                    .as_deref()
                    .and_then(crate::ea_app::prefix_user_reg)
                    .as_deref()
                    .and_then(Path::parent)
                    .and_then(Self::ea_in_prefix),
            },
            _ => return Self::steam_search(Some(&copy.dir)),
        };
        (loc, Vec::new())
    }

    /// Steam's record of what it last synced, beside `remote/`.
    pub fn remotecache(&self) -> Option<PathBuf> {
        match self.layout {
            Layout::Steam => Some(self.dir.parent()?.join("remotecache.vdf")),
            Layout::Ea | Layout::Launcher | Layout::LauncherEa => None,
        }
    }

    pub fn key_of(&self, file_name: &str) -> Option<String> {
        let rest = match self.layout {
            Layout::Steam | Layout::Launcher => file_name.strip_prefix(STEAM_PREFIX)?,
            Layout::Ea | Layout::LauncherEa => file_name,
        };
        (!rest.is_empty() && !rest.starts_with('.')).then(|| format!("{KEY_PREFIX}{rest}"))
    }

    pub fn name_of(&self, key: &str) -> Option<String> {
        let rest = key.strip_prefix(KEY_PREFIX)?;
        if rest.is_empty() || rest.contains(['/', '\\']) || rest.starts_with('.') {
            return None;
        }
        Some(match self.layout {
            Layout::Steam | Layout::Launcher => format!("{STEAM_PREFIX}{rest}"),
            Layout::Ea | Layout::LauncherEa => rest.to_owned(),
        })
    }

    pub fn path_of(&self, key: &str) -> Option<PathBuf> {
        self.name_of(key).map(|n| self.dir.join(n))
    }

    /// The profile save's files, by key. Highlights, ghosts and mid-session
    /// saves stay out.
    pub fn files(&self) -> Result<Vec<SaveFile>> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return match self.dir.exists() {
                true => Err(io_err(&self.dir)(std::io::Error::other(
                    "cannot read the save folder",
                ))),
                false => Ok(Vec::new()),
            };
        };
        let mut out: Vec<SaveFile> = rd
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_owned();
                Some(SaveFile {
                    key: self.key_of(&name)?,
                    path: e.path(),
                })
            })
            .collect();
        out.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-vault-locate-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn steam_layout_keys_profile_files_only() {
        let root = tmp("steam");
        let remote = root.join("userdata/843502855/3059520/remote");
        std::fs::create_dir_all(&remote).unwrap();
        for n in [
            "savegame@profile#NXDSMWW.BWW",
            "savegame@profile#QKRHMYXE",
            "savegame@highlights001#QKRHMYXE",
            "corrupt_saves#76561198803768583.zip",
        ] {
            std::fs::write(remote.join(n), b"x").unwrap();
        }
        let loc = Location::steam(&remote).unwrap();
        assert_eq!(loc.source, "steam-843502855");
        assert_eq!(
            loc.remotecache(),
            Some(root.join("userdata/843502855/3059520/remotecache.vdf"))
        );
        let keys: Vec<String> = loc.files().unwrap().into_iter().map(|f| f.key).collect();
        assert_eq!(keys, ["profile/NXDSMWW.BWW", "profile/QKRHMYXE"]);
        assert_eq!(
            loc.path_of("profile/QKRHMYXE"),
            Some(remote.join("savegame@profile#QKRHMYXE"))
        );
        assert_eq!(loc.path_of("profile/../x"), None);
        assert_eq!(Location::steam(&root.join("remote")), None);
    }

    #[test]
    fn ea_layout_in_a_wine_prefix() {
        let pfx = tmp("ea");
        let profile = pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::create_dir_all(pfx.join("drive_c/users/Public")).unwrap();
        std::fs::create_dir_all(profile.parent().unwrap().join("highlights000")).unwrap();
        std::fs::write(profile.join("NXDSMWW.BWW"), b"data").unwrap();
        std::fs::write(profile.join("QKRHMYXE"), b"index").unwrap();
        let loc = Location::ea_in_prefix(&pfx).unwrap();
        assert_eq!(loc.source, "ea-steamuser");
        assert_eq!(loc.layout, Layout::Ea);
        assert_eq!(loc.dir, profile);
        assert_eq!(loc.remotecache(), None);
        let keys: Vec<String> = loc.files().unwrap().into_iter().map(|f| f.key).collect();
        assert_eq!(keys, ["profile/NXDSMWW.BWW", "profile/QKRHMYXE"]);
        assert_eq!(loc.path_of("profile/QKRHMYXE"), Some(profile.join("QKRHMYXE")));
        assert_eq!(Location::ea_in_prefix(&pfx.join("nothing")), None);
    }

    #[test]
    fn profile_save_follows_launcher_saves() {
        let d = tmp("choose");
        let steam = Location::steam(&{
            let r = d.join("userdata/843502855/3059520/remote");
            std::fs::create_dir_all(&r).unwrap();
            r
        })
        .unwrap();
        let pfx = tmp("choose-ea");
        let ea_dir = pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
        std::fs::create_dir_all(&ea_dir).unwrap();
        std::fs::create_dir_all(pfx.join("drive_c/users/Public")).unwrap();
        let ea = Location::ea_in_prefix(&pfx).unwrap();

        let steam_copy = Some(Source::Steam);
        let ea_copy = Some(Source::EaApp);
        let on = Location::choose(&d, "Race", true, steam_copy, || Some(steam.clone())).unwrap();
        assert_eq!(on.source, "launcher-Race");
        let on_unknown = Location::choose(&d, "Race", true, None, || Some(steam.clone())).unwrap();
        assert_eq!(on_unknown.source, "launcher-Race");
        let off = Location::choose(&d, "Race", false, steam_copy, || Some(steam.clone())).unwrap();
        assert_eq!(off.source, "steam-843502855");
        let off_ea = Location::choose(&d, "Race", false, ea_copy, || Some(ea.clone())).unwrap();
        assert_eq!(off_ea.source, "ea-steamuser");
        assert_eq!(off_ea.layout, Layout::Ea);
        assert!(Location::choose(&d, "Race", false, steam_copy, || None).is_none());
    }

    #[test]
    fn every_profile_boots_its_own_save_of_an_ea_copy() {
        let d = tmp("choose-shared");
        let pfx = tmp("choose-shared-ea");
        std::fs::create_dir_all(pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile"))
            .unwrap();
        let ea = Location::ea_in_prefix(&pfx).unwrap();
        let off = Location::choose(&d, "hi", false, Some(Source::EaApp), || Some(ea.clone())).unwrap();
        assert_eq!(off, ea);
        let on = Location::choose(&d, "hi", true, Some(Source::EaApp), || None).unwrap();
        assert_eq!(on, Location::launcher_ea(&d, "hi"));
    }
}
