//! Incrementally maintained deterministic pressure indices.

use super::machine::{Entry, ResidencyMachine};
use super::pressure::PressureKey;
use super::types::{ResidencyClass, ResidencyKey, ResidencyPhase};

impl ResidencyMachine {
    pub(super) fn index_insert(&mut self, key: ResidencyKey, entry: Entry) {
        let pressure = PressureKey::new(key, entry);
        match (entry.phase, entry.class) {
            (ResidencyPhase::Resident, ResidencyClass::Hot) => {
                self.resident_hot.insert(pressure);
            }
            (ResidencyPhase::Resident, ResidencyClass::Warm) => {
                self.resident_warm.insert(pressure);
            }
            (ResidencyPhase::Requested, _) => {
                self.requested.insert(pressure);
            }
            (ResidencyPhase::ReadyCpu, _) => {
                self.ready_cpu.insert(pressure);
            }
            (ResidencyPhase::Uploading, _) => {
                self.uploading.insert(pressure);
            }
            (ResidencyPhase::Absent, _) => {}
        }
    }

    pub(super) fn index_remove(&mut self, key: ResidencyKey, entry: Entry) {
        let pressure = PressureKey::new(key, entry);
        match (entry.phase, entry.class) {
            (ResidencyPhase::Resident, ResidencyClass::Hot) => {
                self.resident_hot.remove(&pressure);
            }
            (ResidencyPhase::Resident, ResidencyClass::Warm) => {
                self.resident_warm.remove(&pressure);
            }
            (ResidencyPhase::Requested, _) => {
                self.requested.remove(&pressure);
            }
            (ResidencyPhase::ReadyCpu, _) => {
                self.ready_cpu.remove(&pressure);
            }
            (ResidencyPhase::Uploading, _) => {
                self.uploading.remove(&pressure);
            }
            (ResidencyPhase::Absent, _) => {}
        }
    }

    pub(super) fn rebuild_pressure_indices(&mut self) {
        self.resident_hot.clear();
        self.resident_warm.clear();
        self.requested.clear();
        self.ready_cpu.clear();
        self.uploading.clear();
        let entries = self
            .entries
            .iter()
            .map(|(key, entry)| (*key, *entry))
            .collect::<Vec<_>>();
        for (key, entry) in entries {
            self.index_insert(key, entry);
        }
    }
}
