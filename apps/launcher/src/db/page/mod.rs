mod module;
mod nav;
mod rows;
mod search;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf};

use super::{LinkState, Load, Nav};
use crate::pages::GUTTER;
use crate::shell::Shell;
use crate::theme::Palette;

pub(super) const ROW_H: f32 = 40.;
pub(super) const NAME_W: f32 = 220.;
pub(super) const MOVE_W: f32 = 56.;
const NARROW: f32 = 1100.;

pub(crate) fn notice(
    id: &'static str,
    icon: IconName,
    color: Hsla,
    text: String,
    p: Palette,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .px(px(12.))
        .py(px(8.))
        .gap(px(10.))
        .flex_shrink_0()
        .rounded(px(6.))
        .border_1()
        .border_color(p.border_variant)
        .bg(p.surface)
        .text_size(px(13.))
        .child(Icon::new(icon).size(px(14.)).text_color(color))
        .child(div().flex_1().min_w_0().text_color(p.text).child(text))
}

fn state_pill(state: String, color: Hsla) -> Div {
    h_flex()
        .h(px(22.))
        .px(px(8.))
        .gap(px(5.))
        .flex_shrink_0()
        .rounded(px(11.))
        .bg(color.opacity(0.13))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(div().size(px(6.)).rounded_full().bg(color))
        .child(state)
}

impl Shell {
    fn db_header(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let title = div()
            .text_size(px(18.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(tr("DB Editor"));
        let line = self.db_header_line(p, cx);
        let actions = self.db_header_actions(cx);
        if self.content_width < NARROW {
            return v_flex()
                .px(px(GUTTER))
                .pt(px(22.))
                .pb(px(16.))
                .gap(px(10.))
                .flex_shrink_0()
                .child(
                    h_flex()
                        .gap(px(16.))
                        .items_center()
                        .child(title.flex_1())
                        .child(actions),
                )
                .child(line)
                .into_any_element();
        }
        h_flex()
            .px(px(GUTTER))
            .pt(px(22.))
            .pb(px(16.))
            .gap(px(16.))
            .items_end()
            .flex_shrink_0()
            .child(v_flex().flex_1().min_w_0().gap(px(8.)).child(title).child(line))
            .child(h_flex().gap(px(8.)).pb(px(2.)).child(actions))
            .into_any_element()
    }

    fn db_header_line(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let profile = self.library.profile.name.clone();
        let unsaved = self.db.draft.unsaved();
        let bound = self.db.draft.addon.clone();
        let info = self.db_bound().cloned();
        let on = bound.as_deref().is_some_and(|id| self.db_enabled(id));
        let broken = self.db_locked();
        let target = match (&bound, &info) {
            (Some(_), Some(i)) => format!("{} {}", i.name, i.version),
            (Some(id), None) => id.clone(),
            (None, _) => tr("New addon").to_owned(),
        };
        let place = match &bound {
            Some(_) if broken => Some(tr("Could not be read").to_owned()),
            Some(_) if on => Some(trf!("on in {profile}", profile = profile)),
            Some(_) => Some(trf!("off in {profile}", profile = profile)),
            None => None,
        };
        let (state, color) = match (unsaved, &bound) {
            (0, Some(_)) => (tr("Saved").to_owned(), p.success),
            (0, None) => (tr("No changes yet").to_owned(), p.text_placeholder),
            (k, Some(_)) => (trf!("{k} not saved to the addon", k = k), p.warning),
            (k, None) => (trf!("{k} not saved yet", k = k), p.warning),
        };
        let pill = state_pill(state, color);
        let (turn_on, discard, details, fresh) = self.db_header_buttons(&bound, on, broken, unsaved, cx);
        h_flex()
            .gap(px(10.))
            .min_w_0()
            .child(
                Icon::new(if bound.is_some() {
                    IconName::Pencil
                } else {
                    IconName::PackagePlus
                })
                .size(px(14.))
                .text_color(p.text_muted),
            )
            .child(
                div()
                    .min_w(px((target.chars().count() as f32 * 7.).min(160.)))
                    .flex_shrink(1.)
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(target),
            )
            .children(place.map(|t| {
                div()
                    .text_size(px(13.))
                    .flex_shrink_0()
                    .text_color(if broken { p.error } else { p.text_muted })
                    .child(t)
            }))
            .when(!broken, |d| d.child(pill))
            .children(turn_on)
            .children(discard)
            .children(details)
            .children(fresh)
    }

    fn db_header_buttons(
        &self,
        bound: &Option<String>,
        on: bool,
        broken: bool,
        unsaved: usize,
        cx: &mut Context<Self>,
    ) -> (Option<Button>, Option<Button>, Option<Button>, Option<Button>) {
        let turn_on = bound.clone().filter(|_| !on && !broken).map(|id| {
            Button::new("db-turn-on")
                .ghost()
                .tap_xs()
                .icon(IconName::Power)
                .label(tr("Turn on"))
                .when(self.vanilla(), |b| b.tooltip(tr("Vanilla stays stock")))
                .disabled(self.vanilla())
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_addon_enabled(&id, true, window, cx)),
                )
        });
        let details = bound.as_ref().filter(|_| !broken).map(|_| {
            Button::new("db-details")
                .ghost()
                .tap_xs()
                .icon(IconName::SquarePen)
                .label(tr("Details…"))
                .on_click(cx.listener(|this, _, window, cx| this.db_details(window, cx)))
        });
        let fresh = bound.as_ref().map(|_| {
            Button::new("db-new-draft")
                .ghost()
                .tap_xs()
                .icon(IconName::Plus)
                .label(tr("New addon"))
                .tooltip(tr("Start a new addon from the game's stock values"))
                .on_click(cx.listener(|this, _, window, cx| this.db_new_draft(window, cx)))
        });
        let discard = (unsaved > 0 && !broken).then(|| {
            Button::new("db-discard")
                .ghost()
                .tap_xs()
                .icon(IconName::Undo2)
                .label(tr("Discard changes"))
                .on_click(cx.listener(|this, _, window, cx| this.db_discard(window, cx)))
        });
        (turn_on, discard, details, fresh)
    }

