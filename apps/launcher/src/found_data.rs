//! Offering addons and saves the launcher found in another folder: when the
//! store starts with none of the player's addons, and on demand from Settings.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::notices::{Notification, Notify};
use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::builtin::CORE_ID;
use swofty_core::data_move::find::{self, Found};
use swofty_core::data_move::{self, Location};
use swofty_core::managed;
use swofty_core::session::Config;
use swofty_i18n::{tr, trf, trn};

use crate::data_move::install_folder;
use crate::format::{ago_since, size};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::Overlay;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Asking,
    Switching,
}

pub(crate) struct FoundPrompt {
    found: Vec<Found>,
    picked: usize,
    stage: Stage,
}

pub(crate) fn scan_due(user_addons: usize, config: &Config) -> bool {
    user_addons == 0 && !config.no_find_prompt
}

fn epoch_secs(at: SystemTime) -> f64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0., |d| d.as_secs_f64())
}

impl Shell {
    fn user_addon_count(&self) -> usize {
        self.store
            .addons()
            .map(|all| {
                all.keys()
                    .filter(|id| !self.store.is_builtin(id) && !managed::is_managed(id))
                    .count()
            })
            .unwrap_or(0)
    }

    fn bundled_ids(&self) -> Vec<String> {
        let mut ids = vec![CORE_ID.to_owned(), managed::ID.to_owned()];
        ids.extend(
            self.store
                .builtins()
                .iter()
                .map(|b| b.addon.manifest().id.clone()),
        );
        if let Some(shipped) = install_folder().map(|root| root.join("addons"))
            && !swofty_core::process::same_folder(&shipped, &self.store.addons_dir())
            && let Ok(read) = std::fs::read_dir(shipped)
        {
            ids.extend(
                read.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned()),
            );
        }
        ids
    }

    pub(crate) fn scan_for_lost_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.moving.is_some() || !scan_due(self.user_addon_count(), &self.store.config()) {
            return;
        }
        self.scan_for_data(false, window, cx);
    }

    pub(crate) fn scan_for_data(&mut self, announce: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.moving.is_some()
            || self
                .found_data
                .as_ref()
                .is_some_and(|p| p.stage == Stage::Switching)
        {
            return;
        }
        let games: Vec<PathBuf> = self
            .copies
            .iter()
            .map(|c| c.dir.clone())
            .chain(self.install.as_ref().map(|i| i.dir.clone()))
            .collect();
        let location = Location::find(install_folder(), games);
        let installs: Vec<PathBuf> = install_folder().into_iter().collect();
        let bundled = self.bundled_ids();
        cx.spawn_in(window, async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { find::candidates(&location, &installs, &bundled) })
                .await;
            this.update_in(cx, |s, window, cx| {
                crate::app_log::note(&format!("find addons: {} candidate(s)", found.len()));
                if found.is_empty() {
                    if announce {
                        window.notify(Notification::info(tr("Nothing found")), cx);
                    }
                    return;
                }
                s.found_data = Some(FoundPrompt {
                    found,
                    picked: 0,
                    stage: Stage::Asking,
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn use_found_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.found_data.as_mut().filter(|p| p.stage == Stage::Asking) else {
            return;
        };
        let Some(chosen) = prompt.found.get(prompt.picked).map(|f| f.path.clone()) else {
            return;
        };
        if let Some(why) = self.move_refusal() {
            window.notify(Notification::error(why), cx);
            return;
        }
        if let Some(prompt) = self.found_data.as_mut() {
            prompt.stage = Stage::Switching;
        }
        let games: Vec<PathBuf> = self
            .copies
            .iter()
            .map(|c| c.dir.clone())
            .chain(self.install.as_ref().map(|i| i.dir.clone()))
            .collect();
        let location = Location::find(install_folder(), games);
        let installs: Vec<PathBuf> = install_folder().into_iter().collect();
        let bundled = self.bundled_ids();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let target = chosen.clone();
            let adopted = cx
                .background_executor()
                .spawn(async move { data_move::adopt(&location, &target, &installs, &bundled) })
                .await;
            this.update_in(cx, |s, window, cx| match adopted {
                Ok(done) => {
                    crate::app_log::note(&format!("find addons: using {}", done.found.display()));
                    s.relaunch_for_move(window, cx);
                }
                Err(e) => {
                    s.found_data = None;
                    cx.notify();
                    crate::app_log::note(&format!("find addons: {e}"));
                    s.data_move_failed(&e.to_string(), window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn dismiss_found_data(&mut self, forever: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.found_data = None;
        if forever {
            let mut config = self.store.config();
            config.no_find_prompt = true;
            self.save_config_toast(&config, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn found_data_overlay(&self, p: Palette, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let prompt = self.found_data.as_ref()?;
        let body = match prompt.stage {
            Stage::Switching => v_flex()
                .items_center()
                .gap(px(16.))
                .child(Spinner::new().with_size(px(28.)))
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.text)
                        .child(tr("Restarting to use your addons and saves")),
                )
                .into_any_element(),
            Stage::Asking => self.found_data_card(prompt, p, cx).into_any_element(),
        };
        Some(
            Overlay::new("found-data-modal")
                .opaque(p.background)
                .below_title_bar()
                .scrolls()
                .child(
                    v_flex()
                        .w(px(668.))
                        .max_w_full()
                        .px(px(24.))
                        .py(px(32.))
                        .items_center()
                        .child(body),
                ),
        )
    }

    fn found_data_card(&self, prompt: &FoundPrompt, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let many = prompt.found.len() > 1;
        let rows = prompt.found.iter().enumerate().map(|(i, f)| {
            let picked = i == prompt.picked;
            let count = trn(f.addons, "{count} addon", "{count} addons");
            let total = if f.bytes_partial {
                trf!("at least {size}", size = size(f.bytes))
            } else {
                size(f.bytes)
            };
            let used = trf!("last used {ago}", ago = ago_since(epoch_secs(f.newest)));
            let names = f.names.join(", ");
            h_flex()
                .id(("found-data", i))
                .w_full()
                .gap(px(12.))
                .items_start()
                .p(px(14.))
                .rounded(px(10.))
                .bg(p.surface)
                .border_1()
                .border_color(if picked { p.accent } else { p.border_variant })
                .when(many, |row| {
                    row.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(prompt) = this.found_data.as_mut() {
                            prompt.picked = i;
                        }
                        cx.notify();
                    }))
                })
                .child(
                    Icon::new(IconName::FolderOpen)
                        .size(px(20.))
                        .text_color(if picked { p.accent } else { p.text_muted }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.text)
                                .child(f.path.display().to_string()),
                        )
                        .child(div().text_size(px(12.)).text_color(p.text_muted).child(trf!(
                            "{count}, {total}, {used}",
                            count = count,
                            total = total,
                            used = used
                        )))
                        .when(!names.is_empty(), |c| {
                            c.child(div().text_size(px(12.)).text_color(p.text_muted).child(names))
                        }),
                )
        });
        v_flex()
            .w_full()
            .max_w(px(620.))
            .gap(px(18.))
            .items_center()
            .child(
                v_flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_center()
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.text)
                            .child(tr("We found your addons and saves")),
                    )
                    .child(
                        div()
                            .text_center()
                            .text_size(px(14.))
                            .line_height(px(21.))
                            .text_color(p.text_muted)
                            .child(tr(
                                "This launcher has none of your addons yet. Swofty Launcher restarts to use the ones below.",
                            )),
                    ),
            )
            .child(v_flex().w_full().gap(px(10.)).children(rows))
            .child(
                h_flex()
                    .gap(px(10.))
                    .justify_center()
                    .flex_wrap()
                    .child(
                        Button::new("found-use")
                            .primary()
                            .h(px(38.))
                            .px(px(18.))
                            .label(tr("Use them"))
                            .on_click(cx.listener(|this, _, window, cx| this.use_found_data(window, cx))),
                    )
                    .child(
                        Button::new("found-later")
                            .outline()
                            .h(px(38.))
                            .label(tr("Not now"))
                            .on_click(cx.listener(|this, _, window, cx| this.dismiss_found_data(false, window, cx))),
                    )
                    .child(
                        Button::new("found-never")
                            .ghost()
                            .h(px(38.))
                            .label(tr("Don't ask again"))
                            .on_click(cx.listener(|this, _, window, cx| this.dismiss_found_data(true, window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::scan_due;
    use swofty_core::session::Config;

    #[test]
    fn scan_runs_only_when_there_are_no_user_addons() {
        let config = Config::default();
        assert!(scan_due(0, &config));
        assert!(!scan_due(1, &config));
        assert!(!scan_due(14, &config));
    }

    #[test]
    fn dont_ask_again_stops_the_scan_and_is_saved() {
        let config = Config {
            no_find_prompt: true,
            ..Config::default()
        };
        assert!(!scan_due(0, &config));
        let text = serde_json::to_string(&config).unwrap();
        assert!(text.contains("no_find_prompt"));
        let back: Config = serde_json::from_str(&text).unwrap();
        assert!(!scan_due(0, &back));
        assert!(
            !serde_json::to_string(&Config::default())
                .unwrap()
                .contains("no_find_prompt")
        );
    }
}
