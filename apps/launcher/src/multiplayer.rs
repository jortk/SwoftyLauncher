use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog;
use swofty_core::install::Game;
use swofty_core::multiplayer::client::{self as diag, Local, Session, Setup};
use swofty_core::multiplayer::types::{
    GameStatus, MemberStage, MemberState, Phase, Problem, Progress, RoomAddon, RoomContent, RoomSettings,
    RoomSummary, Screen, SessionState, Step,
};
use swofty_core::store::Store;
use swofty_i18n::{fill, tr, trf, trn};

use crate::pages::{GUTTER, badge, label, monogram, tag};
use crate::shell::{Page, Shell};
use crate::theme::{Palette, ScrollArea};

const TICK: Duration = Duration::from_millis(500);
const GAME_EVERY: Duration = Duration::from_secs(2);
const ROOMS_EVERY: Duration = Duration::from_secs(5);
const MIN_PLAYERS: u8 = 2;
const MAX_PLAYERS: u8 = 24;
const AVATARS_DIR: &str = "avatars";

/// Where the game is, in the page header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameState {
    NotRunning,
    MainMenu,
    LanLobby,
    Busy,
}

impl GameState {
    pub(crate) fn of(running: bool, status: Option<&GameStatus>) -> Self {
        if !running {
            return Self::NotRunning;
        }
        match status.map(|s| s.screen) {
            Some(Screen::MainMenu) => Self::MainMenu,
            Some(Screen::LanLobby) => Self::LanLobby,
            Some(Screen::Other) | None => Self::Busy,
        }
    }

    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::NotRunning => "F1 25 isn't running",
            Self::MainMenu => "F1 25 is on the main menu",
            Self::LanLobby => "F1 25 is in a LAN lobby",
            Self::Busy => "F1 25 is busy",
        }
    }
}

/// The room row's second line: host and players.
pub(crate) fn room_meta(room: &RoomSummary) -> String {
    format!("{} \u{00b7} {}", room.host.name, room_meta_players(room))
}

pub(crate) fn room_meta_players(room: &RoomSummary) -> String {
    format!("{}/{} players", room.members, room.max_members)
}

/// What a room's one status line says, and whether it needs the player's attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoomStatus {
    Full,
    Differ,
    Match,
    Unknown,
}

impl RoomStatus {
    pub(crate) fn of(room: &RoomSummary, mine: Option<&str>) -> Self {
        if room.members >= room.max_members {
            return Self::Full;
        }
        match (room.digest.as_deref(), mine) {
            (Some(host), Some(mine)) if host.eq_ignore_ascii_case(mine) => Self::Match,
            (Some(_), Some(_)) => Self::Differ,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Differ => "Your addons differ",
            Self::Match => "Your addons match",
            Self::Unknown => "",
        }
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Working,
    Good,
    Blocked,
    Over,
}

pub(crate) const STEPS: [(Step, &str); 6] = [
    (Step::SigningIn, "Signing in"),
    (Step::Joining, "Joining"),
    (Step::Connecting, "Connecting"),
    (Step::StartingGame, "Starting F1 25"),
    (Step::Loading, "Loading"),
    (Step::InLobby, "In the lobby"),
];

pub(crate) fn step_number(step: Step) -> usize {
    STEPS.iter().position(|(s, _)| *s == step).unwrap_or(0)
}

/// What the status area shows: the step, how far along, and the problem when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) tone: Tone,
    pub(crate) progress: Progress,
    pub(crate) problem: Option<Problem>,
    pub(crate) hint: Option<&'static str>,
}

pub(crate) fn room_status(state: &SessionState) -> Status {
    let progress = state.step();
    if let Phase::Ended(why) = &state.phase {
        return Status {
            tone: Tone::Over,
            progress: Progress {
                label: capitalised(why),
                ..progress
            },
            problem: None,
            hint: None,
        };
    }
    let problem = state.problem.clone();
    let (tone, hint) = match (&problem, progress.step, state.host, &state.phase) {
        (Some(_), ..) => (Tone::Blocked, None),
        (None, Step::InLobby, true, Phase::Racing) => (
            Tone::Good,
            Some("Mark the race over when everyone is back in the lobby."),
        ),
        (None, Step::InLobby, true, _) => (
            Tone::Good,
            Some("Pick tracks and settings in F1 25's lobby, then start the race there."),
        ),
        (None, Step::InLobby, false, _) => {
            (Tone::Good, Some("The host picks the tracks and starts the race."))
        }
        (None, ..) => (Tone::Working, None),
    };
    Status {
        tone,
        progress,
        problem,
        hint,
    }
}

/// F1 25 closing while the player is in a room that started it leaves the room: `seen` is that the room's
/// game has been running, `busy` that a launch, build or restore is still going.
pub(crate) fn exit_leaves(seen: bool, running: bool, busy: bool) -> bool {
    seen && !running && !busy
}

/// The restart dialog's button: what pressing it does.
fn restart_ok_text(start: &Start) -> &'static str {
    match start {
        Start::Create(_) => "Create room and join",
        Start::Join(_) => "Join room",
    }
}

pub(crate) fn exit_notice(host: bool) -> &'static str {
    if host {
        "F1 25 closed, so your room closed."
    } else {
        "F1 25 closed, so you left the room."
    }
}

/// What the page shows in place of its list while the session module isn't loaded: the sentence the last
/// load failed with, kept while another load runs, gone once one succeeds.
pub(crate) fn refusal(shown: Option<String>, loaded: bool, failed: Option<String>) -> Option<String> {
    if loaded {
        return None;
    }
    failed.or(shown)
}

pub(crate) fn refusal_text(why: &str) -> String {
    trf!("Online sessions aren't available: {why}", why = why)
}

/// One member's state as the room lists it: what their game is doing. Only a game in the lobby counts as joined.
pub(crate) fn member_word(m: &MemberState) -> (&'static str, Tone) {
    if !m.connected {
        return ("Disconnected", Tone::Blocked);
    }
    match m.stage {
        Some(MemberStage::InLobby) => (MemberStage::InLobby.word(), Tone::Good),
        Some(stage) => (stage.word(), Tone::Working),
        None => ("Joining\u{2026}", Tone::Working),
    }
}

/// A member's word when the host picked the F1 2026 cars and their game says it doesn't have that content.
pub(crate) fn content_word(content: &RoomContent, m: &MemberState) -> Option<(&'static str, Tone)> {
    (m.connected && content.needs_f1_2026() && content.owns_f1_2026.get(&m.id) == Some(&false))
        .then_some(("No F1 2026 content", Tone::Blocked))
}

/// This player's row for the host's F1 2026 cars: whether their game has the content, `None` until the host
/// picked them.
pub(crate) fn content_check(state: &SessionState) -> Option<(Option<bool>, &'static str)> {
    if !state.content.needs_f1_2026() {
        return None;
    }
    let me = state.members.iter().find(|m| m.me)?;
    let owns = state.content.owns_f1_2026.get(&me.id).copied();
    Some((
        owns,
        match owns {
            Some(true) => "You have the F1 2026 content",
            Some(false) => "Your copy of F1 25 doesn't have the F1 2026 content",
            None => "F1 25 hasn't said yet whether you have it",
        },
    ))
}

