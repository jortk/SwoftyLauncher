use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::timetrial::boards::{Run, check_number, format_time};
use swofty_i18n::tr;

use super::state::{Load, Verdict, View};
use crate::pages::GUTTER;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use crate::ui::{EmptyState, selectable_row};

pub(super) fn flag(p: Palette) -> Div {
    div()
        .flex_shrink_0()
        .child(Icon::new(IconName::CircleX).size(px(16.)).text_color(p.error))
}

const RUNS_WIDTH: f32 = 400.;

fn status_dot(status: &str, p: Palette) -> Div {
    div()
        .flex_shrink_0()
        .size(px(9.))
        .rounded_full()
        .bg(match status {
            "accepted" => rgb(0x4cc38a).into(),
            "pending" => rgb(0xffc933).into(),
            _ => p.error,
        })
}

fn status_name(status: &str) -> &'static str {
    match status {
        "accepted" => tr("Accepted"),
        "pending" => tr("Pending"),
        _ => tr("Rejected"),
    }
}

/// The server's ISO time as a short day and minute.
fn when(date: &str) -> String {
    date.get(..16).unwrap_or(date).replace('T', " ")
}

impl Shell {
    pub(super) fn tt_tabs(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let waiting = self.tt.waiting();
        let tab = |id: &'static str, label: String, view: View| {
            let on = self.tt.view == view;
            Button::new(id)
                .small()
                .when_else(on, |b| b.primary(), |b| b.ghost())
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tt.view = view;
                    this.tt.open_lap = None;
                    this.tt.banning = false;
                    this.tt.notice = None;
                    cx.notify();
                }))
        };
        let _ = p;
        h_flex()
            .gap(px(6.))
            .px(px(GUTTER))
            .pb(px(10.))
            .child(tab("tt-tab-boards", tr("Boards").to_owned(), View::Boards))
            .child(tab(
                "tt-tab-pending",
                match waiting {
                    0 => tr("Runs").to_owned(),
                    n => format!("{} ({n} {})", tr("Runs"), tr("pending")),
                },
                View::Runs,
            ))
            .into_any_element()
    }

    pub(super) fn tt_runs(&mut self, p: Palette, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let search = self.tt_run_search(window, cx);
        let chip = |id: String, label: String, on: bool| {
            Button::new(SharedString::from(id))
                .xsmall()
                .when_else(on, |b| b.primary(), |b| b.outline())
                .label(label)
        };
        let status = self.tt.run_status;
        let statuses = h_flex().gap(px(6.)).flex_wrap().children(
            [
                ("all", tr("All")),
                ("pending", tr("Pending")),
                ("accepted", tr("Accepted")),
                ("rejected", tr("Rejected")),
            ]
            .into_iter()
            .map(|(key, label)| {
                chip(format!("tt-run-status-{key}"), label.to_owned(), status == key).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.tt.run_status = key;
                        cx.notify();
                    },
                ))
            }),
        );
        let filters = h_flex()
            .gap(px(6.))
            .child(self.tt_filter_menu(true, cx))
            .child(self.tt_filter_menu(false, cx));
        let list: AnyElement = match &self.tt.runs {
            Load::Failed(e) => div()
                .text_size(px(13.))
                .text_color(p.error)
                .child(e.clone())
                .into_any_element(),
            Load::Idle | Load::Loading => div()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(tr("Loading runs…"))
                .into_any_element(),
            Load::Ready(_) => {
                let shown = self.tt.shown_runs();
                if shown.is_empty() {
                    div()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(tr("No runs match"))
                        .into_any_element()
                } else {
                    let rows: Vec<AnyElement> = shown
                        .iter()
                        .map(|run| {
                            let id = run.lap_id.clone();
                            selectable_row(
                                SharedString::from(format!("tt-run-{id}")),
                                self.tt.open_lap.as_deref() == Some(&id),
                                p,
                            )
                            .cursor_pointer()
                            .child(status_dot(&run.status, p))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(14.))
                                            .text_color(p.text)
                                            .child(run.driver.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(12.))
                                            .text_color(p.text_muted)
                                            .child(format!("{} · {}", run.season_name(), run.track_title())),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .flex_shrink_0()
                                    .items_end()
                                    .child(div().text_size(px(14.)).child(format_time(run.time_ms)))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(p.text_muted)
                                            .child(when(&run.date)),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| this.tt_select_lap(&id, cx)))
                            .into_any_element()
                        })
                        .collect();
                    div()
                        .flex_1()
                        .min_h_0()
                        .scroll_y("tt-runs")
                        .child(v_flex().gap(px(2.)).pr(px(6.)).children(rows))
                        .into_any_element()
                }
            }
        };
        let left = v_flex()
            .w(px(RUNS_WIDTH))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .gap(px(10.))
            .child(Input::new(&search).small())
            .child(statuses)
            .child(filters)
            .child(list);
        let open = self.tt.open_lap.as_ref().and_then(|id| match &self.tt.runs {
            Load::Ready(runs) => runs.iter().find(|r| &r.lap_id == id).cloned(),
            _ => None,
        });
        let right: AnyElement = match open {
            Some(run) => self.tt_run_detail(&run, p, window, cx),
            None => EmptyState::new(IconName::ShieldCheck, tr("Pick a run to review it")).into_any_element(),
        };
        h_flex()
            .flex_1()
            .min_h_0()
            .gap(px(20.))
            .px(px(GUTTER))
            .pb(px(16.))
            .child(left)
            .child(div().flex_1().min_w_0().h_full().min_h_0().child(right))
            .into_any_element()
    }

    fn tt_filter_menu(&self, season: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let choices = self.tt.choices(season);
        let picked = self.tt.picked(season).to_vec();
        let label = self.tt.filter_label(season, &choices);
        let all = if season {
            tr("All seasons")
        } else {
            tr("All tracks")
        };
        let shell = cx.entity().downgrade();
        Button::new(if season { "tt-run-season" } else { "tt-run-track" })
            .small()
            .when_else(picked.is_empty(), |b| b.outline(), |b| b.primary())
            .max_w(px((RUNS_WIDTH - 6.) / 2.))
            .label(label)
            .dropdown_caret(true)
            .disabled(choices.is_empty() && picked.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                menu = menu.scrollable(true).min_w(px(220.)).max_w(px(420.));
                let clear = shell.clone();
                menu =
                    menu.item(PopupMenuItem::new(all).checked(picked.is_empty()).on_click(
                        move |_, _, cx| {
                            clear.update(cx, |this, cx| this.tt_clear_filter(season, cx)).ok();
                        },
                    ))
                    .separator();
                for (key, label) in &choices {
                    let on = picked.contains(key);
                    let (shell, key) = (shell.clone(), key.clone());
                    menu = menu.item(PopupMenuItem::new(label.clone()).checked(on).on_click(
                        move |_, _, cx| {
                            shell
                                .update(cx, |this, cx| this.tt_toggle_filter(season, &key, cx))
                                .ok();
                        },
                    ));
                }
                menu
            })
    }

    fn tt_run_detail(
        &mut self,
        run: &Run,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let actions = self.tt_actions(
            &run.lap_id,
            &run.reasons,
            run.game_replay.is_some(),
            p,
            window,
            cx,
        );
        let title = |text: &'static str| {
            div()
                .pt(px(8.))
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        let cell =
            |text: String, width: f32| div().w(px(width)).flex_shrink_0().text_size(px(12.)).child(text);
        let head = |names: [(&'static str, f32); 3], first: &'static str| {
            h_flex()
                .gap(px(8.))
                .pb(px(4.))
                .text_color(p.text_muted)
                .child(div().flex_1().min_w_0().text_size(px(12.)).child(tr(first)))
                .children(names.into_iter().map(|(n, w)| cell(tr(n).to_owned(), w)))
        };
        let checks = run.checks.iter().map(|c| {
            h_flex()
                .gap(px(8.))
                .py(px(3.))
                .border_t_1()
                .border_color(p.border_variant)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .child(c.name.replace('.', " · ").replace('_', " ")),
                )
                .child(
                    div()
                        .w(px(60.))
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .text_color(if c.ok { rgb(0x4cc38a).into() } else { p.error })
                        .child(if c.ok { tr("Pass") } else { tr("Fail") }),
                )
                .child(cell(check_number(&c.value), 90.))
                .child(cell(check_number(&c.limit), 90.).text_color(p.text_muted))
        });
        let addons = run.addons.iter().map(|a| {
            h_flex()
                .gap(px(8.))
                .py(px(3.))
                .border_t_1()
                .border_color(p.border_variant)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .child(a.id.clone()),
                )
                .child(cell(a.version.clone().unwrap_or_default(), 90.))
                .child(
                    cell(a.source.clone().unwrap_or_else(|| tr("local").to_owned()), 90.)
                        .text_color(p.text_muted),
                )
        });
        let line = |label: &'static str, value: &Option<String>| {
            value.as_ref().filter(|v| !v.is_empty()).map(|v| {
                div()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(format!("{} {v}", tr(label)))
            })
        };
        let failed = run.checks.iter().filter(|c| !c.ok).count();
        div()
            .size_full()
            .scroll_y("tt-run-detail")
            .child(
                v_flex()
                    .gap(px(8.))
                    .pr(px(6.))
                    .child(
                        h_flex()
                            .gap(px(12.))
                            .items_end()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(run.driver.name.clone()),
                            )
                            .child(div().text_size(px(22.)).child(format_time(run.time_ms)))
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap(px(6.))
                                    .pb(px(3.))
                                    .child(status_dot(&run.status, p))
                                    .child(div().text_size(px(13.)).child(status_name(&run.status))),
                            )
                            .when(run.banned, |d| {
                                d.child(
                                    div()
                                        .pb(px(3.))
                                        .text_size(px(13.))
                                        .text_color(p.error)
                                        .child(tr("Banned")),
                                )
                            }),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(format!(
                        "{} · {} · {} · {}",
                        run.season_name(),
                        run.track_title(),
                        when(&run.date),
                        run.lap_id
                    )))
                    .children(line("Note:", &run.note))
                    .child(actions)
                    .child(title(tr("Verification")))
                    .child(div().text_size(px(12.)).text_color(p.text_muted).child(format!(
                        "{} {} · {} {}",
                        run.checks.len() - failed,
                        tr("passed"),
                        failed,
                        tr("failed")
                    )))
                    .child(head([("Result", 60.), ("Value", 90.), ("Limit", 90.)], "Check"))
                    .children(checks)
                    .child(title(tr("Addon set")))
                    .child(head([("", 0.), ("Version", 90.), ("Source", 90.)], "Addon"))
                    .children(addons)
                    .children(line("Game build", &run.game_build))
                    .children(line("Launcher", &run.launcher))
                    .children(line("Core", &run.core)),
            )
            .into_any_element()
    }

    pub(super) fn tt_actions(
        &mut self,
        lap: &str,
        reasons: &[String],
        game_replay: bool,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.tt.acting;
        let reason = self.tt_reason(window, cx);
        let typed = reason.read(cx).value().trim().to_owned();
        let id = lap.to_owned();
        let mut panel = v_flex().gap(px(8.)).px(px(12.)).pb(px(10.)).pt(px(4.));
        if !reasons.is_empty() {
            panel = panel.child(div().text_size(px(12.)).text_color(p.text_muted).child(format!(
                "{}: {}",
                tr("Flagged"),
                reasons.join(", ")
            )));
        }
        let url = format!("{}/lap/{}", swofty_core::timetrial::boards_url(), lap);
        let buttons = if self.tt.banning {
            let (ban, back) = (id.clone(), reason.clone());
            h_flex()
                .gap(px(8.))
                .items_center()
                .child(div().w(px(280.)).child(Input::new(&reason).small()))
                .child(
                    Button::new("tt-ban-confirm")
                        .small()
                        .danger()
                        .label(tr("Confirm ban"))
                        .disabled(busy || typed.is_empty())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let text = back.read(cx).value().trim().to_owned();
                            back.update(cx, |s, cx| s.set_value("", window, cx));
                            this.tt_moderate(ban.clone(), Verdict::Ban, text, cx);
                        })),
                )
                .child(
                    Button::new("tt-ban-cancel")
                        .small()
                        .ghost()
                        .label(tr("Cancel"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tt.banning = false;
                            cx.notify();
                        })),
                )
        } else {
            let (accept, deny) = (id.clone(), id.clone());
            let add = self.add_to_game_button("tt-add-to-game", lap, game_replay, cx);
            h_flex()
                .gap(px(8.))
                .child(
                    Button::new("tt-watch")
                        .small()
                        .outline()
                        .icon(IconName::Play)
                        .label(tr("Watch Replay"))
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                )
                .child(add.small())
                .child(
                    Button::new("tt-accept")
                        .small()
                        .primary()
                        .label(tr("Accept"))
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tt_moderate(accept.clone(), Verdict::Accept, String::new(), cx)
                        })),
                )
                .child(
                    Button::new("tt-deny")
                        .small()
                        .outline()
                        .label(tr("Deny"))
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tt_moderate(deny.clone(), Verdict::Deny, String::new(), cx)
                        })),
                )
                .child(
                    Button::new("tt-ban")
                        .small()
                        .danger()
                        .label(tr("Ban driver"))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tt.banning = true;
                            cx.notify();
                        })),
                )
        };
        panel = panel.child(buttons);
        if let Some((ok, text)) = self.online_replay_notice(lap) {
            panel = panel.child(
                div()
                    .text_size(px(12.))
                    .text_color(if *ok { p.text_muted } else { p.error })
                    .child(text.clone()),
            );
        }
        if let Some(note) = &self.tt.notice {
            panel = panel.child(div().text_size(px(12.)).text_color(p.error).child(note.clone()));
        }
        panel.into_any_element()
    }
}
