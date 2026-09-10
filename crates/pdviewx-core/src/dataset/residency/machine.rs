//! Core residency state transitions and deterministic pressure handling.
//!
//! Entry lookup, transitions and pressure selection are `O(log chunks)`.

use super::pressure::PressureKey;
use super::types::{
    ChunkFootprint, DeviceLossReport, Eviction, FailureReason, ResidencyBudget, ResidencyClass,
    ResidencyError, ResidencyKey, ResidencyOutput, ResidencyPhase, ResidencyRequest,
    ResidencySnapshot, ResidencyTicket, StaleCompletion, Usage,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "machine_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) generation: u64,
    pub(super) phase: ResidencyPhase,
    pub(super) footprint: ChunkFootprint,
    pub(super) class: ResidencyClass,
    pub(super) priority: i32,
    pub(super) failure: Option<FailureReason>,
}

impl Entry {
    const fn snapshot(self) -> ResidencySnapshot {
        ResidencySnapshot {
            phase: self.phase,
            generation: self.generation,
            failure: self.failure,
        }
    }
}

/// Stateful out-of-core coordinator with incrementally maintained pressure indices.
#[derive(Debug)]
pub struct ResidencyMachine {
    pub(super) budget: ResidencyBudget,
    pub(super) usage: Usage,
    pub(super) entries: BTreeMap<ResidencyKey, Entry>,
    pub(super) resident_hot: BTreeSet<PressureKey>,
    pub(super) resident_warm: BTreeSet<PressureKey>,
    pub(super) requested: BTreeSet<PressureKey>,
    pub(super) ready_cpu: BTreeSet<PressureKey>,
    pub(super) uploading: BTreeSet<PressureKey>,
}

impl ResidencyMachine {
    /// Creates an empty machine with hard independent tier budgets.
    #[must_use]
    pub fn new(budget: ResidencyBudget) -> Self {
        Self {
            budget,
            usage: Usage::default(),
            entries: BTreeMap::new(),
            resident_hot: BTreeSet::new(),
            resident_warm: BTreeSet::new(),
            requested: BTreeSet::new(),
            ready_cpu: BTreeSet::new(),
            uploading: BTreeSet::new(),
        }
    }

    /// Current hard limits.
    #[must_use]
    pub const fn budget(&self) -> ResidencyBudget {
        self.budget
    }

    /// Current charged bytes.
    #[must_use]
    pub const fn usage(&self) -> Usage {
        self.usage
    }

    /// Returns the state of a known key. Unknown keys are absent at generation zero.
    #[must_use]
    pub fn snapshot(&self, key: ResidencyKey) -> ResidencySnapshot {
        match self.entries.get(&key).copied() {
            Some(entry) => entry.snapshot(),
            None => ResidencySnapshot {
                phase: ResidencyPhase::Absent,
                generation: 0,
                failure: None,
            },
        }
    }

    pub(super) fn footprint(&self, key: ResidencyKey) -> Option<ChunkFootprint> {
        self.entries.get(&key).map(|entry| entry.footprint)
    }

    /// Starts a new generation and emits a provider request.
    ///
    /// Existing asynchronous work is cancelled. Resident data is evicted before
    /// the replacement request is emitted.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the request exceeds the in-flight budget or the
    /// key has exhausted its generation counter.
    pub fn request_into(
        &mut self,
        request: ResidencyRequest,
        output: &mut ResidencyOutput,
    ) -> Result<ResidencyTicket, ResidencyError> {
        output.clear();
        let previous = self.entries.get(&request.key).copied();
        let replaced_bytes = match previous {
            Some(entry) if entry.phase == ResidencyPhase::Requested => entry.footprint.source_bytes,
            _ => 0,
        };
        let projected = self
            .usage
            .in_flight
            .saturating_sub(replaced_bytes)
            .saturating_add(request.footprint.source_bytes);
        if request.footprint.source_bytes > self.budget.in_flight
            || projected > self.budget.in_flight
        {
            return Err(ResidencyError::BudgetExceeded { tier: "in-flight" });
        }
        let generation = match previous {
            Some(entry) => match entry.generation.checked_add(1) {
                Some(value) => value,
                None => return Err(ResidencyError::GenerationExhausted),
            },
            None => 1,
        };
        if let Some(entry) = previous {
            self.release(request.key, entry, output);
            self.index_remove(request.key, entry);
        }
        let ticket = ResidencyTicket::issue(request.key, generation);
        let entry = Entry {
            generation,
            phase: ResidencyPhase::Requested,
            footprint: request.footprint,
            class: request.class,
            priority: request.priority,
            failure: None,
        };
        self.entries.insert(request.key, entry);
        self.index_insert(request.key, entry);
        self.usage.in_flight = self
            .usage
            .in_flight
            .saturating_add(request.footprint.source_bytes);
        output.requests.push(ticket);
        Ok(ticket)
    }

