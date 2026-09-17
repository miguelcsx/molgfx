//! Deterministic accounting and eviction for recomputable derived data.

mod ledger;
mod model;

pub(crate) use ledger::DerivedCache;
pub use model::{DerivedCacheBudget, DerivedCacheUsage};
pub(crate) use model::{DerivedCacheKey, DerivedFootprint, MaterializationPlan};

#[cfg(test)]
pub(super) use model::DerivedCacheClass;

#[cfg(test)]
mod tests;
