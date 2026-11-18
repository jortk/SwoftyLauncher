use crate::notices::{Notification, Notify};
use crate::pages::GUTTER;
use crate::scale::{Hit, px};
use crate::shell::{Page, Shell};
use crate::theme::Palette;
use crate::ui::tokens::Tone;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use swofty_core::order;
use swofty_i18n::{tr, trf, trn};

impl Shell {
    pub(super) fn installed_manifests(
        &self,
    ) -> std::collections::BTreeMap<String, swofty_core::manifest::AddonManifest> {
        self.library
            .addons
            .iter()
            .map(|a| (a.manifest.id.clone(), a.manifest.clone()))
            .collect()
    }

    pub(super) fn refresh_problems(&mut self) {
        let built = self.addons.report.as_ref().map_or(0, |r| r.built_at.to_bits());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.library.profile.enabled().for_each(|id| id.hash(&mut hasher));
        let key = (self.library.loaded, built, hasher.finish());
        if self.addons.problems_key == Some(key) {
            return;
        }
        self.addons.problems_key = Some(key);
        self.addons.diagnosis = self.diagnose_now();
    }

    fn diagnose_now(&mut self) -> order::Diagnosis {
        if self.vanilla() {
            return order::Diagnosis::empty();
        }
        let installed = self.installed_manifests();
        self.addons.careers =
            swofty_core::stdlib::careers::read(&swofty_core::stdlib::careers::data_path(&self.store.root));
        let tracks: HashSet<&str> = self
            .library
            .addons
            .iter()
            .filter(|a| a.track)
            .map(|a| a.manifest.id.as_str())
            .collect();
        let store = &self.store;
        let pinned = |id: &str| store.is_builtin(id) || order::is_pinned(id);
        order::diagnose(
            &self.library.profile,
            &installed,
            self.addons.report.as_ref(),
            &self.addons.careers,
            &|id| tracks.contains(id),
            &pinned,
            &|id| self.library.place(id),
        )
    }

    pub(super) fn problems_for(&self, id: &str) -> Vec<&order::Problem> {
        self.addons
            .diagnosis
            .problems
            .iter()
            .filter(|p| p.addon == id)
            .collect()
    }

    pub(super) fn attention(&self) -> usize {
        let ids: HashSet<&str> = self
            .addons
            .diagnosis
            .problems
            .iter()
            .filter(|p| !p.fixable_by_reorder())
            .map(|p| p.addon.as_str())
            .collect();
        ids.len()
    }

    pub(super) fn problem_color(p: Palette, severity: order::Severity) -> Hsla {
        match severity {
            order::Severity::Red => p.error,
            order::Severity::Amber => p.warning,
        }
    }

    fn default_place(&self, id: &str) -> Option<swofty_core::folders::Key> {
        let a = self.library.addons.iter().find(|a| a.manifest.id == id)?;
        swofty_core::folders::default_place(a.source == crate::library::Source::Builtin, a.category())
    }

    pub(super) fn fix_load_order(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let order::FixState::Ready(plan, _) = self.addons.diagnosis.fix.clone() else {
            return;
        };
        let profile = self.library.profile.name.clone();
        if let Err(e) = self
            .store
            .apply_fix(&profile, &plan, &|id| self.default_place(id))
        {
            crate::app_log::action("fix load order").failed(&e);
            window.notify(
                Notification::error(e.to_string()).title(tr("Could not fix the load order")),
                cx,
            );
            return;
        }
        self.reload(cx);
        self.refresh_problems();
        let left: Vec<String> = match &self.addons.diagnosis.fix {
            order::FixState::Ready(..) => self
                .addons
                .diagnosis
                .problems
                .iter()
                .filter(|p| p.fixable_by_reorder())
                .map(|p| self.addon_name(&p.addon))
                .collect(),
            _ => Vec::new(),
        };
        if !left.is_empty() {
            let text = trf!(
                "The load order still has problems for {names}",
                names = left.join(", ")
            );
            crate::app_log::action("fix load order").failed(&text);
            window.notify(Notification::error(text), cx);
            return;
        }
        window.notify(
            Notification::success(trn(
                plan.moved,
                "Reordered {count} addon",
                "Reordered {count} addons",
            )),
            cx,
        );
        if !plan.ejected.is_empty() {
            let names: Vec<String> = plan.ejected.iter().map(|(id, _)| self.addon_name(id)).collect();
            let text = trn(
                plan.ejected.len(),
                "Moved {names} out of its folder so the order holds",
                "Moved {names} out of their folders so the order holds",
            )
            .replace("{names}", &names.join(", "));
            window.notify(Notification::info(text), cx);
        }
        if self.addons.report.is_some() {
            self.check_conflicts(window, cx);
        }
    }

