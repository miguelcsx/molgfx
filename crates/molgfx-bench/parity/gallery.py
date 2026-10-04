"""Authoritative table of gallery cases; writes gallery.json beside this file.

Cases use a molgfx command script or a native scene-domain builder, with
reference-engine commands or segment surfaces on the same fixture and camera.
Review items name the checklist ids the reviewer scores.
"""

import json
from pathlib import Path

HERE = Path(__file__).parent
STILL = ["GEN-1", "GEN-2", "GEN-3"]
ELEMENT = ["EL-1", "EL-2"]


def rep(type_, color="chain-id", size="uniform", **type_params):
    params = {"type": type_, "color": color, "size": size}
    if type_params:
        params["typeParams"] = type_params
    return {"kind": "representation", "params": params}


def case(
    id_, fixture, molgfx, pymol, molstar, checklist, fit="all", margin=1.15, video=None, **extra
):
    return {
        "id": id_,
        "fixture": fixture,
        "molgfx": molgfx,
        "pymol": pymol,
        "molstar": molstar,
        "checklist": checklist,
        "fit": fit,
        "margin": margin,
        "video": video,
        **extra,
    }


PY_CARTOON = ["show cartoon, fixture", "util.cbc fixture"]
PY_SS = ["show cartoon, fixture", "color red, ss h", "color yellow, ss s", "color green, ss l+''"]
PY_STICKS = ["show sticks, fixture", "util.cbag fixture"]
PY_BAS = [
    "show sticks, fixture",
    "show spheres, fixture",
    "set sphere_scale, 0.25",
    "util.cbag fixture",
]
PY_SURF = lambda kind: [
    "set surface_solvent, on" if kind == 6 else f"set surface_type, {kind}",
    "show surface, fixture",
    "util.cbc fixture",
]


def color_case(scheme, pymol_color, mol_color):
    return case(
        f"B-color-{scheme}-4HHB",
        "4HHB",
        ["show cartoon as main, protein", f"color {scheme}, @main"],
        ["show cartoon, fixture", *pymol_color],
        [rep("cartoon", mol_color)],
        [
            *STILL,
            "COL-1"
            if scheme
            in (
                "chain",
                "entity",
                "molecule_type",
                "residue_name",
                "carbon_by_chain",
                "element",
                "secondary_structure",
            )
            else "COL-2",
        ],
    )


