//! The game's own replay file of a lap: uploading the ones the core saves, and "Add to game".
//!
//! The core writes `pb_<unix seconds>_<lap ms>.replay` into the data folder's `tt/replays/` (or
//! `<game>/swofty/tt/replays/` when it can't write the data folder, see [`dirs`]) a few seconds after a
//! personal best, then `pb_<…>.json` with its setup once the replay is complete. [`send`] uploads a
//! pair to the lap the server took with that lap time. [`add`] puts a lap's replay into the data
//! folder's `tt/replays/inbox`, from which the core moves it into the game's Theatre.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::api::{Api, ApiError, Uploaded};
use super::queue::{self, Queue, Waiting};
use super::watch::valid_id;

pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const KEEP_SECS: u64 = 2 * 86_400;
const EARLY_SECS: u64 = 3600;
const SETUP_MAX: usize = 1024;
/// What [`add`] answers for a lap that carries no game replay.
pub const NO_REPLAY: &str = "This lap has no game replay";

pub fn dir(game: &Path) -> PathBuf {
    game.join("swofty").join("tt").join("replays")
}

/// Where the core saves its replays: the data folder, which the game can always write (it can't create
/// files in `<game>/swofty` of an EA app copy under Program Files), then the game folder.
pub fn dirs(data: &Path, game: &Path) -> [PathBuf; 2] {
    [data.join("tt").join("replays"), dir(game)]
}

pub fn inbox(data: &Path) -> PathBuf {
    data.join("tt").join("replays").join("inbox")
}

/// What [`send`] needs of the server, so its logic can run against a fake.
pub trait Replays {
    fn put_game_replay(&self, id: &str, setup: &str, file: &[u8]) -> Result<(), ApiError>;
}

impl Replays for Api {
    fn put_game_replay(&self, id: &str, setup: &str, file: &[u8]) -> Result<(), ApiError> {
        Api::put_game_replay(self, id, setup, file)
    }
}

/// A replay the core finished writing, with its setup.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub name: String,
    pub replay: PathBuf,
    pub json: PathBuf,
    pub secs: u64,
    pub lap_ms: u64,
    pub setup: Value,
}

impl Pair {
    fn remove(&self) {
        std::fs::remove_file(&self.replay).ok();
        std::fs::remove_file(&self.json).ok();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub uploaded: usize,
    pub waiting: usize,
    pub dropped: usize,
}

fn name_secs(name: &str) -> Option<u64> {
    name.strip_prefix("pb_")?.split('_').next()?.parse().ok()
}

fn modified(path: &Path) -> Option<u64> {
    let at = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(at.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
}

/// Whether the core left anything in its replay folders.
pub fn any(dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|dir| {
        std::fs::read_dir(dir).is_ok_and(|d| {
            d.filter_map(Result::ok).any(|e| {
                e.file_name().to_string_lossy().starts_with("pb_")
                    && e.path().extension().is_some_and(|x| x == "replay" || x == "json")
            })
        })
    })
}

/// The ready pairs in the core's replay folders, newest first. Halves and unreadable pairs older than two
/// days are removed.
pub fn ready(dirs: &[PathBuf], now: u64) -> Vec<Pair> {
    let mut pairs: Vec<Pair> = dirs.iter().flat_map(|dir| ready_in(dir, now)).collect();
    pairs.sort_by(|a, b| (b.secs, &b.name).cmp(&(a.secs, &a.name)));
    pairs
}

fn ready_in(dir: &Path, now: u64) -> Vec<Pair> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let names: BTreeSet<String> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "replay" || x == "json"))
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
        .filter(|n| n.starts_with("pb_"))
        .collect();
    let mut pairs = Vec::new();
    for name in names {
        let (replay, json) = (
            dir.join(format!("{name}.replay")),
            dir.join(format!("{name}.json")),
        );
        let secs = name_secs(&name)
            .or_else(|| modified(&json))
            .or_else(|| modified(&replay))
            .unwrap_or(now);
        let setup = replay
            .is_file()
            .then(|| queue::read_json::<Value>(&json))
            .flatten()
            .filter(Value::is_object);
        match setup.and_then(|s| Some((s["lap_ms"].as_u64()?, s))) {
            Some((lap_ms, setup)) => pairs.push(Pair {
                name,
                replay,
                json,
                secs,
                lap_ms,
                setup,
            }),
            None if now.saturating_sub(secs) > KEEP_SECS => {
                std::fs::remove_file(&replay).ok();
                std::fs::remove_file(&json).ok();
            }
            None => {}
        }
    }
    pairs.sort_by(|a, b| (b.secs, &b.name).cmp(&(a.secs, &a.name)));
    pairs
}

