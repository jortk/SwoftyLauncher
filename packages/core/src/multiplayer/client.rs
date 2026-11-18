//! The launcher's side of the session module: loads it, sends it requests and answers the ones it sends
//! back. Every function here does nothing, or returns the reason, while the module isn't loaded.

use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use serde_json::{Value, json};

use super::fingerprint::{self, Fingerprint};
use super::types::{FixStep, GameStatus, RoomSettings, RoomSummary, SessionState};
use crate::baseline::Identity;
use crate::catalog::{self, Addon};
use crate::modules::library::{Hooks, Library};
use crate::modules::{self, Unavailable};
use crate::profile::Profile;
use crate::store::Store;
use crate::timetrial::api::Api;

/// The module's name on the server and the ABI this launcher speaks to it.
pub const MODULE: &str = "mp-launcher";
pub const MODULE_ABI: u32 = 1;
/// The environment variable that carries [`launch_env`] to the process that starts the game, as a JSON
/// list of `[name, value]` pairs.
pub const LAUNCH_ENV: &str = "SWOFTY_SESSION_ENV";
/// How long a failed load stands for the same sign-in before the server is asked again: [`RETRY_SOON`] for a
/// load asked for directly, [`RETRY_LATER`] for one started in the background.
const RETRY_SOON: Duration = Duration::from_secs(30);
const RETRY_LATER: Duration = Duration::from_secs(15 * 60);

/// The session log and what a session saves beside it, under the data folder.
pub const LOG_FILE: &str = "state/multiplayer.log";
pub const LOG_PREVIOUS: &str = "state/multiplayer.1.log";
pub const PRINTS_DIR: &str = "state/multiplayer-prints";
/// Where, under the game folder, the game reports its side of a session.
pub const GAME_STATUS_FILE: &str = "swofty/netplay.json";

/// Reads this player's content again for a running session.
pub type Refresh = Arc<dyn Fn() -> Option<Local> + Send + Sync>;

enum State {
    Idle,
    Loading,
    Loaded(Arc<Library>),
    Failed(Failure),
}

struct Failure {
    why: String,
    at: Instant,
    token: u64,
}

impl Failure {
    /// Whether this failure still answers a load for `token` at `now`, without asking the server.
    fn stands(&self, token: u64, wait: Duration, now: Instant) -> bool {
        self.token == token && now.saturating_duration_since(self.at) < wait
    }
}

fn token_key(token: Option<&str>) -> u64 {
    let mut h = DefaultHasher::new();
    token.hash(&mut h);
    h.finish()
}

static STATE: Mutex<State> = Mutex::new(State::Idle);
static CHANGED: Condvar = Condvar::new();
static REFRESHES: Mutex<Option<HashMap<u64, Refresh>>> = Mutex::new(None);
static NEXT_REF: AtomicU64 = AtomicU64::new(1);
static KNOWN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn loaded() -> Result<Arc<Library>, String> {
    match &*lock(&STATE) {
        State::Loaded(library) => Ok(library.clone()),
        _ => Err("the session module isn't loaded".to_owned()),
    }
}

/// Whether [`load`] has succeeded in this process.
pub fn is_loaded() -> bool {
    matches!(&*lock(&STATE), State::Loaded(_))
}

/// Why the last [`load`] failed, as the sentence to show; `None` when it succeeded or never ran.
pub fn unavailable() -> Option<String> {
    match &*lock(&STATE) {
        State::Failed(f) => Some(f.why.clone()),
        _ => None,
    }
}

fn answer(request: &Value) -> Result<Value, String> {
    match request.get("op").and_then(Value::as_str) {
        Some("local.refresh") => {
            let id = request.get("ref").and_then(Value::as_u64).unwrap_or(0);
            let refresh = lock(&REFRESHES).as_ref().and_then(|r| r.get(&id).cloned());
            let local = refresh
                .and_then(|r| r())
                .ok_or("this player's content couldn't be read")?;
            serde_json::to_value(local).map_err(|e| e.to_string())
        }
        other => Err(format!(
            "the launcher has no answer for `{}`",
            other.unwrap_or("?")
        )),
    }
}

