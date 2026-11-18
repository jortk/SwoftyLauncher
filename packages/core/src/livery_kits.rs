//! The team kit of a special livery. At start-up the game reads every row of
//! `customisation.custom_se_liveries.se_liveries`, finds the
//! `entrants.team_customisation` row labelled with the row's
//! `team_customisation`, and reads that row's `official_driver_customisations`
//! with no check for a kit it did not find
//! (`Sess_Apply_ContentAvailability`, `0x1413e977d` to `0x1413e9797`). It
//! skips a row whose `team_customisation` is empty (`0x1413e9764`).

use std::collections::{BTreeSet, HashMap, HashSet};

use swofty_formats::gvdb::Gvdb;
use swofty_formats::hash::fnv1a64;

pub const TABLE: &str = "customisation.custom_se_liveries.se_liveries";
const ROW: &str = "se_livery";
const FIELD: &str = "team_customisation";
pub const KIT_YEARS: &str = "entrants.team_customisation";
const KIT_YEAR: &str = "team_customisation_for_year";
const KIT: &str = "team_customisation";

/// A special livery row whose `team_customisation` labels no team kit row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kitless {
    /// The `team_customisation` field's key, for who wrote it.
    pub key: u64,
    pub row: String,
    pub kit: String,
}

impl Kitless {
    pub fn line(&self, by: &str) -> String {
        format!(
            "{by}: special livery {} has team_customisation {}, and {KIT_YEARS} has no row with that label; the \
             value is left empty (the game crashes at start-up on a team kit it cannot find)",
            self.row, self.kit
        )
    }
}

/// The labels of the team kit rows of every year.
pub fn kits(db: &Gvdb) -> BTreeSet<String> {
    let mut want: HashSet<u64> = HashSet::new();
    for y in 0..db.instance_count(KIT_YEARS, KIT_YEAR).unwrap_or(0) {
        let year = format!("{KIT_YEARS}.{KIT_YEAR}_{y}");
        for i in 0..db.instance_count(&year, KIT).unwrap_or(0) {
            want.insert(fnv1a64(format!("{year}.{KIT}_{i}._metadata_")));
        }
    }
    db.records
        .iter()
        .filter(|r| want.contains(&r.key))
        .map(|r| r.text().into_owned())
        .collect()
}