fn header(setup: &Value) -> Option<String> {
    let text = setup.to_string();
    (text.len() <= SETUP_MAX && text.bytes().all(|b| (0x20..0x7f).contains(&b))).then_some(text)
}

fn lap_of<'a>(uploaded: &'a [(u64, u64, Uploaded)], pair: &Pair) -> Option<&'a Uploaded> {
    uploaded
        .iter()
        .filter(|(at, ms, _)| *ms == pair.lap_ms && at + EARLY_SECS >= pair.secs)
        .max_by_key(|(at, _, _)| *at)
        .map(|(_, _, answer)| answer)
}

/// Why the replay of a lap must not go up. The server keeps one game replay per driver and board, that of
/// the newest lap given one, and removes the older laps' replays; a replay of a lap that is not the
/// driver's best there would take the place of the best lap's replay.
fn not_for_upload(answer: &Uploaded) -> Option<&'static str> {
    match (answer.status.as_str(), answer.best) {
        ("rejected", _) => Some("the server rejected the lap"),
        ("pending", _) => None,
        (_, Some(false)) => Some("the lap is not your best on its board"),
        _ => None,
    }
}

/// The server's answer behind an upload that lost its connection. The proxy in front of the server resets
/// the connection when the server refuses a replay before reading it, so the refusal never arrives while
/// the body is still being sent. The same request without a body gets the same refusal; a server that
/// would take the replay turns the empty body down as no replay file, and the lost connection stands.
fn answer_behind(api: &dyn Replays, id: &str, setup: &str, lost: ApiError) -> ApiError {
    if lost.status != 0 {
        return lost;
    }
    match api.put_game_replay(id, setup, &[]) {
        Err(answer) if matches!(answer.status, 403 | 404 | 409 | 429) => answer,
        _ => lost,
    }
}

