# Addons and mods

If you're contributing, this is the design for splitting what earlier versions called mods into addons and mods, along with the Launcher API, the Asset API and the built-in Team Simplified addon. Treat each part as unbuilt unless it explicitly says otherwise.

## 1. Terms and the rename

| Term | Meaning |
|---|---|
| Addon | A package with a `swofty.json` manifest: native plugins, declared content, settings and in-game screens. Everything earlier versions called a mod is an addon. The built-in addons are SwoftyMod (`swofty.core`) and Team Simplified (`teamsimplified.mods`). |
| Mod | A content package with no code: liveries, cars, text, shaders, database rows. An addon imports, stores and loads mods. Team Simplified handles `.serps` mods. |
| Track | A community track package. Tracks are mods in Team Simplified's library ([section 5](#_5-tracks-are-mods)). |
| Asset op | One change to game content: an asset group, an archive resource, a database record, a language string or a loose file. |
| Delivery | How an asset op reaches the game. A build delivery rewrites a game file and installs it through the journal. A runtime delivery has swofty_core apply the op in memory while the game boots. |

Your addon has two ways to get assets into the game:

1. Declare asset ops in its manifest. The build loads them at launch.
2. Produce asset ops through the Asset API, using a Rust build hook for built-in addons or `swofty::assets` calls from a native plugin at runtime.

You'll find mods only in the launcher sections their addons contribute; core has no Assets page.

### Renames

The renames and compatibility rules below are already built. In `owned_items` entries in `swofty/settings.json`, you'll also find the addon named under `addon` instead of `mod`.

| Area | Now | After |
|---|---|---|
| Core types | `ModManifest`, `ModOption` | `AddonManifest`, `AddonOption` |
| Store | `Store::mods()`, `mods_dir()`, `install_mod()` | `addons()`, `addons_dir()`, `install_addon()` |
| Profile | `ProfileMod`, `Profile.mods` | `ProfileAddon`, `Profile.addons` |
| Session settings | `SessionMod`, `SessionSettings.mods` | `SessionAddon`, `SessionSettings.addons` |
| Launcher | `ModItem`, `Library.mods`, `Shell.open_mod`, `Page::Mods`, `mods_page`, `mod_row`, `mod_body`, `install_mod` | `AddonItem`, `Library.addons`, `Shell.open_addon`, `Page::Addons`, `addons_page`, `addon_row`, `addon_body`, `install_addon` |
| CLI | `ModsCommand`, `swofty mods list/install/enable/disable` | `AddonsCommand`, `swofty addons list/install/enable/disable` |
| SDK | `SwoftyModInit`, `SwoftyModInitFn`, `INIT_SYMBOL`, `SWOFTY_MOD()` | `SwoftyAddonInit`, `SwoftyAddonInitFn`, `INIT_SYMBOLS`, `SWOFTY_ADDON()` |
| Native core | `ModSettings`, `mods[]` in `manager.cpp`, `settings.cpp`, `ui_vm.cpp` | `AddonSettings`, `addons[]` |
| Repository | `mods/swoftymod`, `mods/custom-grid-size`, `templates/mod` | `addons/swoftymod`, `templates/addon`; Custom Grid Size is its own repository |
| Data dir | `$DATA/mods/<id>/` | `$DATA/addons/<id>/` |
| Data dir (new) | | `$DATA/addon-data/<id>/` for files an addon owns, `$DATA/cache/addons/<id>/` for its caches |
| Profile file | `mods` | `addons`, plus a `state` object keyed by addon id |
| Game folder | `swofty/mods/core/`, `swofty/mods/*.dll` | `swofty/addons/core/`, `swofty/addons/*.dll` |
| `swofty/settings.json` | `mods` | `addons` |
| Screen names | `swofty_mod_<n>` | `swofty_addon_<n>` |
| In game | `MOD SETTINGS` | `ADDON SETTINGS` |
| Launcher text | `Mods` (sidebar), `Install mod…`, `A mod folder that contains swofty.json.`, `<n> mods installed`, `MODS` and `No mods` on profile cards | `Addons`, `Install addon…`, `An addon folder that contains swofty.json.`, `<n> addons installed`, `ADDONS` and `No addons` |
| CLI text | `Swofty Launcher for F1 25: mods and maps`, `mod settings` | `Swofty Launcher for F1 25: addons, mods and tracks`, `addon settings` |
| Manifest errors | `must be a relative path inside the mod` | `must be a relative path inside the addon` |
| SwoftyMod description | `mod loading` | `addon loading` |
| Docs titles | `Mods and maps`, `Making a mod`, `Configuring mods (swofty.json)`, `In-game mod settings`, `Mod manifest` | `Addons and tracks` (`Addons, mods and tracks` with the Mods section), `Making an addon`, `Configuring addons (swofty.json)`, `In-game addon settings`, `Addon manifest` |

These stay unchanged: `swofty.json` and every field in it, manifest `schema_version` 1, the id `swofty.core`, the name SwoftyMod, the `swofty_set:` and `swofty_enable:` commands, the requests file, `$DATA/maps/`, the app id `net.swofty.ModLauncher`, and doc page paths.

### Compatibility and migration

- **Manifests.** Your existing manifests load unchanged. You can add the new optional `operations.assets` field; `operations.database`, `operations.erp` and `operations.files` remain valid and load into the asset op model.
- **Data dir.** `Store::migrate()` runs when the launcher starts and when a `swofty` command parses. `--help`, `--version` and parse errors exit before it:
  1. If `mods/` exists and `addons/` does not, `mods/` is renamed to `addons/`.
  2. If both exist, each `mods/<x>/` whose manifest id is not in `addons/` moves there. The rest stay and are logged. `mods/` is removed when empty.
  3. `config.json` gets `"data_version": 2`.
