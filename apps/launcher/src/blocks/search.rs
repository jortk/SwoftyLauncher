use std::time::Duration;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::*;
use swofty_core::view::Search;
use swofty_i18n::tr;

use super::state::*;
use crate::shell::Shell;
use crate::theme::Palette;

impl Shell {
    pub(super) fn sync_search(
        &mut self,
        key: &str,
        addon: &str,
        section: &str,
        search: &Search,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.blocks.searches.get_mut(key) {
            slot.seen = true;
            return;
        }
        let placeholder = search.placeholder.clone().unwrap_or_else(|| tr("Search").into());
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let (key_owned, addon, section, id, event) = (
            key.to_owned(),
            addon.to_owned(),
            section.to_owned(),
            search.id.clone(),
            search.event,
        );
        let fallback = key.rsplit('/').next().unwrap_or("list").to_owned();
        let sub = cx.subscribe_in(&state, window, move |this, state, ev: &InputEvent, window, cx| {
            if !matches!(ev, InputEvent::Change) {
                return;
            }
            let query = state.read(cx).value().to_string();
            let Some(slot) = this.blocks.searches.get_mut(&key_owned) else {
                return;
            };
            if slot.query == query {
                return;
            }
            slot.query = query.clone();
            slot.generation += 1;
            cx.notify();
            if !event {
                return;
            }
            let (generation, key, addon, section) =
                (slot.generation, key_owned.clone(), addon.clone(), section.clone());
            let id = id.clone().unwrap_or_else(|| fallback.clone());
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                this.update_in(cx, |s, window, cx| {
                    if s.blocks
                        .searches
                        .get(&key)
                        .is_some_and(|slot| slot.generation == generation)
                    {
                        let event = swofty_core::view::Event::Search { id, query };
                        s.send(&addon, &section, key.clone(), event, window, cx);
                    }
                })
                .ok();
            })
            .detach();
        });
        self.blocks.searches.insert(
            key.to_owned(),
            SearchSlot {
                state,
                query: String::new(),
                generation: 0,
                seen: true,
                _sub: sub,
            },
        );
    }

    pub(crate) fn query(&self, key: &str) -> Option<&str> {
        self.blocks
            .searches
            .get(key)
            .map(|s| s.query.as_str())
            .filter(|q| !q.trim().is_empty())
    }

    pub(crate) fn search_box(&self, key: &str, p: Palette) -> Option<AnyElement> {
        let slot = self.blocks.searches.get(key)?;
        Some(
            div()
                .w(px(280.))
                .child(
                    Input::new(&slot.state).tap_sm().cleanable(true).prefix(
                        Icon::new(IconName::Search)
                            .size(px(14.))
                            .text_color(p.text_placeholder),
                    ),
                )
                .into_any_element(),
        )
    }
}
