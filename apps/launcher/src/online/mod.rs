mod api;
mod board;
mod images;
mod landing;
mod mock;
mod model;
mod profile;
mod season;
mod widgets;

use crate::shell::Shell;
use api::{BoardsApi, HttpBoards};
use gpui_kit::component::input::InputState;
use gpui_kit::*;
use images::{Art, Images};
use model::*;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) enum Load<T> {
    Idle,
    Loading,
    Ready(T),
    Failed(OnlineError),
}

impl<T> Load<T> {
    fn from(result: Result<T, OnlineError>) -> Self {
        match result {
            Ok(v) => Self::Ready(v),
            Err(e) => Self::Failed(e),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Season(String, String),
    Board(Box<TrackCard>, String),
    Profile(Driver),
}

pub(crate) struct Online {
    api: Arc<dyn BoardsApi>,
    watch_notice: Option<(bool, String)>,
    watch_lap: Option<String>,
    trail: Trail,
    overview: Load<Overview>,
    seasons: HashMap<String, Load<SeasonPage>>,
    profiles: HashMap<String, Load<Profile>>,
    board: Option<(BoardKey, Load<BoardPage>)>,
    category: Category,
    board_category: Option<Category>,
    more: bool,
    images: Images,
    data: std::path::PathBuf,
    recents: Recents,
    filter_tracks: TrackFilter,
    search: Option<Entity<InputState>>,
    my_times: HashMap<BoardKey, u64>,
    feeds: HashMap<String, Feed>,
    handles: HashMap<String, UniformListScrollHandle>,
    me: Option<Me>,
    setup_open: Option<String>,
    setup_compare: bool,
    cursor: Option<usize>,
    items: Vec<Target>,
    epoch: u64,
}

impl Online {
    pub(crate) fn new(data: &std::path::Path) -> Self {
        let api: Arc<dyn BoardsApi> = match mock_mode() {
            Some(mode) => Arc::new(mock::MockBoards { mode }),
            None => Arc::new(HttpBoards::new()),
        };
        Self {
            api,
            watch_notice: None,
            watch_lap: None,
            trail: Trail::new(),
            overview: Load::Idle,
            seasons: HashMap::new(),
            profiles: HashMap::new(),
            board: None,
            category: Category::load(data),
            board_category: None,
            more: false,
            images: Images::new(data),
            data: data.to_path_buf(),
            recents: Recents::load(data),
            filter_tracks: TrackFilter::default(),
            search: None,
            my_times: HashMap::new(),
            feeds: HashMap::new(),
            handles: HashMap::new(),
            me: None,
            setup_open: None,
            setup_compare: false,
            cursor: None,
            items: Vec::new(),
            epoch: 0,
        }
    }
}

fn mock_mode() -> Option<mock::Mode> {
    if cfg!(debug_assertions) {
        mock::Mode::from_env()
    } else {
        None
    }
}

fn art_urls<'a>(
    drivers: impl IntoIterator<Item = &'a Driver>,
    extra: impl IntoIterator<Item = Option<&'a String>>,
) -> Vec<String> {
    drivers
        .into_iter()
        .filter_map(|d| d.avatar.clone())
        .chain(extra.into_iter().flatten().cloned())
        .collect()
}

impl Shell {
    pub(crate) fn online_enter(&mut self, cx: &mut Context<Self>) {
        if matches!(self.online.overview, Load::Idle) {
            self.online_reload(cx);
        }
    }

