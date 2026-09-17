//! Reusable host workspace for future paged GPU scene residency.
//!
//! This composes the backend-neutral allocator, staging ring and command
//! scratch without coupling them to current render passes. `begin_frame`
//! resets only logical lengths and budgets; all backing storage remains live.

use crate::ResidencyMachineMetrics;
use molgfx_gpu::{
    ArenaError, ArenaMetrics, CommandScratch, CommandScratchMetrics, PagedArena,
    UploadBackpressure, UploadMetrics, UploadRing, UploadRingConfig,
};
use thiserror::Error;

/// Fixed capacities allocated when a residency workspace is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidencyConfig {
    /// GPU arena page size in bytes.
    pub page_size: u64,
    /// Number of pages represented by the arena.
    pub page_count: u32,
    /// Host upload staging and backpressure limits.
    pub uploads: UploadRingConfig,
    /// Maximum lowered commands retained per frame.
    pub command_capacity: usize,
    /// Maximum resources tracked by the lifecycle machine.
    pub machine_capacity: usize,
}

impl Default for ResidencyConfig {
    fn default() -> Self {
        const KIB: usize = 1024;
        Self {
            page_size: 64 * 1024,
            page_count: 256,
            uploads: UploadRingConfig {
                capacity_bytes: 256 * KIB,
                ticket_capacity: 256,
                epoch_budget_bytes: 256 * KIB,
                in_flight_budget_bytes: 256 * KIB,
                alignment: 256,
            },
            command_capacity: 1024,
            machine_capacity: 1024,
        }
    }
}

/// Snapshot of all residency primitive counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidencyMetrics {
    /// Page allocation counters.
    pub arena: ArenaMetrics,
    /// Upload staging counters.
    pub uploads: UploadMetrics,
    /// Reusable command counters.
    pub commands: CommandScratchMetrics,
    /// Resource lifecycle gauges and transitions.
    pub machine: ResidencyMachineMetrics,
    /// Physical storage and traffic for immutable shared asset payloads.
    pub immutable_assets: ImmutableArenaMetrics,
}

/// Physical counters for the immutable shared GPU arena.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImmutableArenaMetrics {
    /// Number of live physical buffers backing all immutable assets.
    pub physical_buffers: u32,
    /// Bytes reserved by the current physical buffer.
    pub physical_bytes: u64,
    /// Page-rounded bytes occupied by live immutable payloads.
    pub resident_bytes: u64,
    /// Highest immutable resident footprint observed.
    pub peak_resident_bytes: u64,
    /// Payload bytes uploaded from the host.
    pub uploaded_bytes: u64,
    /// Host-to-GPU writes issued for immutable payloads.
    pub writes: u64,
    /// GPU-to-GPU relocations caused by physical growth.
    pub relocations: u64,
    /// Requests rejected at the physical device limit.
    pub allocation_stalls: u64,
}

/// Flat counters consumed by profiling and benchmark reports.
///
/// Allocation, upload and stall values are cumulative for the engine
/// lifetime. Resident bytes are the current gauge at snapshot time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidencyCounters {
    /// Host backing-store allocations owned by residency primitives.
    pub allocation_events: u64,
    /// Payload bytes submitted through the upload ring.
    pub upload_bytes: u64,
    /// Page-rounded bytes currently reserved in the GPU arena.
    pub resident_bytes: u64,
    /// Capacity and budget rejections across every residency primitive.
    pub stall_events: u64,
}

impl ResidencyMetrics {
    /// Flattens the detailed metrics without estimating missing values.
    #[must_use]
    pub fn counters(self) -> ResidencyCounters {
        ResidencyCounters {
            allocation_events: self
                .arena
                .host_allocation_events
                .saturating_add(self.uploads.host_allocation_events)
                .saturating_add(self.commands.host_allocation_events),
            upload_bytes: self.uploads.bytes_submitted,
            resident_bytes: self.arena.resident_bytes,
            stall_events: self
                .arena
                .allocation_stalls
                .saturating_add(self.uploads.stall_events)
                .saturating_add(self.commands.capacity_stalls)
                .saturating_add(self.machine.capacity_stalls),
        }
    }
}

