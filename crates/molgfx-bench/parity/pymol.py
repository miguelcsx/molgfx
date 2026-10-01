"""Real PyMOL adapter: raster requires -q (Qt/OpenGL); inspect/ray use -cq."""
import collections
import ctypes
import hashlib
import json
import os
from pathlib import Path
import time
from pymol import cmd


class RendererUnavailable(RuntimeError):
    """The requested real renderer has no valid OpenGL framebuffer."""


class IncompleteImage(RuntimeError):
    """The completed output is missing, mis-sized, or entirely background."""


def raster_context(extent, antialias):
    if cmd._pymol.invocation.options.no_gui:
        raise RendererUnavailable("PyMOL raster requires its GUI/OpenGL context; launch with -q, not -cq")
    from pymol.Qt import QtGui

    def inspect_context():
        context = QtGui.QOpenGLContext.currentContext()
        if context is None or not context.isValid():
            raise RendererUnavailable("PyMOL Qt OpenGL context is not valid")
        fmt = context.format()
        renderer = list(cmd.get_renderer())
        if len(renderer) != 3 or not all(renderer):
            raise RendererUnavailable("PyMOL did not identify a real OpenGL renderer")
        address = context.getProcAddress(b"glGetIntegerv")
        if not address:
            raise RendererUnavailable("Cannot query the real OpenGL viewport limit")
        call = ctypes.WINFUNCTYPE if os.name == "nt" else ctypes.CFUNCTYPE
        query = call(None, ctypes.c_uint, ctypes.POINTER(ctypes.c_int))(int(address))
        dims = (ctypes.c_int * 2)()
        query(0x0D3A, dims)  # GL_MAX_VIEWPORT_DIMS
        factor = 4 if antialias >= 2 else 2 if antialias == 1 else 1
        if any(size * factor >= limit for size, limit in zip(extent, dims)):
            raise RendererUnavailable("Requested fixed raster supersampling exceeds GL viewport; no quality reduction allowed")
        return {"vendor": renderer[0], "renderer": renderer[1], "version": renderer[2],
                "context_valid": True, "context_samples": fmt.samples(),
                "context_depth_bits": fmt.depthBufferSize(),
                "context_version": [fmt.majorVersion(), fmt.minorVersion()],
                "max_viewport_dims": list(dims), "supersample_factor_per_axis": factor,
                "internal_extent": [size * factor for size in extent],
                "sampling_provenance": "PyMOL Scene.cpp ExtentGetUpscaleInfo; runtime GL_MAX_VIEWPORT_DIMS; not Qt context MSAA"}

    return cmd._call_with_opengl_context(inspect_context)


def image_evidence(path, extent, background):
    # Qt is already PyMOL's raster host; decoding is outside every timed interval.
    from pymol.Qt import QtGui
    import numpy as np

    image = QtGui.QImage(str(path))
    if image.isNull() or [image.width(), image.height()] != list(extent):
        raise IncompleteImage("PyMOL PNG did not decode at the requested physical extent")
    image = image.convertToFormat(QtGui.QImage.Format_RGBA8888)
    pixels = np.frombuffer(image.constBits().asstring(image.sizeInBytes()), dtype=np.uint8)
    pixels = pixels.reshape(image.height(), image.bytesPerLine())[:, :image.width() * 4]
    pixels = pixels.reshape(image.height(), image.width(), 4)
    bg = np.rint(np.asarray(background) * 255).astype(np.int16)
    foreground = (np.max(np.abs(pixels[:, :, :3].astype(np.int16) - bg), axis=2) > 2) & (pixels[:, :, 3] > 0)
    y, x = np.nonzero(foreground)
    if not x.size:
        raise IncompleteImage("PyMOL PNG contains only background, not the requested scene")
    return {"decoded_extent": list(extent), "foreground_pixels": int(x.size),
            "foreground_bounds_xyxy": [int(x.min()), int(y.min()), int(x.max()) + 1, int(y.max()) + 1],
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "export": "cached completed image only (png prior=1); no rerender"}


