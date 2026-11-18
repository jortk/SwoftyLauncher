use crate::notices::Notify;
use std::cell::Cell;

use crate::scale::{Hit, px};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::dbedit::NodeId;
use swofty_i18n::{tr, trf};

use super::{ADDABLE, type_name};
use crate::shell::Shell;
use crate::theme::palette;
use crate::ui::Field;

impl Shell {
    pub(crate) fn db_add_dialog(&mut self, at: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(catalog) = self.db.catalog().cloned() else {
            return;
        };
        let base = if at == catalog.root() {
            String::new()
        } else {
            format!("{}.", catalog.path(at))
        };
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("tracks.Track_3.my_new_key")
                .default_value(base)
        });
        let value = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Value")));
        let tag = std::rc::Rc::new(Cell::new(0u8));
        let shell = cx.entity().downgrade();
        let p = palette(cx);
        let focus = Cell::new(true);
        window.open_alert_dialog(cx, move |alert, window, cx| {
            if focus.replace(false) {
                path.update(cx, |s, cx| s.focus(window, cx));
            }
            let (path, value, shell, tag) = (path.clone(), value.clone(), shell.clone(), tag.clone());
            let current = tag.get();
            let picker = {
                let tag = tag.clone();
                Button::new("db-add-type")
                    .outline()
                    .tap_sm()
                    .min_w(px(120.))
                    .label(type_name(current))
                    .dropdown_caret(true)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (t, name) in ADDABLE {
                            let tag = tag.clone();
                            menu = menu.item(PopupMenuItem::new(name).checked(t == current).on_click(move |_, window, _| {
                                tag.set(t);
                                window.refresh();
                            }));
                        }
                        menu
                    })
            };
            alert
                .width(px(560.))
                .title(tr("Add key"))
                .child(
                    v_flex()
                        .gap(px(14.))
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(
                            tr("Adds a key the stock database doesn't have. The game only reads keys it knows, so this is for paths you've confirmed."),
                        ))
                        .child(Field::new(tr("PATH"), Input::new(&path).tap_sm()))
                        .child(
                            h_flex()
                                .gap(px(12.))
                                .items_start()
                                .child(Field::new(tr("TYPE"), picker))
                                .child(div().flex_1().child(Field::new(tr("VALUE"), Input::new(&value).tap_sm()))),
                        ),
                )
                .ok_text(tr("Add key"))
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let p = path.read(cx).value().trim().to_owned();
                    let v = value.read(cx).value().trim().to_owned();
                    let result = shell
                        .update(cx, |s, cx| s.db_add_key(&p, tag.get(), &v, cx))
                        .unwrap_or_else(|_| Err("The launcher is closing".into()));
                    match result {
                        Ok(()) => {
                            window.notify(
                                crate::notices::Notification::success(trf!("Added {p}", p = p)),
                                cx,
                            );
                            true
                        }
                        Err(e) => {
                            crate::app_log::action("db editor: add a key").failed(&e);
                            window.notify(
                                crate::notices::Notification::error(e).title(tr("Could not add the key")),
                                cx,
                            );
                            false
                        }
                    }
                })
        });
    }
}
