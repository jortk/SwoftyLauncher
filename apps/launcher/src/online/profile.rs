use super::model::*;
use super::widgets::*;
use super::{Load, Target};
use crate::format::{iso_date, lap_time};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf};

impl Shell {
    pub(crate) fn online_profile(&mut self, id: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let page = match self.online.profiles.get(id) {
            None | Some(Load::Idle | Load::Loading) => return self.skeleton(p, 4, 100.),
            Some(Load::Failed(e)) => {
                let e = e.clone();
                return self.failed_view(&e, cx);
            }
            Some(Load::Ready(page)) => page.clone(),
        };
        let mut items: Vec<Target> = Vec::new();
        let cards: Vec<TrackCard> = page
            .bests
            .iter()
            .map(|b| TrackCard::bare(b.key.clone(), b.track.clone()))
            .collect();
        let bylines = self.online_bylines(&cards.iter().collect::<Vec<_>>());
        let bests: Vec<AnyElement> = page
            .bests
            .iter()
            .zip(cards)
            .zip(bylines)
            .flat_map(|((b, card), byline)| {
                let idx = self.target_item(&mut items, Target::Board(Box::new(card), b.track.clone()));
                let mut out = vec![self.best_row(b, byline, idx, p, cx)];
                if SHOW_SETUPS
                    && self.online.setup_open.as_deref() == Some(b.lap_id.as_str())
                    && let Some(setup) = &b.setup
                {
                    out.push(self.setup_panel(setup, None, p, cx));
                }
                out
            })
            .collect();
        self.online.items = items;
        let stats = page.global.as_ref().map(|g| {
            h_flex()
                .gap(px(12.))
                .child(stat_card(
                    tr("GLOBAL RANK"),
                    format!("#{}", g.rank),
                    medal(g.rank, &p),
                    p,
                ))
                .child(stat_card(tr("SCORE"), score_text(g.points), p.text, p))
                .child(stat_card(tr("WINS"), g.wins.to_string(), p.text, p))
                .child(stat_card(tr("PODIUMS"), g.podiums.to_string(), p.text, p))
        });
        let standings: Vec<AnyElement> = page
            .seasons
            .iter()
            .map(|s| {
                h_flex()
                    .h(px(40.))
                    .px(px(14.))
                    .gap(px(10.))
                    .items_center()
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(div().flex_1().text_size(px(13.)).child(s.season.clone()))
                    .child(
                        div()
                            .w(px(50.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(medal(s.rank, &p))
                            .child(format!("#{}", s.rank)),
                    )
                    .child(div().w(px(70.)).text_size(px(13.)).child(score_text(s.points)))
                    .into_any_element()
            })
            .collect();
        let recent: Vec<AnyElement> = page
            .recent
            .iter()
            .map(|r| {
                h_flex()
                    .h(px(38.))
                    .px(px(14.))
                    .gap(px(12.))
                    .items_center()
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_size(px(13.))
                            .child(format!("{} - {}", r.season, r.track)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(lap_time(r.time_ms)),
                    )
                    .child(
                        div()
                            .w(px(84.))
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(iso_date(&r.date).to_owned()),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex()
            .flex_1()
            .min_h_0()
            .scroll_y("online-profile")
            .px(px(28.))
            .pb(px(28.))
            .gap(px(18.))
            .child(
                panel(p).child(
                    h_flex()
                        .p(px(22.))
                        .gap(px(22.))
                        .items_center()
                        .child(self.tt_face(&page.driver, 96., p))
                        .child(
                            v_flex()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .text_size(px(28.))
                                        .font_weight(FontWeight::BOLD)
                                        .child(page.driver.name.clone()),
                                )
                                .children(stats)
                                .when(page.global.is_none(), |d| {
                                    d.child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(p.text_muted)
                                            .child(tr("No laps yet.")),
                                    )
                                }),
                        ),
                ),
            )
            .child(
                h_flex()
                    .gap(px(18.))
                    .items_start()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(10.))
                            .child(section_title(tr("BEST LAPS"), p))
                            .child(panel(p).children(bests)),
                    )
                    .child(
                        v_flex()
                            .w(px(340.))
                            .flex_shrink_0()
                            .gap(px(18.))
                            .child(
                                v_flex()
                                    .gap(px(10.))
                                    .child(section_title(tr("SEASON RANKS"), p))
                                    .child(panel(p).children(standings)),
                            )
                            .child(
                                v_flex()
                                    .gap(px(10.))
                                    .child(section_title(tr("RECENT LAPS"), p))
                                    .child(panel(p).children(recent)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn best_row(
        &self,
        b: &BestLap,
        byline: Option<String>,
        idx: usize,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = Target::Board(
            Box::new(TrackCard::bare(b.key.clone(), b.track.clone())),
            b.track.clone(),
        );
        h_flex()
            .id(SharedString::from(format!("best-{idx}")))
            .h(px(42.))
            .px(px(14.))
            .gap(px(12.))
            .items_center()
            .border_b_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .when(self.online.ringed(idx), |d| d.bg(p.selected))
            .hover(move |s| s.bg(p.hover))
            .child(
                div()
                    .w(px(44.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(medal(b.position, &p))
                    .child(trf!("P{position}", position = b.position)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(b.track.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(match byline {
                                Some(by) => format!("{} \u{b7} {by}", b.season),
                                None => b.season.clone(),
                            }),
                    ),
            )
            .child(div().font_weight(FontWeight::BOLD).child(lap_time(b.time_ms)))
            .child(
                div()
                    .w(px(84.))
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(iso_date(&b.date).to_owned()),
            )
            .children(
                SHOW_SETUPS
                    .then(|| self.setup_button(&format!("best-{idx}"), &b.lap_id, b.setup.is_some(), cx)),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.online_go(target.clone(), cx)))
            .into_any_element()
    }
}

fn stat_card(label: &'static str, value: String, colour: Hsla, p: Palette) -> Div {
    v_flex()
        .min_w(px(96.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(p.elevated)
        .child(
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text_placeholder)
                .child(label),
        )
        .child(
            div()
                .text_size(px(22.))
                .font_weight(FontWeight::BOLD)
                .text_color(colour)
                .child(value),
        )
}
