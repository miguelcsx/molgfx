//! Colour is presentation state, so changing a scheme never touches a record.

use super::tests::{camera, engine, represented_scene};
use crate::engine::Engine;
use crate::testing::MockDevice;
use molgfx_core::ColorScheme;

use molgfx_math::Rgba8;

/// Buffer writes observed since `before`, with their creation labels.
fn writes_since(engine: &Engine<MockDevice>, before: usize) -> Vec<&'static str> {
    let Ok(writes) = engine.device.log.writes.lock() else {
        panic!("write log lock")
    };
    let Ok(buffers) = engine.device.log.buffers.lock() else {
        panic!("buffer log lock")
    };
    writes[before..]
        .iter()
        .map(|(buffer, _, _, _)| {
            buffers
                .iter()
                .find(|(id, _, _)| id == buffer)
                .map_or("unknown", |(_, label, _)| *label)
        })
        .collect()
}

/// Renders one frame, returning the writes it performed.
fn frame_writes(engine: &mut Engine<MockDevice>, scene: &molgfx_core::Scene) -> Vec<&'static str> {
    let Ok(writes) = engine.device.log.writes.lock() else {
        panic!("write log lock")
    };
    let before = writes.len();
    drop(writes);
    if let Err(error) = engine.render(scene, &camera()) {
        panic!("frame renders: {error}")
    }
    writes_since(engine, before)
}

#[test]
fn a_colour_scheme_change_writes_only_fixed_size_uniforms() {
    // The scheme is resolved on the GPU from the record's element colour and
    // three packed palette indices, so changing it rewrites the presentation
    // uniforms and nothing else: no repack, no record upload, no new buffer.
    let mut scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("initial frame renders: {error}")
    }
    let Ok(buffers_before) = engine.device.log.buffers.lock().map(|value| value.len()) else {
        panic!("buffer log lock")
    };

    for scheme in [
        ColorScheme::ByChain,
        ColorScheme::ByResidue,
        ColorScheme::BySecondaryStructure,
        ColorScheme::Uniform(Rgba8::opaque(12, 34, 56)),
    ] {
        let Some(value) = scene.representation_mut(representation) else {
            panic!("representation resolves")
        };
        value.color = scheme;
        let changed = frame_writes(&mut engine, &scene);
        assert!(
            changed.iter().all(|label| *label == "frame uniforms"
                || *label == "representation uniforms"
                || *label == "colour scheme uniforms"),
            "{scheme:?} wrote unexpected buffers: {changed:?}"
        );
    }

    let Ok(buffers_after) = engine.device.log.buffers.lock().map(|value| value.len()) else {
        panic!("buffer log lock")
    };
    assert_eq!(
        buffers_after, buffers_before,
        "a scheme change must not allocate"
    );
}

#[test]
fn representations_differing_only_in_colour_share_one_record_set() {
    // Before colour moved to the GPU this was impossible: the scheme was baked
    // into every atom record, so two differently-coloured representations could
    // not share one packed set.
    let mut counts = Vec::new();
    for (index, scheme) in [
        ColorScheme::ByElement,
        ColorScheme::Uniform(Rgba8::opaque(200, 30, 40)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut scene = represented_scene(1, 1);
        let Some((representation, _)) = scene.representations().next() else {
            panic!("fixture has a representation")
        };
        let Some(value) = scene.representation_mut(representation) else {
            panic!("representation resolves")
        };
        value.color = scheme;
        value.order = u16::try_from(index).unwrap_or(0);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(buffers) = engine.device.log.buffers.lock() else {
            panic!("buffer log lock")
        };
        counts.push(
            buffers
                .iter()
                .filter(|(_, label, _)| *label == "atom instances")
                .count(),
        );
    }
    assert!(
        counts.iter().all(|count| *count == 1),
        "each scene allocates exactly one atom record set: {counts:?}"
    );
}

#[test]
fn the_gpu_scheme_palette_matches_the_cpu_palette() {
    // The shader resolves chain and residue colours from the same tables the
    // CPU path uses. This pins the arrangement the shader indexes: getting a
    // base wrong silently draws chains in secondary-structure colours.
    let scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let Some(value) = scene.representation(representation) else {
        panic!("representation resolves")
    };
    let uniforms = crate::scene_gpu::color_uniforms::ColorUniforms::new(value, [0, 1], [0, 1]);
    let palette = uniforms.palette_probe();
    for (index, color) in molgfx_core::CATEGORICAL_COLORS.into_iter().enumerate() {
        assert_eq!(
            palette[9 + index].map(f32::to_bits),
            lanes(color).map(f32::to_bits),
            "categorical slot {index}"
        );
    }
    for (class, color) in molgfx_core::SECONDARY_STRUCTURE_COLORS
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            palette[4 + class].map(f32::to_bits),
            lanes(color).map(f32::to_bits),
            "secondary slot {class}"
        );
    }
}

