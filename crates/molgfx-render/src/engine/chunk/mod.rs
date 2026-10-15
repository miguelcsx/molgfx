//! Provider chunk residency, placement and draw planning for out-of-core scenes.

pub(in crate::engine) mod brick_atlas;
pub(in crate::engine) mod chunk_api;
pub(crate) mod chunk_draw_plan;
pub(in crate::engine) mod chunk_placement;
pub(in crate::engine) mod chunk_residency;
pub(in crate::engine) mod chunk_residency_support;
pub(in crate::engine) mod chunk_residency_types;
pub(in crate::engine) mod derived_cache;
