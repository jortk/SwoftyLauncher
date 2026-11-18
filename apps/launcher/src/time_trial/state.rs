use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use swofty_core::timetrial::api::Api;
use swofty_core::timetrial::barred::{self, Barred};
use swofty_core::timetrial::boards::{Group, Row, Run, RunBoard};
use swofty_i18n::{tr, trf};

use crate::shell::Shell;

pub(crate) enum Load<T> {
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Pick {
    pub content: String,
    pub track: String,
    pub weather: String,
    pub title: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Boards,
    Runs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Accept,
    Deny,
    Ban,
}

pub(crate) struct TtPage {
    pub view: View,
    pub runs: Load<Vec<Run>>,
    pub run_boards: Vec<RunBoard>,
    pub run_seasons: Vec<String>,
    pub run_tracks: Vec<String>,
    pub run_status: &'static str,
    pub run_query: String,
    pub run_search: Option<Entity<InputState>>,
    run_sub: Option<Subscription>,
    run_generation: u64,
    pub open_lap: Option<String>,
    pub banning: bool,
    pub acting: bool,
    pub notice: Option<String>,
    pub reason: Option<Entity<InputState>>,
    pub catalog: Load<Vec<Group>>,
    pub pick: Option<Pick>,
    pub rows: Load<Vec<Row>>,
    pub query: String,
    pub search: Option<Entity<InputState>>,
    sub: Option<Subscription>,
    generation: u64,
    pub barred: Vec<Barred>,
}

impl Default for TtPage {
    fn default() -> Self {
        Self {
            view: View::Runs,
            runs: Load::Idle,
            run_boards: Vec::new(),
            run_seasons: Vec::new(),
            run_tracks: Vec::new(),
            run_status: "all",
            run_query: String::new(),
            run_search: None,
            run_sub: None,
            run_generation: 0,
            open_lap: None,
            banning: false,
            acting: false,
            notice: None,
            reason: None,
            catalog: Load::Idle,
            pick: None,
            rows: Load::Idle,
            query: String::new(),
            search: None,
            sub: None,
            generation: 0,
            barred: Vec::new(),
        }
    }
}

impl TtPage {
    /// The loaded runs that pass the status choice and the search text, newest first as loaded.
    pub(super) fn shown_runs(&self) -> Vec<&Run> {
        let Load::Ready(runs) = &self.runs else {
            return Vec::new();
        };
        runs.iter()
            .filter(|r| self.run_status == "all" || r.status == self.run_status)
            .filter(|r| r.matches(&self.run_query))
            .collect()
    }

    /// The seasons (`season`) or the tracks of the picked seasons that have runs, as key and label,
    /// sorted by label. A name two entries share gets its key after it.
    pub(super) fn choices(&self, season: bool) -> Vec<(String, String)> {
        let boards = self.run_boards.iter();
        if season {
            labelled(boards.map(|b| (b.content.as_str(), b.season.as_deref())))
        } else {
            labelled(
                boards
                    .filter(|b| self.run_seasons.is_empty() || self.run_seasons.contains(&b.content))
                    .map(|b| (b.track.as_str(), b.track_name.as_deref())),
            )
        }
    }

    pub(super) fn picked(&self, season: bool) -> &[String] {
        if season {
            &self.run_seasons
        } else {
            &self.run_tracks
        }
    }

    /// What the season or track filter button reads for the picks.
    pub(super) fn filter_label(&self, season: bool, choices: &[(String, String)]) -> String {
        let name = |key: &String| {
            choices
                .iter()
                .find(|(k, _)| k == key)
                .map_or(key.as_str(), |(_, label)| label.as_str())
                .to_owned()
        };
        match (season, self.picked(season)) {
            (true, []) => tr("All seasons").to_owned(),
            (false, []) => tr("All tracks").to_owned(),
            (true, [one]) => trf!("Season: {name}", name = name(one)),
            (false, [one]) => trf!("Track: {name}", name = name(one)),
            (true, many) => trf!("Season: {count} picked", count = many.len()),
            (false, many) => trf!("Track: {count} picked", count = many.len()),
        }
    }

