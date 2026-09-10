//! Zero-copy adaptation of `pdbiox` out-of-core provider contracts.
//!
//! A provider dataset remains compact: this bridge stores its single
//! [`pdbiox::DatasetDescriptor`] and adapts only chunks delivered by the
//! caller. It neither performs I/O nor enumerates the logical dataset.

use crate::{
    ChunkBounds, ChunkData, ChunkDescriptor, ChunkFootprint, ChunkPayload, ChunkSpan,
    DatasetCatalog, DatasetError, DatasetId, PayloadKind,
};
use thiserror::Error;

/// Typed failure while adapting a native provider contract.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProviderBridgeError {
    /// A chunk belongs to another dataset.
    #[error("provider chunk belongs to dataset {actual}, expected {expected}")]
    DatasetMismatch {
        /// Dataset declared by the compact catalog.
        expected: u64,
        /// Dataset carried by the chunk.
        actual: u64,
    },
    /// A delivered payload category differs from its dataset contract.
    #[error("provider dataset expects {expected:?}, received {actual:?}")]
    PayloadKindMismatch {
        /// Category declared by the provider dataset.
        expected: pdbiox::PayloadKind,
        /// Category carried by the delivered chunk.
        actual: pdbiox::PayloadKind,
    },
    /// A chunk identity lies outside the provider's compact namespace.
    #[error("provider chunk {chunk} lies outside dataset {dataset}")]
    ChunkMismatch {
        /// Dataset owning the namespace.
        dataset: u64,
        /// Rejected chunk identity.
        chunk: u64,
    },
    /// The delivered row interval is empty or exceeds the logical dataset.
    #[error(
        "provider chunk rows [{first}, {end}) do not fit dataset {dataset} with {logical_rows} rows"
    )]
    RowRangeMismatch {
        /// Dataset owning the logical row space.
        dataset: u64,
        /// First delivered logical row.
        first: u64,
        /// Exclusive delivered logical end.
        end: u64,
        /// Declared logical dataset length.
        logical_rows: u64,
    },
    /// A regular provider returned boundaries different from its formula.
    #[error("provider chunk {chunk} does not match its regular row layout")]
    RegularLayoutMismatch {
        /// Rejected chunk identity.
        chunk: u64,
    },
    /// Provider metadata contains no finite bounds for this chunk.
    #[error("provider chunk {chunk} has no finite spatial bounds")]
    MissingBounds {
        /// Chunk whose bounds cannot be consumed by the renderer.
        chunk: u64,
    },
    /// A native pdviewx dataset invariant failed.
    #[error(transparent)]
    Dataset(#[from] DatasetError),
    /// A native provider invariant failed.
    #[error(transparent)]
    Provider(#[from] pdbiox::ProviderError),
}

/// Compact declarative bridge for one `pdbiox` provider dataset.
#[derive(Clone, Debug)]
pub struct ProviderDatasetBridge {
    catalog: DatasetCatalog,
    provider: pdbiox::DatasetDescriptor,
}

impl ProviderDatasetBridge {
    /// Retains only the provider's O(1) dataset descriptor.
    #[must_use]
    pub fn new(descriptor: pdbiox::DatasetDescriptor) -> Self {
        Self {
            catalog: DatasetCatalog::from_provider(descriptor),
            provider: descriptor,
        }
    }

    /// Compact pdviewx catalog; it contains no materialized chunk vector.
    #[must_use]
    pub const fn catalog(&self) -> &DatasetCatalog {
        &self.catalog
    }

    /// Adapts one delivered descriptor using explicit residency estimates.
    ///
    /// # Errors
    ///
    /// The descriptor must belong to this dataset, preserve exact regular
    /// boundaries when applicable, and carry a finite non-empty row range.
    pub fn chunk_descriptor(
        &self,
        source: pdbiox::ChunkDescriptor,
        bounds: ChunkBounds,
        footprint: ChunkFootprint,
    ) -> Result<ChunkDescriptor, ProviderBridgeError> {
        let dataset = self.provider;
        validate_source(dataset, source)?;
        Ok(ChunkDescriptor {
            id: crate::ChunkId::new(source.chunk().get()),
            parent: None,
            level: 0,
            rows: ChunkSpan::new(
                crate::LogicalRow::new(source.logical_start().get()),
                source.rows(),
            )?,
            bounds,
            payload_kind: adapt_kind(dataset.payload()),
            footprint,
        })
    }

    /// Retains a native structure chunk without copying any column.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch for foreign datasets, kinds, rows or bounds.
    pub fn structure_chunk(
        &self,
        chunk: pdbiox::StructureChunk,
        footprint: ChunkFootprint,
    ) -> Result<ChunkData, ProviderBridgeError> {
        self.require_kind(pdbiox::PayloadKind::Structure)?;
        let bounds = adapt_bounds(chunk.descriptor().chunk(), chunk.atoms().stats().bounds)?;
        self.finish(
            chunk.descriptor(),
            bounds,
            footprint,
            ChunkPayload::ProviderStructure(chunk),
        )
    }

    /// Retains native bond topology with global atom endpoints and no copies.
    ///
    /// Bounds are supplied by the caller because deriving them here would
    /// require touching atom coordinates or scanning the delivered bond rows.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch for foreign datasets, kinds, rows, bounds or
    /// understated host footprint.
    pub fn bond_chunk(
        &self,
        chunk: pdbiox::BondChunk,
        bounds: ChunkBounds,
        footprint: ChunkFootprint,
    ) -> Result<ChunkData, ProviderBridgeError> {
        self.require_kind(pdbiox::PayloadKind::BondTopology)?;
        self.finish(
            chunk.descriptor(),
            bounds,
            footprint,
            ChunkPayload::ProviderBond(chunk),
        )
    }

    /// Retains a native typed property chunk without conversion or decoding.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch for foreign datasets, kinds, rows or bounds.
    pub fn property_chunk(
        &self,
        chunk: pdbiox::PropertyChunk,
        footprint: ChunkFootprint,
    ) -> Result<ChunkData, ProviderBridgeError> {
        self.require_kind(pdbiox::PayloadKind::Property)?;
        let bounds = adapt_bounds(chunk.descriptor().chunk(), chunk.bounds())?;
        self.finish(
            chunk.descriptor(),
            bounds,
            footprint,
            ChunkPayload::ProviderProperty(chunk),
        )
    }

    /// Retains a native coordinate frame without copying its backing block.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch for foreign datasets, kinds, rows or bounds.
    pub fn frame_chunk(
        &self,
        chunk: pdbiox::FrameChunk,
        footprint: ChunkFootprint,
    ) -> Result<ChunkData, ProviderBridgeError> {
        self.require_kind(pdbiox::PayloadKind::Frame)?;
        let bounds = adapt_bounds(chunk.descriptor().chunk(), chunk.bounds())?;
        self.finish(
            chunk.descriptor(),
            bounds,
            footprint,
            ChunkPayload::ProviderFrame(chunk),
        )
    }

    fn finish(
        &self,
        source: pdbiox::ChunkDescriptor,
        bounds: ChunkBounds,
        footprint: ChunkFootprint,
        payload: ChunkPayload,
    ) -> Result<ChunkData, ProviderBridgeError> {
        let descriptor = self.chunk_descriptor(source, bounds, footprint)?;
        Ok(ChunkData::from_descriptor(
            DatasetId::new(source.dataset().get()),
            &descriptor,
            payload,
        )?)
    }

    fn require_kind(&self, actual: pdbiox::PayloadKind) -> Result<(), ProviderBridgeError> {
        let expected = self.provider.payload();
        if expected != actual {
            return Err(ProviderBridgeError::PayloadKindMismatch { expected, actual });
        }
        Ok(())
    }
}

const fn adapt_kind(kind: pdbiox::PayloadKind) -> PayloadKind {
    match kind {
        pdbiox::PayloadKind::Structure => PayloadKind::Structure,
        pdbiox::PayloadKind::BondTopology => PayloadKind::BondTopology,
        pdbiox::PayloadKind::Property => PayloadKind::ScalarProperty,
        pdbiox::PayloadKind::Frame => PayloadKind::Trajectory,
    }
}

fn validate_source(
    dataset: pdbiox::DatasetDescriptor,
    source: pdbiox::ChunkDescriptor,
) -> Result<(), ProviderBridgeError> {
    if source.dataset() != dataset.id() {
        return Err(ProviderBridgeError::DatasetMismatch {
            expected: dataset.id().get(),
            actual: source.dataset().get(),
        });
    }
    validate_chunk_identity(dataset, source.chunk())?;
    let first = source.logical_start().get();
    let end = first.checked_add(u64::from(source.rows())).ok_or(
        ProviderBridgeError::RowRangeMismatch {
            dataset: dataset.id().get(),
            first,
            end: u64::MAX,
            logical_rows: dataset.logical_rows(),
        },
    )?;
    if source.rows() == 0 || end > dataset.logical_rows() {
        return Err(ProviderBridgeError::RowRangeMismatch {
            dataset: dataset.id().get(),
            first,
            end,
            logical_rows: dataset.logical_rows(),
        });
    }
    if matches!(dataset.layout(), pdbiox::ChunkLayout::Regular { .. })
        && dataset.regular_chunk(source.chunk())? != source
    {
        return Err(ProviderBridgeError::RegularLayoutMismatch {
            chunk: source.chunk().get(),
        });
    }
    Ok(())
}

fn validate_chunk_identity(
    dataset: pdbiox::DatasetDescriptor,
    chunk: pdbiox::ChunkId,
) -> Result<(), ProviderBridgeError> {
    let Some(ordinal) = chunk.get().checked_sub(dataset.first_chunk().get()) else {
        return Err(chunk_mismatch(dataset, chunk));
    };
    if ordinal >= dataset.chunk_count() {
        return Err(chunk_mismatch(dataset, chunk));
    }
    Ok(())
}

const fn chunk_mismatch(
    dataset: pdbiox::DatasetDescriptor,
    chunk: pdbiox::ChunkId,
) -> ProviderBridgeError {
    ProviderBridgeError::ChunkMismatch {
        dataset: dataset.id().get(),
        chunk: chunk.get(),
    }
}

fn adapt_bounds(
    chunk: pdbiox::ChunkId,
    bounds: pdbiox::Aabb,
) -> Result<ChunkBounds, ProviderBridgeError> {
    if bounds.is_empty() {
        return Err(ProviderBridgeError::MissingBounds { chunk: chunk.get() });
    }
    ChunkBounds::new(bounds.min, bounds.max).map_err(ProviderBridgeError::from)
}
