#!/usr/bin/env python3
"""Run a disposable, matched image smoke matrix for PyMOL and pdviewx.

This is intentionally not a pixel-equivalence test.  It proves that the same
local structure fixture and representation intent can be rendered by both
engines, then records conservative image-shape metrics for later differential
tests.  Reference runtimes and all images stay outside the repository.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class RenderCase:
    name: str
    fixture: str
    pdviewx_representation: str
    pymol_mode: str
    pdviewx_mode: str = "realtime"


CASES = (
    RenderCase("spacefill", "base_pair.cif", "spacefill", "spheres"),
    RenderCase("ball-and-stick", "base_pair.cif", "ball-and-stick", "ball-and-stick"),
    RenderCase(
        "quality-ball-and-stick",
        "base_pair.cif",
        "ball-and-stick",
        "ball-and-stick",
        "quality",
    ),
    RenderCase("licorice", "base_pair.cif", "licorice", "licorice"),
    RenderCase("lines", "base_pair.cif", "lines", "lines"),
    RenderCase("points", "base_pair.cif", "points", "points"),
    RenderCase("cartoon", "1BNA.cif", "cartoon", "cartoon"),
    RenderCase("trace", "1BNA.cif", "trace", "ribbon"),
    RenderCase("tube", "1BNA.cif", "tube", "tube"),
    RenderCase("vdw-surface", "1ubq.cif", "vdw-surface", "surface"),
    RenderCase("sas", "1ubq.cif", "sas", "surface"),
    RenderCase("ses-contour", "1ubq.cif", "ses-contour", "surface"),
    RenderCase("ses-dots", "1ubq.cif", "ses-dots", "surface"),
    RenderCase("layered", "1ubq.cif", "layered", "layered"),
)


def error_summary(error: BaseException) -> str:
    return f"{type(error).__name__}: {error}"[:500]


def probe_pymol_case(case: RenderCase, fixture: Path, output: Path, width: int, height: int) -> dict[str, Any]:
    """Render one case inside the caller-provided disposable PyMOL runtime."""

    try:
        import pymol
        from pymol import cmd
    except Exception as error:
        return {"status": "unavailable", "error": error_summary(error)}

    stdout = io.StringIO()
    stderr = io.StringIO()
    try:
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            pymol.finish_launching(["pymol", "-cq"])
            cmd.load(str(fixture), "structure")
            cmd.hide("everything", "all")
            if case.pymol_mode == "spheres":
                cmd.show("spheres", "all")
                cmd.set("sphere_scale", 0.30, "all")
            elif case.pymol_mode == "ball-and-stick":
                cmd.show("sticks", "all")
                cmd.show("spheres", "all")
                cmd.set("sphere_scale", 0.30, "all")
                cmd.set("stick_radius", 0.12, "all")
            elif case.pymol_mode == "licorice":
                cmd.show("sticks", "all")
                cmd.set("stick_radius", 0.22, "all")
            elif case.pymol_mode == "lines":
                cmd.show("lines", "all")
            elif case.pymol_mode == "points":
                cmd.show("dots", "all")
            elif case.pymol_mode in {"cartoon", "ribbon"}:
                cmd.show(case.pymol_mode, "all")
            elif case.pymol_mode == "tube":
                cmd.show("cartoon", "all")
                cmd.cartoon("tube", "all")
            elif case.pymol_mode == "surface":
                cmd.show("surface", "all")
            elif case.pymol_mode == "layered":
                cmd.show("cartoon", "all")
                cmd.show("surface", "all")
                cmd.set("transparency", 65, "structure")
            else:
                raise ValueError(f"unsupported PyMOL mode: {case.pymol_mode}")
            cmd.bg_color("grey90")
            cmd.orient("all")
            cmd.png(str(output), width=width, height=height, ray=1, quiet=1)
            atoms = cmd.count_atoms("all")
            cmd.quit()
        return {"status": "passed", "atoms": atoms, "bytes": output.stat().st_size}
    except Exception as error:
        return {"status": "failed", "error": error_summary(error)}


def read_png(path: Path) -> tuple[int, int, list[tuple[int, int, int, int]]]:
    """Decode the 8-bit non-interlaced RGB/RGBA PNGs emitted by the engines."""

    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"not a PNG: {path}")
    position = 8
    width = height = color_type = bit_depth = interlace = None
    compressed = bytearray()
    while position < len(data):
        length = int.from_bytes(data[position : position + 4], "big")
        kind = data[position + 4 : position + 8]
        payload = data[position + 8 : position + 8 + length]
        position += length + 12
        if kind == b"IHDR":
            width = int.from_bytes(payload[0:4], "big")
            height = int.from_bytes(payload[4:8], "big")
            bit_depth, color_type, interlace = payload[8], payload[9], payload[12]
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            break
    if None in (width, height, bit_depth, color_type, interlace):
        raise ValueError("PNG has no complete header")
    if bit_depth != 8 or color_type not in (2, 6) or interlace != 0:
        raise ValueError("only non-interlaced 8-bit RGB/RGBA PNG is supported")
    channels = 4 if color_type == 6 else 3
    stride = width * channels
    raw = zlib.decompress(bytes(compressed))
    rows: list[bytes] = []
    prior = bytearray(stride)
    cursor = 0
    for _ in range(height):
        filter_kind = raw[cursor]
        source = raw[cursor + 1 : cursor + 1 + stride]
        cursor += stride + 1
        row = bytearray(source)
        for index in range(stride):
            left = row[index - channels] if index >= channels else 0
            up = prior[index]
            upper_left = prior[index - channels] if index >= channels else 0
            if filter_kind == 1:
                row[index] = (row[index] + left) & 255
            elif filter_kind == 2:
                row[index] = (row[index] + up) & 255
            elif filter_kind == 3:
                row[index] = (row[index] + ((left + up) // 2)) & 255
            elif filter_kind == 4:
                estimate = left + up - upper_left
                distances = (abs(estimate - left), abs(estimate - up), abs(estimate - upper_left))
                predictor = (left, up, upper_left)[distances.index(min(distances))]
                row[index] = (row[index] + predictor) & 255
            elif filter_kind != 0:
                raise ValueError(f"unsupported PNG filter {filter_kind}")
        rows.append(bytes(row))
        prior = row
    pixels = []
    for row in rows:
        for index in range(0, stride, channels):
            rgb = tuple(row[index : index + 3])
            alpha = row[index + 3] if channels == 4 else 255
            pixels.append((*rgb, alpha))
    return int(width), int(height), pixels


def image_summary(path: Path) -> dict[str, Any]:
    width, height, pixels = read_png(path)
    background = pixels[0]
    luma = [sum(pixel[:3]) / 3.0 for pixel in pixels]
    colorful = sum(max(pixel[:3]) - min(pixel[:3]) > 12 for pixel in pixels)
    edge_points: list[tuple[int, int]] = []
    for index, pixel in enumerate(pixels):
        x, y = index % width, index // width
        neighbours = []
        for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
            neighbour_x, neighbour_y = x + dx, y + dy
            if 0 <= neighbour_x < width and 0 <= neighbour_y < height:
                neighbours.append(pixels[neighbour_y * width + neighbour_x])
        local_contrast = max(
            abs(pixel[channel] - sum(neighbour[channel] for neighbour in neighbours) / len(neighbours))
            for channel in range(3)
        )
        if local_contrast > 2.0 or max(pixel[:3]) - min(pixel[:3]) > 15:
            edge_points.append((x, y))
    if edge_points:
        edge_x, edge_y = zip(*edge_points)
        edge_bounds = [min(edge_x), min(edge_y), max(edge_x) + 1, max(edge_y) + 1]
        edge_centroid = [sum(edge_x) / len(edge_x), sum(edge_y) / len(edge_y)]
    else:
        edge_bounds = None
        edge_centroid = None
    luma_mean = sum(luma) / len(luma)
    luma_variance = sum((value - luma_mean) ** 2 for value in luma) / len(luma)
    return {
        "width": width,
        "height": height,
        "bytes": path.stat().st_size,
        "background": list(background),
        "colorful_pixels": colorful,
        "edge_pixels": len(edge_points),
        "edge_bounds": edge_bounds,
        "edge_centroid": edge_centroid,
        "luma_min": min(luma),
        "luma_max": max(luma),
        "luma_mean": luma_mean,
        "luma_variance": luma_variance,
        "non_opaque_pixels": sum(pixel[3] != 255 for pixel in pixels),
    }


def structural_comparison(first: dict[str, Any], second: dict[str, Any]) -> dict[str, Any]:
    """Report normalized silhouette geometry without declaring equivalence."""

    first_bounds = first.get("edge_bounds")
    second_bounds = second.get("edge_bounds")
    intersection = union = 0.0
    if first_bounds and second_bounds:
        left = max(first_bounds[0] / first["width"], second_bounds[0] / second["width"])
        top = max(first_bounds[1] / first["height"], second_bounds[1] / second["height"])
        right = min(first_bounds[2] / first["width"], second_bounds[2] / second["width"])
        bottom = min(first_bounds[3] / first["height"], second_bounds[3] / second["height"])
        intersection = max(0.0, right - left) * max(0.0, bottom - top)
        first_area = (first_bounds[2] / first["width"] - first_bounds[0] / first["width"]) * (
            first_bounds[3] / first["height"] - first_bounds[1] / first["height"]
        )
        second_area = (second_bounds[2] / second["width"] - second_bounds[0] / second["width"]) * (
            second_bounds[3] / second["height"] - second_bounds[1] / second["height"]
        )
        union = first_area + second_area - intersection
    first_centroid = first.get("edge_centroid")
    second_centroid = second.get("edge_centroid")
    centroid_distance = None
    if first_centroid and second_centroid:
        dx = first_centroid[0] / first["width"] - second_centroid[0] / second["width"]
        dy = first_centroid[1] / first["height"] - second_centroid[1] / second["height"]
        centroid_distance = (dx * dx + dy * dy) ** 0.5
    return {
        "normalized_edge_bbox_iou": intersection / union if union else None,
        "normalized_edge_centroid_distance": centroid_distance,
        "edge_pixel_ratio": [
            first.get("edge_pixels", 0) / (first["width"] * first["height"]),
            second.get("edge_pixels", 0) / (second["width"] * second["height"]),
        ],
        "interpretation": "diagnostic geometry only; no pass/fail equivalence threshold",
    }


def render_pdviewx(
    executable: Path, fixture: Path, output: Path, case: RenderCase, width: int, height: int, root: Path
) -> dict[str, Any]:
    command = [
        str(executable),
        str(fixture),
        str(output),
        case.pdviewx_representation,
        str(width),
        str(height),
        "1.0",
        "1.0",
        case.pdviewx_mode,
        "inspection",
    ]
    environment = os.environ.copy()
    environment.setdefault("WGPU_BACKEND", "metal")
    completed = subprocess.run(command, cwd=root, capture_output=True, text=True, env=environment, check=False)
    if completed.returncode != 0:
        return {
            "status": "failed",
            "exit_code": completed.returncode,
            "stderr": completed.stderr[-1000:],
        }
    return {"status": "passed", "stdout": completed.stdout[-1000:], "bytes": output.stat().st_size}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--pymol-python", help="disposable Python interpreter containing PyMOL")
    parser.add_argument("--pdviewx-example", type=Path, help="built headless_smoke example executable")
    parser.add_argument("--output", type=Path, help="write JSON evidence here")
    parser.add_argument("--width", type=int, default=128)
    parser.add_argument("--height", type=int, default=128)
    parser.add_argument("--case", action="append", choices=[case.name for case in CASES])
    parser.add_argument("--pymol-case", help=argparse.SUPPRESS)
    parser.add_argument("--fixture", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--image", type=Path, help=argparse.SUPPRESS)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.pymol_case:
        selected = next(case for case in CASES if case.name == args.pymol_case)
        if not args.fixture or not args.image:
            raise SystemExit("PyMOL child mode requires --fixture and --image")
        result = probe_pymol_case(selected, args.fixture, args.image, args.width, args.height)
        encoded = json.dumps(result, sort_keys=True)
        if args.output:
            args.output.write_text(encoded + "\n", encoding="utf-8")
        else:
            print(encoded)
        return 0 if result["status"] == "passed" else 1
    if not args.pymol_python or not args.pdviewx_example:
        raise SystemExit("--pymol-python and --pdviewx-example are required")
    root = args.repo.resolve()
    selected_names = set(args.case or [case.name for case in CASES])
    selected = [case for case in CASES if case.name in selected_names]
    with tempfile.TemporaryDirectory(prefix="pdviewx-cross-render-") as directory:
        work = Path(directory)
        records = []
        for case in selected:
            fixture = root / "benchmarks/scenes" / case.fixture
            pymol_image = work / f"pymol-{case.name}.png"
            pdviewx_image = work / f"pdviewx-{case.name}.png"
            child = [
                args.pymol_python,
                str(Path(__file__).resolve()),
                "--pymol-case",
                case.name,
                "--fixture",
                str(fixture),
                "--image",
                str(pymol_image),
                "--width",
                str(args.width),
                "--height",
                str(args.height),
            ]
            completed = subprocess.run(child, capture_output=True, text=True, check=False)
            try:
                pymol_result = json.loads(completed.stdout)
            except json.JSONDecodeError:
                pymol_result = {"status": "failed", "error": completed.stderr[-1000:]}
            pdviewx_result = render_pdviewx(
                args.pdviewx_example, fixture, pdviewx_image, case, args.width, args.height, root
            )
            record: dict[str, Any] = {
                "name": case.name,
                "fixture": str(fixture.relative_to(root)),
                "pymol": pymol_result,
                "pdviewx": pdviewx_result,
            }
            if pymol_result.get("status") == "passed" and pdviewx_result.get("status") == "passed":
                record["pymol"]["image"] = image_summary(pymol_image)
                record["pdviewx"]["image"] = image_summary(pdviewx_image)
                record["structural"] = structural_comparison(
                    record["pymol"]["image"], record["pdviewx"]["image"]
                )
                record["status"] = "passed"
            else:
                record["status"] = "failed"
            records.append(record)
    result = {
        "schema": 1,
        "status": "passed" if all(record["status"] == "passed" for record in records) else "failed",
        "scope": "matched semantic image smoke only; not pixel or scientific equivalence",
        "cases": records,
    }
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        sys.stdout.write(encoded)
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
