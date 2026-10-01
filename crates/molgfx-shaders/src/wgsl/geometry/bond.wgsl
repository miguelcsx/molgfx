// Specialized analytic bond impostors.
//
// Line and capsule rendering use independent entry points and payloads.
// Each instance expands to a six-vertex triangle-list quad.
//
// Every vertex carries flat per-bond data so clipping preserves the payload.
//
// The two pipelines share only their payload and projection helpers, so each
// owns its stages under include/bond/ and neither carries the other's work.

//!include "include/camera.wgsl"
//!include "include/atom.wgsl"
//!include "include/intersect.wgsl"
//!include "include/material_lighting.wgsl"
//!include "include/oit_input.wgsl"
//!include "include/representation.wgsl"
//!include "include/motion.wgsl"
//!include "include/depth_tie.wgsl"
//!include "include/visual/fragment.wgsl"
//!include "include/bond/types.wgsl"
//!include "include/bond/line.wgsl"
//!include "include/bond/capsule.wgsl"