    pub(super) fn turn_off_for_cycle(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.set_addon_enabled(id, false, window, cx);
    }

    pub(super) fn install_missing(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.browser.has(id) {
            true => self.install_from_catalog(id, cx),
            false => self.open_page(Page::Browser, cx),
        }
    }

    pub(super) fn problem_actions(
        &self,
        scope: &str,
        pr: &order::Problem,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let name = |id: &str| self.addon_name(id);
        let key = |what: &str| SharedString::from(format!("fix-{scope}-{what}-{}-{}", pr.addon, pr.other));
        let (addon, other) = (pr.addon.clone(), pr.other.clone());
        let button = |id: SharedString, label: String| Button::new(id).tap_sm().outline().label(label);
        let mut out: Vec<AnyElement> = Vec::new();
        match &pr.kind {
            order::Kind::Missing => {
                let id = other.clone();
                out.push(
                    button(key("install"), trf!("Install {name}", name = name(&other)))
                        .on_click(cx.listener(move |this, _, _, cx| this.install_missing(&id, cx)))
                        .into_any_element(),
                );
            }
            order::Kind::Off => {
                let id = other.clone();
                out.push(
                    button(key("on"), trf!("Turn {name} on", name = name(&other)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_addon_enabled(&id, true, window, cx)
                        }))
                        .into_any_element(),
                );
            }
            order::Kind::Below | order::Kind::Waits => {}
            order::Kind::Version { .. } => {
                out.push(
                    button(key("update"), tr("Open Addon Browser").to_owned())
                        .on_click(cx.listener(|this, _, _, cx| this.open_page(Page::Browser, cx)))
                        .into_any_element(),
                );
            }
            order::Kind::Conflict | order::Kind::Clash { .. } => {
                let id = other.clone();
                out.push(
                    button(key("off"), trf!("Turn {name} off", name = name(&other)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_addon_enabled(&id, false, window, cx)
                        }))
                        .into_any_element(),
                );
            }
            order::Kind::Career { why, .. } => {
                let id = addon.clone();
                match &**why {
                    order::Kind::Missing => out.push(
                        button(key("install"), trf!("Install {name}", name = name(&addon)))
                            .on_click(cx.listener(move |this, _, _, cx| this.install_missing(&id, cx)))
                            .into_any_element(),
                    ),
                    order::Kind::Off => out.push(
                        button(key("on"), trf!("Turn {name} on", name = name(&addon)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_addon_enabled(&id, true, window, cx)
                            }))
                            .into_any_element(),
                    ),
                    _ => out.push(
                        button(key("update"), tr("Open Addon Browser").to_owned())
                            .on_click(cx.listener(|this, _, _, cx| this.open_page(Page::Browser, cx)))
                            .into_any_element(),
                    ),
                }
            }
        }
        out
    }

    pub(super) fn problem_list(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let found = self.problems_for(id);
        if found.is_empty() {
            return None;
        }
        let name = |id: &str| self.addon_name(id);
        let rows: Vec<AnyElement> = found
            .iter()
            .map(|f| {
                h_flex()
                    .gap(px(8.))
                    .items_start()
                    .flex_wrap()
                    .child(
                        Icon::new(IconName::TriangleAlert)
                            .size(px(14.))
                            .mt(px(2.))
                            .text_color(Self::problem_color(p, f.severity())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.))
                            .text_size(px(13.))
                            .text_color(p.text)
                            .child(f.line(&name)),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .flex_wrap()
                            .children(self.problem_actions("detail", f, cx)),
                    )
                    .into_any_element()
            })
            .collect();
        let ordered = found.iter().any(|f| f.fixable_by_reorder());
        Some(
            v_flex()
                .gap(px(6.))
                .children(rows)
                .when(ordered, |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(tr("Fix load order at the top of the page puts it right.")),
                    )
                })
                .into_any_element(),
        )
    }

    pub(super) fn status_icon(&self, id: &str, p: Palette) -> Option<AnyElement> {
        let name = |id: &str| self.addon_name(id);
        let lines = status_icon_lines(&self.addons.diagnosis, id, &name)?;
        let color = self
            .problems_for(id)
            .iter()
            .map(|f| f.severity())
            .find(|s| *s == order::Severity::Red)
            .map_or(p.warning, |_| p.error);
        let tip = lines.join("\n");
        Some(
            div()
                .id(SharedString::from(format!("problem-{id}")))
                .flex_shrink_0()
                .child(Icon::new(IconName::TriangleAlert).size(px(14.)).text_color(color))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .into_any_element(),
        )
    }

    pub(super) fn fix_bar(&self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let name = |id: &str| self.addon_name(id);
        let (tone, title, body, button) = match &self.addons.diagnosis.fix {
            order::FixState::Nothing => return None,
            order::FixState::Ready(_, count) => (
                Tone::Warning,
                trn(
                    *count,
                    "{count} addon loads in the wrong order",
                    "{count} addons load in the wrong order",
                ),
                tr("Fix load order moves them below what they need. Nothing else changes.").to_owned(),
                Button::new("fix-order")
                    .primary()
                    .large()
                    .h(px(46.))
                    .min_w(px(210.))
                    .icon(IconName::ArrowUpDown)
                    .label(trf!("Fix load order ({count})", count = count))
                    .on_click(cx.listener(|this, _, window, cx| this.fix_load_order(window, cx))),
            ),
            order::FixState::Blocked(e) => {
                let off = e.turn_off()?.to_owned();
                let label = trf!("Turn off {name}", name = name(&off));
                (
                    Tone::Error,
                    tr("These addons can't all load in a good order").to_owned(),
                    e.line(&name),
                    Button::new("fix-order-off")
                        .danger()
                        .large()
                        .h(px(46.))
                        .min_w(px(210.))
                        .icon(IconName::PowerOff)
                        .label(label)
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.turn_off_for_cycle(&off, window, cx)),
                        ),
                )
            }
        };
        let color = tone.color(&p);
        Some(
            h_flex()
                .mx(px(GUTTER))
                .mb(px(12.))
                .px(px(16.))
                .py(px(12.))
                .gap(px(14.))
                .flex_shrink_0()
                .items_center()
                .rounded(px(8.))
                .border_1()
                .border_color(color.opacity(0.6))
                .bg(color.opacity(0.1))
                .child(
                    Icon::new(tone.icon())
                        .size(px(22.))
                        .flex_shrink_0()
                        .text_color(color),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(200.))
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.text)
                                .child(title),
                        )
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(body)),
                )
                .child(button)
                .into_any_element(),
        )
    }
}

