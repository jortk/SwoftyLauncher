//! The loop that gets laps to the server: one pass over the laps folder and the outbox, and a watcher
//! that makes passes while the game runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::api::{Api, Boards, Nonce};
use super::bridge::Pump;
use super::game_replay;
use super::queue::{Event, Folder, Limits, Queue, Report, unix_now};
use super::toast;
use super::{laps_dirs, token, write_session_keys};
use crate::session::Config;

const WATCH_EVERY: Duration = Duration::from_secs(5);
const FINAL_SWEEP: Duration = Duration::from_secs(6);
/// How long a new lap waits for the replay the core saves of a personal best, so the two upload together.
const REPLAY_WAIT: Duration = Duration::from_secs(10);
const HOLD_POLL: Duration = Duration::from_millis(300);
const NONCE_RETRIES: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(8),
];
const NONCE_LATER: Duration = Duration::from_secs(60);

/// The run log line for an event, tagged `laps` by the caller.
pub fn line(event: &Event) -> String {
    match event {
        Event::Uploaded { time_ms, answer, .. } => {
            let rank = answer.rank.map_or(String::new(), |r| format!(" -> P{r}"));
            format!("uploaded {time_ms} ms{rank} ({})", answer.status)
        }
        Event::Retry { error, .. } => format!("upload failed, will retry: {error}"),
        Event::Rejected { name, error } => format!("{name} not uploaded: {error}"),
    }
}

/// Tell the core a lap is the player's new best. Only while the game runs: the final sweep after it
/// exits has nobody to show it to.
fn toast(data: &Path, queue: &Queue, event: &Event, say: &dyn Fn(&str)) {
    let Event::Uploaded {
        board,
        time_ms,
        answer,
        ..
    } = event
    else {
        return;
    };
    let previous = toast::previous_best(queue, board, &answer.id);
    let Some(doc) = toast::message(board, *time_ms, answer, previous, unix_now()) else {
        return;
    };
    match toast::write(data, &doc) {
        Ok(()) => say(&format!(
            "toast: P{} on {}/{}",
            doc["position"], board.content, board.track
        )),
        Err(e) => say(&format!("toast: couldn't write the message: {e}")),
    }
}

/// Ask for a nonce for the game session about to start. A failure is not fatal here: the watcher keeps
/// asking while the game runs, and the core picks the nonce up for the laps written after it arrives.
pub fn nonce(data: &Path, config: &Config) -> Result<Nonce, String> {
    let token = token(data, config).ok_or_else(|| "nobody is signed in".to_owned())?;
    Api::new(&token).sessions().map_err(|e| e.to_string())
}

/// How long the watcher waits before asking for the nonce again: three quick tries over about 15 s,
/// then once a minute while the game runs.
pub fn nonce_retry_pause(attempt: usize) -> Duration {
    NONCE_RETRIES.get(attempt).copied().unwrap_or(NONCE_LATER)
}