/// Upload the ready pairs whose lap the server took, newest first. A pair goes once it is uploaded,
/// refused for good, too big, or two days old with no lap; one the server could not take for now stays
/// with a backoff and ends the pass.
pub fn send(
    queue: &Queue,
    dirs: &[PathBuf],
    api: &dyn Replays,
    now: u64,
    stop: &dyn Fn() -> bool,
    say: &mut dyn FnMut(&str),
) -> Report {
    let mut report = Report::default();
    let pairs = ready(dirs, now);
    let state_file = queue.replays_state();
    if pairs.is_empty() {
        std::fs::remove_file(&state_file).ok();
        return report;
    }
    let uploaded = queue.uploaded();
    let before: BTreeMap<String, Waiting> = queue::read_json(&state_file).unwrap_or_default();
    let mut state = BTreeMap::new();
    let mut taken = BTreeSet::new();
    let mut held: Option<u64> = None;
    for pair in &pairs {
        let waiting = before.get(&pair.name).cloned().unwrap_or_default();
        let mut keep = |w: Waiting| {
            if w != Waiting::default() {
                state.insert(pair.name.clone(), w);
            }
        };
        let size = std::fs::metadata(&pair.replay).map_or(0, |m| m.len());
        let setup = header(&pair.setup);
        let refusal = match (&setup, size) {
            (_, s) if s > MAX_BYTES => Some(format!("is {s} bytes, more than the server takes")),
            (None, _) => Some("has a setup the server cannot take".to_owned()),
            _ => None,
        };
        if let Some(why) = refusal {
            say(&format!("game replay {} {why}; removed", pair.name));
            pair.remove();
            report.dropped += 1;
            continue;
        }
        let Some(answer) = lap_of(&uploaded, pair) else {
            if now.saturating_sub(pair.secs) > KEEP_SECS {
                pair.remove();
                report.dropped += 1;
            } else {
                keep(waiting);
                report.waiting += 1;
            }
            continue;
        };
        let id = answer.id.clone();
        if let Some(why) = not_for_upload(answer) {
            say(&format!(
                "game replay {} of lap {id} not uploaded: {why}; removed",
                pair.name
            ));
            pair.remove();
            report.dropped += 1;
            continue;
        }
        if !taken.insert(id.clone()) {
            pair.remove();
            report.dropped += 1;
            continue;
        }
        if let Some(until) = held {
            keep(Waiting {
                next_try: waiting.next_try.max(until),
                ..waiting
            });
            report.waiting += 1;
            continue;
        }
        if waiting.next_try > now || stop() {
            keep(waiting);
            report.waiting += 1;
            continue;
        }
        let setup = setup.as_deref().unwrap_or_default();
        let sent = std::fs::read(&pair.replay)
            .map_err(|e| ApiError::network(format!("{}: {e}", pair.replay.display())))
            .and_then(|bytes| {
                api.put_game_replay(&id, setup, &bytes)
                    .map_err(|e| answer_behind(api, &id, setup, e))
            });
        match sent {
            Ok(()) => {
                say(&format!("uploaded the game replay of lap {id} ({size} bytes)"));
                pair.remove();
                report.uploaded += 1;
            }
            Err(e) if e.transient() => {
                let attempts = waiting.attempts + 1;
                let next_try = now + e.retry_after.unwrap_or_else(|| queue::backoff(attempts));
                say(&format!("game replay upload failed, will retry: {}", e.message));
                held = Some(next_try);
                keep(Waiting {
                    attempts,
                    next_try,
                    last_error: Some(e.message),
                    ..waiting
                });
                report.waiting += 1;
            }
            Err(e) => {
                say(&format!(
                    "game replay of lap {id} not uploaded: {}; removed",
                    e.message
                ));
                pair.remove();
                report.dropped += 1;
            }
        }
    }
    if state.is_empty() {
        std::fs::remove_file(&state_file).ok();
    } else if state != before {
        queue::write_json(&state_file, &state).ok();
    }
    report
}

/// Write a lap's replay and its setup into the inbox, the setup last.
pub fn place(data: &Path, id: &str, setup: &Value, replay: &[u8]) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err("not a lap id".to_owned());
    }
    if !setup.is_object() {
        return Err("the lap's game replay has no setup".to_owned());
    }
    let dir = inbox(data);
    let say = |what: &str, e: std::io::Error| format!("couldn't {what} in {}: {e}", dir.display());
    std::fs::create_dir_all(&dir).map_err(|e| say("make the folder", e))?;
    let put = |name: String, bytes: &[u8], what: &str| {
        let (tmp, to) = (dir.join(format!("{name}.tmp")), dir.join(&name));
        std::fs::write(&tmp, bytes)
            .and_then(|()| std::fs::rename(&tmp, &to))
            .map_err(|e| {
                std::fs::remove_file(&tmp).ok();
                say(what, e)
            })?;
        Ok::<PathBuf, String>(to)
    };
    put(format!("tt_{id}.frr"), replay, "write the replay")?;
    put(
        format!("tt_{id}.frr.json"),
        setup.to_string().as_bytes(),
        "write the replay's setup",
    )
}

