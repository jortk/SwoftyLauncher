use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::{self, Addon};
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf, trn};

use super::detail_cards::detail_about;
use super::*;
use crate::format;
use crate::pages::{GUTTER, label, tag};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;

/// How many of `versions`, newest first, the Addons page shows before its older-versions toggle.
pub(super) fn shown_versions(versions: &[&catalog::Version], installed: &semver::Version) -> usize {
    let through = versions
        .iter()
        .position(|v| v.version <= *installed)
        .map_or(versions.len(), |i| {
            i + usize::from(versions[i].version == *installed)
        });
    through.max(2).min(versions.len())
}

fn changelog_summary(loading: bool, newer: usize, total: usize) -> String {
    if loading {
        tr("Loading…").to_owned()
    } else if newer > 0 {
        trn(
            newer,
            "{count} new version since yours",
            "{count} new versions since yours",
        )
    } else {
        trn(total, "{count} version", "{count} versions")
    }
}

impl Shell {
    /// One registry version and its changelog, for the Addon Browser and the Addons page.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn release_row(
        &self,
        key: &str,
        v: &catalog::Version,
        first: bool,
        installed: Option<&semver::Version>,
        newer: bool,
        inset: f32,
        mono: &SharedString,
        p: Palette,
    ) -> AnyElement {
        let launcher = current_version();
        let fits = v.accepts(&launcher);
        let current = installed == Some(&v.version);
        v_flex()
            .px(px(inset))
            .py(px(12.))
            .gap(px(6.))
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("v{}", v.version)),
                    )
                    .when(current, |d| {
                        d.child(tag(tr("Installed"), p).text_color(p.success))
                    })
                    .when(newer, |d| d.child(tag(tr("New"), p).text_color(p.accent)))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(format!("{} · {}", day(&v.published_at), format::size(v.size))),
                    ),
            )
            .when(!fits, |d| {
                d.child(div().text_size(px(12.)).text_color(p.warning).child(trf!(
                    "Needs launcher {version}",
                    version = v.launcher.as_ref().map_or_else(String::new, |r| r.to_string())
                )))
            })
            .when_some(
                v.requires
                    .as_ref()
                    .filter(|r| !r.is_empty())
                    .map(|r| self.requires_text(r)),
                |d, text| d.child(div().text_size(px(12.)).text_color(p.text_muted).child(text)),
            )
            .child(
                v_flex()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .when(v.changelog.trim().is_empty(), |d| {
                        d.child(div().text_color(p.text_placeholder).child(tr("No changelog")))
                    })
                    .when(!v.changelog.trim().is_empty(), |d| {
                        d.children(crate::news::render_markdown(
                            &format!("{key}-{}", v.version),
                            &v.changelog,
                            p,
                            mono,
                        ))
                    }),
            )
            .into_any_element()
    }

    /// The registry's changelog of an installed addon, for its panel on the Addons page.
    pub(crate) fn changelog_section(
        &mut self,
        id: &str,
        installed: &semver::Version,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.browser.has(id) {
            return None;
        }
        if !self.browser.details.contains_key(id) && self.browser.asked.insert(id.to_owned()) {
            let id = id.to_owned();
            cx.spawn(async move |this, cx| {
                this.update(cx, |s, cx| s.fetch_details(&id, cx)).ok();
            })
            .detach();
        }
        let listed = self.browser.find(id)?;
        let full = self.browser.details.get(id);
        let mut versions: Vec<&catalog::Version> = full.map(Addon::listed).unwrap_or_default();
        for v in listed.listed() {
            if !versions.iter().any(|o| o.version == v.version) {
                versions.push(v);
            }
        }
        versions.sort_by(|a, b| b.version.cmp(&a.version));
        let loading = full.is_none() && self.browser.loading.contains(id);
        if versions.is_empty() && !loading {
            return None;
        }
        let newer = versions.iter().filter(|v| v.version > *installed).count();
        let shown = shown_versions(&versions, installed);
        let older = versions.len() - shown;
        let open = self.browser.history.contains(id);
        let mono = cx.theme().mono_font_family.clone();
        let key = format!("addon-changelog-{id}");
        let rows: Vec<AnyElement> = versions
            .iter()
            .take(if open { versions.len() } else { shown })
            .enumerate()
            .map(|(i, v)| {
                self.release_row(
                    &key,
                    v,
                    i == 0,
                    Some(installed),
                    v.version > *installed,
                    0.,
                    &mono,
                    p,
                )
            })
            .collect();
        let summary = changelog_summary(loading, newer, versions.len());
        let toggle = (older > 0).then(|| {
            let target = id.to_owned();
            h_flex()
                .id("addon-changelog-older")
                .pt(px(10.))
                .gap(px(6.))
                .items_center()
                .border_t_1()
                .border_color(p.border_variant)
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .hover(move |s| s.text_color(p.text_muted))
                .child(crate::pages::chevron(open, p))
                .child(if open {
                    tr("Hide older versions").to_owned()
                } else {
                    trn(older, "Show {count} older version", "Show {count} older versions")
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.browser.history.remove(&target) {
                        this.browser.history.insert(target.clone());
                    }
                    cx.notify();
                }))
        });
        Some(
            v_flex()
                .child(
                    h_flex()
                        .pb(px(2.))
                        .items_center()
                        .child(div().flex_1().child(label(tr("CHANGELOG"), p)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_placeholder)
                                .child(summary),
                        ),
                )
                .children(rows)
                .children(toggle)
                .into_any_element(),
        )
    }

    pub(super) fn browser_detail(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(listed) = self.browser.find(id) else {
            return div().into_any_element();
        };
        let full = self.browser.details.get(id);
        let a = full.unwrap_or(listed);
        let mono = cx.theme().mono_font_family.clone();
        let installed = self.installed_version(&a.id);
        let loading = full.is_none() && self.browser.loading.contains(id);
        let side = v_flex()
            .w(px(SIDE_W))
            .flex_shrink_0()
            .gap(px(16.))
            .children(self.ad_addon(SIDE_W, p))
            .children(self.published_by(listed, p, cx))
            .children(self.more_from(listed, p, cx))
            .child(self.browser_versions(a, installed.as_ref(), loading, &mono, p));
        let main = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(16.))
            .children(self.browser_gallery(a, p, cx))
            .children(self.trend_chart(a, p))
            .child(detail_about(a, &mono, p));
        let body = v_flex()
            .id("browser-detail")
            .px(px(GUTTER))
            .pb(px(24.))
            .gap(px(18.))
            .child(self.back_bar(p, cx))
            .child(self.browser_hero(listed, a, p, cx))
            .children(self.browser_working(a, p))
            .children(self.browser_failure(a, installed.is_some(), cx))
            .children(self.browser_needs(listed, installed.is_some()))
            .child(self.browser_facts(listed, a, installed.as_ref(), &mono, p))
            .child(h_flex().items_start().gap(px(16.)).child(main).child(side));
        self.scroll_area(body, cx).into_any_element()
    }
}
