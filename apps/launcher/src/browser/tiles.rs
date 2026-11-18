use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_i18n::{tr, trn};

use super::*;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;

const TILE_MIN: f32 = 220.;
const TILE_ART: f32 = 96.;
const TILE_FACES: usize = 4;
const FACE: f32 = 40.;

fn shelf_columns(width: f32) -> usize {
    [5, 2]
        .into_iter()
        .find(|&n| n as f32 * TILE_MIN + (n - 1) as f32 * GAP <= width)
        .unwrap_or(1)
}

fn tile_faces(width: f32) -> usize {
    let room = width - 28. - 64. - 40.;
    (1 + ((room - FACE) / (FACE - 8.)).floor().max(0.) as usize).min(TILE_FACES)
}

pub(super) fn landing_rows(width: f32) -> usize {
    shelves().count().div_ceil(shelf_columns(width)) + 1
}

fn shelved(addons: &[Addon], tab: Tab, stars: impl Fn(&Addon) -> u64) -> Vec<&Addon> {
    let mut found: Vec<&Addon> = addons
        .iter()
        .filter(|a| tab.is_none_or(|t| a.category() == t))
        .collect();
    found.sort_by(|a, b| {
        stars(b)
            .cmp(&stars(a))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    found
}

impl Shell {
    pub(super) fn landing_row(
        &self,
        row: usize,
        width: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if row == 0 {
            return h_flex()
                .h(px(28.))
                .flex_shrink_0()
                .items_center()
                .pb(px(GAP))
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr("Browse by category"))
                .into_any_element();
        }
        let columns = shelf_columns(width);
        let tile_w = ((width - GAP * (columns - 1) as f32) / columns as f32).floor() - 1.;
        h_flex()
            .pb(px(GAP))
            .gap(px(GAP))
            .children(
                shelves()
                    .skip((row - 1) * columns)
                    .take(columns)
                    .map(|tab| self.shelf_tile(tab, tile_w, p, cx)),
            )
            .into_any_element()
    }

    fn has_icon(&self, a: &Addon) -> bool {
        a.icon
            .as_ref()
            .is_some_and(|u| self.browser.icons.contains_key(u))
    }

    fn faces(&self, addons: &[&Addon], n: usize, ring: Hsla, p: Palette) -> Div {
        let mut picked: Vec<&Addon> = addons.to_vec();
        picked.sort_by_key(|a| !self.has_icon(a));
        let rest = addons.len().saturating_sub(n);
        h_flex()
            .flex_shrink_0()
            .items_center()
            .children(picked.into_iter().take(n).enumerate().map(|(i, a)| {
                div()
                    .p(px(2.))
                    .rounded(px(FACE * 0.22 + 2.))
                    .bg(ring)
                    .when(i > 0, |d| d.ml(px(-12.)))
                    .child(self.catalog_icon(a, FACE, p))
            }))
            .when(rest > 0, |d| {
                d.child(
                    div()
                        .ml(px(6.))
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(12.))
                        .bg(ring.blend(p.text.opacity(0.07)))
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text_muted)
                        .child(format!("+{rest}")),
                )
            })
    }

    fn shelf_tile(&self, tab: Tab, width: f32, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let addons = shelved(&self.browser.addons, tab, |a| self.browser.stars_of(a));
        let n = addons.len();
        let empty = n == 0;
        let tint = shelf_tint(tab, p);
        let band = if empty {
            p.surface
        } else {
            p.surface.blend(tint.opacity(0.13))
        };
        let slot = div()
            .size(px(FACE + 4.))
            .rounded(px(10.))
            .border_1()
            .border_dashed()
            .border_color(p.border)
            .flex()
            .items_center()
            .justify_center()
            .child(shelf_glyph(tab, 20., p.text_placeholder));
        let art = h_flex()
            .relative()
            .h(px(TILE_ART))
            .w_full()
            .px(px(14.))
            .flex_shrink_0()
            .items_center()
            .overflow_hidden()
            .rounded_t(px(11.))
            .bg(band)
            .border_b_1()
            .border_color(if empty {
                p.border_variant
            } else {
                tint.opacity(0.18)
            })
            .child(
                div()
                    .absolute()
                    .right(px(-10.))
                    .bottom(px(-18.))
                    .child(shelf_glyph(
                        tab,
                        TILE_ART + 8.,
                        if empty {
                            p.text_placeholder.opacity(0.1)
                        } else {
                            tint.opacity(0.22)
                        },
                    )),
            )
            .map(|d| {
                if empty {
                    d.child(slot)
                } else {
                    d.child(self.faces(&addons, tile_faces(width), band, p))
                }
            });
        v_flex()
            .id(SharedString::from(format!("browser-shelf-{}", tab_label(tab))))
            .w(px(width))
            .flex_none()
            .rounded(px(12.))
            .border_1()
            .border_color(if empty { p.border_variant } else { p.border })
            .bg(p.surface)
            .cursor_pointer()
            .hover(move |s| {
                if empty {
                    s.bg(p.hover)
                } else {
                    s.bg(p.hover).border_color(tint.opacity(0.75))
                }
            })
            .child(art)
            .child(
                h_flex()
                    .px(px(14.))
                    .py(px(12.))
                    .gap(px(10.))
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if empty { p.text_muted } else { p.text })
                                    .truncate()
                                    .child(tab_label(tab)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .truncate()
                                    .child(if empty {
                                        tr("No addons yet").to_owned()
                                    } else {
                                        trn(n, "{count} addon", "{count} addons")
                                    }),
                            ),
                    )
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(px(16.))
                            .text_color(p.text_placeholder),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.shelve(tab, cx)))
            .into_any_element()
    }

    pub(super) fn category_bar(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let counts = self
            .browser
            .grid_cache
            .as_ref()
            .map(|cache| cache.categories.clone())
            .unwrap_or_default();
        let pills: Vec<AnyElement> = [None]
            .into_iter()
            .chain(shelves())
            .enumerate()
            .map(|(i, tab)| {
                let n = counts.get(i).copied().unwrap_or(0);
                let on = self.browser.tab == tab;
                h_flex()
                    .id(SharedString::from(format!("browser-tab-{tab:?}")))
                    .h(px(28.))
                    .px(px(11.))
                    .gap(px(6.))
                    .flex_shrink_0()
                    .rounded(px(14.))
                    .border_1()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .map(|d| {
                        if on {
                            d.bg(p.accent.opacity(0.14))
                                .border_color(p.accent.opacity(0.6))
                                .text_color(p.text)
                                .font_weight(FontWeight::MEDIUM)
                        } else {
                            d.border_color(p.border)
                                .text_color(p.text_muted)
                                .hover(move |s| s.bg(p.hover).text_color(p.text))
                        }
                    })
                    .child(tab_label(tab))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(if on { p.text_muted } else { p.text_placeholder })
                            .child(n.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.shelve(tab, cx)))
                    .into_any_element()
            })
            .collect();
        let back = h_flex()
            .id("browser-categories")
            .h(px(28.))
            .px(px(8.))
            .ml(px(-8.))
            .gap(px(6.))
            .flex_shrink_0()
            .rounded(px(5.))
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(p.text_muted)
            .hover(move |s| s.bg(p.hover).text_color(p.text))
            .child(Icon::new(IconName::ArrowLeft).size(px(14.)))
            .child(tr("Categories"))
            .tooltip(|window, cx| Tooltip::new(tr("Back to the category tiles")).build(window, cx))
            .on_click(cx.listener(|this, _, _, cx| this.show_categories(cx)));
        h_flex()
            .gap(px(6.))
            .flex_wrap()
            .child(back)
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(1.))
                    .h(px(14.))
                    .mx(px(4.))
                    .my(px(7.))
                    .bg(p.border),
            )
            .children(pills)
    }
}
