// Bundled against the requested local Mol* source checkout; runs in a real DOM.
const { PluginContext } = require('@molstar/mol-plugin/context');
const { DefaultPluginSpec } = require('@molstar/mol-plugin/spec');
const { ParamDefinition: PD } = require('@molstar/mol-util/param-definition');
const { Canvas3DParams } = require('@molstar/mol-canvas3d/canvas3d');
const { RuntimeContext } = require('@molstar/mol-task');
const { SecondaryStructureProvider } = require('@molstar/mol-model-props/computed/secondary-structure');
const { computeUnitDSSP, DefaultDSSPComputationProps } = require("@molstar/mol-model-props/computed/secondary-structure/dssp");
const { SecondaryStructureType, BondType } = require('@molstar/mol-model/structure/model/types');
const { Ccp4Provider } = require('@molstar/mol-plugin-state/formats/volume');
const { createVolumeRepresentationParams } = require('@molstar/mol-plugin-state/helpers/volume-representation-params');
const { StateTransforms } = require('@molstar/mol-plugin-state/transforms');
const { Grid } = require('@molstar/mol-model/volume/grid');
const { Mat4 } = require('@molstar/mol-math/linear-algebra');
const { PixelData } = require('@molstar/mol-util/image');
const { Script } = require('@molstar/mol-script/script');
const { MolScriptBuilder } = require('@molstar/mol-script/language/builder');
const { StructureSelection, StructureElement } = require('@molstar/mol-model/structure');
const { getElementMoleculeType } = require('@molstar/mol-model/structure/util');
const { MoleculeType } = require('@molstar/mol-model/structure/model/types');
const segmentation = require("./molstar-segmentation.cjs");
const effects = require("./molstar-effects.cjs");

function secondaryState(flag, available, unit, element) {
    const f = SecondaryStructureType.Flag;
    if (!available) return 'unknown';
    if (flag & f.Helix) {
        if (flag & f.Helix3Ten) return 'three_ten_helix';
        if (flag & f.HelixPi) return 'pi_helix';
        if (flag & f.HelixPolyproline) return 'polyproline';
        if (flag & f.HelixAlpha) return 'alpha_helix';
        return 'other_helix';
    }
    if (flag & f.Beta) {
        if (flag & f.BetaSheet) return 'strand';
        if (flag & f.BetaStrand) return 'beta_bridge';
        return 'other_beta';
    }
    if (flag & f.Bend) return 'bend';
    if (flag & f.Turn) return 'turn';
    return getElementMoleculeType(unit, element) === MoleculeType.Protein ? 'coil' : 'unknown';
}

// Each atom entry selects auth ids against the state structure.
function atomLoci(structure, atom) {
    const MS = MolScriptBuilder, P = MS.struct.atomProperty.macromolecular;
    const expr = MS.struct.generator.atomGroups({
        'chain-test': MS.core.rel.eq([P.auth_asym_id(), atom.auth_asym_id]),
        'residue-test': MS.core.rel.eq([P.auth_seq_id(), atom.auth_seq_id]),
        'atom-test': MS.core.rel.eq([P.auth_atom_id(), atom.auth_atom_id]) });
    const loci = StructureSelection.toLociWithSourceUnits(Script.getStructureSelection(expr, structure));
    if (StructureElement.Loci.isEmpty(loci)) throw new Error(`Mol* script atom not found: ${JSON.stringify(atom)}`);
    return loci;
}

async function parseStructure(plugin, data) {
    const trajectory = await plugin.builders.structure.parseTrajectory(data, 'mmcif');
    const model = await plugin.builders.structure.createModel(trajectory, { modelIndex: 0 });
    const structure = await plugin.builders.structure.createStructure(model, { name: 'model', params: {} });
    await SecondaryStructureProvider.attach({ runtime: RuntimeContext.Synchronous, assetManager: plugin.managers.asset }, structure.data);
    return structure;
}

// Each entry is literal Mol* input; an unknown kind is an error, never skipped.
async function applyScript(plugin, object, entries) {
    let last;
    for (const entry of entries) {
        switch (entry.kind) {
            case 'representation':
                last = await plugin.builders.structure.representation.addRepresentation(object, entry.params);
                break;
            case 'volume':
                last = await plugin.build().to(object).apply(StateTransforms.Representation.VolumeRepresentation3D,
                    createVolumeRepresentationParams(plugin, object.data, entry.params)).commit();
                break;
            case 'measurement': {
                const loci = entry.atoms.map(a => atomLoci(object.data, a));
                const m = plugin.managers.structure.measurement;
                const add = { distance: () => m.addDistance(...loci), angle: () => m.addAngle(...loci),
                    dihedral: () => m.addDihedral(...loci) }[entry.measure];
                if (!add) throw new Error(`unknown Mol* measurement ${entry.measure}`);
                await add();
                last = last || { data: true };
                break;
            }
            case 'label':
                await plugin.managers.structure.measurement.addLabel(atomLoci(object.data, entry.atoms[0]));
                last = last || { data: true };
                break;
            default:
                throw new Error(`unknown Mol* script entry kind ${entry.kind}`);
        }
    }
    return last;
}

