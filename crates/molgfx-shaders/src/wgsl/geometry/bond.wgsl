// Specialized analytic bond impostors.
//
// Line and capsule rendering use independent entry points and payloads.
// Each instance expands to a six-vertex triangle-list quad.
//
// Flat per-bond data is authored only by vertices 0 and 3, the provoking
// vertices of the two triangles: (0,1,2) and (3,4,5).
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
//!include "include/visual/fragment.wgsl"
//!include "include/bond/types.wgsl"
//!include "include/bond/line.wgsl"
//!include "include/bond/capsule.wgsl"
