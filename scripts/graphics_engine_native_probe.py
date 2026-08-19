#!/usr/bin/env python3
"""Run the existing headless pdviewx visual examples in a disposable matrix.

The examples are the native-side counterpart to the optional PyMOL and OVITO
probes.  They are not golden-image tests: each case records process output,
PNG dimensions, and a conservative image signal so a blank render is visible
in the audit instead of being counted as success merely because a file exists.
All generated images stay in a temporary directory.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from graphics_engine_cross_render import image_summary, read_png


@dataclass(frozen=True)
class NativeCase:
    name: str
    executable: str
    arguments: tuple[str, ...]
    outputs: tuple[str, ...]
    capability: str


def executable(binary_dir: Path, name: str) -> Path | None:
    candidate = binary_dir / name
    if candidate.is_file() and os.access(candidate, os.X_OK):
        return candidate
    return None


def cases(binary_dir: Path, root: Path, output: Path) -> list[NativeCase]:
    fixture = root / "benchmarks/scenes"
    cases: list[NativeCase] = []

    def add(
        name: str,
        binary: str,
        arguments: tuple[str, ...],
        outputs: tuple[str, ...],
        capability: str,
    ) -> None:
        if executable(binary_dir, binary) is not None:
            cases.append(NativeCase(name, binary, arguments, outputs, capability))

    add(
        "annotations-and-measurements",
        "annotations",
        (str(fixture / "optics-depth-stack.cif"), str(output / "annotations.png")),
        ("annotations.png",),
        "typed labels, markers, distance and angle guides",
    )
    add(
        "interaction-glyphs",
        "interactions",
        (str(fixture / "optics-depth-stack.cif"), str(output / "interactions.png")),
        ("interactions.png",),
        "hydrogen-bond, salt, pi, hydrophobic and metal glyphs",
    )
    add(
        "putty-property-radius",
        "putty",
        (str(fixture / "4hhb.cif"), str(output / "putty.png")),
        ("putty.png",),
        "B-factor-driven variable-radius tube",
    )
    add(
        "scalar-surface-overlay",
        "scalar_overlay",
        (str(fixture / "optics-depth-stack.cif"), str(output / "scalar-overlay.png")),
        ("scalar-overlay.png",),
        "caller-supplied scalar field, contours and surface color mapping",
    )
    add(
        "property-landscape",
        "property_landscape",
        (str(fixture / "4hhb.cif"), str(output / "property-landscape.png")),
        ("property-landscape.png",),
        "property-driven cartoon, ligand and solvent composition",
    )
    add(
        "anisotropic-material-and-alpha",
        "anisotropic_ribbons",
        (
            str(fixture / "4hhb.cif"),
            str(output / "anisotropic.png"),
            "",
            str(output / "anisotropic-transparent.png"),
        ),
        ("anisotropic.png", "anisotropic-transparent.png"),
        "anisotropic material path and transparent off-screen output",
    )
    add(
        "selection-camera-focus",
        "selection_focus",
        (
            str(fixture / "optics-depth-stack.cif"),
            str(output / "focus-camera.png"),
            str(output / "focus-selection.png"),
        ),
        ("focus-camera.png", "focus-selection.png"),
        "camera-target versus selection-tracked depth of field",
    )
    add(
        "gpu-picking",
        "picking_smoke",
        (
            str(fixture / "1BNA.cif"),
            str(output / "picking.png"),
        ),
        ("picking.png",),
        "ID-buffer pixel readback resolves a typed molecular entity",
    )
    add(
        "primitives-and-streamline",
        "primitives",
        (
            str(fixture / "optics-depth-stack.cif"),
            str(output / "primitives.png"),
        ),
        ("primitives.png",),
        "generic analytic particles, scientific glyphs and caller-integrated streamline",
    )
    add(
        "trajectory-frames",
        "trajectory",
        (
            str(fixture / "optics-depth-stack.cif"),
            "synthetic",
            str(output / "trajectory"),
        ),
        ("trajectory-start.png", "trajectory-mid.png", "trajectory-end.png"),
        "topology-stable start, midpoint and end trajectory states",
    )
    add(
        "clipped-ligand-pocket",
        "ligand_cutaway",
        (
            str(fixture / "4hhb.cif"),
            "HEM",
            str(output / "ligand-cutaway.png"),
            "inspection",
        ),
        ("ligand-cutaway.png",),
        "focus/context composition, SES pocket and clipping plane",
    )
    add(
        "multi-structure-quality-scene",
        "showcase",
        (str(output / "showcase.png"),),
        ("showcase.png",),
        "multi-structure placement, chain color and quality rendering",
    )
    for mode in ("direct", "isosurface", "medium", "slice", "crop"):
        add(
            f"volume-{mode}",
            "volume_smoke",
            (str(output / f"volume-{mode}.png"), mode),
            (f"volume-{mode}.png",),
            f"synthetic volume {mode} mode",
        )
    return cases


def native_image_summary(path: Path) -> dict[str, Any]:
    """Summarize large native images with a bounded pixel sampling pass."""

    width, height, pixels = read_png(path)
    if width * height <= 400_000:
        return image_summary(path)
    step = max(1, math.ceil(math.sqrt((width * height) / 250_000)))
    sampled = [
        (x, y, pixels[y * width + x])
        for y in range(0, height, step)
        for x in range(0, width, step)
    ]
    luma = [sum(pixel[2][:3]) / 3.0 for pixel in sampled]
    colorful = sum(max(pixel[2][:3]) - min(pixel[2][:3]) > 12 for pixel in sampled)
    edge_pixels = 0
    for x, y, pixel in sampled:
        if x + step >= width and y + step >= height:
            continue
        neighbours = []
        if x + step < width:
            neighbours.append(pixels[y * width + x + step])
        if y + step < height:
            neighbours.append(pixels[(y + step) * width + x])
        if neighbours and max(
            abs(pixel[channel] - sum(neighbour[channel] for neighbour in neighbours) / len(neighbours))
            for channel in range(3)
        ) > 2.0:
            edge_pixels += 1
    return {
        "width": width,
        "height": height,
        "bytes": path.stat().st_size,
        "sample_step": step,
        "sampled_pixels": len(sampled),
        "background": list(pixels[0]),
        "colorful_pixels": colorful,
        "edge_pixels": edge_pixels,
        "luma_min": min(luma),
        "luma_max": max(luma),
        "luma_mean": sum(luma) / len(luma),
        "non_opaque_pixels": sum(pixel[2][3] != 255 for pixel in sampled),
    }


def run_case(case: NativeCase, binary_dir: Path, output: Path, backend: str) -> dict[str, Any]:
    command = [str(binary_dir / case.executable), *case.arguments]
    environment = os.environ.copy()
    environment["WGPU_BACKEND"] = backend
    completed = subprocess.run(
        command,
        cwd=binary_dir.parents[2],
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    result: dict[str, Any] = {
        "name": case.name,
        "capability": case.capability,
        "command": command,
        "returncode": completed.returncode,
        "stdout": completed.stdout[-2000:],
        "stderr": completed.stderr[-2000:],
    }
    if completed.returncode != 0:
        result["status"] = "failed"
        return result
    images: list[dict[str, Any]] = []
    missing = []
    for relative in case.outputs:
        path = output / relative
        if not path.is_file():
            missing.append(relative)
            continue
        summary = native_image_summary(path)
        summary["signal"] = bool(summary["edge_pixels"] or summary["colorful_pixels"])
        images.append({"file": relative, **summary})
    result["images"] = images
    result["missing_outputs"] = missing
    result["status"] = "passed" if images and not missing else "failed"
    if result["status"] == "passed" and not all(image["signal"] for image in images):
        result["status"] = "weak-image-signal"
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--backend", default="metal")
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.repo.resolve()
    binary_dir = (args.binary_dir or root / "target/release/examples").resolve()
    if not binary_dir.is_dir():
        raise SystemExit(f"example directory does not exist: {binary_dir}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="pdviewx-native-probe-") as directory:
        output = Path(directory)
        matrix = cases(binary_dir, root, output)
        results = [run_case(case, binary_dir, output, args.backend) for case in matrix]
        result = {
            "schema": 1,
            "scope": "native pdviewx visual examples; not a golden or cross-engine equivalence test",
            "binary_dir": str(binary_dir),
            "backend": args.backend,
            "case_count": len(results),
            "status_counts": {
                status: sum(item["status"] == status for item in results)
                for status in sorted({item["status"] for item in results})
            },
            "cases": results,
        }
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
