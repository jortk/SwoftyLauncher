use std::collections::{BTreeMap, HashSet};

use swofty_core::catalog::requires::{Event, Plan, Step};
use swofty_core::catalog::{Addon, Stage, Update, Version};

use super::detail::shown_versions;
use super::host;
use super::jobs::{RetryAction, requested_version_available, retry_action};
use super::model::{Job, Progress, Verb};
use super::updates::{due_updates, held_failure, takes_updates, updated_addons};
use crate::format;
use crate::library::{AddonItem, Source};

fn versions(list: &[&str]) -> Vec<Version> {
    list.iter()
        .map(|v| {
            serde_json::from_value(serde_json::json!({
                "version": v, "size": 0, "sha256": "", "url": ""
            }))
            .unwrap()
        })
        .collect()
}

fn shown(list: &[&str], installed: &str) -> usize {
    let all = versions(list);
    let refs: Vec<&Version> = all.iter().collect();
    shown_versions(&refs, &semver::Version::parse(installed).unwrap())
}

#[test]
fn shows_the_newer_versions_and_the_installed_one() {
    assert_eq!(shown(&["1.3.0", "1.2.0", "1.1.0", "1.0.0"], "1.1.0"), 3);
}

#[test]
fn shows_two_when_up_to_date() {
    assert_eq!(shown(&["1.2.0", "1.1.0", "1.0.0"], "1.2.0"), 2);
    assert_eq!(shown(&["1.2.0"], "1.2.0"), 1);
}

#[test]
fn an_unlisted_install_stops_at_the_versions_above_it() {
    assert_eq!(shown(&["1.3.0", "1.2.0", "1.0.0"], "1.1.0"), 2);
    assert_eq!(shown(&["1.3.0", "1.2.5", "1.2.0", "1.0.0"], "1.1.0"), 3);
    assert_eq!(shown(&["1.3.0", "1.2.0"], "2.0.0"), 2);
}

fn step(id: &str, size: u64) -> Step {
    let addon: Addon =
        serde_json::from_value(serde_json::json!({ "id": id, "name": format!("{id} name") })).unwrap();
    let version: Version = serde_json::from_value(serde_json::json!({
        "version": "1.0.0", "size": size, "sha256": "", "url": ""
    }))
    .unwrap();
    Step {
        addon,
        version,
        replaces: None,
    }
}

#[test]
fn queued_install_reports_partial_failure_and_retries_without_duplication() {
    use crate::download_queue::{Queue, State};
    use std::collections::BTreeSet;
    let mut queue = Queue::default();
    assert!(queue.enqueue("root".into(), "1.0.0", BTreeSet::new()));
    assert!(!queue.enqueue("root".into(), "2.0.0", BTreeSet::new()));
    assert_eq!(queue.start_next().as_deref(), Some("root"));
    let job = Job::new(100);
    let dependency: swofty_core::manifest::AddonManifest = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "id": "dep", "name": "Dependency", "version": "1.0.0",
        "author": "test", "compat": {"game": "f1_25"}
    }))
    .unwrap();
    job.track("root", Event::Installed(&dependency));
    assert!(queue.finish("root", Err(job.failure("root failed"))));
    assert!(matches!(&queue.jobs()[0].state, State::Failed(message)
        if message.contains("root failed") && message.contains("Dependency 1.0.0")));
    assert!(queue.retry_at(0, "root"));
    assert_eq!(queue.jobs()[0].request, "1.0.0");
    assert!(matches!(queue.jobs()[0].state, State::Queued));
    assert_eq!(queue.start_next().as_deref(), Some("root"));
    assert!(queue.finish("root", Ok(())));
    assert_eq!(queue.pending(), 0);
    assert!(matches!(queue.jobs()[0].state, State::Completed));
}

#[test]
fn retry_action_distinguishes_original_and_current_versions() {
    let old = step("test.addon", 100);
    let mut addon = old.addon.clone();
    addon.versions = vec![old.version.clone()];
    assert_eq!(retry_action(Some(&addon), &old.version), RetryAction::Retry);
    addon.versions[0].yanked = true;
    assert_eq!(retry_action(Some(&addon), &old.version), RetryAction::Unavailable);
    let mut newer = old.version.clone();
    newer.version = "2.0.0".parse().unwrap();
    addon.versions.push(newer);
    assert_eq!(
        retry_action(Some(&addon), &old.version),
        RetryAction::RequestCurrent
    );
    assert_eq!(retry_action(None, &old.version), RetryAction::Unavailable);
}

