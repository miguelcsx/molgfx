"""Stateful PyMOL probes for renderer and scene controls.

The exhaustive command audit deliberately uses bounded generic arguments.  This
module supplies the state that those commands require, so a rejected generic
call is not mistaken for a missing graphics capability.
"""

from __future__ import annotations

from typing import Any, Callable


CRYSTAL_PDB = b"""HEADER    PYMOL CRYSTAL CONTROL PROBE
CRYST1   20.000   20.000   20.000  90.00  90.00  90.00 P 1           1
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 11.00           C
ATOM      3  C   ALA A   1       2.000   1.400   0.000  1.00 12.00           C
ATOM      4  O   ALA A   1       1.500   2.500   0.000  1.00 13.00           O
END
"""


def renderer_control_probes(
    cmd: Any, run_probe: Callable[[str, Callable[[], Any]], dict[str, Any]]
) -> list[dict[str, Any]]:
    """Exercise stateful camera, map, symmetry, scene and movie commands."""

    def camera_clip_modes() -> dict[str, Any]:
        cmd.orient("ala")
        cmd.look_at("ala")
        operations = (
            ("near", (0.5,)),
            ("far", (100.0,)),
            ("move", (1.0,)),
            ("slab", (10.0,)),
            ("atoms", (2.0, "ala")),
            ("near_set", (1.0, "ala and name CA")),
            ("far_set", (1.0, "ala and name CA")),
        )
        for mode, arguments in operations:
            cmd.clip(mode, *arguments)
        return {"modes": [mode for mode, _ in operations], "view_values": len(cmd.get_view())}

    def scene_and_view_round_trip() -> dict[str, Any]:
        cmd.view("probe_view", "store")
        cmd.scene("probe_scene", "store")
        cmd.scene_order(["probe_scene"], "yes", "top")
        cmd.view("probe_view")
        cmd.scene("probe_scene", "recall")
        return {"view": "probe_view", "scene": "probe_scene"}

    def map_operators_and_ramp_updates() -> dict[str, Any]:
        cmd.map_new("probe_map_a", "gaussian", 1.0, "ala", 2.0)
        cmd.map_new("probe_map_b", "gaussian", 1.0, "ala", 2.0)
        for name, operator in (
            ("probe_map_sum", "sum"),
            ("probe_map_average", "average"),
            ("probe_map_difference", "difference"),
        ):
            cmd.map_set(name, operator, "probe_map_a probe_map_b")
        cmd.ramp_new(
            "probe_ramp_controls",
            "probe_map_a",
            [-1.0, 0.0, 1.0],
            ["red", "white", "blue"],
        )
        cmd.ramp_update("probe_ramp_controls", range=[-2.0, 0.0, 2.0])
        cmd.ramp_update("probe_ramp_controls", color=["green", "white", "orange"])
        return {
            "operators": ["sum", "average", "difference"],
            "map_objects": [name for name in cmd.get_names("objects") if name.startswith("probe_map_")],
            "ramp": "probe_ramp_controls",
        }

    def crystal_symmetry_controls() -> dict[str, Any]:
        cmd.read_pdbstr(CRYSTAL_PDB, "probe_crystal_controls")
        cmd.create("probe_crystal_copy", "probe_crystal_controls")
        symmetry = cmd.get_symmetry("probe_crystal_controls")
        cmd.symmetry_copy("probe_crystal_controls", "probe_crystal_copy")
        cmd.show("cell", "probe_crystal_controls")
        return {"symmetry": symmetry, "copied": "probe_crystal_copy"}

    def movie_keyframe_editing() -> dict[str, Any]:
        cmd.mset("1 x4")
        cmd.mview("store", 1)
        cmd.mview("store", 3)
        cmd.mcopy(4, 1)
        cmd.minsert(1, 2)
        cmd.mmove(3, 1, 1)
        cmd.mset("1 - 30")
        cmd.movie.tdroll(10, 0, 0, 10)
        return {"frames": cmd.count_frames(), "keyframes": [1, 3], "tdroll_frames": 10}

    return [
        run_probe("camera-clip-modes", camera_clip_modes),
        run_probe("scene-and-view-round-trip", scene_and_view_round_trip),
        run_probe("map-operators-and-ramp-updates", map_operators_and_ramp_updates),
        run_probe("crystal-symmetry-controls", crystal_symmetry_controls),
        run_probe("movie-keyframe-editing", movie_keyframe_editing),
    ]
