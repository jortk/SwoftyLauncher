use crate::notices::{Notification, Notify};
use gpui_kit::*;
use swofty_core::session::UiMode;

use crate::shell::Shell;
use crate::theme::Palette;
use swofty_i18n::tr;

impl Shell {
    pub(crate) fn start_ui_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = self.ui_mode.unwrap_or_default();
        crate::scale::apply(mode, self.ui_scale, window, cx);
        if !self.store.config().needs_onboarding() {
            window.focus(&self.root_focus, cx);
            self.check_ad_consent(window, cx);
            return;
        }
        self.start_onboarding(mode, window, cx);
    }

    pub(crate) fn set_ui_mode(&mut self, mode: UiMode, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.ui_mode = Some(mode);
        self.save_config_toast(&config, window, cx);
        self.ui_mode = config.ui_mode;
        crate::scale::apply(mode, self.ui_scale, window, cx);
        cx.notify();
    }

    pub(crate) fn set_ui_scale(&mut self, percent: Option<u16>, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.ui_scale = percent.map(swofty_core::session::clamp_ui_scale);
        self.save_config_toast(&config, window, cx);
        self.ui_scale = config.ui_scale_percent();
        self.ui_scale_drag = None;
        let at = f32::from(self.ui_scale.unwrap_or(100));
        self.ui_scale_slider
            .update(cx, |s, cx| s.set_value(at, window, cx));
        crate::scale::apply(config.ui_mode_or_default(), self.ui_scale, window, cx);
        cx.notify();
    }

    pub(crate) fn save_config_toast(
        &mut self,
        config: &swofty_core::session::Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(e) = self.store.save_config(config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
    }
}

impl Shell {
    pub(crate) fn scale_rescue(&self, p: Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        (self.ui_scale.is_some() && crate::scale::current() < 0.6).then(|| {
            let f = crate::scale::fixed;
            div()
                .id("ui-scale-rescue")
                .absolute()
                .right(f(12.))
                .bottom(f(12.))
                .px(f(14.))
                .py(f(8.))
                .rounded(f(6.))
                .bg(p.accent)
                .text_color(p.on_accent)
                .text_size(f(14.))
                .cursor_pointer()
                .child(tr("Reset interface scale"))
                .on_click(cx.listener(|this, _, window, cx| this.set_ui_scale(None, window, cx)))
        })
    }
}
