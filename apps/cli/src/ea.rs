use std::ffi::OsStr;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use swofty_core::app_control::Block;
use swofty_core::compat::{self, RunAsAdmin};
use swofty_core::ea_app::{self, Handoff};
use swofty_core::ea_diag::{self, Tracker};
use swofty_core::install::{Game, Source};
use swofty_core::launch;
use swofty_core::offline::{self, Slot};
use swofty_core::process::own_elevation;
use swofty_core::process::{GameProcess, game_processes, stop_process};
use swofty_core::recovery::Scope;
use swofty_core::session::{LOADER_LOG, LogMark};
use swofty_core::steam_client;
use swofty_core::store::Store;

use crate::run::{StageFile, StopPhase, ending, say, session_stop};

const APP_START_TIMEOUT: Duration = Duration::from_secs(90);
// A link handed over while the EA app is still starting can be dropped:
// under Proton one sent a second after EADesktop.exe appeared never arrived.
const APP_SETTLE: Duration = Duration::from_secs(10);
// The EA app syncs cloud saves before it starts the game, which took about
// 170 s on a test copy, so the wait for the link outlasts that sync.
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(not(windows))]
const SERVER_STOP_TIMEOUT: Duration = Duration::from_secs(30);
const PRESS_PLAY_TIMEOUT: Duration = Duration::from_secs(600);
const STUB_LOG_WINDOW: i64 = 6 * 3600;
// An antivirus removes a file it flags within a second or two of it being
// written, so the slot is checked this long after staging, before the link.
const STUB_SETTLE: Duration = Duration::from_secs(2);
const STUB_POLL: Duration = Duration::from_secs(5);
const ELEVATION_POLL: Duration = Duration::from_secs(1);
const STUB_LOG_READ: u64 = 64 << 10;
const HANDOFF_LOG: &str = "ea_handoff.log";

/// Which store Play brings up first for a copy of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreStart {
    Steam,
    EaApp,
}

