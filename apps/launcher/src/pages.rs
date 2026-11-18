use crate::scale::px;
use crate::shell::{Page, Shell};
use crate::theme::{Palette, ScrollArea};
use crate::ui::{EmptyState, PageHeader, Toggle, row, row_head};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::scroll::Scrollable;
use gpui_kit::component::v_flex;
use gpui_kit::*;
use swofty_i18n::tr;

pub(crate) use crate::ui::{
    TRACK, addon_icon, avatar, badge, chevron, fact, game_art, label, logo, monogram, names, stat, tag, tile,
    track_badge, wordmark,
};

pub(crate) const GUTTER: f32 = crate::ui::tokens::space::GUTTER;

impl Shell {
    pub(crate) fn header(
        &self,
        title: impl Into<SharedString>,
        subtitle: String,
        actions: impl IntoElement,
        _: Palette,
    ) -> PageHeader {
        PageHeader::new(title)
            .subtitle(subtitle)
            .error(self.library.error.clone())
            .actions(actions)
    }

    pub(crate) fn list(
        &self,
        id: impl Into<ElementId>,
        rows: Vec<AnyElement>,
        p: Palette,
    ) -> Scrollable<Div> {
        v_flex()
            .flex_1()
            .min_h_0()
            .scroll_y(id)
            .px(px(GUTTER))
            .pt(px(4.))
            .pb(px(24.))
            .child(
                v_flex()
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border)
                    .overflow_hidden()
                    .children(rows),
            )
    }

    pub(crate) fn empty(
        &self,
        icon: IconName,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        action: impl IntoElement,
        _: Palette,
    ) -> AnyElement {
        EmptyState::new(icon, title)
            .body(body)
            .action(action)
            .into_any_element()
    }

    pub(crate) fn row(&self, first: bool, open: bool, p: Palette) -> Div {
        row(first, open, p)
    }

    pub(crate) fn row_head(&self, id: String, open: bool, p: Palette) -> Stateful<Div> {
        row_head(id, open, p)
    }

    pub(crate) fn toggle(
        &self,
        id: String,
        on: bool,
        cx: &mut Context<Self>,
        change: impl Fn(&mut Self, bool, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        Toggle::new(id, on)
            .disabled(self.vanilla())
            .on_change(cx.listener(move |this, on: &bool, window, cx| change(this, *on, window, cx)))
            .into_any_element()
    }

    pub(crate) fn vanilla(&self) -> bool {
        self.library.profile.is_vanilla()
    }

    // On every page in the Addons group.
    pub(crate) fn install_addon_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().downgrade();
        Button::new("install-addon")
            .outline()
            .icon(IconName::Plus)
            .label(tr("Install addon…"))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                let (file, folder) = (shell.clone(), shell.clone());
                menu.item(
                    PopupMenuItem::new(tr("Choose a file…"))
                        .icon(IconName::File)
                        .on_click(move |_, window, cx| {
                            file.update(cx, |s, cx| s.pick_addons(false, window, cx)).ok();
                        }),
                )
                .item(
                    PopupMenuItem::new(tr("Choose a folder…"))
                        .icon(IconName::FolderOpen)
                        .on_click(move |_, window, cx| {
                            folder.update(cx, |s, cx| s.pick_addons(true, window, cx)).ok();
                        }),
                )
            })
    }

    pub(crate) fn go(&mut self, page: Page, cx: &mut Context<Self>) {
        let moved = self.page != page;
        if moved {
            self.cancel_scroll();
        }
        self.db_page_change(page, cx);
        self.page = page;
        if let Page::Section(i) = page {
            self.fetch_views(Some(i), cx);
        } else if moved {
            self.reload(cx);
        }
        cx.notify();
    }
}
