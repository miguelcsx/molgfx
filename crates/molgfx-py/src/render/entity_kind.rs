//! Stable Python spelling for renderer entity identities.

pub(super) const fn name(kind: molgfx::core::EntityKind) -> &'static str {
    match kind {
        molgfx::core::EntityKind::Atom => "atom",
        molgfx::core::EntityKind::Bond => "bond",
        molgfx::core::EntityKind::Edge => "edge",
        molgfx::core::EntityKind::Label => "label",
        molgfx::core::EntityKind::Primitive => "primitive",
        molgfx::core::EntityKind::Mesh => "mesh",
        molgfx::core::EntityKind::LigandPoseBatch => "ligand_pose_batch",
        molgfx::core::EntityKind::Guide => "guide",
        molgfx::core::EntityKind::DynamicBond => "dynamic_bond",
        molgfx::core::EntityKind::Point => "point",
        molgfx::core::EntityKind::Instance => "instance",
        molgfx::core::EntityKind::TemplatePart => "template_part",
        molgfx::core::EntityKind::Relation => "relation",
    }
}
