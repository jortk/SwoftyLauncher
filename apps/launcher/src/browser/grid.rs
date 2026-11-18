use std::collections::BTreeSet;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::catalog::publishers::{self};
use swofty_core::catalog::search::Query;
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf, trn};

use super::tiles::landing_rows;
use super::*;
use crate::library::Type;
use crate::pages::{GUTTER, tile};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::facet::FILTERS_W;

pub(super) type GridKey = (String, Sort, Tab, BTreeSet<Facet>, bool, u64);

pub(super) struct GridCache {
    pub(super) key: GridKey,
    pub(super) query: Arc<Query>,
    pub(super) scores: Vec<Option<u32>>,
    pub(super) shown: Arc<[usize]>,
    pub(super) types: Vec<Facet>,
    pub(super) authors: Vec<Facet>,
    pub(super) natives: usize,
    pub(super) categories: Vec<usize>,
}

impl Shell {
    fn rebuild_grid_cache(&mut self, key: GridKey, query: &Arc<Query>, sort: Sort) {
        let scores: Vec<_> = self.browser.addons.iter().map(|a| query.score(a)).collect();
        let names: Vec<_> = self
            .browser
            .addons
            .iter()
            .map(|a| a.name.to_lowercase())
            .collect();
        let mut shown: Vec<usize> = self
            .browser
            .addons
            .iter()
            .enumerate()
            .filter(|(i, a)| self.catalog_passes(a, scores[*i].is_some(), None))
            .map(|(i, _)| i)
            .collect();
        shown.sort_by(|&ai, &bi| {
            let a = &self.browser.addons[ai];
            let b = &self.browser.addons[bi];
            let first = match sort {
                Sort::Match => scores[bi]
                    .cmp(&scores[ai])
                    .then_with(|| self.browser.stars_of(b).cmp(&self.browser.stars_of(a))),
                Sort::Stars => self.browser.stars_of(b).cmp(&self.browser.stars_of(a)),
                Sort::Downloads => b.downloads.cmp(&a.downloads),
                Sort::Trending => b.downloads_30d.cmp(&a.downloads_30d),
                Sort::Updated => b.updated_at.cmp(&a.updated_at),
                Sort::Newest => b.created_at.cmp(&a.created_at),
                Sort::Name => std::cmp::Ordering::Equal,
            };
            first.then_with(|| names[ai].cmp(&names[bi]))
        });
        let types = Type::ALL
            .into_iter()
            .map(Facet::Type)
            .filter(|f| {
                self.browser.facets.contains(f)
                    || self.browser.addons.iter().any(|a| self.catalog_facet(a, f))
            })
            .collect();
        let authors = publishers::by_publisher(&self.browser.addons)
            .into_iter()
            .map(|(pb, _)| Facet::Author {
                id: pb.id,
                name: pb.name,
            })
            .enumerate()
            .filter(|(i, f)| *i < AUTHORS_SHOWN || self.browser.facets.contains(f))
            .map(|(_, f)| f)
            .collect();
        let natives = self
            .browser
            .addons
            .iter()
            .zip(&scores)
            .filter(|(a, score)| {
                a.native(&current_version()) && self.catalog_passes(a, score.is_some(), Some(Group::Code))
            })
            .count();
        let categories = [None]
            .into_iter()
            .chain(shelves())
            .map(|tab| {
                self.browser
                    .addons
                    .iter()
                    .zip(&scores)
                    .filter(|(a, score)| {
                        self.catalog_passes(a, score.is_some(), Some(Group::Category))
                            && tab.is_none_or(|t| a.category() == t)
                    })
                    .count()
            })
            .collect();
        self.browser.grid_cache = Some(GridCache {
            key,
            query: query.clone(),
            scores,
            shown: shown.into(),
            types,
            authors,
            natives,
            categories,
        });
        self.browser.facet_counts.borrow_mut().clear();
    }

