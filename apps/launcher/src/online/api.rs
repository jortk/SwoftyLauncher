use serde::Deserialize;
use std::time::Duration;
use swofty_core::timetrial::boards::{content_listed, track_addon};
use swofty_i18n::trf;

use super::model::*;

pub trait BoardsApi: Send + Sync {
    fn overview(&self, cat: &Category) -> Result<Overview, OnlineError>;
    fn season(&self, id: &str, cat: &Category) -> Result<SeasonPage, OnlineError>;
    fn board(&self, key: &BoardKey, cat: &Category, offset: u32) -> Result<BoardPage, OnlineError>;
    fn profile(&self, id: &str, cat: &Category) -> Result<Profile, OnlineError>;
    fn rankings(&self, scope: &str, cat: &Category, offset: u32) -> Result<RankingPage, OnlineError>;
    fn me(&self) -> Result<Option<Me>, OnlineError>;
    fn set_token(&self, token: Option<String>);
}

#[derive(Deserialize, Default)]
struct UserDto {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    avatar: Option<String>,
}

impl From<UserDto> for Driver {
    fn from(u: UserDto) -> Self {
        Driver {
            id: u.id,
            name: u.name,
            avatar: u.avatar,
        }
    }
}

#[derive(Deserialize)]
struct RankDto {
    #[serde(default)]
    rank: u32,
    user: UserDto,
    #[serde(default)]
    score: f32,
    #[serde(default)]
    wins: u32,
    #[serde(default)]
    podiums: u32,
}

impl From<RankDto> for RankEntry {
    fn from(r: RankDto) -> Self {
        RankEntry {
            rank: r.rank,
            driver: r.user.into(),
            points: r.score,
            wins: r.wins,
            podiums: r.podiums,
        }
    }
}

#[derive(Deserialize, Default)]
struct RowsDto {
    #[serde(default)]
    rows: Vec<RankDto>,
}

#[derive(Deserialize, Default)]
struct CountsDto {
    #[serde(default)]
    tracks: u32,
    #[serde(default)]
    drivers: u32,
}

#[derive(Deserialize)]
struct SeasonTopDto {
    user: UserDto,
}

#[derive(Deserialize)]
struct SeasonDto {
    id: String,
    name: String,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    counts: CountsDto,
    #[serde(default)]
    top: Vec<SeasonTopDto>,
    #[serde(default)]
    tracks: Vec<TrackDto>,
    #[serde(default)]
    ranking: RowsDto,
}

#[derive(Deserialize)]
struct RecordDto {
    time_ms: u64,
    user: UserDto,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    weather: Option<String>,
}

#[derive(Deserialize)]
struct PodiumDto {
    user: UserDto,
    time_ms: u64,
}

#[derive(Deserialize)]
struct TrackDto {
    #[serde(default)]
    track: String,
    #[serde(default)]
    weather: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    layout: Option<String>,
    #[serde(default)]
    drivers: u32,
    #[serde(default)]
    laps: u32,
    #[serde(default)]
    record: Option<RecordDto>,
    #[serde(default)]
    podium: Vec<PodiumDto>,
    #[serde(default, alias = "track_addon")]
    addon: Option<String>,
}

fn absolute(server: &str, url: Option<String>) -> Option<String> {
    url.filter(|u| !u.is_empty()).map(|u| match u.starts_with('/') {
        true => format!("{server}{u}"),
        false => u,
    })
}

fn track_card(server: &str, content: &str, t: TrackDto, custom: bool) -> TrackCard {
    let weather = t.weather.clone().unwrap_or_else(|| "dry".to_owned());
    let content = t
        .record
        .as_ref()
        .and_then(|r| r.content.clone())
        .unwrap_or_else(|| content.to_owned());
    let weather = t
        .record
        .as_ref()
        .and_then(|r| r.weather.clone())
        .unwrap_or(weather);
    let name = match t.name.filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => track_title(&t.track),
    };
    let addon = track_addon(&t.track, t.addon.as_deref()).map(str::to_owned);
    TrackCard {
        key: BoardKey {
            content,
            track: t.track,
            weather,
        },
        name,
        flag: t.country,
        layout: absolute(server, t.layout),
        record: t.record.map(|r| PodiumEntry {
            driver: r.user.into(),
            time_ms: r.time_ms,
        }),
        podium: t
            .podium
            .into_iter()
            .map(|p| PodiumEntry {
                driver: p.user.into(),
                time_ms: p.time_ms,
            })
            .collect(),
        drivers: t.drivers,
        laps: t.laps,
        custom,
        addon,
    }
}

