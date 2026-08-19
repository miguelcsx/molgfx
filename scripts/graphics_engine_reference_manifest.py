"""Static capability manifests for the non-PyMOL graphics references.

The parser and evidence collection live in ``graphics_engine_reference_inventory``.
This module keeps the reviewed capability vocabulary and its ownership decisions
separate from HTML/PDF parsing so the manifest remains auditable as it grows.
"""

from __future__ import annotations

from typing import Any


VMD_STYLES = [
    ("Lines", "bond lines and atom points"),
    ("Bonds", "lighted cylinders"),
    ("DynamicBonds", "distance-based bonds"),
    ("HBonds", "hydrogen-bond display"),
    ("Points", "atom points without bonds"),
    ("VDW", "van der Waals spheres"),
    ("CPK", "scaled spheres with bond cylinders"),
    ("Licorice", "same-radius spheres and cylinders"),
    ("Polyhedra", "polyhedra between atoms within a cutoff"),
    ("Trace", "connected cylinders through C-alpha atoms"),
    ("Tube", "smooth C-alpha tube"),
    ("Ribbons", "flat C-alpha ribbon"),
    ("NewRibbons", "smooth C-alpha ribbon"),
    ("Cartoon", "secondary-structure cylinders and ribbons"),
    ("NewCartoon", "smooth secondary-structure ribbons"),
    ("PaperChain", "ring polygons coloured by ring pucker"),
    ("Twister", "glycosidic-bond ribbon with sugar twists"),
    ("QuickSurf", "Gaussian-density molecular surface"),
    ("MSMS", "MSMS molecular surface"),
    ("Surf", "SURF molecular surface"),
    ("VolumeSlice", "texture-mapped volume slice"),
    ("Isosurface", "volume isovalue surface"),
    ("FieldLines", "integrated volume-gradient field lines"),
    ("Orbital", "wavefunction-selected molecular orbital"),
    ("Beads", "per-residue bounding spheres"),
    ("Dotted", "dotted van der Waals spheres"),
    ("Solvent", "dotted solvent-accessible surface"),
]

VMD_GRAPHICS_PRIMITIVES = [
    "point", "line", "cylinder", "cone", "triangle", "trinorm", "tricolor",
    "sphere", "text",
]
VMD_GRAPHICS_COMMANDS = [
    *VMD_GRAPHICS_PRIMITIVES,
    "color", "materials", "material", "delete", "list", "replace", "exists",
    "info",
]
VMD_CONTROLS = [
    "colors", "materials", "ambient/diffuse/specular/reflection", "opacity",
    "clipping", "depth cueing", "antialiasing", "backface culling", "stereo",
]
VMD_STEREO_MODES = [
    "quad-buffer", "side-by-side", "cross-eyed", "HDTV side-by-side",
    "checkerboard", "column-interleaved", "row-interleaved", "anaglyph",
]
VMD_RENDERERS = ["Tachyon CPU/GPU", "OSPRay", "external renderers"]
VMD_TRAJECTORY_SCENE = [
    "multi-frame drawing", "trajectory smoothing", "unit-cell display",
    "atom/bond/angle/dihedral/spring labels", "graphics primitives",
    "image and movie output",
]

PYMOL_CGO_OPCODES = [
    "ALPHA", "ALPHA_TRIANGLE", "BEGIN", "BEZIER", "CHAR", "COLOR", "CONE",
    "CUSTOM_CYLINDER", "CYLINDER", "DISABLE", "DOTWIDTH", "ELLIPSOID", "ENABLE",
    "END", "FONT", "FONT_AXES", "FONT_SCALE", "FONT_VERTEX", "LIGHTING", "LINES",
    "LINEWIDTH", "LINE_LOOP", "LINE_STRIP", "NORMAL", "NULL", "PICK_COLOR", "POINTS",
    "QUADRIC", "SAUSAGE", "SPHERE", "STOP", "TRIANGLE", "TRIANGLES", "TRIANGLE_FAN",
    "TRIANGLE_STRIP", "VERTEX", "WIDTHSCALE",
]

