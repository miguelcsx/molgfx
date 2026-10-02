//! Scalar columns the structure defines for itself, bound on demand.
//!
//! A colour theme such as "by chain" or "by hydropathy" needs one number per
//! atom. The number is a pure function of the structure, so the scene derives
//! it the first time a colour asks for it, binds it through the same scalar
//! property path as any caller-supplied column, and caches it by name. The
//! renderer, the geometry and the serialized scene therefore see an ordinary
//! property; only its descriptor says it is derived, so a replica can rebuild
//! it from the structure instead of receiving it.

mod category;
mod metric;

pub use category::AtomCategory;
pub use metric::AtomMetric;

mod column;

pub(crate) use column::DERIVED_FORMAT;
pub use column::DerivedColumn;
