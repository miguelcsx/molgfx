//! Explicit ownership vocabulary for Python buffer contracts.

use pyo3::prelude::*;

/// Ownership applied by a Python-facing memory route.
#[pyclass(name = "MemoryOwnership", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyMemoryOwnership {
    /// Rust and the provider retain shared ownership.
    Shared,
    /// Ownership moves to the returned Python object exactly once.
    Transferred,
    /// The route allocates a distinct destination buffer.
    Copied,
}

/// Typed reason why a zero-copy transfer is unavailable.
#[pyclass(name = "MemoryTransferExclusion", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyMemoryTransferExclusion {
    /// The Rust facade exposes only borrowed storage, so it cannot be moved.
    RustStorageNotMovable,
    /// No safe `DLPack` lifetime and deleter contract is available.
    DlpackLifetimeUnavailable,
}
