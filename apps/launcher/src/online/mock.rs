use super::api::BoardsApi;
use super::model::*;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Full,
    Empty,
    Failing,
    Slow,
}

impl Mode {
    pub fn from_env() -> Option<Self> {
        match std::env::var("SWOFTY_ONLINE_MOCK").ok()?.as_str() {
            "empty" => Some(Self::Empty),
            "error" => Some(Self::Failing),
            "slow" => Some(Self::Slow),
            _ => Some(Self::Full),
        }
    }
}

pub struct MockBoards {
    pub mode: Mode,
}

const NAMES: [&str; 14] = [
    "swofty",
    "Tommaso23",
    "driver1",
    "apex_hunter",
    "kerbs",
    "lateBraker",
    "slipstream",
    "ersTom",
    "pitwall",
    "Nyx",
    "oversteer",
    "darkMode",
    "hairpin",
    "chicane",
];
const YEARS: [&str; 7] = ["2016", "2017", "2018", "2019", "2020", "2021", "2022"];
const TRACKS: [(&str, &str, &str); 8] = [
    ("bahrain", "Bahrain", "bh"),
    ("melbourne", "Melbourne", "au"),
    ("suzuka", "Suzuka", "jp"),
    ("monza", "Monza", "it"),
    ("spa", "Spa-Francorchamps", "be"),
    ("silverstone", "Silverstone", "gb"),
    ("interlagos", "Interlagos", "br"),
    ("baku", "Baku", "az"),
];

fn label(year: &str) -> String {
    ["SWOFTY ", year].concat()
}

fn mock_setup(d: usize, s: usize) -> Setup {
    let n = |k: usize, lo: u64, span: u64| (lo + seed(d * 7 + s, k) % span) as f32;
    Setup {
        values: vec![
            ("front_wing", n(1, 15, 20)),
            ("rear_wing", n(2, 20, 20)),
            ("diff_on_throttle", n(3, 50, 40)),
            ("diff_off_throttle", n(4, 40, 40)),
            ("engine_braking", n(5, 30, 60)),
            ("front_camber", -3.5 + n(6, 0, 5) * 0.1),
            ("rear_camber", -1.8 + n(7, 0, 5) * 0.1),
            ("front_toe", 0.05 + n(8, 0, 5) * 0.01),
            ("rear_toe", 0.2 + n(9, 0, 5) * 0.01),
            ("front_suspension", n(10, 10, 12)),
            ("rear_suspension", n(11, 10, 12)),
            ("front_anti_roll_bar", n(12, 5, 10)),
            ("rear_anti_roll_bar", n(13, 5, 10)),
            ("front_ride_height", n(14, 20, 20)),
            ("rear_ride_height", n(15, 40, 20)),
            ("brake_pressure", n(16, 90, 10)),
            ("brake_bias", n(17, 50, 10)),
            ("tyre_pressure_fl", 22. + n(18, 0, 6) * 0.1),
            ("tyre_pressure_fr", 22. + n(19, 0, 6) * 0.1),
            ("tyre_pressure_rl", 20.5 + n(20, 0, 6) * 0.1),
            ("tyre_pressure_rr", 20.5 + n(21, 0, 6) * 0.1),
            ("fuel_load", n(22, 20, 30)),
        ],
        compound: Some(["soft", "medium", "hard"][d % 3].to_owned()),
        changed: false,
    }
}

fn driver(i: usize) -> Driver {
    Driver {
        id: format!("{}", 1000 + i),
        name: NAMES[i % NAMES.len()].to_owned(),
        avatar: (i % 5 != 4).then(|| format!("mock:av-{}.png", i % 14)),
    }
}

fn season_id(year: &str) -> String {
    format!("swofty.swofty-seasons-{year}/sw_{year}")
}

fn seed(a: usize, b: usize) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (b as u64 + 1).wrapping_mul(0xC2B2AE3D27D4EB4F);
    h ^= h >> 31;
    h = h.wrapping_mul(0x94D049BB133111EB);
    h ^ (h >> 29)
}

fn record_ms(season: usize, track: usize) -> u64 {
    80_000 + (track as u64) * 6_200 + (seed(season, track) % 3_000)
}

fn time_of(season: usize, track: usize, place: usize) -> u64 {
    let base = record_ms(season, track);
    base + place as u64 * 311 + (place as u64).pow(2) * 23 + seed(track, place) % 120
}

fn key(season: usize, track: usize) -> BoardKey {
    BoardKey {
        content: season_id(YEARS[season]),
        track: TRACKS[track].0.to_owned(),
        weather: "dry".to_owned(),
    }
}

