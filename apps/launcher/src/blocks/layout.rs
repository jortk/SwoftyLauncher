use crate::scale::px;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::view::{Block, Card, DropZone, Grid, Hero, Tabs};

use super::widgets::*;
use super::*;
use crate::sections::{art, icon};
use crate::shell::Shell;
use crate::theme::Palette;

impl Shell {
    pub(crate) fn rich_block(
        &self,
        at: At,
        b: &Block,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        Some(match b {
            Block::Hero(h) => self.hero(at, h, p, cx),
            Block::Pills(items) => pills(items, p).into_any_element(),
            Block::Tiles(tiles) => self.tiles(at, tiles, p),
            Block::Meters(meters) => self.meters(meters, p),
            Block::Card(c) => self.card(at, c, p, cx),
            Block::Grid(g) => self.grid(at, g, p, cx),
            Block::Form(f) => self.form(at, f, p, cx),
            Block::Tabs(t) => self.tabs(at, t, p, cx),
            Block::Image(i) => self.image(at, i, p),
            Block::Chart(c) => {
                let key = at.key(&c.id);
                let prepared = self.blocks.charts.get(&key).map(|(_, scene)| scene.clone());
                chart::render(key, c, p, prepared)
            }
            Block::DropZone(d) => self.drop_zone(at, d, p, cx),
            Block::Table(t) if !t.is_plain() => self.table(at, t, p, cx),
            _ => return None,
        })
    }