#[test]
fn queued_version_must_remain_available_and_unchanged() {
    let requested = step("test.addon", 100);
    let mut addon = requested.addon.clone();
    addon.versions = vec![requested.version.clone()];
    assert!(requested_version_available(Some(&addon), &requested.version));
    assert!(!requested_version_available(None, &requested.version));
    addon.versions[0].yanked = true;
    assert!(!requested_version_available(Some(&addon), &requested.version));
    addon.versions[0].yanked = false;
    addon.versions[0].sha256 = "changed".into();
    assert!(!requested_version_available(Some(&addon), &requested.version));
    addon.versions[0] = requested.version.clone();
    addon.versions[0].launcher = Some(">=999.0.0".parse().unwrap());
    assert!(!requested_version_available(Some(&addon), &requested.version));
}

#[test]
fn tallies_every_download_of_an_install() {
    let (dep, root) = (step("dep", 300), step("root", 100));
    let job = Job::new(100);
    job.track("root", Event::Addon(&root.addon, &root.version));
    job.track("root", Event::Stage(Stage::Downloading(100)));
    let plan = Plan {
        steps: vec![dep.clone(), root.clone()],
    };
    job.track("root", Event::Planned(&plan));
    assert_eq!(job.also(), ["dep name 1.0.0"]);
    job.track("root", Event::Addon(&dep.addon, &dep.version));
    job.track("root", Event::Stage(Stage::Downloading(200)));
    assert_eq!(job.item().as_deref(), Some("dep name"));
    let Progress::Downloading { done, size, .. } = job.progress() else {
        panic!("still downloading");
    };
    assert_eq!((done, size), (300, 400));
    assert!(
        job.progress()
            .long(job.item().as_deref())
            .starts_with("Downloading dep name · ")
    );
    job.track("root", Event::Addon(&root.addon, &root.version));
    job.track("root", Event::Stage(Stage::Installing));
    assert_eq!(job.item(), None);
    assert_eq!(job.progress().long(None), "Installing…");
}

#[test]
fn names_sizes_counts_and_hosts() {
    assert_eq!(host("https://api.f1.swofty.net"), "api.f1.swofty.net");
    assert_eq!(host("http://localhost:8471"), "localhost:8471");
    assert_eq!(format::size(512), "512 B");
    assert_eq!(format::size(1_892_759), "1.8 MB");
    assert_eq!(format::compact(999), "999");
    assert_eq!(format::compact(3412), "3.4k");
    assert_eq!(format::compact(41_000), "41k");
}

fn update(id: &str, to: &str) -> Update {
    Update {
        id: id.into(),
        name: id.into(),
        installed: semver::Version::new(1, 0, 0),
        to: serde_json::from_value(serde_json::json!({
            "version": to, "size": 1, "sha256": "", "url": ""
        }))
        .unwrap(),
    }
}

#[test]
fn only_addons_that_are_on_hold_up_play() {
    let updates = [update("on", "1.1.0"), update("off", "1.1.0")];
    let on = HashSet::from(["on"]);
    let due = due_updates(&updates, &on, &BTreeMap::new(), &BTreeMap::new());
    assert_eq!(due.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(), ["on"]);
    assert!(due_updates(&updates, &HashSet::new(), &BTreeMap::new(), &BTreeMap::new()).is_empty());
}

fn installed(manifest: &str, enabled: bool) -> AddonItem {
    AddonItem {
        manifest: swofty_core::manifest::AddonManifest::parse(manifest).unwrap(),
        enabled,
        locked: false,
        icon: None,
        track: false,
        package: false,
        source: Source::Installed,
        left_out: Vec::new(),
    }
}

#[test]
fn a_shader_only_addon_takes_updates_without_a_switch() {
    let shader = r#"{"schema_version":1,"id":"t.look","name":"Look","version":"1.0.0","author":"t",
        "category":"shaders","compat":{"game":"f1_25"},"shaders":[{"config":"look.swfx"}]}"#;
    let plain = r#"{"schema_version":1,"id":"t.plain","name":"Plain","version":"1.0.0","author":"t",
        "compat":{"game":"f1_25"}}"#;
    let addons = [
        installed(shader, false),
        installed(plain, false),
        installed(plain, true),
    ];
    assert_eq!(addons.each_ref().map(takes_updates), [true, false, true]);
    let on: HashSet<&str> = addons[..2]
        .iter()
        .filter(|a| takes_updates(a))
        .map(|a| a.manifest.id.as_str())
        .collect();
    let updates = [update("t.look", "1.1.0"), update("t.plain", "1.1.0")];
    let due = due_updates(&updates, &on, &BTreeMap::new(), &BTreeMap::new());
    assert_eq!(due.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(), ["t.look"]);
}