CASES = [
    case(
        "B-cartoon-4HHB",
        "4HHB",
        ["show cartoon as main, protein", "color chain, @main"],
        PY_CARTOON,
        [rep("cartoon")],
        [*STILL, "CT-1", "CT-2", "CT-5", "SS-1"],
    ),
    case(
        "B-cartoon-ss-1AON",
        "1AON",
        ["show cartoon as main, protein", "color secondary_structure, @main"],
        PY_SS,
        [rep("cartoon", "secondary-structure")],
        [*STILL, "CT-6", "SS-2", "SS-3"],
    ),
    case(
        "P1-ss-palette-1AON",
        "1AON",
        ["show cartoon as main, protein", "color secondary_structure, @main"],
        PY_SS,
        [rep("cartoon", "secondary-structure")],
        [*STILL, "SS-1", "SS-2", "SS-3", "SS-4"],
    ),
    case(
        "P1-ss-palette-7QPD",
        "7QPD",
        ["show cartoon as main, protein", "color secondary_structure, @main"],
        PY_SS,
        [rep("cartoon", "secondary-structure")],
        [*STILL, "SS-1", "SS-2", "SS-3", "SS-4"],
    ),
    case(
        "B-dna-1BNA",
        "1BNA",
        ["show nucleic_acid as main, nucleic", "color chain, @main"],
        ["show cartoon, fixture", "set cartoon_ring_mode, 0", "util.cbc fixture"],
        [rep("cartoon", visuals=["polymer-trace"])],
        [*STILL, "NA-1", "NA-2", "NA-6"],
    ),
    case(
        "B-trna-1EHZ",
        "1EHZ",
        ["show nucleic_acid as main, nucleic", "color chain, @main"],
        PY_CARTOON,
        [rep("cartoon")],
        [*STILL, "NA-5"],
    ),
    case(
        "B-bases-1BNA",
        "1BNA",
        ["show nucleic_acid as main, nucleic", "show bases, nucleic", "color chain, @main"],
        [
            "show cartoon, fixture",
            "set cartoon_ring_mode, 3",
            "set cartoon_ring_finder, 1",
            "util.cbc fixture",
        ],
        [rep("cartoon")],
        [*STILL, "NA-2", "NA-3"],
    ),
    case(
        "B-basepairs-1BNA",
        "1BNA",
        ["show nucleic_acid as main, nucleic", "show base_pairs, nucleic"],
        ["show cartoon, fixture", "set cartoon_ladder_mode, 1", "util.cbc fixture"],
        [rep("cartoon")],
        [*STILL, "NA-4"],
    ),
    case(
        "B-bas-ligand-orders",
        "bond_order_ligand",
        ["show ball_and_stick, all", "color element, all"],
        [
            "show sticks, fixture",
            "show spheres, fixture",
            "set sphere_scale, 0.25",
            "set valence, 1",
            "util.cbag fixture",
        ],
        [rep("ball-and-stick", "element-symbol", sizeFactor=0.2, multipleBonds="symmetric")],
        [*STILL, *ELEMENT, "BS-1", "BS-2"],
        fit="all",
        margin=1.1,
    ),
    case(
        "B-licorice-1HVR",
        "1HVR",
        ["show licorice, hetero", "show cartoon, protein", "color element, hetero"],
        ["show cartoon, polymer", "show sticks, hetatm", "util.cbag hetatm"],
        [rep("cartoon"), rep("ball-and-stick", "element-symbol", sizeFactor=0.2)],
        [*STILL, *ELEMENT, "BS-4", "BS-5"],
        fit="hetero",
    ),
    case(
        "B-lines-1HVR",
        "1HVR",
        ["show lines, hetero", "show cartoon, protein", "color element, hetero"],
        ["show cartoon, polymer", "show lines, hetatm", "util.cbag hetatm"],
        [rep("cartoon"), rep("line", "element-symbol")],
        [*STILL, *ELEMENT, "BS-7"],
        fit="hetero",
    ),
    case(
        "B-spacefill-4HHB",
        "4HHB",
        ["show spacefill, protein", "color element, protein"],
        ["show spheres, polymer", "util.cbag polymer"],
        [rep("spacefill", "element-symbol")],
        [*STILL, *ELEMENT],
    ),
    case(
        "B-points-4HHB",
        "4HHB",
        ["show points, protein", "color element, protein"],
        ["set sphere_scale, 0.12", "show spheres, fixture", "util.cbag fixture"],
        [rep("point", "element-symbol")],
        [*STILL, *ELEMENT],
    ),
    case(
        "B-dots-4HHB",
        "4HHB",
        ["show dots, protein", "color element, protein"],
        ["show dots, polymer", "util.cbag polymer"],
        [rep("point", "element-symbol")],
        [*STILL, "DOT-1"],
    ),
    *[
        case(
            f"B-surface-{name}-4HHB",
            "4HHB",
            [
                "show cartoon, protein",
                f"show surface {options} as shell, protein",
                "opacity 0.6, @shell",
                "color chain, @shell",
            ],
            [*PY_SURF(pymol_kind), "set transparency, 0.4"],
            [rep(mol_type, "chain-id", alpha=0.6, **mol_params)],
            [
                *STILL,
                "SURF-2" if name in ("vdw", "sas") else "SURF-1" if name == "ses" else "SURF-3",
            ],
        )
        for name, options, pymol_kind, mol_type, mol_params in [
            ("vdw", "kind=van_der_waals", 5, "molecular-surface", {"probeRadius": 0}),
            (
                "sas",
                "kind=solvent_accessible",
                6,
                "molecular-surface",
                {"probeRadius": 1.4, "ignoreHydrogens": True},
            ),
            ("ses", "kind=solvent_excluded", 0, "molecular-surface", {"probeRadius": 1.4}),
            ("gaussian", "kind=gaussian", 0, "gaussian-surface", {}),
            ("blob", "kind=gaussian style=soft_union", 0, "gaussian-surface", {"smoothness": 1.5}),
        ]
    ],
    case(
        "B-putty-bfactor-4HHB",
        "4HHB",
        ["show putty as main, protein", "color b_factor, @main"],
        ["show cartoon, fixture", "cartoon putty", "spectrum b, blue_white_red, fixture"],
        [rep("putty", "uncertainty", "uncertainty")],
        [*STILL, "COL-2", "SIZE-1"],
    ),
    case(
        "B-tube-1AON",
        "1AON",
        ["show tube as main, protein", "color chain, @main"],
        ["show cartoon, fixture", "cartoon tube", "util.cbc fixture"],
        [rep("cartoon", aspectRatio=1, tubularHelices=True, visuals=["polymer-trace"])],
        [*STILL, "CT-1"],
    ),
    case(
        "B-trace-1AON",
        "1AON",
        ["show trace as main, protein", "color chain, @main"],
        ["show ribbon, fixture", "util.cbc fixture"],
        [rep("cartoon", sizeFactor=0.05, aspectRatio=1, visuals=["polymer-trace"])],
        [*STILL, "CT-1"],
    ),
    case(
        "B-backbone-1AON",
        "1AON",
        ["show backbone as main, protein", "color chain, @main"],
        ["show sticks, name ca+n+c", "util.cbc fixture"],
        [rep("backbone")],
        [*STILL, "CT-1"],
    ),
    case(
        "B-beads-1AON",
        "1AON",
        ["show beads as main, protein", "color chain, @main"],
        ["show spheres, name ca", "util.cbc fixture"],
        [rep("spacefill")],
        [*STILL, "EL-2"],
    ),
    case(
        "B-glycan-1HZH",
        "1HZH",
        ["show cartoon, protein", "show glycan, saccharide"],
        ["show cartoon, polymer", "show sticks, not polymer and not solvent", "util.cbc fixture"],
        [rep("cartoon"), rep("carbohydrate", "carbohydrate-symbol")],
        [*STILL, "GLY-4"],
    ),
    *[
        color_case(*args)
        for args in [
            ("element", ["util.cbag fixture"], "element-symbol"),
            ("chain", ["util.cbc fixture"], "chain-id"),
            ("entity", ["util.cbc fixture"], "entity-id"),
            ("molecule_type", ["util.cbc fixture"], "molecule-type"),
            ("residue_name", ["util.cbc fixture"], "residue-name"),
            (
                "secondary_structure",
                ["color red, ss h", "color yellow, ss s", "color green, ss l+''"],
                "secondary-structure",
            ),
            ("carbon_by_chain", ["util.cbc fixture"], "chain-id"),
            ("b_factor", ["spectrum b, blue_white_red, fixture"], "uncertainty"),
            ("hydrophobicity", ["spectrum resi, red_yellow_green, fixture"], "hydrophobicity"),
            ("rainbow", ["spectrum count, rainbow, fixture"], "sequence-id"),
            ("plddt", ["spectrum b, blue_white_red, fixture"], "uncertainty"),
            ("sasa", ["spectrum b, blue_white_red, fixture"], "uncertainty"),
        ]
    ],
    case(
        "B-pocket-HEM-4HHB",
        "4HHB",
        ["pocket, resname HEM"],
        [
            "show cartoon, fixture",
            "show sticks, resn HEM",
            "show lines, byres (resn HEM around 6)",
            "util.cbc fixture",
        ],
        [rep("cartoon"), rep("ball-and-stick", "element-symbol")],
        [*STILL, "BS-3", "SURF-5"],
        fit="resname HEM",
        margin=2.5,
    ),
    case(
        "B-unitcell-1HVR",
        "1HVR",
        [
            "show cartoon, protein",
            'assembly {"structures":[1],"instances":[],"unit_cell":{"lengths":[58.4,86.1,46.2],"angles_degrees":[90,90,90],"origin":[0,0,0]}}',
        ],
        ["show cartoon, fixture", "util.cbc fixture"],
        [rep("cartoon")],
        [*STILL, "ASM-2"],
    ),
    case(
        "B-plane",
        "4HHB",
        [
            "show cartoon, protein",
            'plane {"structure":1,"center":[0,0,0],"normal":[0,0,1],"tangent":[1,0,0],"size":[40,40],"color":[230,50,50,255]}',
        ],
        ["show cartoon, fixture", "util.cbc fixture"],
        [rep("cartoon")],
        [*STILL],
    ),
    case(
        "B-label-4HHB",
        "4HHB",
        ["show cartoon, protein", 'label "Heme", resname HEM'],
        ["show cartoon, fixture", "label resn HEM and name FE, resn"],
        [
            rep("cartoon"),
            {
                "kind": "label",
                "atoms": [{"auth_asym_id": "A", "auth_seq_id": 142, "auth_atom_id": "FE"}],
            },
        ],
        [*STILL, "LBL-1", "LBL-5"],
    ),
    case(
        "B-distance-angle-dihedral-4HHB",
        "4HHB",
        ["show cartoon, protein", "distance, resname HEM, resid 87 and name NE2"],
        [
            "show cartoon, fixture",
            "distance d1, resn HEM and name FE and chain A, resi 87 and name NE2 and chain A",
        ],
        [
            rep("cartoon"),
            {
                "kind": "measurement",
                "measure": "distance",
                "atoms": [
                    {"auth_asym_id": "A", "auth_seq_id": 142, "auth_atom_id": "FE"},
                    {"auth_asym_id": "A", "auth_seq_id": 87, "auth_atom_id": "NE2"},
                ],
            },
        ],
        [*STILL, "MEAS-1"],
        fit="resname HEM",
        margin=3.0,
    ),
    case(
        "B-orbit-video-1AON",
        "1AON",
        ["show cartoon as main, protein", "color chain, @main"],
        PY_CARTOON,
        [rep("cartoon")],
        ["VID-1", "CT-6"],
        video={"kind": "orbit_y", "frames": 120, "fps": 30},
    ),
    case(
        "B-trajectory-video-1D3Z",
        "1D3Z",
        ["show cartoon as main, protein", "color chain, @main"],
        PY_CARTOON,
        [rep("cartoon")],
        ["VID-2"],
        video={"kind": "trajectory", "frames": 10, "fps": 5},
    ),
]


