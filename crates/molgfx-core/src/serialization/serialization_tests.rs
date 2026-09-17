use super::*;
use crate::{
    AnisotropicEllipsoid, Annotation, AnnotationAnchor, AtomSelection, Guide, GuideStyle,
    InteractionAnchor, InteractionEdge, InteractionGeometry, InteractionKind, Measurement,
    Particle, ParticleBoundary, ParticleMotion, ParticleShape, PlanarRegion, Primitive,
    RepresentationKind, SceneDescriptionSources, read_manifest, write_manifest,
};
use molgfx_math::{Aabb, Quat, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn current_schema_visual_programs_rehydrate_with_live_parameters() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("owner exists")
    };
    let property = crate::AtomProperty::new(
        owner,
        "signal",
        Arc::from(vec![0.0; structure.atom_count() as usize]),
        crate::AtomPropertyMeaning::Generic,
        crate::ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("property validates: {error}"));
    let property_handle = scene
        .add_atom_property(property.clone())
        .unwrap_or_else(|error| panic!("property attaches: {error}"));
    let mut builder = crate::VisualProgramBuilder::new();
    let signal = builder
        .atom_property(property_handle)
        .unwrap_or_else(|error| panic!("property input builds: {error}"));
    let (parameter, gain) = builder
        .scalar_parameter(1.0)
        .unwrap_or_else(|error| panic!("parameter builds: {error}"));
    let opacity = builder
        .multiply(signal, gain)
        .unwrap_or_else(|error| panic!("expression builds: {error}"));
    builder
        .set_opacity(opacity)
        .unwrap_or_else(|error| panic!("output builds: {error}"));
    let mut style = crate::VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("program builds: {error}")),
    );
    style
        .set_scalar(parameter, 0.25)
        .unwrap_or_else(|error| panic!("parameter updates: {error}"));
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, crate::Representation::spacefill().visual(style))
        .unwrap_or_else(|error| panic!("visual representation builds: {error}"));
    let description = scene.describe();
    assert_eq!(description.schema, SCHEMA_VERSION);
    let rebuilt = Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: std::slice::from_ref(&property),
            meshes: &[],
        },
    )
    .unwrap_or_else(|error| panic!("visual scene rehydrates: {error}"));
    assert_eq!(rebuilt.describe(), description);
}

#[test]
fn scene_manifest_round_trips_and_validates_source_fingerprints() {
    let structure = crate::fixture::structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let representation = match scene.represent(selection, RepresentationKind::Spacefill) {
        Ok(handle) => handle,
        Err(error) => panic!("representation builds: {error}"),
    };
    if let Some(value) = scene.representation_mut(representation) {
        value.order = 3;
        value.material.opacity = 0.7;
    }
    let manifest = scene.manifest(Vec::new());
    let mut encoded = Vec::new();
    if let Err(error) = write_manifest(&mut encoded, &manifest) {
        panic!("manifest encodes: {error}");
    }
    let decoded = match read_manifest(encoded.as_slice()) {
        Ok(manifest) => manifest,
        Err(error) => panic!("manifest decodes: {error}"),
    };
    assert_eq!(decoded, manifest);
    assert!(scene.validate_description(&decoded.scene).is_ok());
}

#[test]
fn scene_manifest_rehydrates_against_a_cold_source() {
    let structure = crate::fixture::structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    if scene
        .represent(selection, RepresentationKind::Spacefill)
        .is_err()
    {
        panic!("fixture representation builds");
    }
    let description = scene.describe();
    let sources = SceneDescriptionSources {
        structures: std::slice::from_ref(&structure),
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    };
    let rebuilt = match Scene::from_description(&description, sources) {
        Ok(scene) => scene,
        Err(error) => panic!("cold-source scene rehydrates: {error}"),
    };
    assert_eq!(rebuilt.describe(), description);
}

#[test]
fn occupancy_manifest_rehydrates_without_a_host_volume_payload() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture scene has one structure")
    };
    let stream = crate::OccupancyStream::new(
        [20, 18, 16],
        Vec3::splat(-4.0),
        Vec3::splat(0.5),
        0.97,
        1.0,
        50.0,
    )
    .unwrap_or_else(|error| panic!("occupancy stream validates: {error}"));
    let volume = scene
        .add_occupancy_stream(owner, &AtomSelection::Sparse(vec![0, 2]), stream)
        .unwrap_or_else(|error| panic!("occupancy binds: {error}"));
    scene
        .represent(volume, crate::Representation::volume())
        .unwrap_or_else(|error| panic!("occupancy represents: {error}"));
    let description = scene.describe();
    assert!(description.volumes[0].occupancy.is_some());
    let rebuilt = Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    )
    .unwrap_or_else(|error| panic!("occupancy manifest rehydrates: {error}"));
    assert_eq!(rebuilt.describe(), description);
}

