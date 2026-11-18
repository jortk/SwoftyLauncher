use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::launcher::{TaskItem, TaskList, TaskState, Tasks};
use swofty_core::view::{Effect, Shown};
use swofty_i18n::trf;

use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::{Overlay, ProgressBar};

pub(crate) type Feed = Arc<Mutex<Option<TaskList>>>;

pub(crate) struct Sink(pub(crate) Feed);

impl Tasks for Sink {
    fn show(&self, list: &TaskList) {
        if let Ok(mut feed) = self.0.lock() {
            *feed = Some(list.clone());
        }
    }
}

pub(crate) struct TaskDialog {
    control: String,
    list: TaskList,
    outcome: Option<Outcome>,
    scroll: ScrollHandle,
    running: Option<usize>,
}

enum Outcome {
    Quiet,
    Notice(String),
    Error(String),
}

#[derive(Default)]
struct Counts {
    done: usize,
    failed: usize,
    skipped: usize,
    total: usize,
}

impl Counts {
    fn of(items: &[TaskItem]) -> Self {
        let mut c = Self {
            total: items.len(),
            ..Self::default()
        };
        for item in items {
            match item.state {
                TaskState::Done => c.done += 1,
                TaskState::Failed => c.failed += 1,
                TaskState::Skipped => c.skipped += 1,
                TaskState::Waiting | TaskState::Running => {}
            }
        }
        c
    }

    fn settled(&self) -> usize {
        self.done + self.failed + self.skipped
    }

    fn line(&self) -> String {
        let mut parts = vec![trf!(
            "{done} of {total} done",
            done = self.done,
            total = self.total
        )];
        if self.failed > 0 {
            parts.push(trf!("{count} failed", count = self.failed));
        }
        if self.skipped > 0 {
            parts.push(trf!("{count} skipped", count = self.skipped));
        }
        parts.join(" · ")
    }
}

