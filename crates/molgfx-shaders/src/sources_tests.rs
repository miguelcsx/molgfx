use super::{
    GEOMETRY_BOND, GEOMETRY_BOND_SPECIALIZED, GEOMETRY_CARTOON, GEOMETRY_CARTOON_SPECIALIZED,
    GEOMETRY_SPHERE, GEOMETRY_SPHERE_SPECIALIZED, GEOMETRY_SURFACE, SHADOW_RIBBON,
};

#[test]
fn specialized_units_replace_the_interpreter_instead_of_adding_to_it() {
    for (interpreted, specialized) in [
        (GEOMETRY_SPHERE, GEOMETRY_SPHERE_SPECIALIZED),
        (GEOMETRY_BOND, GEOMETRY_BOND_SPECIALIZED),
        (GEOMETRY_CARTOON, GEOMETRY_CARTOON_SPECIALIZED),
    ] {
        let interpreter = "// -- begin include: include/visual/interpreter.wgsl";

        // The interpreted unit keeps the loop; the specialized unit does not.
        assert_eq!(interpreted.matches("fn visual_resolve(").count(), 1);
        assert_eq!(specialized.matches("fn visual_resolve(").count(), 1);
        assert_eq!(interpreted.matches(interpreter).count(), 1);
        assert_eq!(specialized.matches(interpreter).count(), 0);
        assert!(interpreted.contains("counts.x"));
        assert!(!specialized.contains("counts.x"));

        // The marker was consumed, and the shared ladder survived in both.
        assert!(!specialized.contains("{{visual_program}}"));
        for source in [interpreted, specialized] {
            assert!(source.contains("fn visual_resolve_registers("));
            assert!(source.contains("var registers: array<vec4f, 64>"));
        }
    }
}

#[test]
fn visual_programs_split_cull_critical_and_compacted_shading_work() {
    assert!(super::VISUAL_PROGRAM.contains("var registers: array<vec4f, 64>"));
    assert!(super::VISUAL_PROGRAM.contains("@compute @workgroup_size(64)"));
    assert!(super::VISUAL_PROGRAM.contains("instruction.control.w < 2u"));
    assert!(super::VISUAL_PROGRAM.contains("fn evaluate_visual_cull"));
    assert!(super::VISUAL_PROGRAM.contains("fn evaluate_visual_shading"));
    assert!(super::VISUAL_PROGRAM.contains("visible_atoms[compact]"));
    assert!(super::VISUAL_PROGRAM.contains("var<storage, read_write> visual_results: array<u32>"));
    assert!(!super::VISUAL_PROGRAM.contains("struct VisualEntityResult"));
}

#[test]
fn typed_visual_attributes_load_each_physical_layout_without_expansion() {
    let source = super::VISUAL_PROGRAM;
    assert!(source.contains("visual_config.attribute_layouts[property]"));
    assert!(source.contains("inputs.entity * stride"));
    assert!(source.contains("f32(visual_properties[sampled_base])"));
    assert!(source.contains("bitcast<f32>(visual_properties[sampled_base + 2u])"));
    assert!(source.contains("unpack4x8unorm(visual_properties[base])"));
}

#[test]
fn visual_position_inputs_read_the_live_coordinate_column() {
    let source = super::VISUAL_PROGRAM;
    assert!(source.contains("visual_coordinates[coordinate_base]"));
    assert!(source.contains("visual_model_to_world[0].xyz * local_position.x"));
    assert!(!source.contains("vec4f(atom.position, 0.0)"));
}

#[test]
fn fragment_visuals_share_one_interpreter_and_keep_a_specialized_builtin_path() {
    for source in [GEOMETRY_SPHERE, GEOMETRY_BOND, GEOMETRY_SURFACE] {
        assert!(source.contains("fn visual_evaluate_instruction"));
        assert!(source.contains("var<uniform> visual_fragment_program"));
        assert!(!source.contains("@binding(18) var<storage, read> visual_parameters"));
        assert!(source.contains("override VISUAL_PROGRAM_ENABLED: bool = false"));
        assert!(source.contains("if !VISUAL_FRAGMENT_ENABLED"));
    }
    assert!(super::GEOMETRY_POINT.contains("fn visual_fragment"));
    assert!(GEOMETRY_BOND.contains("along >= 0.5"));
    for source in [GEOMETRY_CARTOON, SHADOW_RIBBON] {
        assert!(source.contains("fn visual_evaluate_instruction"));
    }
    assert!(GEOMETRY_CARTOON.contains("ribbon_visual_offset"));
    assert!(SHADOW_RIBBON.contains("ribbon_shadow_offset"));
    assert!(GEOMETRY_CARTOON.contains("ribbon_visual_gbuffer_material"));
    assert!(SHADOW_RIBBON.contains("ribbon_shadow_visible"));
}

