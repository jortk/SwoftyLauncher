//! Developer mode: a section on the Tools page with the `swofty dev` actions
//! for one working folder. Each action runs the CLI's `swofty dev` command, so
//! the launcher and the CLI share one implementation.

use std::path::PathBuf;
use std::process::Command;

use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::pages::label;
use crate::shell::{PlayState, Shell};
use crate::theme::Palette;
use swofty_i18n::tr;

#[derive(Default)]
pub struct Developer {
    pub folder: Option<PathBuf>,
    pub running: Option<&'static str>,
    pub output: Option<(String, bool)>,
}

pub const ACTIONS: &[(&str, &str)] = &[
    ("check", "Check"),
    ("test", "Test"),
    ("pack", "Pack"),
    ("logs", "Logs"),
];

pub fn dev_args(verb: &str, folder: &std::path::Path) -> Vec<std::ffi::OsString> {
    vec!["dev".into(), verb.into(), "--".into(), folder.into()]
}

/// Why Run, which presses Play, can't start now: `None` only when the Play button would play.
fn run_blocked(state: &PlayState) -> Option<&'static str> {
    match state {
        PlayState::Ready | PlayState::Crashed(_) => None,
        PlayState::Running | PlayState::Elsewhere(_) => Some(tr("Close F1 25 before running it again")),
        PlayState::Preparing | PlayState::Launching => Some(tr("F1 25 is launching")),
        PlayState::Restoring => Some(tr("Restoring the game files…")),
        PlayState::Building => Some(tr("Wait for the build to finish")),
        PlayState::NotInstalled => Some(tr("Install it with Steam or the EA app, or locate its folder")),
        PlayState::Detecting => Some(tr("Detecting F1 25…")),
        PlayState::Checking => Some(tr("Checking conflicts…")),
    }
}

impl Shell {
    pub(crate) fn set_developer(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.developer = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.developer_mode = config.developer;
        cx.notify();
    }

    fn developer_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Link".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(folder) = paths.into_iter().next()
            {
                this.update_in(cx, |s, window, cx| {
                    s.developer.folder = Some(folder);
                    s.developer_run("link", window, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    fn developer_run(&mut self, verb: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.developer.folder.clone() else {
            return;
        };
        let Some(cli) = crate::game::cli_path(&self.store) else {
            window.notify(Notification::error(tr("The swofty command isn't installed")), cx);
            return;
        };
        self.developer.running = Some(verb);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let out = cx
                .background_executor()
                .spawn(async move {
                    Command::new(cli)
                        .args(dev_args(verb, &folder))
                        .env("SWOFTY_DEVELOPER", "1")
                        .output()
                })
                .await;
            this.update_in(cx, |s, _, cx| {
                s.developer.running = None;
                s.developer.output = Some(match out {
                    Ok(o) => {
                        let text = format!(
                            "{}{}",
                            String::from_utf8_lossy(&o.stdout),
                            String::from_utf8_lossy(&o.stderr)
                        );
                        (text.trim_end().to_owned(), o.status.success())
                    }
                    Err(e) => (e.to_string(), false),
                });
                if verb == "link" {
                    s.reload(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn developer_section(&mut self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.developer_mode {
            return None;
        }
        let running = self.developer.running;
        let has_folder = self.developer.folder.is_some();
        let blocked = run_blocked(&self.play_state()).or(self.mp_starting_label());
        let folder = self
            .developer
            .folder
            .as_ref()
            .map(|f| f.display().to_string())
            .unwrap_or_else(|| "Link a working folder to install it by reference".into());
        let mut buttons = h_flex().gap(px(8.)).child(
            Button::new("dev-link")
                .outline()
                .tap_sm()
                .label(if has_folder {
                    tr("Link another folder…")
                } else {
                    "Link a folder…"
                })
                .disabled(running.is_some())
                .on_click(cx.listener(|this, _, window, cx| this.developer_link(window, cx))),
        );
        for (verb, title) in ACTIONS {
            buttons = buttons.child(
                Button::new(SharedString::from(format!("dev-{verb}")))
                    .outline()
                    .tap_sm()
                    .label(if running == Some(*verb) {
                        format!("{title}…")
                    } else {
                        (*title).to_owned()
                    })
                    .disabled(!has_folder || running.is_some())
                    .on_click(cx.listener(move |this, _, window, cx| this.developer_run(verb, window, cx))),
            );
        }
        buttons = buttons.child(
            Button::new("dev-run")
                .primary()
                .tap_sm()
                .label("Run")
                .when_some(blocked, |b, why| b.tooltip(why))
                .disabled(!has_folder || running.is_some() || blocked.is_some())
                .on_click(cx.listener(|this, _, window, cx| this.play(window, cx))),
        );
        let output = self.developer.output.clone().map(|(text, ok)| {
            div()
                .id("dev-output")
                .max_h(px(220.))
                .overflow_y_scroll()
                .p(px(10.))
                .rounded(px(5.))
                .border_1()
                .border_color(if ok { p.border_variant } else { p.error })
                .font_family("monospace")
                .text_size(px(12.))
                .text_color(p.text_muted)
                .child(text)
        });
        Some(
            v_flex()
                .gap(px(8.))
                .max_w(px(760.))
                .child(label("DEVELOPER", p))
                .child(
                    v_flex()
                        .gap(px(10.))
                        .p(px(12.))
                        .rounded(px(5.))
                        .border_1()
                        .border_color(p.border_variant)
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(folder))
                        .child(buttons)
                        .when_some(output, |d, o| d.child(o)),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIONS, dev_args, run_blocked};
    use crate::game::Owner;
    use crate::shell::PlayState;

    #[test]
    fn actions_run_the_cli_dev_commands() {
        assert_eq!(
            dev_args("check", std::path::Path::new("/work/mod.esp")),
            ["dev", "check", "--", "/work/mod.esp"]
        );
        assert_eq!(
            dev_args("link", std::path::Path::new("-mods/x")),
            ["dev", "link", "--", "-mods/x"]
        );
        assert_eq!(
            ACTIONS.iter().map(|a| a.0).collect::<Vec<_>>(),
            ["check", "test", "pack", "logs"]
        );
    }

    #[test]
    fn run_waits_for_the_game_like_play_does() {
        assert_eq!(run_blocked(&PlayState::Ready), None);
        for state in [
            PlayState::Running,
            PlayState::Elsewhere(Owner::Unknown),
            PlayState::Preparing,
            PlayState::Launching,
            PlayState::Restoring,
            PlayState::Building,
            PlayState::NotInstalled,
            PlayState::Detecting,
            PlayState::Checking,
        ] {
            assert!(run_blocked(&state).is_some());
        }
    }
}
