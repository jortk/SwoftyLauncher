use std::path::Path;
use std::time::Duration;

use crate::notices::{Notification, Notify};
use crate::scale::px;
use gpui_kit::component::WindowExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::install::Source;
use swofty_core::{compat, copies};

use crate::app_log;
use crate::game::Install;
use crate::shell::Shell;
use swofty_i18n::{tr, trf};

const TITLE: &str = "F1 25 needs administrator rights";

pub(crate) fn text(dir: &Path, source: Source, windows: bool) -> String {
    if !windows {
        return format!("{}.", compat::write_blocked_text_for(dir, false));
    }
    trf!(
        "F1 25 is installed in {folder}, a folder Windows lets only administrators change, so Swofty can't add your mods there.\n\nRestart Swofty Launcher as administrator to play with mods, or move F1 25 to a folder outside Program Files{hint}.",
        folder = dir.display(),
        hint = copies::move_hint(source)
    )
}

pub(crate) fn run_as_admin_title() -> &'static str {
    tr("A program is set to run as administrator")
}

/// [`compat::RunAsAdmin::headline`] in the launcher's language.
pub(crate) fn run_as_admin_headline(r: &compat::RunAsAdmin) -> String {
    match r.names().as_slice() {
        [name] => trf!(
            "Windows won't start {name}: it is set to run as administrator",
            name = name
        ),
        names => trf!(
            "Windows won't start {names}: they are set to run as administrator",
            names = names.join(", ")
        ),
    }
}

/// [`compat::RunAsAdmin::text`] in the launcher's language, a paragraph each
/// for what Windows refuses, why running as administrator doesn't help, and
/// what to untick.
pub(crate) fn run_as_admin_text(r: &compat::RunAsAdmin) -> String {
    let (names, paths) = (r.names(), r.paths());
    let (what, fix) = match (names.as_slice(), paths.as_slice()) {
        ([name], [path]) => (
            trf!(
                "{name} is set to run as administrator, so Windows won't start it for an EA app session (error 740).",
                name = name
            ),
            trf!(
                "Right-click {path}, open Properties > Compatibility and untick \"Run this program as an administrator\", also under \"Change settings for all users\". Then press Play again.",
                path = path
            ),
        ),
        _ => (
            trf!(
                "These files are set to run as administrator, so Windows won't start them for an EA app session (error 740):\n{paths}",
                paths = paths.join("\n")
            ),
            tr("For each one, right-click it, open Properties > Compatibility and untick \"Run this program as an administrator\", also under \"Change settings for all users\". Then press Play again.").to_owned(),
        ),
    };
    let note = r.game.then(|| {
        tr("The EA app starts F1 25 without administrator rights, so running Swofty Launcher as administrator doesn't help.")
            .to_owned()
    });
    std::iter::once(what)
        .chain(note)
        .chain([fix])
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl Shell {
    /// The game folder is one this launcher can't write in: say so and offer
    /// to restart as administrator, on Windows.
    pub(crate) fn show_admin_needed(
        &mut self,
        install: &Install,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let windows = cfg!(windows);
        let text = text(&install.dir, install.source, windows);
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .width(px(560.))
                .title(TITLE)
                .description(text.clone())
                .map(|a| {
                    if !windows {
                        return a.ok_text("OK");
                    }
                    a.ok_text("Restart as administrator")
                        .show_cancel(true)
                        .on_ok(|_, window, cx| {
                            restart_elevated(window, cx);
                            true
                        })
                })
        });
    }
}

/// How long the administrator copy has to keep running before this one closes.
const ELEVATED_SETTLE: Duration = Duration::from_secs(6);

fn not_restarted(text: String, window: &mut Window, cx: &mut App) {
    window.notify(
        Notification::warning(text)
            .title(tr("Not restarted as administrator"))
            .autohide(false),
        cx,
    );
}

/// Starts the administrator copy and closes this one once it is still
/// running a few seconds later. One that closes before then is reported.
fn restart_elevated(window: &mut Window, cx: &mut App) {
    app_log::note("restart as administrator: asking Windows");
    let started = cx.background_executor().spawn(async move {
        std::thread::spawn(compat::relaunch_elevated)
            .join()
            .unwrap_or_else(|_| Err("the administrator prompt failed".to_owned()))
    });
    window
        .spawn(cx, async move |cx| {
            let started = started.await;
            cx.update(|window, cx| elevated_started(started, window, cx)).ok();
        })
        .detach();
}

