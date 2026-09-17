//! Deterministic pressure ordering and hard-budget reconciliation.

use super::machine::{Entry, ResidencyMachine};
use super::types::{FailureReason, ResidencyClass, ResidencyKey, ResidencyOutput, ResidencyPhase};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) struct PressureKey {
    class: u8,
    detail: u8,
    priority: i32,
    key: ResidencyKey,
}

impl PressureKey {
    pub(super) const fn new(key: ResidencyKey, entry: Entry) -> Self {
        let class = match entry.class {
            ResidencyClass::Warm => 0,
            ResidencyClass::Hot => 1,
        };
        let detail = match key.detail {
            super::types::ResidencyDetail::Atom => 0,
            super::types::ResidencyDetail::Residue => 1,
            super::types::ResidencyDetail::SecondaryStructure => 2,
            super::types::ResidencyDetail::Domain => 3,
        };
        Self {
            class,
            detail,
            priority: entry.priority,
            key,
        }
    }

    pub(super) const fn residency_key(self) -> ResidencyKey {
        self.key
    }
}

impl ResidencyMachine {
    pub(super) fn evict_for_cpu(
        &mut self,
        added: u64,
        protected: Option<ResidencyKey>,
        output: &mut ResidencyOutput,
    ) {
        while self.usage.cpu.saturating_add(added) > self.budget.cpu {
            let Some(key) = self.next_resident(protected, None) else {
                break;
            };
            self.evict(key, output);
        }
    }

    pub(super) fn evict_for_gpu(
        &mut self,
        class: ResidencyClass,
        added: u64,
        protected: Option<ResidencyKey>,
        output: &mut ResidencyOutput,
    ) {
        while !self.gpu_fits(class, added) {
            let Some(key) = self.next_resident(protected, Some(class)) else {
                break;
            };
            self.evict(key, output);
        }
    }

    pub(super) fn reconcile_resident(&mut self, output: &mut ResidencyOutput) {
        while self.usage.gpu_warm > self.budget.gpu_warm
            || self.usage.gpu_hot > self.budget.gpu_hot
            || self.usage.cpu > self.budget.cpu
        {
            let class = if self.usage.gpu_warm > self.budget.gpu_warm {
                Some(ResidencyClass::Warm)
            } else if self.usage.gpu_hot > self.budget.gpu_hot {
                Some(ResidencyClass::Hot)
            } else {
                None
            };
            let Some(key) = self.next_resident(None, class) else {
                break;
            };
            self.evict(key, output);
        }
        self.cancel_active_pressure(output);
    }

    fn cancel_active_pressure(&mut self, output: &mut ResidencyOutput) {
        while self.usage.in_flight > self.budget.in_flight {
            let Some(key) = self.next_active(ResidencyPhase::Requested, None) else {
                break;
            };
            self.discard_for_budget(key, output);
        }
        while self.usage.staging > self.budget.staging {
            let Some(key) = self.next_active(ResidencyPhase::Uploading, None) else {
                break;
            };
            self.discard_for_budget(key, output);
        }
        while self.usage.cpu > self.budget.cpu {
            let Some(key) =
                self.next_active(ResidencyPhase::ReadyCpu, Some(ResidencyPhase::Uploading))
            else {
                break;
            };
            self.discard_for_budget(key, output);
        }
    }

    fn next_resident(
        &mut self,
        protected: Option<ResidencyKey>,
        required_class: Option<ResidencyClass>,
    ) -> Option<ResidencyKey> {
        match required_class {
            Some(ResidencyClass::Warm) => first_except(&self.resident_warm, protected),
            Some(ResidencyClass::Hot) => first_except(&self.resident_hot, protected),
            None => first_except(&self.resident_warm, protected)
                .or_else(|| first_except(&self.resident_hot, protected)),
        }
    }

    fn next_active(
        &mut self,
        first: ResidencyPhase,
        second: Option<ResidencyPhase>,
    ) -> Option<ResidencyKey> {
        let first = first_in(self.phase_index(first));
        let second = second.and_then(|phase| first_in(self.phase_index(phase)));
        match (first, second) {
            (Some(left), Some(right)) => Some(left.min(right).residency_key()),
            (Some(value), None) | (None, Some(value)) => Some(value.residency_key()),
            (None, None) => None,
        }
    }

    fn discard_for_budget(&mut self, key: ResidencyKey, output: &mut ResidencyOutput) {
        let Some(entry) = self.entries.get(&key).copied() else {
            return;
        };
        self.release(key, entry, output);
        self.set_absent(key, Some(FailureReason::BudgetExceeded));
    }

    fn evict(&mut self, key: ResidencyKey, output: &mut ResidencyOutput) {
        let Some(entry) = self.entries.get(&key).copied() else {
            return;
        };
        self.release(key, entry, output);
        self.set_absent(key, None);
    }

    pub(super) fn gpu_fits(&self, class: ResidencyClass, added: u64) -> bool {
        match class {
            ResidencyClass::Hot => self.usage.gpu_hot.saturating_add(added) <= self.budget.gpu_hot,
            ResidencyClass::Warm => {
                self.usage.gpu_warm.saturating_add(added) <= self.budget.gpu_warm
            }
        }
    }

    fn phase_index(&self, phase: ResidencyPhase) -> &BTreeSet<PressureKey> {
        match phase {
            ResidencyPhase::Requested | ResidencyPhase::Absent => &self.requested,
            ResidencyPhase::ReadyCpu => &self.ready_cpu,
            ResidencyPhase::Uploading => &self.uploading,
            ResidencyPhase::Resident => &self.resident_warm,
        }
    }

    #[cfg(test)]
    pub(super) fn pressure_index_len(&self) -> usize {
        self.resident_hot.len()
            + self.resident_warm.len()
            + self.requested.len()
            + self.ready_cpu.len()
            + self.uploading.len()
    }
}

fn first_in(index: &BTreeSet<PressureKey>) -> Option<PressureKey> {
    index.first().copied()
}

fn first_except(
    index: &BTreeSet<PressureKey>,
    protected: Option<ResidencyKey>,
) -> Option<ResidencyKey> {
    index
        .iter()
        .find(|value| Some(value.residency_key()) != protected)
        .copied()
        .map(PressureKey::residency_key)
}