/// Failure while reserving fixed workspace resources.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ResidencyInitError {
    /// Arena configuration is invalid.
    #[error(transparent)]
    Arena(#[from] ArenaError),
    /// Upload ring configuration is invalid.
    #[error(transparent)]
    Uploads(#[from] UploadBackpressure),
}

/// Fixed-capacity reusable state for residency planning and command lowering.
#[derive(Debug)]
pub struct ResidencyWorkspace<C: Copy> {
    arena: PagedArena,
    uploads: UploadRing,
    commands: CommandScratch<C>,
}

/// Upload storage whose configured capacity is committed only on first use.
#[derive(Debug)]
pub(crate) struct LazyUploadRing {
    config: UploadRingConfig,
    ring: Option<UploadRing>,
}

impl LazyUploadRing {
    pub(crate) fn new(config: UploadRingConfig) -> Result<Self, UploadBackpressure> {
        config.validate()?;
        Ok(Self { config, ring: None })
    }

    pub(crate) fn begin_epoch(&mut self) {
        if let Some(ring) = &mut self.ring {
            ring.begin_epoch();
        }
    }

    pub(crate) fn ensure(&mut self) -> Result<&mut UploadRing, UploadBackpressure> {
        if self.ring.is_none() {
            self.ring = Some(UploadRing::new(self.config)?);
        }
        self.ring
            .as_mut()
            .ok_or(UploadBackpressure::InvalidConfiguration)
    }

    pub(crate) const fn get_mut(&mut self) -> Option<&mut UploadRing> {
        self.ring.as_mut()
    }

    pub(crate) fn metrics(&self) -> UploadMetrics {
        self.ring
            .as_ref()
            .map_or_else(UploadMetrics::default, UploadRing::metrics)
    }
}

impl<C: Copy> ResidencyWorkspace<C> {
    /// Allocates every host-side backing store before frame processing starts.
    ///
    /// # Errors
    ///
    /// Returns a typed configuration error for invalid capacities.
    pub fn new(config: ResidencyConfig) -> Result<Self, ResidencyInitError> {
        Ok(Self {
            arena: PagedArena::new(config.page_size, config.page_count)?,
            uploads: UploadRing::new(config.uploads)?,
            commands: CommandScratch::new(config.command_capacity),
        })
    }

    /// Starts a frame without releasing or growing host storage.
    pub fn begin_frame(&mut self) {
        self.uploads.begin_epoch();
        self.commands.clear();
    }

    /// Page allocator used to assign future GPU arena ranges.
    #[must_use]
    pub const fn arena(&self) -> &PagedArena {
        &self.arena
    }

    /// Mutable page allocator used to assign and release ranges.
    pub const fn arena_mut(&mut self) -> &mut PagedArena {
        &mut self.arena
    }

    /// Upload staging ring and lifecycle state.
    #[must_use]
    pub const fn uploads(&self) -> &UploadRing {
        &self.uploads
    }

    /// Mutable upload ring used during staging and retirement.
    pub const fn uploads_mut(&mut self) -> &mut UploadRing {
        &mut self.uploads
    }

    /// Commands lowered during the current frame.
    #[must_use]
    pub const fn commands(&self) -> &CommandScratch<C> {
        &self.commands
    }

    /// Mutable reusable command storage.
    pub const fn commands_mut(&mut self) -> &mut CommandScratch<C> {
        &mut self.commands
    }

    /// Current real counters from all three storage primitives.
    #[must_use]
    pub fn metrics(&self) -> ResidencyMetrics {
        ResidencyMetrics {
            arena: self.arena.metrics(),
            uploads: self.uploads.metrics(),
            commands: self.commands.metrics(),
            machine: ResidencyMachineMetrics::default(),
            immutable_assets: ImmutableArenaMetrics::default(),
        }
    }
}

#[cfg(test)]
#[path = "residency_tests.rs"]
mod tests;
