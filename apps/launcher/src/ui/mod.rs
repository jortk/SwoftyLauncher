//! Shared UI components. Pages build on these instead of keeping their own copies
//! of the header, empty state, callout, progress bar, overlay, card and pill.
//! Sizes go through [`crate::scale::px`], colours come from [`crate::theme::Palette`].

// Pages adopt these one at a time, so part of the set has no caller yet.
#![allow(dead_code, unused_imports)]

mod atoms;
mod callout;
mod card;
mod chip;
mod empty;
pub(crate) mod facet;
pub mod form;
mod header;
mod lock;
pub(crate) mod memory_image;
mod overlay;
mod pill;
mod progress;
mod row;
mod section;
mod segmented;
pub mod tokens;

pub(crate) use atoms::*;
pub(crate) use callout::Callout;
pub(crate) use card::Card;
pub(crate) use chip::Chip;
pub(crate) use empty::EmptyState;
pub(crate) use header::PageHeader;
pub(crate) use lock::LockPanel;
pub(crate) use overlay::{Overlay, takeover, title_bar};
pub(crate) use pill::Pill;
pub(crate) use progress::ProgressBar;
pub(crate) use row::{Toggle, row, row_head, selectable_row};
pub(crate) use section::{Field, SectionHead};
pub(crate) use segmented::Segmented;
