//! Every unit must translate to the GLSL the OpenGL backend emits.
//!
//! The OpenGL backend is the one a software-rendered headless host falls back
//! to. It translates each WGSL entry point through naga when a pipeline is
//! created, so a construct its GLSL writer rejects only surfaces on such a
//! host, at the first draw. Translating here makes that a test failure.

use super::{
    AMBIENT_OCCLUSION, AO_DENOISE, ATTRIBUTE_TIMELINE, BLOOM, CULL, DEPTH_OF_FIELD,
    GENERIC_INSTANCE, GENERIC_INSTANCE_CULL, GENERIC_INSTANCE_SPECIALIZED, GENERIC_POINT,
    GENERIC_POINT_CULL, GENERIC_POINT_SPECIALIZED, GEOMETRY_BOND, GEOMETRY_BOND_SPECIALIZED,
    GEOMETRY_CARTOON, GEOMETRY_CARTOON_SPECIALIZED, GEOMETRY_INTERACTION, GEOMETRY_LABEL,
    GEOMETRY_LIGAND_POSE, GEOMETRY_POINT, GEOMETRY_POINT_SPECIALIZED, GEOMETRY_PRIMITIVE,
    GEOMETRY_SPHERE, GEOMETRY_SPHERE_SPECIALIZED, GEOMETRY_SURFACE, GEOMETRY_SURFACE_SPECIALIZED,
    INSTANCE_TIMELINE, LABEL_DECLUTTER, LIGAND_POSE_SHADOW, LIGHTING, MOTION_BLUR, OCCUPANCY,
    OCCUPANCY_RGBA, OIT_COMPOSITE, OVERLAY, PAGED_BOND, PAGED_CHUNK, PARTICLE_ADVECTION,
    POINT_TIMELINE, QUALITY_AO, QUALITY_AO_SPECIALIZED, RELATION_CULL, RELATION_RESOLVE,
    SEGMENTATION, SHADOW, SHADOW_RIBBON, SHADOW_RIBBON_SPECIALIZED, SHADOW_SPECIALIZED,
    SURFACE_COMPONENT_FILTER, SURFACE_FIELD_COMPUTE, SURFACE_FIELD_ERODE, SURFACE_FIELD_NORMAL,
    TEMPORAL_RESOLVE, TONEMAP, TRAJECTORY, VISUAL_PROGRAM, VOLUME,
};
use naga::back::glsl;

