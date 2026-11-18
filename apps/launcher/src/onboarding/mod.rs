mod art;
mod steps;
mod terms;
mod welcome;

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex};
use gpui_kit::*;
use swofty_core::session::{ONBOARDING_VERSION, UiMode};
use swofty_i18n::tr;

use crate::ads::Unit;
use crate::notices::{Notification, Notify};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;
use swofty_ads::Consent;

const BUTTON_HEIGHT: f32 = 48.;

/// How long the consent form may take to load before the player can move on without it.
const CONSENT_WAIT: Duration = Duration::from_secs(25);

/// How long the check at start waits for the consent page to say whether it needs an answer.
const CONSENT_CHECK: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Step {
    Welcome,
    Language,
    Display,
    Terms,
    Ads,
    Done,
}

impl Step {
    const ALL: [Step; 6] = [
        Step::Welcome,
        Step::Language,
        Step::Display,
        Step::Terms,
        Step::Ads,
        Step::Done,
    ];

    fn order(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

pub(crate) struct Onboarding {
    step: Step,
    mode: UiMode,
    /// Only the ad privacy choices: from Settings, or at start when no answer is saved.
    privacy_only: bool,
    /// Opens the form over a saved answer, so it can be changed.
    reopen: bool,
    consent_since: Instant,
    pub(crate) focus: FocusHandle,
}

impl Onboarding {
    pub(crate) fn new(mode: UiMode, cx: &mut App) -> Self {
        Self {
            step: Step::Welcome,
            mode,
            privacy_only: false,
            reopen: false,
            consent_since: Instant::now(),
            focus: cx.focus_handle(),
        }
    }
}

impl Shell {
    /// Opens onboarding when this version of it hasn't been seen.
    pub(crate) fn start_onboarding(&mut self, mode: UiMode, window: &mut Window, cx: &mut Context<Self>) {
        let wizard = Onboarding::new(mode, cx);
        window.focus(&wizard.focus, cx);
        self.onboarding = Some(wizard);
        self.watch_consent(cx);
    }

    /// The ad privacy choices on their own, from Settings.
    pub(crate) fn open_ad_privacy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ads.restart_consent();
        let mut wizard = Onboarding::new(self.ui_mode.unwrap_or_default(), cx);
        wizard.step = Step::Ads;
        wizard.privacy_only = true;
        wizard.reopen = true;
        window.focus(&wizard.focus, cx);
        self.onboarding = Some(wizard);
        self.watch_consent(cx);
        cx.notify();
    }

    /// Loads the consent page out of sight at start and opens the ad privacy
    /// choices when it still needs an answer, whoever the player is.
    pub(crate) fn check_ad_consent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ads.on() {
            return;
        }
        self.consent_check = true;
        let started = Instant::now();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                let checking = this
                    .update_in(cx, |s, window, cx| s.consent_check_step(started, window, cx))
                    .unwrap_or(false);
                if !checking {
                    break;
                }
            }
        })
        .detach();
    }

    fn consent_check_step(&mut self, started: Instant, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.onboarding.is_some() {
            self.consent_check = false;
            return false;
        }
        self.ads.poll_consent();
        match self.ads.consent() {
            Some(Consent::Asking) => {
                self.consent_check = false;
                let mut wizard = Onboarding::new(self.ui_mode.unwrap_or_default(), cx);
                wizard.step = Step::Ads;
                wizard.privacy_only = true;
                window.focus(&wizard.focus, cx);
                self.onboarding = Some(wizard);
                self.watch_consent(cx);
            }
            None if started.elapsed() < CONSENT_CHECK => return true,
            _ => {
                self.consent_check = false;
                self.ads.close(Unit::Consent);
            }
        }
        cx.notify();
        false
    }

    /// The consent page the check at start loads, out of sight.
    pub(crate) fn consent_probe(&self, p: Palette) -> Option<impl IntoElement> {
        if !self.consent_check || self.onboarding.is_some() {
            return None;
        }
        terms::preload(&self.ads, p)
    }

    /// Reads the consent page's reports while onboarding is open.
    fn watch_consent(&self, cx: &mut Context<Self>) {
        if !self.ads.on() {
            return;
        }
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                let open = this
                    .update(cx, |s, cx| {
                        let waiting = s.onboarding.as_ref().is_some_and(|w| w.step == Step::Ads)
                            && s.ads.consent().is_none();
                        if s.ads.poll_consent() || waiting {
                            cx.notify();
                        }
                        s.onboarding.is_some()
                    })
                    .unwrap_or(false);
                if !open {
                    break;
                }
            }
        })
        .detach();
    }

    /// Whether the ads step is part of onboarding: for everyone while ads are on.
    fn asks_ad_consent(&self) -> bool {
        self.ads.on()
    }

    fn onboarding_steps(&self) -> Vec<Step> {
        match &self.onboarding {
            Some(w) if w.privacy_only => vec![Step::Ads],
            Some(_) => Step::ALL
                .into_iter()
                .filter(|s| *s != Step::Ads || self.asks_ad_consent())
                .collect(),
            None => Vec::new(),
        }
    }

    fn onboarding_next(&self, step: Step) -> Option<Step> {
        self.onboarding_steps()
            .into_iter()
            .find(|s| s.order() > step.order())
    }

    fn onboarding_previous(&self, step: Step) -> Option<Step> {
        self.onboarding_steps()
            .into_iter()
            .rev()
            .find(|s| s.order() < step.order())
    }

    /// The consent form answered, nothing to ask, or given up on after [`CONSENT_WAIT`].
    fn consent_settled(&self) -> bool {
        match self.ads.consent() {
            Some(consent) => consent.settled(),
            None => self
                .onboarding
                .as_ref()
                .is_some_and(|w| w.consent_since.elapsed() >= CONSENT_WAIT),
        }
    }

    fn onboarding_can_forward(&self, step: Step) -> bool {
        step != Step::Ads || self.consent_settled()
    }

    fn forward_label(&self, step: Step) -> &'static str {
        match step {
            _ if self.onboarding.as_ref().is_some_and(|w| w.privacy_only) => tr("Done"),
            Step::Welcome => tr("Get started"),
            Step::Terms | Step::Ads => tr("Accept"),
            Step::Done => tr("Start"),
            Step::Language | Step::Display => tr("Next"),
        }
    }

    fn onboarding_go(&mut self, step: Step, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.onboarding {
            wizard.step = step;
        }
        cx.notify();
    }

    fn onboarding_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(step) = self.onboarding.as_ref().map(|w| w.step) else {
            return;
        };
        if !self.onboarding_can_forward(step) {
            return;
        }
        if step == Step::Terms && !self.accept_terms(window, cx) {
            return;
        }
        match self.onboarding_next(step) {
            Some(next) => self.onboarding_go(next, cx),
            None => self.onboarding_finish(window, cx),
        }
    }

    fn accept_terms(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match swofty_core::offline::record_accept(&self.store.root) {
            Ok(_) => true,
            Err(e) => {
                crate::app_log::action("accept the terms").failed(&e);
                window.notify(Notification::error(e.to_string()), cx);
                false
            }
        }
    }

    fn onboarding_back(&mut self, cx: &mut Context<Self>) {
        let previous = self
            .onboarding
            .as_ref()
            .and_then(|w| self.onboarding_previous(w.step));
        if let Some(previous) = previous {
            self.onboarding_go(previous, cx);
        }
    }

    /// Skipping goes straight to the terms, which can't be skipped.
    fn onboarding_skip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wizard) = &self.onboarding else {
            return;
        };
        if wizard.privacy_only || wizard.step == Step::Done {
            self.onboarding_finish(window, cx);
        } else if wizard.step.order() < Step::Terms.order() {
            self.onboarding_go(Step::Terms, cx);
        }
    }

    fn onboarding_pick_mode(&mut self, mode: UiMode, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.onboarding {
            wizard.mode = mode;
        }
        self.set_ui_mode(mode, window, cx);
    }

    fn onboarding_pick_language(&mut self, offset: isize, window: &mut Window, cx: &mut Context<Self>) {
        let languages = swofty_i18n::languages();
        let current = languages
            .iter()
            .position(|l| l.code == swofty_i18n::language())
            .unwrap_or(0);
        let target = current.saturating_add_signed(offset).min(languages.len() - 1);
        self.set_language(languages[target].code, window, cx);
    }

    pub(crate) fn onboarding_finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wizard) = self.onboarding.take() else {
            return;
        };
        self.ads.close(Unit::Consent);
        window.focus(&self.root_focus, cx);
        if wizard.privacy_only {
            cx.notify();
            return;
        }
        let mut config = self.store.config();
        config.onboarding = Some(ONBOARDING_VERSION);
        config.ui_mode = Some(wizard.mode);
        self.save_config_toast(&config, window, cx);
        self.ui_mode = config.ui_mode;
        cx.notify();
    }

    fn onboarding_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((step, root_focused)) = self
            .onboarding
            .as_ref()
            .map(|w| (w.step, w.focus.is_focused(window)))
        else {
            return;
        };
        match (key, step) {
            ("escape", _) => self.onboarding_skip(window, cx),
            ("enter", _) if root_focused => self.onboarding_forward(window, cx),
            ("down", Step::Language) => self.onboarding_pick_language(1, window, cx),
            ("up", Step::Language) => self.onboarding_pick_language(-1, window, cx),
            ("left", Step::Display) => self.onboarding_pick_mode(UiMode::Pc, window, cx),
            ("right", Step::Display) => self.onboarding_pick_mode(UiMode::Tv, window, cx),
            ("left", _) => self.onboarding_back(cx),
            ("right", _) => self.onboarding_forward(window, cx),
            _ => {}
        }
    }

    pub(crate) fn onboarding_view(&self, p: Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let wizard = self.onboarding.as_ref()?;
        let step = wizard.step;
        let body = match step {
            Step::Welcome => welcome::view(p),
            Step::Language => self.language_step(p, cx),
            Step::Display => self.display_step(wizard.mode, p, cx),
            Step::Terms => terms::eula(p),
            Step::Ads => self.ads_step(wizard.privacy_only, wizard.reopen, p),
            Step::Done => steps::done(p),
        };
        let stage = div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .child(steps::enter(step, body));
        let early = step.order() < Step::Ads.order() && self.asks_ad_consent();
        Some(
            crate::ui::takeover("onboarding", p)
                .track_focus(&wizard.focus)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    this.onboarding_key(ev.keystroke.key.as_str(), window, cx);
                }))
                .child(welcome::backdrop(step, p))
                .child(crate::ui::title_bar(p))
                .child(stage)
                .child(self.footer(step, p, cx))
                .children(early.then(|| terms::preload(&self.ads, p)).flatten()),
        )
    }

    fn footer(&self, step: Step, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy_only = self.onboarding.as_ref().is_some_and(|w| w.privacy_only);
        let back = match self.onboarding_previous(step) {
            Some(_) => Button::new("onboarding-back")
                .outline()
                .large()
                .h(px(BUTTON_HEIGHT))
                .px(px(24.))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_center()
                        .child(Icon::new(IconName::ArrowLeft).size(px(18.)))
                        .child(div().text_size(px(16.)).child(tr("Back"))),
                )
                .on_click(cx.listener(|this, _, _, cx| this.onboarding_back(cx)))
                .into_any_element(),
            None if step.order() < Step::Terms.order() || privacy_only => Button::new("onboarding-skip")
                .ghost()
                .large()
                .h(px(BUTTON_HEIGHT))
                .px(px(20.))
                .child(
                    div()
                        .text_size(px(15.))
                        .text_color(p.text_muted)
                        .child(if privacy_only { tr("Close") } else { tr("Skip") }),
                )
                .on_click(cx.listener(|this, _, window, cx| this.onboarding_skip(window, cx)))
                .into_any_element(),
            None => div().into_any_element(),
        };
        let forward = Button::new("onboarding-next")
            .primary()
            .large()
            .h(px(BUTTON_HEIGHT))
            .px(px(28.))
            .disabled(!self.onboarding_can_forward(step))
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.forward_label(step)),
                    )
                    .child(Icon::new(IconName::ArrowRight).size(px(18.))),
            )
            .on_click(cx.listener(|this, _, window, cx| this.onboarding_forward(window, cx)));
        let steps = self.onboarding_steps();
        h_flex()
            .w_full()
            .px(px(40.))
            .pb(px(32.))
            .items_center()
            .child(div().flex_1().flex().justify_start().child(back))
            .children((steps.len() > 1).then(|| steps::dots(step, &steps, p)))
            .child(div().flex_1().flex().justify_end().child(forward))
    }
}
