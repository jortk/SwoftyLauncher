use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use swofty_addon::launcher::Builtin;

use crate::builtin::{CORE_ID, core_manifest};
use crate::helper::Helpers;
use crate::manifest::{AddonManifest, MANIFEST_FILE};
use crate::profile::{self, Profile};
use crate::session::Config;
use crate::settings::{self, Merge, Requests, SessionSettings};
use crate::{Error, Result, io_err, with_retry};

pub const DEFAULT_PROFILE: &str = "default";
pub const DATA_VERSION: u32 = 4;
const LEGACY_ADDONS_DIR: &str = "mods";

static PROFILE_WRITE: Mutex<()> = Mutex::new(());

/// Where an addon's old copy waits to be deleted, next to the addons.
const OLD_PREFIX: &str = ".old-";
const STAGING_PREFIX: &str = ".installing-";

type Rename<'a> = &'a mut dyn FnMut(&Path, &Path) -> std::io::Result<()>;
type Delete<'a> = &'a mut dyn FnMut(&Path) -> std::io::Result<()>;

/// Runs a folder move or delete, waiting out a file that is still briefly in
/// use once its helper has exited.
fn waiting_out_locks(mut op: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
    const DELAYS_MS: [u64; 5] = [50, 150, 300, 600, 1000];
    let mut attempt = 0;
    loop {
        match op() {
            Err(e) if crate::is_locked_file_error(&e) && attempt < DELAYS_MS.len() => {
                std::thread::sleep(std::time::Duration::from_millis(DELAYS_MS[attempt]));
                attempt += 1;
            }
            r => return r,
        }
    }
}

fn not_done(what: &str, e: &std::io::Error) -> Error {
    Error::Other(if crate::is_locked_file_error(e) {
        format!(
            "{what}: its files are in use. Close other Swofty Launcher windows and F1 25, then try again."
        )
    } else {
        format!("{what}: {e}")
    })
}

fn swap_lock_path(addons: &Path, id: &str) -> PathBuf {
    addons.join(format!(".swap-{id}.lock"))
}

/// Held while one process swaps an addon folder, so another launcher's
/// start-up cleanup leaves the half-finished swap alone. The operating system
/// drops it when the process ends, which is how a later start tells a dead
/// swap from a live one. Where the file system cannot lock, swaps go unguarded.
struct SwapLock {
    _file: std::fs::File,
}

impl SwapLock {
    fn try_acquire(addons: &Path, id: &str) -> Option<Option<Self>> {
        let path = swap_lock_path(addons, id);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .ok()?;
        match file.try_lock() {
            Err(std::fs::TryLockError::WouldBlock) => Some(None),
            Ok(()) | Err(std::fs::TryLockError::Error(_)) => Some(Some(Self { _file: file })),
        }
    }
}

fn swap_lock_for(dest: &Path, name: &str) -> Result<Option<SwapLock>> {
    let (Some(addons), Some(id)) = (dest.parent(), dest.file_name()) else {
        return Ok(None);
    };
    match SwapLock::try_acquire(addons, &id.to_string_lossy()) {
        Some(None) => Err(Error::Other(format!(
            "Couldn't change {name}: another Swofty launcher is changing it right now"
        ))),
        Some(Some(lock)) => Ok(Some(lock)),
        None => Ok(None),
    }
}

/// Moves `dir` to a hidden `.old-` folder beside it. A file still in use stops
/// the move, and then nothing in `dir` has changed.
fn set_aside(dir: &Path, rename: Rename) -> std::io::Result<PathBuf> {
    let parent = dir.parent().unwrap_or(dir);
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    let aside = (0..)
        .map(|n| match n {
            0 => parent.join(format!("{OLD_PREFIX}{name}")),
            n => parent.join(format!("{OLD_PREFIX}{name}-{n}")),
        })
        .find(|p| !p.exists())
        .expect("a free name");
    waiting_out_locks(|| rename(dir, &aside))?;
    Ok(aside)
}

/// Puts `staging` in place of `dest`. The installed copy is moved aside first
/// and deleted last, so a failure at any step leaves it whole. An old copy that
/// can't be deleted yet is deleted on the next start; one that can't be put
/// back is put back then.
fn swap_in(staging: &Path, dest: &Path, name: &str, rename: Rename, delete: Delete) -> Result<()> {
    let verb = if dest.exists() { "update" } else { "install" };
    let what = format!("Couldn't {verb} {name}");
    let _lock = swap_lock_for(dest, name)?;
    let aside = match dest.exists().then(|| set_aside(dest, &mut *rename)).transpose() {
        Ok(aside) => aside,
        Err(e) => {
            std::fs::remove_dir_all(staging).ok();
            return Err(not_done(&what, &e));
        }
    };
    if let Err(e) = waiting_out_locks(|| rename(staging, dest)) {
        let restored = aside
            .as_deref()
            .is_none_or(|aside| waiting_out_locks(|| rename(aside, dest)).is_ok());
        if restored {
            std::fs::remove_dir_all(staging).ok();
        }
        return Err(not_done(&what, &e));
    }
    if let Some(aside) = aside {
        waiting_out_locks(|| delete(&aside)).ok();
    }
    Ok(())
}

/// Deletes an addon's folder whole or not at all: it is moved aside first, so
/// a file still in use stops the removal before anything is deleted. An old
/// copy that can't be deleted yet is deleted on the next start.
fn delete_addon_dir(dir: &Path, name: &str, rename: Rename, delete: Delete) -> Result<()> {
    let _lock = swap_lock_for(dir, name)?;
    let aside = set_aside(dir, rename).map_err(|e| not_done(&format!("Couldn't remove {name}"), &e))?;
    waiting_out_locks(|| delete(&aside)).ok();
    Ok(())
}

fn profile_write() -> MutexGuard<'static, ()> {
    PROFILE_WRITE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Where an addon being installed came from, which decides what it may be.
#[derive(Debug, Clone, Copy)]
enum InstallFrom<'a> {
    /// A file the player chose.
    Other,
    /// A folder the helper of the addon with this id asked to install.
    Helper(&'a str),
    /// A download the catalog lists as this id and version.
    Catalog(&'a str, &'a semver::Version),
    /// A release of the addon the launcher manages.
    ManagedRelease,
}

