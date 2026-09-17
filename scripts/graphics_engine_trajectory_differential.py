#!/usr/bin/env python3
"""Exercise one two-frame structure through NGL, Mol* and molgfx.

The PDB fixture and all PNGs are disposable.  The probe records frame counts,
the actual frame mutation API and conservative image diagnostics; it does not
claim pixel equivalence or movie-encoder parity.
"""

from __future__ import annotations

import argparse
import base64
import functools
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from graphics_engine_cross_render import image_summary, structural_comparison


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


def html_for(engine: str, fixture: str) -> str:
    fixture_literal = javascript_string(fixture)
    if engine == "ngl":
        body = f"""
<script>
window.audit={{status:'loading',error:null,frames:[]}};
const fixture={fixture_literal};
function wait(ms) {{ return new Promise(resolve => setTimeout(resolve, ms)); }}
function setFrame(trajectory, index) {{
  return new Promise((resolve, reject) => {{
    const timer=setTimeout(() => reject(Error('NGL frame callback timed out')), 5000);
    try {{ trajectory.setFrame(index, () => {{ clearTimeout(timer); resolve(); }}); }}
    catch(error) {{ clearTimeout(timer); reject(error); }}
  }});
}}
function capture(canvas) {{
  const capture=document.createElement('canvas'); capture.width=256; capture.height=256;
  capture.getContext('2d').drawImage(canvas,0,0,256,256); return capture.toDataURL('image/png');
}}
addEventListener('load', async () => {{ try {{
  const stage=new NGL.Stage('viewport',{{backgroundColor:'white'}});
  const component=await stage.loadFile(new Blob([fixture],{{type:'text/plain'}}),{{ext:'pdb',asTrajectory:true}});
  const element=component.trajList && component.trajList[0] || component.addTrajectory();
  const trajectory=element && element.trajectory;
  if(!trajectory) throw Error('NGL did not create a trajectory element');
  component.addRepresentation('ball+stick',{{color:'element'}});
  component.autoView(); stage.handleResize(); stage.viewer.requestRender(); await wait(700);
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('NGL canvas was not created');
  const frames=[];
  for(const index of [0,1]) {{
    await setFrame(trajectory,index); stage.viewer.requestRender(); await wait(500);
    frames.push({{index,currentFrame:trajectory.currentFrame,dataUrl:capture(canvas)}});
  }}
  window.audit={{status:'passed',frameCount:trajectory.frameCount,atomCount:trajectory.atomCount,
    frames:frames.map(frame=>({{index:frame.index,currentFrame:frame.currentFrame}})),
    dataUrls:frames.map(frame=>frame.dataUrl)}};
}} catch(error) {{ window.audit={{status:'failed',error:String(error)}}; }} }});
</script>"""
        return NGL_PREFIX + body
    body = f"""
<script>
window.audit={{status:'loading',error:null,frames:[]}};
const fixture={fixture_literal};
function wait(ms) {{ return new Promise(resolve => setTimeout(resolve, ms)); }}
function capture(canvas) {{
  const capture=document.createElement('canvas'); capture.width=256; capture.height=256;
  capture.getContext('2d').drawImage(canvas,0,0,256,256); return capture.toDataURL('image/png');
}}
async function setModel(plugin, modelCell, index) {{
  const update=plugin.state.data.build().to(modelCell).update({{modelIndex:index}});
  await plugin.runTask(plugin.state.data.updateTree(update)); await wait(700);
  const cell=plugin.state.data.cells.get(modelCell.transform.ref);
  return cell && cell.transform.params.modelIndex;
}}
addEventListener('load', async () => {{ try {{
  const viewer=await molstar.Viewer.create('viewport',{{layoutShowControls:false,
    viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'}});
  await viewer.loadStructureFromData(fixture,'pdb');
  const plugin=viewer.plugin;
  const structures=plugin.managers.structure.hierarchy.current.structures;
  const structure=structures[0]; if(!structure) throw Error('Mol* structure was not created');
  const component=await plugin.builders.structure.tryCreateComponentStatic(structure.cell,'all');
  if(!component) throw Error('Mol* component was not created');
  await plugin.builders.structure.representation.addRepresentation(component,{{type:'ball-and-stick',color:'element-symbol'}});
  const cells=Array.from(plugin.state.data.cells.values());
  const modelCell=cells.find(cell=>cell.transform && cell.transform.transformer &&
    String(cell.transform.transformer.name||cell.transform.transformer.id||'').includes('model-from-trajectory'));
  const trajectoryCell=cells.find(cell=>cell.obj && cell.obj.data && cell.obj.data.frameCount>1);
  if(!modelCell || !trajectoryCell) throw Error('Mol* trajectory/model state was not created: '+JSON.stringify(cells.map(cell=>({{ref:cell.transform&&cell.transform.ref,transformer:cell.transform&&cell.transform.transformer&&(cell.transform.transformer.name||cell.transform.transformer.id),object:cell.obj&&cell.obj.type&&cell.obj.type.name,frameCount:cell.obj&&cell.obj.data&&cell.obj.data.frameCount,params:cell.transform&&cell.transform.params}}))));
  const animation=plugin.managers.animation.animations.find(item=>item.name==='built-in.animate-model-index');
  const animationCanApply=animation ? animation.canApply({{state:{{data:plugin.state.data}}}}) : null;
  plugin.managers.camera.reset({{durationMs:0}}); await wait(900);
  const canvas=document.querySelector('#viewport canvas'); if(!canvas) throw Error('Mol* canvas was not created');
  const frames=[];
  for(const index of [0,1]) {{
    const modelIndex=await setModel(plugin,modelCell,index); await wait(500);
    frames.push({{index,modelIndex,dataUrl:capture(canvas)}});
  }}
  window.audit={{status:'passed',frameCount:trajectoryCell.obj.data.frameCount,
    animationApi:!!animation,animationCanApply,frames:frames.map(frame=>({{index:frame.index,modelIndex:frame.modelIndex}})),
    dataUrls:frames.map(frame=>frame.dataUrl)}};
}} catch(error) {{ window.audit={{status:'failed',error:String(error)}}; }} }});
</script>"""
    return MOLSTAR_PREFIX + body