    pub(super) fn drop_zone(&self, at: At, d: &DropZone, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .when(at.fill, |x| x.flex_1())
            .min_h(px(280.))
            .items_center()
            .justify_center()
            .gap(px(12.))
            .px(px(32.))
            .py(px(36.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .when_some(d.icon, |x, i| {
                x.child(
                    div()
                        .size(px(64.))
                        .rounded_full()
                        .bg(p.elevated)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(icon(i)).size(px(30.)).text_color(p.accent)),
                )
            })
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text)
                    .child(d.title.clone()),
            )
            .when(!d.text.is_empty(), |x| {
                x.child(
                    div()
                        .max_w(px(440.))
                        .text_center()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(d.text.clone()),
                )
            })
            .child(
                div()
                    .pt(px(8.))
                    .child(self.button(at.addon, at.section, &d.button, false, cx)),
            )
            .into_any_element()
    }

    pub(super) fn tabs(&self, at: At, t: &Tabs, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let active = t.active.clone().or_else(|| t.items.first().map(|i| i.id.clone()));
        let items = t.items.iter().map(|item| {
            let on = active.as_deref() == Some(item.id.as_str());
            let control = format!("{}/{}/{}:{}", at.addon, at.section, t.id, item.id);
            let busy = self.busy.contains(&control);
            let fg = if item.disabled {
                p.text_placeholder
            } else if on {
                p.on_accent
            } else {
                p.text_muted
            };
            let mut tab = div()
                .id(SharedString::from(format!("tab-{control}")))
                .h(px(32.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(7.))
                .rounded(px(7.))
                .text_size(px(13.))
                .font_weight(if on {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(fg)
                .whitespace_nowrap()
                .when(on, |d| d.bg(p.accent))
                .when_some(item.icon, |d, i| d.child(Icon::new(icon(i)).size(px(14.))))
                .child(item.label.clone())
                .when_some(item.badge.clone(), |d, badge| {
                    d.child(
                        div()
                            .h(px(18.))
                            .min_w(px(18.))
                            .px(px(5.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(9.))
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .bg(if on { p.on_accent.opacity(0.22) } else { p.elevated })
                            .text_color(if on { p.on_accent } else { p.text })
                            .child(badge),
                    )
                })
                .when(busy, |d| d.opacity(0.6));
            if !on && !item.disabled && !busy {
                let (addon, section, id, tab_id) = (
                    at.addon.to_owned(),
                    at.section.to_owned(),
                    t.id.clone(),
                    item.id.clone(),
                );
                tab = tab
                    .cursor_pointer()
                    .hover(|d| d.bg(p.hover).text_color(p.text))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        let event = swofty_core::view::Event::Tab {
                            id: id.clone(),
                            tab: tab_id.clone(),
                        };
                        this.send(&addon, &section, control.clone(), event, window, cx);
                    }));
            }
            tab
        });
        h_flex()
            .p(px(4.))
            .gap(px(4.))
            .flex_wrap()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.surface)
            .children(items)
            .into_any_element()
    }

    pub(super) fn hero(&self, at: At, h: &Hero, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let buttons: Vec<AnyElement> = h
            .buttons
            .iter()
            .map(|b| self.button(at.addon, at.section, b, true, cx))
            .collect();
        let facts = (!h.facts.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .gap_x(px(28.))
                .gap_y(px(10.))
                .pt(px(14.))
                .border_t_1()
                .border_color(p.border_variant)
                .children(h.facts.iter().map(|f| {
                    v_flex()
                        .gap(px(3.))
                        .child(eyebrow(&f.label, p))
                        .child(div().text_size(px(13.)).text_color(p.text).child(f.value.clone()))
                }))
        });
        surface(p)
            .p(px(20.))
            .gap(px(16.))
            .child(
                h_flex()
                    .gap(px(14.))
                    .items_center()
                    .when(!h.art.is_none(), |d| d.child(art(&h.art, 48., p)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(6.))
                            .child(
                                h_flex()
                                    .gap(px(10.))
                                    .flex_wrap()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(p.text)
                                            .child(h.title.clone()),
                                    )
                                    .child(pills(&h.pills, p)),
                            )
                            .when_some(h.subtitle.clone(), |d, s| {
                                d.child(div().text_size(px(13.)).text_color(p.text_muted).child(s))
                            }),
                    )
                    .when(!buttons.is_empty(), |d| {
                        d.child(h_flex().gap(px(6.)).children(buttons))
                    }),
            )
            .children(facts)
            .into_any_element()
    }

    pub(super) fn card(&self, at: At, c: &Card, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let inner = at.inner(at.width - 34.);
        let head = c.title.is_some() || c.subtitle.is_some() || !c.pills.is_empty() || !c.buttons.is_empty();
        let buttons: Vec<AnyElement> = c
            .buttons
            .iter()
            .map(|b| self.button(at.addon, at.section, b, true, cx))
            .collect();
        surface(p)
            .when(c.dashed, |d| {
                d.border_2().border_dashed().border_color(p.text_muted)
            })
            .when(at.fill, |d| d.flex_1())
            .p(px(16.))
            .gap(px(14.))
            .when(head, |d| {
                d.child(
                    h_flex()
                        .gap(px(10.))
                        .min_h(px(24.))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.))
                                .when_some(c.title.clone(), |d, t| {
                                    d.child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(p.text)
                                            .truncate()
                                            .child(t),
                                    )
                                })
                                .when_some(c.subtitle.clone(), |d, s| {
                                    d.child(div().text_size(px(12.)).text_color(p.text_muted).child(s))
                                }),
                        )
                        .child(pills(&c.pills, p))
                        .children(buttons),
                )
            })
            .children(c.blocks.iter().map(|b| self.block_at(inner, b, p, cx)))
            .into_any_element()
    }

    pub(super) fn grid(&self, at: At, g: &Grid, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut weights = g.columns.weights();
        let fits = ((at.width + GAP) / (GRID_COLUMN + GAP)).floor().max(1.) as usize;
        if weights.len() > fits {
            weights = vec![1.];
        }
        let cols = weights.len();
        let total: f32 = weights.iter().sum();
        let usable = at.width - GAP * (cols as f32 - 1.);
        v_flex()
            .gap(px(GAP))
            .children(g.blocks.chunks(cols).map(|row| {
                h_flex()
                    .items_stretch()
                    .gap(px(GAP))
                    .children(row.iter().zip(&weights).map(|(b, w)| {
                        let cell = At {
                            width: usable * w / total,
                            fill: true,
                            ..at
                        };
                        v_flex()
                            .flex_basis(px(0.))
                            .flex_grow(*w)
                            .min_w_0()
                            .child(self.block_at(cell, b, p, cx))
                    }))
                    .children(
                        weights[row.len()..]
                            .iter()
                            .map(|w| div().flex_basis(px(0.)).flex_grow(*w)),
                    )
            }))
            .into_any_element()
    }
}
