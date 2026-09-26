//! A live authoring session over one scene.
//!
//! The session owns names — structures, named selections and layers — plus
//! the colour rules and focus it created and the history of what it did. It
//! does not own the scene: every call borrows the scene it edits, so a
//! notebook, a viewer and the session all share one scene and one revision
//! stream. When the scene is edited elsewhere the session notices the moved
//! revision, forgets whatever vanished, and drops its history, because its
//! inverses were computed against a scene that no longer exists.

mod complete;
mod explain;
mod history;
mod plan;
mod property;
mod resolve;
mod state;
#[cfg(test)]
mod tests;

pub use complete::Completion;
pub use state::{LayerSpec, RuleSpec, SessionSpec};

use crate::error::{CommandError, CommandErrors, ErrorKind};
use crate::ir::{Command, Program};
use history::{Entry, History};
use molgfx_api::{Scene, ScenePatch};
use plan::{Planner, scene_error};
use serde::Serialize;
use state::State;

/// What executing a program did.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Outcome {
    /// The patch applied to the scene, for a viewer to apply in turn; `None`
    /// when only the session's names changed.
    pub patch: Option<ScenePatch>,
    /// The scene revision afterwards.
    pub revision: u64,
    /// Notes for the author, such as a reused layer.
    pub messages: Vec<String>,
}

/// Names, rules and history for authoring one scene.
#[derive(Clone, Debug, Default)]
pub struct Session {
    state: State,
    history: History,
    /// The scene revision after this session's last edit.
    revision: Option<u64>,
}

impl Session {
    /// A session with no names but the scene's structures.
    #[must_use]
    pub fn new(scene: &Scene) -> Self {
        let mut session = Self::default();
        session.state.sync(scene.spec());
        session.revision = Some(scene.revision());
        session
    }

    /// Restores a session's names over `scene`.
    ///
    /// Layers and rules whose scene objects are missing are dropped.
    #[must_use]
    pub fn from_spec(spec: SessionSpec, scene: &Scene) -> Self {
        let mut session = Self {
            state: State::from_spec(spec),
            ..Self::default()
        };
        session.state.sync(scene.spec());
        session.revision = Some(scene.revision());
        session
    }

    /// The session's portable state.
    #[must_use]
    pub const fn spec(&self) -> &SessionSpec {
        &self.state.spec
    }

    /// Parses and executes command text as one atomic program.
    ///
    /// # Errors
    ///
    /// Returns every parse error, or the first planning or scene error; the
    /// scene and the session are unchanged in either case.
    pub fn execute_text(
        &mut self,
        scene: &mut Scene,
        source: &str,
    ) -> Result<Outcome, CommandErrors> {
        let program = Program::parse(source)?;
        self.execute(scene, &program)
    }

    /// Executes a program atomically: every statement or none.
    ///
    /// A program of only `undo` and `redo` steps through history; history
    /// commands cannot be mixed with edits.
    ///
    /// # Errors
    ///
    /// Returns the first failing statement's error, located in the program's
    /// text when it has one; the scene and the session are unchanged.
    pub fn execute(
        &mut self,
        scene: &mut Scene,
        program: &Program,
    ) -> Result<Outcome, CommandErrors> {
        let stale = self.sync(scene);
        let statements = program.statements();
        if !statements.is_empty()
            && statements
                .iter()
                .all(|statement| statement.command.is_history())
        {
            if stale {
                return Err(CommandError::new(
                    ErrorKind::StaleSession,
                    "the scene was changed outside this session, so its history was cleared",
                )
                .into());
            }
            return self.travel(scene, program);
        }
        let mut planner = Planner {
            state: self.state.clone(),
            transaction: scene.begin(),
            messages: Vec::new(),
            site: None,
        };
        if stale {
            planner.messages.push(
                "the scene changed outside this session; undo history was cleared".to_owned(),
            );
        }
        for (index, statement) in statements.iter().enumerate() {
            planner.site = program.source().zip(statement.span);
            planner
                .plan(&statement.command)
                .map_err(|error| error.in_statement(index, statement.span))?;
        }
        let Planner {
            state,
            transaction,
            messages,
            ..
        } = planner;
        let base = scene.spec().clone();
        let patch = transaction
            .into_patch()
            .map_err(|error| scene_error(&error))?;
        let (patch, inverse) = if patch.operations.is_empty() {
            (None, Vec::new())
        } else {
            let inverse = patch.inverse(&base).map_err(|error| scene_error(&error))?;
            scene.apply(&patch).map_err(|error| scene_error(&error))?;
            (Some(patch), inverse.operations)
        };
        let before = std::mem::replace(&mut self.state, state).spec;
        if patch.is_some() || before != self.state.spec {
            self.history.record(Entry {
                label: label(program),
                forward: patch
                    .as_ref()
                    .map_or_else(Vec::new, |patch| patch.operations.clone()),
                inverse,
                before,
                after: self.state.spec.clone(),
            });
        }
        self.revision = Some(scene.revision());
        Ok(Outcome {
            patch,
            revision: scene.revision(),
            messages,
        })
    }

