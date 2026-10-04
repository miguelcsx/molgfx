"""Real PyMOL adapter: raster requires -q (Qt/OpenGL); inspect/ray use -cq."""

import collections
import ctypes
import hashlib
import json
import math
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
        raise RendererUnavailable(
            "PyMOL raster requires its GUI/OpenGL context; launch with -q, not -cq"
        )
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
            raise RendererUnavailable(
                "Requested fixed raster supersampling exceeds GL viewport; no quality reduction allowed"
            )
        return {
            "vendor": renderer[0],
            "renderer": renderer[1],
            "version": renderer[2],
            "context_valid": True,
            "context_samples": fmt.samples(),
            "context_depth_bits": fmt.depthBufferSize(),
            "context_version": [fmt.majorVersion(), fmt.minorVersion()],
            "max_viewport_dims": list(dims),
            "supersample_factor_per_axis": factor,
            "internal_extent": [size * factor for size in extent],
            "sampling_provenance": "PyMOL Scene.cpp ExtentGetUpscaleInfo; runtime GL_MAX_VIEWPORT_DIMS; not Qt context MSAA",
        }

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
    pixels = pixels.reshape(image.height(), image.bytesPerLine())[:, : image.width() * 4]
    pixels = pixels.reshape(image.height(), image.width(), 4)
    bg = np.rint(np.asarray(background) * 255).astype(np.int16)
    foreground = (np.max(np.abs(pixels[:, :, :3].astype(np.int16) - bg), axis=2) > 2) & (
        pixels[:, :, 3] > 0
    )
    y, x = np.nonzero(foreground)
    if not x.size:
        raise IncompleteImage("PyMOL PNG contains only background, not the requested scene")
    return {
        "decoded_extent": list(extent),
        "foreground_pixels": int(x.size),
        "foreground_bounds_xyxy": [int(x.min()), int(y.min()), int(x.max()) + 1, int(y.max()) + 1],
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "export": "cached completed image only (png prior=1); no rerender",
    }


def segment_labels(field, thresholds):
    import numpy as np

    low, middle, high = np.asarray(thresholds, dtype=np.float32)
    return np.select([field >= high, field >= middle, field >= low], [1, 2, 3], default=0)


def segmentation_surfaces(request, labels):
    """Project each visible categorical label to a binary-mask surface."""
    bounds = []
    for index, spec in enumerate(request["fixture"]["segmentations"]):
        x, y, z = spec["translation"]
        matrix = [1, 0, 0, x, 0, 1, 0, y, 0, 0, 1, z, 0, 0, 0, 1]
        for style in spec["styles"]:
            if not style["visible"] or style["opacity"] == 0:
                continue
            name = f"segment_{index}_{style['label']}"
            map_name = name + "_map"
            cmd.load(request["path"], map_name)
            # Assign immediately while the map owns this borrowed memory. No
            # PyMOL command may invalidate the array before it is released.
            field = cmd.get_volume_field(map_name, state=1, copy=0)
            field[...] = labels == style["label"]
            del field
            cmd.transform_object(map_name, matrix, homogenous=1)
            cmd.isosurface(name, map_name, 0.5)
            bounds.append({"surface": name, "bounds": cmd.get_extent(name)})
            cmd.set_color(name + "_color", [value / 255 for value in style["color_rgb"]])
            cmd.color(name + "_color", name)
            cmd.set("transparency", 1 - style["opacity"], name)
            cmd.hide("everything", map_name)
    return bounds