PYMOL_CGO_DISPOSITIONS = {
    "ALPHA": ("β", "typed material and per-vertex alpha through OIT"),
    "ALPHA_TRIANGLE": ("β", "per-vertex alpha on indexed meshes"),
    "BEGIN": ("β", "typed mesh and polyline constructors replace stream delimiters"),
    "BEZIER": ("β", "bounded cubic Bezier sampling into indirect guides"),
    "CHAR": ("β", "world-space SDF labels"),
    "COLOR": ("β", "typed material and per-vertex color"),
    "CONE": ("β", "caller cone geometry through validated indexed meshes"),
    "CUSTOM_CYLINDER": ("β", "analytic cylinder and per-vertex mesh color paths"),
    "CYLINDER": ("β", "pdviewx analytic capsule/cylinder particle path"),
    "DISABLE": ("β", "typed object visibility state"),
    "DOTWIDTH": ("β", "typed point and guide width"),
    "ELLIPSOID": ("β", "pdviewx analytic anisotropic ellipsoid"),
    "ENABLE": ("β", "typed object visibility state"),
    "END": ("β", "typed mesh and polyline constructors replace stream delimiters"),
    "FONT": ("β", "SDF label font policy"),
    "FONT_AXES": ("β", "world-space label orientation"),
    "FONT_SCALE": ("β", "SDF label scale"),
    "FONT_VERTEX": ("β", "world-space label placement"),
    "LIGHTING": ("β", "typed material and lighting profiles"),
    "LINES": ("β", "open polyline lowering to indirect guides"),
    "LINEWIDTH": ("β", "typed guide width"),
    "LINE_LOOP": ("β", "closed polyline lowering to indirect guides"),
    "LINE_STRIP": ("β", "open polyline lowering to indirect guides"),
    "NORMAL": ("β", "normal-bearing indexed mesh vertices"),
    "NULL": ("out-of-scope", "typed APIs require no stream null sentinel"),
    "PICK_COLOR": ("β", "typed mesh identity in the GPU picking attachment"),
    "POINTS": ("β", "pdviewx analytic point/particle path"),
    "QUADRIC": ("β", "bounded quadratic zero-set lowered to the resident indexed-mesh path"),
    "SAUSAGE": ("β", "analytic spherocylinder particle path"),
    "SPHERE": ("β", "pdviewx analytic sphere particle"),
    "STOP": ("out-of-scope", "typed APIs require no stream stop sentinel"),
    "TRIANGLE": ("β", "validated indexed triangle mesh"),
    "TRIANGLES": ("β", "typed triangle topology constructor"),
    "TRIANGLE_FAN": ("β", "typed triangle-fan topology constructor"),
    "TRIANGLE_STRIP": ("β", "typed triangle-strip topology constructor"),
    "VERTEX": ("β", "typed mesh vertices with position normal color and alpha"),
    "WIDTHSCALE": ("β", "typed guide and particle scale"),
}

CHIMERAX_IMAGE_COMMANDS = [
    "2dlabels", "align", "camera", "cartoon", "ribbon", "clip", "color",
    "rainbow", "coulombic", "distance", "graphics", "hbonds", "key",
    "label", "lighting", "matchmaker", "mmaker", "material", "mlp",
    "move", "nucleotides", "preset", "save", "scalebar", "scenes", "set",
    "show", "hide", "size", "style", "surface", "transparency", "view",
    "volume", "windowsize", "zoom",
]

CHIMERAX_VOLUME_GROUPS = [
    "General Display Options", "Sampling and Size Options",
    "Dimension and Scale Options", "Planes Options",
    "Surface and Mesh Display Options", "Image Display Options",
    "Setting Defaults for New Maps", "Volume Operations (Map Editing)",
    "New-Map Options", "Notes",
]

