use super::model::{AddonListItem, PINNED};
use crate::library::{AddonItem, Source};
use crate::notices::{Notification, Notify};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, palette};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashSet;
use swofty_core::folders::{self, Entry, Key, Spot};
use swofty_i18n::trn;

#[derive(Clone, PartialEq)]
pub(super) enum Dragging {
    Addon(String),
    Addons(Vec<String>),
    Folder(Key),
}

impl Dragging {
    fn carries(&self, id: &str) -> bool {
        matches!(self, Self::Addons(ids) if ids.iter().any(|i| i == id))
    }
}

#[derive(Clone)]
pub(super) struct Dragged {
    pub(super) item: Dragging,
    pub(super) name: SharedString,
}

impl Render for Dragged {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let (icon, count) = match &self.item {
            Dragging::Addon(_) => (IconName::GripVertical, 1),
            Dragging::Addons(ids) => (IconName::GripVertical, ids.len()),
            Dragging::Folder(_) => (IconName::Folder, 1),
        };
        let card = || {
            h_flex()
                .px(px(10.))
                .py(px(6.))
                .gap(px(6.))
                .rounded(px(6.))
                .bg(p.elevated)
                .border_1()
                .border_color(p.accent)
                .text_size(px(13.))
                .text_color(p.text)
        };
        let front = card()
            .child(Icon::new(icon).size(px(13.)).text_color(p.text_muted))
            .child(self.name.clone())
            .when(count > 1, |d| {
                d.child(
                    div()
                        .px(px(6.))
                        .rounded(px(4.))
                        .bg(p.accent)
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.on_accent)
                        .child(trn(count, "{count} addon", "{count} addons")),
                )
            });
        if count < 2 {
            return front.into_any_element();
        }
        div()
            .relative()
            .flex()
            .child(
                card()
                    .absolute()
                    .top(px(4.))
                    .left(px(4.))
                    .size_full()
                    .border_color(p.accent.opacity(0.5)),
            )
            .child(front)
            .into_any_element()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Edge {
    Above,
    Below,
    Into,
}

/// Where a dragged addon or folder lands, and the list item that marks it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DropAt {
    pub(super) spot: Spot,
    pub(super) ix: usize,
    pub(super) edge: Edge,
    pub(super) nested: bool,
}

pub(super) const NESTED: f32 = 22.;

/// The line, or for a folder the outline, that shows where a drop lands.
pub(super) fn drop_mark(edge: Edge, nested: bool, p: Palette) -> Div {
    let line = || {
        div()
            .absolute()
            .left(px(if nested { 6. + NESTED } else { 6. }))
            .right(px(8.))
            .h(px(2.))
            .rounded(px(1.))
            .bg(p.accent)
    };
    match edge {
        Edge::Above => line().top_0(),
        Edge::Below => line().bottom_0(),
        Edge::Into => div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .right_0()
            .border_1()
            .border_color(p.accent)
            .bg(p.accent.opacity(0.08)),
    }
}

impl Shell {
    /// Addons that can be dragged and moved with the arrows: anything outside Built In.
    pub(super) fn movable(&self, m: &AddonItem) -> bool {
        !self.vanilla()
            && !m.locked
            && m.source != Source::Builtin
            && !m.manifest.is_builtin()
            && !PINNED.contains(&m.manifest.id.as_str())
    }

    /// What dragging the row `id` moves: every ticked addon that can move when
    /// the row is one of two or more of them, else the row alone.
    pub(super) fn drag_item(&self, id: &str) -> Dragging {
        let single = || Dragging::Addon(id.to_owned());
        if self.addons.selection.len() < 2 || !self.addons.selection.contains(id) {
            return single();
        }
        let ids: Vec<String> = self
            .addons
            .selection
            .ids()
            .iter()
            .filter(|s| {
                self.library
                    .addons
                    .iter()
                    .any(|a| &a.manifest.id == *s && self.movable(a))
            })
            .cloned()
            .collect();
        match ids.len() {
            0 | 1 => single(),
            _ => Dragging::Addons(ids),
        }
    }

