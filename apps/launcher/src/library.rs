use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Instant;
use swofty_i18n::{tr, trn};

use gpui_kit::{Image, ImageFormat};
use swofty_core::asset::AssetOp;
use swofty_core::folders::{Entry, Folders, Key};
use swofty_core::manifest::{AddonManifest, Category, Kind};
use swofty_core::ops::ErpOp;
use swofty_core::profile::{Profile, VANILLA};
use swofty_core::registry::Registry;
use swofty_core::settings::OnAddons;
use swofty_core::store::{Store, Unreadable};
use swofty_core::view::SectionSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Type {
    Database,
    Archives,
    Text,
    Simplified,
    Files,
    Code,
}

impl Type {
    pub(crate) const ALL: [Type; 6] = [
        Self::Database,
        Self::Archives,
        Self::Text,
        Self::Simplified,
        Self::Files,
        Self::Code,
    ];

    pub(crate) fn of(kind: Kind) -> Self {
        match kind {
            Kind::Database => Self::Database,
            Kind::Erp => Self::Archives,
            Kind::Text | Kind::Ui => Self::Text,
            Kind::Simplified => Self::Simplified,
            Kind::Files => Self::Files,
            Kind::Native => Self::Code,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Database => tr("Database"),
            Self::Archives => tr("Archives"),
            Self::Text => tr("Text & UI"),
            Self::Simplified => tr("Simplified"),
            Self::Files => tr("Files"),
            Self::Code => tr("Code"),
        }
    }

    /// An addon's types in [`Type::ALL`] order, as "Database, Archives".
    pub(crate) fn list(m: &AddonItem) -> String {
        let types: BTreeSet<Type> = m.manifest.kind.iter().map(|k| Self::of(*k)).collect();
        types.into_iter().map(Self::label).collect::<Vec<_>>().join(", ")
    }
}

pub struct AddonItem {
    pub manifest: AddonManifest,
    pub enabled: bool,
    pub locked: bool,
    pub icon: Option<AddonIcon>,
    /// Only tracks (see [`swofty_core::tracks::is_track_addon`]): listed on an
    /// addon's Tracks page while the sidebar shows one.
    pub track: bool,
    /// Built from the DB Editor (see [`swofty_core::dbedit::package::is_package`]).
    pub package: bool,
    pub source: Source,
    /// Why builds leave out each track package it adds that can't be used.
    pub left_out: Vec<String>,
}

impl AddonItem {
    /// The manifest's category; a track addon without one is under Tracks.
    pub fn category(&self) -> Option<Category> {
        self.manifest.category.or(self.track.then_some(Category::Tracks))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    Builtin,
    DbEditor,
    Importer,
    Installed,
}

impl Source {
    pub const ALL: [Source; 4] = [Self::Builtin, Self::DbEditor, Self::Importer, Self::Installed];

    pub fn label(self) -> &'static str {
        match self {
            Self::Builtin => tr("Built in"),
            Self::DbEditor => tr("DB Editor"),
            Self::Importer => tr("Imported"),
            Self::Installed => tr("Installed"),
        }
    }

    fn of(store: &Store, m: &AddonManifest, package: bool) -> Self {
        let tool = m.origin.as_ref().map(|o| o.tool.as_str());
        if store.is_builtin(&m.id) || m.id == swofty_core::managed::ID {
            Self::Builtin
        } else if package || tool == Some(swofty_core::dbedit::package::ORIGIN_TOOL) {
            Self::DbEditor
        } else if tool == Some(swofty_core::managed::ID) {
            Self::Importer
        } else {
            Self::Installed
        }
    }
}

/// What an addon's op files change, counted by kind.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    pub database: usize,
    pub resources: usize,
    pub text: usize,
    pub files: usize,
    pub tracks: usize,
    pub screens: usize,
    pub plugins: usize,
    pub unreadable: Vec<String>,
}