CHIMERAX_VOLUME_OPERATIONS = [
    "add", "bin", "boxes", "copy", "cover", "erase", "falloff", "flatten",
    "flip", "fourier", "gaussian", "laplacian", "localCorrelation", "mask",
    "maximum", "median", "minimum", "morph", "multiply", "new", "onesmask",
    "octant", "permuteAxes", "resample", "ridges", "scale", "sharpen",
    "splitbyzone", "subtract", "threshold", "tile", "unbend", "unroll",
    "unzone", "zone",
]

CHIMERAX_SURFACE_OPTIONS = [
    "color", "transparency", "enclose", "include", "replace", "probeRadius",
    "resolution", "level", "gridSpacing", "update", "sharpBoundaries",
    "visiblePatches", "cap", "dust", "hidefarblobs", "invertShown", "showall",
    "splitbycolor", "smooth", "squaremesh", "transform", "zone", "style",
    "solid", "mesh", "dot",
]
CHIMERAX_MOVIE_ACTIONS = [
    "record", "encode", "stop", "reset", "abort", "crossfade", "duplicate",
    "ignore", "formats", "status",
]
CHIMERAX_MOVIE_RECORD_OPTIONS = [
    "supersample", "size", "format", "transparentBackground", "directory",
    "pattern", "limit",
]
CHIMERAX_MOVIE_ENCODE_OPTIONS = [
    "output", "verbose", "format", "quality", "qscale", "bitrate", "framerate",
    "roundTrip", "resetMode", "wait",
]

OVITO_DATA_OBJECTS = [
    "Particles", "Bonds", "SimulationCell", "SurfaceMesh", "TriangleMesh",
    "Lines", "Vectors", "DislocationNetwork", "VoxelGrid", "DataTable",
]
OVITO_PARTICLE_SHAPES = [
    "sphere",
    "ellipsoid",
    "superquadric",
    "box",
    "circle",
    "square",
    "cylinder",
    "spherocylinder",
    "mesh",
]
OVITO_PIPELINE_CAPABILITIES = [
    "Pipeline", "StaticSource", "CreateBondsModifier", "ColorCodingModifier",
    "CalculateDisplacementsModifier", "ComputePropertyModifier",
    "DislocationAnalysisModifier",
]
OVITO_VISUAL_CLASSES = [
    "ParticlesVis", "BondsVis", "DislocationVis", "SimulationCellVis",
    "SurfaceMeshVis", "TriangleMeshVis", "LinesVis", "VectorVis", "VoxelGridVis",
    "TextLabelOverlay", "CoordinateTripodOverlay", "ColorLegendOverlay",
    "PythonViewportOverlay", "Viewport", "ViewportOverlay", "ViewportOverlayInterface",
]
OVITO_RENDERERS = ["OpenGLRenderer", "TachyonRenderer", "OSPRayRenderer", "AnariRenderer"]
OVITO_EXPORTS = ["PNG", "JPEG", "TIFF", "MP4"]

# These are the expected emitted record counts, not claims about the number of
# APIs in a vendor product. The check catches an inventory family accidentally
# being declared but omitted from ``build_feature_coverage``.
STATIC_EXPECTED_COUNTS = {
    "PyMOL": len(PYMOL_CGO_OPCODES),
    "VMD": 70,
    "ChimeraX": 134,
    "OVITO": 50,
    "Protein Imager": 55,
}

YASARA_GRAPHICS_SECTIONS = ("File", "View", "Effects", "Window")
YASARA_FEATURE_SECTIONS = (
    "File", "Edit", "Simulation", "Analyze", "View", "Effects", "Options",
    "Window", "Help",
)

