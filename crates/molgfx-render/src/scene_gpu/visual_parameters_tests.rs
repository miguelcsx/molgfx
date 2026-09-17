use super::VisualParameterTable;
use crate::testing::MockDevice;

#[test]
fn parameter_ranges_are_fixed_width_and_growth_is_reused() {
    let device = MockDevice::default();
    let mut table = VisualParameterTable::<MockDevice>::new();

    assert!(
        table
            .reserve(&device, 1_000)
            .unwrap_or_else(|error| panic!("parameter arena should reserve: {error}"))
    );
    let revision = table.binding_revision();
    assert!(
        !table
            .reserve(&device, 900)
            .unwrap_or_else(|error| panic!("smaller resident count should reuse storage: {error}"))
    );

    assert_eq!(VisualParameterTable::<MockDevice>::offset(0), 0);
    assert_eq!(VisualParameterTable::<MockDevice>::offset(7), 112);
    assert_eq!(table.binding_revision(), revision);
}
