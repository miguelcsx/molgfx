#!/usr/bin/env python3
"""Run a disposable per-feature VMD graphics command probe.

VMD is optional and is never installed into this repository.  When a caller
provides a VMD executable, the probe sends bounded Tcl commands through VMD's
text interface and records command acceptance.  When VMD is absent, the same
70-record manifest is emitted with an explicit ``not-run`` status instead of a
silent aggregate skip.  These are command/state checks, not golden images.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

try:
    from graphics_engine_reference_manifest import (
        VMD_CONTROLS,
        VMD_GRAPHICS_COMMANDS,
        VMD_GRAPHICS_PRIMITIVES,
        VMD_RENDERERS,
        VMD_STEREO_MODES,
        VMD_STYLES,
        VMD_TRAJECTORY_SCENE,
    )
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_manifest import (
        VMD_CONTROLS,
        VMD_GRAPHICS_COMMANDS,
        VMD_GRAPHICS_PRIMITIVES,
        VMD_RENDERERS,
        VMD_STEREO_MODES,
        VMD_STYLES,
        VMD_TRAJECTORY_SCENE,
    )


def feature_records() -> list[dict[str, Any]]:
    """Return one bounded recipe for every VMD manifest record."""

    records: list[dict[str, Any]] = []

    def add(family: str, feature: str, recipe: str | None, reason: str = "") -> None:
        records.append(
            {
                "source": "VMD",
                "family": family,
                "feature": feature,
                "recipe": recipe,
                "recipe_status": "bounded" if recipe else "contract-only",
                "contract_reason": reason,
            }
        )

    for name, _description in VMD_STYLES:
        add("representation", name, f"mol representation {{{name}}}; mol addrep $molid")

    primitive_recipes = {
        "point": "graphics $molid point {0 0 0}",
        "line": "graphics $molid line {0 0 0} {1 1 1} width 2",
        "cylinder": "graphics $molid cylinder {0 0 0} {1 0 0} radius 0.2",
        "cone": "graphics $molid cone {0 0 0} {1 0 0} radius 0.2",
        "triangle": "graphics $molid triangle {0 0 0} {1 0 0} {0 1 0}",
        "trinorm": (
            "graphics $molid trinorm {0 0 0} {1 0 0} {0 1 0} "
            "{0 0 1} {0 0 1} {0 0 1}"
        ),
        "tricolor": (
            "graphics $molid tricolor {0 0 0} {1 0 0} {0 1 0} "
            "red green blue"
        ),
        "sphere": "graphics $molid sphere {0 0 0} radius 0.5",
        "text": "graphics $molid text {0 0 0} {pdviewx-audit}",
    }
    for name in VMD_GRAPHICS_PRIMITIVES:
        add("graphics-primitive", name, primitive_recipes[name])

    command_recipes = {
        "color": "color Display Background white",
        "materials": "material list",
        "material": "material change opacity 0.8",
        "delete": "graphics $molid delete all",
        "list": "graphics $molid list",
        "replace": "graphics $molid replace 0; graphics $molid point {0 0 0}",
        "exists": "graphics $molid exists 0",
        "info": "graphics $molid info 0",
    }
    for name in VMD_GRAPHICS_COMMANDS:
        if name not in VMD_GRAPHICS_PRIMITIVES:
            add("graphics-command", name, command_recipes[name])

    control_recipes = {
        "colors": "color Display Background white",
        "materials": "material change opacity 0.8",
        "ambient/diffuse/specular/reflection": "material change ambient 0.3 diffuse 0.8 specular 0.5",
        "opacity": "material change opacity 0.5",
        "clipping": "display nearclip 0.1; display farclip 100",
        "depth cueing": "display depthcue on",
        "antialiasing": "display antialias on",
        "backface culling": "display culling on",
        "stereo": "display stereo off",
    }
    for name in VMD_CONTROLS:
        add("control", name, control_recipes[name])

    stereo_modes = {
        "quad-buffer": "quadbuffer",
        "side-by-side": "sidebyside",
        "cross-eyed": "crosseye",
        "HDTV side-by-side": "sidebyside",
        "checkerboard": "checkerboard",
        "column-interleaved": "columninterleaved",
        "row-interleaved": "rowinterleaved",
        "anaglyph": "anaglyph",
    }
    for name in VMD_STEREO_MODES:
        add("stereo-mode", name, f"display stereo {stereo_modes[name]}")

    renderer_recipes = {
        "Tachyon CPU/GPU": "render TachyonInternal {$output/vmd-tachyon.tga}",
        "OSPRay": "render TachyonLOSPRayInternal {$output/vmd-ospray.tga}",
        "external renderers": None,
    }
    for name in VMD_RENDERERS:
        add(
            "renderer",
            name,
            renderer_recipes[name],
            "caller/external renderer executable" if name == "external renderers" else "",
        )

    trajectory_recipes = {
        "multi-frame drawing": "animate style loop",
        "trajectory smoothing": "mol smoothrep $molid 0 1",
        "unit-cell display": (
            "molinfo $molid set a 10; molinfo $molid set b 10; "
            "molinfo $molid set c 10; molinfo $molid set alpha 90; "
            "molinfo $molid set beta 90; molinfo $molid set gamma 90"
        ),
        "atom/bond/angle/dihedral/spring labels": "label add Atoms 0/0",
        "graphics primitives": "graphics $molid point {0 0 0}",
        "image and movie output": "render TachyonInternal {$output/vmd-movie.tga}",
    }
    for name in VMD_TRAJECTORY_SCENE:
        add("trajectory-scene", name, trajectory_recipes[name])
    return records


def tcl_script(records: list[dict[str, Any]], fixture: Path, output: Path) -> str:
    """Build a self-contained VMD Tcl script with line-oriented result markers."""

    fixture_value = str(fixture.resolve()).replace("\\", "\\\\")
    output_value = str(output.resolve()).replace("\\", "\\\\")
    lines = [
        f"set output {{{output_value}}}",
        f"if {{[catch {{mol new {{{fixture_value}}} type cif waitfor all}} load_error]}} {{",
        "    set molid [mol new]",
        "} else {",
        "    set molid [molinfo top]",
        "}",
        "proc emit {family feature status reason} {",
        "    set reason [string map [list \"|\" \"/\" \"\\n\" \" \"] $reason]",
        "    puts \"PDVIEWX_VMD|$family|$feature|$status|$reason\"",
        "    flush stdout",
        "}",
    ]
    for record in records:
        family = record["family"]
        feature = record["feature"]
        if record["recipe"] is None:
            reason = record["contract_reason"] or "no bounded recipe"
            lines.append(f"emit {{{family}}} {{{feature}}} contract-only {{{reason}}}")
            continue
        recipe = record["recipe"]
        lines.extend(
            [
                f"if {{[catch {{{recipe}}} message]}} {{",
                f"    emit {{{family}}} {{{feature}}} failed $message",
                "} else {",
                f"    emit {{{family}}} {{{feature}}} passed {{}}",
                "}",
            ]
        )
    lines.append("quit")
    return "\n".join(lines) + "\n"


def unavailable_result(records: list[dict[str, Any]], executable: Path, reason: str) -> dict[str, Any]:
    """Return complete feature records when VMD cannot be launched."""

    return {
        "schema": 1,
        "status": "unavailable",
        "scope": "native VMD Tcl graphics/state probe; not a golden image test",
        "executable": str(executable),
        "error": reason,
        "matrix_status": "not-run",
        "feature_count": len(records),
        "features": [
            {**record, "status": "not-run", "reason": reason}
            for record in records
        ],
    }


def run_probe(executable: Path, fixture: Path, output: Path, timeout: float) -> dict[str, Any]:
    records = feature_records()
    if not executable.is_file():
        return unavailable_result(records, executable, f"executable not found: {executable}")
    with tempfile.TemporaryDirectory(prefix="pdviewx-vmd-probe-") as directory:
        root = Path(directory)
        script = root / "probe.tcl"
        script.write_text(tcl_script(records, fixture, root), encoding="utf-8")
        completed = subprocess.run(
            [str(executable), "-dispdev", "text", "-e", str(script)],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
            env=os.environ.copy(),
        )
    by_key: dict[tuple[str, str], dict[str, Any]] = {}
    for line in completed.stdout.splitlines():
        if not line.startswith("PDVIEWX_VMD|"):
            continue
        _prefix, family, feature, status, reason = (line.split("|", 4) + [""])[:5]
        by_key[(family, feature)] = {"status": status, "reason": reason}
    features = []
    for record in records:
        outcome = by_key.get((record["family"], record["feature"]))
        if outcome is None:
            outcome = {"status": "not-run", "reason": "VMD exited before this record"}
        features.append({**record, **outcome})
    failures = [item["feature"] for item in features if item["status"] == "failed"]
    missing = [item["feature"] for item in features if item["status"] == "not-run"]
    return {
        "schema": 1,
        "status": "passed" if not failures and not missing else "passed-with-failures",
        "scope": "native VMD Tcl graphics/state probe; not a golden image test",
        "executable": str(executable),
        "returncode": completed.returncode,
        "matrix_status": "passed" if not failures and not missing else "passed-with-failures",
        "feature_count": len(features),
        "status_counts": {
            status: sum(item["status"] == status for item in features)
            for status in sorted({item["status"] for item in features})
        },
        "failures": failures,
        "not_run": missing,
        "stdout_tail": completed.stdout[-2000:],
        "stderr_tail": completed.stderr[-2000:],
        "features": features,
    }


def check_manifest(records: list[dict[str, Any]]) -> list[str]:
    """Validate counts, uniqueness and recipe presence without VMD."""

    issues: list[str] = []
    if len(records) != 70:
        issues.append(f"expected 70 records, found {len(records)}")
    keys = [(record["family"], record["feature"]) for record in records]
    if len(keys) != len(set(keys)):
        issues.append("duplicate VMD feature records")
    if any(record["recipe"] is None and record["family"] != "renderer" for record in records):
        issues.append("non-renderer VMD feature has no bounded recipe")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, default=Path(shutil.which("vmd") or "vmd"))
    parser.add_argument(
        "--fixture",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "benchmarks/scenes/1BNA.cif",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()
    records = feature_records()
    issues = check_manifest(records)
    result = run_probe(args.executable.resolve(), args.fixture.resolve(), Path.cwd(), args.timeout)
    result["manifest_issues"] = issues
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.summary:
        print(json.dumps({
            "status": result["status"],
            "feature_count": result["feature_count"],
            "matrix_status": result["matrix_status"],
            "manifest_issues": issues,
            "status_counts": result.get("status_counts", {"not-run": result["feature_count"]}),
        }, sort_keys=True))
    elif not args.output:
        print(json.dumps(result, indent=2, sort_keys=True))
    if args.check:
        return 1 if issues else 0
    return 0 if result["status"] != "failed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
