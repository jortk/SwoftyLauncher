use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::vault::careers::State as CareerState;
use swofty_i18n::{tr, trf};

use super::labels::listed_later;
use super::state::{Entry, Imported, SavesView};
use crate::format::ago_since as ago;
use crate::pages::label;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;

use super::list::{list_box, placeholder};
use crate::ui::Pill;
use crate::ui::tokens::Tone;
pub(super) const MODE_W: f32 = 124.;
pub(super) const PLAYED_W: f32 = 104.;
pub(super) const CHIP_W: f32 = 112.;

impl Shell {
    pub(super) fn careers_block(&self, view: &SavesView, p: Palette, _cx: &mut Context<Self>) -> Div {
        let rows: Vec<AnyElement> = match (&view.imported, view.entries.is_empty()) {
            (Some(i), true) => vec![self.imported_row(i, p)],
            (None, true) => vec![placeholder(
                tr("No careers yet. They appear after you play this profile."),
                p,
            )],
            _ => view.entries.iter().map(|e| self.career_row(e, p)).collect(),
        };
        v_flex()
            .gap(px(8.))
            .child(label(tr("CAREERS"), p))
            .child(list_box(rows, p))
    }

    pub(super) fn imported_row(&self, i: &Imported, p: Palette) -> AnyElement {
        h_flex()
            .min_h(px(52.))
            .px(px(12.))
            .py(px(8.))
            .gap(px(10.))
            .items_center()
            .child(
                div().w(px(MODE_W)).flex_shrink_0().child(
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(Icon::new(IconName::Save).size(px(15.)))
                        .child(tr("Saved game")),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(i.title.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(listed_later()),
                    ),
            )
            .child(
                div()
                    .w(px(PLAYED_W + CHIP_W))
                    .flex_shrink_0()
                    .truncate()
                    .text_right()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .children(i.at.map(|at| trf!("imported {when}", when = ago(at as f64)))),
            )
            .into_any_element()
    }

    pub(super) fn career_row(&self, e: &Entry, p: Palette) -> AnyElement {
        let chip = match e.l.state {
            CareerState::Present => None,
            CareerState::Missing => Some(Pill::new(tr("Missing")).tone(Tone::Warning)),
            CareerState::DeletedInGame => Some(Pill::new(tr("Deleted in game"))),
        };
        let dim = chip.is_some();
        h_flex()
            .min_h(px(52.))
            .px(px(12.))
            .py(px(8.))
            .gap(px(10.))
            .items_center()
            .child(
                div()
                    .w(px(MODE_W))
                    .flex_shrink_0()
                    .truncate()
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(e.mode()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .when(dim, |d| d.text_color(p.text_muted))
                            .child(e.l.title()),
                    )
                    .children(e.place.clone().map(|pl| {
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(pl)
                    })),
            )
            .child(
                div()
                    .w(px(PLAYED_W))
                    .flex_shrink_0()
                    .truncate()
                    .text_right()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .children(e.l.last_played.map(|at| ago(at as f64))),
            )
            .child(
                h_flex()
                    .w(px(CHIP_W))
                    .flex_shrink_0()
                    .justify_end()
                    .children(chip),
            )
            .into_any_element()
    }
}
