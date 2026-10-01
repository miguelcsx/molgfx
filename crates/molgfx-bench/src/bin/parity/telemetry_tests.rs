use super::normalize_external;
use serde_json::{Value, json};

#[test]
fn external_counter_absence_does_not_become_measured_zero() {
    let mut report = json!({
        "cold_output":{"sample":{"cpu_ns":100,"frame_ns":110,"gpu_ns":null}},
        "warmup_samples":[{"sample":{"cpu_ns":60,"frame_ns":70,"gpu_ns":null}}],
        "samples":[
            {"sample":{"cpu_ns":10,"frame_ns":20,"gpu_ns":0}},
            {"sample":{"cpu_ns":30,"frame_ns":40,"gpu_ns":null}},
            {"sample":{"cpu_ns":50,"frame_ns":60,"gpu_ns":null}}
        ]
    });
    normalize_external(&mut report).expect("external report normalizes");
    let summary = &report["summary"];
    assert_eq!(summary["output_count"], 3);
    assert_eq!(summary["frame_median_ns"], 40);
    assert_eq!(summary["frame_p95_ns"], 60);
    assert_eq!(summary["gpu_resolved_count"], 1);
    assert_eq!(summary["gpu_median_ns"], 0);
    for field in super::SUMMARY_COUNTERS {
        assert_eq!(summary[field], Value::Null, "{field}");
        assert_eq!(
            summary[format!("{field}_unavailable_reason")],
            super::REASON
        );
    }
    for row in [
        &report["cold_output"],
        &report["warmup_samples"][0],
        &report["samples"][0],
    ] {
        for field in super::COUNTERS {
            assert_eq!(row["sample"][field], Value::Null, "{field}");
            assert_eq!(row[format!("{field}_unavailable_reason")], super::REASON);
        }
        assert_eq!(row["cpu_stages"], Value::Null);
        assert_eq!(row["cpu_timing_scope"], super::CPU_SCOPE);
    }
    assert_eq!(report["cold_output"]["sample"]["frame_ns"], 110);
    assert_eq!(report["warmup_samples"][0]["sample"]["frame_ns"], 70);
    assert_eq!(report["samples"][0]["sample"]["gpu_ns"], 0);
    assert_eq!(report["samples"][1]["sample"]["gpu_ns"], Value::Null);
    assert!(report["samples"][1]["gpu_unavailable_reason"].is_string());
}
