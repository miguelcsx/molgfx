//! The render graph: declared passes over declared resources.
//!
//! A frame is a set of passes with explicit reads and writes. The graph
//! orders them (topological sort with a stable declaration-order tie-break),
//! aliases transient textures whose lifetimes do not overlap, and resolves
//! resource ids to concrete views at record time. A pass touches only what
//! it declared, which is what makes the ordering and aliasing sound.

mod context;
mod node;
mod pool;
mod schedule;

pub(crate) use context::{DisplayEncoding, PassContext, ResourceTable};
pub(crate) use node::{PassKind, PassNode, ResourceDesc, ResourceId, SizeClass};
pub(crate) use pool::{TransientPool, plan_aliases};
pub(crate) use schedule::schedule;