- **Profiles.** `Profile.addons` also reads `mods`, so the old field still loads. Your next save writes `addons` and `schema_version: 2`, with option values still keyed by addon id.
- **Game folder.** You don't need to migrate the game folder. The loader also reads `swofty/mods/` until the next minor release.
- **Plugins.** The loader calls `SwoftyAddonInit`, or `SwoftyModInit` when that is the only export. `SWOFTY_MOD()` stays as an alias of `SWOFTY_ADDON()`. Built plugins keep working.
- **Settings file.** swofty_core reads `addons`, or `mods` when `addons` is absent.
- **CLI.** `swofty mods` stays a hidden alias of `swofty addons` until Team Simplified provides `swofty mods`. From then until the next minor release, `swofty mods install <dir>` for a folder with `swofty.json`, and `swofty mods enable|disable <id>` for an addon id, run the `swofty addons` command and print a note.
- Once you've migrated the data, older launchers cannot read it.

## 2. Launcher API

The API lives in a new crate, `packages/addon-api` (`swofty-addon`). `manifest` and `ops` move there from core, which re-exports them. Core depends on `swofty-addon` to run addons; when you write a built-in addon, you depend on `swofty-addon` and `swofty-formats`, never on `swofty-core`. The apps register built-ins through `addons/builtins` (`swofty-builtins`), so core names no addon.

You can already use the types below, the registry, `Profile.state`, the `addon-data` and cache folders, and the build hooks. They're built in `swofty_addon::launcher` and `swofty_addon::view`, core's `registry` module, and `Store::with_builtins`. The launcher renders sections and profile card summaries, runs events and importers on the background executor, and routes dropped files through `swofty_core::import`. The CLI adds addon commands and forwards the old `swofty mods` forms.

```rust
pub trait Addon: Send + Sync {
    fn manifest(&self) -> &AddonManifest;
    fn sections(&self) -> Vec<SectionSpec> { Vec::new() }
    fn view(&self, cx: &Cx, section: &str) -> Result<View> { Err(Error::NoSection(section.into())) }
    fn event(&self, cx: &Cx, section: &str, event: Event) -> Result<Effect> { Ok(Effect::None) }
    fn importer(&self) -> Option<&dyn Importer> { None }
    fn build(&self, cx: &BuildCx) -> Result<AssetBatch> { Ok(AssetBatch::default()) }
    fn commands(&self) -> Vec<CommandSpec> { Vec::new() }
    fn command(&self, cx: &Cx, path: &[String], args: &[String]) -> Result<String> { Err(Error::NoCommand(path.join(" "))) }
    fn summary(&self, cx: &Cx) -> Option<Summary> { None }
}

pub trait Importer: Send + Sync {
    fn claims(&self, path: &Path) -> Claim;
    fn import(&self, cx: &Cx, paths: &[PathBuf], progress: &dyn Progress) -> Result<Vec<String>>;
}

pub enum Claim { No, Maybe, Yes }

pub trait Progress: Send + Sync {
    fn report(&self, done: f32, text: &str);
}

pub struct Cx<'a> {
    pub addon: &'a str,
    pub profile: &'a ProfileInfo,
    pub data_dir: &'a Path,
    pub cache_dir: &'a Path,
    pub game: Option<&'a dyn GameFiles>,
    state: &'a dyn StateStore,
}

impl Cx<'_> {
    pub fn state<T: DeserializeOwned + Default>(&self) -> Result<T>;
    pub fn set_state<T: Serialize>(&self, value: &T) -> Result<()>;
}

pub struct ProfileInfo {
    pub name: String,
    pub vanilla: bool,
    pub enabled: bool,
}

pub trait GameFiles: Send + Sync {
    fn build_key(&self) -> String;
    fn exe_version(&self) -> Option<String>;
    fn is_stock(&self, rel: &str) -> bool;
    fn original(&self, rel: &str) -> Result<PathBuf>;
    fn original_lang(&self, code: &str) -> Result<PathBuf>;
}

pub struct BuildCx<'a> {
    pub cx: Cx<'a>,
    pub options: &'a BTreeMap<String, serde_json::Value>,
    pub features: &'a Features,
}

pub struct Summary {
    pub label: String,
    pub items: Vec<String>,
}

pub struct CommandSpec {
    pub name: String,
    pub about: String,
    pub args: Vec<ArgSpec>,
    pub subcommands: Vec<CommandSpec>,
}

pub struct ArgSpec {
    pub name: String,
    pub help: String,
    pub required: bool,
    pub many: bool,
}
```

