//! The Addons page: install cards, filters, the list in load order and the
//! selected addon's details and conflicts.

mod bulk;
mod conflicts;
mod detail;
mod drag;
mod filters;
mod folders;
mod list;
mod model;
mod needs;
mod options;
mod page;
mod problems;
mod row;
mod select;
mod sync;

use std::time::Duration;

pub(crate) use model::AddonsPage;

const FADE: Duration = Duration::from_secs(4);
pub(crate) const DETAIL_W: f32 = 380.;
pub(crate) const SPLIT_AT: f32 = 1000.;
const ITEMS_SHOWN: usize = 10;
