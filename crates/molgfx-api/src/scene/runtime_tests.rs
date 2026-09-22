use super::structure_hash;

fn parse(source: &str) -> molframe::Structure {
    let result = molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("identity.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn structure_identity_includes_topology_not_only_coordinates() {
    let alanine = parse(
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n",
    );
    let glycine = parse(
        "ATOM      1  N   GLY A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n",
    );
    assert_ne!(
        structure_hash(&molgfx_core::MolecularSource::from_molframe(&alanine)),
        structure_hash(&molgfx_core::MolecularSource::from_molframe(&glycine))
    );
}

/// The digest of one fixed structure, pinned as a contract.
///
/// `content_hash` is published in the specification, exported through
/// `MolViewSpec` and *verified* when a specification is reimported, so the value
/// is a contract rather than an implementation detail. A retuned hashing loop
/// that changed it would invalidate every saved scene, which is what this test
/// refuses to let happen silently.
#[test]
fn the_structure_digest_is_a_pinned_value() {
    let structure = parse(
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\n\
         ATOM      2  CA  ALA A   1      12.104   6.134  -6.504  1.00  0.00           C\n\
         ATOM      3  C   ALA A   1      13.104   7.134  -6.504  1.00  0.00           C\n\
         HETATM    4  O   HOH B   2      14.104   7.134  -6.504  1.00  0.00           O\n\
         END\n",
    );
    let hash = structure_hash(&molgfx_core::MolecularSource::from_molframe(&structure));
    assert_eq!(
        hash.as_ref(),
        "30f78498b6486f27db6c750a5dd857abcb393a0dec3902672161cca160c281b4",
        "the structure digest is a published contract"
    );
}

/// A local edit must not copy the patch that carried it.
///
/// The plan used to own a second copy of the operation list purely to answer
/// whether the patch was empty, which cost one allocation and one deep clone of
/// the most expensive variant per edit. The plan records the emptiness instead,
/// and this holds the shape that keeps it that way: the patch a caller holds is
/// still the patch after it applies.
#[test]
fn applying_a_local_edit_does_not_copy_its_operation_list() {
    use crate::{rep, sel};
    let mut scene =
        super::Scene::from_structure(&fixture()).unwrap_or_else(|error| panic!("{error}"));
    let id = scene
        .add(rep::spacefill(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));

    let patch = crate::ScenePatch {
        base_revision: scene.revision(),
        operations: vec![crate::PatchOperation::SetOpacity { id, opacity: 0.5 }],
    };
    let operations = patch.operations.as_ptr();
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        patch.operations.as_ptr(),
        operations,
        "the caller's operation list is neither moved nor reallocated"
    );
    assert_eq!(patch.operations.len(), 1);
}

fn fixture() -> molframe::Structure {
    parse("ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n")
}
