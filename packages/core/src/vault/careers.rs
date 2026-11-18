//! The vault's career table: `careers.json` in the vault folder, one row per
//! career id, updated from the career index each snapshot stores. A career in
//! the index of one snapshot and missing from the next one is
//! `missing-since:<snapshot>`. Nothing here reads F1 25's save: the index comes
//! from the core, so a launcher with no index yet (first run, EA before the
//! core ran, core off) has no rows and never claims a career is gone.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::stdlib::careers::{Career, CareerAddon};

pub const FILE: &str = "careers.json";
pub const INDEX_DIR: &str = "index";
const MISSING: &str = "missing-since:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Present,
    MissingSince(String),
    DeletedInGame,
    Archived,
}

impl Status {
    pub fn slug(&self) -> String {
        match self {
            Self::Present => "present".to_owned(),
            Self::MissingSince(s) => format!("{MISSING}{s}"),
            Self::DeletedInGame => "deleted-in-game".to_owned(),
            Self::Archived => "archived".to_owned(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "deleted-in-game" => Self::DeletedInGame,
            "archived" => Self::Archived,
            _ => match s.strip_prefix(MISSING) {
                Some(id) => Self::MissingSince(id.to_owned()),
                None => Self::Present,
            },
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Self::MissingSince(_))
    }
}

impl Serialize for Status {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.slug())
    }
}

impl<'de> Deserialize<'de> for Status {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::parse(&String::deserialize(d)?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub career: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub addons: Vec<CareerAddon>,
    pub first_seen: String,
    pub first_seen_at: i64,
    /// The newest snapshot whose index held the career: the one to restore it from.
    pub last_seen: String,
    pub last_seen_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<i64>,
    pub status: Status,
    /// When a check found the career missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_at: Option<i64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dismissed: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Row {
    pub fn name(&self) -> &str {
        match self.label.is_empty() {
            true => &self.career,
            false => &self.label,
        }
    }

    pub fn needs(&self) -> impl Iterator<Item = &str> {
        self.addons.iter().map(|a| a.id.as_str())
    }

    pub fn place(&self) -> Option<String> {
        match (self.season, self.round) {
            (Some(s), Some(r)) => Some(format!("season {s}, round {r}")),
            (Some(s), None) => Some(format!("season {s}")),
            _ => None,
        }
    }
}

fn text(c: &Career, key: &str) -> Option<String> {
    c.field(key).map(str::to_owned)
}

fn number(c: &Career, key: &str) -> Option<i64> {
    c.extra.get(key).and_then(|v| v.as_i64())
}

fn deleted(c: &Career) -> bool {
    c.status() == "deleted"
}

/// Careers an index says are in the save.
pub fn in_save(index: &[Career]) -> impl Iterator<Item = &Career> {
    index
        .iter()
        .filter(|c| !matches!(c.status(), "missing" | "deleted"))
}

/// Where a career stands against the game's own list of save slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Present,
    /// The profile load did not list the slot and the core cannot say why.
    Missing,
    /// The game listed the slot earlier in the same session and no longer does.
    DeletedInGame,
}

impl State {
    pub fn parse(status: &str) -> Self {
        match status {
            "missing" => Self::Missing,
            "deleted" => Self::DeletedInGame,
            _ => Self::Present,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Present => "in the game",
            Self::Missing => "missing from the game",
            Self::DeletedInGame => "deleted in game",
        }
    }
}

/// One career of a profile as the Career saves page lists it: the core's career index record
/// with the game's slot details (`SaveSlotInfo`, what the Resume Career screen reads).
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub career: String,
    pub state: State,
    pub mode: Option<String>,
    pub label: String,
    pub driver: Option<String>,
    pub team: Option<String>,
    /// The raw `info_driver` and `info_team` values of the slot, which the core could read
    /// without a name for them.
    pub driver_id: Option<i64>,
    pub team_id: Option<i64>,
    /// The season number the game shows, and the round index of the last save.
    pub season_no: Option<i64>,
    pub weekend: Option<i64>,
    /// Unix seconds: the slot's `info_date`, else the core's last save of the career.
    pub last_played: Option<i64>,
    pub slot_name: Option<String>,
    /// The game keeps the slot out of its Resume Career list.
    pub hidden: bool,
    pub profile: Option<String>,
}

