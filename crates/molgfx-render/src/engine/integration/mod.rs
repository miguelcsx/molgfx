//! Engine-level integration tests over the mock device.

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod offscreen_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod exposure_tests;

#[cfg(test)]
mod pipeline_cache_tests;

#[cfg(test)]
mod indirect_arena_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod slot_cache_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod surface_field_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod color_scheme_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod residency_integration_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(in crate::engine) mod chunk_residency_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "../chunk/chunk_residency/generic_tests.rs"]
pub(in crate::engine) mod generic_chunk_residency_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "../chunk/chunk_residency/generic_visual_tests.rs"]
mod generic_chunk_visual_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "../chunk/chunk_residency/instance_timeline_tests.rs"]
mod chunk_instance_timeline_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod chunk_render_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod chunk_bond_render_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod asset_sharing_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod scene_identity_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod draw_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod generic_instance_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod generic_point_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod generic_relation_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod generic_timeline_budget_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod ligand_pose_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod putty_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod async_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod device_loss_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod picking_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod scalar_overlay_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod surface_component_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod surface_memory_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod interaction_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod label_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod property_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod trajectory_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod volume_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod segmentation_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod completion_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod volume_boundary_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod ribbon_quality_tests;