/// Keeps asking for the session nonce until the server issues one or the game ends, and puts it in the
/// session file for the core. Without it every lap of the session is refused by the server.
fn nonce_until_issued(
    data: &Path,
    game: &Path,
    config: &Config,
    stop: &dyn Fn() -> bool,
    say: &dyn Fn(&str),
) {
    for attempt in 0.. {
        let until = Instant::now() + nonce_retry_pause(attempt);
        while Instant::now() < until {
            if stop() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if stop() || token(data, config).is_none() {
            return;
        }
        match nonce(data, config) {
            Ok(n) => {
                match write_session_keys(game, Some(&n.nonce), config.time_trial_ingame) {
                    Ok(()) => say(&format!(
                        "session nonce issued on retry {}: the laps written from here carry it",
                        attempt + 1
                    )),
                    Err(e) => say(&format!(
                        "session nonce issued but the session file is not writable: {e}"
                    )),
                }
                return;
            }
            Err(e) => say(&format!(
                "session nonce: {e}; laps without one are refused by the server, asking again in {} s",
                nonce_retry_pause(attempt + 1).as_secs()
            )),
        }
    }
}

/// One pass: collect the game's laps into the outbox, then upload what is due. With no token the laps
/// are collected and the old ones in the outbox expire.
pub fn sweep(
    data: &Path,
    game: Option<&Path>,
    api: Option<&dyn Boards>,
    fingerprint: Option<&Value>,
    deadline: Option<Instant>,
    on: &mut dyn FnMut(Event),
) -> Report {
    let queue = Queue::new(data);
    if let Some(game) = game {
        for laps in laps_dirs(data, game) {
            queue.collect(&laps, fingerprint).ok();
        }
    }
    let now = unix_now();
    queue.prune(now);
    match api {
        Some(api) => queue.drain(
            api,
            fingerprint.unwrap_or(&Value::Null),
            Limits {
                now,
                deadline,
                ignore_backoff: false,
            },
            on,
        ),
        None => Report {
            waiting: queue.list(Folder::Outbox).len(),
            ..Report::default()
        },
    }
}

/// The pass the launcher makes when it starts, for laps and game replays a session left queued.
pub fn flush(data: &Path, game: Option<&Path>, config: &Config) -> Report {
    let api = token(data, config).map(|t| Api::new(&t));
    let report = sweep(
        data,
        None,
        api.as_ref().map(|a| a as &dyn Boards),
        None,
        None,
        &mut |_| {},
    );
    if let (Some(game), Some(api)) = (game, &api) {
        game_replay::send(
            &Queue::new(data),
            &game_replay::dirs(data, game),
            api,
            unix_now(),
            &|| false,
            &mut |_| {},
        );
    }
    report
}

/// Upload the game replays the core saved in `game` whose laps the server has, if someone is signed in.
pub fn replays(data: &Path, game: &Path, config: &Config, stop: &dyn Fn() -> bool, say: &dyn Fn(&str)) {
    static BUSY: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let Ok(_busy) = BUSY.try_lock() else {
        return;
    };
    let dirs = game_replay::dirs(data, game);
    if !game_replay::any(&dirs) {
        return;
    }
    let Some(api) = token(data, config).map(|t| Api::new(&t)) else {
        return;
    };
    game_replay::send(&Queue::new(data), &dirs, &api, unix_now(), stop, &mut |l| say(l));
}

/// Sweeps while the game runs. [`Watcher::finish`] makes the last sweep, with a few seconds at most.
/// Game replays upload on a thread of their own that the finish does not wait for.
pub struct Watcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pump: Option<Pump>,
    data: PathBuf,
    game: PathBuf,
    config: Config,
    fingerprint: Arc<dyn Fn() -> Option<Value> + Send + Sync>,
    say: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Watcher {
    /// With In-game leaderboards on and a sign-in, the bridge's mailbox pump runs beside it.
    /// With Time Trial boards off or nobody signed in, the laps are still collected into the outbox and
    /// nothing leaves the machine. With `retry_nonce` the session nonce the start could not get is asked
    /// for again on a thread of its own.
    pub fn start(
        data: &Path,
        game: &Path,
        config: &Config,
        retry_nonce: bool,
        fingerprint: impl Fn() -> Option<Value> + Send + Sync + 'static,
        say: impl Fn(&str) + Send + Sync + 'static,
    ) -> Self {
        let say: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(say);
        let mut w = Self {
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            pump: config
                .time_trial_ingame
                .then(|| token(data, config))
                .flatten()
                .map(|t| {
                    let say = say.clone();
                    Pump::start(&super::bridge::mailboxes(data, game), Api::new(&t), move |l| {
                        say(l)
                    })
                }),
            data: data.to_owned(),
            game: game.to_owned(),
            config: config.clone(),
            fingerprint: Arc::new(fingerprint),
            say,
        };
        let (stop, data, game, config) = (w.stop.clone(), w.data.clone(), w.game.clone(), w.config.clone());
        let (fingerprint, say) = (w.fingerprint.clone(), w.say.clone());
        w.thread = std::thread::Builder::new()
            .name("tt-laps".into())
            .spawn(move || {
                let mut cached: Option<Option<Value>> = None;
                while !stop.load(Ordering::Relaxed) {
                    let laps = has_laps(&data, &game);
                    if laps && waiting_for_replay(&data, &game) {
                        std::thread::sleep(HOLD_POLL);
                        continue;
                    }
                    if laps || has_outbox(&data) {
                        let print = cached.get_or_insert_with(|| fingerprint()).clone();
                        pass(&data, &game, &config, print.as_ref(), None, &*say);
                        if laps {
                            replays(&data, &game, &config, &|| stop.load(Ordering::Relaxed), &*say);
                        }
                    }
                    let until = Instant::now() + WATCH_EVERY;
                    while Instant::now() < until && !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
            })
            .ok();
        if retry_nonce {
            let (stop, data, game, config) =
                (w.stop.clone(), w.data.clone(), w.game.clone(), w.config.clone());
            let say = w.say.clone();
            std::thread::Builder::new()
                .name("tt-nonce".into())
                .spawn(move || {
                    let stopped = || stop.load(Ordering::Relaxed);
                    nonce_until_issued(&data, &game, &config, &stopped, &*say);
                })
                .ok();
        }
        let (stop, data, game, config) = (w.stop.clone(), w.data.clone(), w.game.clone(), w.config.clone());
        let say = w.say.clone();
        std::thread::Builder::new()
            .name("tt-replays".into())
            .spawn(move || {
                let stopped = || stop.load(Ordering::Relaxed);
                while !stopped() {
                    replays(&data, &game, &config, &stopped, &*say);
                    let until = Instant::now() + WATCH_EVERY;
                    while Instant::now() < until && !stopped() {
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
            })
            .ok();
        w
    }

    /// Stop the watcher and the bridge and sweep once more, giving up after a few seconds.
    pub fn finish(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(pump) = self.pump.take() {
            pump.finish();
        }
        if let Some(t) = self.thread.take() {
            t.join().ok();
        }
        let print = has_laps(&self.data, &self.game)
            .then(|| (self.fingerprint)())
            .flatten();
        pass(
            &self.data,
            &self.game,
            &self.config,
            print.as_ref(),
            Some(Instant::now() + FINAL_SWEEP),
            &*self.say,
        );
    }
}

/// Whether a lap is held back for its replay: the newest lap is younger than [`REPLAY_WAIT`] and no
/// replay saved since it was written is ready yet.
fn hold_for_replay(lap_age: Duration, replay_ready: bool) -> bool {
    !replay_ready && lap_age < REPLAY_WAIT
}

fn waiting_for_replay(data: &Path, game: &Path) -> bool {
    let Some(written) = laps_dirs(data, game)
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flat_map(|d| d.filter_map(Result::ok))
        .filter(|e| e.path().extension().is_some_and(|x| x == "swlap"))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
    else {
        return false;
    };
    let age = written.elapsed().unwrap_or_default();
    let since = written
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .saturating_sub(2);
    let ready = game_replay::ready(&game_replay::dirs(data, game), unix_now())
        .iter()
        .any(|p| p.secs >= since);
    hold_for_replay(age, ready)
}

fn has_laps(data: &Path, game: &Path) -> bool {
    laps_dirs(data, game).iter().any(|dir| {
        std::fs::read_dir(dir).is_ok_and(|d| {
            d.filter_map(Result::ok)
                .any(|e| e.path().extension().is_some_and(|x| x == "swlap"))
        })
    })
}

fn has_outbox(data: &Path) -> bool {
    !Queue::new(data).list(Folder::Outbox).is_empty()
}

fn pass(
    data: &Path,
    game: &Path,
    config: &Config,
    fingerprint: Option<&Value>,
    deadline: Option<Instant>,
    say: &dyn Fn(&str),
) {
    let api = token(data, config).map(|t| match deadline {
        Some(d) => Api::new(&t).within(d.saturating_duration_since(Instant::now())),
        None => Api::new(&t),
    });
    let queue = Queue::new(data);
    sweep(
        data,
        Some(game),
        api.as_ref().map(|a| a as &dyn Boards),
        fingerprint,
        deadline,
        &mut |e| {
            say(&line(&e));
            if deadline.is_none() {
                toast(data, &queue, &e, say);
            }
        },
    );
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_new_lap_waits_for_its_replay_but_never_long() {
        assert!(super::hold_for_replay(Duration::from_secs(2), false));
        assert!(!super::hold_for_replay(Duration::from_secs(2), true));
        assert!(!super::hold_for_replay(super::REPLAY_WAIT, false));
    }

    use super::*;
    use crate::timetrial::api::Uploaded;

    #[test]
    fn the_nonce_is_asked_for_three_times_quickly_and_then_once_a_minute() {
        let quick: Duration = (0..3).map(nonce_retry_pause).sum();
        assert_eq!(quick, Duration::from_secs(15));
        assert_eq!(nonce_retry_pause(3), Duration::from_secs(60));
        assert_eq!(nonce_retry_pause(40), Duration::from_secs(60));
    }

    #[test]
    fn the_run_log_line_names_the_time_rank_and_status() {
        let answer = Uploaded {
            id: "x".into(),
            status: "ranked".into(),
            rank: Some(12),
            best: Some(true),
            total: None,
            previous_best_ms: None,
            reasons: vec![],
            url: None,
        };
        let e = Event::Uploaded {
            name: "a".into(),
            board: Box::default(),
            time_ms: 91234,
            answer,
        };
        assert_eq!(line(&e), "uploaded 91234 ms -> P12 (ranked)");
    }

    #[test]
    fn a_sweep_with_no_sign_in_collects_and_uploads_nothing() {
        let root = std::env::temp_dir().join(format!("swofty-ttu-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let [_, laps] = laps_dirs(&root.join("data"), &root.join("game"));
        std::fs::create_dir_all(&laps).unwrap();
        std::fs::write(laps.join("a.swlap"), crate::timetrial::swlap::sample(1, true)).unwrap();
        let r = sweep(
            &root.join("data"),
            Some(&root.join("game")),
            None,
            None,
            None,
            &mut |_| {},
        );
        assert_eq!((r.uploaded, r.waiting), (0, 1));
        assert!(!laps.join("a.swlap").exists());
        assert_eq!(Queue::new(&root.join("data")).list(Folder::Outbox).len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn laps_in_the_data_folder_and_the_game_folder_are_both_collected() {
        let root = std::env::temp_dir().join(format!("swofty-ttu-both-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (data, game) = (root.join("data"), root.join("game"));
        assert!(!has_laps(&data, &game));
        let [new, old] = laps_dirs(&data, &game);
        assert_eq!(new, data.join("laps"));
        assert_eq!(old, game.join("swofty").join("laps"));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("a.swlap"), crate::timetrial::swlap::sample(1, true)).unwrap();
        assert!(has_laps(&data, &game));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("b.swlap"), crate::timetrial::swlap::sample(2, true)).unwrap();
        let r = sweep(&data, Some(&game), None, None, None, &mut |_| {});
        assert_eq!((r.uploaded, r.waiting), (0, 2));
        assert!(!new.join("a.swlap").exists() && !old.join("b.swlap").exists());
        assert!(!has_laps(&data, &game));
        std::fs::remove_dir_all(&root).ok();
    }
}
