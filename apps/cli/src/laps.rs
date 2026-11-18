use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::Value;
use swofty_core::baseline::Identity;
use swofty_core::install::Game;
use swofty_core::store::Store;
use swofty_core::timetrial::api::{Api, Boards};
use swofty_core::timetrial::barred::{self, Barred};
use swofty_core::timetrial::integrity::{self, Module};
use swofty_core::timetrial::queue::{Entry, Folder, Limits, Queue, unix_now};
use swofty_core::timetrial::upload::{self, Watcher};
use swofty_core::timetrial::{self, token};

use crate::run::say;

#[derive(Subcommand)]
pub enum LapsCommand {
    #[command(about = "List the Time Trial laps waiting to upload and the ones sent, with their status")]
    List,
    #[command(about = "Upload the waiting laps now, or one lap file")]
    Upload {
        #[arg(help = "A .swlap file; without it every waiting lap is sent")]
        file: Option<PathBuf>,
    },
    #[command(about = "Print the address of the Time Trial boards")]
    Open,
    #[command(about = "Send one in-game bridge request file to the server and print the answer")]
    BridgeTest {
        #[arg(help = "A request in the format the game writes to swofty/tt/outbox")]
        file: PathBuf,
    },
    #[command(about = "Show a made-up lap position toast in the running Time Trial session")]
    ToastTest {
        #[arg(long, help = "Show the NEW RECORD form (position 1)")]
        record: bool,
    },
}

pub fn command(store: &Store, profile: &str, cmd: LapsCommand) -> Result<ExitCode> {
    match cmd {
        LapsCommand::List => list(store, profile),
        LapsCommand::Upload { file } => send(store, profile, file)?,
        LapsCommand::Open => println!("{}", timetrial::boards_url()),
        LapsCommand::BridgeTest { file } => bridge_test(store, &file)?,
        LapsCommand::ToastTest { record } => toast_test(store, record)?,
    }
    Ok(ExitCode::SUCCESS)
}

fn toast_test(store: &Store, record: bool) -> Result<()> {
    let doc = timetrial::toast::sample(unix_now(), record);
    timetrial::toast::write(&store.root, &doc)?;
    println!(
        "wrote {}; it shows within a second in a running Time Trial session",
        timetrial::toast::dir(&store.root).display()
    );
    Ok(())
}

fn bridge_test(store: &Store, file: &std::path::Path) -> Result<()> {
    let Some(token) = token(&store.root, &store.config()) else {
        bail!("Time Trial boards is off or you are not signed in");
    };
    let mut request: Value = serde_json::from_slice(&std::fs::read(file)?)?;
    let Some(object) = request.as_object_mut() else {
        bail!("{} is not a JSON object", file.display());
    };
    object.remove("seq");
    let answer = Api::new(&token).game_f1os(&request)?;
    println!("status {}", answer.get("status").unwrap_or(&Value::Null));
    match answer.get("body") {
        Some(Value::String(body)) => println!("{body}"),
        Some(other) => println!("{other}"),
        None => println!("(no body)"),
    }
    Ok(())
}

fn clock(ms: u64) -> String {
    format!("{}:{:02}.{:03}", ms / 60_000, ms / 1000 % 60, ms % 1000)
}

fn describe(e: &Entry) -> String {
    let (track, weather, ms, valid) = match (&e.done, &e.header) {
        (Some(d), _) => (d.track.clone(), d.weather.clone(), d.time_ms, d.valid),
        (None, Some(h)) => (
            h.board.track.clone(),
            h.board.weather.clone(),
            h.lap.time_ms,
            h.lap.valid,
        ),
        (None, None) => ("?".into(), String::new(), 0, false),
    };
    let state = match e.folder {
        Folder::Outbox => match e.waiting.as_ref().filter(|w| w.attempts > 0) {
            Some(w) => format!(
                "waiting (tried {}x: {})",
                w.attempts,
                w.last_error.as_deref().unwrap_or("")
            ),
            None => "waiting".into(),
        },
        Folder::Sent => match e.done.as_ref().and_then(|d| d.answer.as_ref()) {
            Some(a) => match a.rank {
                Some(r) => format!("{} P{r}", a.status),
                None => a.status.clone(),
            },
            None => "sent".into(),
        },
        Folder::Rejected => format!(
            "not sent: {}",
            e.done
                .as_ref()
                .and_then(|d| d.error.as_deref())
                .unwrap_or("refused")
        ),
    };
    let lap = if valid { "" } else { " (invalid lap)" };
    format!("{track} {weather} {}{lap}  {state}", clock(ms))
}

fn list(store: &Store, profile: &str) {
    let queue = Queue::new(&store.root);
    let mut any = false;
    for folder in [Folder::Outbox, Folder::Rejected, Folder::Sent] {
        let mut entries = queue.list(folder);
        entries.reverse();
        for e in entries.iter().take(50) {
            any = true;
            println!("{}", describe(e));
        }
    }
    if !any {
        println!("no Time Trial laps yet");
    }
    if let Ok(game) = crate::build::game(store, None) {
        let dirs = timetrial::game_replay::dirs(&store.root, &game.dir);
        for pair in timetrial::game_replay::ready(&dirs, unix_now()) {
            println!("game replay of a {} lap  waiting", clock(pair.lap_ms));
        }
    }
    if token(&store.root, &store.config()).is_none() {
        println!(
            "Time Trial boards uploads are off: sign in with Discord and leave Time Trial boards on in Settings"
        );
    }
    let listed = barred::current(&store.root, unix_now()).unwrap_or_default();
    let enabled = barred::for_store(store, profile, &listed);
    if let Some(line) = barred::warning(&enabled.iter().collect::<Vec<_>>()) {
        println!("{line}");
        for b in &enabled {
            match b.reason.trim() {
                "" => println!("  {} ({})", b.title(), b.id),
                reason => println!("  {} ({}): {reason}", b.title(), b.id),
            }
        }
    }
}

