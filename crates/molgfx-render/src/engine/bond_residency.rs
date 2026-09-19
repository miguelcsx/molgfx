//! Bounded provider-bond upload and endpoint lifecycle over resident atom pages.

#[path = "bond_residency_support.rs"]
mod support;

use super::bond_draw_plan::{
    PagedBondGpu, ResidentAtomPage, ResidentBondPlacement, ResidentBondRange,
};
use super::{BondChunkPlacement, ChunkPlacementError, ChunkPlacementId, ChunkPlacementStatus};
use crate::residency::LazyUploadRing;
use crate::{RenderError, ResidencyConfig};
use hashbrown::HashMap;
use molgfx_core::{ChunkPayload, ChunkSpan, LogicalRow, ResidencyTicket};
use molgfx_gpu::{ArenaAllocation, BufferDesc, BufferUsage, Device, FenceValue, PagedArena, Queue};
use support::{depends_on, endpoints_resident, resolve_endpoint, validate_bond_placements};

#[derive(Clone, Debug)]
enum BondState {
    Uploading {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        span: ChunkSpan,
        fence: FenceValue,
        payload: ChunkPayload,
        cancelled: bool,
        blocked: bool,
        announce: bool,
    },
    Resident {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        span: ChunkSpan,
        payload: ChunkPayload,
    },
    Blocked {
        ticket: ResidencyTicket,
        payload: ChunkPayload,
        announce: bool,
    },
}

impl BondState {
    const fn ticket(&self) -> ResidencyTicket {
        match self {
            Self::Uploading { ticket, .. }
            | Self::Resident { ticket, .. }
            | Self::Blocked { ticket, .. } => *ticket,
        }
    }
}

#[derive(Debug)]
pub(crate) struct BondGpuResidency<D: Device> {
    arena: PagedArena,
    uploads: LazyUploadRing,
    pub(crate) buffer: D::Buffer,
    buffer_allocated: bool,
    binding_revision: u64,
    tracked: Vec<BondState>,
    tracked_index: HashMap<(molgfx_core::ResidencyKey, u64), usize>,
    placements: Vec<BondChunkPlacement>,
    placement_index: HashMap<ChunkPlacementId, usize>,
    draw_plan: Vec<ResidentBondPlacement>,
    scratch: Vec<PagedBondGpu>,
    revision: u64,
    planned_revision: u64,
}

impl<D: Device> BondGpuResidency<D> {
    pub(crate) fn new(
        device: &D,
        config: ResidencyConfig,
    ) -> Result<Self, super::ChunkResidencyError> {
        let arena = PagedArena::new(config.page_size, config.page_count)?;
        let uploads = LazyUploadRing::new(config.uploads)?;
        let buffer = device.create_buffer(&BufferDesc {
            label: "unused resident provider bonds",
            size: 256,
            usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
        })?;
        Ok(Self {
            arena,
            uploads,
            buffer,
            buffer_allocated: false,
            binding_revision: 1,
            tracked: Vec::with_capacity(config.machine_capacity),
            tracked_index: HashMap::with_capacity(config.machine_capacity),
            placements: Vec::with_capacity(config.machine_capacity),
            placement_index: HashMap::with_capacity(config.machine_capacity),
            draw_plan: Vec::with_capacity(config.machine_capacity),
            scratch: Vec::new(),
            revision: 1,
            planned_revision: 0,
        })
    }

    pub(crate) fn begin_epoch(&mut self) {
        self.uploads.begin_epoch();
    }

    pub(crate) fn stage(
        &mut self,
        device: &D,
        queue: &D::Queue,
        ticket: ResidencyTicket,
        payload: &ChunkPayload,
        atoms: &[ResidentAtomPage],
    ) -> Result<(), super::ChunkResidencyError> {
        if self.tracked.len() == self.tracked.capacity() {
            return Err(super::ChunkResidencyError::TrackingCapacity);
        }
        if !endpoints_resident(payload, atoms)? {
            self.insert_tracked(BondState::Blocked {
                ticket,
                payload: payload.clone(),
                announce: true,
            });
            self.bump();
            return Ok(());
        }
        self.stage_payload(device, queue, ticket, payload.clone(), atoms, true)
    }

