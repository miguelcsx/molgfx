//! Named portable snapshot commands.

use super::arguments::{Arguments, name_word};
use super::statement::{no_target, syntax};
use crate::error::{CommandError, Span};
use crate::ir::Command;

pub(super) fn parse(
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    no_target(tail, "snapshot")?;
    let [action, name] = arguments.words() else {
        return Err(syntax(
            "snapshot save|restore|remove NAME",
            arguments.anchor(),
        ));
    };
    let name = name_word(*name)?;
    match action.text {
        "save" => Ok(Command::SnapshotSave { name }),
        "restore" => Ok(Command::SnapshotRestore { name }),
        "remove" => Ok(Command::SnapshotRemove { name }),
        _ => Err(syntax(
            "snapshot action must be save, restore or remove",
            action.span,
        )),
    }
}