def main():
    request = json.loads(Path(os.environ["MOLGFX_PARITY_REQUEST"]).read_text())
    catalog, fixture = request["catalog"], request["fixture"]
    output = Path(request["output"])
    recipe = request["recipe"]
    cmd.reinitialize()
    settings = catalog["recipe_settings"][recipe]
    for name, value in settings.items():
        cmd.set(name, value)
    # No engine-specific selection grammar: all source rows, first model only.
    cmd.load(request["path"], "fixture")
    cmd.frame(1)
    metadata = {"source_sha256": fixture["sha256"]}
    if fixture["format"] == "mrc":
        field = cmd.get_volume_field("fixture", state=1, copy=1)
        metadata.update({"dimensions": list(field.shape), "voxels": int(field.size),
                         "value_range": [float(field.min()), float(field.max())],
                         "affine_provenance": "PyMOL CCP4 loader from hashed file"})
    else:
        model = cmd.get_model("fixture", state=1)
        residues = {(a.chain, a.resi, a.resn): a.ss for a in model.atom}
        ss = collections.Counter("H" if v == "H" else "E" if v == "S" else "U" for v in residues.values())
        orders = collections.Counter(str(b.order) for b in model.bond)
        radii = {}
        for atom in model.atom:
            radii.setdefault(atom.symbol, set()).add(atom.vdw)
        metadata.update({"atoms": len(model.atom), "residues": len(residues), "bonds": len(model.bond),
                         "aromatic_bonds": sum(b.order == 4 for b in model.bond), "bond_orders": dict(orders),
                         "bond_provenance": None, "bond_provenance_reason": "PyMOL model does not expose file/CCD/inference provenance",
                         "secondary_structure": dict(ss), "secondary_structure_policy": "PyMOL H/S; blank is unknown, not reassigned",
                         "vdw_radii_angstrom": {k: sorted(v) for k, v in radii.items()}})
    response = {"engine": "pymol", "engine_version": list(cmd.get_version()), "metadata": metadata}
    if not request["inspect"]:
        style, camera = catalog["style"], fixture["camera"]
        cmd.hide("everything", "all")
        cmd.set_color("parity_color", [v / 255 for v in style["color_rgb"]])
        if fixture["format"] == "mrc":
            cmd.isosurface("iso", "fixture", fixture["isovalue"])
            cmd.color("parity_color", "iso")
        else:
            cmd.color("parity_color", "fixture")
            cmd.set("sphere_scale", style["atom_radius_scale"])
            cmd.set("stick_radius", style["bond_radius_angstrom"])
            cmd.set("cartoon_rect_width", style["cartoon_width_angstrom"])
            if fixture["form"] == "cartoon": cmd.show("cartoon", "fixture")
            elif fixture["form"] == "spacefill": cmd.show("spheres", "fixture")
            elif fixture["form"] == "ball_and_stick":
                cmd.show("spheres", "fixture")
                cmd.show("sticks", "fixture")
            else: raise ValueError("Unsupported PyMOL representation")
        # This manifest uses axis-aligned physical cameras, not auto-zoom.
        p, t = camera["position"], camera["target"]
        if p[0] != t[0] or p[1] != t[1] or p[2] <= t[2] or camera["up"] != [0, 1, 0]:
            raise ValueError("PyMOL camera adapter requires +Z/upY manifest camera")
        cmd.set("field_of_view", camera["fov_y_degrees"])
        cmd.set_view([1,0,0,0,1,0,0,0,1,0,0,-(p[2]-t[2]),*t,camera["near"],camera["far"],0])
        width, height = catalog["extent"]
        ray = recipe == "pymol-ray"
        gl = None if ray else raster_context([width, height], settings["antialias"])
        def completed_output():
            start = time.perf_counter_ns()
            if ray: cmd.ray(width, height, antialias=settings["antialias"], async_=0)
            else: cmd.draw(width, height, antialias=settings["antialias"])
            return time.perf_counter_ns() - start
        def measured_output():
            elapsed = completed_output()
            return {"sample": {"cpu_ns": elapsed, "frame_ns": elapsed, "gpu_ns": None},
                    "gpu_unavailable_reason": "PyMOL draw/ray provides no GPU timestamp query"}
        cold_output = measured_output()
        warmup_samples = [measured_output() for _ in range(catalog["warmup_outputs"])]
        samples = [measured_output() for _ in range(catalog["measured_outputs"])]
        cmd.png(str(output / "image.png"), prior=1)
        evidence = image_evidence(output / "image.png", [width, height], settings["bg_rgb"])
        response.update({"samples": samples, "cold_output": cold_output, "warmup_samples": warmup_samples, "image": "image.png",
                         "image_evidence": evidence,
                         "measurement_scope": "completed PyMOL draw (framebuffer readback and supersampling included) or blocking ray; cached PNG export/decoding excluded; cpu_ns is blocking API wall time, not exclusive CPU work",
                         "effective_settings": {"settings": {name: cmd.get(name) for name in settings},
                                                "view": list(cmd.get_view()), "extent": [width, height], "style": style,
                                                "gl": gl, "renderer_kind": "software-ray" if ray else "OpenGL-raster",
                                                "antialias": cmd.get_setting_int("antialias"),
                                                "recipe_policy": catalog.get("recipe_policy", {}).get(recipe),
                                                "ray_supersample_factor_per_axis": min(settings["antialias"], 4) if ray else None,
                                                "sampling_provenance": "PyMOL Ray.cpp RayRender clamps antialias to 4; 4x uniform plus adaptive edge samples" if ray else gl["sampling_provenance"]},
                         "telemetry_unavailable": ["heap", "GPU timestamps", "residency counters"],
                         "physical_settings_equivalent": False})
    Path(request["response"]).write_text(json.dumps(response, indent=2))


try:
    main()
except Exception as error:
    import traceback
    traceback.print_exc()
    request = json.loads(Path(os.environ["MOLGFX_PARITY_REQUEST"]).read_text())
    Path(request["response"]).write_text(json.dumps({"engine": "pymol",
        "error": {"type": type(error).__name__, "message": str(error)}}, indent=2))
    cmd.quit(1)
cmd.quit()