/// Hand the core the barred addons the profile has enabled, from the cached list, and ask the server for a
/// newer list on a thread of its own when the cached one is old or missing. The launch never waits for it.
fn barred_for_launch(store: &Store, profile: &str, game: &std::path::Path) {
    let kept = barred::cached(&store.root);
    let shown = kept
        .as_ref()
        .map(|c| barred::for_store(store, profile, &c.addons))
        .unwrap_or_default();
    hand_over(game, &shown);
    if kept.is_some_and(|c| c.fresh(unix_now())) {
        return;
    }
    let (s, p, g) = (store.clone(), profile.to_owned(), game.to_owned());
    std::thread::spawn(move || {
        let list = match barred::refresh(&s.root, &swofty_core::catalog::base_url(), unix_now()) {
            Ok(list) => list,
            Err(e) => {
                say("laps", &format!("barred addons list: {e}"));
                return;
            }
        };
        let next = barred::for_store(&s, &p, &list);
        if next != shown {
            hand_over(&g, &next);
        }
    });
}

fn hand_over(game: &std::path::Path, addons: &[Barred]) {
    if let Err(e) = barred::write_session(game, addons) {
        say(
            "laps",
            &format!("barred addons: the session file is not writable: {e}"),
        );
        return;
    }
    if let Some(line) = barred::warning(&addons.iter().collect::<Vec<_>>()) {
        say("laps", &line);
    }
}

fn fingerprint(store: &Store, profile: &str, identity: &Identity) -> Option<Value> {
    let p = store.profile(profile).ok()?;
    let fp = swofty_core::multiplayer::fingerprint::fingerprint(store, &p, identity).ok()?;
    Some(swofty_core::timetrial::fingerprint::wire(&fp))
}

fn send(store: &Store, profile: &str, file: Option<PathBuf>) -> Result<()> {
    let Some(token) = token(&store.root, &store.config()) else {
        bail!(
            "Time Trial boards is off or you are not signed in: sign in with Discord in the launcher and turn Time Trial boards on in Settings"
        );
    };
    let api = Api::new(&token);
    let print = crate::build::game(store, None)
        .ok()
        .and_then(|g| fingerprint(store, profile, &g.identity))
        .unwrap_or(Value::Null);
    let queue = Queue::new(&store.root);
    match file {
        Some(file) => {
            let a = queue.send_one(&file, &api as &dyn Boards, &print, unix_now())?;
            let rank = a.rank.map_or(String::new(), |r| format!(" -> P{r}"));
            println!("uploaded{rank} ({})", a.status);
            if let Some(url) = a.url {
                println!("{url}");
            }
            for r in a.reasons {
                println!("  {r}");
            }
        }
        None => {
            let report = queue.drain(
                &api,
                &print,
                Limits {
                    ignore_backoff: true,
                    ..Limits::at(unix_now())
                },
                &mut |e| println!("{}", upload::line(&e)),
            );
            println!(
                "{} uploaded, {} still waiting, {} not sent",
                report.uploaded, report.waiting, report.rejected
            );
            if let Ok(game) = crate::build::game(store, None) {
                let dirs = timetrial::game_replay::dirs(&store.root, &game.dir);
                timetrial::game_replay::send(&queue, &dirs, &api, unix_now(), &|| false, &mut |l| {
                    println!("{l}")
                });
            }
        }
    }
    Ok(())
}

pub fn begin(
    store: &Store,
    config: &swofty_core::session::Config,
    profile: &str,
    g: &Game,
) -> Option<Watcher> {
    barred_for_launch(store, profile, &g.dir);
    let Some(_) = token(&store.root, config) else {
        timetrial::clear_session_keys(&g.dir);
        return None;
    };
    let nonce = match upload::nonce(&store.root, config) {
        Ok(n) => Some(n.nonce),
        Err(e) => {
            say(
                "laps",
                &format!("session nonce: {e}; the watcher asks again while the game runs"),
            );
            None
        }
    };
    if let Err(e) = timetrial::write_session_keys(&g.dir, nonce.as_deref(), config.time_trial_ingame) {
        say("laps", &format!("lap recording is off: {e}"));
        return None;
    }
    let (s, p, identity) = (store.clone(), profile.to_owned(), g.identity.clone());
    Some(Watcher::start(
        &store.root,
        &g.dir,
        config,
        nonce.is_none(),
        move || fingerprint(&s, &p, &identity),
        |line| say("laps", line),
    ))
}

/// The verified integrity module for a session when Time Trial boards is on and the player is signed in.
/// Any trouble is one log line and the session goes on without it.
pub fn integrity(store: &Store, out: &dyn Fn(&str)) -> Option<Module> {
    let token = token(&store.root, &store.config())?;
    let found = match integrity::local() {
        Some(local) => {
            out(&format!(
                "integrity module: {} is set; the local build is placed unsigned and the server's module is not used",
                integrity::LOCAL_ENV
            ));
            local
        }
        None => integrity::ensure(&store.root, &Api::new(&token)),
    };
    match found {
        Ok(m) => Some(m),
        Err(why) => {
            out(&format!("integrity module: not loaded for this session: {why}"));
            None
        }
    }
}

pub fn end(watcher: Option<Watcher>, game: &std::path::Path) {
    if let Some(w) = watcher {
        w.finish();
        timetrial::clear_session_keys(game);
    }
    barred::write_session(game, &[]).ok();
}