/// Every unit the renderer can create a pipeline from on OpenGL. The two
/// ray-query units are absent: hardware ray queries do not exist there, and
/// the renderer never selects them on that backend.
const UNITS: &[(&str, &str)] = &[
    ("ambient_occlusion", AMBIENT_OCCLUSION),
    ("ao_denoise", AO_DENOISE),
    ("attribute_timeline", ATTRIBUTE_TIMELINE),
    ("bloom", BLOOM),
    ("cull", CULL),
    ("depth_of_field", DEPTH_OF_FIELD),
    ("generic_instance", GENERIC_INSTANCE),
    ("generic_instance_cull", GENERIC_INSTANCE_CULL),
    ("generic_instance_specialized", GENERIC_INSTANCE_SPECIALIZED),
    ("generic_point", GENERIC_POINT),
    ("generic_point_cull", GENERIC_POINT_CULL),
    ("generic_point_specialized", GENERIC_POINT_SPECIALIZED),
    ("geometry_bond", GEOMETRY_BOND),
    ("geometry_bond_specialized", GEOMETRY_BOND_SPECIALIZED),
    ("geometry_cartoon", GEOMETRY_CARTOON),
    ("geometry_cartoon_specialized", GEOMETRY_CARTOON_SPECIALIZED),
    ("geometry_interaction", GEOMETRY_INTERACTION),
    ("geometry_label", GEOMETRY_LABEL),
    ("geometry_ligand_pose", GEOMETRY_LIGAND_POSE),
    ("geometry_point", GEOMETRY_POINT),
    ("geometry_point_specialized", GEOMETRY_POINT_SPECIALIZED),
    ("geometry_primitive", GEOMETRY_PRIMITIVE),
    ("geometry_sphere", GEOMETRY_SPHERE),
    ("geometry_sphere_specialized", GEOMETRY_SPHERE_SPECIALIZED),
    ("geometry_surface", GEOMETRY_SURFACE),
    ("geometry_surface_specialized", GEOMETRY_SURFACE_SPECIALIZED),
    ("instance_timeline", INSTANCE_TIMELINE),
    ("label_declutter", LABEL_DECLUTTER),
    ("ligand_pose_shadow", LIGAND_POSE_SHADOW),
    ("lighting", LIGHTING),
    ("motion_blur", MOTION_BLUR),
    ("occupancy", OCCUPANCY),
    ("occupancy_rgba", OCCUPANCY_RGBA),
    ("oit_composite", OIT_COMPOSITE),
    ("overlay", OVERLAY),
    ("paged_bond", PAGED_BOND),
    ("paged_chunk", PAGED_CHUNK),
    ("particle_advection", PARTICLE_ADVECTION),
    ("point_timeline", POINT_TIMELINE),
    ("quality_ao", QUALITY_AO),
    ("quality_ao_specialized", QUALITY_AO_SPECIALIZED),
    ("relation_cull", RELATION_CULL),
    ("relation_resolve", RELATION_RESOLVE),
    ("segmentation", SEGMENTATION),
    ("shadow", SHADOW),
    ("shadow_ribbon", SHADOW_RIBBON),
    ("shadow_ribbon_specialized", SHADOW_RIBBON_SPECIALIZED),
    ("shadow_specialized", SHADOW_SPECIALIZED),
    ("surface_component_filter", SURFACE_COMPONENT_FILTER),
    ("surface_field_compute", SURFACE_FIELD_COMPUTE),
    ("surface_field_erode", SURFACE_FIELD_ERODE),
    ("surface_field_normal", SURFACE_FIELD_NORMAL),
    ("temporal_resolve", TEMPORAL_RESOLVE),
    ("tonemap", TONEMAP),
    ("trajectory", TRAJECTORY),
    ("visual_program", VISUAL_PROGRAM),
    ("volume", VOLUME),
];

fn translation_failures(name: &str, source: &str) -> Vec<String> {
    let module = match naga::front::wgsl::parse_str(source) {
        Ok(module) => module,
        Err(error) => return vec![format!("{name}: parse: {error}")],
    };
    let info = match naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    {
        Ok(info) => info,
        Err(error) => return vec![format!("{name}: validate: {error:?}")],
    };
    // A desktop driver — including Mesa's software rasterizer on a headless
    // host — gives wgpu a desktop context, and wgpu caps its GLSL at 4.50.
    let options = glsl::Options {
        version: glsl::Version::Desktop(450),
        ..glsl::Options::default()
    };
    let mut failures = Vec::new();
    for entry in &module.entry_points {
        let pipeline = glsl::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        // wgpu resolves pipeline-overridable constants to their defaults
        // before handing a module to a backend writer; so does this test.
        let resolved = naga::back::pipeline_constants::process_overrides(
            &module,
            &info,
            Some((entry.stage, entry.name.as_str())),
            &pipeline_constants(source),
        );
        let (module, info) = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                failures.push(format!("{name}::{}: overrides: {error}", entry.name));
                continue;
            }
        };
        let mut out = String::new();
        let result = glsl::Writer::new(
            &mut out,
            &module,
            &info,
            &options,
            &pipeline,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .and_then(|mut writer| writer.write().map(|_| ()));
        if let Err(error) = result {
            failures.push(format!("{name}::{}: {error}", entry.name));
        }
    }
    failures
}

/// Constants the renderer always supplies because they have no default.
///
/// Each is supplied only to a unit that declares it, as the renderer does:
/// naga rejects a value for a constant the unit does not have.
fn pipeline_constants(source: &str) -> naga::back::PipelineConstants {
    let mut constants = naga::back::PipelineConstants::default();
    for name in ["PRESENTATION_GAMUT_TAG", "PRESENTATION_TRANSFER_TAG"] {
        if source.contains(&format!("override {name}")) {
            let _ = constants.insert(name.to_owned(), 0.0);
        }
    }
    constants
}

#[test]
fn every_entry_point_translates_to_opengl_glsl() {
    let failures: Vec<String> = UNITS
        .iter()
        .flat_map(|(name, source)| translation_failures(name, source))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}
