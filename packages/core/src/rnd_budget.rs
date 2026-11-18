//! R&D upgrade slots: how many rows a team's upgrade tree gets from each upgrades group.
//!
//! A career builds every team's R&D tree from its group's rows: the team's own tree
//! (`upgrades.<group>.trees.<token>.<department>.upgrade_N`) plus the rows of every engine supplier
//! (`upgrades.<group>.engine_suppliers.trees.engine_supplier_upgrades_N.engine.upgrade_N`), whichever engine the team
//! uses. Each tree holds 384 instance pointers, and the game's R&D lists collect the instances of all of a model's trees
//! into one 384-slot buffer without a bound check. A group whose components run PerDriverUnique (the My Team groups)
//! builds two trees per team, so one tree may hold 192. More rows overflow Team HQ's lists.
//!
//! [`report`] tells the build which groups pass that budget. The core then loads a team's own supplier first and
//! leaves suppliers that do not fit out of that team's trees (native/src/core/rnd_suppliers_guard.cpp).

use std::collections::HashMap;

use swofty_formats::gvdb::{Gvdb, Record, instance_count_path};
use swofty_formats::hash::fnv1a64;

/// The upgrades groups of the stock game (the game modes' ComUpgrades rows name them).
pub const GROUPS: [&str; 4] = ["2022", "myteam_2025", "2026", "myteam_om_xm"];
/// Instance pointers one tree, and one R&D list over all trees of a team, holds (0xc00 / 8).
pub const SLOTS: usize = 384;
/// Rows a tree may hold when its team has two trees (PerDriverUnique).
pub const TWO_TREE_BUDGET: usize = SLOTS / 2;
const DEPARTMENTS: [&str; 5] = ["aerodynamics", "chassis", "durability", "engine", "fake"];
const TOKENS: [&str; 12] = [
    "mercedes",
    "ferrari",
    "redbull",
    "williams",
    "tororosso",
    "renault",
    "haas",
    "mclaren",
    "sauber",
    "force_india",
    "gm_cadillac",
    "user_defined",
];

/// The database's records by key, built once per report.
pub struct Index<'a>(HashMap<u64, &'a Record>);

impl<'a> Index<'a> {
    pub fn new(db: &'a Gvdb) -> Self {
        let mut at = HashMap::with_capacity(db.records.len());
        for r in &db.records {
            at.entry(r.key).or_insert(r);
        }
        Index(at)
    }

    fn count(&self, container: &str, row: &str) -> usize {
        self.0
            .get(&fnv1a64(instance_count_path(container, row)))
            .and_then(|r| r.text().trim().parse().ok())
            .unwrap_or(0)
    }

    fn rows(&self, path: &str) -> usize {
        DEPARTMENTS
            .iter()
            .map(|d| self.count(&format!("{path}.{d}"), "upgrade"))
            .sum()
    }
}

/// The rows one tree of a group gets: the largest team tree, and each supplier's rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub team: usize,
    pub suppliers: Vec<usize>,
}

impl Group {
    pub fn per_tree(&self) -> usize {
        self.team + self.suppliers.iter().sum::<usize>()
    }
}

pub fn group(db: &Index, name: &str) -> Option<Group> {
    let base = format!("upgrades.{name}");
    let team = TOKENS
        .iter()
        .map(|t| db.rows(&format!("{base}.trees.{t}")))
        .max()
        .unwrap_or(0);
    let table = format!("{base}.engine_suppliers.trees");
    let suppliers: Vec<usize> = (0..db.count(&table, "engine_supplier_upgrades"))
        .map(|i| db.rows(&format!("{table}.engine_supplier_upgrades_{i}")))
        .collect();
    (team > 0 || !suppliers.is_empty()).then(|| Group {
        name: name.to_owned(),
        team,
        suppliers,
    })
}

/// One line per group whose trees pass the two-tree budget; nothing for a stock database.
pub fn report(db: &Gvdb) -> Vec<String> {
    let db = Index::new(db);
    GROUPS
        .iter()
        .filter_map(|g| group(&db, g))
        .filter(|g| g.per_tree() > TWO_TREE_BUDGET)
        .map(|g| {
            let supplier_rows: usize = g.suppliers.iter().sum();
            let fits = if g.per_tree() > SLOTS {
                format!("past {SLOTS} even with one tree")
            } else {
                format!("past {TWO_TREE_BUDGET} when a career builds two trees per team")
            };
            format!(
                "R&D upgrades group {}: {} team + {} supplier rows ({} suppliers) = {} per tree, {fits}; the core loads a \
                 team's own engine supplier first and leaves the suppliers that do not fit out of that team's trees \
                 (about {} supplier rows fit beside a two-tree team)",
                g.name,
                g.team,
                supplier_rows,
                g.suppliers.len(),
                g.per_tree(),
                TWO_TREE_BUDGET.saturating_sub(g.team),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(records: &mut Vec<Record>, container: &str, n: usize) {
        records.push(Record::new(
            &instance_count_path(container, "upgrade"),
            0,
            n.to_string(),
        ));
    }

    /// myteam_2025 as the stock game has it: 24/23/21/17 + hub, four suppliers of 6, 8, 6, 6 rows.
    fn stock(extra: &[usize]) -> Gvdb {
        let mut r = Vec::new();
        let tree = "upgrades.myteam_2025.trees.user_defined";
        for (d, n) in DEPARTMENTS.iter().zip([24, 23, 21, 17, 1]) {
            table(&mut r, &format!("{tree}.{d}"), n);
        }
        let es = "upgrades.myteam_2025.engine_suppliers.trees";
        let all: Vec<usize> = [6, 8, 6, 6]
            .iter()
            .copied()
            .chain(extra.iter().copied())
            .collect();
        r.push(Record::new(
            &instance_count_path(es, "engine_supplier_upgrades"),
            0,
            all.len().to_string(),
        ));
        for (i, n) in all.iter().enumerate() {
            table(&mut r, &format!("{es}.engine_supplier_upgrades_{i}.engine"), *n);
        }
        Gvdb { records: r }
    }

    #[test]
    fn the_stock_group_fits_and_says_nothing() {
        let db = stock(&[]);
        let g = group(&Index::new(&db), "myteam_2025").unwrap();
        assert_eq!(g.team, 86);
        assert_eq!(g.suppliers, vec![6, 8, 6, 6]);
        assert_eq!(g.per_tree(), 112);
        assert!(report(&db).is_empty());
    }

    #[test]
    fn eighty_more_supplier_rows_still_fit_and_eighty_one_do_not() {
        assert!(report(&stock(&[8; 10])).is_empty(), "86 + 26 + 80 = 192");
        let lines = report(&stock(
            &[8; 10][..].iter().copied().chain([1]).collect::<Vec<_>>(),
        ));
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("86 team + 107 supplier rows (15 suppliers) = 193 per tree"),
            "{lines:?}"
        );
        assert!(
            lines[0].contains("past 192 when a career builds two trees"),
            "{lines:?}"
        );
        assert!(lines[0].contains("about 106 supplier rows fit"), "{lines:?}");
    }

    #[test]
    fn a_group_the_database_lacks_is_skipped() {
        assert!(group(&Index::new(&stock(&[])), "2026").is_none());
    }
}
