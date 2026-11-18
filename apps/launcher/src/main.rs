#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod addons;
mod admin_needed;
mod ads;
mod app_log;
mod avatar;
mod ban;
mod blocks;
mod browser;
mod career_saves;
mod chart_demo;
mod colour;
mod console;
mod crash_modal;
mod data_move;
mod db;
mod developer;
mod dialog_keys;
mod discord;
mod download_dock;
mod download_queue;
mod ea_app;
mod fallback_fonts;
mod format;
mod found_data;
mod game;
mod game_edited;
mod importer;
mod library;
mod multiplayer;
mod news;
mod notices;
mod onboarding;
mod online;
mod pages;
mod play_bar;
mod play_checks;
mod profile_panel;
mod profiles;
mod reveal;
mod save_area;
mod scale;
mod scroll;
mod sections;
mod send_logs;
mod settings;
mod setup;
mod shaders;
mod shell;
mod support;
mod tasks;
mod theme;
mod time_trial;
mod tools;
mod ui;
mod ui_mode;
mod unsupported_build;
mod update;

use gpui_kit::component::{TitleBar, v_flex};
use gpui_kit::*;

fn main() {
    if std::env::args().any(|a| a == "--chart-demo") {
        chart_demo::run();
        return;
    }
    swofty_core::data_move::recover_at_start();
    app_log::start();
    app_log::step("starting the app runtime");
    let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    let prepared = app.background_executor().spawn(async { shell::Prepared::load() });
    app_log::step("app runtime ready, entering the event loop");
    app.run(move |cx| {
        app_log::step("event loop running");
        fallback_fonts::install(cx);
        gpui_kit::init(cx);
        app_log::step("ui kit ready");
        theme::init(cx);
        dialog_keys::init(cx);
        app_log::step("theme ready, opening the window");
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.), px(900.)),
                cx,
            ))),
            window_min_size: Some(size(px(1024.), px(680.))),
            app_id: Some("net.swofty.ModLauncher".into()),
            titlebar: Some(TitlebarOptions {
                title: Some("Swofty Launcher for F1 25".into()),
                ..TitleBar::title_bar_options()
            }),
            ..TitleBar::window_options()
        };
        let (main, _) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Launcher::new(prepared, window, cx))
        })
        .expect("failed to open window");
        app_log::step("window opened");
        cx.on_window_closed(move |cx, id| {
            if id == main.window_id() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

struct Launcher {
    shell: Option<Entity<shell::Shell>>,
}

impl Launcher {
    fn new(prepared: Task<shell::Prepared>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.spawn_in(window, async move |this, cx| {
            let prepared = prepared.await;
            app_log::step("store and library loaded");
            this.update_in(cx, |this, window, cx| {
                let shell = cx.new(|cx| shell::Shell::new(prepared, window, cx));
                shell.update(cx, |s, cx| {
                    s.restore_console(cx);
                    s.run_startup(window, cx);
                    s.show_data_move_result(window, cx);
                    s.scan_for_lost_data(window, cx);
                    s.start_ui_mode(window, cx);
                });
                this.shell = Some(shell);
                app_log::step("launcher shell built");
                cx.notify();
            })
            .ok();
        })
        .detach();
        Self { shell: None }
    }
}

impl Render for Launcher {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        app_log::window_shown();
        if let Some(shell) = &self.shell {
            return shell.clone().into_any_element();
        }
        let p = theme::palette(cx);
        v_flex()
            .size_full()
            .bg(p.background)
            .text_color(p.text_muted)
            .child(TitleBar::new().bg(p.chrome))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap(px(20.))
                    .child(pages::wordmark(220.))
                    .child(swofty_i18n::tr("Loading your launcher…")),
            )
            .into_any_element()
    }
}
