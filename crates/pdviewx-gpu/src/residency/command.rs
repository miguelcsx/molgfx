//! Fixed-capacity reusable command storage.

use std::fmt;
use thiserror::Error;

/// Command scratch counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandScratchMetrics {
    /// Commands accepted over the lifetime of the scratch storage.
    pub commands_recorded: u64,
    /// Largest command count in one recording epoch.
    pub peak_commands: usize,
    /// Commands rejected because the fixed capacity was exhausted.
    pub capacity_stalls: u64,
    /// Host storage allocations performed by this primitive.
    pub host_allocation_events: u64,
}

/// A command did not fit in fixed scratch capacity.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("command scratch capacity {capacity} is exhausted")]
pub struct ScratchFull {
    /// Configured command capacity.
    pub capacity: usize,
}

/// Reusable storage for trivially owned command descriptors.
///
/// The caller clears the logical length between recording epochs. Capacity
/// never grows, so `push` is allocation-free after construction.
pub struct CommandScratch<C: Copy> {
    commands: Vec<C>,
    limit: usize,
    metrics: CommandScratchMetrics,
}

impl<C: Copy> CommandScratch<C> {
    /// Allocates storage for at most `capacity` commands.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
            limit: capacity,
            metrics: CommandScratchMetrics {
                host_allocation_events: u64::from(capacity > 0),
                ..CommandScratchMetrics::default()
            },
        }
    }

    /// Starts a new recording epoch while retaining allocated storage.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Appends one command without growing storage.
    ///
    /// # Errors
    ///
    /// Returns [`ScratchFull`] when fixed capacity is exhausted.
    pub fn push(&mut self, command: C) -> Result<(), ScratchFull> {
        if self.commands.len() == self.limit {
            self.metrics.capacity_stalls = self.metrics.capacity_stalls.saturating_add(1);
            return Err(ScratchFull {
                capacity: self.limit,
            });
        }
        self.commands.push(command);
        self.metrics.commands_recorded = self.metrics.commands_recorded.saturating_add(1);
        self.metrics.peak_commands = self.metrics.peak_commands.max(self.commands.len());
        Ok(())
    }

    /// Commands recorded in the current epoch.
    #[must_use]
    pub fn as_slice(&self) -> &[C] {
        &self.commands
    }

    /// Mutable command storage for in-place lowering or sorting.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [C] {
        &mut self.commands
    }

    /// Fixed command capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.limit
    }

    /// Current cumulative counters.
    #[must_use]
    pub const fn metrics(&self) -> CommandScratchMetrics {
        self.metrics
    }
}

impl<C: Copy + fmt::Debug> fmt::Debug for CommandScratch<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandScratch")
            .field("commands", &self.commands)
            .field("limit", &self.limit)
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
