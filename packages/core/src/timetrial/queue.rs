//! The laps waiting to be uploaded, and the ones that were: `<data>/timetrial/{outbox,sent,rejected}`.
//!
//! A lap is moved out of the game folder into the outbox as soon as the wrapper sees it, so a rollback
//! or a game-folder cleanup never loses it. It is uploaded oldest first, then moved to `sent` with the
//! server's answer next to it as `<name>.json`. A lap that cannot be uploaded yet stays in the outbox
//! with its retry count and backoff in the same sidecar.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::api::{ApiError, Boards, Meta, Uploaded};
use super::swlap::{self, Board, Header};

pub const EXT: &str = "swlap";
pub const SENT_KEEP: usize = 200;
pub const KEEP_SECS: u64 = 30 * 86_400;
pub const OUTBOX_SECS: u64 = 14 * 86_400;
const BACKOFF_FIRST: u64 = 30;
const BACKOFF_MAX: u64 = 3600;

/// What the outbox sidecar of a lap holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Waiting {
    /// The addon set the lap was recorded with, taken when the lap was collected.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub fingerprint: Value,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub next_try: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// What the sidecar of a lap in `sent` or `rejected` holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Done {
    pub at: u64,
    pub content: String,
    pub track: String,
    pub weather: String,
    pub time_ms: u64,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<Uploaded>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Done {
    fn of(header: Option<&Header>, at: u64) -> Self {
        let h = header.cloned().unwrap_or_default();
        Self {
            at,
            content: h.board.content,
            track: h.board.track,
            weather: h.board.weather,
            time_ms: h.lap.time_ms,
            valid: h.lap.valid,
            answer: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Outbox,
    Sent,
    Rejected,
}

impl Folder {
    fn name(self) -> &'static str {
        match self {
            Self::Outbox => "outbox",
            Self::Sent => "sent",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub folder: Folder,
    pub path: PathBuf,
    pub header: Option<Header>,
    pub waiting: Option<Waiting>,
    pub done: Option<Done>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Uploaded {
        name: String,
        board: Box<Board>,
        time_ms: u64,
        answer: Uploaded,
    },
    Retry {
        name: String,
        error: String,
    },
    Rejected {
        name: String,
        error: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub uploaded: usize,
    pub waiting: usize,
    pub rejected: usize,
}

/// How far a drain goes: `now` in unix seconds, and whether it may stop sending at `deadline`.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub now: u64,
    pub deadline: Option<std::time::Instant>,
    /// Send laps whose backoff has not run out, for a lap the player asked to send.
    pub ignore_backoff: bool,
}

impl Limits {
    pub fn at(now: u64) -> Self {
        Self {
            now,
            deadline: None,
            ignore_backoff: false,
        }
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub struct Queue {
    root: PathBuf,
}

impl Queue {
    pub fn new(data: &Path) -> Self {
        Self {
            root: data.join("timetrial"),
        }
    }

    pub fn dir(&self, folder: Folder) -> PathBuf {
        self.root.join(folder.name())
    }

    /// Move every `.swlap` in the game's lap folder into the outbox, with `fingerprint` as what the
    /// laps were recorded with. Returns how many moved.
    pub fn collect(&self, laps: &Path, fingerprint: Option<&Value>) -> std::io::Result<usize> {
        let Ok(read) = std::fs::read_dir(laps) else {
            return Ok(0);
        };
        let outbox = self.dir(Folder::Outbox);
        let mut moved = 0;
        let mut names: Vec<PathBuf> = read
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == EXT) && p.is_file())
            .collect();
        names.sort();
        for from in names {
            std::fs::create_dir_all(&outbox)?;
            let to = free_name(&outbox, from.file_name().expect("file"));
            if move_file(&from, &to).is_err() {
                continue;
            }
            if let Some(fp) = fingerprint {
                let waiting = Waiting {
                    fingerprint: fp.clone(),
                    ..Waiting::default()
                };
                write_json(&to.with_extension("json"), &waiting)?;
            }
            moved += 1;
        }
        Ok(moved)
    }

    /// The laps in `folder`, oldest first.
    pub fn list(&self, folder: Folder) -> Vec<Entry> {
        let Ok(read) = std::fs::read_dir(self.dir(folder)) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = read
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == EXT))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let side = path.with_extension("json");
                Entry {
                    folder,
                    header: swlap::read(&path).ok(),
                    waiting: (folder == Folder::Outbox)
                        .then(|| read_json::<Waiting>(&side).unwrap_or_default()),
                    done: (folder != Folder::Outbox)
                        .then(|| read_json::<Done>(&side))
                        .flatten(),
                    path,
                }
            })
            .collect()
    }

    /// The laps the server took, as `(uploaded at, lap time in ms, the server's answer)`.
    pub fn uploaded(&self) -> Vec<(u64, u64, Uploaded)> {
        let Ok(read) = std::fs::read_dir(self.dir(Folder::Sent)) else {
            return Vec::new();
        };
        read.filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| read_json::<Done>(&p))
            .filter_map(|d| Some((d.at, d.time_ms, d.answer?)))
            .collect()
    }

    /// Where the retry state of the game replays waiting to upload is kept.
    pub fn replays_state(&self) -> PathBuf {
        self.root.join("replays.json")
    }

    /// Upload the outbox, oldest first. A refusal of one lap moves it to `rejected` and goes on; a lap
    /// the server could not take for now (unreachable, busy, signed out) stays queued and ends the
    /// drain, since the next lap would meet the same answer.
    pub fn drain(
        &self,
        api: &dyn Boards,
        fallback_fingerprint: &Value,
        limits: Limits,
        on: &mut dyn FnMut(Event),
    ) -> Report {
        let mut report = Report::default();
        let entries = self.list(Folder::Outbox);
        let total = entries.len();
        for (i, entry) in entries.iter().enumerate() {
            if limits.deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                report.waiting += total - i;
                break;
            }
            let name = file_name(&entry.path);
            let waiting = entry.waiting.clone().unwrap_or_default();
            if !limits.ignore_backoff && waiting.next_try > limits.now {
                report.waiting += 1;
                continue;
            }
            let bytes = match std::fs::read(&entry.path) {
                Ok(b) => b,
                Err(_) => {
                    report.waiting += 1;
                    continue;
                }
            };
            let header = match swlap::parse(&bytes) {
                Ok(h) => h,
                Err(e) => {
                    self.settle(
                        &entry.path,
                        Folder::Rejected,
                        Done {
                            error: Some(e.to_string()),
                            ..Done::of(None, limits.now)
                        },
                    );
                    on(Event::Rejected {
                        name,
                        error: e.to_string(),
                    });
                    report.rejected += 1;
                    continue;
                }
            };
            let fingerprint = if waiting.fingerprint.is_null() {
                fallback_fingerprint.clone()
            } else {
                waiting.fingerprint.clone()
            };
            let meta = Meta::new(header.nonce.clone(), fingerprint);
            match api.upload(&bytes, &meta) {
                Ok(answer) => {
                    let mut done = Done::of(Some(&header), limits.now);
                    done.answer = Some(answer.clone());
                    self.settle(&entry.path, Folder::Sent, done);
                    on(Event::Uploaded {
                        name,
                        board: Box::new(header.board.clone()),
                        time_ms: header.lap.time_ms,
                        answer,
                    });
                    report.uploaded += 1;
                }
                Err(e) if e.transient() => {
                    let attempts = waiting.attempts + 1;
                    let wait = e.retry_after.unwrap_or_else(|| backoff(attempts));
                    let next = Waiting {
                        attempts,
                        next_try: limits.now + wait,
                        last_error: Some(e.message.clone()),
                        ..waiting
                    };
                    for later in &entries[i + 1..] {
                        let w = Waiting {
                            next_try: next.next_try,
                            ..later.waiting.clone().unwrap_or_default()
                        };
                        write_json(&later.path.with_extension("json"), &w).ok();
                    }
                    write_json(&entry.path.with_extension("json"), &next).ok();
                    on(Event::Retry {
                        name,
                        error: e.message,
                    });
                    report.waiting += total - i;
                    break;
                }
                Err(e) => {
                    let mut done = Done::of(Some(&header), limits.now);
                    done.error = Some(e.message.clone());
                    self.settle(&entry.path, Folder::Rejected, done);
                    on(Event::Rejected {
                        name,
                        error: e.message,
                    });
                    report.rejected += 1;
                }
            }
        }
        report
    }

    /// Upload one file the player named, whether or not it is in the outbox. Returns the answer; the
    /// file moves to `sent` on success.
    pub fn send_one(
        &self,
        file: &Path,
        api: &dyn Boards,
        fingerprint: &Value,
        now: u64,
    ) -> Result<Uploaded, ApiError> {
        let bytes = std::fs::read(file).map_err(|e| ApiError::network(format!("{}: {e}", file.display())))?;
        let header = swlap::parse(&bytes).map_err(|e| ApiError {
            status: 400,
            message: e.to_string(),
            retry_after: None,
        })?;
        let answer = api.upload(&bytes, &Meta::new(header.nonce.clone(), fingerprint.clone()))?;
        let mut done = Done::of(Some(&header), now);
        done.answer = Some(answer.clone());
        let inside = file.parent().is_some_and(|p| p == self.dir(Folder::Outbox));
        if inside {
            self.settle(file, Folder::Sent, done);
        } else if let Ok(bytes) = std::fs::read(file) {
            let dir = self.dir(Folder::Sent);
            std::fs::create_dir_all(&dir).ok();
            let to = free_name(&dir, file.file_name().expect("file"));
            if std::fs::write(&to, bytes).is_ok() {
                write_json(&to.with_extension("json"), &done).ok();
            }
        }
        Ok(answer)
    }

    fn settle(&self, from: &Path, to: Folder, done: Done) {
        let dir = self.dir(to);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let target = free_name(&dir, from.file_name().expect("file"));
        if move_file(from, &target).is_ok() {
            write_json(&target.with_extension("json"), &done).ok();
            std::fs::remove_file(from.with_extension("json")).ok();
        }
    }

    /// Delete what has outlived its keep: outbox laps after 14 days, `sent` and `rejected` laps after 30
    /// days or beyond the newest 200.
    pub fn prune(&self, now: u64) {
        for e in self.list(Folder::Outbox) {
            if age(&e.path, now) > OUTBOX_SECS {
                remove_lap(&e.path);
            }
        }
        for folder in [Folder::Sent, Folder::Rejected] {
            let mut entries = self.list(folder);
            entries.reverse();
            for (i, e) in entries.iter().enumerate() {
                if i >= SENT_KEEP || age(&e.path, now) > KEEP_SECS {
                    remove_lap(&e.path);
                }
            }
        }
    }
}

