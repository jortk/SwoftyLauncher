use super::model::*;
use super::widgets::*;
use super::{Load, Target};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trn};

const RAIL_W: f32 = 190.;

impl Shell {
    pub(crate) fn online_season(&mut self, id: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let page = match self.online.seasons.get(id) {
            None | Some(Load::Idle | Load::Loading) => return self.skeleton(p, 6, 42.),
            Some(Load::Failed(e)) => {
                let e = e.clone();
                return self.failed_view(&e, cx);
            }
            Some(Load::Ready(page)) => page.clone(),
        };
        let mut items: Vec<Target> = Vec::new();
        let me = self.account_name();
        let wide = self.content_width >= 1000.;
        let mine_keys: std::collections::HashSet<BoardKey> = self.online.my_times.keys().cloned().collect();
        let mut filter = self.online.filter_tracks.clone();
        if let Some(search) = &self.online.search {
            filter.query = search.read(cx).value().to_string();
        }
        self.online.filter_tracks = filter.clone();
        let shown: Vec<TrackCard> = filter
            .apply(&page.tracks, &mine_keys)
            .into_iter()
            .cloned()
            .collect();
        let custom: Vec<TrackCard> = match &self.online.overview {
            Load::Ready(o) => o.custom.iter().filter(|t| t.key.content == id).cloned().collect(),
            _ => Vec::new(),
        };
        let custom_shown: Vec<TrackCard> = filter.apply(&custom, &mine_keys).into_iter().cloned().collect();
        let mut bylines = self
            .online_bylines(&shown.iter().chain(&custom_shown).collect::<Vec<_>>())
            .into_iter();
        let mut track_row = |this: &mut Self, t: &TrackCard, items: &mut Vec<Target>| {
            let idx = this.target_item(items, Target::Board(Box::new(t.clone()), t.name.clone()));
            let mine = this.online.my_times.get(&t.key).copied();
            this.thin_track_row(t, bylines.next().flatten(), idx, mine, p, cx)
        };
        let rows: Vec<AnyElement> = shown.iter().map(|t| track_row(self, t, &mut items)).collect();
        let custom_rows: Vec<AnyElement> = custom_shown
            .iter()
            .map(|t| track_row(self, t, &mut items))
            .collect();
        let ranking = self.rank_column(id, tr("SEASON RANKING"), &mut items, p, cx);
        self.online.items = items;
        let header = self.season_header(&page.season, p);
        let bar = self.track_filter_bar(&page.tracks, &filter, me.is_some(), p, cx);
        let rail = self.season_rail(id, p, cx);
        let main_width =
            self.content_width - 56. - 20. - RAIL_W - if wide { 20. + super::landing::RIGHT_W } else { 0. };
        let ad = self.ad_banner(crate::ads::Unit::Online, main_width, p);
        let empty = rows.is_empty() && custom_rows.is_empty();
        let (narrow_ranking, side_ranking) = if wide {
            (None, Some(ranking))
        } else {
            (
                Some(
                    div()
                        .h(px(520.))
                        .flex()
                        .flex_col()
                        .child(ranking)
                        .into_any_element(),
                ),
                None,
            )
        };
        let nothing = empty.then(|| {
            div()
                .py(px(30.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(if filter.active() {
                    tr("No track matches these filters.")
                } else if !self.online.category.is_default() {
                    tr("No laps with these filters. Change the assists or input on the Time Trial page.")
                } else {
                    tr("No tracks have laps yet.")
                })
        });
        let main = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .scroll_y("online-season")
            .pb(px(28.))
            .gap(px(14.))
            .child(header)
            .child(bar)
            .children(ad)
            .children(nothing)
            .when(!rows.is_empty(), |d| d.child(panel(p).children(rows)))
            .when(!custom_rows.is_empty(), |d| {
                d.child(section_title(tr("CUSTOM TRACKS"), p))
                    .child(panel(p).children(custom_rows))
            })
            .children(narrow_ranking);
        h_flex()
            .flex_1()
            .min_h_0()
            .px(px(28.))
            .gap(px(20.))
            .items_start()
            .child(rail)
            .child(main)
            .when(wide, |d| {
                d.child(
                    v_flex()
                        .w(px(super::landing::RIGHT_W))
                        .flex_shrink_0()
                        .h_full()
                        .pb(px(20.))
                        .children(side_ranking),
                )
            })
            .into_any_element()
    }

    fn season_rail(&self, current: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let recents: Vec<RecentSeason> = self.online.recents.list().to_vec();
        let covers: Vec<Option<String>> = match &self.online.overview {
            Load::Ready(o) => recents
                .iter()
                .map(|r| {
                    o.seasons
                        .iter()
                        .find(|s| s.id == r.id)
                        .and_then(|s| s.cover.clone())
                })
                .collect(),
            _ => vec![None; recents.len()],
        };
        v_flex()
            .w(px(RAIL_W))
            .flex_shrink_0()
            .h_full()
            .scroll_y("online-rail")
            .gap(px(4.))
            .pb(px(28.))
            .child(
                h_flex()
                    .id("rail-all")
                    .h(px(34.))
                    .px(px(10.))
                    .items_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.text_muted)
                    .hover(move |s| s.bg(p.hover).text_color(p.text))
                    .child(tr("All seasons"))
                    .on_click(cx.listener(|this, _, _, cx| this.online_jump(1, cx))),
            )
            .child(
                div()
                    .px(px(10.))
                    .pt(px(8.))
                    .pb(px(2.))
                    .child(section_title(tr("RECENTLY VIEWED"), p)),
            )
            .children(recents.into_iter().zip(covers).map(|(r, cover)| {
                let active = r.id == current;
                let (id, name) = (r.id.clone(), r.name.clone());
                h_flex()
                    .id(SharedString::from(format!("rail-{}", r.id)))
                    .h(px(44.))
                    .px(px(10.))
                    .gap(px(10.))
                    .items_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .map(|d| {
                        if active {
                            d.bg(p.selected)
                                .text_color(p.text)
                                .font_weight(FontWeight::SEMIBOLD)
                        } else {
                            d.text_color(p.text_muted)
                                .hover(move |s| s.bg(p.hover).text_color(p.text))
                        }
                    })
                    .child(self.season_icon(&r.id, cover.as_ref(), 28., p))
                    .child(div().flex_1().min_w_0().truncate().child(r.name.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.online_open_recent(id.clone(), name.clone(), cx)
                    }))
            }))
            .into_any_element()
    }

