"""Disposable OVITO Python rendering probes for the parity audit."""

from __future__ import annotations

import tempfile
from importlib.metadata import PackageNotFoundError, version as package_version
from pathlib import Path
from typing import Any, Callable

try:
    from graphics_engine_reference_manifest import (
        OVITO_DATA_OBJECTS,
        OVITO_EXPORTS,
        OVITO_PARTICLE_SHAPES,
        OVITO_PIPELINE_CAPABILITIES,
        OVITO_RENDERERS,
        OVITO_VISUAL_CLASSES,
    )
except ModuleNotFoundError:
    from scripts.graphics_engine_reference_manifest import (
        OVITO_DATA_OBJECTS,
        OVITO_EXPORTS,
        OVITO_PARTICLE_SHAPES,
        OVITO_PIPELINE_CAPABILITIES,
        OVITO_RENDERERS,
        OVITO_VISUAL_CLASSES,
    )


MULTIFRAME_XYZ = """8
probe frame 1
C 0 0 0
C 1.5 0 0
C 0 1.5 0
C 0 0 1.5
O 3 0 0
N 0 3 0
H 0 0 3
S 2 2 2
8
probe frame 2
C 0.1 0 0
C 1.6 0 0
C 0 1.6 0
C 0 0 1.6
O 3.1 0 0
N 0 3.1 0
H 0 0 3.1
S 2.1 2 2
"""


