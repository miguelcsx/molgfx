use crate::{
    AppearanceRuleId, AppearanceRuleSpec, Color, ColorSpec, PatchOperation, Scene, ScenePatch,
    SceneSpec, StructureId, color, rep,
};

/// Two protein chains of two residues each, and one ligand in chain C.
const CIF: &str = "\
data_two
_entry.id two
loop_
_entity.id
_entity.type
1 polymer
2 non-polymer
loop_
_entity_poly.entity_id
_entity_poly.type
1 'polypeptide(L)'
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.auth_seq_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_atom_id
ATOM 1 N N GLY A 1 1 0.0 0.0 0.0 1 GLY A N
ATOM 2 C CA GLY A 1 1 1.4 0.0 0.0 1 GLY A CA
ATOM 3 N N GLY A 1 2 3.0 0.5 0.0 2 GLY A N
ATOM 4 C CA GLY A 1 2 4.4 0.5 0.0 2 GLY A CA
ATOM 5 N N GLY B 1 1 0.0 5.0 0.0 1 GLY B N
ATOM 6 C CA GLY B 1 1 1.4 5.0 0.0 1 GLY B CA
ATOM 7 N N GLY B 1 2 3.0 5.5 0.0 2 GLY B N
ATOM 8 C CA GLY B 1 2 4.4 5.5 0.0 2 GLY B CA
HETATM 9 FE FE HEM C 2 . 2.0 2.5 0.0 101 HEM C FE
";

pub(crate) fn two_chains() -> molframe::Structure {
    let Ok((structure, _)) = molframe::read_bytes(
        CIF.as_bytes().to_vec(),
        Some("two.cif"),
        &molframe::ReadOptions::new(),
    ) else {
        panic!("fixture parses")
    };
    structure
}

fn scene() -> Scene {
    match Scene::from_structure(&two_chains()) {
        Ok(scene) => scene,
        Err(error) => panic!("scene builds: {error}"),
    }
}

fn apply(scene: &mut Scene, operations: Vec<PatchOperation>) {
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations,
    };
    if let Err(error) = scene.apply(&patch) {
        panic!("patch applies: {error}")
    }
}

fn rule(target: &str, color: ColorSpec) -> AppearanceRuleSpec {
    AppearanceRuleSpec::new(StructureId(1), target, color)
}

const RED: Color = Color::rgb(255, 0, 0);
const GREEN: Color = Color::rgb(0, 255, 0);

/// The class of every atom, read from the physical overlay column.
fn classes(scene: &Scene, id: crate::RepresentationId) -> Vec<f32> {
    let Some(handle) = scene.representation_handle(id) else {
        panic!("representation is resolved")
    };
    let Some(representation) = scene.resolved().representation(handle) else {
        panic!("physical representation exists")
    };
    let Some(overlay) = representation.color_overlay else {
        return vec![0.0; 9];
    };
    let Some(column) = scene.resolved().atom_property(overlay.classes()) else {
        panic!("class column exists")
    };
    column.values().to_vec()
}

fn cartoon(scene: &mut Scene) -> crate::RepresentationId {
    match scene.add(rep::cartoon("protein").color(color::chain())) {
        Ok(id) => id,
        Err(error) => panic!("cartoon adds: {error}"),
    }
}

#[test]
fn a_rule_over_chain_a_colours_only_chain_a_of_a_whole_protein_cartoon() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    apply(
        &mut scene,
        vec![PatchOperation::AddAppearanceRule {
            id: AppearanceRuleId(1),
            rule: rule("chain A", color::uniform(RED)),
        }],
    );
    assert_eq!(
        classes(&scene, cartoon),
        vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    );
    // The cartoon keeps its own chain colouring everywhere else.
    let Some(spec) = scene.spec().representations.get(&cartoon) else {
        panic!("representation exists")
    };
    assert_eq!(spec.color(), &ColorSpec::Chain);
}

#[test]
fn overlapping_rules_resolve_in_favour_of_the_higher_identity() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    apply(
        &mut scene,
        vec![
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(1),
                rule: rule("protein", color::uniform(RED)),
            },
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(2),
                rule: rule("chain B", color::uniform(GREEN)),
            },
        ],
    );
    assert_eq!(
        classes(&scene, cartoon),
        vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 0.0]
    );
}

#[test]
fn removing_a_rule_restores_what_lay_beneath_it() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    apply(
        &mut scene,
        vec![
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(1),
                rule: rule("protein", color::uniform(RED)),
            },
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(2),
                rule: rule("chain B", color::uniform(GREEN)),
            },
        ],
    );
    apply(
        &mut scene,
        vec![PatchOperation::RemoveAppearanceRule {
            id: AppearanceRuleId(2),
        }],
    );
    assert_eq!(
        classes(&scene, cartoon),
        vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0]
    );
    apply(
        &mut scene,
        vec![PatchOperation::RemoveAppearanceRule {
            id: AppearanceRuleId(1),
        }],
    );
    assert_eq!(classes(&scene, cartoon), vec![0.0; 9]);
}