pub(crate) fn joined(state: &SessionState) -> usize {
    state
        .members
        .iter()
        .filter(|m| m.connected && m.stage == Some(MemberStage::InLobby))
        .count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddonCheck {
    pub(crate) name: String,
    pub(crate) have: bool,
}

/// The room's addons, each with whether this player has it on.
pub(crate) fn addon_checks(addons: &[RoomAddon], on: &HashSet<String>) -> Vec<AddonCheck> {
    addons
        .iter()
        .map(|a| AddonCheck {
            name: a.name.clone(),
            have: on.contains(&a.id),
        })
        .collect()
}

pub(crate) fn missing(checks: &[AddonCheck]) -> usize {
    checks.iter().filter(|c| !c.have).count()
}

/// The names a room card shows: every addon the host has on when the server lists them, else its tracks.
pub(crate) fn card_addons(room: &RoomSummary) -> Vec<String> {
    match &room.addons {
        Some(a) if !a.list.is_empty() => a.list.iter().map(|x| x.name.clone()).collect(),
        Some(a) => a.tracks.clone(),
        None => Vec::new(),
    }
}

fn read_local(store: &Store, profile: &str, dir: &Path) -> Result<Local, String> {
    let game = Game::locate(&store.root, dir.to_path_buf()).map_err(|e| e.to_string())?;
    let profile = store.profile(profile).map_err(|e| e.to_string())?;
    Local::read(store, &profile, &game.identity).map_err(|e| e.to_string())
}

fn read_game(dir: &Path, running: bool) -> Option<GameStatus> {
    diag::game_status(dir, running)
}

#[derive(Clone)]
enum Start {
    Create(RoomSettings),
    Join(String),
}

pub(crate) struct RoomLaunch {
    pub(crate) row_ids: Option<PathBuf>,
    /// The direct start request the game reads at boot, as `swofty\direct_start.json`: the room target, this
    /// player's role, the host's address and the room configuration (`LaunchAsk::direct_start`).
    pub(crate) direct_start: String,
}

pub(crate) struct Multiplayer {
    visible: bool,
    game: Option<GameStatus>,
    game_at: Option<Instant>,
    game_polling: bool,
    rooms: Vec<RoomSummary>,
    rooms_loaded: bool,
    rooms_error: Option<String>,
    refused: Option<String>,
    rooms_at: Option<Instant>,
    rooms_polling: bool,
    digest: Option<String>,
    digest_busy: bool,
    form: bool,
    name: Entity<InputState>,
    code: Entity<InputState>,
    public: bool,
    max: u8,
    starting: Option<String>,
    session: Option<Session>,
    state: Option<SessionState>,
    leaving: bool,
    viewing: Option<String>,
    notice: Option<String>,
    closing: Option<Vec<u32>>,
    game_seen: bool,
    error: Option<String>,
    copied: Option<Instant>,
    launch_seen: u32,
    room_launching: bool,
    held: Option<RoomLaunch>,
    faces: HashMap<String, Arc<Image>>,
    faces_asked: HashSet<String>,
    _inputs: [Subscription; 2],
}

impl Multiplayer {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Room name")));
        let code = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Invite code")));
        let changed = |cx: &mut Context<Shell>, input: &Entity<InputState>| {
            cx.subscribe(input, |_, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            })
        };
        let inputs = [changed(cx, &name), changed(cx, &code)];
        Self {
            visible: false,
            game: None,
            game_at: None,
            game_polling: false,
            rooms: Vec::new(),
            rooms_loaded: false,
            rooms_error: None,
            refused: None,
            rooms_at: None,
            rooms_polling: false,
            digest: None,
            digest_busy: false,
            form: false,
            name,
            code,
            public: true,
            max: 8,
            starting: None,
            session: None,
            state: None,
            leaving: false,
            viewing: None,
            notice: None,
            closing: None,
            game_seen: false,
            error: None,
            copied: None,
            launch_seen: 0,
            room_launching: false,
            held: None,
            faces: HashMap::new(),
            faces_asked: HashSet::new(),
            _inputs: inputs,
        }
    }

    fn busy(&self) -> bool {
        self.starting.is_some() || self.session.is_some()
    }

    fn finish_leaving(&mut self) {
        self.session = None;
        self.launch_seen = 0;
        self.room_launching = false;
        self.held = None;
        self.state = None;
        self.leaving = false;
        self.closing = None;
        self.game_seen = false;
        self.viewing = None;
        self.rooms_at = None;
        self.game_at = None;
        self.digest = None;
    }
}

fn due(at: Option<Instant>, every: Duration, now: Instant) -> bool {
    at.is_none_or(|at| now.duration_since(at) >= every)
}

impl Shell {
    pub(crate) fn mp_allowed(&self) -> bool {
        self.mp.as_ref().is_some_and(|m| m.allowed)
    }

    pub(crate) fn watch_multiplayer(window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update_in(cx, |s, window, cx| s.mp_tick(window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn mp_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mp_room_launch(window, cx);
        self.mp_held_dropped(window, cx);
        self.mp_watch_game(cx);
        let m = &mut self.mp_page;
        if m.leaving && m.session.as_ref().is_none_or(Session::is_finished) {
            m.finish_leaving();
            cx.notify();
        }
        if self.page != Page::Multiplayer || !self.mp_allowed() {
            self.mp_page.visible = false;
            return;
        }
        let now = Instant::now();
        let m = &mut self.mp_page;
        if !m.visible {
            m.visible = true;
            m.rooms_at = None;
            m.game_at = None;
            m.digest = None;
        }
        let refused = refusal(m.refused.clone(), diag::is_loaded(), diag::unavailable());
        if refused != m.refused {
            if refused.is_none() {
                m.rooms_at = None;
            }
            m.refused = refused;
            cx.notify();
        }
        if let Some(session) = &m.session {
            let state = session.state();
            if m.state.as_ref() != Some(&state) {
                m.state = Some(state);
                cx.notify();
            }
        }
        if m.copied
            .is_some_and(|at| now.duration_since(at) > Duration::from_secs(2))
        {
            m.copied = None;
            cx.notify();
        }
        if m.session.is_none() && !m.game_polling && due(m.game_at, GAME_EVERY, now) {
            self.mp_poll_game(cx);
        }
        let m = &self.mp_page;
        if m.session.is_none() && !m.rooms_polling && due(m.rooms_at, ROOMS_EVERY, now) {
            self.mp_poll_rooms(cx);
        }
        let m = &self.mp_page;
        if m.digest.is_none() && !m.digest_busy && m.session.is_none() {
            self.mp_read_digest(cx);
        }
        self.mp_load_faces(cx);
    }

    /// Starts F1 25 for the room when the session asks, the game is closed and the launcher has put the
    /// game folder back.
    fn mp_room_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.mp_page.session else {
            return;
        };
        let Some(ask) = session.state().launch else {
            return;
        };
        if self.mp_page.launch_seen == ask.seq || self.launch.is_some() || self.build.is_some() {
            return;
        }
        if !self.processes.is_empty() || self.restoring {
            return;
        }
        self.mp_page.closing = None;
        if let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) {
            diag::forget_status(&dir);
        }
        let row_ids = match &ask.row_ids {
            Some(ids) => {
                let file = self.store.root.join("state/room-row-ids.json");
                if let Err(e) = std::fs::create_dir_all(self.store.root.join("state"))
                    .map_err(|e| e.to_string())
                    .and_then(|()| ids.write(&file).map_err(|e| e.to_string()))
                {
                    self.mp_page.launch_seen = ask.seq;
                    session.launch_failed(&format!("the host's ids and settings couldn't be written ({e})"));
                    return;
                }
                Some(file)
            }
            None => None,
        };
        self.mp_page.launch_seen = ask.seq;
        self.mp_page.game_seen = false;
        self.mp_page.room_launching = true;
        let direct_start = ask.direct_start.clone();
        let started = self.play_for(
            Some(RoomLaunch {
                row_ids,
                direct_start,
            }),
            window,
            cx,
        );
        if !started {
            self.mp_room_launch_result(false);
        }
        cx.notify();
    }

