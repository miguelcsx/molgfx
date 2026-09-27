//! Every unit must translate to the GLSL the OpenGL backend emits.
//!
//! The OpenGL backend is the one a software-rendered headless host falls back
//! to. It translates each WGSL entry point through naga when a pipeline is
//! created, so a construct its GLSL writer rejects only surfaces on such a
//! host, at the first draw. Translating here makes that a test failure.

use super::UNITS;
use naga::back::glsl;

/// Every unit the renderer can create a pipeline from on OpenGL. The two
/// ray-query units are absent: hardware ray queries do not exist there, and
/// the renderer never selects them on that backend.
fn opengl_units() -> impl Iterator<Item = &'static (&'static str, &'static str)> {
    UNITS.iter().filter(|(name, _)| !name.contains("ray_query"))
}

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
    let failures: Vec<String> = opengl_units()
        .flat_map(|(name, source)| translation_failures(name, source))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}