    fn db_header_actions(&self, cx: &mut Context<Self>) -> Div {
        let ready = self.db.catalog().is_some();
        let n = self.db.draft.edits.len();
        let unsaved = self.db.draft.unsaved();
        let bound = self.db.draft.addon.clone();
        let broken = self.db_locked();
        let save = match &bound {
            Some(_) => Button::new("db-save")
                .primary()
                .icon(IconName::Save)
                .label(tr("Save"))
                .disabled(!ready || broken || unsaved == 0),
            None => Button::new("db-save")
                .primary()
                .icon(IconName::PackagePlus)
                .label(tr("Save as Addon"))
                .disabled(!ready || n == 0),
        }
        .on_click(cx.listener(|this, _, window, cx| this.db_save(window, cx)));
        h_flex()
            .gap(px(8.))
            .child(
                Button::new("db-import")
                    .outline()
                    .icon(IconName::FileDown)
                    .label(tr("Import…"))
                    .loading(self.db.importing)
                    .disabled(!ready || self.db.importing)
                    .on_click(cx.listener(|this, _, window, cx| this.db_import(window, cx))),
            )
            .child(self.db_export_menu(cx))
            .child(
                Button::new("db-changes")
                    .outline()
                    .icon(IconName::ListChecks)
                    .label(trf!("All changes ({n})", n = n))
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.db_go(Nav::Changes, cx))),
            )
            .child(save)
    }

    fn db_career_strip(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let career = Self::career_link();
        let career_label = match self.db_link_state(&career) {
            LinkState::Open => tr("Open Career Mode Editor").to_owned(),
            LinkState::Off(_) => tr("Turn on Career Mode Editor").to_owned(),
            LinkState::Missing => tr("Get Career Mode Editor").to_owned(),
        };
        notice(
            "db-career-notice",
            IconName::Info,
            p.accent,
            tr("Changes apply when you press Play, to careers you start afterwards. To edit a career in progress, use Career Mode Editor.")
                .into(),
            p,
        )
        .mx(px(GUTTER))
        .mb(px(10.))
        .child(
            Button::new("db-career-link")
                .ghost()
                .tap_sm()
                .label(career_label)
                .when(matches!(self.db_link_state(&career), LinkState::Missing), |b| {
                    b.icon(IconName::ExternalLink)
                })
                .disabled(self.vanilla() && matches!(self.db_link_state(&career), LinkState::Off(_)))
                .on_click(cx.listener(move |this, _, window, cx| this.db_follow(&career, window, cx))),
        )
    }

    fn db_load_state(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.db.load {
            Load::Ready(_) => self.db_body(p, cx),
            Load::Loading => self.db_loading(tr("Reading the game database…"), p),
            Load::Failed(e) => self.empty(
                IconName::CircleAlert,
                tr("The database could not be read"),
                e.clone(),
                Button::new("db-retry")
                    .outline()
                    .icon(IconName::RefreshCw)
                    .label(tr("Try again"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.db.load = Load::Idle;
                        this.db.game = None;
                        cx.notify();
                    })),
                p,
            ),
            Load::Idle if !self.detected => self.db_loading(tr("Looking for F1 25…"), p),
            Load::Idle => self.empty(
                IconName::Database,
                tr("F1 25 not found"),
                tr("The DB Editor reads the game's database. Show the launcher where F1 25 is installed."),
                Button::new("db-locate")
                    .outline()
                    .icon(IconName::FolderOpen)
                    .label(tr("Locate F1 25…"))
                    .on_click(cx.listener(|this, _, window, cx| this.locate_game(window, cx))),
                p,
            ),
        }
    }

    fn db_page_keys(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let ctrl = k.modifiers.control || k.modifiers.platform;
        let typing = self
            .db
            .focus
            .as_ref()
            .is_some_and(|f| !f.is_focused(window) && f.contains_focused(window, cx));
        if (ctrl && k.key == "f") || (k.key == "/" && !typing) {
            self.db_focus_search(window, cx);
            cx.stop_propagation();
        } else if k.key == "escape" && self.db.hits_open {
            self.db.hits_open = false;
            cx.notify();
        }
    }

    pub(crate) fn db_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let strip = self.db_career_strip(p, cx);
        let vanilla = self.vanilla().then(|| {
            notice(
                "db-vanilla-notice",
                IconName::Lock,
                p.warning,
                tr("Vanilla stays stock. You can still edit and save addons here; turn them on in another profile.")
                    .into(),
                p,
            )
            .mx(px(GUTTER))
            .mb(px(10.))
        });
        let broken = self.db.addon_error.clone().map(|e| {
            notice(
                "db-addon-error",
                IconName::CircleAlert,
                p.error,
                trf!("This addon could not be read, so it can't be edited: {e}", e = e),
                p,
            )
            .mx(px(GUTTER))
            .mb(px(10.))
            .child(
                Button::new("db-addon-retry")
                    .outline()
                    .tap_sm()
                    .icon(IconName::RefreshCw)
                    .label(tr("Try again"))
                    .on_click(cx.listener(|this, _, _, cx| this.db_retry(cx))),
            )
        });
        let body = self.db_load_state(p, cx);
        let focus = self.db.focus.clone();
        v_flex()
            .id("db-page")
            .size_full()
            .when_some(focus, |d, f| d.track_focus(&f))
            .capture_key_down(cx.listener(Self::db_page_keys))
            .child(self.db_header(p, cx))
            .child(strip)
            .children(vanilla)
            .children(broken)
            .children(self.db.save_error.clone().map(|e| {
                notice("db-save-error", IconName::CircleAlert, p.error, e, p)
                    .mx(px(GUTTER))
                    .mb(px(10.))
            }))
            .children(self.db.newer.then(|| {
                notice(
                    "db-newer-draft",
                    IconName::TriangleAlert,
                    p.warning,
                    tr("Another launcher window saved a newer draft. Changes here are not saved until you pick one.")
                        .into(),
                    p,
                )
                .mx(px(GUTTER))
                .mb(px(10.))
                .child(
                    Button::new("db-take-newer")
                        .outline()
                        .tap_sm()
                        .label(tr("Load the newer draft"))
                        .on_click(cx.listener(|this, _, _, cx| this.db_take_newer(cx))),
                )
                .child(
                    Button::new("db-keep-mine")
                        .ghost()
                        .tap_sm()
                        .label(tr("Keep this window's changes"))
                        .on_click(cx.listener(|this, _, _, cx| this.db_keep_mine(cx))),
                )
            }))
            .child(body)
            .into_any_element()
    }

    fn db_body(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let main = match self.db.nav.clone() {
            Nav::Module(id) => self.db_module(&id, p, cx),
            Nav::Raw(node) => self.db_raw(node, p, cx),
            Nav::Unnamed => self.db_unnamed_view(p, cx),
            Nav::Changes => self.db_changes(p, cx),
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .child(self.db_search_bar(p, cx))
            .child(
                h_flex()
                    .id("db-body")
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .px(px(GUTTER))
                    .pb(px(20.))
                    .gap(px(20.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.db.hits_open {
                                this.db.hits_open = false;
                                cx.notify();
                            }
                        }),
                    )
                    .child(self.db_nav(p, cx))
                    .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(main)),
            )
            .into_any_element()
    }
}
