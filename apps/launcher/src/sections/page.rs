use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::view::{self, Art, Block, Body, ButtonKind, Control, Event, Row, Style};
use swofty_i18n::tr;

use super::*;
use crate::blocks::{At, caps, eyebrow, search_id};
use crate::library::{AddonIcon, SectionItem};
use crate::pages::{GUTTER, badge, chevron, fact, names, stat, tag, tile};
use crate::scroll::SmoothScroll;
use crate::shell::Shell;
use crate::theme::Palette;

#[derive(Clone, Copy)]
pub(super) struct RowAt<'a> {
    pub(super) addon: &'a str,
    pub(super) section: &'a str,
    pub(super) index: usize,
    pub(super) position: usize,
    pub(super) count: usize,
    pub(super) reorder: bool,
}

impl Shell {
    pub(crate) fn section_page(&mut self, index: usize, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(section) = self.library.sections.get(index) else {
            return div().into_any_element();
        };
        let (addon, id, title) = (
            section.addon.clone(),
            section.spec.id.clone(),
            section.spec.title.clone(),
        );
        let section_icon = icon(section.spec.icon);
        let view = match self.views.get(&format!("{addon}/{id}")) {
            Some(Ok(v)) => v.clone(),
            Some(Err(e)) => {
                return v_flex()
                    .size_full()
                    .child(self.header(title, String::new(), div(), p))
                    .child(self.empty(section_icon, tr("This page could not load"), e.clone(), div(), p))
                    .into_any_element();
            }
            None => {
                return v_flex()
                    .size_full()
                    .child(self.header(title, tr("Loading…").into(), div(), p))
                    .into_any_element();
            }
        };
        let mut actions: Vec<AnyElement> = view
            .actions
            .iter()
            .map(|b| self.button(&addon, &id, b, false, cx))
            .collect();
        // Tracks is in the Addons group, whose pages all have Install addon.
        if id == crate::library::TRACKS_SECTION {
            actions.push(self.install_addon_button(cx).into_any_element());
        }
        let notices: Vec<AnyElement> = view
            .notices
            .iter()
            .enumerate()
            .map(|(i, n)| {
                h_flex()
                    .id(SharedString::from(format!("notice-{i}")))
                    .mx(px(GUTTER))
                    .mb(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .gap(px(10.))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border_variant)
                    .bg(p.surface)
                    .text_size(px(13.))
                    .child(
                        Icon::new(IconName::CircleAlert)
                            .size(px(14.))
                            .text_color(tone(n.tone, p)),
                    )
                    .child(div().flex_1().min_w_0().text_color(p.text).child(n.text.clone()))
                    .children(n.action.as_ref().map(|b| self.button(&addon, &id, b, true, cx)))
                    .into_any_element()
            })
            .collect();
        let width = self.content_width;
        let mut search_bar = None;
        let mut footer = view.footer.clone().map(|f| {
            div()
                .flex_shrink_0()
                .py(px(10.))
                .border_t_1()
                .border_color(p.border_variant)
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .child(f)
        });
        let body = match &view.body {
            Body::List {
                rows,
                reorder,
                search,
            } => {
                let key = search
                    .as_ref()
                    .map(|s| format!("{addon}/{id}/{}", search_id(s, "list")));
                let query = key
                    .as_deref()
                    .filter(|_| search.as_ref().is_some_and(|s| s.filter))
                    .and_then(|k| self.query(k));
                search_bar = key.and_then(|k| self.search_box(&k, p));
                let shown: Vec<(usize, &Row)> = rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| {
                        query.is_none_or(|q| {
                            let tags = r.tags.iter().map(|t| t.text.as_str());
                            view::Search::matches(
                                q,
                                [r.title.as_str(), r.subtitle.as_str()].into_iter().chain(tags),
                            )
                        })
                    })
                    .collect();
                let rows: Vec<AnyElement> = shown
                    .iter()
                    .enumerate()
                    .map(|(n, (i, r))| {
                        let at = RowAt {
                            addon: &addon,
                            section: &id,
                            index: *i,
                            position: n,
                            count: rows.len(),
                            reorder: *reorder,
                        };
                        self.view_row(at, r, p, cx)
                    })
                    .collect();
                if rows.is_empty() && query.is_some() {
                    div()
                        .px(px(GUTTER))
                        .py(px(12.))
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(tr("No matches"))
                        .into_any_element()
                } else {
                    self.list(SharedString::from(format!("section-{addon}-{id}")), rows, p)
                        .children(footer.take().map(|f| f.mt(px(16.))))
                        .into_any_element()
                }
            }
            Body::Empty { title, text, action } => {
                let action = action
                    .as_ref()
                    .map(|b| self.button(&addon, &id, b, false, cx))
                    .unwrap_or_else(|| div().into_any_element());
                self.empty(section_icon, title.clone(), text.clone(), action, p)
            }
            // The wheel glides, as on the Addons and Addon Browser pages.
            Body::Blocks(blocks) => {
                let (scroll, smooth) = self
                    .blocks
                    .section_scrolls
                    .entry(format!("{addon}/{id}"))
                    .or_insert_with(|| (ScrollHandle::new(), SmoothScroll::default()))
                    .clone();
                let body = v_flex()
                    .flex_none()
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(24.))
                    .gap(px(16.))
                    .children(blocks.iter().map(|b| {
                        let at = At {
                            addon: &addon,
                            section: &id,
                            width,
                            nested: false,
                            fill: false,
                        };
                        self.block_at(at, b, p, cx)
                    }))
                    .children(footer.take());
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        div()
                            .id(SharedString::from(format!("section-{addon}-{id}")))
                            .size_full()
                            .flex()
                            .flex_col()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .child(body),
                    )
                    .child(smooth.overlay(&scroll, cx))
                    .into_any_element()
            }
        };
        v_flex()
            .size_full()
            .child(self.header(
                title,
                view.subtitle.clone(),
                h_flex().gap(px(8.)).children(actions),
                p,
            ))
            .children(notices)
            .children(search_bar.map(|s| h_flex().px(px(GUTTER)).pb(px(12.)).flex_shrink_0().child(s)))
            .child(body)
            .children(footer.map(|f| f.mx(px(GUTTER))))
            .into_any_element()
    }

    // A row without art shows its page's icon: `sections/<id>.svg` when the addon has one,
    // else the page's icon from the launcher's set.
    pub(super) fn row_art(&self, addon: &str, section: &str, a: &Art, size: f32, p: Palette) -> AnyElement {
        if !a.is_none() {
            return art(a, size, p);
        }
        let s = self
            .library
            .sections
            .iter()
            .find(|s| s.addon == addon && s.spec.id == section);
        match s {
            Some(SectionItem {
                icon: Some(AddonIcon::Glyph(data)),
                ..
            }) => tile(size, p)
                .child(svg().data(data).size(px(size * 0.5)).text_color(p.text_muted))
                .into_any_element(),
            Some(s) => badge(icon(s.spec.icon), size, p).into_any_element(),
            None => art(a, size, p),
        }
    }

    pub(super) fn view_row(&self, at: RowAt, r: &Row, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let RowAt {
            addon,
            section,
            index,
            position,
            count,
            reorder,
        } = at;
        let key = format!("{addon}/{section}/{}", r.id);
        let open = self.open_row.as_ref() == Some(&key);
        let expandable = !r.detail.is_empty() || reorder;
        let trailing = r.trailing.as_ref().map(|c| self.control(addon, section, c, cx));
        let toggle_key = key.clone();
        let head = self
            .row_head(format!("row-{key}"), open, p)
            .child(self.row_art(addon, section, &r.art, 32., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(r.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(r.subtitle.clone()),
                    ),
            )
            .child(
                h_flex().gap(px(4.)).flex_shrink_0().children(
                    r.tags
                        .iter()
                        .map(|t| tag(t.text.clone(), p).text_color(tone(t.tone, p))),
                ),
            )
            .children(trailing.map(|t| div().w(px(96.)).flex().justify_end().child(t)))
            .when(expandable, |d| d.child(chevron(open, p)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_row = (this.open_row.as_ref() != Some(&toggle_key)).then(|| toggle_key.clone());
                cx.notify();
            }));
        let head = match r.menu.is_empty() {
            true => head.into_any_element(),
            false => {
                let shell = cx.entity().downgrade();
                let (addon, section, items, key) =
                    (addon.to_owned(), section.to_owned(), r.menu.clone(), key.clone());
                head.context_menu(move |mut menu, _, _| {
                    for b in &items {
                        let (shell, addon, section, b) =
                            (shell.clone(), addon.clone(), section.clone(), b.clone());
                        let control = format!("{key}/menu-{}", b.id);
                        let mut item = PopupMenuItem::new(b.label.clone());
                        if let Some(i) = b.icon {
                            item = item.icon(icon(i));
                        }
                        menu = menu.item(item.on_click(move |_, window, cx| {
                            shell
                                .update(cx, |this, cx| {
                                    this.press(&addon, &section, control.clone(), &b, window, cx)
                                })
                                .ok();
                        }));
                    }
                    menu
                })
                .into_any_element()
            }
        };
        let moves = reorder.then(|| {
            let mv = |id: &'static str,
                      label: &'static str,
                      ic: IconName,
                      to: usize,
                      enabled: bool,
                      cx: &mut Context<Self>| {
                let (addon, section, row) = (addon.to_owned(), section.to_owned(), r.id.clone());
                let control = format!("{key}/move-{id}");
                Button::new(SharedString::from(control.clone()))
                    .outline()
                    .tap_sm()
                    .icon(ic)
                    .label(label)
                    .disabled(!enabled || self.busy.contains(&control))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let event = Event::Move { row: row.clone(), to };
                        this.send(&addon, &section, control.clone(), event, window, cx)
                    }))
            };
            h_flex()
                .gap(px(6.))
                .child(mv(
                    "up",
                    tr("Move up"),
                    IconName::ArrowUp,
                    index.saturating_sub(1),
                    index > 0,
                    cx,
                ))
                .child(mv(
                    "down",
                    tr("Move down"),
                    IconName::ArrowDown,
                    index + 1,
                    index + 1 < count,
                    cx,
                ))
        });
        self.row(position == 0, open, p)
            .child(head)
            .when(open, |d| {
                d.child(
                    v_flex()
                        .gap(px(16.))
                        .pl(px(58.))
                        .pr(px(20.))
                        .pt(px(14.))
                        .pb(px(18.))
                        .border_t_1()
                        .border_color(p.border_variant)
                        .children(r.detail.iter().map(|b| {
                            let at = At {
                                addon,
                                section,
                                width: (self.content_width - 80.).max(240.),
                                nested: false,
                                fill: false,
                            };
                            self.block_at(at, b, p, cx)
                        }))
                        .children(moves),
                )
            })
            .into_any_element()
    }

    pub(crate) fn block_at(&self, at: At, b: &Block, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        if let Some(rich) = self.rich_block(at, b, p, cx) {
            return rich;
        }
        let (addon, section) = (at.addon, at.section);
        match b {
            Block::Stats(facts) => h_flex()
                .flex_wrap()
                .gap_x(px(32.))
                .gap_y(px(12.))
                .children(
                    facts
                        .iter()
                        .map(|f| stat(caps(&f.label), f.value.clone(), p.text, p)),
                )
                .into_any_element(),
            Block::Facts(facts) => v_flex()
                .max_w(px(560.))
                .children(facts.iter().map(|f| fact(f.label.clone(), f.value.clone(), p)))
                .into_any_element(),
            Block::Label(text) => eyebrow(text, p).into_any_element(),
            Block::Text { text, tone: t } => div()
                .max_w(px(720.))
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(tone(*t, p))
                .child(text.clone())
                .into_any_element(),
            Block::Names { label, items } => names(caps(label), items, p).into_any_element(),
            Block::Table(table) => {
                let columns: Vec<&str> = table.columns.iter().map(|c| c.title()).collect();
                let rows: Vec<Vec<String>> = table
                    .rows
                    .iter()
                    .map(|r| r.cells().iter().map(|c| c.text().to_owned()).collect())
                    .collect();
                let cell = |text: String, head: bool| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(if head { 11. } else { 13. }))
                        .text_color(if head { p.text_placeholder } else { p.text })
                        .child(text)
                };
                v_flex()
                    .max_w(px(760.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(p.border_variant)
                    .child(
                        h_flex()
                            .px(px(12.))
                            .py(px(6.))
                            .gap(px(12.))
                            .children(columns.iter().map(|c| cell(caps(c), true))),
                    )
                    .children(rows.iter().map(|row| {
                        h_flex()
                            .px(px(12.))
                            .py(px(6.))
                            .gap(px(12.))
                            .border_t_1()
                            .border_color(p.border_variant)
                            .children(row.iter().map(|c| cell(c.clone(), false)))
                    }))
                    .into_any_element()
            }
            Block::Buttons(buttons) => h_flex()
                .gap(px(6.))
                .children(buttons.iter().map(|b| self.button(addon, section, b, true, cx)))
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }

    pub(super) fn control(
        &self,
        addon: &str,
        section: &str,
        c: &Control,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match c {
            Control::Switch { id, on, enabled } => {
                let control = format!("{addon}/{section}/{id}");
                let hold = SharedString::from(format!("hold-{control}"));
                let (addon, section, id) = (addon.to_owned(), section.to_owned(), id.clone());
                let switch = Switch::new(SharedString::from(format!("switch-{control}")))
                    .checked(*on)
                    .tap_sm()
                    .disabled(!enabled || self.busy.contains(&control))
                    .on_change(cx.listener(move |this, on: &bool, window, cx| {
                        let event = Event::Toggle {
                            id: id.clone(),
                            on: *on,
                        };
                        this.send(&addon, &section, control.clone(), event, window, cx)
                    }));
                div()
                    .id(hold)
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(switch)
                    .into_any_element()
            }
            Control::Choice { id, options, current } => {
                let control = format!("{addon}/{section}/{id}");
                let shell = cx.entity().downgrade();
                let (options, current) = (options.clone(), *current);
                let (addon, section, id) = (addon.to_owned(), section.to_owned(), id.clone());
                Button::new(SharedString::from(format!("choice-{control}")))
                    .outline()
                    .tap_sm()
                    .min_w(px(140.))
                    .label(options.get(current).cloned().unwrap_or_default())
                    .dropdown_caret(true)
                    .disabled(self.busy.contains(&control))
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                        for (i, text) in options.iter().enumerate() {
                            let (shell, addon, section, id, control) = (
                                shell.clone(),
                                addon.clone(),
                                section.clone(),
                                id.clone(),
                                control.clone(),
                            );
                            menu =
                                menu.item(PopupMenuItem::new(text.clone()).checked(i == current).on_click(
                                    move |_, window, cx| {
                                        let event = Event::Choose {
                                            id: id.clone(),
                                            index: i,
                                        };
                                        shell
                                            .update(cx, |this, cx| {
                                                this.send(
                                                    &addon,
                                                    &section,
                                                    control.clone(),
                                                    event,
                                                    window,
                                                    cx,
                                                )
                                            })
                                            .ok();
                                    },
                                ));
                        }
                        menu
                    })
                    .into_any_element()
            }
            Control::Button(b) => self.button(addon, section, b, true, cx),
        }
    }

    pub(crate) fn button(
        &self,
        addon: &str,
        section: &str,
        b: &view::Button,
        small: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = format!("{addon}/{section}/{}", b.id);
        let mut button = Button::new(SharedString::from(format!("button-{control}")))
            .label(b.label.clone())
            .loading(self.busy.contains(&control))
            .disabled(self.busy.contains(&control));
        button = match b.style {
            Style::Primary => button.primary(),
            Style::Outline => button.outline(),
            Style::Ghost => button.ghost(),
            Style::Danger => button.danger().ghost(),
        };
        button = match b.size {
            view::ButtonSize::Large => button.large().h(px(52.)).px(px(32.)).text_size(px(16.)),
            view::ButtonSize::Normal if small => button.tap_sm(),
            view::ButtonSize::Normal => button,
        };
        if let Some(i) = b.icon {
            button = button.icon(icon(i));
        }
        let (addon, section, b) = (addon.to_owned(), section.to_owned(), b.clone());
        button
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.press(&addon, &section, control.clone(), &b, window, cx);
            }))
            .into_any_element()
    }

    // A click on a button or a row's menu item: send it, ask first, or pick files.
    pub(super) fn press(
        &mut self,
        addon: &str,
        section: &str,
        control: String,
        b: &view::Button,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (addon, section, id) = (addon.to_owned(), section.to_owned(), b.id.clone());
        let click = Event::Click { id: id.clone() };
        match &b.kind {
            ButtonKind::Event => self.send(&addon, &section, control, click, window, cx),
            ButtonKind::Confirm { title, text } => {
                let shell = cx.entity().downgrade();
                let (title, text) = (
                    SharedString::from(title.clone()),
                    SharedString::from(text.clone()),
                );
                window.open_alert_dialog(cx, move |alert, _, _| {
                    let (shell, addon, section, control, id) = (
                        shell.clone(),
                        addon.clone(),
                        section.clone(),
                        control.clone(),
                        id.clone(),
                    );
                    alert
                        .title(title.clone())
                        .description(text.clone())
                        .show_cancel(true)
                        .on_ok(move |_, window, cx| {
                            let click = Event::Click { id: id.clone() };
                            shell
                                .update(cx, |this, cx| {
                                    this.send(&addon, &section, control.clone(), click, window, cx)
                                })
                                .ok();
                            true
                        })
                });
            }
            ButtonKind::Pick {
                title,
                folders,
                multiple,
                ..
            } => {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: *folders,
                    multiple: *multiple,
                    prompt: Some(SharedString::from(title.clone())),
                });
                cx.spawn_in(window, async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = rx.await {
                        this.update_in(cx, |s, window, cx| {
                            let event = Event::Picked { id, paths };
                            s.send(&addon, &section, control, event, window, cx)
                        })
                        .ok();
                    }
                })
                .detach();
            }
        }
    }
}
