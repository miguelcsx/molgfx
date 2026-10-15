// Scalar-field semantics serialization.
use crate::ScalarFieldSemantics;
use crate::serialization::types::ScalarSemanticsDescription;

pub(crate) fn scalar_semantics(value: &ScalarFieldSemantics) -> ScalarSemanticsDescription {
    match value {
        ScalarFieldSemantics::UncalibratedRank => ScalarSemanticsDescription {
            kind: "rank".to_owned(),
            name: None,
            units: None,
            provenance: None,
        },
        ScalarFieldSemantics::Quantity {
            name,
            units,
            provenance,
        } => ScalarSemanticsDescription {
            kind: "quantity".to_owned(),
            name: Some(name.to_string()),
            units: Some(units.to_string()),
            provenance: Some(provenance.to_string()),
        },
    }
}