/// One packed colour as four normalized lanes.
fn lanes(color: Rgba8) -> [f32; 4] {
    [
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    ]
}

#[test]
fn the_colour_block_is_written_with_the_active_scheme() {
    // The shader reads the scheme from the colour block, so a block that never
    // receives the selector leaves every scheme drawing the element colour.
    let mut scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let Some(value) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    value.color = ColorScheme::Uniform(Rgba8::opaque(12, 34, 56));
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(buffers) = engine.device.log.buffers.lock() else {
        panic!("buffer log lock")
    };
    let color_buffer = buffers
        .iter()
        .find(|(_, label, _)| *label == "colour scheme uniforms")
        .map(|(id, _, _)| *id);
    let Some(color_buffer) = color_buffer else {
        panic!("the colour block is allocated")
    };
    drop(buffers);
    let Ok(writes) = engine.device.log.write_payloads.lock() else {
        panic!("payload log lock")
    };
    let Ok(write_log) = engine.device.log.writes.lock() else {
        panic!("write log lock")
    };
    let Some(index) = write_log
        .iter()
        .position(|(buffer, _, _, _)| *buffer == color_buffer)
    else {
        panic!("the colour block is written at least once")
    };
    let Some(payload) = writes.get(index) else {
        panic!("the colour block write captured its bytes")
    };
    // palette is 17 lanes, then the selector: scheme, packed colour, column.
    let selector_at = 17 * 16;
    let scheme = u32::from_le_bytes([
        payload[selector_at],
        payload[selector_at + 1],
        payload[selector_at + 2],
        payload[selector_at + 3],
    ]);
    let packed = u32::from_le_bytes([
        payload[selector_at + 4],
        payload[selector_at + 5],
        payload[selector_at + 6],
        payload[selector_at + 7],
    ]);
    assert_eq!(scheme, 5, "a uniform scheme selects tag 5");
    assert_eq!(
        packed,
        u32::from_le_bytes([12, 34, 56, 255]),
        "the colour travels packed"
    );
}

#[test]
fn an_overlay_packs_its_column_and_scheme_table_for_the_shader() {
    let mut scene = represented_scene(1, 1);
    let Some((structure, placed)) = scene.structures().next() else {
        panic!("fixture has a structure")
    };
    let atoms = placed.atoms.len() as usize;
    let classes = match molgfx_core::AtomProperty::new(
        structure,
        std::sync::Arc::<str>::from("classes"),
        vec![0.0; atoms].into(),
        molgfx_core::AtomPropertyMeaning::Generic,
        molgfx_core::ScalarFieldSemantics::UncalibratedRank,
    ) {
        Ok(property) => property,
        Err(error) => panic!("class column builds: {error}"),
    };
    let Ok(handle) = scene.add_atom_property(classes) else {
        panic!("class column binds")
    };
    let red = molgfx_math::Rgba8::opaque(255, 0, 0);
    let Ok(overlay) =
        molgfx_core::ColorOverlay::new(handle, &[ColorScheme::ByChain, ColorScheme::Uniform(red)])
    else {
        panic!("overlay builds")
    };
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let Some(value) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    value.color_overlay = Some(overlay);
    let packed = crate::scene_gpu::color_uniforms::ColorUniforms::new(value, [0, 1], [40, 1]);
    let (header, table) = packed.overlay_probe();
    assert_eq!(header, [40, 1, 2, 0]);
    // Class one is the chain scheme, class two the packed uniform red.
    assert_eq!(table[0][0], 1);
    assert_eq!(table[0][2], 5);
    assert_eq!(table[0][3], u32::from_le_bytes([255, 0, 0, 255]));
    // An unplanned column disables the overlay instead of reading offset zero.
    let disabled = crate::scene_gpu::color_uniforms::ColorUniforms::new(value, [0, 1], [0, 1]);
    assert_eq!(disabled.overlay_probe().0[0], 0);
}
