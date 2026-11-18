//! The people behind the addons: publisher profiles, the Creators list, "more
//! from" lists, and the trail of views the browser walks back along.

use std::collections::BTreeSet;
use std::time::Instant;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::catalog::publishers::{self, Creator};
use swofty_core::catalog::search::Query;
use swofty_i18n::{tr, trf, trn};

use super::*;
use crate::format;
use crate::pages::{GUTTER, label, tag};
use crate::scale::{Hit, px};
use crate::shell::{Page, Shell};
use crate::theme::Palette;

const MORE_SHOWN: usize = 4;
const MORE_INSTALLED: usize = 3;
const CREATORS_FOUND: usize = 4;
const CRUMBS_SHOWN: usize = 4;
const CREATOR_W: f32 = 300.;
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// What the browser shows. `Installed` is the Addons page, for the way back to it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum View {
    Grid,
    Creators,
    Addon(String),
    Author(String),
    Installed(String),
}

fn joined(iso: &str) -> Option<String> {
    let year = iso.get(..4)?.parse::<u32>().ok()?;
    let month = iso.get(5..7)?.parse::<usize>().ok()?;
    Some(trf!(
        "Joined {month} {year}",
        month = tr(MONTHS.get(month.checked_sub(1)?)?),
        year = year
    ))
}

fn face_color(id: &str, p: Palette) -> Hsla {
    let hash = id
        .bytes()
        .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    Hsla {
        h: (p.accent.h + (hash % 12) as f32 / 12.).fract(),
        ..p.accent
    }
}

fn author_links(links: &[publishers::Link]) -> Vec<AnyElement> {
    links
        .iter()
        .filter(|l| l.url.starts_with("https://"))
        .enumerate()
        .map(|(i, l)| {
            let url = l.url.clone();
            let text = match l.label.trim() {
                "" => host(&l.url).to_owned(),
                label => label.to_owned(),
            };
            Button::new(SharedString::from(format!("browser-author-link-{i}")))
                .outline()
                .tap_sm()
                .icon(IconName::ExternalLink)
                .label(text)
                .tooltip(url.clone())
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element()
        })
        .collect()
}

fn author_totals(totals: publishers::Totals, p: Palette) -> Div {
    let figure = |title: &'static str, value: String| {
        v_flex()
            .flex_1()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(value),
            )
            .child(label(title, p))
    };
    h_flex()
        .px(px(24.))
        .py(px(16.))
        .gap(px(24.))
        .border_t_1()
        .border_color(p.border_variant)
        .child(figure(tr("ADDONS"), totals.addons.to_string()))
        .child(figure(tr("DOWNLOADS"), format::compact(totals.downloads)))
        .child(figure(
            tr("DOWNLOADS THIS MONTH"),
            format::compact(totals.downloads_30d),
        ))
        .child(figure(tr("STARS"), format::compact(totals.stars)))
}

fn text_link(key: String, text: impl Into<SharedString>, p: Palette) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(key))
        .gap(px(4.))
        .flex_shrink_0()
        .cursor_pointer()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.accent)
        .hover(move |s| s.text_color(p.accent_hover).underline())
        .child(text.into())
}

fn stats_line(addons: u64, downloads: u64, stars: u64, p: Palette) -> Div {
    h_flex()
        .gap(px(10.))
        .text_size(px(12.))
        .text_color(p.text_placeholder)
        .whitespace_nowrap()
        .child(
            h_flex()
                .gap(px(4.))
                .child(Icon::new(IconName::Package).size(px(12.)))
                .child(trn(addons as usize, "{count} addon", "{count} addons")),
        )
        .child(
            h_flex()
                .gap(px(4.))
                .child(Icon::new(IconName::Download).size(px(12.)))
                .child(format::compact(downloads)),
        )
        .child(
            h_flex()
                .gap(px(4.))
                .child(Icon::new(IconName::Star).size(px(12.)))
                .child(format::compact(stars)),
        )
}

impl Browser {
    pub(super) fn all_creators(&self) -> Vec<Creator> {
        if self.creators.is_empty() {
            publishers::from_catalog(&self.addons)
        } else {
            self.creators.clone()
        }
    }

    fn creator(&self, id: &str) -> Option<Creator> {
        self.creators.iter().find(|c| c.id == id).cloned().or_else(|| {
            publishers::from_catalog(&self.addons)
                .into_iter()
                .find(|c| c.id == id)
        })
    }

