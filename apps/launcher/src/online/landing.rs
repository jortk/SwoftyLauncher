use super::model::*;
use super::widgets::*;
use super::{Load, Target};
use crate::format::lap_time;
use crate::scale::Hit;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use gpui_kit::SharedString;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf, trn};

pub(super) const RIGHT_W: f32 = 360.;

impl Shell {
    pub(crate) fn online_landing(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.online.overview {
            Load::Idle | Load::Loading => return self.skeleton(p, 5, 76.),
            Load::Failed(e) => {
                let e = e.clone();
                return self.failed_view(&e, cx);
            }
            Load::Ready(o) => match landing_empty(o, &self.online.category) {
                Some(LandingEmpty::NothingYet) => {
                    self.online.items = Vec::new();
                    return self.nothing_yet();
                }
                Some(LandingEmpty::Filtered) => {
                    self.online.items = Vec::new();
                    return v_flex()
                        .flex_1()
                        .min_h_0()
                        .px(px(28.))
                        .gap(px(10.))
                        .child(h_flex().h(px(34.)).items_center().child(self.tt_category_bar(
                            self.online.category,
                            false,
                            p,
                            cx,
                        )))
                        .child(self.no_laps_filtered(cx))
                        .into_any_element();
                }
                None => {}
            },
        }
        let Load::Ready(o) = &self.online.overview else {
            return div().into_any_element();
        };
        let o = o.clone();
        let mut items: Vec<Target> = Vec::new();
        let wide = self.content_width >= 860.;
        let seasons: Vec<AnyElement> = o
            .seasons
            .iter()
            .map(|s| {
                let idx = self.target_item(&mut items, Target::Season(s.id.clone(), s.name.clone()));
                self.season_row(s, idx, p, cx)
            })
            .collect();
        self.online_rank_ensure("all", cx);
        let ranking = self.rank_column("all", tr("GLOBAL RANKING"), &mut items, p, cx);
        self.online.items = items;
        let left_width = self.content_width - 56. - if wide { 20. + RIGHT_W } else { 0. };
        let ad = self.ad_banner(crate::ads::Unit::Online, left_width, p);
        let left = v_flex()
            .gap(px(10.))
            .child(h_flex().h(px(34.)).items_center().child(self.tt_category_bar(
                self.online.category,
                false,
                p,
                cx,
            )))
            .children(ad)
            .child(section_title(tr("SEASONS"), p))
            .child(panel(p).children(seasons));
        if wide {
            h_flex()
                .flex_1()
                .min_h_0()
                .px(px(28.))
                .gap(px(20.))
                .items_start()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .scroll_y("online-seasons")
                        .pb(px(28.))
                        .child(left),
                )
                .child(
                    v_flex()
                        .w(px(RIGHT_W))
                        .flex_shrink_0()
                        .h_full()
                        .pb(px(20.))
                        .child(ranking),
                )
                .into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .scroll_y("online-landing")
                .px(px(28.))
                .pb(px(28.))
                .gap(px(20.))
                .child(left)
                .child(div().h(px(520.)).flex().flex_col().child(ranking))
                .into_any_element()
        }
    }

    fn season_row(&self, s: &SeasonCard, idx: usize, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let (id, name) = (s.id.clone(), s.name.clone());
        h_flex()
            .id(SharedString::from(format!("season-{}", s.id)))
            .h(px(84.))
            .px(px(16.))
            .gap(px(16.))
            .items_center()
            .border_b_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .when(self.online.ringed(idx), |d| d.bg(p.selected))
            .hover(move |st| st.bg(p.hover))
            .child(self.season_icon(&s.id, s.cover.as_ref(), 56., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(s.name.clone()),
                    )
                    .child(div().text_size(px(12.)).text_color(p.text_muted).child(format!(
                        "{} - {}",
                        trn(s.tracks as usize, "{count} track", "{count} tracks"),
                        trn(s.drivers as usize, "{count} driver", "{count} drivers")
                    ))),
            )
            .child(self.tt_faces(&s.top, 30., p))
            .child(
                Icon::new(IconName::ChevronRight)
                    .size(px(16.))
                    .text_color(p.text_placeholder),
            )
            .on_click(
                cx.listener(move |this, _, _, cx| {
                    this.online_go(Target::Season(id.clone(), name.clone()), cx)
                }),
            )
            .into_any_element()
    }

    pub(super) fn rank_column(
        &mut self,
        scope: &str,
        title: &'static str,
        items: &mut Vec<Target>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let feed = self.online.feeds.get(scope).cloned().unwrap_or_default();
        let me = self
            .account_name()
            .and_then(|_| self.online.me.clone())
            .and_then(|m| m.entry);
        for r in feed.rows.iter().take(3) {
            items.push(Target::Profile(r.driver.clone()));
        }
        let header = div()
            .h(px(46.))
            .flex()
            .items_center()
            .justify_center()
            .child(section_title(rank_title(title, &self.online.category), p));
        let body: AnyElement = if !feed.started {
            v_flex()
                .children((0..8).map(|_| {
                    div()
                        .h(px(38.))
                        .mx(px(14.))
                        .my(px(2.))
                        .rounded(px(8.))
                        .bg(p.elevated)
                }))
                .into_any_element()
        } else if let (Some(e), true) = (&feed.error, feed.rows.is_empty()) {
            v_flex()
                .p(px(18.))
                .gap(px(10.))
                .items_center()
                .child(div().text_size(px(12.)).text_color(p.text_muted).child(e.clone()))
                .child(
                    gpui_kit::component::button::Button::new(SharedString::from(format!(
                        "rank-retry-{scope}"
                    )))
                    .outline()
                    .tap_sm()
                    .label(tr("Try again"))
                    .on_click({
                        let scope = scope.to_owned();
                        cx.listener(move |this, _, _, cx| {
                            this.online.feeds.remove(&scope);
                            cx.notify();
                        })
                    }),
                )
                .into_any_element()
        } else if feed.rows.is_empty() {
            div()
                .p(px(18.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(tr(
                    "No ranking yet. A track counts once three drivers have set a lap on it.",
                ))
                .into_any_element()
        } else {
            let handle = self.online.handles.entry(scope.to_owned()).or_default().clone();
            let key = scope.to_owned();
            let count = chunk_count(feed.rows.len());
            div()
                .flex_1()
                .min_h_0()
                .child(
                    uniform_list(
                        SharedString::from(format!("rank-list-{scope}")),
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            let feed = this.online.feeds.get(&key).cloned().unwrap_or_default();
                            if let Some(off) = feed.wants_more(3 + range.end * CHUNK + 3) {
                                this.online_rank_load(&key, off, cx);
                            }
                            let me_id = this.online.me.as_ref().map(|m| m.driver.id.clone());
                            range
                                .map(|chunk| {
                                    let rows: Vec<AnyElement> = chunk_slots(chunk, feed.rows.len())
                                        .into_iter()
                                        .map(|slot| match slot {
                                            Slot::Top(i) => {
                                                let r = &feed.rows[i];
                                                let mine = me_id.as_deref() == Some(r.driver.id.as_str());
                                                this.top_rank_row(r, usize::MAX, mine, p, cx)
                                            }
                                            Slot::Row(i) => {
                                                let r = &feed.rows[i];
                                                let mine = me_id.as_deref() == Some(r.driver.id.as_str());
                                                this.rank_row(r, usize::MAX, mine, p, cx)
                                            }
                                            Slot::Footer => this.rank_footer(&feed, p),
                                        })
                                        .collect();
                                    v_flex().h(px(CHUNK_H)).w_full().children(rows).into_any_element()
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .track_scroll(&handle),
                )
                .into_any_element()
        };
        let pinned = me.map(|m| {
            h_flex()
                .h(px(46.))
                .px(px(14.))
                .gap(px(10.))
                .items_center()
                .bg(p.selected)
                .border_t_1()
                .border_color(p.border)
                .child(self.tt_face(&m.driver, 26., p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(trf!("You: #{rank}", rank = m.rank)),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::BOLD)
                        .child(format!("{} {}", score_text(m.points), tr("pts"))),
                )
        });
        panel(p)
            .flex_1()
            .min_h_0()
            .child(header)
            .child(body)
            .children(pinned)
            .into_any_element()
    }

    fn rank_footer(&self, feed: &Feed, p: Palette) -> AnyElement {
        let text: AnyElement = if feed.error.is_some() {
            div()
                .text_size(px(12.))
                .text_color(p.error)
                .child(tr("Couldn't load more"))
                .into_any_element()
        } else if feed.finished() {
            div()
                .text_size(px(11.))
                .text_color(p.text_placeholder)
                .child(tr("End of the ranking"))
                .into_any_element()
        } else {
            div()
                .h(px(18.))
                .w(px(180.))
                .rounded(px(6.))
                .bg(p.elevated)
                .into_any_element()
        };
        div()
            .h(px(38.))
            .flex()
            .items_center()
            .justify_center()
            .child(text)
            .into_any_element()
    }

    fn top_rank_row(
        &self,
        r: &RankEntry,
        idx: usize,
        mine: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let driver = r.driver.clone();
        let colour = medal(r.rank, &p);
        h_flex()
            .id(SharedString::from(format!("top-{idx}-{}", r.driver.id)))
            .h(px(TOP_ROW_H))
            .px(px(14.))
            .gap(px(12.))
            .items_center()
            .bg(colour.opacity(if mine { 0.18 } else { 0.08 }))
            .border_b_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .when(self.online.ringed(idx), |d| d.border_1().border_color(p.accent))
            .hover(move |s| s.bg(colour.opacity(0.16)))
            .child(
                div()
                    .w(px(30.))
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(colour)
                    .child(format!("#{}", r.rank)),
            )
            .child(self.tt_face(&r.driver, 42., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(r.driver.name.clone()),
                    )
                    .child(div().text_size(px(11.)).text_color(p.text_muted).child(trf!(
                        "{wins} wins - {podiums} podiums",
                        wins = r.wins,
                        podiums = r.podiums
                    ))),
            )
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::BOLD)
                    .child(score_text(r.points)),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.online_go(Target::Profile(driver.clone()), cx)))
            .into_any_element()
    }

    pub(super) fn rank_row(
        &self,
        r: &RankEntry,
        idx: usize,
        mine: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let driver = r.driver.clone();
        h_flex()
            .id(SharedString::from(format!("rank-{idx}-{}", r.driver.id)))
            .w_full()
            .h(px(ROW_H))
            .px(px(14.))
            .gap(px(10.))
            .items_center()
            .border_b_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .when(mine, |d| d.bg(p.selected))
            .when(self.online.ringed(idx), |d| d.border_1().border_color(p.accent))
            .hover(move |s| s.bg(p.hover))
            .child(
                div()
                    .w(px(30.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(medal(r.rank, &p))
                    .child(r.rank.to_string()),
            )
            .child(self.tt_face(&r.driver, 22., p))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(r.driver.name.clone()),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(score_text(r.points)),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.online_go(Target::Profile(driver.clone()), cx)))
            .into_any_element()
    }

    pub(super) fn thin_track_row(
        &self,
        t: &TrackCard,
        byline: Option<String>,
        idx: usize,
        mine: Option<u64>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let card = t.clone();
        let label = t.name.clone();
        h_flex()
            .id(SharedString::from(format!(
                "track-{idx}-{}-{}",
                t.key.track, t.key.weather
            )))
            .h(px(42.))
            .px(px(14.))
            .gap(px(12.))
            .items_center()
            .border_b_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .when(self.online.ringed(idx), |d| d.bg(p.selected))
            .hover(move |s| s.bg(p.hover))
            .child(self.layout_art(t.layout.as_ref(), 28., p.accent_hover, p))
            .child(
                v_flex()
                    .w(px(200.))
                    .flex_shrink_0()
                    .min_w_0()
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(t.name.clone()),
                            )
                            .children(self.flag_chip(&t.flag, p)),
                    )
                    .children(byline.map(|b| {
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(p.text_muted)
                            .child(b)
                    })),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .items_center()
                    .children(t.record.as_ref().map(|r| {
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.accent_hover)
                                    .child(lap_time(r.time_ms)),
                            )
                            .child(self.tt_face(&r.driver, 20., p))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .child(r.driver.name.clone()),
                            )
                    }))
                    .when(t.record.is_none(), |d| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_placeholder)
                                .child(tr("No time yet")),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(150.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(format!(
                        "{} - {}",
                        trn(t.drivers as usize, "{count} driver", "{count} drivers"),
                        trn(t.laps as usize, "{count} lap", "{count} laps")
                    )),
            )
            .child(
                div()
                    .w(px(84.))
                    .flex_shrink_0()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(mine.map(lap_time).unwrap_or_default()),
            )
            .child(
                Icon::new(IconName::ChevronRight)
                    .size(px(14.))
                    .text_color(p.text_placeholder),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.online_go(Target::Board(Box::new(card.clone()), label.clone()), cx)
            }))
            .into_any_element()
    }
}

fn rank_title(title: &str, cat: &Category) -> String {
    match cat.is_default() {
        true => title.to_owned(),
        false => format!("{title} \u{b7} {}", cat.suffix()),
    }
}
