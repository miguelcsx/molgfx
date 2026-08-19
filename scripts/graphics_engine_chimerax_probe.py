#!/usr/bin/env python3
"""Run a disposable native UCSF ChimeraX command/render probe.

The wrapper launches ChimeraX's own Python environment and keeps the child
script, synthetic MRC, session and JSON evidence in a temporary directory. A
headless macOS ChimeraX process can execute the scene commands but cannot save
an image when OpenGL is unavailable; that result is recorded as an expected
runtime limitation rather than a renderer pass.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import struct
import subprocess
import tempfile
from pathlib import Path
from typing import Any

try:
    from graphics_engine_reference_manifest import (
        CHIMERAX_IMAGE_COMMANDS,
        CHIMERAX_SURFACE_OPTIONS,
        CHIMERAX_VOLUME_OPERATIONS,
    )
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_manifest import (
        CHIMERAX_IMAGE_COMMANDS,
        CHIMERAX_SURFACE_OPTIONS,
        CHIMERAX_VOLUME_OPERATIONS,
    )


CHIMERAX_CHILD = r'''import json
import os
from pathlib import Path

from chimerax.core.commands import run


output = Path(os.environ["PDVIEWX_CHIMERAX_OUTPUT"])
structure = Path(os.environ["PDVIEWX_CHIMERAX_STRUCTURE"])
map_path = Path(os.environ["PDVIEWX_CHIMERAX_MAP"])
session_path = output.with_suffix(".cxs")
image_path = output.with_suffix(".png")
volume_operation_names = json.loads(
    os.environ["PDVIEWX_CHIMERAX_VOLUME_OPERATIONS"]
)
surface_option_names = json.loads(os.environ["PDVIEWX_CHIMERAX_SURFACE_OPTIONS"])
image_command_names = json.loads(os.environ["PDVIEWX_CHIMERAX_IMAGE_COMMANDS"])


def model_summary():
    return [
        {"id": str(model.id), "name": model.name, "type": type(model).__name__}
        for model in session.models.list()
    ]


def execute(name, command, expected_unavailable=False):
    try:
        run(session, command)
        return {
            "name": name,
            "status": "passed",
            "models": model_summary(),
        }
    except Exception as error:
        message = f"{type(error).__name__}: {error}"[:500]
        return {
            "name": name,
            "status": "unavailable" if expected_unavailable else "failed",
            "error": message,
            "models": model_summary(),
        }


results = []
results.append(execute("open-structure", f"open {structure}"))
results.append(execute("cartoon-and-ribbon", "cartoon; ribbon"))
results.append(execute("nucleotides", "nucleotides atoms"))
results.append(execute("color-and-rainbow", "color bychain; rainbow"))
results.append(execute("material", "material shiny"))
results.append(execute("surface-and-transparency", "surface; transparency 20"))
results.append(execute("hbonds", "hbonds reveal true"))
results.append(execute("distance", "distance #1/A:1@O5' #1/A:1@C5'"))
results.append(execute("lighting", "lighting soft"))
results.append(execute("clip", "clip near 2; clip far 200"))
results.append(execute("camera-view-and-style", "view; zoom; style stick; show #1"))
results.append(execute("volume-open", f"open {map_path}"))

volume = next(
    (model for model in session.models.list() if type(model).__name__ == "Volume"),
    None,
)
if volume is None:
    results.append({
        "name": "volume-surface-and-level",
        "status": "failed",
        "error": "ChimeraX did not create a Volume model",
        "models": model_summary(),
    })
else:
    volume_id = f"#{volume.id[0]}"
    results.append(
        execute(
            "volume-surface-and-level",
            f"volume {volume_id} style surface; volume {volume_id} level 0.35",
        )
    )
    results.append(
        execute(
            "volume-color-and-region",
            f"volume {volume_id} color blue; volume {volume_id} region all",
        )
    )

results.append(execute("scene-save", f"save {session_path}"))
results.append(execute("movie-state", "movie record; movie stop; movie status"))
results.append(execute("screen-space-label", "2dlabels create audit text 'ChimeraX audit'"))
results.append(
    execute(
        "headless-image",
        f"save {image_path} width 128 height 128 supersample 1",
        expected_unavailable=True,
    )
)


def model_ref(model):
    model_id = model.id if isinstance(model.id, tuple) else (model.id,)
    return "#" + ".".join(str(part) for part in model_id)


def volume_models():
    return [model for model in session.models.list() if type(model).__name__ == "Volume"]


def surface_models():
    return [
        model
        for model in session.models.list()
        if "Surface" in type(model).__name__
    ]


def atomic_model():
    return next(
        (
            model
            for model in session.models.list()
            if type(model).__name__ == "AtomicStructure"
        ),
        None,
    )


def volume_context(map_count=1, with_structure=False, with_surface=False):
    run(session, "close all")
    if with_structure:
        run(session, f"open {structure}")
    for _ in range(map_count):
        run(session, f"open {map_path}")
    maps = volume_models()
    if len(maps) < map_count:
        raise RuntimeError(
            f"expected {map_count} volume models, found {len(maps)}"
        )
    context = {}
    if map_count > 0:
        context["a"] = model_ref(maps[0])
    if map_count > 1:
        context["b"] = model_ref(maps[1])
    if with_structure:
        model = atomic_model()
        if model is None:
            raise RuntimeError("structure fixture did not create AtomicStructure")
        context["atom"] = model_ref(model)
    if with_surface:
        run(session, f"surface {context['atom']} replace false")
        surfaces = surface_models()
        if not surfaces:
            raise RuntimeError("surface fixture did not create a surface model")
        context["surface"] = model_ref(surfaces[-1])
    return context


def surface_context(surface_count=1):
    run(session, "close all")
    run(session, f"open {structure}")
    model = atomic_model()
    if model is None:
        raise RuntimeError("surface fixture did not create AtomicStructure")
    atom = model_ref(model)
    for _ in range(surface_count):
        run(session, f"surface {atom} replace false")
    surfaces = surface_models()
    if len(surfaces) < surface_count:
        raise RuntimeError(
            f"expected {surface_count} surface models, found {len(surfaces)}"
        )
    return {
        "atom": atom,
        "surface": model_ref(surfaces[0]),
        "surface2": model_ref(surfaces[1]) if len(surfaces) > 1 else None,
    }


def matrix_record(name, family, setup, command):
    try:
        context = setup()
        rendered = command(context)
        run(session, rendered)
        return {
            "name": name,
            "family": family,
            "status": "passed",
            "command": rendered,
            "models": model_summary(),
        }
    except Exception as error:
        message = f"{type(error).__name__}: {error}"[:500]
        return {
            "name": name,
            "family": family,
            "status": "failed",
            "command": rendered if "rendered" in locals() else None,
            "error": message,
            "models": model_summary(),
        }


def matrix_summary(records):
    statuses = [record["status"] for record in records]
    return {
        "manifest_count": len(records),
        "passed": statuses.count("passed"),
        "failed": statuses.count("failed"),
        "external_input": statuses.count("external-input"),
        "contract_only": statuses.count("contract-only"),
        "not_run": statuses.count("not-run"),
        "records": records,
    }


def image_structure_context():
    run(session, "close all")
    run(session, f"open {structure}")
    model = atomic_model()
    if model is None:
        raise RuntimeError("image-command fixture did not create AtomicStructure")
    return {"atom": model_ref(model)}


def image_dual_structure_context():
    run(session, "close all")
    run(session, f"open {structure}")
    run(session, f"open {structure}")
    models = [
        model for model in session.models.list()
        if type(model).__name__ == "AtomicStructure"
    ]
    if len(models) < 2:
        raise RuntimeError("image-command fixture did not create two AtomicStructures")
    return {"a": model_ref(models[0]), "b": model_ref(models[1])}


def image_volume_context():
    context = volume_context(1)
    return {"map": context["a"]}


IMAGE_COMMAND_SPECS = {
    "2dlabels": ("structure", "2dlabels create audit text 'ChimeraX audit'"),
    "align": ("dual", "align {a} to {b}"),
    "camera": ("structure", "camera ortho"),
    "cartoon": ("structure", "cartoon"),
    "ribbon": ("structure", "ribbon"),
    "clip": ("structure", "clip near 2; clip far 200"),
    "color": ("structure", "color bychain"),
    "rainbow": ("structure", "rainbow chain"),
    "coulombic": ("structure", "coulombic {atom}"),
    "distance": ("structure", "distance {atom}/A:1@O5' {atom}/A:1@C5'"),
    "graphics": ("structure", "graphics silhouettes true"),
    "hbonds": ("structure", "hbonds reveal true"),
    "key": ("structure", "key red,blue A,B"),
    "label": ("structure", "label {atom} text 'audit'"),
    "lighting": ("structure", "lighting soft"),
    "matchmaker": ("dual", "matchmaker {a} to {b}"),
    "mmaker": ("dual", "mmaker {a} to {b}"),
    "material": ("structure", "material shiny"),
    "mlp": ("structure", "mlp {atom}"),
    "move": ("structure", "move x 1"),
    "nucleotides": ("structure", "nucleotides atoms"),
    "preset": ("structure", "preset publication 1"),
    "save": ("structure", f"save {session_path}"),
    "scalebar": ("structure", "scalebar show true"),
    "scenes": ("structure", "scenes save audit; scenes restore audit"),
    "set": ("structure", "set bgColor white"),
    "show": ("structure", "show {atom}"),
    "hide": ("structure", "hide {atom}"),
    "size": ("structure", "size atomRadius 1.5"),
    "style": ("structure", "style stick"),
    "surface": ("structure", "surface {atom}"),
    "transparency": ("structure", "transparency 20"),
    "view": ("structure", "view"),
    "volume": ("volume", "volume {map} style surface"),
    "windowsize": ("structure", "windowsize 128 128"),
    "zoom": ("structure", "zoom"),
}


IMAGE_CONTEXTS = {
    "structure": image_structure_context,
    "dual": image_dual_structure_context,
    "volume": image_volume_context,
}


image_command_records = []
for name in image_command_names:
    specification = IMAGE_COMMAND_SPECS.get(name)
    if specification is None:
        image_command_records.append(
            {
                "name": name,
                "family": "image-command",
                "status": "contract-only",
                "reason": "manifest item has no bounded native command recipe",
            }
        )
        continue
    context_name, template = specification
    image_command_records.append(
        matrix_record(
            name,
            "image-command",
            IMAGE_CONTEXTS[context_name],
            lambda context, template=template: template.format(**context),
        )
    )


volume_recipes = {
    "add": (2, False, False, lambda context: f"volume add {context['a']},{context['b']}"),
    "bin": (1, False, False, lambda context: f"volume bin {context['a']} binSize 2"),
    "boxes": (1, True, False, lambda context: f"volume boxes {context['a']} centers {context['atom']} isize 4"),
    "copy": (1, False, False, lambda context: f"volume copy {context['a']} subregion all"),
    "cover": (1, True, False, lambda context: f"volume cover {context['a']} atomBox {context['atom']} pad 2"),
    "erase": (1, False, False, lambda context: f"volume erase {context['a']} center 0,0,0 radius 2"),
    "falloff": (1, False, False, lambda context: f"volume falloff {context['a']} iterations 1"),
    "flatten": (1, False, False, lambda context: f"volume flatten {context['a']} method multiply"),
    "flip": (1, False, False, lambda context: f"volume flip {context['a']} axis z"),
    "fourier": (1, False, False, lambda context: f"volume fourier {context['a']} phase false"),
    "gaussian": (1, False, False, lambda context: f"volume gaussian {context['a']} sDev 1"),
    "laplacian": (1, False, False, lambda context: f"volume laplacian {context['a']}"),
    "localCorrelation": (2, False, False, lambda context: f"volume localCorrelation {context['a']} {context['b']} windowSize 3"),
    "mask": (1, True, True, lambda context: f"volume mask {context['a']} surfaces {context['surface']}"),
    "maximum": (2, False, False, lambda context: f"volume maximum {context['a']},{context['b']}"),
    "median": (1, False, False, lambda context: f"volume median {context['a']} binSize 3 iterations 1"),
    "minimum": (2, False, False, lambda context: f"volume minimum {context['a']},{context['b']}"),
    "morph": (2, False, False, lambda context: f"volume morph {context['a']},{context['b']} frames 3 slider false"),
    "multiply": (2, False, False, lambda context: f"volume multiply {context['a']},{context['b']}"),
    "new": (0, False, False, lambda context: "volume new audit-map size 8,8,8 gridSpacing 1"),
    "onesmask": (1, True, True, lambda context: f"volume onesmask {context['surface']} onGrid {context['a']}"),
    "octant": (1, False, False, lambda context: f"volume octant {context['a']} iCenter 8,8,8"),
    "permuteAxes": (1, False, False, lambda context: f"volume permuteAxes {context['a']} yxz"),
    "resample": (1, False, False, lambda context: f"volume resample {context['a']} spacing 0.5"),
    "ridges": (1, False, False, lambda context: f"volume ridges {context['a']} level 0.2"),
    "scale": (1, False, False, lambda context: f"volume scale {context['a']} shift 0.1 factor 1.2"),
    "sharpen": (1, False, False, lambda context: f"volume sharpen {context['a']} bfactor 1"),
    "splitbyzone": (1, True, False, lambda context: f"color zone {context['a']} near {context['atom']} distance 4; volume splitbyzone {context['a']}"),
    "subtract": (2, False, False, lambda context: f"volume subtract {context['a']} {context['b']}"),
    "threshold": (1, False, False, lambda context: f"volume threshold {context['a']} minimum 0.2 maximum 0.8"),
    "tile": (1, False, False, lambda context: f"volume tile {context['a']} axis z columns 4 rows 4"),
    "unbend": (1, True, False, lambda context: f"volume unbend {context['a']} path {context['atom']} yaxis z xsize 10 ysize 10"),
    "unroll": (1, False, False, lambda context: f"volume unroll {context['a']} center 0,0,0 axis z length 10 innerRadius 1 outerRadius 6 gridSpacing 1"),
    "unzone": (1, True, False, lambda context: f"volume zone {context['a']} nearAtoms {context['atom']} range 4; volume unzone {context['a']}"),
    "zone": (1, True, False, lambda context: f"volume zone {context['a']} nearAtoms {context['atom']} range 4 newMap true"),
}

volume_operation_records = []
for name in volume_operation_names:
    recipe = volume_recipes.get(name)
    if recipe is None:
        volume_operation_records.append(
            {
                "name": name,
                "family": "volume-operation",
                "status": "contract-only",
                "reason": "manifest item has no bounded command recipe",
            }
        )
        continue
    map_count, with_structure, with_surface, command = recipe
    volume_operation_records.append(
        matrix_record(
            name,
            "volume-operation",
            lambda map_count=map_count, with_structure=with_structure, with_surface=with_surface: volume_context(
                map_count, with_structure, with_surface
            ),
            command,
        )
    )


surface_recipes = {
    "color": (1, lambda context: f"surface {context['atom']} color blue"),
    "transparency": (1, lambda context: f"surface {context['atom']} transparency 20"),
    "enclose": (1, lambda context: f"surface {context['atom']} enclose {context['atom']}"),
    "include": (1, lambda context: f"surface {context['atom']} include {context['atom']}"),
    "replace": (1, lambda context: f"surface {context['atom']} replace false"),
    "probeRadius": (1, lambda context: f"surface {context['atom']} probeRadius 1.4"),
    "resolution": (1, lambda context: f"surface {context['atom']} resolution 3"),
    "level": (1, lambda context: f"surface {context['atom']} resolution 3 level 1"),
    "gridSpacing": (1, lambda context: f"surface {context['atom']} gridSpacing 0.5"),
    "update": (1, lambda context: f"surface {context['atom']} update true"),
    "sharpBoundaries": (1, lambda context: f"surface {context['atom']} sharpBoundaries true"),
    "visiblePatches": (1, lambda context: f"surface {context['atom']} visiblePatches 1"),
    "cap": (1, lambda context: f"surface cap {context['surface']} false"),
    "dust": (1, lambda context: f"surface dust {context['surface']} size 1"),
    "hidefarblobs": (2, lambda context: f"surface hidefarblobs {context['surface']} nearSurface {context['surface2']} distance 5"),
    "invertShown": (1, lambda context: f"surface invertShown {context['surface']}"),
    "showall": (1, lambda context: f"surface showall {context['surface']}"),
    "splitbycolor": (1, lambda context: f"surface splitbycolor {context['surface']}"),
    "smooth": (1, lambda context: f"surface smooth {context['surface']} factor 0.1 iterations 1 inPlace true"),
    "squaremesh": (1, lambda context: f"surface squaremesh {context['surface']}"),
    "transform": (1, lambda context: f"surface transform {context['surface']} scale 1.01 move 0,0,0"),
    "zone": (1, lambda context: f"surface zone {context['surface']} nearAtoms {context['atom']} distance 4"),
    "style": (1, lambda context: f"surface style {context['surface']} solid"),
    "solid": (1, lambda context: f"surface style {context['surface']} solid"),
    "mesh": (1, lambda context: f"surface style {context['surface']} mesh"),
    "dot": (1, lambda context: f"surface style {context['surface']} dot"),
}

surface_option_records = []
for name in surface_option_names:
    recipe = surface_recipes.get(name)
    if recipe is None:
        surface_option_records.append(
            {
                "name": name,
                "family": "surface-option",
                "status": "contract-only",
                "reason": "manifest item has no bounded command recipe",
            }
        )
        continue
    surface_count, command = recipe
    surface_option_records.append(
        matrix_record(
            name,
            "surface-option",
            lambda surface_count=surface_count: surface_context(surface_count),
            command,
        )
    )

volume_matrix = matrix_summary(volume_operation_records)
surface_matrix = matrix_summary(surface_option_records)
image_matrix = matrix_summary(image_command_records)
matrix_failures = [
    f"volume:{record['name']}"
    for record in volume_operation_records
    if record["status"] == "failed"
] + [
    f"surface:{record['name']}"
    for record in surface_option_records
    if record["status"] == "failed"
] + [
    f"image:{record['name']}"
    for record in image_command_records
    if record["status"] == "failed"
]
hard_failures = [item["name"] for item in results if item["status"] == "failed"]
hard_failures.extend(matrix_failures)
unavailable = [item["name"] for item in results if item["status"] == "unavailable"]
output.write_text(
    json.dumps(
        {
            "status": "passed" if not hard_failures else "passed-with-failures",
            "scope": "native ChimeraX command and scene-state probe",
            "gui": session.ui.is_gui,
            "matrix_status": "passed" if not matrix_failures else "passed-with-failures",
            "hard_failures": hard_failures,
            "unavailable": unavailable,
            "results": results,
            "chimerax_volume_operation_matrix": volume_matrix,
            "chimerax_surface_option_matrix": surface_matrix,
            "chimerax_image_command_matrix": image_matrix,
            "final_models": model_summary(),
        },
        indent=2,
    ),
    encoding="utf-8",
)
'''


def write_mrc(path: Path, size: int = 16) -> None:
    """Write a small valid little-endian MRC MODE 2 density fixture."""

    center = (size - 1) * 0.5
    values = []
    for z in range(size):
        for y in range(size):
            for x in range(size):
                distance = sum(
                    ((coordinate - center) / (size * 0.2)) ** 2
                    for coordinate in (x, y, z)
                )
                values.append(2.0 ** (-distance))
    minimum, maximum = min(values), max(values)
    mean = sum(values) / len(values)
    variance = sum((value - mean) ** 2 for value in values) / len(values)
    header = bytearray(1024)
    struct.pack_into("<3i", header, 0, size, size, size)
    struct.pack_into("<i", header, 12, 2)
    struct.pack_into("<3i", header, 28, size, size, size)
    struct.pack_into("<3f", header, 40, float(size), float(size), float(size))
    struct.pack_into("<3f", header, 52, 90.0, 90.0, 90.0)
    struct.pack_into("<3i", header, 64, 1, 2, 3)
    struct.pack_into("<3f", header, 76, minimum, maximum, mean)
    header[208:212] = b"MAP "
    struct.pack_into("<I", header, 212, 0x44410000)
    struct.pack_into("<f", header, 216, variance**0.5)
    path.write_bytes(header + struct.pack(f"<{len(values)}f", *values))


def executable_path(value: Path) -> Path:
    """Accept a ChimeraX executable or an application bundle."""

    if value.is_dir() and value.suffix == ".app":
        return value / "Contents" / "MacOS" / "ChimeraX"
    return value


def version(executable: Path, timeout: float) -> str:
    """Read the native version without starting the command probe."""

    try:
        completed = subprocess.run(
            [str(executable), "--version"],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return f"unavailable: {type(error).__name__}: {error}"[:300]
    text = f"{completed.stdout}\n{completed.stderr}"
    match = re.search(r"UCSF ChimeraX version:\s*([^\n]+)", text)
    return match.group(1).strip() if match else text.strip()[:300]


def not_run_matrix(names: list[str], family: str, reason: str) -> list[dict[str, str]]:
    """Keep every unavailable matrix feature visible with an explicit outcome."""

    return [
        {"name": name, "family": family, "status": "not-run", "reason": reason}
        for name in names
    ]


def not_run_matrices(reason: str) -> dict[str, list[dict[str, str]]]:
    """Build the per-feature matrix payload for an unavailable executable."""

    return {
        "chimerax_volume_operation_matrix": not_run_matrix(
            CHIMERAX_VOLUME_OPERATIONS, "volume-operation", reason
        ),
        "chimerax_surface_option_matrix": not_run_matrix(
            CHIMERAX_SURFACE_OPTIONS, "surface-option", reason
        ),
        "chimerax_image_command_matrix": not_run_matrix(
            CHIMERAX_IMAGE_COMMANDS, "image-command", reason
        ),
    }


def probe_chimerax(
    executable: Path,
    structure: Path,
    output: Path,
    timeout: float = 180.0,
) -> dict[str, Any]:
    """Run the native probe in a disposable child-script directory."""

    matrix_manifest = {
        "volume_operations": len(CHIMERAX_VOLUME_OPERATIONS),
        "surface_options": len(CHIMERAX_SURFACE_OPTIONS),
        "image_commands": len(CHIMERAX_IMAGE_COMMANDS),
    }
    executable = executable_path(executable.resolve())
    output.parent.mkdir(parents=True, exist_ok=True)
    if not executable.is_file():
        return {
            "status": "unavailable",
            "scope": "native ChimeraX command and scene-state probe",
            "error": f"executable not found: {executable}",
            "matrix_status": "not-run",
            "matrix_manifest": matrix_manifest,
            **not_run_matrices(f"executable not found: {executable}"),
        }
    if not structure.is_file():
        return {
            "status": "failed",
            "scope": "native ChimeraX command and scene-state probe",
            "error": f"structure fixture not found: {structure}",
            "matrix_status": "not-run",
            "matrix_manifest": matrix_manifest,
            **not_run_matrices(f"structure fixture not found: {structure}"),
        }

    with tempfile.TemporaryDirectory(prefix="pdviewx-chimerax-probe-") as directory:
        root = Path(directory)
        child_script = root / "probe.py"
        child_output = root / "result.json"
        map_path = root / "probe.mrc"
        child_script.write_text(CHIMERAX_CHILD, encoding="utf-8")
        write_mrc(map_path)
        environment = os.environ.copy()
        environment.update(
            {
                "PDVIEWX_CHIMERAX_OUTPUT": str(child_output),
                "PDVIEWX_CHIMERAX_STRUCTURE": str(structure.resolve()),
                "PDVIEWX_CHIMERAX_MAP": str(map_path),
                "PDVIEWX_CHIMERAX_VOLUME_OPERATIONS": json.dumps(
                    CHIMERAX_VOLUME_OPERATIONS
                ),
                "PDVIEWX_CHIMERAX_SURFACE_OPTIONS": json.dumps(
                    CHIMERAX_SURFACE_OPTIONS
                ),
                "PDVIEWX_CHIMERAX_IMAGE_COMMANDS": json.dumps(
                    CHIMERAX_IMAGE_COMMANDS
                ),
            }
        )
        command = [
            str(executable),
            "--nogui",
            "--silent",
            "--exit",
            "--script",
            str(child_script),
        ]
        try:
            completed = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
                env=environment,
            )
        except subprocess.TimeoutExpired as error:
            return {
                "status": "unavailable",
                "scope": "native ChimeraX command and scene-state probe",
                "version": version(executable, min(timeout, 30.0)),
                "error": f"timeout after {timeout}s: {error}",
                "matrix_status": "not-run",
                "matrix_manifest": matrix_manifest,
                **not_run_matrices(f"timeout after {timeout}s"),
            }
        except OSError as error:
            return {
                "status": "unavailable",
                "scope": "native ChimeraX command and scene-state probe",
                "version": version(executable, min(timeout, 30.0)),
                "error": f"launch failed: {type(error).__name__}: {error}",
                "matrix_status": "not-run",
                "matrix_manifest": matrix_manifest,
                **not_run_matrices(f"launch failed: {type(error).__name__}"),
            }

        if child_output.is_file():
            result = json.loads(child_output.read_text(encoding="utf-8"))
        else:
            result = {
                "status": "failed",
                "scope": "native ChimeraX command and scene-state probe",
                "hard_failures": ["child-output"],
                "unavailable": [],
                "results": [],
                "error": "ChimeraX did not write child JSON",
                "matrix_status": "not-run",
                "matrix_manifest": matrix_manifest,
                **not_run_matrices("ChimeraX did not write child JSON"),
            }
        result.update(
            {
                "version": version(executable, min(timeout, 30.0)),
                "executable": str(executable),
                "returncode": completed.returncode,
                "stdout_tail": completed.stdout[-1000:],
                "stderr_tail": completed.stderr[-1000:],
                "reference": "https://www.cgl.ucsf.edu/chimerax/docs/index.html",
            }
        )
        output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument(
        "--structure",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "benchmarks/scenes/1BNA.cif",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()
    result = probe_chimerax(args.executable, args.structure, args.output, args.timeout)
    if not args.output.is_file():
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    return 0 if result.get("status") in {"passed", "passed-with-unavailable"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
