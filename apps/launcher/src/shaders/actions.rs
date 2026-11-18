use std::path::PathBuf;

use crate::scale::px;
use gpui_kit::component::WindowExt;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use swofty_core::shaders::{self, ConfigRef};
use swofty_core::store::Store;
use swofty_i18n::{tr, trf};

use super::list::Rename;
use super::{Event, ShadersView};

pub(super) type Job = Box<dyn FnOnce(&Store) -> Result<Option<ConfigRef>, String> + Send>;

impl ShadersView {
    pub(super) fn run(&mut self, cx: &mut Context<Self>, job: Job) {
        self.busy = true;
        let store = self.store.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let out = cx.background_executor().spawn(async move { job(&store) }).await;
            this.update(cx, |v, cx| {
                v.busy = false;
                match out {
                    Ok(selected) => {
                        v.error = None;
                        if let Some(r) = selected {
                            if std::mem::take(&mut v.list.rename_new) {
                                v.list.pending_rename = Some(r.clone());
                            }
                            v.selected = Some(r);
                        }
                        v.refresh(cx);
                    }
                    Err(e) => {
                        v.list.rename_new = false;
                        v.error = Some(e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn use_config(&mut self, r: Option<ConfigRef>, cx: &mut Context<Self>) {
        if self.forced.is_some() || self.busy {
            return;
        }
        let name = self.info.profile.clone();
        let live = self.info.running.then(|| self.info.game_dir.clone()).flatten();
        self.busy = true;
        let store = self.store.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let out = cx
                .background_executor()
                .spawn(async move {
                    let mut profile = store.profile(&name).map_err(|e| e.to_string())?;
                    shaders::set_active(&store, &mut profile, r).map_err(|e| e.to_string())?;
                    if let Some(dir) = live {
                        shaders::push_live(&dir, &shaders::active_file(&store, &profile))
                            .map_err(|e| e.to_string())?;
                    }
                    Ok::<(), String>(())
                })
                .await;
            this.update(cx, |v, cx| {
                v.busy = false;
                v.error = out.err().map(|e| trf!("Couldn't change the shaders: {e}", e = e));
                v.refresh(cx);
                cx.emit(Event::ActiveChanged);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn new_config(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let names: Vec<String> = self.entries.iter().map(|e| e.name.clone()).collect();
        let name = unused_name(tr("New config"), &names);
        self.list.rename_new = true;
        self.run(
            cx,
            Box::new(move |store| {
                shaders::create(store, &name, None)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }),
        );
    }

    pub(super) fn copy_to_personal(&mut self, r: ConfigRef, _window: &mut Window, cx: &mut Context<Self>) {
        self.run(
            cx,
            Box::new(move |store| {
                shaders::copy_to_personal(store, &r)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }),
        );
    }

    pub(super) fn begin_rename(&mut self, r: ConfigRef, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.entry_of(&r).map(|e| e.name.clone()) else {
            self.list.pending_rename = Some(r);
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let events = cx.subscribe_in(
            &input,
            window,
            |this, input, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let value = input.read(cx).value().to_string();
                    this.commit_rename(value, window, cx);
                }
                _ => {}
            },
        );
        input.update(cx, |s, cx| s.focus(window, cx));
        self.list.rename = Some(Rename {
            target: r,
            input,
            _events: events,
        });
        cx.notify();
    }

    pub(super) fn start_pending_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.list.pending_rename.clone() else {
            return;
        };
        if self.entry_of(&r).is_some() {
            self.list.pending_rename = None;
            self.begin_rename(r, window, cx);
        }
    }

    fn commit_rename(&mut self, value: String, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.list.rename.take() else {
            return;
        };
        cx.notify();
        let value = value.trim().to_owned();
        let current = self.entry_of(&rename.target).map(|e| e.name.clone());
        if value.is_empty() || current.as_deref() == Some(value.as_str()) {
            return;
        }
        let old = rename.target;
        let was_active = self.active.as_ref() == Some(&old);
        let was_selected = self.selected.as_ref() == Some(&old);
        let profile = self.info.profile.clone();
        let live = self.info.running.then(|| self.info.game_dir.clone()).flatten();
        self.run(
            cx,
            Box::new(move |store| {
                let new = shaders::rename(store, &old, &value).map_err(|e| e.to_string())?;
                if was_active && new != old {
                    let mut p = store.profile(&profile).map_err(|e| e.to_string())?;
                    shaders::set_active(store, &mut p, Some(new.clone())).map_err(|e| e.to_string())?;
                    if let Some(dir) = live {
                        shaders::push_live(&dir, &shaders::active_file(store, &p))
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(was_selected.then_some(new))
            }),
        );
    }

    pub(super) fn export(&mut self, r: ConfigRef, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .entry_of(&r)
            .map_or_else(|| "shaders".to_owned(), |e| e.name.clone());
        let file = format!("{}.swfx", name.replace(['/', '\\', ':'], "-"));
        let start = dirs_start();
        let rx = cx.prompt_for_new_path(&start, Some(&file));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |v, cx| {
                    v.run(
                        cx,
                        Box::new(move |store| {
                            shaders::export(store, &r, &path)
                                .map(|()| None)
                                .map_err(|e| e.to_string())
                        }),
                    );
                })
                .ok();
            }
        })
        .detach();
    }

    pub(super) fn ask_delete(&mut self, r: ConfigRef, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.entry_of(&r).cloned() else {
            return;
        };
        let shaders = cx.entity().downgrade();
        let was_active = self.active.as_ref() == Some(&r);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shaders, r) = (shaders.clone(), r.clone());
            alert
                .width(px(460.))
                .title(trf!("Delete {name}?", name = entry.name))
                .description(if was_active {
                    tr("This config is on right now. The game plays without shaders after it is deleted.")
                } else {
                    tr("The config file is removed from your computer.")
                })
                .ok_text(tr("Delete"))
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let r = r.clone();
                    shaders.update(cx, |v, cx| v.delete(r, cx)).ok();
                    true
                })
        });
    }