impl Changes {
    /// Reads the op files as a build does, with the profile's settings for the addon's options.
    pub fn read(store: &Store, profile: &Profile, on: &OnAddons, item: &AddonItem) -> Self {
        let m = &item.manifest;
        let dir = store.addons_dir().join(&m.id);
        let mut c = Self {
            screens: m.operations.ui.len() + m.operations.screens.len(),
            plugins: m.operations.native.len(),
            ..Self::default()
        };
        let ops = m.operations.op_files().flat_map(|(kind, rel)| {
            let file = swofty_core::build::read_op_file(&dir.join(rel), kind, m, profile, on).ok();
            if file.is_none() {
                c.unreadable.push(rel.to_owned());
            }
            file.map(|f| f.ops).unwrap_or_default()
        });
        let ops: Vec<AssetOp> = ops.chain(m.operations.file_ops()).collect();
        for op in &ops {
            match op {
                AssetOp::Db(_) => c.database += 1,
                AssetOp::Erp(ErpOp::Overlay { erps, source }) => {
                    let names = source
                        .resolve(&swofty_core::build::source_dir(store, &m.id, source.root))
                        .ok()
                        .and_then(|path| swofty_core::erpbuild::overlay_names(&path).ok());
                    c.resources += names.map_or(1, |n| n.len() * erps.len().max(1));
                }
                AssetOp::Erp(_) => c.resources += 1,
                AssetOp::Lang(_) => c.text += 1,
                AssetOp::File(_) => c.files += 1,
                AssetOp::Track(_) => c.tracks += 1,
                AssetOp::Group(_) | AssetOp::Item(_) | AssetOp::Warn { .. } => {}
            }
        }
        if item.track && c.tracks == 0 {
            c.tracks = 1;
        }
        c
    }

    /// Each non-zero count as "3 database values", in a fixed order.
    pub fn parts(&self) -> Vec<String> {
        [
            (self.database, "{count} database value", "{count} database values"),
            (self.resources, "{count} game asset", "{count} game assets"),
            (self.text, "{count} text entry", "{count} text entries"),
            (self.files, "{count} file", "{count} files"),
            (self.tracks, "{count} track", "{count} tracks"),
            (self.screens, "{count} UI screen", "{count} UI screens"),
            (self.plugins, "{count} code plugin", "{count} code plugins"),
        ]
        .into_iter()
        .filter(|(n, ..)| *n > 0)
        .map(|(n, one, many)| trn(n, one, many))
        .collect()
    }
}

#[derive(Clone)]
pub enum AddonIcon {
    Glyph(Arc<[u8]>),
    Image(Arc<Image>),
}

fn addon_icon(store: &Store, registry: &Registry, m: &AddonManifest) -> Option<AddonIcon> {
    let (bytes, format) = match registry.get(&m.id).and_then(|e| e.addon.icon()) {
        Some(bytes) if bytes.starts_with(b"\x89PNG") => (bytes.to_vec(), ImageFormat::Png),
        Some(bytes) => (bytes.to_vec(), ImageFormat::Svg),
        None => {
            let path = store.addons_dir().join(&m.id).join(m.icon.as_ref()?);
            let format = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
                "svg" => ImageFormat::Svg,
                "png" => ImageFormat::Png,
                "jpg" | "jpeg" => ImageFormat::Jpeg,
                "webp" => ImageFormat::Webp,
                _ => return None,
            };
            (std::fs::read(path).ok()?, format)
        }
    };
    Some(match format {
        ImageFormat::Svg => AddonIcon::Glyph(bytes.into()),
        _ => AddonIcon::Image(Arc::new(Image::from_bytes(format, bytes))),
    })
}

pub const TRACKS_SECTION: &str = "tracks";
const IMPORTER_SECTION: &str = "mods";

pub struct SectionItem {
    pub addon: String,
    pub spec: SectionSpec,
    /// `sections/<id>.svg` in the addon's folder, shown in the sidebar in place of the addon's icon.
    pub icon: Option<AddonIcon>,
}

impl SectionItem {
    /// The Simplified Importer's old Tracks page, which the sidebar leaves out.
    pub fn is_importer_tracks(&self) -> bool {
        self.addon == swofty_core::managed::ID && self.spec.id == TRACKS_SECTION
    }

    /// The Simplified Importer's own page.
    pub fn is_importer(&self) -> bool {
        self.addon == swofty_core::managed::ID && self.spec.id == IMPORTER_SECTION
    }