/// Makes the session module ready: fetched and verified (or the cached copy), then loaded once for this
/// process. `token` is the signed-in session, when there is one. The error is the sentence to show. A load
/// another thread already runs is waited for instead of started twice; nothing else waits on a load. A load
/// that failed less than [`RETRY_SOON`] ago with the same `token` is answered with that failure.
pub fn load(data: &Path, token: Option<&str>) -> Result<(), Unavailable> {
    let key = token_key(token);
    {
        let mut state = lock(&STATE);
        loop {
            match &*state {
                State::Loaded(_) => return Ok(()),
                State::Loading => state = CHANGED.wait(state).unwrap_or_else(|e| e.into_inner()),
                State::Failed(f) if f.stands(key, RETRY_SOON, Instant::now()) => {
                    return Err(Unavailable::from(f.why.clone()));
                }
                State::Idle | State::Failed(_) => break,
            }
        }
        *state = State::Loading;
    }
    if let Some(token) = token {
        know_secret(token);
    }
    let opened = open(data, token);
    let mut state = lock(&STATE);
    let result = match opened {
        Ok(library) => {
            *state = State::Loaded(Arc::new(library));
            Ok(())
        }
        Err(e) => {
            *state = State::Failed(Failure {
                why: e.why.clone(),
                at: Instant::now(),
                token: key,
            });
            Err(e)
        }
    };
    drop(state);
    CHANGED.notify_all();
    if result.is_ok() {
        remember_launch_env(data);
    }
    result
}

fn open(data: &Path, token: Option<&str>) -> Result<Library, Unavailable> {
    let api = Api::new(token.unwrap_or_default());
    let module = modules::ensure(data, &api, MODULE, modules::platform(), MODULE_ABI)?;
    let config = json!({
        "data": data,
        "server": api.server(),
        "launcher": crate::update::current_version().to_string(),
    });
    let hooks = Hooks {
        call: Box::new(answer),
        notify: Box::new(|_| {}),
    };
    Library::open(&module, MODULE_ABI, &config, hooks).map_err(Unavailable::from)
}

/// Starts [`load`] on its own thread unless the module is loaded, a load is running, or a load with the same
/// `token` failed less than [`RETRY_LATER`] ago. Returns at once.
pub fn load_in_background(data: &Path, token: &str) {
    match &*lock(&STATE) {
        State::Loaded(_) | State::Loading => return,
        State::Failed(f) if f.stands(token_key(Some(token)), RETRY_LATER, Instant::now()) => return,
        State::Idle | State::Failed(_) => {}
    }
    let (data, token) = (data.to_path_buf(), token.to_owned());
    std::thread::Builder::new()
        .name("session-module".into())
        .spawn(move || {
            if let Err(e) = load(&data, Some(&token)) {
                eprintln!("session module: not loaded: {e}");
            }
        })
        .ok();
}

/// Forgets a failed load, so the next one asks the server whatever the wait: for an account that just gained
/// access.
pub fn forget_failure() {
    let mut state = lock(&STATE);
    if matches!(&*state, State::Failed(_)) {
        *state = State::Idle;
    }
}

