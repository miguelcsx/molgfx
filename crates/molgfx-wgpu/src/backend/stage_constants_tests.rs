//! The override scan, on small modules and against naga for the whole library.

use super::EntryOverrides;
use std::collections::BTreeSet;

fn used(source: &str, entry: &str) -> Vec<String> {
    match EntryOverrides::scan(source).used_by(entry) {
        Some(used) => used.into_iter().map(str::to_owned).collect(),
        None => panic!("{entry} should be found"),
    }
}

const MODULE: &str = "
override FRAGMENT_ONLY: u32 = 1u;
override VERTEX_ONLY: f32 = 2.0;
override DERIVED: f32 = VERTEX_ONLY * 2.0;
override WIDTH: u32 = 64u;
struct Out { @builtin(position) position: vec4f, @location(0) fragment_only: f32 }
fn position_of(index: u32) -> vec4f {
    // FRAGMENT_ONLY is only named in this comment.
    /* nor /* here */ FRAGMENT_ONLY */
    return vec4f(f32(index) * DERIVED, 0.0, 0.0, 1.0);
}
fn shade(out: Out) -> vec4f {
    return vec4f(out.fragment_only, f32(FRAGMENT_ONLY), 1.0e-4, 1.0);
}
@vertex fn vs(@builtin(vertex_index) index: u32) -> Out {
    var out: Out;
    out.position = position_of(index);
    return out;
}
@fragment fn fs(input: Out) -> @location(0) vec4f {
    return shade(input);
}
var<workgroup> tile: array<f32, WIDTH>;
@compute @workgroup_size(WIDTH) fn cs() {
    tile[0] = 1.0;
}
";

#[test]
fn each_stage_reaches_only_its_own_overrides() {
    assert_eq!(used(MODULE, "vs"), ["DERIVED", "VERTEX_ONLY"]);
    assert_eq!(used(MODULE, "fs"), ["FRAGMENT_ONLY"]);
}

#[test]
fn attributes_and_workgroup_variable_types_count_as_uses() {
    assert_eq!(used(MODULE, "cs"), ["WIDTH"]);
}

#[test]
fn a_stage_keeps_only_the_constants_it_uses() {
    let scan = EntryOverrides::scan(MODULE);
    let constants = [
        ("FRAGMENT_ONLY", 3.0),
        ("VERTEX_ONLY", 4.0),
        ("UNKNOWN", 5.0),
    ];
    assert_eq!(
        scan.filter("vs", &constants),
        [("VERTEX_ONLY", 4.0), ("UNKNOWN", 5.0)]
    );
    assert_eq!(
        scan.filter("fs", &constants),
        [("FRAGMENT_ONLY", 3.0), ("UNKNOWN", 5.0)]
    );
    // An entry point the scan cannot find keeps every constant.
    assert_eq!(scan.filter("missing", &constants), constants);
}

/// The overrides naga keeps once every other entry point is removed.
fn naga_used(module: &naga::Module, entry: &str) -> BTreeSet<String> {
    let mut module = module.clone();
    module.entry_points.retain(|point| point.name == entry);
    naga::compact::compact(&mut module, naga::compact::KeepUnused::No);
    module
        .overrides
        .iter()
        .filter_map(|(_, value)| value.name.clone())
        .collect()
}

#[test]
fn the_scan_matches_naga_for_every_entry_point_in_the_library() {
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for (unit, source) in molgfx_shaders::UNITS {
        let module = match naga::front::wgsl::parse_str(source) {
            Ok(module) => module,
            Err(error) => panic!("{unit} should parse: {error}"),
        };
        let scan = EntryOverrides::scan(source);
        for point in &module.entry_points {
            let expected = naga_used(&module, &point.name);
            let found: BTreeSet<String> = match scan.used_by(&point.name) {
                Some(used) => used.into_iter().map(str::to_owned).collect(),
                None => panic!("{unit}: the scan misses entry point {}", point.name),
            };
            if found != expected {
                mismatches.push(format!(
                    "{unit}::{}: scan {found:?}, naga {expected:?}",
                    point.name
                ));
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} entry points were checked");
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}
