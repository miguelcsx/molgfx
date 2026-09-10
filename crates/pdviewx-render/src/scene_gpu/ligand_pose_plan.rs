//! Revision-time planning for compact ligand pose GPU tables.
//!
//! Each source pose is classified and hashed once into reusable opaque or
//! translucent index columns. Upload follows those indices, so it never
//! rescans the complete source once per opacity class.

use super::ligand_pose_sampling::{PoseBatchSample, select_pose_batches};
use super::ligand_pose_types::{
    POSE_HASH_OFFSET, PoseBatchGpu, batch_entity, checked_add, checked_count, classify_poses,
    mix_pose_key,
};
use super::structure::GpuStructure;
use crate::error::RenderError;
use pdviewx_core::{LigandPoseBatch, LigandPoseBatchHandle, Scene};
use pdviewx_gpu::Device;
use pdviewx_math::Mat4;

#[derive(Clone, Copy, Debug)]
pub(super) struct BatchPlan {
    pub(super) handle: LigandPoseBatchHandle,
    pub(super) batch_index: u32,
    pub(super) atom_count: u32,
    pub(super) bond_count: u32,
    pub(super) sampled_opaque: u32,
    pub(super) sampled_translucent: u32,
    pub(super) source_opaque: u32,
    pub(super) source_translucent: u32,
    pub(super) opaque_index_first: usize,
    pub(super) translucent_index_first: usize,
    pick_page: u32,
    radii: [f32; 2],
    model: Mat4,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PoseSourceStats {
    pub(super) resident: u64,
    pub(super) visible: u64,
}

pub(super) struct PosePlanScratch<'a> {
    pub(super) plans: &'a mut Vec<BatchPlan>,
    pub(super) batches: &'a mut Vec<PoseBatchGpu>,
    pub(super) samples: &'a mut Vec<PoseBatchSample>,
    pub(super) opaque_indices: &'a mut Vec<u32>,
    pub(super) translucent_indices: &'a mut Vec<u32>,
}

pub(super) fn plan_pose_batches<D: Device>(
    scene: &Scene,
    structures: &[GpuStructure<D>],
    quality: bool,
    scratch: PosePlanScratch<'_>,
) -> Result<PoseSourceStats, RenderError> {
    let PosePlanScratch {
        plans,
        batches,
        samples,
        opaque_indices,
        translucent_indices,
    } = scratch;
    plans.clear();
    batches.clear();
    samples.clear();
    opaque_indices.clear();
    translucent_indices.clear();
    let mut source = PoseSourceStats::default();
    collect_candidates(
        scene,
        structures,
        plans,
        samples,
        opaque_indices,
        translucent_indices,
        &mut source,
    )?;
    select_pose_batches(samples, quality);
    for sample in samples.iter() {
        let Some(plan) = plans.get_mut(sample.plan_index) else {
            continue;
        };
        plan.sampled_opaque = sample.selected_opaque;
        plan.sampled_translucent = sample.selected_translucent;
    }
    plans.retain(|plan| plan.sampled_opaque > 0 || plan.sampled_translucent > 0);
    build_gpu_batches(plans, batches)?;
    Ok(source)
}