/// Waits up to `limit` for a load that is running now; returns at once when none is.
fn settle(limit: Duration) {
    let deadline = Instant::now() + limit;
    let mut state = lock(&STATE);
    while matches!(&*state, State::Loading) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        state = CHANGED
            .wait_timeout(state, left)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

fn call(op: &str, args: Value) -> Result<Value, String> {
    loaded()?.call(op, args)
}

fn read<T: serde::de::DeserializeOwned>(op: &str, args: Value) -> Result<T, String> {
    serde_json::from_value(call(op, args)?)
        .map_err(|e| format!("the answer to `{op}` is not what the launcher reads: {e}"))
}

/// The public sessions, for the signed-in session `token`.
pub fn rooms(token: &str) -> Result<Vec<RoomSummary>, String> {
    read("rooms.list", json!({ "token": token }))
}

/// What the game last reported; with `ask`, the game is asked when it has reported nothing.
pub fn game_status(game_dir: &Path, ask: bool) -> Option<GameStatus> {
    read("game.status", json!({ "game_dir": game_dir, "ask": ask })).ok()
}

/// Drops the game's last report, before a game is started.
pub fn forget_status(game_dir: &Path) {
    call("game.forget_status", json!({ "game_dir": game_dir })).ok();
}

/// The environment a game started for sessions gets; empty while the module isn't loaded.
pub fn launch_env(for_session: bool) -> Vec<(String, String)> {
    read("launch.env", json!({ "room": for_session })).unwrap_or_default()
}

/// Where the last environment the module gave is kept, under the data folder.
const LAUNCH_ENV_FILE: &str = "state/session-env.json";
/// The environment a game was started with before the module named it; used until the module has been
/// loaded once on this machine.
const FIRST_LAUNCH_ENV: [(&str, &str); 4] = [
    ("SWOFTY_NETPLAY", "armed"),
    ("SWOFTY_NET_TRACE", "1"),
    ("SWOFTY_NET_TRACE_CAP_KB", "16384"),
    ("SWOFTY_GAME_LOG_CHANNELS", "default,NETWORK*:info,SSL*:info"),
];

fn remember_launch_env(data: &Path) -> Option<Vec<(String, String)>> {
    let env = launch_env(false);
    if env.is_empty() {
        return None;
    }
    let file = data.join(LAUNCH_ENV_FILE);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(file, json!(env).to_string()).ok();
    Some(env)
}

/// The value of [`LAUNCH_ENV`] for a game started now. Never waits on the network: a load that is
/// already running is given up to `wait`; with the module loaded the environment is the module's;
/// otherwise it is the one the module last gave on this machine, or the first-launch one.
pub fn launch_env_value(data: &Path, wait: Duration) -> String {
    settle(wait);
    let env = is_loaded()
        .then(|| remember_launch_env(data))
        .flatten()
        .or_else(|| {
            let text = std::fs::read_to_string(data.join(LAUNCH_ENV_FILE)).ok()?;
            Some(parse_launch_env(&text)).filter(|env| !env.is_empty())
        })
        .unwrap_or_else(|| {
            FIRST_LAUNCH_ENV
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        });
    json!(env).to_string()
}

/// The pairs a [`LAUNCH_ENV`] value carries.
pub fn parse_launch_env(value: &str) -> Vec<(String, String)> {
    serde_json::from_str(value).unwrap_or_default()
}

/// One line into the session log.
pub fn line(text: impl std::fmt::Display) {
    call("diag.line", json!({ "text": text.to_string() })).ok();
}

const REDACTED: &str = "[redacted]";
const TOKEN_MIN: usize = 20;
const KNOWN_MIN: usize = 6;

/// Names a secret this process holds (a sign-in token), so [`scrub`] replaces it wherever it appears.
pub fn know_secret(value: &str) {
    let mut known = lock(&KNOWN);
    if value.len() >= KNOWN_MIN && !known.iter().any(|k| k == value) {
        known.push(value.to_owned());
    }
}

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')
}

/// Whether `word` looks like a credential rather than a name, a path or a digest: long, with digits and
/// both letter cases. Lower-case hex digests and plain words stay readable.
fn token_like(word: &str) -> bool {
    word.len() >= TOKEN_MIN
        && word.bytes().any(|b| b.is_ascii_digit())
        && word.bytes().any(|b| b.is_ascii_uppercase())
        && word.bytes().any(|b| b.is_ascii_lowercase())
        && word.bytes().filter(|b| *b == b'/').count() <= 3
}

fn redact_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let start = rest.find(token_char).unwrap_or(rest.len());
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c| !token_char(c)).unwrap_or(rest.len());
        let word = &rest[..end];
        out.push_str(if token_like(word) { REDACTED } else { word });
        rest = &rest[end..];
    }
    out
}

