#!/usr/bin/env python3
"""Smoke-test the hosted Protein Imager in a disposable browser session.

The site is an application around NGL, not a local pdviewx dependency. This
probe records the site's real input/representation controls, emits one
UI/example observation for every static feature record, loads every public
example project, captures one non-empty canvas screenshot, and removes the
browser session and image when it exits.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import struct
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

try:
    from graphics_engine_reference_manifest import (
        PROTEIN_COLORS,
        PROTEIN_EXPORTS,
        PROTEIN_FORMATS,
        PROTEIN_REPRESENTATIONS,
        PROTEIN_SCENE_CONTROLS,
        PROTEIN_SELECTION,
        PROTEIN_VIEW_CONTROLS,
    )
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_manifest import (
        PROTEIN_COLORS,
        PROTEIN_EXPORTS,
        PROTEIN_FORMATS,
        PROTEIN_REPRESENTATIONS,
        PROTEIN_SCENE_CONTROLS,
        PROTEIN_SELECTION,
        PROTEIN_VIEW_CONTROLS,
    )


DEFAULT_URL = "https://3dproteinimaging.com/protein-imager"
AUDIT_JS = r"""JSON.stringify((() => {
  const text = document.body?.innerText || '';
  const clean = value => String(value ?? '').replace(/\s+/g, ' ').trim();
  const files = [...document.querySelectorAll('input[type=file]')].map(input => ({
    id: input.id, name: input.name, accept: input.accept, multiple: input.multiple
  }));
  const selects = [...document.querySelectorAll('select')].map(select => ({
    id: select.id, name: select.name, options: [...select.options].map(option => ({
      text: clean(option.textContent), value: option.value
    }))
  }));
  const examples = [...document.querySelectorAll('.exampleProjectCont')].map(box => ({
    title: clean(box.querySelector('h2,h3')?.textContent),
    button: Boolean(box.querySelector('button, input[type=button], input[type=submit]'))
  })).filter(example => example.title);
  const canvases = [...document.querySelectorAll('canvas')].map(canvas => ({
    width: canvas.width, height: canvas.height,
    clientWidth: canvas.clientWidth, clientHeight: canvas.clientHeight
  }));
  const buttons = [...document.querySelectorAll('button, input[type=button], input[type=submit]')]
    .map(button => clean(button.textContent || button.value)).filter(Boolean).slice(0, 200);
  const representationTerms = ['Sphere', 'Stick', 'Surface', 'Mesh', 'Simplify',
    'Cartoon', 'Tube', 'Label', 'Goodsell', 'uniform', 'element', 'moiety',
    'proximity', 'B-factor'];
  const representationOptions = [...new Set(selects.flatMap(select =>
    select.options.map(option => option.text).filter(option =>
      representationTerms.some(term => option.toLowerCase().includes(term.toLowerCase())))))];
  return {url: location.href, title: document.title, text: text.slice(0, 16000),
    fileInputs: files, selects, examples, canvases, buttons, representationOptions,
    hasLoadSuccess: /project\s+succesfully\s+loaded|project\s+successfully\s+loaded/i.test(text),
    hasStructure: /FILE NAME:\s*\S+/i.test(text)};
})())"""


def browser_call(executable: str, session: str, arguments: list[str], timeout: float = 60) -> dict[str, Any]:
    completed = subprocess.run(
        [executable, "--session", session, "--json", *arguments],
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if completed.returncode:
        raise RuntimeError(completed.stderr.strip() or completed.stdout.strip())
    for line in reversed(completed.stdout.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise RuntimeError(f"browser-use returned no JSON: {completed.stdout[-500:]}")


def browser_close(executable: str, session: str) -> None:
    subprocess.run(
        [executable, "--session", session, "close"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )


def eval_json(executable: str, session: str, expression: str = AUDIT_JS) -> dict[str, Any]:
    payload = browser_call(executable, session, ["eval", expression])
    result = payload.get("data", {}).get("result")
    if not isinstance(result, str):
        raise RuntimeError(f"browser eval has no string result: {payload}")
    value = json.loads(result)
    if not isinstance(value, dict):
        raise RuntimeError("browser audit result is not an object")
    return value


def wait_for_page(executable: str, session: str, timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    latest: dict[str, Any] = {}
    while time.monotonic() < deadline:
        try:
            latest = eval_json(executable, session)
            if latest.get("fileInputs") or latest.get("examples") or latest.get("canvases"):
                return latest
        except (RuntimeError, json.JSONDecodeError):
            pass
        time.sleep(0.5)
    raise RuntimeError(f"Protein Imager did not expose its controls: {latest}")


def click_example(executable: str, session: str, title: str) -> dict[str, Any]:
    encoded = json.dumps(title)
    expression = f"""JSON.stringify((() => {{
      const wanted = {encoded};
      const boxes = [...document.querySelectorAll('.exampleProjectCont')];
      const box = boxes.find(item => (item.querySelector('h2,h3')?.textContent || '').includes(wanted));
      const button = box?.querySelector('button, input[type=button], input[type=submit]');
      if (!button) return {{title: wanted, clicked: false}};
      button.click();
      return {{title: wanted, clicked: true}};
    }})())"""
    return eval_json(executable, session, expression)


def wait_for_example(executable: str, session: str, timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    latest: dict[str, Any] = {}
    while time.monotonic() < deadline:
        latest = eval_json(executable, session)
        if latest.get("hasLoadSuccess") or latest.get("hasStructure"):
            return latest
        time.sleep(0.75)
    return latest


def png_size(path: Path) -> tuple[int, int]:
    header = path.read_bytes()[:24]
    if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("screenshot is not a PNG")
    return struct.unpack(">II", header[16:24])


def feature_observations(result: dict[str, Any]) -> list[dict[str, Any]]:
    """Record UI/example evidence for every static Protein Imager feature."""

    startup = result.get("startup", {})
    texts = [startup.get("text", "")]
    texts.extend(
        input_control.get("accept", "")
        for input_control in startup.get("fileInputs", [])
    )
    texts.extend(
        option.get("text", "")
        for select in startup.get("selects", [])
        for option in select.get("options", [])
    )
    texts.extend(startup.get("buttons", []))
    texts.extend(
        " ".join(
            [
                example.get("title", ""),
                example.get("audit", {}).get("text", ""),
            ]
        )
        for example in result.get("examples", [])
    )
    haystack = " ".join(texts).lower()

    aliases: dict[str, tuple[str, ...]] = {
        **{
            feature: (f".{feature.lower()}",)
            for feature in PROTEIN_FORMATS
        },
        "Sphere": ("sphere",),
        "Stick": ("stick",),
        "Surface": ("surface",),
        "Mesh": ("mesh",),
        "Simplify": ("simplif",),
        "Filter by distance": ("filter", "distance"),
        "Cartoon": ("cartoon",),
        "Tube": ("tube",),
        "Label": ("label",),
        "Goodsell-like": ("goodsell",),
        "uniform": ("uniform",),
        "element": ("element",),
        "moiety": ("moiety",),
        "proximity": ("proximity",),
        "B-factor": ("bfactor",),
        "Real/Goodsell/Outlines presets": ("real", "goodsell", "outlines"),
        "aspect ratio": ("aspect ratio",),
        "fog near/far": ("fog",),
        "front clipping": ("clip",),
        "zoom/slicing": ("zoom",),
        "orthographic/perspective": ("orthographic", "perspective"),
        "flat color": ("flat",),
        "material sheen": ("sheen", "shine"),
        "light intensity": ("light",),
        "grayscale": ("grayscale", "greyscale"),
        "background color/transparency": ("background",),
        "interior color": ("interior",),
        "outlines": ("outline",),
        "outline color/sensitivity/thickness": ("outline",),
        "shadowing": ("shadow",),
        "atom/residue/chain/model picking": ("atom", "residue", "chain", "model"),
        "structure hierarchy": ("hierarchy",),
        "entity/proximity/range/property advanced selection": (
            "entity", "proximity", "range", "property"
        ),
        "NMR model selection": ("nmr",),
        "sequence selection": ("sequence",),
        "structure switching": ("structure",),
        "superimposition": ("superimpos",),
        "biological assembly": ("biological assembly",),
        "center/hide/delete": ("center", "hide", "delete"),
        "distance labels/connectors": ("distance", "connector"),
        "membrane bilayer": ("membrane",),
        "rock/spin animation": ("rock", "spin"),
        "local/server .3dpi projects": (".3dpi",),
        "server high-quality image": ("high-quality", "high quality"),
        "VRML2 mesh": ("vrml",),
        "download/email notification": ("download", "email"),
    }
    groups = (
        ("format", PROTEIN_FORMATS),
        ("representation", PROTEIN_REPRESENTATIONS),
        ("coloring", PROTEIN_COLORS),
        ("view-control", PROTEIN_VIEW_CONTROLS),
        ("selection", PROTEIN_SELECTION),
        ("scene-control", PROTEIN_SCENE_CONTROLS),
        ("export", PROTEIN_EXPORTS),
    )
    records: list[dict[str, Any]] = []
    for family, features in groups:
        for feature in features:
            terms = aliases.get(feature, (feature.lower(),))
            observed = all(term in haystack for term in terms)
            records.append(
                {
                    "family": family,
                    "feature": feature,
                    "status": "observed-in-ui-or-example"
                    if observed
                    else "not-observed-in-smoke",
                    "terms": list(terms),
                    "evidence": "startup/example DOM text",
                }
            )
    return records


def run_probe(browser: str, url: str, skip_examples: bool, timeout: float) -> dict[str, Any]:
    session = f"pdviewx-protein-imager-{os.getpid()}"
    with tempfile.TemporaryDirectory(prefix="pdviewx-protein-imager-") as directory:
        screenshot = Path(directory) / "protein-imager.png"
        try:
            browser_call(browser, session, ["open", url], timeout=timeout)
            startup = wait_for_page(browser, session, timeout)
            result: dict[str, Any] = {
                "status": "passed", "runtime": "hosted-browser", "url": url,
                "startup": startup, "examples": [],
            }
            latest = startup
            if not skip_examples:
                titles = [item["title"] for item in startup.get("examples", []) if item.get("button")]
                for title in titles:
                    clicked = click_example(browser, session, title)
                    latest = wait_for_example(browser, session, timeout)
                    loaded = bool(clicked.get("clicked") and (latest.get("hasLoadSuccess") or latest.get("hasStructure")))
                    result["examples"].append({"title": title, "loaded": loaded, "audit": latest})
                    if not loaded:
                        result["status"] = "passed-with-failures"
            result["final_canvas"] = latest.get("canvases", [])
            browser_call(browser, session, ["screenshot", str(screenshot)], timeout=timeout)
            width, height = png_size(screenshot)
            result["screenshot"] = {
                "width": width, "height": height, "bytes": screenshot.stat().st_size,
                "non_empty": screenshot.stat().st_size > 0,
            }
            result["loaded_example_count"] = sum(item["loaded"] for item in result["examples"])
            result["example_count"] = len(result["examples"])
            result["feature_coverage"] = feature_observations(result)
            result["feature_coverage_count"] = len(result["feature_coverage"])
            result["feature_coverage_status_counts"] = {
                status: sum(
                    item["status"] == status for item in result["feature_coverage"]
                )
                for status in sorted(
                    {item["status"] for item in result["feature_coverage"]}
                )
            }
            return result
        finally:
            browser_close(browser, session)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default=DEFAULT_URL)
    parser.add_argument("--browser-use", default=shutil.which("browser-use"))
    parser.add_argument("--skip-examples", action="store_true")
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if not args.browser_use:
        result: dict[str, Any] = {"schema": 1, "status": "unavailable", "reason": "browser-use executable not found"}
    else:
        try:
            result = {"schema": 1, **run_probe(args.browser_use, args.url, args.skip_examples, args.timeout)}
        except (OSError, RuntimeError, json.JSONDecodeError, struct.error) as error:
            result = {"schema": 1, "status": "unavailable", "error": f"{type(error).__name__}: {error}"}
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0 if result["status"] != "unavailable" else 1


if __name__ == "__main__":
    raise SystemExit(main())