    /// Executes one typed command.
    ///
    /// # Errors
    ///
    /// As [`Self::execute`].
    pub fn run(&mut self, scene: &mut Scene, command: Command) -> Result<Outcome, CommandErrors> {
        self.execute(scene, &Program::from(command))
    }

    /// Labels of the programs undo would reverse, oldest first.
    #[must_use]
    pub fn history(&self) -> Vec<String> {
        self.history.labels()
    }

    /// Whether there is an edit to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Whether there is an undone edit to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Brings the session in line with the scene; true when the scene moved
    /// since this session last edited it.
    fn sync(&mut self, scene: &Scene) -> bool {
        let stale = self
            .revision
            .is_some_and(|revision| revision != scene.revision());
        if stale {
            self.history.clear();
        }
        self.state.sync(scene.spec());
        self.revision = Some(scene.revision());
        stale
    }

    /// Steps through history. Every step's operations are applied as one
    /// patch, so several steps are still one scene revision.
    fn travel(&mut self, scene: &mut Scene, program: &Program) -> Result<Outcome, CommandErrors> {
        let mut steps: Vec<(bool, Entry)> = Vec::new();
        for (index, statement) in program.statements().iter().enumerate() {
            let undo = matches!(statement.command, Command::Undo);
            let entry = if undo {
                self.history.take_undo()
            } else {
                self.history.take_redo()
            };
            let Some(entry) = entry else {
                self.restore(steps);
                let message = if undo {
                    "nothing to undo"
                } else {
                    "nothing to redo"
                };
                return Err(CommandError::new(ErrorKind::History, message)
                    .in_statement(index, statement.span)
                    .into());
            };
            if undo {
                self.history.undone(entry.clone());
            } else {
                self.history.redone(entry.clone());
            }
            steps.push((undo, entry));
        }
        let operations: Vec<_> = steps
            .iter()
            .flat_map(|(undo, entry)| {
                if *undo {
                    entry.inverse.iter().cloned()
                } else {
                    entry.forward.iter().cloned()
                }
            })
            .collect();
        let patch = if operations.is_empty() {
            None
        } else {
            let patch = ScenePatch {
                base_revision: scene.revision(),
                operations,
            };
            if let Err(error) = scene.apply(&patch) {
                self.restore(steps);
                return Err(scene_error(&error).into());
            }
            Some(patch)
        };
        let mut messages = Vec::new();
        if let Some((undo, entry)) = steps.last() {
            let spec = if *undo { &entry.before } else { &entry.after };
            self.state = State::from_spec(spec.clone());
        }
        for (undo, entry) in &steps {
            messages.push(format!(
                "{} {}",
                if *undo { "undid" } else { "redid" },
                entry.label
            ));
        }
        self.state.sync(scene.spec());
        self.revision = Some(scene.revision());
        Ok(Outcome {
            patch,
            revision: scene.revision(),
            messages,
        })
    }

    /// Puts steps taken by a failed history program back where they were.
    fn restore(&mut self, steps: Vec<(bool, Entry)>) {
        for (undo, _) in steps.into_iter().rev() {
            if undo {
                if let Some(entry) = self.history.take_redo() {
                    self.history.redone(entry);
                }
            } else if let Some(entry) = self.history.take_undo() {
                self.history.undone(entry);
            }
        }
    }
}

fn label(program: &Program) -> String {
    match program.source() {
        Some(source) => source.trim().to_owned(),
        None => program
            .statements()
            .iter()
            .map(|statement| statement.command.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    }
}
