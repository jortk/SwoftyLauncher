use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use crate::scale::px;
use gpui_kit::component::select::{SelectDelegate, SelectItem, SelectState};
use gpui_kit::component::{IndexPath, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::view::{Choice, SelectField};

use super::widgets::*;
use super::*;

#[derive(Clone, Default)]
pub(super) struct Shown {
    pub(super) label: SharedString,
    pub(super) image: Option<PathBuf>,
}

#[derive(Clone)]
pub(super) struct Opt {
    pub(super) value: String,
    pub(super) label: SharedString,
    pub(super) image: Option<PathBuf>,
    pub(super) thumbs: bool,
    pub(super) shown: Rc<RefCell<Shown>>,
}

pub(super) fn thumb(image: Option<PathBuf>, size: f32, bg: Option<Hsla>) -> Div {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(4.))
        .overflow_hidden()
        .when_some(bg, |d, bg| d.bg(bg))
        .when_some(image, |d, path| {
            d.child(
                img(path)
                    .size(px(size))
                    .aspect_square()
                    .object_fit(ObjectFit::Cover),
            )
        })
}

impl SelectItem for Opt {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    // The closed field keeps showing the committed value while the cursor moves.
    fn display_title(&self) -> Option<AnyElement> {
        let shown = self.shown.borrow().clone();
        Some(match shown.image {
            Some(image) => h_flex()
                .gap(px(8.))
                .min_w_0()
                .child(thumb(Some(image), SELECT_THUMB_SMALL, None))
                .child(div().min_w_0().truncate().child(shown.label))
                .into_any_element(),
            None => shown.label.into_any_element(),
        })
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap(px(10.))
            .min_w_0()
            .when(self.thumbs, |d| {
                d.child(thumb(
                    self.image.clone(),
                    SELECT_THUMB,
                    Some(crate::theme::palette(cx).elevated),
                ))
            })
            .child(div().min_w_0().truncate().child(self.label.clone()))
    }

    fn value(&self) -> &String {
        &self.value
    }
}

pub(super) struct OptGroup {
    pub(super) title: Option<SharedString>,
    pub(super) items: Vec<Opt>,
}

pub(super) struct Opts {
    pub(super) groups: Vec<OptGroup>,
    pub(super) matched: Vec<(usize, Vec<usize>)>,
    pub(super) titled: bool,
}

impl Opts {
    pub(super) fn new(groups: Vec<OptGroup>) -> Self {
        let titled = groups.iter().any(|g| g.title.is_some());
        let matched = groups
            .iter()
            .enumerate()
            .map(|(g, group)| (g, (0..group.items.len()).collect()))
            .collect();
        Self {
            groups,
            matched,
            titled,
        }
    }
}

impl SelectDelegate for Opts {
    type Item = Opt;

    fn sections_count(&self, _: &App) -> usize {
        self.matched.len()
    }

    fn items_count(&self, section: usize) -> usize {
        self.matched.get(section).map_or(0, |(_, rows)| rows.len())
    }

    fn item(&self, ix: IndexPath) -> Option<&Opt> {
        let (g, rows) = self.matched.get(ix.section)?;
        self.groups[*g].items.get(*rows.get(ix.row)?)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Opt: SelectItem<Value = V>,
        V: PartialEq,
    {
        self.matched.iter().enumerate().find_map(|(section, (g, rows))| {
            let items = &self.groups[*g].items;
            let row = rows.iter().position(|&r| items[r].value() == value)?;
            Some(IndexPath::default().section(section).row(row))
        })
    }

    fn perform_search(&mut self, query: &str, _: &mut Window, _: &mut App) -> Task<()> {
        let needle = query.to_lowercase();
        self.matched = self
            .groups
            .iter()
            .enumerate()
            .filter_map(|(g, group)| {
                let whole = group
                    .title
                    .as_ref()
                    .is_some_and(|t| t.to_lowercase().contains(&needle));
                let rows: Vec<usize> = (0..group.items.len())
                    .filter(|&r| whole || group.items[r].matches(query))
                    .collect();
                (!rows.is_empty()).then_some((g, rows))
            })
            .collect();
        Task::ready(())
    }

    fn render_section_header(&self, section: usize, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
        if !self.titled {
            return None;
        }
        let p = crate::theme::palette(cx);
        let title = self
            .matched
            .get(section)
            .and_then(|(g, _)| self.groups[*g].title.clone());
        Some(
            h_flex()
                .h(px(SELECT_HEADING_H))
                .px_2()
                .items_end()
                .pb(px(4.))
                .when(section > 0, |d| d.border_t_1().border_color(p.border_variant))
                .children(title.map(|t| eyebrow(&t, p)))
                .into_any_element(),
        )
    }
}

pub(super) struct SelectSlot {
    pub(super) state: Entity<SelectState<Opts>>,
    pub(super) options: Vec<Choice>,
    pub(super) shown: Rc<RefCell<Shown>>,
}

pub(super) struct ImageRoots(pub(super) [PathBuf; 2]);

impl ImageRoots {
    pub(super) fn allow(&self, path: &Path) -> Option<PathBuf> {
        let plain = path.is_absolute()
            && path.components().all(|c| {
                matches!(
                    c,
                    Component::Prefix(_) | Component::RootDir | Component::Normal(_)
                )
            });
        (plain && self.0.iter().any(|root| path.starts_with(root))).then(|| path.to_path_buf())
    }
}

pub(super) fn opts(sf: &SelectField, roots: &ImageRoots, shown: &Rc<RefCell<Shown>>) -> Opts {
    let images: Vec<Option<PathBuf>> = sf
        .options
        .iter()
        .map(|o| o.image().and_then(|i| roots.allow(i)))
        .collect();
    let thumbs = images.iter().any(Option::is_some);
    let mut groups = vec![OptGroup {
        title: None,
        items: Vec::new(),
    }];
    for (o, image) in sf.options.iter().zip(images) {
        if o.is_heading() {
            groups.push(OptGroup {
                title: Some(SharedString::from(o.label().to_owned())),
                items: Vec::new(),
            });
        } else if let Some(group) = groups.last_mut() {
            group.items.push(Opt {
                value: o.value().to_owned(),
                label: SharedString::from(o.label().to_owned()),
                image,
                thumbs,
                shown: shown.clone(),
            });
        }
    }
    groups.retain(|g| !g.items.is_empty());
    Opts::new(groups)
}

pub(super) fn shown_for(sf: &SelectField, roots: &ImageRoots, value: &str) -> Shown {
    match sf.options.iter().find(|o| !o.is_heading() && o.value() == value) {
        Some(o) => Shown {
            label: SharedString::from(o.label().to_owned()),
            image: o.image().and_then(|i| roots.allow(i)),
        },
        None => Shown {
            label: SharedString::from(value.to_owned()),
            image: None,
        },
    }
}