def segmentation_case(base):
    """Procedural native categorical domain, with reference-engine segment surfaces."""
    entry = {k: v for k, v in base.items() if k not in ("id", "form", "script")}
    entry.update(
        id="P2-segmentation-two-maps",
        form="script",
        camera={
            "position": [0, 0, 80],
            "target": [0, 0, 0],
            "up": [0, 1, 0],
            "fov_y_degrees": 45,
            "near": 1,
            "far": 2000,
        },
        script={
            "molgfx": [],
            "pymol": [],
            "molstar": [],
            "checklist": ["SEG-1", "SEG-2"],
            "fit": None,
            "video": None,
        },
    )
    entry.update(
        segmentation_thresholds=[0.05, 0.2, 0.5],
        segmentations=[
            {
                "name": "left",
                "translation": [-10, 0, 0],
                "styles": [
                    {"label": 1, "color_rgb": [220, 50, 40], "opacity": 0.8, "visible": True},
                    {"label": 2, "color_rgb": [240, 170, 40], "opacity": 0.3, "visible": True},
                ],
            },
            {
                "name": "right",
                "translation": [10, 0, 0],
                "styles": [
                    {"label": 1, "color_rgb": [40, 100, 230], "opacity": 0.8, "visible": True},
                    {"label": 2, "color_rgb": [40, 200, 170], "opacity": 0.3, "visible": False},
                ],
            },
        ],
    )
    return entry


