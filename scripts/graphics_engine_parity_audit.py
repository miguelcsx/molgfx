#!/usr/bin/env python3
"""Audit graphics-engine capabilities without installing reference engines here.

The PyMOL, pdbiox, OVITO and native ChimeraX probes are optional and run in
caller-selected disposable runtimes. The default process only inventories the
molgfx checkout and can delegate the optional probes to disposable Python
environments or application executables with the corresponding options.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Callable

from graphics_engine_taxonomy import (
    NON_RENDERING_FAMILIES,
    classify_command,
    command_execution_evidence,
    molgfx_mapping,
)
from graphics_engine_pymol_cgo_probe import cgo_opcode_matrix
from graphics_engine_pymol_probes import extended_pymol_probes
from graphics_engine_pymol_module_probe import module_graphics_api_surface
from graphics_engine_pymol_renderer_controls import renderer_control_probes
from graphics_engine_provider_probe import probe_pdbiox
from graphics_engine_ovito_probe import probe_ovito
from graphics_engine_chimerax_probe import probe_chimerax
from graphics_engine_facade_audit import audit as audit_facade


REFERENCE_URLS = {
    "pymol": "https://pymol.org/pymol-command-ref.html",
    "pymol_wiki": "https://pymol.org/dokuwiki/",
    "chimerax": "https://www.cgl.ucsf.edu/chimerax/docs/user/images.html",
    "vmd": "https://www.ks.uiuc.edu/Research/vmd/current/ug.pdf",
    "vmd_docs": "http://www.ks.uiuc.edu/Research/vmd/current/docs.html",
    "molstar": "https://molstar.org/viewer-docs/managing-the-display/",
    "ngl": "https://nglviewer.org/ngl/api/manual/molecular-representations.html",
    "ovito": "https://www.ovito.org/docs/current/python/",
    "yasara": "https://www.yasara.org/features.htm",
    "yasara_graphics": "https://www.yasara.org/mg.htm",
    "yasara_md": "http://www.yasara.org/md.htm",
    "protein_imaging": "https://3dproteinimaging.com/",
    "protein_imaging_overview": "https://3dproteinimaging.com/info/interface/overview.pdf",
}


def compact_error(error: BaseException) -> str:
    return f"{type(error).__name__}: {error}"[:500]


def json_value(value: Any) -> Any:
    """Convert NumPy/PyO3 scalar and array values into JSON-safe evidence."""

    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, dict):
        return {str(key): json_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [json_value(item) for item in value]
    item = getattr(value, "item", None)
    if callable(item):
        try:
            return json_value(item())
        except Exception:
            pass
    tolist = getattr(value, "tolist", None)
    if callable(tolist):
        try:
            return json_value(tolist())
        except Exception:
            pass
    return repr(value)


def run_probe(name: str, function: Callable[[], Any]) -> dict[str, Any]:
    """Run one optional operation and keep failures as evidence."""

    try:
        detail = json_value(function())
        if isinstance(detail, dict) and detail.get("status") in {
            "failed",
            "unavailable",
            "not-exposed",
        }:
            return {"name": name, **detail}
        return {"name": name, "status": "passed", "detail": detail}
    except Exception as error:  # probes must report unsupported operations
        return {"name": name, "status": "failed", "error": compact_error(error)}


def pymol_help_summary(command: str, cmd: Any) -> dict[str, str | bool]:
    """Capture the documented description and usage for one runtime command."""

    output = io.StringIO()
    try:
        with contextlib.redirect_stdout(output):
            cmd.help(command)
    except Exception as error:
        return {"available": False, "error": compact_error(error)}
    text = re.sub(r"\x1b\[[0-9;]*m", "", output.getvalue())
    lines = [line.strip() for line in text.splitlines()]

    def section(title: str, next_titles: tuple[str, ...]) -> str:
        try:
            start = next(index for index, line in enumerate(lines) if line == title) + 1
        except StopIteration:
            return ""
        values: list[str] = []
        for line in lines[start:]:
            if line in next_titles:
                break
            if line:
                values.append(line)
        return " ".join(values)[:500]

    description = section(
        "DESCRIPTION", ("USAGE", "ARGUMENTS", "EXAMPLES", "SEE ALSO", "PYMOL API")
    )
    usage = section("USAGE", ("ARGUMENTS", "EXAMPLES", "SEE ALSO", "PYMOL API"))
    return {"available": bool(description or usage), "description": description, "usage": usage}


def probe_pymol() -> dict[str, Any]:
    """Run a headless PyMOL smoke matrix and enumerate its public commands."""

    try:
        import pymol
        from pymol import cmd
    except Exception as error:
        return {
            "status": "unavailable",
            "error": compact_error(error),
            "reference": REFERENCE_URLS["pymol"],
        }

    with tempfile.TemporaryDirectory(prefix="molgfx-pymol-probe-") as directory:
        root = Path(directory)
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            pymol.finish_launching(["pymol", "-cq"])
            cmd.reinitialize()

            def create_fragment() -> str:
                cmd.fragment("ala", "ala")
                return f"{cmd.count_atoms('ala')} atoms"

            probes = [
                run_probe("create-and-count", create_fragment),
                run_probe(
                    "selection",
                    lambda: (
                        cmd.select("backbone_probe", "ala and backbone"),
                        cmd.count_atoms("backbone_probe"),
                    )[1],
                ),
                run_probe(
                    "representations",
                    lambda: (
                        [cmd.show(kind, "ala") for kind in ("sticks", "spheres", "cartoon")],
                        cmd.hide("everything", "ala"),
                        cmd.show("sticks", "ala"),
                        "sticks+spheres+cartoon",
                    )[-1],
                ),
                run_probe(
                    "color-and-material-settings",
                    lambda: (
                        cmd.set_color("probe_red", [1.0, 0.0, 0.0]),
                        cmd.color("probe_red", "ala"),
                        cmd.bg_color("white"),
                        cmd.set("ray_opaque_background", 0),
                        "custom color, background and transparency setting",
                    )[-1],
                ),
                run_probe(
                    "camera-and-clipping",
                    lambda: (
                        cmd.orient("ala"),
                        cmd.clip("near", 0.5),
                        len(cmd.get_view()),
                    )[-1],
                ),
                run_probe(
                    "measurement",
                    lambda: cmd.distance(
                        "probe_distance", "ala and name N", "ala and name CA", quiet=1
                    ),
                ),
                run_probe(
                    "secondary-structure-assignment",
                    lambda: (cmd.dss("ala", quiet=1), "dss" )[1],
                ),
                run_probe(
                    "editing-and-hydrogens",
                    lambda: (
                        cmd.alter("ala and name CA", "b=42.0"),
                        cmd.h_add("ala"),
                        cmd.create("ala_copy", "ala"),
                        "alter+h_add+create",
                    )[-1],
                ),
                run_probe(
                    "map-surface-mesh-volume",
                    lambda: (
                        cmd.map_new("probe_map", "gaussian", 1.0, "ala", 2.0),
                        cmd.isosurface("probe_iso", "probe_map", 0.2),
                        cmd.isomesh("probe_mesh", "probe_map", 0.2),
                        cmd.ramp_new(
                            "probe_ramp", "probe_map", [-1.0, 0.0, 1.0], ["red", "white", "blue"]
                        ),
                        cmd.volume("probe_volume", "probe_map"),
                        "map+isosurface+mesh+volume+ramp",
                    )[-1],
                ),
                run_probe(
                    "scene-and-session",
                    lambda: (
                        cmd.scene("probe_scene", "store"),
                        cmd.scene("probe_scene", "recall"),
                        cmd.save(str(root / "probe.pse"), "ala"),
                        (root / "probe.pse").stat().st_size,
                    )[-1],
                ),
                run_probe(
                    "movie-state-and-keyframes",
                    lambda: (
                        cmd.mset("1 x2"),
                        cmd.frame(1),
                        cmd.mview("store", 1),
                        cmd.frame(2),
                        cmd.mview("store", 2),
                        cmd.count_frames(),
                    )[-1],
                ),
                run_probe(
                    "ray-traced-png-and-coordinate-export",
                    lambda: (
                        cmd.orient("ala"),
                        cmd.png(str(root / "probe.png"), width=128, height=128, ray=1, quiet=1),
                        cmd.save(str(root / "probe.pdb"), "ala"),
                        {
                            "png_bytes": (root / "probe.png").stat().st_size,
                            "pdb_bytes": (root / "probe.pdb").stat().st_size,
                        },
                    )[-1],
                ),
            ]
            probes.extend(extended_pymol_probes(cmd, root, run_probe))
            probes.append(
                run_probe(
                    "cgo-opcode-matrix",
                    lambda: cgo_opcode_matrix(cmd, root),
                )
            )
            probes.append(
                run_probe(
                    "module-only-graphics-api-surface",
                    lambda: module_graphics_api_surface(cmd, root),
                )
            )
            probes.extend(renderer_control_probes(cmd, run_probe))
            command_names = sorted(name for name in cmd.keyword if not name.startswith("_"))

            def command_record(name: str) -> dict[str, Any]:
                family = classify_command(name)
                return {
                    "name": name,
                    "family": family,
                    "execution_evidence": command_execution_evidence(name),
                    "molgfx": molgfx_mapping(name, family),
                    "scope": "non-rendering-owner"
                    if family in NON_RENDERING_FAMILIES
                    else "graphics-or-scene-owner",
                    "keyword": json_value(cmd.keyword[name]),
                    "help": pymol_help_summary(name, cmd),
                }

            command_inventory = [command_record(name) for name in command_names]
            try:
                version = cmd.get_version()
            except Exception as error:
                version = compact_error(error)
            cmd.quit()

    family_counts: dict[str, int] = {}
    mapping_counts: dict[str, int] = {}
    execution_counts: dict[str, int] = {}
    for command in command_inventory:
        family = command["family"]
        family_counts[family] = family_counts.get(family, 0) + 1
        status = command["molgfx"]["status"]
        mapping_counts[status] = mapping_counts.get(status, 0) + 1
        evidence = command["execution_evidence"]
        execution_counts[evidence] = execution_counts.get(evidence, 0) + 1
    failed_probes = [probe["name"] for probe in probes if probe["status"] != "passed"]
    return {
        "status": "passed" if not failed_probes else "passed-with-failures",
        "reference": REFERENCE_URLS["pymol"],
        "version": version,
        "command_count": len(command_inventory),
        "probe_count": len(probes),
        "passed_probe_count": len(probes) - len(failed_probes),
        "failed_probes": failed_probes,
        "command_family_counts": dict(sorted(family_counts.items())),
        "command_execution_evidence_counts": dict(sorted(execution_counts.items())),
        "molgfx_mapping_counts": dict(sorted(mapping_counts.items())),
        "unclassified_commands": [
            command["name"] for command in command_inventory if command["family"] == "review-needed"
        ],
        "non_rendering_commands": [
            command["name"]
            for command in command_inventory
            if command["family"] in NON_RENDERING_FAMILIES
        ],
        "command_inventory": command_inventory,
        "probes": probes,
    }


def relative_path(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def probe_molgfx(root: Path) -> dict[str, Any]:
    """Inventory the Rust facade and its existing test evidence."""

    facade = root / "crates/molgfx/src/lib.rs"
    source = facade.read_text(encoding="utf-8") if facade.is_file() else ""
    rust_files = list((root / "crates").rglob("*.rs"))
    test_files = [path for path in rust_files if path.name.endswith("_tests.rs")]
    core_source = root / "crates/molgfx-core/src"
    core_text = "\n".join(path.read_text(encoding="utf-8") for path in core_source.rglob("*.rs"))
    signals = {
        "scene-and-pdbiox-coordinate-seam": "Scene" in source and "pdbiox" in core_text,
        "analytic-representations": "RepresentationKind" in source,
        "surfaces-and-volumes": "SurfaceKind" in source and "VolumeRendering" in source,
        "camera-and-clipping": "Camera" in source and "ClipSet" in source,
        "picking-and-annotations": "PickEntity" in source and "Annotation" in source,
        "headless-image-engine": "Engine" in source and "Image" in source,
        "python-binding": (root / "crates/molgfx-py").exists()
        or (root / "python/molgfx").exists(),
    }
    test_text = "\n".join(path.read_text(encoding="utf-8") for path in test_files)
    return {
        "status": "passed" if facade.is_file() else "failed",
        "scope": "Rust facade and source/test evidence; no molgfx Python binding exists",
        "facade": relative_path(facade, root),
        "signals": signals,
        "rust_source_file_count": len(rust_files),
        "sibling_test_file_count": len(test_files),
        "named_test_count": len(re.findall(r"^fn [a-z0-9_]+\(", test_text, flags=re.MULTILINE)),
        "evidence": [relative_path(path, root) for path in test_files[:20]],
    }


def run_child(probe: str, python: str, root: Path) -> dict[str, Any]:
    """Run one probe in a caller-selected interpreter."""

    with tempfile.TemporaryDirectory(prefix="molgfx-parity-child-") as directory:
        output = Path(directory) / "result.json"
        command = [
            python,
            str(Path(__file__).resolve()),
            "--probe",
            probe,
            "--repo",
            str(root),
            "--output",
            str(output),
        ]
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        if completed.returncode != 0:
            return {
                "status": "unavailable",
                "error": f"child exit {completed.returncode}: {completed.stderr[-500:]}",
            }
        try:
            return json.loads(output.read_text(encoding="utf-8"))
        except (FileNotFoundError, json.JSONDecodeError) as error:
            return {
                "status": "failed",
                "error": f"child did not write JSON: {error}; stderr={completed.stderr[-500:]}",
            }


def run_chimerax_disposable(executable: Path, root: Path) -> dict[str, Any]:
    """Run native ChimeraX with all child artifacts outside the checkout."""

    with tempfile.TemporaryDirectory(prefix="molgfx-parity-chimerax-") as directory:
        output = Path(directory) / "result.json"
        return probe_chimerax(
            executable,
            root / "benchmarks/scenes/1BNA.cif",
            output,
        )


def unavailable_chimerax(root: Path) -> dict[str, Any]:
    """Return the full per-feature not-run matrix when no bundle is supplied."""

    return run_chimerax_disposable(
        root / "target/molgfx-chimerax-not-installed/ChimeraX", root
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--probe",
        choices=("all", "facade", "pymol", "pdbiox", "ovito", "chimerax", "molgfx"),
        default="all",
    )
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--pymol-python", help="disposable interpreter containing pymol")
    parser.add_argument("--pdbiox-python", help="disposable interpreter containing a fresh pdbiox wheel")
    parser.add_argument("--ovito-python", help="disposable interpreter containing OVITO")
    parser.add_argument("--chimerax-executable", type=Path, help="disposable native ChimeraX executable or .app")
    parser.add_argument("--output", type=Path, help="write JSON results to this path")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.repo.resolve()
    if args.probe == "pymol":
        result: dict[str, Any] = probe_pymol()
    elif args.probe == "facade":
        result = audit_facade(root)
    elif args.probe == "pdbiox":
        result = probe_pdbiox(run_probe, compact_error)
    elif args.probe == "ovito":
        result = probe_ovito(root, run_probe, compact_error)
    elif args.probe == "chimerax":
        result = (
            run_chimerax_disposable(args.chimerax_executable, root)
            if args.chimerax_executable
            else unavailable_chimerax(root)
        )
    elif args.probe == "molgfx":
        result = probe_molgfx(root)
    else:
        result = {
            "schema": 1,
            "reference_urls": REFERENCE_URLS,
            "facade": audit_facade(root),
            "molgfx": probe_molgfx(root),
            "pymol": run_child("pymol", args.pymol_python, root)
            if args.pymol_python
            else probe_pymol(),
            "pdbiox_provider": run_child("pdbiox", args.pdbiox_python, root)
            if args.pdbiox_python
            else probe_pdbiox(run_probe, compact_error),
            "ovito": run_child("ovito", args.ovito_python, root)
            if args.ovito_python
            else probe_ovito(root, run_probe, compact_error),
            "chimerax": run_chimerax_disposable(args.chimerax_executable, root)
            if args.chimerax_executable
            else unavailable_chimerax(root),
        }
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        sys.stdout.write(encoded)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
