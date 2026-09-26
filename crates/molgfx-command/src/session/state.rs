//! What a session knows: names, selections, layers, colour rules and focus.

use crate::ir::{ColorValue, Form, Name, QueryText};
use molgfx_api::{AppearanceRuleId, RepresentationId, SceneSpec, StructureId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A drawn layer: one representation the session created.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LayerSpec {
    /// The scene representation that draws it.
    pub id: RepresentationId,
    /// The structure it draws from.
    pub structure: StructureId,
    /// Its form and controls.
    pub form: Form,
    /// Its target as written, named selections included.
    pub target: QueryText,
}

/// A colour rule the session created.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RuleSpec {
    /// The structure whose atoms it colours.
    pub structure: StructureId,
    /// Its target as written, named selections included.
    pub target: QueryText,
    /// The colour.
    pub color: ColorValue,
}

/// A session's portable symbolic state.
///
/// Selections keep the text their author wrote, `$name` references included,
/// and layers and rules keep their declared targets, so reloading this state
/// preserves what each depends on and not only what it currently resolves to.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct SessionSpec {
    /// Structure names.
    pub structures: BTreeMap<Name, StructureId>,
    /// Named selections, as declared.
    pub selections: BTreeMap<Name, QueryText>,
    /// Layers by name.
    pub layers: BTreeMap<Name, LayerSpec>,
    /// Colour rules by scene identity.
    pub rules: BTreeMap<AppearanceRuleId, RuleSpec>,
    /// The focus target, as declared.
    pub focus: Option<QueryText>,
}

impl SessionSpec {
    /// Deterministic JSON.
    ///
    /// # Errors
    ///
    /// Returns the serializer's error.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Reads JSON written by [`Self::to_json`].
    ///
    /// # Errors
    ///
    /// Returns the parser's error, including any invalid name or query.
    pub fn from_json(source: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(source)
    }
}

/// The spec together with the alias table derived from its selections.
#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    pub(crate) spec: SessionSpec,
    pub(crate) aliases: molframe::QueryAliases,
}

impl State {
    pub(crate) fn from_spec(spec: SessionSpec) -> Self {
        let mut aliases = molframe::QueryAliases::new();
        for (name, query) in &spec.selections {
            // Every `Name` is a valid alias name, so defining cannot fail.
            let _ = aliases.define(name.as_str(), query.query().clone());
        }
        Self { spec, aliases }
    }

    /// Brings names in line with the scene: every structure gets a name, and
    /// layers and rules whose scene objects are gone are forgotten.
    pub(crate) fn sync(&mut self, scene: &SceneSpec) {
        let spec = &mut self.spec;
        spec.structures
            .retain(|_, id| scene.structures.contains_key(id));
        for id in scene.structures.keys() {
            if spec.structures.values().any(|known| known == id) {
                continue;
            }
            let mut ordinal = id.get();
            loop {
                if let Ok(name) = Name::new(&format!("s{ordinal}"))
                    && !spec.structures.contains_key(&name)
                {
                    let _ = spec.structures.insert(name, *id);
                    break;
                }
                ordinal = ordinal.wrapping_add(1);
            }
        }
        spec.layers.retain(|_, layer| {
            scene.representations.contains_key(&layer.id)
                && scene.structures.contains_key(&layer.structure)
        });
        spec.rules.retain(|id, _| scene.appearance.contains_key(id));
        if scene.focus.is_none() {
            spec.focus = None;
        }
    }

    /// The structure's name.
    pub(crate) fn structure_name(&self, id: StructureId) -> Option<&Name> {
        self.spec
            .structures
            .iter()
            .find_map(|(name, known)| (*known == id).then_some(name))
    }

    /// Whether `query` depends on `name`, directly or through other
    /// selections.
    pub(crate) fn depends_on(&self, query: &QueryText, name: &str) -> bool {
        let mut pending: Vec<&str> = query.references();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(reference) = pending.pop() {
            if reference == name {
                return true;
            }
            if !seen.insert(reference) {
                continue;
            }
            if let Some(definition) = self.selection(reference) {
                pending.extend(definition.references());
            }
        }
        false
    }

    /// The path of references from `from` back to `name`, if one exists.
    pub(crate) fn path_to(&self, from: &str, name: &str) -> Option<Vec<String>> {
        let mut stack = vec![(from.to_owned(), vec![from.to_owned()])];
        let mut seen = std::collections::BTreeSet::new();
        while let Some((current, path)) = stack.pop() {
            if current == name {
                return Some(path);
            }
            if !seen.insert(current.clone()) {
                continue;
            }
            if let Some(definition) = self.selection(&current) {
                for reference in definition.references() {
                    let mut next = path.clone();
                    next.push(reference.to_owned());
                    stack.push((reference.to_owned(), next));
                }
            }
        }
        None
    }

    pub(crate) fn selection(&self, name: &str) -> Option<&QueryText> {
        let name = Name::new(name).ok()?;
        self.spec.selections.get(&name)
    }

    /// Who refers to the selection `name` directly.
    pub(crate) fn users_of(&self, name: &str) -> Vec<String> {
        let refers = |query: &QueryText| query.references().contains(&name);
        let mut users: Vec<String> = self
            .spec
            .selections
            .iter()
            .filter(|(_, query)| refers(query))
            .map(|(user, _)| format!("selection '{user}'"))
            .collect();
        users.extend(
            self.spec
                .layers
                .iter()
                .filter(|(_, layer)| refers(&layer.target))
                .map(|(user, _)| format!("layer '@{user}'")),
        );
        users.extend(
            self.spec
                .rules
                .values()
                .filter(|rule| refers(&rule.target))
                .map(|rule| format!("colour rule '{}'", rule.target)),
        );
        if self.spec.focus.as_ref().is_some_and(refers) {
            users.push("the focus".to_owned());
        }
        users
    }
}
