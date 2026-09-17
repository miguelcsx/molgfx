#!/usr/bin/env python3
"""Attempt every public PyMOL command in an isolated, disposable fixture.

This is deliberately not a claim that a no-argument call is a meaningful
feature test.  It records the stronger distinction the parity audit needs:
directly attempted, passed, rejected by the fixture, requiring external input,
or skipped because executing it would leave the process or run arbitrary code.
The command registry and help text remain the authoritative inventory.
"""

from __future__ import annotations

import argparse
import contextlib
import inspect
import io
import json
import sys
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any

from graphics_engine_taxonomy import (
    NON_RENDERING_FAMILIES,
    classify_command,
    molgfx_mapping,
)
from graphics_engine_volume_differential import write_mrc


SKIP_REASONS = {
    "quit": "would terminate the audit process",
    "abort": "process-control command",
    "reinitialize": "would erase the shared fixture",
    "system": "would execute an arbitrary host command",
    "spawn": "would execute an arbitrary host process",
    "fork": "would execute an arbitrary host process",
    "run": "would execute an arbitrary script",
    "python": "would execute arbitrary Python",
    "exec": "would execute arbitrary Python",
    "import": "would execute arbitrary Python import code",
    "class": "Python language command",
    "def": "Python language command",
    "del": "Python language command",
    "for": "Python language command",
    "global": "Python language command",
    "if": "Python language command",
    "pass": "Python language command",
    "raise": "Python language command",
    "try": "Python language command",
    "while": "Python language command",
    "assert": "Python language command",
    "button": "GUI-only command",
    "cls": "GUI-only command",
    "drag": "GUI-only command",
    "edit": "GUI-only command",
    "embed": "GUI/application command",
    "feedback": "GUI/application command",
    "full_screen": "GUI-only command",
    "refresh_wizard": "GUI/application command",
    "splash": "GUI/application command",
    "update": "application update command",
    "window": "GUI-only command",
    "wizard": "GUI/application command",
    "volume_panel": "GUI-only command",
    "cycle_valence": "requires an interactive picked bond in editing mode",
    "h_fill": "requires an interactive picked atom in editing mode",
    "h_fix": "requires an interactive picked atom in editing mode",
    "invert": "requires an interactive picked stereocenter in editing mode",
    "remove_picked": "requires an interactive picked atom in editing mode",
    "torsion": "requires an interactive picked bond in editing mode",
}


EXTERNAL_INPUT = {
    "fetch": "network/database fetch intentionally disabled",
    "load_mtz": "requires an MTZ/reflection fixture",
    "load_png": "requires a valid image/movie input fixture",
    "load_traj": "requires a trajectory format fixture",
    "movie.load": "requires an image sequence fixture",
    "resume": "requires a saved session or script",
}


PDB_TEXT = """ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.000   1.300   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.300   2.250   0.000  1.00 10.00           O
TER
END
"""

MULTISTATE_PDB = """MODEL        1
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.000   1.300   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.300   2.250   0.000  1.00 10.00           O
ENDMDL
MODEL        2
ATOM      1  N   ALA A   1       0.100   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.550   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.100   1.300   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.400   2.250   0.000  1.00 10.00           O
ENDMDL
END
"""

CRYSTAL_PDB = """CRYST1   20.000   20.000   20.000  90.00  90.00  90.00 P 1           1
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.000   1.300   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.300   2.250   0.000  1.00 10.00           O
END
"""


def alignment_pdb() -> str:
    """Build a minimal multi-residue fixture for structure-alignment commands."""

    lines: list[str] = []
    serial = 1
    for residue in range(1, 17):
        offset = (residue - 1) * 3.8
        for atom, x_offset, y_offset, element in (
            ("N", 0.0, 0.0, "N"),
            ("CA", 1.45, 0.2, "C"),
            ("C", 2.8, 0.0, "C"),
            ("O", 3.3, 1.0, "O"),
        ):
            lines.append(
                f"ATOM  {serial:5d} {atom:>4s} ALA A{residue:4d}"
                f"    {offset + x_offset:8.3f}{y_offset:8.3f}{0.0:8.3f}"
                f"  1.00 10.00           {element:>2s}"
            )
            serial += 1
    lines.extend(("TER", "END"))
    return "\n".join(lines) + "\n"


