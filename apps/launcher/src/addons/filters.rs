use super::model::{Facet, Group, State};
use crate::library::Type;
use crate::library::{AddonItem, Source};
use crate::pages::label;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, SCROLLBAR_GAP, ScrollArea};
use crate::ui::facet::{FILTERS_W, facet_row};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::manifest::Category;
use swofty_i18n::tr;

pub(super) fn matches_query(m: &AddonItem, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let man = &m.manifest;
    [
        Some(&man.name),
        Some(&man.id),
        Some(&man.author),
        man.description.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|s| s.to_lowercase().contains(query))
}

impl Shell {
    pub(super) fn matches_facet(&self, m: &AddonItem, f: Facet) -> bool {
        match f {
            Facet::Category(c) => m.category() == c,
            Facet::Type(t) => m.manifest.kind.iter().any(|k| Type::of(*k) == t),
            Facet::Source(s) => m.source == s,
            Facet::State(State::On) => self.addon_on(m),
            Facet::State(State::Off) => !self.addon_on(m),
            Facet::State(State::Conflicts) => !self.live_pairs(m).is_empty(),
            Facet::State(State::Update) => self.update_ready(m),
        }
    }

    pub(super) fn passes(&self, m: &AddonItem, query: &str, except: Option<Group>) -> bool {
        let groups = [Group::Category, Group::Type, Group::Source, Group::State];
        matches_query(m, query)
            && groups.into_iter().filter(|g| Some(*g) != except).all(|g| {
                let mut chosen = self.addons.facets.iter().filter(|f| f.group() == g).peekable();
                chosen.peek().is_none() || chosen.any(|f| self.matches_facet(m, *f))
            })
    }

    pub(super) fn filters(&self, query: &str, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<&AddonItem> = self.library.listed().collect();
        let active = !self.addons.facets.is_empty() || !query.is_empty();
        let group = |title: &'static str, facets: Vec<Facet>, cx: &mut Context<Self>| {
            let rows: Vec<AnyElement> = facets
                .into_iter()
                .filter(|f| {
                    f.group() != Group::Category
                        || self.addons.facets.contains(f)
                        || items.iter().any(|m| self.matches_facet(m, *f))
                })
                .map(|f| {
                    let n = items
                        .iter()
                        .filter(|m| self.passes(m, query, Some(f.group())) && self.matches_facet(m, f))
                        .count();
                    self.filter_row(f, n, p, cx)
                })
                .collect();
            v_flex()
                .gap(px(1.))
                .child(div().pb(px(4.)).child(label(title, p)))
                .children(rows)
        };
        let categories = Category::ALL
            .into_iter()
            .map(|c| Facet::Category(Some(c)))
            .chain([Facet::Category(None)])
            .collect();
        let types = Type::ALL.into_iter().map(Facet::Type).collect();
        let sources = Source::ALL.into_iter().map(Facet::Source).collect();
        let states = State::ALL.into_iter().map(Facet::State).collect();
        let clear = active.then(|| {
            div()
                .id("filters-clear")
                .text_size(px(12.))
                .text_color(p.accent)
                .cursor_pointer()
                .hover(move |s| s.text_color(p.accent_hover))
                .child(tr("Clear"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.addons.facets.clear();
                    this.addons.clear_search(cx);
                    cx.notify();
                }))
        });
        v_flex()
            .w(px(FILTERS_W))
            .flex_shrink_0()
            .h_full()
            .gap(px(14.))
            .child(
                Input::new(&self.addons.search).cleanable(true).prefix(
                    Icon::new(IconName::Search)
                        .size(px(14.))
                        .text_color(p.text_placeholder),
                ),
            )
            .child(
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
                    .children(clear),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("addon-filters")
                    .gap(px(16.))
                    .pr(px(SCROLLBAR_GAP))
                    .pb(px(8.))
                    .child(group(tr("CATEGORY"), categories, cx))
                    .child(group(tr("TYPE"), types, cx))
                    .child(group(tr("SOURCE"), sources, cx))
                    .child(group(tr("STATE"), states, cx)),
            )
    }

    pub(super) fn filter_row(
        &self,
        f: Facet,
        count: usize,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = self.addons.facets.contains(&f);
        facet_row(format!("filter-{f:?}"), on, f.label(), count, p)
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.addons.facets.remove(&f) {
                    this.addons.facets.insert(f);
                }
                cx.notify();
            }))
            .into_any_element()
    }
}
