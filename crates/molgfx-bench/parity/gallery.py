"""Authoritative table of gallery cases; writes gallery.json beside this file.

Each case runs one molgfx command script, the equivalent PyMOL commands and
the equivalent Mol* representation entries on the same fixture and fitted
camera. Review items name the checklist ids the reviewer scores.
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


def case(id_, fixture, molgfx, pymol, molstar, checklist, fit="all", margin=1.15, video=None, **extra):
    return {"id": id_, "fixture": fixture, "molgfx": molgfx, "pymol": pymol, "molstar": molstar,
            "checklist": checklist, "fit": fit, "margin": margin, "video": video, **extra}


PY_CARTOON = ["show cartoon, fixture", "util.cbc fixture"]
PY_SS = ["show cartoon, fixture", "color red, ss h", "color yellow, ss s", "color green, ss l+''"]
PY_STICKS = ["show sticks, fixture", "util.cbag fixture"]
PY_BAS = ["show sticks, fixture", "show spheres, fixture", "set sphere_scale, 0.25", "util.cbag fixture"]
PY_SURF = lambda kind: [f"set surface_type, {kind}", "show surface, fixture", "util.cbc fixture"]


def color_case(scheme, pymol_color, mol_color):
    return case(f"B-color-{scheme}-4HHB", "4HHB",
                ["show cartoon as main, protein", f"color {scheme}, @main"],
                ["show cartoon, fixture", *pymol_color],
                [rep("cartoon", mol_color)],
                [*STILL, "COL-1" if scheme in ("chain", "entity", "molecule_type", "residue_name",
                                              "carbon_by_chain", "element", "secondary_structure") else "COL-2"])


CASES = [
    case("B-cartoon-4HHB", "4HHB", ["show cartoon as main, protein", "color chain, @main"],
         PY_CARTOON, [rep("cartoon")], [*STILL, "CT-1", "CT-2", "CT-5", "SS-1"]),
    case("B-cartoon-ss-1AON", "1AON", ["show cartoon as main, protein", "color secondary_structure, @main"],
         PY_SS, [rep("cartoon", "secondary-structure")], [*STILL, "CT-6", "SS-2", "SS-3"]),
    case("B-dna-1BNA", "1BNA", ["show nucleic_acid as main, nucleic", "color chain, @main"],
         ["show cartoon, fixture", "set cartoon_ring_mode, 0", "util.cbc fixture"],
         [rep("cartoon", visuals=["polymer-trace"])], [*STILL, "NA-1", "NA-2", "NA-6"]),
    case("B-trna-1EHZ", "1EHZ", ["show nucleic_acid as main, nucleic", "color chain, @main"],
         PY_CARTOON, [rep("cartoon")], [*STILL, "NA-5"]),
    case("B-bases-1BNA", "1BNA", ["show nucleic_acid as main, nucleic", "show bases, nucleic",
                                  "color chain, @main"],
         ["show cartoon, fixture", "set cartoon_ring_mode, 3", "set cartoon_ring_finder, 1", "util.cbc fixture"],
         [rep("cartoon")],
         [*STILL, "NA-2", "NA-3"]),
    case("B-basepairs-1BNA", "1BNA", ["show nucleic_acid as main, nucleic", "show base_pairs, nucleic"],
         ["show cartoon, fixture", "set cartoon_ladder_mode, 1", "util.cbc fixture"],
         [rep("cartoon")], [*STILL, "NA-4"]),
    case("B-bas-ligand-orders", "bond_order_ligand", ["show ball_and_stick, all", "color element, all"],
         ["show sticks, fixture", "show spheres, fixture", "set sphere_scale, 0.25", "set valence, 1",
          "util.cbag fixture"],
         [rep("ball-and-stick", "element-symbol", sizeFactor=0.2, multipleBonds="symmetric")],
         [*STILL, *ELEMENT, "BS-1", "BS-2"], fit="all", margin=1.1),
    case("B-licorice-1HVR", "1HVR", ["show licorice, hetero", "show cartoon, protein", "color element, hetero"],
         ["show cartoon, polymer", "show sticks, hetatm", "util.cbag hetatm"],
         [rep("cartoon"), rep("ball-and-stick", "element-symbol", sizeFactor=0.2)],
         [*STILL, *ELEMENT, "BS-4", "BS-5"], fit="hetero"),
    case("B-lines-1HVR", "1HVR", ["show lines, hetero", "show cartoon, protein", "color element, hetero"],
         ["show cartoon, polymer", "show lines, hetatm", "util.cbag hetatm"],
         [rep("cartoon"), rep("line", "element-symbol")], [*STILL, *ELEMENT, "BS-7"], fit="hetero"),
    case("B-spacefill-4HHB", "4HHB", ["show spacefill, protein", "color element, protein"],
         ["show spheres, polymer", "util.cbag polymer"], [rep("spacefill", "element-symbol")],
         [*STILL, *ELEMENT]),
    case("B-points-4HHB", "4HHB", ["show points, protein", "color element, protein"],
         ["show nonbonded, polymer", "util.cbag polymer"], [rep("point", "element-symbol")],
         [*STILL, *ELEMENT]),
    case("B-dots-4HHB", "4HHB", ["show dots, protein", "color element, protein"],
         ["show dots, polymer", "util.cbag polymer"], [rep("point", "element-symbol")],
         [*STILL, "DOT-1"]),
    *[case(f"B-surface-{name}-4HHB", "4HHB",
           ["show cartoon, protein", f"show surface {options} as shell, protein", "opacity 0.6, @shell",
            "color chain, @shell"],
           [*PY_SURF(pymol_kind), "set transparency, 0.4"],
           [rep(mol_type, "chain-id", alpha=0.6, **mol_params)],
           [*STILL, "SURF-2" if name in ("vdw", "sas") else "SURF-1" if name == "ses" else "SURF-3"])
      for name, options, pymol_kind, mol_type, mol_params in [
          ("vdw", "kind=van_der_waals", 5, "molecular-surface", {"probeRadius": 0}),
          ("sas", "kind=solvent_accessible", 6, "molecular-surface", {"probeRadius": 1.4, "ignoreHydrogens": True}),
          ("ses", "kind=solvent_excluded", 0, "molecular-surface", {"probeRadius": 1.4}),
          ("gaussian", "kind=gaussian", 0, "gaussian-surface", {}),
          ("blob", "kind=gaussian style=soft_union", 0, "gaussian-surface", {"smoothness": 1.5}),
      ]],
    case("B-putty-bfactor-4HHB", "4HHB", ["show putty as main, protein", "color b_factor, @main"],
         ["show cartoon, fixture", "cartoon putty", "spectrum b, blue_white_red, fixture"],
         [rep("putty", "uncertainty", "uncertainty")], [*STILL, "COL-2", "SIZE-1"]),
    case("B-tube-1AON", "1AON", ["show tube as main, protein", "color chain, @main"],
         ["show cartoon, fixture", "cartoon tube", "util.cbc fixture"], [rep("cartoon", aspectRatio=1, tubularHelices=True, visuals=["polymer-trace"])],
         [*STILL, "CT-1"]),
    case("B-trace-1AON", "1AON", ["show trace as main, protein", "color chain, @main"],
         ["show ribbon, fixture", "util.cbc fixture"],
         [rep("cartoon", sizeFactor=0.05, aspectRatio=1, visuals=["polymer-trace"])], [*STILL, "CT-1"]),
    case("B-backbone-1AON", "1AON", ["show backbone as main, protein", "color chain, @main"],
         ["show sticks, name ca+n+c", "util.cbc fixture"], [rep("backbone")], [*STILL, "CT-1"]),
    case("B-beads-1AON", "1AON", ["show beads as main, protein", "color chain, @main"],
         ["show spheres, name ca", "util.cbc fixture"], [rep("spacefill")], [*STILL, "EL-2"]),
    case("B-glycan-1HZH", "1HZH", ["show cartoon, protein", "show glycan, glycans"],
         ["show cartoon, polymer", "show sticks, not polymer and not solvent", "util.cbc fixture"],
         [rep("cartoon"), rep("carbohydrate", "carbohydrate-symbol")], [*STILL, "GLY-4"]),
    *[color_case(*args) for args in [
        ("element", ["util.cbag fixture"], "element-symbol"),
        ("chain", ["util.cbc fixture"], "chain-id"),
        ("entity", ["util.cbc fixture"], "entity-id"),
        ("molecule_type", ["util.cbc fixture"], "molecule-type"),
        ("residue_name", ["util.cbc fixture"], "residue-name"),
        ("secondary_structure", ["color red, ss h", "color yellow, ss s", "color green, ss l+''"],
         "secondary-structure"),
        ("carbon_by_chain", ["util.cbc fixture"], "chain-id"),
        ("b_factor", ["spectrum b, blue_white_red, fixture"], "uncertainty"),
        ("hydrophobicity", ["spectrum resi, red_yellow_green, fixture"], "hydrophobicity"),
        ("rainbow", ["spectrum count, rainbow, fixture"], "sequence-id"),
        ("plddt", ["spectrum b, blue_white_red, fixture"], "uncertainty"),
        ("sasa", ["spectrum b, blue_white_red, fixture"], "uncertainty"),
    ]],
    case("B-pocket-HEM-4HHB", "4HHB", ["pocket, resname HEM"],
         ["show cartoon, fixture", "show sticks, resn HEM", "show lines, byres (resn HEM around 6)",
          "util.cbc fixture"],
         [rep("cartoon"), rep("ball-and-stick", "element-symbol")], [*STILL, "BS-3", "SURF-5"],
         fit="resname HEM", margin=2.5),
    case("B-unitcell-1HVR", "1HVR", ["show cartoon, protein",
                                     'assembly {"unit_cell":{"lengths":[58.4,86.1,46.2],"angles":[90,90,90]}}'],
         ["show cartoon, fixture", "util.cbc fixture"], [rep("cartoon")], [*STILL, "ASM-2"]),
    case("B-plane", "4HHB", ["show cartoon, protein",
                             'plane {"structure":1,"center":[0,0,0],"normal":[0,0,1],"size":[40,40]}'],
         ["show cartoon, fixture", "util.cbc fixture"], [rep("cartoon")], [*STILL]),
    case("B-label-4HHB", "4HHB", ["show cartoon, protein", 'label "Heme", resname HEM'],
         ["show cartoon, fixture", "label resn HEM and name FE, resn"], [rep("cartoon"),
          {"kind": "label", "atoms": [{"auth_asym_id": "A", "auth_seq_id": 142, "auth_atom_id": "FE"}]}],
         [*STILL, "LBL-1", "LBL-5"]),
    case("B-distance-angle-dihedral-4HHB", "4HHB",
         ["show cartoon, protein", "distance, resname HEM, resid 87 and name NE2"],
         ["show cartoon, fixture", "distance d1, resn HEM and name FE and chain A, resi 87 and name NE2 and chain A"],
         [rep("cartoon"),
          {"kind": "measurement", "measure": "distance", "atoms": [
              {"auth_asym_id": "A", "auth_seq_id": 142, "auth_atom_id": "FE"},
              {"auth_asym_id": "A", "auth_seq_id": 87, "auth_atom_id": "NE2"}]}],
         [*STILL, "MEAS-1"], fit="resname HEM", margin=3.0),
    case("B-orbit-video-1AON", "1AON", ["show cartoon as main, protein", "color chain, @main"],
         PY_CARTOON, [rep("cartoon")], ["VID-1", "CT-6"], video={"kind": "orbit_y", "frames": 120, "fps": 30}),
    case("B-trajectory-video-1D3Z", "1D3Z", ["show cartoon as main, protein", "color chain, @main"],
         PY_CARTOON, [rep("cartoon")], ["VID-2"], video={"kind": "trajectory", "frames": 10, "fps": 5}),
]


def build():
    corpus = json.loads((HERE / "corpus.json").read_text())
    base = {f["id"]: f for f in corpus["fixtures"]}
    out = {key: corpus[key] for key in ("extent", "physical", "style", "recipe_policy", "recipe_settings")}
    out["extent"] = [768, 768]
    out["warmup_outputs"], out["measured_outputs"] = 0, 1
    out["recipes"] = ["molgfx-publication", "molgfx-interactive", "pymol-ray", "molstar-imagepass"]
    out["recipe_settings"] = {k: v for k, v in corpus["recipe_settings"].items() if k in out["recipes"]}
    fixtures = []
    for c in CASES:
        b = base[c["fixture"]]
        script = {"molgfx": c["molgfx"], "pymol": c["pymol"], "molstar": c["molstar"],
                  "checklist": c["checklist"], "fit": {"selection": c["fit"], "fov_y_degrees": 45, "margin": c["margin"]}}
        if c["video"]:
            script["video"] = c["video"]
        entry = {k: b[k] for k in b if k not in ("id", "form", "script")}
        entry.update({"id": c["id"], "form": "script", "script": script})
        fixtures.append(entry)
    out["fixtures"] = fixtures
    return out


if __name__ == "__main__":
    (HERE / "gallery.json").write_text(json.dumps(build(), indent=2) + "\n")
    print(len(CASES), "cases")