/// Download the game replay of lap `id` and put it in the inbox of the data folder `data`, where the
/// core picks it up.
pub fn add(api: &Api, data: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("not a lap id".to_owned());
    }
    let none = || NO_REPLAY.to_owned();
    let meta: Value = api
        .get_json(&format!("/v1/tt/laps/{id}"))
        .map_err(|e| e.message)?;
    let setup = &meta["game_replay"]["setup"];
    if !setup.is_object() {
        return Err(none());
    }
    let replay = api.game_replay(id).map_err(|e| match e.status {
        404 => none(),
        _ => e.message,
    })?;
    if replay.is_empty() {
        return Err(none());
    }
    place(data, id, setup, &replay).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;
    use crate::timetrial::api::{Boards, Meta, Uploaded};
    use crate::timetrial::queue::{Folder, Limits};
    use crate::timetrial::swlap::sample;

    struct Root(PathBuf);

    impl Root {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("swofty-ttr-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(dir.join("game")).unwrap();
            Self(dir)
        }

        fn game(&self) -> PathBuf {
            self.0.join("game")
        }

        fn queue(&self) -> Queue {
            Queue::new(&self.0.join("data"))
        }

        fn data(&self) -> PathBuf {
            self.0.join("data")
        }

        fn dirs(&self) -> [PathBuf; 2] {
            dirs(&self.data(), &self.game())
        }

        /// Record a lap of `ms` as uploaded at `now` with the server's id `id`.
        fn lap(&self, name: &str, ms: u64, id: &str, now: u64) {
            self.lap_as(name, ms, answer(id, "ranked", Some(true)), now);
        }

        /// Record a lap of `ms` as uploaded at `now` with the server's `answer`.
        fn lap_as(&self, name: &str, ms: u64, answer: Uploaded, now: u64) {
            let laps = self.0.join("laps");
            std::fs::create_dir_all(&laps).unwrap();
            std::fs::write(laps.join(name), sample(ms, answer.status != "rejected")).unwrap();
            let q = self.queue();
            q.collect(&laps, None).unwrap();
            let r = q.drain(&Answer(answer), &Value::Null, Limits::at(now), &mut |_| {});
            assert_eq!(r.uploaded, 1);
        }

        fn pair(&self, secs: u64, ms: u64, bytes: &[u8]) -> PathBuf {
            self.pair_in(&dir(&self.game()), secs, ms, bytes)
        }

        fn pair_in(&self, dir: &Path, secs: u64, ms: u64, bytes: &[u8]) -> PathBuf {
            std::fs::create_dir_all(dir).unwrap();
            let replay = dir.join(format!("pb_{secs}_{ms}.replay"));
            std::fs::write(&replay, bytes).unwrap();
            std::fs::write(replay.with_extension("json"), setup(ms).to_string()).unwrap();
            replay
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn setup(ms: u64) -> Value {
        json!({"mode": "tt", "box": "f1_2025", "track": "monaco", "team": "mclaren", "lap_ms": ms, "tail_ms": 1500})
    }

    fn answer(id: &str, status: &str, best: Option<bool>) -> Uploaded {
        Uploaded {
            id: id.to_owned(),
            status: status.to_owned(),
            rank: best.map(|_| 1),
            best,
            total: None,
            previous_best_ms: None,
            reasons: vec![],
            url: None,
        }
    }

    struct Answer(Uploaded);

    impl Boards for Answer {
        fn upload(&self, _: &[u8], _: &Meta) -> Result<Uploaded, ApiError> {
            Ok(self.0.clone())
        }
    }

    #[derive(Default)]
    struct Fake {
        script: RefCell<Vec<Result<(), ApiError>>>,
        seen: RefCell<Vec<(String, String, Vec<u8>)>>,
    }

    impl Fake {
        fn new(script: Vec<Result<(), ApiError>>) -> Self {
            Self {
                script: RefCell::new(script),
                seen: RefCell::default(),
            }
        }
    }

    impl Replays for Fake {
        fn put_game_replay(&self, id: &str, setup: &str, file: &[u8]) -> Result<(), ApiError> {
            self.seen
                .borrow_mut()
                .push((id.to_owned(), setup.to_owned(), file.to_vec()));
            self.script.borrow_mut().remove(0)
        }
    }

    fn pass(root: &Root, api: &Fake, now: u64) -> (Report, Vec<String>) {
        let mut lines = Vec::new();
        let r = send(&root.queue(), &root.dirs(), api, now, &|| false, &mut |l| {
            lines.push(l.to_owned())
        });
        (r, lines)
    }

    #[test]
    fn a_pair_waits_for_its_lap_and_goes_once_uploaded() {
        let root = Root::new("match");
        let replay = root.pair(1000, 91000, b"FRR");
        let (r, _) = pass(&root, &Fake::default(), 1010);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (0, 1, 0));
        assert!(replay.exists());

        root.lap("a.swlap", 92000, "00000000000000aa", 1020);
        root.lap("b.swlap", 91000, "00000000000000bb", 1021);
        let api = Fake::new(vec![Ok(())]);
        let (r, lines) = pass(&root, &api, 1030);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (1, 0, 0));
        let seen = api.seen.borrow();
        assert_eq!(seen[0].0, "00000000000000bb");
        assert_eq!(serde_json::from_str::<Value>(&seen[0].1).unwrap(), setup(91000));
        assert!(!seen[0].1.contains(' '));
        assert_eq!(seen[0].2, b"FRR");
        assert!(lines[0].contains("00000000000000bb"));
        assert!(!replay.exists() && !replay.with_extension("json").exists());
        assert_eq!(root.queue().list(Folder::Sent).len(), 2);
    }

    #[test]
    fn a_pair_in_the_data_folder_goes_beside_one_in_the_game_folder() {
        let root = Root::new("both");
        let [new, _] = root.dirs();
        let fresh = root.pair_in(&new, 1001, 91000, b"NEW");
        let old = root.pair(1000, 92000, b"OLD");
        assert!(any(&root.dirs()));
        let pairs = ready(&root.dirs(), 1010);
        assert_eq!(pairs.iter().map(|p| p.lap_ms).collect::<Vec<_>>(), [91000, 92000]);
        std::fs::remove_file(&old).ok();
        std::fs::remove_file(old.with_extension("json")).ok();
        root.lap("b.swlap", 91000, "00000000000000bb", 1021);
        let api = Fake::new(vec![Ok(())]);
        let (r, _) = pass(&root, &api, 1030);
        assert_eq!(r.uploaded, 1);
        assert_eq!(api.seen.borrow()[0].2, b"NEW");
        assert!(!fresh.exists());
    }

    #[test]
    fn a_replay_without_its_setup_is_not_ready() {
        let root = Root::new("half");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        let replay = root.pair(1000, 91000, b"FRR");
        std::fs::remove_file(replay.with_extension("json")).unwrap();
        assert!(any(&root.dirs()));
        let (r, _) = pass(&root, &Fake::default(), 1010);
        assert_eq!(r, Report::default());
        assert!(replay.exists());
        std::fs::write(replay.with_extension("json"), "{\"lap_ms\":91").unwrap();
        assert!(ready(&root.dirs(), 1010).is_empty());
        assert!(replay.exists());
        assert!(ready(&root.dirs(), 1000 + KEEP_SECS + 1).is_empty());
        assert!(!replay.exists() && !replay.with_extension("json").exists());
    }

    #[test]
    fn a_failure_keeps_the_pair_with_a_backoff_across_restarts() {
        let root = Root::new("retry");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        root.lap("b.swlap", 92000, "00000000000000bb", 1000);
        let old = root.pair(1001, 91000, b"A");
        let new = root.pair(1002, 92000, b"B");
        let api = Fake::new(vec![
            Err(ApiError::network("down")),
            Err(ApiError::network("down")),
        ]);
        let (r, lines) = pass(&root, &api, 1010);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (0, 2, 0));
        assert_eq!(api.seen.borrow()[0].0, "00000000000000bb");
        assert!(api.seen.borrow()[1].2.is_empty());
        assert!(lines[0].contains("will retry"));
        let state: BTreeMap<String, Waiting> = queue::read_json(&root.queue().replays_state()).unwrap();
        assert_eq!(
            (state["pb_1002_92000"].attempts, state["pb_1002_92000"].next_try),
            (1, 1040)
        );
        assert_eq!(state["pb_1001_91000"].next_try, 1040);

        let (r, _) = pass(&root, &Fake::default(), 1020);
        assert_eq!((r.uploaded, r.waiting), (0, 2));

        let api = Fake::new(vec![Err(ApiError::from_body(503, ""))]);
        pass(&root, &api, 1041);
        let state: BTreeMap<String, Waiting> = queue::read_json(&root.queue().replays_state()).unwrap();
        assert_eq!(
            (state["pb_1002_92000"].attempts, state["pb_1002_92000"].next_try),
            (2, 1041 + 60)
        );

        let api = Fake::new(vec![Ok(()), Ok(())]);
        let (r, _) = pass(&root, &api, 5000);
        assert_eq!(r.uploaded, 2);
        assert!(!old.exists() && !new.exists());
        assert!(!root.queue().replays_state().exists());
    }

    #[test]
    fn a_permanent_refusal_removes_the_pair_and_the_rest_go_on() {
        let root = Root::new("refused");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        root.lap("b.swlap", 92000, "00000000000000bb", 1000);
        let old = root.pair(1001, 91000, b"A");
        let new = root.pair(1002, 92000, b"B");
        for status in [400, 403, 404, 409, 413] {
            assert!(!ApiError::from_body(status, "").transient());
        }
        for status in [429, 500, 507] {
            assert!(ApiError::from_body(status, "").transient());
        }
        let api = Fake::new(vec![Err(ApiError::from_body(404, "")), Ok(())]);
        let (r, lines) = pass(&root, &api, 1010);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (1, 0, 1));
        assert!(lines[0].contains("not uploaded"));
        assert!(!old.exists() && !new.exists() && !new.with_extension("json").exists());
    }

    fn not_a_replay() -> ApiError {
        ApiError::from_body(400, r#"{"error": "the body is not a game replay file"}"#)
    }

    #[test]
    fn a_refusal_lost_with_the_connection_is_asked_for_again_without_the_body() {
        let root = Root::new("lost");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        root.lap("b.swlap", 92000, "00000000000000bb", 1000);
        let old = root.pair(1001, 91000, b"A");
        let new = root.pair(1002, 92000, b"B");
        let refusal = ApiError::from_body(409, r#"{"error": "this lap is not your best on this board"}"#);
        let api = Fake::new(vec![Err(ApiError::network("aborted")), Err(refusal), Ok(())]);
        let (r, lines) = pass(&root, &api, 1010);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (1, 0, 1));
        let seen = api.seen.borrow();
        assert_eq!(
            seen.iter().map(|s| (s.0.as_str(), s.2.len())).collect::<Vec<_>>(),
            [
                ("00000000000000bb", 1),
                ("00000000000000bb", 0),
                ("00000000000000aa", 1)
            ]
        );
        assert!(lines[0].contains("not uploaded") && lines[0].contains("not your best"));
        assert!(!old.exists() && !new.exists());
    }

    #[test]
    fn a_lost_rate_limit_is_waited_out_and_a_real_network_failure_stands() {
        let root = Root::new("lost429");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        let replay = root.pair(1001, 91000, b"A");
        let mut busy = ApiError::from_body(429, "");
        busy.retry_after = Some(45);
        let api = Fake::new(vec![Err(ApiError::network("aborted")), Err(busy)]);
        let (r, _) = pass(&root, &api, 1010);
        assert_eq!(r.waiting, 1);
        let state: BTreeMap<String, Waiting> = queue::read_json(&root.queue().replays_state()).unwrap();
        assert_eq!(state["pb_1001_91000"].next_try, 1055);

        let api = Fake::new(vec![Err(ApiError::network("aborted")), Err(not_a_replay())]);
        let (r, lines) = pass(&root, &api, 1060);
        assert_eq!(r.waiting, 1);
        assert!(lines[0].contains("will retry: aborted"));
        assert!(replay.exists());

        let api = Fake::new(vec![Err(ApiError::from_body(503, ""))]);
        pass(&root, &api, 5000);
        assert_eq!(api.seen.borrow().len(), 1);
    }

    #[test]
    fn a_file_over_the_limit_is_dropped_without_a_request() {
        let root = Root::new("big");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        let replay = root.pair(1001, 91000, b"");
        std::fs::File::create(&replay)
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        let api = Fake::default();
        let (r, lines) = pass(&root, &api, 1010);
        assert_eq!((r.uploaded, r.dropped, lines.len()), (0, 1, 1));
        assert!(api.seen.borrow().is_empty());
        assert!(!replay.exists() && !replay.with_extension("json").exists());
        let (_, lines) = pass(&root, &api, 1020);
        assert!(lines.is_empty());
    }

    #[test]
    fn a_pair_with_no_lap_goes_after_two_days() {
        let root = Root::new("stale");
        root.lap("a.swlap", 91000, "00000000000000aa", 10);
        let early = root.pair(5000, 91000, b"A");
        let other = root.pair(5000, 77000, b"B");
        let (r, _) = pass(&root, &Fake::default(), 5000 + KEEP_SECS);
        assert_eq!((r.waiting, r.dropped), (2, 0));
        let (r, _) = pass(&root, &Fake::default(), 5001 + KEEP_SECS);
        assert_eq!((r.waiting, r.dropped), (0, 2));
        assert!(!early.exists() && !other.exists());
    }

    #[test]
    fn two_pairs_of_one_lap_send_the_newest() {
        let root = Root::new("twice");
        root.lap("a.swlap", 91000, "00000000000000aa", 1000);
        let old = root.pair(1001, 91000, b"old");
        root.pair(1002, 91000, b"new");
        let api = Fake::new(vec![Ok(())]);
        let (r, _) = pass(&root, &api, 1010);
        assert_eq!((r.uploaded, r.dropped), (1, 1));
        assert_eq!(api.seen.borrow()[0].2, b"new");
        assert!(!old.exists());
    }

    #[test]
    fn a_rejected_lap_or_one_that_is_no_best_never_takes_the_best_laps_replay() {
        let root = Root::new("notbest");
        root.lap_as(
            "a.swlap",
            87599,
            answer("00000000000000aa", "accepted", Some(true)),
            1000,
        );
        root.pair(1001, 87599, b"BEST");
        let api = Fake::new(vec![Ok(())]);
        let (r, _) = pass(&root, &api, 1005);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (1, 0, 0));
        assert_eq!(api.seen.borrow()[0].0, "00000000000000aa");

        root.lap_as(
            "b.swlap",
            35875,
            answer("00000000000000bb", "rejected", Some(false)),
            1180,
        );
        let bogus = root.pair(1179, 35875, b"CLOCK LAP");
        root.lap_as(
            "c.swlap",
            88000,
            answer("00000000000000cc", "accepted", Some(false)),
            1300,
        );
        let slower = root.pair(1299, 88000, b"SLOWER");
        let api = Fake::default();
        let (r, lines) = pass(&root, &api, 1310);
        assert_eq!((r.uploaded, r.waiting, r.dropped), (0, 0, 2));
        assert!(api.seen.borrow().is_empty());
        assert!(!bogus.exists() && !slower.exists());
        assert!(
            lines
                .iter()
                .any(|l| l.contains("00000000000000bb") && l.contains("rejected"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("00000000000000cc") && l.contains("not your best"))
        );

        root.lap_as(
            "d.swlap",
            87100,
            answer("00000000000000dd", "pending", Some(false)),
            1400,
        );
        root.pair(1399, 87100, b"HELD");
        let api = Fake::new(vec![Ok(())]);
        let (r, _) = pass(&root, &api, 1410);
        assert_eq!(r.uploaded, 1);
        assert_eq!(api.seen.borrow()[0].0, "00000000000000dd");
    }

    #[test]
    fn add_to_game_writes_the_pair_into_the_inbox() {
        let root = Root::new("inbox");
        let json = place(&root.data(), "2949b7d7eaf78422", &setup(91000), b"FRR bytes").unwrap();
        let inbox = inbox(&root.data());
        assert_eq!(inbox, root.dirs()[0].join("inbox"));
        assert_eq!(json, inbox.join("tt_2949b7d7eaf78422.frr.json"));
        assert_eq!(
            std::fs::read(inbox.join("tt_2949b7d7eaf78422.frr")).unwrap(),
            b"FRR bytes"
        );
        let doc: Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
        assert_eq!(doc, setup(91000));
        assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 2);
        assert!(ready(&root.dirs(), 1).is_empty());
    }

    #[test]
    fn a_bad_id_or_a_missing_setup_writes_nothing() {
        let root = Root::new("bad");
        assert!(place(&root.data(), "../../etc/passwd", &setup(1), b"x").is_err());
        assert!(place(&root.data(), "2949b7d7eaf78422", &Value::Null, b"x").is_err());
        assert!(!inbox(&root.data()).exists());
    }
}
