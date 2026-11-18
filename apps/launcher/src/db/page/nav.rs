use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{LinkSpec, Module};
use swofty_i18n::{tr, trf};

use crate::db::{LinkState, Nav, icon_named};
use crate::pages::label;
use crate::shell::Shell;
use crate::theme::{Palette, SCROLLBAR_GAP, ScrollArea};

pub(crate) fn nav_row(
    id: impl Into<SharedString>,
    icon: IconName,
    title: impl Into<SharedString>,
    count: Option<String>,
    active: bool,
    p: Palette,
) -> Stateful<Div> {
    h_flex()
        .id(ElementId::Name(id.into()))
        .h(px(32.))
        .px(px(10.))
        .gap(px(9.))
        .flex_shrink_0()
        .rounded(px(6.))
        .cursor_pointer()
        .text_size(px(13.))
        .map(|d| {
            if active {
                d.bg(p.selected)
                    .text_color(p.text)
                    .font_weight(FontWeight::MEDIUM)
            } else {
                d.text_color(p.text_muted)
                    .hover(move |s| s.bg(p.hover).text_color(p.text))
            }
        })
        .child(
            Icon::new(icon)
                .size(px(15.))
                .text_color(if active { p.accent } else { p.text_placeholder }),
        )
        .child(div().flex_1().min_w_0().truncate().child(title.into()))
        .children(count.map(|n| div().text_size(px(12.)).text_color(p.text_placeholder).child(n)))
}

impl Shell {
    pub(super) fn db_nav(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let mut groups: Vec<(&str, Vec<&Module>)> = Vec::new();
        for m in self.db.modules {
            match groups.iter_mut().find(|(g, _)| *g == m.group) {
                Some((_, list)) => list.push(m),
                None => groups.push((&m.group, vec![m])),
            }
        }
        let nav = &self.db.nav;
        let catalog = self.db.catalog().cloned();
        let item = |id: String,
                    icon: IconName,
                    title: String,
                    count: Option<String>,
                    to: Nav,
                    cx: &mut Context<Self>| {
            let active = match (&to, nav) {
                (Nav::Raw(_), Nav::Raw(_)) => true,
                (a, b) => a == b,
            };
            nav_row(id, icon, title, count, active, p)
                .on_click(cx.listener(move |this, _, _, cx| this.db_go(to.clone(), cx)))
                .into_any_element()
        };
        let mut sections: Vec<AnyElement> = groups
            .into_iter()
            .map(|(g, list)| {
                let rows: Vec<AnyElement> = list
                    .into_iter()
                    .map(|m| {
                        item(
                            format!("db-nav-{}", m.id),
                            icon_named(&m.icon),
                            m.title.clone(),
                            None,
                            Nav::Module(m.id.clone()),
                            cx,
                        )
                    })
                    .collect();
                v_flex()
                    .gap(px(2.))
                    .child(label(g.to_uppercase(), p).px(px(10.)).pb(px(4.)))
                    .children(rows)
                    .into_any_element()
            })
            .collect();
        let root = catalog.as_ref().map_or(0, |c| c.root());
        let unnamed = catalog.as_ref().map(|c| c.records().saturating_sub(c.named()));
        let advanced: Vec<AnyElement> = vec![
            item(
                "db-nav-raw".into(),
                IconName::FolderTree,
                tr("All tables").into(),
                None,
                Nav::Raw(root),
                cx,
            ),
            item(
                "db-nav-unnamed".into(),
                IconName::Binary,
                tr("Unnamed keys").into(),
                unnamed.map(|n| n.to_string()),
                Nav::Unnamed,
                cx,
            ),
            item(
                "db-nav-changes".into(),
                IconName::ListChecks,
                tr("Changes").into(),
                Some(self.db.draft.edits.len().to_string()),
                Nav::Changes,
                cx,
            ),
        ];
        sections.push(
            v_flex()
                .gap(px(2.))
                .child(label(tr("ADVANCED"), p).px(px(10.)).pb(px(4.)))
                .children(advanced)
                .into_any_element(),
        );
        v_flex()
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .scroll_y("db-nav")
            .gap(px(18.))
            .pr(px(SCROLLBAR_GAP))
            .pb(px(12.))
            .children(sections)
    }

    pub(crate) fn db_link_button(&self, i: usize, link: &LinkSpec, cx: &mut Context<Self>) -> Button {
        let state = self.db_link_state(link);
        let label = match &state {
            LinkState::Open | LinkState::Missing => link.label.clone(),
            LinkState::Off(name) => trf!("Turn on {name}", name = name),
        };
        let link = link.clone();
        Button::new(SharedString::from(format!("db-link-{i}")))
            .outline()
            .tap_sm()
            .label(label)
            .tooltip(link.text.clone())
            .map(|b| match state {
                LinkState::Missing => b.icon(IconName::ExternalLink),
                LinkState::Off(_) => b.icon(IconName::Power).disabled(self.vanilla()),
                LinkState::Open => b.icon(IconName::ArrowUpRight),
            })
            .on_click(cx.listener(move |this, _, window, cx| this.db_follow(&link, window, cx)))
    }
}