fn order(season: usize, track: usize) -> Vec<usize> {
    let mut ids: Vec<usize> = (0..NAMES.len()).collect();
    ids.sort_by_key(|i| seed(*i + season * 17, track * 31 + 5));
    ids
}

fn podium(season: usize, track: usize) -> Vec<PodiumEntry> {
    order(season, track)
        .into_iter()
        .take(3)
        .enumerate()
        .map(|(place, d)| PodiumEntry {
            driver: driver(d),
            time_ms: time_of(season, track, place),
        })
        .collect()
}

fn track_card(season: usize, track: usize) -> TrackCard {
    let p = podium(season, track);
    TrackCard {
        key: key(season, track),
        name: TRACKS[track].1.to_owned(),
        flag: Some(TRACKS[track].2.to_owned()),
        layout: Some(format!("mock:layout-{track}.svg")),
        record: p.first().cloned(),
        podium: p,
        drivers: 9 + (seed(season, track) % 6) as u32,
        laps: 40 + (seed(track, season) % 90) as u32,
        custom: false,
        addon: None,
    }
}

const CUSTOM: [(&str, &str, &str); 4] = [
    ("dromodf.fuji-speedway:suzuka24", "Fuji Speedway", "jp"),
    ("mod.suzuka24:suzuka24", "Fuji Speedway", "jp"),
    ("twilight.bahrain-reverse:sakhir_reverse", "Bahrain Reverse", "bh"),
    ("mod.hockenheim:hockenheim", "Hockenheimring", "de"),
];

fn custom_card(i: usize) -> TrackCard {
    let (track, name, flag) = CUSTOM[i];
    let mut card = track_card(YEARS.len() - 1, i % TRACKS.len());
    card.key.track = track.to_owned();
    card.name = name.to_owned();
    card.flag = Some(flag.to_owned());
    card.custom = true;
    card.addon = swofty_core::timetrial::boards::track_addon(track, None).map(str::to_owned);
    card
}

fn ranking(season: usize) -> Vec<RankEntry> {
    let mut points: Vec<(usize, f32, u32, u32)> = (0..NAMES.len())
        .map(|d| {
            let mut score = 0.;
            let (mut wins, mut podiums) = (0, 0);
            for t in 0..TRACKS.len() {
                let place = order(season, t).iter().position(|x| *x == d).unwrap_or(13);
                score +=
                    100. * (0.7 * (-(place as f32) * 0.012 / 0.015).exp() + 0.3 * (13 - place) as f32 / 13.);
                wins += u32::from(place == 0);
                podiums += u32::from(place < 3);
            }
            (d, score, wins, podiums)
        })
        .collect();
    points.sort_by(|a, b| b.1.total_cmp(&a.1));
    points
        .into_iter()
        .enumerate()
        .map(|(i, (d, score, wins, podiums))| RankEntry {
            rank: i as u32 + 1,
            driver: driver(d),
            points: score,
            wins,
            podiums,
        })
        .collect()
}

fn season_card(season: usize) -> SeasonCard {
    SeasonCard {
        id: season_id(YEARS[season]),
        name: label(YEARS[season]),
        cover: None,
        tracks: TRACKS.len() as u32,
        drivers: NAMES.len() as u32,
        top: ranking(season).into_iter().take(3).map(|r| r.driver).collect(),
    }
}

fn global() -> Vec<RankEntry> {
    let mut total: Vec<(usize, f32, u32, u32)> = (0..NAMES.len()).map(|d| (d, 0., 0, 0)).collect();
    for s in 0..YEARS.len() {
        for r in ranking(s) {
            let at = r.driver.id.parse::<usize>().unwrap_or(1000) - 1000;
            total[at].1 += r.points / (100. * TRACKS.len() as f32) * 100.;
            total[at].2 += r.wins;
            total[at].3 += r.podiums;
        }
    }
    total.sort_by(|a, b| b.1.total_cmp(&a.1));
    total
        .into_iter()
        .enumerate()
        .map(|(i, (d, score, wins, podiums))| RankEntry {
            rank: i as u32 + 1,
            driver: driver(d),
            points: score,
            wins,
            podiums,
        })
        .collect()
}

fn gate(mode: Mode) -> Result<(), OnlineError> {
    match mode {
        Mode::Failing => Err(OnlineError::network(swofty_i18n::tr(
            "couldn't reach the boards server: connection refused",
        ))),
        Mode::Slow => {
            std::thread::sleep(Duration::from_secs(3600));
            Ok(())
        }
        _ => Ok(()),
    }
}

fn find(season_key: &str) -> usize {
    YEARS.iter().position(|y| season_id(y) == season_key).unwrap_or(0)
}

