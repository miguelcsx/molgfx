//! The authoring command language and session for `MolGFX` scenes.
//!
//! Text such as `select pocket, byres (within 5 of resname HEM); show
//! ball_and_stick, $pocket` is parsed into typed [`Command`]s. Molecular
//! selections are `MolFrame` queries, compiled by `MolFrame` from the exact
//! span the author wrote; this crate never re-tokenizes one. A [`Session`]
//! resolves names — named selections, layers and structures — and plans each
//! program into one atomic scene patch, so an application sends the viewer a
//! small semantic edit rather than a new scene.

#![forbid(unsafe_code)]

mod error;
mod ir;
mod language;
pub mod registry;
mod session;

pub use error::{CommandError, CommandErrors, ErrorKind, Span};
pub use ir::{
    ColorValue, Command, Finite, Form, FormKind, InvalidName, Name, Opacity, OptionError,
    OptionInfo, OptionKind, Positive, Program, QueryText, Show, Statement, Target,
};
pub use session::{Completion, LayerSpec, Outcome, RuleSpec, Session, SessionSpec};
