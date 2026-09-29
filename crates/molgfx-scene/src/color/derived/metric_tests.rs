use super::AtomMetric;
use molgfx_core::MolecularSource;

fn structure() -> molframe::Structure {
    const PDB: &str = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  0.50 12.00           N\n\
ATOM      2  CA  ALA A   1       1.460   0.000   0.000  1.00 20.00           C\n\
ATOM      3  N   ARG A   2       3.800   0.000   0.000  1.00 30.00           N\n\
ATOM      4  CA  ARG A   2       5.260   0.000   0.000  1.00 40.00           C\n\
ATOM      5  N   ILE A   3       7.600   0.000   0.000  1.00 50.00           N\n\
END\n";
    let result = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("metrics.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

fn source() -> MolecularSource {
    MolecularSource::from_molframe(&structure())
}

fn column(metric: AtomMetric) -> Vec<f32> {
    metric
        .column(&source())
        .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn occupancy_and_b_factor_read_the_atom_columns() {
    assert_eq!(column(AtomMetric::Occupancy), [0.5, 1.0, 1.0, 1.0, 1.0]);
    assert_eq!(column(AtomMetric::BFactor), [12.0, 20.0, 30.0, 40.0, 50.0]);
}

#[test]
fn hydrophobicity_is_constant_across_a_residue_and_follows_the_scale() {
    assert_eq!(
        column(AtomMetric::Hydrophobicity),
        [1.8, 1.8, -4.5, -4.5, 4.5]
    );
}

#[test]
fn sequence_position_runs_from_zero_at_the_first_residue_to_one_at_the_last() {
    assert_eq!(
        column(AtomMetric::SequencePosition),
        [0.0, 0.0, 0.5, 0.5, 1.0]
    );
}

#[test]
fn every_metric_colors_through_a_derived_property_bound_on_first_use() {
    let mut scene =
        crate::Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    // A PDB may carry no formal charges. The program still applies — every
    // atom keeps its own colour — rather than being refused, whether or not
    // that column happens to hold values.
    let missing = crate::color::metric(AtomMetric::FormalCharge);
    let applied = scene.add(crate::rep::spacefill(crate::sel::all()).color(missing));
    assert!(applied.is_ok(), "{applied:?}");
    for metric in AtomMetric::ALL {
        let spec = crate::color::metric(metric);
        assert!(spec.validate().is_ok(), "{metric:?}");
        assert!(spec.legend().is_some(), "{metric:?}");
        let added = scene.add(crate::rep::spacefill(crate::sel::all()).color(spec));
        assert!(added.is_ok(), "{metric:?}: {added:?}");
    }
    let name = crate::color::DerivedColumn::Metric(AtomMetric::Occupancy)
        .property_name(crate::StructureId::new(1));
    assert!(scene.spec().properties.contains_key(name.as_str()));
}

#[test]
fn sasa_reads_the_current_coordinates_and_is_finite_for_every_atom() {
    let values = column(AtomMetric::Sasa);
    assert_eq!(values.len(), 5);
    assert!(
        values.iter().all(|value| value.is_finite() && *value > 0.0),
        "{values:?}"
    );
    // The buried central atoms are less exposed than the terminal ones, which
    // is the ordering the metric exists to show.
    let terminal = values[0];
    let middle = values[2];
    assert!(middle < terminal, "middle {middle} vs terminal {terminal}");
}

#[test]
fn every_metric_has_a_distinct_name_and_a_ramp_that_covers_its_domain() {
    let mut names: Vec<&str> = AtomMetric::ALL.into_iter().map(AtomMetric::name).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "names are distinct");
    for metric in AtomMetric::ALL {
        let [low, high] = metric.domain();
        assert!(low < high, "{metric:?}");
        assert!(!metric.ramp().is_empty(), "{metric:?}");
        assert!(
            crate::color::ramp_base_name(metric.ramp()).len() == metric.ramp().len(),
            "{metric:?} names a real ramp"
        );
    }
}
