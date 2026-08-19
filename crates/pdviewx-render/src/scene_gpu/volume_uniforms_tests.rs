use super::*;
use pdviewx_core::{
    DensityVolume, Representation, RepresentationKind, Scene, VolumeRendering, VolumeStyle,
};
use std::sync::Arc;

#[test]
fn isosurface_mode_and_level_are_packed_independently_of_the_grid() {
    let volume = match DensityVolume::new(
        [2, 2, 2],
        Mat4::IDENTITY,
        Arc::from([0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0]),
    ) {
        Ok(volume) => volume,
        Err(error) => panic!("volume builds: {error}"),
    };
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume.clone());
    let representation = match scene.represent(
        handle,
        Representation::volume().volume_style(VolumeStyle::isosurface()),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("isosurface applies: {error}"),
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("representation resolves")
    };
    assert_eq!(representation.kind, RepresentationKind::Volume);
    assert_eq!(representation.volume.rendering, VolumeRendering::Isosurface);
    let uniforms = VolumeUniforms::new(&volume, representation);
    assert_eq!(
        uniforms.transfer_meta[1],
        VolumeRendering::Isosurface as u32
    );
    assert!((uniforms.scalar[2] - 1.0).abs() < f32::EPSILON);
}

#[test]
fn participating_medium_is_an_explicit_volume_algorithm() {
    let volume = match DensityVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0.5; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("medium volume builds: {error}"),
    };
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume.clone());
    let representation = match scene.represent(
        handle,
        Representation::volume().volume_style(VolumeStyle::medium()),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("medium applies: {error}"),
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("medium representation resolves")
    };
    assert_eq!(representation.volume.rendering, VolumeRendering::Medium);
    let uniforms = VolumeUniforms::new(&volume, representation);
    assert_eq!(uniforms.transfer_meta[1], VolumeRendering::Medium as u32);
}

#[test]
fn liquid_surface_mode_keeps_the_caller_grid_and_selects_the_surface_branch() {
    let volume = match DensityVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0.5; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("liquid volume builds: {error}"),
    };
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume.clone());
    let representation = match scene.represent(
        handle,
        Representation::volume().volume_style(VolumeStyle::liquid_surface()),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("liquid surface applies: {error}"),
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("liquid representation resolves")
    };
    let uniforms = VolumeUniforms::new(&volume, representation);
    assert_eq!(
        uniforms.transfer_meta[1],
        VolumeRendering::LiquidSurface as u32
    );
    assert_eq!(volume.dimensions(), [2, 2, 2]);
}

#[test]
fn arbitrary_slice_plane_is_packed_in_world_space() {
    let volume = match DensityVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0.5; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("slice volume builds: {error}"),
    };
    let plane = match pdviewx_core::ClipPlane::from_point_normal(
        pdviewx_math::Vec3::new(1.0, 2.0, 3.0),
        pdviewx_math::Vec3::new(0.0, 2.0, 0.0),
    ) {
        Ok(plane) => plane,
        Err(error) => panic!("slice plane builds: {error}"),
    };
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume.clone());
    let representation = match scene.represent(
        handle,
        Representation::volume()
            .volume_style(VolumeStyle::slice(pdviewx_core::VolumeSlice::new(plane))),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("slice applies: {error}"),
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("slice representation resolves")
    };
    let uniforms = VolumeUniforms::new(&volume, representation);
    assert_eq!(uniforms.transfer_meta[1], VolumeRendering::Slice as u32);
    assert_eq!(
        uniforms.slice_plane.map(f32::to_bits),
        [0.0, 1.0, 0.0, -2.0].map(f32::to_bits)
    );
}

#[test]
fn cropped_volume_bounds_are_packed_as_a_half_open_region() {
    let volume = match DensityVolume::new([8, 9, 10], Mat4::IDENTITY, Arc::from([0.5; 720])) {
        Ok(volume) => volume,
        Err(error) => panic!("crop volume builds: {error}"),
    };
    let region = match pdviewx_core::VolumeRegion::new([1, 2, 3], [7, 8, 9], [8, 9, 10]) {
        Ok(region) => region,
        Err(error) => panic!("crop region builds: {error}"),
    };
    let mut scene = Scene::new();
    let handle = scene.add_volume(volume.clone());
    let representation = match scene.represent(
        handle,
        Representation::volume().volume_style(VolumeStyle::default().region(region)),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("cropped volume applies: {error}"),
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("crop representation resolves")
    };
    let uniforms = VolumeUniforms::new(&volume, representation);
    assert_eq!(uniforms.crop_minimum, [1, 2, 3, 0]);
    assert_eq!(uniforms.crop_maximum, [7, 8, 9, 0]);
}
