// Bundled against the requested local Mol* source checkout; runs in a real DOM.
const effects = require("./molstar-effects.cjs");
const { PluginContext } = require('@molstar/mol-plugin/context');
const { DefaultPluginSpec } = require('@molstar/mol-plugin/spec');
const { ParamDefinition: PD } = require('@molstar/mol-util/param-definition');
const { Canvas3DParams } = require('@molstar/mol-canvas3d/canvas3d');
const { RuntimeContext } = require('@molstar/mol-task');
const { SecondaryStructureProvider } = require('@molstar/mol-model-props/computed/secondary-structure');
const { SecondaryStructureType, BondType } = require('@molstar/mol-model/structure/model/types');
const { Ccp4Provider } = require('@molstar/mol-plugin-state/formats/volume');
const { createVolumeRepresentationParams } = require('@molstar/mol-plugin-state/helpers/volume-representation-params');
const { StateTransforms } = require('@molstar/mol-plugin-state/transforms');
const { PixelData } = require('@molstar/mol-util/image');
const { Script } = require('@molstar/mol-script/script');
const { MolScriptBuilder } = require('@molstar/mol-script/language/builder');
const { StructureSelection, StructureElement } = require('@molstar/mol-model/structure');
// `entry.atoms` selects one atom each by auth ids; the loci come from the state structure.
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
        let object, metadata, representation;
        if (fixture.format === 'mrc') {
            object = (await Ccp4Provider.parse(plugin, data)).volume;
            const grid = object.data.grid;
            metadata = { dimensions: grid.cells.space.dimensions, voxels: grid.cells.data.length,
                value_range: [grid.stats.min, grid.stats.max], affine: grid.transform,
                affine_provenance: 'Mol* CCP4 parser from hashed file' };
        } else {
            const trajectory = await plugin.builders.structure.parseTrajectory(data, 'mmcif');
            const model = await plugin.builders.structure.createModel(trajectory, { modelIndex: 0 });
            object = await plugin.builders.structure.createStructure(model, { name: 'model', params: {} });
            const structure = object.data;
            await SecondaryStructureProvider.attach({ runtime: RuntimeContext.Synchronous, assetManager: plugin.managers.asset }, structure);
            const assignments = SecondaryStructureProvider.get(structure).value;
            const ss = {}, orders = {}, provenance = {};
            let aromatic = 0;
            for (const unit of structure.units) {
                if (unit.kind !== 0) throw new Error('Coarse unit in atom-only corpus');
                const residues = new Set();
                for (const element of unit.elements) residues.add(unit.residueIndex[element]);
                const secondary = assignments?.get(unit.invariantId);
                for (const residue of residues) {
                    const flag = secondary ? secondary.type[secondary.getIndex(residue)] : 0;
                    const f = SecondaryStructureType.Flag;
                    const code = flag & f.Helix ? 'H' : flag & f.Beta ? 'E' : flag & f.Turn ? 'T' : flag ? 'C' : 'U';
                    ss[code] = (ss[code] || 0) + 1;
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
                secondary_structure: ss, secondary_structure_policy: 'Mol* auto: model/file or DSSP/Zhang-Skolnick' };
        }
        metadata.source_sha256 = fixture.sha256;
        const response = { engine: 'molstar', metadata };
        if (request.inspect) return response;
        const style = catalog.style;
        const color = (style.color_rgb[0] << 16) | (style.color_rgb[1] << 8) | style.color_rgb[2];
        if (fixture.script) {
            representation = await applyScript(plugin, object, fixture.script.molstar);
        } else if (fixture.format === 'mrc') {
            representation = await plugin.build().to(object).apply(StateTransforms.Representation.VolumeRepresentation3D,
                createVolumeRepresentationParams(plugin, object.data, { type: 'isosurface', typeParams: { isoValue: { kind: 'absolute', absoluteValue: fixture.isovalue }, alpha: style.opacity, quality: 'highest' }, color: 'uniform', colorParams: { value: color } })).commit();
        } else {
            const type = fixture.form === 'ball_and_stick' ? 'ball-and-stick' : fixture.form;
            representation = await plugin.builders.structure.representation.addRepresentation(object, {
                type, typeParams: { sizeFactor: style.atom_radius_scale, alpha: style.opacity, quality: 'highest', ignoreHydrogens: false },
                color: 'uniform', colorParams: { value: color }, size: 'physical' });
        }
        if (!representation?.data) throw new Error('Mol* representation failed');
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