    fn publisher_name(&self, id: &str) -> String {
        self.profiles
            .get(id)
            .map(|pr| pr.name.clone())
            .or_else(|| self.creator(id).map(|c| c.name))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| tr("Creator").to_owned())
    }

    fn view_label(&self, v: &View) -> String {
        match v {
            View::Grid => match self.tab {
                None => tr("All addons").to_owned(),
                tab => tab_label(tab).to_owned(),
            },
            View::Creators => tr("Creators").to_owned(),
            View::Addon(id) => self.find(id).map_or_else(|| id.clone(), |a| a.name.clone()),
            View::Author(id) => self.publisher_name(id),
            View::Installed(_) => tr("Addons").to_owned(),
        }
    }

    fn author_addons(&self, id: &str) -> Vec<&Addon> {
        let listed: BTreeSet<&str> = self
            .profiles
            .get(id)
            .map(|pr| pr.addons.iter().map(|a| a.id.as_str()).collect())
            .unwrap_or_default();
        self.addons
            .iter()
            .filter(|a| a.published_by(id) || listed.contains(a.id.as_str()))
            .collect()
    }
}

impl Shell {
    /// Shows `to`, remembering the current view and its scroll to come back to.
    /// A view already on the trail is gone back to instead.
    pub(super) fn browse(&mut self, to: View, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        if b.view == to {
            return;
        }
        if let Some(i) = b.trail.iter().position(|(v, _)| *v == to) {
            self.browse_back_to(i, cx);
            return;
        }
        let from = std::mem::replace(&mut b.view, to.clone());
        b.trail.push((from, b.scroll.offset()));
        b.smooth.cancel();
        b.scroll.set_offset(Point::default());
        b.activity_open = false;
        self.load_view(&to, cx);
        cx.notify();
    }

    fn browse_back_to(&mut self, i: usize, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        let Some((view, offset)) = b.trail.get(i).cloned() else {
            return;
        };
        b.trail.truncate(i);
        b.activity_open = false;
        if let View::Installed(id) = view {
            b.view = View::Grid;
            b.trail.clear();
            b.smooth.cancel();
            b.scroll.set_offset(Point::default());
            self.open_addon = Some(id);
            self.go(Page::Addons, cx);
            return;
        }
        b.view = view.clone();
        b.smooth.cancel();
        b.scroll.set_offset(offset);
        self.load_view(&view, cx);
        cx.notify();
    }

    /// Opens the Creators list as a section of its own: back from it is All addons.
    fn browse_creators(&mut self, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        if b.view == View::Creators {
            return;
        }
        let grid = match (&b.view, b.trail.first()) {
            (View::Grid, _) => b.scroll.offset(),
            (_, Some((View::Grid, offset))) => *offset,
            _ => Point::default(),
        };
        b.trail = vec![(View::Grid, grid)];
        b.view = View::Creators;
        b.smooth.cancel();
        b.scroll.set_offset(Point::default());
        b.activity_open = false;
        cx.notify();
    }

    fn browse_back(&mut self, cx: &mut Context<Self>) {
        match self.browser.trail.len().checked_sub(1) {
            Some(i) => self.browse_back_to(i, cx),
            None => self.browse(View::Grid, cx),
        }
    }

    /// Opens `to` in the browser from the Addons page, with `id` open there to come back to.
    pub(crate) fn browse_from_addons(&mut self, id: &str, to: View, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        b.trail = vec![(View::Installed(id.to_owned()), Point::default())];
        b.view = to.clone();
        b.smooth.cancel();
        b.scroll.set_offset(Point::default());
        b.activity_open = false;
        self.load_view(&to, cx);
        self.go(Page::Browser, cx);
    }

    fn load_view(&mut self, view: &View, cx: &mut Context<Self>) {
        match view {
            View::Addon(id) => self.fetch_details(id, cx),
            View::Author(id) => self.fetch_profile(id, cx),
            _ => {}
        }
    }