VOLUME_CASES = [
    ("P2-iso-solid-skewMRC", "skewMRC", "isosurface", 0.35, None, ["VOL-2"]),
    ("P2-iso-mesh-model_density", "model_density", "iso_mesh", 0.5, None, ["VOL-3"]),
    ("P2-iso-dots-model_density", "model_density", "iso_dots", 0.5, None, ["VOL-3"]),
    ("P2-direct-model_density", "model_density", "direct", 0.5, None, ["VOL-4"]),
    ("P2-slice-model_density", "model_density", "slice", 0.5, None, ["VOL-5"]),
    ("P2-region-model_density", "model_density", "region", 0.5, None, ["VOL-6"]),
    (
        "P2-iso-over-cartoon-4HHB",
        "model_density",
        "isosurface",
        0.5,
        "4HHB.cif",
        ["VOL-1", "VOL-2"],
    ),
]


def volume_cases(base):
    for identity, fixture, form, level, structure, checklist in VOLUME_CASES:
        entry = {k: v for k, v in base[fixture].items() if k not in ("id", "form", "script")}
        entry.update(
            id=identity,
            form=form,
            checklist=checklist,
            isovalue=level,
            camera={
                "position": [0, 0, 24 if fixture == "skewMRC" else 160],
                "target": [0, 0, 0],
                "up": [0, 1, 0],
                "fov_y_degrees": 45,
                "near": 1,
                "far": 2000,
            },
        )
        if form == "iso_mesh":
            entry["line_width_voxels"] = 0.1
        elif form == "iso_dots":
            entry["dot_radius_voxels"] = 0.16
        if structure is not None:
            entry["structure_file"] = structure
            entry["opacity"] = 0.45
        if form == "direct":
            entry["omissions"] = {
                "pymol-ray": "PyMOL 3.1 experimental ray_volume does not produce an image for this map; use pymol-raster for volume comparison."
            }
        elif form == "region":
            entry["omissions"] = {
                "pymol-ray": "PyMOL has no exact voxel-region crop for this case.",
                "pymol-raster": "PyMOL has no exact voxel-region crop for this case.",
                "molstar-imagepass": "Mol* adapter has no exact voxel-region crop for this case.",
            }
        elif form == "slice":
            entry.update(
                slice_point=[10.770298331975937, 13.442623138427734, 40.59999930858612],
                slice_normal=[0, 0, 1],
                slice_domain=[0, 1],
                slice_colors=[
                    0x440154,
                    0x482878,
                    0x3E4989,
                    0x31688E,
                    0x26828E,
                    0x1F9E89,
                    0x35B779,
                    0x6ECE58,
                    0xB5DE2B,
                    0xFDE725,
                ],
            )
        elif form == "iso_dots":
            entry["omissions"] = {
                "molstar-imagepass": f"Mol* adapter has no exact {form} presentation for this case."
            }
        yield entry