const FIELD: usize = 420;

fn cat_shift(cat: &Category) -> usize {
    cat.assists as usize * 3 + cat.device as usize
}

fn device_of(d: usize) -> Option<InputDevice> {
    match d % 5 {
        0 | 1 => Some(InputDevice::Wheel),
        2 | 3 => Some(InputDevice::Controller),
        _ => d.is_multiple_of(2).then_some(InputDevice::Keyboard),
    }
}

fn aids_of_driver(d: usize) -> Assists {
    let on = !d.is_multiple_of(3);
    let level = |k: usize| {
        if on && !seed(d, k).is_multiple_of(3) {
            1 + (seed(d, k + 9) % 2) as u32
        } else {
            0
        }
    };
    Assists {
        levels: vec![
            (Aid::TractionControl, level(1)),
            (Aid::Abs, level(2)),
            (Aid::BrakingAssist, level(3)),
            (Aid::SteeringAssist, level(4)),
            (Aid::RacingLine, level(5)),
            (Aid::Gearbox, level(6)),
            (Aid::Ers, level(7)),
        ],
    }
}

fn matches_category(d: usize, cat: &Category) -> bool {
    let aids = aids_of_driver(d);
    let none = aids.none();
    let assist_ok = match cat.assists {
        AssistFilter::Either => true,
        AssistFilter::None => none,
        AssistFilter::Assists => !none,
    };
    let device_ok = match cat.device {
        DeviceFilter::All => true,
        DeviceFilter::Wheel => device_of(d) == Some(InputDevice::Wheel),
        DeviceFilter::Controller => device_of(d) == Some(InputDevice::Controller),
        DeviceFilter::Keyboard => device_of(d) == Some(InputDevice::Keyboard),
    };
    assist_ok && device_ok
}

fn big_driver(i: usize) -> Driver {
    if i < NAMES.len() {
        return driver(i);
    }
    Driver {
        id: format!("{}", 2000 + i),
        name: format!("racer{i:03}"),
        avatar: i.is_multiple_of(3).then(|| format!("mock:av-{}.png", i % 14)),
    }
}

fn big_ranking(scope: usize) -> Vec<RankEntry> {
    (0..FIELD)
        .map(|i| RankEntry {
            rank: i as u32 + 1,
            driver: big_driver(i),
            points: (900. - i as f32 * 2.1 - scope as f32).max(1.),
            wins: (40usize.saturating_sub(i / 3)) as u32 / 4,
            podiums: (60usize.saturating_sub(i / 2)) as u32 / 3,
        })
        .collect()
}

impl BoardsApi for MockBoards {
    fn set_token(&self, _: Option<String>) {}

    fn rankings(&self, scope: &str, cat: &Category, offset: u32) -> Result<RankingPage, OnlineError> {
        gate(self.mode)?;
        let scope_no = YEARS
            .iter()
            .position(|y| season_id(y) == scope)
            .map_or(0, |i| i + 1);
        let all = if self.mode == Mode::Empty {
            Vec::new()
        } else {
            big_ranking(scope_no + cat_shift(cat))
        };
        let rows: Vec<RankEntry> = all
            .iter()
            .skip(offset as usize)
            .take(RANK_PAGE as usize)
            .cloned()
            .collect();
        let end = offset as usize + rows.len();
        std::thread::sleep(Duration::from_millis(250));
        Ok(RankingPage {
            total: all.len() as u32,
            next_offset: (end < all.len()).then_some(end as u32),
            rows,
        })
    }

    fn me(&self) -> Result<Option<Me>, OnlineError> {
        gate(self.mode)?;
        let entry = big_ranking(0).into_iter().nth(137).map(|mut r| {
            r.driver.name = "swofty".into();
            r
        });
        Ok(entry.map(|entry| Me {
            driver: entry.driver.clone(),
            entry: Some(entry),
        }))
    }

    fn overview(&self, _: &Category) -> Result<Overview, OnlineError> {
        gate(self.mode)?;
        if self.mode == Mode::Empty {
            return Ok(Overview {
                seasons: Vec::new(),
                global: Vec::new(),
                custom: Vec::new(),
                me: None,
            });
        }
        Ok(Overview {
            seasons: (0..YEARS.len()).rev().map(season_card).collect(),
            global: global().into_iter().take(10).collect(),
            custom: (0..CUSTOM.len()).map(custom_card).collect(),
            me: None,
        })
    }

    fn season(&self, id: &str, _: &Category) -> Result<SeasonPage, OnlineError> {
        gate(self.mode)?;
        let s = find(id);
        Ok(SeasonPage {
            season: season_card(s),
            ranking: ranking(s),
            tracks: (0..TRACKS.len()).map(|t| track_card(s, t)).collect(),
        })
    }

