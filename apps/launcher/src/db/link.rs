use gpui_kit::*;
use swofty_core::dbedit::LinkSpec;
use swofty_core::view::{Block, Body};

use crate::shell::{Page, Shell};

const CAREER_EDITOR: &str = "swofty.career-editor";
const CAREER_EDITOR_URL: &str = "https://f1.swofty.net/using/career-editor";

impl Shell {
    pub(crate) fn db_link_state(&self, link: &LinkSpec) -> LinkState {
        match link
            .addon
            .as_deref()
            .and_then(|id| self.library.addons.iter().find(|a| a.manifest.id == id))
        {
            Some(a) if a.enabled => LinkState::Open,
            Some(a) => LinkState::Off(a.manifest.name.clone()),
            None => LinkState::Missing,
        }
    }

    pub(crate) fn db_follow(&mut self, link: &LinkSpec, window: &mut Window, cx: &mut Context<Self>) {
        match self.db_link_state(link) {
            LinkState::Open => self.db_open_section(link, cx),
            LinkState::Off(_) => {
                if let Some(id) = link.addon.clone() {
                    self.db.pending_link = Some(link.clone());
                    self.set_addon_enabled(&id, true, window, cx);
                    if let Some(link) = self.db.pending_link.take()
                        && matches!(self.db_link_state(&link), LinkState::Open)
                    {
                        self.db_open_section(&link, cx);
                    }
                }
            }
            LinkState::Missing => {
                if let Some(url) = &link.url {
                    cx.open_url(url);
                }
            }
        }
    }

    pub(super) fn db_open_section(&mut self, link: &LinkSpec, cx: &mut Context<Self>) {
        let Some(addon) = link.addon.clone() else {
            return;
        };
        let Some(i) = self
            .library
            .sections
            .iter()
            .position(|s| s.addon == addon && link.section.as_ref().is_none_or(|id| *id == s.spec.id))
        else {
            return;
        };
        let section = self.library.sections[i].spec.id.clone();
        self.db.pending_tab = link.tab.clone().map(|t| (addon, section, t));
        self.open_page(Page::Section(i), cx);
    }

    pub(crate) fn career_link() -> LinkSpec {
        LinkSpec {
            label: "Open Career Mode Editor".into(),
            text: String::new(),
            addon: Some(CAREER_EDITOR.into()),
            section: Some("editor".into()),
            tab: None,
            url: Some(CAREER_EDITOR_URL.into()),
        }
    }

    // A link that named a tab: once the section's view is in, switch it to that tab.
    pub(crate) fn db_pending_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((addon, section, tab)) = self.db.pending_tab.clone() else {
            return;
        };
        let Page::Section(i) = self.page else {
            self.db.pending_tab = None;
            return;
        };
        if self
            .library
            .sections
            .get(i)
            .is_none_or(|s| s.addon != addon || s.spec.id != section)
        {
            self.db.pending_tab = None;
            return;
        }
        let view = match self.views.get(&format!("{addon}/{section}")) {
            Some(Ok(v)) => v.clone(),
            Some(Err(_)) => {
                self.db.pending_tab = None;
                return;
            }
            None => return,
        };
        self.db.pending_tab = None;
        let mut blocks = Vec::new();
        if let Body::Blocks(b) = &view.body {
            crate::blocks::walk(b, &mut blocks);
        }
        let Some(tabs) = blocks.into_iter().find_map(|b| match b {
            Block::Tabs(t) if t.items.iter().any(|i| i.id == tab) => Some(t.clone()),
            _ => None,
        }) else {
            return;
        };
        if tabs.active.as_deref() == Some(tab.as_str()) {
            return;
        }
        let control = format!("{addon}/{section}/{}:{tab}", tabs.id);
        let event = swofty_core::view::Event::Tab {
            id: tabs.id.clone(),
            tab,
        };
        self.send(&addon, &section, control, event, window, cx);
    }
}

pub(crate) enum LinkState {
    Open,
    Off(String),
    Missing,
}