    fn keep_listed_tracks(&mut self) {
        if self.run_boards.is_empty() {
            return;
        }
        let listed = self.choices(false);
        self.run_tracks.retain(|t| listed.iter().any(|(k, _)| k == t));
    }

    pub(super) fn waiting(&self) -> usize {
        match &self.runs {
            Load::Ready(runs) => runs.iter().filter(|r| r.pending()).count(),
            _ => 0,
        }
    }

    pub(super) fn visible(&self) -> Vec<&Row> {
        let Load::Ready(rows) = &self.rows else {
            return Vec::new();
        };
        swofty_core::timetrial::boards::by_driver(rows, &self.query)
    }
}

fn labelled<'a>(items: impl Iterator<Item = (&'a str, Option<&'a str>)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (key, name) in items {
        if out.iter().any(|(k, _)| k == key) {
            continue;
        }
        let name = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(key);
        out.push((key.to_owned(), name.to_owned()));
    }
    let names: Vec<String> = out.iter().map(|(_, label)| label.to_lowercase()).collect();
    for (i, (key, label)) in out.iter_mut().enumerate() {
        if label != key && names.iter().filter(|n| **n == names[i]).count() > 1 {
            *label = format!("{label} ({key})");
        }
    }
    out.sort_by_cached_key(|(key, label)| (label.to_lowercase(), key.clone()));
    out
}

impl Shell {
    pub(super) fn tt_api(&self) -> Option<Api> {
        self.mp.as_ref().map(|s| Api::new(&s.token))
    }

    pub(crate) fn tt_admin(&self) -> bool {
        self.mp.as_ref().is_some_and(|s| s.admin)
    }

    pub(crate) fn tt_open(&mut self, cx: &mut Context<Self>) {
        self.tt_load_barred(cx);
        if self.tt_admin() {
            self.tt_load_runs(cx);
        } else {
            self.tt.view = View::Boards;
        }
        if matches!(self.tt.catalog, Load::Idle | Load::Failed(_)) {
            self.tt_load_catalog(cx);
        }
    }

