use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::catalog::search::Query;
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf, trn};

use super::*;
use crate::format;
use crate::pages::{badge, tag};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::ProgressBar;

pub(super) fn card_border(failed: bool, updating: bool, p: Palette) -> Hsla {
    if failed {
        p.error.opacity(0.6)
    } else if updating {
        p.accent.opacity(0.6)
    } else {
        p.border
    }
}

fn highlighted_name(a: &Addon, query: &Query, p: Palette) -> StyledText {
    let marks: Vec<(std::ops::Range<usize>, HighlightStyle)> = query
        .marks(&a.name)
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
    StyledText::new(a.name.clone()).with_highlights(marks)
}

fn card_stats(a: &Addon) -> Vec<AnyElement> {
    let mut stats = vec![
        h_flex()
            .gap(px(4.))
            .child(Icon::new(IconName::Download).size(px(12.)))
            .child(format::compact(a.downloads))
            .into_any_element(),
    ];
    if a.downloads_30d > 0 {
        stats.push(
            div()
                .child(trf!(
                    "{count} this month",
                    count = format::compact(a.downloads_30d)
                ))
                .into_any_element(),
        );
    }
    if let Some(when) = since(&a.updated_at) {
        stats.push(
            div()
                .child(trf!("Updated {when}", when = when))
                .into_any_element(),
        );
    }
    stats
}

fn card_version_line(
    a: &Addon,
    version: Option<&semver::Version>,
    also: &[String],
    p: Palette,
) -> Stateful<Div> {
    let version = version.map(|v| format!("v{v}")).unwrap_or_default();
    let line = div()
        .id(SharedString::from(format!("browser-card-also-{}", a.id)))
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(px(12.))
        .text_color(p.text_placeholder);
    if also.is_empty() {
        return line.child(version);
    }
    let tip = trf!("Also installs: {names}", names = also.join(", "));
    line.child(format!("{version} · {tip}"))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
}

fn card_note(id: String, icon: IconName, color: Hsla, text: String) -> Stateful<Div> {
    let tip = text.clone();
    h_flex()
        .id(SharedString::from(id))
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(color)
        .child(Icon::new(icon).size(px(12.)))
        .child(div().flex_1().min_w_0().truncate().child(text))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
}

fn platform_tag(w: Where, p: Palette) -> impl IntoElement {
    let tested = w != Where::NotTested;
    tag(w.label(), p).when(tested, |d| {
        d.text_color(p.success)
            .bg(p.success.opacity(0.1))
            .border_color(p.success.opacity(0.35))
    })
}

pub(super) fn native_badge(id: String, p: Palette) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(id))
        .h(px(20.))
        .px(px(6.))
        .gap(px(4.))
        .flex_shrink_0()
        .rounded(px(4.))
        .bg(p.warning.opacity(0.14))
        .text_color(p.warning)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(Icon::new(IconName::Cpu).size(px(11.)))
        .child(tr("Runs code in the game"))
        .tooltip(|window, cx| {
            Tooltip::new(tr("It ships native code that runs inside F1 25 while it's on.")).build(window, cx)
        })
}