impl Listed {
    /// The game's "none" value for a driver or team id.
    pub const NONE_ID: i64 = 0x3fff;

    fn no_entrants(&self) -> bool {
        let none = |id: Option<i64>| id.is_none_or(|i| i & Self::NONE_ID == Self::NONE_ID);
        self.driver.is_none()
            && self.team.is_none()
            && (self.driver_id.is_some() || self.team_id.is_some())
            && none(self.driver_id)
            && none(self.team_id)
    }

    pub fn mode_label(&self) -> String {
        match self.mode.as_deref().filter(|m| !m.is_empty()) {
            Some(m) => mode_label(Some(m)),
            None if self.career.contains("my_team") => "My Team".to_owned(),
            None => "Driver Career".to_owned(),
        }
    }

    pub fn title(&self) -> String {
        let who = [self.driver.as_deref(), self.team.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if !who.is_empty() {
            return who.join(", ");
        }
        if self.no_entrants() {
            return "New career".to_owned();
        }
        if let Some(n) = self
            .slot_name
            .as_deref()
            .filter(|n| !n.is_empty() && !n.contains(['_', ':']))
        {
            return n.to_owned();
        }
        let season = year_in(&self.label)
            .or_else(|| year_in(&self.career))
            .map(|y| format!("{y} season"))
            .or_else(|| self.season_no.map(|s| format!("season {s}")));
        match season {
            Some(s) => format!("{} · {s}", self.mode_label()),
            None => self.mode_label(),
        }
    }

    pub fn place(&self) -> Option<String> {
        self.season_no.map(|s| format!("season {s}"))
    }
}

fn year_in(text: &str) -> Option<u32> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|t| t.len() == 4)
        .filter_map(|t| t.parse::<u32>().ok())
        .find(|y| (1950..=2100).contains(y))
}

