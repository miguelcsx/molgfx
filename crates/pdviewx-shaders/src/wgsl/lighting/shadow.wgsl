// Scene-fit directional shadow map for analytic molecular geometry.
//
// Optimizations:
//   - six-vertex triangle-list impostors;
//   - orthographic shadow rays are always light-space -Z;
//   - sphere depth is solved directly from its circular cross-section;
//   - bond/primitive hits only resolve t; light-space hit Z is therefore -t;
//   - primitive type is specialized at pipeline creation;
//   - primitive quaternions are caller-normalized;
//   - ribbons use native indexed vertex fetching.
//
// Host contracts:
//   - visible_atoms / visible_bonds contain only drawable instances;
//   - shadow_primitives contains only shadow-casting opaque primitives;
//   - frame.shadow_view / shadow_inv_view are rigid transforms;
//   - frame.shadow_projection is orthographic;
//   - impostors use triangle-list with vertexCount = 6.
//
// The casters share only their light-space setup, so each family owns its
// stages under include/shadow/ and none carries another's intersection code.

//!include "include/camera.wgsl"
//!include "include/atom.wgsl"
//!include "include/quad.wgsl"
//!include "include/intersect.wgsl"
//!include "include/shadow/types.wgsl"
//!include "include/shadow/molecular.wgsl"
//!include "include/shadow/primitive.wgsl"