    /// Accepts a decoded payload from the provider.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a current ticket in the wrong phase or when
    /// neither resident eviction nor the CPU budget can admit the payload.
    pub fn ready_cpu_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<bool, ResidencyError> {
        output.clear();
        let Some(entry) = self.current(ticket, output) else {
            return Ok(false);
        };
        if entry.phase != ResidencyPhase::Requested {
            return Err(Self::invalid(ResidencyPhase::Requested, entry.phase));
        }
        self.usage.in_flight = self
            .usage
            .in_flight
            .saturating_sub(entry.footprint.source_bytes);
        self.evict_for_cpu(entry.footprint.host_bytes, Some(ticket.key), output);
        if self.usage.cpu.saturating_add(entry.footprint.host_bytes) > self.budget.cpu {
            self.set_absent(ticket.key, Some(FailureReason::BudgetExceeded));
            return Err(ResidencyError::BudgetExceeded { tier: "cpu" });
        }
        self.usage.cpu = self.usage.cpu.saturating_add(entry.footprint.host_bytes);
        self.set_phase(ticket.key, ResidencyPhase::ReadyCpu);
        Ok(true)
    }

    /// Reserves staging for an upload.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a current ticket in the wrong phase or when
    /// the staging budget cannot admit the upload.
    pub fn begin_upload_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<bool, ResidencyError> {
        output.clear();
        let Some(entry) = self.current(ticket, output) else {
            return Ok(false);
        };
        if entry.phase != ResidencyPhase::ReadyCpu {
            return Err(Self::invalid(ResidencyPhase::ReadyCpu, entry.phase));
        }
        if self
            .usage
            .staging
            .saturating_add(entry.footprint.staging_bytes)
            > self.budget.staging
        {
            return Err(ResidencyError::BudgetExceeded { tier: "staging" });
        }
        self.usage.staging = self
            .usage
            .staging
            .saturating_add(entry.footprint.staging_bytes);
        self.set_phase(ticket.key, ResidencyPhase::Uploading);
        Ok(true)
    }

    /// Releases staging after backpressure while preserving the CPU payload.
    ///
    /// # Errors
    ///
    /// Rejects stale tickets and phases other than uploading.
    pub fn defer_upload_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<bool, ResidencyError> {
        output.clear();
        let Some(entry) = self.current(ticket, output) else {
            return Ok(false);
        };
        if entry.phase != ResidencyPhase::Uploading {
            return Err(Self::invalid(ResidencyPhase::Uploading, entry.phase));
        }
        self.usage.staging = self
            .usage
            .staging
            .saturating_sub(entry.footprint.staging_bytes);
        self.set_phase(ticket.key, ResidencyPhase::ReadyCpu);
        Ok(true)
    }

    /// Commits an uploaded allocation to its requested device tier.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a current ticket in the wrong phase or when
    /// deterministic eviction cannot free enough device memory.
    pub fn complete_upload_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<bool, ResidencyError> {
        output.clear();
        let Some(entry) = self.current(ticket, output) else {
            return Ok(false);
        };
        if entry.phase != ResidencyPhase::Uploading {
            return Err(Self::invalid(ResidencyPhase::Uploading, entry.phase));
        }
        self.usage.staging = self
            .usage
            .staging
            .saturating_sub(entry.footprint.staging_bytes);
        self.evict_for_gpu(
            entry.class,
            entry.footprint.gpu_bytes,
            Some(ticket.key),
            output,
        );
        if !self.gpu_fits(entry.class, entry.footprint.gpu_bytes) {
            self.set_phase(ticket.key, ResidencyPhase::ReadyCpu);
            return Err(ResidencyError::BudgetExceeded {
                tier: Self::gpu_tier(entry.class),
            });
        }
        self.add_gpu(entry.class, entry.footprint.gpu_bytes);
        self.set_phase(ticket.key, ResidencyPhase::Resident);
        Ok(true)
    }

    /// Cancels current provider or upload work. Stale tickets are ignored.
    pub fn cancel_into(&mut self, ticket: ResidencyTicket, output: &mut ResidencyOutput) -> bool {
        output.clear();
        let Some(entry) = self.current(ticket, output) else {
            return false;
        };
        self.release(ticket.key, entry, output);
        self.set_absent(ticket.key, None);
        true
    }

    /// Records a provider or validation failure and releases active resources.
    pub fn fail_into(
        &mut self,
        ticket: ResidencyTicket,
        reason: FailureReason,
        output: &mut ResidencyOutput,
    ) -> bool {
        output.clear();
        let Some(entry) = self.current(ticket, output) else {
            return false;
        };
        self.release(ticket.key, entry, output);
        self.set_absent(ticket.key, Some(reason));
        true
    }

    /// Updates hard budgets and evicts deterministically until resident tiers fit.
    pub fn set_budget_into(&mut self, budget: ResidencyBudget, output: &mut ResidencyOutput) {
        output.clear();
        self.budget = budget;
        self.reconcile_resident(output);
    }