    /// A prompt Play opened holds the room launch: its Play goes on with it through [`Shell::play`].
    pub(crate) fn mp_hold_room(&mut self, room: RoomLaunch) {
        self.mp_page.held = Some(room);
    }

    pub(crate) fn mp_take_held(&mut self) -> Option<RoomLaunch> {
        self.mp_page.held.take()
    }

    fn mp_held_dropped(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mp_page.held.is_none() || window.has_active_dialog(cx) {
            return;
        }
        self.mp_page.held = None;
        self.mp_page.room_launching = false;
        if let Some(session) = &self.mp_page.session {
            session.launch_failed("Play was cancelled");
        }
        cx.notify();
    }

    pub(crate) fn mp_room_launch_result(&mut self, started: bool) {
        let Some(session) = &self.mp_page.session else {
            return;
        };
        if started {
            session.launched();
        } else {
            self.mp_page.room_launching = false;
            session.launch_failed("see the notice, finish what it asks, then try again");
        }
    }

    /// The launch `mp_room_launch` started ended without the game: `why` says what stopped it.
    pub(crate) fn mp_launch_ended(&mut self, why: Option<String>) {
        if !std::mem::take(&mut self.mp_page.room_launching) {
            return;
        }
        match why {
            None => self.mp_page.game_seen = true,
            Some(why) => {
                if let Some(session) = &self.mp_page.session {
                    session.launch_failed(&why);
                }
            }
        }
    }

    fn mp_watch_game(&mut self, cx: &mut Context<Self>) {
        let busy = self.launch.is_some() || self.build.is_some() || self.restoring;
        let m = &mut self.mp_page;
        if m.session.is_none() {
            return;
        }
        if !self.processes.is_empty() && m.launch_seen != 0 && m.closing.is_none() {
            m.game_seen = true;
        }
        if m.closing.is_some() && self.processes.is_empty() {
            m.closing = None;
        }
        let over = m.state.as_ref().is_some_and(|s| s.phase.is_over());
        if over || m.leaving || !exit_leaves(m.game_seen, !self.processes.is_empty(), busy) {
            return;
        }
        let host = m.state.as_ref().is_some_and(|s| s.host);
        m.notice = Some(exit_notice(host).to_owned());
        diag::line(format!(
            "launcher: F1 25 exited; leaving the room ({})",
            exit_notice(host)
        ));
        self.mp_leave(cx);
    }

