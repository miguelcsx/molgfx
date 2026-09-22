use super::pipeline_cache::{
    MAX_SPECIALIZED_PIPELINES, Specialization, SpecializationKey, SpecializedPipelines,
    VisualFamily,
};
use crate::error::RenderError;
use crate::testing::MockDevice;
use molgfx_core::{VisualPipeline, VisualProgram, VisualProgramBuilder, VisualStage};

const FAMILY: VisualFamily = VisualFamily::new("test-family");

fn key(fingerprint: u64) -> SpecializationKey {
    SpecializationKey::new(fingerprint, VisualStage::Fragment, FAMILY)
}

/// Builds a program, failing loudly when the builder refuses it.
fn build(builder: VisualProgramBuilder) -> VisualProgram {
    builder
        .finish()
        .unwrap_or_else(|error| panic!("program builds: {error}"))
}

/// Two nodes over constant and input values: a program the emitter lowers.
fn simple_style() -> VisualProgram {
    let mut builder = VisualProgramBuilder::new();
    let opacity = builder
        .base_opacity()
        .unwrap_or_else(|error| panic!("opacity builds: {error}"));
    let half = builder
        .scalar(0.5)
        .unwrap_or_else(|error| panic!("constant builds: {error}"));
    let scaled = builder
        .multiply(opacity, half)
        .unwrap_or_else(|error| panic!("multiply builds: {error}"));
    let color = builder
        .color([0.25, 0.5, 0.75, 1.0])
        .unwrap_or_else(|error| panic!("color builds: {error}"));
    builder
        .set_opacity(scaled)
        .unwrap_or_else(|error| panic!("opacity sets: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color sets: {error}"));
    build(builder)
}

/// A program that reads an interaction channel, which the emitter rejects.
fn rejected_style() -> VisualProgram {
    let mut builder = VisualProgramBuilder::new();
    let state = builder
        .interaction_state(1)
        .unwrap_or_else(|error| panic!("state builds: {error}"));
    let color = builder
        .color([1.0, 1.0, 1.0, 1.0])
        .unwrap_or_else(|error| panic!("color builds: {error}"));
    builder
        .set_visibility(state)
        .unwrap_or_else(|error| panic!("visibility sets: {error}"));
    builder
        .set_base_color(color)
        .unwrap_or_else(|error| panic!("color sets: {error}"));
    build(builder)
}

#[test]
fn a_specializable_style_compiles_once_and_is_ready_afterwards() {
    let program = simple_style();
    let mut cache = SpecializedPipelines::<MockDevice>::new();
    let mut builds = 0;

    let first = cache.acquire(key(program.fingerprint()), &program, || {
        builds += 1;
        Ok(7_u32)
    });
    assert_eq!(first.state(), VisualPipeline::Specialized);
    assert_eq!(first.pipeline(), Some(&7));
    assert!(
        first
            .report(key(program.fingerprint()))
            .contains("pipeline: specialized")
    );

    // The pipeline became resident, and the second frame reuses it.
    let second = cache.acquire(key(program.fingerprint()), &program, || {
        builds += 1;
        Err(RenderError::InvalidImageSize)
    });
    assert_eq!(second.pipeline(), Some(&7));
    assert_eq!(builds, 1);
    assert_eq!(cache.len(), 1);
    assert!(cache.is_ready(key(program.fingerprint())));
}

#[test]
fn the_two_phase_frame_settles_every_compile_before_it_reads_any_pipeline() {
    let first = simple_style();
    let second = simple_style();
    let mut cache = SpecializedPipelines::<MockDevice>::new();
    let mut builds = 0;

    // Phase one: every style the frame will draw is settled, so no compile can
    // happen once draws are being recorded.
    for (offset, program) in [&first, &second].into_iter().enumerate() {
        cache.settle(key(offset as u64 + 1), program, || {
            builds += 1;
            Ok(builds)
        });
    }
    assert_eq!(builds, 2);

    // Phase two: read-only, so both references are held at once — which is what
    // recording a frame needs.
    let one = cache.resolve(key(1));
    let two = cache.resolve(key(2));
    assert_eq!(one.pipeline(), Some(&1));
    assert_eq!(two.pipeline(), Some(&2));
    assert_eq!(
        one.report(key(1)).lines().next(),
        Some("pipeline: specialized")
    );
    assert_eq!(builds, 2, "resolving must never compile");
}

