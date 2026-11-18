use gpui_kit::assets::IconName;
use swofty_i18n::tr;

use super::Page;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Home,
    Library,
    Discover,
    Online,
    Support,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Home, Tab::Library, Tab::Discover, Tab::Online, Tab::Support];

    pub fn label(self) -> &'static str {
        match self {
            Self::Home => tr("Home"),
            Self::Library => tr("Library"),
            Self::Discover => tr("Discover"),
            Self::Online => tr("Online"),
            Self::Support => tr("Support"),
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::Newspaper,
            Self::Library => IconName::Puzzle,
            Self::Discover => IconName::PackageSearch,
            Self::Online => IconName::Trophy,
            Self::Support => IconName::Heart,
        }
    }

    pub fn tint(self, p: &crate::theme::Palette) -> gpui_kit::Hsla {
        match self {
            Self::Home => p.tab_home,
            Self::Library => p.tab_library,
            Self::Discover => p.tab_discover,
            Self::Online => p.tab_trial,
            Self::Support => p.tab_support,
        }
    }

    pub fn home(self) -> Page {
        match self {
            Self::Home => Page::News,
            Self::Library => Page::Addons,
            Self::Discover => Page::Browser,
            Self::Online => Page::Online,
            Self::Support => Page::Support,
        }
    }
}

pub const ACTIONS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Notifications,
    Settings,
    Account,
}

impl Action {
    pub const ALL: [Action; ACTIONS] = [Action::Notifications, Action::Settings, Action::Account];
}

impl Page {
    pub fn tab(self) -> Option<Tab> {
        match self {
            Self::News => Some(Tab::Home),
            Self::Browser => Some(Tab::Discover),
            Self::Online | Self::Multiplayer | Self::Moderation => Some(Tab::Online),
            Self::Support => Some(Tab::Support),
            Self::Settings | Self::Tools => None,
            Self::Addons
            | Self::Shaders
            | Self::Downloads
            | Self::Importer
            | Self::Careers
            | Self::Database
            | Self::Section(_) => Some(Tab::Library),
        }
    }

    pub fn sub_label(self) -> &'static str {
        match self {
            Self::Addons => tr("Addons"),
            Self::Online => tr("Time Trial"),
            _ => self.nav_label(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row {
    Tabs,
    Links,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Stay,
    IntoPage,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cursor {
    pub row: Row,
    pub col: usize,
}

impl Cursor {
    pub fn on_tab(tab: Option<Tab>) -> Self {
        let col = tab
            .and_then(|t| Tab::ALL.iter().position(|x| *x == t))
            .unwrap_or(0);
        Self { row: Row::Tabs, col }
    }

    pub fn apply(&mut self, key: Key, tabs: usize, links: usize) -> Step {
        let len = match self.row {
            Row::Tabs => tabs,
            Row::Links => links,
        };
        match key {
            Key::Left => self.col = self.col.saturating_sub(1),
            Key::Right => self.col = (self.col + 1).min(len.saturating_sub(1)),
            Key::Up => {
                if self.row == Row::Links {
                    self.row = Row::Tabs;
                    self.col = self.col.min(tabs.saturating_sub(1));
                }
            }
            Key::Down => match self.row {
                Row::Tabs if links > 0 => {
                    self.row = Row::Links;
                    self.col = 0;
                }
                _ => return Step::IntoPage,
            },
        }
        Step::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOP: usize = Tab::ALL.len() + ACTIONS;

    #[test]
    fn every_page_has_a_tab_or_the_cog() {
        assert_eq!(Page::Browser.tab(), Some(Tab::Discover));
        assert_eq!(Page::Shaders.tab(), Some(Tab::Library));
        assert_eq!(Page::Section(3).tab(), Some(Tab::Library));
        assert_eq!(Page::Multiplayer.tab(), Some(Tab::Online));
        assert_eq!(Page::Settings.tab(), None);
        assert_eq!(Page::Tools.tab(), None);
        for tab in Tab::ALL {
            assert_eq!(tab.home().tab(), Some(tab));
        }
    }

    #[test]
    fn the_tabs_never_name_the_gated_page() {
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels, ["Home", "Library", "Discover", "Online", "Support"]);
        assert!(labels.iter().all(|l| !l.to_lowercase().contains("multiplayer")));
    }

    #[test]
    fn left_and_right_stop_at_the_ends() {
        let mut c = Cursor::on_tab(Some(Tab::Home));
        assert_eq!(c.apply(Key::Left, TOP, 0), Step::Stay);
        assert_eq!(c.col, 0);
        for _ in 0..12 {
            c.apply(Key::Right, TOP, 0);
        }
        assert_eq!(c.col, TOP - 1);
    }

    #[test]
    fn the_bar_reaches_the_actions_after_the_tabs() {
        let mut c = Cursor::on_tab(Some(Tab::Support));
        c.apply(Key::Right, TOP, 0);
        assert_eq!(c.col, Tab::ALL.len());
        assert_eq!(Action::ALL[c.col - Tab::ALL.len()], Action::Notifications);
    }

    #[test]
    fn down_goes_to_the_links_then_the_page() {
        let mut c = Cursor::on_tab(Some(Tab::Library));
        assert_eq!(c.apply(Key::Down, TOP, 3), Step::Stay);
        assert_eq!((c.row, c.col), (Row::Links, 0));
        c.apply(Key::Right, TOP, 3);
        assert_eq!(c.apply(Key::Up, TOP, 3), Step::Stay);
        assert_eq!(c.row, Row::Tabs);
        c.apply(Key::Down, TOP, 3);
        assert_eq!(c.apply(Key::Down, TOP, 3), Step::IntoPage);
    }

    #[test]
    fn down_skips_an_empty_link_row() {
        let mut c = Cursor::on_tab(Some(Tab::Home));
        assert_eq!(c.apply(Key::Down, TOP, 0), Step::IntoPage);
    }
}
