//! A default for an absent value, without the panicking family of accessors.

/// `candidate`, or `default` when it is absent.
#[inline]
pub fn fallback<T>(candidate: Option<T>, default: T) -> T {
    candidate.into_iter().fold(default, |_, value| value)
}