/// `text` safe to send: the secrets the module registered (when it is loaded), the ones this process
/// named with [`know_secret`], and anything that looks like a credential are replaced. Works without the
/// module, for files an earlier run wrote.
pub fn scrub(text: &str) -> String {
    let mut text = read::<String>("diag.scrub", json!({ "text": text })).unwrap_or_else(|_| text.to_owned());
    for secret in lock(&KNOWN).iter() {
        if text.contains(secret.as_str()) {
            text = text.replace(secret.as_str(), REDACTED);
        }
    }
    redact_tokens(&text)
}

/// [`scrub`] for a JSON document: values under secret-looking names are replaced as well.
pub fn scrub_json(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(mut doc) => {
            crate::send_logs::redact_json(&mut doc);
            scrub(&serde_json::to_string_pretty(&doc).unwrap_or_default())
        }
        Err(_) => scrub(text),
    }
}

/// The running session's state for a sent logs bundle, secrets redacted.
pub fn snapshot() -> Option<Value> {
    let mut doc = call("session.snapshot", Value::Null)
        .ok()
        .filter(|v| !v.is_null())?;
    crate::send_logs::redact_json(&mut doc);
    Some(doc)
}

/// The newest `n` comparisons a session saved, newest first.
pub fn newest_prints(data_root: &Path, n: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(data_root.join(PRINTS_DIR)) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|e| {
            let path = e.ok()?.path();
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            (path.extension()? == "json").then_some((modified, path))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    files.into_iter().take(n).map(|(_, p)| p).collect()
}

/// What this player's profile plays, as the module compares it with the host's.
#[derive(Debug, Clone, Serialize)]
pub struct Local {
    pub fingerprint: Fingerprint,
    /// Each addon's category, by id.
    pub categories: BTreeMap<String, String>,
    /// The Addon Browser's cached listing.
    pub catalog: Vec<Addon>,
    /// Each built addon's read paths, by id, in its detail's read order. The fingerprint leaves them out;
    /// the module puts them back to name a file that differs.
    pub read_paths: BTreeMap<String, Vec<String>>,
}

impl Local {
    pub fn new(fingerprint: Fingerprint, categories: BTreeMap<String, String>, catalog: Vec<Addon>) -> Self {
        let read_paths = fingerprint
            .addons
            .iter()
            .filter_map(|a| Some((a.id.clone(), a.detail.as_ref()?.paths.clone())))
            .filter(|(_, paths)| !paths.is_empty())
            .collect();
        Self {
            fingerprint,
            categories,
            catalog,
            read_paths,
        }
    }

    /// The fingerprint of `profile` on the game `game`, with the cached Addon Browser listing.
    pub fn read(store: &Store, profile: &Profile, game: &Identity) -> crate::Result<Local> {
        let fingerprint = fingerprint::fingerprint(store, profile, game)?;
        let installed = store.addons()?;
        let categories = fingerprint
            .addons
            .iter()
            .filter_map(|a| {
                let category = installed.get(&a.id)?.category?;
                let name = serde_json::to_value(category).ok()?.as_str()?.to_owned();
                Some((a.id.clone(), name))
            })
            .collect();
        let catalog = catalog::cached(&store.root).unwrap_or_default();
        Ok(Local::new(fingerprint, categories, catalog))
    }

    pub fn digest(&self) -> String {
        format!("sha256:{}", self.fingerprint.digest)
    }
}

/// What a session starts with.
pub struct Setup {
    pub token: String,
    pub game_dir: PathBuf,
    pub local: Local,
    /// The launcher starts the game for the session when the module asks ([`SessionState::launch`]).
    pub room_launch: bool,
    pub refresh: Option<Refresh>,
}

impl Setup {
    pub fn new(token: &str, game_dir: &Path, local: Local) -> Self {
        Self {
            token: token.to_owned(),
            game_dir: game_dir.to_path_buf(),
            local,
            room_launch: false,
            refresh: None,
        }
    }
}

struct View {
    state: SessionState,
    fix_plan: Vec<FixStep>,
    finished: bool,
}

/// One session in the module. Dropping it leaves the session.
pub struct Session {
    library: Arc<Library>,
    id: u64,
    refresh: u64,
    last: Mutex<View>,
}

impl Session {
    pub fn create(setup: Setup, settings: &RoomSettings) -> Result<Session, String> {
        Self::start("session.create", setup, json!({ "settings": settings }))
    }

    pub fn join(setup: Setup, code: &str) -> Result<Session, String> {
        Self::start("session.join", setup, json!({ "code": code.trim() }))
    }

    fn start(op: &str, setup: Setup, mut args: Value) -> Result<Session, String> {
        let library = loaded()?;
        know_secret(&setup.token);
        let refresh = NEXT_REF.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = setup.refresh {
            lock(&REFRESHES)
                .get_or_insert_with(HashMap::new)
                .insert(refresh, f);
        }
        args["token"] = json!(setup.token);
        args["game_dir"] = json!(setup.game_dir);
        args["local"] = serde_json::to_value(&setup.local).map_err(|e| e.to_string())?;
        args["room_launch"] = json!(setup.room_launch);
        args["ref"] = json!(refresh);
        let started = library.call(op, args).inspect_err(|_| {
            if let Some(r) = lock(&REFRESHES).as_mut() {
                r.remove(&refresh);
            }
        })?;
        let id = started
            .get("session")
            .and_then(Value::as_u64)
            .ok_or("the module named no session")?;
        Ok(Session {
            library,
            id,
            refresh,
            last: Mutex::new(View {
                state: SessionState::new(op == "session.create"),
                fix_plan: Vec::new(),
                finished: false,
            }),
        })
    }

    fn send(&self, op: &str, mut args: Value) -> Result<Value, String> {
        args["session"] = json!(self.id);
        self.library.call(op, args)
    }

    fn poll(&self) -> MutexGuard<'_, View> {
        let mut last = lock(&self.last);
        if let Ok(mut doc) = self.send("session.state", json!({})) {
            let take = |doc: &mut Value, key: &str| doc.get_mut(key).map(Value::take).unwrap_or(Value::Null);
            if let Ok(mut state) = serde_json::from_value::<SessionState>(take(&mut doc, "state")) {
                state.progress = serde_json::from_value(take(&mut doc, "progress")).ok();
                state.problem = serde_json::from_value(take(&mut doc, "problem")).ok().flatten();
                last.state = state;
                last.fix_plan = serde_json::from_value(take(&mut doc, "fix_plan")).unwrap_or_default();
                last.finished = doc.get("finished").and_then(Value::as_bool).unwrap_or(false);
            }
        }
        last
    }

    /// The session's state now; the last one read when the module doesn't answer.
    pub fn state(&self) -> SessionState {
        self.poll().state.clone()
    }

    pub fn fix_plan(&self) -> Vec<FixStep> {
        self.poll().fix_plan.clone()
    }

    pub fn is_finished(&self) -> bool {
        self.poll().finished
    }

    pub fn mark_race(&self, started: bool) {
        self.send("session.mark_race", json!({ "started": started })).ok();
    }

    pub fn retry(&self) {
        self.send("session.retry", json!({})).ok();
    }

    pub fn reconnect_relay(&self) {
        self.send("session.reconnect_relay", json!({})).ok();
    }

    pub fn update_local(&self, local: &Local) {
        self.send("session.update_local", json!({ "local": local })).ok();
    }

    pub fn launched(&self) {
        self.send("session.launched", json!({})).ok();
    }

    pub fn launch_failed(&self, why: &str) {
        self.send("session.launch_failed", json!({ "why": why })).ok();
    }

    pub fn leave(&self) {
        self.send("session.leave", json!({})).ok();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.leave();
        self.send("session.close", json!({})).ok();
        if let Some(r) = lock(&REFRESHES).as_mut() {
            r.remove(&self.refresh);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_load_stands_for_its_sign_in_until_the_wait_is_over() {
        let at = Instant::now();
        let failed = Failure {
            why: "needs the beta role".into(),
            at,
            token: token_key(Some("one")),
        };
        let one = token_key(Some("one"));
        assert!(failed.stands(one, RETRY_LATER, at + Duration::from_secs(10)));
        assert!(failed.stands(one, RETRY_SOON, at + Duration::from_secs(29)));
        assert!(!failed.stands(one, RETRY_SOON, at + RETRY_SOON));
        assert!(!failed.stands(one, RETRY_LATER, at + RETRY_LATER));
        assert!(!failed.stands(token_key(Some("two")), RETRY_LATER, at));
        assert!(!failed.stands(token_key(None), RETRY_LATER, at));
        assert!(RETRY_LATER >= Duration::from_secs(600));
    }

    #[test]
    fn the_local_document_carries_the_read_paths_the_fingerprint_leaves_out() {
        use super::fingerprint::{AddonPrint, Detail, Source};
        let print = |id: &str, paths: &[&str]| AddonPrint {
            id: id.into(),
            name: id.into(),
            version: semver::Version::new(1, 0, 0),
            source: Source::Built("a".repeat(64)),
            registry: None,
            inputs: None,
            order: Some(0),
            detail: Some(Detail {
                read_count: paths.len(),
                paths: paths.iter().map(|p| (*p).into()).collect(),
                ..Detail::default()
            }),
            stat_ids: BTreeMap::new(),
        };
        let game = Identity {
            build_id: "25251167".into(),
            exe_sha256: "a".repeat(64),
        };
        let f = Fingerprint::new(
            game,
            vec![print("t.cars", &["a.erp", "t.other:b.erp"]), print("t.none", &[])],
            BTreeMap::new(),
            None,
        );
        let doc = serde_json::to_value(Local::new(f, BTreeMap::new(), Vec::new())).unwrap();
        assert_eq!(doc["read_paths"], json!({ "t.cars": ["a.erp", "t.other:b.erp"] }));
        assert!(doc.pointer("/fingerprint/addons/0/detail/paths").is_none());
    }

    #[test]
    fn a_game_started_without_the_module_gets_the_last_environment_or_the_first_launch_one() {
        let data = std::env::temp_dir().join(format!("swofty-session-env-{}", std::process::id()));
        std::fs::create_dir_all(data.join("state")).unwrap();
        let started = Instant::now();
        let first = parse_launch_env(&launch_env_value(&data, Duration::from_secs(5)));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "nothing is loading, so nothing is waited for"
        );
        assert_eq!(first.len(), FIRST_LAUNCH_ENV.len());
        assert_eq!(first[0], ("SWOFTY_NETPLAY".to_owned(), "armed".to_owned()));
        std::fs::write(data.join(LAUNCH_ENV_FILE), r#"[["A","1"],["B","2"]]"#).unwrap();
        let kept = parse_launch_env(&launch_env_value(&data, Duration::ZERO));
        assert_eq!(
            kept,
            [("A".to_owned(), "1".to_owned()), ("B".to_owned(), "2".to_owned())]
        );
        std::fs::write(data.join(LAUNCH_ENV_FILE), "not json").unwrap();
        assert_eq!(parse_launch_env(&launch_env_value(&data, Duration::ZERO)), first);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn credentials_are_replaced_and_names_paths_and_digests_stay() {
        let digest = "0f".repeat(32);
        let text = format!(
            "relay user 1731800000:u1 pass Zm9vYmFyMTIzNDU2Nzg5MGFiY2RlZg== at /mnt/work/F1/launcher-todo/x.json \
             print {digest} addon swofty.seasons-2014"
        );
        let out = scrub(&text);
        assert!(!out.contains("Zm9vYmFy"), "{out}");
        assert!(
            out.contains(&digest) && out.contains("swofty.seasons-2014") && out.contains("1731800000:u1")
        );
        assert!(out.contains("/mnt/work/F1/launcher-todo/x.json"), "{out}");
        let doc = scrub_json(r#"{"turn":{"credential":"abc","username":"1:u"},"n":3}"#);
        assert!(!doc.contains("abc") && doc.contains("1:u"), "{doc}");
    }
}
