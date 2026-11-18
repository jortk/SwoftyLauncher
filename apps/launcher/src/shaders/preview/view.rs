use std::sync::Arc;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf};

use crate::shaders::ShadersView;
use crate::theme::Palette;
use crate::ui::ProgressBar;

use super::control::clock;
use super::{Effects, Status};

impl ShadersView {
    pub(in crate::shaders) fn preview_pane(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let status = self.preview.status.clone();
        let body = match &status {
            Status::Idle | Status::Opening => {
                self.preview_message(Icon::new(IconName::LoaderCircle), tr("Opening the clip"), None, p)
            }
            Status::Downloading => self.download_card(p),
            Status::DownloadFailed(e) => {
                let retry = Button::new("shaders-clips-retry")
                    .outline()
                    .icon(IconName::RotateCw)
                    .label(tr("Try again"))
                    .on_click(cx.listener(|this, _, _, cx| this.preview_download(cx)));
                self.preview_message(
                    Icon::new(IconName::CloudOff),
                    tr("The preview clips couldn't be downloaded"),
                    Some((e.clone(), retry.into_any_element())),
                    p,
                )
            }
            Status::NoGpu(e) => self.preview_message(
                Icon::new(IconName::Cpu),
                tr("The preview can't use the graphics card"),
                Some((e.clone(), div().into_any_element())),
                p,
            ),
            Status::Failed(e) => self.preview_message(
                Icon::new(IconName::CircleAlert),
                tr("The preview can't run"),
                Some((e.clone(), div().into_any_element())),
                p,
            ),
            Status::Ready => {
                let area = self.preview.area.clone();
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .flex()
                    .justify_center()
                    .child(
                        canvas(
                            move |b, window, _| {
                                if area.get() != b.size {
                                    area.set(b.size);
                                    window.refresh();
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(self.viewport(p, cx))
                    .into_any_element()
            }
        };
        let notes = self.preview_notes(p);
        let strip = self.triggers_strip(p, cx);
        v_flex()
            .size_full()
            .min_h_0()
            .gap(px(10.))
            .child(body)
            .children(notes)
            .child(self.transport(p, cx))
            .children(strip)
            .into_any_element()
    }

    pub(super) fn unavailable_note(&self) -> Option<String> {
        let used: Vec<&str> = self
            .preview
            .instances
            .iter()
            .map(|i| i.effect.as_str())
            .filter(|id| self.preview.unavailable.iter().any(|u| u == id))
            .collect();
        (!used.is_empty()).then(|| {
            trf!(
                "Not shown in the preview, still applied in game: {effects}.",
                effects = used.join(", ")
            )
        })
    }

    pub(super) fn preview_notes(&self, p: Palette) -> Option<AnyElement> {
        let line = match (&self.preview.effects, &self.preview.render_error) {
            (Effects::Failed(e), _) => Some((IconName::TriangleAlert, p.warning, e.clone())),
            (_, Some(e)) => Some((IconName::TriangleAlert, p.warning, e.clone())),
            _ if self.unavailable_note().is_some() => {
                self.unavailable_note().map(|n| (IconName::Info, p.text_muted, n))
            }
            _ if self.editor.lint.errors() > 0 => Some((
                IconName::Info,
                p.text_muted,
                tr("The preview shows the last config without problems.").to_owned(),
            )),
            _ => None,
        }?;
        Some(
            h_flex()
                .gap(px(8.))
                .items_center()
                .text_size(px(12.))
                .text_color(line.1)
                .child(Icon::new(line.0).size(px(13.)))
                .child(div().flex_1().min_w_0().child(line.2))
                .into_any_element(),
        )
    }

    pub(super) fn preview_message(
        &self,
        icon: Icon,
        title: &'static str,
        detail: Option<(String, AnyElement)>,
        p: Palette,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .flex_1()
            .min_h(px(240.))
            .items_center()
            .justify_center()
            .gap(px(6.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .child(icon.size(px(24.)).text_color(p.text_placeholder))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .when_some(detail, |d, (text, action)| {
                d.child(
                    div()
                        .max_w(px(420.))
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .text_center()
                        .child(text),
                )
                .child(div().pt(px(6.)).child(action))
            })
            .into_any_element()
    }

    pub(super) fn download_card(&self, p: Palette) -> AnyElement {
        let (fraction, label) = self
            .preview
            .progress
            .lock()
            .map(|g| g.clone())
            .unwrap_or((0., String::new()));
        v_flex()
            .w_full()
            .flex_1()
            .min_h(px(240.))
            .items_center()
            .justify_center()
            .gap(px(8.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .child(Icon::new(IconName::Download).size(px(24.)).text_color(p.accent))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("Getting the preview clips")),
            )
            .child(div().text_size(px(13.)).text_color(p.text_muted).child(tr(
                "A few seconds of F1 25 footage to try your shaders on. This happens once.",
            )))
            .child(
                div()
                    .mt(px(8.))
                    .w(px(280.))
                    .child(ProgressBar::new(fraction.clamp(0.02, 1.)).height(6.)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(label),
            )
            .into_any_element()
    }

    pub(super) fn viewport(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let bounds = self.preview.bounds.clone();
        let compare = self.preview.compare && self.preview.original_image.is_some();
        let split = self.preview.split.clamp(0.02, 0.98);
        let config = self.preview.config_image.clone();
        let original = self.preview.original_image.clone();
        let ratio = self
            .preview
            .info
            .as_ref()
            .filter(|i| i.height > 0)
            .map_or(16. / 9., |i| i.width as f32 / i.height as f32);
        let area = self.preview.area.get();
        let fit = (area.width > px(0.) && area.height > px(0.)).then(|| {
            let w = area.width.min(area.height * ratio);
            (w, w / ratio)
        });
        let frame = div()
            .id("shaders-viewport")
            .relative()
            .map(|d| match fit {
                Some((w, h)) => d.w(w).h(h),
                None => d.w_full().aspect_ratio(ratio),
            })
            .rounded(px(10.))
            .overflow_hidden()
            .bg(p.chrome)
            .border_1()
            .border_color(p.border)
            .child(frame_image(config, p))
            .child(
                canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            );
        let frame = if compare {
            let drag_start = cx.listener(|this, e: &MouseDownEvent, _, cx| {
                this.preview.dragging = true;
                this.wipe_to(e.position.x, cx);
            });
            frame
                .child(original_half(original, split))
                .child(wipe_bar(split, p))
                .child(wipe_knob(split, p))
                .child(wipe_label(tr("Original"), true, p))
                .child(wipe_label(tr("Config"), false, p))
                .cursor_col_resize()
                .on_mouse_down(MouseButton::Left, drag_start)
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                    if this.preview.dragging && e.dragging() {
                        this.wipe_to(e.position.x, cx);
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.preview.dragging = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.preview.dragging = false;
                        cx.notify();
                    }),
                )
        } else {
            frame
        };
        div().w_full().child(frame).into_any_element()
    }

    pub(super) fn transport(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let ready = self.preview.status == Status::Ready && self.preview.info.is_some();
        let (frames, fps) = self.preview.info.as_ref().map_or((0, 25.), |i| (i.frames, i.fps));
        let time = format!(
            "{} / {}",
            clock(self.preview.frame, fps),
            clock(frames.saturating_sub(1), fps)
        );
        let playing = self.preview.playing;
        let compare = self.preview.compare;
        let titles: Vec<String> = self.preview.clips.iter().map(|c| c.title.clone()).collect();
        let current = titles
            .get(self.preview.clip)
            .cloned()
            .unwrap_or_else(|| tr("Clip").into());
        let shaders = cx.entity().downgrade();
        let at = self.preview.clip;
        let picker = Button::new("shaders-clip")
            .outline()
            .tap_sm()
            .icon(IconName::Flag)
            .label(current)
            .dropdown_caret(true)
            .disabled(titles.len() < 2)
            .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                for (i, title) in titles.iter().enumerate() {
                    let shaders = shaders.clone();
                    menu = menu.item(PopupMenuItem::new(title.clone()).checked(i == at).on_click(
                        move |_, _, cx| {
                            shaders.update(cx, |v, cx| v.preview_pick(i, cx)).ok();
                        },
                    ));
                }
                menu
            });
        h_flex()
            .flex_shrink_0()
            .gap(px(10.))
            .items_center()
            .child(
                Button::new("shaders-play")
                    .outline()
                    .tap_sm()
                    .icon(if playing { IconName::Pause } else { IconName::Play })
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_play(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Slider::new(&self.preview.slider).disabled(!ready)),
            )
            .child(
                div()
                    .w(px(112.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(time),
            )
            .child(
                Button::new("shaders-compare")
                    .tap_sm()
                    .map(|b| if compare { b } else { b.outline() })
                    .icon(IconName::ArrowRightLeft)
                    .label(tr("Compare"))
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_compare(cx))),
            )
            .child(picker)
            .into_any_element()
    }
}

pub(super) fn wipe_label(text: &'static str, left: bool, p: Palette) -> Div {
    div()
        .absolute()
        .top(px(10.))
        .map(|d| if left { d.left(px(10.)) } else { d.right(px(10.)) })
        .px(px(8.))
        .py(px(3.))
        .rounded(px(4.))
        .bg(p.chrome.opacity(0.75))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text)
        .child(text)
}

fn frame_image(config: Option<Arc<RenderImage>>, p: Palette) -> AnyElement {
    match config {
        Some(image) => img(image)
            .size_full()
            .object_fit(ObjectFit::Fill)
            .into_any_element(),
        None => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(IconName::LoaderCircle)
                    .size(px(20.))
                    .text_color(p.text_placeholder),
            )
            .into_any_element(),
    }
}

fn original_half(original: Option<Arc<RenderImage>>, split: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .h_full()
        .w(relative(split))
        .overflow_hidden()
        .when_some(original, |d, image| {
            d.child(
                img(image)
                    .h_full()
                    .w(relative(1. / split))
                    .flex_shrink_0()
                    .object_fit(ObjectFit::Fill),
            )
        })
}

fn wipe_bar(split: f32, p: Palette) -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(split))
        .w(px(2.))
        .ml(px(-1.))
        .bg(p.on_accent)
}

fn wipe_knob(split: f32, p: Palette) -> Div {
    div()
        .absolute()
        .top(relative(0.5))
        .left(relative(split))
        .size(px(28.))
        .ml(px(-14.))
        .mt(px(-14.))
        .rounded_full()
        .bg(p.accent)
        .border_2()
        .border_color(p.on_accent)
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(IconName::ChevronsUpDown)
                .size(px(14.))
                .text_color(p.on_accent),
        )
}
