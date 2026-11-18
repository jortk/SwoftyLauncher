use std::time::Duration;

use semver::{Version, VersionReq};

use crate::launcher::Progress;
use crate::store::Store;
use crate::update::{self, Failure, Install, Release};
use crate::{Error, Result, managed};

pub const CHECK_INTERVAL: Duration = Duration::from_secs(60);
pub const RELEASE_RECHECK: Duration = Duration::from_secs(5 * 60);
pub const LIST_RECHECK: Duration = Duration::from_secs(15 * 60);
pub const UPDATED_ENV: &str = "SWOFTY_UPDATED_TO";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Addon,
    Launcher,
}

impl Part {
    pub fn name(self) -> &'static str {
        match self {
            Self::Addon => managed::NAME,
            Self::Launcher => "Swofty Launcher",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Addon => "Simplified",
            Self::Launcher => "Launcher",
        }
    }

    pub fn releases_url(self) -> &'static str {
        match self {
            Self::Addon => managed::RELEASES_URL,
            Self::Launcher => update::RELEASES_URL,
        }
    }

    pub fn api_url(self) -> &'static str {
        match self {
            Self::Addon => managed::LATEST_URL,
            Self::Launcher => update::LATEST_URL,
        }
    }

    pub fn ready(self, release: &Release) -> bool {
        match self {
            Self::Addon => managed::ready(release),
            Self::Launcher => release
                .platform_asset()
                .is_ok_and(|a| update::verifiable(release, a)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    Tag(String),
    NoRelease,
    Refused(Failure),
}

fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn tag_from_location(location: &str) -> Option<String> {
    let (_, rest) = location.split_once("/releases/tag/")?;
    let tag = unescape(rest.split(['?', '#', '/']).next()?);
    (!tag.is_empty()).then_some(tag)
}

pub fn read_probe(url: &str, status: u16, location: Option<&str>) -> Probe {
    if !(300..400).contains(&status) {
        return Probe::Refused(Failure::Failed(format!(
            "{url}: expected a redirect to the latest release, got HTTP {status}"
        )));
    }
    location
        .and_then(tag_from_location)
        .map_or(Probe::NoRelease, Probe::Tag)
}

pub fn probe(part: Part, timeout: Duration) -> Probe {
    let url = format!("{}/latest", part.releases_url());
    match update::request(&url, true, Some(timeout)) {
        Ok(r) => read_probe(
            &url,
            r.status().as_u16(),
            r.headers().get("location").and_then(|v| v.to_str().ok()),
        ),
        Err(f) => Probe::Refused(f),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Track {
    pub tag: Option<String>,
    pub release: Option<Release>,
    pub probe_at: u64,
    pub fetch_at: u64,
    pub error: Option<String>,
}

impl Track {
    pub fn probe_due(&self, now: u64) -> bool {
        now >= self.probe_at
    }

    pub fn version(&self) -> Option<Version> {
        update::parse_version(self.tag.as_deref()?)
    }

    pub fn newer_than(&self, installed: Option<&Version>) -> bool {
        self.version().is_some_and(|v| managed::newer(installed, &v))
    }

    pub fn on_probe(&mut self, probe: Probe, now: u64) {
        let interval = CHECK_INTERVAL.as_secs();
        self.probe_at = now + interval;
        self.error = None;
        match probe {
            Probe::Tag(tag) => self.tag = Some(tag),
            Probe::NoRelease => {
                self.tag = None;
                self.release = None;
            }
            Probe::Refused(Failure::Wait(secs, text)) => {
                self.probe_at = now + secs.max(interval);
                self.error = Some(text);
            }
            Probe::Refused(Failure::Failed(text)) => self.error = Some(text),
        }
    }

    pub fn on_manifest(&mut self, part: Part, release: Release, now: u64) {
        self.probe_at = now + CHECK_INTERVAL.as_secs();
        self.error = None;
        if part.ready(&release) {
            self.tag = Some(release.tag_name.clone());
            self.release = Some(release);
        } else {
            self.error = Some(format!("release {} is not complete yet", release.tag_name));
        }
    }

    pub fn fetch_due(&self, installed: Option<&Version>, now: u64) -> bool {
        self.newer_than(installed)
            && self.release.as_ref().map(|r| &r.tag_name) != self.tag.as_ref()
            && now >= self.fetch_at
    }

    pub fn on_fetch(&mut self, part: Part, fetched: std::result::Result<Release, Failure>, now: u64) {
        self.fetch_at = now + RELEASE_RECHECK.as_secs();
        match fetched {
            Ok(r) if part.ready(&r) && r.version().is_some_and(|v| self.version().is_none_or(|t| v >= t)) => {
                self.tag = Some(r.tag_name.clone());
                self.release = Some(r);
            }
            Ok(r) => self.error = Some(format!("release {} is not complete yet", r.tag_name)),
            Err(Failure::Wait(secs, text)) => {
                self.fetch_at = now + secs.max(CHECK_INTERVAL.as_secs());
                self.error = Some(text);
            }
            Err(Failure::Failed(text)) => self.error = Some(text),
        }
    }

    pub fn offer(&self, installed: Option<&Version>) -> Option<&Release> {
        self.release.as_ref().filter(|r| {
            Some(&r.tag_name) == self.tag.as_ref()
                && r.version().is_some_and(|v| managed::newer(installed, &v))
        })
    }
}

pub fn check(part: Part, track: &mut Track, installed: Option<&Version>, fetch: bool, now: u64) {
    if part == Part::Launcher
        && let Ok(release) = update::fetch_manifest(update::MANIFEST_URL, update::CHECK_TIMEOUT)
    {
        track.on_manifest(part, release, now);
        return;
    }
    track.on_probe(probe(part, update::CHECK_TIMEOUT), now);
    if fetch && track.fetch_due(installed, now) {
        track.on_fetch(
            part,
            update::fetch_release(part.api_url(), update::CHECK_TIMEOUT),
            now,
        );
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub release: Release,
    pub version: Version,
    pub needs: Option<VersionReq>,
    pub learn_at: u64,
}

impl Candidate {
    pub fn accepts(&self, launcher: &Version) -> bool {
        self.needs.as_ref().is_some_and(|r| r.matches(launcher))
    }

    fn same_package(&self, release: &Release) -> bool {
        self.release.tag_name == release.tag_name
            && managed::package_asset(&self.release).ok() == managed::package_asset(release).ok()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub latest: Track,
    pub releases: Vec<Candidate>,
    pub listed_for: Option<String>,
    pub list_at: u64,
    pub wait_until: u64,
    pub error: Option<String>,
}

impl Catalog {
    pub fn probe_due(&self, now: u64) -> bool {
        self.latest.probe_due(now)
    }

    pub fn list_due(&self, now: u64) -> bool {
        now >= self.wait_until && (now >= self.list_at || self.latest.tag != self.listed_for)
    }

    pub fn on_list(&mut self, fetched: std::result::Result<Vec<Release>, Failure>, now: u64) {
        self.listed_for = self.latest.tag.clone();
        let list = match fetched {
            Ok(list) => list,
            Err(Failure::Wait(secs, text)) => {
                self.wait_until = now + secs.max(CHECK_INTERVAL.as_secs());
                self.error = Some(text);
                return;
            }
            Err(Failure::Failed(text)) => {
                self.list_at = now + RELEASE_RECHECK.as_secs();
                self.error = Some(text);
                return;
            }
        };
        let (mut next, mut unfinished) = (Vec::new(), None::<Version>);
        for release in list.into_iter().filter(|r| !r.draft && !r.prerelease) {
            let Some(version) = release.version() else {
                continue;
            };
            if !managed::ready(&release) {
                unfinished = unfinished.max(Some(version));
                continue;
            }
            let known = self.releases.iter().find(|c| c.same_package(&release));
            next.push(Candidate {
                needs: known.and_then(|c| c.needs.clone()),
                learn_at: known.map_or(0, |c| c.learn_at),
                version,
                release,
            });
        }
        next.sort_by(|a, b| b.version.cmp(&a.version));
        let waiting = unfinished.is_some_and(|u| next.first().is_none_or(|c| u > c.version));
        self.list_at = now + if waiting { RELEASE_RECHECK } else { LIST_RECHECK }.as_secs();
        self.releases = next;
        self.error = None;
    }

    fn newer(&self, installed: Option<&Version>) -> Vec<&Candidate> {
        self.releases
            .iter()
            .filter(|c| managed::newer(installed, &c.version))
            .collect()
    }

    pub fn to_learn(&self, installed: Option<&Version>, launcher: &Version, now: u64) -> Vec<Release> {
        let mut out = Vec::new();
        for c in self.newer(installed) {
            if c.accepts(launcher) {
                break;
            }
            if c.needs.is_none() && now >= c.learn_at {
                out.push(c.release.clone());
            }
        }
        out
    }

    pub fn learn(
        &mut self,
        installed: Option<&Version>,
        launcher: &Version,
        now: u64,
        mut needs: impl FnMut(&Release) -> Result<VersionReq>,
    ) {
        for release in self.to_learn(installed, launcher, now) {
            let learned = needs(&release);
            let Some(c) = self
                .releases
                .iter_mut()
                .find(|c| c.release.tag_name == release.tag_name)
            else {
                continue;
            };
            match learned {
                Ok(req) => {
                    c.needs = Some(req);
                    if c.accepts(launcher) {
                        break;
                    }
                }
                Err(e) => {
                    c.learn_at = now + RELEASE_RECHECK.as_secs();
                    self.error = Some(format!("{} {}: {e}", managed::NAME, release.tag_name));
                }
            }
        }
    }

    pub fn choose(&self, launcher: &Version, installed: Option<&Version>) -> Option<&Release> {
        self.releases
            .iter()
            .find(|c| c.accepts(launcher))
            .filter(|c| managed::newer(installed, &c.version))
            .map(|c| &c.release)
    }

    pub fn waiting(&self, launcher: &Version, installed: Option<&Version>) -> Option<&Candidate> {
        let chosen = self.choose(launcher, installed).and_then(Release::version);
        self.newer(installed).into_iter().find(|c| {
            c.needs.is_some() && !c.accepts(launcher) && chosen.as_ref().is_none_or(|v| c.version > *v)
        })
    }

    pub fn keep(&self, installed: Option<&Version>) -> Vec<&str> {
        self.newer(installed)
            .into_iter()
            .map(|c| c.release.tag_name.as_str())
            .collect()
    }
}

pub fn check_importer(
    store: &Store,
    catalog: &mut Catalog,
    installed: Option<&Version>,
    launcher: &Version,
    now: u64,
) {
    catalog
        .latest
        .on_probe(probe(Part::Addon, update::CHECK_TIMEOUT), now);
    if catalog.list_due(now) {
        catalog.on_list(managed::fetch_releases(update::CHECK_TIMEOUT), now);
    }
    catalog.learn(installed, launcher, now, |r| managed::requirement(store, r));
    if !catalog.releases.is_empty() {
        managed::prune(store, &catalog.keep(installed));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Idle,
    Installing,
    Failed(String),
    Restarting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Play,
    Update,
    Installing,
    Retry,
    Restarting,
}

pub fn gate(phase: &Phase, pending: bool, game_busy: bool) -> Gate {
    match phase {
        Phase::Installing => Gate::Installing,
        Phase::Restarting => Gate::Restarting,
        _ if game_busy || !pending => Gate::Play,
        Phase::Failed(_) => Gate::Retry,
        Phase::Idle => Gate::Update,
    }
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub launcher: Option<Release>,
    pub addon: Option<Release>,
    pub fallback: Option<Release>,
}

impl Plan {
    pub fn steps(&self) -> Vec<(Part, &Release)> {
        [(Part::Launcher, &self.launcher), (Part::Addon, &self.addon)]
            .into_iter()
            .filter_map(|(part, release)| release.as_ref().map(|r| (part, r)))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.launcher.is_none() && self.addon.is_none()
    }

    pub fn versions(&self) -> Vec<(Part, Version)> {
        self.steps()
            .into_iter()
            .filter_map(|(part, r)| r.version().map(|v| (part, v)))
            .collect()
    }
}

// The importer is checked against the launcher that runs after this update,
// so one run can take a launcher and the importer that needs it.
pub fn plan(
    catalog: &Catalog,
    addon: Option<&Version>,
    launcher: Option<&Release>,
    current: &Version,
    held: Option<&str>,
) -> Plan {
    let pick = |target: &Version| {
        addon
            .and_then(|installed| catalog.choose(target, Some(installed)))
            .filter(|r| Some(r.tag_name.as_str()) != held)
            .cloned()
    };
    let fallback = pick(current);
    Plan {
        launcher: launcher.cloned(),
        addon: match launcher.and_then(Release::version) {
            Some(next) => pick(&next),
            None => fallback.clone(),
        },
        fallback,
    }
}

pub fn follow_up(restarted: bool, plan: &Plan) -> bool {
    restarted && plan.launcher.is_none() && plan.addon.is_some()
}

pub fn summary(parts: &[(Part, Version)]) -> String {
    match parts {
        [(part, version)] => format!("{} {version}", part.short()),
        _ => parts
            .iter()
            .map(|(part, _)| part.short())
            .collect::<Vec<_>>()
            .join(" + "),
    }
}

pub fn describe(parts: &[(Part, Version)]) -> String {
    parts
        .iter()
        .map(|(part, version)| format!("{} {version}", part.name()))
        .collect::<Vec<_>>()
        .join(" and ")
}

pub struct Step<'a> {
    inner: &'a dyn Progress,
    index: usize,
    count: usize,
}

impl Progress for Step<'_> {
    fn report(&self, done: f32, text: &str) {
        let overall = (self.index as f32 + done.clamp(0., 1.)) / self.count.max(1) as f32;
        if self.count > 1 {
            self.inner
                .report(overall, &format!("{text} ({} of {})", self.index + 1, self.count));
        } else {
            self.inner.report(overall, text);
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Ran {
    pub done: Vec<(Part, Version)>,
    pub failed: Vec<(Part, String, String)>,
}

impl Ran {
    pub fn restart(&self) -> bool {
        self.launcher().is_some() || (!self.done.is_empty() && self.failed.is_empty())
    }

    fn version(&self, part: Part) -> Option<&Version> {
        self.done.iter().find(|(p, _)| *p == part).map(|(_, v)| v)
    }

    pub fn launcher(&self) -> Option<&Version> {
        self.version(Part::Launcher)
    }

    pub fn addon(&self) -> Option<&Version> {
        self.version(Part::Addon)
    }

    pub fn failed_tag(&self, part: Part) -> Option<&str> {
        self.failed
            .iter()
            .find(|(p, _, _)| *p == part)
            .map(|(_, tag, _)| tag.as_str())
    }

    pub fn error(&self) -> Option<String> {
        (!self.failed.is_empty()).then(|| {
            self.failed
                .iter()
                .map(|(part, tag, e)| format!("{} {tag}: {e}", part.name()))
                .collect::<Vec<_>>()
                .join("; ")
        })
    }
}

type Installer<'a> = dyn FnMut(Part, &Release, &Version, &dyn Progress) -> Result<Version> + 'a;

fn attempt(
    ran: &mut Ran,
    install: &mut Installer<'_>,
    part: Part,
    release: &Release,
    launcher: &Version,
    step: Step<'_>,
) -> Option<Version> {
    step.report(0., &format!("Updating {}", part.name()));
    match install(part, release, launcher, &step) {
        Ok(version) => {
            ran.done.push((part, version.clone()));
            Some(version)
        }
        Err(e) => {
            ran.failed.push((part, release.tag_name.clone(), e.to_string()));
            None
        }
    }
}

pub fn run(
    plan: &Plan,
    current: &Version,
    progress: &dyn Progress,
    mut install: impl FnMut(Part, &Release, &Version, &dyn Progress) -> Result<Version>,
) -> Ran {
    let count = plan.steps().len();
    let mut ran = Ran::default();
    let (mut next, mut addon, mut index) = (current.clone(), plan.addon.as_ref(), 0);
    if let Some(release) = &plan.launcher {
        let step = Step {
            inner: progress,
            index,
            count,
        };
        match attempt(&mut ran, &mut install, Part::Launcher, release, current, step) {
            Some(version) => next = version,
            None => addon = plan.fallback.as_ref(),
        }
        index += 1;
    }
    if let Some(release) = addon {
        let step = Step {
            inner: progress,
            index,
            count: count.max(index + 1),
        };
        attempt(&mut ran, &mut install, Part::Addon, release, &next, step);
    }
    ran
}

pub fn install(
    store: &Store,
    copy: Option<&Install>,
    part: Part,
    release: &Release,
    launcher: &Version,
    progress: &dyn Progress,
) -> Result<Version> {
    if crate::process::game_running() {
        return Err(Error::Other("close F1 25 before updating".into()));
    }
    match part {
        Part::Addon => managed::install_release(store, release, launcher, progress),
        Part::Launcher => copy
            .ok_or_else(|| Error::Other("this copy cannot update itself".into()))?
            .update(release, progress),
    }
}

pub fn did_not_take(expected: Option<&str>, current: &Version) -> Option<String> {
    let wanted = update::parse_version(expected?)?;
    (wanted > *current).then(|| {
        format!("Swofty Launcher {wanted} was installed, but the restarted launcher is still {current}")
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::update::{Asset, SUMS_FILE};

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn asset(name: &str) -> Asset {
        Asset {
            name: name.into(),
            size: 10,
            browser_download_url: format!("https://example.invalid/{name}"),
            digest: None,
        }
    }

    fn launcher_release(tag: &str) -> Release {
        let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
        let platform = update::asset_name(tag, os, arch).unwrap_or_else(|| "none".into());
        Release {
            tag_name: tag.into(),
            assets: vec![asset(&platform), asset(SUMS_FILE)],
            ..Default::default()
        }
    }

    fn addon_release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            assets: vec![asset(managed::ASSET), asset(SUMS_FILE)],
            ..Default::default()
        }
    }

    struct Log(Mutex<Vec<(f32, String)>>);

    impl Progress for Log {
        fn report(&self, done: f32, text: &str) {
            self.0.lock().unwrap().push((done, text.to_owned()));
        }
    }

    #[test]
    fn a_cdn_manifest_sets_the_offer_without_a_probe() {
        let mut t = Track::default();
        let mut bare = launcher_release("v1.0.9");
        bare.assets.retain(|a| a.name != SUMS_FILE);
        t.on_manifest(Part::Launcher, bare, 100);
        assert_eq!(t.probe_at, 100 + CHECK_INTERVAL.as_secs());
        assert!(t.tag.is_none() && t.error.is_some());
        let mut ok = launcher_release("v1.0.9");
        ok.assets.retain(|a| a.name != SUMS_FILE);
        for a in &mut ok.assets {
            a.digest = Some(format!("sha256:{}", "a".repeat(64)));
        }
        t.on_manifest(Part::Launcher, ok, 200);
        assert!(t.error.is_none());
        assert_eq!(
            t.offer(Some(&v("1.0.8"))).map(|r| r.tag_name.as_str()),
            Some("v1.0.9")
        );
        assert!(t.offer(Some(&v("1.0.9"))).is_none());
    }

    #[test]
    fn reads_the_tag_from_the_latest_redirect() {
        let url = "https://github.com/o/r/releases/latest";
        assert_eq!(
            read_probe(url, 302, Some("https://github.com/o/r/releases/tag/v1.0.8")),
            Probe::Tag("v1.0.8".into())
        );
        assert_eq!(
            read_probe(url, 302, Some("/o/r/releases/tag/v1.0.9-rc.1%2Bbuild?x=1")),
            Probe::Tag("v1.0.9-rc.1+build".into())
        );
        assert_eq!(
            read_probe(url, 302, Some("https://github.com/o/r/releases")),
            Probe::NoRelease
        );
        assert_eq!(read_probe(url, 302, None), Probe::NoRelease);
        assert!(matches!(
            read_probe(url, 200, None),
            Probe::Refused(Failure::Failed(_))
        ));
    }

    #[test]
    fn checks_every_minute_and_backs_off_when_limited() {
        let mut t = Track::default();
        assert!(t.probe_due(0));
        t.on_probe(Probe::Tag("v1.0.8".into()), 1000);
        assert!(!t.probe_due(1059));
        assert!(t.probe_due(1060));
        t.on_probe(Probe::Refused(Failure::Failed("offline".into())), 1060);
        assert_eq!(
            t.tag.as_deref(),
            Some("v1.0.8"),
            "a failed check keeps what it knew"
        );
        assert_eq!(t.error.as_deref(), Some("offline"));
        assert!(t.probe_due(1120));
        t.on_probe(Probe::Refused(Failure::Wait(900, "limited".into())), 1120);
        assert!(!t.probe_due(1120 + 899));
        assert!(t.probe_due(1120 + 900));
        t.on_probe(Probe::Refused(Failure::Wait(5, "limited".into())), 3000);
        assert!(!t.probe_due(3030), "never sooner than the interval");
        t.on_probe(Probe::Tag("v1.0.9".into()), 4000);
        assert!(t.error.is_none());
        t.on_probe(Probe::NoRelease, 5000);
        assert!(t.tag.is_none() && t.release.is_none());
    }

    #[test]
    fn asks_the_api_once_per_new_release() {
        let current = v("1.0.8");
        let mut t = Track::default();
        t.on_probe(Probe::Tag("v1.0.8".into()), 1000);
        assert!(!t.fetch_due(Some(&current), 1000), "up to date costs no API call");
        t.on_probe(Probe::Tag("v1.0.9".into()), 1060);
        assert!(t.fetch_due(Some(&current), 1060));
        t.on_fetch(Part::Launcher, Ok(launcher_release("v1.0.9")), 1060);
        assert_eq!(t.offer(Some(&current)).unwrap().tag_name, "v1.0.9");
        assert!(t.offer(Some(&v("1.0.9"))).is_none());
        t.on_probe(Probe::Tag("v1.0.9".into()), 1120);
        assert!(
            !t.fetch_due(Some(&current), 1120),
            "the known release is not fetched again"
        );
        t.on_probe(Probe::Tag("v1.1.0".into()), 2000);
        assert!(
            t.offer(Some(&current)).is_none(),
            "a stale release is not offered"
        );
        assert!(t.fetch_due(Some(&current), 2000));
    }

    #[test]
    fn waits_for_an_incomplete_or_limited_release() {
        let current = v("1.0.8");
        let mut t = Track::default();
        t.on_probe(Probe::Tag("v1.0.9".into()), 1000);
        let bare = Release {
            assets: vec![],
            ..launcher_release("v1.0.9")
        };
        t.on_fetch(Part::Launcher, Ok(bare), 1000);
        assert!(t.offer(Some(&current)).is_none());
        assert!(!t.fetch_due(Some(&current), 1000 + RELEASE_RECHECK.as_secs() - 1));
        assert!(t.fetch_due(Some(&current), 1000 + RELEASE_RECHECK.as_secs()));
        t.on_fetch(Part::Launcher, Ok(launcher_release("v1.0.8")), 1300);
        assert!(
            t.offer(Some(&current)).is_none(),
            "an older API answer is ignored"
        );
        t.on_fetch(Part::Launcher, Err(Failure::Wait(3000, "limited".into())), 1600);
        assert!(!t.fetch_due(Some(&current), 4599));
        assert!(t.fetch_due(Some(&current), 4600));
        t.on_fetch(Part::Launcher, Ok(launcher_release("v1.0.9")), 4600);
        assert!(t.offer(Some(&current)).is_some());
    }

    #[test]
    fn a_missing_addon_is_always_offered() {
        let mut t = Track::default();
        t.on_probe(Probe::Tag("v0.5.2".into()), 10);
        assert!(t.fetch_due(None, 10));
        t.on_fetch(Part::Addon, Ok(addon_release("v0.5.2")), 10);
        assert!(t.offer(None).is_some());
        assert!(t.offer(Some(&v("0.5.2"))).is_none());
        let mut l = Track::default();
        l.on_probe(Probe::Tag("v0.5.2".into()), 10);
        l.on_fetch(Part::Launcher, Ok(addon_release("v0.5.2")), 10);
        assert!(l.release.is_none(), "an addon release is not a launcher release");
    }

    #[test]
    fn update_takes_the_play_button_only_when_safe() {
        assert_eq!(gate(&Phase::Idle, false, false), Gate::Play);
        assert_eq!(gate(&Phase::Idle, true, false), Gate::Update);
        assert_eq!(
            gate(&Phase::Idle, true, true),
            Gate::Play,
            "the game keeps its button"
        );
        assert_eq!(gate(&Phase::Installing, true, false), Gate::Installing);
        assert_eq!(gate(&Phase::Restarting, true, true), Gate::Restarting);
        let failed = Phase::Failed("no network".into());
        assert_eq!(gate(&failed, true, false), Gate::Retry);
        assert_eq!(gate(&failed, true, true), Gate::Play);
        assert_eq!(gate(&failed, false, false), Gate::Play);
    }

    fn catalog(releases: &[(&str, Option<&str>)]) -> Catalog {
        let mut c = Catalog::default();
        c.on_list(
            Ok(releases.iter().map(|(tag, _)| addon_release(tag)).collect()),
            0,
        );
        for (tag, needs) in releases {
            let found = c
                .releases
                .iter_mut()
                .find(|r| r.release.tag_name == *tag)
                .unwrap();
            found.needs = needs.map(|n| n.parse().unwrap());
        }
        c
    }

    fn tags(plan: &Plan) -> Vec<(Part, &str)> {
        plan.steps()
            .into_iter()
            .map(|(part, r)| (part, r.tag_name.as_str()))
            .collect()
    }

    #[test]
    fn lists_ready_importers_newest_first() {
        let mut c = Catalog::default();
        let draft = Release {
            draft: true,
            ..addon_release("v0.9.0")
        };
        let pre = Release {
            prerelease: true,
            ..addon_release("v0.8.1")
        };
        let bare = Release {
            assets: vec![],
            ..addon_release("v0.4.0")
        };
        let list = vec![
            addon_release("v0.6.0"),
            draft,
            pre,
            addon_release("v0.8.0"),
            bare,
            addon_release("latest"),
            addon_release("v0.7.0"),
        ];
        c.on_list(Ok(list.clone()), 1000);
        let listed: Vec<&str> = c.releases.iter().map(|r| r.release.tag_name.as_str()).collect();
        assert_eq!(listed, ["v0.8.0", "v0.7.0", "v0.6.0"]);
        assert_eq!(c.list_at, 1000 + LIST_RECHECK.as_secs());
        c.releases[1].needs = Some(">=1.3.0".parse().unwrap());
        c.on_list(Ok(list), 2000);
        assert!(
            c.releases[1].needs.is_some(),
            "a known requirement survives a new list"
        );
        let replaced = Release {
            assets: vec![
                Asset {
                    size: 11,
                    ..asset(managed::ASSET)
                },
                asset(SUMS_FILE),
            ],
            ..addon_release("v0.7.0")
        };
        c.on_list(Ok(vec![replaced]), 3000);
        assert!(c.releases[0].needs.is_none(), "a replaced package is read again");
        c.on_list(
            Ok(vec![
                addon_release("v0.7.0"),
                Release {
                    assets: vec![],
                    ..addon_release("v0.8.0")
                },
            ]),
            4000,
        );
        assert_eq!(
            c.list_at,
            4000 + RELEASE_RECHECK.as_secs(),
            "a release still uploading is looked at again sooner"
        );
    }

    #[test]
    fn lists_releases_on_a_new_latest_tag_or_every_quarter_hour() {
        let mut c = Catalog::default();
        assert!(c.list_due(0));
        c.latest.on_probe(Probe::Tag("v0.6.0".into()), 1000);
        c.on_list(Ok(vec![addon_release("v0.6.0")]), 1000);
        assert!(!c.list_due(1060), "an unchanged latest tag costs no API call");
        assert!(c.list_due(1000 + LIST_RECHECK.as_secs()));
        c.latest.on_probe(Probe::Tag("v0.7.0".into()), 1120);
        assert!(c.list_due(1120));
        c.on_list(Err(Failure::Wait(3000, "limited".into())), 1120);
        assert!(!c.list_due(1120 + 2999));
        assert!(c.list_due(1120 + 3000));
        c.on_list(Err(Failure::Failed("offline".into())), 5000);
        assert!(!c.list_due(5060), "a failed list is not retried every minute");
        assert!(c.list_due(5000 + RELEASE_RECHECK.as_secs()));
        assert_eq!(c.error.as_deref(), Some("offline"));
    }

    #[test]
    fn learns_requirements_newest_first_until_one_fits() {
        let mut c = catalog(&[("v0.8.0", None), ("v0.7.0", None), ("v0.6.0", None)]);
        let needs = |r: &Release| -> Result<VersionReq> {
            Ok(match r.tag_name.as_str() {
                "v0.8.0" => ">=1.4.0",
                "v0.7.0" => ">=1.3.0",
                _ => ">=1.0.4",
            }
            .parse()
            .unwrap())
        };
        let mut asked = Vec::new();
        c.learn(Some(&v("0.6.0")), &v("1.3.1"), 0, |r| {
            asked.push(r.tag_name.clone());
            needs(r)
        });
        assert_eq!(
            asked,
            ["v0.8.0", "v0.7.0"],
            "stops at the first that fits, never reads the installed one"
        );
        assert!(
            c.to_learn(Some(&v("0.6.0")), &v("1.3.1"), 0).is_empty(),
            "each release is read once"
        );
        let mut c = catalog(&[("v0.8.0", None), ("v0.7.0", None)]);
        c.learn(None, &v("1.3.1"), 100, |_| Err(Error::Other("offline".into())));
        assert!(c.error.as_deref().unwrap().contains("offline"));
        assert!(
            c.to_learn(None, &v("1.3.1"), 101).is_empty(),
            "a failed read waits"
        );
        assert_eq!(
            c.to_learn(None, &v("1.3.1"), 100 + RELEASE_RECHECK.as_secs())
                .len(),
            2
        );
    }

    #[test]
    fn chooses_the_newest_importer_the_launcher_accepts() {
        let c = catalog(&[
            ("v0.8.0", Some(">=1.4.0")),
            ("v0.7.0", Some(">=1.3.0")),
            ("v0.6.0", Some(">=1.0.4")),
        ]);
        let installed = v("0.5.2");
        let tag = |launcher: &str| {
            c.choose(&v(launcher), Some(&installed))
                .map(|r| r.tag_name.clone())
        };
        assert_eq!(tag("1.2.6").as_deref(), Some("v0.6.0"));
        assert_eq!(tag("1.3.1").as_deref(), Some("v0.7.0"));
        assert_eq!(tag("1.4.0").as_deref(), Some("v0.8.0"));
        assert!(
            c.choose(&v("1.3.1"), Some(&v("0.7.0"))).is_none(),
            "never a downgrade or the same"
        );
        assert_eq!(
            c.waiting(&v("1.3.1"), Some(&installed)).unwrap().version,
            v("0.8.0")
        );
        assert!(c.waiting(&v("1.4.0"), Some(&installed)).is_none());
        let unknown = catalog(&[("v0.8.0", None), ("v0.7.0", Some(">=1.3.0"))]);
        assert_eq!(
            unknown.choose(&v("1.3.1"), Some(&installed)).unwrap().tag_name,
            "v0.7.0",
            "a release whose requirement isn't known yet is passed over"
        );
    }

    #[test]
    fn the_incident_updates_the_launcher_and_the_importer_in_one_run() {
        let store = managed::testing::store("incident");
        managed::install_release(
            &store,
            &managed::testing::seed(&store, "0.6.0", ">=1.0.4"),
            &v("1.2.6"),
            &Log(Mutex::default()),
        )
        .unwrap();
        let mut c = Catalog::default();
        c.latest.on_probe(Probe::Tag("v0.7.0".into()), 0);
        let newest = managed::testing::seed(&store, "0.7.0", ">=1.3.0");
        c.on_list(Ok(vec![newest, addon_release("v0.6.0")]), 0);
        let (current, installed) = (v("1.2.6"), managed::installed(&store));
        assert_eq!(installed, Some(v("0.6.0")));
        c.learn(installed.as_ref(), &current, 0, |r| {
            managed::requirement(&store, r)
        });
        assert_eq!(c.releases[0].needs, Some(">=1.3.0".parse().unwrap()));

        let alone = plan(&c, installed.as_ref(), None, &current, None);
        assert!(
            alone.is_empty(),
            "1.2.6 alone is never offered an importer it can't load"
        );
        assert_eq!(gate(&Phase::Idle, !alone.is_empty(), false), Gate::Play);

        let launcher = launcher_release("v1.3.0");
        let both = plan(&c, installed.as_ref(), Some(&launcher), &current, None);
        assert_eq!(tags(&both), [(Part::Launcher, "v1.3.0"), (Part::Addon, "v0.7.0")]);
        assert_eq!(gate(&Phase::Idle, !both.is_empty(), false), Gate::Update);
        let log = Log(Mutex::default());
        let mut seen = Vec::new();
        let ran = run(&both, &current, &log, |part, release, next, progress| {
            seen.push((part, next.clone()));
            match part {
                Part::Launcher => Ok(release.version().unwrap()),
                Part::Addon => managed::install_release(&store, release, next, progress),
            }
        });
        assert_eq!(
            seen,
            [(Part::Launcher, v("1.2.6")), (Part::Addon, v("1.3.0"))],
            "the importer is checked against the launcher it updates to"
        );
        assert_eq!(ran.error(), None);
        assert_eq!(
            ran.done,
            [(Part::Launcher, v("1.3.0")), (Part::Addon, v("0.7.0"))]
        );
        assert!(ran.restart(), "one restart, into 1.3.0");
        assert_eq!(managed::installed(&store), Some(v("0.7.0")));
        let after = plan(&c, managed::installed(&store).as_ref(), None, &v("1.3.0"), None);
        assert!(after.is_empty(), "nothing is left after the restart");
        let log = log.0.into_inner().unwrap();
        assert!(
            log.iter().any(|(_, t)| t == "Updating Swofty Launcher (1 of 2)"),
            "{log:?}"
        );
        assert!(
            log.iter()
                .any(|(_, t)| t == "Updating Simplified for Swofty (2 of 2)"),
            "{log:?}"
        );
        assert_eq!(summary(&ran.done), "Launcher + Simplified");
        assert_eq!(
            describe(&ran.done),
            "Swofty Launcher 1.3.0 and Simplified for Swofty 0.7.0"
        );
        std::fs::remove_dir_all(&store.root).ok();
    }

    #[test]
    fn an_old_launcher_update_is_followed_by_the_importer_after_the_restart() {
        let c = catalog(&[("v0.7.0", Some(">=1.3.0")), ("v0.6.0", Some(">=1.0.4"))]);
        let next = plan(&c, Some(&v("0.6.0")), None, &v("1.3.1"), None);
        assert_eq!(tags(&next), [(Part::Addon, "v0.7.0")]);
        assert!(
            follow_up(true, &next),
            "a launcher restarted by an update installs it on its own"
        );
        assert!(!follow_up(false, &next));
        let both = plan(
            &c,
            Some(&v("0.6.0")),
            Some(&launcher_release("v1.3.2")),
            &v("1.3.1"),
            None,
        );
        assert!(!follow_up(true, &both), "a pending launcher waits for a click");
    }

    #[test]
    fn an_importer_for_a_launcher_not_out_yet_is_not_offered() {
        let c = catalog(&[("v0.8.0", Some(">=1.4.0")), ("v0.7.0", Some(">=1.3.0"))]);
        let installed = v("0.7.0");
        let p = plan(&c, Some(&installed), None, &v("1.3.1"), None);
        assert!(p.is_empty());
        assert_eq!(gate(&Phase::Idle, !p.is_empty(), false), Gate::Play);
        assert_eq!(
            c.waiting(&v("1.3.1"), Some(&installed)).unwrap().version,
            v("0.8.0")
        );
        let p = plan(
            &c,
            Some(&installed),
            Some(&launcher_release("v1.3.2")),
            &v("1.3.1"),
            None,
        );
        assert_eq!(
            tags(&p),
            [(Part::Launcher, "v1.3.2")],
            "the launcher still updates"
        );
        let ran = run(&p, &v("1.3.1"), &Log(Mutex::default()), |_, r, _, _| {
            Ok(r.version().unwrap())
        });
        assert_eq!(ran.done, [(Part::Launcher, v("1.3.2"))]);
        assert!(ran.restart());
    }

    #[test]
    fn a_failed_importer_still_updates_the_launcher() {
        let c = catalog(&[("v0.7.0", Some(">=1.3.0"))]);
        let p = plan(
            &c,
            Some(&v("0.6.0")),
            Some(&launcher_release("v1.3.0")),
            &v("1.2.6"),
            None,
        );
        let mut seen = Vec::new();
        let ran = run(&p, &v("1.2.6"), &Log(Mutex::default()), |part, release, _, _| {
            seen.push(part);
            match part {
                Part::Launcher => Ok(release.version().unwrap()),
                Part::Addon => Err(Error::Other("SHA-256 mismatch".into())),
            }
        });
        assert_eq!(seen, [Part::Launcher, Part::Addon]);
        assert_eq!(ran.launcher(), Some(&v("1.3.0")));
        assert!(ran.restart(), "the new launcher still starts");
        assert_eq!(ran.failed_tag(Part::Addon), Some("v0.7.0"));
        assert!(
            ran.error()
                .unwrap()
                .contains("Simplified for Swofty v0.7.0: SHA-256 mismatch")
        );
        let held = plan(&c, Some(&v("0.6.0")), None, &v("1.3.0"), Some("v0.7.0"));
        assert!(held.is_empty(), "an importer that failed doesn't hold Play");
        assert_eq!(
            gate(&Phase::Failed("x".into()), !held.is_empty(), false),
            Gate::Play
        );
    }

    #[test]
    fn a_failed_launcher_still_updates_a_compatible_importer() {
        let c = catalog(&[("v0.8.0", Some(">=1.3.1")), ("v0.7.0", Some(">=1.3.0"))]);
        let p = plan(
            &c,
            Some(&v("0.6.0")),
            Some(&launcher_release("v1.3.1")),
            &v("1.3.0"),
            None,
        );
        assert_eq!(tags(&p), [(Part::Launcher, "v1.3.1"), (Part::Addon, "v0.8.0")]);
        let mut seen = Vec::new();
        let ran = run(
            &p,
            &v("1.3.0"),
            &Log(Mutex::default()),
            |part, release, next, _| {
                seen.push((part, release.tag_name.clone(), next.clone()));
                match part {
                    Part::Launcher => Err(Error::Other("disk full".into())),
                    Part::Addon => Ok(release.version().unwrap()),
                }
            },
        );
        assert_eq!(
            seen,
            [
                (Part::Launcher, "v1.3.1".into(), v("1.3.0")),
                (Part::Addon, "v0.7.0".into(), v("1.3.0"))
            ],
            "the importer falls back to the newest the running launcher accepts"
        );
        assert_eq!(ran.addon(), Some(&v("0.7.0")));
        assert!(!ran.restart(), "the failure stays on screen");
        assert!(ran.error().unwrap().contains("Swofty Launcher v1.3.1: disk full"));
        let only = catalog(&[("v0.8.0", Some(">=1.3.1"))]);
        let p = plan(
            &only,
            Some(&v("0.7.0")),
            Some(&launcher_release("v1.3.1")),
            &v("1.3.0"),
            None,
        );
        let ran = run(
            &p,
            &v("1.3.0"),
            &Log(Mutex::default()),
            |part, release, _, _| match part {
                Part::Launcher => Err(Error::Other("disk full".into())),
                Part::Addon => Ok(release.version().unwrap()),
            },
        );
        assert!(
            ran.done.is_empty(),
            "an importer that needs the failed launcher waits"
        );
        assert_eq!(ran.failed.len(), 1);
        let addon_only = run(
            &plan(&only, Some(&v("0.7.0")), None, &v("1.3.1"), None),
            &v("1.3.1"),
            &Log(Mutex::default()),
            |_, r, _, _| Ok(r.version().unwrap()),
        );
        assert!(addon_only.restart(), "an importer-only update restarts too");
    }

    #[test]
    fn notices_an_update_that_did_not_take() {
        assert!(did_not_take(None, &v("1.0.8")).is_none());
        assert!(did_not_take(Some("v1.0.8"), &v("1.0.8")).is_none());
        assert!(did_not_take(Some("garbage"), &v("1.0.8")).is_none());
        let text = did_not_take(Some("v1.0.9"), &v("1.0.8")).unwrap();
        assert!(text.contains("1.0.9") && text.contains("1.0.8"), "{text}");
    }
}
