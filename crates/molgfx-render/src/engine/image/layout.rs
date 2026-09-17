//! Validated row-padded GPU readback layout.

use super::ImageConfig;
use crate::error::RenderError;

pub(in crate::engine) struct ImageLayout {
    pub(in crate::engine) row: usize,
    pub(in crate::engine) padded_row: u32,
    pub(in crate::engine) buffer_size: u64,
    pub(in crate::engine) height: usize,
}

impl ImageLayout {
    pub(in crate::engine) fn new(
        config: ImageConfig,
        bytes_per_pixel: u32,
    ) -> Result<Self, RenderError> {
        let row = config
            .width
            .checked_mul(bytes_per_pixel)
            .ok_or(RenderError::InvalidImageSize)?;
        let padded_row = row
            .checked_add(255)
            .map(|value| value & !255)
            .ok_or(RenderError::InvalidImageSize)?;
        let buffer_size = u64::from(padded_row)
            .checked_mul(u64::from(config.height))
            .ok_or(RenderError::InvalidImageSize)?;
        if config.width == 0 || config.height == 0 {
            return Err(RenderError::InvalidImageSize);
        }
        Ok(Self {
            row: row as usize,
            padded_row,
            buffer_size,
            height: config.height as usize,
        })
    }

    pub(in crate::engine) fn unpack(&self, mut mapped: Vec<u8>) -> Result<Vec<u8>, RenderError> {
        let mapped_size = (self.padded_row as usize)
            .checked_mul(self.height)
            .ok_or(RenderError::InvalidImageSize)?;
        if mapped.len() < mapped_size {
            return Err(RenderError::ImageEncoding {
                summary: "mapped image buffer is shorter than its declared layout".to_owned(),
            });
        }
        let tight_size = self
            .row
            .checked_mul(self.height)
            .ok_or(RenderError::InvalidImageSize)?;
        if self.row == self.padded_row as usize {
            mapped.truncate(tight_size);
            return Ok(mapped);
        }
        let padded_row = self.padded_row as usize;
        for row in 1..self.height {
            let source = row * padded_row;
            let target = row * self.row;
            mapped.copy_within(source..source + self.row, target);
        }
        mapped.truncate(tight_size);
        Ok(mapped)
    }
}
