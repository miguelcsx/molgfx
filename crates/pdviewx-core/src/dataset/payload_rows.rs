//! Shared molecular, scalar and trajectory row payloads.

use super::payload::{bytes, invalid, row_count};
use crate::DatasetError;
use std::sync::Arc;

/// Dense immutable columns for one molecular chunk.
#[derive(Clone, Debug)]
pub struct StructureChunkPayload {
    positions: Arc<[[f32; 3]]>,
    elements: Arc<[u16]>,
    residues: Arc<[u32]>,
    radii: Arc<[f32]>,
}

impl StructureChunkPayload {
    /// Validates aligned, finite atom columns without copying them.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] when columns are empty,
    /// misaligned, non-finite or exceed chunk-local addressing.
    pub fn new(
        positions: Arc<[[f32; 3]]>,
        elements: Arc<[u16]>,
        residues: Arc<[u32]>,
        radii: Arc<[f32]>,
    ) -> Result<Self, DatasetError> {
        let rows = row_count(positions.len())?;
        if rows == 0
            || elements.len() != positions.len()
            || residues.len() != positions.len()
            || radii.len() != positions.len()
        {
            return Err(invalid("structure columns must be non-empty and aligned"));
        }
        if positions.iter().flatten().any(|value| !value.is_finite())
            || radii
                .iter()
                .any(|radius| !radius.is_finite() || *radius < 0.0)
        {
            return Err(invalid("structure coordinates and radii must be finite"));
        }
        Ok(Self {
            positions,
            elements,
            residues,
            radii,
        })
    }

    /// Model-space coordinates shared with the caller.
    #[must_use]
    pub fn positions(&self) -> &Arc<[[f32; 3]]> {
        &self.positions
    }

    /// Atomic numbers aligned to positions.
    #[must_use]
    pub fn elements(&self) -> &Arc<[u16]> {
        &self.elements
    }

    /// Dataset-defined residue identities aligned to positions.
    #[must_use]
    pub fn residues(&self) -> &Arc<[u32]> {
        &self.residues
    }

    /// Non-negative display radii aligned to positions.
    #[must_use]
    pub fn radii(&self) -> &Arc<[f32]> {
        &self.radii
    }

    pub(super) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        let positions = bytes(self.positions.len(), 12)?;
        let elements = bytes(self.elements.len(), 2)?;
        let residues = bytes(self.residues.len(), 4)?;
        let radii = bytes(self.radii.len(), 4)?;
        positions
            .checked_add(elements)
            .and_then(|value| value.checked_add(residues))
            .and_then(|value| value.checked_add(radii))
            .ok_or(DatasetError::PayloadByteSizeOverflow)
    }
}

/// One scalar value per logical row.
#[derive(Clone, Debug)]
pub struct ScalarChunkPayload {
    values: Arc<[f32]>,
}

impl ScalarChunkPayload {
    /// Validates a non-empty scalar row without copying it.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] for empty, infinite or
    /// over-large rows.
    pub fn new(values: Arc<[f32]>) -> Result<Self, DatasetError> {
        row_count(values.len())?;
        if values.is_empty() || values.iter().any(|value| value.is_infinite()) {
            return Err(invalid("scalar values must be non-empty and not infinite"));
        }
        Ok(Self { values })
    }

    /// Scalar row shared with the caller.
    #[must_use]
    pub fn values(&self) -> &Arc<[f32]> {
        &self.values
    }
}

/// Physical storage for one typed property column.
#[derive(Clone, Debug)]
pub enum PropertyValues {
    /// Bit-packed boolean values and their logical row count.
    Boolean {
        /// Number of represented logical rows.
        rows: u32,
        /// Packed values, least-significant bit first within each word.
        words: Arc<[u64]>,
    },
    /// Signed integer values.
    Integer(Arc<[i64]>),
    /// Double-precision scientific values.
    Real(Arc<[f64]>),
    /// Interned dictionary identities.
    Symbol(Arc<[u32]>),
}

/// A typed property column with optional bit-packed validity.
#[derive(Clone, Debug)]
pub struct PropertyChunkPayload {
    values: PropertyValues,
    validity: Option<Arc<[u64]>>,
    rows: u32,
}

impl PropertyChunkPayload {
    /// Validates compact physical storage without projecting per-row objects.
    ///
    /// Real values must be finite when valid. Missing rows may retain any bit
    /// pattern because consumers must consult the validity mask first.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty, over-large, malformed or non-canonical
    /// columns.
    pub fn new(values: PropertyValues, validity: Option<Arc<[u64]>>) -> Result<Self, DatasetError> {
        let rows = property_rows(&values)?;
        let expected_words = packed_word_count(rows)?;
        if let PropertyValues::Boolean { words, .. } = &values {
            validate_words(words, rows, expected_words)?;
        }
        if let Some(mask) = &validity {
            validate_words(mask, rows, expected_words)?;
        }
        if let PropertyValues::Real(values) = &values {
            validate_reals(values, validity.as_deref())?;
        }
        Ok(Self {
            values,
            validity,
            rows,
        })
    }

    /// Number of rows represented by the column.
    #[must_use]
    pub const fn row_count(&self) -> u32 {
        self.rows
    }

    /// Shared physical values.
    #[must_use]
    pub const fn values(&self) -> &PropertyValues {
        &self.values
    }

    /// Optional bit-packed validity, where one means present.
    #[must_use]
    pub fn validity(&self) -> Option<&Arc<[u64]>> {
        self.validity.as_ref()
    }

