use crate::crash_modal::CrashView;
use crate::game::{Install, Launch, Owner};
use crate::game_edited::Edited;
use crate::library::Library;
use crate::notices::Notification;
use crate::play_checks;
use gpui_kit::component::slider::SliderState;
use gpui_kit::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use swofty_core::busy::Busy;
use swofty_core::crash::StartFailure;
use swofty_core::process::GameProcess;
use swofty_core::recovery::Blocked;
use swofty_core::session::{SessionRecord, UiMode};
use swofty_core::store::Store;
use swofty_core::view::View;
use swofty_i18n::{tr, trf};

mod barfit;
mod config;
mod console_window;
mod dialogs;
mod nav;
mod overflow;
mod page;
mod play;
mod play_dialogs;
mod probe;
mod profile_edit;
mod prompt;
mod reload;
mod render;
mod seen;
mod startup;
mod tabs;
mod topbar;

pub(crate) use dialogs::offline_terms;
pub use page::{Page, PlayState};
pub(crate) use prompt::Prompt;
pub(crate) use seen::Seen;
pub(crate) use startup::Prepared;
pub(crate) use topbar::{BAR_HEIGHT, SUB_HEIGHT};

pub struct Shell {
    pub(crate) store: Store,
    pub(crate) page: Page,
    pub(crate) nav: Option<tabs::Cursor>,
    pub(crate) popover: Option<topbar::Popover>,
    pub(crate) popup_timer: bool,
    pub(crate) notch_beside: bool,
    pub(crate) avatar: Option<Arc<Image>>,
    pub(crate) avatar_link: Option<String>,
    pub(crate) root_focus: FocusHandle,
    pub(crate) profiles_open: bool,
    pub(crate) layout_width: f32,
    pub(crate) online: crate::online::Online,
    pub(crate) library: Library,
    pub(crate) open_addon: Option<String>,
    pub(crate) install: Option<Install>,
    pub(crate) copies: Vec<Install>,
    pub(crate) missing_copies: Vec<PathBuf>,
    pub(crate) detected: bool,
    pub(crate) rediscover: bool,
    pub(crate) processes: Vec<GameProcess>,
    pub(crate) owners: Vec<Owner>,
    pub(crate) session: Option<SessionRecord>,
    pub(crate) mp: Option<swofty_core::mpauth::Session>,
    pub(crate) mp_busy: bool,
    pub(crate) ban: Option<swofty_core::mpauth::Ban>,
    pub(crate) mp_toasts: Vec<Notification>,
    pub(crate) mp_page: crate::multiplayer::Multiplayer,
    pub(crate) crash: Option<(PathBuf, Result<String, StartFailure>)>,
    /// The newest crash's diagnosis, and whether its modal is open.
    pub(crate) crash_view: Option<CrashView>,
    pub(crate) crash_open: bool,
    pub(crate) dismissed: Option<PathBuf>,
    pub(crate) steam_saves_once: bool,
    pub(crate) launch: Option<Launch>,
    pub(crate) notice: Option<String>,
    play_anyway: bool,
    play_pending: play_checks::Pending,
    pub(crate) blocked: Option<Blocked>,
    pub(crate) game_edited: Option<Edited>,
    pub(crate) seen: Seen,
    pub(crate) restoring: bool,
    pub(crate) build: Option<Busy>,
    pub(crate) cancelling: bool,
    pub(crate) sliders: HashMap<String, SliderSlot>,
    pub(crate) requests: Option<SystemTime>,
    pub(crate) console: Option<AnyWindowHandle>,
    pub(crate) developer: crate::developer::Developer,
    pub(crate) views: HashMap<String, Result<Arc<View>, String>>,
    pub(crate) views_profile: String,
    pub(crate) view_gen: u64,
    pub(crate) view_gens: HashMap<String, u64>,
    pub(crate) busy: HashSet<String>,
    pub(crate) open_row: Option<String>,
    pub(crate) progress: Option<(f32, String)>,
    pub(crate) tasks: Option<crate::tasks::TaskDialog>,
    pub(crate) moving: Option<crate::data_move::Moving>,
    pub(crate) found_data: Option<crate::found_data::FoundPrompt>,
    pub(crate) dropping: bool,
    pub(crate) vr_profiles: BTreeSet<String>,
    pub(crate) start_store: bool,
    pub(crate) ui_mode: Option<UiMode>,
    pub(crate) ui_scale: Option<u16>,
    pub(crate) ui_scale_slider: Entity<SliderState>,
    pub(crate) ui_scale_drag: Option<u16>,
    pub(crate) _ui_scale_changes: Subscription,
    pub(crate) onboarding: Option<crate::onboarding::Onboarding>,
    /// The check at start for an unanswered ad consent form is running.
    pub(crate) consent_check: bool,
    pub(crate) developer_mode: bool,
    /// `clear_old_session_logs` from config.json: opt-in, old session logs are cleared at start.
    pub(crate) clear_old_session_logs: bool,
    /// `time_trial_boards` from config.json: on unless turned off, uploads completed Time Trial laps.
    pub(crate) time_trial_boards: bool,
    pub(crate) use_profiles: bool,
    /// `time_trial_ingame` from config.json: on unless turned off, shows the boards inside Time Trial.
    pub(crate) time_trial_ingame: bool,
    /// `home_video` from config.json: on unless turned off, shows the latest video card on the F1 25 home screen.
    pub(crate) home_video: bool,
    /// `update_off_addons` from config.json: opt-in, the play bar also updates addons that are off.
    pub(crate) update_off_addons: bool,
    reload_generation: u64,
    slow_logged: bool,
    /// `updates.check` from config.json, read when the library loads.
    pub(crate) update_check: bool,
    pub(crate) updates: crate::update::Updates,
    pub(crate) blocks: crate::blocks::Blocks,
    pub(crate) content_width: f32,
    pub(crate) short: bool,
    pub(crate) ticking: bool,
    painted: Instant,
    pub(crate) dock: crate::download_dock::Dock,
    pub(crate) dock_wake: bool,
    pub(crate) live_n: u64,
    /// The addon the Addons page highlights, and since when.
    pub(crate) highlight: Option<(String, Instant)>,
    pub(crate) addons: crate::addons::AddonsPage,
    pub(crate) saves: crate::career_saves::SavesPage,
    pub(crate) tt: crate::time_trial::TtPage,
    pub(crate) support: crate::support::SupportPage,
    pub(crate) db: crate::db::Db,
    pub(crate) news: crate::news::News,
    pub(crate) news_scroll: crate::news::NewsScroll,
    pub(crate) browser: crate::browser::Browser,
    pub(crate) shaders: Entity<crate::shaders::ShadersView>,
    _shaders_events: Subscription,
    pub(crate) tools: crate::tools::Carousel,
    pub(crate) settings: crate::settings::Settings,
    pub(crate) ads: crate::ads::Ads,
    pub(crate) ad_video: Option<crate::ads::AdVideo>,
    _window_moved: Subscription,
    _console_closed: Subscription,
    _poll: Task<()>,
    _live: Task<()>,
    _updates: Task<()>,
    _catalog: Task<()>,
}