PROTEIN_FORMATS = ["PDB", "PDBQT", "CIF", "MMTF", "GRO", "PQR", "SDF", "MOL", "MOL2"]
PROTEIN_REPRESENTATIONS = ["Sphere", "Stick", "Surface", "Mesh", "Simplify", "Filter by distance", "Cartoon", "Tube", "Label"]
PROTEIN_COLORS = ["Goodsell-like", "uniform", "element", "moiety", "proximity", "B-factor"]
PROTEIN_VIEW_CONTROLS = [
    "Real/Goodsell/Outlines presets", "aspect ratio", "fog near/far", "front clipping",
    "zoom/slicing", "orthographic/perspective", "flat color", "material sheen",
    "light intensity", "grayscale", "background color/transparency", "interior color",
    "outlines", "outline color/sensitivity/thickness", "shadowing",
]
PROTEIN_SELECTION = [
    "atom/residue/chain/model picking", "structure hierarchy",
    "entity/proximity/range/property advanced selection", "NMR model selection",
    "sequence selection",
]
PROTEIN_SCENE_CONTROLS = [
    "structure switching", "superimposition", "biological assembly",
    "center/hide/delete", "distance labels/connectors", "membrane bilayer",
    "rock/spin animation",
]
PROTEIN_EXPORTS = [
    "local/server .3dpi projects", "server high-quality image", "VRML2 mesh",
    "download/email notification",
]


def coverage_records(
    source: str,
    family: str,
    names: list[str],
    dispositions: dict[str, tuple[str, str]],
) -> list[dict[str, str]]:
    """Attach an explicit owner and disposition to every enumerated feature."""

    return [
        {
            "source": source,
            "family": family,
            "feature": name,
            "status": dispositions.get(name, ("unreviewed", "audit manifest"))[0],
            "owner": dispositions.get(name, ("unreviewed", "audit manifest"))[1],
        }
        for name in names
    ]


VMD_DISPOSITIONS = {
    "Lines": ("β", "pdviewx bond wires and points"),
    "Bonds": ("β", "pdviewx analytic bond capsules"),
    "DynamicBonds": ("provider", "pdbiox infer_bonds is applied per caller-selected frame"),
    "HBonds": ("β", "caller interaction edges"),
    "Points": ("β", "pdviewx point representation"),
    "VDW": ("β", "pdviewx analytic atom spheres"),
    "CPK": ("β", "pdviewx declarative CPK preset"),
    "Licorice": ("β", "pdviewx declarative Licorice preset"),
    "Polyhedra": ("β", "pdviewx deterministic coordination-shell hull"),
    "Trace": ("β", "pdviewx backbone spline"),
    "Tube": ("β", "pdviewx spline tube"),
    "Ribbons": ("β", "pdviewx spline ribbon"),
    "NewRibbons": ("β", "pdviewx smooth ribbon path"),
    "Cartoon": ("β", "pdviewx secondary-structure cross-sections"),
    "NewCartoon": ("β", "pdviewx smooth cartoon path"),
    "PaperChain": ("β", "pdviewx ring polygons, rounded outlines and caller pucker colors"),
    "Twister": ("β", "pdviewx branched glycosidic-tree ribbon"),
    "QuickSurf": ("β", "pdviewx atom-centred Gaussian field"),
    "MSMS": ("provider", "exact MSMS generation is upstream; pdviewx consumes indexed SES meshes"),
    "Surf": ("provider", "exact SURF generation is upstream; pdviewx consumes indexed SES meshes"),
    "VolumeSlice": ("β", "pdviewx transformed volume slice"),
    "Isosurface": ("β", "pdviewx volume hit path"),
    "FieldLines": ("β", "pdbiox vector-field integration plus pdviewx guide bundles"),
    "Orbital": ("β", "caller signed scalar field rendered as two isosurface lobes"),
    "Beads": ("β", "pdviewx per-residue sphere representation"),
    "Dotted": ("β", "pdviewx procedural surface dots"),
    "Solvent": ("β", "pdviewx SAS/SES boundary paths"),
}

