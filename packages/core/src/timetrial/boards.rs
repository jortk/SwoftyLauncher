//! The Time Trial boards the launcher shows: a track's board and the laps waiting for an admin.

use serde::Deserialize;
use serde_json::{Value, json};

use super::api::{Api, ApiError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Driver {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// The game's own replay file a lap carries.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GameReplay {
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub setup: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Row {
    #[serde(default)]
    pub rank: Option<u32>,
    pub lap_id: String,
    pub driver: Driver,
    pub time_ms: u64,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub game_replay: Option<GameReplay>,
}

impl Row {
    pub fn pending(&self) -> bool {
        self.status == "pending"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingLap {
    pub lap_id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub track_name: String,
    pub driver: Driver,
    pub time_ms: u64,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub reasons: Vec<String>,
}

/// One verification check of a lap, as moderators see it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Check {
    pub name: String,
    #[serde(default = "yes")]
    pub ok: bool,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub limit: Option<Value>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RunAddon {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// A lap as the moderation page lists it: any status, with what only moderators see.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Run {
    pub lap_id: String,
    pub driver: Driver,
    #[serde(default)]
    pub banned: bool,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub season: Option<String>,
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub track_name: Option<String>,
    pub time_ms: u64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub addons: Vec<RunAddon>,
    #[serde(default)]
    pub game_build: Option<String>,
    #[serde(default)]
    pub launcher: Option<String>,
    #[serde(default)]
    pub core: Option<String>,
    #[serde(default)]
    pub game_replay: Option<GameReplay>,
}

impl Run {
    pub fn pending(&self) -> bool {
        self.status == "pending"
    }

    pub fn season_name(&self) -> &str {
        self.season
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.content)
    }

    pub fn track_title(&self) -> &str {
        self.track_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.track)
    }

    /// Whether every word of `query` is in the driver, season, track, status or lap id.
    pub fn matches(&self, query: &str) -> bool {
        let hay = format!(
            "{} {} {} {} {} {} {}",
            self.driver.name,
            self.season_name(),
            self.content,
            self.track_title(),
            self.track,
            self.status,
            self.lap_id
        )
        .to_lowercase();
        query.to_lowercase().split_whitespace().all(|w| hay.contains(w))
    }
}

/// A season and track that has laps, for the moderation filters.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RunBoard {
    pub content: String,
    #[serde(default)]
    pub season: Option<String>,
    pub track: String,
    #[serde(default)]
    pub track_name: Option<String>,
    #[serde(default)]
    pub laps: u64,
}

/// How a check's value or limit reads in a table.
pub fn check_number(v: &Option<Value>) -> String {
    match v {
        None | Some(Value::Null) => "-".to_owned(),
        Some(Value::Number(n)) => match n.as_f64() {
            Some(f) if f.fract() != 0. => format!("{f:.3}"),
            _ => n.to_string(),
        },
        Some(Value::String(t)) => t.clone(),
        Some(other) => other.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TrackEntry {
    pub track: String,
    #[serde(default = "dry")]
    pub weather: String,
    #[serde(default)]
    pub laps: u32,
    #[serde(default)]
    pub best_ms: Option<u64>,
    #[serde(default, alias = "name")]
    pub track_name: Option<String>,
    #[serde(default, alias = "addon")]
    pub track_addon: Option<String>,
}

fn dry() -> String {
    "dry".to_owned()
}

impl TrackEntry {
    pub fn title(&self) -> String {
        match &self.track_name {
            Some(n) if !n.is_empty() => n.clone(),
            _ => self.track.clone(),
        }
    }

    pub fn addon(&self) -> Option<&str> {
        track_addon(&self.track, self.track_addon.as_deref())
    }
}

/// The addon a board's track comes from: the one the server names, else the `addon` of an `addon:track`
/// key. A stock track such as `bahrain` has none, whatever box its board is in.
pub fn track_addon<'a>(track: &'a str, named: Option<&'a str>) -> Option<&'a str> {
    named.map(str::trim).filter(|n| !n.is_empty()).or_else(|| {
        track
            .split_once(':')
            .map(|(addon, _)| addon.trim())
            .filter(|a| !a.is_empty())
    })
}

/// A track as a list shows it, for [`bylines`]: its name, its addon and that addon's author.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed<'a> {
    pub name: &'a str,
    pub addon: Option<&'a str>,
    pub author: Option<&'a str>,
}

/// What a listed track shows next to its name, so two tracks with the same name can be told apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Byline {
    pub author: Option<String>,
    /// The addon's id, when the author alone does not tell the track from another of the same name.
    pub addon: Option<String>,
}

/// Each track's byline. An addon track names its addon's author, and also the addon's id when another
/// addon's track has the same name and an author that is unknown or the same. A stock track has none.
pub fn bylines(tracks: &[Listed<'_>]) -> Vec<Option<Byline>> {
    let names: Vec<String> = tracks.iter().map(|t| t.name.trim().to_lowercase()).collect();
    fn author<'a>(t: &Listed<'a>) -> Option<&'a str> {
        t.author.map(str::trim).filter(|a| !a.is_empty())
    }
    tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let addon = t.addon?;
            let by = author(t);
            let needs_id = tracks
                .iter()
                .zip(&names)
                .filter(|(o, name)| o.addon != Some(addon) && **name == names[i])
                .any(|(o, _)| match (by, author(o)) {
                    (None, _) => true,
                    (Some(a), Some(b)) => o.addon.is_some() && a.to_lowercase() == b.to_lowercase(),
                    (Some(_), None) => false,
                });
            (by.is_some() || needs_id).then(|| Byline {
                author: by.map(str::to_owned),
                addon: needs_id.then(|| addon.to_owned()),
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Group {
    #[serde(alias = "id")]
    pub content: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tracks: Vec<TrackEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum Rows<T> {
    List(Vec<T>),
    Wrapped {
        #[serde(alias = "laps", alias = "entries", alias = "boards", alias = "supporters")]
        rows: Vec<T>,
        #[serde(default)]
        next_offset: Option<u64>,
    },
}

impl<T> Rows<T> {
    pub(crate) fn into_vec(self) -> Vec<T> {
        match self {
            Self::List(v) | Self::Wrapped { rows: v, .. } => v,
        }
    }
}

/// Whether the launcher shows Time Trial content and the rankings count it: the stock game's boxes
/// (`game/...`) and the SWOFTY seasons (`swofty.swofty-seasons-<year>/...`). Every other addon's box,
/// such as Classic Seasons' `swofty.classic-seasons-2020/f1_2020`, is hidden. The boards server ranks by
/// the same rule.
pub fn content_listed(content: &str) -> bool {
    let addon = content.split('/').next().unwrap_or_default().to_lowercase();
    addon == "game"
        || addon
            .strip_prefix("swofty.swofty-seasons-")
            .is_some_and(|year| year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()))
}

/// `m:ss.mmm` for a lap time.
pub fn format_time(ms: u64) -> String {
    format!("{}:{:02}.{:03}", ms / 60_000, ms / 1000 % 60, ms % 1000)
}

/// The rows whose driver name contains `query`, ignoring case; every row for a blank query.
pub fn by_driver<'a>(rows: &'a [Row], query: &str) -> Vec<&'a Row> {
    let needle = query.trim().to_lowercase();
    rows.iter()
        .filter(|r| needle.is_empty() || r.driver.name.to_lowercase().contains(&needle))
        .collect()
}

/// How many pages of a hundred runs the moderation page loads.
const RUN_PAGES: usize = 5;

fn segment(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl Api {
    pub fn catalog(&self) -> Result<Vec<Group>, ApiError> {
        let mut groups = self.get_json::<Rows<Group>>("/v1/tt/boards")?.into_vec();
        groups.retain(|g| content_listed(&g.content));
        Ok(groups)
    }

    pub fn board(
        &self,
        content: &str,
        track: &str,
        weather: &str,
        query: Option<&str>,
    ) -> Result<Vec<Row>, ApiError> {
        let mut path = format!(
            "/v1/tt/boards/{}/{}?weather={}",
            content.split('/').map(segment).collect::<Vec<_>>().join("/"),
            segment(track),
            segment(weather)
        );
        if let Some(q) = query.filter(|q| !q.is_empty()) {
            path.push_str(&format!("&q={}", segment(q)));
        }
        Ok(self.get_json::<Rows<Row>>(&path)?.into_vec())
    }

    /// Every lap waiting for review, following the server's pages.
    pub fn pending_laps(&self) -> Result<Vec<PendingLap>, ApiError> {
        let mut all = Vec::new();
        let mut offset = 0u64;
        for _ in 0..40 {
            let page: Rows<PendingLap> =
                self.get_json(&format!("/v1/tt/admin/laps?status=pending&offset={offset}"))?;
            let next = match &page {
                Rows::Wrapped { next_offset, .. } => *next_offset,
                Rows::List(_) => None,
            };
            all.extend(page.into_vec());
            match next {
                Some(n) if n > offset => offset = n,
                _ => break,
            }
        }
        all.retain(|l| content_listed(&l.content));
        Ok(all)
    }

    /// The newest laps of any status for the moderation page, narrowed to these seasons and tracks
    /// when either list is not empty.
    pub fn runs(&self, contents: &[String], tracks: &[String]) -> Result<Vec<Run>, ApiError> {
        let list = |name: &str, values: &[String]| match values.is_empty() {
            true => String::new(),
            false => format!(
                "&{name}={}",
                values.iter().map(|v| segment(v)).collect::<Vec<_>>().join(",")
            ),
        };
        let filter = format!("{}{}", list("content", contents), list("track", tracks));
        let mut all = Vec::new();
        let mut offset = 0u64;
        for _ in 0..RUN_PAGES {
            let page: Rows<Run> = self.get_json(&format!(
                "/v1/tt/admin/laps?status=all&view=runs&limit=100&offset={offset}{filter}"
            ))?;
            let next = match &page {
                Rows::Wrapped { next_offset, .. } => *next_offset,
                Rows::List(_) => None,
            };
            all.extend(page.into_vec());
            match next {
                Some(n) if n > offset => offset = n,
                _ => break,
            }
        }
        all.retain(|r| content_listed(&r.content));
        Ok(all)
    }

    /// Every season and track that has laps.
    pub fn run_boards(&self) -> Result<Vec<RunBoard>, ApiError> {
        let mut boards = self.get_json::<Rows<RunBoard>>("/v1/tt/admin/boards")?.into_vec();
        boards.retain(|b| content_listed(&b.content));
        Ok(boards)
    }

    pub fn accept_lap(&self, id: &str) -> Result<(), ApiError> {
        self.post_json(&format!("/v1/tt/admin/laps/{}/accept", segment(id)), &json!({}))
    }

    pub fn deny_lap(&self, id: &str) -> Result<(), ApiError> {
        self.post_json(&format!("/v1/tt/admin/laps/{}/deny", segment(id)), &json!({}))
    }

    pub fn ban_driver(&self, lap_id: &str, reason: &str) -> Result<(), ApiError> {
        self.post_json(
            &format!("/v1/tt/admin/laps/{}/ban", segment(lap_id)),
            &json!({ "reason": reason }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_stock_game_and_the_swofty_seasons_are_listed() {
        for listed in [
            "game/f1_2025",
            "GAME/f1_2025",
            "swofty.swofty-seasons-2015/sw_2015",
            "swofty.swofty-seasons-2024/sw_2024",
        ] {
            assert!(content_listed(listed), "{listed}");
        }
        for hidden in [
            "swofty.classic-seasons-2020/f1_2020",
            "bydanix.f1-2012-season/ex_f1_2012",
            "msyeett.classic-seasons-2015/f1_2015",
            "swofty.swofty-seasons-2021-real/sw_2021",
            "swofty.swofty-seasons-progressive/x",
            "swofty.swofty-seasons-20211/sw_2021",
            "swofty.swofty-seasons-/sw",
            "games/f1_2025",
            "mod.game/f1_2025",
            "",
        ] {
            assert!(!content_listed(hidden), "{hidden}");
        }
    }

    #[test]
    fn the_catalog_and_the_moderation_filters_leave_out_unlisted_seasons() {
        let (url, t) = super::super::api::tests::serve(
            "200 OK",
            r#"[{"content":"game/f1_2025","name":"F1 25 (stock)","tracks":[]},{"content":"swofty.classic-seasons-2020/f1_2020","name":"F1 2020 GP/TT Addon","tracks":[]},{"content":"swofty.swofty-seasons-2021/sw_2021","name":"SWOFTY 2021","tracks":[]}]"#,
        );
        let groups = Api::with_server(&url, "tok").catalog().unwrap();
        t.join().unwrap();
        assert_eq!(
            groups.iter().map(|g| g.content.as_str()).collect::<Vec<_>>(),
            ["game/f1_2025", "swofty.swofty-seasons-2021/sw_2021"]
        );
        let (url, t) = super::super::api::tests::serve(
            "200 OK",
            r#"{"boards":[{"content":"bydanix.f1-2012-season/ex_f1_2012","track":"monaco","laps":3},{"content":"game/f1_2025","track":"monaco","laps":2}]}"#,
        );
        let boards = Api::with_server(&url, "tok").run_boards().unwrap();
        t.join().unwrap();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].content, "game/f1_2025");
    }

    #[test]
    fn a_track_names_its_addon_and_a_stock_track_has_none() {
        assert_eq!(
            track_addon("twilight.bahrain-reverse:sakhir_reverse", None),
            Some("twilight.bahrain-reverse")
        );
        assert_eq!(track_addon("lisgp", Some("mod.lisgp")), Some("mod.lisgp"));
        assert_eq!(track_addon("mod.lisgp:lisgp", Some(" ")), Some("mod.lisgp"));
        for stock in ["bahrain", "silverstone_reverse", ":bahrain", ""] {
            assert_eq!(track_addon(stock, None), None, "{stock}");
        }
        let entry: TrackEntry = serde_json::from_str(
            r#"{"track":"dromodf.fuji-speedway:suzuka24","name":"Fuji Speedway","custom":true,"track_addon":"dromodf.fuji-speedway","weather":"dry","laps":4}"#,
        )
        .unwrap();
        assert_eq!(entry.addon(), Some("dromodf.fuji-speedway"));
        let stock: TrackEntry =
            serde_json::from_str(r#"{"track":"monaco","name":"Circuit de Monaco"}"#).unwrap();
        assert_eq!(stock.addon(), None);
    }

    fn listed<'a>(name: &'a str, addon: Option<&'a str>, author: Option<&'a str>) -> Listed<'a> {
        Listed { name, addon, author }
    }

    fn by(author: Option<&str>, addon: Option<&str>) -> Option<Byline> {
        Some(Byline {
            author: author.map(str::to_owned),
            addon: addon.map(str::to_owned),
        })
    }

    #[test]
    fn an_addon_track_names_its_author_and_a_stock_track_nothing() {
        let got = bylines(&[
            listed(
                "Bahrain Reverse",
                Some("twilight.bahrain-reverse"),
                Some("Twilight Sparkle"),
            ),
            listed("Bahrain International Circuit", None, None),
            listed("Okayama", Some("mod.okayama"), None),
            listed("Okayama", Some("mod.okayama"), Some(" ")),
        ]);
        assert_eq!(got, [by(Some("Twilight Sparkle"), None), None, None, None]);
    }

    #[test]
    fn tracks_with_the_same_name_are_told_apart() {
        let got = bylines(&[
            listed(
                "Fuji Speedway",
                Some("dromodf.fuji-speedway"),
                Some("[FEL] Dromodf"),
            ),
            listed("fuji speedway ", Some("mod.suzuka24"), None),
            listed("Laguna Seca", Some("kencitocco.laguna-seca"), Some("ashton")),
            listed("Laguna Seca", Some("mod.lgsgp"), Some("Ashton")),
            listed("Bahrain", None, None),
            listed("Bahrain", Some("mod.bahrain"), None),
            listed("Monza", Some("track.mza26"), Some("w7")),
            listed("Monza", None, None),
        ]);
        assert_eq!(
            got,
            [
                by(Some("[FEL] Dromodf"), None),
                by(None, Some("mod.suzuka24")),
                by(Some("ashton"), Some("kencitocco.laguna-seca")),
                by(Some("Ashton"), Some("mod.lgsgp")),
                None,
                by(None, Some("mod.bahrain")),
                by(Some("w7"), None),
                None,
            ]
        );
    }

    #[test]
    fn the_same_track_listed_twice_is_not_a_twin() {
        let got = bylines(&[
            listed("Fuji Speedway", Some("mod.suzuka24"), None),
            listed("Fuji Speedway", Some("mod.suzuka24"), None),
        ]);
        assert_eq!(got, [None, None]);
    }

    #[test]
    fn formats_a_lap_time() {
        assert_eq!(format_time(83_456), "1:23.456");
        assert_eq!(format_time(9_005), "0:09.005");
    }

    #[test]
    fn a_lap_reads_its_game_replay() {
        let with = r#"{"lap_id":"a","driver":{"id":"1","name":"X"},"time_ms":1,"game_replay":{"size":9,"setup":{"lap_ms":1}}}"#;
        let row: Row = serde_json::from_str(with).unwrap();
        assert_eq!(row.game_replay.map(|g| g.size), Some(9));
        let without = r#"{"lap_id":"a","driver":{"id":"1","name":"X"},"time_ms":1,"game_replay":null}"#;
        assert!(
            serde_json::from_str::<Row>(without)
                .unwrap()
                .game_replay
                .is_none()
        );
        assert!(serde_json::from_str::<Run>(with).unwrap().game_replay.is_some());
    }

    #[test]
    fn reads_a_bare_list_or_a_wrapped_one() {
        let bare = r#"[{"lap_id":"a","driver":{"id":"1","name":"X"},"time_ms":1,"status":"ranked"}]"#;
        let wrapped = format!(r#"{{"rows":{bare}}}"#);
        assert_eq!(
            serde_json::from_str::<Rows<Row>>(bare).unwrap().into_vec().len(),
            1
        );
        assert_eq!(
            serde_json::from_str::<Rows<Row>>(&wrapped)
                .unwrap()
                .into_vec()
                .len(),
            1
        );
    }

    #[test]
    fn search_filters_rows_by_driver_name_ignoring_case() {
        let row = |name: &str| Row {
            rank: None,
            lap_id: name.into(),
            driver: Driver {
                id: String::new(),
                name: name.into(),
                avatar: None,
            },
            time_ms: 1,
            date: String::new(),
            status: "ranked".into(),
            reasons: Vec::new(),
            game_replay: None,
        };
        let rows = vec![row("Swofty"), row("Max"), row("swofty2")];
        assert_eq!(by_driver(&rows, "").len(), 3);
        assert_eq!(by_driver(&rows, " SWOF ").len(), 2);
        assert!(by_driver(&rows, "zzz").is_empty());
    }

    #[test]
    fn path_segments_are_escaped() {
        assert_eq!(segment("a b/c"), "a%20b%2Fc");
    }

    /// The board reply as the server sends it: each row names the driver as both `driver` and `user` and
    /// the time as both `date` and `set_at`.
    #[test]
    fn a_board_reply_with_both_names_for_the_driver_and_the_date_is_read() {
        let reply = r#"{"content": "swofty.swofty-seasons-2019/sw_2019", "track": "monaco", "total": 1, "limit": 50, "offset": 0, "record": {"lap_id": "2949b7d7eaf78422", "time_ms": 71105, "user": {"id": "1", "name": "Driver", "avatar": null}}, "rows": [{"rank": 1, "lap_id": "2949b7d7eaf78422", "user": {"id": "1", "name": "Driver", "avatar": null}, "driver": {"id": "1", "name": "Driver", "avatar": null}, "status": "accepted", "date": "2026-11-10T23:41:25Z", "time_ms": 71105, "sectors_ms": [18844, 33714, 18546], "car": {"team": "sw19_mclaren"}, "assists": {"packed": 93, "any": true, "traction_control": "full", "abs": true, "braking_assist": false, "racing_line": "full", "gearbox": "automatic", "ers_assist": true, "other": false}, "input_device": "wheel", "set_at": "2026-11-10T23:41:25Z", "gap_ms": 0, "gap_pct": 0.0, "car_label": "sw19_mclaren", "input": "wheel", "verification": "unattested", "has_replay": true}], "history": []}"#;
        let rows = serde_json::from_str::<Rows<Row>>(reply).unwrap().into_vec();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].driver.name, "Driver");
        assert_eq!(rows[0].date, "2026-11-10T23:41:25Z");
        assert_eq!(rows[0].time_ms, 71105);
    }

    #[test]
    fn a_run_reads_the_moderator_fields_and_matches_a_search() {
        let reply = r#"{"total":1,"limit":100,"offset":0,"next_offset":null,"laps":[{"lap_id":"ab12","driver":{"id":"1","name":"Driver One","avatar":null},"banned":false,"content":"a.b/sw_2019","season":"SWOFTY 2019","track":"monaco","track_name":"Circuit de Monaco","weather":"dry","time_ms":71105,"reasons":[],"failed_checks":[],"status":"accepted","date":"2026-11-10T23:41:25Z","replay_url":"x","message":null,"valid":true,"note":null,"checks":[{"name":"physics.top_speed","ok":true,"value":91.25,"limit":120},{"name":"structure.board","ok":false}],"addons":[{"id":"a.b","version":"1.0.3","source":"registry"}],"game_build":"d873","launcher":"1.8.37","core":"0.2.13"}]}"#;
        let runs = serde_json::from_str::<Rows<Run>>(reply).unwrap().into_vec();
        let run = &runs[0];
        assert_eq!(
            (run.season_name(), run.track_title()),
            ("SWOFTY 2019", "Circuit de Monaco")
        );
        assert_eq!(run.checks.len(), 2);
        assert!(run.checks[0].ok && !run.checks[1].ok);
        assert_eq!(check_number(&run.checks[0].value), "91.250");
        assert_eq!(check_number(&run.checks[0].limit), "120");
        assert_eq!(check_number(&run.checks[1].value), "-");
        assert_eq!(run.addons[0].source.as_deref(), Some("registry"));
        assert!(run.matches("monaco one") && run.matches("2019 ACCEPTED") && run.matches(""));
        assert!(!run.matches("monaco two"));
    }
}