    /// Invalidates all device allocations while preserving CPU-ready payloads.
    ///
    /// # Errors
    ///
    /// Returns a typed error without changing state if an uploading ticket can
    /// no longer advance to a fresh generation.
    pub fn device_lost_into(
        &mut self,
        output: &mut ResidencyOutput,
    ) -> Result<DeviceLossReport, ResidencyError> {
        output.clear();
        if self
            .entries
            .values()
            .any(|entry| entry.phase == ResidencyPhase::Uploading && entry.generation == u64::MAX)
        {
            return Err(ResidencyError::GenerationExhausted);
        }
        let mut report = DeviceLossReport::default();
        for (key, entry) in &mut self.entries {
            if matches!(
                entry.phase,
                ResidencyPhase::Uploading | ResidencyPhase::Resident
            ) {
                if entry.phase == ResidencyPhase::Resident {
                    output.evictions.push(Eviction {
                        key: *key,
                        generation: entry.generation,
                    });
                    report.invalidated = report.invalidated.saturating_add(1);
                } else {
                    output
                        .cancellations
                        .push(ResidencyTicket::issue(*key, entry.generation));
                    entry.generation += 1;
                }
                entry.phase = ResidencyPhase::ReadyCpu;
                output
                    .ready_uploads
                    .push(ResidencyTicket::issue(*key, entry.generation));
                report.ready_cpu = report.ready_cpu.saturating_add(1);
            }
        }
        self.usage.staging = 0;
        self.usage.gpu_hot = 0;
        self.usage.gpu_warm = 0;
        self.rebuild_pressure_indices();
        Ok(report)
    }

    fn current(&self, ticket: ResidencyTicket, output: &mut ResidencyOutput) -> Option<Entry> {
        let current = self.entries.get(&ticket.key).copied();
        let (generation, phase) = match current {
            Some(entry) => (entry.generation, entry.phase),
            None => (0, ResidencyPhase::Absent),
        };
        if generation != ticket.generation() || phase == ResidencyPhase::Absent {
            output.stale.push(StaleCompletion {
                ticket,
                current_generation: generation,
                current_phase: phase,
            });
            return None;
        }
        current
    }

    fn set_phase(&mut self, key: ResidencyKey, phase: ResidencyPhase) {
        let Some(previous) = self.entries.get(&key).copied() else {
            return;
        };
        self.index_remove(key, previous);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.phase = phase;
            entry.failure = None;
        }
        if let Some(entry) = self.entries.get(&key).copied() {
            self.index_insert(key, entry);
        }
    }

    pub(super) fn set_absent(&mut self, key: ResidencyKey, failure: Option<FailureReason>) {
        let Some(previous) = self.entries.get(&key).copied() else {
            return;
        };
        self.index_remove(key, previous);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.phase = ResidencyPhase::Absent;
            entry.failure = failure;
        }
    }

    pub(super) fn release(
        &mut self,
        key: ResidencyKey,
        entry: Entry,
        output: &mut ResidencyOutput,
    ) {
        if matches!(
            entry.phase,
            ResidencyPhase::Requested | ResidencyPhase::Uploading
        ) {
            output
                .cancellations
                .push(ResidencyTicket::issue(key, entry.generation));
        }
        if entry.phase == ResidencyPhase::Resident {
            output.evictions.push(Eviction {
                key,
                generation: entry.generation,
            });
        }
        self.release_accounting(entry);
    }

    fn release_accounting(&mut self, entry: Entry) {
        match entry.phase {
            ResidencyPhase::Absent => {}
            ResidencyPhase::Requested => {
                self.usage.in_flight = self
                    .usage
                    .in_flight
                    .saturating_sub(entry.footprint.source_bytes);
            }
            ResidencyPhase::ReadyCpu => {
                self.usage.cpu = self.usage.cpu.saturating_sub(entry.footprint.host_bytes);
            }
            ResidencyPhase::Uploading => {
                self.usage.cpu = self.usage.cpu.saturating_sub(entry.footprint.host_bytes);
                self.usage.staging = self
                    .usage
                    .staging
                    .saturating_sub(entry.footprint.staging_bytes);
            }
            ResidencyPhase::Resident => {
                self.usage.cpu = self.usage.cpu.saturating_sub(entry.footprint.host_bytes);
                self.sub_gpu(entry.class, entry.footprint.gpu_bytes);
            }
        }
    }

    const fn invalid(required: ResidencyPhase, actual: ResidencyPhase) -> ResidencyError {
        ResidencyError::InvalidPhase { required, actual }
    }

    const fn gpu_tier(class: ResidencyClass) -> &'static str {
        match class {
            ResidencyClass::Hot => "gpu-hot",
            ResidencyClass::Warm => "gpu-warm",
        }
    }

    fn add_gpu(&mut self, class: ResidencyClass, bytes: u64) {
        match class {
            ResidencyClass::Hot => self.usage.gpu_hot = self.usage.gpu_hot.saturating_add(bytes),
            ResidencyClass::Warm => self.usage.gpu_warm = self.usage.gpu_warm.saturating_add(bytes),
        }
    }

    fn sub_gpu(&mut self, class: ResidencyClass, bytes: u64) {
        match class {
            ResidencyClass::Hot => self.usage.gpu_hot = self.usage.gpu_hot.saturating_sub(bytes),
            ResidencyClass::Warm => self.usage.gpu_warm = self.usage.gpu_warm.saturating_sub(bytes),
        }
    }
}
