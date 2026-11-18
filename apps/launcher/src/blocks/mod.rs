use std::sync::Arc;

use gpui_kit::*;
use swofty_core::view::{Block, Body, Form, Search, View};

use crate::shell::Shell;

mod cell;
pub(crate) mod chart;
mod colour_edit;
mod form;
mod image_data;
mod layout;
mod search;
mod select;
mod state;
mod table;
mod tiles;
mod widgets;

use select::ImageRoots;

pub(crate) use state::Blocks;
pub(crate) use widgets::{caps, eyebrow};

const GAP: f32 = 16.;
/// Tables with more rows than this draw only the rows in view.
const LONG_TABLE: usize = 30;
/// A table's detail panel, as wide as the DB Editor's, and the narrowest a column without a
/// fixed width may get beside it before the panel opens under its row instead.
const DETAIL_W: f32 = 380.;
const DETAIL_COLUMN_MIN: f32 = 96.;
const COLUMN_MIN_W: f32 = 48.;
const SELECT_MENU_H: f32 = 320.;
const SELECT_HEADING_H: f32 = 26.;
const SELECT_THUMB: f32 = 30.;
const SELECT_THUMB_SMALL: f32 = 18.;
const GRID_COLUMN: f32 = 280.;
const TILE_COLUMN: f32 = 170.;

#[derive(Clone, Copy)]
pub(crate) struct At<'a> {
    pub addon: &'a str,
    pub section: &'a str,
    pub width: f32,
    pub nested: bool,
    pub fill: bool,
}

impl At<'_> {
    pub(crate) fn key(&self, id: &str) -> String {
        format!("{}/{}/{id}", self.addon, self.section)
    }

    fn inner(self, width: f32) -> Self {
        Self {
            width,
            nested: true,
            fill: false,
            ..self
        }
    }
}

/// Which table rows' details to walk into: `open(table id, row id)`.
type OpenRows<'f> = &'f dyn Fn(&str, &str) -> bool;

pub(crate) fn walk<'a>(blocks: &'a [Block], out: &mut Vec<&'a Block>) {
    walk_open(blocks, out, &|_, _| true);
}

fn walk_open<'a>(blocks: &'a [Block], out: &mut Vec<&'a Block>, open: OpenRows) {
    for b in blocks {
        out.push(b);
        match b {
            Block::Card(c) => walk_open(&c.blocks, out, open),
            Block::Grid(g) => walk_open(&g.blocks, out, open),
            Block::Table(t) => {
                for (i, r) in t.rows.iter().enumerate() {
                    if open(t.id(), &r.id(i)) {
                        walk_open(r.detail(), out, open);
                    }
                }
            }
            _ => {}
        }
    }
}

fn view_blocks(view: &View) -> Vec<&Block> {
    view_blocks_open(view, &|_, _| true)
}

/// The view's blocks, walking into the details of the table rows `open` names.
fn view_blocks_open<'a>(view: &'a View, open: OpenRows) -> Vec<&'a Block> {
    let mut out = Vec::new();
    match &view.body {
        Body::Blocks(blocks) => walk_open(blocks, &mut out, open),
        Body::List { rows, .. } => {
            for r in rows {
                walk(&r.detail, &mut out);
            }
        }
        Body::Empty { .. } => {}
    }
    out
}

pub(crate) fn search_id(search: &Search, fallback: &str) -> String {
    search.id.clone().unwrap_or_else(|| fallback.to_owned())
}

impl Shell {
    fn current_view(&self) -> Option<(String, String, Arc<View>, u64)> {
        let i = self.open_section()?;
        let s = self.library.sections.get(i)?;
        let key = format!("{}/{}", s.addon, s.spec.id);
        let view = self.views.get(&key)?.as_ref().ok()?.clone();
        let generation = self.view_gens.get(&key).copied().unwrap_or(0);
        Some((s.addon.clone(), s.spec.id.clone(), view, generation))
    }

    fn image_roots(&self, addon: &str) -> ImageRoots {
        ImageRoots([
            self.store.addon_cache_dir(addon),
            self.store.addon_data_dir(addon),
        ])
    }

    pub(crate) fn find_form(&self, addon: &str, section: &str, id: &str) -> Option<Form> {
        let view = self.views.get(&format!("{addon}/{section}"))?.as_ref().ok()?;
        view_blocks(view).into_iter().find_map(|b| match b {
            Block::Form(f) if f.id == id => Some(f.clone()),
            _ => None,
        })
    }

    pub(crate) fn sync_blocks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for image in self.blocks.image_data.borrow_mut().retired() {
            window.drop_image(image).ok();
        }
        let Some((addon, section, view, generation)) = self.current_view() else {
            for image in self.blocks.image_data.borrow_mut().clear() {
                window.drop_image(image).ok();
            }
            self.blocks.charts.clear();
            self.blocks.forms.clear();
            self.blocks.searches.clear();
            self.blocks.cell = None;
            self.blocks.colour = None;
            return;
        };
        let prefix = format!("{addon}/{section}/");
        for slot in self.blocks.forms.values_mut() {
            slot.seen = false;
        }
        for slot in self.blocks.searches.values_mut() {
            slot.seen = false;
        }
        if let Body::List { search: Some(s), .. } = &view.body {
            let key = format!("{prefix}{}", search_id(s, "list"));
            self.sync_search(&key, &addon, &section, s, window, cx);
        }
        // A form in a table row's detail is set up only while the row is open: a long table can
        // carry a form in every row.
        let open_rows = self.blocks.table_rows.clone();
        let open = |table: &str, row: &str| {
            open_rows
                .get(&format!("{prefix}table:{table}"))
                .is_some_and(|o| o == row)
        };
        let mut chart_keys = std::collections::HashSet::new();
        for b in view_blocks_open(&view, &open) {
            match b {
                Block::Chart(c) => {
                    let key = format!("{prefix}{}", c.id);
                    chart_keys.insert(key.clone());
                    if c.validate().is_ok()
                        && self.blocks.charts.get(&key).is_none_or(|(g, _)| *g != generation)
                    {
                        self.blocks.charts.insert(key, (generation, chart::prepare(c)));
                    }
                }
                Block::Form(f) => self.sync_form(&addon, &section, f, generation, window, cx),
                Block::Table(t) => {
                    if let Some(s) = &t.search {
                        let key = format!("{prefix}{}", search_id(s, t.id()));
                        self.sync_search(&key, &addon, &section, s, window, cx);
                    }
                }
                _ => {}
            }
        }
        // Retain only charts in the current view, never old sessions or tabs.
        self.blocks
            .charts
            .retain(|k, (g, _)| chart_keys.contains(k) && *g == generation);
        self.blocks.forms.retain(|k, s| k.starts_with(&prefix) && s.seen);
        self.blocks
            .searches
            .retain(|k, s| k.starts_with(&prefix) && s.seen);
        if self
            .blocks
            .cell
            .as_ref()
            .is_some_and(|c| !c.target.key.starts_with(&prefix))
        {
            self.blocks.cell = None;
        }
        if self
            .blocks
            .colour
            .as_ref()
            .is_some_and(|c| !c.key.starts_with(&prefix))
        {
            self.blocks.colour = None;
        }
    }
}
