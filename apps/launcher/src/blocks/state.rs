use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::input::InputState;
use gpui_kit::component::slider::SliderState;
use gpui_kit::*;
use serde_json::Value;
use swofty_core::view::Field;

use super::select::*;
use crate::colour::ColourPicker;
use crate::scroll::SmoothScroll;

pub(crate) enum Reset {
    No,
    Now,
    After(u64),
}

pub(crate) struct FormSlot {
    pub(super) inputs: HashMap<String, Entity<InputState>>,
    pub(super) sliders: HashMap<String, Entity<SliderState>>,
    pub(super) selects: HashMap<String, SelectSlot>,
    pub(super) picks: HashMap<String, Value>,
    pub(super) synced: HashMap<String, Value>,
    pub(crate) reset: Reset,
    pub(super) seen: bool,
    pub(super) _subs: Vec<Subscription>,
}

pub(crate) struct SearchSlot {
    pub(super) state: Entity<InputState>,
    pub(crate) query: String,
    pub(super) generation: u64,
    pub(super) seen: bool,
    pub(super) _sub: Subscription,
}

pub(crate) struct CellEdit {
    pub(super) target: CellTarget,
    pub(super) field: Field,
    pub(super) original: Value,
    pub(super) input: Entity<InputState>,
    pub(super) _subs: Vec<Subscription>,
}

pub(crate) struct ColourEdit {
    pub(super) key: String,
    pub(super) target: ColourTarget,
    pub(super) picker: Entity<ColourPicker>,
    pub(super) trigger: Rc<std::cell::Cell<Bounds<Pixels>>>,
    pub(super) _subs: Vec<Subscription>,
}

#[derive(Clone)]
pub(super) enum ColourTarget {
    Form {
        key: String,
        field: String,
        addon: String,
        section: String,
        form: String,
    },
    Cell {
        target: CellTarget,
        original: Value,
    },
}

#[derive(Clone)]
pub(crate) struct CellTarget {
    pub(super) key: String,
    pub(super) addon: String,
    pub(super) section: String,
    pub(super) table: String,
    pub(super) row: String,
    pub(super) column: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColumnDrag {
    pub table: String,
    pub column: String,
    pub start_x: f32,
    pub start_width: f32,
}

#[derive(Default)]
pub(crate) struct Blocks {
    pub(super) image_data: RefCell<super::image_data::Images>,
    pub(crate) charts: HashMap<String, (u64, std::sync::Arc<super::chart::Scene>)>,
    pub(crate) forms: HashMap<String, FormSlot>,
    pub(crate) searches: HashMap<String, SearchSlot>,
    pub(crate) sorts: HashMap<String, (String, bool)>,
    pub(crate) column_widths: HashMap<(String, String), f32>,
    pub(crate) column_bounds: Rc<RefCell<HashMap<(String, String), f32>>>,
    pub(crate) column_drag: Option<ColumnDrag>,
    pub(crate) table_rows: HashMap<String, String>,
    pub(crate) section_scrolls: HashMap<String, (ScrollHandle, SmoothScroll)>,
    pub(crate) table_tops: Rc<RefCell<HashMap<String, f32>>>,
    pub(crate) cell: Option<CellEdit>,
    pub(crate) colour: Option<ColourEdit>,
    pub(crate) pending_cells: HashMap<String, String>,
}