ALIGNMENT_PDB = alignment_pdb()


def compact_error(error: BaseException) -> str:
    return f"{type(error).__name__}: {error}"[:500]


def failure_class(error: BaseException) -> str:
    text = compact_error(error)
    if "Incentive-Only-Error" in text:
        return "runtime-feature-unavailable"
    if "ModuleNotFoundError" in text or "ImportError" in text:
        return "runtime-dependency-unavailable"
    if any(token in text for token in ("Invalid selection", "object not found", "Object not found", "No object")):
        return "fixture-context-rejected"
    if any(token in text for token in ("unknown mode", "unknown operator", "unknown flag", "could not convert")):
        return "fixture-enum-rejected"
    if any(token in text for token in ("No scenes", "No movie", "frame", "not defined")):
        return "fixture-state-rejected"
    return "fixture-argument-rejected"


def value_for(parameter: inspect.Parameter, command: str, root: Path) -> Any:
    """Build a bounded fixture value from a PyMOL parameter name."""

    name = parameter.name.lower()
    overrides = {
        ("clip", "mode"): "slab",
        ("flag", "flag"): "fix",
        ("fragment", "fragment"): "ala",
        ("get_symmetry", "selection"): "model probe_crystal",
        ("get_type", "name"): "ala",
        ("isolevel", "name"): "probe_iso",
        ("load", "filename"): str(root / "fixture.pdb"),
        ("look_at", "target_obj"): "ala",
        ("map_set", "operator"): "sum",
        ("matrix_reset", "name"): "ala",
        ("scene_order", "names"): "probe_scene",
        ("select", "name"): "probe_selection",
        ("set", "name"): "orthoscopic",
        ("set_bond", "name"): "stick_radius",
        ("unset", "name"): "orthoscopic",
        ("unset_bond", "name"): "stick_radius",
        ("view", "key"): "store",
    }
    if (command, name) in overrides:
        return overrides[(command, name)]
    if command in {"mcopy", "mmove"} and name in {"target", "source"}:
        return 2 if name == "target" else 1
    if command == "minsert" and name in {"count", "frame"}:
        return 1
    if command == "movie.tdroll" and name in {"first", "rangex", "rangey", "rangez"}:
        return 1
    if name in {"selection", "selection1", "selection2", "selection_1", "selection_2"}:
        return "ala"
    if name in {"mobile", "target", "source", "source_name", "target_name", "oname"}:
        return "ala2" if name == "mobile" else "ala"
    if name in {"object", "object_name", "object1", "object2", "name", "old_name"}:
        return "probe_object"
    if name in {"new_name", "target_name"}:
        return "probe_object"
    if name in {"filename", "fnam", "file", "pattern", "prefix"}:
        extension = ".pdb" if name in {"filename", "fnam", "file"} else "probe"
        return str(root / f"{command.replace('.', '_')}{extension}")
    if name in {"map", "map_name", "source_map"}:
        return "probe_map"
    if name in {"ramp", "ramp_name"}:
        return "probe_ramp"
    if name in {"view"}:
        return [1.0] * 18
    if name in {"vector", "pos", "center", "origin", "shift"}:
        return [0.0, 0.0, 0.0]
    if name in {"rgb", "color_rgb"}:
        return [1.0, 0.0, 0.0]
    if name in {"range", "clamp"}:
        return [-1.0, 0.0, 1.0]
    if name in {"color", "colour"}:
        return "red"
    if name in {"representation", "rep"}:
        return "sticks"
    if name in {"type"}:
        return "automatic" if command == "cartoon" else "wire"
    if name in {"mode"}:
        return "store" if command.startswith("movie.") else 1
    if name in {"action"}:
        return "store"
    if name in {"axis"}:
        return "x"
    if name in {"element"}:
        return "C"
    if name in {"geometry", "valence", "order"}:
        return 1
    if name in {"expression"}:
        return "b=1.0"
    if name in {"string", "command", "text", "label"}:
        return "probe"
    if name in {"input"}:
        return "A"
    if name in {"spacegroup"}:
        return "P1"
    if name in {"key"}:
        return "probe"
    if name in {"state", "state1", "state2", "source_state", "target_state", "target_state"}:
        return 1
    if name in {"first", "last", "frame", "trigger", "cycles", "step", "level"}:
        return 1
    if name in {"distance", "angle", "cutoff", "buffer", "minimum", "maximum"}:
        return 1.0
    if name in {"a", "b", "c", "alpha", "beta", "gamma"}:
        return 10.0 if name in {"a", "b", "c"} else 90.0
    if name in {"quiet", "zoom", "quiet", "animate", "updates", "log", "async_"}:
        return 1
    if name in {"name", "map", "selection"}:
        return "probe"
    if parameter.annotation is int:
        return 1
    if parameter.annotation is float:
        return 1.0
    return "probe"


