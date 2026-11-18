use crate::notices::{Notification, Notify};
use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt, h_flex};
use gpui_kit::*;
use std::path::PathBuf;
use std::rc::Rc;
use swofty_core::install::Source;
use swofty_core::store::Store;

use super::Shell;
use swofty_i18n::{tr, trf};

pub type Action = Rc<dyn Fn(&Store, &str) -> Result<String, String>>;

pub struct Prompt {
    pub title: &'static str,
    pub description: String,
    pub initial: String,
    pub placeholder: &'static str,
    pub ok: &'static str,
    pub browse: Option<PathPromptOptions>,
    pub action: Action,
}

impl Shell {
    pub(crate) fn locate_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let adding = !self.copies.is_empty();
        self.prompt(
            Prompt {
                title: if adding { tr("Add another folder") } else { tr("Locate F1 25") },
                description: trf!(
                    "The folder that contains {exe}, from Steam, the EA app or a copy of your own. Play uses it once it's added.",
                    exe = swofty_core::GAME_EXE
                ),
                initial: String::new(),
                placeholder: if cfg!(windows) {
                    r"C:\Program Files\EA Games\F1 25"
                } else {
                    "/path/to/F1 25"
                },
                ok: tr("Add and play this copy"),
                browse: Some(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some(tr("Use folder").into()),
                }),
                action: Rc::new(|store, value| {
                    let dir = PathBuf::from(value);
                    let mut config = store.config();
                    swofty_core::copies::add(&mut config, &dir)?;
                    store.save_config(&config).map_err(|e| e.to_string())?;
                    Ok(trf!(
                        "Added the {store} copy",
                        store = swofty_core::copies::store_name(Source::detect(&dir))
                    ))
                }),
            },
            window,
            cx,
        );
    }

    pub(crate) fn prompt(&mut self, prompt: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(prompt.placeholder)
                .default_value(prompt.initial.clone())
        });
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        let focus = std::cell::Cell::new(true);
        window.open_alert_dialog(cx, move |alert, window, cx| {
            if focus.replace(false) {
                input.update(cx, |s, cx| s.focus(window, cx));
            }
            let field = input.clone();
            let browse = prompt.browse.clone().map(|options| {
                Button::new("browse")
                    .icon(IconName::FolderOpen)
                    .label(tr("Browse…"))
                    .on_click(move |_, window, cx| {
                        let rx = cx.prompt_for_paths(options.clone());
                        let field = field.clone();
                        window
                            .spawn(cx, async move |cx| {
                                if let Ok(Ok(Some(paths))) = rx.await
                                    && let Some(path) = paths.first()
                                {
                                    let path = path.display().to_string();
                                    cx.update(|window, cx| {
                                        field.update(cx, |s, cx| s.set_value(path, window, cx))
                                    })
                                    .ok();
                                }
                            })
                            .detach();
                    })
            });
            let (input, shell, store, action, title) = (
                input.clone(),
                shell.clone(),
                store.clone(),
                prompt.action.clone(),
                prompt.title,
            );
            alert
                .width(px(560.))
                .title(prompt.title)
                .description(prompt.description.clone())
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(div().flex_1().child(Input::new(&input)))
                        .children(browse),
                )
                .ok_text(prompt.ok)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let value = input.read(cx).value().trim().to_owned();
                    match action(&store, &value) {
                        Ok(message) => {
                            shell
                                .update(cx, |s, cx| {
                                    s.rediscover = true;
                                    s.reload(cx)
                                })
                                .ok();
                            window.notify(Notification::success(message), cx);
                            true
                        }
                        Err(e) => {
                            crate::app_log::action(title)
                                .file(std::path::Path::new(&value))
                                .failed(&e);
                            window.notify(Notification::error(e), cx);
                            false
                        }
                    }
                })
        });
    }
}