fn elevated_started(started: Result<compat::Elevated, String>, window: &mut Window, cx: &mut App) {
    let elevated = match started {
        Ok(e) => e,
        Err(why) => {
            app_log::note(&format!("restart as administrator: {why}"));
            return not_restarted(
                trf!(
                    "Windows didn't restart Swofty Launcher: {why}. It keeps running without administrator rights.",
                    why = why
                ),
                window,
                cx,
            );
        }
    };
    app_log::note(&format!(
        "restart as administrator: started the administrator copy{}",
        elevated
            .pid()
            .map(|pid| format!(", process {pid}"))
            .unwrap_or_default()
    ));
    window.minimize_window();
    let wait = cx
        .background_executor()
        .spawn(async move { elevated.exited_within(ELEVATED_SETTLE) });
    window
        .spawn(cx, async move |cx| {
            let exited = wait.await;
            cx.update(|window, cx| match exited {
                None => {
                    app_log::note("restart as administrator: the administrator copy is running; closing");
                    cx.quit();
                }
                Some(code) => {
                    app_log::note(&format!(
                        "restart as administrator: the administrator copy closed with exit code {code:#x}"
                    ));
                    window.activate_window();
                    not_restarted(
                        trf!(
                            "The administrator copy of Swofty Launcher closed as it started (exit code {code}). This one keeps running without administrator rights. Press {button} so the developers can see why.",
                            code = format!("{code:#x}"),
                            button = crate::send_logs::label()
                        ),
                        window,
                        cx,
                    );
                }
            })
            .ok();
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::{run_as_admin_headline, run_as_admin_text, text};
    use std::path::{Path, PathBuf};
    use swofty_core::compat::RunAsAdmin;
    use swofty_core::install::Source;

    #[test]
    fn run_as_admin_says_what_the_cli_says_a_paragraph_at_a_time() {
        let game = RunAsAdmin::new([PathBuf::from(r"D:\Games\F1 25\F1_25.exe")], true);
        let shown = run_as_admin_text(&game);
        assert_eq!(shown.split("\n\n").count(), 3, "{shown}");
        assert_eq!(shown.replace("\n\n", " "), game.text());
        assert_eq!(run_as_admin_headline(&game), game.headline());
        let ea = RunAsAdmin::new(
            [
                PathBuf::from(r"C:\EA Desktop\EALauncher.exe"),
                PathBuf::from(r"C:\EA Desktop\EADesktop.exe"),
            ],
            false,
        );
        let shown = run_as_admin_text(&ea);
        assert!(
            shown.contains(
                "(error 740):\nC:\\EA Desktop\\EALauncher.exe\nC:\\EA Desktop\\EADesktop.exe\n\nFor each one"
            ),
            "{shown}"
        );
        assert!(!shown.contains("Swofty Launcher as administrator"), "{shown}");
        assert_eq!(run_as_admin_headline(&ea), ea.headline());
    }

    #[test]
    fn names_the_folder_and_both_ways_out() {
        let ea = text(Path::new(r"C:\Program Files\EA Games\F1 25"), Source::EaApp, true);
        assert!(
            ea.starts_with(r"F1 25 is installed in C:\Program Files\EA Games\F1 25, a folder"),
            "{ea}"
        );
        assert!(ea.contains("Restart Swofty Launcher as administrator"), "{ea}");
        assert!(
            ea.ends_with("outside Program Files with EA app > F1 25 > Manage > Move, not File Explorer."),
            "{ea}"
        );
        let steam = text(
            Path::new(r"C:\Program Files\Steam\steamapps\common\F1 25"),
            Source::Steam,
            true,
        );
        assert!(steam.contains("Move install folder"), "{steam}");
        assert!(!steam.contains("EA app"), "{steam}");
        let linux = text(Path::new("/games/F1 25"), Source::Steam, false);
        assert!(
            linux.contains("/games/F1 25") && !linux.contains("administrator"),
            "{linux}"
        );
    }
}