    pub(crate) fn online_reload(&mut self, cx: &mut Context<Self>) {
        self.online.epoch += 1;
        let epoch = self.online.epoch;
        self.online.overview = Load::Loading;
        self.online.seasons.clear();
        self.online.profiles.clear();
        self.online.feeds.clear();
        self.online
            .api
            .set_token(self.mp.as_ref().map(|m| m.token.clone()));
        let me_api = self.online.api.clone();
        cx.spawn(async move |this, cx| {
            let me = cx.background_executor().spawn(async move { me_api.me() }).await;
            this.update(cx, |s, cx| {
                s.online.me = me.ok().flatten();
                cx.notify();
            })
            .ok();
        })
        .detach();
        let api = self.online.api.clone();
        let cat = self.online.category;
        cx.spawn(async move |this, cx| {
            let overview = cx
                .background_executor()
                .spawn(async move { api.overview(&cat) })
                .await;
            this.update(cx, |s, cx| {
                if s.online.epoch != epoch {
                    return;
                }
                if let Ok(o) = &overview {
                    let urls = art_urls(
                        o.global
                            .iter()
                            .map(|r| &r.driver)
                            .chain(o.seasons.iter().flat_map(|s| &s.top)),
                        o.seasons
                            .iter()
                            .map(|s| s.cover.as_ref())
                            .chain(o.custom.iter().map(|t| t.layout.as_ref())),
                    );
                    s.online_prefetch(urls, cx);
                }
                let me = s.account_name();
                if let (Ok(o), Some(me)) = (&overview, me)
                    && let Some(r) = o.global.iter().find(|r| r.driver.name == me)
                {
                    let id = r.driver.id.clone();
                    s.online_load_profile(&id, cx);
                }
                if let (true, Ok(o)) = (cfg!(debug_assertions), &overview)
                    && let Ok(start) = std::env::var("SWOFTY_ONLINE_START")
                    && let Some(first) = o.seasons.first()
                {
                    let (id, name) = (first.id.clone(), first.name.clone());
                    s.online.overview = Load::from(overview.clone());
                    s.online_go(Target::Season(id.clone(), name), cx);
                    let board = match start.as_str() {
                        "board" => Some(TrackCard::bare(
                            BoardKey {
                                content: id,
                                track: "bahrain".into(),
                                weather: "dry".into(),
                            },
                            "Bahrain".into(),
                        )),
                        "custom" => o.custom.first().cloned(),
                        _ => None,
                    };
                    if let Some(card) = board {
                        let name = card.name.clone();
                        s.online_go(Target::Board(Box::new(card), name), cx);
                    }
                    return;
                }
                s.online.overview = Load::from(overview);
                cx.notify();
            })
            .ok();
        })
        .detach();
        match self.online.trail.current().clone() {
            Screen::Landing => {}
            Screen::Season(id) => self.online_load_season(&id, cx),
            Screen::Board(key) => self.online_load_board(key, 0, cx),
            Screen::Profile(id) => self.online_load_profile(&id, cx),
        }
        cx.notify();
    }