def prepare_fixture(cmd: Any, root: Path) -> None:
    cmd.reinitialize()
    cmd.fragment("ala", "ala")
    cmd.create("ala2", "ala")
    cmd.create("probe_object", "ala")
    cmd.select("probe_selection", "ala")
    cmd.select("pk1", "ala and name N")
    cmd.select("pk2", "ala and name CA")
    cmd.select("pk3", "ala and name C")
    cmd.select("pk4", "ala and name O")
    (root / "fixture.pdb").write_text(PDB_TEXT, encoding="utf-8")
    cmd.read_pdbstr(MULTISTATE_PDB, "probe_ensemble")
    cmd.read_pdbstr(CRYSTAL_PDB, "probe_crystal")
    cmd.create("probe_crystal_copy", "probe_crystal")
    cmd.read_pdbstr(ALIGNMENT_PDB, "probe_alignment")
    cmd.create("probe_alignment_copy", "probe_alignment")
    density_path = root / "probe-density.mrc"
    write_mrc(density_path, (16, 16, 16))
    cmd.load(str(density_path), "probe_density")
    cmd.map_new("probe_map", "gaussian", 1.0, "ala", 2.0)
    cmd.ramp_new("probe_ramp", "probe_map", [-1.0, 0.0, 1.0], ["red", "white", "blue"])
    cmd.isosurface("probe_iso", "probe_density", 0.2)
    cmd.mset("1 x2")
    cmd.scene("probe_scene", "store")


def required_parameters(function: Any) -> list[inspect.Parameter] | None:
    try:
        signature = inspect.signature(function)
    except (TypeError, ValueError):
        return None
    required: list[inspect.Parameter] = []
    for parameter in signature.parameters.values():
        if parameter.name in {"_self", "self"} or parameter.kind in {
            inspect.Parameter.VAR_POSITIONAL,
            inspect.Parameter.VAR_KEYWORD,
        }:
            continue
        if parameter.default is inspect.Parameter.empty:
            required.append(parameter)
    return required