#[test]
fn visual_emission_stays_hdr_through_deferred_and_transparent_paths() {
    assert!(GEOMETRY_CARTOON.contains("visual.color.rgb"));
    assert!(GEOMETRY_CARTOON.contains("visual.emission"));
    assert!(GEOMETRY_CARTOON.contains("ribbon_visual_gbuffer_material"));
    assert!(super::GEOMETRY_POINT.contains("visual.color.rgb + visual.emission"));
    assert!(super::LIGHTING.contains("emission_enabled"));
    assert!(super::LIGHTING.contains("unmarked_payload >= 8.0"));
}

#[test]
fn every_gbuffer_form_carries_its_marker_to_the_lighting_edge() {
    for source in [
        GEOMETRY_CARTOON,
        GEOMETRY_SPHERE,
        GEOMETRY_BOND,
        super::GEOMETRY_POINT,
    ] {
        assert!(source.contains("marker_encode_payload"));
    }
    assert!(super::LIGHTING.contains("apply_marker_edge"));
    assert!(super::LIGHTING.contains("marker_from_payload"));
}

#[test]
fn the_molecular_bond_pipeline_displaces_every_packed_multi_bond_variant() {
    // The molecular bond pipeline carries the packed chemical order; the
    // provider-backed paged-bond pipeline is authored per placement and has no
    // order lane, so only the molecular unit is asserted here.
    assert!(GEOMETRY_BOND.contains("bond_variant_geometry"));
    assert!(GEOMETRY_BOND.contains("bond_perpendicular"));
    assert!(GEOMETRY_BOND.contains("offset_radii"));
    // The order and variant lanes the packer writes are actually read.
    assert!(GEOMETRY_BOND.contains("bond.order"));
    assert!(GEOMETRY_BOND.contains("(bond.flags >> 1u) & 3u"));
    // The single-strand case must stay exactly one full-width capsule.
    assert!(GEOMETRY_BOND.contains("BOND_DOUBLE_OFFSET"));
    assert!(GEOMETRY_BOND.contains("BOND_TRIPLE_OFFSET"));
    assert!(GEOMETRY_BOND.contains("BOND_STYLE_AROMATIC_INNER"));
}

#[test]
fn every_analytic_depth_writer_ranks_a_coincident_tie_by_entity() {
    // A capsule, a wire and an atom sphere can all meet at one point at the
    // same analytic depth, and culling compacts visibility in parallel. Each
    // analytic writer must rank the tie by entity, or the winning fragment
    // varies between submissions on the same input. The point pipeline is
    // exempt because it writes the rasterized device depth, which the
    // rasterizer already fixes for a given vertex position.
    for source in [GEOMETRY_BOND, GEOMETRY_SPHERE] {
        assert!(
            source.contains("stable_entity_depth"),
            "an analytic depth writer must rank by entity"
        );
        assert!(source.contains("in.entity_id"));
    }
    // The provider-backed paths carry their identity as a pick page and local
    // row rather than an entity id, and must rank the tie too.
    for source in [super::PAGED_CHUNK, super::PAGED_BOND] {
        assert!(source.contains("stable_entity_depth"));
        assert!(source.contains("in.pick_page * 65536u + in.local_row"));
    }
}