    fn stage_payload(
        &mut self,
        device: &D,
        queue: &D::Queue,
        ticket: ResidencyTicket,
        payload: ChunkPayload,
        atoms: &[ResidentAtomPage],
        announce: bool,
    ) -> Result<(), super::ChunkResidencyError> {
        let ChunkPayload::ProviderBond(bonds) = &payload else {
            return Err(super::ChunkResidencyError::UnsupportedPayload);
        };
        let rows = bonds.descriptor().rows();
        if rows == 0 {
            return Err(super::ChunkResidencyError::EmptyChunk);
        }
        self.scratch.clear();
        self.scratch
            .try_reserve(rows as usize)
            .map_err(|_| super::ChunkResidencyError::SizeOverflow)?;
        for local in 0..rows {
            let record = bonds
                .record(molframe::LocalRow::new(local))
                .map_err(|_| super::ChunkResidencyError::ProviderBondRecord { local_row: local })?;
            self.scratch.push(PagedBondGpu {
                coordinate_a: resolve_endpoint(
                    atoms,
                    record.atom_a.dataset().get(),
                    record.atom_a.row().get(),
                )?,
                coordinate_b: resolve_endpoint(
                    atoms,
                    record.atom_b.dataset().get(),
                    record.atom_b.row().get(),
                )?,
                _padding: [0; 2],
            });
        }
        self.ensure_buffer(device)?;
        let bytes = bytemuck::cast_slice(self.scratch.as_slice());
        let len =
            u64::try_from(bytes.len()).map_err(|_| super::ChunkResidencyError::SizeOverflow)?;
        let allocation = self.arena.allocate(len)?;
        let reservation = match self.uploads.ensure()?.reserve(bytes.len()) {
            Ok(value) => value,
            Err(error) => {
                self.arena.release(allocation)?;
                return Err(error.into());
            }
        };
        let uploads = self.uploads.ensure()?;
        uploads.bytes_mut(reservation)?.copy_from_slice(bytes);
        uploads.commit(reservation.ticket())?;
        if let Err(error) = uploads.ensure_submittable(reservation.ticket()) {
            uploads.cancel(reservation.ticket())?;
            self.arena.release(allocation)?;
            return Err(error.into());
        }
        let start = reservation.offset();
        let end = start + reservation.len();
        queue.write_buffer(
            &self.buffer,
            allocation.byte_offset(),
            &uploads.staging_bytes()[start..end],
        );
        let fence = queue.submit_tracked(device.create_command_encoder());
        uploads.submit(reservation.ticket(), fence)?;
        self.insert_tracked(BondState::Uploading {
            ticket,
            allocation,
            span: ChunkSpan::new(
                LogicalRow::new(bonds.descriptor().logical_start().get()),
                rows,
            )?,
            fence,
            payload,
            cancelled: false,
            blocked: false,
            announce,
        });
        Ok(())
    }

    pub(crate) fn poll(
        &mut self,
        device: &D,
        queue: &D::Queue,
        atoms: &[ResidentAtomPage],
        completed_out: &mut Vec<ResidencyTicket>,
    ) -> Result<(), super::ChunkResidencyError> {
        let completed = queue.completed_fence(device)?;
        if let Some(uploads) = self.uploads.get_mut() {
            let _ = uploads.retire(completed);
        }
        let mut index = 0;
        while index < self.tracked.len() {
            let BondState::Uploading {
                ticket,
                allocation,
                span,
                fence,
                ref payload,
                cancelled,
                blocked,
                announce,
            } = self.tracked[index].clone()
            else {
                index += 1;
                continue;
            };
            if fence > completed {
                index += 1;
                continue;
            }
            if cancelled {
                self.arena.release(allocation)?;
                self.remove_tracked(index);
                continue;
            }
            self.tracked[index] = if blocked {
                self.arena.release(allocation)?;
                BondState::Blocked {
                    ticket,
                    payload: payload.clone(),
                    announce,
                }
            } else {
                if announce {
                    completed_out.push(ticket);
                }
                BondState::Resident {
                    ticket,
                    allocation,
                    span,
                    payload: payload.clone(),
                }
            };
            self.bump();
            index += 1;
        }
        self.retry_blocked(device, queue, atoms)
    }

