use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::manifest::Category;
use swofty_i18n::tr;

use super::*;
use crate::pages::{TRACK, label};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::{Palette, SCROLLBAR_GAP, ScrollArea};
use crate::ui::facet::{FILTERS_W, facet_row};

pub(super) fn shelf_tint(tab: Tab, p: Palette) -> Hsla {
    let hue = match tab {
        None => return p.accent,
        Some(None) => return p.text_muted,
        Some(Some(Category::Seasons)) => 38.,
        Some(Some(Category::Tracks)) => 145.,
        Some(Some(Category::Cars)) => 2.,
        Some(Some(Category::Helmets)) => 205.,
        Some(Some(Category::Shaders)) => 320.,
        Some(Some(Category::Career)) => 280.,
        Some(Some(Category::Ai)) => 175.,
        Some(Some(Category::Performance)) => 85.,
        Some(Some(Category::Utility)) => 245.,
    };
    Hsla {
        h: hue / 360.,
        ..p.accent
    }
}

pub(super) fn shelf_glyph(tab: Tab, size: f32, color: Hsla) -> AnyElement {
    let icon = match tab {
        Some(Some(Category::Tracks)) => {
            return svg()
                .data(TRACK)
                .size(px(size))
                .text_color(color)
                .into_any_element();
        }
        Some(Some(Category::Helmets)) => {
            return svg()
                .data(HELMET)
                .size(px(size))
                .text_color(color)
                .into_any_element();
        }
        None => IconName::LayoutGrid,
        Some(None) => IconName::Shapes,
        Some(Some(Category::Seasons)) => IconName::CalendarRange,
        Some(Some(Category::Cars)) => IconName::Car,
        Some(Some(Category::Shaders)) => IconName::Palette,
        Some(Some(Category::Career)) => IconName::Briefcase,
        Some(Some(Category::Ai)) => IconName::Bot,
        Some(Some(Category::Performance)) => IconName::Gauge,
        Some(Some(Category::Utility)) => IconName::Wrench,
    };
    Icon::new(icon)
        .size(px(size))
        .text_color(color)
        .into_any_element()
}

impl Shell {
    fn facet_count(&self, f: &Facet) -> usize {
        let b = &self.browser;
        if let Some(&count) = b.facet_counts.borrow().get(f) {
            return count;
        }
        let count = b
            .addons
            .iter()
            .zip(&b.grid_cache.as_ref().unwrap().scores)
            .filter(|(a, s)| self.catalog_passes(a, s.is_some(), Some(f.group())) && self.catalog_facet(a, f))
            .count();
        b.facet_counts.borrow_mut().insert(f.clone(), count);
        count
    }

    fn facet_filter_row(&self, f: Facet, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let n = self.facet_count(&f);
        let on = self.browser.facets.contains(&f);
        let key = f.clone();
        facet_row(format!("browser-filter-{f:?}"), on, f.label(), n, p)
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.browser.facets.remove(&key) {
                    this.browser.facets.insert(key.clone());
                }
                cx.notify();
            }))
            .into_any_element()
    }

    fn platform_all_row(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.browser;
        let none = !b.facets.iter().any(|f| f.group() == Group::Platform);
        let n = b
            .addons
            .iter()
            .zip(&b.grid_cache.as_ref().unwrap().scores)
            .filter(|(a, s)| self.catalog_passes(a, s.is_some(), Some(Group::Platform)))
            .count();
        facet_row("browser-filter-platform-all", none, tr("All"), n, p)
            .on_click(cx.listener(|this, _, _, cx| {
                this.browser.facets.retain(|f| f.group() != Group::Platform);
                cx.notify();
            }))
            .into_any_element()
    }

    fn filters_header(&self, active: bool, p: Palette, cx: &mut Context<Self>) -> Div {
        let clear = active.then(|| {
            div()
                .id("browser-filters-clear")
                .text_size(px(12.))
                .text_color(p.accent)
                .cursor_pointer()
                .hover(move |s| s.text_color(p.accent_hover))
                .child(tr("Clear filters"))
                .on_click(cx.listener(|this, _, _, cx| this.clear_filters(cx)))
        });
        h_flex()
            .pb(px(8.))
            .mb(px(-6.))
            .border_b_1()
            .border_color(p.border_variant)
            .justify_between()
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_color(p.text_muted)
                    .child(Icon::new(IconName::Funnel).size(px(13.)))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr("Filters")),
                    ),
            )
            .children(clear)
    }

    pub(super) fn filter_column(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let b = &self.browser;
        let query = b.search.read(cx).value().trim().to_owned();
        let active = !b.facets.is_empty() || b.hide_code || !query.is_empty();
        let group = |title: &'static str, rows: Vec<AnyElement>| {
            v_flex()
                .gap(px(1.))
                .child(div().pb(px(4.)).child(label(title, p)))
                .children(rows)
        };
        let cache = b.grid_cache.as_ref().unwrap();
        let types = cache
            .types
            .iter()
            .cloned()
            .map(|f| self.facet_filter_row(f, p, cx))
            .collect();
        let states = Want::ALL
            .into_iter()
            .map(|w| self.facet_filter_row(Facet::State(w), p, cx))
            .collect();
        let all_platforms = self.platform_all_row(p, cx);
        let platforms: Vec<_> = std::iter::once(all_platforms)
            .chain(
                Where::ALL
                    .into_iter()
                    .map(|w| self.facet_filter_row(Facet::Platform(w), p, cx)),
            )
            .collect();
        let authors: Vec<_> = cache
            .authors
            .iter()
            .cloned()
            .map(|f| self.facet_filter_row(f, p, cx))
            .collect();
        let code = facet_row(
            "browser-filter-code",
            b.hide_code,
            tr("Hide addons that run code"),
            cache.natives,
            p,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.browser.hide_code = !this.browser.hide_code;
            cx.notify();
        }))
        .into_any_element();
        v_flex()
            .w(px(FILTERS_W))
            .flex_shrink_0()
            .h_full()
            .gap(px(14.))
            .child(
                Input::new(&b.search).cleanable(true).prefix(
                    Icon::new(IconName::Search)
                        .size(px(14.))
                        .text_color(p.text_placeholder),
                ),
            )
            .child(self.filters_header(active, p, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("browser-filters")
                    .gap(px(16.))
                    .pr(px(SCROLLBAR_GAP))
                    .pb(px(8.))
                    .child(group(tr("TYPE"), types))
                    .child(group(tr("STATE"), states))
                    .child(group(tr("PLATFORM"), platforms))
                    .child(group(tr("RUNS CODE IN THE GAME"), vec![code]))
                    .when(!authors.is_empty(), |d| d.child(group(tr("AUTHOR"), authors))),
            )
    }

    pub(super) fn sort_button(
        &self,
        sort: Sort,
        searching: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let shell = cx.entity().downgrade();
        Button::new("browser-sort")
            .outline()
            .tap_sm()
            .icon(IconName::ArrowUpDown)
            .label(sort.label())
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for s in Sort::ALL.into_iter().filter(|s| searching || *s != Sort::Match) {
                    let shell = shell.clone();
                    menu = menu.item(PopupMenuItem::new(s.label()).checked(s == sort).on_click(
                        move |_, _, cx| {
                            shell.update(cx, |this, cx| this.set_sort(s, cx)).ok();
                        },
                    ));
                }
                menu
            })
    }
}