def attempt_command(cmd: Any, name: str, root: Path) -> dict[str, Any]:
    family = classify_command(name)
    mapping = molgfx_mapping(name, family)
    record: dict[str, Any] = {
        "name": name,
        "family": family,
        "scope": "non-rendering-owner" if family in NON_RENDERING_FAMILIES else "graphics-or-scene-owner",
        "molgfx": mapping,
        "function": getattr(cmd.keyword[name][0], "__name__", "unknown"),
    }
    if name in SKIP_REASONS:
        record.update({"status": "skipped-safety", "reason": SKIP_REASONS[name]})
        return record
    if name in EXTERNAL_INPUT:
        record.update({"status": "requires-external-input", "reason": EXTERNAL_INPUT[name]})
        return record
    function = cmd.keyword[name][0]
    required = required_parameters(function)
    if required is None:
        record.update({"status": "not-attempted", "reason": "signature unavailable"})
        return record
    if any(parameter.kind == inspect.Parameter.KEYWORD_ONLY for parameter in required):
        record.update({"status": "not-attempted", "reason": "required keyword-only fixture is ambiguous"})
        return record
    special_arguments = {
        "angle": ["probe_angle", "pk1", "pk2", "pk3"],
        "align": ["ala2", "ala"],
        "alignto": ["probe_crystal", "align", "probe_crystal_copy"],
        "bond": ["pk1", "pk2"],
        "cealign": ["probe_alignment", "probe_alignment_copy"],
        "dihedral": ["probe_dihedral", "pk1", "pk2", "pk3", "pk4"],
        "distance": ["probe_distance", "pk1", "pk2"],
        "fragment": ["ala", "probe_fragment"],
        "fuse": ["pk1", "pk2"],
        "get_angle": ["pk1", "pk2", "pk3"],
        "get_area": ["ala and name CA"],
        "get": ["orthoscopic"],
        "get_bond": ["stick_radius", "ala", "ala"],
        "get_dihedral": ["pk1", "pk2", "pk3", "pk4"],
        "get_distance": ["pk1", "pk2"],
        "get_symmetry": ["model probe_crystal"],
        "gradient": ["probe_gradient", "probe_map"],
        "group": ["probe_group", "ala ala2"],
        "id_atom": ["pk1"],
        "isodot": ["probe_dot", "probe_density"],
        "isolevel": ["probe_iso", 0.2],
        "isomesh": ["probe_mesh", "probe_density"],
        "isosurface": ["probe_surface", "probe_density"],
        "map_set": ["probe_map_sum", "sum", "probe_map"],
        "multifilesave": [str(root / "multifilesave-{num}.pdb"), "ala", 1],
        "pair_fit": ["ala and name N", "ala2 and name N", "ala and name CA", "ala2 and name CA"],
        "ramp_update": ["probe_ramp", [-1.0, 0.0, 1.0], ["red", "white", "blue"]],
        "set_bond": ["stick_radius", 0.2, "ala"],
        "set_dihedral": ["pk1", "pk2", "pk3", "pk4", 60.0],
        "select": ["probe_selection", "ala"],
        "slice_new": ["probe_slice", "probe_density"],
        "split_states": ["probe_ensemble"],
        "pop": ["probe_object", "probe_selection"],
        "super": ["probe_alignment_copy", "probe_alignment"],
        "symmetry_copy": ["probe_crystal", "probe_crystal_copy"],
        "view": ["probe_view", "store"],
        "volume": ["probe_volume", "probe_density"],
        "iterate_state": [1, "ala", "print(x)"],
        "uniquify": ["chain", "ala"],
    }
    args = special_arguments.get(
        name, [value_for(parameter, name, root) for parameter in required]
    )
    record["required_parameters"] = [parameter.name for parameter in required]
    record["fixture_arguments"] = [repr(value)[:160] for value in args]
    output = io.StringIO()
    try:
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
            prepare_fixture(cmd, root)
            result = function(*args)
        record.update({"status": "passed", "result": repr(result)[:300]})
    except Exception as error:
        record.update(
            {
                "status": "failed",
                "failure_class": failure_class(error),
                "error": compact_error(error),
            }
        )
    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        import pymol
        from pymol import cmd
    except Exception as error:
        result = {"status": "unavailable", "error": compact_error(error)}
    else:
        pymol.finish_launching(["pymol", "-cq"])
        with tempfile.TemporaryDirectory(prefix="molgfx-pymol-command-audit-") as directory:
            root = Path(directory)
            records = [
                attempt_command(cmd, name, root)
                for name in sorted(cmd.keyword)
                if not name.startswith("_")
            ]
            try:
                version = cmd.get_version()
            except Exception as error:
                version = compact_error(error)
        cmd.quit()
        result = {
            "schema": 1,
            "status": "passed",
            "version": version,
            "command_count": len(records),
            "attempted_count": sum(record["status"] in {"passed", "failed"} for record in records),
            "status_counts": dict(sorted(Counter(record["status"] for record in records).items())),
            "failure_class_counts": dict(
                sorted(Counter(record.get("failure_class", "") for record in records if record["status"] == "failed").items())
            ),
            "family_counts": dict(sorted(Counter(record["family"] for record in records).items())),
            "records": records,
        }
    encoded = json.dumps(result, indent=2, sort_keys=True, default=str) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        sys.stdout.write(encoded)
    return 0 if result["status"] in {"passed", "unavailable"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
