#!/usr/bin/env python3
"""Create a source-grounded inventory for the non-PyMOL references.

Reference manuals are deliberately not vendored. When a caller supplies the
downloaded HTML/PDF text from a disposable directory, this script records the
headings and command counts it saw. The static manifest keeps every reviewed
visual surface attached to an explicit owner or missing-capability decision.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
from html.parser import HTMLParser
from pathlib import Path
from typing import Any

from graphics_engine_reference_manifest import (
    CHIMERAX_IMAGE_COMMANDS,
    CHIMERAX_MOVIE_ACTIONS,
    CHIMERAX_MOVIE_ENCODE_OPTIONS,
    CHIMERAX_MOVIE_RECORD_OPTIONS,
    CHIMERAX_SURFACE_OPTIONS,
    CHIMERAX_VOLUME_GROUPS,
    CHIMERAX_VOLUME_OPERATIONS,
    OVITO_DATA_OBJECTS,
    OVITO_EXPORTS,
    OVITO_PARTICLE_SHAPES,
    OVITO_PIPELINE_CAPABILITIES,
    OVITO_RENDERERS,
    OVITO_VISUAL_CLASSES,
    PROTEIN_COLORS,
    PROTEIN_EXPORTS,
    PROTEIN_FORMATS,
    PROTEIN_REPRESENTATIONS,
    PROTEIN_SCENE_CONTROLS,
    PROTEIN_SELECTION,
    PROTEIN_VIEW_CONTROLS,
    PYMOL_CGO_OPCODES,
    STATIC_EXPECTED_COUNTS,
    VMD_GRAPHICS_COMMANDS,
    VMD_GRAPHICS_PRIMITIVES,
    VMD_CONTROLS,
    VMD_RENDERERS,
    VMD_STEREO_MODES,
    VMD_STYLES,
    VMD_TRAJECTORY_SCENE,
    YASARA_FEATURE_SECTIONS,
    YASARA_GRAPHICS_SECTIONS,
    build_feature_coverage,
)


class HeadingParser(HTMLParser):
    """Collect heading text without requiring BeautifulSoup."""

    def __init__(self) -> None:
        super().__init__()
        self.level: str | None = None
        self.parts: list[str] = []
        self.headings: list[tuple[str, str]] = []

    def handle_starttag(self, tag: str, _attrs: list[tuple[str, str | None]]) -> None:
        if tag in {"h1", "h2", "h3", "h4", "h5"}:
            self.level = tag
            self.parts = []

    def handle_data(self, data: str) -> None:
        if self.level:
            self.parts.append(data)

    def handle_endtag(self, tag: str) -> None:
        if tag == self.level:
            value = re.sub(r"\s+", " ", html.unescape("".join(self.parts))).strip()
            if value and value not in {"°", "-"}:
                self.headings.append((tag, value))
            self.level = None
            self.parts = []


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def source_evidence(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {"status": "not-supplied"}
    if not path.is_file():
        return {"status": "missing", "path": str(path)}
    data = path.read_bytes()
    return {
        "status": "read",
        "path": str(path),
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def parse_yasara(path: Path | None) -> dict[str, Any]:
    evidence = source_evidence(path)
    result: dict[str, Any] = {
        "feature_sections": list(YASARA_FEATURE_SECTIONS),
        "graphics_sections": list(YASARA_GRAPHICS_SECTIONS),
        "manual_boundary": "The online feature list is not the shipped full manual; product-tier availability is vendor-defined.",
        "source": evidence,
    }
    if path is None or not path.is_file():
        return result
    parser = HeadingParser()
    parser.feed(read_text(path))
    current: str | None = None
    pending: str | None = None
    commands: dict[str, list[dict[str, str]]] = {name: [] for name in YASARA_FEATURE_SECTIONS}
    seen: set[tuple[str, str, str]] = set()
    duplicate_commands: list[dict[str, str]] = []
    raw_command_count = 0
    for level, value in parser.headings:
        if level == "h2" and value in YASARA_FEATURE_SECTIONS:
            current, pending = value, None
        elif level == "h4" and current:
            if pending is None:
                pending = value
            else:
                raw_command_count += 1
                key = (current, pending, value)
                command = {"name": pending, "description": value}
                if key in seen:
                    duplicate_commands.append({"section": current, **command})
                else:
                    commands[current].append(command)
                    seen.add(key)
                pending = None
    result["commands_by_section"] = commands
    result["source_command_row_count"] = raw_command_count
    result["duplicate_commands"] = duplicate_commands
    result["command_count"] = sum(len(values) for values in commands.values())
    result["graphics_command_count"] = sum(len(commands[name]) for name in YASARA_GRAPHICS_SECTIONS)
    return result


def parse_html_headings(path: Path | None) -> dict[str, Any]:
    evidence = source_evidence(path)
    result: dict[str, Any] = {"source": evidence}
    if path is None or not path.is_file():
        return result
    content = read_text(path)
    parser = HeadingParser()
    parser.feed(content)
    result["headings"] = [value for _level, value in parser.headings]
    result["image_count"] = len(re.findall(r"<img\b", content, re.IGNORECASE))
    return result


def source_term_matches(paths: list[Path | None], terms: list[str]) -> dict[str, Any]:
    """Report which reviewed vocabulary terms occur in supplied source text."""

    supplied = [path for path in paths if path is not None and path.is_file()]
    if not supplied:
        return {"status": "not-supplied", "matched": [], "missing": [], "total": len(terms)}
    content = " ".join(read_text(path) for path in supplied)
    normalized = re.sub(r"[^a-z0-9]+", "", html.unescape(content).lower())
    matched = [term for term in terms if re.sub(r"[^a-z0-9]+", "", term.lower()) in normalized]
    return {
        "status": "read",
        "matched": matched,
        "missing": [term for term in terms if term not in matched],
        "total": len(terms),
    }


def build_inventory(args: argparse.Namespace) -> dict[str, Any]:
    vmd_text = read_text(args.vmd_text) if args.vmd_text and args.vmd_text.is_file() else ""
    vmd_names = [name for name, _description in VMD_STYLES]
    result: dict[str, Any] = {
        "schema": 2,
        "status": "passed",
        "scope": "graphics-engine capability inventory; pdbiox is provider-only",
        "sources": {
            "vmd": "http://www.ks.uiuc.edu/Research/vmd/current/docs.html",
            "pymol": "https://pymol.org/dokuwiki/",
            "chimerax": "https://www.cgl.ucsf.edu/chimerax/docs/index.html",
            "ovito": "https://www.ovito.org/docs/current/python/",
            "yasara": "http://www.yasara.org/md.htm",
            "protein_imager": "https://3dproteinimaging.com/",
        },
        "pymol": {
            "cgo_opcodes": PYMOL_CGO_OPCODES,
            "cgo_source": "https://wiki.pymol.org/index.php/Category%3ACGO",
            "text_source": "https://wiki.pymol.org/index.php/CGO_Text",
        },
        "vmd": {
            "representation_styles": [{"name": name, "description": description} for name, description in VMD_STYLES],
            "style_count": len(VMD_STYLES),
            "graphics_primitives": VMD_GRAPHICS_PRIMITIVES,
            "graphics_commands": VMD_GRAPHICS_COMMANDS,
            "controls": VMD_CONTROLS,
            "stereo_modes": VMD_STEREO_MODES,
            "renderers": VMD_RENDERERS,
            "trajectory_and_scene": VMD_TRAJECTORY_SCENE,
            "source": source_evidence(args.vmd_text),
            "image_gallery": parse_html_headings(args.vmd_repimages),
            "source_style_matches": [
                name for name in vmd_names
                if re.search(rf"^\s*{re.escape(name)}\s", vmd_text, re.MULTILINE)
            ],
        },
        "chimerax": {
            "image_commands": CHIMERAX_IMAGE_COMMANDS,
            "volume_option_groups": CHIMERAX_VOLUME_GROUPS,
            "volume_operations": CHIMERAX_VOLUME_OPERATIONS,
            "surface_options": CHIMERAX_SURFACE_OPTIONS,
            "movie_actions": CHIMERAX_MOVIE_ACTIONS,
            "movie_record_options": CHIMERAX_MOVIE_RECORD_OPTIONS,
            "movie_encode_options": CHIMERAX_MOVIE_ENCODE_OPTIONS,
            "sources": {
                "images": parse_html_headings(args.chimerax_images),
                "volume": parse_html_headings(args.chimerax_volume),
                "surface": parse_html_headings(args.chimerax_surface),
                "movie": parse_html_headings(args.chimerax_movie),
            },
        },
        "ovito": {
            "data_objects": OVITO_DATA_OBJECTS,
            "pipeline_capabilities": OVITO_PIPELINE_CAPABILITIES,
            "visual_classes": OVITO_VISUAL_CLASSES,
            "renderers": OVITO_RENDERERS,
            "exports": OVITO_EXPORTS,
            "source": {"url": "https://www.ovito.org/docs/current/python/"},
        },
        "yasara": parse_yasara(args.yasara_features),
        "protein_imager": {
            "accepted_formats": PROTEIN_FORMATS,
            "representations": PROTEIN_REPRESENTATIONS,
            "coloring": PROTEIN_COLORS,
            "view_controls": PROTEIN_VIEW_CONTROLS,
            "selection": PROTEIN_SELECTION,
            "scene_controls": PROTEIN_SCENE_CONTROLS,
            "exports": PROTEIN_EXPORTS,
            "source": source_evidence(args.protein_overview),
            "site_source": source_evidence(args.protein_site),
        },
    }
    result["yasara"]["graphics_source"] = source_evidence(args.yasara_graphics)
    result["yasara"]["lighting_source"] = source_evidence(args.yasara_lighting)
    result["yasara"]["md_source"] = source_evidence(args.yasara_md)
    result["source_term_matches"] = {
        "VMD": {
            "styles": source_term_matches(
                [args.vmd_text], [name for name, _description in VMD_STYLES]
            ),
            "primitives": source_term_matches([args.vmd_text], VMD_GRAPHICS_PRIMITIVES),
        },
        "ChimeraX": {
            "image_commands": source_term_matches([args.chimerax_images], CHIMERAX_IMAGE_COMMANDS),
            "volume_groups": source_term_matches([args.chimerax_volume], CHIMERAX_VOLUME_GROUPS),
            "volume_operations": source_term_matches([args.chimerax_volume], CHIMERAX_VOLUME_OPERATIONS),
            "surface_options": source_term_matches([args.chimerax_surface], CHIMERAX_SURFACE_OPTIONS),
            "movie": source_term_matches(
                [args.chimerax_movie],
                CHIMERAX_MOVIE_ACTIONS + CHIMERAX_MOVIE_RECORD_OPTIONS + CHIMERAX_MOVIE_ENCODE_OPTIONS,
            ),
        },
        "OVITO": {
            "data_objects": source_term_matches([args.ovito_data], OVITO_DATA_OBJECTS),
            "particle_shapes": source_term_matches([args.ovito_data, args.ovito_vis], OVITO_PARTICLE_SHAPES),
            "pipeline": source_term_matches(
                [
                    args.ovito_data,
                    args.ovito_vis,
                    getattr(args, "ovito_modifiers", None),
                    getattr(args, "ovito_pipeline", None),
                ],
                OVITO_PIPELINE_CAPABILITIES,
            ),
            "visual_classes": source_term_matches([args.ovito_vis], OVITO_VISUAL_CLASSES),
            "renderers": source_term_matches([args.ovito_vis, args.ovito_rendering], OVITO_RENDERERS),
            "exports": source_term_matches([args.ovito_rendering], OVITO_EXPORTS),
        },
        "Protein Imager": {
            "site_and_interface": source_term_matches(
                [args.protein_site, args.protein_overview_text],
                PROTEIN_FORMATS + PROTEIN_REPRESENTATIONS,
            ),
        },
    }
    result["feature_coverage"] = build_feature_coverage(result["yasara"])
    result["feature_coverage_counts"] = {
        status: sum(item["status"] == status for item in result["feature_coverage"])
        for status in sorted({item["status"] for item in result["feature_coverage"]})
    }
    result["feature_coverage_by_source"] = {
        source: sum(item["source"] == source for item in result["feature_coverage"])
        for source in sorted({item["source"] for item in result["feature_coverage"]})
    }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vmd-text", type=Path)
    parser.add_argument("--vmd-repimages", type=Path)
    parser.add_argument("--chimerax-images", type=Path)
    parser.add_argument("--chimerax-volume", type=Path)
    parser.add_argument("--chimerax-surface", type=Path)
    parser.add_argument("--chimerax-movie", type=Path)
    parser.add_argument("--ovito-data", type=Path)
    parser.add_argument("--ovito-vis", type=Path)
    parser.add_argument("--ovito-modifiers", type=Path)
    parser.add_argument("--ovito-pipeline", type=Path)
    parser.add_argument("--ovito-rendering", type=Path)
    parser.add_argument("--yasara-features", type=Path)
    parser.add_argument("--yasara-graphics", type=Path)
    parser.add_argument("--yasara-lighting", type=Path)
    parser.add_argument("--yasara-md", type=Path)
    parser.add_argument("--protein-overview", type=Path)
    parser.add_argument("--protein-overview-text", type=Path)
    parser.add_argument("--protein-site", type=Path)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if a supplied source snapshot is not fully represented in the manifest",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = build_inventory(args)
    coverage_issues: list[str] = []
    if args.check:
        if result["yasara"]["source"]["status"] != "read":
            coverage_issues.append("YASARA feature-list source must be supplied for --check")
        else:
            yasara_records = [
                item for item in result["feature_coverage"] if item["source"] == "YASARA"
            ]
            expected = result["yasara"].get("command_count", 0)
            if len(yasara_records) != expected:
                coverage_issues.append(
                    f"YASARA manifest has {len(yasara_records)} records for {expected} parsed commands"
                )
        for source, groups in result["source_term_matches"].items():
            for family, evidence in groups.items():
                if evidence["status"] == "read" and not evidence["matched"]:
                    coverage_issues.append(f"{source}/{family} source has no matched vocabulary terms")
        for source, expected in STATIC_EXPECTED_COUNTS.items():
            actual = result["feature_coverage_by_source"].get(source, 0)
            if actual != expected:
                coverage_issues.append(
                    f"{source} manifest has {actual} records for expected {expected}"
                )
        unreviewed = [
            item["feature"]
            for item in result["feature_coverage"]
            if item["status"] == "unreviewed"
        ]
        if unreviewed:
            coverage_issues.append(f"unreviewed feature items: {', '.join(unreviewed)}")
        seen: set[tuple[str, str, str]] = set()
        duplicates: list[str] = []
        for item in result["feature_coverage"]:
            key = (item["source"], item["family"], item["feature"])
            if key in seen:
                duplicates.append("/".join(key))
            seen.add(key)
        if duplicates:
            coverage_issues.append(f"duplicate feature records: {', '.join(duplicates)}")
        result["coverage_complete"] = not coverage_issues
        result["coverage_issues"] = coverage_issues
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 1 if coverage_issues else 0


if __name__ == "__main__":
    raise SystemExit(main())