def effect_cases(base):
    reference = next(c for c in CASES if c["id"] == "B-cartoon-4HHB")
    for effect, checks in (
        ("bloom", ["FX-3"]),
        ("bloom_control", ["FX-3"]),
        ("dof", ["FX-2"]),
        ("motion_blur", ["FX-4"]),
        ("motion_blur_control", ["FX-4"]),
        ("depth_cue", ["FX-1"]),
        ("shape_cues", ["GEN-1"]),
        ("dof_control", ["FX-2"]),
        ("depth_cue_control", ["FX-1"]),
        ("shape_cues_control", ["GEN-1"]),
    ):
        entry = {k: v for k, v in base["1AON"].items() if k not in ("id", "form", "script")}
        entry.update(
            id=f"P2-effects-1AON-{effect}",
            effect=effect,
            form="script",
            script={
                "molgfx": reference["molgfx"],
                "pymol": reference["pymol"],
                "molstar": reference["molstar"],
                "checklist": checks,
                "fit": {"selection": "all", "fov_y_degrees": 45, "margin": 1.15},
            },
        )
        if effect in ("motion_blur", "motion_blur_control"):
            entry["script"]["video"] = {"kind": "orbit_y", "frames": 24, "fps": 24}
        if effect in (
            "motion_blur_control",
            "dof_control",
            "depth_cue_control",
            "shape_cues_control",
        ):
            del entry["effect"]
        if effect not in ("dof_control", "depth_cue_control", "shape_cues_control"):
            entry["omissions"] = {}
        if effect in ("bloom", "dof", "motion_blur", "shape_cues"):
            reason = f"PyMOL 3.1 has no equivalent {effect} presentation effect; effect parity is omitted."
            entry["omissions"].update({"pymol-ray": reason, "pymol-raster": reason})
        if effect in ("motion_blur", "shape_cues"):
            entry["omissions"]["molstar-imagepass"] = (
                f"Mol* ImagePass has no equivalent {effect} presentation effect; effect parity is omitted."
            )
        yield entry


def build():
    corpus = json.loads((HERE / "corpus.json").read_text())
    base = {f["id"]: f for f in corpus["fixtures"]}
    out = {
        key: corpus[key]
        for key in ("extent", "physical", "style", "recipe_policy", "recipe_settings")
    }
    out["extent"] = [768, 768]
    out["warmup_outputs"], out["measured_outputs"] = 0, 1
    out["recipes"] = [
        "molgfx-converged",
        "molgfx-interactive",
        "pymol-ray",
        "pymol-raster",
        "molstar-imagepass",
    ]
    out["recipe_settings"] = {
        k: v for k, v in corpus["recipe_settings"].items() if k in out["recipes"]
    }
    fixtures = []
    for c in CASES:
        b = base[c["fixture"]]
        script = {
            "molgfx": c["molgfx"],
            "pymol": c["pymol"],
            "molstar": c["molstar"],
            "checklist": c["checklist"],
            "fit": {"selection": c["fit"], "fov_y_degrees": 45, "margin": c["margin"]},
        }
        if c["video"]:
            script["video"] = c["video"]
        entry = {k: b[k] for k in b if k not in ("id", "form", "script")}
        entry.update({"id": c["id"], "form": "script", "script": script})
        fixtures.append(entry)
    fixtures.extend(volume_cases(base))
    fixtures.extend(effect_cases(base))
    fixtures.append(segmentation_case(base["skewMRC"]))
    out["fixtures"] = fixtures
    return out


if __name__ == "__main__":
    (HERE / "gallery.json").write_text(json.dumps(build(), indent=2) + "\n")
    print(len(CASES), "cases")
