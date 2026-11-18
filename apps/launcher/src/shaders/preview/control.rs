use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::*;
use swofty_i18n::{tr, trf};
use swofty_preview::ActiveInstance;
use swofty_shaders::Rule;

use crate::shaders::clips::{self, ClipItem};
use crate::shaders::{Mode, ShadersView};

use super::worker::{Done, Job, Worker, compile_effects, previewable, work};
use super::{Effects, POLL, Status};

/// Shows `next` in `slot` and frees the image it replaces from the window's texture atlas.
pub(super) fn show(slot: &mut Option<Arc<RenderImage>>, next: Option<Arc<RenderImage>>, cx: &mut App) {
    if let Some(old) = std::mem::replace(slot, next)
        && slot.as_ref().is_none_or(|n| !Arc::ptr_eq(n, &old))
    {
        cx.drop_image(old, None);
    }
}

pub(super) fn clock(frame: usize, fps: f32) -> String {
    let secs = frame as f32 / fps.max(1.);
    format!("{}:{:04.1}", (secs / 60.) as u32, secs % 60.)
}

impl ShadersView {
    pub(super) fn worker(&mut self) -> &Worker {
        self.preview.worker.get_or_insert_with(|| {
            let (jobs, job_rx) = mpsc::channel();
            let (done_tx, done) = mpsc::channel();
            std::thread::Builder::new()
                .name("shaders-preview".into())
                .spawn(move || work(job_rx, done_tx))
                .ok();
            Worker { jobs, done }
        })
    }

