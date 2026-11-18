use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use swofty_core::support::{BIO_MAX, Claimed, Goal, Kind, Me, Supporter, dollars_to_cents};
use swofty_core::timetrial::api::Api;
use swofty_i18n::tr;

use crate::shell::Shell;
use crate::time_trial::Load;

pub(super) struct Inputs {
    pub bio: Entity<InputState>,
    pub amount: Entity<InputState>,
    pub email: Entity<InputState>,
    pub payment: Entity<InputState>,
    _subs: Vec<Subscription>,
}

pub(crate) struct SupportPage {
    pub goal: Load<Goal>,
    pub supporters: Load<Vec<Supporter>>,
    pub me: Load<Me>,
    pub(super) inputs: Option<Inputs>,
    pub bio_synced: bool,
    pub mode: String,
    pub busy: bool,
    pub notice: Option<(bool, String)>,
    pub claimed: Option<Claimed>,
    pub claim_open: bool,
}

impl Default for SupportPage {
    fn default() -> Self {
        Self {
            goal: Load::Idle,
            supporters: Load::Idle,
            me: Load::Idle,
            inputs: None,
            bio_synced: false,
            mode: "discord".to_owned(),
            busy: false,
            notice: None,
            claimed: None,
            claim_open: false,
        }
    }
}

impl Shell {
    fn support_api(&self) -> Api {
        Api::new(self.mp.as_ref().map(|s| s.token.as_str()).unwrap_or_default())
    }

    pub(super) fn support_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (
        Entity<InputState>,
        Entity<InputState>,
        Entity<InputState>,
        Entity<InputState>,
    ) {
        if self.support.inputs.is_none() {
            let make = |hint: &'static str, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| InputState::new(window, cx).placeholder(tr(hint)))
            };
            let bio = make("One sentence about you (shown next to your name)", window, cx);
            let amount = make("Amount, e.g. 20", window, cx);
            let email = make("Email used on Stripe", window, cx);
            let payment = make("Invoice or payment ID", window, cx);
            let subs = [&bio, &amount, &email, &payment]
                .into_iter()
                .map(|state| {
                    cx.subscribe(state, |_, _, ev: &InputEvent, cx| {
                        if matches!(ev, InputEvent::Change) {
                            cx.notify();
                        }
                    })
                })
                .collect();
            self.support.inputs = Some(Inputs {
                bio,
                amount,
                email,
                payment,
                _subs: subs,
            });
        }
        let i = self.support.inputs.as_ref().expect("made above");
        (
            i.bio.clone(),
            i.amount.clone(),
            i.email.clone(),
            i.payment.clone(),
        )
    }

    pub(crate) fn support_open(&mut self, cx: &mut Context<Self>) {
        if matches!(self.support.goal, Load::Idle | Load::Failed(_))
            || (self.mp.is_some() && matches!(self.support.me, Load::Idle | Load::Failed(_)))
        {
            self.support_reload(cx);
        }
    }

    pub(crate) fn support_fetch_me(&mut self, cx: &mut Context<Self>) {
        let api = self.support_api();
        self.support.me = Load::Loading;
        cx.spawn(async move |this, cx| {
            let me = cx
                .background_executor()
                .spawn(async move { api.support_me() })
                .await;
            this.update(cx, |s, cx| {
                if s.mp.is_some() {
                    s.support.me = load(me);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn support_reload(&mut self, cx: &mut Context<Self>) {
        let api = self.support_api();
        let signed_in = self.mp.is_some();
        self.support.goal = Load::Loading;
        self.support.supporters = Load::Loading;
        self.support.me = if signed_in { Load::Loading } else { Load::Idle };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (goal, supporters, me) = cx
                .background_executor()
                .spawn(async move {
                    (
                        api.support_goal(),
                        api.support_supporters(),
                        signed_in.then(|| api.support_me()),
                    )
                })
                .await;
            this.update(cx, |s, cx| {
                let faces: Vec<String> = supporters
                    .iter()
                    .flatten()
                    .filter_map(|x| x.display.avatar_url.clone())
                    .collect();
                s.support.goal = load(goal);
                s.support.supporters = load(supporters);
                s.support.me = me.map(load).unwrap_or(Load::Idle);
                if let Load::Ready(me) = &s.support.me {
                    s.support.mode = me.display_mode.clone().unwrap_or_else(|| "discord".into());
                }
                s.support.bio_synced = false;
                s.load_faces(faces, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn support_checkout(&mut self, kind: Kind, amount: Option<String>, cx: &mut Context<Self>) {
        let cents = match (kind, amount) {
            (Kind::OneTime, text) => match text.as_deref().and_then(dollars_to_cents) {
                Some(c) => Some(c),
                None => {
                    self.support.notice =
                        Some((false, tr("Enter an amount in dollars, like 20.").to_owned()));
                    cx.notify();
                    return;
                }
            },
            _ => None,
        };
        if self.support.busy || (kind != Kind::OneTime && self.mp.is_none()) {
            return;
        }
        let api = self.support_api();
        self.support.busy = true;
        self.support.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let url = cx
                .background_executor()
                .spawn(async move { api.support_checkout(kind, cents) })
                .await;
            this.update(cx, |s, cx| {
                s.support.busy = false;
                match url {
                    Ok(url) => cx.open_url(&url),
                    Err(e) => s.support.notice = Some((false, e.message)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn support_claim(&mut self, email: String, payment: String, cx: &mut Context<Self>) {
        let Some(api) = self
            .mp
            .is_some()
            .then(|| self.support_api())
            .filter(|_| !self.support.busy)
        else {
            return;
        };
        self.support.busy = true;
        self.support.notice = None;
        self.support.claimed = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_executor()
                .spawn(async move { api.support_claim(email.trim(), payment.trim()) })
                .await;
            this.update(cx, |s, cx| {
                s.support.busy = false;
                match got {
                    Ok(claimed) => {
                        s.support.claimed = Some(claimed);
                        s.support_reload(cx);
                    }
                    Err(e) => s.support.notice = Some((false, e.message)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn support_save(&mut self, bio: String, cx: &mut Context<Self>) {
        let Some(api) = self
            .mp
            .is_some()
            .then(|| self.support_api())
            .filter(|_| !self.support.busy)
        else {
            return;
        };
        let bio: String = bio.trim().chars().take(BIO_MAX).collect();
        let mode = self.support.mode.clone();
        self.support.busy = true;
        self.support.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_executor()
                .spawn(async move { api.support_update(Some(&bio), Some(&mode)) })
                .await;
            this.update(cx, |s, cx| {
                s.support.busy = false;
                s.support.notice = Some(match got {
                    Ok(()) => (true, tr("Saved.").to_owned()),
                    Err(e) => (false, e.message),
                });
                if got_ok(&s.support.notice) {
                    s.support_reload(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn got_ok(notice: &Option<(bool, String)>) -> bool {
    notice.as_ref().is_some_and(|(ok, _)| *ok)
}

fn load<T>(got: Result<T, swofty_core::timetrial::api::ApiError>) -> Load<T> {
    match got {
        Ok(v) => Load::Ready(v),
        Err(e) => Load::Failed(e.message),
    }
}
