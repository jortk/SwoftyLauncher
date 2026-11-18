//! Runs shader configs on preview clips (DESIGN.md §7).
//!
//! `Preview` compiles nothing itself: it takes effects already compiled to SPIR-V and runs their
//! passes with wgpu on the frames of a `Clip`, with the instance semantics of the core's postfx
//! runtime. Create and use it off the UI thread.

mod clip;
mod gpu;

pub use clip::{CachedClip, Clip, ClipEntry, ClipInfo, ClipStore, Frame, INDEX_URL};
pub use gpu::Preview;
pub use swofty_shaders::{ActiveInstance, CompiledEffect};
