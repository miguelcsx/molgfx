#!/usr/bin/env python3
"""Exercise a shared disposable MRC map through NGL, Mol* and molgfx.

The map is synthetic but encoded as a real little-endian MRC MODE 2 file. NGL
and Mol* consume that file in a browser; molgfx consumes the same file through
the extended volume_smoke example. Images are structural diagnostics, not a
claim of pixel or scientific equivalence because the engines use different
transfer, camera and volume algorithms.
"""

from __future__ import annotations

import argparse
import base64
import functools
import hashlib
import json
import os
import shutil
import struct
import subprocess
import tempfile
import threading
import time
from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from graphics_engine_cross_render import image_summary, structural_comparison


@dataclass(frozen=True)
class VolumeCase:
    name: str
    ngl: str | None
    molstar: str | None
    molgfx: str | None


CASES = (
    VolumeCase("isosurface", "surface", "isosurface", "isosurface"),
    VolumeCase("slice", "slice", "slice", "slice"),
    VolumeCase("dot", "dot", "dot", None),
    VolumeCase("direct-volume", None, "direct-volume", "direct"),
    VolumeCase("participating-medium", None, None, "medium"),
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


def html_for(engine: str, representation: str) -> str:
    representation_literal = javascript_string(representation)
    if engine == "ngl":
        body = f"""
<script>
window.audit={{status:'loading',dataUrl:null,error:null}};
const representation={representation_literal};
addEventListener('load', async () => {{ try {{
  const stage=new NGL.Stage('viewport',{{backgroundColor:'white'}});
  const component=await stage.loadFile('/fixture.mrc',{{ext:'mrc'}});
  if(component.type!=='volume') throw Error('NGL did not create a volume component');
  component.addRepresentation(representation,{{isolevel:{{type:'value',value:0.35}}}});
  component.autoView(); stage.handleResize(); stage.viewer.requestRender();
  await new Promise(resolve=>setTimeout(resolve,900));
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('NGL canvas was not created');
  const capture=document.createElement('canvas'); capture.width=256; capture.height=256;
  capture.getContext('2d').drawImage(canvas,0,0,256,256);
  window.audit={{status:'passed',width:canvas.width,height:canvas.height,
    captureWidth:256,captureHeight:256,type:component.type,
    dataUrl:capture.toDataURL('image/png')}};
}} catch(error) {{ window.audit={{status:'failed',error:String(error)}}; }} }});
</script>"""
        return NGL_PREFIX + body
    body = f"""
<script>
window.audit={{status:'loading',dataUrl:null,error:null}};
const representation={representation_literal};
addEventListener('load', async () => {{ try {{
  const viewer=await molstar.Viewer.create('viewport',{{layoutShowControls:false,
    viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'}});
  await viewer.loadVolumeFromUrl({{url:'/fixture.mrc',format:'ccp4',isBinary:true}},[]);
  const plugin=viewer.plugin, hierarchy=plugin.managers.volume.hierarchy;
  const volume=hierarchy.current.volumes[0];
  if(!volume) throw Error('Mol* volume was not created');
  const typeNames=plugin.representation.volume.registry.types.map(item=>typeof item==='string'?item:(item.name||item.label||String(item)));
  await hierarchy.addRepresentation(volume,representation);
  plugin.managers.camera.reset({{durationMs:0}});
  await new Promise(resolve=>setTimeout(resolve,1200));
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('Mol* canvas was not created');
  const capture=document.createElement('canvas'); capture.width=256; capture.height=256;
  capture.getContext('2d').drawImage(canvas,0,0,256,256);
  window.audit={{status:'passed',width:canvas.width,height:canvas.height,
    captureWidth:256,captureHeight:256,volumeTypes:typeNames,
    dataUrl:capture.toDataURL('image/png')}};
}} catch(error) {{ window.audit={{status:'failed',error:String(error)}}; }} }});
</script>"""
    return MOLSTAR_PREFIX + body


def write_mrc(path: Path, dimensions: tuple[int, int, int] = (64, 64, 64)) -> None:
    nx, ny, nz = dimensions
    spacing = 0.08
    center = (nx - 1) * 0.5
    origin = -center * spacing
    values: list[float] = []
    for z in range(nz):
        for y in range(ny):
            for x in range(nx):
                point = (
                    (x - center) / center,
                    (y - center) / center,
                    (z - center) / center,
                )
                value = gaussian(point, (-0.30, 0.02, 0.08), (0.46, 0.31, 0.38))
                value += 0.88 * gaussian(point, (0.34, 0.10, -0.08), (0.36, 0.43, 0.30))
                value += 0.52 * gaussian(point, (0.02, -0.40, 0.18), (0.22, 0.22, 0.22))
                value -= 0.46 * gaussian(point, (0.02, 0.02, 0.20), (0.17, 0.22, 0.18))
                values.append(max(value, 0.0))
    minimum, maximum = min(values), max(values)
    mean = sum(values) / len(values)
    variance = sum((value - mean) ** 2 for value in values) / len(values)
    header = bytearray(1024)
    struct.pack_into("<3i", header, 0, nx, ny, nz)
    struct.pack_into("<i", header, 12, 2)
    struct.pack_into("<3i", header, 28, nx, ny, nz)
    struct.pack_into("<3f", header, 40, nx * spacing, ny * spacing, nz * spacing)
    struct.pack_into("<3f", header, 52, 90.0, 90.0, 90.0)
    struct.pack_into("<3i", header, 64, 1, 2, 3)
    struct.pack_into("<3f", header, 76, minimum, maximum, mean)
    struct.pack_into("<2i", header, 88, 0, 0)
    struct.pack_into("<3f", header, 196, origin, origin, origin)
    header[208:212] = b"MAP "
    struct.pack_into("<I", header, 212, 0x44410000)
    struct.pack_into("<f", header, 216, variance**0.5)
    struct.pack_into("<i", header, 220, 0)
    path.write_bytes(header + struct.pack(f"<{len(values)}f", *values))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def gaussian(point: tuple[float, float, float], center: tuple[float, float, float], sigma: tuple[float, float, float]) -> float:
    normalized = [(point[index] - center[index]) / sigma[index] for index in range(3)]
    return pow(2.718281828459045, -0.5 * sum(value * value for value in normalized))


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
                "JSON.stringify({status:window.audit?.status,error:window.audit?.error,width:window.audit?.width,height:window.audit?.height,captureWidth:window.audit?.captureWidth,captureHeight:window.audit?.captureHeight,volumeTypes:window.audit?.volumeTypes,type:window.audit?.type})",
            )
            latest = json.loads(state)
            if latest.get("status") in {"passed", "failed"}:
                return latest
        except (RuntimeError, json.JSONDecodeError):
            pass
        time.sleep(0.5)
    raise RuntimeError(f"browser volume representation did not settle: {latest}")


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


