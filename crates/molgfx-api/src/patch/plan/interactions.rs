//! Interaction channels a patch re-states, and the delta from the live scene.
//!
//! A patch records what it was asked to change, not what a channel used to
//! hold; the previous query lives in the scene specification. Resolving the two
//! is the difference between an interaction edit that re-derives every channel
//! and one that writes only what moved.

use crate::representation::Selection;
use crate::spec::{InteractionChannel, SceneSpec};
use std::collections::BTreeMap;

use super::{Change, assign};

pub(super) struct InteractionUpdates {
    pub(super) focus: Change<Selection>,
    pub(super) selected: Change<Selection>,
    pub(super) hovered: Change<Selection>,
    pub(super) muted: Change<Selection>,
    pub(super) hidden: Change<Selection>,
    pub(super) custom: BTreeMap<Box<str>, Option<Selection>>,
}

impl Default for InteractionUpdates {
    fn default() -> Self {
        Self {
            focus: Change::Unchanged,
            selected: Change::Unchanged,
            hovered: Change::Unchanged,
            muted: Change::Unchanged,
            hidden: Change::Unchanged,
            custom: BTreeMap::new(),
        }
    }
}

impl InteractionUpdates {
    pub(super) fn set(&mut self, channel: &InteractionChannel, selection: Option<Selection>) {
        match channel {
            InteractionChannel::Selected => self.selected = Change::Set(selection),
            InteractionChannel::Hovered => self.hovered = Change::Set(selection),
            InteractionChannel::Focused => self.focus = Change::Set(selection),
            InteractionChannel::Muted => self.muted = Change::Set(selection),
            InteractionChannel::Hidden => self.hidden = Change::Set(selection),
            InteractionChannel::Custom(name) => {
                let _ = self.custom.insert(name.clone(), selection);
            }
        }
    }

    /// Channel names visible to a visual program after this patch commits:
    /// the base scene's channels, plus the ones this patch adds, minus the ones
    /// it clears. A style may read a channel the same patch declares.
    pub(super) fn channel_names(&self, base: &SceneSpec) -> Vec<Box<str>> {
        let mut names: Vec<Box<str>> = base
            .custom_interactions
            .keys()
            .filter(|name| !matches!(self.custom.get(*name), Some(None)))
            .cloned()
            .collect();
        for (name, selection) in &self.custom {
            if selection.is_some() && !names.contains(name) {
                names.push(name.clone());
            }
        }
        names.sort_unstable();
        names
    }

    /// Every channel whose query this patch moved, with the query it now has.
    ///
    /// The delta is computed from the resolved channel sets rather than from
    /// the patch's own bookkeeping, so a channel the patch adds, replaces, or
    /// clears all fall out of the same comparison, and a channel it does not
    /// mention is absent from the result and left untouched. A cleared channel
    /// carries no successor, so the rows still holding its bit are retired
    /// without evaluating any query.
    pub(super) fn restated_channels(
        &self,
        spec: &SceneSpec,
    ) -> Vec<(crate::property::registry::StateChannel, Option<Selection>)> {
        let mut after_spec = spec.clone();
        self.clone_into_spec(&mut after_spec);
        let before = crate::scene::interaction::channels(spec);
        let after = crate::scene::interaction::channels(&after_spec);
        let mut moved: Vec<_> = after
            .iter()
            .filter(|(channel, next)| {
                before
                    .iter()
                    .find(|(candidate, _)| candidate == channel)
                    .is_none_or(|(_, previous)| previous.source() != next.source())
            })
            .map(|(channel, next)| (*channel, Some(next.clone())))
            .collect();
        for (channel, _) in &before {
            if !after.iter().any(|(candidate, _)| candidate == channel) {
                moved.push((*channel, None));
            }
        }
        moved
    }

    /// True when this patch changes any interaction channel.
    pub(super) fn touched(&self) -> bool {
        !matches!(self.focus, Change::Unchanged)
            || !matches!(self.selected, Change::Unchanged)
            || !matches!(self.hovered, Change::Unchanged)
            || !matches!(self.muted, Change::Unchanged)
            || !matches!(self.hidden, Change::Unchanged)
            || !self.custom.is_empty()
    }

    /// The channels this patch leaves behind, paired with their queries.
    pub(super) fn channels(
        &self,
        base: &SceneSpec,
    ) -> Vec<(crate::property::registry::StateChannel, Selection)> {
        let mut candidate = base.clone();
        self.clone_into_spec(&mut candidate);
        crate::scene::interaction::channels(&candidate)
    }

    pub(super) fn clone_into_spec(&self, spec: &mut SceneSpec) {
        assign(&mut spec.focus, self.focus.clone());
        assign(&mut spec.selected, self.selected.clone());
        assign(&mut spec.hovered, self.hovered.clone());
        assign(&mut spec.muted, self.muted.clone());
        assign(&mut spec.hidden, self.hidden.clone());
        for (name, selection) in &self.custom {
            if let Some(selection) = selection {
                let _ = spec
                    .custom_interactions
                    .insert(name.clone(), selection.clone());
            } else {
                let _ = spec.custom_interactions.remove(name);
            }
        }
    }

    pub(super) fn selections(&self) -> impl Iterator<Item = &Selection> {
        [
            self.focus.value(),
            self.selected.value(),
            self.hovered.value(),
            self.muted.value(),
            self.hidden.value(),
        ]
        .into_iter()
        .flatten()
        .chain(self.custom.values().filter_map(Option::as_ref))
    }

    pub(super) fn commit(self, spec: &mut SceneSpec) {
        assign(&mut spec.focus, self.focus);
        assign(&mut spec.selected, self.selected);
        assign(&mut spec.hovered, self.hovered);
        assign(&mut spec.muted, self.muted);
        assign(&mut spec.hidden, self.hidden);
        for (name, selection) in self.custom {
            if let Some(selection) = selection {
                let _ = spec.custom_interactions.insert(name, selection);
            } else {
                let _ = spec.custom_interactions.remove(&name);
            }
        }
    }
}