VMD_PRIMITIVE_DISPOSITIONS = {
    "point": ("β", "pdviewx point/particle path"),
    "line": ("β", "pdviewx guide and bond path"),
    "cylinder": ("β", "pdviewx analytic capsule path"),
    "cone": ("β", "caller cone lowered to validated triangle mesh"),
    "triangle": ("β", "validated caller triangle mesh"),
    "trinorm": ("β", "normal-bearing caller triangle mesh"),
    "tricolor": ("β", "per-vertex-color caller triangle mesh"),
    "sphere": ("β", "pdviewx analytic sphere particle"),
    "text": ("β", "pdviewx world-space SDF label"),
}
VMD_GRAPHICS_COMMAND_DISPOSITIONS = {
    **VMD_PRIMITIVE_DISPOSITIONS,
    "color": ("β", "pdviewx typed color/material state"),
    "materials": ("β", "pdviewx typed material state"),
    "material": ("β", "pdviewx typed material state"),
    "delete": ("out-of-scope", "caller scene/object lifecycle"),
    "list": ("out-of-scope", "application introspection"),
    "replace": ("out-of-scope", "application graphics-object editing"),
    "exists": ("out-of-scope", "application introspection"),
    "info": ("out-of-scope", "application introspection"),
}
VMD_CONTROL_DISPOSITIONS = {
    "colors": ("β", "pdviewx color state"),
    "materials": ("β", "pdviewx material state"),
    "ambient/diffuse/specular/reflection": ("β", "pdviewx lighting/material profile"),
    "opacity": ("β", "pdviewx alpha/OIT path"),
    "clipping": ("β", "pdviewx clip set"),
    "depth cueing": ("β", "pdviewx depth-cue profile"),
    "antialiasing": ("β", "wgpu multisample/quality path"),
    "backface culling": ("β", "pdviewx FaceVisibility state"),
    "stereo": ("out-of-scope", "host display integration"),
}
VMD_STEREO_DISPOSITIONS = {name: ("out-of-scope", "host display integration") for name in VMD_STEREO_MODES}
VMD_RENDERER_DISPOSITIONS = {
    "Tachyon CPU/GPU": ("out-of-scope", "caller-owned external renderer adapter"),
    "OSPRay": ("out-of-scope", "caller-owned external renderer adapter"),
    "external renderers": ("out-of-scope", "caller-owned renderer adapter"),
}
VMD_TRAJECTORY_DISPOSITIONS = {
    "multi-frame drawing": ("β", "pdviewx topology-stable trajectory"),
    "trajectory smoothing": ("β", "pdbiox bounded linear/Catmull-Rom interpolation"),
    "unit-cell display": ("β", "pdviewx crystallographic scene state"),
    "atom/bond/angle/dihedral/spring labels": ("β", "typed guides including spring pattern"),
    "graphics primitives": ("β", "typed analytic primitives and arbitrary indexed meshes"),
    "image and movie output": ("out-of-scope", "pdviewx returns frames; codecs stay caller-owned"),
}


def chimerax_command_disposition(name: str) -> tuple[str, str]:
    if name in {"2dlabels", "key", "scalebar"}:
        return ("β", "pdviewx post-tonemap typed screen overlays")
    if name == "graphics":
        return ("β", "pdviewx typed primitives and indexed meshes")
    if name in {"align", "matchmaker", "mmaker"}:
        return ("out-of-scope", "caller/provider structure comparison")
    if name == "preset":
        return ("out-of-scope", "application preset workflow")
    if name in {"save", "scenes"}:
        return ("β", "pdviewx versioned scene manifests and render sessions")
    return ("β", "pdviewx typed scene/render state")


def chimerax_surface_disposition(name: str) -> tuple[str, str]:
    if name in {"enclose", "include", "smooth", "squaremesh"}:
        return ("provider", "caller/provider surface generation or topology operation")
    if name in {"replace", "update"}:
        return ("β", "mutable mesh slots with revision-driven resident GPU updates")
    if name == "sharpBoundaries":
        return ("β", "caller normals and indexed boundaries are preserved")
    return ("β", "pdviewx surface field, component, clipping and render state")


