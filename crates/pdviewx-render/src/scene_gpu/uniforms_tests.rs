use super::*;

#[test]
fn surface_grid_uses_angstrom_spacing_until_the_dimension_cap() {
    assert_eq!(axis_cells(10.0, 0.25), 41);
    assert_eq!(axis_cells(100.0, 0.25), SURFACE_GRID_MAX_DIMENSION);
}

#[test]
fn realtime_surface_grid_halves_each_axis_without_changing_quality_spacing() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    let bounds = Aabb::from_points([Vec3::ZERO, Vec3::splat(10.0)]);
    let realtime = RepresentationUniforms::for_quality(&representation, bounds, None, false);
    let quality = RepresentationUniforms::for_quality(&representation, bounds, None, true);

    assert!((realtime.grid_cell[0] - 0.5).abs() < f32::EPSILON);
    assert!((quality.grid_cell[0] - 0.25).abs() < f32::EPSILON);
    assert_eq!(realtime.grid_size[0], 27);
    assert_eq!(quality.grid_size[0], 53);
}

#[test]
fn surface_grid_always_has_an_interpolatable_cell() {
    assert_eq!(axis_cells(0.0, 0.25), 2);
    assert_eq!(axis_cells(0.1, 0.25), 2);
}

#[test]
fn grid_traversal_budget_is_bounded_by_the_longest_axis() {
    assert_eq!(march_steps([2, 2, 2]), 2);
    assert_eq!(march_steps([192, 121, 87]), 192);
}

#[test]
fn surface_presentation_is_packed_without_changing_the_field() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene, SurfaceStyle};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    representation.params.surface_style = SurfaceStyle::Dots;
    representation.params.surface_pattern_spacing = 2.25;
    representation.params.surface_pattern_width_pixels = 1.75;
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(uniforms.options[3], SurfaceStyle::Dots as u32);
    assert_eq!(uniforms.visual[1..3], [2.25, 1.75]);
}

#[test]
fn line_width_is_enabled_only_for_the_line_representation() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut lines = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Lines,
    );
    lines.params.line_width_pixels = 2.25;
    let bounds = Aabb::from_points([Vec3::ZERO, Vec3::ONE]);
    let line_uniforms = RepresentationUniforms::new(&lines, bounds, None);
    let capsules = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::BallAndStick,
    );
    let capsule_uniforms = RepresentationUniforms::new(&capsules, bounds, None);
    assert_eq!(line_uniforms.visual[3].to_bits(), 2.25f32.to_bits());
    assert_eq!(capsule_uniforms.visual[3].to_bits(), 0.0f32.to_bits());
}

#[test]
fn material_response_is_packed_once_per_representation() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::BallAndStick,
    );
    representation.material.roughness = 0.73;
    representation.material.specular = 0.19;
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(
        uniforms.material.map(f32::to_bits),
        [0.73, 0.19, 0.0, 0.0].map(f32::to_bits)
    );
}

#[test]
fn principled_material_tag_and_metallicity_share_the_material_uniform() {
    use pdviewx_core::{AtomSelection, Material, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    representation.material = Material::principled(0.82);
    representation.material.roughness = 0.31;
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(
        uniforms.material.map(f32::to_bits),
        [0.31, Material::default().specular_strength(), 1.0, 0.82].map(f32::to_bits)
    );
}

#[test]
fn anisotropic_ribbon_tag_and_strength_share_the_material_uniform() {
    use pdviewx_core::{AtomSelection, Material, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Cartoon,
    );
    representation.material = Material::anisotropic_ribbon(0.68);
    representation.material.roughness = 0.36;
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(
        uniforms.material.map(f32::to_bits),
        [0.36, Material::default().specular_strength(), 2.0, 0.68].map(f32::to_bits)
    );
}

#[test]
fn putty_mapping_lowers_to_raw_reversible_shader_parameters() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Tube,
    );
    representation.params.tube_radius = 0.3;
    representation.params.tube_radius_mapping =
        pdviewx_core::TubeRadiusMapping::b_factor([10.0, 50.0], [0.2, 0.8])
            .unwrap_or_else(|error| panic!("mapping validates: {error}"));

    let uniforms = ClipUniforms::new(&representation);
    assert_eq!(uniforms.meta[3], 1);
    assert_eq!(
        uniforms.tube_mapping.map(f32::to_bits),
        [10.0, 50.0, 0.2, 0.8].map(f32::to_bits)
    );
    assert_eq!(uniforms.tube[0].to_bits(), 0.3f32.to_bits());
}

#[test]
fn diffusion_material_tag_and_strength_share_the_material_uniform() {
    use pdviewx_core::{AtomSelection, Material, Representation, RepresentationKind, Scene};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    representation.material = Material::diffusion(0.64);
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(
        uniforms.material.map(f32::to_bits),
        [0.34, 0.5, 3.0, 0.64].map(f32::to_bits)
    );
}