def atom_line(serial: int, name: str, element: str, position: tuple[float, float, float]) -> str:
    x, y, z = position
    return f"ATOM  {{serial:5d}} {{name:^4s}} ALA A   1    {{x:8.3f}}{{y:8.3f}}{{z:8.3f}}  1.00 10.00          {{element:>2s}}".format(
        serial=serial, name=name, x=x, y=y, z=z, element=element
    )


def write_fixture(directory: Path) -> dict[str, Any]:
    start = [("N", "N", (0.000, 0.000, 0.000)), ("CA", "C", (1.450, 0.000, 0.000)),
             ("C", "C", (2.100, 1.300, 0.000)), ("O", "O", (3.300, 1.300, 0.000)),
             ("CB", "C", (1.450, -1.250, 0.000))]
    end = [(name, element, (x + dx, y + dy, z + dz)) for (name, element, (x, y, z)), (dx, dy, dz) in
           zip(start, [(0.0, 0.0, 0.0), (0.35, 0.95, 0.20), (0.70, 0.55, 0.25), (0.75, 0.50, 0.25), (0.15, 1.15, -0.15)])]

    def lines(atoms: list[tuple[str, str, tuple[float, float, float]]]) -> list[str]:
        return [atom_line(index, name, element, position) for index, (name, element, position) in enumerate(atoms, 1)]

    model = "\n".join(["MODEL        1", *lines(start), "ENDMDL", "MODEL        2", *lines(end), "ENDMDL", "END", ""])
    start_text = "\n".join([*lines(start), "END", ""])
    end_text = "\n".join([*lines(end), "END", ""])
    multi_path, start_path, end_path = directory / "trajectory.pdb", directory / "start.pdb", directory / "end.pdb"
    multi_path.write_text(model, encoding="utf-8"); start_path.write_text(start_text, encoding="utf-8"); end_path.write_text(end_text, encoding="utf-8")
    return {"multi_model": multi_path, "start": start_path, "end": end_path, "sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in (("multi_model", multi_path), ("start", start_path), ("end", end_path))}, "atom_count": len(start)}


def browser_json(executable: str, session: str, arguments: list[str], timeout: float = 90) -> dict[str, Any]:
    completed = subprocess.run([executable, "--session", session, "--json", *arguments], capture_output=True, text=True, timeout=timeout)
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
    deadline, latest = time.monotonic() + timeout, {}
    while time.monotonic() < deadline:
        try:
            latest = json.loads(browser_result(executable, session, "JSON.stringify(window.audit)"))
            if latest.get("status") in {"passed", "failed"}:
                return latest
        except (RuntimeError, json.JSONDecodeError):
            pass
        time.sleep(0.5)
    raise RuntimeError(f"browser trajectory did not settle: {latest}")


def capture_frame(executable: str, session: str, index: int, output: Path) -> dict[str, Any]:
    data_url = browser_result(executable, session, f"window.audit.dataUrls[{index}]")
    prefix, separator, encoded = data_url.partition(",")
    if not separator or not prefix.startswith("data:image/png;base64"):
        raise RuntimeError("browser did not return a frame PNG data URL")
    output.write_bytes(base64.b64decode(encoded, validate=True))
    return image_summary(output)


def close_browser(executable: str, session: str) -> None:
    subprocess.run([executable, "--session", session, "close"], capture_output=True, text=True, timeout=30, check=False)


def frame_change(first: Path, second: Path) -> dict[str, Any]:
    first_summary, second_summary = image_summary(first), image_summary(second)
    return {"different_png_bytes": hashlib.sha256(first.read_bytes()).digest() != hashlib.sha256(second.read_bytes()).digest(), "geometry": structural_comparison(first_summary, second_summary)}


def run_browsers(browser: str, node_root: Path, fixture: Path, output: Path) -> dict[str, Any]:
    session = f"molgfx-trajectory-differential-{os.getpid()}"
    result: dict[str, Any] = {"status": "passed", "engines": {"ngl": {}, "molstar": {}}}
    with tempfile.TemporaryDirectory(prefix="molgfx-trajectory-browser-") as directory:
        webroot = Path(directory); (webroot / "node_modules").symlink_to(node_root / "node_modules", target_is_directory=True)
        (webroot / "trajectory.pdb").write_text(fixture.read_text(encoding="utf-8"), encoding="utf-8")

        class QuietHandler(SimpleHTTPRequestHandler):
            def log_message(self, _format: str, *_args: Any) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(QuietHandler, directory=str(webroot)))
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        try:
            for engine in ("ngl", "molstar"):
                page = webroot / f"{engine}.html"; page.write_text(html_for(engine, fixture.read_text(encoding="utf-8")), encoding="utf-8")
                try:
                    browser_json(browser, session, ["open", f"http://127.0.0.1:{server.server_port}/{page.name}"])
                    audit = wait_audit(browser, session)
                    record: dict[str, Any] = {key: value for key, value in audit.items() if key != "dataUrls"}
                    if audit.get("status") == "passed":
                        for index in (0, 1):
                            record.setdefault("frames", [])[index]["image"] = capture_frame(browser, session, index, output / f"{engine}-frame-{index}.png")
                        record["frame_change"] = frame_change(output / f"{engine}-frame-0.png", output / f"{engine}-frame-1.png")
                    else:
                        result["status"] = "passed-with-failures"
                    result["engines"][engine] = record
                except (OSError, RuntimeError, json.JSONDecodeError, ValueError) as error:
                    result["status"] = "passed-with-failures"; result["engines"][engine] = {"status": "failed", "error": f"{type(error).__name__}: {error}"}
        finally:
            close_browser(browser, session); server.shutdown(); thread.join(timeout=5)
    return result