def probe_ovito(
    root: Path,
    run_probe: Callable[[str, Callable[[], Any]], dict[str, Any]],
    compact_error: Callable[[BaseException], str],
) -> dict[str, Any]:
    """Run headless pipeline, visualization and still-image probes."""

    try:
        import numpy as np
        from ovito.data import DataCollection, DataTable, DislocationNetwork, VoxelGrid
        from ovito.io import export_file, import_file
        from ovito.modifiers import (
            CalculateDisplacementsModifier,
            ColorCodingModifier,
            ComputePropertyModifier,
            CreateBondsModifier,
            DislocationAnalysisModifier,
        )
        from ovito.pipeline import Pipeline, StaticSource
        from ovito.vis import (
            AnariRenderer,
            BondsVis,
            ColorLegendOverlay,
            CoordinateTripodOverlay,
            DislocationVis,
            LinesVis,
            OSPRayRenderer,
            OpenGLRenderer,
            ParticlesVis,
            PythonViewportOverlay,
            SimulationCellVis,
            SurfaceMeshVis,
            TachyonRenderer,
            TextLabelOverlay,
            TriangleMeshVis,
            VectorVis,
            Viewport,
            ViewportOverlayInterface,
            VoxelGridVis,
        )
        try:
            runtime_version = package_version("ovito")
        except PackageNotFoundError:
            runtime_version = "unknown"
    except Exception as error:
        return {
            "status": "unavailable",
            "error": compact_error(error),
            "scope": "graphics engine; OVITO is a renderer and data pipeline",
        }

    try:
        with tempfile.TemporaryDirectory(prefix="molgfx-ovito-probe-") as directory:
            work = Path(directory)
            input_path = work / "probe.xyz"
            input_path.write_text(MULTIFRAME_XYZ, encoding="utf-8")
            pipeline = import_file(str(input_path))

            def import_and_compute() -> dict[str, Any]:
                data = pipeline.compute(frame=0)
                return {
                    "frames": pipeline.num_frames,
                    "particles": data.particles.count,
                    "particle_properties": sorted(data.particles.keys()),
                }

            def modifiers_and_bonds() -> dict[str, Any]:
                pipeline.modifiers.append(CreateBondsModifier(cutoff=2.0))
                pipeline.modifiers.append(ColorCodingModifier(property="Position.X"))
                data = pipeline.compute(frame=0)
                return {
                    "bonds": data.particles.bonds.count,
                    "has_color": "Color" in data.particles,
                }

            def render(renderer: Any, filename: str) -> dict[str, Any]:
                pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(2.0, 1.0, -1.0),
                )
                viewport.zoom_all()
                output = work / filename
                viewport.render_image(size=(128, 128), filename=str(output), renderer=renderer)
                return {"bytes": output.stat().st_size, "filename": output.name}

            def render_anari() -> dict[str, Any]:
                try:
                    return render(AnariRenderer(), "ovito-anari.png")
                except Exception as error:
                    message = compact_error(error)
                    lowered = message.lower()
                    if any(token in lowered for token in ("anari", "visrtx", "cuda", "nvidia")):
                        return {
                            "status": "unavailable",
                            "reason": "optional ANARI/VisRTX backend could not initialize on this host",
                            "error": message,
                        }
                    raise

            def render_animation() -> dict[str, Any]:
                pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(2.0, 1.0, -1.0),
                )
                viewport.zoom_all()
                output = work / "ovito-animation.mp4"
                viewport.render_anim(
                    filename=str(output),
                    size=(64, 64),
                    fps=4,
                    range=(0, 1),
                    renderer=TachyonRenderer(),
                )
                return {"bytes": output.stat().st_size, "filename": output.name}

            def still_image_format_exports() -> dict[str, Any]:
                pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(2.0, 1.0, -1.0),
                )
                viewport.zoom_all(size=(64, 64))
                outputs = {
                    "png": work / "ovito-export.png",
                    "jpeg": work / "ovito-export.jpeg",
                    "tiff": work / "ovito-export.tiff",
                }
                for output in outputs.values():
                    viewport.render_image(
                        size=(64, 64),
                        filename=str(output),
                        renderer=TachyonRenderer(),
                    )
                return {
                    "formats": {
                        name: {
                            "bytes": output.stat().st_size,
                            "signature": output.read_bytes()[:4].hex(),
                        }
                        for name, output in outputs.items()
                    }
                }

            def pipeline_modifiers_and_displacement() -> dict[str, Any]:
                modifier_pipeline = import_file(str(input_path))
                displacement = CalculateDisplacementsModifier()
                displacement.vis.enabled = True
                displacement.vis.color = (1.0, 0.1, 0.1)
                modifier_pipeline.modifiers.append(displacement)
                modifier_pipeline.modifiers.append(
                    ComputePropertyModifier(
                        output_property="Radius", expressions="Position.X + 1.0"
                    )
                )

                def annotate(frame: int, data: Any) -> None:
                    data.attributes["Audit.Frame"] = frame
                    data.attributes["Audit.ParticleCount"] = data.particles.count

                modifier_pipeline.modifiers.append(annotate)
                data = modifier_pipeline.compute(frame=1)
                modifier_pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(2.0, 1.0, -1.0),
                )
                viewport.zoom_all(size=(128, 128))
                output = work / "ovito-displacement.png"
                viewport.render_image(
                    size=(128, 128),
                    filename=str(output),
                    renderer=OSPRayRenderer(),
                    frame=1,
                )
                return {
                    "frames": modifier_pipeline.num_frames,
                    "displacement_shape": list(data.particles["Displacement"].shape),
                    "radius_shape": list(data.particles["Radius"].shape),
                    "custom_attributes": {
                        "frame": data.attributes["Audit.Frame"],
                        "particle_count": data.attributes["Audit.ParticleCount"],
                    },
                    "png_bytes": output.stat().st_size,
                }

            def manual_visual_objects() -> dict[str, Any]:
                data = DataCollection()
                cell = data.create_cell(
                    [[10, 0, 0, 0], [0, 10, 0, 0], [0, 0, 10, 0]],
                    (False, False, False),
                )
                particles = data.create_particles(count=4, vis_params={"radius": 0.5})
                particles.create_property(
                    "Position", data=[[2, 2, 2], [5, 2, 2], [2, 5, 2], [2, 2, 5]]
                )
                particles.vis.shape = ParticlesVis.Shape.Sphere
                particle_types = particles.create_property(
                    "Particle Type", data=[1, 1, 1, 1]
                )
                particle_type = particle_types.add_type_id(1, particles, "ProbeMesh")
                particle_type.shape = ParticlesVis.Shape.Mesh
                shape_mesh = data.triangle_meshes.create(identifier="particle-shape")
                shape_mesh.set_vertices(
                    [[0, 0, 1], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0]]
                )
                shape_mesh.set_faces(
                    [[0, 1, 3], [0, 3, 2], [0, 2, 4], [0, 4, 1], [1, 4, 2], [1, 2, 3]]
                )
                particle_type.mesh = shape_mesh
                particle_type.radius = 1.2
                shape_modes = {}
                for feature, enum_name in (
                    ("sphere", "Sphere"),
                    ("box", "Box"),
                    ("circle", "Circle"),
                    ("square", "Square"),
                    ("cylinder", "Cylinder"),
                    ("spherocylinder", "Spherocylinder"),
                    ("mesh", "Mesh"),
                ):
                    particles.vis.shape = getattr(ParticlesVis.Shape, enum_name)
                    shape_modes[feature] = str(particles.vis.shape)
                particles.create_property(
                    "Aspherical Shape",
                    data=np.array([[1.0, 0.6, 1.4]] * particles.count),
                )
                particles.vis.shape = ParticlesVis.Shape.Sphere
                shape_modes["ellipsoid"] = {
                    "mode": str(particles.vis.shape),
                    "property": "Aspherical Shape",
                }
                particles.create_property(
                    "Superquadric Roundness",
                    data=np.array([[0.5, 0.7]] * particles.count),
                )
                shape_modes["superquadric"] = {
                    "mode": str(particles.vis.shape),
                    "property": "Superquadric Roundness",
                }
                lines = data.lines.create(identifier="paths")
                lines.create_line([[2, 2, 2], [5, 2, 2], [5, 5, 2]])
                lines.vis.width = 0.18
                vectors = data.vectors.create(identifier="vectors")
                vectors.create_property("Position", data=[[2, 2, 2], [5, 5, 5]])
                vectors.create_property("Direction", data=[[1, 0, 0], [0, 1, 0]])
                vectors.vis.enabled = True
                vectors.vis.width = 0.2
                grid = data.grids.create(
                    identifier="field",
                    domain=cell,
                    shape=(8, 8, 8),
                    grid_type=VoxelGrid.GridType.CellData,
                    vis_params={"enabled": True, "transparency": 0.6},
                )
                field = np.zeros((8, 8, 8), dtype=np.float32)
                field[2:6, 2:6, 2:6] = 1.0
                grid.create_property("Field Value", data=field.flatten(order="F"))
                grid.vis.color_mapping_property = "Field Value"
                grid.vis.color_mapping_interval = (0.0, 1.0)
                grid.vis.representation = VoxelGridVis.RepresentationMode.Volume
                grid.vis.opacity_function = np.array([0.0, 0.8], dtype=np.float64)
                manual = Pipeline(source=StaticSource(data=data))
                manual.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(1.0, 1.0, -1.0),
                )
                viewport.zoom_all()

                class AuditOverlay(ViewportOverlayInterface):
                    def render(self, canvas: Any, **_kwargs: Any) -> None:
                        canvas.draw_text("OVITO Python overlay", pos=(0.5, 0.9))

                viewport.overlays.append(TextLabelOverlay(text="OVITO visual-object probe"))
                viewport.overlays.append(CoordinateTripodOverlay())
                viewport.overlays.append(ColorLegendOverlay(color_mapping_source=grid.vis))
                viewport.overlays.append(PythonViewportOverlay(delegate=AuditOverlay()))
                output = work / "ovito-visual-objects.png"
                viewport.render_image(
                    size=(128, 128), filename=str(output), renderer=OSPRayRenderer()
                )
                return {
                    "bytes": output.stat().st_size,
                    "particles": particles.count,
                    "lines": lines.count,
                    "vectors": vectors.count,
                    "grid_shape": list(grid.shape),
                    "overlays": len(viewport.overlays),
                    "particle_mesh": particle_type.mesh.identifier,
                    "particle_shape": str(particle_type.shape),
                    "shape_modes": shape_modes,
                    "aspherical_shape": list(particles["Aspherical Shape"][0]),
                    "superquadric_roundness": list(
                        particles["Superquadric Roundness"][0]
                    ),
                }

            def particle_shape_render_matrix() -> dict[str, Any]:
                """Render every documented OVITO particle-shape family."""

                shape_records = []
                enum_names = {
                    "sphere": "Sphere",
                    "box": "Box",
                    "circle": "Circle",
                    "square": "Square",
                    "cylinder": "Cylinder",
                    "spherocylinder": "Spherocylinder",
                    "mesh": "Mesh",
                }
                for feature in OVITO_PARTICLE_SHAPES:
                    pipeline = None
                    try:
                        data = DataCollection()
                        particles = data.create_particles(count=1)
                        particles.create_property("Position", data=[[0.0, 0.0, 0.0]])
                        particles.create_property("Radius", data=[1.0])
                        if feature == "ellipsoid":
                            particles.create_property(
                                "Aspherical Shape", data=[[1.0, 0.6, 1.4]]
                            )
                            particles.vis.shape = ParticlesVis.Shape.Sphere
                            mode = str(particles.vis.shape)
                        elif feature == "superquadric":
                            particles.create_property(
                                "Aspherical Shape", data=[[1.0, 0.8, 1.2]]
                            )
                            particles.create_property(
                                "Superquadric Roundness", data=[[0.5, 0.7]]
                            )
                            particles.vis.shape = ParticlesVis.Shape.Sphere
                            mode = str(particles.vis.shape)
                        else:
                            shape = getattr(ParticlesVis.Shape, enum_names[feature])
                            particles.vis.shape = shape
                            mode = str(shape)
                            if feature == "mesh":
                                types = particles.create_property(
                                    "Particle Type", data=[1]
                                )
                                particle_type = types.add_type_id(
                                    1, particles, "ProbeMesh"
                                )
                                particle_type.shape = ParticlesVis.Shape.Mesh
                                mesh = data.triangle_meshes.create(
                                    identifier="shape-mesh"
                                )
                                mesh.set_vertices(
                                    [[0, 0, 1], [1, 0, 0], [-1, 0, 0], [0, 1, 0]]
                                )
                                mesh.set_faces([[0, 1, 3], [0, 3, 2]])
                                particle_type.mesh = mesh
                        pipeline = Pipeline(source=StaticSource(data=data))
                        pipeline.add_to_scene()
                        viewport = Viewport(
                            type=Viewport.Type.Perspective,
                            camera_dir=(1.0, 1.0, -1.0),
                        )
                        viewport.zoom_all(size=(64, 64))
                        output = work / f"ovito-shape-{feature}.png"
                        renderer_name = (
                            "OpenGLRenderer"
                            if feature in {"circle", "square"}
                            else "TachyonRenderer"
                        )
                        renderer = (
                            OpenGLRenderer()
                            if renderer_name == "OpenGLRenderer"
                            else TachyonRenderer()
                        )
                        viewport.render_image(
                            size=(64, 64),
                            filename=str(output),
                            renderer=renderer,
                        )
                        shape_records.append(
                            {
                                "feature": feature,
                                "status": "passed",
                                "mode": mode,
                                "renderer": renderer_name,
                                "bytes": output.stat().st_size,
                            }
                        )
                    except Exception as error:
                        shape_records.append(
                            {
                                "feature": feature,
                                "status": "failed",
                                "error": compact_error(error),
                            }
                        )
                    finally:
                        if pipeline is not None:
                            pipeline.remove_from_scene()
                failures = [
                    record for record in shape_records if record["status"] != "passed"
                ]
                return {
                    "status": "failed" if failures else "passed",
                    "features": shape_records,
                    "feature_count": len(shape_records),
                    "passed_count": len(shape_records) - len(failures),
                    "failed_features": [record["feature"] for record in failures],
                }

            def manual_data_objects() -> dict[str, Any]:
                """Exercise OVITO's non-particle 3-D data-object surfaces."""

                data = DataCollection()
                cell = data.create_cell(
                    [[10, 0, 0, 0], [0, 10, 0, 0], [0, 0, 10, 0]],
                    (False, False, False),
                    vis_params={"enabled": True},
                )
                cell.vis.render_cell = True
                cell.vis.line_width = 0.12

                surface = data.surfaces.create(
                    identifier="audit-surface",
                    title="Audit surface",
                    domain=cell,
                )
                surface.create_vertices(
                    [[1, 1, 1], [5, 1, 1], [1, 5, 1], [1, 1, 5]]
                )
                surface.create_faces(
                    [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]]
                )
                surface.faces.create_property(
                    "Color", data=np.full((4, 3), (0.8, 0.2, 0.2))
                )
                surface.vis.surface_color = (0.8, 0.2, 0.2)
                surface.vis.highlight_edges = True
                surface.vis.wireframe_width = 0.15
                triangle_mesh = data.triangle_meshes.create(identifier="audit-mesh")
                triangle_mesh.set_vertices([[6, 6, 1], [9, 6, 1], [6, 9, 1]])
                triangle_mesh.set_faces([[0, 1, 2]])
                triangle_mesh.vis.color = (0.2, 0.8, 0.2)
                triangle_mesh.vis.highlight_edges = True

                table = data.tables.create(
                    identifier="audit-histogram",
                    title="Audit histogram",
                    plot_mode=DataTable.PlotMode.Histogram,
                    interval=(0.0, 4.0),
                    count=4,
                    axis_label_x="X",
                    axis_label_y="Y",
                )
                table.y = table.create_property("Y", data=[1, 3, 2, 4])

                network = DislocationNetwork()
                network.identifier = "audit-dislocations"
                network.title = "Audit dislocations"
                network.domain = cell
                network.vis.enabled = True
                network.vis.line_width = 0.5
                network.vis.show_burgers_vectors = True
                data.objects.append(network)

                pipeline = Pipeline(source=StaticSource(data=data))
                pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(1.0, 1.0, -1.0),
                )
                viewport.zoom_all(size=(128, 128))
                output = work / "ovito-data-objects.png"
                viewport.render_image(
                    size=(128, 128), filename=str(output), renderer=OSPRayRenderer()
                )
                triangulated, caps, face_map = surface.to_triangle_mesh()
                return {
                    "bytes": output.stat().st_size,
                    "objects": [type(obj).__name__ for obj in data.objects],
                    "surface": {
                        "vertices": surface.topology.vertex_count,
                        "faces": surface.topology.face_count,
                        "triangle_faces": triangulated.face_count,
                        "caps": caps is not None,
                        "face_map_length": len(face_map),
                        "highlight_edges": surface.vis.highlight_edges,
                    },
                    "triangle_mesh": {
                        "vertices": triangle_mesh.vertex_count,
                        "faces": triangle_mesh.face_count,
                        "highlight_edges": triangle_mesh.vis.highlight_edges,
                    },
                    "table": {
                        "rows": table.count,
                        "values": table.y.array.tolist(),
                        "three_d_visual": table.vis is not None,
                    },
                    "dislocation_network": {
                        "present": data.dislocations is not None,
                        "lines": len(network.lines),
                        "segments": len(network.segments),
                        "show_burgers_vectors": network.vis.show_burgers_vectors,
                    },
                    "simulation_cell": {
                        "present": data.cell is not None,
                        "render_cell": cell.vis.render_cell,
                    },
                }

            def dislocation_analysis() -> dict[str, Any]:
                """Run DXA on a deterministic perfect FCC fixture."""

                data = DataCollection()
                cells = 6
                lattice = 2.0
                basis = np.array(
                    [[0, 0, 0], [0.5, 0.5, 0], [0.5, 0, 0.5], [0, 0.5, 0.5]]
                )
                positions = np.array(
                    [
                        lattice * (np.array([i, j, k]) + offset)
                        for i in range(cells)
                        for j in range(cells)
                        for k in range(cells)
                        for offset in basis
                    ]
                )
                data.create_cell(
                    [
                        [cells * lattice, 0, 0, 0],
                        [0, cells * lattice, 0, 0],
                        [0, 0, cells * lattice, 0],
                    ],
                    (True, True, True),
                )
                particles = data.create_particles(count=len(positions))
                particles.create_property("Position", data=positions)
                modifier = DislocationAnalysisModifier(
                    input_crystal_structure=DislocationAnalysisModifier.Lattice.FCC
                )
                modifier.line_smoothing_enabled = False
                modifier.line_coarsening_enabled = False
                pipeline = Pipeline(source=StaticSource(data=data))
                pipeline.modifiers.append(modifier)
                result = pipeline.compute()
                network = result.dislocations
                output = work / "ovito-dislocations.ca"
                export_file(result, str(output), format="ca")
                manual_output = work / "ovito-dislocations-nonzero.ca"
                manual_output.write_text(
                    output.read_text(encoding="utf-8").replace(
                        "DISLOCATIONS 0\nDISLOCATION_JUNCTIONS\n",
                        "DISLOCATIONS 1\n"
                        "0\n"
                        "0.5 0.5 0.0\n"
                        "1\n"
                        "4\n"
                        "1.0 1.0 1.0\n"
                        "2.0 1.5 1.0\n"
                        "3.0 1.0 1.0\n"
                        "2.0 0.5 1.0\n"
                        "DISLOCATION_JUNCTIONS\n"
                        "0 0\n"
                        "0 0\n"
                    ),
                    encoding="utf-8",
                )
                imported_pipeline = import_file(str(manual_output))
                imported = imported_pipeline.compute()
                imported_network = imported.dislocations
                imported_pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(1.0, 1.0, -1.0),
                )
                viewport.zoom_all(size=(128, 128))
                image = work / "ovito-dislocation-line.png"
                viewport.render_image(
                    size=(128, 128), filename=str(image), renderer=OSPRayRenderer()
                )
                return {
                    "atoms": particles.count,
                    "network_present": network is not None,
                    "line_count": len(network.lines) if network is not None else 0,
                    "segment_count": len(network.segments) if network is not None else 0,
                    "total_line_length": result.attributes[
                        "DislocationAnalysis.total_line_length"
                    ],
                    "output_objects": sorted(type(obj).__name__ for obj in result.objects),
                    "ca_bytes": output.stat().st_size,
                    "nonzero_ca": {
                        "bytes": manual_output.stat().st_size,
                        "line_count": len(imported_network.lines)
                        if imported_network is not None
                        else 0,
                        "segment_count": len(imported_network.segments)
                        if imported_network is not None
                        else 0,
                        "line_points": len(imported_network.lines[0].points)
                        if imported_network is not None and imported_network.lines
                        else 0,
                        "png_bytes": image.stat().st_size,
                    },
                    "fixture": "perfect-fcc-zero-line baseline",
                }

            def renderer_parameter_controls() -> dict[str, Any]:
                pipeline.add_to_scene()
                viewport = Viewport(
                    type=Viewport.Type.Perspective,
                    camera_dir=(2.0, 1.0, -1.0),
                )
                viewport.zoom_all(size=(128, 128))
                renderer = OSPRayRenderer()
                renderer.samples_per_pixel = 4
                renderer.dof_enabled = True
                renderer.focal_length = 40.0
                renderer.aperture = 0.5
                renderer.material_type = "principled"
                renderer.principled_metalness = 0.25
                renderer.principled_roughness = 0.6
                output = work / "ovito-renderer-controls.png"
                viewport.render_image(
                    size=(128, 128), filename=str(output), renderer=renderer
                )
                tachyon = TachyonRenderer()
                tachyon.shadows = False
                return {
                    "ospray": {
                        "samples_per_pixel": renderer.samples_per_pixel,
                        "dof_enabled": renderer.dof_enabled,
                        "material_type": renderer.material_type,
                        "metalness": renderer.principled_metalness,
                        "roughness": renderer.principled_roughness,
                    },
                    "tachyon": {"shadows": tachyon.shadows},
                    "png_bytes": output.stat().st_size,
                }

            visual_classes = {
                cls.__name__: cls
                for cls in (
                    AnariRenderer,
                    BondsVis,
                    CalculateDisplacementsModifier,
                    ColorLegendOverlay,
                    ComputePropertyModifier,
                    DislocationAnalysisModifier,
                    DislocationVis,
                    LinesVis,
                    OSPRayRenderer,
                    OpenGLRenderer,
                    ParticlesVis,
                    PythonViewportOverlay,
                    SimulationCellVis,
                    SurfaceMeshVis,
                    TachyonRenderer,
                    TextLabelOverlay,
                    TriangleMeshVis,
                    VectorVis,
                    Viewport,
                    ViewportOverlayInterface,
                    VoxelGridVis,
                )
            }
            probes = [
                run_probe("pipeline-import-and-compute", import_and_compute),
                run_probe("bond-and-color-modifiers", modifiers_and_bonds),
                run_probe("tachyon-still-image", lambda: render(TachyonRenderer(), "ovito-tachyon.png")),
                run_probe("ospray-still-image", lambda: render(OSPRayRenderer(), "ovito-ospray.png")),
                run_probe("opengl-still-image", lambda: render(OpenGLRenderer(), "ovito-opengl.png")),
                run_probe("anari-still-image", render_anari),
                run_probe("render-animation", render_animation),
                run_probe("still-image-format-exports", still_image_format_exports),
                run_probe("pipeline-modifiers-and-displacement", pipeline_modifiers_and_displacement),
                run_probe("manual-visual-objects", manual_visual_objects),
                run_probe("particle-shape-render-matrix", particle_shape_render_matrix),
                run_probe("renderer-parameter-controls", renderer_parameter_controls),
                run_probe(
                    "visual-element-and-renderer-surface",
                    lambda: {"classes": sorted(visual_classes)},
                ),
                run_probe(
                    "manual-surface-mesh-triangle-table-cell-objects", manual_data_objects
                ),
                run_probe("dxa-dislocation-network-and-ca-export", dislocation_analysis),
            ]
            unavailable_probes = [probe["name"] for probe in probes if probe["status"] == "unavailable"]
            failed_probes = [
                probe["name"]
                for probe in probes
                if probe["status"] not in {"passed", "unavailable"}
            ]
            probe_by_name = {probe["name"]: probe for probe in probes}

            def feature_record(
                family: str, feature: str, probe_name: str
            ) -> dict[str, Any]:
                probe = probe_by_name.get(probe_name)
                record: dict[str, Any] = {
                    "family": family,
                    "feature": feature,
                    "status": probe["status"] if probe else "not-run",
                    "probe": probe_name,
                }
                if probe and "error" in probe:
                    record["error"] = probe["error"]
                return record

            feature_probes = {
                "data-object": {
                    "Particles": "manual-visual-objects",
                    "Bonds": "bond-and-color-modifiers",
                    "SimulationCell": "manual-surface-mesh-triangle-table-cell-objects",
                    "SurfaceMesh": "manual-surface-mesh-triangle-table-cell-objects",
                    "TriangleMesh": "manual-surface-mesh-triangle-table-cell-objects",
                    "Lines": "manual-visual-objects",
                    "Vectors": "manual-visual-objects",
                    "DislocationNetwork": "dxa-dislocation-network-and-ca-export",
                    "VoxelGrid": "manual-visual-objects",
                    "DataTable": "manual-surface-mesh-triangle-table-cell-objects",
                },
                "particle-shape": {
                    feature: "particle-shape-render-matrix"
                    for feature in OVITO_PARTICLE_SHAPES
                },
                "pipeline": {
                    "Pipeline": "manual-visual-objects",
                    "StaticSource": "manual-visual-objects",
                    "CreateBondsModifier": "bond-and-color-modifiers",
                    "ColorCodingModifier": "bond-and-color-modifiers",
                    "CalculateDisplacementsModifier": "pipeline-modifiers-and-displacement",
                    "ComputePropertyModifier": "pipeline-modifiers-and-displacement",
                    "DislocationAnalysisModifier": "dxa-dislocation-network-and-ca-export",
                },
                "visual-class": {
                    "ParticlesVis": "manual-visual-objects",
                    "BondsVis": "bond-and-color-modifiers",
                    "DislocationVis": "dxa-dislocation-network-and-ca-export",
                    "SimulationCellVis": "manual-surface-mesh-triangle-table-cell-objects",
                    "SurfaceMeshVis": "manual-surface-mesh-triangle-table-cell-objects",
                    "TriangleMeshVis": "manual-surface-mesh-triangle-table-cell-objects",
                    "LinesVis": "manual-visual-objects",
                    "VectorVis": "pipeline-modifiers-and-displacement",
                    "VoxelGridVis": "manual-visual-objects",
                    "TextLabelOverlay": "manual-visual-objects",
                    "CoordinateTripodOverlay": "manual-visual-objects",
                    "ColorLegendOverlay": "manual-visual-objects",
                    "PythonViewportOverlay": "manual-visual-objects",
                    "Viewport": "tachyon-still-image",
                    "ViewportOverlay": "manual-visual-objects",
                    "ViewportOverlayInterface": "manual-visual-objects",
                },
                "renderer": {
                    "OpenGLRenderer": "opengl-still-image",
                    "TachyonRenderer": "tachyon-still-image",
                    "OSPRayRenderer": "ospray-still-image",
                    "AnariRenderer": "anari-still-image",
                },
                "export": {
                    "PNG": "still-image-format-exports",
                    "JPEG": "still-image-format-exports",
                    "TIFF": "still-image-format-exports",
                    "MP4": "render-animation",
                },
            }
            expected_features = {
                "data-object": OVITO_DATA_OBJECTS,
                "particle-shape": OVITO_PARTICLE_SHAPES,
                "pipeline": OVITO_PIPELINE_CAPABILITIES,
                "visual-class": OVITO_VISUAL_CLASSES,
                "renderer": OVITO_RENDERERS,
                "export": OVITO_EXPORTS,
            }
            for family, expected in expected_features.items():
                observed = set(feature_probes[family])
                if observed != set(expected):
                    raise RuntimeError(
                        f"feature matrix mismatch for {family}: "
                        f"missing={sorted(set(expected) - observed)} "
                        f"extra={sorted(observed - set(expected))}"
                    )
            feature_coverage = [
                feature_record(family, feature, probe_name)
                for family, features in feature_probes.items()
                for feature, probe_name in features.items()
            ]
            feature_status_counts: dict[str, int] = {}
            for record in feature_coverage:
                status = record["status"]
                feature_status_counts[status] = feature_status_counts.get(status, 0) + 1
            return {
                "status": "failed" if failed_probes else "passed-with-unavailable" if unavailable_probes else "passed",
                "scope": "graphics engine; OVITO is a renderer and data pipeline",
                "reference": "https://www.ovito.org/docs/current/python/",
                "version": runtime_version,
                "probe_count": len(probes),
                "passed_probe_count": sum(probe["status"] == "passed" for probe in probes),
                "unavailable_probe_count": len(unavailable_probes),
                "failed_probes": failed_probes,
                "unavailable_probes": unavailable_probes,
                "feature_coverage": feature_coverage,
                "feature_coverage_count": len(feature_coverage),
                "feature_coverage_status_counts": dict(sorted(feature_status_counts.items())),
                "feature_coverage_failed": [
                    record["feature"]
                    for record in feature_coverage
                    if record["status"] == "failed"
                ],
                "feature_coverage_unavailable": [
                    record["feature"]
                    for record in feature_coverage
                    if record["status"] == "unavailable"
                ],
                "probes": probes,
            }
    except Exception as error:
        return {
            "status": "failed",
            "scope": "graphics engine; OVITO is a renderer and data pipeline",
            "error": compact_error(error),
        }
