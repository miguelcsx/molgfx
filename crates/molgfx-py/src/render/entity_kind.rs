//! Stable Python spelling for renderer entity identities.

pub(super) const fn name(kind: molgfx::EntityKind) -> &'static str {
    match kind {
        molgfx::EntityKind::Atom => "atom",
        molgfx::EntityKind::Bond => "bond",
        molgfx::EntityKind::Edge => "edge",
        molgfx::EntityKind::Label => "label",
        molgfx::EntityKind::Primitive => "primitive",
        molgfx::EntityKind::Mesh => "mesh",
        molgfx::EntityKind::LigandPoseBatch => "unknown",
        molgfx::EntityKind::Guide => "guide",
        molgfx::EntityKind::DynamicBond => "dynamic_bond",
        molgfx::EntityKind::Point => "point",
        molgfx::EntityKind::Instance => "instance",
        molgfx::EntityKind::TemplatePart => "template_part",
        molgfx::EntityKind::Relation => "relation",
    }
}