#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
    builtins: Arc<[Builtin]>,
    helpers: Arc<Helpers>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileDeletion {
    pub name: String,
    pub switch_to: Option<String>,
    pub saves: bool,
    pub paths: Vec<PathBuf>,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            builtins: Arc::new([]),
            helpers: Arc::default(),
        }
    }

    pub fn with_builtins(mut self, builtins: Vec<Builtin>) -> Self {
        self.builtins = builtins.into();
        self
    }

    pub fn builtins(&self) -> &[Builtin] {
        &self.builtins
    }

    pub fn helpers(&self) -> &Helpers {
        &self.helpers
    }

    pub fn registry(&self) -> Result<crate::registry::Registry> {
        crate::registry::Registry::load(self)
    }

    pub fn open_default() -> Self {
        Self::new(crate::paths::data_dir())
    }

    pub fn addons_dir(&self) -> PathBuf {
        self.root.join("addons")
    }

    /// Personal shader configs and the shader settings.
    pub fn shaders_dir(&self) -> PathBuf {
        self.root.join("shaders")
    }

    /// Compiled effects and the extracted effect library.
    pub fn shaders_cache_dir(&self) -> PathBuf {
        self.root.join("cache/shaders")
    }

    pub fn addon_data_dir(&self, id: &str) -> PathBuf {
        self.root.join("addon-data").join(id)
    }

    pub fn addon_cache_dir(&self, id: &str) -> PathBuf {
        self.root.join("cache/addons").join(id)
    }

    /// Where an Addon Browser install records the download it installed `id` from; any other
    /// install or removal of `id` deletes it.
    pub fn registry_install_path(&self, id: &str) -> PathBuf {
        self.root
            .join("state/registry-installs")
            .join(format!("{id}.json"))
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    pub fn overlays_dir(&self, profile: &str) -> PathBuf {
        overlay_root(&self.root).join(profile)
    }

    pub fn overlay_dir(&self, profile: &str, buildhash: &str) -> PathBuf {
        self.overlays_dir(profile).join(buildhash)
    }

    pub fn saves_dir(&self, profile: &str) -> PathBuf {
        self.root.join("saves").join(profile)
    }

    pub fn baseline_dir(&self, build: &str) -> PathBuf {
        self.root.join("baseline").join(build)
    }

    pub fn journal_dir(&self) -> PathBuf {
        self.root.join("state/journal")
    }

    pub fn requests_dir(&self) -> PathBuf {
        self.root.join("state/requests")
    }

    pub fn launch_request_path(&self) -> PathBuf {
        self.root.join("state/launch.json")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn migrate(&self) -> Result<Vec<String>> {
        let mut notes = Vec::new();
        if let Some(target) = crate::paths::dangling_link(&self.root) {
            return Err(Error::Other(format!(
                "the data folder {} links to {}, which is missing. Reconnect that drive or restore the folder; the launcher will not start a new data folder in its place",
                self.root.display(),
                target.display()
            )));
        }
        if !self.root.is_dir() {
            return Ok(notes);
        }
        notes.extend(self.finish_swaps());
        let config: Result<Config> = match crate::load_json(&self.config_path()) {
            Ok(crate::Loaded::Found(c)) => Ok(c),
            Ok(crate::Loaded::Missing | crate::Loaded::Damaged(_)) => Ok(Config::default()),
            Err(e) => Err(e),
        };
        if config.as_ref().is_ok_and(|c| c.data_version >= DATA_VERSION) {
            return Ok(notes);
        }
        let old = self.root.join(LEGACY_ADDONS_DIR);
        let new = self.addons_dir();
        if old.is_dir() && !new.exists() {
            std::fs::rename(&old, &new).map_err(io_err(&old))?;
            notes.push(format!("moved {} to {}", old.display(), new.display()));
        } else if old.is_dir() {
            let taken: BTreeSet<String> = subdirs(&new)?.iter().filter_map(|d| manifest_id(d)).collect();
            for dir in subdirs(&old)? {
                let to = new.join(dir.file_name().expect("has a name"));
                match manifest_id(&dir) {
                    Some(id) if !taken.contains(&id) && !to.exists() => {
                        std::fs::rename(&dir, &to).map_err(io_err(&dir))?;
                        notes.push(format!("moved {} to {}", dir.display(), to.display()));
                    }
                    Some(id) => notes.push(format!(
                        "kept {}: {} already has {id}",
                        dir.display(),
                        new.display()
                    )),
                    None => notes.push(format!(
                        "kept {}: no addon id in its {MANIFEST_FILE}",
                        dir.display()
                    )),
                }
            }
            if std::fs::remove_dir(&old).is_ok() {
                notes.push(format!("removed the empty {}", old.display()));
            }
        }
        let from = config.as_ref().map_or(0, |c| c.data_version);
        for b in self.builtins.iter() {
            let id = &b.addon.manifest().id;
            match b.addon.migrate(&self.root, from) {
                Ok(n) => notes.extend(n.into_iter().map(|n| format!("{id}: {n}"))),
                Err(e) => return Err(Error::Other(format!("migrating {id}: {e}"))),
            }
        }
        let backfilled = match config
            .as_ref()
            .map_or(Ok(Vec::new()), |_| self.backfill_profile_addons(from))
        {
            Ok(n) => {
                notes.extend(n);
                true
            }
            Err(e) => {
                let text = format!("couldn't list installed addons in the profiles, will retry: {e}");
                warn(text.clone());
                notes.push(text);
                false
            }
        };
        match config {
            Ok(_) if !backfilled => {}
            Ok(mut config) => {
                config.data_version = DATA_VERSION;
                self.save_config(&config)?;
            }
            Err(e) => notes.push(format!("data_version not recorded: {e}")),
        }
        Ok(notes)
    }

    /// Data version 4: a profile lists every installed addon, as the Addons page showed
    /// them before profiles listed their own. Each one a profile doesn't list is added off,
    /// at the end, in the order that page showed them; the profile's own entries stay as
    /// they are.
    fn backfill_profile_addons(&self, from: u32) -> Result<Vec<String>> {
        let mut notes = Vec::new();
        if from >= 4 {
            return Ok(notes);
        }
        let mut installed: Vec<AddonManifest> = self
            .addons()?
            .into_values()
            .filter(|m| {
                !m.is_builtin() && !self.is_builtin(&m.id) && m.id != crate::managed::ID && m.id != CORE_ID
            })
            .collect();
        installed.sort_by_cached_key(|m| m.name.to_lowercase());
        if installed.is_empty() {
            return Ok(notes);
        }
        for name in self.profile_names()? {
            if name == profile::VANILLA {
                continue;
            }
            let added = self.update_profile_if(&name, |p| {
                let mut added = Vec::new();
                for m in &installed {
                    if !p.has(&m.id) {
                        p.add_addon(&m.id)?;
                        added.push(m.id.clone());
                    }
                }
                Ok(added)
            })?;
            if !added.is_empty() {
                notes.push(format!(
                    "listed {} installed addons in the {name} profile",
                    added.len()
                ));
            }
        }
        Ok(notes)
    }

    fn update_profile_if(
        &self,
        name: &str,
        f: impl FnOnce(&mut Profile) -> Result<Vec<String>>,
    ) -> Result<Vec<String>> {
        let _write = profile_write();
        let mut p = self.profile(name)?;
        let added = f(&mut p)?;
        if !added.is_empty() {
            self.save_profile(&p)?;
        }
        Ok(added)
    }

    /// Finishes addon updates and removals an earlier run left: an old copy
    /// whose update stopped before the new copy was in place is put back, and
    /// any other old copy is deleted.
    fn finish_swaps(&self) -> Vec<String> {
        let addons = self.addons_dir();
        let Ok(rd) = std::fs::read_dir(&addons) else {
            return Vec::new();
        };
        let mut old: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(OLD_PREFIX))
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        old.sort();
        let mut notes = Vec::new();
        for dir in old {
            let stem = dir
                .file_name()
                .map(|n| n.to_string_lossy().trim_start_matches(OLD_PREFIX).to_owned())
                .unwrap_or_default();
            let base = match stem.rsplit_once('-') {
                Some((b, n)) if !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) => b.to_owned(),
                _ => stem.clone(),
            };
            let mut names = vec![stem, base];
            names.extend(manifest_id(&dir));
            names.sort();
            names.dedup();
            let mut guards = Vec::new();
            let mut busy = false;
            for n in &names {
                match SwapLock::try_acquire(&addons, n) {
                    Some(None) => busy = true,
                    Some(Some(g)) => guards.push(g),
                    None => {}
                }
            }
            if busy {
                notes.push(format!(
                    "left {} alone: another launcher is changing it",
                    dir.display()
                ));
                continue;
            }
            let interrupted = manifest_id(&dir).filter(|id| {
                !addons.join(id).exists() && addons.join(format!("{STAGING_PREFIX}{id}")).is_dir()
            });
            if let Some(id) = interrupted {
                let to = addons.join(&id);
                if std::fs::rename(&dir, &to).is_ok() {
                    std::fs::remove_dir_all(addons.join(format!("{STAGING_PREFIX}{id}"))).ok();
                    notes.push(format!("put back {id}: its update stopped part way"));
                }
            } else if std::fs::remove_dir_all(&dir).is_ok() {
                notes.push(format!("deleted the old copy {}", dir.display()));
            }
        }
        notes
    }

    /// The installed addons this launcher can read. One it can't is left out with a warning;
    /// [`unreadable_addons`](Self::unreadable_addons) lists them.
    pub fn addons(&self) -> Result<BTreeMap<String, AddonManifest>> {
        Ok(self.scan_addons()?.0)
    }

    /// Installed addons this launcher can't read: made for a newer launcher, or damaged.
    pub fn unreadable_addons(&self) -> Vec<Unreadable> {
        self.scan_addons().map(|(_, bad)| bad).unwrap_or_default()
    }

    fn scan_addons(&self) -> Result<(BTreeMap<String, AddonManifest>, Vec<Unreadable>)> {
        let mut out = BTreeMap::from([(CORE_ID.to_owned(), core_manifest())]);
        for b in self.builtins.iter() {
            let m = b.addon.manifest();
            out.entry(m.id.clone()).or_insert_with(|| m.clone());
        }
        let mut bad = Vec::new();
        for dir in subdirs(&self.addons_dir())? {
            if !dir.join(MANIFEST_FILE).is_file() {
                continue;
            }
            let m = match AddonManifest::load(&dir) {
                Ok(m) => m,
                Err(e) => {
                    let u = Unreadable::of(&dir, &e, &crate::update::current_version());
                    warn_once(format!("skipped addon {}: it {}", u.id, u.reason));
                    bad.push(u);
                    continue;
                }
            };
            if self.is_builtin(&m.id) {
                continue;
            }
            out.insert(m.id.clone(), m);
        }
        Ok((out, bad))
    }

    pub fn is_builtin(&self, id: &str) -> bool {
        id == CORE_ID || self.builtins.iter().any(|b| b.addon.manifest().id == id)
    }

    fn adopt_builtins(&self, p: &mut Profile) {
        if p.is_vanilla() {
            return;
        }
        for b in self.builtins.iter() {
            let id = &b.addon.manifest().id;
            let listed = p.addons.iter().any(|a| &a.id == id);
            if b.locked || (b.default_on && !listed) {
                p.set_enabled(id, true).expect("not vanilla");
            }
        }
        if self
            .addons_dir()
            .join(crate::managed::ID)
            .join(MANIFEST_FILE)
            .is_file()
        {
            p.set_enabled(crate::managed::ID, true).expect("not vanilla");
        }
    }

    pub fn install_addon(&self, src: &Path) -> Result<AddonManifest> {
        self.install_addon_for(src, &crate::update::current_version())
    }

    /// Every addon `src` holds: a zip or rar may carry several addon folders
    /// and packages.
    pub fn install_addons(&self, src: &Path) -> Vec<Result<AddonManifest>> {
        let launcher = crate::update::current_version();
        if !src.is_file() {
            return vec![self.install_addon_for(src, &launcher)];
        }
        let scratch = self.scratch_dir();
        let installed = match self.unpacked_addons(src, &scratch) {
            Ok(found) => found
                .iter()
                .map(|addon| self.install_addon_for(addon, &launcher))
                .collect(),
            Err(e) => vec![Err(e)],
        };
        std::fs::remove_dir_all(&scratch).ok();
        installed
    }

    pub fn install_addon_for(&self, src: &Path, launcher: &semver::Version) -> Result<AddonManifest> {
        self.install_addon_target(src, launcher, None, InstallFrom::Other)
    }

    /// Install into a specific existing profile rather than whichever profile is active later.
    pub fn install_addon_in_profile(&self, src: &Path, profile: &str) -> Result<AddonManifest> {
        self.install_addon_target(
            src,
            &crate::update::current_version(),
            Some(profile),
            InstallFrom::Other,
        )
    }

    /// Install a folder the helper of the addon `helper` made. An addon the Simplified Importer
    /// made may be installed under another id ([`crate::imported::install_id`]).
    pub(crate) fn install_addon_by(&self, src: &Path, helper: &str) -> Result<AddonManifest> {
        self.install_addon_target(
            src,
            &crate::update::current_version(),
            None,
            InstallFrom::Helper(helper),
        )
    }

    /// Install what the catalog lists as `id` at `version`, into `profile` when one is named.
    pub(crate) fn install_listed(
        &self,
        src: &Path,
        profile: Option<&str>,
        id: &str,
        version: &semver::Version,
    ) -> Result<AddonManifest> {
        self.install_addon_target(
            src,
            &crate::update::current_version(),
            profile,
            InstallFrom::Catalog(id, version),
        )
    }

    /// Install a release of the addon the launcher manages.
    pub(crate) fn install_managed_release(
        &self,
        src: &Path,
        launcher: &semver::Version,
    ) -> Result<AddonManifest> {
        self.install_addon_target(src, launcher, None, InstallFrom::ManagedRelease)
    }

    fn install_addon_target(
        &self,
        src: &Path,
        launcher: &semver::Version,
        profile: Option<&str>,
        from: InstallFrom,
    ) -> Result<AddonManifest> {
        if src.is_file() {
            let scratch = self.scratch_dir();
            let result = self
                .unpacked_addons(src, &scratch)
                .and_then(|found| match found.as_slice() {
                    [one] => self.install_addon_target(one, launcher, profile, from),
                    many => Err(Error::Other(format!(
                        "{} holds {} addons; drop it on the launcher to install them all",
                        src.display(),
                        many.len()
                    ))),
                });
            std::fs::remove_dir_all(&scratch).ok();
            return result;
        }
        let src = &crate::import::addon_root(src).unwrap_or_else(|| src.to_path_buf());
        if src.join(crate::dbedit::package::MARKER).is_file() {
            return crate::dbedit::package::install(self, src).map(|(p, _)| {
                std::fs::remove_file(self.registry_install_path(&p.manifest.id)).ok();
                self.add_to_install_profile(&p.manifest.id, profile);
                p.manifest
            });
        }
        let mut m = AddonManifest::load(src).map_err(|e| {
            let u = Unreadable::of(src, &e, launcher);
            if u.newer {
                Error::Other(u.line())
            } else {
                Error::from(e)
            }
        })?;
        if self.is_builtin(&m.id) {
            return Err(Error::Other(format!("{} is built in", m.id)));
        }
        if let InstallFrom::Catalog(id, version) = from
            && (m.id != id || m.version != *version)
        {
            return Err(Error::Other(format!(
                "the registry lists {id} {version}, the download holds {} {}",
                m.id, m.version
            )));
        }
        if matches!(from, InstallFrom::Catalog(..)) {
            crate::imported::refuse_listed(self, &m)?;
        }
        let id = match from {
            InstallFrom::Helper(helper) if crate::imported::made_by(helper, &m) => {
                crate::imported::install_id(self, src, &m)?
            }
            _ => m.id.clone(),
        };
        if crate::managed::is_managed(&m.id) && !matches!(from, InstallFrom::ManagedRelease) {
            return Err(Error::Other(format!(
                "{} is managed by the launcher, which installs and updates it itself",
                m.id
            )));
        }
        let prefix = crate::dbedit::package::ID_PREFIX;
        if crate::dbedit::package::is_package(self, &m.id) {
            return Err(Error::Other(format!(
                "{} is a DB Editor addon you have; installing over it would delete its edits",
                m.id
            )));
        }
        if m.id.starts_with(prefix) {
            return Err(Error::Other(format!(
                "{}: the {prefix} prefix is kept for addons the DB Editor saves",
                m.id
            )));
        }
        if let Some(why) = crate::resolver::launcher_refusal(&m, launcher) {
            return Err(Error::Other(why));
        }
        crate::busy::refuse_change(&self.root, &id, &m.name)?;
        let dest = self.addons_dir().join(&id);
        let fresh = !dest.exists();
        self.helpers.forget(&id);
        let staging = self.addons_dir().join(format!("{STAGING_PREFIX}{id}"));
        if staging.exists() {
            std::fs::remove_dir_all(&staging).map_err(io_err(&staging))?;
        }
        let moved = self.in_scratch(src) && std::fs::rename(src, &staging).is_ok();
        if !moved && let Err(e) = copy_dir(src, &staging) {
            std::fs::remove_dir_all(&staging).ok();
            return Err(e);
        }
        if id != m.id {
            match crate::imported::rename(&staging, &id) {
                Ok(renamed) => m = renamed,
                Err(e) => {
                    std::fs::remove_dir_all(&staging).ok();
                    return Err(e);
                }
            }
        }
        swap_in(
            &staging,
            &dest,
            &m.name,
            &mut |from, to| std::fs::rename(from, to),
            &mut |d| std::fs::remove_dir_all(d),
        )?;
        std::fs::remove_file(self.registry_install_path(&m.id)).ok();
        if fresh {
            self.add_to_install_profile(&m.id, profile);
        }
        Ok(m)
    }

    /// A new addon goes into the profile it was installed from, off, rather than
    /// every profile; the others add it from their own Addons page. A failure
    /// only leaves it out of the profile, so it doesn't fail the install.
    /// Every installed addon is in every profile, off until a profile turns it on, so an
    /// install lists it in all of them at the end of their order. `target` only names a
    /// profile that must still exist for a queued install to count.
    fn add_to_install_profile(&self, id: &str, target: Option<&str>) {
        let names = match self.profile_names() {
            Ok(names) => names,
            Err(e) => {
                warn(format!("couldn't list the profiles for {id}: {e}"));
                return;
            }
        };
        if let Some(target) = target
            && target != profile::VANILLA
            && !names.iter().any(|n| n == target)
        {
            warn(format!(
                "profile {target} no longer exists; {id} is listed in the others"
            ));
        }
        for name in names.iter().filter(|n| n.as_str() != profile::VANILLA) {
            if let Err(e) = self.update_profile_if(name, |p| {
                let had = p.has(id);
                p.add_addon(id)?;
                Ok(if had { Vec::new() } else { vec![id.to_owned()] })
            }) {
                warn(format!("couldn't add {id} to the {name} profile: {e}"));
            }
        }
    }

    fn in_scratch(&self, path: &Path) -> bool {
        path.strip_prefix(self.addons_dir())
            .ok()
            .and_then(|rest| rest.components().next())
            .is_some_and(|c| c.as_os_str().to_string_lossy().starts_with(".importing-"))
    }

    fn scratch_dir(&self) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.addons_dir().join(format!(".importing-{n}"))
    }

    // A package is one addon; any other archive holds whatever addons it has.
    fn unpacked_addons(&self, archive: &Path, scratch: &Path) -> Result<Vec<PathBuf>> {
        if scratch.exists() {
            std::fs::remove_dir_all(scratch).map_err(io_err(scratch))?;
        }
        crate::import::unpack(archive, scratch)?;
        let found = if crate::import::is_package(archive) {
            crate::import::addon_root(scratch).into_iter().collect()
        } else {
            crate::import::found_addons(scratch)
        };
        if found.is_empty() {
            return Err(Error::Other(format!(
                "{} has no {MANIFEST_FILE}",
                archive.display()
            )));
        }
        Ok(found)
    }

    pub fn remove_addon(&self, id: &str) -> Result<()> {
        self.remove_addon_with(id, |d| std::fs::remove_dir_all(d))
    }

    pub(crate) fn remove_addon_with(
        &self,
        id: &str,
        mut delete: impl FnMut(&Path) -> std::io::Result<()>,
    ) -> Result<()> {
        if self.is_builtin(id) {
            return Err(Error::Other(format!("{id} is built in and cannot be removed")));
        }
        if crate::managed::is_managed(id) {
            return Err(Error::Other(format!(
                "{id} is managed by the launcher and cannot be removed"
            )));
        }
        let dir = self.addons_dir().join(id);
        if !dir.exists() {
            return Err(Error::Other(format!("no addon named {id} is installed")));
        }
        let name = AddonManifest::load(&dir).map_or_else(|_| id.to_owned(), |m| m.name);
        crate::busy::refuse_change(&self.root, id, &name)?;
        let _write = profile_write();
        self.helpers.forget(id);
        delete_addon_dir(
            &dir,
            &name,
            &mut |from, to| std::fs::rename(from, to),
            &mut delete,
        )?;
        std::fs::remove_file(self.registry_install_path(id)).ok();
        for name in self.profile_names()? {
            let path = self.profiles_dir().join(format!("{name}.json"));
            if name == profile::VANILLA || !path.is_file() {
                continue;
            }
            let Some(mut p) = load_profile(&path)? else {
                continue;
            };
            let listed = p.addons.iter().any(|a| a.id == id);
            let had_options = p.options.contains_key(id) || p.applied.contains_key(id);
            let shader = p.shader.as_deref().is_some_and(|r| {
                r.strip_prefix("addon:")
                    .and_then(|r| r.split_once('/'))
                    .is_some_and(|(a, _)| a == id)
            });
            if listed || had_options || shader {
                if shader {
                    p.shader = None;
                }
                p.addons.retain(|a| a.id != id);
                p.options.remove(id);
                p.applied.remove(id);
                p.normalise();
                self.save_profile(&p)?;
            }
        }
        Ok(())
    }

    pub fn profile(&self, name: &str) -> Result<Profile> {
        if name == crate::profile::VANILLA {
            return Ok(Profile::vanilla());
        }
        let path = self.profiles_dir().join(format!("{name}.json"));
        let mut p = load_profile(&path)?.unwrap_or_else(|| Profile::new(name));
        let unknown = p.unknown_fields();
        if !unknown.is_empty() {
            warn_once(format!(
                "profile {name} has settings from a newer Swofty Launcher ({}); they are kept but not used",
                unknown.join(", ")
            ));
        }
        p.normalise();
        self.adopt_builtins(&mut p);
        Ok(p)
    }

    pub fn profile_names(&self) -> Result<Vec<String>> {
        let mut names = vec![profile::VANILLA.to_owned()];
        let rd = match std::fs::read_dir(self.profiles_dir()) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                names.push(DEFAULT_PROFILE.into());
                return Ok(names);
            }
            Err(e) => return Err(io_err(self.profiles_dir())(e)),
        };
        let mut found: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_str()?.strip_suffix(".json")?.to_owned();
                profile::valid_name(&name).then_some(name)
            })
            .collect();
        if found.is_empty() {
            found.push(DEFAULT_PROFILE.into());
        }
        found.sort();
        names.extend(found);
        Ok(names)
    }

    /// Refuses a name that isn't valid, or that another profile holds. Windows ignores
    /// capitals in file names, so `Grid` and `grid` would be one profile's files.
    fn check_new_profile_name(&self, name: &str) -> Result<()> {
        check_new_name(name)?;
        if let Some(taken) = self
            .profile_names()?
            .iter()
            .find(|n| n.eq_ignore_ascii_case(name))
        {
            return Err(Error::Other(format!("a profile named {taken} already exists")));
        }
        Ok(())
    }

    pub fn create_profile(&self, name: &str, from: Option<&str>) -> Result<Profile> {
        self.check_new_profile_name(name)?;
        let mut p = match from {
            Some(src) => self.profile(src)?,
            None => Profile::new(name),
        };
        p.name = name.into();
        p.normalise();
        self.adopt_builtins(&mut p);
        if from.is_none() {
            for m in self.addons()?.into_values().filter(|m| {
                !m.is_builtin() && !self.is_builtin(&m.id) && m.id != crate::managed::ID && m.id != CORE_ID
            }) {
                p.add_addon(&m.id)?;
            }
        }
        self.save_profile(&p)?;
        crate::addon_storage::profile_created(&self.root, name, from)?;
        Ok(p)
    }

    pub fn rename_profile(&self, from: &str, to: &str) -> Result<Profile> {
        if from == profile::VANILLA {
            return Err(Error::Other("the vanilla profile cannot be renamed".into()));
        }
        // The running game keeps writing to the saves folder this would move.
        if let Some(j) = crate::journal::Journal::open(&self.root)?
            .into_iter()
            .find(|j| j.profile == from)
        {
            return Err(Error::Other(format!(
                "{from} is installed in the game folder (journal {}); close F1 25 or run \
                 swofty restore, then rename it",
                j.id
            )));
        }
        if from != to && from.eq_ignore_ascii_case(to) {
            return self.rename_profile_capitals(from, to);
        }
        self.check_new_profile_name(to)?;
        let saves = self.saves_dir(from);
        let saves_to = self.saves_dir(to);
        let has_saves = saves.is_dir();
        if has_saves {
            if std::fs::read_dir(&saves_to).is_ok_and(|mut rd| rd.next().is_some()) {
                return Err(Error::Other(format!(
                    "{} already holds saves; move them away, then rename {from}",
                    saves_to.display()
                )));
            }
            std::fs::remove_dir(&saves_to).ok();
        }
        let p = self.create_profile(to, Some(from))?;
        // The saves move first: they fail most often, and until the old profile is
        // gone everything can still go back to how it was.
        let undo = || {
            std::fs::remove_file(self.profiles_dir().join(format!("{to}.json"))).ok();
            crate::addon_storage::profile_deleted(&self.root, to).ok();
        };
        if has_saves && let Err(e) = with_retry(&saves, || std::fs::rename(&saves, &saves_to)) {
            undo();
            return Err(e);
        }
        let old = self.profiles_dir().join(format!("{from}.json"));
        if old.is_file()
            && let Err(e) = with_retry(&old, || std::fs::remove_file(&old))
        {
            if has_saves {
                std::fs::rename(&saves_to, &saves).ok();
            }
            undo();
            return Err(e);
        }
        crate::launcher_saves::rename_vaults(&self.root, from, to)?;
        crate::launcher_saves::rename_in_marker(&self.root, from, to)?;
        crate::stdlib::careers::rename_profile(&self.root, from, to)?;
        crate::addon_storage::profile_deleted(&self.root, from)?;
        if self.config().active_profile.as_deref().unwrap_or(DEFAULT_PROFILE) == from {
            self.set_active_profile(to)?;
        }
        let mut config = self.config();
        if config.vr_profiles.remove(from) {
            config.vr_profiles.insert(to.to_owned());
            self.save_config(&config)?;
        }
        Ok(p)
    }

    /// `from` and `to` differ only in capitals. On Windows they name the same files, so going
    /// straight across would write over the profile and then delete it: it goes by way of
    /// another name.
    fn rename_profile_capitals(&self, from: &str, to: &str) -> Result<Profile> {
        check_new_name(to)?;
        if self.profile_names()?.iter().any(|n| n == to) {
            return Err(Error::Other(format!("a profile named {to} already exists")));
        }
        let between = format!("renaming-{}", std::process::id());
        self.rename_profile(from, &between)?;
        self.rename_profile(&between, to).inspect_err(|_| {
            self.rename_profile(&between, from).ok();
        })
    }

    pub fn plan_profile_deletion(&self, name: &str) -> Result<ProfileDeletion> {
        if name == profile::VANILLA {
            return Err(Error::Other(
                "the vanilla profile is built in and cannot be deleted".into(),
            ));
        }
        let names = self.profile_names()?;
        if !names.iter().any(|n| n == name) {
            return Err(Error::Other(format!("no profile named {name}")));
        }
        let others: Vec<&String> = names
            .iter()
            .filter(|n| *n != profile::VANILLA && *n != name)
            .collect();
        let Some(first) = others.first() else {
            return Err(Error::Other(format!(
                "{name} is the only profile; create another profile before deleting it"
            )));
        };
        if let Some(j) = crate::journal::Journal::open(&self.root)?
            .into_iter()
            .find(|j| j.profile == name)
        {
            return Err(Error::Other(format!(
                "{name} is installed in the game folder (journal {}); close F1 25 or run \
                 swofty restore, then delete it",
                j.id
            )));
        }
        let switch_to = (self.active_profile() == name).then(|| {
            others
                .iter()
                .find(|n| n.as_str() == DEFAULT_PROFILE)
                .unwrap_or(first)
                .to_string()
        });
        let saves = self.saves_dir(name);
        let has_saves = std::fs::read_dir(&saves).is_ok_and(|mut rd| rd.next().is_some());
        let paths = [
            self.overlays_dir(name),
            self.requests_path(name),
            saves,
            self.profiles_dir().join(format!("{name}.json")),
        ]
        .into_iter()
        .filter(|p| p.symlink_metadata().is_ok())
        .collect();
        Ok(ProfileDeletion {
            name: name.into(),
            switch_to,
            saves: has_saves,
            paths,
        })
    }

    pub fn delete_profile(&self, name: &str) -> Result<ProfileDeletion> {
        let _write = profile_write();
        let plan = self.plan_profile_deletion(name)?;
        if let Some(to) = &plan.switch_to {
            self.set_active_profile(to)?;
        }
        self.forget_launch_profile(name)?;
        let mut config = self.config();
        if config.vr_profiles.remove(name) {
            self.save_config(&config)?;
        }
        for path in &plan.paths {
            let removed = if path.is_dir() && !path.is_symlink() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
            if let Err(e) = removed
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(io_err(path)(e));
            }
        }
        crate::addon_storage::profile_deleted(&self.root, name)?;
        Ok(plan)
    }

    fn forget_launch_profile(&self, name: &str) -> Result<()> {
        let path = self.launch_request_path();
        let Ok(mut request) = crate::read_json::<serde_json::Value>(&path) else {
            return Ok(());
        };
        if request.get("profile").and_then(|p| p.as_str()) != Some(name) {
            return Ok(());
        }
        if let Some(fields) = request.as_object_mut() {
            fields.remove("profile");
        }
        crate::write_json(&path, &request)
    }

    pub fn config(&self) -> Config {
        let mut config: Config = load_settings(&self.config_path());
        config.migrate_games();
        config
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        let path = self.config_path();
        set_aside_unreadable::<Config>(&path)?;
        crate::write_json(&path, config)
    }

    pub fn folders_path(&self) -> PathBuf {
        self.root.join(crate::folders::FILE)
    }

    pub fn folders(&self) -> crate::folders::Folders {
        load_settings(&self.folders_path())
    }

    pub fn save_folders(&self, folders: &crate::folders::Folders) -> Result<()> {
        let path = self.folders_path();
        set_aside_unreadable::<crate::folders::Folders>(&path)?;
        crate::write_json(&path, folders)
    }

    pub fn active_profile(&self) -> String {
        let names = self.profile_names().unwrap_or_default();
        self.config()
            .active_profile
            .filter(|p| names.contains(p))
            .or_else(|| names.get(1).cloned())
            .unwrap_or_else(|| DEFAULT_PROFILE.into())
    }

    pub fn set_active_profile(&self, name: &str) -> Result<()> {
        let mut config = self.config();
        config.active_profile = Some(name.into());
        self.save_config(&config)
    }

    pub fn latest_session(&self) -> Option<PathBuf> {
        let dir = std::fs::read_to_string(self.sessions_dir().join("latest.txt")).ok()?;
        Some(PathBuf::from(dir.trim()))
    }

    pub fn requests_path(&self, profile: &str) -> PathBuf {
        self.requests_dir().join(format!("{profile}.json"))
    }

    pub fn session_settings(&self, profile: &str) -> Result<SessionSettings> {
        self.session_settings_of(&self.profile(profile)?)
    }

    /// The settings of a session of `p`, a profile as the caller has it, such as one a room lends the host's
    /// load order and options.
    pub fn session_settings_of(&self, p: &Profile) -> Result<SessionSettings> {
        let requests = crate::paths::game_folder_view(&self.requests_path(&p.name));
        let installed = self.addons()?;
        let mut s = SessionSettings::build(p, &installed, Some(requests))?;
        // Each addon's folder and track flag, as the launcher's Addons page has them.
        let folders = self.folders();
        for a in &mut s.addons {
            let Some(m) = installed.get(&a.id) else { continue };
            let builtin = self.is_builtin(&a.id) || a.id == crate::managed::ID;
            a.track |=
                !self.is_builtin(&a.id) && crate::tracks::is_track_addon(&self.addons_dir().join(&a.id), m);
            let category = m
                .category
                .or(a.track.then_some(swofty_addon::manifest::Category::Tracks));
            a.folder = folders.place(&a.id, builtin, category).map(|k| folders.name(&k));
        }
        let redirect = p.launcher_saves && !crate::launcher_saves::steam_save_pending(&self.root, &p.name);
        s.launcher_saves = redirect.then(|| settings::SessionSaves {
            profile: p.name.clone(),
            remote: crate::paths::game_folder_view(&crate::launcher_saves::remote_dir(&self.root, &p.name)),
        });
        s.addons
            .retain(|a| a.id == CORE_ID || !self.is_builtin(&a.id) || !a.options.is_empty());
        Ok(s)
    }

    pub fn merge_requests(&self, profile: &str, consume: bool) -> Result<Option<Merge>> {
        let path = self.requests_path(profile);
        if !path.is_file() {
            return Ok(None);
        }
        let req: Requests = crate::read_json(&path)?;
        let _write = profile_write();
        let merged = if req.profile.is_empty() || req.profile == profile {
            let mut p = self.profile(profile)?;
            let out = settings::merge(&mut p, &self.addons()?, &req);
            if !out.changed.is_empty() {
                self.save_profile(&p)?;
            }
            Some(out)
        } else {
            None
        };
        if consume {
            std::fs::remove_file(&path).map_err(io_err(&path))?;
        }
        Ok(merged)
    }

    pub fn update_profile<T>(&self, name: &str, f: impl FnOnce(&mut Profile) -> Result<T>) -> Result<T> {
        let _write = profile_write();
        let mut p = self.profile(name)?;
        let out = f(&mut p)?;
        self.save_profile(&p)?;
        Ok(out)
    }

    /// Writes a load-order fix: the profile's new order and the folders its
    /// ejected addons leave, together. A folders failure puts the profile back.
    pub fn apply_fix(
        &self,
        name: &str,
        plan: &crate::order::FixPlan,
        default_place: &dyn Fn(&str) -> Option<crate::folders::Key>,
    ) -> Result<()> {
        let _write = profile_write();
        let saved = self.profile(name)?;
        let mut fixed = saved.clone();
        crate::folders::reorder(&mut fixed, &plan.order, &|_| false);
        self.save_profile(&fixed)?;
        if plan.ejected.is_empty() {
            return Ok(());
        }
        let mut folders = self.folders();
        for (id, _) in &plan.ejected {
            folders.set_place(id, None, default_place(id).as_ref());
        }
        if let Err(e) = self.save_folders(&folders) {
            self.save_profile(&saved)?;
            return Err(e);
        }
        Ok(())
    }

    pub fn save_profile(&self, p: &Profile) -> Result<()> {
        if p.is_vanilla() {
            return Err(Error::Other(
                "the vanilla profile is built in and cannot be changed".into(),
            ));
        }
        crate::write_json(&self.profiles_dir().join(format!("{}.json", p.name)), p)
    }
}

