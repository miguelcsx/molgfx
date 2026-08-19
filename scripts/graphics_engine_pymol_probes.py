"""Extended, disposable PyMOL feature probes for the parity audit."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from graphics_engine_volume_differential import write_mrc
from graphics_engine_cross_render import read_png


MULTISTATE_PDB = b"""HEADER    PYMOL MULTISTATE PROBE
MODEL        1
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 11.00           C
ATOM      3  C   ALA A   1       2.000   1.400   0.000  1.00 12.00           C
ATOM      4  O   ALA A   1       1.500   2.500   0.000  1.00 13.00           O
ENDMDL
MODEL        2
ATOM      1  N   ALA A   1       0.100   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.550   0.000   0.000  1.00 11.00           C
ATOM      3  C   ALA A   1       2.100   1.400   0.000  1.00 12.00           C
ATOM      4  O   ALA A   1       1.600   2.500   0.000  1.00 13.00           O
ENDMDL
END
"""


CRYSTAL_PDB = b"""HEADER    PYMOL CRYSTAL PROBE
CRYST1   20.000   20.000   20.000  90.00  90.00  90.00 P 1           1
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 11.00           C
ATOM      3  C   ALA A   1       2.000   1.400   0.000  1.00 12.00           C
ATOM      4  O   ALA A   1       1.500   2.500   0.000  1.00 13.00           O
END
"""


def extended_pymol_probes(
    cmd: Any, root: Path, run_probe: Callable[[str, Callable[[], Any]], dict[str, Any]]
) -> list[dict[str, Any]]:
    """Exercise documented feature groups not covered by the core smoke matrix."""

    def representation_families() -> dict[str, Any]:
        kinds = ("lines", "sticks", "spheres", "ribbon", "cartoon", "surface", "mesh", "dots")
        for kind in kinds:
            cmd.show_as(kind, "ala")
        cmd.hide("everything", "ala")
        return {"representations": kinds, "count": cmd.count_atoms("ala")}

    def measurements_and_queries() -> dict[str, Any]:
        return {
            "angle": cmd.angle(
                "probe_angle", "ala and name N", "ala and name CA", "ala and name C", quiet=1
            ),
            "dihedral": cmd.dihedral(
                "probe_dihedral",
                "ala and name N",
                "ala and name CA",
                "ala and name C",
                "ala and name O",
                quiet=1,
            ),
            "area": cmd.get_area("ala", load_b=1),
            "extent": cmd.get_extent("ala"),
            "pairs": cmd.find_pairs("ala and name N", "ala and name O", cutoff=3.0),
        }

    def labels_and_custom_annotations() -> dict[str, Any]:
        cmd.pseudoatom("probe_pseudo", pos=[3.0, 3.0, 3.0], label="pseudo")
        cmd.label("ala and name CA", '"CA"')
        cmd.set("label_color", "yellow")
        cmd.distance("probe_guide", "ala and name N", "ala and name O", quiet=1)
        return {"pseudoatoms": cmd.count_atoms("probe_pseudo"), "labels": cmd.count_atoms("ala and name CA")}

    def surface_variants() -> tuple[str, ...]:
        for kind in ("surface", "mesh", "dots", "nonbonded", "nb_spheres"):
            cmd.show(kind, "ala")
        cmd.set("transparency", 20, "ala")
        cmd.set("dot_density", 2)
        return ("surface", "mesh", "dots", "nonbonded", "nb_spheres")

    def map_slice_and_transfer_functions() -> dict[str, Any]:
        map_path = root / "pymol-map-ext.mrc"
        write_mrc(map_path, (32, 32, 32))
        cmd.load(str(map_path), "probe_map_ext")
        cmd.isosurface("probe_iso_ext", "probe_map_ext", 0.2)
        cmd.isomesh("probe_mesh_ext", "probe_map_ext", 0.2)
        cmd.isodot("probe_dot_ext", "probe_map_ext", 0.2)
        cmd.slice_new("probe_slice_ext", "probe_map_ext")
        cmd.volume("probe_volume_ext", "probe_map_ext")
        cmd.volume_ramp_new("probe_ramp_ext", [-1.0, "red", 0.0, 0.0, "white", 0.5, 1.0, "blue", 0.0])
        cmd.orient("all")
        image = root / "pymol-map.png"
        cmd.png(str(image), width=96, height=96, ray=0, quiet=1)
        return {
            "objects": [
                name for name in cmd.get_names("objects") if name.startswith("probe_") and name.endswith("_ext")
            ],
            "states": cmd.count_states("probe_map_ext"),
            "png_bytes": image.stat().st_size,
        }

    def multistate_and_frame_controls() -> dict[str, Any]:
        cmd.read_pdbstr(MULTISTATE_PDB, "probe_ensemble")
        cmd.frame(2)
        cmd.mset("1 x2")
        cmd.mview("store", 1)
        cmd.mview("store", 2)
        return {"states": cmd.count_states("probe_ensemble"), "frames": cmd.count_frames()}

    def symmetry_and_unit_cell() -> dict[str, Any]:
        cmd.read_pdbstr(CRYSTAL_PDB, "probe_crystal")
        symmetry = cmd.get_symmetry("probe_crystal")
        cmd.show("cell", "probe_crystal")
        cmd.symexp("probe_mates", "probe_crystal", "all", 5.0)
        return {"symmetry": symmetry, "cell_objects": cmd.get_object_list("probe_mates*")}

    def cgo_and_viewport_controls() -> dict[str, Any]:
        from pymol.cgo import BEGIN, COLOR, END, SPHERE

        cmd.load_cgo([BEGIN, SPHERE, 3.0, 3.0, 3.0, 0.5, COLOR, 1.0, 0.0, 0.0, END], "probe_cgo")
        cmd.viewport(128, 128)
        cmd.stereo("on")
        cmd.stereo("off")
        return {"cgo_states": cmd.count_states("probe_cgo"), "viewport": cmd.get_viewport()}

    def cgo_primitive_family() -> dict[str, Any]:
        """Exercise the runtime's compiled-graphics primitive vocabulary."""

        from pymol import cgo
        from pymol.vfont import plain

        objects: dict[str, list[float]] = {
            "sphere": [cgo.COLOR, 1, 0, 0, cgo.SPHERE, 0, 0, 0, 0.5],
            "cylinder": [
                cgo.CYLINDER,
                -2,
                0,
                0,
                -1,
                0,
                0,
                0.2,
                1,
                0,
                0,
                1,
                1,
                0,
            ],
            "cone": [
                cgo.CONE,
                -1,
                0,
                0,
                0,
                0,
                0,
                0.35,
                0,
                1,
                0,
                0,
                0,
                1,
                1,
                1,
            ],
            "sausage": [
                cgo.SAUSAGE,
                0,
                0,
                0,
                1,
                0,
                0,
                0.25,
                1,
                0,
                0,
                0,
                1,
                0,
            ],
            "custom-cylinder": [
                cgo.CUSTOM_CYLINDER,
                0,
                -1,
                0,
                1,
                -1,
                0,
                0.15,
                1,
                0.5,
                0,
                0,
                1,
                0.5,
                0,
                0,
                1,
                0,
                0,
                0,
            ],
            "triangles": [
                cgo.BEGIN,
                cgo.TRIANGLES,
                cgo.COLOR,
                0,
                1,
                0,
                cgo.NORMAL,
                0,
                0,
                1,
                cgo.VERTEX,
                0,
                0,
                0,
                cgo.NORMAL,
                0,
                0,
                1,
                cgo.VERTEX,
                1,
                0,
                0,
                cgo.NORMAL,
                0,
                0,
                1,
                cgo.VERTEX,
                0,
                1,
                0,
                cgo.END,
            ],
            "alpha-triangle": [
                cgo.ALPHA_TRIANGLE,
                0.5,
                0,
                0,
                0,
                0,
                1,
                1,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
                0,
            ],
            "line-strip": [
                cgo.BEGIN,
                cgo.LINE_STRIP,
                cgo.LINEWIDTH,
                2.0,
                cgo.COLOR,
                1,
                1,
                0,
                cgo.VERTEX,
                0,
                0,
                0,
                cgo.VERTEX,
                1,
                1,
                0,
                cgo.VERTEX,
                2,
                0,
                0,
                cgo.END,
            ],
            "points": [
                cgo.BEGIN,
                cgo.POINTS,
                cgo.COLOR,
                1,
                0,
                1,
                cgo.VERTEX,
                0,
                0,
                0,
                cgo.VERTEX,
                1,
                1,
                1,
                cgo.END,
            ],
            "ellipsoid": [
                cgo.ELLIPSOID,
                0,
                0,
                1,
                1,
                2,
                3,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
            ],
            "quadric": [
                cgo.QUADRIC,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
            ],
            "bezier": [
                cgo.BEZIER,
                0,
                0,
                0,
                1,
                0,
                0,
                1,
                1,
                0,
                0,
                1,
                1,
                0,
            ],
        }
        text_object: list[float] = []
        cgo.wire_text(
            text_object,
            plain,
            [-1, -1, 0],
            "CGO",
            [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        )
        objects["wire-text"] = text_object
        loaded: list[str] = []
        for kind, payload in objects.items():
            object_name = f"probe_cgo_{kind.replace('-', '_')}"
            cmd.load_cgo(payload, object_name)
            loaded.append(object_name)
        cmd.set("cgo_line_radius", 0.03)
        cmd.zoom("all", 2.0)
        image = root / "pymol-cgo-primitives.png"
        cmd.png(str(image), width=192, height=192, ray=1, quiet=1)
        width, height, pixels = read_png(image)
        signal = sum(1 for red, green, blue, _alpha in pixels if max(red, green, blue) > 4)
        if signal == 0:
            raise RuntimeError("CGO primitive fixture rendered no non-background pixels")
        return {
            "primitive_count": len(objects),
            "loaded_objects": loaded,
            "states": {name: cmd.count_states(name) for name in loaded},
            "opcodes": sorted(
                name
                for name in dir(cgo)
                if name.isupper() and name not in {"DEFAULT_ERROR", "DEFAULT_SUCCESS"}
            ),
            "image": {"width": width, "height": height, "signal_pixels": signal},
        }

    def movie_frame_export() -> dict[str, Any]:
        prefix = str(root / "movie-frame")
        cmd.mset("1 x2")
        cmd.mpng(prefix, first=1, last=2)
        return {"frames": sorted(path.name for path in root.glob("movie-frame*.png"))}

    def selection_language() -> dict[str, int]:
        expressions = {
            "byres": "byres (ala and name CA)",
            "bychain": "bychain ala",
            "backbone": "ala and backbone",
            "within": "ala within 3 of ala and name CA",
            "organic": "ala and organic",
        }
        counts = {name: cmd.count_atoms(expression) for name, expression in expressions.items()}
        cmd.select("probe_selection", "ala and (name N or name CA or name C or name O)")
        counts["named"] = cmd.count_atoms("probe_selection")
        return counts

    def cartoon_styles() -> dict[str, str]:
        styles = ("automatic", "loop", "oval", "rectangle", "dumbbell", "putty", "tube", "arrow")
        cmd.show("cartoon", "ala")
        for style in styles:
            cmd.cartoon(style, "ala")
        return {"styles": ",".join(styles), "object": "ala"}

    def presentation_and_materials() -> dict[str, Any]:
        settings = {
            "orthoscopic": 1,
            "depth_cue": 1,
            "fog": 1,
            "ambient_occlusion_mode": 1,
            "ambient_occlusion_scale": 3.0,
            "ray_trace_mode": 1,
            "ray_shadow": 1,
            "specular": 0.5,
            "shininess": 50.0,
        }
        for name, value in settings.items():
            cmd.set(name, value)
        cmd.set("transparency", 35, "ala")
        cmd.set("cartoon_transparency", 20, "ala")
        cmd.bg_color("white")
        cmd.set("bg_gradient", 1)
        return {"settings": sorted(settings), "transparent_atoms": True}

    def surface_controls() -> dict[str, Any]:
        cmd.show("surface", "ala")
        for name, value in (
            ("surface_quality", 1),
            ("surface_solvent", 1),
            ("solvent_radius", 1.4),
            ("surface_carve_cutoff", 5.0),
            ("surface_carve_state", 1),
            ("dot_density", 3),
        ):
            cmd.set(name, value)
        cmd.rebuild("ala")
        return {"surface_quality": cmd.get("surface_quality"), "dot_density": cmd.get("dot_density")}

    def property_coloring_and_bonds() -> dict[str, Any]:
        cmd.spectrum("b", "blue_white_red", "ala")
        cmd.set_bond("stick_radius", 0.22, "ala", "ala")
        cmd.set("line_width", 2.0)
        cmd.set("stick_quality", 10)
        cmd.set("sphere_quality", 2)
        cmd.set("sphere_scale", 0.32, "ala")
        values = [atom.b for atom in cmd.get_model("ala").atom]
        return {"b_range": [min(values), max(values)], "atoms": len(values)}

    def object_groups_and_visibility() -> dict[str, Any]:
        cmd.create("probe_copy", "ala")
        cmd.group("probe_group", "ala probe_copy")
        cmd.disable("probe_copy")
        cmd.enable("probe_copy")
        cmd.order("probe_*", sort="yes")
        return {"objects": cmd.get_object_list("probe_*"), "grouped": True}

    def analysis_and_alignment() -> dict[str, Any]:
        cmd.create("probe_aligned", "ala")
        cmd.translate([0.1, 0.0, 0.0], "probe_aligned")
        alignment = cmd.align("probe_aligned", "ala", cycles=0, quiet=1)
        rms = cmd.rms_cur("probe_aligned", "ala", cycles=0, quiet=1)
        return {"alignment": alignment, "rms": rms, "sasa_relative": cmd.get_sasa_relative("ala")}

    def camera_and_stereo() -> dict[str, Any]:
        cmd.orient("ala")
        cmd.set_view(cmd.get_view())
        cmd.clip("slab", 10.0)
        cmd.stereo("walleye")
        cmd.stereo("off")
        return {"view_values": len(cmd.get_view()), "viewport": cmd.get_viewport()}

    def publication_export() -> dict[str, int]:
        povray = cmd.get_povray()
        cmd.draw(width=96, height=96, quiet=1)
        cmd.png(str(root / "plain.png"), width=96, height=96, ray=0, quiet=1)
        return {
            "povray_characters": len(povray or ""),
            "png_bytes": (root / "plain.png").stat().st_size,
        }

    return [
        run_probe("representation-families", representation_families),
        run_probe("measurements-and-queries", measurements_and_queries),
        run_probe("labels-and-custom-annotations", labels_and_custom_annotations),
        run_probe("surface-variants", surface_variants),
        run_probe("map-slice-and-transfer-functions", map_slice_and_transfer_functions),
        run_probe("multistate-and-frame-controls", multistate_and_frame_controls),
        run_probe("symmetry-and-unit-cell", symmetry_and_unit_cell),
        run_probe("cgo-and-viewport-controls", cgo_and_viewport_controls),
        run_probe("cgo-primitive-family", cgo_primitive_family),
        run_probe("movie-frame-export", movie_frame_export),
        run_probe("selection-language", selection_language),
        run_probe("cartoon-styles", cartoon_styles),
        run_probe("presentation-and-materials", presentation_and_materials),
        run_probe("surface-controls", surface_controls),
        run_probe("property-coloring-and-bonds", property_coloring_and_bonds),
        run_probe("object-groups-and-visibility", object_groups_and_visibility),
        run_probe("analysis-and-alignment", analysis_and_alignment),
        run_probe("camera-and-stereo", camera_and_stereo),
        run_probe("publication-export", publication_export),
    ]
