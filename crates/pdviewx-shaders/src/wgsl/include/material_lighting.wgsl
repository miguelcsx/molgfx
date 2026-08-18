// Shared energy-conserving molecular lighting for deferred opaque and forward OIT.
//
// Material payload is decoded once per shaded fragment. Isotropic surfaces
// never evaluate anisotropic GGX, tangent frames or anisotropic visibility.
// Fixed integer powers use multiplication instead of generic pow().
//
// The stages are separated under include/material/ so the deferred and
// forward paths include one lighting model rather than drifting into two.

//!include "include/surface_frame.wgsl"
//!include "include/material/payload.wgsl"
//!include "include/material/brdf.wgsl"
//!include "include/material/lights.wgsl"
//!include "include/material/shade.wgsl"
//!include "include/material/transparency.wgsl"