/// The lines a row's warning triangle shows, or `None` when the addon has no problem.
pub(super) fn status_icon_lines(
    diagnosis: &order::Diagnosis,
    id: &str,
    name: &dyn Fn(&str) -> String,
) -> Option<Vec<String>> {
    let mut lines: Vec<String> = diagnosis
        .problems
        .iter()
        .filter(|p| p.addon == id)
        .map(|p| p.line(name))
        .collect();
    if lines.is_empty() {
        return None;
    }
    lines.push(tr("Click the row for options").to_owned());
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::status_icon_lines;
    use swofty_core::order;

    fn problem(addon: &str, kind: order::Kind) -> order::Problem {
        order::Problem {
            addon: addon.into(),
            other: "dep".into(),
            kind,
        }
    }

    #[test]
    fn a_row_has_an_icon_only_when_it_has_a_problem() {
        let diagnosis = order::Diagnosis {
            problems: vec![problem("a", order::Kind::Below), problem("b", order::Kind::Off)],
            fix: order::FixState::Nothing,
        };
        let name = |id: &str| id.to_owned();
        let lines = status_icon_lines(&diagnosis, "a", &name).unwrap();
        assert_eq!(lines[0], "Needs dep above it");
        assert_eq!(lines.len(), 2);
        assert!(status_icon_lines(&diagnosis, "c", &name).is_none());
    }
}
