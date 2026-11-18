use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use gpui_kit::*;
use swofty_core::shaders::CompiledEffect;
use swofty_i18n::tr;
use swofty_preview::{ActiveInstance, Clip, ClipInfo, Frame, Preview};
use swofty_shaders::rules::{Engine, Signals};
use swofty_shaders::{Catalog, Rule, Target};

pub(super) enum Job {
    Open(PathBuf),
    Effects(Vec<CompiledEffect>, usize),
    Rules(Vec<Rule>),
    Triggers(Signals),
    Fire(String, f64),
    Instances(Vec<ActiveInstance>),
    Render {
        frame: usize,
        compare: bool,
        settle: bool,
    },
}

pub(super) enum Done {
    Opened(Result<ClipInfo, String>),
    Ack,
    Skipped,
    Frame {
        index: usize,
        config: Arc<RenderImage>,
        original: Option<Arc<RenderImage>>,
        weights: Vec<f64>,
        busy: bool,
    },
    Failed {
        message: String,
        gpu: bool,
    },
}

pub(super) struct Worker {
    pub(super) jobs: Sender<Job>,
    pub(super) done: Receiver<Done>,
}

/// A frame as an image the window draws as it is: gpui takes BGRA.
pub(super) fn image_of(frame: &Frame) -> Result<Arc<RenderImage>, String> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if frame.rgba.len() < w * h * 4 || w == 0 || h == 0 {
        return Err(tr("The frame is empty").into());
    }
    crate::ui::memory_image::rgba(frame.width, frame.height, &frame.rgba[..w * h * 4], true)
        .ok_or_else(|| tr("The frame is empty").into())
}

pub(super) const STEP: usize = 8;

pub(super) struct Want {
    index: usize,
    compare: bool,
    settle: bool,
}

#[derive(Default)]
pub(super) struct Rig {
    rules: Vec<Rule>,
    engine: Option<Engine>,
    base: Vec<ActiveInstance>,
    signals: Signals,
    pending: Vec<(String, f64)>,
    last: Option<usize>,
    applied: Option<Vec<ActiveInstance>>,
    settle_left: f64,
    last_settle: Option<Instant>,
}

impl Rig {
    fn set_rules(&mut self, rules: Vec<Rule>) {
        self.engine = (!rules.is_empty()).then(|| Engine::new(&rules));
        self.rules = rules;
        self.pending.clear();
        self.last = None;
        self.applied = None;
    }

    fn touched(&mut self) {
        let hold = self
            .rules
            .iter()
            .map(|r| r.hold_ms + r.min_ms + r.for_ms)
            .fold(0.0, f64::max);
        self.settle_left = hold + 150.0;
    }

    fn drive(&mut self, p: &mut Preview, index: usize, fps: f32, settle: bool) -> (Vec<f64>, bool) {
        let Some(engine) = self.engine.as_mut() else {
            return (Vec::new(), false);
        };
        let dt = match self.last {
            Some(l) if index > l && index - l <= STEP => {
                Some((index - l) as f64 * 1000.0 / f64::from(fps.max(1.0)))
            }
            Some(l) if index == l && settle => Some(
                self.last_settle
                    .map_or(33.0, |t| t.elapsed().as_secs_f64() * 1000.0)
                    .clamp(1.0, 100.0),
            ),
            Some(l) if index == l => None,
            _ => {
                engine.reset();
                Some(0.0)
            }
        };
        if let Some(dt) = dt {
            let mut signals = self.signals.clone();
            for (id, magnitude) in self.pending.drain(..) {
                signals.fire(&id, magnitude);
            }
            engine.step(dt, &signals);
            self.settle_left = (self.settle_left - dt).max(0.0);
        }
        self.last_settle = Some(Instant::now());
        self.last = Some(index);
        let (blended, off) = engine.apply(&self.base);
        let shown: Vec<ActiveInstance> = blended
            .into_iter()
            .zip(off)
            .filter_map(|(i, off)| (!off).then_some(i))
            .collect();
        if self.applied.as_ref() != Some(&shown) {
            p.set_instances(shown.clone());
            self.applied = Some(shown);
        }
        let weights = engine.weights().to_vec();
        let busy = self.settle_left > 0.0
            || weights.iter().any(|&w| w > 0.0 && w < 1.0)
            || self
                .rules
                .iter()
                .enumerate()
                .any(|(i, r)| r.event && engine.is_on(i));
        (weights, busy)
    }
}

#[derive(Default)]
struct Session {
    clip: Option<Clip>,
    preview: Option<Preview>,
    gpu_error: Option<String>,
    original: Option<(usize, Arc<RenderImage>)>,
    rig: Rig,
}