    fn fetch_profile(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.browser.profile_loading.insert(id.to_owned()) {
            return;
        }
        let id = id.to_owned();
        let dir = avatar_dir(&self.store.root);
        let known: BTreeSet<String> = self.browser.avatars.keys().cloned().collect();
        let started = Instant::now();
        cx.spawn(async move |this, cx| {
            let job = id.clone();
            let (result, faces) = cx
                .background_executor()
                .spawn(async move {
                    let result = publishers::fetch_profile(&job);
                    let faces = match &result {
                        Ok(pr) => images(&dir, pr.avatar.iter(), &known),
                        Err(_) => Vec::new(),
                    };
                    (result, faces)
                })
                .await;
            this.update(cx, |s, cx| {
                s.browser.profile_loading.remove(&id);
                let took = started.elapsed();
                match result {
                    Ok(pr) => {
                        let detail = format!(
                            "{} · {}",
                            trn(pr.addons.len(), "{count} addon", "{count} addons"),
                            trn(pr.links.len(), "{count} link", "{count} links")
                        );
                        s.record(
                            Op::Profile,
                            true,
                            trf!("Fetched {name}'s profile", name = pr.name),
                            detail,
                            took,
                        );
                        s.browser.profile_failed.remove(&id);
                        s.browser.profiles.insert(id, pr);
                    }
                    Err(e) => {
                        s.record(
                            Op::Profile,
                            false,
                            trf!("Couldn't fetch publisher {id}", id = id),
                            e.to_string(),
                            took,
                        );
                        s.browser.profile_failed.insert(id, e.to_string());
                    }
                }
                s.browser.avatars.extend(faces);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn publisher_avatar(
        &self,
        id: &str,
        name: &str,
        url: Option<&str>,
        size: f32,
        p: Palette,
    ) -> Div {
        let face = div()
            .size(px(size))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden();
        match url.and_then(|u| self.browser.avatars.get(u)) {
            Some(image) => face.child(img(image.clone()).size_full().rounded_full()),
            None => {
                let c = face_color(id, p);
                let letter: String = name.trim().chars().take(1).flat_map(char::to_uppercase).collect();
                face.flex()
                    .items_center()
                    .justify_center()
                    .bg(c.opacity(0.16))
                    .border_1()
                    .border_color(c.opacity(0.45))
                    .text_color(c)
                    .text_size(px(size * 0.44))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(letter)
            }
        }
    }

    fn author_link(
        &self,
        key: String,
        id: &str,
        name: &str,
        size: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let target = id.to_owned();
        div()
            .id(SharedString::from(key))
            .min_w_0()
            .truncate()
            .cursor_pointer()
            .text_size(px(size))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text)
            .hover(move |s| s.text_color(p.accent).underline())
            .child(name.to_owned())
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.browse(View::Author(target.clone()), cx);
            }))
    }

    /// "by <publisher>", the publisher a link to their page, with the
    /// manifest's author after it when that's someone else.
    pub(super) fn byline(
        &self,
        a: &Addon,
        key: String,
        size: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let row = h_flex()
            .min_w_0()
            .gap(px(4.))
            .text_size(px(size))
            .text_color(p.text_muted)
            .whitespace_nowrap()
            .overflow_hidden();
        let Some(owner) = a.owner() else {
            return row.child(div().truncate().child(match a.author_name() {
                "" => a.id.clone(),
                name => trf!("by {name}", name = name),
            }));
        };
        let credit = a.credit().map(|credit| {
            let tip = trf!(
                "{name} published it. Its manifest credits {credit}.",
                name = owner.name,
                credit = credit
            );
            div()
                .id(SharedString::from(format!("{key}-credit")))
                .min_w_0()
                .truncate()
                .text_color(p.text_placeholder)
                .child(format!("· {}", trf!("credited to {credit}", credit = credit)))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        });
        row.child(div().flex_shrink_0().child(tr("by")))
            .child(
                self.author_link(key, &owner.id, &owner.name, size, p, cx)
                    .flex_shrink_0()
                    .max_w(relative(0.75)),
            )
            .children(credit)
    }

    pub(super) fn back_bar(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let b = &self.browser;
        let mut crumbs: Vec<(Option<usize>, String)> = b
            .trail
            .iter()
            .enumerate()
            .map(|(i, (v, _))| (Some(i), b.view_label(v)))
            .collect();
        crumbs.push((None, b.view_label(&b.view)));
        let cut = crumbs.len().saturating_sub(CRUMBS_SHOWN);
        let mut parts: Vec<AnyElement> = Vec::new();
        if cut > 0 {
            parts.push(div().child("…").into_any_element());
        }
        for (n, (at, text)) in crumbs.into_iter().skip(cut).enumerate() {
            if n > 0 || cut > 0 {
                parts.push(
                    Icon::new(IconName::ChevronRight)
                        .size(px(12.))
                        .text_color(p.text_placeholder)
                        .into_any_element(),
                );
            }
            parts.push(match at {
                Some(i) => div()
                    .id(SharedString::from(format!("browser-crumb-{i}")))
                    .max_w(px(220.))
                    .truncate()
                    .cursor_pointer()
                    .hover(move |s| s.text_color(p.text).underline())
                    .child(text)
                    .on_click(cx.listener(move |this, _, _, cx| this.browse_back_to(i, cx)))
                    .into_any_element(),
                None => div()
                    .max_w(px(260.))
                    .truncate()
                    .text_color(p.text)
                    .font_weight(FontWeight::MEDIUM)
                    .child(text)
                    .into_any_element(),
            });
        }
        h_flex()
            .gap(px(14.))
            .flex_shrink_0()
            .items_center()
            .text_size(px(13.))
            .text_color(p.text_muted)
            .child(
                h_flex()
                    .id("browser-back")
                    .h(px(26.))
                    .px(px(8.))
                    .ml(px(-8.))
                    .gap(px(6.))
                    .flex_shrink_0()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover).text_color(p.text))
                    .child(Icon::new(IconName::ArrowLeft).size(px(14.)))
                    .child(tr("Back"))
                    .on_click(cx.listener(|this, _, _, cx| this.browse_back(cx))),
            )
            .child(div().w(px(1.)).h(px(14.)).bg(p.border))
            .child(h_flex().gap(px(6.)).min_w_0().overflow_hidden().children(parts))
    }

    pub(super) fn creators_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("browser-creators")
            .map(|b| {
                if self.browser.view == View::Creators {
                    b.outline()
                } else {
                    b.ghost()
                }
            })
            .tap_sm()
            .icon(IconName::Users)
            .label(tr("Creators"))
            .on_click(cx.listener(|this, _, _, cx| this.browse_creators(cx)))
    }

    fn creator_chip(&self, c: &Creator, query: &Query, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let marks: Vec<(std::ops::Range<usize>, HighlightStyle)> = query
            .marks(&c.name)
            .into_iter()
            .map(|r| {
                (
                    r,
                    HighlightStyle {
                        color: Some(p.accent),
                        background_color: Some(p.accent.opacity(0.12)),
                        ..HighlightStyle::default()
                    },
                )
            })
            .collect();
        let target = c.id.clone();
        h_flex()
            .id(SharedString::from(format!("browser-found-{}", c.id)))
            .w(px(CREATOR_W - 24.))
            .flex_shrink_0()
            .p(px(12.))
            .gap(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .cursor_pointer()
            .hover(move |s| s.border_color(p.accent).bg(p.hover))
            .child(self.publisher_avatar(&c.id, &c.name, c.avatar.as_deref(), 40., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(StyledText::new(c.name.clone()).with_highlights(marks)),
                    )
                    .child(stats_line(c.addons, c.downloads, c.stars, p)),
            )
            .child(
                Icon::new(IconName::ChevronRight)
                    .size(px(14.))
                    .text_color(p.text_placeholder),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.browse(View::Author(target.clone()), cx)))
            .into_any_element()
    }

    /// The creators whose names match the search, to show above the addons.
    pub(super) fn creators_found(&self, query: &Query, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        if query.is_empty() {
            return None;
        }
        let mut found: Vec<(Creator, u32)> = self
            .browser
            .all_creators()
            .into_iter()
            .filter_map(|c| query.score_name(&c.name).map(|s| (c, s)))
            .collect();
        if found.is_empty() {
            return None;
        }
        found.sort_by(|(a, sa), (b, sb)| sb.cmp(sa).then_with(|| b.downloads.cmp(&a.downloads)));
        let total = found.len();
        let chips: Vec<AnyElement> = found
            .iter()
            .take(CREATORS_FOUND)
            .map(|(c, _)| self.creator_chip(c, query, p, cx))
            .collect();
        let more = (total > CREATORS_FOUND).then(|| {
            text_link(
                "browser-found-all".into(),
                trf!("All {count} creators", count = total),
                p,
            )
            .on_click(cx.listener(|this, _, _, cx| this.browse_creators(cx)))
        });
        Some(
            v_flex()
                .flex_shrink_0()
                .gap(px(10.))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_center()
                        .child(label(tr("CREATORS"), p))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(p.text_placeholder)
                                .child(total.to_string()),
                        )
                        .child(div().flex_1())
                        .children(more),
                )
                .child(h_flex().flex_wrap().gap(px(GAP)).children(chips))
                .child(
                    div()
                        .pt(px(10.))
                        .border_t_1()
                        .border_color(p.border_variant)
                        .child(label(tr("ADDONS"), p)),
                ),
        )
    }

    fn creator_card(
        &self,
        c: &Creator,
        rank: usize,
        width: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = c.id.clone();
        let bio = c
            .bio
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .to_owned();
        v_flex()
            .id(SharedString::from(format!("browser-creator-{}", c.id)))
            .w(px(width))
            .flex_none()
            .p(px(16.))
            .gap(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .cursor_pointer()
            .hover(move |s| s.border_color(p.accent).bg(p.hover))
            .on_click(cx.listener(move |this, _, _, cx| this.browse(View::Author(target.clone()), cx)))
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_center()
                    .child(self.publisher_avatar(&c.id, &c.name, c.avatar.as_deref(), 48., p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(c.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .truncate()
                                    .child(joined(&c.joined).unwrap_or_else(|| tr("Publisher").to_owned())),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if rank <= 3 { p.accent } else { p.text_placeholder })
                            .child(format!("#{rank}")),
                    ),
            )
            .child(
                div()
                    .h(px(38.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(if bio.is_empty() {
                        p.text_placeholder
                    } else {
                        p.text_muted
                    })
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(if bio.is_empty() {
                        tr("No bio yet.").to_owned()
                    } else {
                        bio
                    }),
            )
            .child(
                h_flex()
                    .gap(px(10.))
                    .child(stats_line(c.addons, c.downloads, c.stars, p))
                    .when(c.downloads_30d > 0, |d| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_placeholder)
                                .whitespace_nowrap()
                                .child(trf!(
                                    "{downloads} this month",
                                    downloads = format::compact(c.downloads_30d)
                                )),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(super) fn creators_page(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let creators = self.browser.all_creators();
        let width = self.content_width;
        let columns = ((width + GAP) / (CREATOR_W + GAP)).floor().max(1.) as usize;
        let card_w = ((width - GAP * (columns - 1) as f32) / columns as f32).floor() - 1.;
        let rows: Vec<AnyElement> = creators
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(columns)
            .map(|chunk| {
                h_flex()
                    .flex_shrink_0()
                    .items_stretch()
                    .gap(px(GAP))
                    .children(
                        chunk
                            .iter()
                            .map(|(i, c)| self.creator_card(c, i + 1, card_w, p, cx)),
                    )
                    .into_any_element()
            })
            .collect();
        let empty = creators.is_empty().then(|| {
            div()
                .text_size(px(13.))
                .text_color(p.text_placeholder)
                .child(tr("No creators to show yet."))
        });
        let body = v_flex()
            .id("browser-creators-page")
            .px(px(GUTTER))
            .pb(px(24.))
            .gap(px(GAP))
            .child(self.back_bar(p, cx))
            .child(
                v_flex()
                    .gap(px(2.))
                    .pb(px(4.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .child(tr("Creators")),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(trn(
                        creators.len(),
                        "{count} creator publishing to the registry, most downloaded first",
                        "{count} creators publishing to the registry, most downloaded first",
                    ))),
            )
            .children(rows)
            .children(empty);
        self.scroll_area(body, cx).into_any_element()
    }

    fn author_sort_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shell = cx.entity().downgrade();
        let sort = self.browser.author_sort;
        Button::new("browser-author-sort")
            .outline()
            .tap_sm()
            .icon(IconName::ArrowUpDown)
            .label(sort.label())
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for s in [Sort::Stars, Sort::Downloads, Sort::Updated] {
                    let shell = shell.clone();
                    menu = menu.item(PopupMenuItem::new(s.label()).checked(s == sort).on_click(
                        move |_, _, cx| {
                            shell
                                .update(cx, |this, cx| {
                                    this.browser.author_sort = s;
                                    cx.notify();
                                })
                                .ok();
                        },
                    ));
                }
                menu
            })
    }

    fn author_head(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> Div {
        let b = &self.browser;
        let profile = b.profiles.get(id);
        let creator = b.creator(id);
        let name = b.publisher_name(id);
        let avatar = profile
            .and_then(|pr| pr.avatar.clone())
            .or_else(|| creator.as_ref().and_then(|c| c.avatar.clone()));
        let bio = profile
            .map(|pr| pr.bio.clone())
            .or_else(|| creator.as_ref().map(|c| c.bio.clone()))
            .unwrap_or_default();
        let since = profile
            .map(|pr| pr.joined.clone())
            .or_else(|| creator.as_ref().map(|c| c.joined.clone()))
            .and_then(|j| joined(&j));
        let totals = profile
            .map(|pr| pr.totals)
            .or_else(|| creator.as_ref().map(Creator::totals))
            .unwrap_or_default();
        let loading = profile.is_none() && b.profile_loading.contains(id);
        let links = author_links(profile.map(|pr| pr.links.as_slice()).unwrap_or_default());
        let paragraphs: Vec<AnyElement> = bio
            .lines()
            .map(|l| div().min_h(px(8.)).child(l.to_owned()).into_any_element())
            .collect();
        let about = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(24.))
                    .line_height(px(30.))
                    .font_weight(FontWeight::BOLD)
                    .child(name.clone()),
            )
            .when_some(since, |d, since| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .text_color(p.text_placeholder)
                        .child(Icon::new(IconName::CalendarDays).size(px(13.)))
                        .child(since),
                )
            })
            .child(
                v_flex()
                    .max_w(px(720.))
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(if paragraphs.is_empty() {
                        p.text_placeholder
                    } else {
                        p.text.opacity(0.84)
                    })
                    .when(paragraphs.is_empty() && !loading, |d| d.child(tr("No bio yet.")))
                    .children(paragraphs),
            )
            .when(!links.is_empty(), |d| {
                d.child(h_flex().pt(px(4.)).gap(px(8.)).flex_wrap().children(links))
            })
            .children(self.author_status(id, p, cx));
        v_flex()
            .flex_shrink_0()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .child(
                h_flex()
                    .p(px(24.))
                    .gap(px(20.))
                    .items_start()
                    .child(self.publisher_avatar(id, &name, avatar.as_deref(), 84., p))
                    .child(about),
            )
            .child(author_totals(totals, p))
    }

    fn author_status(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let b = &self.browser;
        let profile = b.profiles.get(id);
        let loading = profile.is_none() && b.profile_loading.contains(id);
        let failed = profile.is_none().then(|| b.profile_failed.get(id)).flatten();
        let target = id.to_owned();
        if loading {
            return Some(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(Spinner::new().icon(IconName::LoaderCircle).with_size(px(12.)))
                    .child(tr("Loading the profile…"))
                    .into_any_element(),
            );
        }
        failed.map(|e| {
            let tip = e.clone();
            h_flex()
                .id("browser-author-failed")
                .gap(px(8.))
                .text_size(px(12.))
                .text_color(p.warning)
                .child(Icon::new(IconName::CircleAlert).size(px(13.)))
                .child(tr("Couldn't load the full profile"))
                .child(
                    text_link("browser-author-retry".into(), tr("Retry"), p)
                        .on_click(cx.listener(move |this, _, _, cx| this.fetch_profile(&target, cx))),
                )
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .into_any_element()
        })
    }

    pub(super) fn author_page(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.browser;
        let name = b.publisher_name(id);
        let loading = !b.profiles.contains_key(id) && b.profile_loading.contains(id);
        let head = self.author_head(id, p, cx);
        let mut addons = b.author_addons(id);
        let sort = b.author_sort;
        addons.sort_by(|x, y| {
            let first = match sort {
                Sort::Downloads => y.downloads.cmp(&x.downloads),
                Sort::Updated => y.updated_at.cmp(&x.updated_at),
                _ => b.stars_of(y).cmp(&b.stars_of(x)),
            };
            first.then_with(|| x.name.to_lowercase().cmp(&y.name.to_lowercase()))
        });
        let count = addons.len();
        let rows = self.card_rows(&addons, &Query::default(), self.content_width, p, cx);
        let empty = (count == 0).then(|| {
            div()
                .text_size(px(13.))
                .text_color(p.text_placeholder)
                .child(if loading {
                    tr("Loading their addons…")
                } else {
                    tr("No listed addons right now.")
                })
        });
        let body = v_flex()
            .id("browser-author")
            .px(px(GUTTER))
            .pb(px(24.))
            .gap(px(GAP))
            .child(self.back_bar(p, cx))
            .child(head)
            .child(
                h_flex()
                    .pt(px(6.))
                    .gap(px(10.))
                    .items_center()
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(trf!("Addons by {name}", name = name)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(count.to_string()),
                    )
                    .child(div().flex_1())
                    .child(self.author_sort_button(cx)),
            )
            .children(rows)
            .children(empty);
        self.scroll_area(body, cx).into_any_element()
    }

    pub(super) fn published_by(&self, a: &Addon, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let owner = a.owner()?;
        let creator = self.browser.creator(&owner.id);
        let bio = creator
            .as_ref()
            .and_then(|c| c.bio.lines().find(|l| !l.trim().is_empty()))
            .map(str::to_owned);
        let target = owner.id.clone();
        let others: Vec<AnyElement> = a
            .publishers
            .iter()
            .filter(|pb| pb.id != owner.id)
            .map(|pb| {
                h_flex()
                    .gap(px(8.))
                    .child(self.publisher_avatar(&pb.id, &pb.name, pb.avatar.as_deref(), 20., p))
                    .child(self.author_link(
                        format!("browser-copublisher-{}", pb.id),
                        &pb.id,
                        &pb.name,
                        13.,
                        p,
                        cx,
                    ))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .p(px(16.))
                .gap(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .child(label(tr("PUBLISHED BY"), p))
                .child(
                    h_flex()
                        .gap(px(12.))
                        .items_center()
                        .child(self.publisher_avatar(&owner.id, &owner.name, owner.avatar.as_deref(), 40., p))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(3.))
                                .child(self.author_link(
                                    "browser-publisher".into(),
                                    &owner.id,
                                    &owner.name,
                                    14.,
                                    p,
                                    cx,
                                ))
                                .when_some(creator.as_ref(), |d, c| {
                                    d.child(stats_line(c.addons, c.downloads, c.stars, p))
                                }),
                        ),
                )
                .when_some(bio, |d, bio| {
                    d.child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(p.text_muted)
                            .line_clamp(3)
                            .text_ellipsis()
                            .child(bio),
                    )
                })
                .when(!others.is_empty(), |d| {
                    d.child(v_flex().gap(px(6.)).child(label(tr("WITH"), p)).children(others))
                })
                .child(
                    text_link("browser-publisher-profile".into(), tr("View profile"), p)
                        .child(Icon::new(IconName::ArrowRight).size(px(12.)))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.browse(View::Author(target.clone()), cx)),
                        ),
                ),
        )
    }

    fn compact_card(&self, a: &Addon, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let target = a.id.clone();
        h_flex()
            .id(SharedString::from(format!("browser-more-{}", a.id)))
            .px(px(16.))
            .py(px(10.))
            .gap(px(12.))
            .items_center()
            .border_t_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .child(self.catalog_icon(a, 36., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(a.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(a.summary.clone()),
                    ),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .items_end()
                    .gap(px(2.))
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .child(
                        h_flex()
                            .gap(px(3.))
                            .when(self.browser.stars.has(&a.id), |d| d.text_color(p.warning))
                            .child(Icon::new(IconName::Star).size(px(11.)))
                            .child(format::compact(self.browser.stars_of(a))),
                    )
                    .child(
                        h_flex()
                            .gap(px(3.))
                            .child(Icon::new(IconName::Download).size(px(11.)))
                            .child(format::compact(a.downloads)),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open_details(&target, cx)))
            .into_any_element()
    }

    pub(super) fn more_from(&self, a: &Addon, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let owner = a.owner()?;
        let others = publishers::more_from(&self.browser.addons, a, MORE_SHOWN);
        if others.is_empty() {
            return None;
        }
        let target = owner.id.clone();
        Some(
            v_flex()
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .overflow_hidden()
                .child(
                    h_flex()
                        .px(px(16.))
                        .h(px(46.))
                        .gap(px(10.))
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(trf!("More from {name}", name = owner.name)),
                        )
                        .child(
                            text_link("browser-more-all".into(), tr("See all"), p)
                                .child(Icon::new(IconName::ArrowRight).size(px(12.)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.browse(View::Author(target.clone()), cx)
                                })),
                        ),
                )
                .children(others.into_iter().map(|o| self.compact_card(o, p, cx))),
        )
    }

    fn small_offer(&self, a: &Addon, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let key = SharedString::from(format!("addon-more-offer-{}", a.id));
        let target = a.id.clone();
        match self.offer(a) {
            Offer::Install => Button::new(key)
                .primary()
                .tap_xs()
                .icon(IconName::Download)
                .label(tr("Install"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.install_from_catalog(&target, cx);
                }))
                .into_any_element(),
            Offer::Failed => Button::new(key)
                .danger()
                .tap_xs()
                .icon(IconName::RotateCw)
                .label(tr("Retry"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.retry_download(&target, cx);
                }))
                .into_any_element(),
            Offer::Queued => Button::new(key)
                .outline()
                .tap_xs()
                .label(tr("Queued"))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_page(crate::shell::Page::Downloads, cx);
                }))
                .into_any_element(),
            Offer::Working(progress) => Button::new(key)
                .primary()
                .tap_xs()
                .loading(true)
                .label(progress.short())
                .disabled(true)
                .into_any_element(),
            Offer::Installed(_) | Offer::Update(..) | Offer::UpdateHere(..) => {
                tag(tr("Installed"), p).text_color(p.success).into_any_element()
            }
            Offer::Separate(_) => tag(tr("Your import has this id"), p)
                .text_color(p.warning)
                .into_any_element(),
            Offer::TooOld => tag(tr("Needs a newer launcher"), p).into_any_element(),
            Offer::Blocked => tag(tr("Missing requirements"), p)
                .text_color(p.warning)
                .into_any_element(),
        }
    }

    fn registry_owner(&self, a: &Addon, from: &str, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        a.owner().map(|o| {
            let (from, target) = (from.to_owned(), o.id.clone());
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(8.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(self.publisher_avatar(&o.id, &o.name, o.avatar.as_deref(), 24., p))
                .child(div().flex_shrink_0().child(tr("by")))
                .child(
                    div()
                        .id("addon-registry-author")
                        .min_w_0()
                        .truncate()
                        .cursor_pointer()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text)
                        .hover(move |s| s.text_color(p.accent).underline())
                        .child(o.name.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.browse_from_addons(&from, View::Author(target.clone()), cx)
                        })),
                )
        })
    }

    fn registry_more(&self, a: &Addon, from: &str, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let others = publishers::more_from(&self.browser.addons, a, MORE_INSTALLED);
        let rows: Vec<AnyElement> = others
            .iter()
            .map(|o| {
                let (from, target) = (from.to_owned(), o.id.clone());
                h_flex()
                    .id(SharedString::from(format!("addon-more-{}", o.id)))
                    .px(px(8.))
                    .py(px(6.))
                    .mx(px(-8.))
                    .gap(px(10.))
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(self.catalog_icon(o, 28., p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(o.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .truncate()
                                    .child(o.summary.clone()),
                            ),
                    )
                    .child(self.small_offer(o, p, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.browse_from_addons(&from, View::Addon(target.clone()), cx)
                    }))
                    .into_any_element()
            })
            .collect();
        (!rows.is_empty()).then(|| {
            let name = a
                .owner()
                .map_or_else(|| tr("this author").to_owned(), |o| o.name.clone());
            v_flex()
                .gap(px(2.))
                .child(
                    div()
                        .pb(px(4.))
                        .child(label(trf!("MORE FROM {name}", name = name.to_uppercase()), p)),
                )
                .children(rows)
        })
    }

    /// The registry's side of an installed addon, for its panel on the Addons page.
    pub(crate) fn registry_section(
        &self,
        id: &str,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let a = self.browser.find(id)?;
        let from = id.to_owned();
        let owner = self.registry_owner(a, &from, p, cx);
        let more = self.registry_more(a, &from, p, cx);
        let view = {
            let from = from.clone();
            text_link("addon-registry-view".into(), tr("View in Addon Browser"), p)
                .child(Icon::new(IconName::ArrowRight).size(px(12.)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.browse_from_addons(&from, View::Addon(from.clone()), cx)
                }))
        };
        Some(
            v_flex()
                .gap(px(10.))
                .child(label(tr("FROM THE ADDON BROWSER"), p))
                .child(
                    h_flex()
                        .gap(px(10.))
                        .items_center()
                        .children(owner)
                        .when(a.owner().is_none(), |d| d.child(div().flex_1()))
                        .child(self.star_button(a, "addon-registry-star".into(), false, p, cx)),
                )
                .child(
                    h_flex()
                        .gap(px(10.))
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .child(Icon::new(IconName::Download).size(px(12.)))
                                .child(trn(a.downloads as usize, "{count} download", "{count} downloads")),
                        )
                        .when(a.downloads_30d > 0, |d| {
                            d.child(trf!(
                                "{count} this month",
                                count = format::compact(a.downloads_30d)
                            ))
                        }),
                )
                .children(more)
                .child(view)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::joined;

    #[test]
    fn names_the_month_someone_joined() {
        assert_eq!(
            joined("2025-12-27T13:13:04Z").as_deref(),
            Some("Joined December 2025")
        );
        assert_eq!(joined("2026-01-01").as_deref(), Some("Joined January 2026"));
        assert!(joined("2026-13-01").is_none());
        assert!(joined("").is_none());
    }
}
