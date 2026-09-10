//! Validated association between catalog descriptors and shared payloads.

use crate::{
    AttributeChunkPayload, ChunkId, ChunkSpan, DatasetCatalog, DatasetError, DatasetId,
    InstanceChunkPayload, LabelBrickPayload, MeshChunkPayload, OccupancyBrickPayload, PayloadKind,
    PointChunkPayload, PropertyChunkPayload, ProxyChunkPayload, RelationChunkPayload,
    ScalarChunkPayload, StructureChunkPayload, TrajectoryChunkPayload, TrajectoryFramesPayload,
    VolumeBrickPayload,
};

/// One of the host-neutral payload categories declared by [`PayloadKind`].
#[derive(Clone, Debug)]
pub enum ChunkPayload {
    /// Molecular atom columns.
    Structure(StructureChunkPayload),
    /// Scalar row values.
    ScalarProperty(ScalarChunkPayload),
    /// A physically typed property column with optional validity bits.
    Property(PropertyChunkPayload),
    /// Decoded trajectory positions.
    Trajectory(TrajectoryChunkPayload),
    /// Multiple contiguous decoded frames over one topology row span.
    TrajectoryFrames(TrajectoryFramesPayload),
    /// Scalar volume brick.
    VolumeBrick(VolumeBrickPayload),
    /// Categorical label brick.
    LabelBrick(LabelBrickPayload),
    /// Bit-packed occupancy brick in the categorical-brick category.
    OccupancyBrick(OccupancyBrickPayload),
    /// Indexed triangles.
    Mesh(MeshChunkPayload),
    /// Coarse point proxies.
    Proxy(ProxyChunkPayload),
    /// Native `pdbiox` structure storage retained without copying.
    ProviderStructure(pdbiox::StructureChunk),
    /// Native `pdbiox` bond topology retained without materialising endpoints.
    ProviderBond(pdbiox::BondChunk),
    /// Native typed `pdbiox` property storage retained without conversion.
    ProviderProperty(pdbiox::PropertyChunk),
    /// Native `pdbiox` frame storage retained without copying.
    ProviderFrame(pdbiox::FrameChunk),
    /// Generic tightly packed point positions.
    PointBatch(PointChunkPayload),
    /// Generic tightly packed rigid transforms.
    InstanceBatch(InstanceChunkPayload),
    /// Generic globally anchored relation rows.
    RelationBatch(RelationChunkPayload),
    /// Generic native-width visual attribute values.
    Attribute(AttributeChunkPayload),
}

impl ChunkPayload {
    /// Declared category of this payload.
    #[must_use]
    pub const fn kind(&self) -> PayloadKind {
        match self {
            Self::Structure(_) | Self::ProviderStructure(_) => PayloadKind::Structure,
            Self::ProviderBond(_) => PayloadKind::BondTopology,
            Self::ScalarProperty(_) | Self::Property(_) | Self::ProviderProperty(_) => {
                PayloadKind::ScalarProperty
            }
            Self::Trajectory(_) | Self::TrajectoryFrames(_) | Self::ProviderFrame(_) => {
                PayloadKind::Trajectory
            }
            Self::VolumeBrick(_) => PayloadKind::VolumeBrick,
            Self::LabelBrick(_) | Self::OccupancyBrick(_) => PayloadKind::LabelBrick,
            Self::Mesh(_) => PayloadKind::Mesh,
            Self::Proxy(_) => PayloadKind::Proxy,
            Self::PointBatch(_) => PayloadKind::PointBatch,
            Self::InstanceBatch(_) => PayloadKind::InstanceBatch,
            Self::RelationBatch(_) => PayloadKind::RelationBatch,
            Self::Attribute(_) => PayloadKind::Attribute,
        }
    }

    /// Number of rows addressed by the descriptor span.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] if a platform can represent a
    /// payload allocation larger than the chunk-local `u32` address space.
    pub fn row_count(&self) -> Result<u32, DatasetError> {
        match self {
            Self::Structure(value) => row_count(value.positions().len()),
            Self::ScalarProperty(value) => row_count(value.values().len()),
            Self::Property(value) => Ok(value.row_count()),
            Self::Trajectory(value) => row_count(value.positions().len()),
            Self::TrajectoryFrames(value) => Ok(value.rows_per_frame()),
            Self::VolumeBrick(value) => row_count(value.values().len()),
            Self::LabelBrick(value) => row_count(value.labels().len()),
            Self::OccupancyBrick(value) => Ok(value.voxel_count()),
            Self::Mesh(value) => row_count(value.positions().len()),
            Self::Proxy(value) => row_count(value.centers().len()),
            Self::ProviderStructure(value) => Ok(value.descriptor().rows()),
            Self::ProviderBond(value) => Ok(value.descriptor().rows()),
            Self::ProviderProperty(value) => Ok(value.descriptor().rows()),
            Self::ProviderFrame(value) => Ok(value.descriptor().rows()),
            Self::PointBatch(value) => value.row_count(),
            Self::InstanceBatch(value) => value.row_count(),
            Self::RelationBatch(value) => value.row_count(),
            Self::Attribute(value) => value.row_count(),
        }
    }

