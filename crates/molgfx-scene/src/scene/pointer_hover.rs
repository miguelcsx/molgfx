//! Transient molecular hover without changing authored selections or revisions.
use super::{ResolvedPick, Scene};
use crate::Error;

impl Scene {
    /// Highlights a picked atom or both endpoints of a picked bond.
    /// This overrides the hovered channel while the pointer is over a molecule;
    /// clearing it restores the authored hovered selection. The portable scene
    /// and command history stay unchanged. Returns whether GPU state changed.
    ///
    /// # Errors
    /// Rejects picks from a removed structure or a different topology.
    pub fn set_pointer_hover(&mut self, pick: Option<&ResolvedPick>) -> Result<bool, Error> {
        let (structure, dataset, revision, rows, count) = match pick {
            Some(ResolvedPick::Atom(atom)) => (
                atom.structure,
                atom.dataset,
                atom.topology_revision,
                [atom.atom_index, 0],
                1,
            ),
            Some(ResolvedPick::Bond(bond)) => (
                bond.structure,
                bond.dataset,
                bond.topology_revision,
                [bond.atom_a, bond.atom_b],
                2,
            ),
            _ => {
                if self.spec.hovered.is_some() {
                    let before = self.resolved.interaction_state_revision();
                    super::interaction::update_channel(
                        &mut self.resolved,
                        crate::property::registry::StateChannel::Hovered,
                        self.spec.hovered.as_ref(),
                    )?;
                    return Ok(before != self.resolved.interaction_state_revision());
                }
                return self
                    .resolved
                    .update_pointer_hover(None, &[])
                    .map_err(Error::from);
            }
        };
        if revision != self.resolved.structure_revision() {
            return Err(Error::StalePick { dataset });
        }
        let source = self
            .structures
            .get(&structure)
            .ok_or(Error::StalePick { dataset })?;
        let mut target = None;
        for (handle, placed) in self.resolved.structures() {
            if placed.dataset_id().get() == dataset && placed.source.shares_storage_with(source) {
                if target.is_some() {
                    return Err(Error::AmbiguousPick { dataset });
                }
                target = Some(handle);
            }
        }
        let target = target.ok_or(Error::StalePick { dataset })?;
        self.resolved
            .update_pointer_hover(Some(target), &rows[..count])
            .map_err(Error::from)
    }
}

#[cfg(test)]
#[path = "pointer_hover_tests.rs"]
mod tests;
