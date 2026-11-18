//! The Addon Browser: the addons published to the Swofty registry, to search,
//! sort, filter, star, read about, and install or update in one click.

mod authors;
mod card;
mod detail;
mod detail_cards;
mod downloads;
mod filters;
mod gallery;
mod grid;
mod jobs;
mod model;
mod offer;
mod page;
mod panels;
mod sidebar;
mod state;
mod sync;
#[cfg(test)]
mod tests;
mod tiles;
mod tracks;
mod updates;

pub(crate) use state::Browser;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use swofty_core::catalog;
use swofty_i18n::trf;

use crate::format;
use crate::news::format_date;

use filters::*;
use grid::*;
use jobs::*;
use model::*;
use sidebar::*;
use sync::*;

const STALE: Duration = Duration::from_secs(30 * 60);
const TICK: Duration = Duration::from_secs(5 * 60);
const STAR_DELAY: Duration = Duration::from_secs(1);
const ACTIVITY_KEPT: usize = 50;
const ICONS_DIR: &str = "icons";
const AVATARS_DIR: &str = "avatars";
const CARD_W: f32 = 300.;
const GAP: f32 = 14.;
const SIDE_W: f32 = 340.;
const AUTHORS_SHOWN: usize = 8;
const HELMET: &[u8] = include_bytes!("../../assets/helmet.svg");
const VERIFYING: u8 = 1;
const INSTALLING: u8 = 2;

fn day(iso: &str) -> String {
    format_date(iso.get(..10).unwrap_or(iso))
}

fn since(iso: &str) -> Option<String> {
    catalog::epoch(iso).map(|s| format::ago_since(s as f64))
}

fn since_time(t: SystemTime) -> String {
    format::ago_since(t.duration_since(UNIX_EPOCH).map_or(0., |d| d.as_secs_f64()))
}

fn host(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| rest)
}

fn web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn took_text(d: Duration) -> String {
    match d.as_millis() {
        ms @ 0..1000 => trf!("{count} ms", count = ms),
        ms => trf!("{seconds} s", seconds = format!("{:.1}", ms as f64 / 1000.)),
    }
}

type AddonNeeds = (Vec<String>, Option<String>);
