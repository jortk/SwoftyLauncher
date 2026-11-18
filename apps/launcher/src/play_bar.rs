use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, Side, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf, trn};

use crate::format::{ago, duration};
use crate::game::{self, Owner};

pub(crate) const HEIGHT: f32 = 72.;
use crate::shell::{Page, PlayState, Shell};
use crate::theme::Palette;
use swofty_core::install::Source;
use swofty_core::session::LaunchMode;
use swofty_core::updater::Gate;

pub(crate) fn store_badge(source: Source, p: Palette) -> Div {
    let (text, color) = match source {
        Source::Steam => ("STEAM", p.accent),
        Source::EaApp => ("EA APP", p.warning),
        Source::Unknown => (tr("FOLDER"), p.text_muted),
    };
    div()
        .flex_shrink_0()
        .px(px(5.))
        .rounded(px(4.))
        .bg(color.opacity(0.14))
        .text_size(px(10.))
        .line_height(px(16.))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(text)
}

pub(crate) fn link(id: &'static str, label: &'static str, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(p.text_muted)
        .underline()
        .hover(move |s| s.text_color(p.text))
        .child(label)
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn status_row() -> Div {
    h_flex()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .gap(px(9.))
        .text_size(px(13.))
}

fn launch_failed(row: Div, text: impl std::fmt::Display, p: Palette) -> Div {
    let full = text.to_string();
    let text = SharedString::from(trf!("Launch failed: {cause}", cause = game::cause_first(&full)));
    let tip = SharedString::from(trf!("Launch failed: {cause}", cause = full));
    row.text_color(p.error)
        .child(Icon::new(IconName::CircleAlert).size(px(16.)))
        .child(
            div()
                .id("launch-failed")
                .min_w_0()
                .truncate()
                .child(text)
                .tooltip(move |window, cx| {
                    let tip = tip.clone();
                    Tooltip::element(move |_, _| div().max_w(px(480.)).child(tip.clone())).build(window, cx)
                }),
        )
}

impl Shell {
    fn game_info(&self, p: Palette, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let width = if wide { 320. } else { 240. };
        let text = |title: String, line: AnyElement| {
            v_flex()
                .flex_1()
                .min_w_0()
                .items_start()
                .gap(px(1.))
                .child(
                    div()
                        .w_full()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .truncate()
                        .child(title),
                )
                .child(line)
        };
        let muted = |s: String| {
            div()
                .min_w_0()
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .truncate()
                .child(s)
                .into_any_element()
        };
        let Some(i) = &self.install else {
            let (title, line) = if self.detected {
                (tr("F1 25 not found"), tr("Not in Steam or the EA app"))
            } else {
                ("F1 25", tr("Looking for the game…"))
            };
            return v_flex()
                .w(px(width))
                .flex_shrink_1()
                .min_w(px(170.))
                .child(text(title.into(), muted(line.into())))
                .into_any_element();
        };
        let title = match i.version() {
            Some(v) => format!("F1 25 · {v}"),
            None => "F1 25".into(),
        };
        let mode = match i.source {
            Source::EaApp if !cfg!(windows) => "Proton".to_owned(),
            _ => i.mode.clone(),
        };
        let line = h_flex()
            .w_full()
            .min_w_0()
            .gap(px(6.))
            .child(store_badge(i.source, p))
            .child(muted(mode))
            .into_any_element();
        let shell = cx.entity().downgrade();
        let copies = self.copies.clone();
        let missing = self.missing_copies.clone();
        let active = i.dir.clone();
        Button::new("game-switcher")
            .ghost()
            .h(px(48.))
            .w(px(width))
            .flex_shrink_1()
            .min_w(px(170.))
            .px(px(8.))
            .ml(px(-8.))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(8.))
                    .child(text(title, line))
                    .child(
                        Icon::new(IconName::ChevronsUpDown)
                            .size(px(14.))
                            .text_color(p.text_muted)
                            .flex_shrink_0(),
                    ),
            )
            .tooltip(tr("Switch the copy of F1 25 Play uses"))
            .disabled(matches!(
                self.play_state(),
                PlayState::Running
                    | PlayState::Preparing
                    | PlayState::Launching
                    | PlayState::Restoring
                    | PlayState::Building
            ))
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                menu = menu
                    .min_w(px(360.))
                    .max_w(px(560.))
                    .check_side(Side::Right)
                    .label(tr("Play this copy"));
                for c in &copies {
                    let (shell, dir) = (shell.clone(), c.dir.clone());
                    let (source, version) = (
                        c.source,
                        c.version().unwrap_or_else(|| tr("version unknown").into()),
                    );
                    let folder = c.dir.display().to_string();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| {
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .gap(px(3.))
                                .py(px(3.))
                                .child(
                                    h_flex()
                                        .gap(px(8.))
                                        .child(store_badge(source, p))
                                        .child(div().text_size(px(13.)).child(version.clone())),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.text_muted)
                                        .truncate()
                                        .child(folder.clone()),
                                )
                        })
                        .checked(c.dir == active)
                        .on_click(move |_, window, cx| {
                            shell.update(cx, |s, cx| s.use_copy(dir.clone(), window, cx)).ok();
                        }),
                    );
                }
                for dir in &missing {
                    let folder = dir.display().to_string();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| {
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .gap(px(3.))
                                .py(px(3.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(p.text_muted)
                                        .child(tr("Folder not found")),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.text_placeholder)
                                        .truncate()
                                        .child(folder.clone()),
                                )
                        })
                        .disabled(true),
                    );
                }
                let shell = shell.clone();
                menu.separator().item(
                    PopupMenuItem::new(tr("Add another folder…"))
                        .icon(IconName::FolderPlus)
                        .on_click(move |_, window, cx| {
                            shell.update(cx, |s, cx| s.locate_game(window, cx)).ok();
                        }),
                )
            })
            .into_any_element()
    }

    fn status(&self, state: &PlayState, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let dot = |c: Hsla| div().size(px(7.)).rounded_full().bg(c).flex_shrink_0();
        let row = status_row();
        if self.game_edited.is_some() {
            return row
                .text_color(p.warning)
                .child(Icon::new(IconName::TriangleAlert).size(px(16.)))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(tr("Your game files were edited")),
                )
                .child(
                    link("game-edited-what", tr("What to do"), p)
                        .on_click(cx.listener(|this, _, window, cx| this.show_game_edited(window, cx))),
                )
                .when_some(self.launch_log(), |d, log| {
                    d.child(
                        link("launch-log", tr("View log"), p)
                            .on_click(move |_, _, cx| cx.open_with_system(&log)),
                    )
                });
        }
        if let Some(notice) = &self.notice {
            return launch_failed(row, notice, p).when_some(self.launch_log(), |d, log| {
                d.child(
                    link("launch-log", tr("View log"), p).on_click(move |_, _, cx| cx.open_with_system(&log)),
                )
            });
        }
        if let Some(row) = self.update_status(self.update_gate(state), status_row(), p, cx) {
            return row;
        }
        if let Some(row) = self.addon_update_status(state, status_row(), p, cx) {
            return row;
        }
        let rebuild = !self.library.profile.is_vanilla() && !self.library.rebuild.is_empty();
        match state {
            PlayState::Running => row,
            PlayState::Elsewhere(owner) => {
                let text = match owner {
                    Owner::Other(i) => match i.source {
                        Source::Steam => tr("F1 25 from Steam is running"),
                        Source::EaApp => tr("F1 25 from the EA app is running"),
                        Source::Unknown => tr("F1 25 from another folder is running"),
                    },
                    _ => tr(
                        "F1 25 is running from a folder the launcher doesn't know · close it to play this copy",
                    ),
                };
                let row = row
                    .text_color(p.text_muted)
                    .child(dot(p.warning))
                    .child(div().min_w_0().truncate().child(text));
                match owner {
                    Owner::Other(i) => {
                        let dir = i.dir.clone();
                        row.child(link("switch-to-running", tr("Switch to it"), p).on_click(
                            cx.listener(move |this, _, window, cx| this.use_copy(dir.clone(), window, cx)),
                        ))
                    }
                    _ => row,
                }
            }
            PlayState::Restoring => row.child(dot(p.warning)).child(tr("Restoring the game files…")),
            PlayState::Building => {
                let Some(b) = &self.build else {
                    return row;
                };
                let text = match self.cancelling {
                    true => tr("Cancelling the build…").to_owned(),
                    false => format!(
                        "{} for {} · {}",
                        capitalised(&b.progress()),
                        b.profile,
                        duration((b.minutes() * 60).max(1))
                    ),
                };
                row.child(dot(p.warning))
                    .child(div().min_w_0().truncate().child(text))
                    .when_some(self.wrapper_log(), |d, log| {
                        d.child(
                            link("launch-log", tr("Log"), p)
                                .on_click(move |_, _, cx| cx.open_with_system(&log)),
                        )
                    })
            }
            PlayState::Launching => row
                .child(dot(p.warning))
                .child(format!(
                    "{} {}",
                    match self.launch.as_ref().and_then(|l| l.stage.as_deref()) {
                        Some(stage) => stage.to_owned(),
                        None if !self.use_profiles && self.launch_mode().is_vr() => {
                            tr("Starting in VR…").to_owned()
                        }
                        None if !self.use_profiles => tr("Starting…").to_owned(),
                        None if self.launch_mode().is_vr() => trf!(
                            "Starting with profile {name} in VR…",
                            name = self.library.profile.name
                        ),
                        None => trf!("Starting with profile {name}…", name = self.library.profile.name),
                    },
                    self.launch
                        .as_ref()
                        .map(|l| duration(l.since.elapsed().as_secs()))
                        .unwrap_or_default()
                ))
                .when_some(self.launch_log(), |d, log| {
                    d.child(
                        link("launch-log", tr("Log"), p).on_click(move |_, _, cx| cx.open_with_system(&log)),
                    )
                }),
            PlayState::Crashed(session) => {
                let dir = session.dir.clone();
                let ending = self
                    .crash
                    .as_ref()
                    .filter(|(d, _)| *d == session.dir)
                    .map(|(_, e)| e);
                let open =
                    link("crash-log", tr("Open logs"), p).on_click(move |_, _, cx| cx.open_with_system(&dir));
                let dismiss = link("dismiss", tr("Dismiss"), p).on_click(cx.listener(|this, _, _, cx| {
                    this.dismissed = this.session.as_ref().map(|s| s.dir.clone());
                    cx.notify();
                }));
                if let Some(Err(failure)) = ending {
                    return launch_failed(row, failure, p).child(open).child(dismiss);
                }
                let view = self.crash_view.as_ref().filter(|v| v.dir == session.dir);
                let details = view.map(|_| {
                    link("crash-details", tr("What happened"), p).on_click(cx.listener(|this, _, _, cx| {
                        this.crash_open = true;
                        cx.notify();
                    }))
                });
                let summary = view
                    .map(|v| v.headline())
                    .or_else(|| ending.and_then(|e| e.as_ref().ok()).cloned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| tr("reading the log…").into());
                let crashed = trf!(
                    "Crashed {when}",
                    when = session
                        .ended
                        .map(|e| ago(game::now_secs() - e))
                        .unwrap_or_default()
                );
                row.items_start()
                    .child(div().mt(px(6.)).child(dot(p.error)))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                h_flex()
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .text_color(p.error)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(crashed),
                                    )
                                    .children(details)
                                    .child(open)
                                    .child(dismiss),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .truncate()
                                    .child(summary),
                            ),
                    )
            }
            PlayState::NotInstalled => row.text_color(p.text_muted).child(dot(p.text_placeholder)).child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(tr("Install it with Steam or the EA app, or locate its folder")),
            ),
            PlayState::Detecting => row.text_color(p.text_muted).child(tr("Detecting F1 25…")),
            PlayState::Checking => row.text_color(p.text_muted).child(tr("Checking conflicts…")),
            PlayState::Preparing => row.child(dot(p.warning)).child(tr("Checking addons…")),
            PlayState::Ready => {
                if let Some(n) = self
                    .install
                    .as_ref()
                    .and_then(|i| crate::unsupported_build::notice(&i.dir.join(swofty_core::GAME_EXE)))
                {
                    return row
                        .items_start()
                        .child(div().mt(px(6.)).child(dot(p.warning)))
                        .child(
                            v_flex()
                                .min_w_0()
                                .gap(px(2.))
                                .child(div().min_w_0().truncate().child(n.headline))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.text_muted)
                                        .truncate()
                                        .child(n.detail),
                                ),
                        );
                }
                if rebuild {
                    let changes = trn(
                        self.library.rebuild.len(),
                        "{count} changed setting",
                        "{count} changed settings",
                    );
                    let text = match self.use_profiles {
                        true => trf!(
                            "Ready · rebuilds {profile} for {changes} on launch",
                            profile = self.library.profile.name,
                            changes = changes
                        ),
                        false => trf!("Ready · rebuilds for {changes} on launch", changes = changes),
                    };
                    return row
                        .text_color(p.text_muted)
                        .child(dot(p.warning))
                        .child(div().min_w_0().truncate().child(text));
                }
                if let Some(refused) = self.saves_refused() {
                    let dir = refused.dir.clone();
                    return row
                        .child(dot(p.warning))
                        .child(div().min_w_0().truncate().child(tr(
                            "Launcher saves could not be used, so F1 25 closed before loading a save",
                        )))
                        .child(
                            link("steam-once", tr("Play once with your Steam saves"), p).on_click(
                                cx.listener(|this, _, window, cx| this.play_with_steam_saves(window, cx)),
                            ),
                        )
                        .child(
                            link("saves-log", tr("Open logs"), p)
                                .on_click(move |_, _, cx| cx.open_with_system(&dir)),
                        )
                        .child(link("saves-dismiss", tr("Dismiss"), p).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.dismissed = this.session.as_ref().map(|s| s.dir.clone());
                                cx.notify();
                            },
                        )));
                }
                if let Some((names, source, snapshot, at)) = self.saves.banner.as_ref().and_then(|b| b.lost())
                {
                    let names = names.join("; ");
                    let (source, snapshot) = (source.to_owned(), snapshot.to_owned());
                    return row
                        .child(dot(p.warning))
                        .child(div().min_w_0().truncate().child(trf!(
                            "Addon careers may have been removed from your Steam save: {names}. Last version: {at}",
                            names = names,
                            at = swofty_core::vault::human(at)
                        )))
                        .child(link("career-restore", tr("Restore"), p).on_click(cx.listener(
                            move |this, _, window, cx| this.restore_version(&source, &snapshot, window, cx),
                        )))
                        .child(link("career-open", tr("Open Career saves"), p).on_click(
                            cx.listener(|this, _, _, cx| this.open_saves(None, cx)),
                        ));
                }
                if self.saves_await_import() {
                    return row.text_color(p.text_muted).child(dot(p.warning)).child(
                        div()
                            .min_w_0()
                            .truncate()
                            .child(crate::career_saves::empty_save_short()),
                    );
                }
                row
            }
        }
    }

    fn save_area(&self, state: &PlayState, p: Palette) -> Option<AnyElement> {
        if !matches!(
            state,
            PlayState::Ready | PlayState::Crashed(_) | PlayState::Preparing
        ) {
            return None;
        }
        let install = self.install.as_ref()?;
        let blocked = swofty_core::build_support::blocked(&install.dir.join(swofty_core::GAME_EXE));
        let line = crate::save_area::line(
            &self.library.profile,
            install.source,
            blocked.as_deref(),
            self.steam_saves_once,
        );
        let colour = if line.warn { p.warning } else { p.text_muted };
        Some(
            v_flex()
                .id("save-area")
                .flex_shrink_0()
                .max_w(px(330.))
                .min_w_0()
                .gap(px(1.))
                .text_size(px(12.))
                .text_color(colour)
                .child(div().min_w_0().truncate().child(line.headline))
                .when_some(line.reason, |d, reason| {
                    d.child(div().min_w_0().truncate().text_color(p.text_muted).child(reason))
                })
                .into_any_element(),
        )
    }

    fn saves_await_import(&self) -> bool {
        self.saves
            .view
            .as_ref()
            .is_some_and(|v| v.awaiting_import_for(&self.library.profile.name))
    }

    fn wrapper_log(&self) -> Option<std::path::PathBuf> {
        let log = self.store.root.join("state/wrapper-run.log");
        log.is_file().then_some(log)
    }

    fn launch_log(&self) -> Option<std::path::PathBuf> {
        let log = self.store.root.join("state/launcher-run.log");
        log.is_file().then_some(log)
    }

    fn play_caret(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().downgrade();
        let vr = self.launch_mode().is_vr();
        let start_store = self.start_store;
        let start_label = match self.install.as_ref().map(|i| i.source) {
            Some(Source::EaApp) => tr("Start the EA app automatically"),
            _ => tr("Start Steam automatically"),
        };
        Button::new("play-menu")
            .h(px(52.))
            .w(px(34.))
            .rounded_l(px(0.))
            .rounded_r(px(9.))
            .primary()
            .child(Icon::new(IconName::ChevronDown).size(px(16.)))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, cx| {
                let dev_mode = shell
                    .upgrade()
                    .is_some_and(|s| s.read(cx).store.config().developer);
                let mode = |label: &'static str, icon: IconName, mode: LaunchMode| {
                    let shell = shell.clone();
                    PopupMenuItem::new(label)
                        .icon(icon)
                        .checked(vr == mode.is_vr())
                        .on_click(move |_, window, cx| {
                            shell
                                .update(cx, |s, cx| {
                                    s.set_launch_mode(mode, window, cx);
                                    s.play(window, cx);
                                })
                                .ok();
                        })
                };
                let (console, steam, developer) = (shell.clone(), shell.clone(), shell.clone());
                let (saves, once) = (shell.clone(), shell.clone());
                let profile = shell.upgrade().map(|s| {
                    let p = &s.read(cx).library.profile;
                    (p.is_vanilla(), p.launcher_saves)
                });
                let (vanilla, saves_on) = profile.unwrap_or((true, false));
                menu.min_w(px(220.))
                    .check_side(Side::Right)
                    .item(mode(tr("Play"), IconName::Play, LaunchMode::Normal))
                    .item(mode(tr("Play in VR"), IconName::RectangleGoggles, LaunchMode::Vr))
                    .separator()
                    .item(
                        PopupMenuItem::new(tr("Play with console"))
                            .icon(IconName::SquareTerminal)
                            .on_click(move |_, window, cx| {
                                console
                                    .update(cx, |s, cx| {
                                        s.set_console(true, window, cx);
                                        s.play(window, cx);
                                    })
                                    .ok();
                            }),
                    )
                    .separator()
                    .when(!vanilla, |menu| {
                        menu.item(
                            PopupMenuItem::new(tr("Use launcher saves"))
                                .icon(IconName::Save)
                                .checked(saves_on)
                                .on_click(move |_, window, cx| {
                                    saves
                                        .update(cx, |s, cx| s.set_launcher_saves(!saves_on, window, cx))
                                        .ok();
                                }),
                        )
                        .when(saves_on, |menu| {
                            menu.item(
                                PopupMenuItem::new(tr("Play once with your Steam saves"))
                                    .icon(IconName::Play)
                                    .on_click(move |_, window, cx| {
                                        once.update(cx, |s, cx| s.play_with_steam_saves(window, cx)).ok();
                                    }),
                            )
                        })
                    })
                    .separator()
                    .item(
                        PopupMenuItem::new(start_label)
                            .icon(IconName::CirclePower)
                            .checked(start_store)
                            .on_click(move |_, window, cx| {
                                steam
                                    .update(cx, |s, cx| s.set_start_store(!start_store, window, cx))
                                    .ok();
                            }),
                    )
                    .item(
                        PopupMenuItem::new(tr("Developer mode"))
                            .icon(IconName::SquareTerminal)
                            .checked(dev_mode)
                            .on_click(move |_, window, cx| {
                                developer
                                    .update(cx, |s, cx| {
                                        s.set_developer(!dev_mode, window, cx);
                                        if !dev_mode {
                                            s.go(Page::Tools, cx);
                                        }
                                    })
                                    .ok();
                            }),
                    )
            })
    }

    fn big_button(&self, state: &PlayState, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let gate = self.update_gate(state);
        if gate != Gate::Play {
            return self.update_button(gate, p, cx);
        }
        if self.addon_update_takes_play(state) {
            return self.addon_update_button(p, cx);
        }
        let vr = self.launch_mode().is_vr();
        let starting = self.mp_starting_label();
        let (icon, label, bg, fg, enabled) = match state {
            PlayState::Ready | PlayState::Crashed(_) if starting.is_some() => (
                IconName::LoaderCircle,
                starting.unwrap_or_default(),
                p.elevated,
                p.accent,
                false,
            ),
            PlayState::Ready | PlayState::Crashed(_) if vr => (
                IconName::RectangleGoggles,
                tr("Play in VR"),
                p.accent,
                p.on_accent,
                true,
            ),
            PlayState::Ready | PlayState::Crashed(_) => {
                (IconName::Play, tr("Play"), p.accent, p.on_accent, true)
            }
            PlayState::Preparing | PlayState::Launching => (
                IconName::LoaderCircle,
                tr("Launching…"),
                p.elevated,
                p.accent,
                false,
            ),
            PlayState::Restoring => (
                IconName::LoaderCircle,
                tr("Restoring…"),
                p.elevated,
                p.accent,
                false,
            ),
            PlayState::Building if self.cancelling => (
                IconName::LoaderCircle,
                tr("Cancelling…"),
                p.elevated,
                p.accent,
                false,
            ),
            PlayState::Building => (
                IconName::LoaderCircle,
                tr("Building…"),
                p.elevated,
                p.accent,
                true,
            ),
            PlayState::Running => (IconName::Square, tr("Stop"), p.elevated, p.error, true),
            PlayState::NotInstalled => (
                IconName::FolderSearch,
                tr("Locate game"),
                p.elevated,
                p.text,
                true,
            ),
            PlayState::Detecting | PlayState::Checking => (
                IconName::LoaderCircle,
                tr("Play"),
                p.elevated,
                p.text_muted,
                false,
            ),
            PlayState::Elsewhere(_) => (IconName::Play, tr("Play"), p.elevated, p.text_muted, false),
        };
        let ready = matches!(state, PlayState::Ready | PlayState::Crashed(_)) && starting.is_none();
        let missing = matches!(state, PlayState::NotInstalled);
        let running = matches!(state, PlayState::Running);
        let building = matches!(state, PlayState::Building);
        let button = div()
            .id("play")
            .h(px(52.))
            .w(px(170.))
            .flex_shrink_0()
            .when(ready, |d| d.rounded_l(px(9.)))
            .when(!ready, |d| d.rounded(px(9.)))
            .bg(bg)
            .text_color(fg)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .when(!ready, |d| d.border_1().border_color(p.border))
            .when(ready, |d| d.hover(move |s| s.bg(p.accent_hover)))
            .when(missing, |d| d.hover(move |s| s.bg(p.selected)))
            .when(enabled, |d| d.cursor_pointer())
            .when(building && enabled, |d| d.hover(move |s| s.bg(p.selected)))
            .map(|d| match icon {
                IconName::LoaderCircle => d.child(Spinner::new().icon(icon).with_size(px(18.))),
                _ => d.child(Icon::new(icon).size(px(18.))),
            })
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label),
            )
            .when(enabled, |d| {
                d.on_click(cx.listener(move |this, _, window, cx| {
                    if running {
                        this.confirm_stop(window, cx);
                    } else if building {
                        this.cancel_build(window, cx);
                    } else if missing {
                        this.locate_game(window, cx);
                    } else {
                        this.play(window, cx);
                    }
                }))
            });
        h_flex()
            .flex_shrink_0()
            .child(button)
            .when(ready, |d| d.child(self.play_caret(cx)))
            .into_any_element()
    }

    /// "Play <host>'s Lobby" while a room is open on the Multiplayer page. `primary` gives it the Play button's
    /// filled look, for when it stands in for Play.
    fn room_button(
        &self,
        state: &PlayState,
        primary: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let ready = matches!(
            state,
            PlayState::Ready | PlayState::Running | PlayState::Crashed(_)
        );
        if !ready || !self.mp_can_play_room() {
            return None;
        }
        let host = self.mp_viewed_room()?.host.name.clone();
        Some(
            div()
                .id("play-room")
                .h(px(52.))
                .px(px(18.))
                .flex_shrink_0()
                .rounded(px(9.))
                .when(primary, |d| {
                    d.min_w(px(170.))
                        .bg(p.accent)
                        .text_color(p.on_accent)
                        .hover(move |s| s.bg(p.accent_hover))
                })
                .when(!primary, |d| {
                    d.border_1()
                        .border_color(p.accent)
                        .text_color(p.accent)
                        .hover(move |s| s.bg(p.selected))
                })
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .cursor_pointer()
                .child(Icon::new(IconName::Users).size(px(18.)))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(trf!("Play {host}'s Lobby", host = host)),
                )
                .on_click(cx.listener(|this, _, window, cx| this.mp_view_play(window, cx)))
                .into_any_element(),
        )
    }

    fn footer_links(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let tip = crate::send_logs::label();
        h_flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(18.))
            .child(
                h_flex()
                    .id("send-logs")
                    .gap(px(6.))
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .hover(move |s| s.text_color(p.text))
                    .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                    .child(Icon::new(crate::send_logs::icon()).size(px(13.)))
                    .child(crate::send_logs::short_label())
                    .on_click(cx.listener(|this, _, window, cx| this.send_logs(window, cx))),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
            )
    }

    pub(crate) fn play_bar(&self, p: Palette, wide: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.play_state();
        // With a room open on the Multiplayer page and the game ready, the room's button replaces Play.
        let ready = matches!(state, PlayState::Ready | PlayState::Crashed(_));
        let room = self.room_button(&state, ready, p, cx);
        let lobby_only = ready && room.is_some();
        h_flex()
            .h(px(HEIGHT))
            .flex_shrink_0()
            .bg(p.chrome)
            .border_t_1()
            .border_color(p.border_variant)
            .child(crate::pages::game_art(56., 56.).m(px(8.)))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .pl(px(16.))
                    .pr(px(16.))
                    .gap(px(20.))
                    .child(self.game_info(p, wide, cx))
                    .child(self.status(&state, p, cx))
                    .children(self.save_area(&state, p))
                    .children(room)
                    .child(self.footer_links(p, cx))
                    .when(!lobby_only, |d| d.child(self.big_button(&state, p, cx))),
            )
    }
}
