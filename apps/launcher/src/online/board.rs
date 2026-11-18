use super::model::*;
use super::widgets::*;
use super::{Load, Target};
use crate::format::{iso_date, lap_time, sector_time};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::tr;

impl Shell {
    pub(crate) fn online_board(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let page = match &self.online.board {
            None | Some((_, Load::Idle | Load::Loading)) => return self.skeleton(p, 6, 56.),
            Some((_, Load::Failed(e))) => {
                let e = e.clone();
                return self.failed_view(&e, cx);
            }
            Some((_, Load::Ready(page))) => page.clone(),
        };
        let mut items: Vec<Target> = Vec::new();
        let sectors = self.content_width >= 900.;
        let best = best_sectors(&page.rows, 3);
        let leader = page.rows.first().map_or(0, |r| r.time_ms);
        let me = self.account_name();
        let mut body: Vec<AnyElement> = Vec::new();
        for r in &page.rows {
            let idx = self.target_item(&mut items, Target::Profile(r.driver.clone()));
            let mine = me.as_deref() == Some(r.driver.name.as_str());
            body.push(self.board_row(r, idx, leader, &best, sectors, mine, p, cx));
            if SHOW_SETUPS
                && self.online.setup_open.as_deref() == Some(r.lap_id.as_str())
                && let Some(setup) = &r.setup
            {
                let mine = page
                    .mine
                    .as_ref()
                    .filter(|m| m.lap_id != r.lap_id)
                    .and_then(|m| m.setup.as_ref());
                body.push(self.setup_panel(setup, mine, p, cx));
            }
        }
        if page.rows.is_empty() {
            body.push(
                div()
                    .px(px(14.))
                    .py(px(22.))
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .border_t_1()
                    .border_color(p.border_variant)
                    .child(if self.board_category().is_default() {
                        tr("No laps on this board yet.")
                    } else {
                        tr("No laps with these filters. Change the assists or input above.")
                    })
                    .into_any_element(),
            );
        }
        self.online.items = items;
        let remaining = page.total.saturating_sub(page.rows.len() as u32);
        let more = (remaining > 0).then(|| {
            let key = page.track.key.clone();
            let at = page.rows.len() as u32;
            Button::new("board-more")
                .outline()
                .tap_sm()
                .disabled(self.online.more)
                .label(if self.online.more {
                    tr("Loading…")
                } else {
                    tr("Show more laps")
                })
                .on_click(cx.listener(move |this, _, _, cx| this.online_load_board(key.clone(), at, cx)))
        });
        let header = self.board_header(&page, p, cx);
        let filters = self.filter_chips(p, cx);
        let ad = self.ad_banner(crate::ads::Unit::Online, self.content_width - 56., p);
        let pinned = page
            .mine
            .clone()
            .filter(|m| page.rows.iter().all(|r| r.lap_id != m.lap_id))
            .map(|m| self.board_row(&m, usize::MAX, leader, &best, sectors, true, p, cx));
        let history = self.history_strip(&page, p);
        let watch = self.online_watch_notice().map(|(ok, text)| {
            div()
                .text_size(px(13.))
                .text_color(if *ok { p.text_muted } else { p.error })
                .child(text.clone())
        });
        let head = h_flex()
            .h(px(32.))
            .px(px(14.))
            .gap(px(12.))
            .items_center()
            .bg(p.elevated)
            .child(head_cell(54., "RANK", p))
            .child(div().flex_1().min_w_0().child(head_cell(120., "DRIVER", p)))
            .child(head_cell(88., "TIME", p))
            .child(head_cell(74., "GAP", p))
            .when(sectors, |d| {
                d.child(head_cell(66., "S1", p))
                    .child(head_cell(66., "S2", p))
                    .child(head_cell(66., "S3", p))
            })
            .when(self.content_width >= 1100., |d| {
                d.child(head_cell(130., "CAR", p))
            })
            .child(head_cell(84., "DATE", p))
            .child(head_cell(if SHOW_SETUPS { 190. } else { 160. }, "", p));
        v_flex()
            .flex_1()
            .min_h_0()
            .scroll_y("online-board")
            .px(px(28.))
            .pb(px(28.))
            .gap(px(16.))
            .child(header)
            .child(filters)
            .children(ad)
            .children(watch)
            .child(panel(p).child(head).children(pinned).children(body))
            .children(more)
            .children(history)
            .into_any_element()
    }