OVITO_DISPOSITIONS = {
    **{name: ("β", "pdviewx molecular semantic equivalent") for name in ("Particles", "Bonds", "SimulationCell", "Lines", "VoxelGrid")},
    "SurfaceMesh": ("β", "pdviewx validated caller mesh"),
    "TriangleMesh": ("β", "pdviewx validated caller triangle mesh"),
    "Vectors": ("β", "pdbiox vector grids lower to pdviewx streamline guide bundles"),
    "DislocationNetwork": ("provider", "caller/provider crystal-defect topology"),
    "DataTable": ("out-of-scope", "caller analysis/data pipeline"),
    "Pipeline": ("out-of-scope", "caller-owned generic data pipeline"),
    "StaticSource": ("out-of-scope", "caller-owned generic data pipeline"),
    "CreateBondsModifier": ("provider", "caller/provider topology construction"),
    "ColorCodingModifier": ("β", "pdviewx property/color mapping"),
    "CalculateDisplacementsModifier": ("provider", "caller/provider trajectory analysis"),
    "ComputePropertyModifier": ("provider", "caller/provider derived properties"),
    "DislocationAnalysisModifier": ("provider", "caller/provider crystal-defect analysis"),
    "ParticlesVis": ("β", "pdviewx analytic particles"),
    "BondsVis": ("β", "pdviewx analytic bonds"),
    "DislocationVis": ("β", "typed polyline and tube styling for caller defect networks"),
    "SimulationCellVis": ("β", "pdviewx crystallographic cell state"),
    "SurfaceMeshVis": ("β", "pdviewx persistent caller mesh slot"),
    "TriangleMeshVis": ("β", "pdviewx persistent caller mesh slot"),
    "LinesVis": ("β", "pdviewx guides and lines"),
    "VectorVis": ("β", "pdviewx batched streamline guide styling"),
    "VoxelGridVis": ("β", "pdviewx density/segmentation volume"),
    "TextLabelOverlay": ("β", "pdviewx typed text screen overlay"),
    "CoordinateTripodOverlay": ("β", "pdviewx coordinate tripod overlay"),
    "ColorLegendOverlay": ("β", "pdviewx color legend overlay"),
    "PythonViewportOverlay": ("out-of-scope", "host scripting/overlay integration"),
    "Viewport": ("β", "pdviewx off-screen render target"),
    "ViewportOverlay": ("β", "pdviewx post-tonemap overlay table"),
    "ViewportOverlayInterface": ("out-of-scope", "host scripting/overlay integration"),
    "sphere": ("β", "pdviewx analytic sphere particles"),
    "ellipsoid": ("β", "pdviewx anisotropic ellipsoid particles"),
    "superquadric": ("β", "pdviewx bounded analytic superquadric particle"),
    "box": ("β", "pdviewx analytic box particles"),
    "circle": ("β", "pdviewx billboard circle particle"),
    "square": ("β", "pdviewx billboard square particle"),
    "cylinder": ("β", "pdviewx analytic cylinder particles"),
    "spherocylinder": ("β", "pdviewx analytic spherocylinder particles"),
    "mesh": ("β", "pdviewx validated caller mesh"),
    "OpenGLRenderer": ("out-of-scope", "backend selection is not a renderer capability contract"),
    "TachyonRenderer": ("out-of-scope", "caller-owned external renderer adapter"),
    "OSPRayRenderer": ("out-of-scope", "caller-owned external renderer adapter"),
    "AnariRenderer": ("out-of-scope", "caller-owned external renderer adapter"),
    "PNG": ("β", "pdviewx image output"),
    "JPEG": ("out-of-scope", "caller-owned image encoder"),
    "TIFF": ("out-of-scope", "caller-owned image encoder"),
    "MP4": ("out-of-scope", "caller-owned movie encoder"),
}


