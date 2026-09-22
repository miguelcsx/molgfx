//! Settling every generated visual pipeline before any pass records.
//!
//! A specialized pipeline replaces the interpreted fragment unit of exactly
//! one family — one fragment entry of one pass — so a style is settled once
//! per family it draws through, keyed on the program, the fragment stage and
//! the family name. Compilation is inline on the frame that first needs the
//! style: [`Device`] declares its resources as bare associated types with no
//! `Send` bound, so the compile cannot leave the frame thread, and a style
//! either holds generated code from here on or keeps the interpreter
//! permanently.
//!
//! The pass runs after slot reconciliation and before any render pass is
//! recorded, which is the two-phase frame the cache documents: every compile
//! settles while the cache is borrowed exclusively, and recording then reads
//! settled handles through a shared borrow.

use super::GpuScene;
use crate::engine::pipeline_cache::{SpecializationKey, VisualFamily};
use crate::passes::PassRegistry;
use crate::scene_gpu::slot_types::DrawFamily;
use molgfx_core::{Scene, VisualStage};
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    /// Settles the generated pipeline of every fragment-stage style the frame
    /// will draw, and clears the key of every other style.
    ///
    /// A style whose program the emitter does not lower, and a style whose
    /// compile fails, record their reason once and keep the interpreter for
    /// the process lifetime; nothing is retried and no draw is blocked.
    pub(crate) fn settle_specializations(
        &mut self,
        device: &D,
        scene: &Scene,
        passes: &PassRegistry<D>,
    ) {
        // Disjoint field borrows: the slot table and the layouts stay shared
        // while the cache takes the exclusive borrow it needs to compile.
        let layouts = SpecializationLayouts::<D> {
            group0: &self.group0_layout,
            group2: &self.group2_layout,
            ribbon: &self.ribbon_layout,
            quality: &self.quality_layout,
        };
        let specialized = &mut self.specialized;
        let slot_index = &mut self.slots;
        for slot in slot_index {
            // The family list reads the slot's own draw routing, so it must be
            // materialized before the key table is taken mutably.
            let mut drawn = [false; DrawFamily::COUNT];
            if slot.has_fragment_style()
                && scene
                    .representation(slot.key.representation)
                    .is_some_and(|representation| representation.visual.is_some())
            {
                for family in slot.drawn_families() {
                    drawn[family.index()] = true;
                }
            }
            let program = if drawn.iter().any(|drawn| *drawn) {
                scene
                    .representation(slot.key.representation)
                    .and_then(|representation| representation.visual.as_ref())
                    .map(molgfx_core::VisualStyle::program)
            } else {
                None
            };
            let keys = slot.keys_mut();
            for (index, family) in DrawFamily::ALL.into_iter().enumerate() {
                if !drawn[index] {
                    keys[index] = None;
                    continue;
                }
                let Some(program) = program else {
                    keys[index] = None;
                    continue;
                };
                let key = SpecializationKey::new(
                    program.fingerprint(),
                    VisualStage::Fragment,
                    VisualFamily::new(family.name()),
                );
                specialized.settle(key, program, || {
                    build_specialized(device, &layouts, passes, family)
                });
                keys[index] = Some(key);
            }
        }
    }

    /// The generated pipeline one settled key resolved to.
    ///
    /// The cache was settled before any pass recorded, so this shared-borrow
    /// read never compiles and never blocks: an unset key answers `None` and
    /// the draw keeps the interpreter.
    pub(crate) fn specialized_pipeline(
        &self,
        key: Option<SpecializationKey>,
    ) -> Option<&D::Pipeline> {
        self.specialized.resolve(key?).pipeline()
    }

    /// One report block per resolved family, in slot order.
    ///
    /// Each block names the executed strategy, the cache key and the reason,
    /// so a caller's diagnostic spells the state exactly once.
    pub(crate) fn specialization_report(&self) -> String {
        let mut report = String::new();
        for slot in &self.slots {
            for key in slot.keys() {
                let Some(key) = key else { continue };
                report.push_str(&self.specialized.resolve(*key).report(*key));
                report.push('\n');
            }
        }
        report
    }
}

/// Builds one family's pipeline from that pass's generated sibling.
///
/// Every family compiles the same way from the settle pass's point of view: one
/// pass, one entry point, one set of layouts. Naming that here keeps the settle
/// loop about which families a frame needs rather than about how each one is
/// built.
fn build_specialized<D: Device>(
    device: &D,
    layouts: &SpecializationLayouts<'_, D>,
    passes: &PassRegistry<D>,
    family: DrawFamily,
) -> Result<D::Pipeline, crate::RenderError> {
    match family {
        DrawFamily::Sphere => crate::passes::SpherePass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            false,
        ),
        DrawFamily::SphereClipped => crate::passes::SpherePass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            true,
        ),
        DrawFamily::Bond => crate::passes::BondPass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            false,
        ),
        DrawFamily::BondWire => {
            crate::passes::BondPass::build_specialized(device, layouts.group0, layouts.group2, true)
        }
        DrawFamily::Point => {
            crate::passes::PointPass::build_specialized(device, layouts.group0, layouts.group2)
        }
        DrawFamily::Cartoon => {
            crate::passes::CartoonPass::build_specialized(device, layouts.group0, layouts.ribbon)
        }
        DrawFamily::SurfaceUnion => crate::passes::SurfacePass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            false,
        ),
        DrawFamily::SurfaceGrid => crate::passes::SurfacePass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            true,
        ),
        DrawFamily::ShadowSphere => crate::passes::ShadowPass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            false,
        ),
        DrawFamily::ShadowBond => crate::passes::ShadowPass::build_specialized(
            device,
            layouts.group0,
            layouts.group2,
            true,
        ),
        DrawFamily::ShadowRibbon => crate::passes::ShadowPass::build_specialized_ribbon(
            device,
            layouts.group0,
            layouts.ribbon,
        ),
        DrawFamily::AmbientOcclusion => passes.ambient_occlusion.build_specialized(
            device,
            layouts.group0,
            layouts.quality,
            false,
        ),
        DrawFamily::AmbientOcclusionRayQuery => passes.ambient_occlusion.build_specialized(
            device,
            layouts.group0,
            layouts.quality,
            true,
        ),
    }
}

/// The layouts the specialized builders close over.
///
/// [`GpuScene`](super::GpuScene) owns these for the whole session, so the
/// builders borrow them instead of retaining handles.
#[derive(Clone, Copy)]
struct SpecializationLayouts<'a, D: Device> {
    group0: &'a D::BindGroupLayout,
    group2: &'a D::BindGroupLayout,
    ribbon: &'a D::BindGroupLayout,
    quality: &'a D::BindGroupLayout,
}