impl Session {
    fn handle(&mut self, job: Job) -> Done {
        match job {
            Job::Open(dir) => self.open(&dir),
            Job::Effects(effects, rules) => self.set_effects(effects, rules),
            Job::Rules(rules) => {
                self.rig.set_rules(rules);
                Done::Ack
            }
            Job::Triggers(signals) => {
                self.rig.signals = signals;
                self.rig.touched();
                Done::Ack
            }
            Job::Fire(id, magnitude) => {
                self.rig.pending.push((id, magnitude));
                self.rig.touched();
                Done::Ack
            }
            Job::Instances(instances) => self.set_instances(instances),
            Job::Render {
                frame,
                compare,
                settle,
            } => render(
                &mut self.clip,
                &mut self.preview,
                &mut self.gpu_error,
                &mut self.original,
                &mut self.rig,
                Want {
                    index: frame,
                    compare,
                    settle,
                },
            ),
        }
    }

    fn open(&mut self, dir: &std::path::Path) -> Done {
        self.original = None;
        self.rig.last = None;
        match Clip::open(dir) {
            Ok(opened) => {
                let info = opened.info.clone();
                self.clip = Some(opened);
                match gpu(&mut self.preview, &mut self.gpu_error) {
                    Ok(p) => {
                        p.set_clip(&info);
                        Done::Opened(Ok(info))
                    }
                    Err(message) => Done::Failed { message, gpu: true },
                }
            }
            Err(e) => {
                self.clip = None;
                Done::Opened(Err(e))
            }
        }
    }

    fn set_effects(&mut self, effects: Vec<CompiledEffect>, rules: usize) -> Done {
        match gpu(&mut self.preview, &mut self.gpu_error) {
            Ok(p) => {
                crate::app_log::note(&describe(&effects, rules));
                p.set_effects(effects);
                Done::Ack
            }
            Err(message) => Done::Failed { message, gpu: true },
        }
    }

    fn set_instances(&mut self, instances: Vec<ActiveInstance>) -> Done {
        match gpu(&mut self.preview, &mut self.gpu_error) {
            Ok(p) => {
                self.rig.applied = None;
                if self.rig.engine.is_none() {
                    p.set_instances(instances.clone());
                }
                self.rig.base = instances;
                Done::Ack
            }
            Err(message) => Done::Failed { message, gpu: true },
        }
    }
}

pub(super) fn work(jobs: Receiver<Job>, done: Sender<Done>) {
    let mut session = Session::default();
    let mut logged_failure: Option<String> = None;
    while let Ok(first) = jobs.recv() {
        let mut batch = vec![first];
        while let Ok(job) = jobs.try_recv() {
            batch.push(job);
        }
        let last_render = batch.iter().rposition(|j| matches!(j, Job::Render { .. }));
        for (i, job) in batch.into_iter().enumerate() {
            let reply = if matches!(job, Job::Render { .. }) && Some(i) != last_render {
                Done::Skipped
            } else {
                session.handle(job)
            };
            if let Done::Failed { message, .. } = &reply
                && logged_failure.as_ref() != Some(message)
            {
                crate::app_log::note(&format!("[shaders] preview: {message}"));
                logged_failure = Some(message.clone());
            }
            if done.send(reply).is_err() {
                return;
            }
        }
    }
}

/// The effects the preview loads, each with its textures: a declared one's format, or an image's file and size.
pub(super) fn describe(effects: &[CompiledEffect], rules: usize) -> String {
    let list: Vec<String> = effects
        .iter()
        .map(|e| {
            let textures: Vec<String> = e
                .effect
                .textures
                .iter()
                .map(|t| match &t.file {
                    Some(file) => match e.images.iter().find(|i| i.0 == t.name) {
                        Some((_, w, h, _)) => format!("{} {file} {w}x{h}", t.name),
                        None => format!("{} {file} not loaded", t.name),
                    },
                    None => format!("{} {}", t.name, t.format.as_deref().unwrap_or("rgba16f")),
                })
                .collect();
            if textures.is_empty() {
                e.effect.id.clone()
            } else {
                format!("{} ({})", e.effect.id, textures.join(", "))
            }
        })
        .collect();
    format!("[shaders] preview effects: {}; {rules} rule(s)", list.join("; "))
}

pub(super) fn gpu<'a>(
    preview: &'a mut Option<Preview>,
    error: &mut Option<String>,
) -> Result<&'a mut Preview, String> {
    if let Some(e) = error {
        return Err(e.clone());
    }
    if preview.is_none() {
        match Preview::new() {
            Ok(p) => {
                crate::app_log::note(&format!("[shaders] preview runs on {}", p.adapter_name()));
                *preview = Some(p);
            }
            Err(e) => {
                crate::app_log::note(&format!("[shaders] preview has no GPU: {e}"));
                *error = Some(e.clone());
                return Err(e);
            }
        }
    }
    preview.as_mut().ok_or_else(|| tr("No GPU").to_owned())
}

