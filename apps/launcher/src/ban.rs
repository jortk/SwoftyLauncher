use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::*;
use swofty_i18n::{tr, trf};

use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::Overlay;

impl Shell {
    pub(crate) fn ban_overlay(&self, p: Palette, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let ban = self.ban.as_ref()?;
        let reason = match ban.reason.is_empty() {
            true => tr("No reason was given.").to_owned(),
            false => ban.reason.clone(),
        };
        let who = ban.by.as_ref().map(|by| trf!("Banned by {name}", name = by));
        let when = ban.at.map(format_when);
        let details = [who, when].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let card = v_flex()
            .w(px(460.))
            .max_w_full()
            .gap(px(14.))
            .p(px(28.))
            .items_center()
            .rounded(px(12.))
            .bg(p.surface)
            .border_1()
            .border_color(p.error.opacity(0.45))
            .child(
                div()
                    .size(px(56.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(p.error.opacity(0.14))
                    .child(Icon::new(IconName::CircleX).size(px(28.)).text_color(p.error)),
            )
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("Your account is banned from SwoftyLauncher")),
            )
            .child(
                div()
                    .w_full()
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(p.background)
                    .text_size(px(14.))
                    .child(reason),
            )
            .children(
                (!details.is_empty())
                    .then(|| div().text_size(px(12.)).text_color(p.text_muted).child(details)),
            )
            .child(
                h_flex().pt(px(6.)).child(
                    Button::new("ban-close")
                        .primary()
                        .h(px(38.))
                        .px(px(18.))
                        .label(tr("Close"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ban = None;
                            cx.notify();
                        })),
                ),
            );
        Some(Overlay::new("ban-overlay").heavy().below_title_bar().child(card))
    }
}

fn format_when(at: u64) -> String {
    let days = at / 86_400;
    let (y, m, d) = civil(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
