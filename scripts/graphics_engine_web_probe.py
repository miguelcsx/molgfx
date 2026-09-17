#!/usr/bin/env python3
"""Probe disposable browser runtimes for NGL and Mol*.

The package directory is caller-owned and is never installed into this
repository. The browser pass is optional because browser-use is a host tool,
not a molgfx dependency; when present it exercises the package's own browser
bundles against the same local PDB fixture.
"""

from __future__ import annotations

import argparse
import functools
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


NGL_HTML = r"""<!doctype html>
<meta charset="utf-8"><title>NGL disposable parity probe</title>
<style>html,body,#viewport{width:100%;height:100%;margin:0;background:#fff}</style>
<div id="viewport"></div>
<script>window.audit={status:'script-loading',probes:[],errors:[]};window.addEventListener('error',e=>window.audit.errors.push(String(e.error||e.message)));window.addEventListener('unhandledrejection',e=>window.audit.errors.push(String(e.reason)));</script>
<script src="/node_modules/three/build/three.js"></script><script>window.three=window.THREE;</script>
<script src="/node_modules/chroma-js/chroma.min.js"></script>
<script src="/node_modules/signals/dist/signals.js"></script><script>window.signalsWrapper=window.signals;</script>
<script src="/node_modules/sprintf-js/dist/sprintf.min.js"></script><script>window.sprintfJs={sprintf:window.sprintf,vsprintf:window.vsprintf};</script>
<script src="/node_modules/ngl/dist/ngl.umd.js"></script>
<script>
const pdb='ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N\nATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C\nATOM      3  C   ALA A   1       2.000   1.300   0.000  1.00 10.00           C\nATOM      4  O   ALA A   1       1.300   2.250   0.000  1.00 10.00           O\nATOM      5  N   GLY A   2       3.300   1.450   0.000  1.00 10.00           N\nATOM      6  CA  GLY A   2       4.000   2.650   0.000  1.00 10.00           C\nATOM      7  C   GLY A   2       5.450   2.350   0.000  1.00 10.00           C\nATOM      8  O   GLY A   2       6.050   3.350   0.000  1.00 10.00           O\nEND';
const probe=(name,fn)=>{try{const detail=fn();window.audit.probes.push({name,status:'passed',detail:String(detail??'')});}catch(e){window.audit.probes.push({name,status:'failed',error:String(e)});}};
const names=['spacefill','ball+stick','licorice','line','point','cartoon','backbone','ribbon','trace','tube','rocket','rope','surface','contact','distance','angle','dihedral','label','unitcell','validation'];
addEventListener('load',async()=>{try{
 const stage=new NGL.Stage('viewport',{backgroundColor:'white'});
 window.audit.nglExports=Object.keys(NGL).filter(k=>/Representation|Surface|Volume|Selection|Trajectory|Color|Component|Shape|Stage|Unitcell|Label|Distance|Angle|Dihedral|Validation/i.test(k)).sort();
 const component=await stage.loadFile(new Blob([pdb],{type:'text/plain'}),{ext:'pdb'});window.audit.atomCount=component.structureView.atomCount;
 for(const name of names)probe('representation:'+name,()=>{const r=component.addRepresentation(name,name==='label'?{labelType:'atomname',color:'black'}:{});r.setVisibility(false);return name;});
 probe('selection:backbone',()=>component.addRepresentation('line',{sele:'backbone'}).dispose());
 probe('selection:within',()=>component.addRepresentation('line',{sele:'within 2 of (protein and name CA)'}).dispose());
 probe('color:bfactor',()=>component.addRepresentation('cartoon',{color:'bfactor'}).dispose());
 probe('color:chainindex',()=>component.addRepresentation('ball+stick',{color:'chainindex'}).dispose());
 probe('camera:autoView',()=>{component.autoView();return 'camera fit';});component.addRepresentation('cartoon',{color:'chainindex'});component.addRepresentation('ball+stick',{sele:'backbone',color:'element'});
 stage.handleResize();stage.viewer.requestRender();await new Promise(r=>setTimeout(r,500));window.audit.rendered=true;window.audit.status=window.audit.probes.some(p=>p.status==='failed')?'passed-with-failures':'passed';
}catch(e){window.audit.status='failed';window.audit.error=String(e);}});
</script>"""


