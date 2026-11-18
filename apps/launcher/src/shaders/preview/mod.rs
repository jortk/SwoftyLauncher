mod control;
mod view;
mod worker;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::component::slider::SliderState;
use gpui_kit::*;
use swofty_preview::{ActiveInstance, ClipInfo};

use crate::shaders::ShadersView;
use crate::shaders::clips::ClipItem;
use crate::shaders::triggers::Triggers;
use worker::Worker;

pub(super) const POLL: Duration = Duration::from_millis(30);

#[derive(Clone, PartialEq)]
pub(super) enum Status {
    Idle,
    Downloading,
    DownloadFailed(String),
    Opening,
    Ready,
    NoGpu(String),
    Failed(String),
}

#[derive(Clone, PartialEq)]
pub(super) enum Effects {
    NotStarted,
    Compiling,
    Ready,
    Failed(String),
}

pub(super) struct PreviewState {
    worker: Option<Worker>,
    clips: Vec<ClipItem>,
    scanned: bool,
    clip: usize,
    info: Option<ClipInfo>,
    frame: usize,
    pub(super) playing: bool,
    config_image: Option<Arc<RenderImage>>,
    original_image: Option<Arc<RenderImage>>,
    compare: bool,
    split: f32,
    dragging: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    area: Rc<Cell<Size<Pixels>>>,
    status: Status,
    effects: Effects,
    recompile: bool,
    unavailable: Vec<String>,
    instances: Vec<ActiveInstance>,
    pub(super) triggers: Triggers,
    slider: Entity<SliderState>,
    slider_events: Option<Subscription>,
    pending: usize,
    tick: Option<Task<()>>,
    poll: Option<Task<()>>,
    download: Option<Task<()>>,
    progress: Arc<Mutex<(f32, String)>>,
    render_error: Option<String>,
}

impl PreviewState {
    pub fn new(_window: &mut Window, cx: &mut Context<ShadersView>) -> Self {
        Self {
            worker: None,
            clips: Vec::new(),
            scanned: false,
            clip: 0,
            info: None,
            frame: 0,
            playing: true,
            config_image: None,
            original_image: None,
            compare: false,
            split: 0.5,
            dragging: false,
            bounds: Rc::new(Cell::new(Bounds::default())),
            area: Rc::new(Cell::new(Size::default())),
            status: Status::Idle,
            effects: Effects::NotStarted,
            recompile: false,
            unavailable: Vec::new(),
            instances: Vec::new(),
            triggers: Triggers::default(),
            slider: cx.new(|_| SliderState::new().min(0.).max(1.).step(1.)),
            slider_events: None,
            pending: 0,
            tick: None,
            poll: None,
            download: None,
            progress: Arc::new(Mutex::new((0., String::new()))),
            render_error: None,
        }
    }
}