/// A saved profile. One that is damaged (not JSON at all) was set aside by
/// [`crate::load_json`] and starts over empty, as a missing one does, so it
/// cannot block a launch; the set-aside copy is kept for the player.
/// An installed addon whose `swofty.json` this launcher can't read.
#[derive(Debug, Clone, PartialEq)]
pub struct Unreadable {
    /// The manifest's id when it has one, else the folder name.
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub dir: PathBuf,
    /// Why, after "it": "needs a newer Swofty Launcher (unknown field `widgets`)".
    pub reason: String,
    /// Made for a newer launcher rather than damaged.
    pub newer: bool,
}

impl Unreadable {
    pub fn of(dir: &Path, err: &swofty_addon::Error, launcher: &semver::Version) -> Self {
        let doc: Option<serde_json::Value> = std::fs::read(dir.join(MANIFEST_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let text = |k: &str| {
            doc.as_ref()
                .and_then(|d| d.get(k))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|s| !s.trim().is_empty())
        };
        let folder = dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let id = text("id")
            .filter(|i| crate::manifest::valid_id(i))
            .unwrap_or(folder);
        let name = text("name").unwrap_or_else(|| id.clone());
        let req = doc
            .as_ref()
            .and_then(|d| d.pointer("/compat/launcher"))
            .and_then(|v| v.as_str())
            .and_then(|r| semver::VersionReq::parse(r).ok())
            .filter(|r| !r.matches(launcher));
        let schema = doc
            .as_ref()
            .and_then(|d| d.get("schema_version"))
            .and_then(|v| v.as_u64())
            .filter(|v| *v > u64::from(crate::manifest::SCHEMA_VERSION));
        let message = match err {
            swofty_addon::Error::Json { source, .. } => source.to_string(),
            other => other.to_string(),
        };
        let unknown = unknown_part(&message);
        let reason = match (&req, &unknown) {
            (Some(req), Some(u)) => format!("needs Swofty Launcher {req}; this is {launcher} ({u})"),
            (Some(req), None) => format!("needs Swofty Launcher {req}; this is {launcher}"),
            (None, Some(u)) => format!("needs a newer Swofty Launcher ({u})"),
            (None, None) => match schema {
                Some(v) => format!("needs a newer Swofty Launcher (schema_version {v})"),
                None => format!("can't be read: {message}"),
            },
        };
        Self {
            newer: req.is_some() || unknown.is_some() || schema.is_some(),
            version: text("version"),
            dir: dir.to_path_buf(),
            id,
            name,
            reason,
        }
    }

    /// "Race HUD (w3.hud) needs a newer Swofty Launcher (unknown field `widgets`)".
    pub fn line(&self) -> String {
        if self.name == self.id {
            format!("{} {}", self.id, self.reason)
        } else {
            format!("{} ({}) {}", self.name, self.id, self.reason)
        }
    }
}

/// "unknown field `hud`" or "unknown value `postfx`" from a serde message.
fn unknown_part(message: &str) -> Option<String> {
    for (marker, word) in [("unknown field `", "field"), ("unknown variant `", "value")] {
        if let Some(at) = message.find(marker) {
            let rest = &message[at + marker.len()..];
            let name = &rest[..rest.find('`')?];
            return Some(format!("unknown {word} `{name}`"));
        }
    }
    None
}

/// Prints a warning to stderr once per process.
static WARNING_SINK: std::sync::OnceLock<fn(&str)> = std::sync::OnceLock::new();

/// Sends the store's warnings to `sink` as well as stderr; the launcher uses its app log.
pub fn set_warning_sink(sink: fn(&str)) {
    WARNING_SINK.set(sink).ok();
}

fn warn(text: String) {
    eprintln!("swofty: warning: {text}");
    if let Some(sink) = WARNING_SINK.get() {
        sink(&text);
    }
}

fn warn_once(text: String) {
    static SEEN: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if seen.insert(text.clone()) {
        eprintln!("swofty: warning: {text}");
    }
}

/// A settings file, or the defaults when there is none or this build can't read it. One that is
/// JSON in a shape this build can't read (a hand edit with a wrong type, a value a newer launcher
/// wrote) stays where it is, so fixing it or going back to that launcher brings it back, until a
/// save moves it aside with [`set_aside_unreadable`].
fn load_settings<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    match crate::load_json(path) {
        Ok(crate::Loaded::Found(v)) => v,
        Ok(crate::Loaded::Missing | crate::Loaded::Damaged(_)) => T::default(),
        Err(e) => {
            warn_once(format!("{e}; using the defaults until it can be read"));
            T::default()
        }
    }
}

