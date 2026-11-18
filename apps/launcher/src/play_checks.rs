use std::fmt::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use swofty_core::data_usage::Room;
use swofty_core::store::Store;
use swofty_i18n::{fill, tr, trf, trn};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    pub level: NoteLevel,
    pub title: String,
    pub body: String,
    pub sticky: bool,
}

impl Note {
    fn warning(title: &str, body: String) -> Self {
        Self {
            level: NoteLevel::Warning,
            title: title.to_owned(),
            body,
            sticky: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    Go,
    Stop(String),
    Confirm(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Gate {
    pub write_blocked: bool,
    pub setup_done: bool,
    pub offline_accepted: bool,
    pub first_launch: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Timings(Vec<(&'static str, Duration)>);

impl Timings {
    pub(crate) fn time<T>(&mut self, step: &'static str, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let out = f();
        self.0.push((step, started.elapsed()));
        out
    }

    pub(crate) fn line(&self) -> String {
        let total: Duration = self.0.iter().map(|(_, d)| *d).sum();
        Self::format(total, &self.0)
    }

    fn format(total: Duration, steps: &[(&'static str, Duration)]) -> String {
        let parts: Vec<String> = steps
            .iter()
            .map(|(name, d)| format!("{name} {}", d.as_millis()))
            .collect();
        format!("[play] checks {} ms ({})", total.as_millis(), parts.join(", "))
    }
}

pub(crate) struct Inputs {
    pub store: Store,
    pub profile: String,
    pub game: PathBuf,
    pub build_id: Option<u64>,
    pub anyway: bool,
}

pub(crate) struct Checked {
    pub profile: String,
    pub game: PathBuf,
    pub gate: Gate,
    pub notes: Vec<Note>,
    pub verdict: Verdict,
    pub room: Room,
    pub timings: Timings,
    /// Lines for the app log after the timings line.
    pub lines: Vec<String>,
}

pub(crate) fn run(inputs: Inputs) -> Checked {
    let Inputs {
        store,
        profile,
        game,
        build_id,
        anyway,
    } = inputs;
    let mut timings = Timings::default();
    let gate = timings.time("gate", || Gate {
        write_blocked: swofty_core::compat::write_blocker(&game).is_some(),
        setup_done: swofty_core::setup::done(&store.root),
        offline_accepted: swofty_core::offline::is_accepted(&store.root),
        first_launch: swofty_core::setup::first_launch(&store.root),
    });
    let mut notes = Vec::new();
    let mut lines = Vec::new();
    let verdict = match (
        timings.time("profile", || store.profile(&profile)),
        timings.time("addons", || store.addons()),
    ) {
        (Ok(p), Ok(installed)) => {
            let catalog = timings.time("catalog", || {
                swofty_core::catalog::cached(&store.root).unwrap_or_default()
            });
            timings.time("lonely", || {
                notes.extend(lonely(&store, &p, &installed, &catalog))
            });
            timings.time("track limit", || {
                let (note, line) = track_limit_note(&store, &game, build_id, &p);
                notes.extend(note);
                lines.extend(line);
            });
            registry(&store, &game, &p, &installed, anyway, &mut notes, &mut timings)
        }
        _ => Verdict::Go,
    };
    let room = timings.time("room", || Room::now(&store.root));
    Checked {
        profile,
        game,
        gate,
        notes,
        verdict,
        room,
        timings,
        lines,
    }
}

fn track_limit_note(
    store: &Store,
    game: &std::path::Path,
    build_id: Option<u64>,
    profile: &swofty_core::profile::Profile,
) -> (Option<Note>, Option<String>) {
    if profile.is_vanilla() {
        return (None, None);
    }
    let started = Instant::now();
    let identity = match swofty_core::baseline::Identity::detect_cached(&store.root, game, build_id) {
        Ok(identity) => identity,
        Err(e) => {
            let mut line = String::new();
            let _ = write!(
                line,
                "[play] track limit: the game's version could not be read ({e}), so no check ran ({} ms)",
                started.elapsed().as_millis()
            );
            return (None, Some(line));
        }
    };
    let identified = started.elapsed();
    let game = swofty_core::install::Game {
        dir: game.to_path_buf(),
        user_reg: None,
        identity,
    };
    let checked = Instant::now();
    let check = swofty_core::build::track_limit(store, &profile.name, &game);
    let checking = checked.elapsed();
    let check = match check {
        Ok(check) => check,
        Err(e) => {
            let mut line = String::new();
            let _ = write!(
                line,
                "[play] track limit: the check failed after {} ms (identity {} ms, check {} ms): {e}",
                started.elapsed().as_millis(),
                identified.as_millis(),
                checking.as_millis()
            );
            return (None, Some(line));
        }
    };
    let recorded = Instant::now();
    swofty_core::tracks::record_track_limit(&store.root, &profile.name, check.limit.as_ref()).ok();
    let parts = LimitTimes {
        total: started.elapsed(),
        identity: identified,
        record: recorded.elapsed(),
    };
    let line = limit_line(&check, &parts);
    (check.limit.as_ref().map(limit_note), Some(line))
}

pub(crate) struct LimitTimes {
    pub total: Duration,
    pub identity: Duration,
    pub record: Duration,
}

struct Tracks(usize);

impl std::fmt::Display for Tracks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            1 => write!(f, "1 track"),
            n => write!(f, "{n} tracks"),
        }
    }
}

struct LeftOut(usize);

impl std::fmt::Display for LeftOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => write!(f, "none left out"),
            n => write!(f, "{} left out", Tracks(n)),
        }
    }
}

pub(crate) fn limit_line(check: &swofty_core::build::TrackLimitCheck, t: &LimitTimes) -> String {
    use swofty_core::tracks::{FILL_FROM, TRACK_LIST_ROOM};
    let added = Tracks(check.added);
    let identity = t.identity.as_millis();
    let counting = check.counting.as_millis();
    let mut line = String::new();
    if !check.built() {
        let _ = write!(
            line,
            "[play] track limit: the on addons add {added}, under the {FILL_FROM} that can fill the game's track lists \
             ({TRACK_LIST_ROOM} tracks), so no check build ran ({} ms: identity {identity} ms, counting {counting} ms, \
             record {} ms)",
            t.total.as_millis(),
            t.record.as_millis()
        );
        return line;
    }
    let steps: Vec<String> = check
        .steps
        .iter()
        .map(|(name, d)| format!("{name} {} ms", d.as_millis()))
        .collect();
    let left_out = LeftOut(check.limit.as_ref().map_or(0, |l| l.names.len()));
    let reuse = match (check.reused, check.saved) {
        (true, _) => "the check restored the saved build of these addons, so nothing was built again",
        (false, true) => {
            "the check build is saved, so Play restores it unless the addons or game files change first"
        }
        (false, false) => {
            "the check build is not saved (it gave added rows ids a check does not keep), so Play builds again"
        }
    };
    let _ = write!(
        line,
        "[play] track limit: the on addons add {added}, at least the {FILL_FROM} that can fill the game's track lists \
         ({TRACK_LIST_ROOM} tracks), so the check planned the whole build to find the tracks left out: {} ms (identity \
         {identity} ms, counting {counting} ms; check build: {}; record {} ms); {left_out}; {reuse}",
        t.total.as_millis(),
        steps.join(", "),
        t.record.as_millis()
    );
    line
}

pub(crate) fn limit_text(room: usize, names: &[String]) -> String {
    const SHOWN: usize = 4;
    let list = match names.len().saturating_sub(SHOWN) {
        0 => names.join(", "),
        more => trf!(
            "{shown} and {more} more",
            shown = names[..SHOWN].join(", "),
            more = more
        ),
    };
    fill(
        &trn(
            names.len(),
            "The game holds {room} tracks, so {count} track is left out of this build: {list}. Turn some track addons off to play it.",
            "The game holds {room} tracks, so {count} tracks are left out of this build: {list}. Turn some track addons off to play them.",
        ),
        &[("room", room.to_string()), ("list", list)],
    )
}

pub(crate) fn limit_note(limit: &swofty_core::tracks::TrackLimit) -> Note {
    Note::warning(tr("Too many tracks"), limit_text(limit.room, &limit.names))
}

fn lonely(
    store: &Store,
    profile: &swofty_core::profile::Profile,
    installed: &std::collections::BTreeMap<String, swofty_core::manifest::AddonManifest>,
    catalog: &[swofty_core::catalog::Addon],
) -> Vec<Note> {
    let mut notes = Vec::new();
    for (base, family) in swofty_core::resolver::lonely_bases(profile, installed, catalog) {
        if installed
            .values()
            .any(|m| m.name == base && store.is_builtin(&m.id))
        {
            continue;
        }
        notes.push(lonely_note(&base, &family));
    }
    notes
}

fn lonely_note(base: &str, family: &[String]) -> Note {
    let shown = family.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
    let more = family.len().saturating_sub(4);
    let body = if more > 0 {
        trf!(
            "{base} is on, but none of its addons are, so it adds nothing to pick on its own. Install at least one in the Addon Browser, such as {shown} and {more} more, and turn it on.",
            base = base,
            shown = shown,
            more = more
        )
    } else {
        trf!(
            "{base} is on, but none of its addons are, so it adds nothing to pick on its own. Install at least one in the Addon Browser, such as {shown}, and turn it on.",
            base = base,
            shown = shown
        )
    };
    Note::warning(&trf!("{base} needs one of its addons", base = base), body)
}

fn registry(
    store: &Store,
    game: &std::path::Path,
    profile: &swofty_core::profile::Profile,
    installed: &std::collections::BTreeMap<String, swofty_core::manifest::AddonManifest>,
    anyway: bool,
    notes: &mut Vec<Note>,
    timings: &mut Timings,
) -> Verdict {
    use swofty_core::stdlib::{careers, deps};
    if profile.is_vanilla() {
        let found = timings.time("careers", || {
            careers::sync(&store.root, Some(game), None).unwrap_or_default()
        });
        let using = found.iter().filter(|c| c.uses_addons()).count();
        notes.extend(vanilla_note(using));
        return Verdict::Go;
    }
    let Ok(plan) = timings.time("resolve", || swofty_core::resolver::resolve(profile, installed)) else {
        return Verdict::Go;
    };
    let all = timings.time("deps", || deps::read_all(store, installed));
    let report = timings.time("deps check", || {
        deps::for_order(&profile.name, &plan.order, installed, &all)
    });
    let (saved, scoped) = timings.time("careers", || {
        let saved = careers::read(&careers::data_path(&store.root));
        let mut scoped: Vec<_> = careers::in_save_of(&saved, &store.root, &profile.name)
            .into_iter()
            .cloned()
            .collect();
        careers::drop_former_owners(
            &mut scoped,
            &swofty_core::row_ids::id_owners(&store.root),
            installed,
            &plan.order,
        );
        (saved, scoped)
    });
    let stop = careers::stop_line(&scoped, &plan.order, installed);
    let play = careers::play_line(&saved, &plan.order, installed);
    let findings = report.findings.iter().map(finding_note).collect();
    assemble(stop, play, findings, report.stop_text(), anyway, notes)
}

fn vanilla_note(using: usize) -> Option<Note> {
    if using == 0 {
        return None;
    }
    let text = fill(
        &trn(
            using,
            "{count} career uses addons; loading it in {profile} can lose its progress.",
            "{count} careers use addons; loading them in {profile} can lose their progress.",
        ),
        &[("profile", swofty_core::profile::VANILLA.to_string())],
    );
    Some(Note::warning(tr("Careers that need addons"), text))
}

fn finding_note(f: &swofty_core::stdlib::deps::Finding) -> Note {
    use swofty_core::stdlib::deps::Level;
    let (level, sticky) = match f.level {
        Level::Stop => (NoteLevel::Error, true),
        Level::Warn => (NoteLevel::Warning, true),
        Level::Hint => (NoteLevel::Info, false),
    };
    Note {
        level,
        title: f.headline.clone(),
        body: f.detail.join(" "),
        sticky,
    }
}

pub(crate) fn assemble(
    career_stop: Option<String>,
    career_play: Option<String>,
    findings: Vec<Note>,
    dependency_stop: Option<String>,
    anyway: bool,
    notes: &mut Vec<Note>,
) -> Verdict {
    if let Some(stop) = career_stop {
        if anyway {
            notes.push(Note::warning("Careers that need addons", stop));
        } else {
            return Verdict::Confirm(stop);
        }
    }
    notes.extend(career_play.map(|line| Note::warning("Careers that need addons", line)));
    notes.extend(findings);
    match dependency_stop {
        Some(stop) => Verdict::Stop(stop),
        None => Verdict::Go,
    }
}

#[derive(Debug, Default)]
pub(crate) struct Pending {
    generation: u64,
    active: Option<u64>,
}

impl Pending {
    pub(crate) fn begin(&mut self) -> Option<u64> {
        if self.active.is_some() {
            return None;
        }
        self.generation += 1;
        self.active = Some(self.generation);
        Some(self.generation)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn finish(&mut self, generation: u64) -> bool {
        if self.active == Some(generation) {
            self.active = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.generation += 1;
        self.active = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(body: &str) -> Note {
        Note {
            level: NoteLevel::Info,
            title: "t".into(),
            body: body.into(),
            sticky: false,
        }
    }

    #[test]
    fn career_stop_asks_before_anything_else() {
        let mut notes = Vec::new();
        let v = assemble(
            Some("stop".into()),
            Some("play".into()),
            vec![note("f")],
            None,
            false,
            &mut notes,
        );
        assert_eq!(v, Verdict::Confirm("stop".into()));
        assert!(notes.is_empty());
    }

    #[test]
    fn play_anyway_turns_the_stop_into_a_warning_first() {
        let mut notes = Vec::new();
        let v = assemble(
            Some("stop".into()),
            Some("play".into()),
            vec![note("f")],
            None,
            true,
            &mut notes,
        );
        assert_eq!(v, Verdict::Go);
        let bodies: Vec<_> = notes.iter().map(|n| n.body.as_str()).collect();
        assert_eq!(bodies, ["stop", "play", "f"]);
        assert!(notes[0].sticky);
    }

    #[test]
    fn dependency_stop_keeps_its_findings_and_blocks() {
        let mut notes = Vec::new();
        let v = assemble(None, None, vec![note("f")], Some("dep".into()), false, &mut notes);
        assert_eq!(v, Verdict::Stop("dep".into()));
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn nothing_found_goes_on() {
        let mut notes = Vec::new();
        assert_eq!(assemble(None, None, vec![], None, false, &mut notes), Verdict::Go);
        assert!(notes.is_empty());
    }

    #[test]
    fn vanilla_note_counts_careers() {
        assert!(vanilla_note(0).is_none());
        assert!(vanilla_note(1).unwrap().body.starts_with("1 career uses addons"));
        assert!(vanilla_note(3).unwrap().body.starts_with("3 careers use addons"));
    }

    #[test]
    fn lonely_note_names_four_and_counts_the_rest() {
        let family: Vec<String> = (1..=6).map(|i| format!("S{i}")).collect();
        let n = lonely_note("Base", &family);
        assert!(n.body.contains("S1, S2, S3, S4 and 2 more"));
        assert_eq!(n.title, "Base needs one of its addons");
    }

    #[test]
    fn limit_text_names_the_tracks_left_out() {
        let names: Vec<String> = ["Mugello", "Okayama"].map(String::from).into();
        assert_eq!(
            limit_text(64, &names),
            "The game holds 64 tracks, so 2 tracks are left out of this build: Mugello, Okayama. Turn some track addons off to play them."
        );
        assert_eq!(
            limit_text(64, &names[..1]),
            "The game holds 64 tracks, so 1 track is left out of this build: Mugello. Turn some track addons off to play it."
        );
    }

    #[test]
    fn limit_text_names_four_and_counts_the_rest() {
        let names: Vec<String> = (1..=13).map(|i| format!("T{i}")).collect();
        let text = limit_text(64, &names);
        assert!(text.contains("13 tracks are left out"));
        assert!(text.contains("T1, T2, T3, T4 and 9 more."));
        assert!(!text.contains("T5"));
    }

    #[test]
    fn limit_note_is_a_sticky_warning() {
        let limit = swofty_core::tracks::TrackLimit {
            room: 64,
            stock: 35,
            names: vec!["Mugello".into()],
            addons: vec!["mod.mugello".into()],
        };
        let n = limit_note(&limit);
        assert_eq!(n.title, "Too many tracks");
        assert_eq!(n.level, NoteLevel::Warning);
        assert!(n.sticky);
    }

    #[test]
    fn timing_line_lists_every_step() {
        let steps = [
            ("deps", Duration::from_millis(300)),
            ("resolve", Duration::from_millis(40)),
        ];
        assert_eq!(
            Timings::format(Duration::from_millis(412), &steps),
            "[play] checks 412 ms (deps 300, resolve 40)"
        );
    }

    fn limit_times() -> LimitTimes {
        LimitTimes {
            total: Duration::from_millis(155_812),
            identity: Duration::from_millis(3),
            record: Duration::from_millis(1),
        }
    }

    #[test]
    fn limit_line_without_a_check_build_says_why() {
        let check = swofty_core::build::TrackLimitCheck {
            added: 5,
            counting: Duration::from_millis(28),
            ..Default::default()
        };
        let t = LimitTimes {
            total: Duration::from_millis(32),
            ..limit_times()
        };
        assert_eq!(
            limit_line(&check, &t),
            "[play] track limit: the on addons add 5 tracks, under the 22 that can fill the game's track lists (64 \
             tracks), so no check build ran (32 ms: identity 3 ms, counting 28 ms, record 1 ms)"
        );
    }

    #[test]
    fn limit_line_lists_the_check_builds_steps_and_whether_play_reuses_it() {
        let step = |n: &str, ms: u64| (n.to_owned(), Duration::from_millis(ms));
        let mut check = swofty_core::build::TrackLimitCheck {
            added: 31,
            counting: Duration::from_millis(8),
            steps: vec![
                step("resolving the addons", 40),
                step("reading the addons", 2100),
                step("editing the game database", 150_000),
                step("placing files", 3000),
                step("saving the build", 600),
            ],
            limit: Some(swofty_core::tracks::TrackLimit {
                room: 64,
                stock: 43,
                names: vec!["Mugello".into(), "Kyalami".into()],
                addons: vec!["mod.mugello".into(), "mod.kyalami".into()],
            }),
            saved: true,
            ..Default::default()
        };
        assert_eq!(
            limit_line(&check, &limit_times()),
            "[play] track limit: the on addons add 31 tracks, at least the 22 that can fill the game's track lists \
             (64 tracks), so the check planned the whole build to find the tracks left out: 155812 ms (identity 3 ms, \
             counting 8 ms; check build: resolving the addons 40 ms, reading the addons 2100 ms, editing the game \
             database 150000 ms, placing files 3000 ms, saving the build 600 ms; record 1 ms); 2 tracks left out; the \
             check build is saved, so Play restores it unless the addons or game files change first"
        );
        check.saved = false;
        check.limit = None;
        let line = limit_line(&check, &limit_times());
        assert!(
            line.contains("; none left out; the check build is not saved"),
            "{line}"
        );
        assert!(line.ends_with("so Play builds again"), "{line}");
        check.reused = true;
        check.saved = true;
        assert!(limit_line(&check, &limit_times()).ends_with("so nothing was built again"));
    }

    #[test]
    fn timings_record_in_order() {
        let mut t = Timings::default();
        assert_eq!(t.time("a", || 1), 1);
        t.time("b", || ());
        assert!(t.line().starts_with("[play] checks "));
        assert!(t.line().contains("(a "));
        assert!(t.line().contains(", b "));
    }

    #[test]
    fn a_second_press_while_checking_is_refused() {
        let mut p = Pending::default();
        let first = p.begin().unwrap();
        assert!(p.is_active());
        assert!(p.begin().is_none());
        assert!(p.finish(first));
        assert!(!p.is_active());
        assert!(p.begin().is_some());
    }

    #[test]
    fn a_cancelled_check_result_is_stale() {
        let mut p = Pending::default();
        let old = p.begin().unwrap();
        p.cancel();
        assert!(!p.is_active());
        let new = p.begin().unwrap();
        assert!(!p.finish(old));
        assert!(p.is_active());
        assert!(p.finish(new));
    }
}
