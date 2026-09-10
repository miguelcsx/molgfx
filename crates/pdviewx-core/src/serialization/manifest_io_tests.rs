use super::*;
use crate::Scene;
use std::io::{Cursor, Read};

fn payload_reference(bytes: &[u8]) -> PayloadReference {
    PayloadReference {
        kind: ReferencedPayloadKind::Structure,
        dataset: 9,
        chunk: u64::from(u32::MAX) + 7,
        byte_len: bytes.len() as u64,
        address: ContentAddress::digest(bytes),
    }
}

#[test]
fn manifest_streams_without_intermediate_json_strings() {
    let payload = b"immutable payload";
    let manifest = Scene::new().manifest(vec![payload_reference(payload)]);
    let mut encoded = Vec::new();
    assert!(write_manifest(&mut encoded, &manifest).is_ok());
    let decoded = read_manifest(encoded.as_slice());
    assert_eq!(decoded.ok(), Some(manifest));
}

#[test]
fn scene_builds_a_canonical_manifest_from_metadata_only() {
    let first = payload_reference(b"first");
    let mut second = payload_reference(b"second");
    second.dataset = 3;
    let manifest = Scene::new().manifest(vec![first, second]);
    assert_eq!(manifest.scene, Scene::new().describe());
    assert_eq!(manifest.payloads, vec![second, first]);
}

#[test]
fn every_noncurrent_schema_is_rejected_by_the_stream_reader() {
    for schema in [3, 4, 5, SCHEMA_VERSION + 1] {
        let mut manifest = Scene::new().manifest(Vec::new());
        manifest.scene.schema = schema;
        manifest.scene.engine = format!("pdviewx-scene-{schema}");
        let encoded = match serde_json::to_vec(&manifest) {
            Ok(value) => value,
            Err(error) => panic!("test manifest encodes: {error}"),
        };
        assert!(matches!(
            read_manifest(encoded.as_slice()),
            Err(ManifestError::UnsupportedSchema { found, expected })
                if found == schema && expected == SCHEMA_VERSION
        ));
    }
}

#[test]
fn current_schema_with_a_foreign_engine_identifier_is_rejected() {
    let mut manifest = Scene::new().manifest(Vec::new());
    manifest.scene.engine = "foreign-scene-format".to_owned();
    let encoded = match serde_json::to_vec(&manifest) {
        Ok(value) => value,
        Err(error) => panic!("test manifest encodes: {error}"),
    };
    assert!(matches!(
        read_manifest(encoded.as_slice()),
        Err(ManifestError::EngineMismatch)
    ));
}

#[test]
fn current_schema_requires_every_current_scene_field() {
    let manifest = Scene::new().manifest(Vec::new());
    let mut value = match serde_json::to_value(manifest) {
        Ok(value) => value,
        Err(error) => panic!("test manifest encodes: {error}"),
    };
    let Some(scene) = value
        .get_mut("scene")
        .and_then(serde_json::Value::as_object_mut)
    else {
        panic!("manifest scene is an object")
    };
    scene.remove("overlays");
    let encoded = match serde_json::to_vec(&value) {
        Ok(value) => value,
        Err(error) => panic!("test manifest encodes: {error}"),
    };
    assert!(matches!(
        read_manifest(encoded.as_slice()),
        Err(ManifestError::Json(_))
    ));
}

#[test]
fn scene_rehydration_rejects_a_previous_schema_even_without_stream_io() {
    let mut description = Scene::new().describe();
    description.schema = SCHEMA_VERSION - 1;
    description.engine = format!("pdviewx-scene-{}", description.schema);
    let result = Scene::from_description(
        &description,
        crate::SceneDescriptionSources {
            structures: &[],
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    );
    assert!(result.is_err());
}

#[derive(Debug)]
struct Resolver(Vec<u8>);

impl PayloadResolver for Resolver {
    type Reader = Cursor<Vec<u8>>;

    fn open(&self, _reference: &PayloadReference) -> Result<Option<Self::Reader>, ManifestError> {
        Ok(Some(Cursor::new(self.0.clone())))
    }
}

#[test]
fn lazy_payloads_verify_digest_length_and_large_chunk_identity() {
    let bytes = b"chunk bytes";
    let reference = payload_reference(bytes);
    let manifest = SceneManifest::new(Scene::new().describe(), vec![reference]);
    let lazy = LazyManifest::new(manifest, Resolver(bytes.to_vec()));
    let Ok(lazy) = lazy else {
        panic!("valid manifest must attach to resolver")
    };
    let Ok(mut reader) = lazy.open(&reference) else {
        panic!("resolver must expose payload")
    };
    let mut output = Vec::new();
    assert!(reader.read_to_end(&mut output).is_ok());
    assert_eq!(output, bytes);
    assert_eq!(reference.chunk, u64::from(u32::MAX) + 7);
}

#[test]
fn lazy_payloads_reject_corruption_at_end_of_stream() {
    let expected = payload_reference(b"expected");
    let manifest = SceneManifest::new(Scene::new().describe(), vec![expected]);
    let Ok(lazy) = LazyManifest::new(manifest, Resolver(b"corrupt".to_vec())) else {
        panic!("valid manifest must attach")
    };
    let Ok(mut reader) = lazy.open(&expected) else {
        panic!("resolver opens")
    };
    let mut output = Vec::new();
    assert!(reader.read_to_end(&mut output).is_err());
}