    fn board_header(&self, page: &BoardPage, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let t = &page.track;
        let byline = self.online_bylines(&[t]).into_iter().next().flatten();
        let download = t
            .addon
            .as_deref()
            .and_then(|addon| self.track_download_button(addon, "board-download-track", cx));
        panel(p)
            .child(
                h_flex()
                    .p(px(14.))
                    .gap(px(16.))
                    .items_center()
                    .child(self.layout_art(t.layout.as_ref(), 84., p.accent_hover, p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(
                                h_flex()
                                    .gap(px(10.))
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(26.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(t.name.clone()),
                                    )
                                    .children(self.flag_chip(&t.flag, p)),
                            )
                            .children(byline.map(|b| div().text_size(px(14.)).text_color(p.text).child(b)))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child(format!("{} - {}", page.season, t.key.weather)),
                            )
                            .children(t.record.as_ref().map(|r| {
                                h_flex()
                                    .gap(px(10.))
                                    .items_center()
                                    .pt(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(p.text_placeholder)
                                            .child(tr("RECORD")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(28.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(p.accent_hover)
                                            .child(lap_time(r.time_ms)),
                                    )
                                    .child(self.tt_face(&r.driver, 26., p))
                                    .child(div().text_size(px(14.)).child(r.driver.name.clone()))
                            })),
                    )
                    .children(download),
            )
            .into_any_element()
    }

    fn filter_chips(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let cat = self.board_category();
        let differs = self.online.board_category.is_some();
        h_flex()
            .gap(px(14.))
            .items_center()
            .child(self.tt_category_bar(cat, true, p, cx))
            .when(differs, |d| {
                d.child(
                    div()
                        .id("board-reset-category")
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(p.accent_hover)
                        .hover(move |s| s.text_color(p.text))
                        .child(tr("Reset to my categories"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let global = this.online.category;
                            this.online.board_category = None;
                            this.online_set_board_category(global, cx);
                        })),
                )
            })
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn board_row(
        &self,
        r: &Row,
        idx: usize,
        leader: u64,
        best: &[Option<u64>],
        sectors: bool,
        mine: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let driver = r.driver.clone();
        let personal: Vec<Option<u64>> = (0..3).map(|i| r.sectors_ms.get(i).copied()).collect();
        let colour = medal(r.rank, &p);
        let shown = shown_rank(idx, r.rank, !self.board_category().is_default());
        h_flex()
            .id(SharedString::from(format!("row-{idx}-{}", r.lap_id)))
            .h(px(38.))
            .px(px(14.))
            .gap(px(12.))
            .items_center()
            .border_t_1()
            .border_color(p.border_variant)
            .when(r.rank <= 3, |d| d.bg(colour.opacity(0.06)))
            .when(mine, |d| d.bg(p.selected))
            .when(self.online.ringed(idx), |d| d.border_1().border_color(p.accent))
            .child(
                div()
                    .w(px(54.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(colour)
                    .child(shown.0.to_string())
                    .children(shown.1.map(|o| {
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(p.text_placeholder)
                            .child(format!("({o})"))
                    })),
            )
            .child(
                h_flex()
                    .id(SharedString::from(format!("drv-{idx}")))
                    .flex_1()
                    .min_w_0()
                    .gap(px(10.))
                    .items_center()
                    .cursor_pointer()
                    .hover(move |s| s.text_color(p.accent_hover))
                    .child(self.tt_face(&r.driver, 28., p))
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(r.driver.name.clone()),
                    )
                    .when(mine, |d| d.child(chip(tr("YOU"), p)))
                    .children(self.input_icons(r, p))
                    .on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.online_go(Target::Profile(driver.clone()), cx)
                        }),
                    ),
            )
            .child(
                div()
                    .w(px(88.))
                    .flex_shrink_0()
                    .font_weight(FontWeight::BOLD)
                    .child(lap_time(r.time_ms)),
            )
            .child(
                div()
                    .w(px(74.))
                    .flex_shrink_0()
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(gap_text(r.time_ms, leader)),
            )
            .when(sectors, |d| {
                d.children((0..3).map(|i| {
                    let t = personal[i];
                    let colour = match t.map(|t| sector_mark(t, best[i], None)) {
                        Some(SectorMark::Overall) => hsla(0.78, 0.7, 0.72, 1.),
                        _ => p.text_muted,
                    };
                    div()
                        .w(px(66.))
                        .flex_shrink_0()
                        .text_size(px(13.))
                        .font_weight(if colour == p.text_muted {
                            FontWeight::NORMAL
                        } else {
                            FontWeight::BOLD
                        })
                        .text_color(colour)
                        .child(t.map(sector_time).unwrap_or_else(|| "-".into()))
                }))
            })
            .when(self.content_width >= 1100., |d| {
                d.child(
                    div()
                        .w(px(130.))
                        .flex_shrink_0()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(p.text_muted)
                        .child(r.car.clone().unwrap_or_default()),
                )
            })
            .child(
                div()
                    .w(px(84.))
                    .flex_shrink_0()
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(iso_date(&r.date).to_owned()),
            )
            .child(
                h_flex()
                    .w(px(if SHOW_SETUPS { 190. } else { 160. }))
                    .flex_shrink_0()
                    .justify_end()
                    .gap(px(4.))
                    .child(self.lap_buttons(&format!("{idx}-{}", r.lap_id), &r.lap_id, r.game_replay, cx))
                    .children(SHOW_SETUPS.then(|| {
                        self.setup_button(&format!("{idx}-{}", r.lap_id), &r.lap_id, r.setup.is_some(), cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn lap_buttons(
        &self,
        key: &str,
        lap_id: &str,
        game_replay: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = lap_url(lap_id);
        let add = game_replay.then(|| {
            self.add_to_game_button(&format!("add-{key}"), lap_id, true, cx)
                .tap_xs()
        });
        h_flex()
            .flex_shrink_0()
            .gap(px(6.))
            .child(
                Button::new(SharedString::from(format!("web-{key}")))
                    .outline()
                    .tap_xs()
                    .icon(IconName::ExternalLink)
                    .label(tr("Web"))
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            )
            .children(add)
            .into_any_element()
    }

    pub(crate) fn add_to_game_button(
        &self,
        key: &str,
        lap_id: &str,
        game_replay: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let known = self.install.is_some();
        let id = lap_id.to_owned();
        Button::new(SharedString::from(key.to_owned()))
            .outline()
            .icon(IconName::Download)
            .label(tr("Add to game"))
            .disabled(!(game_replay && known))
            .tooltip(if !game_replay {
                tr("This lap has no game replay")
            } else if known {
                tr("Put this lap's replay into the game's Theatre")
            } else {
                tr("The game folder is not known")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.online_add_to_game(id.clone(), cx);
            }))
    }

    fn history_strip(&self, page: &BoardPage, p: Palette) -> Option<AnyElement> {
        if page.history.is_empty() {
            return None;
        }
        let mut last: Option<u64> = None;
        let points: Vec<AnyElement> = page
            .history
            .iter()
            .map(|h| {
                let step = last.map(|l| l.saturating_sub(h.time_ms));
                last = Some(h.time_ms);
                v_flex()
                    .w(px(150.))
                    .flex_shrink_0()
                    .gap(px(4.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border_variant)
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .child(iso_date(&h.date).to_owned()),
                    )
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .child(lap_time(h.time_ms)),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .items_center()
                            .child(self.tt_face(&h.driver, 18., p))
                            .child(div().truncate().text_size(px(12.)).child(h.driver.name.clone())),
                    )
                    .children(step.map(|s| {
                        div()
                            .text_size(px(11.))
                            .text_color(p.success)
                            .child(format!("-{}", sector_time(s)))
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .gap(px(10.))
                .child(section_title(tr("RECORD HISTORY"), p))
                .child(h_flex().gap(px(10.)).overflow_x_hidden().children(points))
                .into_any_element(),
        )
    }

    pub(super) fn setup_button(&self, id: &str, lap: &str, has: bool, cx: &mut Context<Self>) -> AnyElement {
        let lap = lap.to_owned();
        Button::new(SharedString::from(format!("setup-{id}")))
            .ghost()
            .tap_xs()
            .icon(IconName::Wrench)
            .disabled(!has)
            .tooltip(if has {
                tr("Show the car setup")
            } else {
                tr("No setup was recorded for this lap")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                let same = this.online.setup_open.as_deref() == Some(lap.as_str());
                this.online.setup_open = (!same).then(|| lap.clone());
                cx.notify();
            }))
            .into_any_element()
    }

    pub(super) fn setup_panel(
        &self,
        setup: &Setup,
        mine: Option<&Setup>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compare = self.online.setup_compare && mine.is_some();
        let text = setup.text();
        let groups: Vec<AnyElement> = SETUP_GROUPS
            .iter()
            .filter_map(|(title, rows)| {
                let lines: Vec<AnyElement> = rows
                    .iter()
                    .filter_map(|(key, label, unit)| {
                        let v = setup.get(key)?;
                        let diff = mine.filter(|_| compare).and_then(|m| setup.difference(m, key));
                        Some(
                            h_flex()
                                .gap(px(8.))
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.))
                                        .text_color(p.text_muted)
                                        .child(*label),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(setup_value_text(v, *unit)),
                                )
                                .children(diff.map(|d| {
                                    div()
                                        .w(px(52.))
                                        .text_size(px(11.))
                                        .text_color(if d > 0. { p.warning } else { p.accent_hover })
                                        .child(format!(
                                            "{}{}",
                                            if d > 0. { "+" } else { "" },
                                            setup_value_text(d, *unit)
                                        ))
                                }))
                                .into_any_element(),
                        )
                    })
                    .collect();
                let compound = (*title == "Tyres").then(|| setup.compound.clone()).flatten();
                if lines.is_empty() && compound.is_none() {
                    return None;
                }
                Some(
                    v_flex()
                        .w(px(260.))
                        .gap(px(6.))
                        .p(px(12.))
                        .rounded(px(10.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border_variant)
                        .child(section_title(tr(title), p))
                        .children(compound.map(|c| {
                            h_flex()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.))
                                        .text_color(p.text_muted)
                                        .child(tr("Compound")),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(crate::format::title_case(&c)),
                                )
                        }))
                        .children(lines)
                        .into_any_element(),
                )
            })
            .collect();
        v_flex()
            .gap(px(10.))
            .px(px(14.))
            .py(px(12.))
            .bg(p.background)
            .border_t_1()
            .border_color(p.border_variant)
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(div().flex_1().text_size(px(12.)).text_color(p.text_muted).child(
                        if setup.changed {
                            tr("The setup changed during this lap; these are the values it started with.")
                        } else {
                            tr("Car setup")
                        },
                    ))
                    .children(mine.map(|_| {
                        Button::new("setup-compare")
                            .outline()
                            .tap_sm()
                            .label(tr("Compare with mine"))
                            .when(compare, |b| b.primary())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.online.setup_compare = !this.online.setup_compare;
                                cx.notify();
                            }))
                    }))
                    .child(
                        Button::new("setup-copy")
                            .outline()
                            .tap_sm()
                            .icon(IconName::Copy)
                            .label(tr("Copy"))
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                            }),
                    ),
            )
            .child(h_flex().flex_wrap().items_start().gap(px(10.)).children(groups))
            .into_any_element()
    }
}

fn head_cell(width: f32, text: &'static str, p: Palette) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text_placeholder)
        .child(text)
}
