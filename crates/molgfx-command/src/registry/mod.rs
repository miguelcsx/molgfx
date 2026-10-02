//! The command vocabulary, listed for completion, help and suggestions.
//!
//! Everything the parser accepts by name is declared here once: the verbs, the
//! forms and their controls (through [`FormKind`](crate::ir::FormKind)), the colour schemes, the
//! named colours and the property ramps. A user interface lists exactly what
//! the parser accepts, and a misspelling is matched against the same table.

mod suggest;

pub use suggest::suggest;

mod catalogues;
mod colors;
mod names;
mod verbs;

pub use catalogues::{palettes, ramps, schemes};
pub use colors::{NAMED_COLORS, color_words, named_color};
pub use names::{form_names, verb_names};
pub use verbs::{VERBS, VerbInfo};