#[test]
fn deletion_heavy_scene_manifests_round_trip_sparse_rows() {
    let mut scene = Scene::new();
    let anchor = match crate::OverlayAnchor::new([0.5, 0.5], [0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("overlay anchor validates: {error}"),
    };
    let mut handles = Vec::new();
    for row in 0..100 {
        let overlay = match crate::ScreenOverlay::new(
            crate::OverlayContent::Text {
                text: format!("row-{row}"),
                color: Rgba8::WHITE,
                size_pixels: 12.0,
            },
            anchor,
        ) {
            Ok(value) => value,
            Err(error) => panic!("overlay validates: {error}"),
        };
        handles.push(scene.add_overlay(overlay));
    }
    for handle in handles.iter().take(99).copied() {
        assert!(scene.remove_overlay(handle).is_some());
    }
    let description = scene.describe();
    assert_eq!(description.overlays.len(), 1);
    assert_eq!(description.overlays[0].row, 99);

    let restored = match Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: &[],
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    ) {
        Ok(value) => value,
        Err(error) => panic!("sparse manifest rehydrates: {error}"),
    };
    assert_eq!(restored.describe(), description);
}

#[test]
fn surface_styles_survive_cold_source_rehydration() {
    let structure = crate::fixture::structure();
    for style in [crate::SurfaceStyle::Mesh, crate::SurfaceStyle::SoftUnion] {
        let mut scene = match Scene::from_structure(&structure) {
            Ok(scene) => scene,
            Err(error) => panic!("fixture scene builds: {error}"),
        };
        let selection = scene.add_selection(AtomSelection::All);
        let surface = match scene.represent(selection, RepresentationKind::Surface) {
            Ok(handle) => handle,
            Err(error) => panic!("surface builds: {error}"),
        };
        let Some(representation) = scene.representation_mut(surface) else {
            panic!("surface resolves")
        };
        representation.params.surface_style = style;
        let description = scene.describe();
        let rebuilt = match Scene::from_description(
            &description,
            SceneDescriptionSources {
                structures: std::slice::from_ref(&structure),
                volumes: &[],
                segmentations: &[],
                atom_properties: &[],
                meshes: &[],
            },
        ) {
            Ok(scene) => scene,
            Err(error) => panic!("surface rehydrates: {error}"),
        };
        let Some((_, representation)) = rebuilt.representations().next() else {
            panic!("rehydrated surface exists")
        };
        assert_eq!(representation.params.surface_style, style);
    }
}

#[test]
fn manifest_round_trips_primitive_and_annotation_payloads() {
    let (mut scene, structure) = manifest_fixture_scene();
    let Some((owner, _)) = scene.structures().next() else {
        panic!("manifest fixture has a structure")
    };
    add_primitive_payloads(&mut scene, owner);
    add_annotation_payloads(&mut scene, owner);
    let manifest = scene.manifest(Vec::new());
    let description = &manifest.scene;
    assert_eq!(description.primitives.len(), 4);
    assert_eq!(description.guides.len(), 1);
    assert_eq!(description.interactions.len(), 1);
    assert_eq!(description.annotations.len(), 1);
    assert_eq!(description.measurements.len(), 1);
    let mut encoded = Vec::new();
    if let Err(error) = write_manifest(&mut encoded, &manifest) {
        panic!("manifest payload encodes: {error}");
    }
    let decoded = match read_manifest(encoded.as_slice()) {
        Ok(manifest) => manifest,
        Err(error) => panic!("manifest payload decodes: {error}"),
    };
    assert_eq!(decoded, manifest);
    assert!(scene.validate_description(&decoded.scene).is_ok());
    let rebuilt = match Scene::from_description(
        description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    ) {
        Ok(scene) => scene,
        Err(error) => panic!("full manifest rehydrates: {error}"),
    };
    assert_eq!(rebuilt.describe(), *description);
}

fn manifest_fixture_scene() -> (Scene, pdbiox::Structure) {
    let structure = crate::fixture::structure();
    match Scene::from_structure(&structure) {
        Ok(scene) => (scene, structure),
        Err(error) => panic!("manifest fixture scene builds: {error}"),
    }
}