    pub(super) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        let values = match &self.values {
            PropertyValues::Boolean { words, .. } => bytes(words.len(), 8)?,
            PropertyValues::Integer(values) => bytes(values.len(), 8)?,
            PropertyValues::Real(values) => bytes(values.len(), 8)?,
            PropertyValues::Symbol(values) => bytes(values.len(), 4)?,
        };
        let validity = match &self.validity {
            Some(mask) => bytes(mask.len(), 8)?,
            None => 0,
        };
        values
            .checked_add(validity)
            .ok_or(DatasetError::PayloadByteSizeOverflow)
    }
}

/// One decoded trajectory frame segment.
#[derive(Clone, Debug)]
pub struct TrajectoryChunkPayload {
    frame: u64,
    time_seconds: f64,
    positions: Arc<[[f32; 3]]>,
}

impl TrajectoryChunkPayload {
    /// Validates a finite, non-empty coordinate frame without copying it.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] for invalid time, coordinates
    /// or chunk-local row count.
    pub fn new(
        frame: u64,
        time_seconds: f64,
        positions: Arc<[[f32; 3]]>,
    ) -> Result<Self, DatasetError> {
        row_count(positions.len())?;
        if positions.is_empty()
            || !time_seconds.is_finite()
            || positions.iter().flatten().any(|value| !value.is_finite())
        {
            return Err(invalid("trajectory time and positions must be finite"));
        }
        Ok(Self {
            frame,
            time_seconds,
            positions,
        })
    }

    /// Stable caller frame identity.
    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    /// Frame time in seconds.
    #[must_use]
    pub const fn time_seconds(&self) -> f64 {
        self.time_seconds
    }

    /// Topology-aligned coordinates shared with the caller.
    #[must_use]
    pub fn positions(&self) -> &Arc<[[f32; 3]]> {
        &self.positions
    }
}

/// Several contiguous decoded trajectory frames sharing one topology span.
#[derive(Clone, Debug)]
pub struct TrajectoryFramesPayload {
    first_frame: u64,
    times_seconds: Arc<[f64]>,
    rows_per_frame: u32,
    positions: Arc<[[f32; 3]]>,
}

impl TrajectoryFramesPayload {
    /// Validates frame identities, times and packed frame-major coordinates.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty input, non-finite values, identity
    /// overflow or a coordinate count inconsistent with `rows_per_frame`.
    pub fn new(
        first_frame: u64,
        times_seconds: Arc<[f64]>,
        rows_per_frame: u32,
        positions: Arc<[[f32; 3]]>,
    ) -> Result<Self, DatasetError> {
        let frames = row_count(times_seconds.len())?;
        if frames == 0 || rows_per_frame == 0 {
            return Err(invalid("trajectory frame batches must be non-empty"));
        }
        first_frame
            .checked_add(u64::from(frames - 1))
            .ok_or_else(|| invalid("trajectory frame identity range overflows u64"))?;
        let expected = u64::from(frames)
            .checked_mul(u64::from(rows_per_frame))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(DatasetError::PayloadByteSizeOverflow)?;
        if positions.len() != expected
            || times_seconds.iter().any(|value| !value.is_finite())
            || positions.iter().flatten().any(|value| !value.is_finite())
        {
            return Err(invalid(
                "trajectory frame times and packed coordinates must be finite and aligned",
            ));
        }
        Ok(Self {
            first_frame,
            times_seconds,
            rows_per_frame,
            positions,
        })
    }

    /// Identity of the first frame.
    #[must_use]
    pub const fn first_frame(&self) -> u64 {
        self.first_frame
    }

    /// Contiguous frame times.
    #[must_use]
    pub fn times_seconds(&self) -> &Arc<[f64]> {
        &self.times_seconds
    }

    /// Topology rows represented by every frame.
    #[must_use]
    pub const fn rows_per_frame(&self) -> u32 {
        self.rows_per_frame
    }

    /// Frame-major coordinates shared with the caller.
    #[must_use]
    pub fn positions(&self) -> &Arc<[[f32; 3]]> {
        &self.positions
    }

    pub(super) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        bytes(self.times_seconds.len(), 8)?
            .checked_add(bytes(self.positions.len(), 12)?)
            .ok_or(DatasetError::PayloadByteSizeOverflow)
    }
}

fn property_rows(values: &PropertyValues) -> Result<u32, DatasetError> {
    let rows = match values {
        PropertyValues::Boolean { rows, .. } => *rows,
        PropertyValues::Integer(values) => row_count(values.len())?,
        PropertyValues::Real(values) => row_count(values.len())?,
        PropertyValues::Symbol(values) => row_count(values.len())?,
    };
    if rows == 0 {
        return Err(invalid("property columns must be non-empty"));
    }
    Ok(rows)
}

fn packed_word_count(rows: u32) -> Result<usize, DatasetError> {
    u64::from(rows)
        .checked_add(63)
        .map(|value| value / 64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(DatasetError::PayloadByteSizeOverflow)
}

fn validate_words(words: &[u64], rows: u32, expected: usize) -> Result<(), DatasetError> {
    if words.len() != expected {
        return Err(invalid(
            "bit-packed property storage has the wrong word count",
        ));
    }
    let used = rows % 64;
    let Some(last) = words.last().copied() else {
        return Err(invalid("bit-packed property storage must be non-empty"));
    };
    if used != 0 && last >> used != 0 {
        return Err(invalid("unused property bits must be zero"));
    }
    Ok(())
}

fn validate_reals(values: &[f64], validity: Option<&[u64]>) -> Result<(), DatasetError> {
    for (index, value) in values.iter().enumerate() {
        let valid = match validity {
            Some(mask) => (mask[index / 64] & (1u64 << (index % 64))) != 0,
            None => true,
        };
        if valid && !value.is_finite() {
            return Err(invalid("valid real property values must be finite"));
        }
    }
    Ok(())
}