Use `GameFiles` when you need clean files: it gives you baseline objects, game files the baseline has not recorded, and stock language files ([section 3.6](#_3-6-language-baseline)). `is_stock` tells you whether a path ships with the game.

Every addon goes in core's registry. Installed addons and SwoftyMod are `DeclaredAddon`s: a manifest and a folder, using the default methods. Built-in entries carry `locked` and `default_on`. These keep SwoftyMod always on and Team Simplified on in profiles that don't list it: `Store::profile` enables an unlisted `default_on` addon and always enables a `locked` one.

### Sections and pages

```rust
pub struct SectionSpec {
    pub id: String,
    pub title: String,
    pub icon: Icon,
}

pub struct View {
    pub subtitle: String,
    pub actions: Vec<Button>,
    pub notices: Vec<Notice>,
    pub body: Body,
    pub footer: Option<String>,
}

pub enum Body {
    List { rows: Vec<Row>, reorder: bool },
    Empty { title: String, text: String, action: Option<Button> },
    Blocks(Vec<Block>),
}

pub struct Row {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub art: Art,
    pub tags: Vec<Tag>,
    pub trailing: Option<Control>,
    pub detail: Vec<Block>,
}

pub enum Block {
    Stats(Vec<Fact>),
    Facts(Vec<Fact>),
    Label(String),
    Text { text: String, tone: Tone },
    Names { label: String, items: Vec<String> },
    Table { columns: Vec<String>, rows: Vec<Vec<String>> },
    Buttons(Vec<Button>),
}

pub enum Control {
    Switch { id: String, on: bool, enabled: bool },
    Choice { id: String, options: Vec<String>, current: usize },
    Button(Button),
}

pub struct Button {
    pub id: String,
    pub label: String,
    pub icon: Option<Icon>,
    pub style: Style,
    pub kind: ButtonKind,
}

pub enum ButtonKind {
    Event,
    Confirm { title: String, text: String },
    Pick { title: String, extensions: Vec<String>, folders: bool, multiple: bool },
}

pub enum Event {
    Click { id: String },
    Toggle { id: String, on: bool },
    Choose { id: String, index: usize },
    Move { row: String, to: usize },
    Picked { id: String, paths: Vec<PathBuf> },
}

pub enum Effect {
    None,
    Reload,
    Notice(String),
    SetAddonEnabled(bool),
    Open(PathBuf),
}
```

`Fact`, `Tag`, `Notice`, `Tone`, `Style`, `Art` and `Icon` are small value types: label and value, text and tone, a fixed icon set, a monogram or an image path. Every type is serde-serialisable.

- Each section gives you one page and one sidebar entry. The sidebar lists Profiles, Addons, a divider, then addon sections in registry order. If your section's body is a list, its entry shows the row count.
- `footer` is a muted line under the body. `Effect::Open` opens a file or folder with the system's handler.
- Return a `View` and the shell renders it with the same components as core pages: page header, list, row, switch, choice button, stats, facts, notices and empty state. Put a row's expanded body in `detail`. A `reorder` list gets a drag handle and Move up and Move down entries.
- The shell keeps your section's `View` until `Effect::Reload`, a profile switch or a store reload makes it ask again.
- Keep `view` to in-memory reads: it runs on the UI thread. `event`, `import`, `command` and `build` run on the background executor, and the control that sent an event stays busy until its `Effect` returns.
- Turn an addon off in the active profile and its sections disappear.

### Import handlers

- Use `ButtonKind::Pick` to open the system file picker. Your addon receives the selection as `Event::Picked`.
- When you drop a file on the window, it goes to the importer whose `claims` returns `Yes`, or, failing that, the only one returning `Maybe`. Otherwise the shell names the file and says no addon imports it. Core gets asked first: it imports addon folders (`swofty.json`), `.swoftyaddon` files (and legacy `.swoftymod` files), and zips containing an addon. Team Simplified claims track zips ([section 5](#_5-tracks-are-mods)).
- `Progress` reports appear as a notification with a progress bar.

### Per-profile state

- Your addon owns the JSON value in `Profile.state[<addon id>]`; core doesn't read inside it.
- Read it with `Cx::state`, which returns `T::default()` if nothing is stored. Write it to the active profile with `Cx::set_state`. Writes to the vanilla profile are refused.
- When you duplicate or rename a profile, its state is copied too.

### Build hooks

- The build visits enabled addons in resolver order. For each addon, it passes the manifest's declared ops to the asset planner, followed by the ops your `build` returns ([section 3](#_3-asset-api)).
- The build hash covers every op, plus the size and modification time of its source files. For a folder source, that includes every file inside it.
- Read `BuildCx.options` for your addon's option values in this profile.
- `BuildCx.features` holds the features from the loader's `features.json`. Hook ops go through the planner after the addon's declared ops, with the origin the hook gives them.
- If your op file or hook result uses an unsupported `patch-pkg` edit (anything but `clone-entry` and `set-value`), the build skips all its ERP ops and warns.

### Commands and profile cards

- Each `CommandSpec` becomes a top-level CLI subcommand, with its arguments passed to `command`. Choose a name that core and earlier addons haven't taken, or registration refuses it.
- Use `summary` to add a list after `ADDONS` on the profile cards.

### Third-party addons {#third-party-addons}

To give an installed addon pages, an importer, a build hook, commands and a profile summary, ship a helper program. This is already built. Core runs it as `HelperAddon` (`swofty_core::helper`), an `Addon` backed by a process. Each installed addon with a `launcher` block gets one `HelperAddon` in the registry, marked `helper`. You'll find the protocol types in `swofty_addon::helper` and a Rust example in [`templates/launcher-addon`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/launcher-addon).

Declare your helper and what it provides in the manifest:

```json
"launcher": {
  "program": { "windows": "bin/example-helper.exe", "linux": "bin/example-helper" },
  "sections": [{ "id": "items", "title": "Example", "icon": "package" }],
  "imports": ["exmod"],
  "build": true,
  "commands": [{ "name": "example", "about": "List the example items" }]
}
```

| Field | Meaning |
|---|---|
| `program` | The helper's path inside the addon folder, keyed by `std::env::consts::OS` (`windows`, `linux`) |
| `sections` | `SectionSpec`s. The sidebar lists them without starting the helper. |
| `imports` | File extensions without the dot. A file with one of them gets `Claim::Yes`. |
| `build` | `true` when the helper has a build hook |
| `commands` | `CommandSpec`s added to the CLI |

**Start.** Your helper starts when the host first needs it: you open one of its sections, drop a file routed to it, build a profile that enables it, or run one of its commands. It runs from the addon folder, with stderr written to `$DATA/cache/addons/<id>/helper.log`. The launcher requests a section's view only while that section is open, and a summary only while the helper is running with no call in flight. Closing stdin ends the helper.

**Messages.** Exchange messages over the helper's stdin and stdout, each as one UTF-8 JSON object on one line. Requests carry `id`, `method` and `params`; replies carry the request's `id` and either `result` or `error` (a string). Leave `id` out of notifications. The host sends one request at a time and waits for its reply.

```text
host   {"id":0,"method":"hello","params":{"api":1,"addon":"example.launcher-addon","data_dir":"…","cache_dir":"…","profile":{"name":"default","vanilla":false,"enabled":true}}}
helper {"id":0,"result":{"api":1}}
host   {"id":1,"method":"view","params":{"section":"items"}}
helper {"id":0,"method":"get_state"}
host   {"id":0,"result":{"on":["a.exmod"]}}
helper {"id":1,"result":{"subtitle":"1 item","body":{"list":{"rows":[…],"reorder":false}}}}
```

Host to helper:

| Method | `params` | `result` |
|---|---|---|
| `hello` | `api`, `addon`, `data_dir`, `cache_dir`, `profile` (`ProfileInfo`) | `{"api": <n>}` |
| `profile` (notification) | `ProfileInfo` | |
| `view` | `section` | `View` |
| `event` | `section`, `event` (`Event`) | `Effect` |
| `import` | `paths` | lines to show, as strings |
| `build` | `options`, `features` (`Features`) | `{"ops": [asset op], "lines": [], "warnings": []}` |
| `command` | `path`, `args` | text to print |
| `summary` | none | `Summary` or `null` |

Helper to host, while a request is in flight:

| Method | `params` | `result` |
|---|---|---|
| `progress` (notification) | `done` (0 to 1), `text` | |
| `get_state` | none | the profile's state for the addon, or `null` |
| `set_state` | the new state | `null`. An error on the vanilla profile. |

- Values use the serde forms of the Launcher API and Asset API types: enums are externally tagged in snake_case (`{"toggle":{"id":"a","on":true}}`, `"reload"`), `Icon` is kebab-case, and asset ops are written as in an ops file. Build ops get the origin `<id>`, and their sources resolve against the addon's `addon`, `data` and `cache` roots.
- `api` is `swofty_addon::helper::API_VERSION`, 1. The host stops a helper that answers `hello` with another version.
- The host sends `profile` before a request when the active profile changed since the last one.
- Reply with an `error` if your helper doesn't know the method.

**Failures.** Returning a helper error fails only that call. The host stops your helper and marks it failed if it cannot start or misses a reply deadline: 5 seconds for `hello`, `view` and `summary`, or 300 seconds for other calls. Each message resets the wait. Exiting, closing stdout, writing a line that isn't a message, replying to another `id` or returning a result of the wrong shape also fails the helper.

Its sections then show the error in a notice with a Restart button (`swofty.restart`). Restart clears the failure so the next call can start the helper again; other calls to a failed helper return the error. Reinstalling the addon stops its helper.

## 3. Asset API

### 3.1 Ops

| Op | Fields | Change |
|---|---|---|
| `group.register` | `group`, `erp` or `erps`, optional `deps`, `template` | Add an asset group to the engine's manifest |
| `erp.replace-resource`, `erp.add-resource`, `erp.copy-resource`, `erp.remove-resource`, `erp.add-mipmaps`, `erp.patch-pkg` | As in [archive edits](/reference/ops/erp) | Unchanged. All six are built (`packages/core/src/erpbuild.rs`) |
| `erp.put-resource` | `erp`, `resource`, `source`, optional `as` | Copy a resource from an archive in the addon, replacing or adding it |
| `db.set`, `db.add-row`, `db.remove-row`, `db.list-append`, `db.list-remove` | As in [database edits](/reference/ops/database) | Unchanged |
| `db.put-record` | `key` (path or `0x` and 16 hex digits), `type`, `value` (hex) | Write one raw record |
| `db.remove-record` | `key` | Remove one record |
| `lang.set` | `language` (code or `*`), `key`, `value` | Set one string |
| `lang.table` | `language`, `source` (a partial `.lng`) | Set every string in a table |
| `file.put` | `source`, `target` | Replace or add a loose game file |
| `warn` | `message` | Print a message at build |

You can use `when` and `{{id}}` placeholders on every op, just as with database ops. Give `source` paths relative to your addon folder. The ops, `operations.assets` and the planner are already built (`packages/core/src/planner.rs`).

### 3.2 Manifest

```json
"operations": {
  "assets": ["assets/livery.json"]
}
```

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "erp.put-resource", "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep", "source": "erp/car.erp" },
    { "op": "db.set", "select": "entrants.teams.team[_metadata_=toro_rosso_26]", "field": "display_name_code", "value": "AT" },
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" },
    { "op": "file.put", "source": "files/sponsor.dds", "target": "livery_package_shared/sponsor.dds" }
  ]
}
```

In `operations.database` files, write `db.*` ops without the prefix; do the same for `erp.*` ops in `operations.erp` files. Each `operations.files` entry is a `file.put`.

### 3.3 Rust API

```rust
pub enum AssetOp {
    Group(GroupOp),
    Erp(ErpOp),
    Db(DbOp),
    Lang(LangOp),
    File(FileOp),
    Warn { message: String },
}

pub struct Op {
    pub op: AssetOp,
    pub origin: Origin,
}

pub struct Origin {
    pub addon: String,
    pub item: Option<String>,
}

pub struct Source {
    pub root: Root,
    pub path: String,
}

pub enum Root {
    Addon,
    Data,
    Cache,
}

#[derive(Default)]
pub struct AssetBatch {
    pub ops: Vec<Op>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}
```

Keep `Source` paths inside your addon's folder, its `addon-data` folder or its cache folder; any other path fails the build. Use `Origin.item` to name content within the addon, such as a mod id. In JSON, write a `source` as a path in the addon folder, or use `{ "root": "data", "path": ... }` for the other roots.

Core takes those ops through the planner and applies them:

```rust
pub enum Kind { Group, Erp, Db, Lang, File }

pub enum Route { Build, Runtime, Waiting }

pub struct Features {
    pub stable: BTreeSet<Kind>,
    pub experimental: BTreeSet<Kind>,
}

pub struct Planner<'a> { /* deliveries, targets */ }

impl<'a> Planner<'a> {
    pub fn new(features: &'a Features, opt_in: &'a BTreeSet<Kind>, game: &'a dyn GameFiles) -> Self;
    pub fn route(&self, kind: Kind) -> Route;
    pub fn push(&mut self, op: Op, sources: &dyn Fn(&Source) -> Result<PathBuf>) -> Result<()>;
    pub fn finish(self, overlay: &Path) -> Result<Planned>;
}

pub struct Planned {
    pub plan: Plan,
    pub runtime: serde_json::Value,
    pub waiting: Vec<Op>,
    pub conflicts: Vec<Conflict>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

pub struct Conflict {
    pub target: Target,
    pub winner: Origin,
    pub overridden: Vec<Origin>,
}

pub enum Target {
    Group(String),
    Resource { erp: String, uri: String },
    Record(u64),
    String { language: String, key: String },
    File(String),
}
```

`Planned.runtime` is written to `swofty/assets.json`, and `Built.conflicts` keeps the conflicts for the launcher. `Kind` and `Features` are in `swofty_addon::asset`, and `planner::load_features` reads `features.json`.

### 3.4 Deliveries

| Kind | Build delivery (today) | Runtime delivery (swofty_core) | Runtime state | Route now |
|---|---|---|---|---|
| `group` | none | `assets.cpp`: entries inserted into the asset manifest after it is parsed | Stable: a Grand Prix loaded a track from registered groups (T018) | Runtime |
| `erp` | `erpbuild.rs` rewrites the archive into the overlay | `resources.cpp`: an overlay archive first in every group that lists the target, bound by resource URI | Stable (T011, T013) | Runtime |
| `db` | `ops.rs` rewrites `gamedatabases.2025.bin` | `gamedb.cpp`: edits applied when the database loads, with the early stage from `stage.cpp` | Stable (T014, T015, T018) | Runtime |
| `lang` | none: the game reads `.lng` only from its packed archive | `overlay.cpp`: partial string tables added after the stock table | Stable (T016) | Runtime |
| `file` | journaled copy into the game folder | `overlay.cpp`: file system overlay from `swofty/overlay/` | Experimental: not run in game | Runtime when turned on, otherwise Build |

`RUNTIME_DELIVERIES` in `planner.rs` lists all five kinds. `packages/core/src/runtime.rs` writes `swofty/assets.json` and its files. A `db` build whose ops remove records, or write a record of tag 10, 15 or 16 or one that is not text, falls back to the build route with a warning.

The planner chooses a route for each kind:

- Without a build delivery, a kind uses runtime when its feature is `stable`, or `experimental` and listed in `config.json` `"runtime_assets"`. Otherwise it waits. The build lists waiting ops with their origins.
- With both deliveries available, a kind uses runtime when its feature is `stable`, or `experimental` and listed in `config.json` `"runtime_assets"`. Otherwise it uses the build route. To force a kind onto the build route, list it in `config.json` `"build_assets"`.
- Every op of one kind takes the same route in a build.
- An `erp.remove-resource` puts every `erp` op on the build route, because the core cannot take a resource out of a stock archive. `Planner::route` returns `Build` for `erp` once one is pushed, and `Planner::route_note` gives the build line.

Whichever route a kind takes, you get the same result:

- `erp`: `ErpBuild` produces each changed resource. `replace-resource` refills one fragment as `set_fragment` does for `patch-pkg`; `add-resource` builds the resource with `Resource::like`, which copies the type argument, fragment names, flags and compression of a resource of the same type and fragment count in the target archive. The build route writes the whole archive, keeping unchanged resources as byte copies (`ErpFile::write`). `add-mipmaps` files are installed as loose game files through the journal on both routes, as a track's mip files are. The runtime route writes the changed resources as overlay archives under `swofty/assets/resources/`, one per set of target archives, and lists them in `resource_erps`. It leaves out a replacement whose type, argument or fragment layout differs from the stock resource. The core registers the overlay archives in the manifest without rebuilding them.
- `db`: the planner builds the database. The build route writes the file. The runtime route writes the records that differ from the database after track activation as `db_edits` by hash, with their tag and text, and sets `early_stage`. Track activation stays in the file. On both routes, `owned.rs` finds the `unlockable_se_livery` rows the built database has and stock lacks, whichever op added them, and writes their keys to `owned_items` in `swofty/settings.json`.
- `file`: the build route installs into the game folder through the journal. The runtime route copies under `swofty/overlay/` and sets `overlay`.
- `lang`: the runtime route writes one partial table per language under `swofty/assets/lang/` and lists it as `lang.<code>.table`. `*` writes `all.lng`, which applies to every language.
- `group`: the runtime route lists the group in `asset_groups`. Core skips a group that the stock manifest already has. A database field pointed at a stock group whose ERP is missing hangs the session load.

`native/build.sh` writes `swofty/addons/core/features.json`:

```json
{ "schema": 1, "features": { "group": "stable", "erp": "stable", "db": "stable", "lang": "stable", "file": "experimental" } }
```

To mark a native feature `stable`, its runtime test must pass and a verifier must agree under `re/PROTOCOL.md`. You don't need a launcher change. If the loader has no `features.json`, it has no runtime features.

`swofty/assets.json` uses the prototype's schema (`early_stage`, `asset_groups`, `resources`, `db_edits`, `lang`, `overlay`), plus `resource_erps` for overlay archives the launcher built, `features` for the experimental features the profile turned on and an `origin` on each asset group for the log.

### 3.5 Order and conflicts

- Track activation comes first, followed by addons in resolver order. Within your addon, declared ops apply before build hook ops, which keep the order you returned them in. The later write wins.
- The planner records the target of each write. A write that replaces a target (`put`, `replace` and `remove` ops, `db.set`, `db.put-record`, `lang.set`, `file.put`, `erp.add-mipmaps`) after another origin wrote it is a conflict. Edits inside a resource (`patch-pkg`) and row additions compose and are not conflicts.
- You'll see each conflict in the build output with its winner and overridden origins, and in the launcher under the play bar.

### 3.6 Language baseline

To diff text, you need the stock language files. They live in a packed archive: its index is in `F1_25.exe`, its data in `game.dat`. To read them, `swofty-formats` gains:

- `nefs`: reads that archive and extracts `localisation/2025_release/language_<code>.lng` into `$DATA/baseline/<build key>/lang/` once per game build;
- `lng`: reads, writes and diffs LNGT language files.

`GameFiles::original_lang` returns the extracted files. `GameFiles::exe_version` reads the exe's version resource (for example `1.0.141.7865`).

### 3.7 Native runtime SDK

For native plugins, API version 4 adds an `assets` table to `SwoftyApi`. Use it through the wrapper in `swofty/assets.hpp`:

```cpp
namespace swofty::assets {

enum class Result { ok, late, unavailable, invalid };
struct Fragment {
    std::string_view file;
    std::string_view fourcc = {};
    uint32_t flags = 0;
};

bool available(std::string_view feature);
Result register_group(std::string_view group, std::span<const std::string_view> erps,
                      std::span<const std::string_view> deps = {}, std::string_view templ = {});
Result put_resource(std::string_view uri, std::span<const Fragment> fragments,
                    std::span<const std::string_view> erps = {});
Result db_set(std::string_view key, std::string_view type, std::string_view value);
Result db_set(uint64_t hash, uint8_t tag, std::span<const std::byte> value);
Result lang_set(std::string_view language, std::string_view key, std::string_view text);
Result lang_table(std::string_view language, std::string_view file);
Result overlay_file(std::string_view game_path, std::string_view file);

}
```

| Call | Accepted until | Core part |
|---|---|---|
| `register_group` | the asset manifest is parsed (boot step 24) | `assets.cpp` |
| `put_resource` | the resource overlay is written, at the end of plugin init | `resources.cpp` |
| `db_set` | the game database loads (step 10). Only plugins with `"early": true` load before it | `gamedb.cpp`, `stage.cpp` |
| `lang_set`, `lang_table` | the language file loads (step 24) | `overlay.cpp` |
| `overlay_file` | the game first reads that path | `overlay.cpp` |

- Call after the cutoff above and you get `late`. If core lacks the feature, or the profile hasn't turned on an experimental feature, you get `unavailable`. Every call is logged with the addon id.
- `type` takes the `db_edits` type names (`int`, `float`, `string`, `enum`, `id`, `bool`, ...) or a tag number. `tag` is the raw record's tag.
- Give file arguments relative to your addon's folder, `swofty/addons/<id>/`. Core identifies your addon from the calling DLL.
- The build takes each plugin's `dll` and everything in `files` on its `operations.native` entry and installs them through the journal to `swofty/addons/<id>/`, preserving their paths within the addon. It lists the DLLs in `plugins` in `swofty/settings.json`, in load order. The loader loads them from there.
- SDK calls feed the same lists as `swofty/assets.json`. Entries from `assets.json` apply first, then calls in plugin load order.
- Set `"early": true` on your native plugin's `operations.native` entry to load it at the early stage for the assets API.

## 4. Team Simplified addon

### 4.1 The addon

- The crate `addons/simplified` (`swofty-addon-simplified`) is Rust and PolyForm-Noncommercial-1.0.0, like the rest of the repository. Its `mod.serps` reader, package root, content root and entry path checks are ported from the Simplified Launcher (GPL-3.0, used with Team Simplified's permission) and carry the Simplified Suite notice. `THIRD_PARTY.md` credits Team Simplified for the `.serps` format and the Simplified Suite, and the Mods section shows the credit.
- You can already use import, the library, profile state, the section and `swofty mods` (work item 13), diffs (work item 14), and merge and conflicts (work item 15).
- Manifest: id `teamsimplified.mods`, name `Team Simplified`, author `Team Simplified`. No options and no native plugin.
- It's built in and on by default in new profiles. Turn it off for a profile to hide its section.
- It uses `swofty-addon` and `swofty-formats` only. Core has no code for it.

### 4.2 The .serps format

- You can supply a folder or a zip archive under any name, including `.serps`.
- `mod.serps` at the package root is JSON: `mod_name` or `name` (`mod_name` wins), `author`, `version`, `game` (for example `F1® 25`), `game_version` (for example `1.0.141.7865`), and `studio_version` from Simplified Studio. Every field is optional. The reader accepts a UTF-8 byte order mark, comments and trailing commas.
- `F1 25/` holds files in the game folder's layout. RM Designs' Scuderia AlphaTauri ORLEN package has the whole `game_package/gamedatabases/gamedatabases.2025.bin`, 11 whole language files under `localisatioa/2025_release/` and four whole shader archives under `shader_package_2025/`.
- You'll see `localisatioa/` because Simplified's launcher points a patched exe there. Both `localisatioa/` and `localisation/` refer to the stock language file with that file name.
- If there's no `F1 25/`, the importer looks within three levels for the first folder whose subfolders match the installed game's top-level folders, or `localisation` or `localisatioa`. That becomes the content root.

### 4.3 Import

1. The importer unpacks your package into `addon-data/teamsimplified.mods/.import-<n>/`, skipping and counting entries with absolute paths or `..`.
2. It reads `mod.serps` and finds the content root.
3. It classifies each file:

| Path | Class |
|---|---|
| `game_package/gamedatabases/gamedatabases.2025.bin` | Database. It must parse as F1 25's database, or the import fails |
| `localisation/*/language_<code>.lng`, `localisatioa/*/language_<code>.lng` | Text |
| A `.erp` the game ships | Archive |
| Any other file the game ships | Replacement |
| A file the game does not ship | Addition |
| `.exe`, `.dll`, `.asi` | Skipped and named in the report |

4. The id is `<author>.<name>`, lowercased to `[a-z0-9-]`, or the file name if both are missing. If you've already imported that id, the importer stages the new copy, moves the old one aside, moves the new one in, then deletes the old one. Each profile keeps the id's position and switch setting.
5. The importer diffs the package ([section 4.5](#_4-5-diff)) and writes `package.json`.

You can import zip archives and folders. Support for 7z comes later; RAR isn't supported.

### 4.4 Library

```
$DATA/addon-data/teamsimplified.mods/library/<id>/
  package.json          name, author, version, game_version, studio_version, source name, sha256, import time, file list
  mod.serps             as shipped
  files/                the content root, by game path
  diff/<build key>.json the diff for one game build
  diff/<build key>.db   its database records
```

All your profiles share this library. Each profile keeps its own list of mods in its state, in load order:

```json
"state": {
  "teamsimplified.mods": {
    "mods": [{ "id": "rm-designs.scuderia-alphatauri-orlen", "enabled": true }]
  }
}
```

Mods outside that list are off and appear after it, sorted by name. Remove a mod from the library and you remove it from every profile.

### 4.5 Diff

To find what a mod changes, compare it with clean files from `GameFiles`, never the modded game folder:

| Class | Unit | Changed | Added | Removed |
|---|---|---|---|---|
| Database | record key | same key, other tag or bytes | key not in stock | stock key missing from the package |
| Archive | resource URI | index digest differs (fragment data when a digest is empty) | URI not in stock | stock URI missing from the package |
| Text | language and key | value differs | key not in stock | never |
| Replacement, Addition | file | whole file | whole file | never |

- Removals count only if `game_version` matches the installed exe's version. If it doesn't, they're dropped; the row tells you how many and which version the package was made for.
- For a language missing from the package, the diff uses the changes in the package's `language_eng.lng`, or its first language file by name.
- Archive diffs read the index only (`ErpFile::open`). The database diff parses both files once.
- Diffs are cached per game build key in `diff/<build key>.json`; changed and added records go in `diff/<build key>.db`. Import computes the diff if it finds a game. A mod missing a diff for the installed build gets one at the next build, or when you choose `Compare` in the section.
- Until there's a diff for the installed build, expanding the row shows the package's files by class.

### 4.6 Merge

The build hook emits ops for each enabled mod, in profile order, with origin `teamsimplified.mods` and the mod id:

| Class | Ops |
|---|---|
| Database | `db.put-record`, `db.remove-record` |
| Archive | `erp.put-resource` from the mod's archive, `erp.remove-resource` |
| Text | `lang.set` |
| Replacement, Addition | `file.put` |

- These ops apply after track activation and earlier addons. Only the database records the package changed are touched; archives merge one resource at a time.
- When a package adds a special livery row, it's reported as owned through `owned_items` ([section 3.4](#_3-4-deliveries)), so you'll see the livery in the picker.
- Ops follow the planner's routes ([section 3.4](#_3-4-deliveries)). If you don't have the in-game loader, text ops wait; the build tells you how many entries are waiting.
- Resource removals are `erp.remove-resource` ops on the archive the mod removed them from, and put every `erp` op on the build route. A removal of a record or resource an earlier mod already removed is left out.
- Database records are written as raw `db.put-record` values from `diff/<build key>.db`. Archive and file sources are `{ "root": "data", "path": "library/<id>/files/<game path>" }`.
- A write is left out if it repeats the value an earlier mod has already put in place.

#### Shader archive carriers

Simplified Studio puts a mod's textures and models in archives under `shader_package_2025/`, which the game loads at boot. But none of the asset groups the loader patches lists those archives. Target them with an overlay and the game never reads it. The handling below is already built.

- Added shader archive resources go into `2025_asset_groups/vfx_package/common.erp` instead. The boot groups that list this archive load its overlay before any car or UI group, just as the shader archive would. Changed resources stay on the shader archive.
- Team logo texture arrays in `ui_texture*` archives merge across every enabled mod. For each `.texarray.image`, start with the game's array from the first of eight `ui_package` archives that holds it. Split fragment 1 into slices, then apply each mod's slices that differ from the game's; later mods win. An older game's array with fewer slices of the same format and size merges over just the slices it has. A different format uses the whole array and produces a build warning. The result goes to `cache/addons/teamsimplified.mods/ui_textures.erp` and into `2025_asset_groups/vfx_package/param_particles/systemstartup.erp`, loading as its own overlay with the boot group. This is ported from the Simplified Launcher's `merge_resources.py` and the Team Editor's logo code.
- Simplified Studio's `eaid://tools_package/` marker is left out.

#### Text a package fills in

A team rename package points team name records at its own keys, such as `lng_team_ast_title_26`. It supplies a value for every team: the new name for the renamed team, the game's text for everyone else. Apply all those values and you'd undo another package's rename.

- If an added key repeats the game's text for the key the stock record pointed at, it's a default, stored in the diff's `defaults`. English text counts too, since packages fill in untranslated names.
- A changed translation whose text is the game's English text of the same key is left out.
- Defaults go in once for each key no package renames, before every package's own changes. The diff has a `version`, and a cached diff of another version is computed again.

### 4.7 Conflicts

- The section computes conflicts between enabled mods from their cached diffs, in profile order. A mod counts once for each target. Simplified Studio's marker and the logo texture arrays merged slice by slice are not targets. A row shows `Overrides <n> in <mod>` when it wins and `<n> overridden by <mod>` when it loses. Its expanded body lists the targets under `CONFLICTS`: resource URIs with their archive, record keys in hex, text keys with their language, and files.
- The build reports conflicts with other addons and tracks through the planner ([section 3.5](#_3-5-order-and-conflicts)).

### 4.8 Section

You'll find `Mods` after `Addons` in the sidebar, with the library size as its count.

- **Header.** The title is `Mods`, with subtitle `<n> in library · <k> on in <profile> · lower in the list wins`. Choose `Import…` to pick `.serps` and `.zip` files and folders, several at once.
- **Notices.** You'll see a notice when the addon is off in this profile (button `Turn on`), the vanilla profile is active, or mods haven't been compared with the installed game build (button `Compare`).
- **Rows.** Reorderable, in load order. Title from `mod.serps`, subtitle `<author> · <version>`, tags `Database`, `Text`, `<n> archives`, `<n> files`, conflict tags and `Made for <game_version>` when it differs from the game. A switch turns the mod on in the profile.
- **Expanded row.** Stats: status, version, author, made for, Studio version, size, imported. `WHAT IT CHANGES`: records changed, added and removed; text entries by language; resources by archive; files. `CONFLICTS`. Buttons: Move up, Move down, Show files, and Remove from library with a confirmation.
- **Empty.** `No mods yet`, `Import a .serps package, a zip or a folder that holds an F1 25 folder.` and `Import…`.
- **Footer.** `Mod packages in the Simplified Suite's .serps format. Based on Simplified Suite by Team Simplified - https://github.com/TeamSimplified/simplified-suite`
- **Profile cards.** `summary` adds `MODS` with the enabled mods' names.
- **CLI.** `swofty mods list`, `import <path>…`, `remove <id>`, `enable <id>`, `disable <id>`, `move <id> <position>`.

## 5. Tracks are mods

You manage tracks as mods in the Team Simplified library. That's already built, with no tracks addon or separate tracks page.

- **Raw track zips.** Team Simplified claims zips containing a track package (`manifest.json` with `track_code`), or downloads containing one such zip. Import stores it in `library/<id>/`: `package.json` has `track` set to the code, `mod.serps` is written from the manifest, and the package goes under `track/`. The id is the track code, or `<code>-track` if a library mod already uses that id. Import the track again and it replaces the existing copy, keeping its id.
- **Track SERPs.** A package built by the Simplified Track Installer (the track archive in an invitational slot, mipmaps under `environment_package/tracks/<code>/mipmaps/`, the `track_info` shader archives and optionally a whole game database) imports, diffs and merges like any other `.serps` mod ([sections 4.3 to 4.6](#_4-3-import)), with the slot as shipped.
- **Build.** The build hook emits `track.add` for each enabled raw track in profile order, with `source` `{ "root": "data", "path": "library/<id>/track" }`. Core processes `track.add` through `activation/` before other database ops, handling the database rows, archive, menus and ERS. If your loader's `features.json` includes `group`, the archive uses its natural path `2025_asset_groups/environment_package/tracks/<code>/<code>.erp`, with `group.register` ops and natural resource-row values. You can then add tracks without a limit. Otherwise they use the invitational slots, at most eight.
- **Migration.** `Addon::migrate` runs from `Store::migrate` (`data_version` 3). Team Simplified moves each `$DATA/maps/<code>/` into the library and each profile's `maps` list into its state, enabled, after the mods already listed. The track's id is its code, or `<code>-track` when a library mod already has that id. Each track is staged under `library/.migrate-<folder>/`, and the next run finishes a staged track. A code that matches no track in the library is dropped. `Profile` still reads `maps` and no longer writes it.
- **Removed from core.** The old tracks page and its sidebar entry, `Profile.maps`, `Store::maps`, `import_map`, `remove_map`, the core track importer and the `swofty maps` commands. `swofty maps` is a hidden alias of `swofty mods`.

## 6. Work items

Work through these in order, with one PR per item.

| # | Item | Scope | Depends on |
|---|---|---|---|
| 1 | Rename in Rust | Core, launcher and CLI identifiers; `mods/` and `templates/mod` to `addons/` and `templates/addon`; `Store::migrate`; `Profile.addons` with the `mods` alias; `swofty/settings.json` still writes `mods` | |
| 2 | Rename in the launcher and CLI | Launcher text; `swofty addons`; `swofty mods` as a hidden alias | 1 |
| 3 | Rename in game and the SDK | `addons` in `swofty/settings.json` with the native fallback; `ADDON SETTINGS`; `swofty_addon_<n>`; `swofty/addons/` loader folders; `SwoftyAddonInit` and `SWOFTY_ADDON()` with the old names accepted; Wine test | 1 |
| 4 | Rename in the docs | Titles, text, sidebar and README | 2, 3 |
| 5 | Asset op model | `packages/addon-api` with `manifest` and `ops` moved in; `AssetOp`, `Op`, `Origin`, `Source`; unified op files and `operations.assets`; build behaviour unchanged | 1 |
| 6 | Asset planner | Routing, `features.json` reading, build deliveries for `erp`, `db` and `file` (`file.put` is built for the first time), `erp.put-resource` with streamed copies, `db.put-record` and `db.remove-record`, targets and conflicts, waiting ops; `owned_items` from the built database instead of from `add-row` ops | 5 |
| 7 | Runtime asset groups | `assets.cpp` ported from the prototype into `native/`; `features.json` from `native/build.sh`; `group.register` runtime delivery into `swofty/assets.json`; stable since T018 | 3, 6 |
| 8 | Launcher API in core | `Addon`, `Importer`, `Cx`, `BuildCx`, `GameFiles`, `View` types; addon registry with built-ins; `Profile.state`; `addon-data` and cache folders; build hooks in `build.rs`; `addons/builtins` | 5 |
| 9 | Addon sections in the launcher | Generic `View` renderer, sidebar entries, events and effects on the background executor, pickers, drag and drop routing, progress, profile card summaries | 2, 8 |
| 10 | Addon commands in the CLI | `CommandSpec` dispatch; forwarding of the old `swofty mods` forms | 2, 8 |
| 11 | Language files | `swofty-formats` `lng`: read, write and diff LNGT | |
| 12 | Language baseline | `swofty-formats` `nefs` for the archive indexed in `F1_25.exe`; stock `.lng` extraction per game build; `exe_version` | 11 |
| 13 | Team Simplified: import and library | `addons/simplified` crate; zip and folder import; content root and `mod.serps`; classes; library; profile state; section with list, switches, order and removal; `swofty mods`; `THIRD_PARTY.md` credit | 9, 10 |
| 14 | Team Simplified: diff | Database, archive, file and text diffs against `GameFiles`; cache per build key; version rule for removals | 11, 12, 13 |
| 15 | Team Simplified: merge | Build hook emitting ops in profile order; conflict tags and lists in the section | 6, 14 |
| 16 | Runtime text | `overlay.cpp` string tables ported; `lang` runtime delivery writing partial tables; stable since T016 | 3, 6, 11 |
| 17 | Runtime resources | `resources.cpp` ported; `erp` runtime delivery; stable since T013 | 6, 7 |
| 18 | Runtime database | `gamedb.cpp` with raw values and `stage.cpp` ported; `db` runtime delivery; stable since T014, T015 and T018 | 6, 7 |
| 19 | Runtime file overlay | `overlay.cpp` file system overlay ported; `file` runtime delivery; experimental until it runs in game | 6, 7 |
| 20 | Native assets API | API version 4 `assets` table, `swofty/assets.hpp`, the `assets` capability and `"early": true` | 7 (groups), 16 to 19 (other calls) |
| 21 | Docs | Player docs for the Mods section; modding docs for asset ops, the native assets API and the Launcher API | 15, 20 |
| 22 | Tracks as mods | [Section 5](#_5-tracks-are-mods) | 8, 7, 15 |
