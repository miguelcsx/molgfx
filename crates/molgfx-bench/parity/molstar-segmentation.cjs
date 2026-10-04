// Reference segment surfaces use the same categorical thresholds and per-map styles.
const { Ccp4Provider } = require('@molstar/mol-plugin-state/formats/volume');
const { Volume } = require('@molstar/mol-model/volume/volume');
const { Box3D } = require('@molstar/mol-math/geometry/primitives/box3d');
const { Vec3 } = require('@molstar/mol-math/linear-algebra');
const { createVolumeRepresentationParams } = require('@molstar/mol-plugin-state/helpers/volume-representation-params');
const { StateTransforms } = require('@molstar/mol-plugin-state/transforms');

async function prepare(plugin, data, fixture, affine) {
    const thresholds = fixture.segmentation_thresholds.map(Math.fround);
    const maps = [];
    for (const spec of fixture.segmentations) {
        const object = (await Ccp4Provider.parse(plugin, data)).volume;
        const volume = object.data;
        const cells = volume.grid.cells;
        const matrix = Array.from(affine);
        for (let axis = 0; axis < 3; axis++) matrix[12 + axis] += spec.translation[axis];
        const bounds = {}, segments = new Map(), sets = new Map(), counts = [0, 0, 0, 0];
        const coordinates = [0, 0, 0];
        for (let index = 0; index < cells.data.length; index++) {
            const value = cells.data[index];
            const label = value >= thresholds[2] ? 1 : value >= thresholds[1] ? 2 : value >= thresholds[0] ? 3 : 0;
            cells.data[index] = label;
            counts[label]++;
            if (label === 0) continue;
            cells.space.getCoords(index, coordinates);
            if (!bounds[label]) {
                bounds[label] = Box3D.create(Vec3.create(Infinity, Infinity, Infinity), Vec3.create(-Infinity, -Infinity, -Infinity));
                segments.set(label, new Set([label]));
                sets.set(label, new Set([label]));
            }
            for (let axis = 0; axis < 3; axis++) {
                bounds[label].min[axis] = Math.min(bounds[label].min[axis], coordinates[axis]);
                bounds[label].max[axis] = Math.max(bounds[label].max[axis], coordinates[axis]);
            }
        }
        volume.grid = { cells, transform: { kind: 'matrix', matrix }, stats: { min: 0, max: 3, mean: 0, sigma: 1 } };
        Volume.Segmentation.set(volume, { segments, sets, bounds, labels: {} });
        maps.push({ object, spec, counts, matrix });
    }
    return maps;
}

async function representations(plugin, maps) {
    let last;
    for (const { object, spec } of maps) {
        for (const style of spec.styles) {
            if (!style.visible || style.opacity === 0) continue;
            const [r, g, b] = style.color_rgb;
            const params = createVolumeRepresentationParams(plugin, object.data, {
                type: 'segment', typeParams: { segments: [style.label], alpha: style.opacity, quality: 'highest' },
                color: 'uniform', colorParams: { value: (r << 16) | (g << 8) | b },
            });
            last = await plugin.build().to(object).apply(StateTransforms.Representation.VolumeRepresentation3D, params).commit();
        }
    }
    return last;
}

module.exports = { prepare, representations };
