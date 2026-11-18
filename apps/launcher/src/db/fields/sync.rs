use std::collections::HashMap;

use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, NumberInputEvent};
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::*;
use swofty_core::dbedit::{Backing, Kind};
use swofty_i18n::tr;

use super::value::{color_text, list_items, parse_curve, sep_of, shown_y, swatch_color};
use super::{Choice, Choices, ColorSlot, CopySlot, InputSlot, SelectSlot, SelectSpec};
use crate::shell::Shell;

impl Shell {
    pub(crate) fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for s in self.db.slots.inputs.values_mut() {
            s.seen = false;
        }
        for s in self.db.slots.selects.values_mut() {
            s.seen = false;
        }
        for s in self.db.slots.colors.values_mut() {
            s.seen = false;
        }
        for s in self.db.slots.copies.values_mut() {
            s.seen = false;
        }
        let fields = self.db_detail_fields();
        let mut missing: Vec<usize> = fields
            .iter()
            .filter_map(|f| match &f.row {
                Some((spec, Backing::Missing)) => Some(*spec),
                _ => None,
            })
            .collect();
        missing.dedup();
        for spec in missing {
            self.sync_copy(spec, window, cx);
        }
        for f in fields {
            let Some(key) = f.key else {
                continue;
            };
            match f.kind {
                Kind::Int | Kind::Float => {
                    let (min, max, step) = (f.min, f.max, f.step);
                    self.sync_input(&f.id, key, &f.value, window, cx, move |s| {
                        let mut s = s.step(step.unwrap_or(1.));
                        if let Some(m) = min {
                            s = s.min(m);
                        }
                        if let Some(m) = max {
                            s = s.max(m);
                        }
                        s
                    });
                }
                Kind::Text | Kind::Time => {
                    let ph = if f.kind == Kind::Time { "HH,MM" } else { "" };
                    self.sync_input(&f.id, key, &f.value, window, cx, move |s| s.placeholder(ph));
                }
                Kind::List | Kind::Vector => {
                    for (i, v) in list_items(&f.value).iter().enumerate() {
                        self.sync_input(&format!("{}/i{i}", f.id), key, v, window, cx, |s| s);
                    }
                }
                Kind::Color => {
                    self.sync_input(&f.id, key, &f.value, window, cx, |s| s);
                    self.sync_color(&f.id, key, &f.value, window, cx);
                }
                Kind::Choice | Kind::Ref => {
                    self.sync_select(
                        SelectSpec {
                            id: &f.id,
                            key,
                            value: &f.value,
                            whole: &f.value,
                            options: &f.options,
                        },
                        window,
                        cx,
                    );
                }
                Kind::RefList => {
                    let have = list_items(&f.value);
                    let rest: Vec<(String, String)> = f
                        .options
                        .iter()
                        .filter(|(v, _)| !have.contains(v))
                        .cloned()
                        .collect();
                    self.sync_select(
                        SelectSpec {
                            id: &format!("{}/add", f.id),
                            key,
                            value: "",
                            whole: &f.value,
                            options: &rest,
                        },
                        window,
                        cx,
                    );
                }
                Kind::Curve => match parse_curve(&f.value) {
                    Some(c) => {
                        for (i, (x, y)) in c.points.iter().enumerate() {
                            let y = shown_y(&c, y);
                            self.sync_input(&format!("{}/x{i}", f.id), key, x, window, cx, |s| s);
                            self.sync_input(&format!("{}/y{i}", f.id), key, &y, window, cx, |s| s);
                        }
                    }
                    None => self.sync_input(&f.id, key, &f.value, window, cx, |s| s),
                },
                Kind::Bool => {}
            }
        }
        self.db.slots.inputs.retain(|_, s| s.seen);
        self.db.slots.selects.retain(|_, s| s.seen);
        self.db.slots.colors.retain(|_, s| s.seen);
        self.db.slots.copies.retain(|_, s| s.seen);
    }

    pub(super) fn copy_id(&self, spec: usize) -> Option<String> {
        let slot = self.db.view.as_ref()?;
        let row = self.db.row.as_ref()?;
        Some(format!(
            "{}/{}/{row}/{spec}",
            slot.module,
            slot.scope.as_deref().unwrap_or_default()
        ))
    }

    /// The "Copy from…" picker of a row spec: every entity with a row of its own, and the game default.
    pub(super) fn sync_copy(&mut self, spec: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.copy_id(spec) else {
            return;
        };
        if let Some(s) = self.db.slots.copies.get_mut(&id) {
            s.seen = true;
            return;
        }
        let (Some(catalog), Some(slot)) = (self.db.catalog().cloned(), self.db.view.as_ref()) else {
            return;
        };
        let Some(module) = self.db.module(&slot.module) else {
            return;
        };
        let (labels, default) = module.row_sources(&catalog, spec, slot.scope.as_deref());
        let names: HashMap<&str, &str> = slot
            .view
            .rows
            .iter()
            .filter(|r| !r.name.is_empty())
            .map(|r| (r.label.as_str(), r.name.as_str()))
            .collect();
        let mut items: Vec<Choice> = labels
            .into_iter()
            .map(|l| Choice {
                title: SharedString::from(names.get(l.as_str()).map_or_else(|| l.clone(), |n| n.to_string())),
                value: l,
            })
            .collect();
        items.sort_by_key(|c| c.title.to_lowercase());
        if default {
            items.insert(
                0,
                Choice {
                    value: String::new(),
                    title: tr("Game default").into(),
                },
            );
        }
        let state =
            cx.new(|cx| SelectState::new(SearchableVec::new(items), None, window, cx).searchable(true));
        let sub = cx.subscribe_in(
            &state,
            window,
            move |this, _, ev: &SelectEvent<Choices>, _, cx| {
                if let SelectEvent::Confirm(Some(v)) = ev {
                    let from = (!v.is_empty()).then(|| v.clone());
                    this.db_copy_row(spec, from, cx);
                }
            },
        );
        self.db.slots.copies.insert(
            id,
            CopySlot {
                state,
                seen: true,
                _sub: sub,
            },
        );
    }

    pub(super) fn sync_input(
        &mut self,
        id: &str,
        key: u64,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        build: impl FnOnce(InputState) -> InputState,
    ) {
        if let Some(slot) = self.db.slots.inputs.get_mut(id) {
            slot.seen = true;
            let focused = slot.input.read(cx).focus_handle(cx).is_focused(window);
            if slot.synced != value && !focused {
                slot.synced = value.to_owned();
                let text = value.to_owned();
                slot.input.update(cx, |s, cx| s.set_value(text, window, cx));
            }
            return;
        }
        let text = value.to_owned();
        let input = cx.new(|cx| build(InputState::new(window, cx).default_value(text)));
        let slot_id = id.to_owned();
        let commit = slot_id.clone();
        let sub = cx.subscribe_in(&input, window, move |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.db_commit_input(&commit, cx);
            }
        });
        let step = cx.subscribe_in(&input, window, move |this, _, _: &NumberInputEvent, _, cx| {
            this.db_commit_input(&slot_id, cx);
        });
        self.db.slots.inputs.insert(
            id.to_owned(),
            InputSlot {
                key,
                input,
                synced: value.to_owned(),
                seen: true,
                _subs: vec![sub, step],
            },
        );
    }

    pub(super) fn sync_select(&mut self, spec: SelectSpec, window: &mut Window, cx: &mut Context<Self>) {
        let SelectSpec {
            id,
            key,
            value,
            whole,
            options,
        } = spec;
        let synced = format!("{whole}\u{0}{}", options.len());
        if let Some(slot) = self.db.slots.selects.get_mut(id) {
            slot.seen = true;
            if slot.synced == synced {
                return;
            }
        }
        let items: Vec<Choice> = options
            .iter()
            .map(|(v, name)| Choice {
                value: v.clone(),
                title: SharedString::from(if name.is_empty() { v.clone() } else { name.clone() }),
            })
            .collect();
        let at = items
            .iter()
            .position(|c| c.value == value)
            .map(|r| gpui_kit::component::IndexPath::default().row(r));
        let state = cx.new(|cx| SelectState::new(SearchableVec::new(items), at, window, cx).searchable(true));
        let adds = id.ends_with("/add");
        let slot_id = id.to_owned();
        let sub = cx.subscribe_in(
            &state,
            window,
            move |this, _, ev: &SelectEvent<Choices>, _, cx| {
                let SelectEvent::Confirm(Some(v)) = ev else {
                    return;
                };
                let Some(key) = this.db_slot_key(&slot_id) else {
                    return;
                };
                if adds {
                    let current = this.db.current(key);
                    let mut items = list_items(&current);
                    items.push(v.clone());
                    this.db_set(key, &items.join(sep_of(&current)), cx);
                } else {
                    this.db_set(key, v, cx);
                }
            },
        );
        self.db.slots.selects.insert(
            id.to_owned(),
            SelectSlot {
                key,
                state,
                synced,
                seen: true,
                _sub: sub,
            },
        );
    }

    pub(super) fn sync_color(
        &mut self,
        id: &str,
        key: u64,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.db.slots.colors.get_mut(id) {
            slot.seen = true;
            if slot.synced != value {
                slot.synced = value.to_owned();
                if let Some(c) = swatch_color(value) {
                    slot.state.update(cx, |s, cx| s.set_value(c, window, cx));
                }
            }
            return;
        }
        let color = swatch_color(value);
        let state = cx.new(|cx| {
            let s = ColorPickerState::new(window, cx);
            match color {
                Some(c) => s.default_value(c),
                None => s,
            }
        });
        let slot_id = id.to_owned();
        let sub = cx.subscribe_in(&state, window, move |this, _, ev: &ColorPickerEvent, _, cx| {
            let ColorPickerEvent::Change(Some(c)) = ev else {
                return;
            };
            if let Some(key) = this.db_slot_key(&slot_id) {
                this.db_set(key, &color_text(*c), cx);
            }
        });
        self.db.slots.colors.insert(
            id.to_owned(),
            ColorSlot {
                key,
                state,
                synced: value.to_owned(),
                seen: true,
                _sub: sub,
            },
        );
    }
}
