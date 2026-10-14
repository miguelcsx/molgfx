use super::HermiteGaps;

fn blend(weights: [f64; 4], values: [f64; 4]) -> f64 {
    weights.iter().zip(values).map(|(w, v)| w * v).sum()
}

#[test]
fn hermite_weights_sum_to_one_so_a_constant_field_stays_constant() {
    let gaps = HermiteGaps {
        interval: 0.7,
        before: Some(0.3),
        after: Some(2.0),
    };
    for step in 0..=10 {
        let sum: f64 = gaps.weights(f64::from(step) / 10.0).iter().sum();
        assert!((sum - 1.0).abs() < 1.0e-12, "{sum}");
    }
}

#[test]
fn hermite_without_neighbours_has_the_linear_weights() {
    let gaps = HermiteGaps {
        interval: 3.0,
        before: None,
        after: None,
    };
    for step in 0..=8 {
        let alpha = f64::from(step) / 8.0;
        let weights = gaps.weights(alpha);
        let linear = [0.0, 1.0 - alpha, alpha, 0.0];
        for (got, want) in weights.iter().zip(linear) {
            assert!((got - want).abs() < 1.0e-12, "{weights:?} vs {linear:?}");
        }
    }
}

#[test]
fn hermite_with_neighbours_reproduces_a_quadratic_exactly() {
    // The three-point tangent is exact for parabolas, so the spline is too.
    let times = [0.0_f64, 0.4, 1.5, 2.0];
    let parabola = |t: f64| 3.0 * t * t - 2.0 * t + 1.0;
    let values = times.map(parabola);
    let gaps = HermiteGaps {
        interval: times[2] - times[1],
        before: Some(times[1] - times[0]),
        after: Some(times[3] - times[2]),
    };
    for step in 0..=10 {
        let alpha = f64::from(step) / 10.0;
        let time = times[1] + alpha * (times[2] - times[1]);
        let got = blend(gaps.weights(alpha), values);
        assert!((got - parabola(time)).abs() < 1.0e-12, "{got}");
    }
}