    fn season_header(&self, s: &SeasonCard, p: Palette) -> AnyElement {
        h_flex()
            .gap(px(16.))
            .items_center()
            .pt(px(4.))
            .child(self.season_icon(&s.id, s.cover.as_ref(), 64., p))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .child(s.name.clone()),
                    )
                    .child(
                        h_flex()
                            .gap(px(12.))
                            .items_center()
                            .child(div().text_size(px(13.)).text_color(p.text_muted).child(format!(
                                "{} - {}",
                                trn(s.tracks as usize, "{count} track", "{count} tracks"),
                                trn(s.drivers as usize, "{count} driver", "{count} drivers")
                            )))
                            .child(self.tt_faces(&s.top, 24., p)),
                    ),
            )
            .into_any_element()
    }

    fn track_filter_bar(
        &self,
        tracks: &[TrackCard],
        f: &TrackFilter,
        signed_in: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weathers = weathers(tracks);
        let sort = |id: &'static str, label: &'static str, which: Sort, cx: &mut Context<Self>| {
            self.filter_chip(id, label, f.sort == which, p, cx, move |t| t.sort = which)
        };
        v_flex()
            .gap(px(10.))
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .w(px(260.))
                            .children(self.online.search.as_ref().map(Input::new)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .child(tr("SORT")),
                    )
                    .child(sort("sort-name", tr("Name"), Sort::Name, cx))
                    .child(sort("sort-record", tr("Record"), Sort::Record, cx))
                    .child(sort("sort-drivers", tr("Most drivers"), Sort::Drivers, cx))
                    .child(sort("sort-laps", tr("Most laps"), Sort::Laps, cx)),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(self.filter_chip(
                        "laps-has",
                        tr("Has laps"),
                        f.laps == LapsFilter::Has,
                        p,
                        cx,
                        |t| {
                            t.laps = if t.laps == LapsFilter::Has {
                                LapsFilter::All
                            } else {
                                LapsFilter::Has
                            }
                        },
                    ))
                    .child(self.filter_chip(
                        "laps-none",
                        tr("No laps"),
                        f.laps == LapsFilter::None,
                        p,
                        cx,
                        |t| {
                            t.laps = if t.laps == LapsFilter::None {
                                LapsFilter::All
                            } else {
                                LapsFilter::None
                            }
                        },
                    ))
                    .children(weathers.into_iter().map(|w| {
                        let on = f.weather.as_deref() == Some(w.as_str());
                        let label = crate::format::title_case(&w);
                        self.filter_chip(format!("weather-{w}"), label, on, p, cx, move |t| {
                            t.weather = if on { None } else { Some(w.clone()) }
                        })
                    }))
                    .when(signed_in, |d| {
                        d.child(self.filter_chip("mine", tr("My times"), f.mine, p, cx, |t| t.mine = !t.mine))
                    }),
            )
            .into_any_element()
    }
}
