use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swofty_addon::launcher::{
    Addon, AddonInfo, AddonStore, CommandSpec, Cx, GameFiles, GameStatus, ProfileInfo, StateStore,
};

use crate::baseline::{Baseline, FileRecord, Objects};
use crate::builtin::CORE_ID;
use crate::helper::HelperAddon;
use crate::install::Game;
use crate::manifest::AddonManifest;
use crate::profile::Profile;
use crate::saves::SavesWarning;
use crate::store::Store;
use crate::{Error, Result};

pub struct DeclaredAddon {
    manifest: AddonManifest,
}

impl DeclaredAddon {
    pub fn new(manifest: AddonManifest) -> Self {
        Self { manifest }
    }
}

impl Addon for DeclaredAddon {
    fn manifest(&self) -> &AddonManifest {
        &self.manifest
    }
}

#[derive(Clone)]
pub struct Entry {
    pub addon: Arc<dyn Addon>,
    pub declared: bool,
    pub helper: bool,
    pub plugin: Option<Arc<HelperAddon>>,
    pub locked: bool,
    pub default_on: bool,
}

impl Entry {
    pub fn id(&self) -> &str {
        &self.addon.manifest().id
    }
}

#[derive(Clone, Default)]
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    pub fn load(store: &Store) -> Result<Self> {
        let entries = store
            .addons()?
            .into_values()
            .map(|m| {
                let managed = crate::managed::is_managed(&m.id);
                match store.builtins().iter().find(|b| b.addon.manifest().id == m.id) {
                    Some(b) => Entry {
                        addon: b.addon.clone(),
                        declared: false,
                        helper: false,
                        plugin: None,
                        locked: b.locked,
                        default_on: b.default_on,
                    },
                    None => match store.helpers().get(&m, &store.addons_dir().join(&m.id)) {
                        Some(h) => Entry {
                            addon: h.clone(),
                            declared: false,
                            helper: true,
                            plugin: Some(h),
                            locked: managed,
                            default_on: managed,
                        },
                        None => Entry {
                            locked: m.id == CORE_ID || managed,
                            default_on: m.id == CORE_ID || managed,
                            declared: true,
                            helper: false,
                            plugin: None,
                            addon: Arc::new(DeclaredAddon::new(m)),
                        },
                    },
                }
            })
            .collect();
        Ok(Self { entries })
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        // Store::addons returns a BTreeMap keyed by id, so entries stay sorted.
        self.entries
            .binary_search_by(|e| e.id().cmp(id))
            .ok()
            .map(|i| &self.entries[i])
    }

    pub fn set_enabled(&self, p: &mut Profile, id: &str, on: bool) -> Result<()> {
        if let Some(e) = self.get(id).filter(|e| e.locked && !on && !p.is_vanilla()) {
            return Err(Error::Other(format!(
                "{} ({id}) is always on and cannot be disabled",
                e.addon.manifest().name
            )));
        }
        p.set_enabled(id, on)
    }

    pub fn turn_off_warning(&self, p: &Profile, id: &str) -> Option<SavesWarning> {
        let e = self.get(id).filter(|e| !e.locked)?;
        if !p.enabled().any(|on| on == id) {
            return None;
        }
        SavesWarning::of(e.addon.manifest())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn manifests(&self) -> BTreeMap<String, AddonManifest> {
        self.entries
            .iter()
            .map(|e| (e.id().to_owned(), e.addon.manifest().clone()))
            .collect()
    }

    pub fn commands(&self, reserved: &[&str]) -> (Vec<(String, CommandSpec)>, Vec<String>) {
        let mut taken: BTreeSet<String> = reserved.iter().map(|s| (*s).to_owned()).collect();
        let mut out = Vec::new();
        let mut refused = Vec::new();
        for e in &self.entries {
            for spec in e.addon.commands() {
                if taken.insert(spec.name.clone()) {
                    out.push((e.id().to_owned(), spec));
                } else {
                    refused.push(format!("{}: the command {} is taken", e.id(), spec.name));
                }
            }
        }
        (out, refused)
    }
}

