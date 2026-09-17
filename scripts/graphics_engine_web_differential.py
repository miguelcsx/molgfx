#!/usr/bin/env python3
"""Capture one shared scene per representation through NGL, Mol* and molgfx.

NGL and Mol* are browser runtimes, so the reference images are captured from
their actual WebGL canvas rather than from a DOM screenshot.  The npm tree,
browser pages and PNGs live in disposable directories.  The output is a
diagnostic corpus: it records which representation calls rendered and image
geometry, but does not claim pixel equivalence across engines.
"""

from __future__ import annotations

import argparse
import base64
import functools
import json
import os
import shutil
import subprocess
import tempfile
import threading
import time
from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from graphics_engine_cross_render import RenderCase, image_summary, render_molgfx, structural_comparison


@dataclass(frozen=True)
class WebCase:
    name: str
    fixture: str
    ngl: str | None
    molstar: str | None
    molgfx: str | None
    molgfx_mode: str = "realtime"


CASES = (
    WebCase("spacefill", "1BNA.cif", "spacefill", "spacefill", "spacefill"),
    WebCase("ball-and-stick", "1BNA.cif", "ball+stick", "ball-and-stick", "ball-and-stick"),
    WebCase("licorice", "1BNA.cif", "licorice", None, "licorice"),
    WebCase("line", "1BNA.cif", "line", "line", "lines"),
    WebCase("point", "1BNA.cif", "point", "point", "points"),
    WebCase("cartoon", "1BNA.cif", "cartoon", "cartoon", "cartoon"),
    WebCase("backbone", "1BNA.cif", "backbone", "backbone", "trace"),
    WebCase("ribbon", "1BNA.cif", "ribbon", "ribbon", "cartoon"),
    WebCase("trace", "1BNA.cif", "trace", "trace", "trace"),
    WebCase("tube", "1BNA.cif", "tube", "tube", "tube"),
    WebCase("surface", "1ubq.cif", "surface", "molecular-surface", "sas"),
    WebCase("contact", "1BNA.cif", "contact", None, None),
    WebCase("distance", "1BNA.cif", "distance", None, None),
    WebCase("angle", "1BNA.cif", "angle", None, None),
    WebCase("dihedral", "1BNA.cif", "dihedral", None, None),
    WebCase("label", "1BNA.cif", "label", "label", None),
    WebCase("unitcell", "1BNA.cif", "unitcell", None, None),
    WebCase("validation", "1BNA.cif", "validation", None, None),
    WebCase("putty", "1BNA.cif", None, "putty", None),
    WebCase("gaussian-surface", "1ubq.cif", None, "gaussian-surface", None),
    WebCase("ellipsoid", "1BNA.cif", None, "ellipsoid", None),
    WebCase("orientation", "1BNA.cif", None, "orientation", None),
)


NGL_PREFIX = r"""<!doctype html><meta charset="utf-8">
<style>html,body{width:100%;height:100%;margin:0;background:#fff}#viewport{width:256px;height:256px}</style>
<div id="viewport"></div>
<script src="/node_modules/three/build/three.js"></script><script>window.three=window.THREE;</script>
<script src="/node_modules/chroma-js/chroma.min.js"></script>
<script src="/node_modules/signals/dist/signals.js"></script><script>window.signalsWrapper=window.signals;</script>
<script src="/node_modules/sprintf-js/dist/sprintf.min.js"></script><script>window.sprintfJs={sprintf:window.sprintf,vsprintf:window.vsprintf};</script>
<script src="/node_modules/ngl/dist/ngl.umd.js"></script>
"""
MOLSTAR_PREFIX = r"""<!doctype html><meta charset="utf-8">
<link rel="stylesheet" href="/node_modules/molstar/build/viewer/molstar.css">
<style>html,body{width:100%;height:100%;margin:0;background:#fff}#viewport{width:256px;height:256px;position:relative}</style>
<div id="viewport"></div><script src="/node_modules/molstar/build/viewer/molstar.js"></script>
"""


def javascript_string(value: str) -> str:
    return json.dumps(value).replace("<", "\\u003c")