fn due_ids(addons: &[AddonItem], vanilla: bool, off_too: bool) -> Vec<String> {
    let updates = [update("t.on", "1.1.0"), update("t.off", "1.1.0")];
    let on = updated_addons(addons, vanilla, off_too);
    due_updates(&updates, &on, &BTreeMap::new(), &BTreeMap::new())
        .iter()
        .map(|u| u.id.clone())
        .collect()
}

fn on_and_off() -> [AddonItem; 2] {
    [
        installed(
            r#"{"schema_version":1,"id":"t.on","name":"On","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#,
            true,
        ),
        installed(
            r#"{"schema_version":1,"id":"t.off","name":"Off","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#,
            false,
        ),
    ]
}

#[test]
fn by_default_only_addons_that_are_on_are_updated() {
    let addons = on_and_off();
    let off_too = swofty_core::session::Config::default().update_off_addons;
    assert_eq!(due_ids(&addons, false, off_too), ["t.on"]);
    assert!(due_ids(&addons, true, off_too).is_empty());
}

#[test]
fn updating_turned_off_addons_offers_every_installed_addon() {
    let addons = on_and_off();
    assert_eq!(due_ids(&addons, false, true), ["t.on", "t.off"]);
    assert_eq!(due_ids(&addons, true, true), ["t.on", "t.off"]);
}

#[test]
fn a_successfully_updated_addon_is_not_reported_as_failed_during_refresh() {
    assert!(!held_failure(""));
    assert!(held_failure("network error"));
}

#[test]
fn failed_install_toast_title_names_the_addon() {
    assert_eq!(Verb::Install.failure_title("Foo", false), "Could not install Foo");
    assert_eq!(
        Verb::Update.failure_title("Foo", true),
        "Can't update Foo: missing requirements"
    );
}

#[test]
fn an_update_that_failed_at_that_version_is_skipped() {
    let updates = [update("on", "1.1.0")];
    let on = HashSet::from(["on"]);
    let mut held = BTreeMap::new();
    held.insert("on".to_owned(), (semver::Version::new(1, 1, 0), "x".to_owned()));
    assert!(due_updates(&updates, &on, &held, &BTreeMap::new()).is_empty());
    held.insert("on".to_owned(), (semver::Version::new(1, 0, 5), "x".to_owned()));
    assert_eq!(due_updates(&updates, &on, &held, &BTreeMap::new()).len(), 1);
}

#[test]
fn a_skipped_version_gives_play_back_until_the_next_one() {
    let updates = [update("on", "1.1.0"), update("other", "2.0.0")];
    let on = HashSet::from(["on", "other"]);
    let mut skipped = BTreeMap::new();
    skipped.insert("on".to_owned(), semver::Version::new(1, 1, 0));
    let due = due_updates(&updates, &on, &BTreeMap::new(), &skipped);
    assert_eq!(due.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(), ["other"]);
    let newer = [update("on", "1.2.0")];
    assert_eq!(due_updates(&newer, &on, &BTreeMap::new(), &skipped).len(), 1);
}

#[test]
fn an_import_is_kept_separate_and_a_skipped_version_is_updated_from_the_addon_browser_page() {
    use super::model::Offer;
    use super::offer::{browser_offer, standing};
    let listed = listed_addon(serde_json::json!([
        {"version": "1.1.0", "size": 9, "sha256": "", "url": ""}
    ]));
    let v = |s: &str| semver::Version::parse(s).unwrap();
    let standing = || {
        standing(
            listed.offered(&swofty_core::update::current_version()),
            Some(v("1.0.0")),
            || false,
        )
    };
    use crate::library::Source;
    assert_eq!(
        browser_offer(standing(), Some((Source::Installed, v("1.0.0"))), false),
        Offer::Update(v("1.0.0"), v("1.1.0"))
    );
    assert_eq!(
        browser_offer(standing(), Some((Source::Installed, v("1.0.0"))), true),
        Offer::UpdateHere(v("1.0.0"), v("1.1.0"))
    );
    assert_eq!(
        browser_offer(standing(), Some((Source::DbEditor, v("1.0.0"))), false),
        Offer::Update(v("1.0.0"), v("1.1.0"))
    );
    assert_eq!(
        browser_offer(standing(), Some((Source::Importer, v("1.0.0"))), false),
        Offer::Separate(v("1.0.0"))
    );
    assert_eq!(browser_offer(Offer::Install, None, false), Offer::Install);
}