    fn retry_blocked(
        &mut self,
        device: &D,
        queue: &D::Queue,
        atoms: &[ResidentAtomPage],
    ) -> Result<(), super::ChunkResidencyError> {
        let mut index = 0;
        while index < self.tracked.len() {
            let BondState::Blocked {
                ticket,
                ref payload,
                announce,
            } = self.tracked[index].clone()
            else {
                index += 1;
                continue;
            };
            if endpoints_resident(payload, atoms)? {
                let payload = payload.clone();
                self.remove_tracked(index);
                if let Err(error) =
                    self.stage_payload(device, queue, ticket, payload.clone(), atoms, announce)
                {
                    self.insert_tracked(BondState::Blocked {
                        ticket,
                        payload,
                        announce,
                    });
                    return Err(error);
                }
                self.bump();
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    pub(crate) fn invalidate_atom(
        &mut self,
        dataset: molgfx_core::DatasetId,
        span: ChunkSpan,
    ) -> Result<(), super::ChunkResidencyError> {
        let mut changed = false;
        for state in &mut self.tracked {
            if !depends_on(state, dataset, span)? {
                continue;
            }
            match state {
                BondState::Uploading { blocked, .. } => *blocked = true,
                BondState::Resident {
                    ticket,
                    allocation,
                    span: _,
                    payload,
                } => {
                    self.arena.release(*allocation)?;
                    *state = BondState::Blocked {
                        ticket: *ticket,
                        payload: payload.clone(),
                        announce: false,
                    };
                }
                BondState::Blocked { .. } => {}
            }
            changed = true;
        }
        if changed {
            self.bump();
        }
        Ok(())
    }

    pub(crate) fn cancel_or_evict(
        &mut self,
        ticket: ResidencyTicket,
    ) -> Result<(), super::ChunkResidencyError> {
        let Some(&index) = self.tracked_index.get(&ticket_key(ticket)) else {
            return Ok(());
        };
        match &mut self.tracked[index] {
            BondState::Uploading { cancelled, .. } => *cancelled = true,
            BondState::Resident { allocation, .. } => {
                self.arena.release(*allocation)?;
                self.remove_tracked(index);
            }
            BondState::Blocked { .. } => {
                self.remove_tracked(index);
            }
        }
        self.bump();
        Ok(())
    }

    pub(crate) fn evict_generation(
        &mut self,
        key: molgfx_core::ResidencyKey,
        generation: u64,
    ) -> Result<(), super::ChunkResidencyError> {
        let Some(&index) = self.tracked_index.get(&(key, generation)) else {
            return Ok(());
        };
        let ticket = self.tracked[index].ticket();
        self.cancel_or_evict(ticket)
    }

    pub(crate) fn replace_placements(
        &mut self,
        placements: &[BondChunkPlacement],
    ) -> Result<(), ChunkPlacementError> {
        validate_bond_placements(placements, self.placements.capacity())?;
        if self.placements != placements {
            self.placements.clear();
            self.placements.extend_from_slice(placements);
            self.placement_index.clear();
            self.placement_index.extend(
                self.placements
                    .iter()
                    .enumerate()
                    .map(|(index, placement)| (placement.id, index)),
            );
            self.bump();
        }
        Ok(())
    }

    pub(crate) fn status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        let Some(&index) = self.placement_index.get(&id) else {
            return ChunkPlacementStatus::Missing;
        };
        let value = &self.placements[index];
        if self.resident(value.ticket).is_some() {
            ChunkPlacementStatus::Resident
        } else {
            ChunkPlacementStatus::NotResident
        }
    }

    pub(crate) fn declared_placements(&self) -> &[BondChunkPlacement] {
        &self.placements
    }

    pub(crate) fn sync_scene(
        &mut self,
        scene: &mut crate::scene_gpu::GpuScene<D>,
        device: &D,
        queue: &D::Queue,
        coordinates: &D::Buffer,
        coordinate_binding_revision: u64,
    ) -> Result<(), RenderError> {
        let _ = self.draw_plan();
        let input = crate::scene_gpu::paged_bonds::PagedBondSceneSync {
            device,
            queue,
            coordinates,
            bonds: &self.buffer,
            plan: &self.draw_plan,
            revision: self.revision,
            coordinate_binding_revision,
            bond_binding_revision: self.binding_revision,
        };
        scene.sync_paged_bonds(&input)
    }

    pub(crate) fn draw_plan(&mut self) -> (&[ResidentBondPlacement], u64) {
        if self.planned_revision != self.revision {
            self.draw_plan.clear();
            for placement in &self.placements {
                if let Some(range) = self.resident(placement.ticket) {
                    self.draw_plan.push(ResidentBondPlacement {
                        placement: *placement,
                        range,
                    });
                }
            }
            self.planned_revision = self.revision;
        }
        (&self.draw_plan, self.revision)
    }

    fn resident(&self, ticket: ResidencyTicket) -> Option<ResidentBondRange> {
        let index = *self.tracked_index.get(&ticket_key(ticket))?;
        self.tracked.get(index).and_then(|state| match state {
            BondState::Resident {
                ticket: owner,
                allocation,
                span,
                ..
            } if *owner == ticket => Some(ResidentBondRange {
                ticket,
                allocation: *allocation,
                span: *span,
            }),
            _ => None,
        })
    }

    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
    }

    fn insert_tracked(&mut self, state: BondState) {
        let index = self.tracked.len();
        self.tracked_index.insert(ticket_key(state.ticket()), index);
        self.tracked.push(state);
    }

    fn remove_tracked(&mut self, index: usize) -> Option<BondState> {
        let removed = self.tracked.get(index)?;
        self.tracked_index.remove(&ticket_key(removed.ticket()));
        let removed = self.tracked.swap_remove(index);
        if let Some(moved) = self.tracked.get(index) {
            self.tracked_index.insert(ticket_key(moved.ticket()), index);
        }
        Some(removed)
    }
}

fn ticket_key(ticket: ResidencyTicket) -> (molgfx_core::ResidencyKey, u64) {
    (ticket.key, ticket.generation())
}