    pub(super) fn browser_grid(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let text = self.browser.search.read(cx).value().trim().to_owned();
        if text.is_empty() {
            self.browser.search_sort = None;
        }
        let query = self
            .browser
            .grid_cache
            .as_ref()
            .filter(|cache| cache.key.0 == text)
            .map_or_else(|| Arc::new(Query::parse(&text)), |cache| cache.query.clone());
        let sort = self.sort_now(&query);
        let key = (
            text.clone(),
            sort,
            self.browser.tab,
            self.browser.facets.clone(),
            self.browser.hide_code,
            self.browser.catalog_revision,
        );
        let stale = self
            .browser
            .grid_cache
            .as_ref()
            .is_none_or(|cache| cache.key != key);
        if stale {
            self.rebuild_grid_cache(key, &query, sort);
        }
        let cache = self.browser.grid_cache.as_ref().unwrap();
        let shown = cache.shown.clone();
        let filtering = !query.is_empty()
            || !self.browser.facets.is_empty()
            || self.browser.tab.is_some()
            || self.browser.hide_code;
        let count = if filtering {
            trn(shown.len(), "{count} result", "{count} results")
        } else {
            trn(shown.len(), "{count} addon", "{count} addons")
        };
        let width = self.content_width - FILTERS_W - 20.;
        let ad = (!self.ad_header_room(crate::ads::Unit::Discover))
            .then(|| self.ad_banner(crate::ads::Unit::Discover, width - GUTTER, p))
            .flatten();
        let landing = !self.browser.addons.is_empty() && self.browser.on_landing(!query.is_empty());
        if landing {
            let filters = self.filter_column(p, cx);
            let body = self.virtual_cards(shown, query, width, true, count, sort, p, cx);
            return self.browser_columns(filters, div(), ad, body);
        }
        let bar = self.grid_bar(&count, sort, !query.is_empty(), p, cx);
        let filters = self.filter_column(p, cx);
        let found = shown
            .is_empty()
            .then(|| self.creators_found(&query, p, cx))
            .flatten();
        let bare = query.is_empty() && self.browser.facets.is_empty() && !self.browser.hide_code;
        let body = if shown.is_empty() && found.is_some() {
            self.scroll_area(
                v_flex()
                    .id("browser-cards")
                    .pb(px(24.))
                    .gap(px(GAP))
                    .children(found)
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_placeholder)
                            .child(tr("No addons match this search and these filters.")),
                    ),
                cx,
            )
            .into_any_element()
        } else if shown.is_empty() && bare && !self.browser.addons.is_empty() && self.browser.tab.is_some() {
            self.empty_shelf(self.browser.tab, p, cx)
        } else if shown.is_empty() {
            self.grid_empty(p, cx)
        } else {
            self.virtual_cards(shown, query, width, false, count, sort, p, cx)
        };
        self.browser_columns(filters, bar, ad, body)
    }

    fn grid_bar(&self, count: &str, sort: Sort, searching: bool, p: Palette, cx: &mut Context<Self>) -> Div {
        h_flex()
            .pb(px(14.))
            .gap(px(12.))
            .flex_shrink_0()
            .items_start()
            .child(div().flex_1().min_w_0().child(self.category_bar(p, cx)))
            .child(
                h_flex()
                    .gap(px(10.))
                    .flex_shrink_0()
                    .items_center()
                    .h(px(28.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(count.to_owned()),
                    )
                    .child(self.sort_button(sort, searching, cx)),
            )
    }

    fn grid_empty(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let (title, body, action) = if self.browser.addons.is_empty() {
            (
                tr("No addons to show yet"),
                if self.browser.fetching {
                    tr("Checking the registry for addons…")
                } else {
                    tr("The registry couldn't be reached. Retry to check again.")
                },
                Button::new("browser-empty-retry")
                    .outline()
                    .tap_sm()
                    .icon(IconName::RefreshCw)
                    .label(tr("Retry"))
                    .disabled(self.browser.fetching)
                    .on_click(cx.listener(|this, _, _, cx| this.fetch_catalog(cx))),
            )
        } else {
            (
                tr("No addons match"),
                tr(
                    "Nothing matches this search and these filters. Check the spelling, or try author: or category:.",
                ),
                Button::new("browser-empty-clear")
                    .outline()
                    .tap_sm()
                    .icon(IconName::X)
                    .label(tr("Clear search and filters"))
                    .on_click(cx.listener(|this, _, _, cx| this.clear_filters(cx))),
            )
        };
        self.empty(IconName::PackageSearch, title, body, action, p)
    }

    pub(super) fn browser_columns(
        &self,
        filters: Div,
        bar: Div,
        ad: Option<AnyElement>,
        body: impl IntoElement,
    ) -> AnyElement {
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .pl(px(GUTTER))
            .pb(px(20.))
            .gap(px(20.))
            .child(filters)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(bar.pr(px(GUTTER)))
                    .children(ad.map(|ad| div().pr(px(GUTTER)).pb(px(14.)).child(ad)))
                    .child(body),
            )
            .into_any_element()
    }

    pub(super) fn empty_shelf(&self, tab: Tab, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .pb(px(48.))
            .child(tile(44., p).child(shelf_glyph(tab, 22., p.text_muted)))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(trf!("Nothing in {category} yet", category = tab_label(tab))),
            )
            .child(
                div()
                    .max_w(px(400.))
                    .text_center()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .child(tr("Addons published in this category show here.")),
            )
            .child(
                h_flex().pt(px(8.)).gap(px(8.)).child(
                    Button::new("browser-shelf-empty-all")
                        .outline()
                        .tap_sm()
                        .icon(IconName::LayoutGrid)
                        .label(tr("All addons"))
                        .on_click(cx.listener(|this, _, _, cx| this.shelve(None, cx))),
                ),
            )
            .into_any_element()
    }

    pub(super) fn card_rows(
        &self,
        addons: &[&Addon],
        query: &Query,
        width: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let columns = ((width + GAP) / (CARD_W + GAP)).floor().max(1.) as usize;
        let card_w = ((width - GAP * (columns - 1) as f32) / columns as f32).floor() - 1.;
        addons
            .chunks(columns)
            .map(|chunk| {
                h_flex()
                    .flex_shrink_0()
                    .items_stretch()
                    .gap(px(GAP))
                    .children(chunk.iter().map(|a| self.browser_card(a, query, card_w, p, cx)))
                    .into_any_element()
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn virtual_cards(
        &mut self,
        indices: Arc<[usize]>,
        query: Arc<Query>,
        width: f32,
        landing: bool,
        count: String,
        sort: Sort,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let columns = ((width + GAP) / (CARD_W + GAP)).floor().max(1.) as usize;
        let card_w = ((width - GAP * (columns - 1) as f32) / columns as f32).floor() - 1.;
        let rows = indices.len().div_ceil(columns);
        let header_rows = if landing { landing_rows(width) + 1 } else { 1 };
        let layout = (indices.clone(), columns, landing, width.to_bits());
        if self.browser.grid_layout != layout {
            let offset = self.browser.grid_scroll.logical_scroll_top();
            // Give unvisited rows height so scrolling is not clamped to only the measured content.
            self.browser
                .grid_scroll
                .reset_with_uniform_height(rows + header_rows, px(260.));
            self.browser.grid_scroll.scroll_to(offset);
            self.browser.grid_layout = layout;
        }
        let content = list(
            self.browser.grid_scroll.clone(),
            cx.processor(move |this, row: usize, _, cx| {
                if row < header_rows {
                    return if landing {
                        if row + 1 == header_rows {
                            this.all_heading(count.clone(), sort, width, p, cx)
                                .pb(px(GAP))
                                .into_any_element()
                        } else {
                            this.landing_row(row, width, p, cx)
                        }
                    } else {
                        v_flex()
                            .children(this.creators_found(&query, p, cx))
                            .into_any_element()
                    };
                }
                let start = (row - header_rows) * columns;
                let end = (start + columns).min(indices.len());
                h_flex()
                    .pb(px(GAP))
                    .items_stretch()
                    .gap(px(GAP))
                    .children(indices[start..end].iter().filter_map(|&index| {
                        this.browser
                            .addons
                            .get(index)
                            .map(|a| this.browser_card(a, &query, card_w, p, cx))
                    }))
                    .into_any_element()
            }),
        )
        .size_full();
        self.browser
            .smooth
            .wrap(content, &self.browser.grid_scroll, cx)
            .into_any_element()
    }

    fn all_heading(&self, count: String, sort: Sort, width: f32, p: Palette, cx: &mut Context<Self>) -> Div {
        h_flex()
            .w(px(width - 1.))
            .pt(px(10.))
            .gap(px(12.))
            .flex_shrink_0()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("All addons")),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(count),
            )
            .child(self.sort_button(sort, false, cx))
    }

    pub(super) fn scroll_area(&self, body: Stateful<Div>, cx: &Context<Self>) -> Div {
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                div()
                    .id("browser-scroll")
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.browser.scroll)
                    .child(body.flex_none()),
            )
            .child(self.browser.smooth.overlay(&self.browser.scroll, cx))
    }
}