/// An EA app copy always starts through the EA app, which launches only a
/// game its account owns; `--direct` is for Steam copies.
pub fn store_start(source: Source) -> StoreStart {
    match source {
        Source::EaApp => StoreStart::EaApp,
        _ => StoreStart::Steam,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    NoHandoff,
    Running,
    Start,
    Off,
}

pub fn policy(handoff: bool, on: bool, running: bool) -> Policy {
    match (handoff, running, on) {
        (false, _, _) => Policy::NoHandoff,
        (true, true, _) => Policy::Running,
        (true, false, true) => Policy::Start,
        (true, false, false) => Policy::Off,
    }
}

fn handoff(store: &Store) -> Option<Handoff> {
    ea_app::handoff(store.config().launch.ea_launch_command.as_deref())
}

/// Stop before anything is built when the EA app has F1 25 in another
/// folder: its launch link can't start this copy.
pub fn check_recorded(game: &Path) -> Result<()> {
    let Some(m) = ea_app::moved(game, &ea_app::recorded_here(), ea_app::is_install) else {
        return Ok(());
    };
    let pick = if m.there {
        format!(
            ". To play the copy in {0}: swofty game use \"{0}\"",
            m.recorded.display()
        )
    } else {
        String::new()
    };
    bail!("{}{pick}", m.text())
}

fn desktop_opener(h: &Handoff) -> Option<(Command, bool)> {
    ea_app::desktop_opener(h, compat::elevated())
}

/// Stop before anything is built when Windows won't start a program the
/// session needs because it is set to run as administrator: the program that
/// hands the EA app the launch link, or for a modded session F1 25.
pub fn check_elevation(store: &Store, game: &Path, modded: bool) -> Result<()> {
    let program = handoff(store).and_then(|h| h.program());
    match compat::ea_blocker(game, program.as_deref(), modded) {
        Some(r) => Err(refused(store, r)),
        None => Ok(()),
    }
}

/// A start Windows refused with error 740: `program` is set to run as
/// administrator and this process isn't.
fn refused_elevation(store: &Store, program: &OsStr, e: &std::io::Error) -> Option<anyhow::Error> {
    compat::needs_elevation(e).then(|| {
        let program = Path::new(program);
        say(
            "ea app",
            &format!(
                "Windows won't start {} for this process: it is set to run as administrator (error 740)",
                program.display()
            ),
        );
        refused(store, compat::refused_start(program))
    })
}

/// The launch link never reached the EA app: the command that hands it over
/// didn't start, so the session ends at once.
fn handoff_failed(store: &Store, c: &Command, h: &Handoff, e: &std::io::Error) -> anyhow::Error {
    refused_elevation(store, c.get_program(), e).unwrap_or_else(|| {
        anyhow!("could not run {h} to hand the launch link to the EA app ({e}); open the EA app and press Play again")
    })
}

/// Whether a session puts offline play's file in EA AntiCheat's launcher's
/// place. The EA app runs that launcher to start F1 25 and asks to repair
/// F1 25 when another file is there, so a vanilla session through the EA
/// app leaves it as installed.
pub fn stages_offline(via_ea: bool, vanilla: bool) -> bool {
    !(via_ea && vanilla)
}

/// An EA app copy needs the EA app: start it when it is closed and wait for
/// it, as Steam is for a Steam copy. On Linux the launch.ea_launch_command
/// setting, run without a link, starts it; a command that needs the link
/// leaves the start to the link.
pub fn ensure(store: &Store, opted_out: bool) -> Result<()> {
    let h = handoff(store);
    let on = store.config().launch.start_store && !opted_out;
    match policy(h.is_some(), on, ea_app::running()) {
        Policy::NoHandoff => bail!("{}", ea_app::no_handoff()),
        Policy::Running => say("ea app", "detected: running"),
        Policy::Off => bail!(
            "the EA app is closed and starting it is turned off (launch.start_store or --no-start-store); \
             open the EA app and press Play again"
        ),
        Policy::Start => match h.as_ref().and_then(desktop_opener) {
            Some((mut open, exit_tells)) => {
                say("ea app", "detected: closed; starting the EA app");
                let stage = StageFile(store.root.join(steam_client::STAGE_FILE));
                stage.set("Starting the EA app…");
                let mut child = detach(&mut open).map_err(|e| {
                    refused_elevation(store, open.get_program(), &e).unwrap_or_else(|| {
                        anyhow!("could not start the EA app ({e}); open the EA app and press Play again")
                    })
                })?;
                let since = Instant::now();
                match wait_for_app(&mut child, exit_tells, || since.elapsed() >= APP_START_TIMEOUT) {
                    AppStart::Running => {
                        say(
                            "ea app",
                            &format!("running after {:.1} s", since.elapsed().as_secs_f64()),
                        );
                        std::thread::sleep(APP_SETTLE);
                    }
                    AppStart::CommandFailed(exit) => say(
                        "ea app",
                        &format!("the command did not open the EA app ({exit}); the launch link starts it"),
                    ),
                    AppStart::TimedOut => bail!(
                        "the EA app didn't start within {} s; open the EA app and press Play again",
                        APP_START_TIMEOUT.as_secs()
                    ),
                }
            }
            None => say("ea app", "detected: closed; the launch link starts it"),
        },
    }
    Ok(())
}

/// Wine reads a prefix's user.reg only when its wineserver starts, and the
/// EA app's wineserver also runs F1 25. So the version.dll override goes into
/// the EA app's prefix while the EA app is closed, and stays there: without
/// Swofty's version.dll in the game folder Wine loads its own.
#[cfg(not(windows))]
pub fn dll_setting(store: &Store, g: &Game, opted_out: bool) -> Result<()> {
    use swofty_core::install::{add_override, has_override};
    let Some((reg, pfx)) = g.user_reg.as_deref().and_then(|r| Some((r, r.parent()?))) else {
        return Ok(());
    };
    if has_override(&std::fs::read_to_string(reg)?) {
        return Ok(());
    }
    if let Some(server) = ea_app::prefix_server(pfx) {
        if !store.config().launch.start_store || opted_out {
            bail!(
                "close the EA app and press Play again: Wine loads Swofty's version.dll in F1 25 only when the EA \
                 app starts after Swofty sets it up in the EA app's prefix"
            );
        }
        say(
            "ea app",
            "closing the EA app so Wine loads Swofty's version.dll in F1 25; it starts again with the setting",
        );
        stop_process(server)?;
        let since = Instant::now();
        while ea_app::prefix_server(pfx).is_some() || ea_app::running() {
            if since.elapsed() >= SERVER_STOP_TIMEOUT {
                bail!(
                    "the EA app didn't close within {} s; close it and press Play again",
                    SERVER_STOP_TIMEOUT.as_secs()
                );
            }
            std::thread::sleep(steam_client::POLL_INTERVAL);
        }
    }
    if let Some(out) = add_override(&std::fs::read_to_string(reg)?) {
        std::fs::write(reg, out)?;
    }
    say(
        "ea app",
        &format!("{} has Wine load Swofty's version.dll in F1 25", reg.display()),
    );
    Ok(())
}

#[cfg(windows)]
pub fn dll_setting(_: &Store, _: &Game, _: bool) -> Result<()> {
    Ok(())
}

#[derive(Debug, PartialEq)]
enum AppStart {
    Running,
    CommandFailed(std::process::ExitStatus),
    TimedOut,
}

/// Wait for EADesktop.exe after `child` was asked to open the EA app. A
/// command that exits with an error opened nothing, so waiting is pointless,
/// unless its exit code says nothing about the EA app (`exit_tells` false).
fn wait_for_app(child: &mut Child, exit_tells: bool, timed_out: impl Fn() -> bool) -> AppStart {
    loop {
        if ea_app::running() {
            return AppStart::Running;
        }
        if let Ok(Some(exit)) = child.try_wait()
            && exit_tells
            && !exit.success()
        {
            return AppStart::CommandFailed(exit);
        }
        if timed_out() {
            return AppStart::TimedOut;
        }
        std::thread::sleep(steam_client::POLL_INTERVAL);
    }
}

fn detach(c: &mut Command) -> std::io::Result<Child> {
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(c, 0);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(c, 0x0000_0008 | 0x0000_0200);
    c.spawn()
}

/// A game started since `before` that is not another copy's than `game`'s.
fn new_copy(game: &Path, before: &[u32]) -> Option<u32> {
    game_processes()
        .into_iter()
        .find(|g| !before.contains(&g.pid) && !g.elsewhere(game))
        .map(|g| g.pid)
}

fn wait_for_game(
    game: &Path,
    before: &[u32],
    timeout: Duration,
    handoff: &mut Option<Child>,
    mut watch: Option<&mut Tracker>,
    mut stop: impl FnMut() -> Result<()>,
) -> Result<Option<u32>> {
    let since = Instant::now();
    loop {
        if let Some(t) = watch.as_deref_mut() {
            for line in t.observe(&ea_diag::running_watched(), Instant::now()) {
                say("ea app", &line);
            }
        }
        if let Some(pid) = new_copy(game, before) {
            return Ok(Some(pid));
        }
        if ending() {
            bail!("asked to stop before F1 25 started");
        }
        stop()?;
        if let Some(c) = handoff.as_mut()
            && let Ok(Some(exit)) = c.try_wait()
        {
            *handoff = None;
            if !exit.success() {
                say("ea app", &format!("the hand-off command exited with {exit}"));
                return Ok(None);
            }
        }
        if since.elapsed() >= timeout {
            if let Some(t) = watch.as_deref() {
                say("ea app", &format!("timeout: {}", t.summary(since.elapsed())));
            }
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn say_elevation() {
    say(
        "ea app",
        &ea_diag::elevation_line(own_elevation(), ea_diag::ea_app_elevation()),
    );
}

/// The session's game, launched by the EA app: hand it the launch link and
/// wait for F1 25 to start. The game files are staged before this.
pub fn start(store: &Store, game: &Path, args: &[String], session: &Path, before: &[u32]) -> Result<Started> {
    let url = ea_app::install_url(game, args).map_err(|e| anyhow!(e))?;
    let h = handoff(store).ok_or_else(|| anyhow!("{}", ea_app::no_handoff()))?;
    check_stub(store, game)?;
    let stage = StageFile(store.root.join(steam_client::STAGE_FILE));
    stage.set("Starting F1 25 through the EA app…");
    say("ea app", &format!("asking the EA app to launch F1 25: {url}"));
    say_elevation();
    let mut tracker = Tracker::new(Instant::now(), &ea_diag::running_watched());
    say(
        "ea app",
        &format!(
            "launch link sent; waiting up to {} s for F1 25",
            HANDOFF_TIMEOUT.as_secs()
        ),
    );
    let log = std::fs::File::create(session.join(HANDOFF_LOG))?;
    let mut c = h.command(&url);
    c.stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut c, 0);
    let mut removed = removed_after_link(store, game);
    let mut elevation = refused_after_link(store, game);
    let mut stop = || -> Result<()> {
        removed()?;
        elevation()
    };
    let mut handoff = match c.spawn() {
        Ok(child) => Some(child),
        Err(e) => {
            say("ea app", &format!("could not run {h}: {e}"));
            return Err(handoff_failed(store, &c, &h, &e));
        }
    };
    let pid = match wait_for_game(
        game,
        before,
        HANDOFF_TIMEOUT,
        &mut handoff,
        Some(&mut tracker),
        &mut stop,
    )? {
        Some(pid) => pid,
        None => {
            if let Some((mut open, _)) = desktop_opener(&h) {
                detach(&mut open).ok();
            }
            let slot = offline::slot(&offline::journal_path(&store.root));
            match &slot {
                Ok(Some(Slot::Staged)) => say("ea app", "offline play's file is in place"),
                Ok(Some(Slot::Replaced)) => say("ea app", "the EA app put its own file over offline play's"),
                Ok(Some(Slot::Missing)) => say("ea app", "offline play's file is gone from the game folder"),
                Ok(Some(Slot::Unknown)) => {
                    say("ea app", "offline play's file was changed by another program")
                }
                Ok(None) => {}
                Err(e) => say("ea app", &format!("could not check offline play's file: {e}")),
            }
            let (line, status) = match no_game(slot.ok().flatten(), || stub_block(store, game)) {
                NoGame::Antivirus(block) => return Err(antivirus(store, block)),
                NoGame::Wait { line, status } => (line, status),
            };
            say("ea app", line);
            stage.set(status);
            wait_for_game(
                game,
                before,
                PRESS_PLAY_TIMEOUT,
                &mut None,
                Some(&mut tracker),
                &mut stop,
            )?
            .ok_or_else(|| {
                anyhow!(
                    "F1 25 was not started from the EA app within {} min; the game files are rolled back",
                    PRESS_PLAY_TIMEOUT.as_secs() / 60
                )
            })?
        }
    };
    say("ea app", &format!("F1 25 started (pid {pid})"));
    restage(store, game);
    Ok(Started { pid, handoff })
}

/// Before the launch link: an antivirus that removes offline play's file
/// does so within seconds of staging, and an EA app that finds the slot
/// empty repairs F1 25. So the slot is checked a moment after staging, and
/// an empty one stops the session before the EA app sees it.
fn check_stub(store: &Store, game: &Path) -> Result<()> {
    let journal = offline::journal_path(&store.root);
    if let Some(left) =
        offline::staged_at(&journal).and_then(|at| (at + STUB_SETTLE).duration_since(SystemTime::now()).ok())
    {
        std::thread::sleep(left.min(STUB_SETTLE));
    }
    if !matches!(offline::slot(&journal), Ok(Some(Slot::Missing))) {
        return Ok(());
    }
    say(
        "ea app",
        "offline play's file is gone from the game folder seconds after staging; not sending the launch link",
    );
    Err(antivirus(store, offline::stub_removed(game)))
}

/// After the launch link: the EA app's proxy logs it when it finds no file
/// to start, or an antivirus blocks it, which ends the wait at once.
fn removed_after_link<'a>(store: &'a Store, game: &'a Path) -> impl FnMut() -> Result<()> + 'a {
    let staged = staged_secs(store);
    let mut next = Instant::now();
    move || {
        let Some(at) = staged else {
            return Ok(());
        };
        if Instant::now() < next {
            return Ok(());
        }
        next = Instant::now() + STUB_POLL;
        match offline::stub_blocked(game, &ea_diag::proxy_logs(), at, Some(at)) {
            Some(block) => {
                say(
                    "ea app",
                    "the EA app could not start offline play's file: it is gone from the game folder or blocked",
                );
                Err(antivirus(store, block))
            }
            None => Ok(()),
        }
    }
}

/// After the launch link: the session launcher logs it when Windows would
/// start F1 25 only as administrator, which the EA app, running without
/// administrator rights, never can. That ends the wait at once. Only lines
/// written after the link count.
fn refused_after_link<'a>(store: &'a Store, game: &'a Path) -> impl FnMut() -> Result<()> + 'a {
    let log = game.join(LOADER_LOG);
    let before = LogMark::read(&log);
    let from = std::fs::metadata(&log).map_or(0, |m| m.len());
    let mut next = Instant::now();
    move || {
        if Instant::now() < next {
            return Ok(());
        }
        next = Instant::now() + ELEVATION_POLL;
        if !compat::stub_needs_elevation(&written_after(&log, before.as_ref(), from)) {
            return Ok(());
        }
        say(
            "ea app",
            "offline play's session launcher could not start F1 25: Windows starts it only as administrator \
             (error 740)",
        );
        Err(refused(store, compat::game_refused(game)))
    }
}

/// What `log` holds past `from`, its length when `before` was read; all of
/// it when it was started again since.
fn written_after(log: &Path, before: Option<&LogMark>, from: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(log) else {
        return String::new();
    };
    let len = f.metadata().map_or(0, |m| m.len());
    let start = if len < from || LogMark::replaced(log, before) {
        0
    } else {
        from
    };
    let mut data = Vec::new();
    if f.seek(SeekFrom::Start(start)).is_ok() {
        f.take(STUB_LOG_READ).read_to_end(&mut data).ok();
    }
    String::from_utf8_lossy(&data).into_owned()
}

fn staged_secs(store: &Store) -> Option<i64> {
    let at = offline::staged_at(&offline::journal_path(&store.root))?;
    Some(at.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64)
}

#[derive(Debug, PartialEq)]
enum NoGame {
    Antivirus(Block),
    Wait {
        line: &'static str,
        status: &'static str,
    },
}

/// What the session does once the launch link started nothing. An empty
/// slot the EA app's proxy logged as blocked or missing is an antivirus;
/// otherwise the EA app is repairing F1 25 or waiting for Play.
fn no_game(slot: Option<Slot>, blocked: impl FnOnce() -> Option<Block>) -> NoGame {
    if slot == Some(Slot::Missing)
        && let Some(block) = blocked()
    {
        return NoGame::Antivirus(block);
    }
    let (line, status) = waiting(slot);
    NoGame::Wait { line, status }
}

/// What the session says and shows while it waits for the EA app after the
/// launch link started nothing. An EA app that took offline play's file out
/// is repairing F1 25 and starts it once the repair is done.
fn waiting(slot: Option<Slot>) -> (&'static str, &'static str) {
    match slot {
        Some(Slot::Missing | Slot::Replaced) => (
            "the EA app took offline play's file out to repair F1 25; F1 25 starts without mods after the repair",
            "The EA app is repairing F1 25",
        ),
        _ => (
            "F1 25 did not start from the launch link; press Play on F1 25 in the EA app",
            "Press Play on F1 25 in the EA app",
        ),
    }
}

fn stub_block(store: &Store, game: &Path) -> Option<Block> {
    let since = swofty_core::catalog::now_secs() - STUB_LOG_WINDOW;
    offline::stub_blocked(game, &ea_diag::proxy_logs(), since, staged_secs(store))
}

/// Record an antivirus block of offline play's file for the launcher and
/// Send Logs, and end the session with its text.
fn antivirus(store: &Store, block: Block) -> anyhow::Error {
    let failure = record(store, launch::Cause::Blocked(block));
    anyhow!("{}", failure.text())
}

/// Record programs Windows won't start because they are set to run as
/// administrator, and end the session with what to untick.
fn refused(store: &Store, r: RunAsAdmin) -> anyhow::Error {
    let failure = record(store, launch::Cause::RunAsAdmin(r));
    anyhow!("{}", failure.text())
}

fn record(store: &Store, cause: launch::Cause) -> launch::Failure {
    let failure = launch::Failure::new(cause);
    say("warning", &format!("{}: {}", failure.title(), failure.text()));
    if let Err(e) = failure.write(&store.root) {
        say("info", &format!("could not record it for the launcher: {e}"));
    }
    failure
}

pub fn note_stub_blocked(store: &Store, game: &Path) -> bool {
    let Some(block) = stub_block(store, game) else {
        return false;
    };
    record(store, launch::Cause::Blocked(block));
    true
}

/// An EA app repair puts EA's anti-cheat launcher over offline play's file,
/// and the game it then starts has no mods. Put the file back for the next
/// Play and tell the player.
fn restage(store: &Store, game: &Path) {
    let journal = offline::journal_path(&store.root);
    if matches!(offline::slot(&journal), Ok(None)) {
        return;
    }
    let done = offline::artifacts(&store.root).and_then(|a| offline::restage(&a, &journal));
    match done {
        Ok(Some(Slot::Staged) | None) => {}
        Ok(Some(Slot::Replaced | Slot::Missing)) => {
            if note_stub_blocked(store, game) {
                return;
            }
            let failure = launch::Failure::new(launch::Cause::EaRepaired);
            say("warning", &format!("{}: {}", failure.title(), failure.text()));
            if let Err(e) = failure.write(&store.root) {
                say("info", &format!("could not record it for the launcher: {e}"));
            }
        }
        Ok(Some(Slot::Unknown)) => say(
            "ea app",
            "EAAntiCheat.GameServiceLauncher.exe changed after staging and is neither offline play's nor EA's; \
             left as is",
        ),
        Err(e) => say("ea app", &format!("could not put offline play's file back: {e}")),
    }
}

/// An EA app copy started by the EA app as it is installed, with nothing
/// staged: the profile and offline play stay out, so EA AntiCheat runs as
/// it does from the EA app's own Play button.
pub fn launch_installed(store: &Store, g: &Game, vr: bool, opted_out: bool) -> Result<()> {
    if Source::detect(&g.dir) != Source::EaApp {
        bail!(
            "{} is not an EA app copy; start a Steam copy from Steam, or pick the EA app copy with swofty game use ea",
            g.dir.display()
        );
    }
    if let Some(running) = game_processes().first() {
        bail!("F1 25 is already running (pid {}); close it first", running.pid);
    }
    {
        let out = |l: &str| say("info", l);
        let lock = crate::recover::lock(store, &out)?;
        crate::recover::stale(store, &lock, g, Scope::All, &out)?;
    }
    check_elevation(store, &g.dir, false)?;
    ensure(store, opted_out)?;
    let args: Vec<String> = if vr { vec!["-vr".into()] } else { Vec::new() };
    let url = ea_app::install_url(&g.dir, &args).map_err(|e| anyhow!(e))?;
    let h = handoff(store).ok_or_else(|| anyhow!("{}", ea_app::no_handoff()))?;
    say("ea app", &format!("asking the EA app to launch F1 25: {url}"));
    say_elevation();
    let mut tracker = Tracker::new(Instant::now(), &ea_diag::running_watched());
    say(
        "ea app",
        &format!(
            "launch link sent; waiting up to {} s for F1 25",
            HANDOFF_TIMEOUT.as_secs()
        ),
    );
    let state = store.root.join("state");
    std::fs::create_dir_all(&state)?;
    let log = std::fs::File::create(state.join(HANDOFF_LOG))?;
    let mut c = h.command(&url);
    c.stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut c, 0);
    let mut child = Some(c.spawn().map_err(|e| handoff_failed(store, &c, &h, &e))?);
    let pid = wait_for_game(
        &g.dir,
        &[],
        HANDOFF_TIMEOUT,
        &mut child,
        Some(&mut tracker),
        || Ok(()),
    )?
    .ok_or_else(|| {
        anyhow!(
            "F1 25 did not start within {} min of the launch link; press Play on F1 25 in the EA app",
            HANDOFF_TIMEOUT.as_secs() / 60
        )
    })?;
    say("ea app", &format!("F1 25 started (pid {pid})"));
    if let Some(line) = command_line(pid) {
        say("ea app", &format!("command line: {line}"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn command_line(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let words: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|w| !w.is_empty())
        .map(|w| String::from_utf8_lossy(w).into_owned())
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(not(target_os = "linux"))]
fn command_line(_: u32) -> Option<String> {
    None
}

pub struct Started {
    pid: u32,
    handoff: Option<Child>,
}

/// Until every game of this copy started since the launch has closed. Only
/// the game the EA app started for the session, and games that are this
/// copy's, are closed on a stop. The EA app owns the game's process, so its
/// exit code is not the session's to read.
pub fn wait(game: &Path, dir: &Path, before: &[u32], started: Started, mut tick: impl FnMut()) {
    let mut asked = false;
    loop {
        let running: Vec<GameProcess> = game_processes()
            .into_iter()
            .filter(|g| !before.contains(&g.pid) && !g.elsewhere(game))
            .collect();
        if running.is_empty() {
            break;
        }
        tick();
        let phase = session_stop(dir);
        if phase == StopPhase::GiveUp {
            say(
                "info",
                "F1 25 is still open 20 s after Stop; rolling the game files back without waiting for it",
            );
            break;
        }
        if !asked && phase != StopPhase::Running {
            asked = true;
            say(
                "info",
                "asked to stop: closing F1 25, then rolling its game files back",
            );
            for g in running.iter().filter(|g| g.pid == started.pid || g.is_in(game)) {
                stop_process(g.pid).ok();
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if let Some(mut c) = started.handoff {
        std::thread::spawn(move || c.wait().ok());
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn the_active_copy_picks_the_store_to_start() {
        assert_eq!(store_start(Source::Steam), StoreStart::Steam);
        assert_eq!(store_start(Source::EaApp), StoreStart::EaApp);
        assert_eq!(store_start(Source::Unknown), StoreStart::Steam);
    }

    #[test]
    fn starts_the_ea_app_only_when_it_can_and_may() {
        assert_eq!(policy(false, true, true), Policy::NoHandoff);
        assert_eq!(policy(true, false, true), Policy::Running);
        assert_eq!(policy(true, true, false), Policy::Start);
        assert_eq!(policy(true, false, false), Policy::Off);
    }

    #[test]
    fn stops_waiting_when_the_open_command_fails() {
        if ea_app::running() {
            return;
        }
        let mut fail = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/c", "exit 3"]);
            c
        } else {
            std::process::Command::new("false")
        };
        let mut child = fail.spawn().unwrap();
        assert!(matches!(
            wait_for_app(&mut child, true, || false),
            AppStart::CommandFailed(_)
        ));
        let mut child = fail.spawn().unwrap();
        let polls = std::cell::Cell::new(0);
        let timed_out = || {
            polls.set(polls.get() + 1);
            polls.get() > 3
        };
        assert_eq!(wait_for_app(&mut child, false, timed_out), AppStart::TimedOut);
        let mut ok = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/c", "exit 0"]);
            c
        } else {
            std::process::Command::new("true")
        };
        let mut child = ok.spawn().unwrap();
        let polls = std::cell::Cell::new(0);
        let timed_out = || {
            polls.set(polls.get() + 1);
            polls.get() > 3
        };
        assert_eq!(wait_for_app(&mut child, true, timed_out), AppStart::TimedOut);
    }

    #[test]
    fn a_vanilla_session_through_the_ea_app_leaves_the_anti_cheat_launcher_alone() {
        assert!(!stages_offline(true, true));
        assert!(stages_offline(true, false));
        assert!(stages_offline(false, true));
        assert!(stages_offline(false, false));
    }

    #[test]
    fn says_the_ea_app_is_repairing_when_it_took_offline_plays_file_out() {
        for slot in [Slot::Missing, Slot::Replaced] {
            let (line, status) = waiting(Some(slot));
            assert!(
                line.starts_with("the EA app took offline play's file out"),
                "{line}"
            );
            assert_eq!(status, "The EA app is repairing F1 25");
        }
        for slot in [Some(Slot::Staged), Some(Slot::Unknown), None] {
            let (line, status) = waiting(slot);
            assert!(
                line.starts_with("F1 25 did not start from the launch link"),
                "{line}"
            );
            assert_eq!(status, "Press Play on F1 25 in the EA app");
        }
    }

    #[test]
    fn a_genuine_repair_still_waits_for_the_ea_app() {
        let repairing = NoGame::Wait {
            line: "the EA app took offline play's file out to repair F1 25; F1 25 starts without mods after the \
                   repair",
            status: "The EA app is repairing F1 25",
        };
        assert_eq!(no_game(Some(Slot::Replaced), || None), repairing);
        assert_eq!(no_game(Some(Slot::Missing), || None), repairing);
        assert_eq!(
            no_game(Some(Slot::Replaced), || panic!("a replaced file is the EA app's")),
            repairing
        );
        let block = offline::stub_removed(Path::new("F1 25"));
        assert_eq!(
            no_game(Some(Slot::Missing), || Some(block.clone())),
            NoGame::Antivirus(block)
        );
        assert!(matches!(
            no_game(Some(Slot::Staged), || None),
            NoGame::Wait {
                status: "Press Play on F1 25 in the EA app",
                ..
            }
        ));
    }

    #[cfg(unix)]
    fn ea_copy(name: &str) -> (PathBuf, PathBuf, Store, PathBuf) {
        let root = std::env::temp_dir().join(format!("swofty-ea-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("F1 25");
        std::fs::create_dir_all(game.join("__Installer")).unwrap();
        std::fs::write(
            game.join(ea_app::MARKER),
            "<contentIDs><contentID>16425845</contentID><contentID>16425845_oa</contentID></contentIDs>",
        )
        .unwrap();
        let store = Store::new(root.join("data"));
        std::fs::create_dir_all(&store.root).unwrap();
        let got = root.join("url.txt");
        let mut config = store.config();
        config.launch.ea_launch_command = Some(format!("printf %s {{url}} > '{}'", got.display()));
        store.save_config(&config).unwrap();
        (root, game, store, got)
    }

    #[cfg(unix)]
    fn stage_journal(store: &Store, game: &Path, sha256: &str) -> PathBuf {
        let journal = offline::journal_path(&store.root);
        std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
        let slot = game.join("EAAntiCheat.GameServiceLauncher.exe");
        let line = serde_json::json!({
            "op": "copy", "from": "", "to": slot.display().to_string(), "source": "stub", "sha256": sha256,
        });
        std::fs::write(&journal, format!("{line}\n")).unwrap();
        journal
    }

    #[cfg(unix)]
    #[test]
    fn a_stub_gone_seconds_after_staging_stops_before_the_launch_link() {
        let (root, game, store, got) = ea_copy("stub-gone");
        stage_journal(&store, &game, "0123");
        let session = root.join("session");
        std::fs::create_dir_all(&session).unwrap();
        let since = Instant::now();
        let err = start(&store, &game, &[], &session, &[]).err().unwrap();
        assert!(since.elapsed() >= STUB_SETTLE - Duration::from_millis(500));
        let block = offline::stub_removed(&game);
        assert_eq!(err.to_string(), block.text());
        assert!(err.to_string().contains("antivirus"), "{err}");
        assert!(err.to_string().contains(&game.display().to_string()), "{err}");
        assert_eq!(
            launch::Failure::read(&store.root).map(|f| f.cause),
            Some(launch::Cause::Blocked(block))
        );
        assert!(!got.exists(), "the launch link was sent");
        assert!(!session.join(HANDOFF_LOG).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_stub_in_place_after_staging_lets_the_link_go() {
        let (root, game, store, _) = ea_copy("stub-there");
        let slot = game.join("EAAntiCheat.GameServiceLauncher.exe");
        std::fs::write(&slot, "stub").unwrap();
        let sha = swofty_core::baseline::sha256_file(&slot).unwrap();
        let journal = stage_journal(&store, &game, &sha);
        std::fs::File::options()
            .write(true)
            .open(&journal)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(30))
            .unwrap();
        let since = Instant::now();
        check_stub(&store, &game).unwrap();
        assert!(since.elapsed() < Duration::from_secs(1));
        assert_eq!(launch::Failure::read(&store.root), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_handoff_windows_refuses_with_740_names_what_to_untick() {
        let root = std::env::temp_dir().join(format!("swofty-ea-740-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(root.join("data"));
        std::fs::create_dir_all(&store.root).unwrap();
        let exe = r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe";
        let h = Handoff::Program(vec![exe.into(), "%1".into()]);
        let c = h.command("origin2://game/launch?offerIds=16425845");
        let err = handoff_failed(&store, &c, &h, &std::io::Error::from_raw_os_error(740)).to_string();
        assert!(
            err.starts_with("EALauncher.exe is set to run as administrator, so Windows won't start it"),
            "{err}"
        );
        assert!(
            err.contains(&format!("Right-click {exe}, open Properties > Compatibility")),
            "{err}"
        );
        assert!(err.contains("\"Change settings for all users\""), "{err}");
        assert!(!err.contains("Swofty Launcher as administrator"), "{err}");
        match launch::Failure::read(&store.root).map(|f| f.cause) {
            Some(launch::Cause::RunAsAdmin(r)) => {
                assert_eq!(r.exes.first(), Some(&std::path::PathBuf::from(exe)));
                assert!(!r.game);
            }
            other => panic!("{other:?}"),
        }
        launch::Failure::clear(&store.root).unwrap();
        let err = handoff_failed(&store, &c, &h, &std::io::Error::from_raw_os_error(2)).to_string();
        assert!(
            err.starts_with(&format!("could not run {h} to hand the launch link")),
            "{err}"
        );
        assert_eq!(launch::Failure::read(&store.root), None);
        assert!(refused_elevation(&store, c.get_program(), &std::io::Error::other("x")).is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    const REFUSED_LINE: &str = "10:06:01.380 [stub] [offline] CreateProcessW failed, error 740 \
                                (ERROR_ELEVATION_REQUIRED): Windows will only start F1_25.exe as administrator";

    #[test]
    fn only_the_session_launchers_lines_since_the_link_count() {
        let root = std::env::temp_dir().join(format!("swofty-ea-stub-log-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let log = root.join("loader.log");
        let since = |log: &Path| (LogMark::read(log), std::fs::metadata(log).map_or(0, |m| m.len()));
        assert_eq!(written_after(&log, None, 0), "");
        std::fs::write(&log, format!("{REFUSED_LINE}\n")).unwrap();
        let (mark, from) = since(&log);
        assert!(!compat::stub_needs_elevation(&written_after(
            &log,
            mark.as_ref(),
            from
        )));
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .and_then(|mut f| std::io::Write::write_all(&mut f, format!("{REFUSED_LINE}\n").as_bytes()))
            .unwrap();
        assert!(compat::stub_needs_elevation(&written_after(
            &log,
            mark.as_ref(),
            from
        )));
        std::fs::write(
            &log,
            "10:07:00.000 [stub] [offline] session launcher invoked, args: <none>\n",
        )
        .unwrap();
        let (mark, from) = since(&log);
        std::fs::write(&log, format!("{REFUSED_LINE}\n")).unwrap();
        assert!(
            compat::stub_needs_elevation(&written_after(&log, mark.as_ref(), from)),
            "a log started again counts from its start"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_session_launcher_windows_refuses_with_740_ends_the_wait_at_once() {
        let (root, game, store, _) = ea_copy("stub-740");
        let log = game.join(LOADER_LOG);
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, format!("{REFUSED_LINE} (an earlier session)\n")).unwrap();
        let mut config = store.config();
        config.launch.ea_launch_command = Some(format!(
            ": {{url}}; sleep 0.3; printf '%s\\n' '{REFUSED_LINE}' >> '{}'",
            log.display()
        ));
        store.save_config(&config).unwrap();
        let session = root.join("session");
        std::fs::create_dir_all(&session).unwrap();
        let before: Vec<u32> = game_processes().iter().map(|g| g.pid).collect();
        let since = Instant::now();
        let err = start(&store, &game, &[], &session, &before).err().unwrap();
        assert!(since.elapsed() < Duration::from_secs(10), "{:?}", since.elapsed());
        let refused = compat::game_refused_in(&[], &game);
        assert_eq!(err.to_string(), refused.text());
        assert!(
            err.to_string()
                .contains("running Swofty Launcher as administrator doesn't help"),
            "{err}"
        );
        assert_eq!(
            launch::Failure::read(&store.root).map(|f| f.cause),
            Some(launch::Cause::RunAsAdmin(refused))
        );
        assert!(!store.root.join(steam_client::STAGE_FILE).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn hands_the_link_to_the_configured_command() {
        let root = std::env::temp_dir().join(format!("swofty-ea-handoff-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("F1 25");
        std::fs::create_dir_all(game.join("__Installer")).unwrap();
        std::fs::write(
            game.join(ea_app::MARKER),
            "<contentIDs><contentID>16425845</contentID><contentID>16425845_oa</contentID></contentIDs>",
        )
        .unwrap();
        let store = Store::new(root.join("data"));
        std::fs::create_dir_all(&store.root).unwrap();
        let got = root.join("url.txt");
        let mut config = store.config();
        config.launch.ea_launch_command = Some(format!("printf %s {{url}} > '{}'", got.display()));
        store.save_config(&config).unwrap();
        let url = ea_app::install_url(&game, &["-vr".into()]).unwrap();
        let mut c = handoff(&store).unwrap().command(&url);
        assert!(c.status().unwrap().success());
        assert_eq!(
            std::fs::read_to_string(&got).unwrap(),
            "origin2://game/launch?offerIds=16425845,16425845_oa&autoDownload=1&cmdParams=-vr"
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
