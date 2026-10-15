//! Deterministic accounting and eviction for recomputable derived data.

mod ledger;
mod model;

pub(crate) use ledger::DerivedCache;
pub use model::{DerivedCacheBudget, DerivedCacheUsage};
pub(crate) use model::{DerivedCacheClass, DerivedCacheKey, DerivedFootprint, MaterializationPlan};

#[cfg(test)]
mod tests;