    fn mp_close_game(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            return true;
        };
        let pids: Vec<u32> = self
            .processes
            .iter()
            .filter(|g| g.is_in(&dir))
            .map(|g| g.pid)
            .collect();
        if pids.is_empty() {
            return true;
        }
        swofty_core::session::request_stop(&self.store.root);
        for pid in &pids {
            if let Err(e) = swofty_core::process::stop_process(*pid) {
                crate::app_log::action("stop f1 25").failed(&e);
                window.notify(Notification::error(e.to_string()), cx);
                return false;
            }
        }
        self.mp_page.closing = Some(pids);
        true
    }

    fn mp_begin(&mut self, start: Start, window: &mut Window, cx: &mut Context<Self>) {
        if self.mp_page.busy() || !self.mp_close_game(window, cx) {
            return;
        }
        self.mp_page.notice = None;
        self.mp_start(start, cx);
    }

    fn mp_play(&mut self, start: Start, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.install.as_ref().map(|i| i.dir.clone());
        let running = dir
            .as_ref()
            .is_some_and(|d| self.processes.iter().any(|g| g.is_in(d)));
        if !running {
            self.mp_begin(start, window, cx);
            return;
        }
        let shell = cx.entity().downgrade();
        let ok = restart_ok_text(&start);
        // The dialog's builder runs on every frame it draws, so each run gets its own copy of the start.
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, start) = (shell.clone(), start.clone());
            alert
                .title(tr("Restart F1 25?"))
                .description(tr(
                    "F1 25 is open, so it restarts for this room. Unsaved progress is lost.",
                ))
                .ok_text(ok)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    shell
                        .update(cx, |s, cx| s.mp_begin(start.clone(), window, cx))
                        .ok();
                    true
                })
        });
    }

    pub(crate) fn mp_view_play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(code) = self.mp_page.viewing.clone() else {
            return;
        };
        self.go(Page::Multiplayer, cx);
        self.mp_play(Start::Join(code), window, cx);
    }

    pub(crate) fn mp_viewed_room(&self) -> Option<&RoomSummary> {
        let code = self.mp_page.viewing.as_deref()?;
        self.mp_page.rooms.iter().find(|r| r.code == code)
    }

    /// The play button's word while a shared session starts and F1 25 isn't building or running yet, so it
    /// can't be pressed as a second Play.
    pub(crate) fn mp_starting_label(&self) -> Option<&'static str> {
        let m = &self.mp_page;
        if !m.busy() {
            return None;
        }
        let hosting = m.starting.as_deref() == Some("create") || m.state.as_ref().is_some_and(|s| s.host);
        Some(if hosting {
            tr("Opening room…")
        } else {
            tr("Joining…")
        })
    }

    pub(crate) fn mp_can_play_room(&self) -> bool {
        self.page == Page::Multiplayer
            && self.mp_allowed()
            && !self.mp_page.busy()
            && self.mp_viewed_room().is_some()
    }

    fn mp_load_faces(&mut self, cx: &mut Context<Self>) {
        let m = &mut self.mp_page;
        let members = m
            .state
            .iter()
            .flat_map(|s| &s.members)
            .filter_map(|x| x.avatar.as_ref());
        let hosts = m.rooms.iter().filter_map(|r| r.host.avatar.as_ref());
        let wanted: Vec<String> = members.chain(hosts).cloned().collect();
        self.load_faces(wanted, cx);
    }

    pub(crate) fn load_faces(&mut self, urls: Vec<String>, cx: &mut Context<Self>) {
        let m = &mut self.mp_page;
        let wanted: Vec<String> = urls
            .into_iter()
            .filter(|u| u.starts_with("http") && !m.faces_asked.contains(u))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if wanted.is_empty() {
            return;
        }
        m.faces_asked.extend(wanted.iter().cloned());
        let dir = self.store.root.join(catalog::CACHE_DIR).join(AVATARS_DIR);
        cx.spawn(async move |this, cx| {
            let faces: Vec<(String, Arc<Image>)> = cx
                .background_executor()
                .spawn(async move {
                    wanted
                        .into_iter()
                        .filter_map(|url| crate::news::image(&dir, &url).map(|i| (url, i)))
                        .collect()
                })
                .await;
            this.update(cx, |s, cx| {
                s.mp_page.faces.extend(faces);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn face(&self, name: &str, url: Option<&str>, size: f32, p: Palette) -> Div {
        match url.and_then(|u| self.mp_page.faces.get(u)) {
            Some(image) => div()
                .size(px(size))
                .flex_shrink_0()
                .rounded_full()
                .overflow_hidden()
                .child(img(image.clone()).size_full()),
            None => monogram(name, size, p).rounded_full(),
        }
    }

    fn mp_poll_game(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            self.mp_page.game = None;
            self.mp_page.game_at = Some(Instant::now());
            return;
        };
        let running = !self.processes.is_empty();
        self.mp_page.game_polling = true;
        cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async move { read_game(&dir, running) })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.game_polling = false;
                m.game_at = Some(Instant::now());
                if m.game != status {
                    m.game = status;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn mp_poll_rooms(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.mp.as_ref().map(|m| m.token.clone()) else {
            return;
        };
        self.mp_page.rooms_polling = true;
        let data = self.store.root.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    diag::load(&data, Some(&token)).map_err(|e| e.to_string())?;
                    diag::rooms(&token)
                })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.rooms_polling = false;
                m.rooms_at = Some(Instant::now());
                match result {
                    Ok(rooms) => {
                        m.rooms = rooms;
                        m.rooms_loaded = true;
                        m.rooms_error = None;
                    }
                    Err(e) => m.rooms_error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn mp_read_digest(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            return;
        };
        let (store, profile) = (self.store.clone(), self.library.profile.name.clone());
        self.mp_page.digest_busy = true;
        cx.spawn(async move |this, cx| {
            let digest = cx
                .background_executor()
                .spawn(async move { read_local(&store, &profile, &dir).map(|l| l.digest()) })
                .await;
            this.update(cx, |s, cx| {
                s.mp_page.digest_busy = false;
                match digest {
                    Ok(d) => s.mp_page.digest = Some(d),
                    Err(e) => eprintln!("swofty-launcher: couldn't read this profile's addons: {e}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn mp_game_state(&self) -> GameState {
        let status = match &self.mp_page.state {
            Some(state) if self.mp_page.session.is_some() => state.game.as_ref(),
            _ => self.mp_page.game.as_ref(),
        };
        GameState::of(!self.processes.is_empty(), status)
    }

    fn mp_start(&mut self, start: Start, cx: &mut Context<Self>) {
        if self.mp_page.busy() {
            return;
        }
        let Some(token) = self.mp.as_ref().map(|m| m.token.clone()) else {
            return;
        };
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            self.mp_page.error = Some("F1 25 was not found. Locate it from the play bar first.".into());
            cx.notify();
            return;
        };
        let (store, profile) = (self.store.clone(), self.library.profile.name.clone());
        let m = &mut self.mp_page;
        m.starting = Some(match &start {
            Start::Create(_) => "create".into(),
            Start::Join(code) => code.clone(),
        });
        m.error = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    diag::load(&store.root, Some(&token)).map_err(|e| e.to_string())?;
                    let local = read_local(&store, &profile, &dir)
                        .inspect_err(|e| {
                            diag::line(format!("session not started: couldn't read the addons: {e}"));
                        })
                        .map_err(|e| trf!("Couldn't read your addons: {e}", e = e).to_string())?;
                    let mut setup = Setup::new(&token, &dir, local);
                    setup.room_launch = true;
                    let (refresh_store, refresh_profile, refresh_dir) =
                        (store.clone(), profile.clone(), dir.clone());
                    setup.refresh = Some(Arc::new(move || {
                        read_local(&refresh_store, &refresh_profile, &refresh_dir).ok()
                    }));
                    match start {
                        Start::Create(settings) => Session::create(setup, &settings),
                        Start::Join(code) => Session::join(setup, &code),
                    }
                })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.starting = None;
                match result {
                    Ok(session) => {
                        m.state = Some(session.state());
                        m.session = Some(session);
                        m.form = false;
                    }
                    Err(e) => m.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn mp_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.mp_page.name.read(cx).value().trim().to_owned();
        let name = if typed.is_empty() {
            self.mp_default_name()
        } else {
            typed
        };
        let settings = RoomSettings {
            name: Some(name),
            public: self.mp_page.public,
            max_members: self.mp_page.max,
            ..RoomSettings::default()
        };
        self.mp_play(Start::Create(settings), window, cx);
    }

    fn mp_default_name(&self) -> String {
        let you = self.mp.as_ref().map_or("Your", |m| m.name.as_str());
        format!("{you}'s room")
    }

    fn mp_open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mp_page.form = !self.mp_page.form;
        if self.mp_page.form && self.mp_page.name.read(cx).value().is_empty() {
            let name = self.mp_default_name();
            self.mp_page
                .name
                .update(cx, |s, cx| s.set_value(name, window, cx));
        }
        cx.notify();
    }

    fn mp_join_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.mp_page.code.read(cx).value().trim().to_owned();
        if !code.is_empty() {
            self.mp_play(Start::Join(code), window, cx);
        }
    }

    pub(crate) fn mp_leave(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.mp_page.session else {
            return;
        };
        session.leave();
        self.mp_page.leaving = true;
        cx.notify();
    }

    fn mp_retry(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.mp_page.session {
            session.retry();
        }
        cx.notify();
    }

    fn mp_open_room(&mut self, code: String, cx: &mut Context<Self>) {
        self.mp_page.viewing = Some(code);
        cx.notify();
    }

    fn mp_close_view(&mut self, cx: &mut Context<Self>) {
        self.mp_page.viewing = None;
        cx.notify();
    }

    fn mp_copy_code(&mut self, code: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(code));
        self.mp_page.copied = Some(Instant::now());
        cx.notify();
    }

    pub(crate) fn multiplayer_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let in_room = self.mp_page.session.is_some();
        match self.mp_page.state.clone() {
            Some(state) if in_room => self.mp_room(&state, p, cx),
            _ => match self.mp_viewed_room().cloned() {
                Some(room) => self.mp_room_view(&room, p, cx),
                None => self.mp_lobby(p, cx),
            },
        }
    }

    fn game_status(&self, p: Palette) -> Div {
        h_flex()
            .gap(px(6.))
            .flex_shrink_0()
            .text_size(px(13.))
            .text_color(p.text_muted)
            .child(
                Icon::new(IconName::Gamepad2)
                    .size(px(15.))
                    .text_color(p.text_placeholder),
            )
            .child(self.mp_game_state().text())
    }

    fn section_head(&self, title: impl Into<SharedString>, p: Palette) -> Div {
        h_flex()
            .h(px(28.))
            .gap(px(10.))
            .items_center()
            .child(label(title, p))
    }

    fn mp_lobby(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let subtitle =
            "Race with players who use the same addons. The host picks tracks in F1 25's LAN lobby."
                .to_owned();
        let actions = h_flex()
            .gap(px(14.))
            .items_center()
            .child(self.game_status(p))
            .child(
                Button::new("mp-create-open")
                    .outline()
                    .icon(IconName::Plus)
                    .label(tr("Create room"))
                    .disabled(self.mp_page.form)
                    .on_click(cx.listener(|this, _, window, cx| this.mp_open_form(window, cx))),
            );
        let rooms: AnyElement = if let Some(why) = &self.mp_page.refused {
            div()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(refusal_text(why))
                .into_any_element()
        } else if !self.mp_page.rooms_loaded && self.mp_page.rooms_error.is_none() {
            div()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(tr("Loading rooms\u{2026}"))
                .into_any_element()
        } else if self.mp_page.rooms.is_empty() {
            let body = match &self.mp_page.rooms_error {
                Some(_) => tr("The room list didn't load. It tries again in a few seconds."),
                None => tr("Create one, or join a private room with its invite code."),
            };
            v_flex()
                .py(px(40.))
                .items_center()
                .gap(px(8.))
                .rounded(px(6.))
                .border_1()
                .border_color(p.border)
                .child(badge(IconName::Users, 40., p))
                .child(
                    div()
                        .pt(px(6.))
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr("No open rooms")),
                )
                .child(div().text_size(px(13.)).text_color(p.text_muted).child(body))
                .into_any_element()
        } else {
            let list = self.mp_page.rooms.clone();
            v_flex()
                .rounded(px(6.))
                .border_1()
                .border_color(p.border)
                .overflow_hidden()
                .children(
                    list.iter()
                        .enumerate()
                        .map(|(i, r)| self.room_row(r, i == 0, p, cx)),
                )
                .into_any_element()
        };
        v_flex()
            .size_full()
            .child(self.header(Page::Multiplayer.label(), subtitle, actions, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("mp-lobby")
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(24.))
                    .gap(px(24.))
                    .when(self.mp_page.form, |d| d.child(self.create_form(p, cx)))
                    .child(
                        v_flex()
                            .gap(px(8.))
                            .child(
                                self.section_head(
                                    if self.mp_page.rooms_loaded {
                                        trf!("OPEN ROOMS \u{00b7} {count}", count = self.mp_page.rooms.len())
                                    } else {
                                        tr("OPEN ROOMS").to_owned()
                                    },
                                    p,
                                )
                                .child(div().flex_1())
                                .child(self.code_join(p, cx)),
                            )
                            .when_some(self.mp_page.notice.clone(), |d, n| {
                                d.child(div().text_size(px(13.)).text_color(p.text_muted).child(n))
                            })
                            .when_some(self.mp_page.error.clone(), |d, e| {
                                d.child(div().text_size(px(13.)).text_color(p.error).child(e))
                            })
                            .child(rooms),
                    ),
            )
            .into_any_element()
    }

    fn code_join(&self, _p: Palette, cx: &mut Context<Self>) -> Div {
        let typed = !self.mp_page.code.read(cx).value().trim().is_empty();
        let joining = self
            .mp_page
            .starting
            .as_ref()
            .is_some_and(|s| s.as_str() != "create");
        h_flex()
            .gap(px(8.))
            .child(
                div().w(px(170.)).child(
                    Input::new(&self.mp_page.code)
                        .tap_sm()
                        .disabled(self.mp_page.busy()),
                ),
            )
            .child(
                Button::new("mp-join-code")
                    .outline()
                    .tap_sm()
                    .label(tr("Join with code"))
                    .loading(joining && typed)
                    .disabled(!typed || self.mp_page.busy())
                    .on_click(cx.listener(|this, _, window, cx| this.mp_join_code(window, cx))),
            )
    }

    fn room_row(&self, room: &RoomSummary, first: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let status = RoomStatus::of(room, self.mp_page.digest.as_deref());
        let code = room.code.clone();
        let names = card_addons(room);
        h_flex()
            .px(px(16.))
            .py(px(14.))
            .gap(px(14.))
            .items_center()
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(self.face(&room.host.name, room.host.avatar.as_deref(), 36., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(room.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(room_meta(room)),
                    )
                    .when(!names.is_empty(), |d| {
                        d.child(
                            h_flex()
                                .gap(px(6.))
                                .items_start()
                                .text_size(px(12.))
                                .text_color(p.text_placeholder)
                                .child(
                                    Icon::new(IconName::Package)
                                        .size(px(13.))
                                        .mt(px(1.))
                                        .text_color(p.text_placeholder),
                                )
                                .child(div().flex_1().min_w_0().child(names.join(", "))),
                        )
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(13.))
                    .text_color(if status == RoomStatus::Differ {
                        p.warning
                    } else {
                        p.text_muted
                    })
                    .child(status.text()),
            )
            .child(
                Button::new(SharedString::from(format!("mp-view-{}", room.code)))
                    .tap_sm()
                    .outline()
                    .label("View")
                    .disabled(self.mp_page.busy())
                    .on_click(cx.listener(move |this, _, _, cx| this.mp_open_room(code.clone(), cx))),
            )
            .into_any_element()
    }

    fn create_form(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let m = &self.mp_page;
        let creating = m.starting.as_deref() == Some("create");
        let shell = cx.entity().downgrade();
        let players = Button::new("mp-max")
            .outline()
            .tap_sm()
            .w(px(160.))
            .label(trn(usize::from(m.max), "{count} player", "{count} players"))
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                for n in MIN_PLAYERS..=MAX_PLAYERS {
                    let shell = shell.clone();
                    menu = menu.item(
                        PopupMenuItem::new(trn(usize::from(n), "{count} player", "{count} players"))
                            .on_click(move |_, _, cx| {
                                shell
                                    .update(cx, |s, cx| {
                                        s.mp_page.max = n;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            });
        let public = h_flex()
            .gap(px(10.))
            .child(
                Switch::new("mp-public")
                    .checked(m.public)
                    .tap_sm()
                    .on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.mp_page.public = *on;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(if m.public {
                        tr("Listed: anyone can see it and join")
                    } else {
                        tr("Private: only people with the code can join")
                    }),
            );
        let row = |title: &'static str, control: AnyElement, first: bool| {
            h_flex()
                .px(px(16.))
                .py(px(10.))
                .gap(px(12.))
                .when(!first, |d| d.border_t_1().border_color(p.border_variant))
                .child(
                    div()
                        .w(px(120.))
                        .flex_shrink_0()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(title),
                )
                .child(h_flex().flex_1().min_w_0().child(control))
        };
        v_flex()
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
            .child(
                v_flex()
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(12.))
                    .gap(px(3.))
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("New room")),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(tr(
                        "F1 25 opens its LAN lobby, where you pick the tracks and settings.",
                    ))),
            )
            .child(row(
                "Name",
                div()
                    .w(px(320.))
                    .child(Input::new(&m.name).tap_sm())
                    .into_any_element(),
                true,
            ))
            .child(row("Visibility", public.into_any_element(), false))
            .child(row("Players", players.into_any_element(), false))
            .child(
                h_flex()
                    .px(px(16.))
                    .py(px(12.))
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(p.border_variant)
                    .bg(p.surface)
                    .child(div().flex_1())
                    .child(
                        Button::new("mp-create-cancel")
                            .ghost()
                            .tap_sm()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mp_page.form = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mp-create")
                            .primary()
                            .tap_sm()
                            .label(tr("Create room"))
                            .loading(creating)
                            .disabled(m.busy())
                            .on_click(cx.listener(|this, _, window, cx| this.mp_create(window, cx))),
                    ),
            )
    }

    fn mp_card(&self, p: Palette) -> Div {
        v_flex()
            .p(px(16.))
            .gap(px(12.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
    }

    fn mp_enabled_addons(&self) -> HashSet<String> {
        self.library
            .profile
            .addons
            .iter()
            .filter(|a| a.enabled)
            .map(|a| a.id.clone())
            .collect()
    }

    fn mp_room_view(&mut self, room: &RoomSummary, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let on = self.mp_enabled_addons();
        let listed = room.addons.as_ref().filter(|a| !a.list.is_empty());
        let checks = listed.map(|a| addon_checks(&a.list, &on)).unwrap_or_default();
        let lacking = missing(&checks);
        let subtitle = room_meta(room);
        let actions = h_flex().gap(px(8.)).items_center().child(
            Button::new("mp-view-back")
                .outline()
                .icon(IconName::ArrowLeft)
                .label(tr("Back to rooms"))
                .on_click(cx.listener(|this, _, _, cx| this.mp_close_view(cx))),
        );
        let summary = if checks.is_empty() {
            tr("The host's launcher hasn't listed its addons.").to_owned()
        } else if lacking == 0 {
            tr("You have every addon the host has on.").to_owned()
        } else {
            fill(
                &trn(
                    checks.len(),
                    "You are missing or have off {lacking} of {count} addon.",
                    "You are missing or have off {lacking} of {count} addons.",
                ),
                &[("lacking", lacking.to_string())],
            )
        };
        let rows: Vec<AnyElement> = checks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                h_flex()
                    .px(px(16.))
                    .py(px(10.))
                    .gap(px(12.))
                    .items_center()
                    .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
                    .child(
                        Icon::new(if c.have { IconName::Check } else { IconName::X })
                            .size(px(16.))
                            .text_color(if c.have { p.success } else { p.error }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(14.))
                            .child(c.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .child(if c.have {
                                tr("You have it")
                            } else {
                                tr("You don't have it on")
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex()
            .size_full()
            .child(self.header(room.name.clone(), subtitle, actions, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("mp-room-view")
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(24.))
                    .gap(px(16.))
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(summary))
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(trf!(
                        "Press Play {host}'s Lobby in the bar below to start F1 25 in this room.",
                        host = room.host.name
                    )))
                    .when(!rows.is_empty(), |d| {
                        d.child(
                            v_flex()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(p.border)
                                .overflow_hidden()
                                .children(rows),
                        )
                    }),
            )
            .into_any_element()
    }

    fn status_card(&self, st: &Status, state: &SessionState, p: Palette, cx: &mut Context<Self>) -> Div {
        let m = &self.mp_page;
        let (icon, color) = match st.tone {
            Tone::Working => (IconName::LoaderCircle, p.accent),
            Tone::Blocked => (IconName::CircleAlert, p.warning),
            Tone::Good => (IconName::CircleCheck, p.success),
            Tone::Over => (IconName::Info, p.text_placeholder),
        };
        let over = state.phase.is_over();
        let current = step_number(st.progress.step);
        let steps = h_flex()
            .gap(px(14.))
            .flex_wrap()
            .children(STEPS.iter().enumerate().map(|(i, (_, name))| {
                div()
                    .text_size(px(12.))
                    .text_color(if i == current && !over {
                        p.text
                    } else if i < current && !over {
                        p.text_muted
                    } else {
                        p.text_placeholder
                    })
                    .when(i == current && !over, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(*name)
            }));
        let fraction = f32::from(st.progress.percent) / 100.0;
        let bar = div()
            .h(px(6.))
            .w_full()
            .rounded_full()
            .bg(p.border_variant)
            .child(
                div()
                    .h_full()
                    .w(relative(fraction))
                    .rounded_full()
                    .bg(if st.tone == Tone::Blocked {
                        p.warning
                    } else {
                        p.accent
                    }),
            );
        let headline = match &st.problem {
            Some(problem) => problem.text.clone(),
            None => st.progress.label.clone(),
        };
        let action = st
            .problem
            .as_ref()
            .filter(|_| !m.leaving && !over)
            .map(|problem| {
                if problem.retry {
                    Button::new("mp-retry")
                        .primary()
                        .tap_sm()
                        .icon(IconName::RefreshCw)
                        .label("Retry")
                        .on_click(cx.listener(|this, _, _, cx| this.mp_retry(cx)))
                } else {
                    Button::new("mp-problem-leave")
                        .primary()
                        .tap_sm()
                        .icon(IconName::LogOut)
                        .label("Leave")
                        .on_click(cx.listener(|this, _, _, cx| this.mp_leave(cx)))
                }
            });
        let running = !self.processes.is_empty();
        let reopen = (st.problem.is_some()
            && state.launch.is_some()
            && running
            && m.closing.is_none()
            && !over
            && !m.leaving)
            .then(|| {
                Button::new("mp-restart-game")
                    .primary()
                    .tap_sm()
                    .label(tr("Restart F1 25"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.mp_close_game(window, cx);
                        cx.notify();
                    }))
            });
        let race =
            (state.host && !over && !m.leaving && matches!(state.phase, Phase::InLobby | Phase::Racing))
                .then(|| {
                    let started = state.room.as_ref().is_some_and(|r| r.started);
                    Button::new("mp-race")
                        .outline()
                        .tap_sm()
                        .icon(IconName::Flag)
                        .label(if started {
                            tr("Race over")
                        } else {
                            tr("Race started")
                        })
                        .tooltip(if started {
                            tr("Show the room in the list again")
                        } else {
                            tr("Hide the room from the list while you race")
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(s) = &this.mp_page.session {
                                s.mark_race(!started);
                            }
                            cx.notify();
                        }))
                });
        self.mp_card(p)
            .gap(px(14.))
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_center()
                    .map(|d| match icon {
                        IconName::LoaderCircle => {
                            d.child(Spinner::new().icon(icon).with_size(px(22.)).color(color))
                        }
                        _ => d.child(Icon::new(icon).size(px(22.)).text_color(color)),
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(
                                if m.leaving && !over {
                                    "Leaving the room\u{2026}".to_owned()
                                } else {
                                    headline
                                },
                            ))
                            .children(
                                st.hint
                                    .filter(|_| !m.leaving)
                                    .map(|h| div().text_size(px(13.)).text_color(p.text_muted).child(h)),
                            )
                            .children(
                                st.problem
                                    .as_ref()
                                    .and_then(|problem| problem.detail.clone())
                                    .filter(|_| !m.leaving)
                                    .map(|d| div().text_size(px(13.)).text_color(p.text_muted).child(d)),
                            )
                            .children(reopen.is_some().then(|| {
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child(tr("F1 25 is open. It restarts for this session."))
                            })),
                    )
                    .children(race)
                    .children(reopen)
                    .children(action),
            )
            .when(!over, |d| d.child(bar).child(steps))
    }

    fn mp_room(&mut self, state: &SessionState, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.mp_page;
        let over = state.phase.is_over();
        let room = state.room.clone();
        let title = room
            .as_ref()
            .map_or_else(|| "Multiplayer".to_owned(), |r| r.name.clone());
        let subtitle = room.as_ref().map_or_else(
            || tr("Setting up the room\u{2026}").to_owned(),
            |r| {
                let mut parts = vec![
                    trf!("{count} in the lobby", count = joined(state)),
                    trf!(
                        "{joined}/{max} players",
                        joined = state.members.len(),
                        max = r.max_members
                    ),
                    if r.public { tr("Listed") } else { tr("Private") }.to_owned(),
                ];
                if state.host {
                    parts.push(tr("You're the host").into());
                }
                parts.join(" \u{00b7} ")
            },
        );
        let code = room.as_ref().map(|r| r.code.clone());
        let actions = h_flex()
            .gap(px(8.))
            .items_center()
            .when_some(code, |d, code| {
                let copy = code.clone();
                d.child(
                    Button::new("mp-copy-code")
                        .outline()
                        .icon(if m.copied.is_some() {
                            IconName::Check
                        } else {
                            IconName::Copy
                        })
                        .label(if m.copied.is_some() {
                            "Copied".to_owned()
                        } else {
                            code
                        })
                        .tooltip(tr("Copy the invite code"))
                        .on_click(cx.listener(move |this, _, _, cx| this.mp_copy_code(copy.clone(), cx))),
                )
            })
            .child(
                Button::new("mp-leave")
                    .outline()
                    .icon(IconName::LogOut)
                    .label(if m.leaving {
                        "Leaving\u{2026}"
                    } else if over {
                        "Back to rooms"
                    } else {
                        "Leave"
                    })
                    .disabled(m.leaving)
                    .on_click(cx.listener(|this, _, _, cx| this.mp_leave(cx))),
            );
        let st = room_status(state);
        let status = self.status_card(&st, state, p, cx);
        let players: Vec<AnyElement> = state
            .members
            .iter()
            .enumerate()
            .map(|(i, member)| self.member_row(member, &state.content, i == 0, p))
            .collect();
        let cars = content_check(state).map(|(owns, text)| {
            v_flex()
                .gap(px(8.))
                .child(self.section_head("CARS".to_owned(), p))
                .child(
                    h_flex()
                        .px(px(16.))
                        .py(px(10.))
                        .gap(px(12.))
                        .items_center()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(p.border)
                        .child(
                            Icon::new(match owns {
                                Some(true) => IconName::Check,
                                Some(false) => IconName::X,
                                None => IconName::Info,
                            })
                            .size(px(16.))
                            .text_color(match owns {
                                Some(true) => p.success,
                                Some(false) => p.error,
                                None => p.text_muted,
                            }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(14.))
                                .child(tr("F1 2026 cars")),
                        )
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(text)),
                )
        });
        v_flex()
            .size_full()
            .child(self.header(title, subtitle, actions, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("mp-room")
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(24.))
                    .gap(px(24.))
                    .child(status)
                    .children(cars)
                    .child(
                        v_flex()
                            .gap(px(8.))
                            .child(self.section_head(
                                trf!("PLAYERS \u{00b7} {count}", count = state.members.len()),
                                p,
                            ))
                            .child(if players.is_empty() {
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child(tr("Nobody is here yet."))
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(p.border)
                                    .overflow_hidden()
                                    .children(players)
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }

    fn member_row(&self, m: &MemberState, content: &RoomContent, first: bool, p: Palette) -> AnyElement {
        let (word, tone) = content_word(content, m).unwrap_or_else(|| member_word(m));
        h_flex()
            .px(px(16.))
            .py(px(12.))
            .gap(px(12.))
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(self.face(&m.name, m.avatar.as_deref(), 32., p))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(m.name.clone()),
                    )
                    .when(m.host, |d| d.child(tag("Host", p)))
                    .when(m.me, |d| d.child(tag("You", p))),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(match tone {
                        Tone::Good => p.success,
                        Tone::Blocked => p.warning,
                        _ => p.text_muted,
                    })
                    .child(word),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use swofty_core::multiplayer::types::{
        GameStatus, MemberStage, MemberState, Phase, PrintMatch, Problem, RelayStatus, Role, RoomAddon,
        RoomAddons, RoomSummary, Screen, SessionState, User,
    };

    use super::{
        GameState, RoomContent, RoomStatus, Start, Tone, addon_checks, card_addons, content_check,
        content_word, exit_leaves, exit_notice, joined, member_word, missing, refusal, refusal_text,
        restart_ok_text, room_meta,
    };

    #[test]
    fn a_problem_reads_with_or_without_its_detail() {
        let read = |json: &str| serde_json::from_str::<Problem>(json).unwrap();
        assert_eq!(
            read(r#"{"text":"Simplified doesn't match the host's.","retry":false}"#).detail,
            None
        );
        let said = read(
            r#"{"text":"Simplified doesn't match the host's.","retry":false,"detail":"Its build step made more."}"#,
        );
        assert_eq!(said.detail.as_deref(), Some("Its build step made more."));
        assert!(
            !serde_json::to_string(&read(r#"{"text":"x","retry":true}"#))
                .unwrap()
                .contains("detail")
        );
    }

    fn status(screen: Screen) -> GameStatus {
        GameStatus {
            screen,
            in_session: false,
            role: Role::None,
            configured: false,
            self_member: 0,
            base: 0,
            cover: None,
            room_failed: None,
            content: None,
        }
    }

    fn addon(id: &str, name: &str) -> RoomAddon {
        RoomAddon {
            id: id.into(),
            name: name.into(),
        }
    }

    fn room(members: u32, max: u32, digest: Option<&str>) -> RoomSummary {
        RoomSummary {
            code: "F1-7KQ2MX".into(),
            name: "Sunday league".into(),
            host: User {
                id: "u1".into(),
                name: "swofty".into(),
                avatar: None,
            },
            members,
            max_members: max,
            category: "f1-2026".into(),
            digest: digest.map(str::to_owned),
            addons: Some(RoomAddons {
                count: 4,
                tracks: vec!["Okayama".into()],
                list: vec![
                    addon("a.tracks", "Okayama"),
                    addon("b.cars", "2026 Cars"),
                    addon("c.hud", "Timing HUD"),
                ],
            }),
            created_at: 0,
        }
    }

    fn session(phase: Phase, host: bool) -> SessionState {
        SessionState {
            phase,
            host,
            room: None,
            self_member: None,
            members: Vec::new(),
            mismatches: Vec::new(),
            lobby_connected: true,
            relay: None,
            base: None,
            bridge: None,
            game: None,
            last_error: None,
            launch: None,
            content: RoomContent::default(),
            progress: None,
            problem: None,
        }
    }

    fn member(connected: bool, stage: Option<MemberStage>) -> MemberState {
        MemberState {
            id: "u2".into(),
            name: "Max".into(),
            avatar: None,
            number: 2,
            ip: "10.77.0.2".into(),
            host: false,
            me: false,
            connected,
            fingerprint: PrintMatch::Matches,
            relay: RelayStatus::Ready,
            stage,
            traffic: None,
        }
    }

    #[test]
    fn names_where_the_game_is() {
        assert_eq!(
            GameState::of(false, Some(&status(Screen::MainMenu))),
            GameState::NotRunning
        );
        assert_eq!(
            GameState::of(true, Some(&status(Screen::MainMenu))),
            GameState::MainMenu
        );
        assert_eq!(
            GameState::of(true, Some(&status(Screen::LanLobby))),
            GameState::LanLobby
        );
        assert_eq!(GameState::of(true, Some(&status(Screen::Other))), GameState::Busy);
        assert_eq!(GameState::of(true, None), GameState::Busy);
    }

    #[test]
    fn gives_each_room_one_status() {
        assert_eq!(
            RoomStatus::of(&room(8, 8, Some("sha256:a")), Some("sha256:a")),
            RoomStatus::Full
        );
        assert_eq!(
            RoomStatus::of(&room(3, 8, Some("sha256:ABC")), Some("sha256:abc")),
            RoomStatus::Match
        );
        assert_eq!(
            RoomStatus::of(&room(3, 8, Some("sha256:a")), Some("sha256:b")),
            RoomStatus::Differ
        );
        assert_eq!(
            RoomStatus::of(&room(3, 8, None), Some("sha256:b")),
            RoomStatus::Unknown
        );
    }

    #[test]
    fn describes_a_room_with_every_addon() {
        assert_eq!(room_meta(&room(3, 8, None)), "swofty \u{00b7} 3/8 players");
        assert_eq!(
            card_addons(&room(3, 8, None)),
            ["Okayama", "2026 Cars", "Timing HUD"]
        );
        let mut old = room(3, 8, None);
        old.addons.as_mut().unwrap().list.clear();
        assert_eq!(
            card_addons(&old),
            ["Okayama"],
            "a server that lists no addons shows the tracks"
        );
        old.addons = None;
        assert!(card_addons(&old).is_empty());
    }

    #[test]
    fn marks_each_room_addon_with_whether_the_player_has_it_on() {
        let listed = room(3, 8, None).addons.unwrap().list;
        let on: HashSet<String> = ["a.tracks".to_owned(), "c.hud".to_owned()].into();
        let checks = addon_checks(&listed, &on);
        assert_eq!(
            checks
                .iter()
                .map(|c| (c.name.as_str(), c.have))
                .collect::<Vec<_>>(),
            [("Okayama", true), ("2026 Cars", false), ("Timing HUD", true)]
        );
        assert_eq!(missing(&checks), 1);
    }

    #[test]
    fn lists_each_player_by_what_their_game_is_doing() {
        assert_eq!(
            member_word(&member(true, Some(MemberStage::Starting))).0,
            "Starting the game"
        );
        assert_eq!(
            member_word(&member(true, Some(MemberStage::Loading))).0,
            "Loading"
        );
        assert_eq!(
            member_word(&member(true, Some(MemberStage::InLobby))).0,
            "In the lobby"
        );
        assert_eq!(
            member_word(&member(false, Some(MemberStage::InLobby))).0,
            "Disconnected"
        );
        assert_eq!(member_word(&member(true, None)).0, "Joining\u{2026}");
    }

    #[test]
    fn only_a_game_in_the_lobby_counts_as_joined() {
        let mut s = session(Phase::InLobby, false);
        s.members = vec![
            member(true, Some(MemberStage::InLobby)),
            member(true, Some(MemberStage::Loading)),
            member(true, Some(MemberStage::Starting)),
            member(false, Some(MemberStage::InLobby)),
            member(true, None),
        ];
        assert_eq!(joined(&s), 1);
    }

    #[test]
    fn leaves_the_room_when_the_rooms_game_exits() {
        assert!(exit_leaves(true, false, false));
        assert!(
            !exit_leaves(false, false, false),
            "a game that never ran is not an exit"
        );
        assert!(!exit_leaves(true, true, false), "a running game stays");
        assert!(
            !exit_leaves(true, false, true),
            "a launch or restore in flight stays"
        );
        assert_eq!(exit_notice(false), "F1 25 closed, so you left the room.");
        assert_eq!(exit_notice(true), "F1 25 closed, so your room closed.");
    }

    #[test]
    fn the_servers_refusal_shows_until_a_load_succeeds() {
        let beta = || Some("needs the beta role".to_owned());
        assert_eq!(refusal(None, false, None), None);
        assert_eq!(refusal(None, false, beta()), beta());
        assert_eq!(
            refusal(beta(), false, None),
            beta(),
            "kept while another load runs"
        );
        assert_eq!(
            refusal(beta(), false, Some("not signed in".into())),
            Some("not signed in".into())
        );
        assert_eq!(refusal(beta(), true, None), None);
        assert_eq!(
            refusal_text("needs the beta role"),
            "Online sessions aren't available: needs the beta role"
        );
    }

    #[test]
    fn the_restart_button_says_what_it_does() {
        use swofty_core::multiplayer::types::RoomSettings;
        assert_eq!(
            restart_ok_text(&Start::Create(RoomSettings::default())),
            "Create room and join"
        );
        assert_eq!(restart_ok_text(&Start::Join("F1-ABC123".into())), "Join room");
    }

    #[test]
    fn a_member_without_the_f1_2026_content_is_marked_once_the_host_picked_it() {
        let max = member(true, Some(MemberStage::InLobby));
        let mut content = RoomContent::default();
        content.owns_f1_2026.insert("u2".into(), false);
        assert_eq!(
            content_word(&content, &max),
            None,
            "nothing before the host picked the 2026 cars"
        );
        content.host = Some("f1_2026".into());
        assert_eq!(
            content_word(&content, &max),
            Some(("No F1 2026 content", Tone::Blocked))
        );
        content.owns_f1_2026.insert("u2".into(), true);
        assert_eq!(content_word(&content, &max), None);
        content.owns_f1_2026.insert("u2".into(), false);
        assert_eq!(
            content_word(&content, &member(false, None)),
            None,
            "a disconnected member stays Disconnected"
        );
    }

    #[test]
    fn the_cars_row_shows_this_players_f1_2026_answer() {
        let mut s = session(Phase::InLobby, false);
        let mut me = member(true, Some(MemberStage::InLobby));
        me.me = true;
        s.members = vec![me];
        assert_eq!(content_check(&s), None);
        s.content.host = Some("f1_2025".into());
        assert_eq!(content_check(&s), None, "the 2025 cars need no content");
        s.content.host = Some("f1_2026".into());
        assert_eq!(content_check(&s).map(|(owns, _)| owns), Some(None));
        s.content.owns_f1_2026.insert("u2".into(), false);
        assert_eq!(
            content_check(&s),
            Some((Some(false), "Your copy of F1 25 doesn't have the F1 2026 content"))
        );
    }
}