    /// The barred list, from the cache while it is fresh, else from the server.
    pub(super) fn tt_load_barred(&mut self, cx: &mut Context<Self>) {
        let data = self.store.root.clone();
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_executor()
                .spawn(async move { barred::current(&data, swofty_core::timetrial::queue::unix_now()) })
                .await;
            this.update(cx, |s, cx| {
                if let Some(list) = got {
                    s.tt.barred = list;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The barred addons the active profile has enabled, at the versions installed.
    pub(super) fn tt_barred_enabled(&self) -> Vec<&Barred> {
        let registry = &self.library.registry;
        barred::in_profile(&self.tt.barred, &self.library.profile, |id| {
            registry.get(id).map(|e| e.addon.manifest().version.to_string())
        })
    }

    pub(super) fn tt_load_catalog(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.tt_api() else {
            return;
        };
        self.tt.catalog = Load::Loading;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = cx.background_executor().spawn(async move { api.catalog() }).await;
            this.update(cx, |s, cx| {
                s.tt.catalog = match got {
                    Ok(groups) => Load::Ready(groups),
                    Err(e) => Load::Failed(e.message),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn tt_load_runs(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.tt_api().filter(|_| self.tt_admin()) else {
            return;
        };
        self.tt.run_generation += 1;
        let generation = self.tt.run_generation;
        let (seasons, tracks) = (self.tt.run_seasons.clone(), self.tt.run_tracks.clone());
        if !matches!(self.tt.runs, Load::Ready(_)) {
            self.tt.runs = Load::Loading;
        }
        cx.spawn(async move |this, cx| {
            let (runs, boards) = cx
                .background_executor()
                .spawn(async move { (api.runs(&seasons, &tracks), api.run_boards()) })
                .await;
            this.update(cx, |s, cx| {
                if s.tt.run_generation != generation {
                    return;
                }
                s.tt.runs = match runs {
                    Ok(laps) => {
                        if s.tt.open_lap.is_none() {
                            s.tt.open_lap = laps.first().map(|r| r.lap_id.clone());
                        }
                        Load::Ready(laps)
                    }
                    Err(e) => Load::Failed(e.message),
                };
                if let Ok(boards) = boards {
                    s.tt.run_boards = boards;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Turn one season or track filter on or off and load the runs again. Picked tracks that no picked
    /// season has are dropped.
    pub(super) fn tt_toggle_filter(&mut self, season: bool, value: &str, cx: &mut Context<Self>) {
        let list = if season {
            &mut self.tt.run_seasons
        } else {
            &mut self.tt.run_tracks
        };
        match list.iter().position(|v| v == value) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(value.to_owned()),
        }
        if season {
            self.tt.keep_listed_tracks();
        }
        self.tt_load_runs(cx);
        cx.notify();
    }

    pub(super) fn tt_clear_filter(&mut self, season: bool, cx: &mut Context<Self>) {
        if season {
            self.tt.run_seasons.clear();
        } else {
            self.tt.run_tracks.clear();
        }
        self.tt_load_runs(cx);
        cx.notify();
    }

    pub(super) fn tt_run_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(state) = &self.tt.run_search {
            return state.clone();
        }
        let state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(swofty_i18n::tr("Search driver, season, track or lap id"))
        });
        self.tt.run_sub = Some(cx.subscribe(&state, |this, state, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.tt.run_query = state.read(cx).value().to_string();
                cx.notify();
            }
        }));
        self.tt.run_search = Some(state.clone());
        state
    }

    pub(super) fn tt_select_lap(&mut self, lap: &str, cx: &mut Context<Self>) {
        self.tt.open_lap = Some(lap.to_owned());
        self.tt.banning = false;
        self.tt.notice = None;
        cx.notify();
    }

    pub(super) fn tt_moderate(
        &mut self,
        lap: String,
        verdict: Verdict,
        reason: String,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.tt_api().filter(|_| self.tt_admin() && !self.tt.acting) else {
            return;
        };
        self.tt.acting = true;
        self.tt.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move {
                    match verdict {
                        Verdict::Accept => api.accept_lap(&lap),
                        Verdict::Deny => api.deny_lap(&lap),
                        Verdict::Ban => api.ban_driver(&lap, &reason),
                    }
                })
                .await;
            this.update(cx, |s, cx| {
                s.tt.acting = false;
                match done {
                    Ok(()) => {
                        s.tt.banning = false;
                        s.tt_load_runs(cx);
                        s.tt_reload(cx);
                    }
                    Err(e) => s.tt.notice = Some(e.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn tt_reason(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(state) = &self.tt.reason {
            return state.clone();
        }
        let state =
            cx.new(|cx| InputState::new(window, cx).placeholder(swofty_i18n::tr("Reason for the ban")));
        cx.subscribe(&state, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        self.tt.reason = Some(state.clone());
        state
    }

    pub(super) fn tt_pick(&mut self, pick: Pick, cx: &mut Context<Self>) {
        self.tt.pick = Some(pick);
        self.tt_reload(cx);
    }

    pub(super) fn tt_reload(&mut self, cx: &mut Context<Self>) {
        let (Some(api), Some(pick)) = (self.tt_api(), self.tt.pick.clone()) else {
            return;
        };
        self.tt.generation += 1;
        let generation = self.tt.generation;
        self.tt.rows = Load::Loading;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_executor()
                .spawn(async move { api.board(&pick.content, &pick.track, &pick.weather, None) })
                .await;
            this.update(cx, |s, cx| {
                if s.tt.generation != generation {
                    return;
                }
                s.tt.rows = match got {
                    Ok(rows) => Load::Ready(rows),
                    Err(e) => Load::Failed(e.message),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn tt_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(state) = &self.tt.search {
            return state.clone();
        }
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(swofty_i18n::tr("Search drivers")));
        self.tt.sub = Some(cx.subscribe(&state, |this, state, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.tt.query = state.read(cx).value().to_string();
                cx.notify();
            }
        }));
        self.tt.search = Some(state.clone());
        state
    }
}

#[cfg(test)]
mod tests {
    use super::{RunBoard, TtPage};

    fn board(content: &str, season: &str, track: &str, name: &str) -> RunBoard {
        RunBoard {
            content: content.to_owned(),
            season: Some(season.to_owned()).filter(|s| !s.is_empty()),
            track: track.to_owned(),
            track_name: Some(name.to_owned()).filter(|n| !n.is_empty()),
            laps: 1,
        }
    }

    fn page() -> TtPage {
        TtPage {
            run_boards: vec![
                board(
                    "swofty.swofty-seasons-2016/f1_2016",
                    "SWOFTY 2016",
                    "silverstone",
                    "Silverstone",
                ),
                board("game/f1_25", "F1 25", "silverstone", "Silverstone"),
                board("game/f1_25", "F1 25", "austria_reverse", ""),
                board("mod.fuji/season", "Fuji", "mod.fuji", "Fuji Speedway"),
                board("other.fuji/season", "", "other.fuji", "Fuji Speedway"),
                board("game/f1_25", "F1 25", "bahrain", "Bahrain"),
            ],
            ..TtPage::default()
        }
    }

    #[test]
    fn seasons_are_sorted_by_name_once_each() {
        let labels: Vec<String> = page().choices(true).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["F1 25", "Fuji", "other.fuji/season", "SWOFTY 2016"]);
    }

    #[test]
    fn tracks_sharing_a_name_show_their_code() {
        let tracks = page().choices(false);
        let labels: Vec<&str> = tracks.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "austria_reverse",
                "Bahrain",
                "Fuji Speedway (mod.fuji)",
                "Fuji Speedway (other.fuji)",
                "Silverstone"
            ]
        );
    }

    #[test]
    fn tracks_follow_the_picked_seasons() {
        let mut tt = page();
        tt.run_seasons = vec!["swofty.swofty-seasons-2016/f1_2016".to_owned()];
        let keys: Vec<String> = tt.choices(false).into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["silverstone"]);
    }

    #[test]
    fn a_season_pick_drops_tracks_it_does_not_have() {
        let mut tt = page();
        tt.run_tracks = vec!["bahrain".to_owned(), "silverstone".to_owned()];
        tt.run_seasons = vec!["swofty.swofty-seasons-2016/f1_2016".to_owned()];
        tt.keep_listed_tracks();
        assert_eq!(tt.run_tracks, ["silverstone"]);
    }

    #[test]
    fn the_button_names_the_picks() {
        let mut tt = page();
        assert_eq!(tt.filter_label(true, &tt.choices(true)), "All seasons");
        assert_eq!(tt.filter_label(false, &tt.choices(false)), "All tracks");
        tt.run_seasons = vec!["swofty.swofty-seasons-2016/f1_2016".to_owned()];
        assert_eq!(tt.filter_label(true, &tt.choices(true)), "Season: SWOFTY 2016");
        tt.run_seasons.clear();
        tt.run_tracks = vec!["mod.fuji".to_owned()];
        assert_eq!(
            tt.filter_label(false, &tt.choices(false)),
            "Track: Fuji Speedway (mod.fuji)"
        );
        tt.run_tracks
            .extend(["bahrain".to_owned(), "silverstone".to_owned()]);
        assert_eq!(tt.filter_label(false, &tt.choices(false)), "Track: 3 picked");
    }
}