def yasara_command_disposition(section: str, name: str, description: str) -> tuple[str, str]:
    text = f"{name} {description}".lower()
    visual_tokens = (
        "show", "color", "surf", "atom", "ball", "stick", "label", "cut",
        "light", "shadow", "fog", "ray", "render", "camera", "projection",
        "stereo", "image", "polygon", "sphere", "cone", "arrow", "plane",
        "torus", "texture", "mesh", "background", "movie", "mpg", "png",
        "bmp", "pov", "obj",
    )
    if section == "Window":
        return ("out-of-scope", "desktop window and application shell")
    if section in {"View", "Effects"} or any(token in text for token in visual_tokens):
        if any(token in text for token in ("pov", "obj", "mpg", "movie", "plugin", "macro")):
            return ("out-of-scope", "downstream exporter or application workflow")
        return ("β", "typed pdviewx scene/render state; exact YASARA response unverified")
    if section in {"Analyze", "Simulation"}:
        return ("provider", "caller/provider analysis or simulation")
    return ("out-of-scope", "caller/provider or application workflow")


def build_feature_coverage(yasara: dict[str, Any]) -> list[dict[str, str]]:
    """Return one auditable disposition for each source-inventory item."""

    records: list[dict[str, str]] = []
    static_groups = (
        ("PyMOL", "cgo-opcode", PYMOL_CGO_OPCODES, PYMOL_CGO_DISPOSITIONS),
        ("VMD", "representation", [name for name, _description in VMD_STYLES], VMD_DISPOSITIONS),
        ("VMD", "graphics-primitive", VMD_GRAPHICS_PRIMITIVES, VMD_PRIMITIVE_DISPOSITIONS),
        (
            "VMD",
            "graphics-command",
            [name for name in VMD_GRAPHICS_COMMANDS if name not in VMD_GRAPHICS_PRIMITIVES],
            VMD_GRAPHICS_COMMAND_DISPOSITIONS,
        ),
        ("VMD", "control", VMD_CONTROLS, VMD_CONTROL_DISPOSITIONS),
        ("VMD", "stereo-mode", VMD_STEREO_MODES, VMD_STEREO_DISPOSITIONS),
        ("VMD", "renderer", VMD_RENDERERS, VMD_RENDERER_DISPOSITIONS),
        ("VMD", "trajectory-scene", VMD_TRAJECTORY_SCENE, VMD_TRAJECTORY_DISPOSITIONS),
        ("ChimeraX", "image-command", CHIMERAX_IMAGE_COMMANDS, {name: chimerax_command_disposition(name) for name in CHIMERAX_IMAGE_COMMANDS}),
        ("ChimeraX", "volume-option-group", CHIMERAX_VOLUME_GROUPS, {
            name: (
                "out-of-scope" if name == "Volume Operations (Map Editing)" else "β",
                "caller/provider map editing" if name == "Volume Operations (Map Editing)"
                else "pdviewx volume input/display state",
            )
            for name in CHIMERAX_VOLUME_GROUPS
        }),
        ("ChimeraX", "volume-operation", CHIMERAX_VOLUME_OPERATIONS, {name: ("out-of-scope", "caller/provider map editing") for name in CHIMERAX_VOLUME_OPERATIONS}),
        ("ChimeraX", "surface-option", CHIMERAX_SURFACE_OPTIONS, {
            name: chimerax_surface_disposition(name) for name in CHIMERAX_SURFACE_OPTIONS
        }),
        ("ChimeraX", "movie", CHIMERAX_MOVIE_ACTIONS, {
            name: ("β", "pdviewx image configuration") if name in {"size", "transparentBackground"}
            else ("out-of-scope", "caller-owned movie timeline/encoder")
            for name in CHIMERAX_MOVIE_ACTIONS
        }),
        ("ChimeraX", "movie-record-option", CHIMERAX_MOVIE_RECORD_OPTIONS, {
            name: ("β", "pdviewx image configuration") if name in {"size", "transparentBackground"}
            else ("out-of-scope", "caller-owned movie timeline/encoder")
            for name in CHIMERAX_MOVIE_RECORD_OPTIONS
        }),
        ("ChimeraX", "movie-encode-option", CHIMERAX_MOVIE_ENCODE_OPTIONS, {
            name: ("out-of-scope", "caller-owned movie timeline/encoder")
            for name in CHIMERAX_MOVIE_ENCODE_OPTIONS
        }),
        ("Protein Imager", "format", PROTEIN_FORMATS, {name: ("provider", "caller/provider input adapter") for name in PROTEIN_FORMATS}),
        ("Protein Imager", "representation", PROTEIN_REPRESENTATIONS, {
            "Sphere": ("β", "pdviewx analytic particles"), "Stick": ("β", "pdviewx analytic bonds"),
            "Surface": ("β", "pdviewx surface field"), "Mesh": ("β", "pdviewx typed mesh instances"),
            "Simplify": ("provider", "caller/provider mesh reduction"), "Filter by distance": ("β", "pdviewx selection/spatial query"),
            "Cartoon": ("β", "pdviewx cartoon geometry"), "Tube": ("β", "pdviewx spline tube"),
            "Label": ("β", "pdviewx world-space labels"),
        }),
        ("Protein Imager", "coloring", PROTEIN_COLORS, {name: ("β", "pdviewx typed material/property color") for name in PROTEIN_COLORS}),
        ("Protein Imager", "view-control", PROTEIN_VIEW_CONTROLS, {name: ("β", "pdviewx camera/material/profile state") for name in PROTEIN_VIEW_CONTROLS}),
        ("Protein Imager", "selection", PROTEIN_SELECTION, {name: ("β", "pdviewx selection/picking; provider hierarchy") for name in PROTEIN_SELECTION}),
        ("Protein Imager", "scene-control", PROTEIN_SCENE_CONTROLS, {
            "distance labels/connectors": ("β", "pdviewx measurements/guides"),
            "rock/spin animation": ("β", "pdviewx camera/trajectory host"),
            "membrane bilayer": ("provider", "caller/provider domain geometry composed as meshes or particles"),
            "structure switching": ("β", "pdviewx multi-structure scene"),
            "superimposition": ("provider", "caller/provider alignment"),
            "biological assembly": ("provider", "pdbiox assembly instances"),
            "center/hide/delete": ("β", "pdviewx scene state/camera"),
        }),
        ("Protein Imager", "export", PROTEIN_EXPORTS, {
            "local/server .3dpi projects": ("out-of-scope", "host-owned project format and service"),
            "server high-quality image": ("out-of-scope", "host service"),
            "VRML2 mesh": ("out-of-scope", "downstream mesh exporter"),
            "download/email notification": ("out-of-scope", "host service"),
        }),
    )
    for source, family, names, dispositions in static_groups:
        records.extend(coverage_records(source, family, names, dispositions))
    for family, names in (
        ("data-object", OVITO_DATA_OBJECTS),
        ("particle-shape", OVITO_PARTICLE_SHAPES),
        ("pipeline", OVITO_PIPELINE_CAPABILITIES),
        ("visual-class", OVITO_VISUAL_CLASSES),
        ("renderer", OVITO_RENDERERS),
        ("export", OVITO_EXPORTS),
    ):
        records.extend(coverage_records("OVITO", family, names, OVITO_DISPOSITIONS))
    for section, commands in yasara.get("commands_by_section", {}).items():
        for command in commands:
            status, owner = yasara_command_disposition(section, command["name"], command["description"])
            records.append({
                "source": "YASARA",
                "family": section,
                "feature": command["name"],
                "status": status,
                "owner": owner,
            })
    return records