/// Before a save replaces the settings file at `path`, move one this build can't read aside to
/// `<name>.unreadable-<unix time>`, so the defaults [`load_settings`] fell back to don't overwrite
/// the only copy of the player's settings.
fn set_aside_unreadable<T: serde::de::DeserializeOwned>(path: &Path) -> Result<()> {
    if let Err(e @ Error::Json { .. }) = crate::load_json::<T>(path) {
        let to = crate::set_aside(path, crate::UNREADABLE_SUFFIX)?;
        warn(format!(
            "{e}; moved it to {} and saved new settings in its place",
            to.display()
        ));
    }
    Ok(())
}

fn check_new_name(name: &str) -> Result<()> {
    if profile::reads_as_flag(name) {
        return Err(Error::Other(format!(
            "{name:?} starts with -, which commands read as an option; start the profile name with a letter, digit or _"
        )));
    }
    if !profile::valid_name(name) {
        return Err(Error::Other(format!(
            "{name:?} is not a valid profile name (letters, digits, - and _)"
        )));
    }
    Ok(())
}

fn load_profile(path: &Path) -> Result<Option<Profile>> {
    Ok(match crate::load_json(path)? {
        crate::Loaded::Found(p) => Some(p),
        crate::Loaded::Missing | crate::Loaded::Damaged(_) => None,
    })
}

fn manifest_id(dir: &Path) -> Option<String> {
    let doc: serde_json::Value = crate::read_json(&dir.join(MANIFEST_FILE)).ok()?;
    Some(doc.get("id")?.as_str()?.to_owned())
}

fn subdirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(dir)(e)),
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    out.sort();
    Ok(out)
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(io_err(dest))?;
    for entry in std::fs::read_dir(src).map_err(io_err(src))? {
        let entry = entry.map_err(io_err(src))?;
        let to = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to).map_err(io_err(&to))?;
        }
    }
    Ok(())
}