pub fn track_title(code: &str) -> String {
    let code = code.rsplit(':').next().unwrap_or(code);
    code.split(['_', '-'])
        .map(crate::format::title_case)
        .collect::<Vec<_>>()
        .join(" ")
}

fn season_card(server: &str, s: &SeasonDto) -> SeasonCard {
    SeasonCard {
        id: s.id.clone(),
        name: s.name.clone(),
        cover: absolute(server, s.cover.clone()),
        tracks: s.counts.tracks,
        drivers: s.counts.drivers,
        top: s
            .top
            .iter()
            .map(|t| Driver {
                id: t.user.id.clone(),
                name: t.user.name.clone(),
                avatar: t.user.avatar.clone(),
            })
            .collect(),
    }
}

#[derive(Deserialize)]
struct OverviewDto {
    #[serde(default)]
    seasons: Vec<SeasonDto>,
    #[serde(default)]
    global: RowsDto,
    #[serde(default)]
    custom: CustomDto,
}

#[derive(Deserialize, Default)]
struct CustomDto {
    #[serde(default)]
    tracks: Vec<TrackDto>,
}

#[derive(Deserialize)]
struct GameReplayDto {
    #[serde(default)]
    size: u64,
    #[serde(default)]
    setup: serde_json::Value,
}

#[derive(Deserialize)]
struct BoardRowDto {
    rank: u32,
    lap_id: String,
    user: UserDto,
    time_ms: u64,
    #[serde(default)]
    sectors_ms: Vec<u64>,
    #[serde(default)]
    car_label: Option<String>,
    #[serde(default)]
    set_at: String,
    #[serde(default)]
    setup: Option<serde_json::Value>,
    #[serde(default)]
    assists: Option<serde_json::Value>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    game_replay: Option<GameReplayDto>,
}

#[derive(Deserialize)]
struct HistoryDto {
    time_ms: u64,
    #[serde(default)]
    set_at: String,
    user: UserDto,
}

#[derive(Deserialize)]
struct BoardDto {
    content: String,
    track: String,
    weather: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    custom: bool,
    #[serde(default)]
    track_addon: Option<String>,
    #[serde(default)]
    layout: Option<String>,
    #[serde(default)]
    total: u32,
    #[serde(default)]
    record: Option<RecordDto>,
    #[serde(default)]
    rows: Vec<BoardRowDto>,
    #[serde(default)]
    history: Vec<HistoryDto>,
}

fn row(r: BoardRowDto) -> Row {
    Row {
        rank: r.rank,
        lap_id: r.lap_id,
        driver: r.user.into(),
        time_ms: r.time_ms,
        sectors_ms: r.sectors_ms,
        car: r.car_label.filter(|c| !c.is_empty()),
        date: r.set_at,
        input: input_of(r.input.as_ref(), r.assists.as_ref()),
        aids: aids_of(r.assists.as_ref()),
        setup: r.setup.as_ref().and_then(setup_of),
        game_replay: r.game_replay.is_some_and(|g| g.size > 0 && g.setup.is_object()),
    }
}

pub(super) fn input_of(
    input: Option<&serde_json::Value>,
    assists: Option<&serde_json::Value>,
) -> Option<InputDevice> {
    input
        .and_then(|v| v.as_str())
        .or_else(|| {
            assists
                .and_then(|a| a.get("input_device"))
                .and_then(|v| v.as_str())
        })
        .and_then(InputDevice::parse)
}

pub(super) fn aids_of(assists: Option<&serde_json::Value>) -> Option<Assists> {
    let map = assists?.as_object()?;
    let mut levels: Vec<(Aid, u32)> = Vec::new();
    for (key, value) in map {
        if let (Some(aid), Some(n)) = (
            Aid::from_key(key),
            value.as_u64().or_else(|| value.as_bool().map(u64::from)),
        ) {
            levels.push((aid, n as u32));
        }
    }
    (!levels.is_empty()).then_some(Assists { levels })
}

#[derive(Deserialize)]
struct StandingDto {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    rank: Option<u32>,
    #[serde(default)]
    score: f32,
}