    fn board(&self, k: &BoardKey, cat: &Category, _offset: u32) -> Result<BoardPage, OnlineError> {
        gate(self.mode)?;
        let s = find(&k.content);
        let custom = CUSTOM.iter().position(|c| c.0 == k.track);
        let t = TRACKS
            .iter()
            .position(|x| x.0 == k.track)
            .or(custom.map(|i| i % TRACKS.len()))
            .unwrap_or(0);
        let rows: Vec<Row> = order(s, t)
            .into_iter()
            .enumerate()
            .filter(|(_, d)| matches_category(*d, cat))
            .enumerate()
            .map(|(rank, (place, d))| {
                let time = time_of(s, t, place);
                let s1 = time * 31 / 100 + seed(d, 1) % 40;
                let s2 = time * 35 / 100 + seed(d, 2) % 40;
                Row {
                    rank: rank as u32 + 1,
                    lap_id: format!("lap{s}{t}{d:02}"),
                    driver: driver(d),
                    time_ms: time,
                    sectors_ms: vec![s1, s2, time - s1 - s2],
                    input: device_of(d),
                    aids: Some(aids_of_driver(d)),
                    car: Some(["ferrari sf-22", "mercedes w13", "red bull rb18"][d % 3].to_owned()),
                    date: format!("2026-11-{:02}T04:10:11Z", 10 - (d % 9)),
                    setup: (place % 5 != 4).then(|| mock_setup(d, s)),
                    game_replay: place % 3 == 0,
                }
            })
            .collect();
        let history = (0..5)
            .map(|i| HistoryPoint {
                time_ms: record_ms(s, t) + (5 - i) * 420,
                driver: driver((i as usize * 3 + t) % NAMES.len()),
                date: format!("2026-0{}-1{}", 4 + i, i),
            })
            .collect();
        Ok(BoardPage {
            track: custom.map_or_else(|| track_card(s, t), custom_card),
            season: label(YEARS[s]),
            total: rows.len() as u32,
            mine: rows.get(6).cloned(),
            rows,
            history,
        })
    }

    fn profile(&self, id: &str, _: &Category) -> Result<Profile, OnlineError> {
        gate(self.mode)?;
        let d = id.parse::<usize>().unwrap_or(1000) - 1000;
        let all = global();
        let entry = all.iter().find(|r| r.driver.id == id).cloned();
        let seasons = (0..YEARS.len())
            .rev()
            .filter_map(|s| {
                ranking(s)
                    .into_iter()
                    .find(|r| r.driver.id == id)
                    .map(|r| SeasonStanding {
                        season: label(YEARS[s]),
                        rank: r.rank,
                        points: r.points,
                    })
            })
            .collect();
        let best = |s: usize, t: usize| {
            let place = order(s, t).iter().position(|x| *x == d).unwrap_or(5);
            BestLap {
                season: label(YEARS[s]),
                track: TRACKS[t].1.to_owned(),
                key: key(s, t),
                time_ms: time_of(s, t, place),
                position: place as u32 + 1,
                lap_id: format!("lap{s}{t}{d:02}"),
                date: "2026-11-09T18:22:01Z".into(),
                setup: Some(mock_setup(d, s)),
            }
        };
        Ok(Profile {
            driver: driver(d),
            global: entry,
            seasons,
            bests: (0..6).map(|i| best(6 - i % 3, i)).collect(),
            recent: (0..4).map(|i| best(5, i + 2)).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_has_seasons_a_ranking_and_boards() {
        let api = MockBoards { mode: Mode::Full };
        let o = api.overview(&Category::default()).unwrap();
        assert_eq!(o.seasons.len(), 7);
        assert_eq!(o.global[0].rank, 1);
        assert!(o.global[0].points >= o.global[1].points);
        let s = api.season(&o.seasons[0].id, &Category::default()).unwrap();
        assert_eq!(s.tracks.len(), 8);
        let b = api.board(&s.tracks[0].key, &Category::default(), 0).unwrap();
        assert!(b.rows.windows(2).all(|w| w[0].time_ms <= w[1].time_ms));
        assert!(
            o.custom
                .iter()
                .all(|t| t.key.content == o.seasons[0].id && t.addon.is_some())
        );
        let fuji = api.board(&o.custom[0].key, &Category::default(), 0).unwrap();
        assert_eq!(fuji.track, o.custom[0]);
        assert_eq!(fuji.track.addon.as_deref(), Some("dromodf.fuji-speedway"));
    }
}
