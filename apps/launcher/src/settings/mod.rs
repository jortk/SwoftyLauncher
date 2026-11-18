//! The Settings page: a category list on the left and the chosen category's
//! sections on the right. Storage shows where Swofty keeps its files: the data
//! folder's size by part, the room left on its drive and on the drive F1 25
//! needs, and clearing old game session logs.

mod sections;
mod usage;

use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use swofty_core::data_usage::Usage;
use swofty_i18n::tr;

use crate::pages::GUTTER;
use crate::shell::{Page, Shell};
use crate::theme::Palette;
use crate::ui::form::side_nav_item;

pub use usage::{GameCopy, clear_logs_at_start, low_space_warning};

const NAV_WIDTH: f32 = 200.;
const CONTENT_WIDTH: f32 = 760.;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Category {
    #[default]
    General,
    Display,
    Storage,
    Advanced,
    About,
}

impl Category {
    const ALL: [Self; 5] = [
        Self::General,
        Self::Display,
        Self::Storage,
        Self::Advanced,
        Self::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::General => tr("General"),
            Self::Display => tr("Display"),
            Self::Storage => tr("Storage"),
            Self::Advanced => tr("Advanced"),
            Self::About => tr("About"),
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::General => IconName::Settings,
            Self::Display => IconName::Monitor,
            Self::Storage => IconName::HardDrive,
            Self::Advanced => IconName::Wrench,
            Self::About => IconName::Info,
        }
    }
}

#[derive(Default)]
pub struct Settings {
    pub category: Category,
    pub usage: Option<Usage>,
    pub prunable: swofty_core::data_usage::Pruned,
    pub measuring: bool,
    pub clearing: bool,
    pub cache: GameCopy,
}

impl Shell {
    pub(crate) fn settings_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let category = self.settings.category;
        if category == Category::Storage && self.settings.usage.is_none() {
            self.settings_measure(cx);
        }
        v_flex()
            .size_full()
            .child(self.header(
                Page::Settings.label(),
                tr("Launcher options, and where Swofty keeps its files").to_owned(),
                div(),
                p,
            ))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .gap(px(24.))
                    .px(px(GUTTER))
                    .child(self.settings_nav(category, p, cx))
                    .child(
                        div()
                            .id(SharedString::from(format!("settings-scroll-{category:?}")))
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .child(
                                v_flex()
                                    .max_w(px(CONTENT_WIDTH))
                                    .gap(px(24.))
                                    .pb(px(28.))
                                    .pr(px(8.))
                                    .child(self.settings_category(category, p, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn settings_nav(&self, current: Category, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().downgrade();
        v_flex()
            .w(px(NAV_WIDTH))
            .flex_shrink_0()
            .gap(px(2.))
            .tab_group()
            .children(Category::ALL.into_iter().map(|category| {
                let shell = shell.clone();
                side_nav_item(
                    SharedString::from(format!("settings-nav-{category:?}")),
                    category.icon(),
                    category.label(),
                    category == current,
                    move |_, cx| {
                        shell
                            .update(cx, |s, cx| {
                                s.settings.category = category;
                                cx.notify();
                            })
                            .ok();
                    },
                    p,
                )
            }))
    }

    fn settings_category(&mut self, category: Category, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        match category {
            Category::General => self.settings_general(cx),
            Category::Display => self.settings_display(p, cx),
            Category::Storage => self.settings_storage(p, cx),
            Category::Advanced => self.settings_advanced(cx),
            Category::About => self.settings_about(cx),
        }
    }
}