async function execute(request, bytes) {
    const { catalog, fixture, recipe } = request;
    if (fixture.format === 'mrc' &&
        (!Array.isArray(request.voxel_to_world) || request.voxel_to_world.length !== 16 ||
        !request.voxel_to_world.every(Number.isFinite))) {
        throw new Error('MRC comparison requires the canonical voxel-to-world affine');
    }
    const [width, height] = catalog.extent;
    const settings = catalog.recipe_settings[recipe];
    const canvas = PD.getDefaultValues(Canvas3DParams);
    Object.assign(canvas.renderer, settings.renderer);
    Object.assign(canvas.postprocessing, settings.postprocessing);
    canvas.camera.manualReset = true;
    canvas.camera.helper.axes = { name: 'off', params: {} };
    canvas.cameraFog = { name: 'off', params: {} };
    const effectEvidence = effects.configure(canvas, fixture.effect);
    canvas.cameraResetDurationMs = 0;
    const spec = DefaultPluginSpec();
    spec.canvas3d = canvas;
    const plugin = new PluginContext(spec);
    try {
        await plugin.init();
        const container = document.getElementById('viewport');
        const element = document.getElementById('molstar');
        container.style.width = `${width}px`;
        container.style.height = `${height}px`;
        if (!await plugin.initViewerAsync(element, container)) throw new Error('Real Chromium WebGL initialization failed');
        plugin.animationLoop.stop();
        plugin.canvas3d.pause(true);
        const webgl = plugin.canvas3d.webgl;
        const gl = webgl.gl;
        if (!webgl.isWebGL2) throw new Error('WebGL2 required; WebGL1 fallback refused');
        const data = await plugin.builders.data.rawData({ data: fixture.format === 'mrc' ? bytes : new TextDecoder().decode(bytes) });
        let object, metadata, representation, categoricalMaps;
        if (fixture.segmentations) {
            categoricalMaps = await segmentation.prepare(plugin, data, fixture, request.voxel_to_world);
            metadata = {
                dimensions: categoricalMaps[0].object.data.grid.cells.space.dimensions,
                segmentation_thresholds: fixture.segmentation_thresholds,
                segmentations: categoricalMaps.map(({ spec, counts, matrix }) => ({ name: spec.name, label_counts: counts, voxel_to_world: matrix })),
                representation_policy: "Mol* native segment surfaces; not optical ray-volume equivalence",
            };
        } else if (fixture.format === 'mrc') {
            object = (await Ccp4Provider.parse(plugin, data)).volume;
            const grid = object.data.grid;
            const parsedTransform = grid.transform;
            const correction = Mat4.mul(Mat4(), request.voxel_to_world,
                Mat4.invert(Mat4(), Grid.getGridToCartesianTransform(grid)));
            object = await plugin.build().to(object).apply(StateTransforms.Volume.VolumeTransform,
                { transform: { name: 'matrix', params: { data: correction, transpose: false } } }).commit();
            const canonicalGrid = object.data.grid;
            // These fixtures are finite scalar grids, not periodic crystal maps.
            canonicalGrid.periodicity = 'none';
            metadata = { dimensions: grid.cells.space.dimensions, voxels: grid.cells.data.length,
                value_range: [grid.stats.min, grid.stats.max], affine: canonicalGrid.transform,
                parsed_affine: parsedTransform, periodicity: canonicalGrid.periodicity,
                affine_provenance: 'MolFrame canonical affine supplied to Mol* matrix grid; unmodified CCP4 scalar values' };
        } else {
            object = await parseStructure(plugin, data);
            const structure = object.data;
            const assignments = SecondaryStructureProvider.get(structure).value;
            const ss = {}, orders = {}, provenance = {};
            const secondaryResidues = [], computedDssp = {}, revisedDssp = {};
            const computedResidues = [], revisedResidues = [];
            let aromatic = 0;
            for (const unit of structure.units) {
                if (unit.kind !== 0) throw new Error('Coarse unit in atom-only corpus');
                const residues = new Map();
                for (const element of unit.elements) residues.set(unit.residueIndex[element], element);
                const secondary = assignments?.get(unit.invariantId);
                const computed = await computeUnitDSSP(unit, DefaultDSSPComputationProps);
                const revised = await computeUnitDSSP(unit, { oldDefinition: false, oldOrdering: false });
                for (const [residue, element] of residues) {
                    const flag = secondary ? secondary.type[secondary.getIndex(residue)] : 0;
                    const code = secondaryState(flag, !!secondary, unit, element);
                    ss[code] = (ss[code] || 0) + 1;
                    const h = unit.model.atomicHierarchy;
                    const chain = h.chains.label_asym_id.value(unit.chainIndex[element]);
                    const sequence = h.residues.label_seq_id.value(residue);
                    const computedCode = secondaryState(computed.type[computed.getIndex(residue)], true, unit, element);
                    const revisedCode = secondaryState(revised.type[revised.getIndex(residue)], true, unit, element);
                    computedDssp[computedCode] = (computedDssp[computedCode] || 0) + 1;
                    revisedDssp[revisedCode] = (revisedDssp[revisedCode] || 0) + 1;
                    if (sequence > 0) {
                        secondaryResidues.push([chain, sequence, code]);
                        computedResidues.push([chain, sequence, computedCode]);
                        revisedResidues.push([chain, sequence, revisedCode]);
                    }
                }
                const bonds = unit.bonds;
                for (let i = 0; i < bonds.a.length; i++) {
                    if (bonds.a[i] >= bonds.b[i]) continue;
                    const order = bonds.edgeProps.order[i];
                    const flag = bonds.edgeProps.flags[i];
                    orders[String(order)] = (orders[String(order)] || 0) + 1;
                    provenance[String(flag)] = (provenance[String(flag)] || 0) + 1;
                    if (flag & BondType.Flag.Aromatic) aromatic++;
                }
            }
            metadata = { atoms: structure.elementCount, residues: structure.atomicResidueCount, bonds: structure.bondCount,
                bond_orders: orders, aromatic_bonds: aromatic, bond_provenance: provenance,
                bond_provenance_encoding: 'Mol* BondType flags for intra-unit bonds; inter-unit total included in bond count',
                secondary_structure: ss, secondary_structure_policy: 'Mol* auto: model/file or DSSP/Zhang-Skolnick',
                secondary_residues: secondaryResidues,
                computed_dssp: { policy: "Mol* computeUnitDSSP defaults; oldDefinition=true, oldOrdering=true; per-unit H-bonds", counts: computedDssp, residues: computedResidues },
                revised_dssp: { policy: "Mol* computeUnitDSSP; oldDefinition=false, oldOrdering=false; per-unit H-bonds", counts: revisedDssp, residues: revisedResidues },
                secondary_structure_encoding: 'exact Mol* subtype flags; other_beta retained rather than relabelled as strand' };
        }
        metadata.source_sha256 = fixture.sha256;
        const response = { engine: 'molstar', metadata };
        if (request.inspect) return response;
        const style = catalog.style;
        const color = (style.color_rgb[0] << 16) | (style.color_rgb[1] << 8) | style.color_rgb[2];
        if (categoricalMaps) {
            representation = await segmentation.representations(plugin, categoricalMaps);
        } else if (fixture.script) {
            representation = await applyScript(plugin, object, fixture.script.molstar);
        } else if (fixture.format === 'mrc') {
            if (fixture.form === 'iso_dots' || fixture.form === 'region') {
                throw new Error('Mol* adapter does not implement exact voxel dots or crop');
            }
            const type = fixture.form === 'direct' ? 'direct-volume' : fixture.form === 'slice' ? 'slice' : 'isosurface';
            const typeParams = type === 'isosurface' ? { isoValue: { kind: 'absolute', absoluteValue: fixture.isovalue }, alpha: fixture.opacity ?? style.opacity, quality: 'highest', visuals: fixture.form === 'iso_mesh' ? ['wireframe'] : ['solid'] } : { alpha: style.opacity, quality: 'highest' };
            if (fixture.form === 'direct') {
                const { min, max } = object.data.grid.stats;
                const scalarEnd = fixture.isovalue * 2;
                const normalizedEnd = (scalarEnd - min) / (max - min);
                if (!(normalizedEnd > 0 && normalizedEnd < 1)) throw new Error('Direct-volume reference requires transfer endpoint inside scalar range');
                typeParams.controlPoints = [[0, 0], [normalizedEnd, 0.8], [1, 0.8]];
                metadata.optical_transfer = {
                    scalar_points: [[0, 0], [scalarEnd, 0.8]], color_rgb: style.color_rgb,
                    reference_control_points: typeParams.controlPoints,
                    difference: 'Mol* uses Catmull-Rom alpha texture and per-step alpha scaling; native uses piecewise-linear extinction with opacity_scale=2 and step_scale=0.65. These transport laws are not identical.',
                };
            }
            if (fixture.form === 'iso_mesh') {
                const matrix = request.voxel_to_world;
                const voxelSize = Math.min(...[0, 1, 2].map(axis => Math.hypot(...matrix.slice(axis * 4, axis * 4 + 3))));
                const worldWidth = 2 * fixture.line_width_voxels * voxelSize;
                // Mol* attenuated lines multiply size by five and half the viewport.
                typeParams.sizeFactor = worldWidth / (5 * Math.tan(fixture.camera.fov_y_degrees * Math.PI / 360));
                typeParams.lineSizeAttenuation = true;
                metadata.wireframe_width = { world_angstrom: worldWidth, size_factor: typeParams.sizeFactor, minimum_physical_pixels: 1 };
            }
            let colorTheme = 'uniform';
            let colorParams = { value: color };
            if (type === 'slice') {
                Object.assign(typeParams, {
                    mode: 'plane', plane: { point: fixture.slice_point, normal: fixture.slice_normal },
                    isoValue: { kind: 'absolute', absoluteValue: 0 },
                });
                colorTheme = 'volume-value';
                colorParams = {
                    colorList: { kind: 'interpolate', colors: fixture.slice_colors },
                    domain: { name: 'custom', params: fixture.slice_domain }, isRelative: false,
                };
                metadata.slice = { point: fixture.slice_point, normal: fixture.slice_normal, domain: fixture.slice_domain, colors: fixture.slice_colors };
            }
            representation = await plugin.build().to(object).apply(StateTransforms.Representation.VolumeRepresentation3D,
                createVolumeRepresentationParams(plugin, object.data, { type, typeParams, color: colorTheme, colorParams })).commit();
            if (fixture.structure_file) {
                const response = await fetch('/structure');
                if (!response.ok) throw new Error('Hashed overlay structure could not be fetched');
                const structureData = await plugin.builders.data.rawData({ data: await response.text() });
                const structure = await parseStructure(plugin, structureData);
                await plugin.builders.structure.representation.addRepresentation(structure, {
                    type: 'cartoon', typeParams: { quality: 'highest' },
                    color: 'uniform', colorParams: { value: 0x59616e } });
                metadata.overlay_atoms = structure.data.elementCount;
            }

        } else {
            const type = fixture.form === 'ball_and_stick' ? 'ball-and-stick' : fixture.form;
            representation = await plugin.builders.structure.representation.addRepresentation(object, {
                type, typeParams: { sizeFactor: style.atom_radius_scale, alpha: style.opacity, quality: 'highest', ignoreHydrogens: false },
                color: 'uniform', colorParams: { value: color }, size: 'physical' });
        }
        if (!representation?.data) throw new Error('Mol* representation failed: ' + JSON.stringify(plugin.log.entries.toArray().map(entry => entry.message)));
        if (fixture.form === 'slice') {
            metadata.slice_geometry = representation.data.repr.renderObjects.map(object => ({
                position: Array.from(object.values.aPosition?.ref.value ?? []),
                transform: Array.from(object.values.aTransform?.ref.value ?? []),
                model: Array.from(object.values.uModel?.ref.value ?? []),
            }));
        }
        plugin.canvas3d.commit(true);
        const c = fixture.camera;
        const distance = Math.hypot(...c.position.map((v, i) => v - c.target[i]));
        if (!(distance > c.near && c.far > distance)) throw new Error('Camera clipping planes cannot be represented by the Mol* radius contract');
        plugin.canvas3d.camera.setState({ mode: 'perspective', position: c.position, target: c.target, up: c.up,
            fov: c.fov_y_degrees * Math.PI / 180, radius: distance - c.near, radiusMax: c.far - distance,
            clipFar: false, minNear: c.near, minFar: c.far - distance, fog: canvas.cameraFog.name === 'on' ? canvas.cameraFog.params.intensity : 0 }, 0);
        plugin.canvas3d.camera.update();
        const pass = plugin.canvas3d.getImagePass({ renderer: plugin.canvas3d.props.renderer,
            postprocessing: plugin.canvas3d.props.postprocessing, multiSample: settings.multiSample,
            illumination: { ...PD.getDefaultValues(Canvas3DParams).illumination, ...settings.illumination },
            cameraHelper: plugin.canvas3d.props.camera.helper });
        pass.setSize(width, height);
        const illumination = pass.illuminationPass;
        if (settings.illumination.enabled && !illumination.supported) throw new Error('Mol* tracing unsupported by real WebGL extensions; raster fallback refused');
        const aa = pass.props.postprocessing.antialiasing;
        const smaa = pass.drawPass.antialiasing.smaa;
        if (aa.name === 'smaa') {
            if (!smaa.supported) throw new Error('Requested SMAA unsupported; no AA fallback allowed');
            const textures = smaa.weightsRenderable.values;
            const deadline = performance.now() + 10000;
            while (textures.tArea.ref.value.getWidth() <= 1 || textures.tSearch.ref.value.getWidth() <= 1) {
                if (performance.now() > deadline) throw new Error('SMAA lookup images failed to load in the real DOM');
                await new Promise(resolve => setTimeout(resolve, 10));
            }
        }
        await pass.updateBackground();
        async function completed() {
            const start = performance.now();
            await pass.render(RuntimeContext.Synchronous);
            gl.finish();
            const elapsed = Math.round((performance.now() - start) * 1e6);
            if (gl.isContextLost()) throw new Error('WebGL context lost during completed output');
            const error = gl.getError();
            if (error !== gl.NO_ERROR) throw new Error(`WebGL error after completed output: ${error}`);
            const iterations = settings.illumination.enabled ? illumination.iteration : 0;
            if (settings.illumination.enabled && iterations !== illumination.getMaxIterations(pass.props.illumination)) throw new Error(`Incomplete tracing: ${iterations}`);
            return { sample: { cpu_ns: elapsed, frame_ns: elapsed, gpu_ns: null }, tracing_iterations: iterations,
                gpu_unavailable_reason: 'Blocking ImagePass plus GL finish wall time; no exclusive GPU timestamp' };
        }
        response.cold_output = await completed();
        for (let i = 0; i < catalog.warmup_outputs; i++) await completed();
        response.samples = [];
        for (let i = 0; i < catalog.measured_outputs; i++) response.samples.push(await completed());
        pass.colorTarget.bind();
        const pixels = new Uint8Array(width * height * 4);
        webgl.readPixels(0, 0, width, height, pixels);
        const image = PixelData.create(pixels, width, height);
        PixelData.flipY(image);
        PixelData.divideByAlpha(image);
        const capture = document.createElement('canvas');
        capture.width = width;
        capture.height = height;
        capture.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0);
        capture.id = 'completed-output';
        document.body.appendChild(capture);
        response.png_data_url = capture.toDataURL('image/png');
        response.image = 'image.png';
        response.measurement_scope = 'Completed ImagePass draw/multisample/tracing plus GL finish; readback/PNG excluded';
        response.cpu_timing_scope = 'wall-including-GPU-completion-not-exclusive';
        const debug = gl.getExtension('WEBGL_debug_renderer_info');
        response.effective_settings = { canvas: plugin.canvas3d.props, imagePass: pass.props,
            effect: effectEvidence, requested: settings, requested_camera: c, camera: plugin.canvas3d.camera.getSnapshot(),
            view: Array.from(pass._camera.view), projection: Array.from(pass._camera.projection),
            near: pass._camera.near, far: pass._camera.far, extent: [pass.width, pass.height],
            representation: representation.cell.params.values,
            antialiasing: { requested: aa, applied: pass.props.postprocessing.enabled ? aa.name : 'off', smaa_supported: smaa.supported },
            tracing: { supported: illumination.supported, enabled: settings.illumination.enabled,
                completed_iterations: settings.illumination.enabled ? illumination.iteration : 0,
                adjustment_policy: 'Unmodified Mol* initial half-step/min-refine ramp; targetFps=0 prevents FPS-driven quality downshift',
                final_adjusted: settings.illumination.enabled ? {
                    rendersPerFrame: illumination.tracing.traceRenderable.values.dRendersPerFrame.ref.value,
                    steps: illumination.tracing.traceRenderable.values.dSteps.ref.value,
                    refineSteps: illumination.tracing.traceRenderable.values.dRefineSteps.ref.value } : null },
            webgl: { version: gl.getParameter(gl.VERSION), vendor: debug ? gl.getParameter(debug.UNMASKED_VENDOR_WEBGL) : null,
                renderer: debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : null,
                context_attributes: gl.getContextAttributes(), extensions: gl.getSupportedExtensions() } };
        response.physical_settings_equivalent = false;
        response.telemetry_unavailable = ['heap', 'exclusive GPU timestamps', 'residency counters'];
        return response;
    } finally { plugin.dispose(); }
}
window.executeMolstarParity = execute;