fn collect_candidates<D: Device>(
    scene: &Scene,
    structures: &[GpuStructure<D>],
    plans: &mut Vec<BatchPlan>,
    samples: &mut Vec<PoseBatchSample>,
    opaque_indices: &mut Vec<u32>,
    translucent_indices: &mut Vec<u32>,
    stats: &mut PoseSourceStats,
) -> Result<(), RenderError> {
    for (handle, batch) in scene.ligand_pose_batches() {
        add_stat(
            &mut stats.resident,
            batch.poses().len(),
            "resident ligand poses",
        )?;
        if !batch.visible() {
            continue;
        }
        let Some(placed) = scene.structure(batch.owner()) else {
            continue;
        };
        let Some(pick_page) = structures
            .iter()
            .find(|structure| structure.handle == batch.owner())
            .map(|structure| structure.pick_page(pdviewx_core::EntityKind::LigandPoseBatch))
        else {
            continue;
        };
        let atom_count = checked_count(
            batch.template().atom_centers().len(),
            "ligand pose atom topology",
        )?;
        let bond_count = checked_count(
            batch.template().bond_frames().len(),
            "ligand pose bond topology",
        )?;
        let opaque_index_first = opaque_indices.len();
        let translucent_index_first = translucent_indices.len();
        let ranges = classify_poses(
            batch.poses(),
            sampling_seed(batch, placed.model_to_world),
            opaque_indices,
            translucent_indices,
        )?;
        if ranges.visible()? == 0 {
            continue;
        }
        stats.visible = stats
            .visible
            .checked_add(u64::from(ranges.visible()?))
            .ok_or_else(|| limit("visible ligand poses", u64::MAX))?;
        let primitive_cost =
            checked_add(atom_count, bond_count, "ligand pose topology primitives")?;
        let shape_count = u32::from(atom_count > 0) + u32::from(bond_count > 0);
        let plan_index = plans.len();
        plans.push(BatchPlan {
            handle,
            batch_index: 0,
            atom_count,
            bond_count,
            sampled_opaque: 0,
            sampled_translucent: 0,
            source_opaque: ranges.opaque,
            source_translucent: ranges.translucent,
            opaque_index_first,
            translucent_index_first,
            pick_page,
            radii: [
                batch.template().atom_radius(),
                batch.template().bond_radius(),
            ],
            model: placed.model_to_world,
        });
        samples.push(PoseBatchSample::new(
            plan_index,
            ranges.stable_key,
            ranges.opaque,
            ranges.translucent,
            primitive_cost,
            shape_count,
        ));
    }
    Ok(())
}

fn build_gpu_batches(
    plans: &mut [BatchPlan],
    batches: &mut Vec<PoseBatchGpu>,
) -> Result<(), RenderError> {
    let mut atom_first = 0u32;
    let mut bond_first = 0u32;
    let total_opaque = plans.iter().try_fold(0u32, |total, plan| {
        checked_add(total, plan.sampled_opaque, "selected opaque ligand poses")
    })?;
    let mut opaque_first = 0u32;
    let mut translucent_first = total_opaque;
    for plan in plans {
        plan.batch_index = checked_count(batches.len(), "ligand pose batch index")?;
        batches.push(PoseBatchGpu::new(
            [atom_first, plan.atom_count, bond_first, plan.bond_count],
            [opaque_first, translucent_first],
            [
                plan.sampled_opaque,
                plan.sampled_opaque,
                plan.sampled_translucent,
                plan.sampled_translucent,
            ],
            batch_entity(Scene::ligand_pose_batch_entity_row(plan.handle))?,
            plan.pick_page,
            plan.radii,
            plan.model,
        ));
        atom_first = checked_add(atom_first, plan.atom_count, "ligand pose atom topology")?;
        bond_first = checked_add(bond_first, plan.bond_count, "ligand pose bond topology")?;
        opaque_first = checked_add(
            opaque_first,
            plan.sampled_opaque,
            "selected opaque ligand poses",
        )?;
        translucent_first = checked_add(
            translucent_first,
            plan.sampled_translucent,
            "selected translucent ligand poses",
        )?;
    }
    Ok(())
}

fn add_stat(total: &mut u64, count: usize, resource: &'static str) -> Result<(), RenderError> {
    let count = u64::try_from(count).map_err(|_| limit(resource, u64::MAX))?;
    *total = total
        .checked_add(count)
        .ok_or_else(|| limit(resource, u64::MAX))?;
    Ok(())
}

fn limit(resource: &'static str, ceiling: u64) -> RenderError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource,
        limit: ceiling,
    }
    .into()
}

fn sampling_seed(batch: &LigandPoseBatch, model: Mat4) -> u64 {
    let mut hash = POSE_HASH_OFFSET;
    for value in model.to_cols_array() {
        mix_pose_key(&mut hash, value.to_bits());
    }
    mix_pose_key(&mut hash, batch.template().atom_radius().to_bits());
    mix_pose_key(&mut hash, batch.template().bond_radius().to_bits());
    for atom in batch.template().atom_centers() {
        for value in atom.to_array() {
            mix_pose_key(&mut hash, value.to_bits());
        }
    }
    for endpoints in batch.template().bond_indices() {
        mix_pose_key(&mut hash, endpoints[0]);
        mix_pose_key(&mut hash, endpoints[1]);
    }
    hash
}
