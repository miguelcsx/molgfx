//! Immutable typed columns that carry caller-defined data into visual programs.

#[cfg(test)]
#[path = "attribute_tests.rs"]
mod tests;

use crate::{CoreError, RowDomain};
use pdviewx_math::Rgba8;
use std::sync::Arc;

/// Physical row layout used by the shared GPU `array<u32>` arena.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum AttributeKind {
    /// One IEEE-754 scalar word per row. NaN represents missing data.
    Scalar = 0,
    /// One uninterpreted categorical word per row.
    Category = 1,
    /// Three tightly packed IEEE-754 words per row, with no `vec3` padding.
    Vector = 2,
    /// One packed RGBA8 word per row.
    Color = 3,
}

impl AttributeKind {
    /// Native byte stride retained in host and device storage.
    #[must_use]
    pub const fn stride(self) -> u32 {
        match self {
            Self::Scalar | Self::Category | Self::Color => 4,
            Self::Vector => 12,
        }
    }
}

/// Shared physical backing for a typed column.
#[derive(Clone, PartialEq, Debug)]
pub enum AttributeValues {
    /// One scalar per row.
    Scalar(Arc<[f32]>),
    /// One category id per row.
    Category(Arc<[u32]>),
    /// Three tightly packed components per row.
    Vector(Arc<[[f32; 3]]>),
    /// One packed color per row.
    Color(Arc<[Rgba8]>),
}

impl AttributeValues {
    /// Physical kind of this column.
    #[must_use]
    pub const fn kind(&self) -> AttributeKind {
        match self {
            Self::Scalar(_) => AttributeKind::Scalar,
            Self::Category(_) => AttributeKind::Category,
            Self::Vector(_) => AttributeKind::Vector,
            Self::Color(_) => AttributeKind::Color,
        }
    }

    /// Logical rows in this column.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Scalar(values) => values.len(),
            Self::Category(values) => values.len(),
            Self::Vector(values) => values.len(),
            Self::Color(values) => values.len(),
        }
    }

    /// Whether the column has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Native bytes suitable for one direct arena upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Scalar(values) => bytemuck::cast_slice(values),
            Self::Category(values) => bytemuck::cast_slice(values),
            Self::Vector(values) => bytemuck::cast_slice(values),
            Self::Color(values) => bytemuck::cast_slice(values),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        validate_values(self)
    }
}

/// One typed immutable column attached to an exact row domain.
#[derive(Clone, PartialEq, Debug)]
pub struct AttributeColumn {
    domain: RowDomain,
    descriptor: AttributeDescriptor,
    values: AttributeValues,
    fingerprint: u64,
}

/// Domain-independent introspection and reproducibility metadata for a column.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AttributeDescriptor {
    name: Arc<str>,
    quantity: Option<Arc<str>>,
    unit: Option<Arc<str>>,
    provenance: Option<Arc<str>>,
}

impl AttributeDescriptor {
    /// Creates a descriptor with only a caller-facing name.
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            quantity: None,
            unit: None,
            provenance: None,
        }
    }

    /// Declares an optional measured quantity and unit without interpreting it.
    #[must_use]
    pub fn with_quantity(
        mut self,
        quantity: impl Into<Arc<str>>,
        unit: impl Into<Arc<str>>,
    ) -> Self {
        self.quantity = Some(quantity.into());
        self.unit = Some(unit.into());
        self
    }

    /// Retains a caller method, dataset or evidence identifier.
    #[must_use]
    pub fn with_provenance(mut self, provenance: impl Into<Arc<str>>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    /// Caller-facing name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Optional uninterpreted quantity label.
    #[must_use]
    pub fn quantity(&self) -> Option<&str> {
        self.quantity.as_deref()
    }

    /// Optional caller unit string.
    #[must_use]
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }

    /// Optional caller provenance.
    #[must_use]
    pub fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty()
            || self
                .quantity
                .iter()
                .chain(&self.unit)
                .chain(&self.provenance)
                .any(|value| value.trim().is_empty())
        {
            return Err(invalid("attribute descriptor strings must be non-empty"));
        }
        Ok(())
    }
}

impl AttributeColumn {
    /// Validates and retains caller storage without copying it.
    ///
    /// # Errors
    ///
    /// Names and columns must be non-empty. Scalars may use NaN for missing
    /// values, vectors may use an all-NaN row, and infinity is always rejected.
    pub fn new(
        domain: RowDomain,
        name: impl Into<Arc<str>>,
        values: AttributeValues,
    ) -> Result<Self, CoreError> {
        Self::with_descriptor(domain, AttributeDescriptor::new(name), values)
    }

    /// Validates and retains caller storage plus generic metadata.
    ///
    /// # Errors
    ///
    /// Descriptor strings and values must satisfy the same finite, non-empty
    /// column contract as [`Self::new`].
    pub fn with_descriptor(
        domain: RowDomain,
        descriptor: AttributeDescriptor,
        values: AttributeValues,
    ) -> Result<Self, CoreError> {
        descriptor.validate()?;
        if values.is_empty() {
            return Err(invalid("attribute values must be non-empty"));
        }
        values.validate()?;
        let fingerprint = fingerprint(domain, &descriptor, &values);
        Ok(Self {
            domain,
            descriptor,
            values,
            fingerprint,
        })
    }

    /// Exact target table.
    #[must_use]
    pub const fn domain(&self) -> RowDomain {
        self.domain
    }

    /// Caller-authored display and introspection name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.descriptor.name()
    }

    /// Generic metadata retained by manifests and Studio introspection.
    #[must_use]
    pub const fn descriptor(&self) -> &AttributeDescriptor {
        &self.descriptor
    }

    /// Physical column kind.
    #[must_use]
    pub const fn kind(&self) -> AttributeKind {
        self.values.kind()
    }

    /// Native row stride in bytes.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.kind().stride()
    }

    /// Logical row count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the column contains no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.len() == 0
    }

    /// Shared typed backing storage.
    #[must_use]
    pub const fn values(&self) -> &AttributeValues {
        &self.values
    }

    /// Stable content key for upload and instruction deduplication.
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

fn validate_values(values: &AttributeValues) -> Result<(), CoreError> {
    let malformed = match values {
        AttributeValues::Scalar(values) => values.iter().any(|value| value.is_infinite()),
        AttributeValues::Category(_) | AttributeValues::Color(_) => false,
        AttributeValues::Vector(values) => values.iter().any(|value| {
            let finite = value.iter().all(|component| component.is_finite());
            let missing = value.iter().all(|component| component.is_nan());
            !finite && !missing
        }),
    };
    if malformed {
        Err(invalid(
            "attributes may contain finite values or explicit missing NaN rows, never infinity",
        ))
    } else {
        Ok(())
    }
}

fn fingerprint(
    domain: RowDomain,
    descriptor: &AttributeDescriptor,
    values: &AttributeValues,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let metadata = format!(
        "{domain:?}:{:?}:{}:{:?}:{:?}:{:?}",
        values.kind(),
        descriptor.name(),
        descriptor.quantity(),
        descriptor.unit(),
        descriptor.provenance()
    );
    for byte in metadata.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for chunk in values.as_bytes().chunks(8) {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        hash ^= u64::from_le_bytes(word);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidAttribute { reason }
}