MOLSTAR_HTML = r"""<!doctype html>
<meta charset="utf-8"><title>Mol* disposable parity probe</title>
<link rel="stylesheet" href="/node_modules/molstar/build/viewer/molstar.css">
<style>html,body,#viewport{width:100%;height:100%;margin:0;background:#fff}#viewport{position:relative}</style>
<div id="viewport"></div>
<script>window.audit={status:'script-loading',probes:[],errors:[]};addEventListener('error',e=>window.audit.errors.push(String(e.error||e.message)));addEventListener('unhandledrejection',e=>window.audit.errors.push(String(e.reason)));</script>
<script src="/node_modules/molstar/build/viewer/molstar.js"></script>
<script>
const pdb='ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N\nATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C\nATOM      3  C   ALA A   1       2.000   1.300   0.000  1.00 10.00           C\nATOM      4  O   ALA A   1       1.300   2.250   0.000  1.00 10.00           O\nATOM      5  N   GLY A   2       3.300   1.450   0.000  1.00 10.00           N\nATOM      6  CA  GLY A   2       4.000   2.650   0.000  1.00 10.00           C\nATOM      7  C   GLY A   2       5.450   2.350   0.000  1.00 10.00           C\nATOM      8  O   GLY A   2       6.050   3.350   0.000  1.00 10.00           O\nEND';
const probe=async(name,fn)=>{try{const detail=await fn();window.audit.probes.push({name,status:'passed',detail:String(detail??'')});}catch(e){window.audit.probes.push({name,status:'failed',error:String(e)});}};
const names=['spacefill','ball-and-stick','line','cartoon','backbone','putty','ribbon','trace','tube','gaussian-surface','molecular-surface','point','label','ellipsoid','orientation'];
addEventListener('load',async()=>{try{
 await probe('exports:viewer',()=>Object.keys(window.molstar).filter(k=>/Viewer|Structure|Representation|Volume|Snapshot|Plugin|Color|Theme|Trajectory|Camera|Canvas|Animation|State/i.test(k)).length+' relevant exports');
 const viewer=await molstar.Viewer.create('viewport',{layoutShowControls:false,viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'});window.__viewer=viewer;
 window.audit.viewerExports=Object.keys(molstar).filter(k=>/Viewer|Structure|Representation|Volume|Snapshot|Plugin|Color|Theme|Trajectory|Camera|Canvas|Animation|State/i.test(k)).sort();
 await probe('load:pdb-data',async()=>{await viewer.loadStructureFromData(pdb,'pdb');return 'loaded local PDB data';});await new Promise(r=>setTimeout(r,1500));
 const plugin=viewer.plugin,structures=plugin.managers.structure.hierarchy.current.structures;window.audit.structureCount=structures.length;window.audit.atomCount=structures[0]?.cell?.obj?.data?.elementCount||0;
 await probe('selection:polymer-component',async()=>{const c=await plugin.builders.structure.tryCreateComponentStatic(structures[0].cell,'polymer');if(!c)throw Error('polymer component was empty');window.__component=c;return 'polymer component';});const component=window.__component;
 for(const name of names)await probe('representation:'+name,async()=>{const s=await plugin.builders.structure.representation.addRepresentation(component,{type:name,color:'chain-id'});if(!s)throw Error('representation selector was empty');return name;});
 await probe('color:element-symbol',async()=>plugin.builders.structure.representation.addRepresentation(component,{type:'ball-and-stick',color:'element-symbol'}));await probe('color:sequence-id',async()=>plugin.builders.structure.representation.addRepresentation(component,{type:'cartoon',color:'sequence-id'}));
 await probe('selection:focus-interactivity',()=>viewer.structureInteractivity({select:true,focus:true}));await probe('camera:reset',()=>plugin.managers.camera.reset({durationMs:0}));await probe('snapshot:state-api',()=>typeof plugin.managers.snapshot?.getStateSnapshot==='function'||typeof plugin.state?.getSnapshot==='function');await probe('animation:model-index',()=>typeof plugin.managers.animation.play==='function');
 await new Promise(r=>setTimeout(r,2000));window.audit.rendered=Boolean(document.querySelector('#viewport canvas'));window.audit.status=window.audit.probes.some(p=>p.status==='failed')?'passed-with-failures':'passed';
}catch(e){window.audit.status='failed';window.audit.error=String(e);}});
</script>"""


