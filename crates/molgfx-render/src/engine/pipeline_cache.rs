//! Cache of specialized visual pipelines, compiled on first use.
//!
//! One program fingerprint can back several pipelines, because the same style
//! draws through different families at different stages; the key is therefore
//! the triple of program fingerprint, stage and family. The cache answers one
//! question — *may this style draw with generated code, and if not, why not* —
//! and never blocks a frame on anything it cannot already answer.
//!
//! # Compilation is inline on every target
//!
//! The plan for this cache was a dedicated worker thread compiling off the
//! frame, with the frame drawing the interpreter while a compile was in
//! flight. That is not implementable against the current backend abstraction,
//! and it fails closed rather than open:
//!
//! - [`molgfx_gpu::Device`] declares its resources as bare associated types
//!   (`type Pipeline;`) with no `Send` bound, and the trait itself has no
//!   `Send`/`Sync` supertrait. Handing `&D` and the produced `D::Pipeline` to a
//!   worker thread therefore needs *new* bounds — `D: Send + Sync`,
//!   `D::Pipeline: Send`, `D::BindGroupLayout: Send + Sync` — which would have
//!   to be added to the trait and then propagated through [`crate::Engine`],
//!   `Session` and every pass, changing the renderer's public bounds to serve
//!   one cache.
//! - The repository forbids `unsafe` outside bytemuck derives, so the bound
//!   cannot be asserted locally instead.
//!
//! Rather than fake a thread, compilation runs inline on the frame that first
//! needs the style. That costs one frame's stall per style, once, and keeps
//! every later frame on generated code. The consequence for callers is that
//! [`VisualPipeline::Pending`] is unreachable here: with no in-flight window
//! there is no moment at which a compile is neither finished nor failed. A
//! style is [`VisualPipeline::Specialized`] or it keeps the interpreter
//! permanently. `MAX_IN_FLIGHT_COMPILES` is deliberately absent rather than
//! declared and unused.
//!
//! The interpreter remains a full peer, not a degraded path: a program the
//! emitter does not lower, or one whose compile fails, sets the slot's shading
//! back to the interpreter and is never retried.

use crate::error::RenderError;
use molgfx_core::{VisualPipeline, VisualProgram, VisualStage, visual_pipeline};
use molgfx_gpu::Device;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Upper bound on resident specialized pipelines.
///
/// One pipeline per style is small next to the styles a scene can carry, so the
/// bound is a backstop against an unbounded style stream rather than a working
/// set. The least recently used entry is evicted when the bound is reached.
pub(crate) const MAX_SPECIALIZED_PIPELINES: usize = 64;

/// Stable name of the drawable family a specialized pipeline targets.
///
/// The cache only keys on this; each pass names its own families, so a family
/// introduced by a new pass needs no change here.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct VisualFamily(&'static str);

impl VisualFamily {
    /// Names one drawable family.
    #[must_use]
    pub(crate) const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The family's stable name.
    #[must_use]
    pub(crate) const fn name(self) -> &'static str {
        self.0
    }
}

/// Identity of one specialized pipeline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SpecializationKey {
    fingerprint: u64,
    stage: VisualStage,
    family: VisualFamily,
}

impl SpecializationKey {
    /// Keys one pipeline by the program, the stage it runs at and its family.
    #[must_use]
    pub(crate) const fn new(fingerprint: u64, stage: VisualStage, family: VisualFamily) -> Self {
        Self {
            fingerprint,
            stage,
            family,
        }
    }

    /// One-line identity for a report.
    #[must_use]
    pub(crate) fn label(self) -> String {
        format!(
            "{:016x}:{:?}:{}",
            self.fingerprint,
            self.stage,
            self.family.name()
        )
    }
}

// `VisualStage` in the core library does not derive `Hash`, and deriving it
// here over the whole key would need that. Hashing the discriminant keeps the
// key's `Hash` consistent with its derived `PartialEq`, which compares the
// stage by value.
impl Hash for SpecializationKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
        (self.stage as u8).hash(state);
        self.family.hash(state);
    }
}

/// What one style should draw with this frame.
#[derive(Debug)]
pub(crate) enum Specialization<'a, D: Device> {
    /// Draw with the generated pipeline.
    Ready(&'a D::Pipeline),
    /// Draw the interpreter; the state and reason say why.
    Interpreter {
        /// Reported state: `typed-bytecode-interpreter` or `failed`.
        state: VisualPipeline,
        /// Why the specialized pipeline is unavailable.
        reason: &'a str,
    },
}

impl<'a, D: Device> Specialization<'a, D> {
    /// The pipeline to draw with, when this style specializes.
    #[must_use]
    pub(crate) const fn pipeline(&self) -> Option<&'a D::Pipeline> {
        match self {
            Self::Ready(pipeline) => Some(pipeline),
            Self::Interpreter { .. } => None,
        }
    }

    /// The state to report for this draw.
    #[must_use]
    pub(crate) const fn state(&self) -> VisualPipeline {
        match self {
            Self::Ready(_) => VisualPipeline::Specialized,
            Self::Interpreter { state, .. } => *state,
        }
    }

    /// One report block naming the strategy, the cache key and the reason.
    #[must_use]
    pub(crate) fn report(&self, key: SpecializationKey) -> String {
        let reason = match self {
            Self::Ready(_) => VisualPipeline::Specialized.default_reason().to_owned(),
            Self::Interpreter { reason, .. } => (*reason).to_owned(),
        };
        self.state().report(&key.label(), &reason)
    }
}

