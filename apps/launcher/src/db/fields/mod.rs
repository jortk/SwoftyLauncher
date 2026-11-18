mod control;
mod curve;
mod detail;
mod edit;
mod model;
mod panel;
mod sync;
mod value;

use std::collections::HashMap;

use gpui_kit::component::color_picker::ColorPickerState;
use gpui_kit::component::input::InputState;
use gpui_kit::component::select::{SearchableVec, SelectItem, SelectState};
use gpui_kit::*;
use swofty_core::dbedit::{Backing, Kind};

use super::{Nav, bool_text, clean, hex, kind_of, type_name};

#[derive(Clone)]
pub(crate) struct Choice {
    value: String,
    title: SharedString,
}

impl SelectItem for Choice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &String {
        &self.value
    }
}

type Choices = SearchableVec<Choice>;

struct InputSlot {
    key: u64,
    input: Entity<InputState>,
    synced: String,
    seen: bool,
    _subs: Vec<Subscription>,
}

struct SelectSlot {
    key: u64,
    state: Entity<SelectState<Choices>>,
    synced: String,
    seen: bool,
    _sub: Subscription,
}

struct ColorSlot {
    key: u64,
    state: Entity<ColorPickerState>,
    synced: String,
    seen: bool,
    _sub: Subscription,
}

struct CopySlot {
    state: Entity<SelectState<Choices>>,
    seen: bool,
    _sub: Subscription,
}

#[derive(Default)]
pub(crate) struct Slots {
    inputs: HashMap<String, InputSlot>,
    selects: HashMap<String, SelectSlot>,
    colors: HashMap<String, ColorSlot>,
    copies: HashMap<String, CopySlot>,
}

impl Slots {
    pub(crate) fn forget(&mut self, key: u64) {
        self.inputs.retain(|_, s| s.key != key);
        self.selects.retain(|_, s| s.key != key);
        self.colors.retain(|_, s| s.key != key);
    }
}

/// One editable value of the detail panel: a module field of the selected row, or a raw key.
#[derive(Clone)]
pub(crate) struct FieldModel {
    pub id: String,
    pub key: Option<u64>,
    pub tag: u8,
    pub kind: Kind,
    pub label: String,
    pub section: Option<String>,
    pub help: Option<String>,
    pub value: String,
    pub stock: Option<String>,
    pub edited: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub unit: Option<String>,
    pub options: Vec<(String, String)>,
    pub text: Option<String>,
    pub readonly: bool,
    pub optional: bool,
    pub shared: bool,
    pub others: Option<String>,
    /// For a field read from another table's row picked by this row's label: the row spec and where the row comes from.
    pub row: Option<(usize, Backing)>,
    /// The page field's index, when one value can be set on every listed row.
    pub all: Option<usize>,
}

pub(super) struct SelectSpec<'a> {
    id: &'a str,
    key: u64,
    value: &'a str,
    whole: &'a str,
    options: &'a [(String, String)],
}

pub(crate) use detail::{detail_shell, swatch};
pub(crate) use value::{curve_rating, hex_rgb, is_on, list_items};