pub fn overlay_root(root: &std::path::Path) -> PathBuf {
    root.join("cache").join("overlay")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-store-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn apply_fix_writes_the_order_and_the_folders_together() {
        use crate::folders::Key;
        let root = tmp("apply-fix");
        let store = Store::new(&root);
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        for id in ["t.a", "t.c", "t.b"] {
            p.set_enabled(id, true).unwrap();
        }
        store.save_profile(&p).unwrap();
        let plan = crate::order::FixPlan {
            order: [CORE_ID, "t.a", "t.b", "t.c"].map(str::to_owned).to_vec(),
            ejected: vec![("t.c".to_owned(), Key::Own("f".into()))],
            moved: 2,
        };
        let mut f = crate::folders::Folders::default();
        f.placed.insert("t.c".into(), Key::Own("f".into()).text());
        store.save_folders(&f).unwrap();
        let default = Key::Category(swofty_addon::manifest::Category::Seasons);
        store
            .apply_fix(DEFAULT_PROFILE, &plan, &|_| Some(default.clone()))
            .unwrap();
        let ids: Vec<String> = store
            .profile(DEFAULT_PROFILE)
            .unwrap()
            .addons
            .into_iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, plan.order);
        assert_eq!(store.folders().place("t.c", false, None), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn installs_template_addon_and_keeps_core() {
        let root = tmp("addons");
        let store = Store::new(&root);
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/addon");
        let m = store.install_addon(&template).unwrap();
        assert!(store.addons_dir().join(&m.id).join(MANIFEST_FILE).is_file());
        let addons = store.addons().unwrap();
        assert!(addons.contains_key(CORE_ID));
        assert!(addons.contains_key(&m.id));
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled(&m.id, true).unwrap();
        store.save_profile(&p).unwrap();
        let plan = crate::resolver::resolve(&store.profile(DEFAULT_PROFILE).unwrap(), &addons).unwrap();
        assert_eq!(plan.order, [CORE_ID, m.id.as_str()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_addon_in_a_running_build_is_not_replaced_or_removed() {
        let root = tmp("busy-addon");
        let store = Store::new(&root);
        let src = root.join("src");
        for id in ["t.used", "t.other"] {
            put_addon(&store, id, &manifest_json(id));
            std::fs::create_dir_all(src.join(id)).unwrap();
            std::fs::write(src.join(id).join(MANIFEST_FILE), manifest_json(id)).unwrap();
        }
        let busy = crate::busy::Busy {
            pid: std::process::id().wrapping_add(1),
            ..crate::busy::Busy::new("default", vec!["t.used".into()])
        };
        busy.write(&root).unwrap();
        let lock = crate::recovery::SessionLock::try_acquire(&root).unwrap().unwrap();
        let err = store.install_addon(&src.join("t.used")).unwrap_err().to_string();
        assert!(err.contains("is in the build that is running"), "{err}");
        let err = store.remove_addon("t.used").unwrap_err().to_string();
        assert!(err.contains("is in the build that is running"), "{err}");
        assert!(store.addons_dir().join("t.used").join(MANIFEST_FILE).is_file());
        store.install_addon(&src.join("t.other")).unwrap();
        drop(lock);
        store.install_addon(&src.join("t.used")).unwrap();
        store.remove_addon("t.used").unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    fn put_addon(store: &Store, folder: &str, json: &str) -> PathBuf {
        let dir = store.addons_dir().join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(MANIFEST_FILE), json).unwrap();
        dir
    }

    #[test]
    fn an_addon_for_a_newer_launcher_is_skipped_and_named() {
        let root = tmp("future-addon");
        let store = Store::new(&root);
        put_addon(&store, "t.good", &manifest_json("t.good"));
        put_addon(
            &store,
            "w3.hud",
            r#"{"schema_version":1,"id":"w3.hud","name":"Race HUD","version":"0.1.0","author":"t",
                "compat":{"game":"f1_25"},"hud":{"widgets":[]}}"#,
        );
        put_addon(
            &store,
            "t.look",
            r#"{"schema_version":1,"id":"t.look","name":"Look","version":"1.0.0","author":"t",
                "compat":{"game":"f1_25","launcher":">=9.0.0"},"experimental_features":["holograms"]}"#,
        );
        put_addon(&store, "t.broken", "{\"id\": 5}");
        let addons = store.addons().unwrap();
        assert!(addons.contains_key("t.good") && addons.contains_key(CORE_ID));
        assert!(!addons.contains_key("w3.hud") && !addons.contains_key("t.look"));
        let bad = store.unreadable_addons();
        let lines: Vec<String> = bad.iter().map(Unreadable::line).collect();
        let launcher = crate::update::current_version();
        assert_eq!(
            lines,
            [
                "t.broken can't be read: invalid type: integer `5`, expected a string at line 1 column 8"
                    .to_owned(),
                format!(
                    "Look (t.look) needs Swofty Launcher >=9.0.0; this is {launcher} (unknown value `holograms`)"
                ),
                "Race HUD (w3.hud) needs a newer Swofty Launcher (unknown field `widgets`)".to_owned(),
            ]
        );
        assert_eq!(
            bad.iter().map(|u| u.newer).collect::<Vec<_>>(),
            [false, true, true]
        );
        assert_eq!(bad[2].version.as_deref(), Some("0.1.0"));

        store
            .update_profile(DEFAULT_PROFILE, |p| {
                p.set_enabled("t.good", true)?;
                p.set_enabled("w3.hud", true)
            })
            .unwrap();
        let registry = store.registry().unwrap();
        assert!(registry.get("t.good").is_some());
        let err = store
            .install_addon(&store.addons_dir().join("w3.hud"))
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Race HUD (w3.hud) needs a newer Swofty Launcher (unknown field `widgets`)"
        );
        store.remove_addon("w3.hud").unwrap();
        assert!(store.unreadable_addons().iter().all(|u| u.id != "w3.hud"));
        assert!(
            !store
                .profile(DEFAULT_PROFILE)
                .unwrap()
                .addons
                .iter()
                .any(|a| a.id == "w3.hud")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_profile_with_fields_from_a_newer_launcher_loads_and_keeps_them() {
        let root = tmp("future-profile");
        let store = Store::new(&root);
        put_addon(&store, "t.good", &manifest_json("t.good"));
        std::fs::create_dir_all(store.profiles_dir()).unwrap();
        std::fs::write(
            store.profiles_dir().join("default.json"),
            r#"{"schema_version":2,"name":"default","addons":[{"id":"t.good","enabled":true,"pinned":true}],
                "options":{},"overlay_off":["t.good"]}"#,
        )
        .unwrap();
        let p = store.profile(DEFAULT_PROFILE).unwrap();
        assert!(p.enabled().any(|id| id == "t.good"));
        assert_eq!(p.unknown_fields(), ["overlay_off", "addons[t.good].pinned"]);
        store
            .update_profile(DEFAULT_PROFILE, |p| p.set_enabled("t.good", false))
            .unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(store.profiles_dir().join("default.json")).unwrap())
                .unwrap();
        assert_eq!(saved["overlay_off"], serde_json::json!(["t.good"]));
        let good = saved["addons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "t.good")
            .unwrap();
        assert_eq!(good["pinned"], serde_json::json!(true));
        assert_eq!(good["enabled"], serde_json::json!(false));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn installs_addon_zips() {
        let root = tmp("addon-zips");
        let store = Store::new(root.join("data"));
        let manifest = r#"{"schema_version":1,"id":"t.zipped","name":"Zipped","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#;
        for (file, prefix) in [("flat.swoftymod", ""), ("wrapped.zip", "zipped/")] {
            let path = root.join(file);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            z.start_file(format!("{prefix}{MANIFEST_FILE}"), opts).unwrap();
            std::io::Write::write_all(&mut z, manifest.as_bytes()).unwrap();
            z.start_file(format!("{prefix}plugins/a.dll"), opts).unwrap();
            z.finish().unwrap();
            assert_eq!(store.install_addon(&path).unwrap().id, "t.zipped");
            assert!(store.addons_dir().join("t.zipped/plugins/a.dll").is_file());
            assert_eq!(scratch_dirs(&store), 0);
        }
        let bare = root.join("bare.zip");
        zip::ZipWriter::new(std::fs::File::create(&bare).unwrap())
            .finish()
            .unwrap();
        assert!(store.install_addon(&bare).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_install_keeps_the_previous_version() {
        let root = tmp("failed-install");
        let store = Store::new(&root);
        let good = root.join("good");
        std::fs::create_dir_all(good.join("track")).unwrap();
        std::fs::write(good.join(MANIFEST_FILE), manifest_json("a.sochi")).unwrap();
        std::fs::write(good.join("track/sochi.erp"), b"v1").unwrap();
        store.install_addon(&good).unwrap();
        let broken = root.join("broken");
        std::fs::create_dir_all(broken.join("track")).unwrap();
        std::fs::write(
            broken.join(MANIFEST_FILE),
            manifest_json("a.sochi").replace("1.0.0", "1.1.0"),
        )
        .unwrap();
        std::os::unix::fs::symlink(root.join("nowhere"), broken.join("track/sochi.erp")).unwrap();
        assert!(store.install_addon(&broken).is_err());
        let dest = store.addons_dir().join("a.sochi");
        assert_eq!(std::fs::read(dest.join("track/sochi.erp")).unwrap(), b"v1");
        assert_eq!(store.addons().unwrap()["a.sochi"].version.to_string(), "1.0.0");
        let left = std::fs::read_dir(store.addons_dir())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".installing"))
            .count();
        assert_eq!(left, 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_first_install_leaves_nothing_listed() {
        let root = tmp("failed-first-install");
        let store = Store::new(&root);
        let broken = root.join("broken");
        std::fs::create_dir_all(broken.join("track")).unwrap();
        std::fs::write(broken.join(MANIFEST_FILE), manifest_json("a.sochi")).unwrap();
        std::os::unix::fs::symlink(root.join("nowhere"), broken.join("track/sochi.erp")).unwrap();
        assert!(store.install_addon(&broken).is_err());
        assert!(!store.addons().unwrap().contains_key("a.sochi"));
        assert!(!store.addons_dir().join("a.sochi").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    fn scratch_dirs(store: &Store) -> usize {
        std::fs::read_dir(store.addons_dir())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".importing"))
            .count()
    }

    fn manifest_json(id: &str) -> String {
        format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
        )
    }

    fn package(id: &str) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        z.start_file(MANIFEST_FILE, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut z, manifest_json(id).as_bytes()).unwrap();
        z.finish().unwrap().into_inner()
    }

    #[test]
    fn installs_every_addon_a_rar_holds() {
        let root = tmp("addon-rars");
        let store = Store::new(root.join("data"));
        for (name, version) in [
            ("rar5.rar", rars::ArchiveVersion::Rar50),
            ("rar29.rar", rars::ArchiveVersion::Rar29),
        ] {
            let mut b = rars::Builder::new(version);
            let mut add = |path: &str, data: Vec<u8>| {
                b.add_bytes(path.as_bytes().to_vec(), data, None, None).unwrap();
            };
            add("Pack/t.packed.swoftyaddon", package("t.packed"));
            add(
                &format!("Pack/folder/{MANIFEST_FILE}"),
                manifest_json("t.folder").into_bytes(),
            );
            add("Pack/folder/plugins/a.dll", b"dll".to_vec());
            add("Pack/readme.txt", b"hi".to_vec());
            let rar = root.join(name);
            std::fs::write(&rar, b.to_bytes().unwrap()).unwrap();
            assert_eq!(
                crate::import::addon_claim(&rar),
                swofty_addon::launcher::Claim::Yes
            );
            let ids: Vec<String> = store
                .install_addons(&rar)
                .into_iter()
                .map(|m| m.unwrap().id)
                .collect();
            assert_eq!(ids, ["t.folder", "t.packed"], "{name}");
            assert!(store.addons_dir().join("t.folder/plugins/a.dll").is_file());
            assert!(
                store
                    .install_addon(&rar)
                    .unwrap_err()
                    .to_string()
                    .contains("holds 2 addons")
            );
            assert_eq!(scratch_dirs(&store), 0);
        }
        let mut b = rars::Builder::new(rars::ArchiveVersion::Rar50);
        b.add_bytes(b"One/t.one.swoftyaddon".to_vec(), package("t.one"), None, None)
            .unwrap();
        let one = root.join("one.rar");
        std::fs::write(&one, b.to_bytes().unwrap()).unwrap();
        assert_eq!(store.install_addon(&one).unwrap().id, "t.one");
        let mut b = rars::Builder::new(rars::ArchiveVersion::Rar50);
        b.add_bytes(
            b"mod/2025_asset_groups/a.erp".to_vec(),
            b"erp".to_vec(),
            None,
            None,
        )
        .unwrap();
        let game_mod = root.join("mod.rar");
        std::fs::write(&game_mod, b.to_bytes().unwrap()).unwrap();
        assert_eq!(
            crate::import::addon_claim(&game_mod),
            swofty_addon::launcher::Claim::No
        );
        let err = store.install_addons(&game_mod).remove(0).unwrap_err();
        assert!(err.to_string().contains("has no swofty.json"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn only_a_managed_release_installs_under_the_managed_id() {
        let root = tmp("managed-claimed");
        let store = Store::new(root.join("data"));
        let fake = root.join("fake");
        write_addon(&fake, crate::managed::ID);

        for err in [
            store.install_addon(&fake).unwrap_err(),
            store.install_addons(&fake).remove(0).unwrap_err(),
            store.install_addon_in_profile(&fake, "default").unwrap_err(),
        ] {
            assert!(err.to_string().contains("managed by the launcher"), "{err}");
        }
        assert!(!store.addons_dir().join(crate::managed::ID).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_listed_install_refuses_anything_else_before_replacing_an_addon() {
        let root = tmp("listed");
        let store = Store::new(root.join("data"));
        let one = root.join("one");
        write_addon(&one, "t.one");
        store.install_addon(&one).unwrap();
        let v1 = semver::Version::new(1, 0, 0);

        let other = root.join("other");
        write_addon(&other, "t.other");
        let err = store.install_listed(&other, None, "t.one", &v1).unwrap_err();
        assert!(
            err.to_string().contains("the registry lists t.one 1.0.0"),
            "{err}"
        );
        assert!(!store.addons_dir().join("t.other").exists());

        // A listed id that isn't what the download holds can't replace the addon it names, either.
        let err = store
            .install_listed(&one, None, "t.one", &semver::Version::new(2, 0, 0))
            .unwrap_err();
        assert!(
            err.to_string().contains("the download holds t.one 1.0.0"),
            "{err}"
        );

        assert_eq!(
            store.install_listed(&one, None, "t.one", &v1).unwrap().id,
            "t.one"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn manages_the_team_simplified_addon() {
        let root = tmp("managed-addon");
        let store = Store::new(root.join("data"));
        let manifest = r#"{"schema_version":1,"id":"teamsimplified.mods","name":"Simplified for Swofty","version":"0.5.0","author":"Team Simplified","compat":{"game":"f1_25"},"launcher":{"program":{"linux":"bin/helper","windows":"bin/helper.exe"}}}"#;
        let pkg = root.join("teamsimplified.mods.swoftyaddon");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&pkg).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file(MANIFEST_FILE, opts).unwrap();
        std::io::Write::write_all(&mut z, manifest.as_bytes()).unwrap();
        z.start_file("bin/helper", opts).unwrap();
        z.finish().unwrap();

        let m = store
            .install_managed_release(&pkg, &crate::update::current_version())
            .unwrap();
        assert_eq!(m.id, crate::managed::ID);
        assert!(store.addons().unwrap().contains_key(crate::managed::ID));

        let err = store.remove_addon(crate::managed::ID).unwrap_err();
        assert!(err.to_string().contains("managed"), "{err}");
        assert!(
            store
                .addons_dir()
                .join(crate::managed::ID)
                .join(MANIFEST_FILE)
                .is_file()
        );

        let reg = store.registry().unwrap();
        assert!(reg.get(crate::managed::ID).unwrap().locked);

        let p = store.profile(DEFAULT_PROFILE).unwrap();
        assert!(p.enabled().any(|id| id == crate::managed::ID));
        std::fs::remove_dir_all(&root).ok();
    }

    fn write_addon(dir: &Path, id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            format!(
                r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn migrate_renames_the_mods_dir() {
        let root = tmp("migrate-rename");
        let store = Store::new(&root);
        write_addon(&root.join("mods/a.one"), "a.one");
        std::fs::write(store.config_path(), r#"{"active_profile":"grid"}"#).unwrap();
        assert_eq!(store.migrate().unwrap().len(), 2);
        assert!(!root.join("mods").exists());
        assert!(store.addons().unwrap().contains_key("a.one"));
        let config = store.config();
        assert_eq!(config.data_version, DATA_VERSION);
        assert_eq!(config.active_profile.as_deref(), Some("grid"));
        write_addon(&root.join("mods/a.late"), "a.late");
        assert!(store.migrate().unwrap().is_empty());
        assert!(root.join("mods/a.late").is_dir());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn migrate_merges_into_an_existing_addons_dir() {
        let root = tmp("migrate-merge");
        let store = Store::new(&root);
        write_addon(&root.join("addons/a.one"), "a.one");
        write_addon(&root.join("mods/one-copy"), "a.one");
        write_addon(&root.join("mods/a.two"), "a.two");
        std::fs::create_dir_all(root.join("mods/stray")).unwrap();
        std::fs::write(store.config_path(), r#"{"data_version":"from a newer launcher"}"#).unwrap();
        let notes = store.migrate().unwrap();
        assert_eq!(notes.len(), 4, "{notes:?}");
        assert!(notes[3].starts_with("data_version not recorded"));
        assert!(root.join("addons/a.two").join(MANIFEST_FILE).is_file());
        assert!(root.join("mods/one-copy").is_dir() && root.join("mods/stray").is_dir());
        assert_eq!(
            store.addons().unwrap().keys().collect::<Vec<_>>(),
            ["a.one", "a.two", CORE_ID]
        );

        std::fs::remove_dir_all(root.join("mods/one-copy")).unwrap();
        std::fs::remove_dir_all(root.join("mods/stray")).unwrap();
        write_addon(&root.join("mods/a.three"), "a.three");
        std::fs::remove_file(store.config_path()).unwrap();
        assert_eq!(store.migrate().unwrap().len(), 3);
        assert!(!root.join("mods").exists());
        assert!(root.join("addons/a.three").is_dir());
        assert_eq!(store.config().data_version, DATA_VERSION);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_keeps_a_config_or_folders_file_this_build_cant_read() {
        let root = tmp("unreadable-config");
        let store = Store::new(&root);
        let config = r#"{"active_profile":"grid","launch":{"env":{"DXVK_ASYNC":1}}}"#;
        let folders = r#"{"placed":{"t.a":1}}"#;
        std::fs::write(store.config_path(), config).unwrap();
        std::fs::write(store.folders_path(), folders).unwrap();
        assert_eq!(store.config().active_profile, None);
        assert_eq!(store.folders(), crate::folders::Folders::default());
        assert_eq!(std::fs::read_to_string(store.config_path()).unwrap(), config);
        assert_eq!(std::fs::read_to_string(store.folders_path()).unwrap(), folders);

        let aside = |name: &str| -> Vec<PathBuf> {
            let prefix = format!("{name}{}", crate::UNREADABLE_SUFFIX);
            std::fs::read_dir(&root)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&prefix))
                .collect()
        };
        for round in ["other", "third"] {
            store.set_active_profile(round).unwrap();
            store.save_folders(&store.folders()).unwrap();
            assert_eq!(store.config().active_profile.as_deref(), Some(round));
            let kept = aside("config.json");
            assert_eq!(kept.len(), 1, "{round}: {kept:?}");
            assert_eq!(std::fs::read_to_string(&kept[0]).unwrap(), config, "{round}");
            let kept = aside(crate::folders::FILE);
            assert_eq!(kept.len(), 1, "{round}: {kept:?}");
            assert_eq!(std::fs::read_to_string(&kept[0]).unwrap(), folders, "{round}");
        }
        std::fs::remove_dir_all(&root).ok();
    }

    fn v3_store(name: &str, ids: &[&str]) -> (PathBuf, Store) {
        let root = tmp(name);
        let store = Store::new(&root);
        for id in ids {
            write_addon(&root.join("addons").join(id), id);
        }
        std::fs::write(
            store.config_path(),
            r#"{"data_version":3,"active_profile":"grid"}"#,
        )
        .unwrap();
        (root, store)
    }

    fn listed(store: &Store, name: &str) -> Vec<(String, bool)> {
        store
            .profile(name)
            .unwrap()
            .addons
            .iter()
            .filter(|a| a.id != CORE_ID && !store.is_builtin(&a.id))
            .map(|a| (a.id.clone(), a.enabled))
            .collect()
    }

    #[test]
    fn migrate_lists_the_installed_addons_a_profile_lacks() {
        let (root, store) = v3_store("backfill", &["a.e", "a.b", "a.d", "a.a", "a.c"]);
        store
            .update_profile("grid", |p| {
                p.set_enabled("a.d", true)?;
                p.set_enabled("a.b", false)
            })
            .unwrap();
        store.save_profile(&Profile::new("other")).unwrap();
        let builtins_before = store.profile("grid").unwrap().addons.len() - 2;
        store.migrate().unwrap();
        assert_eq!(store.config().data_version, DATA_VERSION);
        let want = [
            ("a.d", true),
            ("a.b", false),
            ("a.a", false),
            ("a.c", false),
            ("a.e", false),
        ];
        let got = listed(&store, "grid");
        assert_eq!(got, want.map(|(i, e)| (i.to_owned(), e)));
        assert_eq!(store.profile("grid").unwrap().addons.len(), builtins_before + 5);
        assert_eq!(listed(&store, "other").len(), 5);
        assert!(store.profile(profile::VANILLA).unwrap().is_vanilla());
        assert!(!store.profiles_dir().join("vanilla.json").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_install_lists_the_addon_off_in_every_profile_and_a_removal_clears_it_everywhere() {
        let root = tmp("everywhere");
        let store = Store::new(root.join("data"));
        store.save_profile(&Profile::new("default")).unwrap();
        store.save_profile(&Profile::new("grid")).unwrap();
        let src = root.join("one");
        write_addon(&src, "t.one");
        store.install_addon(&src).unwrap();
        for name in ["default", "grid"] {
            assert_eq!(
                listed(&store, name).last(),
                Some(&("t.one".to_owned(), false)),
                "{name}"
            );
        }
        store
            .update_profile("default", |p| p.set_enabled("t.one", true))
            .unwrap();
        assert_eq!(
            listed(&store, "default").last(),
            Some(&("t.one".to_owned(), true))
        );
        assert_eq!(listed(&store, "grid").last(), Some(&("t.one".to_owned(), false)));
        let two = root.join("two");
        write_addon(&two, "t.two");
        store.install_addon(&two).unwrap();
        store
            .update_profile("grid", |p| {
                p.set_enabled("t.two", true)?;
                p.set_enabled("t.one", true)
            })
            .unwrap();
        let ids =
            |name: &str| -> Vec<String> { listed(&store, name).into_iter().map(|(id, _)| id).collect() };
        assert_eq!(ids("default"), ["t.one", "t.two"]);
        assert_eq!(ids("grid"), ["t.one", "t.two"]);
        store.remove_addon("t.one").unwrap();
        assert_eq!(ids("default"), ["t.two"]);
        assert_eq!(ids("grid"), ["t.two"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_new_profile_lists_every_installed_addon_off() {
        let root = tmp("new-profile-all");
        let store = Store::new(root.join("data"));
        store.save_profile(&Profile::new("default")).unwrap();
        let src = root.join("one");
        write_addon(&src, "t.one");
        store.install_addon(&src).unwrap();
        store.create_profile("fresh", None).unwrap();
        assert_eq!(listed(&store, "fresh").last(), Some(&("t.one".to_owned(), false)));
        let enabled: Vec<String> = store
            .profile("fresh")
            .unwrap()
            .enabled()
            .map(str::to_owned)
            .collect();
        assert_eq!(enabled, ["swofty.core"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_profile_that_lacked_an_addon_loads_unchanged_and_the_on_set_is_what_it_was() {
        let mut p = Profile::new("old");
        p.set_enabled("t.a", true).unwrap();
        p.set_enabled("t.b", false).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert!(!back.has("t.c"));
        assert_eq!(back.enabled().collect::<Vec<_>>(), ["swofty.core", "t.a"]);
        let mut grown = back.clone();
        grown.add_addon("t.c").unwrap();
        assert_eq!(
            grown.enabled().collect::<Vec<_>>(),
            back.enabled().collect::<Vec<_>>()
        );
    }

    #[test]
    fn migrate_backfill_runs_once() {
        let (root, store) = v3_store("backfill-once", &["a.one", "a.two"]);
        store.save_profile(&Profile::new("grid")).unwrap();
        store.migrate().unwrap();
        let path = store.profiles_dir().join("grid.json");
        let after = std::fs::read(&path).unwrap();
        assert!(store.migrate().unwrap().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), after);

        store.update_profile("grid", |p| p.remove_addon("a.two")).unwrap();
        let removed = std::fs::read(&path).unwrap();
        assert!(store.migrate().unwrap().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), removed);
        assert_eq!(listed(&store, "grid"), [("a.one".to_owned(), false)]);

        std::fs::write(store.config_path(), r#"{"data_version":3}"#).unwrap();
        store.migrate().unwrap();
        assert_eq!(listed(&store, "grid").len(), 2);
        let again = std::fs::read(&path).unwrap();
        std::fs::write(store.config_path(), r#"{"data_version":3}"#).unwrap();
        store.migrate().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), again);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn migrate_leaves_a_missing_data_dir_alone() {
        let root = tmp("migrate-missing").join("data");
        assert!(Store::new(&root).migrate().unwrap().is_empty());
        assert!(!root.exists());
    }

    #[test]
    fn reads_profiles_that_list_mods_and_writes_addons() {
        let root = tmp("old-profile");
        let store = Store::new(&root);
        let path = store.profiles_dir().join("grid.json");
        std::fs::create_dir_all(store.profiles_dir()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema_version":1,"name":"grid","mods":[{"id":"swofty.core"},{"id":"a.b","enabled":false},{"id":"c.d"}],"options":{"c.d":{"x":1}}}"#,
        )
        .unwrap();
        let p = store.profile("grid").unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID, "c.d"]);
        store.save_profile(&p).unwrap();
        let saved: serde_json::Value = crate::read_json(&path).unwrap();
        assert_eq!(saved["schema_version"], profile::SCHEMA_VERSION);
        assert_eq!(
            saved["addons"][1],
            serde_json::json!({"id": "a.b", "enabled": false})
        );
        assert!(saved.get("mods").is_none());
        assert_eq!(saved["options"]["c.d"]["x"], 1);
        assert_eq!(store.profile("grid").unwrap(), p);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn merges_in_game_requests_into_the_profile() {
        let root = tmp("requests");
        let store = Store::new(&root);
        let path = store.requests_path(DEFAULT_PROFILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema":1,"profile":"default","seq":1,"options":{"swofty.core":{"show_launcher_panel":false}}}"#,
        )
        .unwrap();
        let out = store.merge_requests(DEFAULT_PROFILE, false).unwrap().unwrap();
        assert_eq!(out.changed, ["swofty.core.show_launcher_panel = Off"]);
        assert!(path.is_file());
        let core = crate::builtin::core_manifest();
        let p = store.profile(DEFAULT_PROFILE).unwrap();
        assert_eq!(p.option(&core, &core.options[0]), false);
        let again = store.merge_requests(DEFAULT_PROFILE, true).unwrap().unwrap();
        assert!(again.changed.is_empty());
        assert!(!path.exists());
        assert_eq!(store.merge_requests(DEFAULT_PROFILE, true).unwrap(), None);
        let s = store.session_settings(DEFAULT_PROFILE).unwrap();
        assert_eq!(s.addons[0].options[0].current, 0);
        assert!(s.requests.unwrap().ends_with("default.json"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn leaves_builtins_without_options_out_of_addon_settings() {
        let root = tmp("builtin-settings");
        let parse = |json: &str| AddonManifest::parse(json).unwrap();
        let builtin = |m| Builtin {
            addon: Arc::new(crate::registry::DeclaredAddon::new(m)),
            locked: false,
            default_on: true,
        };
        let tuned = || {
            builtin(parse(
                r#"{"schema_version":1,"id":"t.tuned","name":"Tuned","version":"1.0.0","author":"t","compat":{"game":"f1_25"},
                    "options":[{"id":"on","type":"toggle","label":"On","default":true,"binds":"native"}]}"#,
            ))
        };
        let quiet = || {
            vec![
                builtin(parse(
                    r#"{"schema_version":1,"id":"t.library","name":"Library","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#,
                )),
                builtin(parse(
                    r#"{"schema_version":1,"id":"t.native","name":"Native","version":"1.0.0","author":"t","compat":{"game":"f1_25"},
                        "operations":{"native":[{"dll":"native/n.dll"}]}}"#,
                )),
                builtin(parse(
                    r#"{"schema_version":1,"id":"t.pages","name":"Pages","version":"1.0.0","author":"t","compat":{"game":"f1_25"},
                        "operations":{"screens":[{"name":"pages_screen","file":"ui/p.ui"}]}}"#,
                )),
            ]
        };
        write_addon(&Store::new(&root).addons_dir().join("a.user"), "a.user");
        crate::write_json(
            &crate::launcher_saves::marker_path(&root),
            &crate::launcher_saves::Marker {
                at: 0,
                launcher: String::new(),
                profiles: Vec::new(),
                note: String::new(),
                seen: true,
            },
        )
        .unwrap();
        let plain = Store::new(&root).with_builtins(vec![tuned()]);
        let mut expected = SessionSettings::build(
            &plain.profile(DEFAULT_PROFILE).unwrap(),
            &plain.addons().unwrap(),
            Some(crate::paths::game_folder_view(
                &plain.requests_path(DEFAULT_PROFILE),
            )),
        )
        .unwrap();
        for a in &mut expected.addons {
            a.folder = plain.is_builtin(&a.id).then(|| "Built In".to_owned());
        }
        expected.launcher_saves = Some(settings::SessionSaves {
            profile: DEFAULT_PROFILE.into(),
            remote: crate::paths::game_folder_view(&crate::launcher_saves::remote_dir(
                &plain.root,
                DEFAULT_PROFILE,
            )),
        });
        let plain_json = serde_json::to_vec(&plain.session_settings(DEFAULT_PROFILE).unwrap()).unwrap();
        assert_eq!(plain_json, serde_json::to_vec(&expected).unwrap());

        let mut all = quiet();
        all.push(tuned());
        let store = Store::new(&root).with_builtins(all);
        let p = store.profile(DEFAULT_PROFILE).unwrap();
        assert_eq!(
            p.enabled().collect::<Vec<_>>(),
            [CORE_ID, "t.library", "t.native", "t.pages", "t.tuned"]
        );
        let s = store.session_settings(DEFAULT_PROFILE).unwrap();
        let ids: Vec<&str> = s.addons.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, [CORE_ID, "t.tuned", "a.user"]);
        let screens: Vec<&str> =
            s.ui.as_ref()
                .unwrap()
                .screens
                .iter()
                .map(|x| x.name.as_str())
                .collect();
        assert_eq!(screens, ["pages_screen"]);
        let mut without_pages = s.clone();
        without_pages.ui = None;
        assert_eq!(serde_json::to_vec(&without_pages).unwrap(), plain_json);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_profile_keeps_playing_on_the_steam_save_until_an_import_finds_one() {
        let root = tmp("steam-pending");
        let store = Store::new(&root);
        assert!(
            store
                .session_settings(DEFAULT_PROFILE)
                .unwrap()
                .launcher_saves
                .is_none()
        );
        let remote = crate::launcher_saves::remote_dir(&root, DEFAULT_PROFILE);
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join("savegame@profile#X"), b"x").unwrap();
        assert!(
            store
                .session_settings(DEFAULT_PROFILE)
                .unwrap()
                .launcher_saves
                .is_some()
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn manages_profiles() {
        let root = tmp("profiles");
        let store = Store::new(&root);
        assert_eq!(store.profile_names().unwrap(), ["vanilla", "default"]);
        assert_eq!(store.active_profile(), "default");
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled("a.b", true).unwrap();
        store.save_profile(&p).unwrap();
        store.set_active_profile(DEFAULT_PROFILE).unwrap();
        store.create_profile("grid", Some(DEFAULT_PROFILE)).unwrap();
        assert!(store.create_profile("grid", None).is_err());
        assert!(store.create_profile("vanilla", None).is_err());
        store.rename_profile(DEFAULT_PROFILE, "main").unwrap();
        assert_eq!(store.profile_names().unwrap(), ["vanilla", "grid", "main"]);
        assert_eq!(store.active_profile(), "main");
        let grid = store.profile("grid").unwrap();
        assert_eq!(grid.enabled().collect::<Vec<_>>(), [CORE_ID, "a.b"]);
        std::fs::remove_dir_all(&root).ok();
    }

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }

    #[test]
    fn deletes_a_profile_with_its_own_files_only() {
        let root = tmp("delete-profile");
        let store = Store::new(&root);
        write_addon(&store.addons_dir().join("a.user"), "a.user");
        store.create_profile("grid", None).unwrap();
        store.create_profile("main", None).unwrap();
        store.set_active_profile("main").unwrap();
        let own = [
            store.overlay_dir("grid", "h1").join(crate::build::PLAN_FILE),
            store.requests_path("grid"),
            store.saves_dir("grid").join("career.sav"),
        ];
        let kept = [
            store.overlay_dir("main", "h2").join(crate::build::PLAN_FILE),
            store.requests_path("main"),
            store.saves_dir("main").join("career.sav"),
            root.join("cache/maps/okayama/a1/.done"),
            store.addon_data_dir("a.user").join("library.json"),
        ];
        own.iter().chain(&kept).for_each(|f| touch(f));

        let plan = store.plan_profile_deletion("grid").unwrap();
        assert_eq!(plan.switch_to, None);
        assert!(plan.saves);
        assert_eq!(
            plan.paths,
            [
                store.overlays_dir("grid"),
                store.requests_path("grid"),
                store.saves_dir("grid"),
                store.profiles_dir().join("grid.json"),
            ]
        );
        assert!(own.iter().all(|f| f.is_file()));

        assert_eq!(store.delete_profile("grid").unwrap(), plan);
        assert_eq!(store.profile_names().unwrap(), ["vanilla", "main"]);
        assert!(plan.paths.iter().all(|p| !p.exists()));
        for f in &kept {
            assert!(f.is_file(), "{} was removed", f.display());
        }
        assert!(store.addons().unwrap().contains_key("a.user"));
        assert_eq!(store.active_profile(), "main");
        let err = store.delete_profile("grid").unwrap_err();
        assert!(err.to_string().contains("no profile named grid"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_vr_choice_follows_its_profile() {
        use crate::session::LaunchMode;
        let root = tmp("vr-profile");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.create_profile("main", None).unwrap();
        let mut config = store.config();
        config.set_launch_mode("grid", LaunchMode::Vr);
        store.save_config(&config).unwrap();
        store.rename_profile("grid", "vrgrid").unwrap();
        let config = store.config();
        assert_eq!(config.launch_mode("vrgrid"), LaunchMode::Vr);
        assert_eq!(config.launch_mode("grid"), LaunchMode::Normal);
        store.delete_profile("vrgrid").unwrap();
        assert!(store.config().vr_profiles.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_renamed_profile_keeps_its_vault_careers_and_import_record() {
        let root = tmp("rename-vault");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        let vault = root.join("career-vault").join("launcher-grid");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(vault.join("marker"), "x").unwrap();
        let saves = root.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(
            saves.join("imported.json"),
            r#"{"at":1,"launcher":"1","profiles":["grid","other"]}"#,
        )
        .unwrap();
        let careers = root.join("careers");
        std::fs::create_dir_all(&careers).unwrap();
        std::fs::write(
            careers.join("index.json"),
            r#"{"version":1,"careers":[{"career":"a","profile":"grid"},{"career":"b","profile":"other"}]}"#,
        )
        .unwrap();
        store.rename_profile("grid", "vrgrid").unwrap();
        assert!(!vault.exists());
        assert!(
            root.join("career-vault")
                .join("launcher-vrgrid")
                .join("marker")
                .is_file()
        );
        let marker = crate::launcher_saves::marker(&root).unwrap();
        assert_eq!(marker.profiles, ["vrgrid", "other"]);
        let index = crate::stdlib::careers::read(&crate::stdlib::careers::data_path(&root));
        assert_eq!(index[0].profile.as_deref(), Some("vrgrid"));
        assert_eq!(index[1].profile.as_deref(), Some("other"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn deleting_a_profile_keeps_its_versions_in_the_vault() {
        let root = tmp("delete-keeps-vault");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.create_profile("main", None).unwrap();
        let vault = root.join("career-vault").join("launcher-grid");
        std::fs::create_dir_all(&vault).unwrap();
        let plan = store.plan_profile_deletion("grid").unwrap();
        assert!(!plan.paths.contains(&vault));
        store.delete_profile("grid").unwrap();
        assert!(vault.is_dir());
        assert!(!store.profile_names().unwrap().iter().any(|n| n == "grid"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_rename_onto_a_deleted_profiles_name_keeps_both_vaults() {
        let root = tmp("rename-onto-deleted-vault");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.create_profile("main", None).unwrap();
        let vaults = root.join("career-vault");
        for prefix in ["launcher-", "launcher-ea."] {
            for name in ["grid", "main"] {
                let vault = vaults.join(format!("{prefix}{name}"));
                std::fs::create_dir_all(&vault).unwrap();
                std::fs::write(vault.join("marker"), format!("{name}'s")).unwrap();
            }
        }
        store.delete_profile("grid").unwrap();
        store.rename_profile("main", "grid").unwrap();
        for prefix in ["launcher-", "launcher-ea."] {
            let read = |vault: &Path| std::fs::read_to_string(vault.join("marker")).unwrap();
            assert_eq!(read(&vaults.join(format!("{prefix}grid"))), "main's", "{prefix}");
            assert!(!vaults.join(format!("{prefix}main")).exists(), "{prefix}");
            let aside: Vec<PathBuf> = std::fs::read_dir(&vaults)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(&format!("{prefix}grid.deleted."))
                })
                .collect();
            assert_eq!(aside.len(), 1, "{prefix}: {aside:?}");
            assert_eq!(read(&aside[0]), "grid's", "{prefix}");
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn keeps_vanilla_and_the_last_profile() {
        let root = tmp("delete-last");
        let store = Store::new(&root);
        let err = store.delete_profile(DEFAULT_PROFILE).unwrap_err();
        assert!(err.to_string().contains("only profile"), "{err}");
        store.create_profile("grid", None).unwrap();
        let err = store.delete_profile("grid").unwrap_err();
        assert!(err.to_string().contains("only profile"), "{err}");
        let err = store.delete_profile(profile::VANILLA).unwrap_err();
        assert!(err.to_string().contains("built in"), "{err}");
        assert!(store.delete_profile("../config").is_err());
        assert!(store.profiles_dir().join("grid.json").is_file());
        store.create_profile("main", None).unwrap();
        let plan = store.plan_profile_deletion("grid").unwrap();
        assert!(!plan.saves);
        assert_eq!(plan.paths, [store.profiles_dir().join("grid.json")]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn deleting_the_active_profile_switches_first() {
        let root = tmp("delete-active");
        let store = Store::new(&root);
        store
            .save_profile(&store.profile(DEFAULT_PROFILE).unwrap())
            .unwrap();
        store.create_profile("grid", None).unwrap();
        store.create_profile("zeta", None).unwrap();
        store.set_active_profile("grid").unwrap();
        crate::write_json(
            &store.launch_request_path(),
            &serde_json::json!({"id": "r1", "profile": "grid", "apply": true}),
        )
        .unwrap();

        let plan = store.delete_profile("grid").unwrap();
        assert_eq!(plan.switch_to.as_deref(), Some(DEFAULT_PROFILE));
        assert_eq!(store.config().active_profile.as_deref(), Some(DEFAULT_PROFILE));
        assert_eq!(store.active_profile(), DEFAULT_PROFILE);
        let request: serde_json::Value = crate::read_json(&store.launch_request_path()).unwrap();
        assert_eq!(request, serde_json::json!({"id": "r1", "apply": true}));

        store.create_profile("alpha", None).unwrap();
        crate::write_json(
            &store.launch_request_path(),
            &serde_json::json!({"id": "r2", "profile": "zeta"}),
        )
        .unwrap();
        let plan = store.delete_profile(DEFAULT_PROFILE).unwrap();
        assert_eq!(plan.switch_to.as_deref(), Some("alpha"));
        assert_eq!(store.active_profile(), "alpha");
        assert_eq!(store.profile_names().unwrap(), ["vanilla", "alpha", "zeta"]);
        let request: serde_json::Value = crate::read_json(&store.launch_request_path()).unwrap();
        assert_eq!(request["profile"], "zeta");

        assert_eq!(store.delete_profile("zeta").unwrap().switch_to, None);
        assert_eq!(store.active_profile(), "alpha");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_name_that_differs_only_in_capitals_is_taken() {
        let root = tmp("create-capitals");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        for name in ["Grid", "GRID", "Vanilla"] {
            let err = store.create_profile(name, None).unwrap_err();
            assert!(err.to_string().contains("already exists"), "{name}: {err}");
        }
        assert!(!store.profile_names().unwrap().contains(&"Grid".to_owned()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_profile_named_with_a_leading_dash_keeps_working_but_no_new_one_gets_that_name() {
        let root = tmp("leading-dash");
        let store = Store::new(&root);
        // Earlier launchers let a profile be named this way.
        store.save_profile(&Profile::new("-swofty")).unwrap();
        store.set_active_profile("-swofty").unwrap();
        touch(&store.saves_dir("-swofty").join("remote/save"));

        assert!(store.profile_names().unwrap().contains(&"-swofty".to_owned()));
        assert_eq!(store.active_profile(), "-swofty");
        assert_eq!(store.profile("-swofty").unwrap().name, "-swofty");

        for name in ["-grid", "--profile"] {
            let err = store.create_profile(name, None).unwrap_err();
            assert!(err.to_string().contains("starts with -"), "{name}: {err}");
            let err = store.create_profile(name, Some("-swofty")).unwrap_err();
            assert!(err.to_string().contains("starts with -"), "{name}: {err}");
            let err = store.rename_profile("-swofty", name).unwrap_err();
            assert!(err.to_string().contains("starts with -"), "{name}: {err}");
        }
        let err = store.rename_profile("-swofty", "-Swofty").unwrap_err();
        assert!(err.to_string().contains("starts with -"), "{err}");
        assert!(store.profiles_dir().join("-swofty.json").is_file());
        assert!(store.saves_dir("-swofty").join("remote/save").is_file());

        assert_eq!(store.rename_profile("-swofty", "swofty").unwrap().name, "swofty");
        assert_eq!(store.active_profile(), "swofty");
        assert!(store.saves_dir("swofty").join("remote/save").is_file());
        assert!(!store.profile_names().unwrap().contains(&"-swofty".to_owned()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn renames_a_profile_to_other_capitals() {
        let root = tmp("rename-capitals");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.set_active_profile("grid").unwrap();
        touch(&store.saves_dir("grid").join("remote/save"));

        assert_eq!(store.rename_profile("grid", "Grid").unwrap().name, "Grid");

        let names = store.profile_names().unwrap();
        assert!(names.contains(&"Grid".to_owned()), "{names:?}");
        assert!(!names.contains(&"grid".to_owned()), "{names:?}");
        assert!(!names.iter().any(|n| n.starts_with("renaming")), "{names:?}");
        assert_eq!(store.profile("Grid").unwrap().name, "Grid");
        assert_eq!(store.active_profile(), "Grid");
        assert!(store.saves_dir("Grid").join("remote/save").is_file());
        std::fs::remove_dir_all(&root).ok();
    }

    // Both names can only exist where file names are case-sensitive.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_rename_to_the_capitals_of_a_profile_that_exists_changes_nothing() {
        let root = tmp("rename-both");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        // Only a case-sensitive folder from before names were compared this way holds both.
        store.save_profile(&Profile::new("Grid")).unwrap();

        let err = store.rename_profile("grid", "Grid").unwrap_err();
        assert!(err.to_string().contains("already exists"), "{err}");
        let names = store.profile_names().unwrap();
        assert!(names.contains(&"grid".to_owned()) && names.contains(&"Grid".to_owned()));
        assert!(!names.iter().any(|n| n.starts_with("renaming")), "{names:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn refuses_renaming_a_profile_installed_in_the_game_folder() {
        let root = tmp("rename-applied");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.set_active_profile("grid").unwrap();
        touch(&store.saves_dir("grid").join("remote/save"));
        let j = crate::journal::Journal::create(&root, "grid", "build-1", true).unwrap();

        let err = store.rename_profile("grid", "main").unwrap_err();
        assert!(err.to_string().contains("installed in the game folder"), "{err}");
        assert!(err.to_string().contains(&j.id), "{err}");
        assert!(store.profiles_dir().join("grid.json").is_file());
        assert!(!store.profiles_dir().join("main.json").exists());
        assert!(store.saves_dir("grid").join("remote/save").is_file());
        assert_eq!(store.active_profile(), "grid");

        j.close().unwrap();
        store.rename_profile("grid", "main").unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_rename_onto_saves_left_behind_changes_nothing() {
        let root = tmp("rename-orphan");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.set_active_profile("grid").unwrap();
        touch(&store.saves_dir("grid").join("remote/save"));
        touch(&store.saves_dir("main").join("remote/old"));

        let err = store.rename_profile("grid", "main").unwrap_err();
        assert!(err.to_string().contains("already holds saves"), "{err}");
        assert!(store.profiles_dir().join("grid.json").is_file());
        assert!(!store.profiles_dir().join("main.json").exists());
        assert!(store.saves_dir("grid").join("remote/save").is_file());
        assert!(store.saves_dir("main").join("remote/old").is_file());
        assert_eq!(store.active_profile(), "grid");

        // An empty folder holds nothing to lose, and doesn't stop the rename.
        std::fs::remove_dir_all(store.saves_dir("main")).unwrap();
        std::fs::create_dir_all(store.saves_dir("main")).unwrap();
        store.rename_profile("grid", "main").unwrap();
        assert!(store.saves_dir("main").join("remote/save").is_file());
        std::fs::remove_dir_all(&root).ok();
    }

    // A file in the saves' way makes the move fail here; Windows may not fail the same way.
    #[cfg(unix)]
    #[test]
    fn a_rename_that_cannot_move_the_saves_puts_the_profile_back() {
        let root = tmp("rename-undo");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.set_active_profile("grid").unwrap();
        touch(&store.saves_dir("grid").join("remote/save"));
        touch(&store.saves_dir("main"));

        assert!(store.rename_profile("grid", "main").is_err());

        assert!(store.profiles_dir().join("grid.json").is_file());
        assert!(!store.profiles_dir().join("main.json").exists());
        assert!(store.saves_dir("grid").join("remote/save").is_file());
        assert_eq!(store.active_profile(), "grid");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn refuses_a_profile_installed_in_the_game_folder() {
        let root = tmp("delete-applied");
        let store = Store::new(&root);
        store.create_profile("grid", None).unwrap();
        store.create_profile("main", None).unwrap();
        touch(&store.overlay_dir("grid", "h1").join(crate::build::PLAN_FILE));
        let j = crate::journal::Journal::create(&root, "grid", "build-1", false).unwrap();

        let err = store.delete_profile("grid").unwrap_err();
        assert!(err.to_string().contains("installed in the game folder"), "{err}");
        assert!(err.to_string().contains(&j.id), "{err}");
        assert!(store.profiles_dir().join("grid.json").is_file());
        assert!(store.overlays_dir("grid").is_dir());
        assert_eq!(store.plan_profile_deletion("main").unwrap().switch_to, None);

        j.close().unwrap();
        store.delete_profile("grid").unwrap();
        assert!(!store.overlays_dir("grid").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    fn denied() -> std::io::Error {
        std::io::Error::from(std::io::ErrorKind::PermissionDenied)
    }

    fn real_rename(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn real_delete(dir: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(dir)
    }

    fn old_copies(store: &Store) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(store.addons_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(OLD_PREFIX) || n.starts_with(STAGING_PREFIX))
            .collect();
        out.sort();
        out
    }

    fn installed_with_update(name: &str) -> (PathBuf, Store, PathBuf, PathBuf) {
        let root = tmp(name);
        let store = Store::new(&root);
        let v1 = root.join("v1");
        std::fs::create_dir_all(v1.join("base")).unwrap();
        std::fs::write(v1.join(MANIFEST_FILE), manifest_json("a.livery")).unwrap();
        std::fs::write(v1.join("base/common.erp"), b"v1").unwrap();
        store.install_addon(&v1).unwrap();
        let dest = store.addons_dir().join("a.livery");
        let staging = store.addons_dir().join(format!("{STAGING_PREFIX}a.livery"));
        std::fs::create_dir_all(staging.join("base")).unwrap();
        std::fs::write(
            staging.join(MANIFEST_FILE),
            manifest_json("a.livery").replace("1.0.0", "1.1.0"),
        )
        .unwrap();
        std::fs::write(staging.join("base/common.erp"), b"v2").unwrap();
        (root, store, dest, staging)
    }

    #[test]
    fn finish_swaps_leaves_a_swap_another_process_is_making() {
        let (root, store, dest, staging) = installed_with_update("swap-live");
        let aside = store.addons_dir().join(format!("{OLD_PREFIX}a.livery"));
        std::fs::rename(&dest, &aside).unwrap();
        let lock = SwapLock::try_acquire(&store.addons_dir(), "a.livery")
            .unwrap()
            .unwrap();
        let notes = store.finish_swaps();
        assert!(aside.is_dir() && staging.is_dir() && !dest.exists());
        assert!(notes.iter().any(|n| n.contains("another launcher")));
        drop(lock);
        let notes = store.finish_swaps();
        assert!(dest.is_dir() && !aside.exists() && !staging.exists());
        assert!(notes.iter().any(|n| n.contains("put back a.livery")));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_swap_waits_out_nothing_when_another_process_holds_the_same_addon() {
        let (root, store, dest, staging) = installed_with_update("swap-contended");
        let lock = SwapLock::try_acquire(&store.addons_dir(), "a.livery")
            .unwrap()
            .unwrap();
        let e = swap_in(
            &staging,
            &dest,
            "A",
            &mut |from, to| std::fs::rename(from, to),
            &mut |d| std::fs::remove_dir_all(d),
        )
        .unwrap_err();
        assert!(e.to_string().contains("another Swofty launcher"));
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v1");
        drop(lock);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_update_blocked_by_a_file_in_use_keeps_the_installed_copy_whole() {
        let (root, store, dest, staging) = installed_with_update("swap-locked");
        let mut tries = 0;
        let mut deletes = 0;
        let e = swap_in(
            &staging,
            &dest,
            "Livery",
            &mut |from, to| {
                if from == dest.as_path() {
                    tries += 1;
                    return Err(denied());
                }
                real_rename(from, to)
            },
            &mut |_| {
                deletes += 1;
                Ok(())
            },
        )
        .unwrap_err()
        .to_string();
        assert_eq!(tries, 6);
        assert_eq!(deletes, 0);
        assert!(
            e.starts_with("Couldn't update Livery: its files are in use"),
            "{e}"
        );
        assert!(e.contains("Close other Swofty Launcher windows"), "{e}");
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v1");
        assert_eq!(store.addons().unwrap()["a.livery"].version.to_string(), "1.0.0");
        assert!(old_copies(&store).is_empty(), "{:?}", old_copies(&store));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_failed_swap_puts_the_installed_copy_back() {
        let (root, store, dest, staging) = installed_with_update("swap-put-back");
        let e = swap_in(
            &staging,
            &dest,
            "Livery",
            &mut |from, to| {
                if from == staging.as_path() {
                    return Err(std::io::Error::other("disk gone"));
                }
                real_rename(from, to)
            },
            &mut real_delete,
        )
        .unwrap_err()
        .to_string();
        assert_eq!(e, "Couldn't update Livery: disk gone");
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v1");
        assert!(old_copies(&store).is_empty(), "{:?}", old_copies(&store));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_old_copy_still_in_use_is_deleted_on_the_next_start() {
        let (root, store, dest, staging) = installed_with_update("swap-old-locked");
        swap_in(
            &staging,
            &dest,
            "Livery",
            &mut real_rename,
            &mut |_| Err(denied()),
        )
        .unwrap();
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v2");
        assert_eq!(store.addons().unwrap()["a.livery"].version.to_string(), "1.1.0");
        assert_eq!(old_copies(&store), [format!("{OLD_PREFIX}a.livery")]);

        let notes = store.migrate().unwrap();
        assert!(
            notes.iter().any(|n| n.starts_with("deleted the old copy")),
            "{notes:?}"
        );
        assert!(old_copies(&store).is_empty(), "{:?}", old_copies(&store));
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v2");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_update_stopped_part_way_is_put_back_on_the_next_start() {
        let (root, store, dest, staging) = installed_with_update("swap-interrupted");
        let aside = set_aside(&dest, &mut real_rename).unwrap();
        assert!(!dest.exists());
        let notes = store.migrate().unwrap();
        assert!(
            notes.iter().any(|n| n.starts_with("put back a.livery")),
            "{notes:?}"
        );
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v1");
        assert!(!aside.exists());
        assert!(!staging.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_second_old_copy_gets_its_own_name() {
        let (root, store, dest, staging) = installed_with_update("swap-second-old");
        std::fs::create_dir_all(store.addons_dir().join(format!("{OLD_PREFIX}a.livery"))).unwrap();
        swap_in(
            &staging,
            &dest,
            "Livery",
            &mut real_rename,
            &mut |_| Err(denied()),
        )
        .unwrap();
        assert_eq!(
            old_copies(&store),
            [format!("{OLD_PREFIX}a.livery"), format!("{OLD_PREFIX}a.livery-1")]
        );
        store.migrate().unwrap();
        assert!(old_copies(&store).is_empty(), "{:?}", old_copies(&store));
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v2");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_removal_blocked_by_a_file_in_use_deletes_nothing() {
        let (root, _, dest, _) = installed_with_update("remove-locked");
        let mut deletes = 0;
        let e = delete_addon_dir(&dest, "Livery", &mut |_, _| Err(denied()), &mut |_| {
            deletes += 1;
            Ok(())
        })
        .unwrap_err()
        .to_string();
        assert_eq!(deletes, 0);
        assert!(
            e.starts_with("Couldn't remove Livery: its files are in use"),
            "{e}"
        );
        assert!(!e.contains(&root.display().to_string()), "{e}");
        assert_eq!(std::fs::read(dest.join("base/common.erp")).unwrap(), b"v1");

        let e = delete_addon_dir(
            &dest,
            "Livery",
            &mut |_, _| Err(std::io::Error::other("disk gone")),
            &mut real_delete,
        )
        .unwrap_err()
        .to_string();
        assert_eq!(e, "Couldn't remove Livery: disk gone");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_removed_addon_whose_files_are_still_in_use_is_gone_and_deleted_on_the_next_start() {
        let (root, store, dest, staging) = installed_with_update("remove-old-locked");
        std::fs::remove_dir_all(&staging).unwrap();
        store.remove_addon_with("a.livery", |_| Err(denied())).unwrap();
        assert!(!dest.exists());
        assert!(!store.addons().unwrap().contains_key("a.livery"));
        assert_eq!(old_copies(&store), [format!("{OLD_PREFIX}a.livery")]);
        store.migrate().unwrap();
        assert!(old_copies(&store).is_empty(), "{:?}", old_copies(&store));
        assert!(!dest.exists());
        std::fs::remove_dir_all(&root).ok();
    }
}