pub struct ProfileState {
    store: Store,
    profile: String,
    /// The addon these calls come from.
    addon: String,
    read_only: bool,
}

impl StateStore for ProfileState {
    fn get(&self, addon: &str) -> swofty_addon::Result<Option<serde_json::Value>> {
        let p = self.store.profile(&self.profile).map_err(other)?;
        Ok(p.state.get(addon).cloned())
    }

    fn set(&self, addon: &str, value: serde_json::Value) -> swofty_addon::Result<()> {
        self.store
            .update_profile(&self.profile, |p| p.set_state(addon, value))
            .map_err(other)
    }
}

impl ProfileState {
    fn writable(&self) -> swofty_addon::Result<()> {
        match self.read_only {
            true => Err(swofty_addon::Error::Other(
                "addons can't be changed while conflicts are checked".into(),
            )),
            false => Ok(()),
        }
    }

    fn info(&self, m: &AddonManifest, p: &Profile) -> AddonInfo {
        AddonInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.to_string(),
            author: m.author.clone(),
            dir: self.store.addons_dir().join(&m.id),
            enabled: p.enabled().any(|on| on == m.id),
            position: p.addons.iter().position(|a| a.id == m.id),
        }
    }
}

impl AddonStore for ProfileState {
    // The enabled addons in the order a build loads them, then the rest by id.
    fn list(&self) -> swofty_addon::Result<Vec<AddonInfo>> {
        let p = self.store.profile(&self.profile).map_err(other)?;
        let mut addons = self.store.addons().map_err(other)?;
        let order = crate::resolver::resolve(&p, &addons)
            .map(|plan| plan.order)
            .unwrap_or_else(|_| p.enabled().map(str::to_owned).collect());
        let mut out: Vec<AddonInfo> = order
            .iter()
            .filter_map(|id| addons.remove(id))
            .map(|m| self.info(&m, &p))
            .collect();
        out.extend(addons.values().map(|m| self.info(m, &p)));
        Ok(out)
    }

    fn install(&self, path: &Path) -> swofty_addon::Result<AddonInfo> {
        self.writable()?;
        let m = self.store.install_addon_by(path, &self.addon).map_err(other)?;
        let p = self.store.profile(&self.profile).map_err(other)?;
        Ok(self.info(&m, &p))
    }

    // As the Addons page does, except that turning off or removing an addon that
    // keeps data in the saves is refused here, since only that page warns about it.
    fn set_enabled(&self, id: &str, on: bool) -> swofty_addon::Result<()> {
        self.writable()?;
        let registry = self.store.registry().map_err(other)?;
        let Some(entry) = registry.get(id) else {
            return Err(swofty_addon::Error::Other(format!(
                "no addon named {id} is installed"
            )));
        };
        let p = self.store.profile(&self.profile).map_err(other)?;
        if !on && registry.turn_off_warning(&p, id).is_some() {
            return Err(swofty_addon::Error::Other(format!(
                "{} keeps data in your saves: turn it off on the Addons page",
                entry.addon.manifest().name
            )));
        }
        self.store
            .update_profile(&self.profile, |p| registry.set_enabled(p, id, on))
            .map_err(other)
    }

    fn remove(&self, id: &str) -> swofty_addon::Result<()> {
        self.writable()?;
        if let Some(m) = self.store.addons().map_err(other)?.get(id)
            && SavesWarning::of(m).is_some()
        {
            return Err(swofty_addon::Error::Other(format!(
                "{} keeps data in your saves: remove it on the Addons page",
                m.name
            )));
        }
        self.store.remove_addon(id).map_err(other)
    }

    fn move_addon(&self, id: &str, delta: isize) -> swofty_addon::Result<()> {
        self.writable()?;
        self.store
            .update_profile(&self.profile, |p| p.move_addon(id, delta))
            .map_err(other)
    }
}

