const { ParamDefinition: PD } = require('@molstar/mol-util/param-definition');
const { DofParams } = require('@molstar/mol-canvas3d/passes/dof');
const { BloomParams } = require('@molstar/mol-canvas3d/passes/bloom');

// Reference effects establish visual semantics; their physical units differ.
function configure(canvas, effect) {
    const evidence = { requested: effect ?? null, applied: null, omissions: [] };
    switch (effect) {
        case undefined:
        case 'bloom_control':
            break;
        case 'bloom':
            canvas.postprocessing.bloom = { name: 'on', params: {
                ...PD.getDefaultValues(BloomParams), mode: 'luminosity',
                strength: 0.8, radius: 0.5, threshold: 0.85 } };
            evidence.applied = 'Mol* luminosity bloom';
            break;
        case 'dof':
            canvas.postprocessing.dof = { name: 'on', params: {
                ...PD.getDefaultValues(DofParams), center: 'camera-target',
                mode: 'plane', inFocus: 0 } };
            evidence.applied = 'Mol* camera-target focus plane';
            break;
        case 'depth_cue':
            canvas.cameraFog = { name: 'on', params: { intensity: 65 } };
            evidence.applied = 'Mol* camera fog; engine-specific depth range';
            break;
        case 'shape_cues':
            evidence.omissions.push('Mol* has no equivalent to the combined MolGFX shape-cue effect; base lighting retained');
            break;
        case 'motion_blur':
            evidence.omissions.push('Mol* ImagePass has no camera-shutter motion-blur effect; static reference retained');
            break;
        default:
            throw new Error(`unknown reference effect: ${effect}`);
    }
    return evidence;
}
module.exports = { configure };