    /// A page of Simplified for Swofty, which the sidebar lists under its own heading.
    pub fn is_simplified(&self) -> bool {
        self.addon == swofty_core::managed::ID
    }
}

fn tracks_page(sections: &[SectionItem]) -> Option<usize> {
    sections
        .iter()
        .position(|s| s.spec.id == TRACKS_SECTION && !s.is_importer_tracks())
}

fn place(folders: &Folders, a: &AddonItem) -> Option<Key> {
    folders.place(&a.manifest.id, a.source == Source::Builtin, a.category())
}

fn listed(addons: &[AddonItem], tracks_page: Option<usize>) -> impl Iterator<Item = &AddonItem> {
    addons.iter().filter(move |m| !(m.track && tracks_page.is_some()))
}

fn section_icon(store: &Store, addon: &str, section: &str) -> Option<AddonIcon> {
    let path = store
        .addons_dir()
        .join(addon)
        .join("sections")
        .join(format!("{section}.svg"));
    Some(AddonIcon::Glyph(std::fs::read(path).ok()?.into()))
}

pub struct ProfileItem {
    pub name: String,
    pub addons: Vec<String>,
}

impl ProfileItem {
    pub fn vanilla(&self) -> bool {
        self.name == VANILLA
    }
}

pub struct Library {
    pub profile: Profile,
    pub rebuild: Vec<String>,
    pub addons: Vec<AddonItem>,
    pub profiles: Vec<ProfileItem>,
    pub sections: Vec<SectionItem>,
    pub registry: Registry,
    /// The section with id `tracks` the sidebar shows, which lists track
    /// packages and track addons together.
    pub tracks_page: Option<usize>,
    pub error: Option<String>,
    /// Installed addons this launcher can't read, with why.
    pub broken: Vec<Unreadable>,
    pub folders: Folders,
    /// The profile's addons in load order, then the installed ones it doesn't
    /// list, grouped into folders; folders of the player's own that hold none
    /// come last.
    pub entries: Vec<Entry>,
    pub loaded: Instant,
    unsaved_order: bool,
}

pub(crate) fn left_out_limit_line() -> String {
    swofty_i18n::trf!(
        "Left out: the game holds {room} tracks",
        room = swofty_core::tracks::TRACK_LIST_ROOM
    )
}

impl Library {
    pub fn load(store: &Store) -> Self {
        let mut library = Self::read(store);
        library.save_order(store);
        library
    }

