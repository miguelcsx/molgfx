//! Mock command recording, including actual-pass timestamp metadata.

use super::*;
use molgfx_gpu::{PassTimestampCapture, TimestampPassKind};

impl molgfx_gpu::CommandEncoder<MockDevice> for MockEncoder {
    type RenderPass<'e> = MockPass<'e>;
    type ComputePass<'e> = MockPass<'e>;

    fn set_timestamp_capture(
        &mut self,
        capture: Option<PassTimestampCapture<MockQuerySet>>,
    ) -> Option<PassTimestampCapture<MockQuerySet>> {
        std::mem::replace(&mut self.timestamps, capture)
    }

    fn set_timestamp_sample(&mut self, sample: Option<u32>) {
        if let Some(capture) = &mut self.timestamps {
            capture.set_sample(sample);
        }
    }

    fn begin_render_pass<'e>(&'e mut self, desc: &RenderPassDesc<'_, MockDevice>) -> MockPass<'e> {
        if let Some(capture) = &mut self.timestamps {
            capture.record_pass(
                desc.label,
                TimestampPassKind::Render,
                desc.timestamps.is_some(),
            );
        }
        MockPass { log: &self.log }
    }

    fn begin_compute_pass<'e>(
        &'e mut self,
        desc: &ComputePassDesc<'_, MockDevice>,
    ) -> MockPass<'e> {
        if let Some(capture) = &mut self.timestamps {
            capture.record_pass(
                desc.label,
                TimestampPassKind::Compute,
                desc.timestamps.is_some(),
            );
        }
        if let Ok(mut passes) = self.log.compute_passes.lock() {
            passes.push(desc.label);
        }
        MockPass { log: &self.log }
    }

    fn copy_buffer_to_buffer(
        &mut self,
        source: &MockBuffer,
        _so: u64,
        destination: &MockBuffer,
        _do_: u64,
        bytes: u64,
    ) {
        if let Ok(mut copies) = self.log.buffer_copies.lock() {
            copies.push((source.id, destination.id, bytes));
        }
    }

    fn copy_texture_to_buffer(
        &mut self,
        _src: &MockTexture,
        _origin: (u32, u32),
        _size: (u32, u32),
        _bpr: u32,
        _destination_offset: u64,
        _dst: &MockBuffer,
    ) {
    }

    fn resolve_query_set(
        &mut self,
        _queries: &MockQuerySet,
        _range: Range<u32>,
        _dst: &MockBuffer,
        _offset: u64,
    ) {
    }

    fn build_blas(
        &mut self,
        _desc: &molgfx_gpu::BlasBuildDesc<'_, MockDevice>,
    ) -> Result<(), GpuError> {
        fail_mock_ray_query(&self.log)?;
        self.log.blas_builds.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn build_tlas(&mut self, _tlas: &MockTlas) -> Result<(), GpuError> {
        fail_mock_ray_query(&self.log)?;
        self.log.tlas_builds.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

pub(super) fn fail_mock_ray_query(log: &MockLog) -> Result<(), GpuError> {
    if log.fail_ray_query.swap(false, Ordering::AcqRel) {
        Err(GpuError::DeviceLost)
    } else {
        Ok(())
    }
}

impl molgfx_gpu::RenderPassEncoder<MockDevice> for MockPass<'_> {
    fn set_pipeline(&mut self, _pipeline: &u32) {}
    fn set_bind_group(&mut self, _index: u32, _group: &u32, _offsets: &[u32]) {}
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        if let Ok(mut draws) = self.log.draws.lock() {
            draws.push((vertices, instances));
        }
    }
    fn draw_indirect(&mut self, args: &MockBuffer, offset: u64) {
        if let Ok(mut indirect) = self.log.indirect_draws.lock() {
            indirect.push((args.id, offset));
        }
    }
}

impl molgfx_gpu::ComputePassEncoder<MockDevice> for MockPass<'_> {
    fn set_pipeline(&mut self, _pipeline: &u32) {}
    fn set_bind_group(&mut self, _index: u32, _group: &u32, _offsets: &[u32]) {}
    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        if let Ok(mut dispatches) = self.log.dispatches.lock() {
            dispatches.push((x, y, z));
        }
    }
}

#[cfg(test)]
#[path = "encoder_tests.rs"]
mod tests;
