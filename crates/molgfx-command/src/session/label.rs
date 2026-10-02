//! The history label a program gets.

use crate::ir::Program;

pub(super) fn label(program: &Program) -> String {
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
