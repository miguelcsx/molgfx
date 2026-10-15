//! A panicking lock for the mock log, so a test reads a guard in one line.

use std::sync::{Mutex, MutexGuard};

/// Locks a mutex, treating poisoning as a test failure.
pub(crate) trait Locked<T> {
    /// Returns the guard, panicking if another test thread poisoned the lock.
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(error) => panic!("{error}"),
        }
    }
}