    fn delete(&mut self, r: ConfigRef, cx: &mut Context<Self>) {
        let was_active = self.active.as_ref() == Some(&r);
        let was_selected = self.selected.as_ref() == Some(&r);
        let profile = self.info.profile.clone();
        let live = self.info.running.then(|| self.info.game_dir.clone()).flatten();
        if was_selected {
            self.selected = None;
        }
        self.run(
            cx,
            Box::new(move |store| {
                shaders::delete(store, &r).map_err(|e| e.to_string())?;
                if was_active {
                    let mut p = store.profile(&profile).map_err(|e| e.to_string())?;
                    shaders::set_active(store, &mut p, None).map_err(|e| e.to_string())?;
                    if let Some(dir) = live {
                        shaders::push_live(&dir, &shaders::active_file(store, &p))
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(None)
            }),
        );
        if was_active {
            cx.emit(Event::ActiveChanged);
        }
    }

    pub(super) fn import_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Import").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |v, cx| {
                    v.run(
                        cx,
                        Box::new(move |store| {
                            shaders::import(store, &path).map(Some).map_err(|e| e.to_string())
                        }),
                    );
                })
                .ok();
            }
        })
        .detach();
    }

    pub(super) fn import_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            self.error = Some(tr("The clipboard has no text to import.").into());
            cx.notify();
            return;
        };
        let catalog = self.catalog.clone();
        let names: Vec<String> = self.entries.iter().map(|e| e.name.clone()).collect();
        self.run(
            cx,
            Box::new(move |store| {
                let lint = swofty_shaders::lint(&text, &catalog);
                let name = lint
                    .config
                    .as_ref()
                    .map(|c| c.name.trim().to_owned())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| tr("Imported config").to_owned());
                let name = unused_name(&name, &names);
                shaders::create(store, &name, Some(&text))
                    .map(Some)
                    .map_err(|e| e.to_string())
            }),
        );
    }
}

fn unused_name(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|n| n == base) {
        return base.to_owned();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| base.to_owned())
}

fn dirs_start() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
