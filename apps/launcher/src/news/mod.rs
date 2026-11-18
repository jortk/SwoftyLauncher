//! The News page: the Discord banner, the news posts in full, and the list of
//! release notes beside them.

mod feed;
mod markdown;
mod state;
#[cfg(test)]
mod tests;
mod view;

pub(crate) use feed::{format_date, image};
pub(crate) use state::{News, NewsScroll};
pub(crate) use view::render_markdown;