#[test]
fn rules_sharing_a_colour_share_one_class_and_schemes_are_rules_too() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    apply(
        &mut scene,
        vec![
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(1),
                rule: rule("chain A", color::element()),
            },
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(2),
                rule: rule("resname HEM", color::uniform(RED)),
            },
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(3),
                rule: rule("chain B", color::element()),
            },
        ],
    );
    assert_eq!(
        classes(&scene, cartoon),
        vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0]
    );
}

#[test]
fn every_representation_of_a_structure_reads_the_same_class_column() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    let Ok(sticks) = scene.add(rep::ball_and_stick("resname HEM")) else {
        panic!("sticks add")
    };
    apply(
        &mut scene,
        vec![PatchOperation::AddAppearanceRule {
            id: AppearanceRuleId(1),
            rule: rule("resname HEM", color::uniform(RED)),
        }],
    );
    let overlay = |id| {
        let Some(handle) = scene.representation_handle(id) else {
            panic!("representation is resolved")
        };
        scene
            .resolved()
            .representation(handle)
            .and_then(|representation| representation.color_overlay)
    };
    let (Some(first), Some(second)) = (overlay(cartoon), overlay(sticks)) else {
        panic!("both representations carry the overlay")
    };
    assert_eq!(first.classes(), second.classes());
}

#[test]
fn an_appearance_edit_neither_re_resolves_nor_replaces_any_representation() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    let before = scene.representation_handle(cartoon);
    apply(
        &mut scene,
        vec![PatchOperation::AddAppearanceRule {
            id: AppearanceRuleId(1),
            rule: rule("chain A", color::uniform(RED)),
        }],
    );
    assert_eq!(scene.representation_handle(cartoon), before);
}

#[test]
fn appearance_operations_invert_exactly() {
    let mut scene = scene();
    let _ = cartoon(&mut scene);
    let base = scene.to_spec();
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(1),
                rule: rule("chain A", color::uniform(RED)),
            },
            PatchOperation::ReplaceAppearanceRule {
                id: AppearanceRuleId(1),
                rule: rule("chain B", color::uniform(GREEN)),
            },
        ],
    };
    let Ok(inverse) = patch.inverse(&base) else {
        panic!("patch inverts")
    };
    apply(&mut scene, patch.operations.clone());
    if let Err(error) = scene.apply(&inverse) {
        panic!("inverse applies: {error}")
    }
    assert_eq!(scene.spec().appearance, base.appearance);
}

#[test]
fn rules_survive_a_json_round_trip_and_a_fresh_resolution() {
    let mut scene = scene();
    let cartoon = cartoon(&mut scene);
    apply(
        &mut scene,
        vec![PatchOperation::AddAppearanceRule {
            id: AppearanceRuleId(4),
            rule: rule("chain B", color::uniform(GREEN)),
        }],
    );
    let Ok(json) = scene.spec().to_json() else {
        panic!("spec serializes")
    };
    let Ok(spec) = SceneSpec::from_json(&json) else {
        panic!("spec parses")
    };
    assert_eq!(&spec, scene.spec());
    let mut structures = std::collections::BTreeMap::new();
    let _ = structures.insert(StructureId(1), two_chains());
    let Ok(restored) = Scene::from_spec(spec, structures) else {
        panic!("spec resolves")
    };
    assert_eq!(classes(&restored, cartoon), classes(&scene, cartoon));
}

#[test]
fn a_property_colour_cannot_be_a_rule() {
    let mut scene = scene();
    let Ok(property) =
        serde_json::from_str::<crate::ScalarProperty>(r#"{"structure":1,"name":"b"}"#)
    else {
        panic!("property reference parses")
    };
    let result = scene.apply(&ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::AddAppearanceRule {
            id: AppearanceRuleId(1),
            rule: rule(
                "all",
                color::property(property, "viridis", [0.0, 1.0], None, RED),
            ),
        }],
    });
    assert!(result.is_err());
    assert!(scene.spec().appearance.is_empty());
}

#[test]
fn too_many_distinct_colours_fail_and_leave_the_scene_unchanged() {
    let mut scene = scene();
    let _ = cartoon(&mut scene);
    let before = scene.to_spec();
    let operations = (0..=crate::MAX_APPEARANCE_CLASSES)
        .map(|index| {
            let shade = u8::try_from(index).unwrap_or(0);
            PatchOperation::AddAppearanceRule {
                id: AppearanceRuleId(index as u64 + 1),
                rule: rule("all", color::uniform(Color::rgb(shade, 0, 0))),
            }
        })
        .collect();
    let result = scene.apply(&ScenePatch {
        base_revision: scene.revision(),
        operations,
    });
    assert!(result.is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn a_rule_on_an_unknown_structure_is_rejected() {
    let mut scene = scene();
    let result = scene.apply(&ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::AddAppearanceRule {
            id: AppearanceRuleId(1),
            rule: AppearanceRuleSpec::new(StructureId(9), "all", color::element()),
        }],
    });
    assert!(result.is_err());
}
