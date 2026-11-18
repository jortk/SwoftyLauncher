use super::ITEMS_SHOWN;
use super::row::stock_removals_text;
use crate::format::{ago_since, title_case};
use crate::library::AddonItem;
use crate::notices::{Notification, Notify};
use crate::pages::{chevron, label};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::conflicts::{self, Pair};
use swofty_core::profile::VANILLA;
use swofty_i18n::{tr, trf, trn};

pub(super) fn pair_counts(pair: &Pair) -> String {
    let strings = pair.strings;
    let parts: Vec<String> = [
        (pair.database, "{count} database value", "{count} database values"),
        (pair.resources, "{count} game asset", "{count} game assets"),
        (pair.files, "{count} file", "{count} files"),
        (strings, "{count} text string", "{count} text strings"),
    ]
    .into_iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, one, many)| trn(n, one, many))
    .collect();
    match parts.as_slice() {
        [] => trn(pair.items.len(), "{count} item", "{count} items"),
        [one] => one.clone(),
        [rest @ .., last] => trf!("{rest} and {last}", rest = rest.join(", "), last = last),
    }
}

pub(super) fn unbracket(path: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// A conflict item's group (an archive, a database table, text or a folder)
/// and its name within the group.
pub(super) fn item_group(item: &str) -> (String, String) {
    if let Some(key) = item.strip_prefix("text ") {
        return ("Text".to_owned(), key.to_owned());
    }
    if let Some(group) = item.strip_prefix("asset group ") {
        return ("Asset groups".to_owned(), group.to_owned());
    }
    if let Some((erp, uri)) = item.split_once(".erp:") {
        let archive = erp.rsplit('/').next().unwrap_or(erp);
        let uri = uri.trim_start_matches("eaid://");
        return (trf!("{archive}.erp archive", archive = archive), uri.to_owned());
    }
    if let Some((select, field)) = item.rsplit_once("].") {
        let row = select.rsplit_once('=').map_or(select, |(_, l)| l);
        let rows = unbracket(select);
        let module = swofty_core::dbedit::modules().iter().find(|m| {
            let shaped = unbracket(&m.rows);
            shaped == rows || shaped.starts_with(&format!("{rows}."))
        });
        if let Some(m) = module {
            let label = m
                .fields
                .iter()
                .find(|f| f.path == field)
                .map_or_else(|| field.to_owned(), |f| f.label.clone());
            return (m.title.clone(), format!("{} · {label}", title_case(row)));
        }
        let table = rows.split('.').next().unwrap_or(&rows);
        return (
            trf!("{name} table", name = title_case(table)),
            format!("{} · {field}", title_case(row)),
        );
    }
    if item.starts_with("0x") {
        return ("Database".to_owned(), item.to_owned());
    }
    if !item.contains('/') {
        let table = item.split('.').next().unwrap_or(item);
        return (trf!("{name} table", name = title_case(table)), item.to_owned());
    }
    let (dir, file) = item.rsplit_once('/').unwrap_or(("", item));
    (trf!("Files in {dir}", dir = dir), file.to_owned())
}

impl Shell {
    pub(super) fn pairs_of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Pair> + 'a {
        self.addons
            .report
            .iter()
            .flat_map(|r| r.pairs.iter())
            .filter(move |p| p.winner == id || p.loser == id)
    }

    /// The conflicts the list shows a badge for: an addon that is on, from a
    /// report of the current load order.
    pub(super) fn live_pairs<'a>(&'a self, m: &'a AddonItem) -> Vec<&'a Pair> {
        let current = self
            .addons
            .report
            .as_ref()
            .is_some_and(|r| r.is_current(&self.library.profile));
        if !current || !self.addon_on(m) {
            return Vec::new();
        }
        self.pairs_of(&m.manifest.id).collect()
    }

    /// Removals of stock database records the last build of the current load order skipped.
    pub(super) fn stock_removals(&self, m: &AddonItem) -> usize {
        match &self.addons.report {
            Some(r) if r.is_current(&self.library.profile) && self.addon_on(m) => {
                r.stock_removals.get(&m.manifest.id).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    pub(super) fn check_conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.addons.checking || self.vanilla() || self.launching() {
            return;
        }
        self.addons.checking = true;
        let (store, install) = (self.store.clone(), self.install.clone());
        let profile = self.library.profile.name.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let game = crate::sections::game(&store, install.as_ref()).ok_or_else(|| {
                        tr("Locate F1 25 first. Checking conflicts reads its files.").to_owned()
                    })?;
                    conflicts::check(&store, &profile, &game).map_err(|e| e.to_string())
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.addons.checking = false;
                match result {
                    Ok(report) => {
                        let text = match report.pairs.len() {
                            0 => tr("No conflicts between addons").to_owned(),
                            n => trn(n, "Found {count} conflict", "Found {count} conflicts"),
                        };
                        window.notify(Notification::success(text), cx);
                        if report.profile == s.library.profile.name {
                            s.addons.report = Some(report);
                        }
                    }
                    Err(e) => {
                        crate::app_log::action("check conflicts").failed(&e);
                        window.notify(Notification::error(e).title(tr("Could not check conflicts")), cx)
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn check_button(&self, id: &'static str, small: bool, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .outline()
            .when(small, |b| b.tap_xs())
            .icon(IconName::RefreshCw)
            .label(if self.addons.checking {
                tr("Checking…")
            } else {
                tr("Check conflicts")
            })
            .tooltip(tr("Plans this profile's build without installing it"))
            .loading(self.addons.checking)
            .disabled(self.vanilla() || self.addons.checking || self.launching())
            .on_click(cx.listener(|this, _, window, cx| this.check_conflicts(window, cx)))
    }

    pub(super) fn conflicts_block(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let report = self.addons.report.as_ref();
        let kept = self
            .library
            .addons
            .iter()
            .find(|a| a.manifest.id == id)
            .map_or(0, |m| self.stock_removals(m));
        let check = self.check_button("check-conflicts", true, cx);
        let note = match report {
            _ if self.vanilla() => trf!("{vanilla} has no addons to conflict", vanilla = VANILLA),
            None => {
                tr("Not checked yet. Checking plans this profile's build without installing it.").to_owned()
            }
            Some(r) if !r.is_current(&self.library.profile) => trf!(
                "Checked {ago}, before addons were turned on, off or moved. Check again to update it.",
                ago = ago_since(r.built_at)
            ),
            Some(r) if self.changed_since(id, r.built_at) => trf!(
                "Checked {ago}, before this addon last changed. Check again to include it.",
                ago = ago_since(r.built_at)
            ),
            Some(r) => trf!("Checked {ago}", ago = ago_since(r.built_at)),
        };
        let stale = report
            .is_some_and(|r| !r.is_current(&self.library.profile) || self.changed_since(id, r.built_at));
        let name = |a: &str| self.addon_name(a);
        let registry: Vec<String> = self
            .addons
            .registry_conflicts
            .iter()
            .filter(|(about, _)| about.iter().any(|a| a == id))
            .map(|(_, e)| e.line(&name))
            .chain(self.addons.status.get(id).into_iter().flatten().cloned())
            .collect();
        let pairs: Vec<&Pair> = self.pairs_of(id).collect();
        let rows: Vec<AnyElement> = pairs
            .iter()
            .enumerate()
            .map(|(i, pair)| self.pair_row(id, pair, i == 0, p, cx))
            .collect();
        v_flex()
            .gap(px(8.))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(label(tr("CONFLICTS"), p))
                    .child(check),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(note),
            )
            .when(kept > 0, |d| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .items_start()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(14.))
                                .text_color(p.warning),
                        )
                        .child(stock_removals_text(kept)),
                )
            })
            .when(report.is_some() && !stale && pairs.is_empty(), |d| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(
                            Icon::new(IconName::CircleCheck)
                                .size(px(14.))
                                .text_color(p.success),
                        )
                        .child(tr("No conflicts with other addons")),
                )
            })
            .when(!rows.is_empty(), |d| {
                d.child(
                    v_flex()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(p.border_variant)
                        .overflow_hidden()
                        .children(rows),
                )
            })
            .when(!registry.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(tr("In game, as of your last session")),
                )
                .children(registry.into_iter().map(|line| {
                    h_flex()
                        .gap(px(6.))
                        .items_start()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(14.))
                                .text_color(p.warning),
                        )
                        .child(line)
                }))
            })
    }

    pub(super) fn pair_row(
        &self,
        id: &str,
        pair: &Pair,
        first: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wins = pair.winner == id;
        let other = if wins { &pair.loser } else { &pair.winner };
        let key = (pair.winner.clone(), pair.loser.clone());
        let open = self.addons.open_pairs.contains(&key);
        let text = if wins {
            trf!(
                "Overrides {addon} on {counts}",
                addon = self.addon_name(other),
                counts = pair_counts(pair)
            )
        } else {
            trf!(
                "Overridden by {addon} on {counts}",
                addon = self.addon_name(other),
                counts = pair_counts(pair)
            )
        };
        let items = open.then(|| self.pair_items(pair, &key, p, cx));
        v_flex()
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(
                h_flex()
                    .id(SharedString::from(format!("pair-{}-{}", pair.winner, pair.loser)))
                    .px(px(12.))
                    .py(px(8.))
                    .gap(px(10.))
                    .items_start()
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(
                        Icon::new(if wins {
                            IconName::ArrowUp
                        } else {
                            IconName::ArrowDown
                        })
                        .size(px(14.))
                        .mt(px(2.))
                        .text_color(if wins { p.success } else { p.warning }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .child(text),
                    )
                    .child(chevron(open, p))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.addons.open_pairs.remove(&key) {
                            this.addons.open_pairs.insert(key.clone());
                        }
                        cx.notify();
                    })),
            )
            .children(items)
            .into_any_element()
    }
}

