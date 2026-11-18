use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::vault::{Notice, Vault};
use swofty_i18n::tr;

use super::banner_data::Kind;
use crate::pages::GUTTER;
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::Callout;

impl Shell {
    pub(crate) fn saves_banner(&self, on_page: bool, _: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let shown = self.saves.banner.as_ref()?.shown();
        let kind = shown.kind;
        let review = shown.snapshot.clone();
        let restore = (shown.source.clone(), shown.snapshot);
        let dismiss_source = shown.source;
        let actions = h_flex()
            .gap(px(8.))
            .flex_wrap()
            .when(kind != Kind::Hint, |row| {
                row.child(
                    Button::new("saves-banner-restore")
                        .tap_sm()
                        .label(tr("Restore"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.restore_version(&restore.0, &restore.1, window, cx);
                        })),
                )
            })
            .child(
                Button::new("saves-banner-review")
                    .outline()
                    .tap_sm()
                    .label(match on_page {
                        true => tr("Review the restore"),
                        false => tr("Open Career saves"),
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_saves(Some(review.clone()), cx);
                    })),
            )
            .child(
                Button::new("saves-banner-dismiss")
                    .ghost()
                    .tap_sm()
                    .label(tr("Dismiss"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let root = this.store.root.clone();
                        if kind == Kind::Hint {
                            Notice::clear(&root);
                        }
                        if let Ok(v) = Vault::open(&root, &dismiss_source) {
                            v.dismiss_drop().ok();
                            if kind == Kind::Lost {
                                v.dismiss_loss().ok();
                            }
                        }
                        this.refresh_saves_in_background(cx);
                        cx.notify();
                    })),
            );
        Some(
            div()
                .px(px(GUTTER))
                .pb(px(12.))
                .flex_shrink_0()
                .child(Callout::warning(shown.text).actions(actions)),
        )
    }
}