    /// Minimum bytes directly retained by shared payload columns.
    ///
    /// Provider-native views may retain additional shared backing storage; the
    /// caller accounts for that conservative source ownership in the catalog.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::PayloadByteSizeOverflow`] on checked arithmetic
    /// overflow.
    pub fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        match self {
            Self::Structure(value) => value.minimum_host_bytes(),
            Self::ScalarProperty(value) => bytes(value.values().len(), 4),
            Self::Property(value) => value.minimum_host_bytes(),
            Self::Trajectory(value) => bytes(value.positions().len(), 12),
            Self::TrajectoryFrames(value) => value.minimum_host_bytes(),
            Self::VolumeBrick(value) => bytes(value.values().len(), 4),
            Self::LabelBrick(value) => bytes(value.labels().len(), 4),
            Self::OccupancyBrick(value) => bytes(value.words().len(), 8),
            Self::Mesh(value) => value.minimum_host_bytes(),
            Self::Proxy(value) => value.minimum_host_bytes(),
            Self::ProviderStructure(value) => bytes(value.positions().len(), 12),
            Self::ProviderBond(value) => value
                .minimum_host_bytes()
                .map_err(|_| DatasetError::PayloadByteSizeOverflow),
            Self::ProviderProperty(_) => Ok(0),
            Self::ProviderFrame(value) => bytes(value.positions().len(), 12),
            Self::PointBatch(value) => value.minimum_host_bytes(),
            Self::InstanceBatch(value) => value.minimum_host_bytes(),
            Self::RelationBatch(value) => value.minimum_host_bytes(),
            Self::Attribute(value) => value.minimum_host_bytes(),
        }
    }
}

/// A validated payload associated with one catalog descriptor.
#[derive(Clone, Debug)]
pub struct ChunkData {
    dataset: DatasetId,
    chunk: ChunkId,
    span: ChunkSpan,
    footprint: crate::ChunkFootprint,
    payload: ChunkPayload,
}

impl ChunkData {
    /// Matches type and row count against catalog metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the chunk is absent or its payload type or
    /// row count differs from the catalog descriptor.
    pub fn new(
        catalog: &DatasetCatalog,
        chunk: ChunkId,
        payload: ChunkPayload,
    ) -> Result<Self, DatasetError> {
        let Some(descriptor) = catalog.get(chunk) else {
            return Err(DatasetError::MissingChunk { chunk });
        };
        Self::from_descriptor(catalog.dataset_id(), descriptor, payload)
    }

    pub(crate) fn from_descriptor(
        dataset: DatasetId,
        descriptor: &crate::ChunkDescriptor,
        payload: ChunkPayload,
    ) -> Result<Self, DatasetError> {
        let chunk = descriptor.id;
        let actual = payload.kind();
        if descriptor.payload_kind != actual {
            return Err(DatasetError::PayloadKindMismatch {
                chunk,
                expected: descriptor.payload_kind,
                actual,
            });
        }
        let actual = payload.row_count()?;
        if descriptor.rows.row_count() != actual {
            return Err(DatasetError::PayloadRowCountMismatch {
                chunk,
                expected: descriptor.rows.row_count(),
                actual,
            });
        }
        let required = payload.minimum_host_bytes()?;
        if required > descriptor.footprint.host_bytes {
            return Err(DatasetError::PayloadFootprintTooSmall {
                chunk,
                required,
                declared: descriptor.footprint.host_bytes,
            });
        }
        Ok(Self {
            dataset,
            chunk,
            span: descriptor.rows,
            footprint: descriptor.footprint,
            payload,
        })
    }

    /// Dataset expected to consume this payload.
    #[must_use]
    pub const fn dataset_id(&self) -> DatasetId {
        self.dataset
    }

    /// Catalog chunk fulfilled by this payload.
    #[must_use]
    pub const fn chunk_id(&self) -> ChunkId {
        self.chunk
    }

    /// Full-dataset logical rows represented by this payload.
    #[must_use]
    pub const fn span(&self) -> ChunkSpan {
        self.span
    }

    /// Conservative resource charges validated with this payload.
    #[must_use]
    pub const fn footprint(&self) -> crate::ChunkFootprint {
        self.footprint
    }

    /// Validated shared payload.
    #[must_use]
    pub const fn payload(&self) -> &ChunkPayload {
        &self.payload
    }
}

pub(super) fn row_count(value: usize) -> Result<u32, DatasetError> {
    u32::try_from(value).map_err(|_| invalid("payload exceeds chunk-local u32 addressing"))
}

pub(super) fn bytes(count: usize, width: u64) -> Result<u64, DatasetError> {
    u64::try_from(count)
        .ok()
        .and_then(|value| value.checked_mul(width))
        .ok_or(DatasetError::PayloadByteSizeOverflow)
}

pub(super) const fn invalid(reason: &'static str) -> DatasetError {
    DatasetError::InvalidPayload { reason }
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
