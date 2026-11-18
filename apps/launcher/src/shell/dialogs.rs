use crate::notices::Notification;
use crate::play_checks;
use crate::scale::px;
use crate::theme::Palette;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::{AlertDialog, DialogFooter};
use gpui_kit::component::{Icon, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::launch;
use swofty_core::quarantine;
use swofty_core::saves::{self, SavesWarning};

use super::Shell;
use swofty_i18n::tr;

pub(super) fn section_label(title: &'static str, p: Palette) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_placeholder)
        .child(title)
}

pub(super) fn terms_box(p: Palette) -> impl IntoElement {
    v_flex()
        .gap(px(6.))
        .p(px(12.))
        .rounded(px(6.))
        .bg(p.elevated)
        .border_1()
        .border_color(p.border)
        .child(
            h_flex()
                .gap(px(8.))
                .items_center()
                .text_color(p.accent)
                .child(Icon::new(IconName::CircleAlert).size(px(15.)))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child(tr("Outside EA's terms of service")),
                ),
        )
        .child(div().text_size(px(13.)).text_color(p.text).child(tr(
            "EA's terms don't cover playing without EA AntiCheat, even offline. We know of \
                 no action taken against players for offline modding, but EA can still act on \
                 accounts under its terms.",
        )))
}

/// The loader files an antivirus took: before the launch they stopped the
/// game from starting, in game it runs without mods. The dialog says which
/// files and how to get them back. When the session found nothing that
/// explains a miss, it offers Send Logs and the support Discord.
pub(super) fn show_mods_blocked(
    r: &quarantine::Report,
    shell: WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, text) = (r.title(), r.text());
    let unexplained = r.stage == quarantine::Stage::InGame
        && !r.removed
        && r.cause.as_deref().is_some_and(quarantine::Cause::unexplained);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let alert = alert.width(px(560.)).title(title).description(text.clone());
        if !unexplained {
            return alert.ok_text(tr("OK"));
        }
        let shell = shell.clone();
        alert.footer(
            DialogFooter::new()
                .justify_end()
                .gap(px(8.))
                .child(
                    Button::new("blocked-discord")
                        .outline()
                        .label(tr("Join the support Discord"))
                        .on_click(|_, _, cx| cx.open_url(crate::discord::INVITE)),
                )
                .child(
                    Button::new("blocked-send-logs")
                        .primary()
                        .label(crate::send_logs::label())
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            shell.update(cx, |s, cx| s.send_logs(window, cx)).ok();
                        }),
                )
                .child(
                    Button::new("blocked-ok")
                        .ghost()
                        .label(tr("OK"))
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
        )
    });
}

/// A launch that failed without a crash: F1 25 was already running, or the
/// game closed itself as it started. The dialog says why and what to do.
pub(super) fn show_launch_failed(f: &launch::Failure, window: &mut Window, cx: &mut App) {
    let (title, text) = match &f.cause {
        launch::Cause::RunAsAdmin(r) => (
            crate::admin_needed::run_as_admin_title(),
            crate::admin_needed::run_as_admin_text(r),
        ),
        _ => (f.title(), f.text()),
    };
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .width(px(560.))
            .title(title)
            .description(text.clone())
            .ok_text(tr("OK"))
    });
}

/// The play bar's line for a launch that failed without a crash.
pub(super) fn launch_headline(f: &launch::Failure) -> String {
    match &f.cause {
        launch::Cause::RunAsAdmin(r) => crate::admin_needed::run_as_admin_headline(r),
        _ => f.headline(),
    }
}

pub(crate) fn offline_terms(p: Palette) -> impl IntoElement {
    v_flex()
        .gap(px(14.))
        .child(div().text_size(px(13.)).text_color(p.text_muted).child(tr(
            "Modded sessions run F1 25 without EA AntiCheat so the loader \
                 and addons can work. Everything is undone when the game exits.",
        )))
        .child(terms_box(p))
        .child(
            v_flex()
                .gap(px(7.))
                .child(section_label(tr("OFFLINE ONLY"), p))
                .child(div().text_size(px(13.)).text_color(p.text).child(tr(
                    "Multiplayer is unavailable and cannot be enabled. The component is \
                         downloaded automatically and verified against pinned SHA-256 digests.",
                ))),
        )
        .child(div().text_size(px(12.)).text_color(p.text_muted).child(tr(
            "We may collect anonymised usage data from your use of the launcher.",
        )))
}

pub(super) fn saves_alert(
    alert: AlertDialog,
    w: &SavesWarning,
    then: Option<&'static str>,
    p: Palette,
) -> AlertDialog {
    let note = w.message.clone().map(|m| {
        h_flex()
            .gap(px(10.))
            .items_start()
            .p(px(12.))
            .rounded(px(6.))
            .bg(p.elevated)
            .border_1()
            .border_color(p.border)
            .child(
                Icon::new(IconName::CircleAlert)
                    .size(px(15.))
                    .text_color(p.warning),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child(w.name.clone()),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(m)),
            )
    });
    alert
        .width(px(480.))
        .title(saves::TITLE)
        .child(
            v_flex()
                .gap(px(12.))
                .child(div().text_size(px(13.)).text_color(p.text).child(saves::BODY))
                .children(note)
                .when_some(then, |d, t| {
                    d.child(div().text_size(px(13.)).text_color(p.text_muted).child(t))
                }),
        )
        .ok_variant(ButtonVariant::Danger)
        .show_cancel(true)
}

pub(super) fn note_notification(note: &play_checks::Note) -> Notification {
    use play_checks::NoteLevel;
    let n = match note.level {
        NoteLevel::Info => Notification::info(note.body.clone()),
        NoteLevel::Warning => Notification::warning(note.body.clone()),
        NoteLevel::Error => Notification::error(note.body.clone()),
    };
    let n = if note.sticky { n.autohide(false) } else { n };
    n.title(note.title.clone())
}