#[test]
fn a_rejected_style_keeps_the_interpreter_and_is_never_retried() {
    let program = rejected_style();
    let mut cache = SpecializedPipelines::<MockDevice>::new();
    let mut builds = 0;

    let first = cache.acquire(key(program.fingerprint()), &program, || {
        builds += 1;
        Ok(1_u32)
    });
    // The emitter's rejection happens before any backend work.
    assert_eq!(builds, 0);
    // An ineligible program is reported as the interpreter it will keep using,
    // not as a failure: nothing failed, the program was simply never eligible.
    assert_eq!(first.state(), VisualPipeline::Interpreter);
    assert_eq!(first.pipeline(), None);
    let report = first.report(key(program.fingerprint()));
    assert!(report.contains("pipeline: typed-bytecode-interpreter"));
    assert!(
        report.contains("unsupported-opcode:state@"),
        "report was {report}"
    );
    assert!(report.contains("cache-key: "));
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.failures(), 1);

    // A second frame reports the same reason and still does not compile.
    let second = cache.acquire(key(program.fingerprint()), &program, || {
        builds += 1;
        Ok(2_u32)
    });
    assert_eq!(builds, 0);
    assert_eq!(second.state(), VisualPipeline::Interpreter);
    assert_eq!(
        second.report(key(program.fingerprint())),
        report,
        "a rejected key must report one stable reason"
    );
}

#[test]
fn a_failing_compile_keeps_the_interpreter_permanently() {
    let program = simple_style();
    let mut cache = SpecializedPipelines::<MockDevice>::new();
    let mut builds = 0;

    let failed = cache.acquire(key(program.fingerprint()), &program, || {
        builds += 1;
        Err(RenderError::InvalidImageSize)
    });
    assert_eq!(failed.state(), VisualPipeline::Failed);
    let report = failed.report(key(program.fingerprint()));
    assert!(report.contains("pipeline: failed"));

    // The diagnostic is the backend's, and it is not retried next frame.
    let again = cache.acquire(key(program.fingerprint()), &program, || {
        builds += 1;
        Ok(3_u32)
    });
    assert_eq!(builds, 1);
    assert_eq!(again.pipeline(), None);
    let reported = report
        .lines()
        .find_map(|line| line.strip_prefix("reason: "))
        .unwrap_or_default();
    assert_eq!(
        cache.failure_reason(key(program.fingerprint())),
        Some(reported)
    );
}

#[test]
fn the_cache_evicts_the_least_recently_used_pipeline_at_its_cap() {
    let mut cache = SpecializedPipelines::<MockDevice>::with_capacity(2);
    let program = simple_style();
    let mut builds = 0;
    let _ = cache.acquire(key(1), &program, || {
        builds += 1;
        Ok(1_u32)
    });
    let _ = cache.acquire(key(2), &program, || {
        builds += 1;
        Ok(2_u32)
    });
    assert_eq!(cache.len(), 2);
    // Touch 1 so 2 becomes the least recently used. It is already resident, so
    // this must not compile anything.
    let _ = cache.acquire(key(1), &program, || {
        builds += 1;
        Ok(9_u32)
    });
    assert_eq!(builds, 2);

    let _ = cache.acquire(key(3), &program, || {
        builds += 1;
        Ok(3_u32)
    });
    assert_eq!(cache.len(), 2, "the cap is never exceeded");
    assert!(cache.is_ready(key(1)));
    assert!(cache.is_ready(key(3)));
    assert!(
        !cache.is_ready(key(2)),
        "the least recently used entry goes"
    );
    assert_eq!(builds, 3);
}

#[test]
fn the_default_bound_is_the_published_cap() {
    assert_eq!(MAX_SPECIALIZED_PIPELINES, 64);
    let cache = SpecializedPipelines::<MockDevice>::new();
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.failures(), 0);
}

#[test]
fn the_key_separates_stages_and_families_of_one_program() {
    let program = simple_style();
    let mut cache = SpecializedPipelines::<MockDevice>::new();
    let mut builds = 0_u32;
    let other_family = VisualFamily::new("other-family");
    for stage in [VisualStage::Entity, VisualStage::Fragment] {
        for family in [FAMILY, other_family] {
            let key = SpecializationKey::new(program.fingerprint(), stage, family);
            let pipeline = cache.acquire(key, &program, || {
                builds += 1;
                Ok(builds)
            });
            assert_eq!(pipeline.state(), VisualPipeline::Specialized);
        }
    }
    // One program, four independently compiled pipelines.
    assert_eq!(cache.len(), 4);
    assert_eq!(builds, 4);
    // Fingerprints and families are both part of the reported key.
    let label = key(7).label();
    assert!(label.starts_with("0000000000000007:"));
    assert!(label.ends_with(":test-family"));
    assert_eq!(FAMILY.name(), "test-family");
}

#[test]
fn an_interpreter_decision_never_reaches_the_backend() {
    let program = rejected_style();
    let mut cache = SpecializedPipelines::<MockDevice>::new();
    let decision = cache.acquire(key(1), &program, || Ok(9_u32));
    let Specialization::Interpreter { state, reason } = decision else {
        panic!("a rejected program must not specialize");
    };
    assert!(state.is_interpreter());
    assert_eq!(state.label(), "typed-bytecode-interpreter");
    assert!(!reason.is_empty());
    assert_eq!(cache.len(), 0);
}