/// The name the game's mode key goes by: `driver_career` is Driver Career, `my_team` is My Team.
pub fn mode_label(mode: Option<&str>) -> String {
    let raw = mode.unwrap_or("").trim();
    let key: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    match key.as_str() {
        "" => "Career".to_owned(),
        "driver" | "drivercareer" => "Driver Career".to_owned(),
        "myteam" => "My Team".to_owned(),
        "classic" | "classiccareer" => "Classic Career".to_owned(),
        "progressive" => "Progressive".to_owned(),
        "1990" | "f11990" => "1990".to_owned(),
        _ => raw
            .split(['_', '-', ' '])
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn listed(c: &Career) -> Listed {
    Listed {
        career: c.career.clone(),
        state: State::parse(c.status()),
        mode: text(c, "mode"),
        label: c.label.clone(),
        driver: text(c, "driver"),
        team: text(c, "team"),
        driver_id: number(c, "driver_raw"),
        team_id: number(c, "team_raw"),
        season_no: number(c, "slot_season"),
        weekend: number(c, "slot_weekend"),
        last_played: number(c, "last_played").or(number(c, "last_saved").map(|ms| ms / 1000)),
        slot_name: text(c, "slot_name"),
        hidden: c.extra.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false),
        profile: c.profile.clone(),
    }
}

/// Every career the core's index holds for `profile` (and the ones with no profile yet), with its
/// state: the ones in the game's save slots first, the newest played first. Only careers with a save
/// slot are listed: a record a career load made before the career had a slot is not a career in the save.
pub fn listing(index: &[Career], profile: Option<&str>) -> Vec<Listed> {
    let mut out: Vec<Listed> = index
        .iter()
        .filter(|c| {
            c.extra
                .get("slot")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        })
        .filter(|c| match (profile, c.profile.as_deref()) {
            (Some(p), Some(q)) => p == q,
            _ => true,
        })
        .map(listed)
        .collect();
    out.sort_by(|a, b| {
        (a.state, std::cmp::Reverse(a.last_played), &a.career).cmp(&(
            b.state,
            std::cmp::Reverse(b.last_played),
            &b.career,
        ))
    });
    out
}

/// `listing` over the index file in the launcher's data folder.
pub fn read_listing(data: &std::path::Path, profile: Option<&str>) -> Vec<Listed> {
    listing(
        &crate::stdlib::careers::read(&crate::stdlib::careers::data_path(data)),
        profile,
    )
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Careers {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub careers: Vec<Row>,
}

fn one() -> u32 {
    1
}

impl Careers {
    pub fn read(vault: &std::path::Path) -> Self {
        std::fs::read(vault.join(FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn write(&self, vault: &std::path::Path) -> crate::Result<()> {
        crate::write_json_shared(&vault.join(FILE), self)
    }

    pub fn find(&self, id: &str) -> Option<&Row> {
        self.careers.iter().find(|r| r.career == id)
    }

    pub fn missing(&self) -> impl Iterator<Item = &Row> {
        self.careers.iter().filter(|r| r.status.is_missing())
    }

    pub fn drop_to_show(&self) -> Vec<&Row> {
        self.missing().filter(|r| !r.dismissed).collect()
    }

    pub fn dismiss(&mut self) -> bool {
        let mut any = false;
        for r in self
            .careers
            .iter_mut()
            .filter(|r| r.status.is_missing() && !r.dismissed)
        {
            r.dismissed = true;
            any = true;
        }
        any
    }

    pub fn archive(&mut self, id: &str) -> bool {
        match self.careers.iter_mut().find(|r| r.career == id) {
            Some(r) => {
                r.status = Status::Archived;
                true
            }
            None => false,
        }
    }

    /// Fold the index stored with snapshot `snapshot` (taken at `at`) into the
    /// rows. Returns the careers newly found missing, as `(career, the snapshot
    /// that last held it)`.
    pub fn observe(&mut self, snapshot: &str, at: i64, index: &[Career]) -> Vec<(String, String)> {
        let mut dropped = Vec::new();
        let here: BTreeSet<&str> = in_save(index).map(|c| c.career.as_str()).collect();
        let gone: BTreeSet<&str> = index
            .iter()
            .filter(|c| deleted(c))
            .map(|c| c.career.as_str())
            .collect();
        for c in in_save(index) {
            let fresh = |r: &mut Row| {
                r.mode = text(c, "mode").or(r.mode.take());
                if !c.label.is_empty() {
                    r.label = c.label.clone();
                }
                r.driver = text(c, "driver").or(r.driver.take());
                r.team = text(c, "team").or(r.team.take());
                r.season = number(c, "season").or(r.season);
                r.round = number(c, "round").or(r.round);
                r.profile = c.profile.clone().or(r.profile.take());
                r.last_played = number(c, "last_saved").map(|ms| ms / 1000).or(r.last_played);
                r.addons = c.addons.clone();
                r.last_seen = snapshot.to_owned();
                r.last_seen_at = at;
            };
            match self.careers.iter_mut().find(|r| r.career == c.career) {
                Some(r) => {
                    fresh(r);
                    if r.status != Status::Archived {
                        if r.status != Status::Present {
                            r.found_at = None;
                            r.dismissed = false;
                        }
                        r.status = Status::Present;
                    }
                }
                None => {
                    let mut r = Row {
                        career: c.career.clone(),
                        mode: None,
                        label: String::new(),
                        driver: None,
                        team: None,
                        season: None,
                        round: None,
                        profile: None,
                        addons: Vec::new(),
                        first_seen: snapshot.to_owned(),
                        first_seen_at: at,
                        last_seen: snapshot.to_owned(),
                        last_seen_at: at,
                        last_played: None,
                        status: Status::Present,
                        found_at: None,
                        dismissed: false,
                    };
                    fresh(&mut r);
                    self.careers.push(r);
                }
            }
        }
        let twins: Vec<&Career> = in_save(index).collect();
        for r in self.careers.iter_mut() {
            if here.contains(r.career.as_str()) {
                continue;
            }
            if twins.iter().any(|c| twin(r, c)) {
                if matches!(r.status, Status::MissingSince(_)) {
                    r.status = Status::Present;
                    r.found_at = None;
                    r.dismissed = false;
                }
                continue;
            }
            match (&r.status, gone.contains(r.career.as_str())) {
                (Status::Archived | Status::DeletedInGame, _) => {}
                (Status::Present, true) => r.status = Status::DeletedInGame,
                (Status::Present, false) => {
                    r.status = Status::MissingSince(snapshot.to_owned());
                    r.found_at = Some(at);
                    r.dismissed = false;
                    dropped.push((r.career.clone(), r.last_seen.clone()));
                }
                (Status::MissingSince(_), true) => r.status = Status::DeletedInGame,
                (Status::MissingSince(_), false) => {}
            }
        }
        dropped
    }
}

/// A row and an index career that are one career under two ids: the core
/// names a career it cannot tie to a save slot by a key that can change
/// between loads.
fn twin(r: &Row, c: &Career) -> bool {
    r.career != c.career
        && r.mode.is_some()
        && r.mode == text(c, "mode")
        && !r.label.is_empty()
        && r.label == c.label
        && r.profile == c.profile
}

/// A career as the restore panel lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brief {
    pub career: String,
    pub name: String,
    pub place: Option<String>,
}

fn brief(c: &Career) -> Brief {
    let place = match (number(c, "season"), number(c, "round")) {
        (Some(s), Some(r)) => Some(format!("season {s}, round {r}")),
        (Some(s), None) => Some(format!("season {s}")),
        _ => None,
    };
    Brief {
        career: c.career.clone(),
        name: match c.label.is_empty() {
            true => c.career.clone(),
            false => c.label.clone(),
        },
        place,
    }
}

/// Whether a career belongs to an addon's game mode, or was made with addons: the careers a game
/// without those addons does not know how to keep.
pub fn is_addon(c: &Career) -> bool {
    let key: String = text(c, "mode")
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    !c.addons.is_empty() || !matches!(key.as_str(), "" | "driver" | "drivercareer" | "myteam")
}

/// The addon careers an index holds in the save for `profile` (and the ones with no profile yet).
pub fn addon_careers(index: &[Career], profile: Option<&str>) -> Vec<Brief> {
    in_save(index)
        .filter(|c| is_addon(c))
        .filter(|c| match (profile, c.profile.as_deref()) {
            (Some(p), Some(q)) => p == q,
            _ => true,
        })
        .map(brief)
        .collect()
}

/// What restoring a version does to the careers, from two indexes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    /// In the version, not in the save now.
    pub get_back: Vec<Brief>,
    /// In the save now, not in the version.
    pub lose: Vec<Brief>,
    /// In the save now and listed in the version as missing or deleted: restoring it does not bring them back.
    pub marked_missing: Vec<Brief>,
    /// In both, with further progress now than in the version: `(now, in the version)`.
    pub progress: Vec<(Brief, Brief)>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.get_back.is_empty()
            && self.lose.is_empty()
            && self.marked_missing.is_empty()
            && self.progress.is_empty()
    }
}

fn pos(c: &Career) -> (i64, i64) {
    (number(c, "season").unwrap_or(0), number(c, "round").unwrap_or(0))
}

/// `now` is the index of the save as it is; `version` the index stored with
/// the snapshot to restore.
pub fn diff(now: &[Career], version: &[Career]) -> Diff {
    let have: BTreeMap<&str, &Career> = in_save(now).map(|c| (c.career.as_str(), c)).collect();
    let then: BTreeMap<&str, &Career> = in_save(version).map(|c| (c.career.as_str(), c)).collect();
    let mut out = Diff::default();
    for (id, c) in &then {
        match have.get(id) {
            None => out.get_back.push(brief(c)),
            Some(n) if pos(n) > pos(c) => out.progress.push((brief(n), brief(c))),
            Some(_) => {}
        }
    }
    let flagged: BTreeSet<&str> = marked_missing(version)
        .iter()
        .map(|c| c.career.as_str())
        .collect();
    for (id, c) in &have {
        if !then.contains_key(id) {
            out.lose.push(brief(c));
            if flagged.contains(id) {
                out.marked_missing.push(brief(c));
            }
        }
    }
    out
}

/// The careers with a save slot that an index lists as missing or deleted.
pub fn marked_missing(index: &[Career]) -> Vec<&Career> {
    index
        .iter()
        .filter(|c| matches!(c.status(), "missing" | "deleted"))
        .filter(|c| {
            c.extra
                .get("slot")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        })
        .collect()
}

/// `marked_missing` as the restore panel lists them.
pub fn missing_briefs(index: &[Career]) -> Vec<Brief> {
    marked_missing(index).into_iter().map(brief).collect()
}

/// The career an index holds in the save that was played last.
pub fn newest(index: &[Career]) -> Option<Brief> {
    in_save(index)
        .max_by_key(|c| number(c, "last_played").or(number(c, "last_saved").map(|ms| ms / 1000)))
        .map(brief)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::careers::parse;

    fn index() -> Vec<Career> {
        parse(
            r#"{"version":1,"careers":[
            {"career":"driver_career:a1","profile":"p","slot":"a1","status":"present","mode":"driver_career","driver":"Oscar PIASTRI","team":"McLaren","driver_raw":5,"team_raw":7,"slot_season":3,"slot_weekend":4,"last_played":1761600000,"slot_name":"Slot 1"},
            {"career":"driver_career:b2","profile":"p","slot":"b2","status":"deleted","last_played":1761700000},
            {"career":"my_team:c3","slot":"c3","status":"missing","hidden":true,"last_saved":1761800000000},
            {"career":"driver_career:d4","profile":"q","slot":"d4","status":"present"},
            {"career":"driver_career:e5","profile":"p","slot":"e5","last_played":1761900000},
            {"career":"dc-00320000","profile":"p","label":"f1_career_2026","content":"f1_career_2026","mode":"driver_career","status":"missing","last_saved":1761950000000}
            ]}"#,
        )
    }

    #[test]
    fn lists_every_career_of_the_profile_with_its_state() {
        let l = listing(&index(), Some("p"));
        let ids: Vec<_> = l.iter().map(|c| c.career.as_str()).collect();
        assert_eq!(
            ids,
            [
                "driver_career:e5",
                "driver_career:a1",
                "my_team:c3",
                "driver_career:b2"
            ]
        );
        let by = |id: &str| l.iter().find(|c| c.career == id).unwrap();
        assert_eq!(by("driver_career:a1").state, State::Present);
        assert_eq!(by("my_team:c3").state, State::Missing);
        assert_eq!(by("driver_career:b2").state, State::DeletedInGame);
        assert_eq!(by("driver_career:e5").state, State::Present);
    }

    #[test]
    fn a_career_record_with_no_save_slot_is_not_listed() {
        assert!(
            !listing(&index(), Some("p"))
                .iter()
                .any(|c| c.career == "dc-00320000")
        );
    }

    #[test]
    fn a_career_with_no_profile_yet_is_in_every_profiles_list() {
        let l = listing(&index(), Some("q"));
        assert!(l.iter().any(|c| c.career == "my_team:c3"));
        assert!(l.iter().any(|c| c.career == "driver_career:d4"));
        assert!(!l.iter().any(|c| c.career == "driver_career:a1"));
        assert_eq!(listing(&index(), None).len(), 5);
    }

    #[test]
    fn reads_the_slot_details() {
        let l = listing(&index(), Some("p"));
        let a = l.iter().find(|c| c.career == "driver_career:a1").unwrap();
        assert_eq!(a.title(), "Oscar PIASTRI, McLaren");
        assert_eq!(a.place().as_deref(), Some("season 3"));
        assert_eq!(a.last_played, Some(1_761_600_000));
        assert_eq!((a.driver_id, a.team_id, a.weekend), (Some(5), Some(7), Some(4)));
        let c = l.iter().find(|c| c.career == "my_team:c3").unwrap();
        assert!(c.hidden);
        assert_eq!(c.last_played, Some(1_761_800_000));
        assert_eq!(c.title(), "My Team");
        let b = l.iter().find(|c| c.career == "driver_career:b2").unwrap();
        assert_eq!(b.title(), "Driver Career");
        assert!(!b.hidden);
    }

    #[test]
    fn reads_the_index_file_in_the_data_folder() {
        let dir = std::env::temp_dir().join(format!("careers-listing-{}", std::process::id()));
        let file = crate::stdlib::careers::data_path(&dir);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            r#"{"careers":[{"career":"x:1","slot":"1","status":"deleted"}]}"#,
        )
        .unwrap();
        let l = read_listing(&dir, None);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].state, State::DeletedInGame);
        assert!(read_listing(&dir, None).is_empty());
    }

    fn mt(id: &str, status: &str) -> String {
        format!(
            r#"{{"career":"{id}","profile":"MyTeam","label":"f1_career_2026","mode":"my_team","status":"{status}"}}"#
        )
    }

    fn table(rows: &[String]) -> Vec<Career> {
        parse(&format!(r#"{{"version":1,"careers":[{}]}}"#, rows.join(",")))
    }

    #[test]
    fn a_career_that_alternates_between_two_ids_is_not_missing() {
        let mut t = Careers::default();
        let a = "mt-00000001-00d00000";
        let b = "mt-00000001-00bc0000";
        assert!(t.observe("s1", 1, &table(&[mt(a, "present")])).is_empty());
        assert!(
            t.observe("s2", 2, &table(&[mt(a, "missing"), mt(b, "present")]))
                .is_empty()
        );
        assert!(
            t.observe("s3", 3, &table(&[mt(a, "present"), mt(b, "missing")]))
                .is_empty()
        );
        assert!(t.careers.iter().all(|r| r.status == Status::Present));
        assert!(t.drop_to_show().is_empty());
    }

    #[test]
    fn a_stale_missing_row_of_a_twin_is_cleared() {
        let mut t = Careers::default();
        t.observe("s1", 1, &table(&[mt("mt-1-a", "present")]));
        t.careers[0].status = Status::MissingSince("s0".into());
        t.careers[0].found_at = Some(1);
        t.observe("s2", 2, &table(&[mt("mt-1-b", "present")]));
        assert_eq!(t.find("mt-1-a").unwrap().status, Status::Present);
        assert!(t.drop_to_show().is_empty());
    }

    #[test]
    fn a_career_with_no_twin_is_still_reported_missing() {
        let mut t = Careers::default();
        let other = r#"{"career":"dc-00320000","profile":"MyTeam","label":"f1_career_2026","mode":"driver_career","status":"present"}"#;
        t.observe("s1", 1, &table(&[mt("mt-1-a", "present")]));
        let dropped = t.observe("s2", 2, &table(&[other.to_owned()]));
        assert_eq!(dropped, [("mt-1-a".to_owned(), "s1".to_owned())]);
        assert!(t.find("mt-1-a").unwrap().status.is_missing());
    }

    #[test]
    fn diff_reports_the_careers_a_version_lists_as_missing() {
        let now = parse(
            r#"{"careers":[
            {"career":"a","slot":"a","status":"present"},
            {"career":"b","slot":"b","status":"present"},
            {"career":"c","slot":"c","status":"present"}]}"#,
        );
        let version = parse(
            r#"{"careers":[
            {"career":"a","slot":"a","status":"present"},
            {"career":"b","slot":"b","status":"missing"},
            {"career":"c","slot":"c","status":"deleted"}]}"#,
        );
        let d = diff(&now, &version);
        let ids = |l: &[Brief]| l.iter().map(|b| b.career.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&d.lose), ["b", "c"]);
        assert_eq!(ids(&d.marked_missing), ["b", "c"]);
        assert!(!d.is_empty());
    }

    #[test]
    fn a_career_absent_from_the_version_is_lost_but_not_marked_missing() {
        let now = parse(r#"{"careers":[{"career":"a","slot":"a","status":"present"}]}"#);
        let d = diff(&now, &[]);
        assert_eq!(d.lose.len(), 1);
        assert!(d.marked_missing.is_empty());
    }

    #[test]
    fn marked_missing_ignores_records_without_a_slot() {
        let idx = index();
        let found = marked_missing(&idx);
        let ids: Vec<_> = found.iter().map(|c| c.career.as_str()).collect();
        assert_eq!(ids, ["driver_career:b2", "my_team:c3"]);
    }

    #[test]
    fn newest_is_the_last_played_career_in_the_save() {
        assert_eq!(newest(&index()).unwrap().career, "driver_career:e5");
        assert!(newest(&[]).is_none());
    }
}
