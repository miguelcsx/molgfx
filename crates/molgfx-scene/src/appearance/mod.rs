//! Selection-scoped appearance: colour rules over molecular queries.
//!
//! A representation carries one base colour for everything it draws. An
//! appearance rule instead says "atoms matching this query, in this structure,
//! are coloured this way" — independently of which representation draws them.
//! That is what makes `color red, chain A` mean the right thing for a cartoon
//! of the whole protein: only chain A turns red, and the cartoon keeps its own
//! colour everywhere else.
//!
//! Rules are declarative scene state. The scene stores the rule — a query and a
//! colour — never the atoms it matched, so a rule is a few bytes in a patch no
//! matter how many atoms it covers. Where rules overlap, the rule with the
//! higher [`crate::AppearanceRuleId`] wins; identities are allocated in
//! increasing order, so the most recently added rule wins. Removing a rule
//! restores whatever lies underneath it: an older rule, or the representation's
//! own colour.
//!
//! Physically, a structure's rules resolve to one compact class per atom and a
//! small table of colour schemes — at most [`MAX_APPEARANCE_CLASSES`] distinct
//! colourings per structure — shared by every representation of that
//! structure. Resolution evaluates each rule's query once, at edit time; frames
//! only read the class column.

mod lower;
mod rule;
#[cfg(test)]
pub(crate) mod tests;

pub(crate) use lower::{AppearanceClasses, resolve_classes};
pub use rule::{AppearanceRuleSpec, MAX_APPEARANCE_CLASSES};