pub(super) fn render(
    clip: &mut Option<Clip>,
    preview: &mut Option<Preview>,
    gpu_error: &mut Option<String>,
    original: &mut Option<(usize, Arc<RenderImage>)>,
    rig: &mut Rig,
    want: Want,
) -> Done {
    let Want {
        index,
        compare,
        settle,
    } = want;
    let Some(clip) = clip.as_ref() else {
        return Done::Failed {
            message: tr("No clip is open").into(),
            gpu: false,
        };
    };
    let p = match gpu(preview, gpu_error) {
        Ok(p) => p,
        Err(message) => return Done::Failed { message, gpu: true },
    };
    let index = index.min(clip.len().saturating_sub(1));
    let frame = match clip.frame(index) {
        Ok(f) => f,
        Err(message) => return Done::Failed { message, gpu: false },
    };
    let (weights, busy) = rig.drive(p, index, clip.info.fps, settle);
    let out = match p.render(&frame).and_then(|f| image_of(&f)) {
        Ok(b) => b,
        Err(message) => return Done::Failed { message, gpu: false },
    };
    let original_png = if compare {
        match original {
            Some((i, image)) if *i == index => Some(image.clone()),
            _ => match image_of(&frame) {
                Ok(image) => {
                    *original = Some((index, image.clone()));
                    Some(image)
                }
                Err(message) => return Done::Failed { message, gpu: false },
            },
        }
    } else {
        None
    };
    Done::Frame {
        index,
        config: out,
        original: original_png,
        weights,
        busy,
    }
}

/// Library and addon effects compiled to SPIR-V for the preview.
pub(super) fn compile_effects(
    store: &swofty_core::store::Store,
    catalog: &Catalog,
) -> Result<(Vec<CompiledEffect>, Vec<String>), String> {
    let (effects, warnings) = swofty_core::shaders::compile_all(store, Target::Spirv);
    for warning in &warnings {
        crate::app_log::note(&format!("[shaders] preview: {warning}"));
    }
    if effects.is_empty() {
        return Err(warnings
            .into_iter()
            .next()
            .unwrap_or_else(|| tr("There are no effects to preview").into()));
    }
    let unavailable = unavailable_effects(catalog.effects.iter().map(|e| e.id.as_str()), &effects);
    if !unavailable.is_empty() {
        crate::app_log::note(&format!(
            "[shaders] preview: not available in the preview: {}",
            unavailable.join(", ")
        ));
    }
    Ok((effects, unavailable))
}

pub(super) fn unavailable_effects<'a>(
    catalog: impl Iterator<Item = &'a str>,
    compiled: &[CompiledEffect],
) -> Vec<String> {
    catalog
        .filter(|id| !compiled.iter().any(|c| c.effect.id == *id))
        .map(str::to_owned)
        .collect()
}

pub(super) fn previewable(instances: &[ActiveInstance], unavailable: &[String]) -> Vec<ActiveInstance> {
    instances
        .iter()
        .filter(|i| !unavailable.contains(&i.effect))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{previewable, unavailable_effects};
    use swofty_core::shaders::CompiledEffect;
    use swofty_preview::ActiveInstance;
    use swofty_shaders::{Origin, parse_effect};

    fn compiled(id: &str) -> CompiledEffect {
        let json = format!(
            r#"{{"id":"{id}","name":"x","params":[],"passes":[{{"name":"a","shader":"{id}.hlsl"}}]}}"#
        );
        CompiledEffect {
            effect: parse_effect(&json, Origin::Library).unwrap(),
            blobs: vec![Vec::new()],
            images: Vec::new(),
        }
    }

    fn instance(effect: &str) -> ActiveInstance {
        ActiveInstance {
            effect: effect.into(),
            label: String::new(),
            params: Vec::new(),
        }
    }

    #[test]
    fn effects_that_did_not_compile_are_listed() {
        let ids = ["ambient_light", "grain", "rewind_video"];
        let missing = unavailable_effects(ids.into_iter(), &[compiled("grain")]);
        assert_eq!(missing, ["ambient_light", "rewind_video"]);
    }

    #[test]
    fn instances_of_missing_effects_stay_out_of_the_preview() {
        let instances = [instance("ambient_light"), instance("grain")];
        let kept = previewable(&instances, &["ambient_light".to_owned()]);
        assert_eq!(kept, [instance("grain")]);
    }
}