pub(super) fn backoff(attempts: u32) -> u64 {
    BACKOFF_FIRST
        .saturating_mul(1u64 << attempts.saturating_sub(1).min(10))
        .min(BACKOFF_MAX)
}

fn age(path: &Path, now: u64) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |t| now.saturating_sub(t.as_secs()))
}

fn remove_lap(path: &Path) {
    std::fs::remove_file(path).ok();
    std::fs::remove_file(path.with_extension("json")).ok();
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn free_name(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let stem = Path::new(name)
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    (1..)
        .map(|n| dir.join(format!("{stem}-{n}.{EXT}")))
        .find(|p| !p.exists())
        .expect("a free name")
}

fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

pub(super) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value).unwrap_or_default())?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;
    use crate::timetrial::swlap::sample;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-ttq-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    struct Fake {
        script: RefCell<Vec<Result<Uploaded, ApiError>>>,
        seen: RefCell<Vec<Meta>>,
    }

    impl Fake {
        fn new(script: Vec<Result<Uploaded, ApiError>>) -> Self {
            Self {
                script: RefCell::new(script),
                seen: RefCell::default(),
            }
        }
    }

    impl Boards for Fake {
        fn upload(&self, _: &[u8], meta: &Meta) -> Result<Uploaded, ApiError> {
            self.seen.borrow_mut().push(meta.clone());
            self.script.borrow_mut().remove(0)
        }
    }

    fn ok(id: &str, rank: u32) -> Result<Uploaded, ApiError> {
        Ok(Uploaded {
            id: id.into(),
            status: "ranked".into(),
            rank: Some(rank),
            best: Some(true),
            total: None,
            previous_best_ms: None,
            reasons: vec![],
            url: None,
        })
    }

    fn lap(laps: &Path, name: &str, ms: u64) {
        std::fs::create_dir_all(laps).unwrap();
        std::fs::write(laps.join(name), sample(ms, true)).unwrap();
    }

    fn drain(q: &Queue, api: &Fake, now: u64) -> (Report, Vec<Event>) {
        let mut events = Vec::new();
        let r = q.drain(api, &Value::Null, Limits::at(now), &mut |e| events.push(e));
        (r, events)
    }

    #[test]
    fn moves_laps_out_of_the_game_folder_and_sends_them_oldest_first() {
        let root = dir("order");
        let (laps, q) = (root.join("game/swofty/laps"), Queue::new(&root.join("data")));
        lap(&laps, "20261110T041011-2.swlap", 92000);
        lap(&laps, "20261110T041011-1.swlap", 91000);
        std::fs::write(laps.join("half.swlap.tmp"), b"x").unwrap();
        assert_eq!(q.collect(&laps, Some(&json!({"digest": "d"}))).unwrap(), 2);
        assert!(laps.join("half.swlap.tmp").exists());
        assert!(!laps.join("20261110T041011-1.swlap").exists());

        let api = Fake::new(vec![ok("a", 3), ok("b", 2)]);
        let (r, events) = drain(&q, &api, 1000);
        assert_eq!((r.uploaded, r.waiting, r.rejected), (2, 0, 0));
        let times: Vec<u64> = events
            .iter()
            .map(|e| match e {
                Event::Uploaded { time_ms, .. } => *time_ms,
                _ => 0,
            })
            .collect();
        assert_eq!(times, [91000, 92000]);
        assert_eq!(api.seen.borrow()[0].fingerprint, json!({"digest": "d"}));
        assert!(q.list(Folder::Outbox).is_empty());
        let sent = q.list(Folder::Sent);
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].done.as_ref().unwrap().answer.as_ref().unwrap().id, "a");
        assert_eq!(sent[0].done.as_ref().unwrap().time_ms, 91000);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_failure_keeps_the_lap_with_a_backoff_and_stops_the_drain() {
        let root = dir("retry");
        let (laps, q) = (root.join("laps"), Queue::new(&root.join("data")));
        lap(&laps, "a.swlap", 1);
        lap(&laps, "b.swlap", 2);
        q.collect(&laps, None).unwrap();

        let api = Fake::new(vec![Err(ApiError::network("down"))]);
        let (r, events) = drain(&q, &api, 1000);
        assert_eq!((r.uploaded, r.waiting), (0, 2));
        assert!(matches!(events[..], [Event::Retry { .. }]));
        let first = &q.list(Folder::Outbox)[0];
        let w = first.waiting.clone().unwrap();
        assert_eq!((w.attempts, w.next_try), (1, 1030));

        let (r, _) = drain(&q, &Fake::new(vec![]), 1010);
        assert_eq!((r.uploaded, r.waiting), (0, 2));

        let api = Fake::new(vec![Err(ApiError::from_body(503, ""))]);
        let (_, _) = drain(&q, &api, 1031);
        let w = q.list(Folder::Outbox)[0].waiting.clone().unwrap();
        assert_eq!((w.attempts, w.next_try), (2, 1031 + 60));

        let api = Fake::new(vec![ok("a", 1), ok("b", 2)]);
        let (r, _) = drain(&q, &api, 2000);
        assert_eq!(r.uploaded, 2);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_rate_limit_waits_as_long_as_the_server_says() {
        let root = dir("limit");
        let (laps, q) = (root.join("laps"), Queue::new(&root.join("data")));
        lap(&laps, "a.swlap", 1);
        q.collect(&laps, None).unwrap();
        let api = Fake::new(vec![Err(ApiError::from_body(
            429,
            r#"{"error":"slow","retry_after":120}"#,
        ))]);
        drain(&q, &api, 500);
        assert_eq!(q.list(Folder::Outbox)[0].waiting.as_ref().unwrap().next_try, 620);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_refused_or_unreadable_lap_goes_to_rejected_and_the_rest_go_on() {
        let root = dir("reject");
        let (laps, q) = (root.join("laps"), Queue::new(&root.join("data")));
        lap(&laps, "a.swlap", 1);
        std::fs::write(laps.join("b.swlap"), b"junk junk junk junk").unwrap();
        lap(&laps, "c.swlap", 3);
        q.collect(&laps, None).unwrap();
        let api = Fake::new(vec![
            Err(ApiError::from_body(400, r#"{"error":"bad lap"}"#)),
            ok("c", 9),
        ]);
        let (r, events) = drain(&q, &api, 10);
        assert_eq!((r.uploaded, r.rejected), (1, 2));
        assert_eq!(events.len(), 3);
        let rejected = q.list(Folder::Rejected);
        assert_eq!(rejected.len(), 2);
        assert_eq!(
            rejected[0].done.as_ref().unwrap().error.as_deref(),
            Some("bad lap")
        );
        assert!(
            rejected[1]
                .done
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("not a lap")
        );
        assert_eq!(q.list(Folder::Sent).len(), 1);
        assert!(q.list(Folder::Outbox).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_header_nonce_travels_as_the_session() {
        let root = dir("nonce");
        let (laps, q) = (root.join("laps"), Queue::new(&root.join("data")));
        std::fs::create_dir_all(&laps).unwrap();
        let header = json!({"format": 1, "nonce": "abc", "lap": {"time_ms": 5, "valid": true}}).to_string();
        let mut bytes = swlap::MAGIC.to_vec();
        bytes.extend((header.len() as u32).to_le_bytes());
        bytes.extend(header.as_bytes());
        std::fs::write(laps.join("a.swlap"), bytes).unwrap();
        q.collect(&laps, None).unwrap();
        let api = Fake::new(vec![ok("a", 1)]);
        drain(&q, &api, 1);
        assert_eq!(api.seen.borrow()[0].session.as_deref(), Some("abc"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_deadline_leaves_the_rest_queued() {
        let root = dir("deadline");
        let (laps, q) = (root.join("laps"), Queue::new(&root.join("data")));
        lap(&laps, "a.swlap", 1);
        lap(&laps, "b.swlap", 2);
        q.collect(&laps, None).unwrap();
        let limits = Limits {
            now: 1,
            deadline: Some(std::time::Instant::now()),
            ignore_backoff: false,
        };
        let r = q.drain(&Fake::new(vec![]), &Value::Null, limits, &mut |_| {});
        assert_eq!((r.uploaded, r.waiting), (0, 2));
        assert_eq!(q.list(Folder::Outbox).len(), 2);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_name_that_is_taken_gets_a_suffix() {
        let root = dir("names");
        let (laps, q) = (root.join("laps"), Queue::new(&root.join("data")));
        lap(&laps, "a.swlap", 1);
        q.collect(&laps, None).unwrap();
        lap(&laps, "a.swlap", 2);
        q.collect(&laps, None).unwrap();
        let names: Vec<String> = q
            .list(Folder::Outbox)
            .iter()
            .map(|e| file_name(&e.path))
            .collect();
        assert_eq!(names, ["a-1.swlap", "a.swlap"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn pruning_drops_the_old_and_the_excess() {
        let root = dir("prune");
        let q = Queue::new(&root);
        let (sent, outbox) = (q.dir(Folder::Sent), q.dir(Folder::Outbox));
        std::fs::create_dir_all(&sent).unwrap();
        std::fs::create_dir_all(&outbox).unwrap();
        for n in 0..(SENT_KEEP + 5) {
            std::fs::write(sent.join(format!("{n:04}.swlap")), sample(1, true)).unwrap();
            std::fs::write(sent.join(format!("{n:04}.json")), "{}").unwrap();
        }
        std::fs::write(outbox.join("x.swlap"), sample(1, true)).unwrap();
        let now = unix_now();
        q.prune(now);
        assert_eq!(q.list(Folder::Sent).len(), SENT_KEEP);
        assert_eq!(file_name(&q.list(Folder::Sent)[0].path), "0005.swlap");
        assert!(!sent.join("0000.json").exists());
        assert_eq!(q.list(Folder::Outbox).len(), 1);
        q.prune(now + OUTBOX_SECS + 10);
        assert!(q.list(Folder::Outbox).is_empty());
        q.prune(now + KEEP_SECS + 10);
        assert!(q.list(Folder::Sent).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn backoff_doubles_to_an_hour() {
        assert_eq!([1, 2, 3].map(backoff), [30, 60, 120]);
        assert_eq!(backoff(40), BACKOFF_MAX);
    }
}