#[derive(Debug)]
struct Entry<D: Device> {
    pipeline: D::Pipeline,
    last_use: u64,
}

/// Resident specialized pipelines with their compile outcome.
#[derive(Debug)]
pub(crate) struct SpecializedPipelines<D: Device> {
    entries: HashMap<SpecializationKey, Entry<D>>,
    /// Compile outcomes that will never change, with the state each one
    /// reports, so nothing is retried and a report names the reason exactly.
    failed: HashMap<SpecializationKey, (VisualPipeline, String)>,
    capacity: usize,
    clock: u64,
}

impl<D: Device> Default for SpecializedPipelines<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: Device> SpecializedPipelines<D> {
    /// An empty cache bounded by [`MAX_SPECIALIZED_PIPELINES`].
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::with_capacity(MAX_SPECIALIZED_PIPELINES)
    }

    /// An empty cache with an explicit bound.
    #[must_use]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            failed: HashMap::new(),
            // A zero bound could never hold the pipeline it just compiled, so
            // the cache would thrash on every frame instead of degrading.
            capacity: capacity.max(1),
            clock: 0,
        }
    }

    #[cfg(test)]
    /// Resident pipeline count.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    /// Keys whose compile has permanently failed.
    #[must_use]
    pub(crate) fn failures(&self) -> usize {
        self.failed.len()
    }

    #[cfg(test)]
    /// Whether one key holds a resident pipeline.
    #[must_use]
    pub(crate) fn is_ready(&self, key: SpecializationKey) -> bool {
        self.entries.contains_key(&key)
    }

    #[cfg(test)]
    /// The reason a key was rejected, when it was.
    #[must_use]
    pub(crate) fn failure_reason(&self, key: SpecializationKey) -> Option<&str> {
        self.failed.get(&key).map(|(_, reason)| reason.as_str())
    }

    #[cfg(test)]
    /// Resolves the pipeline for one style, compiling it when this is the first
    /// frame that needs it.
    ///
    /// `build` is called at most once per key and only for a program the
    /// emitter lowers; a program it does not lower, or a failing `build`, marks
    /// the key failed for the process lifetime and never calls `build` again.
    /// The caller decides which device, bind-group layouts and shader module
    /// the pipeline closes over, so this cache stays free of every
    /// family-specific detail.
    pub(crate) fn acquire(
        &mut self,
        key: SpecializationKey,
        program: &VisualProgram,
        build: impl FnOnce() -> Result<D::Pipeline, RenderError>,
    ) -> Specialization<'_, D> {
        self.settle(key, program, build);
        self.resolve(key)
    }

    /// Brings one key to its final state: resident, permanently interpreted, or
    /// freshly compiled.
    ///
    /// This is phase one of a two-phase frame. Call it for every style the
    /// frame will draw, which settles every compile, and then read the results
    /// with [`Self::resolve`], which needs only a shared borrow. That ordering
    /// is what lets a frame hold pipeline references while it records draws:
    /// nothing can compile after the settle phase has run.
    pub(crate) fn settle(
        &mut self,
        key: SpecializationKey,
        program: &VisualProgram,
        build: impl FnOnce() -> Result<D::Pipeline, RenderError>,
    ) {
        // Eligibility is decided from the program itself, before any compile:
        // a program the emitter cannot lower must not reach the backend at all.
        let (eligibility, reason) = visual_pipeline(program);
        if eligibility.is_interpreter() {
            let _ = self.failed.entry(key).or_insert((eligibility, reason));
            return;
        }
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_use = self.clock;
            return;
        }
        // A failed key keeps the interpreter permanently, so the backend is
        // never asked twice for the same style.
        if self.failed.contains_key(&key) {
            return;
        }
        match build() {
            Ok(pipeline) => {
                self.make_room();
                let _ = self.entries.insert(
                    key,
                    Entry {
                        pipeline,
                        last_use: self.clock,
                    },
                );
            }
            Err(error) => {
                let _ = self
                    .failed
                    .entry(key)
                    .or_insert((VisualPipeline::Failed, error.to_string()));
            }
        }
    }

    /// Reads the settled state for one key; phase two of a two-phase frame.
    ///
    /// Takes a shared borrow so it can be called while recording draws.
    pub(crate) fn resolve(&self, key: SpecializationKey) -> Specialization<'_, D> {
        if let Some(entry) = self.entries.get(&key) {
            return Specialization::Ready(&entry.pipeline);
        }
        // Every key `settle` leaves behind records a state and a reason, so a
        // rejected program reports the interpreter and only a failing compile
        // reports a failure. The fallback is unreachable and keeps the reader
        // total rather than panicking.
        let (state, reason) = self.failed.get(&key).map_or(
            (VisualPipeline::Failed, "not-compiled"),
            |(state, reason)| (*state, reason.as_str()),
        );
        Specialization::Interpreter { state, reason }
    }

    /// Evicts the least recently used pipeline when the cache is full.
    fn make_room(&mut self) {
        if self.entries.len() < self.capacity {
            return;
        }
        let victim = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_use)
            .map(|(key, _)| *key);
        if let Some(victim) = victim {
            let _ = self.entries.remove(&victim);
        }
    }
}