def html_for(engine: str, fixture: str, representation: str) -> str:
    fixture_literal = javascript_string(fixture)
    representation_literal = javascript_string(representation)
    if engine == "ngl":
        body = f"""
<script>
window.audit={{status:'loading',dataUrl:null,error:null}};
const fixture={fixture_literal}, representation={representation_literal};
addEventListener('load', async () => {{ try {{
  const stage=new NGL.Stage('viewport',{{backgroundColor:'white'}});
  const component=await stage.loadFile(new Blob([fixture],{{type:'text/plain'}}),{{ext:'cif'}});
  const options=representation==='label'?{{labelType:'atomname',color:'black'}}:{{}};
  component.addRepresentation(representation, options);
  component.autoView(); stage.handleResize(); stage.viewer.requestRender();
  await new Promise(resolve=>setTimeout(resolve,700));
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('NGL canvas was not created');
  const capture=document.createElement('canvas'); capture.width=256; capture.height=256;
  capture.getContext('2d').drawImage(canvas,0,0,256,256);
  window.audit={{status:'passed',width:canvas.width,height:canvas.height,
    captureWidth:256,captureHeight:256,atoms:component.structureView.atomCount,
    dataUrl:capture.toDataURL('image/png')}};
}} catch(error) {{ window.audit={{status:'failed',error:String(error)}}; }} }});
</script>"""
        return NGL_PREFIX + body
    body = f"""
<script>
window.audit={{status:'loading',dataUrl:null,error:null}};
const fixture={fixture_literal}, representation={representation_literal};
addEventListener('load', async () => {{ try {{
  const viewer=await molstar.Viewer.create('viewport',{{layoutShowControls:false,
    viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'}});
  await viewer.loadStructureFromData(fixture,'mmcif');
  const structure=viewer.plugin.managers.structure.hierarchy.current.structures[0];
  if(!structure) throw Error('Mol* structure was not created');
  const component=await viewer.plugin.builders.structure.tryCreateComponentStatic(structure.cell,'polymer')
    || await viewer.plugin.builders.structure.tryCreateComponentStatic(structure.cell,'all');
  if(!component) throw Error('Mol* component was not created');
  await viewer.plugin.builders.structure.representation.addRepresentation(component,{{type:representation,color:'chain-id'}});
  viewer.plugin.managers.camera.reset({{durationMs:0}});
  await new Promise(resolve=>setTimeout(resolve,1200));
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('Mol* canvas was not created');
  const capture=document.createElement('canvas'); capture.width=256; capture.height=256;
  capture.getContext('2d').drawImage(canvas,0,0,256,256);
  window.audit={{status:'passed',width:canvas.width,height:canvas.height,
    captureWidth:256,captureHeight:256,atoms:structure.cell.obj.data.elementCount||0,
    dataUrl:capture.toDataURL('image/png')}};
}} catch(error) {{ window.audit={{status:'failed',error:String(error)}}; }} }});
</script>"""
    return MOLSTAR_PREFIX + body


def browser_json(executable: str, session: str, arguments: list[str], timeout: float = 90) -> dict[str, Any]:
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


def browser_result(executable: str, session: str, expression: str) -> str:
    payload = browser_json(executable, session, ["eval", expression])
    result = payload.get("data", {}).get("result")
    if not isinstance(result, str):
        raise RuntimeError(f"browser eval has no string result: {payload}")
    return result


def wait_audit(executable: str, session: str, timeout: float = 90) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    latest: dict[str, Any] = {}
    while time.monotonic() < deadline:
        try:
            state = browser_result(
                executable,
                session,
                "JSON.stringify({status:window.audit?.status,error:window.audit?.error,width:window.audit?.width,height:window.audit?.height,captureWidth:window.audit?.captureWidth,captureHeight:window.audit?.captureHeight,atoms:window.audit?.atoms})",
            )
            latest = json.loads(state)
            if latest.get("status") in {"passed", "failed"}:
                return latest
        except (RuntimeError, json.JSONDecodeError):
            pass
        time.sleep(0.5)
    raise RuntimeError(f"browser representation did not settle: {latest}")