impl Shell {
    pub(super) fn star_button(
        &self,
        a: &Addon,
        id: String,
        big: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let on = self.browser.stars.has(&a.id);
        let n = self.browser.stars_of(a);
        let target = a.id.clone();
        let color = if on { p.warning } else { p.text_muted };
        h_flex()
            .id(SharedString::from(id))
            .h(px(if big { 28. } else { 24. }))
            .px(px(if big { 10. } else { 7. }))
            .gap(px(5.))
            .flex_shrink_0()
            .rounded(px(5.))
            .border_1()
            .border_color(if on { p.warning.opacity(0.5) } else { p.border })
            .when(on, |d| d.bg(p.warning.opacity(0.1)))
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover).border_color(p.warning.opacity(0.7)))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
            .child(Icon::new(IconName::Star).size(px(13.)).text_color(color))
            .when(big, |d| d.child(if on { tr("Starred") } else { tr("Star") }))
            .child(
                div()
                    .text_color(if on { p.text } else { p.text_muted })
                    .child(format::compact(n)),
            )
            .tooltip(move |window, cx| {
                Tooltip::new(if on {
                    tr("You starred this. Click to unstar it.")
                } else {
                    tr("Star it. Stars sync to every launcher you use.")
                })
                .build(window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_star(&target, cx);
            }))
    }

    pub(crate) fn catalog_icon_of(&self, id: &str, size: f32, p: Palette) -> Option<Div> {
        let a = self.browser.find(id)?;
        a.icon
            .as_ref()
            .is_some_and(|u| self.browser.icons.contains_key(u))
            .then(|| self.catalog_icon(a, size, p))
    }

    pub(super) fn catalog_icon(&self, a: &Addon, size: f32, p: Palette) -> Div {
        match a.icon.as_ref().and_then(|u| self.browser.icons.get(u)) {
            Some(image) => div()
                .size(px(size))
                .flex_shrink_0()
                .rounded(px(size * 0.22))
                .overflow_hidden()
                .child(img(image.clone()).size_full()),
            None => badge(IconName::Package, size, p),
        }
    }

    pub(super) fn tags(&self, a: &Addon, id: String, p: Palette) -> Option<Div> {
        let category = a.category_label();
        let kinds = a.kind_labels();
        let native = a.native(&current_version());
        let platforms = Where::tags(a.platforms());
        Some(
            h_flex()
                .flex_wrap()
                .gap(px(6.))
                .when_some(category, |d, c| {
                    d.child(
                        tag(c, p)
                            .text_color(p.text)
                            .bg(p.accent.opacity(0.1))
                            .border_color(p.accent.opacity(0.35)),
                    )
                })
                .children(platforms.into_iter().map(|w| platform_tag(w, p)))
                .children(kinds.into_iter().map(|k| tag(k, p)))
                .when(native, |d| d.child(native_badge(id, p))),
        )
    }

    pub(super) fn browser_card(
        &self,
        a: &Addon,
        query: &Query,
        width: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let launcher = current_version();
        let offered = a.offered(&launcher).or_else(|| a.newest());
        let updating = self.catalog_update(&a.id).is_some();
        let failed = self.browser.failed.get(&a.id).cloned();
        let (also, blocked) = self.needs_of(a);
        let blocked = blocked.filter(|_| failed.is_none());
        let progress = self.browser.working.get(&a.id).map(Job::progress);
        let id = a.id.clone();
        let name = highlighted_name(a, query, p);
        let author = self.byline(a, format!("browser-card-by-{}", a.id), 12., p, cx);
        v_flex()
            .id(SharedString::from(format!("browser-card-{}", a.id)))
            .w(px(width))
            .flex_none()
            .p(px(16.))
            .gap(px(10.))
            .rounded(px(10.))
            .border_1()
            .border_color(card_border(failed.is_some(), updating, p))
            .bg(p.surface)
            .cursor_pointer()
            .hover(move |s| s.border_color(p.accent).bg(p.hover))
            .on_click(cx.listener(move |this, _, _, cx| this.open_details(&id, cx)))
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_start()
                    .child(self.catalog_icon(a, 44., p))
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
                                    .child(name),
                            )
                            .child(author),
                    )
                    .child(self.star_button(a, format!("browser-star-{}", a.id), false, p, cx)),
            )
            .child(
                div()
                    .h(px(38.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(a.summary.clone()),
            )
            .children(self.tags(a, format!("browser-native-{}", a.id), p))
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(10.))
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .children(card_stats(a)),
            )
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_center()
                    .child(card_version_line(a, offered.map(|v| &v.version), &also, p))
                    .child(self.offer_button(a, format!("browser-offer-{}", a.id), cx)),
            )
            .when_some(progress, |d, pr| {
                d.child(ProgressBar::new(pr.fraction()).height(3.))
            })
            .when_some(failed, |d, e| {
                d.child(card_note(
                    format!("browser-failed-{}", a.id),
                    IconName::CircleAlert,
                    p.error,
                    e,
                ))
            })
            .when_some(blocked, |d, e| {
                d.child(card_note(
                    format!("browser-blocked-{}", a.id),
                    IconName::TriangleAlert,
                    p.warning,
                    e,
                ))
            })
            .into_any_element()
    }

    pub(super) fn trend_chart(&self, a: &Addon, p: Palette) -> Option<Div> {
        let trend = a.trend.as_ref().filter(|t| !t.downloads.is_empty())?;
        let max = trend.downloads.iter().copied().max().unwrap_or(0).max(1);
        let total: u64 = trend.downloads.iter().sum();
        let bars: Vec<AnyElement> = trend
            .downloads
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let when = trend.days.get(i).map(|d| day(d)).unwrap_or_default();
                let tip = trf!(
                    "{when}: {downloads}",
                    when = when,
                    downloads = trn(*n as usize, "{count} download", "{count} downloads")
                );
                div()
                    .id(SharedString::from(format!("browser-trend-{i}")))
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .hover(move |s| s.bg(p.hover))
                    .child(
                        div()
                            .w_full()
                            .h(px((54. * *n as f32 / max as f32).max(2.)))
                            .rounded_t(px(2.))
                            .bg(if *n == 0 { p.border } else { p.accent.opacity(0.75) }),
                    )
                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .p(px(16.))
                .gap(px(10.))
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .child(
                    h_flex()
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr("Downloads, last 30 days")),
                        )
                        .child(div().text_size(px(12.)).text_color(p.text_placeholder).child(trn(
                            total as usize,
                            "{count} download",
                            "{count} downloads",
                        ))),
                )
                .child(h_flex().h(px(56.)).gap(px(2.)).items_end().children(bars))
                .child(
                    h_flex()
                        .justify_between()
                        .text_size(px(11.))
                        .text_color(p.text_placeholder)
                        .child(trend.days.first().map(|d| day(d)).unwrap_or_default())
                        .child(trend.days.last().map(|d| day(d)).unwrap_or_default()),
                ),
        )
    }
}
