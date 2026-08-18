use super::*;
use crate::{
    AnisotropicEllipsoid, Annotation, AnnotationAnchor, AtomSelection, Guide, GuideStyle,
    InteractionAnchor, InteractionEdge, InteractionGeometry, InteractionKind, Measurement,
    Particle, ParticleBoundary, ParticleMotion, ParticleShape, PlanarRegion, RepresentationKind,
    SceneDescriptionSources,
};
use pdviewx_math::{Aabb, Quat, Rgba8, Vec3};

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
    let description = scene.describe();
    let json = match description.to_json() {
        Ok(json) => json,
        Err(error) => panic!("manifest encodes: {error}"),
    };
    let decoded = match SceneDescription::from_json(&json) {
        Ok(description) => description,
        Err(error) => panic!("manifest decodes: {error}"),
    };
    assert_eq!(decoded, description);
    assert!(scene.validate_description(&decoded).is_ok());
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
fn manifest_schema_versions_are_rejected_before_scene_use() {
    let mut description = Scene::new().describe();
    description.schema = SCHEMA_VERSION + 1;
    let json = match serde_json::to_string(&description) {
        Ok(json) => json,
        Err(error) => panic!("test manifest encodes: {error}"),
    };
    assert!(SceneDescription::from_json(&json).is_err());
}

#[test]
fn schema_three_reads_with_empty_version_four_tables() {
    let mut value = match serde_json::to_value(Scene::new().describe()) {
        Ok(value) => value,
        Err(error) => panic!("manifest value encodes: {error}"),
    };
    let Some(object) = value.as_object_mut() else {
        panic!("manifest is an object")
    };
    object.insert("schema".to_owned(), serde_json::Value::from(3));
    object.insert(
        "engine".to_owned(),
        serde_json::Value::from("pdviewx-scene-3"),
    );
    object.remove("meshes");
    object.remove("mesh_instances");
    object.remove("overlays");
    if let Some(primitives) = object.remove("primitives") {
        object.insert("scientific_primitives".to_owned(), primitives);
    }
    let source = match serde_json::to_string(&value) {
        Ok(value) => value,
        Err(error) => panic!("schema three encodes: {error}"),
    };
    let decoded = match SceneDescription::from_json(&source) {
        Ok(value) => value,
        Err(error) => panic!("schema three reads: {error}"),
    };
    assert_eq!(decoded.schema, 3);
    assert!(decoded.meshes.is_empty());
    assert!(decoded.mesh_instances.is_empty());
    assert!(decoded.overlays.is_empty());
}

#[test]
fn manifest_round_trips_primitive_and_annotation_payloads() {
    let (mut scene, structure) = manifest_fixture_scene();
    let Some((owner, _)) = scene.structures().next() else {
        panic!("manifest fixture has a structure")
    };
    add_primitive_payloads(&mut scene, owner);
    add_annotation_payloads(&mut scene, owner);
    let description = scene.describe();
    assert_eq!(description.primitives.len(), 4);
    assert_eq!(description.guides.len(), 1);
    assert_eq!(description.interactions.len(), 1);
    assert_eq!(description.annotations.len(), 1);
    assert_eq!(description.measurements.len(), 1);
    let json = match description.to_json() {
        Ok(json) => json,
        Err(error) => panic!("manifest payload encodes: {error}"),
    };
    let decoded = match SceneDescription::from_json(&json) {
        Ok(description) => description,
        Err(error) => panic!("manifest payload decodes: {error}"),
    };
    assert_eq!(decoded, description);
    assert!(scene.validate_description(&decoded).is_ok());
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
        Err(error) => panic!("full manifest rehydrates: {error}"),
    };
    assert_eq!(rebuilt.describe(), description);
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
    if let Err(error) = scene.add_ellipsoid(owner, ellipsoid, Rgba8::opaque(120, 150, 220), 0.8) {
        panic!("manifest ellipsoid stores: {error}");
    }
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
    if let Err(error) = scene.add_filled_planar_region(plane, Rgba8::WHITE, 0.5) {
        panic!("manifest plane stores: {error}");
    }
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
    if let Err(error) = scene.add_carbohydrate_symbol(symbol) {
        panic!("manifest symbol stores: {error}");
    }
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
    if let Err(error) = scene.add_particle(gaussian) {
        panic!("manifest Gaussian stores: {error}");
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