fn add_primitive_payloads(scene: &mut Scene, owner: crate::StructureHandle) {
    let ellipsoid = match AnisotropicEllipsoid::new(Vec3::ZERO, [4.0, 1.0, 1.0, 0.0, 0.0, 0.0]) {
        Ok(value) => value,
        Err(error) => panic!("manifest ellipsoid validates: {error}"),
    };
    let ellipsoid = match Primitive::ellipsoid(owner, ellipsoid, Rgba8::opaque(120, 150, 220), 0.8)
    {
        Ok(value) => value,
        Err(error) => panic!("manifest ellipsoid declares: {error}"),
    };
    let plane = match PlanarRegion::new(
        owner,
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::Z,
        Vec3::X,
        [2.0, 1.0],
    ) {
        Ok(value) => value,
        Err(error) => panic!("manifest plane validates: {error}"),
    };
    let plane = match Primitive::planar(plane, Rgba8::WHITE, 0.5) {
        Ok(value) => value,
        Err(error) => panic!("manifest plane declares: {error}"),
    };
    let symbol = match crate::CarbohydrateSymbol::new(
        owner,
        Vec3::new(1.0, 0.0, 0.0),
        Quat::IDENTITY,
        Vec3::splat(1.0),
        crate::CarbohydrateShape::Glc,
        Rgba8::opaque(30, 180, 80),
    ) {
        Ok(value) => value,
        Err(error) => panic!("manifest symbol validates: {error}"),
    };
    let symbol = Primitive::carbohydrate(symbol);
    let gaussian_motion = match ParticleMotion::new(
        Vec3::new(0.5, 0.0, 0.0),
        Aabb::new(Vec3::splat(-4.0), Vec3::splat(4.0)),
        0.016,
        19,
        ParticleBoundary::Wrap,
    ) {
        Ok(value) => value,
        Err(error) => panic!("manifest Gaussian motion validates: {error}"),
    }
    .with_respawn_after_steps(64);
    let gaussian = match Particle::new(
        owner,
        Vec3::new(-1.0, 0.0, 0.0),
        Quat::from_rotation_y(0.35),
        Vec3::new(3.0, 2.0, 1.5),
        ParticleShape::Gaussian,
        Rgba8::opaque(220, 120, 40),
        0.65,
    ) {
        Ok(value) => value.with_motion(gaussian_motion),
        Err(error) => panic!("manifest Gaussian validates: {error}"),
    };
    if let Err(error) =
        scene.add_primitives(&[ellipsoid, plane, symbol, Primitive::particle(gaussian)])
    {
        panic!("manifest primitive batch stores: {error}");
    }
    if let Err(error) = scene.add_guide(
        match Guide::new(owner, Vec3::ZERO, Vec3::X, GuideStyle::default()) {
            Ok(value) => value,
            Err(error) => panic!("manifest guide validates: {error}"),
        },
    ) {
        panic!("manifest guide stores: {error}");
    }
}

fn add_annotation_payloads(scene: &mut Scene, owner: crate::StructureHandle) {
    let start = match InteractionAnchor::world(Vec3::ZERO) {
        Ok(value) => value,
        Err(error) => panic!("manifest interaction anchor: {error}"),
    };
    let end = match InteractionAnchor::world(Vec3::Y) {
        Ok(value) => value,
        Err(error) => panic!("manifest interaction anchor: {error}"),
    };
    let geometry = match InteractionGeometry::new(1.0, Some(120.0)) {
        Ok(value) => value,
        Err(error) => panic!("manifest interaction geometry: {error}"),
    };
    let interaction = match InteractionEdge::new(
        owner,
        start,
        end,
        InteractionKind::HydrogenBond,
        geometry,
        "fixture",
    ) {
        Ok(value) => value,
        Err(error) => panic!("manifest interaction validates: {error}"),
    }
    .with_persistence(3, 2.0)
    .unwrap_or_else(|error| panic!("manifest interaction persistence validates: {error}"));
    if let Err(error) = scene.add_interaction(interaction) {
        panic!("manifest interaction stores: {error}");
    }
    let anchor = match AnnotationAnchor::world(Vec3::Z) {
        Ok(value) => value,
        Err(error) => panic!("manifest annotation anchor: {error}"),
    };
    let annotation = match Annotation::note(owner, anchor, "note") {
        Ok(value) => value,
        Err(error) => panic!("manifest annotation validates: {error}"),
    };
    if let Err(error) = scene.add_annotation(annotation) {
        panic!("manifest annotation stores: {error}");
    }
    let second = match AnnotationAnchor::world(Vec3::ONE) {
        Ok(value) => value,
        Err(error) => panic!("manifest measurement anchor: {error}"),
    };
    let measurement = match Measurement::distance(owner, [anchor, second], 1.0, "fixture") {
        Ok(value) => value,
        Err(error) => panic!("manifest measurement validates: {error}"),
    };
    if let Err(error) = scene.add_measurement(measurement) {
        panic!("manifest measurement stores: {error}");
    }
}
