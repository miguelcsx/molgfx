//! Borrowed byte views and metadata for generic paged payloads.

use super::generic::relations::{RelationUpload, RelationUploadLayout};
use crate::engine::ChunkResidencyError;
use pdviewx_core::{AttributeKind, ChunkDomainRef, ChunkPayload};

const POINT_STRIDE: u32 = 12;
const INSTANCE_STRIDE: u32 = 32;

pub(super) enum GenericBytes<'a> {
    Direct(&'a [u8]),
    Relations(RelationUpload<'a>),
}

impl GenericBytes<'_> {
    pub(super) fn byte_len(&self) -> Result<u64, ChunkResidencyError> {
        let bytes = match self {
            Self::Direct(bytes) => bytes.len(),
            Self::Relations(upload) => return Ok(upload.byte_len()),
        };
        u64::try_from(bytes).map_err(|_| ChunkResidencyError::SizeOverflow)
    }

    pub(super) fn write(&self, target: &mut [u8]) -> Result<(), ChunkResidencyError> {
        match self {
            Self::Direct(bytes) => target.copy_from_slice(bytes),
            Self::Relations(upload) => upload.write(target)?,
        }
        Ok(())
    }

    pub(super) const fn relation_layout(&self) -> Option<RelationUploadLayout> {
        match self {
            Self::Direct(_) => None,
            Self::Relations(upload) => Some(upload.layout()),
        }
    }
}

pub(super) fn generic_bytes(
    payload: &ChunkPayload,
) -> Result<(GenericBytes<'_>, u32), ChunkResidencyError> {
    match payload {
        ChunkPayload::PointBatch(points) => Ok((
            GenericBytes::Direct(bytemuck::cast_slice(points.positions().as_ref())),
            POINT_STRIDE,
        )),
        ChunkPayload::InstanceBatch(instances) => Ok((
            GenericBytes::Direct(bytemuck::cast_slice(instances.transforms().as_ref())),
            INSTANCE_STRIDE,
        )),
        ChunkPayload::RelationBatch(relations) => {
            let upload = RelationUpload::new(relations.relations().as_ref())?;
            let stride = upload.layout().uniform_stride();
            Ok((GenericBytes::Relations(upload), stride))
        }
        ChunkPayload::Attribute(attribute) => Ok((
            GenericBytes::Direct(attribute.values().as_bytes()),
            attribute.kind().stride(),
        )),
        _ => Err(ChunkResidencyError::UnsupportedPayload),
    }
}

pub(super) const fn attribute_target(payload: &ChunkPayload) -> Option<ChunkDomainRef> {
    match payload {
        ChunkPayload::Attribute(attribute) => Some(attribute.target()),
        _ => None,
    }
}

pub(super) const fn attribute_payload_kind(payload: &ChunkPayload) -> Option<AttributeKind> {
    match payload {
        ChunkPayload::Attribute(attribute) => Some(attribute.kind()),
        _ => None,
    }
}
