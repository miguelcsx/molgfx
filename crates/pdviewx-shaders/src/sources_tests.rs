use super::{
    CULL, GEOMETRY_BOND, GEOMETRY_CARTOON, GEOMETRY_SPHERE, GEOMETRY_SURFACE, SHADOW_RIBBON,
    SURFACE_FIELD_NORMAL,
};

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
    assert!(GEOMETRY_CARTOON.contains("visual.color.rgb + visual.emission"));
    assert!(GEOMETRY_CARTOON.contains("ribbon_visual_gbuffer_material"));
    assert!(super::GEOMETRY_POINT.contains("visual.color.rgb + visual.emission"));
    assert!(super::LIGHTING.contains("emission_enabled"));
    assert!(super::LIGHTING.contains("albedo_material.a >= 8.0"));
}

#[test]
fn composed_molecular_impostors_include_parallel_camera_rays() {
    assert!(GEOMETRY_SPHERE.contains("fn sphere_ray"));
    assert!(GEOMETRY_SPHERE.contains("fn representation_transform_point"));
    assert!(GEOMETRY_BOND.contains("fn bond_ray_xy"));
    assert!(GEOMETRY_BOND.contains("ray_origin + resolved.t * ray_direction"));
    assert!(GEOMETRY_SPHERE.contains("fn sphere_quad_half_extent"));
    assert!(GEOMETRY_SPHERE.contains("let shift = abs(center.xy)"));
}

#[test]
fn composed_culling_uses_projection_independent_clip_extents() {
    assert!(!CULL.contains("MIN_DISTANCE"));
    assert!(CULL.contains("frame.projection_kind.yz * radius"));
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
    assert!(SURFACE_FIELD_NORMAL.contains("texture_storage_3d<rgba8snorm, write>"));
}

#[test]
fn soft_union_is_explicit_and_keeps_the_exact_surface_path() {
    assert!(GEOMETRY_SURFACE.contains("representation.options.w == 5u"));
    assert!(GEOMETRY_SURFACE.contains("soft_union_parameter(nearest_parameters"));
    assert!(GEOMETRY_SURFACE.contains("let soft_union ="));
}

#[test]
fn animated_ribbons_pull_resident_coordinates_in_beauty_and_shadow_paths() {
    for source in [GEOMETRY_CARTOON, SHADOW_RIBBON] {
        assert!(source.contains("ribbon_base_coordinates"));
        assert!(source.contains("fn ribbon_catmull_position"));
        assert!(source.contains("fn ribbon_rotation_arc"));
        assert!(source.contains("ribbon_deform(vertex_id"));
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