fn other(e: Error) -> swofty_addon::Error {
    match e {
        Error::Addon(e) => e,
        e => swofty_addon::Error::Other(e.to_string()),
    }
}

pub struct GameDir {
    data: PathBuf,
    game: Game,
    objects: Objects,
    files: BTreeMap<String, FileRecord>,
}

impl GameDir {
    pub fn new(store: &Store, game: &Game) -> Result<Self> {
        Ok(Self {
            data: store.root.clone(),
            game: game.clone(),
            objects: Objects {
                root: store.root.join("baseline/objects"),
            },
            files: Baseline::load_or_new(&store.root, &game.identity)?.files,
        })
    }
}

impl GameFiles for GameDir {
    fn build_key(&self) -> String {
        self.game.identity.key()
    }

    fn exe_version(&self) -> Option<String> {
        crate::exe_version::read(&self.game.dir.join(crate::GAME_EXE))
    }

    fn is_stock(&self, rel: &str) -> bool {
        self.files.contains_key(rel) || self.game.dir.join(rel).is_file()
    }

    fn original(&self, rel: &str) -> swofty_addon::Result<PathBuf> {
        let path = match self.files.get(rel) {
            Some(rec) => self.objects.path(&rec.sha256),
            None => self.game.dir.join(rel),
        };
        if path.is_file() {
            Ok(path)
        } else {
            Err(swofty_addon::Error::Other(format!(
                "{rel} is not in the game folder; if it is one of the game's own files, verify the game files in Steam or the EA app"
            )))
        }
    }

    fn original_lang(&self, code: &str) -> swofty_addon::Result<PathBuf> {
        crate::baseline::original_lang(&self.data, &self.game.dir, &self.game.identity, code).map_err(other)
    }

    fn dir(&self) -> Option<PathBuf> {
        Some(self.game.dir.clone())
    }

    fn addon_dir(&self, id: &str) -> Option<PathBuf> {
        Some(crate::addon_storage::live_dir(&self.data, &self.game.dir, id))
    }

    fn status(&self) -> GameStatus {
        crate::process::game_status(Some(&self.data))
    }

    fn steam_save_dir(&self) -> Option<PathBuf> {
        crate::steam::find_save_dir()
    }

    fn save_dir(&self, profile: &str) -> Option<PathBuf> {
        crate::vault::Location::of_profile(&self.data, profile)
            .map(|l| l.dir)
            .filter(|d| d.is_dir())
    }

    fn profile_tracks(
        &self,
        profile: &str,
    ) -> swofty_addon::Result<Option<swofty_addon::launcher::ProfileTracks>> {
        crate::tracks::for_profile(&self.data, &self.game, profile)
            .map(Some)
            .map_err(other)
    }

    fn profile_heads(
        &self,
        profile: &str,
    ) -> swofty_addon::Result<Option<swofty_addon::launcher::ProfileHeads>> {
        crate::heads::for_profile(&Store::new(self.data.clone()), &self.game, profile)
            .map(Some)
            .map_err(other)
    }
}

pub struct Host {
    pub addon: String,
    pub profile: ProfileInfo,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub game: Option<GameDir>,
    state: ProfileState,
}

impl Host {
    pub fn new(store: &Store, addon: &str, profile: &Profile, game: Option<&Game>) -> Result<Self> {
        let data_dir = store.addon_data_dir(addon);
        let cache_dir = store.addon_cache_dir(addon);
        for dir in [&data_dir, &cache_dir] {
            std::fs::create_dir_all(dir).map_err(crate::io_err(dir))?;
        }
        Ok(Self {
            addon: addon.into(),
            profile: ProfileInfo {
                name: profile.name.clone(),
                vanilla: profile.is_vanilla(),
                enabled: profile.enabled().any(|id| id == addon),
            },
            data_dir,
            cache_dir,
            game: game.map(|g| GameDir::new(store, g)).transpose()?,
            state: ProfileState {
                store: store.clone(),
                profile: profile.name.clone(),
                addon: addon.into(),
                read_only: false,
            },
        })
    }