#[test]
fn van_der_waals_surface_never_inflates_atomic_radii() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene, SurfaceKind};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    representation.params.surface_kind = SurfaceKind::VanDerWaals;
    representation.params.probe_radius = 7.0;
    representation.params.radius_scale = 1.75;
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(uniforms.surface[0].to_bits(), 0.0f32.to_bits());
    assert_eq!(uniforms.visual[3].to_bits(), 1.75f32.to_bits());
    assert_eq!(uniforms.options[0], SurfaceKind::VanDerWaals as u32);
}

#[test]
fn gaussian_surface_uniforms_encode_bounded_support_and_sigma() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene, SurfaceKind};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    representation.params.surface_kind = SurfaceKind::Gaussian;
    representation.params.gaussian_sigma = 0.75;
    representation.params.isolevel = 0.35;
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        None,
    );
    assert_eq!(uniforms.surface[0].to_bits(), (4.0 * 0.75f32).to_bits());
    assert_eq!(uniforms.surface[1].to_bits(), 0.35f32.to_bits());
    assert_eq!(uniforms.surface[2].to_bits(), 0.75f32.to_bits());
    assert_eq!(uniforms.visual[3].to_bits(), 0.0f32.to_bits());
    assert_eq!(uniforms.options[0], SurfaceKind::Gaussian as u32);
}

#[test]
fn gaussian_surface_uniforms_keep_the_level_inside_compact_support() {
    use pdviewx_core::{AtomSelection, Representation, RepresentationKind, Scene, SurfaceKind};

    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        pdviewx_core::RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    representation.params.surface_kind = SurfaceKind::Gaussian;
    let bounds = Aabb::from_points([Vec3::ZERO, Vec3::ONE]);
    let defaults = RepresentationUniforms::new(&representation, bounds, None);
    assert_eq!(defaults.surface[1].to_bits(), 0.5f32.to_bits());

    representation.params.isolevel = f32::MIN_POSITIVE;
    let bounded = RepresentationUniforms::new(&representation, bounds, None);
    assert_eq!(bounded.surface[1].to_bits(), 0.001f32.to_bits());
}

#[test]
fn scalar_overlay_uniforms_preserve_domain_dimensions_and_contour_units() {
    use pdviewx_core::{
        AtomSelection, Representation, RepresentationKind, RepresentationTarget, ScalarContours,
        ScalarRamp, ScalarVolume, Scene, SurfaceScalarOverlay,
    };
    use std::sync::Arc;

    let mut scene = Scene::new();
    let volume = match ScalarVolume::from_spacing(
        [3, 4, 5],
        Vec3::new(-2.0, -3.0, -4.0),
        Vec3::splat(0.5),
        Arc::from(vec![0.0; 60]),
    ) {
        Ok(volume) => volume,
        Err(error) => panic!("volume validates: {error}"),
    };
    let volume = scene.add_volume(volume);
    let selection = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Surface,
    );
    let contours = match ScalarContours::new(0.5, 1.25) {
        Ok(contours) => contours,
        Err(error) => panic!("contours validate: {error}"),
    };
    representation.surface_scalar = Some(SurfaceScalarOverlay {
        contours: Some(contours),
        ..SurfaceScalarOverlay::new(volume, ScalarRamp::diverging(2.0))
    });
    let uniforms = RepresentationUniforms::new(
        &representation,
        Aabb::from_points([Vec3::ZERO, Vec3::ONE]),
        scene.volume(volume),
    );
    assert_eq!(uniforms.overlay_size, [3, 4, 5, 1]);
    for (actual, expected) in uniforms
        .overlay_domain
        .into_iter()
        .zip([-2.0, 0.0, 2.0, 0.5])
    {
        assert!((actual - expected).abs() < f32::EPSILON);
    }
    assert!((uniforms.overlay_visual[0] - 1.25).abs() < f32::EPSILON);
}

#[test]
fn projection_parameters_precompute_exact_sphere_frustum_factors() {
    let perspective = Projection::Perspective {
        fov_y: 1.0,
        aspect: 1.5,
        near: 0.1,
        far: 100.0,
    };
    let perspective_matrix = perspective.matrix();
    let packed = projection_parameters(perspective, perspective_matrix);
    assert_eq!(packed[0].to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        packed[1].to_bits(),
        perspective_matrix.x_axis.x.abs().hypot(1.0).to_bits()
    );

    let orthographic = Projection::Orthographic {
        height: 20.0,
        aspect: 1.5,
        near: 0.1,
        far: 100.0,
    };
    let orthographic_matrix = orthographic.matrix();
    let packed = projection_parameters(orthographic, orthographic_matrix);
    assert_eq!(packed[0].to_bits(), 1.0_f32.to_bits());
    assert_eq!(
        packed[1].to_bits(),
        orthographic_matrix.x_axis.x.abs().to_bits()
    );
}
