//! No ad slots on other systems.

use super::{Area, Unit};
use gpui_kit::Window;
use std::path::Path;

pub(super) enum Opener {}

impl Opener {
    pub(super) fn open(self) -> Option<View> {
        match self {}
    }
}

pub(super) enum View {}

pub(super) enum Backend {}

impl Backend {
    pub(super) fn start(_: &Window, _: &Path) -> Option<Self> {
        None
    }

    pub(super) fn show(&mut self, _: Unit, _: &str, _: Area, _: f32) -> Option<Opener> {
        match *self {}
    }

    pub(super) fn hide(&mut self, _: Unit) {
        match *self {}
    }

    pub(super) fn close(&mut self, _: Unit) {
        match *self {}
    }

    pub(super) fn adopt(&mut self, _: Unit, _: Option<View>) {
        match *self {}
    }

    pub(super) fn moved(&mut self, _: impl Iterator<Item = (Unit, Area)>) {
        match *self {}
    }

    pub(super) fn events(&mut self) -> Vec<swofty_ads::Event> {
        match *self {}
    }
}
