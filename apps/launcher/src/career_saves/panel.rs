use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::vault::{self};
use swofty_i18n::{tr, trf};

use super::labels::list;
use super::state::Version;
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;

impl Shell {
    pub(super) fn restore_panel(
        &self,
        v: &Version,
        layout: vault::Layout,
        blocker: Option<String>,
        (id, source): (String, String),
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let when = vault::human(v.snap.at);
        let line = |head: &'static str, tone: Hsla, text: String| {
            h_flex()
                .gap(px(8.))
                .items_start()
                .text_size(px(13.))
                .child(
                    div()
                        .w(px(100.))
                        .flex_shrink_0()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tone)
                        .child(head),
                )
                .child(div().flex_1().min_w_0().text_color(p.text_muted).child(text))
        };
        let mut lines: Vec<Div> = Vec::new();
        match &v.diff {
            Some(d) => {
                lines.push(match d.get_back.is_empty() {
                    true => line(tr("You get back"), p.text_placeholder, tr("No careers.").into()),
                    false => line(tr("You get back"), p.success, list(&d.get_back)),
                });
                let mut lose = Vec::new();
                let flagged: Vec<&str> = d.marked_missing.iter().map(|b| b.career.as_str()).collect();
                let absent: Vec<_> = d
                    .lose
                    .iter()
                    .filter(|b| !flagged.contains(&b.career.as_str()))
                    .cloned()
                    .collect();
                if !absent.is_empty() {
                    lose.push(trf!("{careers} (not in that version)", careers = list(&absent)));
                }
                if !d.marked_missing.is_empty() {
                    lose.push(trf!(
                        "{careers} (that version lists them as missing)",
                        careers = list(&d.marked_missing)
                    ));
                }
                for (now, then) in &d.progress {
                    lose.push(trf!(
                        "progress in {career}: {now} goes back to {then}",
                        career = now.name,
                        now = now.place.as_deref().unwrap_or(tr("now")),
                        then = then.place.as_deref().unwrap_or(tr("then"))
                    ));
                }
                lines.push(match lose.is_empty() {
                    true => line(tr("You lose"), p.text_placeholder, tr("No careers.").into()),
                    false => line(tr("You lose"), p.error, lose.join("; ")),
                });
            }
            None => lines.push(line(
                tr("Careers"),
                p.text_placeholder,
                tr("No career list to compare.").into(),
            )),
        }
        lines.push(line(
            tr("Also"),
            p.text_placeholder,
            trf!(
                "Settings and setups go back to {when}. The save as it is now is kept as a version.",
                when = when
            ),
        ));
        let close = match layout {
            vault::Layout::Steam => Some(tr("Steam uploads the restored save when it starts.")),
            _ => None,
        };
        v_flex()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(5.))
            .bg(p.surface)
            .border_1()
            .border_color(p.border_variant)
            .children(lines)
            .children(close.map(|c| div().text_size(px(12.)).text_color(p.text_placeholder).child(c)))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        Button::new("restore-confirm")
                            .primary()
                            .tap_sm()
                            .label(tr("Restore"))
                            .disabled(blocker.is_some())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.restore_version(&source, &id, window, cx)
                            })),
                    )
                    .child(
                        Button::new("restore-cancel")
                            .ghost()
                            .tap_sm()
                            .label(tr("Cancel"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.saves.confirm = None;
                                cx.notify();
                            })),
                    ),
            )
    }
}
