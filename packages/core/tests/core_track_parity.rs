use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use serde_json::json;
use swofty_core::activation::db::{Registry, Report, TrackHost, activate, datasets, weekend_format};
use swofty_core::activation::dbx::Db;
use swofty_core::map::TrackPackage;
use swofty_formats::gvdb::{Gvdb, Record};

type Case = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static [&'static str],
    &'static [&'static str],
);

const CASES: &[Case] = &[
    ("okayama", "okayama", "melbourne", Some("melbourne"), &[], &[]),
    ("bahrain24", "bahrain24", "bahrain", None, &[], &[]),
    ("delhi", "delhi", "zandvoort", None, &[], &[]),
    (
        "pictures",
        "pictures",
        "bahrain",
        None,
        &["fe_world.tracks_spline", "tracks.flags_circle"],
        &[],
    ),
    (
        "delhi_sprint",
        "delhi",
        "zandvoort",
        None,
        &[],
        &["f1", "f1_2026"],
    ),
];

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/test/tracks/fixture")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

fn fixture() -> Gvdb {
    let text = std::fs::read_to_string(fixture_dir().join("tracks.tsv"))
        .expect("native/test/tracks/fixture/tracks.tsv");
    let records = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut parts = l.splitn(3, '\t');
            let key = u64::from_str_radix(parts.next().unwrap(), 16).unwrap();
            let tag: u8 = parts.next().unwrap().parse().unwrap();
            Record {
                key,
                tag,
                value: unescape(parts.next().unwrap_or("")).into_bytes(),
            }
        })
        .collect();
    Gvdb { records }
}

fn package(code: &str, from: &str, slot: Option<&str>, sprint: &[&str]) -> TrackPackage {
    serde_json::from_value(json!({
        "track_code": code,
        "erp": format!("{code}.erp"),
        "activation": { "copy_from_track": from, "copy_slot": slot, "sprint": sprint }
    }))
    .unwrap()
}

fn formats(db: &mut Gvdb, code: &str) -> Vec<(String, String)> {
    let reg = Registry::builtin();
    let db = Db::new(db);
    datasets(&db, &reg)
        .unwrap()
        .iter()
        .filter_map(|ds| {
            let n = ds.table.count.and_then(|c| db.count(c)).unwrap_or(0);
            let r = (0..n)
                .map(|i| swofty_core::activation::dbx::row(ds.table.base, i))
                .find(|&r| {
                    db.text(swofty_core::activation::dbx::field(r, "track"))
                        .as_deref()
                        == Some(code)
                })?;
            Some((ds.label.clone(), weekend_format(&db, r)?))
        })
        .collect()
}

fn add(db: &mut Gvdb, code: &str, from: &str, sprint: &[&str]) -> Report {
    let pkg = package(code, from, None, sprint);
    activate(
        db,
        &pkg,
        &Registry::builtin(),
        Some(&TrackHost::Natural),
        &BTreeSet::new(),
    )
    .unwrap()
}

#[test]
fn a_track_added_on_a_sprint_base_runs_normal_weekends() {
    let mut db = fixture();
    let stock = formats(&mut db, "zandvoort");
    assert!(
        stock.iter().any(|(set, f)| set == "f1_2026" && f == "sprint"),
        "zandvoort is a sprint weekend in f1_2026: {stock:?}"
    );
    let report = add(&mut db, "delhi", "zandvoort", &[]);
    let added = formats(&mut db, "delhi");
    assert_eq!(
        added.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(),
        stock.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>()
    );
    assert!(added.iter().all(|(_, f)| f == "default"), "{added:?}");
    assert_eq!(formats(&mut db, "zandvoort"), stock);
    assert!(
        report.warnings.iter().all(|w| !w.contains("sprint")),
        "{:?}",
        report.warnings
    );
    assert!(
        report.lines.iter().all(|l| !l.contains("weekend format")),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_track_runs_sprint_weekends_only_in_the_data_sets_it_names() {
    let mut db = fixture();
    let stock = formats(&mut db, "zandvoort");
    let report = add(&mut db, "delhi", "zandvoort", &["f1", "f1_2025"]);
    let added = formats(&mut db, "delhi");
    assert!(
        added.iter().any(|(set, _)| set == "f1") && added.iter().any(|(set, _)| set == "f1_2026"),
        "{added:?}"
    );
    for (set, f) in &added {
        assert_eq!(f, if set == "f1" { "sprint" } else { "default" }, "{set}");
    }
    assert_eq!(formats(&mut db, "zandvoort"), stock);
    assert!(
        report
            .lines
            .contains(&"weekend format: delhi runs sprint weekends in f1".to_string()),
        "{:?}",
        report.lines
    );
    assert_eq!(
        report.warnings,
        vec![
            "sprint f1_2025: delhi has no weekend format in that data set, so it takes its parent data set's"
        ]
    );
}

#[test]
fn a_sprint_track_added_on_itself_keeps_its_format() {
    let mut db = fixture();
    let stock = formats(&mut db, "zandvoort");
    add(&mut db, "zandvoort", "zandvoort", &[]);
    assert_eq!(formats(&mut db, "zandvoort"), stock);
}

#[test]
fn track_lowering_parity() {
    let base = fixture();
    let before: HashMap<u64, (u8, Vec<u8>)> = base
        .records
        .iter()
        .map(|r| (r.key, (r.tag, r.value.clone())))
        .collect();
    let reg = Registry::builtin();
    for (name, code, from, slot, lists, sprint) in CASES {
        let mut db = base.clone();
        let pkg = package(code, from, *slot, sprint);
        let lists: BTreeSet<String> = lists.iter().map(|l| l.to_string()).collect();
        activate(&mut db, &pkg, &reg, Some(&TrackHost::Natural), &lists).unwrap();
        let mut records: Vec<&Record> = db
            .records
            .iter()
            .filter(|r| before.get(&r.key) != Some(&(r.tag, r.value.clone())))
            .collect();
        records.sort_by_key(|r| r.key);
        let out: Vec<_> = records
            .iter()
            .map(|r| json!({ "key": format!("{:016x}", r.key), "tag": r.tag, "text": r.text() }))
            .collect();
        // Keys go in sorted order: a workspace build turns on serde_json's preserve_order (through gpui) and a
        // swofty-core build alone does not, and the text must be the same under both.
        let doc = json!({ "from": from, "records": out, "slot": slot, "track": code });
        let text = serde_json::to_string_pretty(&doc).unwrap() + "\n";
        let path = fixture_dir().join("expected").join(format!("{name}.json"));
        if std::env::var_os("SWOFTY_WRITE_PARITY").is_some() || !path.exists() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
            continue;
        }
        let have = std::fs::read_to_string(&path).unwrap();
        assert!(
            have == text,
            "{name}: the launcher's records differ from {}; rerun with SWOFTY_WRITE_PARITY=1 and run native/test/run_tracks_test.sh",
            path.display()
        );
    }
}