    pub(super) fn send(&mut self, job: Job, cx: &mut Context<Self>) {
        if self.worker().jobs.send(job).is_err() {
            self.preview.worker = None;
            self.preview.status =
                Status::Failed(tr("The preview stopped. Reopen the page to restart it.").into());
            return;
        }
        self.preview.pending += 1;
        if self.preview.poll.is_none() {
            self.preview.poll = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(POLL).await;
                    let more = this.update(cx, |v, cx| v.drain(cx)).unwrap_or(false);
                    if !more {
                        break;
                    }
                }
                this.update(cx, |v, _| v.preview.poll = None).ok();
            }));
        }
    }

    pub(super) fn drain(&mut self, cx: &mut Context<Self>) -> bool {
        let mut replies = Vec::new();
        if let Some(w) = &self.preview.worker {
            while let Ok(d) = w.done.try_recv() {
                replies.push(d);
            }
        }
        if replies.is_empty() {
            return self.preview.pending > 0;
        }
        for reply in replies {
            self.preview.pending = self.preview.pending.saturating_sub(1);
            self.reply(reply, cx);
        }
        cx.notify();
        self.preview.pending > 0
    }

    pub(super) fn reply(&mut self, reply: Done, cx: &mut Context<Self>) {
        match reply {
            Done::Ack | Done::Skipped => {}
            Done::Opened(Ok(info)) => {
                self.preview.status = Status::Ready;
                self.preview.frame = 0;
                let last = info.frames.saturating_sub(1) as f32;
                self.preview.slider = cx.new(|_| SliderState::new().min(0.).max(last.max(1.)).step(1.));
                self.preview.slider_events = Some(cx.subscribe(
                    &self.preview.slider,
                    |this, _, ev: &SliderEvent, cx| {
                        let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
                        this.preview_seek(v.start().round().max(0.) as usize, cx);
                    },
                ));
                self.preview.info = Some(info);
                self.preview_send_instances(cx);
                self.preview_render(cx);
                if self.preview.playing && self.mode == Mode::Preview {
                    self.preview_play(cx);
                }
            }
            Done::Opened(Err(e)) => {
                self.preview.status = Status::Failed(trf!("Couldn't open the clip: {e}", e = e))
            }
            Done::Frame {
                index,
                config,
                original,
                weights,
                busy,
            } => {
                show(&mut self.preview.config_image, Some(config), cx);
                if let Some(o) = original {
                    show(&mut self.preview.original_image, Some(o), cx);
                }
                self.preview.render_error = None;
                let _ = index;
                self.preview.triggers.weights = weights;
                self.preview.triggers.busy = busy;
                if !self.preview.playing {
                    self.fade_tick(cx);
                }
            }
            Done::Failed { message, gpu: true } => self.preview.status = Status::NoGpu(message),
            Done::Failed { message, gpu: false } => self.preview.render_error = Some(message),
        }
    }

    pub(in crate::shaders) fn preview_open(&mut self, cx: &mut Context<Self>) {
        if !self.preview.scanned {
            self.preview.scanned = true;
            let root = self.store.root.clone();
            cx.spawn(async move |this, cx| {
                let found = cx
                    .background_executor()
                    .spawn(async move { clips::scan(&root) })
                    .await;
                this.update(cx, |v, cx| v.scanned(found, cx)).ok();
            })
            .detach();
        } else if self.preview.info.is_some() && self.preview.status == Status::Ready {
            self.preview_play(cx);
        }
        self.preview_compile(cx);
    }

    pub(in crate::shaders) fn preview_pause(&mut self, _cx: &mut Context<Self>) {
        self.preview.tick = None;
    }

    pub(super) fn scanned(&mut self, found: Vec<ClipItem>, cx: &mut Context<Self>) {
        self.preview.clips = found;
        if self.preview.clips.is_empty() {
            self.preview_download(cx);
        } else {
            self.preview_pick(0, cx);
            self.preview_fetch_rest(cx);
        }
        cx.notify();
    }

    pub(super) fn preview_pick(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(item) = self.preview.clips.get(i).cloned() else {
            return;
        };
        self.preview.clip = i;
        self.preview.status = Status::Opening;
        show(&mut self.preview.config_image, None, cx);
        show(&mut self.preview.original_image, None, cx);
        self.preview.tick = None;
        self.send(Job::Open(item.dir), cx);
        cx.notify();
    }

    pub(super) fn preview_download(&mut self, cx: &mut Context<Self>) {
        if self.preview.status == Status::Downloading {
            return;
        }
        self.preview.status = Status::Downloading;
        let root = self.store.root.clone();
        let progress = self.preview.progress.clone();
        if let Ok(mut p) = progress.lock() {
            *p = (0., tr("Looking for clips").into());
        }
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let going = this
                    .update(cx, |v, cx| {
                        cx.notify();
                        v.preview.status == Status::Downloading
                    })
                    .unwrap_or(false);
                if !going {
                    break;
                }
            }
        });
        self.preview.download = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    clips::download(&root, true, &|f, label| {
                        if let Ok(mut p) = progress.lock() {
                            *p = (f, label.to_owned());
                        }
                    })?;
                    Ok::<Vec<ClipItem>, String>(clips::scan(&root))
                })
                .await;
            drop(ticker);
            let rest = result.is_ok();
            this.update(cx, |v, cx| {
                match result {
                    Ok(found) if !found.is_empty() => {
                        v.preview.clips = found;
                        v.preview_pick(0, cx);
                    }
                    Ok(_) => v.preview.status = Status::DownloadFailed(tr("The clip index is empty.").into()),
                    Err(e) => v.preview.status = Status::DownloadFailed(e),
                }
                cx.notify();
            })
            .ok();
            if rest {
                this.update(cx, |v, cx| v.preview_fetch_rest(cx)).ok();
            }
        }));
    }

    /// Fetches the clips of the CDN index that aren't cached, quietly while one plays; they join the
    /// clip picker.
    pub(super) fn preview_fetch_rest(&mut self, cx: &mut Context<Self>) {
        let root = self.store.root.clone();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move {
                    clips::download(&root, false, &|_, _| {}).ok();
                    clips::scan(&root)
                })
                .await;
            this.update(cx, |v, cx| {
                let ids = |c: &[ClipItem]| c.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
                if ids(&found) != ids(&v.preview.clips) {
                    let playing = v.preview.clips.get(v.preview.clip).map(|c| c.id.clone());
                    v.preview.clips = found;
                    if let Some(i) = playing.and_then(|id| v.preview.clips.iter().position(|c| c.id == id)) {
                        v.preview.clip = i;
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(in crate::shaders) fn preview_catalog_changed(&mut self, cx: &mut Context<Self>) {
        if self.preview.effects == Effects::Compiling {
            self.preview.recompile = true;
            return;
        }
        self.preview.effects = Effects::NotStarted;
        if self.preview.scanned {
            self.preview_compile(cx);
        }
    }

    pub(super) fn preview_compile(&mut self, cx: &mut Context<Self>) {
        if self.preview.effects != Effects::NotStarted || self.catalog.effects.is_empty() {
            return;
        }
        self.preview.effects = Effects::Compiling;
        let store = self.store.clone();
        let catalog = self.catalog.clone();
        cx.spawn(async move |this, cx| {
            let compiled = cx
                .background_executor()
                .spawn(async move { compile_effects(&store, &catalog) })
                .await;
            this.update(cx, |v, cx| {
                match compiled {
                    _ if v.preview.recompile => {
                        v.preview.recompile = false;
                        v.preview.effects = Effects::NotStarted;
                        v.preview_compile(cx);
                    }
                    Ok((effects, unavailable)) => {
                        v.preview.effects = Effects::Ready;
                        v.preview.unavailable = unavailable;
                        let rules = v.preview.triggers.rules.len();
                        v.send(Job::Effects(effects, rules), cx);
                        v.preview_send_instances(cx);
                        v.preview_render(cx);
                    }
                    Err(e) => v.preview.effects = Effects::Failed(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(in crate::shaders) fn preview_instances(
        &mut self,
        instances: Vec<ActiveInstance>,
        rules: Vec<Rule>,
        cx: &mut Context<Self>,
    ) {
        let rules_changed = self.trigger_rules(rules, cx);
        if self.preview.instances == instances && !rules_changed {
            return;
        }
        self.preview.instances = instances;
        if self.preview.worker.is_some() && self.preview.effects == Effects::Ready {
            self.preview_send_instances(cx);
            self.preview_render(cx);
        }
    }

    pub(super) fn preview_send_instances(&mut self, cx: &mut Context<Self>) {
        let rules = self.preview.triggers.rules.clone();
        self.send(Job::Rules(rules), cx);
        self.preview_send_triggers(cx);
        if self.preview.effects != Effects::Ready {
            return;
        }
        let instances = previewable(&self.preview.instances, &self.preview.unavailable);
        self.send(Job::Instances(instances), cx);
    }

    pub(in crate::shaders) fn preview_send_triggers(&mut self, cx: &mut Context<Self>) {
        if self.preview.worker.is_none() {
            return;
        }
        let signals = self.trigger_signals();
        self.send(Job::Triggers(signals), cx);
    }

    pub(in crate::shaders) fn preview_fire(&mut self, id: &str, magnitude: f64, cx: &mut Context<Self>) {
        if self.preview.worker.is_some() {
            self.send(Job::Fire(id.to_owned(), magnitude), cx);
        }
    }

    pub(super) fn preview_render(&mut self, cx: &mut Context<Self>) {
        if self.preview.info.is_none() || self.preview.status != Status::Ready {
            return;
        }
        let (frame, compare) = (self.preview.frame, self.preview.compare);
        self.send(
            Job::Render {
                frame,
                compare,
                settle: false,
            },
            cx,
        );
    }

    pub(in crate::shaders) fn preview_render_settle(&mut self, cx: &mut Context<Self>) {
        if self.preview.info.is_none() || self.preview.status != Status::Ready {
            return;
        }
        let (frame, compare) = (self.preview.frame, self.preview.compare);
        self.send(
            Job::Render {
                frame,
                compare,
                settle: true,
            },
            cx,
        );
    }

    pub(super) fn preview_seek(&mut self, frame: usize, cx: &mut Context<Self>) {
        if self.preview.frame == frame {
            return;
        }
        self.preview.frame = frame;
        self.preview_render(cx);
        cx.notify();
    }

    pub(super) fn preview_play(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.preview.info.clone() else {
            return;
        };
        let frames = info.frames.max(1);
        let fps = info.fps.max(1.);
        let (from, started) = (self.preview.frame, Instant::now());
        self.preview.tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs_f32((1. / fps).clamp(0.02, 0.1)))
                    .await;
                let at = (from + (started.elapsed().as_secs_f32() * fps) as usize) % frames;
                let alive = this
                    .update(cx, |v, cx| {
                        if !v.preview.playing || v.preview.dragging {
                            return v.preview.playing;
                        }
                        if v.preview.frame != at {
                            v.preview.frame = at;
                            v.preview_render(cx);
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        }));
    }

    pub(in crate::shaders) fn sync_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.preview.frame as f32;
        let slider = self.preview.slider.clone();
        if (slider.read(cx).value().start() - at).abs() > 0.5 {
            slider.update(cx, |s, cx| s.set_value(at, window, cx));
        }
    }

    pub(super) fn toggle_play(&mut self, cx: &mut Context<Self>) {
        self.preview.playing = !self.preview.playing;
        if self.preview.playing {
            self.preview_play(cx);
        } else {
            self.preview.tick = None;
            self.fade_tick(cx);
        }
        cx.notify();
    }

    pub(super) fn toggle_compare(&mut self, cx: &mut Context<Self>) {
        self.preview.compare = !self.preview.compare;
        self.preview_render(cx);
        cx.notify();
    }

    pub(super) fn wipe_to(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let b = self.preview.bounds.get();
        let width = f32::from(b.size.width);
        if width > 1. {
            self.preview.split = ((f32::from(x) - f32::from(b.origin.x)) / width).clamp(0.02, 0.98);
            cx.notify();
        }
    }
}
