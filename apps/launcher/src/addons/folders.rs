use super::drag::{Dragged, Dragging};
use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use crate::shell::{Prompt, Shell};
use crate::theme::Palette;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::folders::{Entry, Key};
use swofty_i18n::{tr, trf, trn};

impl Shell {
    pub(super) fn toggle_folder(&mut self, key: &Key, window: &mut Window, cx: &mut Context<Self>) {
        let mut f = self.library.folders.clone();
        f.set_collapsed(key, !f.is_collapsed(key));
        match self.store.save_folders(&f) {
            Ok(()) => self.library.folders = f,
            Err(e) => {
                crate::app_log::action("save folders").failed(&e);
                window.notify(Notification::error(e.to_string()), cx)
            }
        }
        cx.notify();
    }

    pub(super) fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = Prompt {
            title: tr("New folder"),
            description: "Drag addons onto the folder to put them in it. Its addons load together, \
                          and dragging the folder moves them all."
                .to_owned(),
            initial: String::new(),
            placeholder: tr("Folder name"),
            ok: tr("Make folder"),
            browse: None,
            action: std::rc::Rc::new(|store, name| {
                let mut f = store.folders();
                f.check_name(name, None)?;
                f.create(name);
                store.save_folders(&f).map_err(|e| e.to_string())?;
                Ok(trf!("Made the folder {name}", name = name))
            }),
        };
        self.prompt(prompt, window, cx);
    }

    pub(super) fn rename_folder(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self
            .library
            .folders
            .folder(id)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let id = id.to_owned();
        let prompt = Prompt {
            title: tr("Rename folder"),
            description: tr("Its addons stay where they are.").to_owned(),
            initial,
            placeholder: tr("Folder name"),
            ok: tr("Rename"),
            browse: None,
            action: std::rc::Rc::new(move |store, name| {
                let mut f = store.folders();
                f.check_name(name, Some(&id))?;
                f.rename(&id, name);
                store.save_folders(&f).map_err(|e| e.to_string())?;
                Ok(trf!("Renamed the folder to {name}", name = name))
            }),
        };
        self.prompt(prompt, window, cx);
    }

    /// Removes a folder of the player's own; its addons go back to their own folders.
    pub(super) fn remove_folder(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut f = self.library.folders.clone();
        f.remove(id);
        if let Err(e) = self.store.save_folders(&f) {
            crate::app_log::action("save folders").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
            return;
        }
        self.reload(cx);
    }

    /// A folder's row: click to open or close it, drag it to move its addons.
    pub(super) fn folder_row(
        &self,
        key: &Key,
        count: usize,
        on: usize,
        open: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = self.library.folders.name(key);
        let vanilla = self.vanilla();
        let filled = self
            .library
            .entries
            .iter()
            .any(|e| matches!(e, Entry::Folder(k, m) if k == key && !m.is_empty()));
        let draggable = !vanilla && *key != Key::BuiltIn && filled;
        let own = match key {
            Key::Own(id) if !vanilla => Some(id.clone()),
            _ => None,
        };
        let summary = match count {
            0 => tr("Empty · drag addons here").to_owned(),
            n if vanilla => trn(n, "{count} addon", "{count} addons"),
            n => trf!(
                "{amount} · {on} on",
                amount = trn(n, "{count} addon", "{count} addons"),
                on = on
            ),
        };
        let tag = key.text();
        let actions = own.map(|id| self.folder_actions(id, &tag, cx));
        let toggle = key.clone();
        let row = h_flex()
            .id(SharedString::from(format!("folder-{tag}")))
            .h(px(40.))
            .pl(px(6.))
            .pr(px(12.))
            .gap(px(10.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .child(div().w(px(14.)).flex_shrink_0().when(draggable, |d| {
                d.child(
                    Icon::new(IconName::GripVertical)
                        .size(px(14.))
                        .text_color(p.text_placeholder),
                )
            }))
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(px(14.))
                .text_color(p.text_placeholder),
            )
            .child(
                Icon::new(if open {
                    IconName::FolderOpen
                } else {
                    IconName::Folder
                })
                .size(px(16.))
                .text_color(if *key == Key::BuiltIn {
                    p.accent
                } else {
                    p.text_muted
                }),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .items_baseline()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(summary),
                    ),
            )
            .children(actions)
            .on_click(cx.listener(move |this, _, window, cx| this.toggle_folder(&toggle, window, cx)));
        match draggable {
            true => {
                let dragged = Dragged {
                    item: Dragging::Folder(key.clone()),
                    name: name.into(),
                };
                row.on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
                    .into_any_element()
            }
            false => row.into_any_element(),
        }
    }
}

impl Shell {
    fn folder_actions(&self, id: String, tag: &str, cx: &mut Context<Self>) -> Div {
        let (rename, remove) = (id.clone(), id);
        h_flex()
            .gap(px(2.))
            .child(
                Button::new(SharedString::from(format!("folder-rename-{tag}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::Pencil)
                    .tooltip(tr("Rename folder"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.rename_folder(&rename, window, cx)
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("folder-remove-{tag}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::Trash)
                    .tooltip(tr("Remove folder. Its addons go back to their own folders."))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.remove_folder(&remove, window, cx)
                    })),
            )
    }
}