impl Shell {
    pub(crate) fn follow_tasks(&mut self, control: String, feed: Feed, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let list = feed.lock().ok().and_then(|mut f| f.take());
                let running = this.update(cx, |s, cx| {
                    if let Some(list) = list {
                        s.show_tasks(&control, list, cx);
                    }
                    s.busy.contains(&control)
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn show_tasks(&mut self, control: &str, list: TaskList, cx: &mut Context<Self>) {
        let d = match &mut self.tasks {
            Some(d) if d.control == control && d.outcome.is_none() => d,
            Some(d) if d.outcome.is_none() => return,
            slot => slot.insert(TaskDialog {
                control: control.to_owned(),
                list: TaskList::default(),
                outcome: None,
                scroll: ScrollHandle::new(),
                running: None,
            }),
        };
        let running = list.items.iter().position(|i| i.state == TaskState::Running);
        if let Some(ix) = running.filter(|_| running != d.running) {
            d.scroll.scroll_to_item(ix);
        }
        d.running = running;
        d.list = list;
        cx.notify();
    }

    // Moves the effect's notice or error into the event's open task dialog.
    pub(crate) fn finish_tasks(
        &mut self,
        control: &str,
        feed: &Feed,
        effect: Result<Effect, String>,
        cx: &mut Context<Self>,
    ) -> Result<Effect, String> {
        if let Some(list) = feed.lock().ok().and_then(|mut f| f.take()) {
            self.show_tasks(control, list, cx);
        }
        let Some(dialog) = self
            .tasks
            .as_mut()
            .filter(|d| d.control == control && d.outcome.is_none())
        else {
            return effect;
        };
        let (outcome, effect) = match effect {
            Ok(Effect::Notice(text)) => (Outcome::Notice(text), Ok(Effect::Reload)),
            Ok(Effect::ShowAddon(shown)) if shown.notice().is_some() => (
                Outcome::Notice(shown.notice().unwrap_or_default().to_owned()),
                Ok(Effect::ShowAddon(Shown::Id(shown.id().to_owned()))),
            ),
            Err(e) => (Outcome::Error(e), Ok(Effect::None)),
            other => (Outcome::Quiet, other),
        };
        dialog.outcome = Some(outcome);
        if let Some(ix) = dialog
            .list
            .items
            .iter()
            .position(|i| i.state == TaskState::Failed)
        {
            dialog.scroll.scroll_to_item(ix);
        }
        cx.notify();
        effect
    }

    pub(crate) fn task_dialog(
        &self,
        p: Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let d = self.tasks.as_ref()?;
        let viewport = window.viewport_size();
        let width = (f32::from(viewport.width) - 32.).min(560.);
        let list_max = (f32::from(viewport.height) - 380.).clamp(132., 480.);
        let counts = Counts::of(&d.list.items);
        let running = d.outcome.is_none();
        let failed = counts.failed > 0 || matches!(d.outcome, Some(Outcome::Error(_)));
        let (mark, tint): (AnyElement, Hsla) = match &d.outcome {
            None => (
                Spinner::new()
                    .icon(IconName::LoaderCircle)
                    .with_size(px(20.))
                    .color(p.accent_hover)
                    .into_any_element(),
                p.accent,
            ),
            Some(Outcome::Error(_)) => (glyph(IconName::CircleAlert, 20., p.error), p.error),
            Some(_) if failed => (glyph(IconName::TriangleAlert, 20., p.warning), p.warning),
            Some(_) => (glyph(IconName::CircleCheck, 20., p.success), p.success),
        };
        let header = h_flex()
            .gap(px(14.))
            .items_start()
            .child(
                div()
                    .size(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.))
                    .bg(tint.opacity(0.14))
                    .child(mark),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .pt(px(1.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(d.list.title.clone()),
                    )
                    .when(!d.list.subtitle.is_empty(), |c| {
                        c.child(
                            div()
                                .text_size(px(13.))
                                .line_height(px(18.))
                                .text_color(p.text_muted)
                                .child(d.list.subtitle.clone()),
                        )
                    }),
            );
        let last = d.list.items.len().saturating_sub(1);
        let items = div()
            .relative()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.chrome)
            .overflow_hidden()
            .child(
                v_flex()
                    .id("task-items")
                    .max_h(px(list_max))
                    .track_scroll(&d.scroll)
                    .overflow_y_scroll()
                    .children(
                        d.list
                            .items
                            .iter()
                            .enumerate()
                            .map(|(i, item)| task_row(item, i == last, p)),
                    ),
            )
            .child(Scrollbar::new(&d.scroll));
        let fraction = match counts.total {
            0 if running => 0.,
            0 => 1.,
            n => counts.settled() as f32 / n as f32,
        };
        let bar = ProgressBar::new(fraction).color(if running { p.accent } else { tint });
        let note = match &d.outcome {
            Some(Outcome::Notice(text)) => Some((text.clone(), if failed { p.warning } else { p.success })),
            Some(Outcome::Error(e)) => Some((e.clone(), p.error)),
            _ => None,
        };
        let note = note.map(|(text, color)| {
            h_flex()
                .gap(px(10.))
                .items_start()
                .px(px(12.))
                .py(px(10.))
                .rounded(px(8.))
                .bg(color.opacity(0.08))
                .border_1()
                .border_color(color.opacity(0.32))
                .child(div().pt(px(1.)).child(glyph(
                    if failed {
                        IconName::Info
                    } else {
                        IconName::CircleCheck
                    },
                    15.,
                    color,
                )))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(p.text)
                        .child(text),
                )
        });
        let status = h_flex()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .text_size(px(13.))
            .text_color(p.text_muted)
            .when(running, |s| s.child("Working…"))
            .when(running && counts.total > 0, |s| {
                s.child(div().text_color(p.text_placeholder).child("·"))
            })
            .when(counts.total > 0, |s| {
                s.child(div().truncate().child(counts.line()))
            });
        let footer = h_flex()
            .gap(px(10.))
            .items_center()
            .child(status)
            .when(failed && !running, |f| {
                f.child(
                    Button::new("tasks-send-logs")
                        .icon(crate::send_logs::icon())
                        .label(crate::send_logs::label())
                        .on_click(cx.listener(|s, _, window, cx| s.send_logs(window, cx))),
                )
            })
            .child(
                Button::new("tasks-close")
                    .primary()
                    .label("Close")
                    .disabled(running)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.tasks = None;
                        cx.notify();
                    })),
            );
        Some(
            Overlay::new("task-overlay")
                .child(
                    v_flex()
                        .id("task-dialog")
                        .occlude()
                        .w(px(width))
                        .p(px(20.))
                        .gap(px(16.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.background)
                        .shadow_lg()
                        .child(header)
                        .when(counts.total > 0, |c| c.child(items).child(bar))
                        .children(note)
                        .child(footer),
                )
                .into_any_element(),
        )
    }
}

fn glyph(icon: IconName, size: f32, color: Hsla) -> AnyElement {
    Icon::new(icon)
        .size(px(size))
        .text_color(color)
        .into_any_element()
}

fn task_row(item: &TaskItem, last: bool, p: Palette) -> Div {
    let circle = |icon: IconName, color: Hsla| {
        div()
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(color.opacity(0.16))
            .child(glyph(icon, 12., color))
            .into_any_element()
    };
    let mark = match item.state {
        TaskState::Waiting => div()
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .child(div().size(px(7.)).rounded_full().bg(p.switch_off))
            .into_any_element(),
        TaskState::Running => Spinner::new()
            .icon(IconName::LoaderCircle)
            .with_size(px(18.))
            .color(p.accent_hover)
            .into_any_element(),
        TaskState::Done => circle(IconName::Check, p.success),
        TaskState::Failed => circle(IconName::X, p.error),
        TaskState::Skipped => circle(IconName::Minus, p.text_placeholder),
    };
    let label_color = match item.state {
        TaskState::Waiting | TaskState::Skipped => p.text_muted,
        _ => p.text,
    };
    let note_color = match item.state {
        TaskState::Failed => p.error,
        _ => p.text_placeholder,
    };
    h_flex()
        .gap(px(12.))
        .items_start()
        .px(px(12.))
        .py(px(9.))
        .when(!last, |r| r.border_b_1().border_color(p.border_variant))
        .when(item.state == TaskState::Running, |r| r.bg(p.accent.opacity(0.07)))
        .child(div().size(px(20.)).flex_shrink_0().child(mark))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .pt(px(1.))
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .text_color(label_color)
                        .truncate()
                        .child(item.label.clone()),
                )
                .when_some(item.note.clone(), |c, note| {
                    c.child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .text_color(note_color)
                            .child(note),
                    )
                }),
        )
}
