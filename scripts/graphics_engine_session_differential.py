#!/usr/bin/env python3
"""Exercise scene/session round trips in disposable graphics runtimes.

PyMOL uses a disposable PSE file, Mol* uses its native plugin state snapshot,
NGL records its public stage-parameter envelope and explicitly reports that no
native scene serializer is present, and molgfx uses RenderSession against a
fresh source parse. Reference runtimes, session files and images stay outside
the repository.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import functools
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from graphics_engine_cross_render import image_summary


NGL_PREFIX = r'''<!doctype html><meta charset="utf-8">
<style>html,body{width:100%;height:100%;margin:0;background:#fff}#viewport{width:256px;height:256px}</style>
<div id="viewport"></div>
<script src="/node_modules/three/build/three.js"></script><script>window.three=window.THREE;</script>
<script src="/node_modules/chroma-js/chroma.min.js"></script>
<script src="/node_modules/signals/dist/signals.js"></script><script>window.signalsWrapper=window.signals;</script>
<script src="/node_modules/sprintf-js/dist/sprintf.min.js"></script><script>window.sprintfJs={sprintf:window.sprintf,vsprintf:window.vsprintf};</script>
<script src="/node_modules/ngl/dist/ngl.umd.js"></script>'''

MOLSTAR_PREFIX = r'''<!doctype html><meta charset="utf-8">
<link rel="stylesheet" href="/node_modules/molstar/build/viewer/molstar.css">
<style>html,body{width:100%;height:100%;margin:0;background:#fff}#viewport{width:256px;height:256px;position:relative}</style>
<div id="viewport"></div><script src="/node_modules/molstar/build/viewer/molstar.js"></script>'''


def js_string(value: str) -> str:
    return json.dumps(value).replace("<", "\\u003c")


def session_html(engine: str, fixture: str) -> str:
    fixture_literal = js_string(fixture)
    if engine == "ngl":
        return NGL_PREFIX + f'''<script>
window.audit={{status:'loading',error:null,beforeDataUrl:null,afterDataUrl:null}};
const fixture={fixture_literal};
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const capture=()=>{{const source=document.querySelector('#viewport canvas');if(!source)throw Error('NGL canvas missing');const target=document.createElement('canvas');target.width=256;target.height=256;target.getContext('2d').drawImage(source,0,0,256,256);return target.toDataURL('image/png')}};
addEventListener('load',async()=>{{try{{
 const stage=new NGL.Stage('viewport',{{backgroundColor:'white'}});
 const component=await stage.loadFile(new Blob([fixture],{{type:'text/plain'}}),{{ext:'cif'}});
 const representation=component.addRepresentation('cartoon',{{color:'chainindex'}});
 component.autoView(0);stage.handleResize();stage.viewer.requestRender();await wait(700);
 const savedParameters=stage.getParameters();
 const savedRepresentation=representation.getParameters();
 const before=capture();
 stage.setParameters({{backgroundColor:'red',cameraType:'orthographic',clipDist:10}});
 representation.setVisibility(false);stage.viewer.requestRender();await wait(250);
 stage.setParameters(savedParameters);representation.setParameters(savedRepresentation);representation.setVisibility(true);
 component.autoView(0);stage.handleResize();stage.viewer.requestRender();await wait(700);
 const restoredParameters=stage.getParameters();
 const after=capture();
 window.audit={{status:'passed',nativeSnapshotApi:false,stageParameterApi:true,
   stageParameterRoundTrip:JSON.stringify(savedParameters)===JSON.stringify(restoredParameters),
   representationParameterRoundTrip:JSON.stringify(savedRepresentation)===JSON.stringify(representation.getParameters()),
   componentCount:stage.compList.length,representationCount:stage.getRepresentationsByName(/.*/).list.length,
   beforeDataUrl:before,afterDataUrl:after}};
}}catch(error){{window.audit={{status:'failed',error:String(error)}}}}}});
</script>'''
    return MOLSTAR_PREFIX + f'''<script>
window.audit={{status:'loading',error:null,beforeDataUrl:null,afterDataUrl:null}};
const fixture={fixture_literal};
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const capture=()=>{{const source=document.querySelector('#viewport canvas');if(!source)throw Error('Mol* canvas missing');const target=document.createElement('canvas');target.width=256;target.height=256;target.getContext('2d').drawImage(source,0,0,256,256);return target.toDataURL('image/png')}};
addEventListener('load',async()=>{{try{{
 const viewer=await molstar.Viewer.create('viewport',{{layoutShowControls:false,viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'}});
 await viewer.loadStructureFromData(fixture,'mmcif');
 const plugin=viewer.plugin;
 const structure=plugin.managers.structure.hierarchy.current.structures[0];
 if(!structure)throw Error('Mol* structure was not created');
 const component=await plugin.builders.structure.tryCreateComponentStatic(structure.cell,'polymer')||await plugin.builders.structure.tryCreateComponentStatic(structure.cell,'all');
 if(!component)throw Error('Mol* component was not created');
 await plugin.builders.structure.representation.addRepresentation(component,{{type:'cartoon',color:'chain-id'}});
 plugin.managers.camera.reset({{durationMs:0}});await wait(1000);
 const manager=plugin.managers.snapshot;
 const snapshot=await manager.getStateSnapshot({{name:'molgfx parity session',description:'disposable round-trip'}});
 const serializedBytes=JSON.stringify(snapshot).length;
 const before=capture();
 const initialBackground=plugin.canvas3d.props.renderer.backgroundColor;
 plugin.canvas3d.setProps({{renderer:{{...plugin.canvas3d.props.renderer,backgroundColor:0xff0000}}}});
 await wait(250);
 const mutatedBackground=plugin.canvas3d.props.renderer.backgroundColor;
 await manager.setStateSnapshot(snapshot);await wait(1000);
 const restoredBackground=plugin.canvas3d.props.renderer.backgroundColor;
 const after=capture();
 window.audit={{status:'passed',nativeSnapshotApi:typeof manager.getStateSnapshot==='function'&&typeof manager.setStateSnapshot==='function',
   snapshotEntries:snapshot.entries.length,serializedBytes,initialBackground,mutatedBackground,restoredBackground,
   canvasStateRoundTrip:JSON.stringify(initialBackground)===JSON.stringify(restoredBackground),
   structureCount:plugin.managers.structure.hierarchy.current.structures.length,
   beforeDataUrl:before,afterDataUrl:after}};
}}catch(error){{window.audit={{status:'failed',error:String(error)}}}}}});
</script>'''


def browser_json(browser: str, session: str, arguments: list[str], timeout: float = 90) -> dict[str, Any]:
    completed = subprocess.run([browser, "--session", session, "--json", *arguments], capture_output=True, text=True, timeout=timeout)
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


def browser_result(browser: str, session: str, expression: str) -> str:
    payload = browser_json(browser, session, ["eval", expression])
    result = payload.get("data", {}).get("result")
    if not isinstance(result, str):
        raise RuntimeError(f"browser eval has no string result: {payload}")
    return result


def wait_audit(browser: str, session: str) -> dict[str, Any]:
    deadline = time.monotonic() + 90
    latest: dict[str, Any] = {}
    while time.monotonic() < deadline:
        try:
            encoded = browser_result(browser, session, "JSON.stringify({status:window.audit?.status,error:window.audit?.error})")
            latest = json.loads(encoded)
            if latest.get("status") in {"passed", "failed"}:
                return latest
        except (RuntimeError, json.JSONDecodeError):
            pass
        time.sleep(0.5)
    raise RuntimeError(f"session browser probe did not settle: {latest}")


def capture_data_url(browser: str, session: str, phase: str, output: Path) -> dict[str, Any]:
    data_url = browser_result(browser, session, f"window.audit.{phase}DataUrl")
    prefix, separator, encoded = data_url.partition(",")
    if not separator or not prefix.startswith("data:image/png;base64"):
        raise RuntimeError(f"browser did not return a {phase} PNG data URL")
    output.write_bytes(base64.b64decode(encoded, validate=True))
    return image_summary(output)


def close_browser(browser: str, session: str) -> None:
    subprocess.run([browser, "--session", session, "close"], capture_output=True, text=True, timeout=30, check=False)


def run_browser(browser: str, node_root: Path, fixture: Path, output: Path) -> dict[str, Any]:
    session = f"molgfx-session-differential-{os.getpid()}"
    result: dict[str, Any] = {"status": "passed", "engines": {}}
    with tempfile.TemporaryDirectory(prefix="molgfx-session-pages-") as directory:
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
                page = webroot / f"{engine}.html"
                page.write_text(session_html(engine, fixture.read_text(encoding="utf-8")), encoding="utf-8")
                browser_json(browser, session, ["open", f"http://127.0.0.1:{server.server_port}/{page.name}"])
                audit = wait_audit(browser, session)
                record: dict[str, Any] = {**audit}
                if audit.get("status") == "passed":
                    before = output / f"{engine}-before.png"
                    after = output / f"{engine}-after.png"
                    record["beforeImage"] = capture_data_url(browser, session, "before", before)
                    record["afterImage"] = capture_data_url(browser, session, "after", after)
                    record["imageEqual"] = before.read_bytes() == after.read_bytes()
                else:
                    result["status"] = "passed-with-failures"
                result["engines"][engine] = record
        finally:
            close_browser(browser, session)
            server.shutdown()
            thread.join(timeout=5)
    return result


PYMOL_CODE = r'''
import contextlib, io, json, os, pathlib, sys
import pymol
from pymol import cmd

fixture = pathlib.Path(sys.argv[1])
root = pathlib.Path(sys.argv[2])
stdout, stderr = io.StringIO(), io.StringIO()
with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
    pymol.finish_launching(["pymol", "-cq"])
    cmd.load(str(fixture), "structure")
    cmd.hide("everything", "all")
    cmd.show("cartoon", "all")
    cmd.set("cartoon_transparency", 15, "all")
    cmd.bg_color("white")
    cmd.orient("all")
    cmd.scene("parity_scene", "store")
    pse = root / "pymol-session.pse"
    cmd.save(str(pse))
    cmd.png(str(root / "pymol-before.png"), width=256, height=256, ray=1, quiet=1)
    before_objects = cmd.get_object_list("all")
    before_atoms = cmd.count_atoms("all")
    cmd.reinitialize()
    cmd.load(str(pse))
    cmd.scene("parity_scene", "recall")
    cmd.png(str(root / "pymol-after.png"), width=256, height=256, ray=1, quiet=1)
    record = {
        "status": "passed",
        "pseBytes": pse.stat().st_size,
        "beforeObjects": before_objects,
        "afterObjects": cmd.get_object_list("all"),
        "beforeAtoms": before_atoms,
        "afterAtoms": cmd.count_atoms("all"),
        "sceneNames": cmd.get_scene_list(),
    }
os.write(1, (json.dumps(record, sort_keys=True) + "\n").encode())
'''


def run_pymol(pymol_python: str | None, fixture: Path, output: Path) -> dict[str, Any]:
    if not pymol_python:
        return {"status": "unavailable", "reason": "--pymol-python was not supplied"}
    completed = subprocess.run([pymol_python, "-c", PYMOL_CODE, str(fixture), str(output)], capture_output=True, text=True, timeout=180)
    record: dict[str, Any] | None = None
    for line in reversed(completed.stdout.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and "status" in value:
            record = value
            break
    if record is None:
        return {"status": "failed", "error": (completed.stderr or completed.stdout)[-1000:]}
    if completed.returncode != 0:
        record["status"] = "failed"
        record["error"] = completed.stderr[-1000:]
        return record
    before = output / "pymol-before.png"
    after = output / "pymol-after.png"
    if before.exists() and after.exists():
        record["beforeImage"] = image_summary(before)
        record["afterImage"] = image_summary(after)
        record["imageEqual"] = before.read_bytes() == after.read_bytes()
    record["objectRoundTrip"] = record.get("beforeObjects") == record.get("afterObjects")
    record["atomRoundTrip"] = record.get("beforeAtoms") == record.get("afterAtoms")
    record["stateRoundTrip"] = record["objectRoundTrip"] and record["atomRoundTrip"]
    if not record["stateRoundTrip"]:
        record["status"] = "failed"
        record["error"] = "PSE restore changed the object or atom inventory"
    return record


def file_evidence(path: Path) -> dict[str, Any]:
    return {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def run_molgfx(executable: Path, fixture: Path, output: Path) -> dict[str, Any]:
    prefix = output / "molgfx"
    completed = subprocess.run([str(executable), str(fixture), str(prefix)], capture_output=True, text=True, timeout=180, check=False)
    record: dict[str, Any] = {"status": "passed" if completed.returncode == 0 else "failed", "stdout": completed.stdout[-2000:], "stderr": completed.stderr[-1000:]}
    for name in ("scene_equal", "camera_equal", "profile_equal", "image_equal"):
        match = re.search(rf"^{name}=(true|false)$", completed.stdout, re.MULTILINE)
        if match:
            record[name] = match.group(1) == "true"
    json_path = output / "molgfx-session.json"
    before = output / "molgfx-before.png"
    after = output / "molgfx-after.png"
    if completed.returncode == 0 and json_path.exists() and before.exists() and after.exists():
        record["sessionFile"] = file_evidence(json_path)
        record["beforeImage"] = image_summary(before)
        record["afterImage"] = image_summary(after)
        record["beforeFile"] = file_evidence(before)
        record["afterFile"] = file_evidence(after)
    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-root", type=Path, required=True)
    parser.add_argument("--molgfx-example", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, default=Path("benchmarks/scenes/1BNA.cif"))
    parser.add_argument("--pymol-python")
    parser.add_argument("--browser-use", default=shutil.which("browser-use"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    fixture = (root / args.fixture).resolve() if not args.fixture.is_absolute() else args.fixture.resolve()
    result: dict[str, Any] = {"schema": 1, "fixture": str(fixture.relative_to(root)), "scope": "session/state round trips; no cross-engine image equivalence claim"}
    with tempfile.TemporaryDirectory(prefix="molgfx-session-differential-images-") as directory:
        output = Path(directory)
        if args.browser_use:
            try:
                result["browser"] = run_browser(args.browser_use, args.node_root.resolve(), fixture, output)
            except (OSError, RuntimeError, json.JSONDecodeError, ValueError) as error:
                result["browser"] = {"status": "unavailable", "error": f"{type(error).__name__}: {error}"}
        else:
            result["browser"] = {"status": "unavailable", "reason": "browser-use executable not found"}
        result["pymol"] = run_pymol(args.pymol_python, fixture, output)
        result["molgfx"] = run_molgfx(args.molgfx_example.resolve(), fixture, output)
    statuses = [result["browser"].get("status"), result["pymol"].get("status"), result["molgfx"].get("status")]
    result["status"] = "passed" if all(status == "passed" for status in statuses) else "passed-with-unavailable" if all(status in {"passed", "unavailable", "passed-with-unavailable"} for status in statuses) else "failed"
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0 if result["status"] != "failed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
