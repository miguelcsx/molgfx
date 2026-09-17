// Analytic ray-primitive intersections in view space.
//
// Intervals are sorted [near, far]. A miss is represented by near > far.
// Capsule intersection reuses a single set of scalar products for the
// cylinder side and both spherical caps.
//
// One shape per file under include/intersect/, all built on the same interval
// algebra, so the realtime and quality paths intersect identical geometry.

//!include "include/intersect/interval.wgsl"
//!include "include/intersect/sphere.wgsl"
//!include "include/intersect/cylinder.wgsl"
//!include "include/intersect/capsule.wgsl"