def json_from_node(node: str, root: Path) -> dict[str, Any]:
    script = """
const root = process.argv[1];
const ngl = require(root + '/node_modules/ngl');
const molstar = require(root + '/node_modules/molstar/package.json');
const reps = Object.keys(ngl).filter(k => /Representation|Surface|Volume|Selection|Trajectory|Color|Component|Shape|Stage|Unitcell|Label|Distance|Angle|Dihedral|Validation/i.test(k)).sort();
console.log(JSON.stringify({nglVersion: require(root + '/node_modules/ngl/package.json').version, nglExportCount: Object.keys(ngl).length, nglRepresentationExports: reps, molstarVersion: molstar.version, molstarBrowserBundle: require('fs').existsSync(root + '/node_modules/molstar/build/viewer/molstar.js')}));
"""
    completed = subprocess.run([node, "-e", script, str(root)], capture_output=True, text=True, cwd=root, timeout=60)
    if completed.returncode:
        raise RuntimeError(completed.stderr.strip() or completed.stdout.strip())
    return json.loads(completed.stdout)


def browser_call(executable: str, session: str, arguments: list[str], timeout: float = 45) -> dict[str, Any]:
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
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    raise RuntimeError(f"browser-use returned no JSON: {completed.stdout[-500:]}")


def audit_from_browser(payload: dict[str, Any]) -> dict[str, Any]:
    result = payload.get("data", {}).get("result")
    if not isinstance(result, str):
        raise RuntimeError(f"browser eval has no string result: {payload}")
    return json.loads(result)


def run_browser(browser: str, root: Path) -> dict[str, Any]:
    session = f"molgfx-web-probe-{os.getpid()}"
    with tempfile.TemporaryDirectory(prefix="molgfx-web-engines-") as directory:
        webroot = Path(directory)
        (webroot / "node_modules").symlink_to(root / "node_modules", target_is_directory=True)
        (webroot / "ngl-probe.html").write_text(NGL_HTML, encoding="utf-8")
        (webroot / "molstar-probe.html").write_text(MOLSTAR_HTML, encoding="utf-8")

        class QuietHandler(SimpleHTTPRequestHandler):
            def log_message(self, _format: str, *_args: Any) -> None:
                return

        server = ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(QuietHandler, directory=str(webroot))
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base_url = f"http://127.0.0.1:{server.server_port}"
        result: dict[str, Any] = {"status": "passed", "engines": {}}
        try:
            for name in ("ngl", "molstar"):
                browser_call(browser, session, ["open", f"{base_url}/{name}-probe.html"])
                audit: dict[str, Any] | None = None
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    try:
                        audit = audit_from_browser(
                            browser_call(browser, session, ["eval", "JSON.stringify(window.audit)"])
                        )
                    except (RuntimeError, json.JSONDecodeError):
                        audit = None
                    if audit and audit.get("status") in {"passed", "passed-with-failures", "failed"}:
                        break
                    time.sleep(0.5)
                if audit is None:
                    raise RuntimeError(f"{name} browser probe did not return audit state")
                screenshot = webroot / f"{name}.png"
                try:
                    browser_call(browser, session, ["screenshot", str(screenshot)])
                    audit["screenshotBytes"] = screenshot.stat().st_size
                except (RuntimeError, OSError) as error:
                    audit["screenshotError"] = str(error)
                result["engines"][name] = audit
                if audit.get("status") != "passed":
                    result["status"] = "passed-with-failures"
        finally:
            subprocess.run([browser, "--session", session, "close"], capture_output=True, text=True, timeout=30)
            server.shutdown()
            thread.join(timeout=5)
        return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-root", type=Path, required=True, help="disposable npm --prefix directory")
    parser.add_argument("--node", default=shutil.which("node"), help="node executable")
    parser.add_argument("--browser-use", default=shutil.which("browser-use"), help="browser-use executable")
    parser.add_argument("--skip-browser", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = args.node_root.resolve()
    result: dict[str, Any] = {"schema": 1, "status": "passed", "runtime": "external-disposable-node-root"}
    try:
        if not args.node:
            raise RuntimeError("node executable not found")
        result["node"] = json_from_node(args.node, root)
    except (OSError, RuntimeError, json.JSONDecodeError) as error:
        result.update({"status": "unavailable", "error": str(error)})
    if result["status"] == "passed" and not args.skip_browser:
        if not args.browser_use:
            result["status"] = "passed-with-unavailable"
            result["browser"] = {"status": "unavailable", "reason": "browser-use executable not found"}
        else:
            try:
                result["browser"] = run_browser(args.browser_use, root)
                if result["browser"]["status"] != "passed":
                    result["status"] = "passed-with-failures"
            except (OSError, RuntimeError, json.JSONDecodeError) as error:
                result.update({"status": "passed-with-unavailable", "browser": {"status": "unavailable", "error": str(error)}})
    elif args.skip_browser:
        result["browser"] = {"status": "skipped", "reason": "--skip-browser"}
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0 if result["status"] != "unavailable" else 1


if __name__ == "__main__":
    raise SystemExit(main())
