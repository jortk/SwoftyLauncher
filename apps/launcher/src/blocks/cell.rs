use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Map, Value};
use swofty_core::view::{Align, Cell, Field, Fit, Pill, Table, Tone, colour_hex, parse_colour};
use swofty_i18n::trf;

use super::state::*;
use super::widgets::*;
use super::*;
use crate::sections::tone;
use crate::shell::Shell;
use crate::theme::Palette;

#[derive(Clone, Copy)]
pub(super) struct CellCtx<'a> {
    pub(super) at: At<'a>,
    pub(super) table: &'a Table,
    pub(super) table_key: &'a str,
    pub(super) row_id: &'a str,
    pub(super) row_tone: Option<Tone>,
    pub(super) size: f32,
    pub(super) row_h: f32,
}

impl Shell {
    pub(super) fn cell(
        &self,
        ctx: CellCtx,
        spec: &swofty_core::view::ColumnSpec,
        column: &str,
        cell: &Cell,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let CellCtx {
            at,
            table: t,
            table_key,
            row_id,
            row_tone,
            size,
            row_h,
        } = ctx;
        let c = cell.spec();
        let key = format!("{table_key}/{row_id}/{column}");
        let color = c.tone.or(row_tone).map_or(p.text, |t| tone(t, p));
        if let Some(edit) = self.blocks.cell.as_ref().filter(|e| e.target.key == key) {
            let input = edit.input.clone();
            return div()
                .id(SharedString::from(format!("edit-{key}")))
                .w_full()
                .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    if ev.keystroke.key == "escape" {
                        this.blocks.cell = None;
                        cx.notify();
                    }
                }))
                .child(Input::new(&input).tap_xs())
                .into_any_element();
        }
        // A switch cell keeps its pending state as "true" or "false", not as its text.
        let toggle = match &c.edit {
            Some(Field::Toggle(tf)) => Some(tf.clone()),
            _ => None,
        };
        let pending = toggle.is_none() && self.blocks.pending_cells.contains_key(&key);
        let shown = match pending {
            true => self.blocks.pending_cells[&key].clone(),
            false => c.text.clone(),
        };
        let text_el = match c.spans.is_empty() || pending {
            true => div().child(shown.clone()),
            // The spans as one text, each with its weight and colour.
            false => {
                let mut whole = String::new();
                let mut marks = Vec::new();
                for span in &c.spans {
                    let start = whole.len();
                    whole.push_str(&span.text);
                    if span.bold || span.tone.is_some() {
                        marks.push((
                            start..whole.len(),
                            HighlightStyle {
                                font_weight: span.bold.then_some(FontWeight::BOLD),
                                color: span.tone.map(|t| tone(t, p)),
                                ..HighlightStyle::default()
                            },
                        ));
                    }
                }
                div().child(StyledText::new(whole).with_highlights(marks))
            }
        }
        .min_w_0()
        .truncate()
        .text_size(px(size))
        .text_color(if pending { p.text_muted } else { color });
        let mut content = h_flex().gap(px(8.)).min_w_0();
        if spec.align == Align::Right {
            content = content.justify_end();
        }
        let memory_image = c.image_data.as_ref().and_then(|data| {
            self.blocks
                .image_data
                .borrow_mut()
                .get(&data.png)
                .map(|image| (data, image))
        });
        if let Some((data, image)) = memory_image {
            let (w, h) = data.size((row_h - 8.).max(16.));
            let (w, h) = (w.clamp(1., 512.), h.clamp(1., (row_h - 4.).max(1.)));
            let fit = match data.fit {
                Fit::Cover => ObjectFit::Cover,
                Fit::Contain => ObjectFit::Contain,
            };
            content = content.child(
                div()
                    .w(px(w))
                    .h(px(h))
                    .flex_shrink_0()
                    .when(!data.transparent, |d| {
                        d.rounded(px(if h > 24. { 6. } else { 2. }))
                    })
                    .overflow_hidden()
                    .when(!data.transparent, |d| d.bg(p.elevated))
                    .child(img(image).w(px(w)).h(px(h)).object_fit(fit)),
            );
        // Existing file-backed pictures retain their existing allowed-root checks.
        } else if let Some(image) = &c.image {
            let (w, h) = image.size((row_h - 8.).max(16.));
            let (w, h) = (w.clamp(1., 512.), h.clamp(1., (row_h - 4.).max(1.)));
            let fit = match image.fit() {
                Fit::Cover => ObjectFit::Cover,
                Fit::Contain => ObjectFit::Contain,
            };
            content = content.child(
                div()
                    .w(px(w))
                    .h(px(h))
                    .flex_shrink_0()
                    .rounded(px(if h > 24. { 6. } else { 2. }))
                    .overflow_hidden()
                    .when(!image.transparent(), |d| d.bg(p.elevated))
                    .when_some(self.image_roots(at.addon).allow(image.path()), |d, path| {
                        d.child(img(path).w(px(w)).h(px(h)).object_fit(fit))
                    }),
            );
        }
        if let Some(tf) = toggle {
            let on = self
                .blocks
                .pending_cells
                .get(&key)
                .map_or(tf.value, |v| v == "true");
            let busy = self.busy.contains(&key);
            let (addon, section, table, row, column, control) = (
                at.addon.to_owned(),
                at.section.to_owned(),
                t.id().to_owned(),
                row_id.to_owned(),
                column.to_owned(),
                key.clone(),
            );
            let switch = Switch::new(SharedString::from(format!("cell-{key}")))
                .checked(on)
                .tap_sm()
                .disabled(busy || tf.disabled)
                .on_change(cx.listener(move |this, on: &bool, window, cx| {
                    this.blocks.pending_cells.insert(control.clone(), on.to_string());
                    let mut values = Map::new();
                    values.insert(column.clone(), Value::Bool(*on));
                    let event = swofty_core::view::Event::Submit {
                        form: table.clone(),
                        row: Some(row.clone()),
                        values,
                        changed: vec![column.clone()],
                    };
                    this.send(&addon, &section, control.clone(), event, window, cx);
                }));
            // The switch's click stays in the cell, so it doesn't also open the row.
            content = content.child(
                div()
                    .id(SharedString::from(format!("cell-hold-{key}")))
                    .flex_shrink_0()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(switch),
            );
            if !shown.is_empty() {
                content = content.child(text_el);
            }
            return content.into_any_element();
        }
        if let Some(Field::Colour(cf)) = &c.edit {
            let target = CellTarget {
                key: key.clone(),
                addon: at.addon.to_owned(),
                section: at.section.to_owned(),
                table: t.id().to_owned(),
                row: row_id.to_owned(),
                column: column.to_owned(),
            };
            let original = match cf.rgb().or_else(|| parse_colour(&c.text)) {
                Some(rgb) => Value::String(colour_hex(rgb)),
                None => Value::Null,
            };
            let rgb = self
                .colour_shown(&key, cx)
                .or_else(|| parse_colour(&shown).filter(|_| pending))
                .or_else(|| original.as_str().and_then(parse_colour));
            let (busy, disabled) = (self.busy.contains(&key), cf.disabled);
            let start = rgb.unwrap_or([0xFF, 0xFF, 0xFF]);
            let open_key = key.clone();
            return content
                .id(SharedString::from(format!("cell-{key}")))
                .relative()
                .px(px(6.))
                .mx(px(-6.))
                .h(px(size + 12.))
                .rounded(px(4.))
                .child(swatch(rgb, 14., p))
                .when(cf.hex, |d| d.child(text_el))
                .when(!disabled, |d| d.cursor_pointer())
                .when(!busy && !disabled, |d| d.hover(move |s| s.bg(p.hover)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    if disabled || this.busy.contains(&open_key) {
                        return;
                    }
                    let target = ColourTarget::Cell {
                        target: target.clone(),
                        original: original.clone(),
                    };
                    this.open_colour(open_key.clone(), target, start, window, cx)
                }))
                .children(self.colour_overlay(&key, cx))
                .into_any_element();
        }
        if let Some(f) = c.bar {
            content = content.child(div().w(px(56.)).flex_shrink_0().child(bar(
                f,
                fill_tone(c.tone, p),
                4.,
                p,
            )));
        }
        content = if c.pill {
            content.child(pill(
                &Pill {
                    text: shown,
                    tone: c.tone.unwrap_or_default(),
                    ..Pill::default()
                },
                p,
            ))
        } else {
            content.child(text_el)
        };
        let Some(field) = c.edit.clone() else {
            return content.into_any_element();
        };
        let (addon, section, table, row, column) = (
            at.addon.to_owned(),
            at.section.to_owned(),
            t.id().to_owned(),
            row_id.to_owned(),
            column.to_owned(),
        );
        let original = match field.value() {
            Value::Null => match &field {
                Field::Number(nf) => number_text(&c.text).map_or(Value::Null, |v| nf.json(v)),
                _ => Value::String(c.text.clone()),
            },
            v => v,
        };
        let busy = self.busy.contains(&key);
        content
            .id(SharedString::from(format!("cell-{key}")))
            .px(px(6.))
            .mx(px(-6.))
            .h(px(size + 12.))
            .rounded(px(4.))
            .cursor_pointer()
            .when(!busy, |d| d.hover(move |s| s.bg(p.hover)))
            .on_click(cx.listener(move |this, _, window, cx| {
                // Editing the cell doesn't also open the row.
                cx.stop_propagation();
                if this.busy.contains(&key) {
                    return;
                }
                this.edit_cell(
                    CellTarget {
                        key: key.clone(),
                        addon: addon.clone(),
                        section: section.clone(),
                        table: table.clone(),
                        row: row.clone(),
                        column: column.clone(),
                    },
                    field.clone(),
                    original.clone(),
                    window,
                    cx,
                )
            }))
            .into_any_element()
    }

    pub(super) fn edit_cell(
        &mut self,
        target: CellTarget,
        field: Field,
        original: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = field_text(&field, &original);
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).default_value(text);
            if let Field::Number(nf) = &field {
                s = s.step(nf.step());
            }
            if let Field::Text(tf) = &field
                && let Some(ph) = tf.placeholder.clone()
            {
                s = s.placeholder(ph);
            }
            s
        });
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let key = target.key.clone();
        let sub = cx.subscribe_in(&input, window, move |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                && this.blocks.cell.as_ref().is_some_and(|c| c.target.key == key)
            {
                this.commit_cell(window, cx);
            }
        });
        self.blocks.cell = Some(CellEdit {
            target,
            field,
            original,
            input,
            _subs: vec![sub],
        });
        cx.notify();
    }

    pub(super) fn commit_cell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.blocks.cell.take() else {
            return;
        };
        let text = edit.input.read(cx).value().to_string();
        let value = match &edit.field {
            Field::Number(nf) => match number_text(&text) {
                Some(v) => nf.json(nf.clamp(v)),
                None => {
                    window.notify(
                        Notification::error(trf!("{text} is not a number", text = text)),
                        cx,
                    );
                    cx.notify();
                    return;
                }
            },
            Field::Text(tf) => Value::String(match tf.max_len {
                Some(max) => text.chars().take(max).collect(),
                None => text,
            }),
            _ => Value::Null,
        };
        cx.notify();
        if value == edit.original || value.is_null() {
            return;
        }
        let text = field_text(&edit.field, &value);
        self.submit_cell(edit.target, value, text, window, cx);
    }

    pub(super) fn submit_cell(
        &mut self,
        target: CellTarget,
        value: Value,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.blocks.pending_cells.insert(target.key.clone(), text);
        let mut values = Map::new();
        values.insert(target.column.clone(), value);
        let event = swofty_core::view::Event::Submit {
            form: target.table,
            row: Some(target.row),
            values,
            changed: vec![target.column],
        };
        self.send(&target.addon, &target.section, target.key, event, window, cx);
    }
}