    pub(super) fn move_step(&mut self, id: &str, up: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spot) = folders::step(&self.library.entries, id, up) else {
            return;
        };
        let mut entries = self.library.entries.clone();
        if folders::move_addon(&mut entries, id, &spot).is_some() {
            self.save_order(entries, Vec::new(), window, cx);
        }
    }

    /// Where something dragged over list item `ix`, at `frac` of its height from the top, lands.
    pub(super) fn drop_for(&self, ix: usize, frac: f32, dragging: &Dragging) -> Option<DropAt> {
        let items = &self.addons.list_items;
        let folder_drag = matches!(dragging, Dragging::Folder(_));
        let mark = |spot: Spot, ix: usize, edge: Edge, nested: bool| DropAt {
            spot,
            ix,
            edge,
            nested,
        };
        let after = |key: &Key, at: usize, row: usize| {
            let last = items[row + 1..]
                .iter()
                .take_while(|i| match i {
                    AddonListItem::Addon {
                        nested: Some((k, _)), ..
                    } => k == key,
                    AddonListItem::Detail(_) => true,
                    _ => false,
                })
                .count();
            mark(Spot::Top(at + 1), row + last, Edge::Below, false)
        };
        match items.get(ix)? {
            AddonListItem::End => Some(mark(
                Spot::Top(self.library.entries.len()),
                ix,
                Edge::Above,
                false,
            )),
            AddonListItem::Folder {
                key: Key::BuiltIn, ..
            } => None,
            AddonListItem::Folder { key, .. } if *dragging == Dragging::Folder(key.clone()) => None,
            AddonListItem::Addon { id, .. } if dragging.carries(id) => None,
            AddonListItem::Folder { key, at, open, .. } => Some(match folder_drag {
                true if frac < 0.5 => mark(Spot::Top(*at), ix, Edge::Above, false),
                true => after(key, *at, ix),
                false if frac < 0.3 => mark(Spot::Top(*at), ix, Edge::Above, false),
                false if !open && frac > 0.7 => mark(Spot::Top(at + 1), ix, Edge::Below, false),
                false => {
                    let len = self.library.entries.get(*at).map_or(0, |e| match e {
                        Entry::Folder(_, members) => members.len(),
                        Entry::Addon(_) => 0,
                    });
                    mark(Spot::In(key.clone(), len), ix, Edge::Into, false)
                }
            }),
            AddonListItem::Addon { at, nested: None, .. } => Some(match frac < 0.5 {
                true => mark(Spot::Top(*at), ix, Edge::Above, false),
                false => mark(Spot::Top(at + 1), ix, Edge::Below, false),
            }),
            AddonListItem::Addon {
                nested: Some((Key::BuiltIn, _)),
                ..
            } => None,
            AddonListItem::Addon {
                at,
                nested: Some((key, j)),
                ..
            } => {
                if !folder_drag {
                    return Some(match frac < 0.5 {
                        true => mark(Spot::In(key.clone(), *j), ix, Edge::Above, true),
                        false => mark(Spot::In(key.clone(), j + 1), ix, Edge::Below, true),
                    });
                }
                if *dragging == Dragging::Folder(key.clone()) {
                    return None;
                }
                let row = items[..ix]
                    .iter()
                    .rposition(|i| matches!(i, AddonListItem::Folder { key: k, .. } if k == key))?;
                let members: Vec<usize> = (row + 1..items.len())
                    .take_while(|&i| match &items[i] {
                        AddonListItem::Addon {
                            nested: Some((k, _)), ..
                        } => k == key,
                        AddonListItem::Detail(_) => true,
                        _ => false,
                    })
                    .collect();
                let place = members.iter().position(|&i| i == ix).unwrap_or(0) as f32 + frac;
                Some(match place < members.len() as f32 / 2. {
                    true => mark(Spot::Top(*at), row, Edge::Above, false),
                    false => after(key, *at, row),
                })
            }
            _ => None,
        }
    }

    pub(super) fn apply_drop(&mut self, d: &Dragged, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drop) = self.addons.drop.take() else {
            return;
        };
        let mut entries = self.library.entries.clone();
        match &d.item {
            Dragging::Addon(id) => {
                if let Some(place) = folders::move_addon(&mut entries, id, &drop.spot) {
                    self.save_order(entries, vec![(id.clone(), place)], window, cx);
                }
            }
            Dragging::Addons(ids) => {
                let placed = folders::move_addons(&mut entries, ids, &drop.spot);
                if !placed.is_empty() {
                    self.save_order(entries, placed, window, cx);
                }
            }
            Dragging::Folder(key) => {
                if let Spot::Top(to) = drop.spot
                    && folders::move_folder(&mut entries, key, to)
                {
                    self.save_order(entries, Vec::new(), window, cx);
                }
            }
        }
        cx.notify();
    }

    /// Saves a new list: `placed` holds the moved addons and the folder each is in now.
    pub(super) fn save_order(
        &mut self,
        entries: Vec<Entry>,
        placed: Vec<(String, Option<Key>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut moved_folder = false;
        if !placed.is_empty() {
            let mut f = self.library.folders.clone();
            for (id, place) in placed {
                let default = self
                    .library
                    .addons
                    .iter()
                    .find(|a| a.manifest.id == id)
                    .and_then(|a| folders::default_place(a.source == Source::Builtin, a.category()));
                f.set_place(&id, place.as_ref(), default.as_ref());
            }
            if f != self.library.folders {
                if let Err(e) = self.store.save_folders(&f) {
                    crate::app_log::action("save folders").failed(&e);
                    window.notify(Notification::error(e.to_string()), cx);
                    return;
                }
                moved_folder = true;
            }
        }
        let order = folders::flatten(&entries);
        if order == folders::flatten(&self.library.entries) {
            if moved_folder {
                self.reload(cx);
            }
            return;
        }
        let installed: HashSet<String> = self
            .library
            .addons
            .iter()
            .map(|a| a.manifest.id.clone())
            .collect();
        self.update_profile(window, cx, move |p| {
            folders::reorder(p, &order, &|id| installed.contains(id));
            Ok(())
        });
    }
}