def capture_data_url(executable: str, session: str, output: Path) -> dict[str, Any]:
    data_url = browser_result(executable, session, "window.audit.dataUrl")
    prefix, separator, encoded = data_url.partition(",")
    if not separator or not prefix.startswith("data:image/png;base64"):
        raise RuntimeError("browser did not return a PNG data URL")
    output.write_bytes(base64.b64decode(encoded, validate=True))
    return image_summary(output)


def close_browser(executable: str, session: str) -> None:
    subprocess.run(
        [executable, "--session", session, "close"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )


def run_browser_cases(browser: str, node_root: Path, root: Path, selected: list[WebCase], output: Path) -> dict[str, Any]:
    session = f"molgfx-web-differential-{os.getpid()}"
    result: dict[str, Any] = {"status": "passed", "engines": {"ngl": [], "molstar": []}}
    with tempfile.TemporaryDirectory(prefix="molgfx-web-differential-") as directory:
        webroot = Path(directory)
        (webroot / "node_modules").symlink_to(node_root / "node_modules", target_is_directory=True)

        class QuietHandler(SimpleHTTPRequestHandler):
            def log_message(self, _format: str, *_args: Any) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(QuietHandler, directory=str(webroot)))
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            for engine in ("ngl", "molstar"):
                for case in selected:
                    representation = getattr(case, engine)
                    if representation is None:
                        continue
                    fixture = root / "benchmarks/scenes" / case.fixture
                    page = webroot / f"{engine}.html"
                    page.write_text(html_for(engine, fixture.read_text(encoding="utf-8"), representation), encoding="utf-8")
                    browser_json(browser, session, ["open", f"http://127.0.0.1:{server.server_port}/{page.name}"])
                    audit = wait_audit(browser, session)
                    record: dict[str, Any] = {"name": case.name, "fixture": case.fixture, "representation": representation, "audit": audit}
                    if audit.get("status") == "passed":
                        image_path = output / f"{engine}-{case.name}.png"
                        record["image"] = capture_data_url(browser, session, image_path)
                    else:
                        result["status"] = "passed-with-failures"
                    result["engines"][engine].append(record)
        finally:
            close_browser(browser, session)
            server.shutdown()
            thread.join(timeout=5)
    return result


def add_molgfx_results(result: dict[str, Any], executable: Path, root: Path, selected: list[WebCase], output: Path) -> None:
    for case in selected:
        if case.molgfx is None:
            continue
        fixture = root / "benchmarks/scenes" / case.fixture
        render_case = RenderCase(case.name, case.fixture, case.molgfx, "", case.molgfx_mode)
        image_path = output / f"molgfx-{case.name}.png"
        molgfx = render_molgfx(executable, fixture, image_path, render_case, 256, 256, root)
        record: dict[str, Any] = {"name": case.name, "fixture": case.fixture, "representation": case.molgfx, "render": molgfx}
        if molgfx.get("status") == "passed":
            record["image"] = image_summary(image_path)
        result.setdefault("molgfx", []).append(record)
        for engine in ("ngl", "molstar"):
            reference = next((item for item in result["engines"][engine] if item["name"] == case.name), None)
            if reference and "image" in reference and "image" in record:
                reference.setdefault("comparison", structural_comparison(reference["image"], record["image"]))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-root", type=Path, required=True, help="disposable npm --prefix directory")
    parser.add_argument("--molgfx-example", type=Path)
    parser.add_argument("--browser-use", default=shutil.which("browser-use"))
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--case", action="append", choices=[case.name for case in CASES])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = args.repo.resolve()
    selected = [case for case in CASES if not args.case or case.name in set(args.case)]
    with tempfile.TemporaryDirectory(prefix="molgfx-web-differential-images-") as directory:
        image_dir = Path(directory)
        if not args.browser_use:
            result: dict[str, Any] = {"schema": 1, "status": "unavailable", "error": "browser-use executable not found"}
        else:
            try:
                result = {"schema": 1, "scope": "canvas image diagnostics, not pixel/scientific equivalence", **run_browser_cases(args.browser_use, args.node_root.resolve(), root, selected, image_dir)}
                if args.molgfx_example:
                    add_molgfx_results(result, args.molgfx_example.resolve(), root, selected, image_dir)
            except (OSError, RuntimeError, json.JSONDecodeError, ValueError) as error:
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
