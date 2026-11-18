use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf, trn};

use super::{day, since};
use crate::format;
use crate::pages::{label, stat};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::{Callout, ProgressBar};

pub(super) fn detail_about(a: &Addon, mono: &SharedString, p: Palette) -> Div {
    let description = if a.description.trim().is_empty() {
        vec![
            div()
                .text_color(p.text.opacity(0.84))
                .child(if a.summary.is_empty() {
                    tr("No description yet.").to_owned()
                } else {
                    a.summary.clone()
                })
                .into_any_element(),
        ]
    } else {
        crate::news::render_markdown(&format!("browser-{}", a.id), &a.description, p, mono)
    };
    v_flex()
        .p(px(24.))
        .rounded(px(10.))
        .border_1()
        .border_color(p.border)
        .bg(p.surface)
        .text_size(px(14.))
        .line_height(px(22.))
        .child(div().pb(px(12.)).child(label(tr("ABOUT"), p)))
        .children(description)
}

impl Shell {
    pub(super) fn browser_hero(&self, listed: &Addon, a: &Addon, p: Palette, cx: &mut Context<Self>) -> Div {
        let homepage = a
            .homepage
            .clone()
            .filter(|u| u.starts_with("https://"))
            .map(|url| {
                Button::new("browser-homepage")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::ExternalLink)
                    .label(tr("Homepage"))
                    .on_click(move |_, _, cx| cx.open_url(&url))
            });
        let source_code = a.source_url().map(str::to_owned).map(|url| {
            Button::new("browser-source-code")
                .ghost()
                .tap_sm()
                .icon(IconName::ExternalLink)
                .label(tr("Source code"))
                .on_click(move |_, _, cx| cx.open_url(&url))
        });
        let title = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(22.))
                    .line_height(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .child(a.name.clone()),
            )
            .child(
                h_flex()
                    .gap(px(4.))
                    .min_w_0()
                    .child(self.byline(a, "browser-detail-by".into(), 13., p, cx))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(13.))
                            .text_color(p.text_placeholder)
                            .child(format!("· {}", a.id)),
                    ),
            )
            .children(self.tags(a, "browser-detail-native".into(), p));
        h_flex()
            .gap(px(16.))
            .items_start()
            .flex_shrink_0()
            .child(self.catalog_icon(a, 64., p))
            .child(title)
            .child(
                h_flex()
                    .gap(px(8.))
                    .flex_shrink_0()
                    .child(self.star_button(listed, "browser-detail-star".into(), true, p, cx))
                    .children(homepage)
                    .children(source_code)
                    .child(self.offer_button(listed, "browser-detail-offer".into(), cx)),
            )
    }

    pub(super) fn browser_working(&self, a: &Addon, p: Palette) -> Option<Div> {
        let job = self.browser.working.get(&a.id)?;
        let pr = job.progress();
        let item = job.item();
        let also = job.also();
        Some(
            v_flex()
                .gap(px(6.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.accent.opacity(0.4))
                .bg(p.accent.opacity(0.06))
                .child(div().text_size(px(13.)).child(pr.long(item.as_deref())))
                .when(!also.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(trf!("Also installing: {names}", names = also.join(", "))),
                    )
                })
                .child(ProgressBar::new(pr.fraction())),
        )
    }

    pub(super) fn browser_failure(
        &self,
        a: &Addon,
        installed: bool,
        cx: &mut Context<Self>,
    ) -> Option<Callout> {
        let error = self.browser.failed.get(&a.id)?;
        let target = a.id.clone();
        let title = if self.browser.refused.contains(&a.id) {
            tr("Missing requirements")
        } else if installed {
            tr("The update failed")
        } else {
            tr("The install failed")
        };
        let retry = Button::new("browser-detail-retry")
            .outline()
            .tap_sm()
            .icon(IconName::RotateCw)
            .label(tr("Retry"))
            .on_click(cx.listener(move |this, _, _, cx| this.install_from_catalog(&target, cx)));
        Some(
            Callout::error(error.clone())
                .icon(IconName::CircleAlert)
                .title(title)
                .actions(retry),
        )
    }

    pub(super) fn browser_needs(&self, listed: &Addon, installed: bool) -> Option<Callout> {
        let idle = !self.browser.working.contains_key(&listed.id)
            && !self.browser.failed.contains_key(&listed.id)
            && !installed;
        if !idle {
            return None;
        }
        match self.requirements(listed)? {
            Ok(plan) if plan.also().is_empty() => None,
            Ok(plan) => {
                let names = plan
                    .also()
                    .iter()
                    .map(|s| match &s.replaces {
                        Some(old) => trf!("{name} (replaces {old})", name = s.label(), old = old),
                        None => s.label(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(
                    Callout::info(names)
                        .icon(IconName::Package)
                        .title(tr("Also installs")),
                )
            }
            Err(e) => Some(Callout::warning(e.to_string()).title(tr("Missing requirements"))),
        }
    }

    pub(super) fn browser_facts(
        &self,
        listed: &Addon,
        a: &Addon,
        installed: Option<&semver::Version>,
        mono: &SharedString,
        p: Palette,
    ) -> Div {
        let launcher = current_version();
        let offered = a.offered(&launcher).or_else(|| a.newest());
        let requirement = offered
            .and_then(|v| v.launcher.as_ref())
            .map_or_else(|| tr("Any launcher").to_owned(), |r| r.to_string());
        let fits = offered.is_some_and(|v| v.accepts(&launcher));
        let sha = offered
            .map(|v| v.sha256.chars().take(12).collect::<String>())
            .unwrap_or_default();
        h_flex()
            .flex_wrap()
            .gap_x(px(28.))
            .gap_y(px(14.))
            .p(px(16.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .child(stat(
                tr("STARS"),
                format::compact(self.browser.stars_of(listed)),
                p.text,
                p,
            ))
            .child(stat(
                tr("DOWNLOADS"),
                trf!(
                    "{total} · {recent} in 30 days",
                    total = format::compact(a.downloads),
                    recent = format::compact(a.downloads_30d)
                ),
                p.text,
                p,
            ))
            .child(stat(
                tr("SIZE"),
                offered.map(|v| format::size(v.size)).unwrap_or_default(),
                p.text,
                p,
            ))
            .child(stat(
                tr("LAST UPDATED"),
                since(&a.updated_at).unwrap_or_else(|| "–".into()),
                p.text,
                p,
            ))
            .child(stat(tr("FIRST PUBLISHED"), day(&a.created_at), p.text, p))
            .child(stat(
                tr("LAUNCHER"),
                requirement,
                if fits { p.text } else { p.warning },
                p,
            ))
            .child(
                v_flex().gap(px(3.)).child(label("SHA-256", p)).child(
                    div()
                        .text_size(px(13.))
                        .font_family(mono.clone())
                        .child(format!("{sha}…")),
                ),
            )
            .when_some(installed, |d, v| {
                d.child(stat(tr("INSTALLED"), format!("v{v}"), p.success, p))
            })
    }

    pub(super) fn browser_versions(
        &self,
        a: &Addon,
        installed: Option<&semver::Version>,
        loading: bool,
        mono: &SharedString,
        p: Palette,
    ) -> Div {
        let listed = a.listed();
        let count = listed.len();
        let key = format!("browser-{}", a.id);
        let rows: Vec<AnyElement> = listed
            .into_iter()
            .enumerate()
            .map(|(i, v)| self.release_row(&key, v, i == 0, installed, false, 16., mono, p))
            .collect();
        let summary = if loading {
            tr("Loading…").to_owned()
        } else {
            trn(count, "{count} version", "{count} versions")
        };
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
                    .items_center()
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Versions")),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(summary),
                    ),
            )
            .children(rows)
    }
}
