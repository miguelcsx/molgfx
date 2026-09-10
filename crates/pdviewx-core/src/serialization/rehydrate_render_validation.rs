// Validation helpers for render description rehydration.

fn parse_appearance(
    scene: &Scene,
    value: &types::PropertyAppearanceDescription,
) -> Result<PropertyAppearance, crate::CoreError> {
    let raw = resolve_raw(value.property);
    resolve_existing(scene.properties.get(raw))?;
    PropertyAppearance::new(
        crate::AtomPropertyHandle(raw),
        value.domain,
        value.opacity,
        value.softness_pixels,
        crate::PropertyAppearanceSample {
            opacity: value.missing[0],
            softness_pixels: value.missing[1],
        },
    )
}

fn parse_surface_scalar(
    scene: &Scene,
    value: &SurfaceScalarDescription,
) -> Result<SurfaceScalarOverlay, crate::CoreError> {
    let raw = resolve_raw(value.field);
    resolve_existing(scene.volumes.get(raw))?;
    let ramp = ScalarRamp::new(
        value.ramp_values.map(f32::from_bits),
        value.ramp_colors.map(rgba),
    )?;
    let mut overlay = SurfaceScalarOverlay::new(crate::VolumeHandle(raw), ramp);
    overlay.contours = value
        .contours
        .map(|values| ScalarContours::new(values[0], values[1]))
        .transpose()?;
    if !value.sample_offset_angstrom.is_finite() {
        return invalid("surface scalar offset is not finite");
    }
    overlay.sample_offset_angstrom = value.sample_offset_angstrom;
    Ok(overlay)
}

fn validate_regions(
    scene: &Scene,
    target: RepresentationTarget,
    value: &Representation,
) -> Result<(), crate::CoreError> {
    if let Some(region) = value.volume.region {
        let RepresentationTarget::Volume(handle) = target else {
            return invalid("volume region is attached to a non-volume target");
        };
        let volume = scene.volume(handle).ok_or(CoreError::StaleHandle)?;
        validate_region(region, volume.dimensions())?;
    }
    if let Some(region) = value.segmentation.region {
        let RepresentationTarget::SegmentedVolume(handle) = target else {
            return invalid("segmentation region is attached to a non-segmentation target");
        };
        let volume = scene
            .segmented_volume(handle)
            .ok_or(crate::CoreError::StaleHandle)?;
        validate_region(region, volume.dimensions())?;
    }
    Ok(())
}

fn validate_region(region: VolumeRegion, dimensions: [u32; 3]) -> Result<(), crate::CoreError> {
    if (0..3).any(|axis| region.maximum()[axis] > dimensions[axis]) {
        invalid("volume region exceeds its source dimensions")
    } else {
        Ok(())
    }
}
