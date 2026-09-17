use super::*;
use molgfx_core::{
    AttributeColumn, AttributeHandle, AttributeValues, PointBatch, PointGlyph, PointStyle,
    RowDomain, ScalarRamp, Scene, SourceNamespace, SourceRows,
};
use molgfx_math::Rgba8;
use std::sync::Arc;

fn point_domain(scene: &mut Scene, namespace: u64) -> RowDomain {
    let batch = PointBatch::new(
        Arc::from([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
        SourceRows::ordered(SourceNamespace(namespace), 2),
        PointGlyph::Sphere,
        PointStyle::default(),
    )
    .unwrap_or_else(|error| panic!("point batch: {error}"));
    RowDomain::Points(scene.add_point_batch(batch))
}

fn scalar(scene: &mut Scene, domain: RowDomain, name: &str) -> AttributeHandle {
    let column = AttributeColumn::new(domain, name, AttributeValues::Scalar(Arc::from([0.0, 1.0])))
        .unwrap_or_else(|error| panic!("attribute: {error}"));
    scene
        .add_attribute(column)
        .unwrap_or_else(|error| panic!("insert attribute: {error}"))
}

#[test]
fn focus_composition_attaches_a_generic_attribute_program() {
    let mut scene = Scene::new();
    let domain = point_domain(&mut scene, 1);
    let emphasis = scalar(&mut scene, domain, "caller emphasis");
    let view = scene
        .compose_focus(
            FocusLayer { domain, emphasis },
            FocusCompositionStyle::default(),
        )
        .unwrap_or_else(|error| panic!("compose focus: {error}"));
    assert_eq!(view.domains, [domain]);
    assert!(scene.domain_visual(domain).is_some());
}

#[test]
fn difference_target_mismatch_is_atomic() {
    let mut scene = Scene::new();
    let left = point_domain(&mut scene, 2);
    let right = point_domain(&mut scene, 3);
    let delta = scalar(&mut scene, left, "caller delta");
    let result = scene.compose_difference(
        &[
            DifferenceLayer {
                domain: left,
                delta,
            },
            DifferenceLayer {
                domain: right,
                delta,
            },
        ],
        DifferenceCompositionStyle {
            context_threshold: 0.1,
            emphasis_threshold: 1.0,
            context_opacity: 0.1,
            ramp: ScalarRamp::sequential([0.0, 1.0]),
            order: 0,
        },
    );
    assert!(matches!(result, Err(CompositionError::Invalid(_))));
    assert!(scene.domain_visual(left).is_none());
    assert!(scene.domain_visual(right).is_none());
}

#[test]
fn ensemble_normalizes_caller_weights_without_a_builtin_palette() {
    let mut scene = Scene::new();
    let left = point_domain(&mut scene, 4);
    let right = point_domain(&mut scene, 5);
    let view = scene
        .compose_ensemble(
            &[
                EnsembleLayer {
                    domain: left,
                    weight: 1.0,
                    color: Rgba8::opaque(1, 2, 3),
                },
                EnsembleLayer {
                    domain: right,
                    weight: 3.0,
                    color: Rgba8::opaque(4, 5, 6),
                },
            ],
            EnsembleCompositionStyle::default(),
        )
        .unwrap_or_else(|error| panic!("compose ensemble: {error}"));
    assert_eq!(view.normalized_weights, [0.25, 0.75]);
    assert!(scene.domain_visual(left).is_some());
    assert!(scene.domain_visual(right).is_some());
}