def main():
    request = json.loads(Path(os.environ["MOLGFX_PARITY_REQUEST"]).read_text())
    catalog, fixture = request["catalog"], request["fixture"]
    output = Path(request["output"])
    recipe = request["recipe"]
    cmd.reinitialize()
    settings = catalog["recipe_settings"][recipe]
    for name, value in settings.items():
        cmd.set(name, value)
    effect = fixture.get("effect")
    effect_evidence = {"requested": effect, "applied": None, "omissions": []}
    if effect == "depth_cue":
        cmd.set("depth_cue", 1)
        cmd.set("fog_start", 0.25)
        cmd.set("ray_trace_fog", 1)
        cmd.set("ray_trace_fog_start", 0.25)
        effect_evidence["applied"] = "PyMOL depth fog; engine-specific depth range"
    elif effect in {"bloom", "dof", "motion_blur", "shape_cues"}:
        effect_evidence["omissions"].append(
            f"PyMOL has no equivalent {effect} presentation effect; base lighting retained"
        )
    elif effect not in {None, "bloom_control"}:
        raise ValueError(f"unknown reference effect: {effect}")
    if fixture["format"] == "mrc":
        # Isovalues and segmentation thresholds name the caller's raw field.
        # PyMOL otherwise rescales CCP4 data to standard deviations on load.
        cmd.set("normalize_ccp4_maps", 0)
    # No engine-specific selection grammar: all source rows, first model only.
    cmd.load(request["path"], "fixture")
    cmd.frame(1)
    metadata = {"source_sha256": fixture["sha256"]}
    if fixture["format"] == "mrc":
        field = cmd.get_volume_field("fixture", state=1, copy=1)
        metadata.update(
            {
                "dimensions": list(field.shape),
                "voxels": int(field.size),
                "value_range": [float(field.min()), float(field.max())],
                "normalization": "raw caller values; normalize_ccp4_maps disabled before load",
                "affine_provenance": "PyMOL CCP4 loader from hashed file",
            }
        )
        if fixture.get("segmentations"):
            labels = segment_labels(field, fixture["segmentation_thresholds"])
            metadata.update(
                segmentation_thresholds=fixture["segmentation_thresholds"],
                segmentations=fixture["segmentations"],
                label_counts=[int((labels == label).sum()) for label in range(4)],
                representation_policy="PyMOL binary-mask segment surfaces; not optical ray-volume equivalence",
            )
    else:
        model = cmd.get_model("fixture", state=1)
        residues = {(a.chain, a.resi, a.resn): a.ss for a in model.atom}
        ss = collections.Counter(
            "helix" if v == "H" else "strand" if v == "S" else "unknown" for v in residues.values()
        )
        orders = collections.Counter(str(b.order) for b in model.bond)
        radii = {}
        for atom in model.atom:
            radii.setdefault(atom.symbol, set()).add(atom.vdw)
        metadata.update(
            {
                "atoms": len(model.atom),
                "residues": len(residues),
                "bonds": len(model.bond),
                "aromatic_bonds": sum(b.order == 4 for b in model.bond),
                "bond_orders": dict(orders),
                "bond_provenance": None,
                "bond_provenance_reason": "PyMOL model does not expose file/CCD/inference provenance",
                "secondary_structure": dict(ss),
                "secondary_structure_policy": "PyMOL H/S; blank is unknown, not reassigned",
                "secondary_structure_encoding": "coarse PyMOL helix/strand only; helix subtype unavailable",
                "vdw_radii_angstrom": {k: sorted(v) for k, v in radii.items()},
            }
        )
    response = {"engine": "pymol", "engine_version": list(cmd.get_version()), "metadata": metadata}
    if not request["inspect"]:
        style, camera = catalog["style"], fixture["camera"]
        cmd.hide("everything", "all")
        cmd.set_color("parity_color", [v / 255 for v in style["color_rgb"]])
        script = fixture.get("script")
        if fixture.get("segmentations"):
            metadata["segmentation_surface_bounds"] = segmentation_surfaces(request, labels)
        elif script:
            for line in script["pymol"]:
                cmd.do(line)
        elif fixture["format"] == "mrc":
            form = fixture["form"]
            affine = request["voxel_to_world"]
            voxel_size = min(
                math.sqrt(sum(affine[axis * 4 + row] ** 2 for row in range(3))) for axis in range(3)
            )
            if form == "iso_mesh":
                cmd.set("mesh_radius", fixture["line_width_voxels"] * voxel_size)
                metadata["mesh_radius_angstrom"] = fixture["line_width_voxels"] * voxel_size
                cmd.isomesh("iso", "fixture", fixture["isovalue"])
            elif form == "iso_dots":
                cmd.set("dot_radius", fixture["dot_radius_voxels"] * voxel_size)
                metadata["dot_radius_angstrom"] = fixture["dot_radius_voxels"] * voxel_size
                cmd.isodot("iso", "fixture", fixture["isovalue"])
            elif form == "direct":
                cmd.volume("iso", "fixture")
                blue = [49 / 255, 104 / 255, 142 / 255]
                # PyMOL integrates this ramp per voxel, not with MolGFX optical depth;
                # lower alpha preserves the density interior instead of saturating it.
                cmd.volume_color("iso", [0.0, *blue, 0.0, fixture["isovalue"] * 2, *blue, 0.08])
                cmd.bg_color("white")
            elif form == "slice":
                minimum, maximum = cmd.get_extent("fixture")
                slice_point = [(low + high) * 0.5 for low, high in zip(minimum, maximum)]
                if any(
                    abs(actual - expected) > 1.0e-4
                    for actual, expected in zip(slice_point, fixture["slice_point"])
                ):
                    raise ValueError(
                        f"PyMOL slice center {slice_point} does not match authored {fixture['slice_point']}"
                    )
                if list(cmd.get_view()[:9]) != [1, 0, 0, 0, 1, 0, 0, 0, 1] or fixture[
                    "slice_normal"
                ] != [0, 0, 1]:
                    raise ValueError("PyMOL slice adapter requires the initial Z-normal plane")
                cmd.slice_new("iso", "fixture")
                metadata["map_object_matrix"] = cmd.get_object_matrix("fixture")
                cmd.set_object_ttt("iso", metadata["map_object_matrix"])
                metadata["slice_object_matrix"] = cmd.get_object_matrix("iso")
                colors = fixture["slice_colors"]
                low, high = fixture["slice_domain"]
                cmd.ramp_new(
                    "iso_ramp",
                    "fixture",
                    [low + (high - low) * i / (len(colors) - 1) for i in range(len(colors))],
                    [[((color >> shift) & 255) / 255 for shift in (16, 8, 0)] for color in colors],
                )
                metadata["slice"] = {
                    "point": slice_point,
                    "normal": fixture["slice_normal"],
                    "domain": fixture["slice_domain"],
                    "colors": colors,
                }
                cmd.bg_color("white")
            elif form == "region":
                raise ValueError("PyMOL adapter has no exact voxel crop for this fixture")
            else:
                cmd.isosurface("iso", "fixture", fixture["isovalue"])
            if fixture.get("structure_file"):
                cmd.load(str(Path(request["path"]).parent / fixture["structure_file"]), "molecule")
                cmd.show("cartoon", "molecule")

            if form == "slice":
                cmd.color("iso_ramp", "iso")
            else:
                cmd.color("parity_color", "iso")
            if fixture.get("structure_file"):
                cmd.set("transparency", 1.0 - fixture["opacity"], "iso")
        else:
            cmd.color("parity_color", "fixture")
            cmd.set("sphere_scale", style["atom_radius_scale"])
            cmd.set("stick_radius", style["bond_radius_angstrom"])
            cmd.set("cartoon_rect_width", style["cartoon_width_angstrom"])
            if fixture["form"] == "cartoon":
                cmd.show("cartoon", "fixture")
            elif fixture["form"] == "spacefill":
                cmd.show("spheres", "fixture")
            elif fixture["form"] == "ball_and_stick":
                cmd.show("spheres", "fixture")
                cmd.show("sticks", "fixture")
            else:
                raise ValueError("Unsupported PyMOL representation")
        # This manifest uses axis-aligned physical cameras, not auto-zoom.
        p, t = camera["position"], camera["target"]
        if p[0] != t[0] or p[1] != t[1] or p[2] <= t[2] or camera["up"] != [0, 1, 0]:
            raise ValueError("PyMOL camera adapter requires +Z/upY manifest camera")
        cmd.set("field_of_view", camera["fov_y_degrees"])
        cmd.set_view(
            [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, -(p[2] - t[2]), *t, camera["near"], camera["far"], 0]
        )
        width, height = catalog["extent"]
        ray = recipe == "pymol-ray"
        if ray and fixture["format"] == "mrc" and fixture["form"] == "direct":
            cmd.set("ray_volume", 1)
        gl = None if ray else raster_context([width, height], settings["antialias"])

        def completed_output():
            start = time.perf_counter_ns()
            if ray:
                cmd.ray(width, height, antialias=settings["antialias"], async_=0)
            else:
                cmd.draw(width, height, antialias=settings["antialias"])
            return time.perf_counter_ns() - start

        def measured_output():
            elapsed = completed_output()
            return {
                "sample": {"cpu_ns": elapsed, "frame_ns": elapsed, "gpu_ns": None},
                "gpu_unavailable_reason": "PyMOL draw/ray provides no GPU timestamp query",
            }

        cold_output = measured_output()
        warmup_samples = [measured_output() for _ in range(catalog["warmup_outputs"])]
        samples = [measured_output() for _ in range(catalog["measured_outputs"])]
        cmd.png(str(output / "image.png"), prior=1)
        evidence = image_evidence(output / "image.png", [width, height], settings["bg_rgb"])
        response.update(
            {
                "samples": samples,
                "cold_output": cold_output,
                "warmup_samples": warmup_samples,
                "image": "image.png",
                "image_evidence": evidence,
                "measurement_scope": "completed PyMOL draw (framebuffer readback and supersampling included) or blocking ray; cached PNG export/decoding excluded; cpu_ns is blocking API wall time, not exclusive CPU work",
                "effective_settings": {
                    "effect": effect_evidence,
                    "settings": {name: cmd.get(name) for name in settings},
                    "view": list(cmd.get_view()),
                    "extent": [width, height],
                    "style": style,
                    "gl": gl,
                    "renderer_kind": "software-ray" if ray else "OpenGL-raster",
                    "antialias": cmd.get_setting_int("antialias"),
                    "recipe_policy": catalog.get("recipe_policy", {}).get(recipe),
                    "ray_supersample_factor_per_axis": min(settings["antialias"], 4)
                    if ray
                    else None,
                    "sampling_provenance": "PyMOL Ray.cpp RayRender clamps antialias to 4; 4x uniform plus adaptive edge samples"
                    if ray
                    else gl["sampling_provenance"],
                },
                "telemetry_unavailable": ["heap", "GPU timestamps", "residency counters"],
                "physical_settings_equivalent": False,
            }
        )
    Path(request["response"]).write_text(json.dumps(response, indent=2))


try:
    main()
except Exception as error:
    import traceback

    traceback.print_exc()
    request = json.loads(Path(os.environ["MOLGFX_PARITY_REQUEST"]).read_text())
    Path(request["response"]).write_text(
        json.dumps(
            {"engine": "pymol", "error": {"type": type(error).__name__, "message": str(error)}},
            indent=2,
        )
    )
    cmd.quit(1)
cmd.quit()