#[derive(Deserialize)]
struct BestDto {
    #[serde(default)]
    content: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    track: String,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    weather: String,
    lap_id: String,
    time_ms: u64,
    #[serde(default)]
    rank: Option<u32>,
    #[serde(default)]
    position: Option<u32>,
    #[serde(default)]
    set_at: String,
    #[serde(default)]
    setup: Option<serde_json::Value>,
}

pub(super) fn setup_of(v: &serde_json::Value) -> Option<Setup> {
    let map = v.as_object()?;
    let mut values = Vec::new();
    for (_, rows) in SETUP_GROUPS {
        for (key, _, _) in rows {
            if let Some(n) = map.get(*key).and_then(serde_json::Value::as_f64) {
                values.push((*key, n as f32));
            }
        }
    }
    let setup = Setup {
        values,
        compound: map
            .get("tyre_compound")
            .and_then(|c| c.as_str())
            .map(str::to_owned),
        changed: map
            .get("changed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    };
    (!setup.is_empty()).then_some(setup)
}

fn best(b: BestDto) -> BestLap {
    BestLap {
        season: b.name,
        track: b.track_name.clone().unwrap_or_else(|| track_title(&b.track)),
        key: BoardKey {
            content: b.content,
            track: b.track,
            weather: b.weather,
        },
        time_ms: b.time_ms,
        position: b.position.or(b.rank).unwrap_or(0),
        lap_id: b.lap_id,
        date: b.set_at,
        setup: b.setup.as_ref().and_then(setup_of),
    }
}

#[derive(Deserialize)]
struct RecentDto {
    lap_id: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    track: String,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    weather: String,
    time_ms: u64,
    #[serde(default)]
    set_at: String,
}

#[derive(Deserialize)]
struct ProfileDto {
    user: UserDto,
    #[serde(default)]
    global: Option<RankDto2>,
    #[serde(default)]
    seasons: Vec<StandingDto>,
    #[serde(default)]
    bests: Vec<BestDto>,
    #[serde(default)]
    recent: Vec<RecentDto>,
}

#[derive(Deserialize)]
struct RankDto2 {
    #[serde(default)]
    rank: u32,
    #[serde(default)]
    score: f32,
    #[serde(default)]
    wins: u32,
    #[serde(default)]
    podiums: u32,
}

pub struct HttpBoards {
    server: String,
    token: std::sync::Mutex<Option<String>>,
}

impl HttpBoards {
    pub fn new() -> Self {
        Self::with_server(&swofty_core::catalog::base_url())
    }

    pub fn with_server(server: &str) -> Self {
        Self {
            server: server.trim().trim_end_matches('/').to_owned(),
            token: std::sync::Mutex::new(None),
        }
    }

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, OnlineError> {
        let url = format!("{}{path}", self.server);
        let token = self.token.lock().ok().and_then(|t| t.clone());
        let mut request = ureq::get(&url)
            .header("User-Agent", "swofty-launcher")
            .header("Accept", "application/json");
        if let Some(token) = token {
            request = request.header("Authorization", &["Bearer ", token.as_str()].concat());
        }
        let sent = request
            .config()
            .timeout_global(Some(Duration::from_secs(8)))
            .timeout_connect(Some(Duration::from_secs(5)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .call();
        let mut response = sent.map_err(|e| {
            OnlineError::network(trf!(
                "couldn't reach {server}: {error}",
                server = self.server,
                error = e
            ))
        })?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| {
                OnlineError::network(trf!(
                    "reading the answer to GET {path}: {error}",
                    path = path,
                    error = e
                ))
            })?;
        if !(200..300).contains(&status) {
            return Err(OnlineError {
                status,
                message: error_text(status, &body),
            });
        }
        serde_json::from_slice(&body).map_err(|e| OnlineError {
            status,
            message: trf!(
                "the answer to GET {path} is not valid: {error}",
                path = path,
                error = e
            ),
        })
    }
}

impl Default for HttpBoards {
    fn default() -> Self {
        Self::new()
    }
}

fn error_text(status: u16, body: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
        .unwrap_or_else(|| trf!("the boards server answered HTTP {status}", status = status))
}

fn lead(cat: &Category) -> String {
    let q = cat.query();
    if q.is_empty() {
        q
    } else {
        format!("?{}", q.trim_start_matches('&'))
    }
}

pub fn encode(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for b in part.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Deserialize)]
struct RankingDto {
    #[serde(default)]
    total: u32,
    #[serde(default)]
    next_offset: Option<u32>,
    #[serde(default)]
    rows: Vec<RankDto>,
}

#[derive(Deserialize)]
struct MeDto {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    global: Option<RankDto2>,
}

impl BoardsApi for HttpBoards {
    fn set_token(&self, token: Option<String>) {
        if let Ok(mut t) = self.token.lock() {
            *t = token;
        }
    }

    fn rankings(&self, scope: &str, cat: &Category, offset: u32) -> Result<RankingPage, OnlineError> {
        let dto: RankingDto = self.get(&format!(
            "/v1/tt/rankings?season={}&limit={RANK_PAGE}&offset={offset}{}",
            encode(scope),
            cat.query()
        ))?;
        Ok(RankingPage {
            rows: dto.rows.into_iter().map(Into::into).collect(),
            total: dto.total,
            next_offset: dto.next_offset,
        })
    }

    fn me(&self) -> Result<Option<Me>, OnlineError> {
        if self.token.lock().ok().is_none_or(|t| t.is_none()) {
            return Ok(None);
        }
        match self.get::<MeDto>("/v1/tt/me") {
            Ok(dto) => {
                let driver = Driver {
                    id: dto.id,
                    name: dto.name,
                    avatar: dto.avatar,
                };
                Ok(Some(Me {
                    entry: dto.global.map(|g| RankEntry {
                        rank: g.rank,
                        driver: driver.clone(),
                        points: g.score,
                        wins: g.wins,
                        podiums: g.podiums,
                    }),
                    driver,
                }))
            }
            Err(e) if e.status == 401 => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn overview(&self, cat: &Category) -> Result<Overview, OnlineError> {
        let dto: OverviewDto = self.get(&["/v1/tt/overview", &lead(cat)].concat())?;
        Ok(overview_of(&self.server, dto))
    }

    fn season(&self, id: &str, cat: &Category) -> Result<SeasonPage, OnlineError> {
        let dto: SeasonDto = self.get(&format!("/v1/tt/seasons/{}{}", encode(id), lead(cat)))?;
        let season = season_card(&self.server, &dto);
        let tracks = dto
            .tracks
            .into_iter()
            .map(|t| track_card(&self.server, id, t, false))
            .filter(|t| content_listed(&t.key.content))
            .collect();
        Ok(SeasonPage {
            season,
            ranking: dto.ranking.rows.into_iter().map(Into::into).collect(),
            tracks,
        })
    }

    fn board(&self, key: &BoardKey, cat: &Category, offset: u32) -> Result<BoardPage, OnlineError> {
        let mut path = format!(
            "/v1/tt/boards/{}/{}?weather={}&limit={PAGE_SIZE}&offset={offset}",
            encode(&key.content),
            encode(&key.track),
            encode(&key.weather)
        );
        if SHOW_SETUPS {
            path.push_str("&setup=1");
        }
        path.push_str(&cat.query());
        let dto: BoardDto = self.get(&path)?;
        let track = TrackCard {
            key: BoardKey {
                content: dto.content.clone(),
                track: dto.track.clone(),
                weather: dto.weather.clone(),
            },
            name: dto.track_name.clone().unwrap_or_else(|| track_title(&dto.track)),
            flag: dto.country,
            layout: absolute(&self.server, dto.layout),
            record: dto.record.map(|r| PodiumEntry {
                driver: r.user.into(),
                time_ms: r.time_ms,
            }),
            podium: Vec::new(),
            drivers: dto.total,
            laps: dto.total,
            custom: dto.custom,
            addon: track_addon(&dto.track, dto.track_addon.as_deref()).map(str::to_owned),
        };
        Ok(BoardPage {
            track,
            season: dto.name,
            rows: dto.rows.into_iter().map(row).collect(),
            total: dto.total,
            history: dto
                .history
                .into_iter()
                .map(|h| HistoryPoint {
                    time_ms: h.time_ms,
                    driver: h.user.into(),
                    date: h.set_at,
                })
                .collect(),
            mine: None,
        })
    }

    fn profile(&self, id: &str, cat: &Category) -> Result<Profile, OnlineError> {
        let dto: ProfileDto = self.get(&format!("/v1/tt/users/{}{}", encode(id), lead(cat)))?;
        Ok(profile_of(dto))
    }
}

fn overview_of(server: &str, dto: OverviewDto) -> Overview {
    Overview {
        seasons: dto
            .seasons
            .iter()
            .filter(|s| content_listed(&s.id))
            .map(|s| season_card(server, s))
            .collect(),
        global: dto.global.rows.into_iter().map(Into::into).collect(),
        custom: dto
            .custom
            .tracks
            .into_iter()
            .map(|t| track_card(server, "", t, true))
            .filter(|t| content_listed(&t.key.content))
            .collect(),
        me: None,
    }
}

fn profile_of(dto: ProfileDto) -> Profile {
    let driver: Driver = dto.user.into();
    Profile {
        global: dto.global.map(|g| RankEntry {
            rank: g.rank,
            driver: driver.clone(),
            points: g.score,
            wins: g.wins,
            podiums: g.podiums,
        }),
        driver,
        seasons: dto
            .seasons
            .into_iter()
            .filter(|s| content_listed(&s.id))
            .filter_map(|s| {
                s.rank.map(|rank| SeasonStanding {
                    season: s.name,
                    rank,
                    points: s.score,
                })
            })
            .collect(),
        bests: dto
            .bests
            .into_iter()
            .filter(|b| content_listed(&b.content))
            .map(best)
            .collect(),
        recent: dto
            .recent
            .into_iter()
            .filter(|r| content_listed(&r.content))
            .map(|r| {
                best(BestDto {
                    content: r.content,
                    name: r.name,
                    track: r.track,
                    track_name: r.track_name,
                    weather: r.weather,
                    lap_id: r.lap_id,
                    time_ms: r.time_ms,
                    rank: None,
                    position: None,
                    set_at: r.set_at,
                    setup: None,
                })
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_overview() {
        let dto: OverviewDto = serde_json::from_str(
            r#"{"seasons":[{"id":"a/f1_2022","name":"F1 2022","cover":"https://x/icon","counts":{"tracks":22,"drivers":17,"laps":410},
              "top":[{"rank":1,"user":{"id":"1","name":"swofty","avatar":null},"score":1834.21}]}],
              "global":{"total":1,"rows":[{"rank":1,"user":{"id":"1","name":"swofty","avatar":null},"score":187.4,"boards":31,"wins":12,"podiums":20}]},
              "custom":{"tracks":[{"track":"a.h:hockenheim","name":"Hockenheimring","country":"de","layout":null,"boards":2,"drivers":4,"laps":9,
                "record":{"time_ms":58815,"user":{"id":"2","name":"d2","avatar":null},"content":"a/f1_2022","weather":"dry"}}]}}"#,
        )
        .unwrap();
        let card = season_card("https://s", &dto.seasons[0]);
        assert_eq!((card.tracks, card.drivers, card.top.len()), (22, 17, 1));
        let track = track_card(
            "https://s",
            "",
            dto.custom.tracks.into_iter().next().unwrap(),
            true,
        );
        assert_eq!(track.key.content, "a/f1_2022");
        assert_eq!(track.record.unwrap().time_ms, 58815);
    }

    #[test]
    fn the_overview_and_a_profile_leave_out_unlisted_seasons() {
        let dto: OverviewDto = serde_json::from_str(
            r#"{"seasons":[{"id":"game/f1_2025","name":"F1 25 (stock)"},{"id":"swofty.classic-seasons-2020/f1_2020","name":"F1 2020 GP/TT Addon"},{"id":"swofty.swofty-seasons-2019/sw_2019","name":"SWOFTY 2019"}],
              "custom":{"tracks":[{"track":"mod.nycgp:nycgp","record":{"time_ms":1,"user":{"id":"2","name":"d2"},"content":"swofty.classic-seasons-2021/f1_2021","weather":"dry"}},
                {"track":"mod.lbcgp:lbcgp","record":{"time_ms":1,"user":{"id":"2","name":"d2"},"content":"game/f1_2025","weather":"dry"}}]}}"#,
        )
        .unwrap();
        let overview = overview_of("https://s", dto);
        assert_eq!(
            overview.seasons.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["game/f1_2025", "swofty.swofty-seasons-2019/sw_2019"]
        );
        assert_eq!(
            overview
                .custom
                .iter()
                .map(|t| t.key.track.as_str())
                .collect::<Vec<_>>(),
            ["mod.lbcgp:lbcgp"]
        );
        let dto: ProfileDto = serde_json::from_str(
            r#"{"user":{"id":"1","name":"d"},
              "seasons":[{"id":"swofty.swofty-seasons-2021/sw_2021","name":"SWOFTY 2021","rank":1,"score":40},{"id":"bydanix.f1-2012-season/ex_f1_2012","name":"F1 2012","rank":2,"score":9}],
              "bests":[{"content":"msyeett.classic-seasons-2015/f1_2015","track":"monza","weather":"dry","lap_id":"a","time_ms":1},{"content":"game/f1_2025","track":"monza","weather":"dry","lap_id":"b","time_ms":2}],
              "recent":[{"content":"swofty.classic-seasons-2017/f1_2017","lap_id":"c","time_ms":3},{"content":"swofty.swofty-seasons-2015/sw_2015","lap_id":"d","time_ms":4}]}"#,
        )
        .unwrap();
        let profile = profile_of(dto);
        assert_eq!(
            profile
                .seasons
                .iter()
                .map(|s| s.season.as_str())
                .collect::<Vec<_>>(),
            ["SWOFTY 2021"]
        );
        assert_eq!(
            profile
                .bests
                .iter()
                .map(|b| b.lap_id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
        assert_eq!(
            profile
                .recent
                .iter()
                .map(|b| b.lap_id.as_str())
                .collect::<Vec<_>>(),
            ["d"]
        );
    }

    #[test]
    fn a_tracks_addon_is_read_and_a_stock_track_in_a_season_box_has_none() {
        let custom: TrackDto = serde_json::from_str(
            r#"{"track":"twilight.bahrain-reverse:sakhir_reverse","name":"Bahrain Reverse","addon":"twilight.bahrain-reverse","country":"bh","laps":3}"#,
        )
        .unwrap();
        let card = track_card("https://s", "game/f1_2025", custom, true);
        assert_eq!(card.addon.as_deref(), Some("twilight.bahrain-reverse"));
        let listed: TrackDto = serde_json::from_str(
            r#"{"track":"mod.lisgp:lisgp","name":"Portuguese Grand Prix","track_addon":"mod.lisgp"}"#,
        )
        .unwrap();
        assert_eq!(
            track_card("https://s", "", listed, true).addon.as_deref(),
            Some("mod.lisgp")
        );
        let stock: TrackDto =
            serde_json::from_str(r#"{"track":"bahrain","name":"Bahrain International Circuit"}"#).unwrap();
        let season = track_card("https://s", "swofty.swofty-seasons-2015/sw_2015", stock, false);
        assert_eq!(season.addon, None);
        let board: BoardDto = serde_json::from_str(
            r#"{"content":"swofty.swofty-seasons-2015/sw_2015","track":"mod.lemans:le_mans_2017","weather":"dry","name":"SWOFTY 2015","track_name":"Le Mans","custom":true,"track_addon":"mod.lemans"}"#,
        )
        .unwrap();
        assert_eq!(board.track_addon.as_deref(), Some("mod.lemans"));
        assert_eq!(
            TrackCard::bare(
                BoardKey {
                    content: "swofty.swofty-seasons-2015/sw_2015".into(),
                    track: "monaco".into(),
                    weather: "dry".into(),
                },
                "Monaco".into(),
            )
            .addon,
            None
        );
    }

    #[test]
    fn relative_art_is_pointed_at_the_server() {
        assert_eq!(
            absolute("https://s", Some("/v1/x.svg".into())).as_deref(),
            Some("https://s/v1/x.svg")
        );
        assert_eq!(
            absolute("https://s", Some("https://c/a.png".into())).as_deref(),
            Some("https://c/a.png")
        );
        assert_eq!(absolute("https://s", None), None);
        assert_eq!(absolute("https://s", Some(String::new())), None);
    }

    #[test]
    fn encodes_the_slash_and_colon() {
        assert_eq!(encode("a.b/c_d"), "a.b%2Fc_d");
        assert_eq!(encode("addon:track"), "addon%3Atrack");
    }

    #[test]
    fn an_unreachable_server_is_a_network_error() {
        let e = HttpBoards::with_server("http://127.0.0.1:1")
            .overview(&Category::default())
            .unwrap_err();
        assert_eq!(e.status, 0);
        assert!(e.message.contains("couldn't reach"));
    }

    #[test]
    fn the_local_servers_real_board_and_empty_overview_parse() {
        let board: BoardDto = serde_json::from_str(r##"{"content": "swofty.swofty-seasons-2016/sw_2016", "track": "suzuka", "weather": "dry", "name": "swofty.swofty-seasons-2016/sw_2016", "track_name": "Suzuka Circuit", "country": "jp", "custom": false, "track_addon": null, "key": "swofty.swofty-seasons-2016/sw_2016|suzuka|dry", "layout": null, "filters": {"assists": null, "device": null, "ranked": false}, "devices": [], "total": 1, "limit": 50, "offset": 0, "record": null, "rows": [{"rank": 1, "lap_id": "11c96143c8aa631d", "user": {"id": "42", "name": "Swofty", "avatar": null}, "time_ms": 88106, "sectors_ms": [31583, 38881, 17641], "car": {"team": "sw16_ferrari"}, "assists": {"brk": 0, "abs": 1, "trc": 2, "drl": 2, "gbx": 3, "ers": 0, "input": 3}, "set_at": "2026-11-10T07:09:29Z", "status": "unranked", "gap_ms": 0, "gap_pct": 0.0, "car_label": "sw16_ferrari", "input": null, "verification": "unverified", "has_replay": true}], "history": [{"lap_id": "421f3a7eece3a234", "time_ms": 88117, "set_at": "2026-11-10T06:19:01Z", "user": {"id": "42", "name": "Swofty", "avatar": null}, "improved_ms": null}, {"lap_id": "11c96143c8aa631d", "time_ms": 88106, "set_at": "2026-11-10T07:09:29Z", "user": {"id": "42", "name": "Swofty", "avatar": null}, "improved_ms": 11}]}"##).unwrap();
        assert_eq!(board.rows.len(), 1);
        assert_eq!(board.rows[0].time_ms, 88106);
        assert_eq!(row(board.rows.into_iter().next().unwrap()).rank, 1);
        let overview: OverviewDto = serde_json::from_str(r##"{"generated": "2026-11-10T08:11:54Z", "scoring": {"min_field": 3, "pace_scale": 0.015, "pace_share": 0.7, "full_field": 10}, "seasons": [], "global": {"total": 0, "rows": []}, "custom": {"tracks": []}}"##).unwrap();
        assert!(overview.seasons.is_empty() && overview.global.rows.is_empty());
    }

    #[test]
    fn a_setup_object_keeps_known_numbers_and_the_compound() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"front_wing":25,"brake_bias":56,"tyre_pressure_fl":22.5,"tyre_compound":"soft","changed":true,"mystery":1}"#,
        )
        .unwrap();
        let s = setup_of(&v).unwrap();
        assert_eq!(s.get("front_wing"), Some(25.));
        assert_eq!(s.get("tyre_pressure_fl"), Some(22.5));
        assert_eq!(s.compound.as_deref(), Some("soft"));
        assert!(s.changed);
        assert!(setup_of(&serde_json::json!({})).is_none());
        assert!(setup_of(&serde_json::Value::Null).is_none());
    }

    #[test]
    fn a_ranking_page_and_me_parse() {
        let dto: RankingDto = serde_json::from_str(
            r#"{"season":"all","name":"All","total":312,"limit":50,"offset":0,"next_offset":50,
               "rows":[{"rank":1,"user":{"id":"1","name":"swofty","avatar":null},"score":187.4,"boards":31,"wins":12,"podiums":20}]}"#,
        )
        .unwrap();
        assert_eq!((dto.total, dto.next_offset, dto.rows.len()), (312, Some(50), 1));
        let me: MeDto = serde_json::from_str(
            r#"{"id":"1","name":"swofty","avatar":null,"global":{"rank":3,"of":40,"score":187.4,"boards":31,"wins":12,"podiums":20}}"#,
        )
        .unwrap();
        assert_eq!(me.global.unwrap().rank, 3);
        let last: RankingDto = serde_json::from_str(r#"{"total":3,"next_offset":null,"rows":[]}"#).unwrap();
        assert_eq!(last.next_offset, None);
    }

    #[test]
    fn a_rows_assists_and_input_are_read_from_the_server_shape() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"trc":2,"abs":1,"gbx":0,"mystery":5,"input_device":"wheel"}"#).unwrap();
        let aids = aids_of(Some(&v)).unwrap();
        assert_eq!(aids.level(Aid::TractionControl), Some(2));
        assert_eq!(aids.level(Aid::Gearbox), Some(0));
        assert_eq!(input_of(None, Some(&v)), Some(InputDevice::Wheel));
        let direct = serde_json::json!("pad");
        assert_eq!(input_of(Some(&direct), None), Some(InputDevice::Controller));
        assert!(aids_of(Some(&serde_json::json!({}))).is_none());
    }
}
