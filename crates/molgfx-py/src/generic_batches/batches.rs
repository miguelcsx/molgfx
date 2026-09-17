//! Point and rigid-instance insertion from contiguous `NumPy` columns.

use super::super::{PyInstanceBatchHandle, PyPointBatchHandle, PyScene};
use super::{
    PyAnalyticTemplate,
    arrays::{contiguous, ordered_rows, validate_columns, vec3_rows},
};
use crate::error::{core, value};
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

#[pymethods]
impl PyScene {
    #[pyo3(signature = (namespace, positions, radius=0.1, color=(255, 255, 255, 255), sphere=false))]
    fn add_point_batch_from_numpy(
        &mut self,
        namespace: u64,
        positions: PyReadonlyArray2<'_, f32>,
        radius: f32,
        color: (u8, u8, u8, u8),
        sphere: bool,
    ) -> PyResult<PyPointBatchHandle> {
        let positions = vec3_rows("positions", &positions)?;
        let rows = ordered_rows(namespace, positions.len())?;
        let glyph = if sphere {
            molgfx::PointGlyph::Sphere
        } else {
            molgfx::PointGlyph::Disc
        };
        let style = molgfx::PointStyle {
            radius,
            color: molgfx::Rgba8::new(color.0, color.1, color.2, color.3),
        };
        core(molgfx::PointBatch::new(
            Arc::from(positions),
            rows,
            glyph,
            style,
        ))
        .map(|batch| self.inner.add_point_batch(batch).into())
    }

    #[pyo3(signature = (template, namespace, translations, orientations, scales, color=(255, 255, 255, 255)))]
    fn add_instance_batch_from_numpy(
        &mut self,
        template: PyRef<'_, PyAnalyticTemplate>,
        namespace: u64,
        translations: PyReadonlyArray2<'_, f32>,
        orientations: PyReadonlyArray2<'_, f32>,
        scales: PyReadonlyArray1<'_, f32>,
        color: (u8, u8, u8, u8),
    ) -> PyResult<PyInstanceBatchHandle> {
        validate_columns("translations", translations.shape(), 3)?;
        validate_columns("orientations", orientations.shape(), 4)?;
        let count = translations.shape()[0];
        if orientations.shape()[0] != count || scales.shape() != [count] {
            return Err(value(
                "instance transform columns must have the same row count",
            ));
        }
        let translations = contiguous("translations", &translations)?;
        let orientations = contiguous("orientations", &orientations)?;
        let scales = scales
            .as_slice()
            .map_err(|_| value("scales must be C-contiguous float32"))?;
        let mut transforms = Vec::with_capacity(count);
        for ((translation, orientation), scale) in translations
            .chunks_exact(3)
            .zip(orientations.chunks_exact(4))
            .zip(scales)
        {
            transforms.push(core(molgfx::RigidInstance::new(
                molgfx::Vec3::new(translation[0], translation[1], translation[2]),
                molgfx::Quat::from_array([
                    orientation[0],
                    orientation[1],
                    orientation[2],
                    orientation[3],
                ]),
                *scale,
            ))?);
        }
        let rows = ordered_rows(namespace, count)?;
        core(molgfx::InstanceBatch::new(
            Arc::clone(&template.0),
            Arc::from(transforms),
            rows,
        ))
        .map(|batch| {
            batch.with_style(molgfx::InstanceStyle {
                color: molgfx::Rgba8::new(color.0, color.1, color.2, color.3),
            })
        })
        .map(|batch| self.inner.add_instance_batch(batch).into())
    }
}