/// Empties the `team_customisation` of every special livery row of `db` that
/// labels no team kit row. A database without team kit rows is left as it is.
pub fn keep_kits(db: &mut Gvdb) -> Vec<Kitless> {
    let kits = kits(db);
    if kits.is_empty() {
        return Vec::new();
    }
    let fields: HashMap<u64, usize> = (0..db.instance_count(TABLE, ROW).unwrap_or(0))
        .map(|i| (fnv1a64(format!("{TABLE}.{ROW}_{i}.{FIELD}")), i))
        .collect();
    let mut found: Vec<(usize, u64, String)> = Vec::new();
    for r in &mut db.records {
        let Some(&i) = fields.get(&r.key) else { continue };
        let kit = r.text().into_owned();
        if kit.is_empty() || kits.contains(&kit) {
            continue;
        }
        r.value.clear();
        found.push((i, r.key, kit));
    }
    found.sort();
    found
        .into_iter()
        .map(|(i, key, kit)| Kitless {
            key,
            row: db
                .get_text(&format!("{TABLE}.{ROW}_{i}._metadata_"))
                .unwrap_or_else(|| format!("{ROW}_{i}")),
            kit,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::gvdb::Record;

    fn db(liveries: &[(&str, &str)]) -> Gvdb {
        let mut records = vec![
            Record::new(&format!("{KIT_YEARS}._{KIT_YEAR}_instancecount_"), 0, "2"),
            Record::new(&format!("{KIT_YEARS}.{KIT_YEAR}_0._{KIT}_instancecount_"), 0, "1"),
            Record::new(
                &format!("{KIT_YEARS}.{KIT_YEAR}_0.{KIT}_0._metadata_"),
                2,
                "2025-renault",
            ),
            Record::new(&format!("{KIT_YEARS}.{KIT_YEAR}_1._{KIT}_instancecount_"), 0, "2"),
            Record::new(
                &format!("{KIT_YEARS}.{KIT_YEAR}_1.{KIT}_0._metadata_"),
                2,
                "2026-renault",
            ),
            Record::new(
                &format!("{KIT_YEARS}.{KIT_YEAR}_1.{KIT}_1._metadata_"),
                2,
                "2026-mclaren",
            ),
            // Above the year's count: the game does not read it.
            Record::new(
                &format!("{KIT_YEARS}.{KIT_YEAR}_1.{KIT}_2._metadata_"),
                2,
                "2026-lotus",
            ),
            Record::new(
                &format!("{TABLE}._{ROW}_instancecount_"),
                0,
                liveries.len().to_string(),
            ),
        ];
        for (i, (label, kit)) in liveries.iter().enumerate() {
            records.push(Record::new(&format!("{TABLE}.{ROW}_{i}._metadata_"), 2, *label));
            records.push(Record::new(&format!("{TABLE}.{ROW}_{i}.{FIELD}"), 4, *kit));
        }
        Gvdb { records }
    }

    fn kit_of(db: &Gvdb, i: usize) -> Option<String> {
        db.get_text(&format!("{TABLE}.{ROW}_{i}.{FIELD}"))
    }

    #[test]
    fn a_special_livery_whose_team_kit_has_no_row_gets_an_empty_one() {
        let mut db = db(&[
            ("mclaren_monaco_2026", "2026-mclaren"),
            ("lotus_lx02", "2026-lotus"),
            ("lotus_25", "2025-renault"),
            ("core_special", ""),
        ]);
        let before = db.records.len();
        let found = keep_kits(&mut db);
        assert_eq!(
            found,
            [Kitless {
                key: fnv1a64(format!("{TABLE}.{ROW}_1.{FIELD}")),
                row: "lotus_lx02".into(),
                kit: "2026-lotus".into(),
            }]
        );
        assert_eq!(kit_of(&db, 0).as_deref(), Some("2026-mclaren"));
        assert_eq!(kit_of(&db, 1).as_deref(), Some(""));
        assert_eq!(db.get(&format!("{TABLE}.{ROW}_1.{FIELD}")).unwrap().tag, 4);
        assert_eq!(kit_of(&db, 2).as_deref(), Some("2025-renault"));
        assert_eq!(kit_of(&db, 3).as_deref(), Some(""));
        assert_eq!(db.records.len(), before);
        assert_eq!(
            found[0].line("More Liveries"),
            "More Liveries: special livery lotus_lx02 has team_customisation 2026-lotus, and \
             entrants.team_customisation has no row with that label; the value is left empty (the game crashes at \
             start-up on a team kit it cannot find)"
        );
        assert!(keep_kits(&mut db).is_empty());
    }

    #[test]
    fn rows_above_the_count_and_a_database_without_team_kits_are_left_alone() {
        let mut above = db(&[("mclaren_monaco_2026", "2026-mclaren")]);
        above
            .records
            .push(Record::new(&format!("{TABLE}.{ROW}_1.{FIELD}"), 4, "2026-lotus"));
        assert!(keep_kits(&mut above).is_empty());
        assert_eq!(kit_of(&above, 1).as_deref(), Some("2026-lotus"));

        let mut bare = Gvdb {
            records: vec![
                Record::new(&format!("{TABLE}._{ROW}_instancecount_"), 0, "1"),
                Record::new(&format!("{TABLE}.{ROW}_0.{FIELD}"), 4, "2026-lotus"),
            ],
        };
        assert!(keep_kits(&mut bare).is_empty());
        assert_eq!(kit_of(&bare, 0).as_deref(), Some("2026-lotus"));
    }
}