pub struct SliderSlot {
    pub state: Entity<SliderState>,
    pub synced: usize,
    pub _changes: Subscription,
}

impl Shell {
    pub(super) fn mp_sign_in(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.mp_busy || self.mp.is_some() {
            return;
        }
        if let Err(why) = account_guard(self.game_busy()) {
            self.mp_toasts.push(Notification::warning(why));
            cx.notify();
            return;
        }
        let (pending, url) = match swofty_core::mpauth::begin() {
            Ok(v) => v,
            Err(e) => {
                self.mp_toasts
                    .push(Notification::error(trf!("Sign-in failed: {e}", e = e)));
                cx.notify();
                return;
            }
        };
        self.mp_busy = true;
        cx.open_url(&url);
        let dir = self.store.root.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { pending.wait(&dir) })
                .await;
            this.update(cx, |s, cx| {
                s.mp_busy = false;
                match result {
                    Ok(session) => {
                        s.mp_toasts.push(Notification::success(trf!(
                            "Signed in as {name}",
                            name = session.name
                        )));
                        if session.allowed {
                            s.mp_toasts.push(Notification::success(tr(
                                "Your account has been granted access to: Multiplayer",
                            )));
                            swofty_core::multiplayer::client::load_in_background(
                                &s.store.root,
                                &session.token,
                            );
                        }
                        s.mp = Some(session);
                        s.sync_avatar(cx);
                        s.sync_support(cx);
                    }
                    Err(swofty_core::mpauth::SignInError::Banned(ban)) => s.ban = Some(ban),
                    Err(e) => s
                        .mp_toasts
                        .push(Notification::error(trf!("Sign-in failed: {e}", e = e))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn mp_sign_out(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.mp.is_some()
            && let Err(why) = account_guard(self.game_busy())
        {
            self.mp_toasts.push(Notification::warning(why));
            cx.notify();
            return;
        }
        let Some(session) = self.mp.take() else {
            return;
        };
        self.sync_support(cx);
        self.mp_leave(cx);
        let name = session.name.clone();
        let dir = self.store.root.clone();
        cx.background_executor()
            .spawn(async move { swofty_core::mpauth::sign_out(&dir, &session) })
            .detach();
        self.mp_toasts
            .push(Notification::info(trf!("Signed out {name}", name = name)));
        cx.notify();
    }

    pub(super) fn mp_apply_check(
        &mut self,
        checked: swofty_core::mpauth::Refreshed,
        was_allowed: bool,
        cx: &mut Context<Self>,
    ) {
        use swofty_core::mpauth::Refreshed;
        match checked {
            Refreshed::Session(session) => self.mp_apply_refresh(Some(session), was_allowed, cx),
            Refreshed::Gone => self.mp_apply_refresh(None, was_allowed, cx),
            Refreshed::Banned(ban) => {
                self.mp_leave(cx);
                self.mp = None;
                self.sync_support(cx);
                self.ban = Some(ban);
                cx.notify();
            }
        }
    }

    pub(super) fn mp_apply_refresh(
        &mut self,
        refreshed: Option<swofty_core::mpauth::Session>,
        was_allowed: bool,
        cx: &mut Context<Self>,
    ) {
        match refreshed {
            Some(session) => {
                let gained = session.allowed && !was_allowed;
                if gained {
                    swofty_core::multiplayer::client::forget_failure();
                }
                if session.allowed {
                    swofty_core::multiplayer::client::load_in_background(&self.store.root, &session.token);
                }
                self.mp = Some(session);
                self.sync_avatar(cx);
                if gained {
                    self.mp_toasts.push(Notification::success(tr(
                        "Your account has been granted access to: Multiplayer",
                    )));
                }
            }
            None => self.mp = None,
        }
        self.sync_support(cx);
        cx.notify();
    }
}

impl Shell {
    pub(crate) fn sync_avatar(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.mp.as_ref().and_then(|s| s.avatar.clone()) else {
            return;
        };
        if self.avatar_link.as_ref() == Some(&link) {
            return;
        }
        self.avatar_link = Some(link.clone());
        let dir = self.store.root.clone();
        cx.spawn(async move |this, cx| {
            let image = cx
                .background_executor()
                .spawn(async move {
                    crate::avatar::fetch(&dir, &link)
                        .then(|| crate::avatar::load(&dir))
                        .flatten()
                })
                .await;
            this.update(cx, |s, cx| {
                if image.is_some() {
                    s.avatar = image;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}

pub(crate) fn account_guard(game_busy: bool) -> Result<(), &'static str> {
    match game_busy {
        true => Err(swofty_i18n::tr("Close the game to sign in or out")),
        false => Ok(()),
    }
}

#[cfg(test)]
mod account_tests {
    use super::account_guard;

    #[test]
    fn the_account_is_locked_only_while_the_game_runs() {
        assert!(account_guard(false).is_ok());
        assert_eq!(account_guard(true), Err("Close the game to sign in or out"));
    }
}
