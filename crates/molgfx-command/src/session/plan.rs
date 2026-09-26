//! Planning commands into staged scene edits.
//!
//! The planner owns a copy of the session state and a scene transaction. Each
//! command updates both; nothing reaches the scene or the session until the
//! whole program has planned, so a failing statement leaves no trace.

use super::resolve::Resolver;
use super::state::{LayerSpec, RuleSpec, State};
use crate::error::{CommandError, ErrorKind, Span};
use crate::ir::{ColorValue, Command, Look, Name, QueryText, Show, Target};
use crate::registry;
use molgfx_api::{AppearanceRuleSpec, PatchOperation, SceneTransaction, StructureId};

pub(crate) struct Planner<'a> {
    pub(crate) state: State,
    pub(crate) transaction: SceneTransaction,
    pub(crate) messages: Vec<String>,
    pub(crate) site: Option<(&'a str, Span)>,
}

impl Planner<'_> {
    pub(crate) fn plan(&mut self, command: &Command) -> Result<(), CommandError> {
        match command {
            Command::Select { name, query } => self.select(name, query),
            Command::Unselect { name } => self.unselect(name),
            Command::Show(show) => self.show(show),
            Command::Reveal { layer } => self.visibility(layer, true),
            Command::Hide { layer } => self.visibility(layer, false),
            Command::Remove { layer } => {
                let id = self.layer(layer)?.id;
                self.transaction
                    .remove(id)
                    .map_err(|error| scene_error(&error))?;
                let _ = self.state.spec.layers.remove(layer);
                Ok(())
            }
            Command::Color {
                color,
                target,
                structure,
            } => self.color(color, target, structure.as_ref()),
            Command::Uncolor { target, structure } => {
                self.uncolor(target.as_ref(), structure.as_ref())
            }
            Command::Opacity { value, layer } => {
                let id = self.layer(layer)?.id;
                self.transaction.set_opacity(id, value.get());
                Ok(())
            }
            Command::Focus { target } => {
                let declared = self.declared(target)?;
                let selection = self.resolver().selection(&declared)?;
                self.transaction.focus(selection);
                self.state.spec.focus = Some(declared);
                Ok(())
            }
            Command::Unfocus => {
                self.transaction
                    .stage(PatchOperation::SetFocus { selection: None })
                    .map_err(|error| scene_error(&error))?;
                self.state.spec.focus = None;
                Ok(())
            }
            Command::Undo | Command::Redo => Err(CommandError::new(
                ErrorKind::History,
                "undo and redo cannot be combined with edits in one program",
            )),
        }
    }

    fn resolver(&self) -> Resolver<'_> {
        Resolver {
            state: &self.state,
            site: self.site,
        }
    }

    fn select(&mut self, name: &Name, query: &QueryText) -> Result<(), CommandError> {
        let resolver = self.resolver();
        resolver.check_references(query)?;
        if let Some(path) = query
            .references()
            .iter()
            .find_map(|reference| self.state.path_to(reference, name.as_str()))
        {
            return Err(CommandError::new(
                ErrorKind::Cycle,
                format!(
                    "'{name}' would refer to itself: {name} -> {}",
                    path.join(" -> ")
                ),
            ));
        }
        let previous = self
            .state
            .spec
            .selections
            .insert(name.clone(), query.clone());
        let _ = self
            .state
            .aliases
            .define(name.as_str(), query.query().clone());
        if previous.is_some() {
            self.follow(name)?;
        }
        Ok(())
    }

    /// Re-resolves everything that depends on the redefined selection `name`.
    fn follow(&mut self, name: &Name) -> Result<(), CommandError> {
        let mut retargets = Vec::new();
        for layer in self.state.spec.layers.values() {
            if self.state.depends_on(&layer.target, name.as_str()) {
                retargets.push((layer.id, self.resolver().selection(&layer.target)?));
            }
        }
        let mut rules = Vec::new();
        for (id, rule) in &self.state.spec.rules {
            if self.state.depends_on(&rule.target, name.as_str()) {
                let selection = self.resolver().selection(&rule.target)?;
                rules.push((*id, rule.structure, selection, rule.color.clone()));
            }
        }
        let focus = match &self.state.spec.focus {
            Some(focus) if self.state.depends_on(focus, name.as_str()) => {
                Some(self.resolver().selection(focus)?)
            }
            _ => None,
        };
        let count = retargets.len() + rules.len() + usize::from(focus.is_some());
        for (id, selection) in retargets {
            let unchanged = self
                .transaction
                .spec()
                .representations
                .get(&id)
                .is_some_and(|representation| *representation.target() == selection);
            if !unchanged {
                self.transaction
                    .set_target(id, selection)
                    .map_err(|error| scene_error(&error))?;
            }
        }
        for (id, structure, selection, color) in rules {
            let color = self.color_spec(&color, structure)?;
            let rule = AppearanceRuleSpec::new(structure, selection, color);
            self.transaction
                .replace_appearance_rule(id, rule)
                .map_err(|error| scene_error(&error))?;
        }
        if let Some(selection) = focus {
            self.transaction.focus(selection);
        }
        if count > 0 {
            self.messages.push(format!(
                "'{name}' redefined; {count} dependent item(s) follow it"
            ));
        }
        Ok(())
    }

    fn unselect(&mut self, name: &Name) -> Result<(), CommandError> {
        if !self.state.spec.selections.contains_key(name) {
            return Err(self.resolver().unknown_selection(name.as_str()));
        }
        let users = self.state.users_of(name.as_str());
        if !users.is_empty() {
            return Err(CommandError::new(
                ErrorKind::InUse,
                format!("'{name}' is still used by {}", users.join(", ")),
            ));
        }
        let _ = self.state.spec.selections.remove(name);
        let _ = self.state.aliases.remove(name.as_str());
        Ok(())
    }

    fn show(&mut self, show: &Show) -> Result<(), CommandError> {
        let (structure, declared) = match &show.target {
            Target::Layer(layer) => {
                let layer = self.layer(layer)?;
                (layer.structure, layer.target.clone())
            }
            Target::Query(query) => (self.structure(show.structure.as_ref())?, query.clone()),
        };
        let selection = self.resolver().selection(&declared)?;
        let color = match &show.color {
            Some(color) => Some(self.color_spec(color, structure)?),
            None => None,
        };
        let opacity = show.opacity.map(crate::ir::Opacity::get);
        if !show.duplicate
            && let Some((name, id)) = self.existing(structure, &declared, show)
        {
            self.transaction.set_visible(id, true);
            if let Some(color) = color {
                self.transaction
                    .set_color(id, color)
                    .map_err(|error| scene_error(&error))?;
            }
            if let Some(opacity) = opacity {
                self.transaction.set_opacity(id, opacity);
            }
            self.messages
                .push(format!("layer '@{name}' already draws this; it is shown"));
            return Ok(());
        }
        let name = match &show.layer {
            Some(name) if self.state.spec.layers.contains_key(name) => {
                return Err(CommandError::new(
                    ErrorKind::Conflict,
                    format!(
                        "layer '@{name}' already exists; remove it first or choose another name"
                    ),
                ));
            }
            Some(name) => name.clone(),
            None => self.layer_name(show.form.kind().name()),
        };
        let look = Look {
            structure,
            color,
            opacity,
        };
        let id = self
            .transaction
            .add(show.form.specification(selection, look))
            .map_err(|error| scene_error(&error))?;
        self.messages.push(format!("layer '@{name}' added"));
        let _ = self.state.spec.layers.insert(
            name,
            LayerSpec {
                id,
                structure,
                form: show.form,
                target: declared,
            },
        );
        Ok(())
    }

    /// The layer a `show` would duplicate, unless it names a different layer.
    fn existing(
        &self,
        structure: StructureId,
        declared: &QueryText,
        show: &Show,
    ) -> Option<(Name, molgfx_api::RepresentationId)> {
        self.state.spec.layers.iter().find_map(|(name, layer)| {
            let same = layer.structure == structure
                && layer.form == show.form
                && layer.target == *declared
                && show.layer.as_ref().is_none_or(|wanted| wanted == name);
            same.then(|| (name.clone(), layer.id))
        })
    }

    fn layer_name(&self, form: &str) -> Name {
        let mut ordinal = 1_u32;
        loop {
            let candidate = if ordinal == 1 {
                form.to_owned()
            } else {
                format!("{form}_{ordinal}")
            };
            if let Ok(name) = Name::new(&candidate)
                && !self.state.spec.layers.contains_key(&name)
            {
                return name;
            }
            ordinal += 1;
        }
    }

    fn visibility(&mut self, layer: &Name, visible: bool) -> Result<(), CommandError> {
        let id = self.layer(layer)?.id;
        self.transaction.set_visible(id, visible);
        Ok(())
    }

    fn color(
        &mut self,
        color: &ColorValue,
        target: &Target,
        structure: Option<&Name>,
    ) -> Result<(), CommandError> {
        let query = match target {
            Target::Layer(layer) => {
                let layer = self.layer(layer)?;
                let (id, structure) = (layer.id, layer.structure);
                let color = self.color_spec(color, structure)?;
                return self
                    .transaction
                    .set_color(id, color)
                    .map_err(|error| scene_error(&error));
            }
            Target::Query(query) => query,
        };
        if matches!(color, ColorValue::Property { .. }) {
            return Err(CommandError::new(
                ErrorKind::InvalidColor,
                "a property colour applies to a whole layer: color property NAME, @LAYER",
            ));
        }
        let structure = self.structure(structure)?;
        let selection = self.resolver().selection(query)?;
        let replaced: Vec<_> = self
            .state
            .spec
            .rules
            .iter()
            .filter(|(_, rule)| rule.structure == structure && rule.target == *query)
            .map(|(id, _)| *id)
            .collect();
        for id in replaced {
            self.transaction
                .remove_appearance_rule(id)
                .map_err(|error| scene_error(&error))?;
            let _ = self.state.spec.rules.remove(&id);
        }
        let spec = self.color_spec(color, structure)?;
        let id = self
            .transaction
            .add_appearance_rule(AppearanceRuleSpec::new(structure, selection, spec))
            .map_err(|error| scene_error(&error))?;
        let _ = self.state.spec.rules.insert(
            id,
            RuleSpec {
                structure,
                target: query.clone(),
                color: color.clone(),
            },
        );
        Ok(())
    }

    fn uncolor(
        &mut self,
        target: Option<&QueryText>,
        structure: Option<&Name>,
    ) -> Result<(), CommandError> {
        let structure = self.structure(structure)?;
        let removed: Vec<_> = self
            .state
            .spec
            .rules
            .iter()
            .filter(|(_, rule)| {
                rule.structure == structure && target.is_none_or(|target| rule.target == *target)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in &removed {
            self.transaction
                .remove_appearance_rule(*id)
                .map_err(|error| scene_error(&error))?;
            let _ = self.state.spec.rules.remove(id);
        }
        if removed.is_empty() {
            self.messages.push("no colour rule matched".to_owned());
        }
        Ok(())
    }

    fn declared(&self, target: &Target) -> Result<QueryText, CommandError> {
        match target {
            Target::Layer(layer) => Ok(self.layer(layer)?.target.clone()),
            Target::Query(query) => Ok(query.clone()),
        }
    }

    fn layer(&self, name: &Name) -> Result<&LayerSpec, CommandError> {
        self.state
            .spec
            .layers
            .get(name)
            .ok_or_else(|| self.resolver().unknown_layer(name.as_str()))
    }

    fn structure(&self, name: Option<&Name>) -> Result<StructureId, CommandError> {
        let structures = &self.state.spec.structures;
        if let Some(name) = name {
            return structures.get(name).copied().ok_or_else(|| {
                CommandError::new(
                    ErrorKind::UnknownSymbol,
                    format!("there is no structure named '{name}'"),
                )
                .suggest(registry::suggest(
                    name.as_str(),
                    structures.keys().map(Name::as_str),
                ))
            });
        }
        let mut ids = structures.values();
        match (ids.next(), ids.next()) {
            (Some(id), None) => Ok(*id),
            (None, _) => Err(CommandError::new(
                ErrorKind::Scene,
                "the scene has no structure to draw",
            )),
            (Some(_), Some(_)) => Err(CommandError::new(
                ErrorKind::AmbiguousStructure,
                format!(
                    "the scene has several structures ({}); name one with 'in NAME'",
                    structures
                        .keys()
                        .map(Name::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }

    fn color_spec(
        &self,
        color: &ColorValue,
        structure: StructureId,
    ) -> Result<molgfx_api::ColorSpec, CommandError> {
        if let Some(spec) = color.scheme() {
            return Ok(spec);
        }
        super::property::color(self.transaction.spec(), color, structure)
    }
}

pub(crate) fn scene_error(error: &molgfx_api::Error) -> CommandError {
    CommandError::new(ErrorKind::Scene, error.to_string())
}