impl Shell {
    fn pair_items(&self, pair: &Pair, key: &(String, String), p: Palette, cx: &mut Context<Self>) -> Div {
        let key = key.clone();
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for item in &pair.items {
            let (group, name) = item_group(item);
            match groups.iter_mut().find(|(g, _)| *g == group) {
                Some((_, names)) => names.push(name),
                None => groups.push((group, vec![name])),
            }
        }
        let details = self.addons.details.contains(&key);
        let all = self.addons.all_items.contains(&key);
        let total = pair.items.len();
        let unlisted = pair.database + pair.resources + pair.files + pair.strings;
        let link = |id: String, text: String, cx: &mut Context<Self>, f: fn(&mut Self, (String, String))| {
            let key = key.clone();
            div()
                .id(SharedString::from(id))
                .text_size(px(12.))
                .text_color(p.accent)
                .cursor_pointer()
                .hover(move |s| s.text_color(p.accent_hover))
                .child(text)
                .on_click(cx.listener(move |this, _, _, cx| {
                    f(this, key.clone());
                    cx.notify();
                }))
        };
        let tag_id = format!("{}-{}", pair.winner, pair.loser);
        let raw = details.then(|| {
            let mut left = if all { usize::MAX } else { ITEMS_SHOWN };
            let mut out = Vec::new();
            for (g, names) in &groups {
                if left == 0 {
                    break;
                }
                out.push(
                    div()
                        .pt(px(6.))
                        .text_size(px(11.))
                        .text_color(p.text_placeholder)
                        .child(g.clone()),
                );
                out.extend(names.iter().take(left).map(|n| {
                    div()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(p.text_muted)
                        .child(n.clone())
                }));
                left = left.saturating_sub(names.len());
            }
            v_flex().gap(px(1.)).children(out).when(total > ITEMS_SHOWN, |d| {
                d.child(div().pt(px(4.)).child(link(
                    format!("pair-all-{tag_id}"),
                    if all {
                        tr("Show fewer").to_owned()
                    } else {
                        trf!("Show all {total}", total = total)
                    },
                    cx,
                    |this, key| {
                        if !this.addons.all_items.remove(&key) {
                            this.addons.all_items.insert(key);
                        }
                    },
                )))
            })
        });
        v_flex()
            .px(px(12.))
            .pb(px(10.))
            .pl(px(34.))
            .gap(px(4.))
            .children(groups.iter().map(|(g, names)| {
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .child(div().text_color(p.text).child(g.clone()))
                    .child(
                        div()
                            .text_color(p.text_placeholder)
                            .child(names.len().to_string()),
                    )
            }))
            .when(unlisted > total, |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(trf!("{total} more not listed", total = unlisted - total)),
                )
            })
            .child(link(
                format!("pair-details-{tag_id}"),
                if details {
                    tr("Hide details")
                } else {
                    tr("Details")
                }
                .to_owned(),
                cx,
                |this, key| {
                    if !this.addons.details.remove(&key) {
                        this.addons.details.insert(key);
                    }
                },
            ))
            .children(raw)
    }
}