    pub fn read(store: &Store) -> Self {
        let mut error = None;
        let mut note = |e: swofty_core::Error| error = Some(e.to_string());
        let profile = store.profile(&store.active_profile()).unwrap_or_else(|e| {
            note(e);
            Profile::vanilla()
        });
        let registry = store.registry().unwrap_or_else(|e| {
            note(e);
            Registry::default()
        });
        let manifests = registry.manifests();
        let enabled: std::collections::HashSet<String> = profile.enabled().map(str::to_owned).collect();
        let over_limit = swofty_core::tracks::recorded_track_limit(&store.root, &profile.name);
        let mut addons: Vec<AddonItem> = manifests
            .values()
            .map(|m| {
                let package = !m.is_builtin() && swofty_core::dbedit::package::is_package(store, &m.id);
                let source = Source::of(store, m, package);
                AddonItem {
                    enabled: enabled.contains(&m.id),
                    locked: registry.get(&m.id).is_some_and(|e| e.locked),
                    icon: addon_icon(store, &registry, m),
                    track: !m.is_builtin()
                        && swofty_core::tracks::is_track_addon(&store.addons_dir().join(&m.id), m),
                    package,
                    source,
                    left_out: match m.is_builtin() {
                        true => Vec::new(),
                        false => {
                            let mut lines: Vec<String> = swofty_core::tracks::unusable(store, &m.id, m)
                                .iter()
                                .map(|u| format!("{}.", crate::update::sentence(&u.text())))
                                .collect();
                            if enabled.contains(&m.id) && over_limit.contains(&m.id) {
                                lines.push(left_out_limit_line());
                            }
                            lines
                        }
                    },
                    manifest: m.clone(),
                }
            })
            .collect();
        addons.sort_by_cached_key(|m| {
            let declared = registry.get(&m.manifest.id).is_none_or(|e| e.declared);
            (!m.manifest.is_builtin(), declared, m.manifest.name.to_lowercase())
        });
        let folders = store.folders();
        let mut profile = profile;
        let places: HashMap<&str, Option<Key>> = addons
            .iter()
            .map(|a| (a.manifest.id.as_str(), place(&folders, a)))
            .collect();
        let place_of = |id: &str| {
            places
                .get(id)
                .cloned()
                .unwrap_or_else(|| folders.place(id, false, None))
        };
        let unsaved_order = swofty_core::folders::arrange(&mut profile, place_of);
        let listed: std::collections::HashSet<&str> = profile.addons.iter().map(|a| a.id.as_str()).collect();
        let order = profile.addons.iter().map(|a| a.id.clone()).chain(
            addons
                .iter()
                .filter(|a| !listed.contains(a.manifest.id.as_str()))
                .map(|a| a.manifest.id.clone()),
        );
        let mut entries = swofty_core::folders::group(order, place_of);
        for f in &folders.folders {
            let key = Key::Own(f.id.clone());
            if !entries.iter().any(|e| e.folder() == Some(&key)) {
                entries.push(Entry::Folder(key, Vec::new()));
            }
        }
        let profiles = store
            .profile_names()
            .unwrap_or_default()
            .into_iter()
            .map(|name| {
                let p = store.profile(&name).unwrap_or_else(|_| Profile::new(&name));
                ProfileItem {
                    addons: p
                        .enabled()
                        .filter_map(|id| manifests.get(id))
                        .map(|m| m.name.clone())
                        .collect(),
                    name,
                }
            })
            .collect();
        let sections: Vec<SectionItem> = registry
            .iter()
            .filter(|e| !e.declared && enabled.contains(e.id()))
            .flat_map(|e| {
                e.addon.sections().into_iter().map(|spec| SectionItem {
                    addon: e.id().to_owned(),
                    icon: section_icon(store, e.id(), &spec.id),
                    spec,
                })
            })
            .collect();
        let tracks_page = tracks_page(&sections);
        Self {
            tracks_page,
            rebuild: profile.rebuild_pending(&manifests),
            profile,
            addons,
            profiles,
            sections,
            registry,
            error,
            broken: store.unreadable_addons(),
            folders,
            entries,
            loaded: Instant::now(),
            unsaved_order,
        }
    }