fn tested_on(platforms: serde_json::Value) -> Addon {
    serde_json::from_value(serde_json::json!({
        "id": "a.b", "name": "B",
        "latest": { "version": "1.0.0", "size": 0, "sha256": "", "url": "", "platforms": platforms }
    }))
    .unwrap()
}

#[test]
fn platform_tags_name_each_listed_platform_or_not_tested() {
    use super::filters::Where;
    let tags = |platforms| Where::tags(tested_on(platforms).platforms());
    assert_eq!(tags(serde_json::json!([])), [Where::NotTested]);
    assert_eq!(tags(serde_json::json!(["steam"])), [Where::Steam]);
    assert_eq!(
        tags(serde_json::json!(["ea", "steam"])),
        [Where::Steam, Where::Ea]
    );
    assert_eq!(tags(serde_json::json!(["gog"])), [Where::NotTested]);
    assert_eq!(Where::ALL.map(Where::label), ["Steam", "EA", "Not tested"]);
}

fn listed_addon(versions: serde_json::Value) -> Addon {
    serde_json::from_value(serde_json::json!({
        "id": "twilight.bahrain-reverse", "name": "Bahrain Reverse", "author": "Twilight Sparkle",
        "publisher": {"id": "1", "name": "Twilighty Sparkle"}, "versions": versions
    }))
    .unwrap()
}

fn track_offer(addon: &Addon, installed: Option<&str>) -> super::model::Offer {
    let launcher = semver::Version::new(1, 9, 14);
    super::offer::standing(
        addon.offered(&launcher),
        installed.map(|v| semver::Version::parse(v).unwrap()),
        || false,
    )
}

#[test]
fn a_listed_track_that_is_not_installed_offers_its_download() {
    use super::model::Offer;
    use super::tracks::offers_track;
    let listed = listed_addon(serde_json::json!([
        {"version": "1.0.0", "size": 9, "sha256": "", "url": "", "launcher": ">=1.4.0"}
    ]));
    assert_eq!(track_offer(&listed, None), Offer::Install);
    assert!(offers_track(&track_offer(&listed, None)));
    assert!(!offers_track(&track_offer(&listed, Some("1.0.0"))));
    assert!(!offers_track(&track_offer(&listed, Some("0.9.0"))));
    let yanked = listed_addon(serde_json::json!([
        {"version": "1.0.0", "size": 9, "sha256": "", "url": "", "yanked": true}
    ]));
    assert_eq!(track_offer(&yanked, None), Offer::TooOld);
    assert!(!offers_track(&track_offer(&yanked, None)));
    let too_new = listed_addon(serde_json::json!([
        {"version": "1.0.0", "size": 9, "sha256": "", "url": "", "launcher": ">=9.0.0"}
    ]));
    assert!(!offers_track(&track_offer(&too_new, None)));
    for busy in [
        Offer::Queued,
        Offer::Failed,
        Offer::Blocked,
        Offer::Working(Progress::Verifying),
    ] {
        assert!(offers_track(&busy), "{busy:?}");
    }
}

#[test]
fn a_byline_names_the_author_and_the_addon_when_that_tells_tracks_apart() {
    use super::tracks::byline_text;
    use swofty_core::timetrial::boards::Byline;
    let byline = |author: Option<&str>, addon: Option<&str>| Byline {
        author: author.map(str::to_owned),
        addon: addon.map(str::to_owned),
    };
    assert_eq!(
        byline_text(&byline(Some("Twilighty Sparkle"), None)),
        "by Twilighty Sparkle"
    );
    assert_eq!(
        byline_text(&byline(Some("ashton"), Some("mod.lgsgp"))),
        "by ashton \u{b7} mod.lgsgp"
    );
    assert_eq!(byline_text(&byline(None, Some("mod.suzuka24"))), "mod.suzuka24");
    assert_eq!(
        listed_addon(serde_json::json!([])).author_name(),
        "Twilighty Sparkle"
    );
}
