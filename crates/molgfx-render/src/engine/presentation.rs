//! Presentation resources: the display encoding, the transient pool sized to
//! the surface, and acquiring the next surface frame.

use super::Engine;
use crate::error::RenderError;
use crate::graph::{DisplayEncoding, TransientPool, plan_aliases};
use crate::passes::FrameBindings;
use molgfx_gpu::{Device, Surface as _, SurfaceError};

impl<D: Device> Engine<D> {
    /// Whether the tonemap pass smooths edges this frame.
    ///
    /// A profile that states a choice wins. Otherwise the choice follows
    /// convergence, not quality: only a path that accumulates sub-pixel
    /// coverage is already smooth, and the realtime path is not, so it smooths
    /// whatever tier it holds. Keying this to the tier instead left small
    /// interactive scenes — which reach the highest tier — with hard,
    /// stair-stepped silhouettes at every zoom.
    #[inline]
    pub(super) fn edge_smoothing(&self) -> bool {
        match self.resolved_plan.antialias() {
            Some(style) => style.edge_smoothing,
            None => !self.adaptive.converged(),
        }
    }

    /// The display encoding the tonemap pass writes for this frame.
    ///
    /// Selects a pre-built tonemap pipeline rather than a per-pixel branch, so
    /// the encoding is fixed for the whole frame by construction.
    #[inline]
    pub(super) fn display_encoding(&self) -> DisplayEncoding {
        let display = self.resolved_plan.display();

        DisplayEncoding {
            gamut: display.gamut,
            transfer: display.transfer,
        }
    }

    /// Rebuilds transient render resources when the presentation extent changes.
    ///
    /// Existing bindings and the old pool are released before allocating the
    /// replacement pool, minimizing peak RSS during resize.
    ///
    /// Returns `true` when a rebuild occurred.
    ///
    /// # Errors
    ///
    /// Returns an error when the transient pool cannot be built.
    pub(super) fn rebuild_pool_if_needed(&mut self) -> Result<bool, RenderError> {
        let rebuild = self
            .pool
            .as_ref()
            .is_none_or(|pool| !pool.matches(self.width, self.height));

        if !rebuild {
            return Ok(false);
        }

        let plan = plan_aliases(&self.resources, &self.pass_nodes, &self.order);

        // Old views keep their textures alive. Release bindings first so a
        // resize only reserves the new pool, including a large-to-small resize.
        self.bindings = None;
        self.pool = None;
        self.temporal.reset();

        self.pool = Some(TransientPool::build(
            &self.device,
            &self.resources,
            plan,
            self.width,
            self.height,
        )?);

        self.bindings = self
            .pool
            .as_ref()
            .and_then(|pool| FrameBindings::new(&self.device, pool, &self.passes));

        Ok(true)
    }

    /// Acquires the next presentation frame and handles recoverable surfaces.
    ///
    /// Lost and outdated surfaces are reconfigured and reported as `None`.
    /// Timeouts are also reported as `None`, allowing the caller to skip the
    /// current frame rather than failing or blocking.
    ///
    /// # Errors
    ///
    /// Returns an error for surface failures other than lost, outdated, or
    /// timeout conditions.
    pub(super) fn acquire_surface_frame(
        &mut self,
    ) -> Result<Option<<D::Surface as molgfx_gpu::Surface<D>>::Frame>, RenderError> {
        let Some(surface) = &mut self.surface else {
            return Ok(None);
        };

        match surface.acquire() {
            Ok(frame) => Ok(Some(frame)),

            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                surface.configure(
                    &self.device,
                    &molgfx_gpu::SurfaceConfig {
                        width: self.width,
                        height: self.height,
                        format: self.target_format,
                    },
                );

                Ok(None)
            }

            Err(SurfaceError::Timeout) => Ok(None),

            Err(error) => Err(RenderError::Gpu(error.into())),
        }
    }
}
