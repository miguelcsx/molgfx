//! Stable Python spelling for renderer entity identities.

pub(super) const fn name(kind: pdviewx::EntityKind) -> &'static str {
    match kind {
        pdviewx::EntityKind::Atom => "atom",
        pdviewx::EntityKind::Bond => "bond",
        pdviewx::EntityKind::Edge => "edge",
        pdviewx::EntityKind::Label => "label",
        pdviewx::EntityKind::Primitive => "primitive",
        pdviewx::EntityKind::Mesh => "mesh",
        pdviewx::EntityKind::LigandPoseBatch => "unknown",
        pdviewx::EntityKind::Guide => "guide",
        pdviewx::EntityKind::DynamicBond => "dynamic_bond",
        pdviewx::EntityKind::Point => "point",
        pdviewx::EntityKind::Instance => "instance",
        pdviewx::EntityKind::TemplatePart => "template_part",
        pdviewx::EntityKind::Relation => "relation",
    }
}
