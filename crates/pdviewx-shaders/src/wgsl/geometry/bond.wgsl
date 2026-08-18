// Specialized analytic bond impostors.
//
// Line and capsule rendering use independent entry points and payloads.
// Each instance expands to a four-vertex triangle strip.
//
// Flat per-bond data is authored only by vertices 0 and 2, the provoking
// vertices of the two strip triangles: (0,1,2) and (2,1,3).
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
//!include "include/bond/types.wgsl"
//!include "include/bond/line.wgsl"
//!include "include/bond/capsule.wgsl"


