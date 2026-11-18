use crate::notices::{Notification, Notify};
use gpui_kit::*;

use super::Shell;

impl Shell {
    pub(crate) fn restore_console(&mut self, cx: &mut Context<Self>) {
        if self.store.config().console {
            self.console = crate::console::open(self.store.clone(), cx.weak_entity(), cx);
            cx.notify();
        }
    }

    pub(crate) fn set_console(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        match self.console.take() {
            Some(handle) if !on => cx.defer(move |cx| {
                handle.update(cx, |_, window, _| window.remove_window()).ok();
            }),
            Some(handle) => self.console = Some(handle),
            None if on => {
                self.console = crate::console::open(self.store.clone(), cx.weak_entity(), cx);
            }
            None => {}
        }
        let mut config = self.store.config();
        config.console = self.console.is_some();
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        cx.notify();
    }

    pub(super) fn console_closed(&mut self, id: WindowId, cx: &mut Context<Self>) {
        if self.console.is_some_and(|h| h.window_id() == id) {
            self.console = None;
            let mut config = self.store.config();
            config.console = false;
            self.store.save_config(&config).ok();
            cx.notify();
        }
    }
}