#[test]
fn relations_compact_visibility_before_the_indirect_draw() {
    let cull = super::RELATION_CULL;
    assert!(cull.contains("@compute @workgroup_size(64)"));
    assert!(cull.contains("fn cull_relations"));
    assert!(cull.contains("visible_relations[base + local] = global"));
    assert!(cull.contains("fn style_relation"));
    assert!(cull.contains("atomicAdd(&relation_args.instance_count, count)"));
    assert!(super::GEOMETRY_INTERACTION.contains("visible_interactions[instance_index]"));
    for field in [
        "start_width",
        "end_period",
        "style",
        "animation",
        "color",
        "metadata",
    ] {
        assert!(
            super::GEOMETRY_INTERACTION.contains(&format!("interactions[relation_index].{field}"))
        );
    }
}

#[test]
fn paged_chunk_culling_tests_each_cluster_and_reserves_output_once() {
    let source = super::PAGED_CHUNK;
    assert!(source.contains("if lane == 0u"));
    assert!(source.contains("placement.dynamic_coordinates != 0u || cluster_visible"));
    assert!(source.contains("workgroupBarrier()"));
    assert_eq!(
        source
            .matches("atomicAdd(&paged_commands[command].instance_count")
            .count(),
        1
    );
}

#[test]
fn grid_surfaces_use_sign_bracketed_hits_and_continuous_normals() {
    assert!(GEOMETRY_SURFACE.contains("fn grid_refine_hit"));
    assert!(GEOMETRY_SURFACE.contains("if level <= 0.0"));
    assert!(!GEOMETRY_SURFACE.contains("SURFACE_MARCH_CELL_FRACTION"));
    assert!(GEOMETRY_SURFACE.contains("surface_nearest_atom(hit)"));
    // The normal is interpolated between vertex normals taken from the field
    // itself, so no second volume is stored beside it.
    assert!(GEOMETRY_SURFACE.contains("fn grid_vertex_normal"));
    assert!(!GEOMETRY_SURFACE.contains("surface_normals"));
}

#[test]
fn soft_union_is_explicit_and_keeps_the_exact_surface_path() {
    assert!(GEOMETRY_SURFACE.contains("representation.options.w == 5u"));
    assert!(GEOMETRY_SURFACE.contains("soft_union_parameter(nearest_parameters"));
    assert!(GEOMETRY_SURFACE.contains("let soft_union ="));
    // The blend span is the caller's blob spread, not a shader constant, and
    // the impostor bound is derived from the same value so the two cannot
    // disagree about how far a cusp may round outward.
    assert!(GEOMETRY_SURFACE.contains("representation.presentation.y"));
    // The ray-bounds padding and the trace must both derive from the span; a
    // hardcoded blend span or half-ångström pad would let them disagree. A
    // zero span must select the exact union rather than a tiny blend.
    assert!(!GEOMETRY_SURFACE.contains("select(0.0, 2.0"));
    assert!(GEOMETRY_SURFACE.contains("normal_blend_span > 0.0"));
    assert!(GEOMETRY_SURFACE.contains("max(representation.presentation.y, 0.0) * 0.25"));
}

#[test]
fn animated_ribbons_pull_resident_coordinates_in_beauty_and_shadow_paths() {
    for source in [GEOMETRY_CARTOON, SHADOW_RIBBON] {
        assert!(source.contains("ribbon_base_coordinates"));
        assert!(source.contains("fn ribbon_catmull_position"));
        assert!(source.contains("fn ribbon_rotation_arc"));
        assert!(source.matches("ribbon_deform(").count() >= 2);
    }
}

#[test]
fn quality_ao_traverses_packed_bond_data_without_linear_bond_scans() {
    let source = super::QUALITY_AO;
    assert!(source.contains("quality_bond_data"));
    assert!(source.contains("fn quality_bond_node"));
    assert!(source.contains("fn quality_bond_index"));
    assert!(source.contains("fn quality_bond"));
    assert!(source.contains("fn quality_node_interval"));
    assert!(source.contains("let bond_index = quality_bond_index(first + offset)"));
    assert!(!source.contains("arrayLength(&quality_bond"));
    assert!(!source.contains("let atom_node_count = arrayLength"));
    assert!(!source.contains("let atom_escape_base = quality_escape_base(arrayLength"));
    assert!(!source.contains("bond_index < visual_counts.bonds"));
    assert!(!source.contains("for (var bond_index = 0u"));
}