def run_molgfx(executable: Path, start: Path, end: Path, root: Path, output: Path) -> dict[str, Any]:
    prefix = output / "molgfx-trajectory"
    completed = subprocess.run([str(executable), str(start), str(end), str(prefix)], cwd=root, capture_output=True, text=True, env={**os.environ, "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "metal")}, timeout=120)
    paths = {label: Path(f"{prefix}-{label}.png") for label in ("start", "mid", "end")}
    passed = completed.returncode == 0 and all(path.exists() for path in paths.values())
    record: dict[str, Any] = {"status": "passed" if passed else "failed", "returncode": completed.returncode, "stdout": completed.stdout[-4000:], "stderr": completed.stderr[-4000:]}
    if passed:
        record["frames"] = {label: image_summary(path) for label, path in paths.items()}; record["frame_change"] = frame_change(paths["start"], paths["end"])
    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-root", type=Path, required=True, help="disposable npm --prefix directory")
    parser.add_argument("--molgfx-example", type=Path, required=True, help="built trajectory example executable")
    parser.add_argument("--browser-use", default=shutil.which("browser-use"))
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(); root = args.repo.resolve()
    with tempfile.TemporaryDirectory(prefix="molgfx-trajectory-differential-images-") as image_directory, tempfile.TemporaryDirectory(prefix="molgfx-trajectory-differential-fixture-") as fixture_directory:
        fixture = write_fixture(Path(fixture_directory))
        if not args.browser_use:
            result: dict[str, Any] = {"schema": 1, "status": "unavailable", "error": "browser-use executable not found"}
        else:
            result = {"schema": 1, "scope": "shared two-frame state/render diagnostics, not pixel/movie equivalence", "fixture": {"format": "PDB multi-model plus separate endpoints", "sha256": fixture["sha256"], "atom_count": fixture["atom_count"]}, **run_browsers(args.browser_use, args.node_root.resolve(), fixture["multi_model"], Path(image_directory))}
            result["molgfx"] = run_molgfx(args.molgfx_example.resolve(), fixture["start"], fixture["end"], root, Path(image_directory))
            if result["molgfx"].get("status") != "passed":
                result["status"] = "passed-with-failures"
        encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True); args.output.write_text(encoded, encoding="utf-8")
        else:
            print(encoded, end="")
    return 0 if result["status"] != "unavailable" else 1


if __name__ == "__main__":
    raise SystemExit(main())
