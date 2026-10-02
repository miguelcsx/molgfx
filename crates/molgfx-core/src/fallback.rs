//! Explicit defaults for values that may be absent.
//!
//! The workspace forbids the implicit-default family of `Option` methods so that
//! every default is a decision made where the value is read. This helper states that decision in one place: the
//! last value `candidate` yields, or `fallback` when it yields none.

/// The last value of `candidate`, or `fallback` when it is empty.
#[inline]
pub fn fallback<T>(candidate: impl IntoIterator<Item = T>, fallback: T) -> T {
    candidate.into_iter().fold(fallback, |_, value| value)
}

#[cfg(test)]
#[path = "fallback_tests.rs"]
mod tests;