#[test]
fn quality_ao_remains_progressive_and_uses_the_edge_aware_denoiser() {
    assert!(super::QUALITY_AO.contains("u32(frame.temporal.w)"));
    assert!(super::TEMPORAL_RESOLVE.contains("history_hdr_depth"));
    assert!(super::AO_DENOISE.contains("depth_texture"));
    assert!(super::AO_DENOISE.contains("normal_texture"));
}

#[test]
fn hardware_quality_uses_ray_queries_and_shared_analytic_intersections() {
    let hardware = super::QUALITY_AO_RAY_QUERY;
    let compute = super::QUALITY_AO;
    for symbol in ["quality_sphere_distance", "quality_capsule_distance"] {
        assert!(hardware.contains(symbol));
        assert!(compute.contains(symbol));
    }
    assert!(hardware.contains("enable wgpu_ray_query"));
    assert!(hardware.contains("rayQueryGenerateIntersection"));
    assert!(hardware.contains("quality_acceleration"));
}

#[test]
fn paged_bonds_are_self_contained_and_reference_shared_coordinates() {
    let source = super::PAGED_BOND;
    assert!(source.contains("bond_coordinates"));
    assert!(source.contains("coordinate_a"));
    assert!(source.contains("cull_paged_bonds"));
    assert!(source.contains("paged_bond_command"));
    assert!(source.contains("bond_prefix"));
    assert_eq!(
        source
            .matches("atomicAdd(&paged_bond_command.instance_count")
            .count(),
        1
    );
    assert!(source.contains("ray_capsule_interval"));
    assert!(!source.contains("VisualFragmentResult"));
}

#[test]
fn paged_trajectory_interpolates_provider_frames_into_shared_coordinates() {
    let source = super::PAGED_CHUNK;
    assert!(source.contains("interpolate_paged_trajectory"));
    assert!(source.contains("trajectory_frames"));
    assert!(source.contains("paged_coordinates[output] = mix"));
}

/// Every composed unit, exactly as the build wrote it.
fn composed_units() -> Vec<(String, String)> {
    let directory = match std::fs::read_dir(env!("OUT_DIR")) {
        Ok(directory) => directory,
        Err(error) => panic!("the composed shader directory should be readable: {error}"),
    };
    let mut units = Vec::new();
    for entry in directory.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("wgsl") {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(source) => units.push((path.display().to_string(), source)),
            Err(error) => panic!("{} should be readable: {error}", path.display()),
        }
    }
    assert!(units.len() > 20, "expected the whole composed library");
    units
}

#[test]
fn no_composed_unit_uses_a_construct_safari_rejects() {
    for (name, source) in composed_units() {
        // Safari does not accept `@diagnostic` on a function.
        assert!(
            !source.contains("@diagnostic("),
            "{name} uses a @diagnostic attribute"
        );
        // Nor `workgroupUniformLoad` of an atomic.
        for (offset, _) in source.match_indices("workgroupUniformLoad(&") {
            let rest = &source[offset + "workgroupUniformLoad(&".len()..];
            let variable = rest.split(')').next().unwrap_or_default();
            let declaration = format!("var<workgroup> {variable}: ");
            let Some(at) = source.find(&declaration) else {
                panic!("{name}: {variable} is not a workgroup variable");
            };
            let kind = source[at + declaration.len()..]
                .split(';')
                .next()
                .unwrap_or_default();
            assert!(
                !kind.contains("atomic"),
                "{name} loads atomic {variable} uniformly"
            );
        }
    }
}

#[test]
fn diagnostic_directives_precede_every_declaration() {
    for (name, source) in composed_units() {
        let mut declarations_started = false;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with("diagnostic(") {
                assert!(
                    !declarations_started,
                    "{name}: a directive follows a declaration"
                );
            } else {
                declarations_started = true;
            }
        }
    }
}

#[test]
fn the_unit_list_is_exactly_the_composed_library() {
    let composed = composed_units();
    assert_eq!(super::UNITS.len(), composed.len());
    for (name, source) in composed {
        assert!(
            super::UNITS.iter().any(|(_, unit)| *unit == source),
            "{name} is composed but missing from UNITS"
        );
    }
}
