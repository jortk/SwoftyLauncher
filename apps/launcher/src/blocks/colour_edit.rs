use std::rc::Rc;

use crate::scale::px;
use gpui_kit::*;
use serde_json::Value;
use swofty_core::view::colour_hex;

use super::state::*;
use crate::colour::{ColourEvent, ColourPicker};
use crate::shell::Shell;

impl Shell {
    pub(super) fn colour_shown(&self, key: &str, cx: &App) -> Option<[u8; 3]> {
        let edit = self.blocks.colour.as_ref().filter(|c| c.key == key)?;
        Some(edit.picker.read(cx).rgb())
    }

    pub(super) fn colour_overlay(&self, key: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(edit) = self.blocks.colour.as_ref().filter(|c| c.key == key) else {
            return Vec::new();
        };
        let trigger = edit.trigger.clone();
        let picker = edit.picker.clone();
        vec![
            canvas(move |b, _, _| trigger.set(b), |_, _, _, _| {})
                .absolute()
                .size_full()
                .into_any_element(),
            div()
                .absolute()
                .top_full()
                .left_0()
                .child(
                    deferred(
                        anchored().snap_to_window_with_margin(px(8.)).child(
                            div()
                                .id("colour-overlay")
                                .mt(px(6.))
                                .occlude()
                                .on_mouse_down_out(cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                                    let on_trigger = this
                                        .blocks
                                        .colour
                                        .as_ref()
                                        .is_some_and(|c| c.trigger.get().contains(&ev.position));
                                    if !on_trigger {
                                        this.close_colour(true, window, cx);
                                    }
                                }))
                                .child(picker),
                        ),
                    )
                    .with_priority(1),
                )
                .into_any_element(),
        ]
    }

    pub(super) fn open_colour(
        &mut self,
        key: String,
        target: ColourTarget,
        start: [u8; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.blocks.colour.as_ref().is_some_and(|c| c.key == key) {
            self.close_colour(true, window, cx);
            return;
        }
        self.close_colour(true, window, cx);
        let picker = cx.new(|cx| ColourPicker::new(start, window, cx));
        let subs = vec![
            cx.subscribe_in(&picker, window, |this, _, ev: &ColourEvent, window, cx| {
                this.close_colour(matches!(ev, ColourEvent::Commit), window, cx)
            }),
            cx.observe(&picker, |_, _, cx| cx.notify()),
        ];
        self.blocks.colour = Some(ColourEdit {
            key,
            target,
            picker,
            trigger: Rc::default(),
            _subs: subs,
        });
        cx.notify();
    }

    pub(super) fn close_colour(&mut self, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.blocks.colour.take() else {
            return;
        };
        cx.notify();
        if !commit {
            return;
        }
        let hex = colour_hex(edit.picker.read(cx).rgb());
        match edit.target {
            ColourTarget::Form {
                key,
                field,
                addon,
                section,
                form,
            } => {
                let live = self
                    .find_form(&addon, &section, &form)
                    .filter(|f| f.live)
                    .map(|f| (addon, section, f));
                self.pick(&key, &field, Value::String(hex), live, window, cx);
            }
            ColourTarget::Cell { target, original } => {
                let value = Value::String(hex.clone());
                if value != original {
                    self.submit_cell(target, value, hex, window, cx);
                }
            }
        }
    }
}