    pub fn read_only(mut self, on: bool) -> Self {
        self.state.read_only = on;
        self
    }

    pub fn cx(&self) -> Cx<'_> {
        Cx::new(
            &self.addon,
            &self.profile,
            &self.data_dir,
            &self.cache_dir,
            self.game.as_ref().map(|g| g as &dyn GameFiles),
            &self.state,
        )
        .with_addons(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use swofty_addon::launcher::{Builtin, CommandSpec};

    use super::*;

    struct Fake {
        manifest: AddonManifest,
        command: &'static str,
    }

    impl Addon for Fake {
        fn manifest(&self) -> &AddonManifest {
            &self.manifest
        }

        fn commands(&self) -> Vec<CommandSpec> {
            vec![CommandSpec {
                name: self.command.into(),
                about: String::new(),
                args: Vec::new(),
                subcommands: Vec::new(),
            }]
        }
    }

    fn fake(id: &str, command: &'static str, default_on: bool) -> Builtin {
        let manifest = AddonManifest::parse(&format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
        ))
        .unwrap();
        Builtin {
            addon: Arc::new(Fake { manifest, command }),
            locked: false,
            default_on,
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-registry-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn lists_core_builtins_and_refuses_taken_commands() {
        let root = tmp("commands");
        let store =
            Store::new(&root).with_builtins(vec![fake("t.a", "mods", true), fake("t.b", "mods", false)]);
        let reg = Registry::load(&store).unwrap();
        assert_eq!(
            reg.iter().map(Entry::id).collect::<Vec<_>>(),
            [CORE_ID, "t.a", "t.b"]
        );
        assert!(reg.get(CORE_ID).unwrap().declared && reg.get(CORE_ID).unwrap().locked);
        assert!(!reg.get("t.a").unwrap().declared);
        let (commands, refused) = reg.commands(&["build"]);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "t.a");
        assert_eq!(refused, ["t.b: the command mods is taken"]);
        let (_, refused) = reg.commands(&["mods"]);
        assert_eq!(refused.len(), 2);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn locked_entries_stay_on() {
        let root = tmp("locked");
        let locked = Builtin {
            locked: true,
            ..fake("t.a", "a", true)
        };
        let store = Store::new(&root).with_builtins(vec![locked, fake("t.b", "b", true)]);
        let reg = Registry::load(&store).unwrap();
        let mut p = store.profile("default").unwrap();
        let e = reg.set_enabled(&mut p, "t.a", false).unwrap_err();
        assert_eq!(e.to_string(), "t.a (t.a) is always on and cannot be disabled");
        assert!(reg.set_enabled(&mut p, CORE_ID, false).is_err());
        reg.set_enabled(&mut p, "t.a", true).unwrap();
        reg.set_enabled(&mut p, "t.b", false).unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID, "t.a"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn warns_before_turning_off_an_addon_that_affects_saves() {
        let root = tmp("saves");
        let dir = root.join("addons/t.saves");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(crate::manifest::MANIFEST_FILE),
            r#"{"schema_version":1,"id":"t.saves","name":"Saves","version":"1.0.0","author":"t",
                "compat":{"game":"f1_25"},"saves":{"affects":true,"message":"m"}}"#,
        )
        .unwrap();
        let store = Store::new(&root).with_builtins(vec![fake("t.a", "a", true)]);
        let reg = Registry::load(&store).unwrap();
        let mut p = store.profile("default").unwrap();
        assert_eq!(reg.turn_off_warning(&p, "t.saves"), None);
        reg.set_enabled(&mut p, "t.saves", true).unwrap();
        let w = reg.turn_off_warning(&p, "t.saves").unwrap();
        assert_eq!((w.id.as_str(), w.message.as_deref()), ("t.saves", Some("m")));
        assert_eq!(reg.turn_off_warning(&p, "t.a"), None);
        assert_eq!(reg.turn_off_warning(&p, "t.missing"), None);
        assert_eq!(reg.turn_off_warning(&Profile::vanilla(), "t.saves"), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn state_lives_in_the_profile_and_follows_copies() {
        let root = tmp("state");
        let store = Store::new(&root).with_builtins(vec![fake("t.a", "a", true)]);
        let p = store.profile("default").unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID, "t.a"]);
        let host = Host::new(&store, "t.a", &p, None).unwrap();
        assert!(host.data_dir.ends_with("addon-data/t.a") && host.data_dir.is_dir());
        assert!(host.cache_dir.ends_with("cache/addons/t.a") && host.cache_dir.is_dir());
        let cx = host.cx();
        assert!(cx.profile.enabled);
        assert_eq!(cx.state::<Vec<String>>().unwrap(), Vec::<String>::new());
        cx.set_state(&["x"]).unwrap();
        assert_eq!(cx.state::<Vec<String>>().unwrap(), ["x"]);
        store.create_profile("copy", Some("default")).unwrap();
        assert_eq!(
            store.profile("copy").unwrap().state["t.a"],
            serde_json::json!(["x"])
        );
        let vanilla = Host::new(&store, "t.a", &Profile::vanilla(), None).unwrap();
        assert!(vanilla.cx().set_state(&["y"]).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_plugin_lists_installs_switches_and_removes_addons() {
        let root = tmp("manage");
        let src = root.join("src/okayama");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join(crate::manifest::MANIFEST_FILE),
            r#"{"schema_version":1,"id":"t.okayama","name":"Okayama","version":"1.2.0","author":"k",
                "compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        let saves = root.join("data/addons/t.saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(
            saves.join(crate::manifest::MANIFEST_FILE),
            r#"{"schema_version":1,"id":"t.saves","name":"Saves","version":"1.0.0","author":"t",
                "compat":{"game":"f1_25"},"saves":{"affects":true}}"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![fake("t.a", "a", true)]);
        let p = store.profile("default").unwrap();
        let host = Host::new(&store, "t.a", &p, None).unwrap();
        let cx = host.cx();
        let addons = cx.addons().unwrap();
        let info = addons.install(&src).unwrap();
        assert_eq!(
            (info.id.as_str(), info.version.as_str(), info.enabled),
            ("t.okayama", "1.2.0", false)
        );
        assert_eq!(info.dir, store.addons_dir().join("t.okayama"));
        addons.set_enabled("t.okayama", true).unwrap();
        let order = || -> Vec<String> {
            let p = store.profile("default").unwrap();
            p.addons.iter().map(|a| a.id.clone()).collect()
        };
        assert_eq!(order(), [CORE_ID, "t.a", "t.okayama"]);
        addons.move_addon("t.okayama", -1).unwrap();
        assert_eq!(order(), [CORE_ID, "t.okayama", "t.a"]);
        addons.move_addon("t.okayama", -5).unwrap();
        assert_eq!(order(), [CORE_ID, "t.okayama", "t.a"]);
        assert!(addons.move_addon(CORE_ID, 1).is_err());
        assert!(addons.move_addon("t.missing", 1).is_err());
        let listed = addons.list().unwrap();
        let okayama = listed.iter().find(|a| a.id == "t.okayama").unwrap();
        assert!(okayama.enabled && okayama.dir.join(crate::manifest::MANIFEST_FILE).is_file());
        let ids = |l: &[AddonInfo]| -> Vec<(String, Option<usize>)> {
            l.iter().map(|a| (a.id.clone(), a.position)).collect()
        };
        let at = |id: &str, n: Option<usize>| (id.to_owned(), n);
        assert_eq!(
            ids(&listed),
            [
                at(CORE_ID, Some(0)),
                at("t.okayama", Some(1)),
                at("t.a", Some(2)),
                at("t.saves", None)
            ]
        );
        assert!(listed.iter().any(|a| a.id == CORE_ID && a.enabled));
        addons.set_enabled("t.okayama", false).unwrap();
        assert_eq!(
            ids(&addons.list().unwrap()),
            [
                at(CORE_ID, Some(0)),
                at("t.a", Some(2)),
                at("t.okayama", Some(1)),
                at("t.saves", None)
            ]
        );
        addons.set_enabled("t.okayama", true).unwrap();
        assert!(addons.set_enabled("t.missing", true).is_err());
        addons.set_enabled("t.saves", true).unwrap();
        let e = addons.set_enabled("t.saves", false).unwrap_err();
        assert!(e.to_string().contains("Addons page"), "{e}");
        assert!(addons.remove("t.saves").is_err());
        addons.remove("t.okayama").unwrap();
        assert!(!store.addons_dir().join("t.okayama").exists());
        assert!(
            !store
                .profile("default")
                .unwrap()
                .enabled()
                .any(|id| id == "t.okayama")
        );
        assert!(addons.remove(CORE_ID).is_err());
        let vanilla = Host::new(&store, "t.a", &Profile::vanilla(), None).unwrap();
        assert!(
            vanilla
                .cx()
                .addons()
                .unwrap()
                .set_enabled("t.saves", true)
                .is_err()
        );
        std::fs::remove_dir_all(&root).ok();
    }

    fn copy(root: &Path, ea: bool) -> GameDir {
        let dir = root.join("game");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), b"exe").unwrap();
        match ea {
            true => {
                std::fs::create_dir_all(dir.join("__Installer")).unwrap();
                std::fs::write(dir.join("__Installer/installerdata.xml"), b"<game/>").unwrap();
            }
            false => std::fs::write(dir.join("EAJavelinInstaller_installscript.vdf"), b"").unwrap(),
        }
        let pfx = root.join("pfx");
        std::fs::create_dir_all(pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile"))
            .unwrap();
        std::fs::write(pfx.join("user.reg"), b"").unwrap();
        let store = Store::new(root);
        let mut config = crate::session::Config::default();
        config.games.active = Some(dir.clone());
        config.games.added = vec![dir.clone()];
        config.launch.ea_prefix = Some(pfx);
        store.save_config(&config).unwrap();
        let profiles = root.join("profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        std::fs::write(profiles.join("hi.json"), r#"{"name":"hi","launcher_saves":true}"#).unwrap();
        let game = Game {
            dir,
            user_reg: None,
            identity: crate::baseline::Identity {
                build_id: "1".into(),
                exe_sha256: "0".repeat(64),
            },
        };
        GameDir::new(&store, &game).unwrap()
    }

    #[test]
    fn the_save_dir_is_the_launcher_store_with_launcher_saves_on() {
        let root = tmp("save-dir-steam");
        let game = copy(&root, false);
        assert_eq!(
            game.save_dir("hi"),
            None,
            "the store isn't made until the first Play"
        );
        let store = crate::launcher_saves::remote_dir(&root, "hi");
        std::fs::create_dir_all(&store).unwrap();
        assert_eq!(game.save_dir("hi"), Some(store));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_save_dir_of_an_ea_copy_is_its_own_save_per_launcher_profile() {
        if cfg!(windows) {
            return;
        }
        let root = tmp("save-dir-ea");
        let game = copy(&root, true);
        std::fs::create_dir_all(crate::launcher_saves::ea_dir(&root, "hi")).unwrap();
        let ea = root.join("pfx/drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
        assert_eq!(
            game.save_dir("hi").as_deref(),
            Some(crate::launcher_saves::ea_dir(&root, "hi").as_path())
        );
        assert_eq!(
            game.save_dir(crate::profile::VANILLA).as_deref(),
            Some(ea.as_path())
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
