use std::time::Duration;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::data_usage::{self, KEEP_FOR, KEEP_SESSIONS};
use swofty_core::install::Source;
use swofty_core::session::UiMode;
use swofty_core::update;
use swofty_core::updater;
use swofty_i18n::{tr, trf};

use crate::format::size_coarse as size;
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::form::{Choice, Notice, SettingRow, SettingsSection, ValuePill, segmented, select, usage_bar};

const DOCS: &str = "https://f1.swofty.net";

fn link_button(id: &'static str, url: &'static str) -> Button {
    Button::new(id)
        .outline()
        .tap_sm()
        .icon(IconName::ExternalLink)
        .label(tr("Open"))
        .on_click(move |_, _, cx| cx.open_url(url))
}

fn keep_days() -> u64 {
    KEEP_FOR.as_secs() / Duration::from_secs(86_400).as_secs()
}

impl Shell {
    pub(super) fn settings_general(&self, cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity().downgrade();
        let current = swofty_i18n::language();
        let languages = swofty_i18n::languages()
            .into_iter()
            .map(|l| {
                let shell = shell.clone();
                Choice::new(l.name, l.code == current, move |window, cx| {
                    shell.update(cx, |s, cx| s.set_language(l.code, window, cx)).ok();
                })
            })
            .collect();
        let start_label = match self.install.as_ref().map(|i| i.source) {
            Some(Source::EaApp) => tr("Start the EA app automatically"),
            _ => tr("Start Steam automatically"),
        };
        let start = Switch::new("settings-start-store")
            .checked(self.start_store)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_start_store(*on, window, cx)));
        let time_trial = Switch::new("settings-time-trial-boards")
            .checked(self.time_trial_boards)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_time_trial_boards(*on, window, cx)));
        let time_trial_ingame = Switch::new("settings-time-trial-ingame")
            .checked(self.time_trial_ingame)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_time_trial_ingame(*on, window, cx)));
        let home_video = Switch::new("settings-home-video")
            .checked(self.home_video)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_home_video(*on, window, cx)));
        let use_profiles = Switch::new("settings-use-profiles")
            .checked(self.use_profiles)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_use_profiles(*on, window, cx)));
        let update_off_addons = Switch::new("settings-update-off-addons")
            .checked(self.update_off_addons)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_update_off_addons(*on, window, cx)));
        let ad_privacy = self.ads.on().then(|| {
            SettingRow::new(tr("Ad privacy choices"))
                .description(tr("Choose how ads in the launcher may use your data"))
                .control(
                    Button::new("settings-ad-privacy")
                        .outline()
                        .tap_sm()
                        .label(tr("Change"))
                        .on_click(cx.listener(|this, _, window, cx| this.open_ad_privacy(window, cx))),
                )
        });
        let section = SettingsSection::new(tr("General"))
            .row(
                SettingRow::new(tr("Language"))
                    .description(tr("The launcher's own text. Text that isn't translated yet stays in English, and addons and F1 25 keep their own"))
                    .control(select("settings-language", languages)),
            )
            .row(
                SettingRow::new(start_label)
                    .description(tr("When you press Play"))
                    .control(start),
            )
            .row(
                SettingRow::new(tr("Use profiles"))
                    .description(tr("Turned off, the launcher hides every profile and works on the one that is active now, or on your first modded profile if that is vanilla. Your other profiles are kept untouched and come back when you turn this on."))
                    .control(use_profiles),
            )
            .row(
                SettingRow::new(tr("Update turned-off addons"))
                    .description(tr("Also update the addons that are off in your profile when Play updates addons. Turned off, only the addons that are on are updated."))
                    .control(update_off_addons),
            )
            .row(
                SettingRow::new(tr("Time Trial boards"))
                    .description(tr("Upload your Time Trial laps to the Swofty leaderboards. Ranked laps load a closed-source integrity module from Swofty's server."))
                    .control(time_trial),
            )
            .row(
                SettingRow::new(tr("In-game leaderboards"))
                    .description(tr("Show the Swofty boards and rival ghosts inside Time Trial for addon seasons. Experimental."))
                    .control(time_trial_ingame),
            )
            .row(
                SettingRow::new(tr("Home screen video"))
                    .description(tr("Show the latest Swofty video in the bottom right of the F1 25 home screen. A change shows the next time you press Play."))
                    .control(home_video),
            );
        match ad_privacy {
            Some(row) => section.row(row),
            None => section,
        }
        .into_any_element()
    }

    pub(super) fn settings_display(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity().downgrade();
        let tv = self.ui_mode.is_some_and(UiMode::is_tv);
        let choice = |label: &'static str, mode: UiMode| {
            let shell = shell.clone();
            Choice::new(label, tv == mode.is_tv(), move |window, cx| {
                shell.update(cx, |s, cx| s.set_ui_mode(mode, window, cx)).ok();
            })
        };
        let shown = self.ui_scale_drag.or(self.ui_scale);
        let readout = shown.map_or_else(|| tr("Auto").to_owned(), |p| format!("{p}%"));
        let reset = Button::new("settings-ui-scale-reset")
            .outline()
            .tap_sm()
            .label(tr("Reset to automatic"))
            .disabled(self.ui_scale.is_none() && self.ui_scale_drag.is_none())
            .on_click(cx.listener(|this, _, window, cx| {
                this.ui_scale_drag = None;
                this.set_ui_scale(None, window, cx)
            }));
        let scale = h_flex()
            .gap(px(12.))
            .items_center()
            .child(div().w(px(220.)).child(Slider::new(&self.ui_scale_slider)))
            .child(
                div()
                    .w(px(56.))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .text_color(p.text)
                    .child(readout),
            )
            .child(reset);
        SettingsSection::new(tr("Interface"))
            .row(
                SettingRow::new(tr("Interface scale"))
                    .description(tr("Makes the whole launcher larger or smaller, on top of the display mode. Auto keeps the size the display mode gives"))
                    .control(scale),
            )
            .row(
                SettingRow::new(tr("Display mode"))
                    .description(tr("PC is the normal layout. TV mode scales the whole launcher up and widens the scrollbars, for handhelds and couch play"))
                    .control(segmented(
                        "settings-ui-mode",
                        vec![choice(tr("PC mode"), UiMode::Pc), choice(tr("TV mode"), UiMode::Tv)],
                        p,
                    )),
            )
            .into_any_element()
    }

    pub(super) fn settings_storage(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap(px(24.))
            .child(self.storage_location(cx))
            .child(self.storage_usage(p, cx))
            .child(self.storage_cleanup(cx))
            .into_any_element()
    }

    fn storage_location(&self, cx: &mut Context<Self>) -> SettingsSection {
        let root = self.store.root.clone();
        let open = Button::new("settings-open-data")
            .outline()
            .tap_sm()
            .icon(IconName::FolderOpen)
            .label(tr("Open"))
            .on_click(move |_, _, cx| cx.open_with_system(&crate::reveal::real_path(&root)));
        let moving = Button::new("settings-move-data")
            .outline()
            .tap_sm()
            .icon(IconName::FolderInput)
            .label(tr("Move…"))
            .on_click(cx.listener(|this, _, window, cx| this.open_data_move(window, cx)));
        let find = Button::new("settings-find-data")
            .outline()
            .tap_sm()
            .icon(IconName::Search)
            .label(tr("Find"))
            .on_click(cx.listener(|this, _, window, cx| this.scan_for_data(true, window, cx)));
        SettingsSection::new(tr("Location"))
            .row(
                SettingRow::new(tr("Your addons and saves"))
                    .description(
                        swofty_core::data_move::link::real_folder(&self.store.root)
                            .display()
                            .to_string(),
                    )
                    .control(open)
                    .control(moving),
            )
            .row(
                SettingRow::new(tr("Find my addons"))
                    .description(tr(
                        "Look in other folders and drives for addons and saves you used before",
                    ))
                    .control(find),
            )
    }

    fn storage_usage(&self, p: Palette, cx: &mut Context<Self>) -> SettingsSection {
        let refresh = Button::new("settings-refresh")
            .outline()
            .tap_sm()
            .icon(IconName::RefreshCw)
            .label(tr("Refresh"))
            .disabled(self.settings.measuring)
            .on_click(cx.listener(|this, _, _, cx| this.settings_measure(cx)));
        let section = SettingsSection::new(tr("Usage")).action(refresh);
        let Some(u) = &self.settings.usage else {
            return section.row(SettingRow::new(tr("Size")).description(tr("Measuring…")));
        };
        let mut free = trf!(
            "{size} free on this drive",
            size = u.free.map_or("?".to_owned(), size)
        );
        if let Some(pf) = u.profile_free.filter(|pf| Some(*pf) != u.free) {
            free += &trf!(
                ", {size} on {drive}",
                size = size(pf),
                drive = data_usage::GAME_DRIVE
            );
        }
        let short = super::low_space_warning(data_usage::Room {
            game: u.profile_free,
            data: u.free,
        });
        let mut parts: Vec<(String, u64)> = u.parts.iter().map(|x| (x.name.to_owned(), x.bytes)).collect();
        parts.push((tr("Other").to_owned(), u.other));
        parts.sort_by_key(|x| std::cmp::Reverse(x.1));
        let max = parts.first().map_or(1, |x| x.1.max(1));
        let bars = v_flex().w_full().gap(px(10.)).children(
            parts
                .into_iter()
                .filter(|x| x.1 > 0)
                .map(|(name, bytes)| usage_bar(name, size(bytes), bytes as f32 / max as f32, p)),
        );
        section
            .row(
                SettingRow::new(tr("Size"))
                    .description(free)
                    .control(ValuePill::new(size(u.total))),
            )
            .when_some(short, |s, short| {
                s.row(Notice::warning(trf!(
                    "{short} Clear old logs, remove addons you don't use, or move your addons and saves.",
                    short = short
                )))
            })
            .row(SettingRow::new(tr("Breakdown")).below(bars))
    }

    fn storage_cleanup(&self, cx: &mut Context<Self>) -> SettingsSection {
        let mut section = SettingsSection::new(tr("Cleanup"));
        if let Some(u) = &self.settings.usage {
            let old = self.settings.prunable;
            let clear = Button::new("settings-clear-logs")
                .outline()
                .tap_sm()
                .icon(IconName::Eraser)
                .label(if old.sessions == 0 {
                    tr("Nothing to clear").to_owned()
                } else {
                    trf!(
                        "Clear {count} old ({size})",
                        count = old.sessions,
                        size = size(old.bytes)
                    )
                })
                .disabled(old.sessions == 0 || self.settings.clearing)
                .on_click(cx.listener(|this, _, window, cx| this.confirm_clear_logs(window, cx)));
            section = section.row(
                SettingRow::new(tr("Session logs"))
                    .description(trf!(
                        "{count} game sessions kept. Clearing keeps the newest {sessions} and everything from the last {days} days.",
                        count = u.sessions,
                        sessions = KEEP_SESSIONS,
                        days = keep_days()
                    ))
                    .control(clear),
            );
        }
        let cache = self.settings.cache;
        if cache.apart || cache.bytes > 0 {
            let clear = Button::new("settings-clear-cache")
                .outline()
                .tap_sm()
                .icon(IconName::Eraser)
                .label(tr("Clear"))
                .disabled(cache.bytes == 0 || self.settings.clearing)
                .on_click(cx.listener(|this, _, window, cx| this.settings_clear_cache(window, cx)));
            section = section.row(
                SettingRow::new(tr("Game drive copy"))
                    .description(trf!(
                        "Play keeps a copy of your {what} on F1 25's drive and links the game's files to it, which keeps Play fast ({size}). Press Play to rebuild it after clearing.",
                        what = if cache.apart { tr("mods") } else { tr("addon files") },
                        size = size(cache.bytes)
                    ))
                    .control(clear),
            );
        }
        section
    }

    pub(super) fn settings_advanced(&self, cx: &mut Context<Self>) -> AnyElement {
        let developer = Switch::new("settings-developer")
            .checked(self.developer_mode)
            .on_click(cx.listener(|this, on: &bool, window, cx| this.set_developer(*on, window, cx)));
        let auto_clear = Switch::new("settings-auto-clear-logs")
            .checked(self.clear_old_session_logs)
            .on_click(
                cx.listener(|this, on: &bool, window, cx| this.set_clear_old_session_logs(*on, window, cx)),
            );
        v_flex()
            .gap(px(24.))
            .child(
                SettingsSection::new(tr("Developer")).row(
                    SettingRow::new(tr("Developer mode"))
                        .description(tr("Shows the Tools page with the swofty dev actions"))
                        .control(developer),
                ),
            )
            .child(
                SettingsSection::new(tr("Logs")).row(
                    SettingRow::new(tr("Clear old logs"))
                        .description(trf!(
                            "Off by default. When on, the launcher clears old logs as it starts, as Clear old does: keeps the newest {sessions} sessions and the last {days} days",
                            sessions = KEEP_SESSIONS,
                            days = keep_days()
                        ))
                        .control(auto_clear),
                ),
            )
            .into_any_element()
    }

    pub(super) fn settings_about(&self, _: &mut Context<Self>) -> AnyElement {
        let u = &self.updates;
        let status = if u.checking {
            tr("Checking for updates…").to_owned()
        } else {
            let versions = u.plan().versions();
            if versions.is_empty() {
                tr("Up to date").to_owned()
            } else {
                trf!(
                    "Update available: {versions}",
                    versions = updater::describe(&versions)
                )
            }
        };
        v_flex()
            .gap(px(24.))
            .child(
                SettingsSection::new(tr("Swofty Launcher"))
                    .row(
                        SettingRow::new(tr("Version"))
                            .control(ValuePill::new(format!("v{}", update::current_version()))),
                    )
                    .row(SettingRow::new(tr("Updates")).description(status)),
            )
            .child(
                SettingsSection::new(tr("Links"))
                    .row(
                        SettingRow::new(tr("Documentation"))
                            .description(DOCS)
                            .control(link_button("settings-docs", DOCS)),
                    )
                    .row(
                        SettingRow::new(tr("Discord server"))
                            .description(crate::discord::INVITE)
                            .control(link_button("settings-discord", crate::discord::INVITE)),
                    ),
            )
            .into_any_element()
    }
}
