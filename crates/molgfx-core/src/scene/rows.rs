//! Generic row identity shared by batches, attributes and relations.

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;

use crate::StructureHandle;
use crate::TemplatePartRef;
use crate::handle::{InstanceBatchHandle, PointBatchHandle, RelationBatchHandle};
use std::sync::Arc;

/// Caller-owned namespace for stable source-row identity.
///
/// This value is never reused or interpreted by the engine. Scene handles
/// remain generational and independent from this external namespace.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SourceNamespace(pub u64);

/// Stable source identity for one homogeneous row table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SourceRows {
    namespace: SourceNamespace,
    row_count: u32,
    keys: Option<Arc<[u64]>>,
}

impl SourceRows {
    /// Uses row order as identity inside `namespace` without a key allocation.
    #[must_use]
    pub const fn ordered(namespace: SourceNamespace, row_count: u32) -> Self {
        Self {
            namespace,
            row_count,
            keys: None,
        }
    }

    /// Retains explicit source keys without copying their shared allocation.
    ///
    /// # Errors
    ///
    /// Keys must be non-empty, unique and fit the portable row limit.
    pub fn keyed(namespace: SourceNamespace, keys: Arc<[u64]>) -> Result<Self, crate::CoreError> {
        let row_count =
            u32::try_from(keys.len()).map_err(|_| invalid("source row count exceeds u32"))?;
        if keys.is_empty() {
            return Err(invalid("source keys must be non-empty"));
        }
        let mut seen = hashbrown::HashSet::with_capacity(keys.len());
        if keys.iter().any(|key| !seen.insert(*key)) {
            return Err(invalid("source keys must be unique within their namespace"));
        }
        Ok(Self {
            namespace,
            row_count,
            keys: Some(keys),
        })
    }

    /// External namespace shared by related source tables.
    #[must_use]
    pub const fn namespace(&self) -> SourceNamespace {
        self.namespace
    }

    /// Number of rows in the table.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.row_count
    }

    /// Whether this table is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    /// Optional explicit keys in logical row order.
    #[must_use]
    pub fn keys(&self) -> Option<&Arc<[u64]>> {
        self.keys.as_ref()
    }

    /// Resolves the external key for a row. Unkeyed rows use their exact index.
    #[must_use]
    pub fn key(&self, row: u32) -> Option<u64> {
        if row >= self.row_count {
            return None;
        }
        self.keys.as_ref().map_or_else(
            || Some(u64::from(row)),
            |keys| keys.get(row as usize).copied(),
        )
    }
}

/// Exact homogeneous table targeted by attributes and selections.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum RowDomain {
    /// Source atoms of one placed structure.
    Atoms(StructureHandle),
    /// Generic point rows.
    Points(PointBatchHandle),
    /// Rigid transform rows in one instance batch.
    Instances(InstanceBatchHandle),
    /// Shared analytic parts, flattened sphere-first then capsule.
    TemplateParts(InstanceBatchHandle),
    /// Generic relation rows.
    Relations(RelationBatchHandle),
}

impl RowDomain {
    /// Whether rows in this domain resolve directly to world-space positions.
    #[must_use]
    pub const fn is_spatial(self) -> bool {
        !matches!(self, Self::Relations(_))
    }
}

/// Exact scene entity identity: generational domain plus logical row.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RowEntityRef {
    domain: RowDomain,
    row: u32,
}

/// Cold-path resolution of a flattened template-part picking row.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TemplatePartPick {
    reference: TemplatePartRef,
    instance_source_key: u64,
    part_source_key: u64,
}

impl TemplatePartPick {
    pub(crate) const fn new(
        reference: TemplatePartRef,
        instance_source_key: u64,
        part_source_key: u64,
    ) -> Self {
        Self {
            reference,
            instance_source_key,
            part_source_key,
        }
    }

    /// Exact scene occurrence selected by the attachment.
    #[must_use]
    pub const fn reference(self) -> TemplatePartRef {
        self.reference
    }

    /// External key of the rigid instance row.
    #[must_use]
    pub const fn instance_source_key(self) -> u64 {
        self.instance_source_key
    }

    /// External key of the shared local template part.
    #[must_use]
    pub const fn part_source_key(self) -> u64 {
        self.part_source_key
    }
}

impl RowEntityRef {
    /// Creates an unchecked reference. Scene APIs validate row and generation.
    #[must_use]
    pub const fn new(domain: RowDomain, row: u32) -> Self {
        Self { domain, row }
    }

    /// Homogeneous table containing this row.
    #[must_use]
    pub const fn domain(self) -> RowDomain {
        self.domain
    }

    /// Logical row within the domain.
    #[must_use]
    pub const fn row(self) -> u32 {
        self.row
    }
}

const fn invalid(reason: &'static str) -> crate::CoreError {
    crate::CoreError::InvalidAttribute { reason }
}
