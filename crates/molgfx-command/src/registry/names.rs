//! Name iterators for completion.

use super::VERBS;
use crate::ir::FormKind;

/// Every verb name.
pub fn verb_names() -> impl Iterator<Item = &'static str> {
    VERBS.iter().map(|verb| verb.name)
}

/// Every form name.
pub fn form_names() -> impl Iterator<Item = &'static str> {
    FormKind::ALL.iter().map(|kind| kind.name())
}