    pub fn save_order(&mut self, store: &Store) {
        if !std::mem::take(&mut self.unsaved_order) {
            return;
        }
        let place_of = |id: &str| self.place(id);
        match store.update_profile(&self.profile.name, |p| {
            swofty_core::folders::arrange(p, place_of);
            Ok(p.clone())
        }) {
            Ok(p) => self.profile = p,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn place(&self, id: &str) -> Option<Key> {
        match self.addons.iter().find(|a| a.manifest.id == id) {
            Some(a) => place(&self.folders, a),
            None => self.folders.place(id, false, None),
        }
    }

    /// The addons the Addons page lists: track addons are on the Tracks page
    /// while the sidebar shows one.
    /// The addons on in the profile, that options' `needs` and ops' `when.addon` are checked against.
    pub fn on(&self) -> OnAddons {
        OnAddons::new(self.addons.iter().filter(|a| a.enabled).map(|a| &a.manifest))
    }

    pub fn listed(&self) -> impl Iterator<Item = &AddonItem> {
        listed(&self.addons, self.tracks_page)
    }
}

#[cfg(test)]
mod tests {
    use swofty_core::manifest::{AddonManifest, Category};
    use swofty_core::profile::Profile;
    use swofty_core::store::Store;
    use swofty_core::view::{Icon, SectionSpec};

    use super::{AddonIcon, AddonItem, Changes, SectionItem, Source, listed, section_icon, tracks_page};

    fn addon(id: &str, track: bool) -> AddonItem {
        let manifest = AddonManifest::parse(&format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
        ))
        .unwrap();
        AddonItem {
            manifest,
            enabled: true,
            locked: false,
            icon: None,
            track,
            package: false,
            source: Source::Installed,
            left_out: Vec::new(),
        }
    }

    fn section(addon: &str, id: &str) -> SectionItem {
        SectionItem {
            addon: addon.into(),
            spec: SectionSpec {
                id: id.into(),
                title: id.into(),
                icon: Icon::Map,
            },
            icon: None,
        }
    }

    #[test]
    fn lists_track_addons_unless_the_sidebar_shows_a_tracks_page() {
        let addons = [addon("t.mod", false), addon("t.okayama", true)];
        let ids = |page| -> Vec<&str> { listed(&addons, page).map(|m| m.manifest.id.as_str()).collect() };
        let importer = [
            section(swofty_core::managed::ID, "mods"),
            section(swofty_core::managed::ID, "tracks"),
        ];
        assert_eq!(tracks_page(&importer), None);
        assert_eq!(ids(tracks_page(&importer)), ["t.mod", "t.okayama"]);
        let other = [
            section(swofty_core::managed::ID, "mods"),
            section("t.tracks", "tracks"),
        ];
        assert_eq!(tracks_page(&other), Some(1));
        assert_eq!(ids(tracks_page(&other)), ["t.mod"]);
        assert_eq!(addons[1].category(), Some(Category::Tracks));
        assert_eq!(addons[0].category(), None);
    }

    #[test]
    fn lists_every_installed_addon_but_not_a_track_on_the_tracks_page() {
        let addons = [addon("t.mine", false), addon("t.track", true)];
        let all: Vec<&str> = listed(&addons, None).map(|m| m.manifest.id.as_str()).collect();
        assert_eq!(all, ["t.mine", "t.track"]);
        let on_page: Vec<&str> = listed(&addons, Some(0)).map(|m| m.manifest.id.as_str()).collect();
        assert_eq!(on_page, ["t.mine"]);
    }

    #[test]
    fn a_section_icon_comes_from_the_addon_sections_folder() {
        let root = std::env::temp_dir().join(format!("swofty-launcher-section-icon-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let dir = store.addons_dir().join("t.mods").join("sections");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mods.svg"), "<svg/>").unwrap();
        assert!(matches!(
            section_icon(&store, "t.mods", "mods"),
            Some(AddonIcon::Glyph(d)) if &*d == b"<svg/>"
        ));
        assert!(section_icon(&store, "t.mods", "tracks").is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn changes_count_op_files_that_use_option_templating() {
        let root = std::env::temp_dir().join(format!("swofty-launcher-changes-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let dir = store.addons_dir().join("t.dnf").join("db");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("dnf.json"),
            r#"{"schema_version": 1, "ops": [
              {"op": "set", "select": "t.r", "field": "chance", "value": "{{chance}}"},
              {"op": "set", "select": "t.r", "field": "failures", "value": "1", "when": {"failures": true}}
            ]}"#,
        )
        .unwrap();
        let mut item = addon("t.dnf", false);
        item.manifest = AddonManifest::parse(
            r#"{"schema_version":1,"id":"t.dnf","name":"DNF","version":"1.0.0","author":"t","compat":{"game":"f1_25"},
              "options": [
                {"id": "failures", "type": "toggle", "default": false, "binds": "db"},
                {"id": "chance", "type": "slider", "min": 0, "max": 10, "step": 1, "default": 2, "binds": "db"}
              ],
              "operations": {"database": ["db/dnf.json", "db/missing.json"]}}"#,
        )
        .unwrap();
        let mut profile = Profile::new("t");
        let c = Changes::read(
            &store,
            &profile,
            &swofty_core::settings::OnAddons::default(),
            &item,
        );
        assert_eq!(
            (c.database, c.unreadable),
            (1, vec!["db/missing.json".to_owned()])
        );
        profile
            .set_option(&item.manifest, "failures", true.into())
            .unwrap();
        assert_eq!(
            Changes::read(
                &store,
                &profile,
                &swofty_core::settings::OnAddons::default(),
                &item
            )
            .database,
            2
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
