//! Named captures preserve symbolic forms and targets alongside the resolved scene.
use super::{LayerSpec, RuleSpec, SessionSpec};
use crate::ir::Look;
use crate::{Name, QueryText};
use molgfx_scene::{AppearanceRuleId, StructureId, interop::SceneSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A captured scene and the authoring names that produced it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct NamedSnapshot {
    /// Complete semantic state and its verified content identity.
    pub scene: SceneSnapshot,
    /// Symbolic targets, forms and controls associated with that semantic state.
    pub symbols: SnapshotSymbols,
}

/// Authoring state of one capture, excluding the surrounding capture library and history.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SnapshotSymbols {
    /// Names of captured molecular inputs.
    pub structures: BTreeMap<Name, StructureId>,
    /// Original selection expressions, retaining their alias dependencies.
    pub selections: BTreeMap<Name, QueryText>,
    /// Layer forms, numerical controls and original targets.
    pub layers: BTreeMap<Name, LayerSpec>,
    /// Original colour rule targets.
    pub rules: BTreeMap<AppearanceRuleId, RuleSpec>,
    /// Original focus target.
    pub focus: Option<QueryText>,
}

impl NamedSnapshot {
    pub(crate) fn capture(scene: &molgfx_scene::SceneSpec, session: &SessionSpec) -> Self {
        Self {
            scene: SceneSnapshot::capture(scene),
            symbols: SnapshotSymbols {
                structures: session.structures.clone(),
                selections: session.selections.clone(),
                layers: session.layers.clone(),
                rules: session.rules.clone(),
                focus: session.focus.clone(),
            },
        }
    }

    /// Verifies semantic integrity and symbolic ownership before restoration.
    /// # Errors
    /// Refuses corrupt semantic state or names referring to missing captured objects.
    pub fn verify(&self) -> Result<(), molgfx_scene::Error> {
        self.scene.verify()?;
        let scene = &self.scene.scene;
        if self
            .symbols
            .structures
            .values()
            .any(|id| !scene.structures.contains_key(id))
            || self.symbols.layers.values().any(|layer| {
                !scene.representations.contains_key(&layer.id)
                    || !scene.structures.contains_key(&layer.structure)
            })
            || self
                .symbols
                .rules
                .keys()
                .any(|id| !scene.appearance.contains_key(id))
        {
            return Err(molgfx_scene::Error::InvalidSpec(
                "named capture has contradictory symbolic ownership".to_owned(),
            ));
        }
        for layer in self.symbols.layers.values() {
            let Some(actual) = scene.representations.get(&layer.id) else {
                continue;
            };
            let expected = layer.form.specification(
                actual.target().clone(),
                Look {
                    structure: layer.structure,
                    color: None,
                    opacity: None,
                },
            );
            if actual.structure_id() != Some(layer.structure) || !actual.same_geometry(&expected) {
                return Err(molgfx_scene::Error::InvalidSpec(
                    "named capture has contradictory symbolic geometry".to_owned(),
                ));
            }
        }
        let mut symbols = SessionSpec::default();
        self.restore_symbols(&mut symbols);
        let mut state = super::state::State::from_spec(symbols);
        state.sync(scene);
        if scene.selected.is_none() {
            // Existing layers retain their resolved targets after selection is cleared.
            // Historic `$sel` expressions still need reference and cycle validation.
            let placeholder = QueryText::compile("all").map_err(|_| {
                molgfx_scene::Error::InvalidSpec("capture validation query failed".to_owned())
            })?;
            state
                .aliases
                .define("sel", placeholder.query().clone())
                .map_err(|_| {
                    molgfx_scene::Error::InvalidSpec("capture validation alias failed".to_owned())
                })?;
        }
        let resolver = super::resolve::Resolver {
            state: &state,
            site: None,
        };
        for query in self
            .symbols
            .selections
            .values()
            .chain(self.symbols.layers.values().map(|layer| &layer.target))
            .chain(self.symbols.rules.values().map(|rule| &rule.target))
            .chain(self.symbols.focus.iter())
        {
            resolver.selection(query).map_err(|error| {
                molgfx_scene::Error::InvalidSpec(format!(
                    "named capture has invalid symbolic target: {}",
                    error.message
                ))
            })?;
        }
        Ok(())
    }

    pub(crate) fn restore_symbols(&self, current: &mut SessionSpec) {
        current.structures.clone_from(&self.symbols.structures);
        current.selections.clone_from(&self.symbols.selections);
        current.layers.clone_from(&self.symbols.layers);
        current.rules.clone_from(&self.symbols.rules);
        current.focus.clone_from(&self.symbols.focus);
    }
}