    fn online_prefetch(&mut self, urls: Vec<String>, cx: &mut Context<Self>) {
        let dir = self.online.images.dir();
        for url in self.online.images.wanted(urls) {
            let dir = dir.clone();
            cx.spawn(async move |this, cx| {
                let u = url.clone();
                let art: Option<Art> = cx
                    .background_executor()
                    .spawn(async move { images::load(&dir, &u) })
                    .await;
                this.update(cx, |s, cx| {
                    s.online.images.finish(url, art);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    pub(crate) fn online_rank_ensure(&mut self, scope: &str, cx: &mut Context<Self>) {
        let feed = self.online.feeds.get(scope);
        if feed.is_none_or(|f| !f.started && !f.loading) {
            self.online_rank_load(scope, 0, cx);
        }
    }

    pub(crate) fn online_rank_load(&mut self, scope: &str, offset: u32, cx: &mut Context<Self>) {
        let feed = self.online.feeds.entry(scope.to_owned()).or_default();
        if feed.loading {
            return;
        }
        feed.loading = true;
        let (api, key, epoch) = (self.online.api.clone(), scope.to_owned(), self.online.epoch);
        let cat = self.online.category;
        cx.spawn(async move |this, cx| {
            let k = key.clone();
            let page = cx
                .background_executor()
                .spawn(async move { api.rankings(&k, &cat, offset) })
                .await;
            this.update(cx, |s, cx| {
                if s.online.epoch != epoch {
                    return;
                }
                if let Ok(p) = &page {
                    let urls = art_urls(p.rows.iter().map(|r| &r.driver), []);
                    s.online_prefetch(urls, cx);
                }
                let feed = s.online.feeds.entry(key).or_default();
                match page {
                    Ok(p) => feed.apply(offset, p),
                    Err(e) => feed.failed(e.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn online_load_season(&mut self, id: &str, cx: &mut Context<Self>) {
        if matches!(self.online.seasons.get(id), Some(Load::Ready(_) | Load::Loading)) {
            return;
        }
        self.online.seasons.insert(id.to_owned(), Load::Loading);
        self.online_rank_ensure(id, cx);
        let (api, key, epoch) = (self.online.api.clone(), id.to_owned(), self.online.epoch);
        let cat = self.online.category;
        cx.spawn(async move |this, cx| {
            let k = key.clone();
            let page = cx
                .background_executor()
                .spawn(async move { api.season(&k, &cat) })
                .await;
            this.update(cx, |s, cx| {
                if s.online.epoch != epoch {
                    return;
                }
                if let Ok(p) = &page {
                    let urls = art_urls(
                        p.ranking
                            .iter()
                            .map(|r| &r.driver)
                            .chain(p.tracks.iter().flat_map(|t| t.podium.iter().map(|e| &e.driver)))
                            .chain(p.season.top.iter()),
                        p.tracks
                            .iter()
                            .map(|t| t.layout.as_ref())
                            .chain([p.season.cover.as_ref()]),
                    );
                    s.online_prefetch(urls, cx);
                }
                s.online.seasons.insert(key, Load::from(page));
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn online_load_board(&mut self, key: BoardKey, offset: u32, cx: &mut Context<Self>) {
        if offset == 0 {
            self.online.board = Some((key.clone(), Load::Loading));
        } else {
            if self.online.more {
                return;
            }
            self.online.more = true;
        }
        let (api, filter, epoch) = (
            self.online.api.clone(),
            self.online.board_category.unwrap_or(self.online.category),
            self.online.epoch,
        );
        cx.spawn(async move |this, cx| {
            let k = key.clone();
            let page = cx
                .background_executor()
                .spawn(async move { api.board(&k, &filter, offset) })
                .await;
            this.update(cx, |s, cx| {
                s.online.more = false;
                if s.online.epoch != epoch || s.online.board.as_ref().map(|b| &b.0) != Some(&key) {
                    return;
                }
                if let Ok(p) = &page {
                    let urls = art_urls(
                        p.rows
                            .iter()
                            .map(|r| &r.driver)
                            .chain(p.history.iter().map(|h| &h.driver)),
                        [p.track.layout.as_ref()],
                    );
                    s.online_prefetch(urls, cx);
                }
                match (offset, page, s.online.board.as_mut()) {
                    (0, page, Some(slot)) => slot.1 = Load::from(page),
                    (_, Ok(next), Some((_, Load::Ready(cur)))) => {
                        cur.total = next.total;
                        cur.rows.extend(next.rows);
                    }
                    _ => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn online_load_profile(&mut self, id: &str, cx: &mut Context<Self>) {
        if matches!(self.online.profiles.get(id), Some(Load::Ready(_) | Load::Loading)) {
            return;
        }
        self.online.profiles.insert(id.to_owned(), Load::Loading);
        let (api, key, epoch) = (self.online.api.clone(), id.to_owned(), self.online.epoch);
        let cat = self.online.category;
        cx.spawn(async move |this, cx| {
            let k = key.clone();
            let page = cx
                .background_executor()
                .spawn(async move { api.profile(&k, &cat) })
                .await;
            this.update(cx, |s, cx| {
                if s.online.epoch != epoch {
                    return;
                }
                if let Ok(p) = &page {
                    s.online_prefetch(art_urls([&p.driver], p.bests.iter().map(|_| None)), cx);
                }
                if let Ok(p) = &page
                    && s.account_name().as_deref() == Some(p.driver.name.as_str())
                {
                    s.online.my_times = p.bests.iter().map(|b| (b.key.clone(), b.time_ms)).collect();
                }
                s.online.profiles.insert(key, Load::from(page));
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn online_go(&mut self, target: Target, cx: &mut Context<Self>) {
        self.online.cursor = None;
        match target {
            Target::Season(id, name) => {
                self.online.recents.visit(&id, &name);
                self.online.recents.save(&self.online.data);
                self.online.filter_tracks = TrackFilter::default();
                self.online.trail.push(Screen::Season(id.clone()), name);
                self.online_load_season(&id, cx);
            }
            Target::Board(card, label) => {
                let key = card.key.clone();
                self.online.trail.push(Screen::Board(key.clone()), label);
                self.online.board_category = None;
                self.online_load_board(key, 0, cx);
            }
            Target::Profile(driver) => {
                self.online
                    .trail
                    .push(Screen::Profile(driver.id.clone()), driver.name.clone());
                self.online_load_profile(&driver.id, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn online_open_recent(&mut self, id: String, name: String, cx: &mut Context<Self>) {
        self.online.cursor = None;
        self.online.recents.visit(&id, &name);
        self.online.recents.save(&self.online.data);
        self.online.filter_tracks = TrackFilter::default();
        self.online.trail.jump(1);
        self.online.trail.push(Screen::Season(id.clone()), name);
        self.online_load_season(&id, cx);
        cx.notify();
    }

    pub(crate) fn online_back(&mut self, cx: &mut Context<Self>) {
        self.online.cursor = None;
        self.online.trail.back();
        self.online_reenter(cx);
    }

    pub(crate) fn online_jump(&mut self, depth: usize, cx: &mut Context<Self>) {
        self.online.cursor = None;
        self.online.trail.jump(depth);
        self.online_reenter(cx);
    }

    fn online_reenter(&mut self, cx: &mut Context<Self>) {
        match self.online.trail.current().clone() {
            Screen::Board(key) if self.online.board.as_ref().map(|b| &b.0) != Some(&key) => {
                self.online_load_board(key, 0, cx);
            }
            Screen::Season(id) => self.online_load_season(&id, cx),
            Screen::Profile(id) => self.online_load_profile(&id, cx),
            _ => {}
        }
        cx.notify();
    }

    pub(crate) fn online_set_category(&mut self, cat: Category, cx: &mut Context<Self>) {
        if self.online.category == cat {
            return;
        }
        self.online.category = cat;
        cat.save(&self.online.data);
        self.online.board_category = None;
        self.online.board = None;
        self.online.overview = Load::Idle;
        self.online_reload(cx);
    }

    pub(crate) fn online_set_board_category(&mut self, cat: Category, cx: &mut Context<Self>) {
        self.online.board_category = (cat != self.online.category).then_some(cat);
        if let Screen::Board(key) = self.online.trail.current().clone() {
            self.online_load_board(key, 0, cx);
        }
    }

    pub(crate) fn board_category(&self) -> Category {
        self.online.board_category.unwrap_or(self.online.category)
    }

    /// Ask the running game to play lap `id` as the ghost in Time Trial.
    pub(crate) fn online_add_to_game(&mut self, id: String, cx: &mut Context<Self>) {
        let (Some(api), Some(_)) = (
            self.mp
                .as_ref()
                .map(|s| swofty_core::timetrial::api::Api::new(&s.token)),
            self.install.as_ref(),
        ) else {
            return;
        };
        self.online.watch_notice = Some((true, swofty_i18n::tr("Adding the replay to the game…").to_owned()));
        self.online.watch_lap = Some(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let lap = id.clone();
            let done = cx
                .background_executor()
                .spawn(async move {
                    swofty_core::timetrial::game_replay::add(&api, &swofty_core::paths::data_dir(), &lap)
                })
                .await;
            this.update(cx, |s, cx| {
                s.online.watch_notice = Some(match done {
                    Ok(()) => (
                        true,
                        swofty_i18n::tr("Added. Open Theatre in the game to watch it.").to_owned(),
                    ),
                    Err(e) => {
                        crate::app_log::action("add a lap's replay to the game").failed(&e);
                        (
                            false,
                            if e == swofty_core::timetrial::game_replay::NO_REPLAY {
                                swofty_i18n::tr("This lap has no game replay").to_owned()
                            } else {
                                e
                            },
                        )
                    }
                });
                s.online.watch_lap = Some(id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn online_watch_notice(&self) -> Option<&(bool, String)> {
        self.online.watch_notice.as_ref()
    }

    /// The notice of "Add to game" for lap `id`, if that was the last lap it ran for.
    pub(crate) fn online_replay_notice(&self, id: &str) -> Option<&(bool, String)> {
        self.online
            .watch_notice
            .as_ref()
            .filter(|_| self.online.watch_lap.as_deref() == Some(id))
    }

    pub(crate) fn online_has_cursor(&self) -> bool {
        self.online.cursor.is_some()
    }

    pub(crate) fn online_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let n = self.online.items.len();
        if n == 0 {
            return key == "escape" && self.online.trail.depth() > 1 && {
                self.online_back(cx);
                true
            };
        }
        let at = self.online.cursor;
        match key {
            "down" | "right" => self.online.cursor = Some(at.map_or(0, |i| (i + 1).min(n - 1))),
            "up" | "left" => self.online.cursor = Some(at.map_or(0, |i| i.saturating_sub(1))),
            "enter" => {
                if let Some(t) = at.and_then(|i| self.online.items.get(i)).cloned() {
                    self.online_go(t, cx);
                }
                return true;
            }
            "escape" | "backspace" => {
                if self.online.trail.depth() > 1 {
                    self.online_back(cx);
                    return true;
                }
                return false;
            }
            _ => return false,
        }
        cx.notify();
        true
    }
}

impl Shell {
    pub(crate) fn online_page(
        &mut self,
        p: crate::theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::scale::px;
        use gpui_kit::component::v_flex;
        self.online_enter(cx);
        if self.online.search.is_none() {
            self.online.search =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder(swofty_i18n::tr("Search tracks"))));
        }
        let crumbs = self.crumb_bar(p, cx);
        let banner = self.sign_in_banner(p, cx);
        let body = match self.online.trail.current().clone() {
            Screen::Landing => self.online_landing(p, cx),
            Screen::Season(id) => self.online_season(&id, p, cx),
            Screen::Board(_) => self.online_board(p, cx),
            Screen::Profile(id) => self.online_profile(&id, p, cx),
        };
        let _ = px;
        v_flex()
            .size_full()
            .child(crumbs)
            .children(banner)
            .child(v_flex().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}