def run_browser_cases(browser: str, node_root: Path, selected: list[VolumeCase], output: Path) -> dict[str, Any]:
    session = f"molgfx-volume-differential-{os.getpid()}"
    result: dict[str, Any] = {"status": "passed", "engines": {"ngl": [], "molstar": []}}
    with tempfile.TemporaryDirectory(prefix="molgfx-volume-differential-") as directory:
        webroot = Path(directory)
        (webroot / "node_modules").symlink_to(node_root / "node_modules", target_is_directory=True)
        write_mrc(webroot / "fixture.mrc")

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
                    page = webroot / f"{engine}.html"
                    page.write_text(html_for(engine, representation), encoding="utf-8")
                    record: dict[str, Any] = {"name": case.name, "representation": representation}
                    try:
                        browser_json(browser, session, ["open", f"http://127.0.0.1:{server.server_port}/{page.name}"])
                        audit = wait_audit(browser, session)
                        record["audit"] = audit
                        if audit.get("status") == "passed":
                            record["image"] = capture_data_url(browser, session, output / f"{engine}-{case.name}.png")
                        else:
                            result["status"] = "passed-with-failures"
                    except (OSError, RuntimeError, json.JSONDecodeError, ValueError) as error:
                        record["audit"] = {"status": "failed", "error": f"{type(error).__name__}: {error}"}
                        result["status"] = "passed-with-failures"
                    result["engines"][engine].append(record)
        finally:
            close_browser(browser, session)
            server.shutdown()
            thread.join(timeout=5)
    return result


def add_molgfx_results(result: dict[str, Any], executable: Path, mrc: Path, root: Path, selected: list[VolumeCase], output: Path) -> None:
    for case in selected:
        if case.molgfx is None:
            continue
        image_path = output / f"molgfx-{case.name}.png"
        completed = subprocess.run(
            [str(executable), str(image_path), case.molgfx, "probe", str(mrc)],
            capture_output=True,
            text=True,
            cwd=root,
            env={**os.environ, "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "metal")},
            timeout=120,
        )
        uses_external_map = "volume source: external MRC" in completed.stdout
        render_passed = completed.returncode == 0 and uses_external_map and image_path.exists()
        record: dict[str, Any] = {
            "name": case.name,
            "representation": case.molgfx,
            "render": {
                "status": "passed" if render_passed else "failed",
                "returncode": completed.returncode,
                "used_external_mrc": uses_external_map,
                "stdout": completed.stdout[-4000:],
                "stderr": completed.stderr[-4000:],
            },
        }
        if render_passed:
            record["image"] = image_summary(image_path)
        else:
            result["status"] = "passed-with-failures"
        result.setdefault("molgfx", []).append(record)
        for engine in ("ngl", "molstar"):
            reference = next((item for item in result["engines"][engine] if item["name"] == case.name), None)
            if reference and "image" in reference and "image" in record:
                reference["comparison"] = structural_comparison(reference["image"], record["image"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-root", type=Path, required=True, help="disposable npm --prefix directory")
    parser.add_argument("--molgfx-example", type=Path, required=True)
    parser.add_argument("--browser-use", default=shutil.which("browser-use"))
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--case", action="append", choices=[case.name for case in CASES])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = args.repo.resolve()
    selected = [case for case in CASES if not args.case or case.name in set(args.case)]
    with tempfile.TemporaryDirectory(prefix="molgfx-volume-differential-images-") as directory:
        image_dir = Path(directory)
        with tempfile.TemporaryDirectory(prefix="molgfx-volume-differential-map-") as map_directory:
            mrc = Path(map_directory) / "fixture.mrc"
            write_mrc(mrc)
            if not args.browser_use:
                result: dict[str, Any] = {"schema": 1, "status": "unavailable", "error": "browser-use executable not found"}
            else:
                result = {"schema": 1, "scope": "shared MRC volume diagnostics, not pixel/scientific equivalence", "fixture": {"format": "MRC MODE 2", "dimensions": [64, 64, 64], "sha256": sha256(mrc)}, **run_browser_cases(args.browser_use, args.node_root.resolve(), selected, image_dir)}
                add_molgfx_results(result, args.molgfx_example.resolve(), mrc, root, selected, image_dir)
        encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(encoded, encoding="utf-8")
        else:
            print(encoded, end="")
    return 0 if result["status"] != "unavailable" else 1


if __name__ == "__main__":
    raise SystemExit(main())
