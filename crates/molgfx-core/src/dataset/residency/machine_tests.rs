use super::*;
use crate::{
    ChunkFootprint, ChunkId, DatasetId, ResidencyClass, ResidencyDetail, ResidencyRequest,
};

fn key(chunk: u64, detail: ResidencyDetail) -> ResidencyKey {
    ResidencyKey {
        dataset: DatasetId::new(7),
        chunk: ChunkId::new(chunk),
        detail,
    }
}

fn request(chunk: u64, detail: ResidencyDetail, class: ResidencyClass) -> ResidencyRequest {
    let priority = match i32::try_from(chunk) {
        Ok(value) => value,
        Err(_) => i32::MAX,
    };
    ResidencyRequest {
        key: key(chunk, detail),
        footprint: ChunkFootprint::new(2, 4, 3, 5),
        class,
        priority,
    }
}

fn budget() -> ResidencyBudget {
    ResidencyBudget {
        cpu: 128,
        staging: 128,
        gpu_hot: 128,
        gpu_warm: 128,
        in_flight: 128,
    }
}

fn issue(
    machine: &mut ResidencyMachine,
    request: ResidencyRequest,
    output: &mut ResidencyOutput,
) -> ResidencyTicket {
    let Ok(ticket) = machine.request_into(request, output) else {
        panic!("request should fit")
    };
    ticket
}

fn make_resident(
    machine: &mut ResidencyMachine,
    request: ResidencyRequest,
    output: &mut ResidencyOutput,
) -> ResidencyTicket {
    let ticket = issue(machine, request, output);
    assert_eq!(machine.ready_cpu_into(ticket, output), Ok(true));
    assert_eq!(machine.begin_upload_into(ticket, output), Ok(true));
    assert_eq!(machine.complete_upload_into(ticket, output), Ok(true));
    ticket
}

#[test]
fn a_chunk_moves_through_every_residency_phase_with_exact_accounting() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Residue, ResidencyClass::Hot);
    let ticket = issue(&mut machine, request, &mut output);
    assert_eq!(output.requests, vec![ticket]);
    assert_eq!(machine.usage().in_flight, 2);
    assert_eq!(machine.ready_cpu_into(ticket, &mut output), Ok(true));
    assert_eq!(
        machine.snapshot(request.key).phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(machine.usage().cpu, 4);
    assert_eq!(machine.begin_upload_into(ticket, &mut output), Ok(true));
    assert_eq!(
        machine.snapshot(request.key).phase,
        ResidencyPhase::Uploading
    );
    assert_eq!(machine.usage().staging, 3);
    assert_eq!(machine.complete_upload_into(ticket, &mut output), Ok(true));
    assert_eq!(
        machine.snapshot(request.key).phase,
        ResidencyPhase::Resident
    );
    assert_eq!(machine.usage().gpu_hot, 5);
    assert_eq!(machine.usage().staging, 0);
}

#[test]
fn replacing_active_work_cancels_it_and_invalidates_its_ticket() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let first = issue(
        &mut machine,
        request(1, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    let second = issue(
        &mut machine,
        request(1, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    assert_eq!(second.generation(), first.generation() + 1);
    assert_eq!(output.cancellations, vec![first]);
    assert_eq!(machine.ready_cpu_into(first, &mut output), Ok(false));
    assert_eq!(output.stale.len(), 1);
    assert_eq!(output.stale[0].current_generation, second.generation());
}

#[test]
fn cancellation_releases_each_active_tier() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let requested = issue(
        &mut machine,
        request(1, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    assert!(machine.cancel_into(requested, &mut output));
    assert_eq!(output.cancellations, vec![requested]);
    assert_eq!(machine.usage(), Usage::default());

    let uploading = issue(
        &mut machine,
        request(2, ResidencyDetail::Residue, ResidencyClass::Hot),
        &mut output,
    );
    assert_eq!(machine.ready_cpu_into(uploading, &mut output), Ok(true));
    assert_eq!(machine.begin_upload_into(uploading, &mut output), Ok(true));
    assert!(machine.cancel_into(uploading, &mut output));
    assert_eq!(machine.usage(), Usage::default());
}

#[test]
fn failure_is_observable_and_a_retry_advances_the_generation() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Warm);
    let first = issue(&mut machine, request, &mut output);
    assert!(machine.fail_into(first, FailureReason::Provider, &mut output));
    assert_eq!(
        machine.snapshot(request.key),
        ResidencySnapshot {
            phase: ResidencyPhase::Absent,
            generation: 1,
            failure: Some(FailureReason::Provider),
        }
    );
    let second = issue(&mut machine, request, &mut output);
    assert_eq!(second.generation(), 2);
    assert_eq!(machine.snapshot(request.key).failure, None);
}

#[test]
fn an_oversized_request_is_rejected_without_changing_state() {
    let mut limits = budget();
    limits.in_flight = 1;
    let mut machine = ResidencyMachine::new(limits);
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Hot);
    assert_eq!(
        machine.request_into(request, &mut output),
        Err(ResidencyError::BudgetExceeded { tier: "in-flight" })
    );
    assert_eq!(machine.snapshot(request.key).generation, 0);
    assert_eq!(machine.usage(), Usage::default());
}

#[test]
fn staging_pressure_is_retryable_without_losing_cpu_data() {
    let mut limits = budget();
    limits.staging = 2;
    let mut machine = ResidencyMachine::new(limits);
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Hot);
    let ticket = issue(&mut machine, request, &mut output);
    assert_eq!(machine.ready_cpu_into(ticket, &mut output), Ok(true));
    assert_eq!(
        machine.begin_upload_into(ticket, &mut output),
        Err(ResidencyError::BudgetExceeded { tier: "staging" })
    );
    assert_eq!(
        machine.snapshot(request.key).phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(machine.usage().cpu, request.footprint.host_bytes);
}

#[test]
fn gpu_pressure_evicts_warm_before_hot_and_fine_before_coarse() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    make_resident(
        &mut machine,
        request(1, ResidencyDetail::Residue, ResidencyClass::Warm),
        &mut output,
    );
    make_resident(
        &mut machine,
        request(2, ResidencyDetail::Atom, ResidencyClass::Warm),
        &mut output,
    );
    make_resident(
        &mut machine,
        request(3, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    let mut limits = budget();
    limits.cpu = 4;
    machine.set_budget_into(limits, &mut output);
    assert_eq!(
        output
            .evictions
            .iter()
            .map(|item| item.key)
            .collect::<Vec<_>>(),
        vec![
            key(2, ResidencyDetail::Atom),
            key(1, ResidencyDetail::Residue)
        ]
    );
    assert_eq!(
        machine.snapshot(key(3, ResidencyDetail::Atom)).phase,
        ResidencyPhase::Resident
    );
}

#[test]
fn equal_rank_eviction_is_stable_by_key_not_insertion_order() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    make_resident(
        &mut machine,
        request(9, ResidencyDetail::Atom, ResidencyClass::Warm),
        &mut output,
    );
    let mut lower_key = request(2, ResidencyDetail::Atom, ResidencyClass::Warm);
    lower_key.priority = 9;
    make_resident(&mut machine, lower_key, &mut output);
    let mut limits = budget();
    limits.gpu_warm = 5;
    machine.set_budget_into(limits, &mut output);
    assert_eq!(output.evictions[0].key, lower_key.key);
}

#[test]
fn cpu_admission_evicts_resident_data_before_failing() {
    let mut limits = budget();
    limits.cpu = 4;
    let mut machine = ResidencyMachine::new(limits);
    let mut output = ResidencyOutput::default();
    let resident = make_resident(
        &mut machine,
        request(1, ResidencyDetail::Domain, ResidencyClass::Warm),
        &mut output,
    );
    let next = issue(
        &mut machine,
        request(2, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    assert_eq!(machine.ready_cpu_into(next, &mut output), Ok(true));
    assert_eq!(output.evictions[0].generation, resident.generation());
    assert_eq!(machine.usage().cpu, 4);
}

#[test]
fn a_chunk_larger_than_gpu_budget_returns_to_ready_cpu() {
    let mut limits = budget();
    limits.gpu_hot = 4;
    let mut machine = ResidencyMachine::new(limits);
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Hot);
    let ticket = issue(&mut machine, request, &mut output);
    assert_eq!(machine.ready_cpu_into(ticket, &mut output), Ok(true));
    assert_eq!(machine.begin_upload_into(ticket, &mut output), Ok(true));
    assert_eq!(
        machine.complete_upload_into(ticket, &mut output),
        Err(ResidencyError::BudgetExceeded { tier: "gpu-hot" })
    );
    assert_eq!(
        machine.snapshot(request.key).phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(machine.usage().staging, 0);
}

#[test]
fn device_loss_preserves_cpu_payloads_and_invalidates_only_resident_gpu_data() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let resident = make_resident(
        &mut machine,
        request(1, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    let uploading_request = request(2, ResidencyDetail::Residue, ResidencyClass::Warm);
    let uploading = issue(&mut machine, uploading_request, &mut output);
    assert_eq!(machine.ready_cpu_into(uploading, &mut output), Ok(true));
    assert_eq!(machine.begin_upload_into(uploading, &mut output), Ok(true));
    let Ok(report) = machine.device_lost_into(&mut output) else {
        panic!("device loss should preserve both CPU payloads")
    };
    assert_eq!(report.invalidated, 1);
    assert_eq!(report.ready_cpu, 2);
    assert_eq!(output.evictions[0].generation, resident.generation());
    assert_eq!(
        machine.snapshot(resident.key).phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(
        machine.snapshot(uploading.key).phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(output.cancellations, vec![uploading]);
    assert_eq!(output.ready_uploads.len(), 2);
    assert!(output.ready_uploads.contains(&resident));
    let replacement = output
        .ready_uploads
        .iter()
        .find(|ticket| ticket.key == uploading.key);
    let Some(replacement) = replacement else {
        panic!("uploading chunk should receive a replacement ticket")
    };
    assert_eq!(replacement.generation(), uploading.generation() + 1);
    assert_eq!(machine.usage().gpu_hot, 0);
    assert_eq!(machine.usage().staging, 0);
}

#[test]
fn invalid_phase_is_reported_without_mutating_the_entry() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Hot);
    let ticket = issue(&mut machine, request, &mut output);
    assert_eq!(
        machine.begin_upload_into(ticket, &mut output),
        Err(ResidencyError::InvalidPhase {
            required: ResidencyPhase::ReadyCpu,
            actual: ResidencyPhase::Requested,
        })
    );
    assert_eq!(
        machine.snapshot(request.key).phase,
        ResidencyPhase::Requested
    );
}

#[test]
fn pressure_indices_release_every_evicted_entry() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    for chunk in 0..16 {
        make_resident(
            &mut machine,
            request(chunk, ResidencyDetail::Atom, ResidencyClass::Warm),
            &mut output,
        );
    }
    let mut limits = budget();
    limits.gpu_warm = 0;
    machine.set_budget_into(limits, &mut output);
    let output_capacity = output.evictions.capacity();
    assert_eq!(machine.pressure_index_len(), 0);
    limits.gpu_warm = 128;
    machine.set_budget_into(limits, &mut output);
    assert_eq!(output.evictions.capacity(), output_capacity);
    assert_eq!(machine.pressure_index_len(), 0);
}

#[test]
fn replacing_the_only_in_flight_request_uses_its_released_budget() {
    let mut limits = budget();
    limits.in_flight = 2;
    let mut machine = ResidencyMachine::new(limits);
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Hot);
    let first = issue(&mut machine, request, &mut output);
    let second = issue(&mut machine, request, &mut output);
    assert_eq!(output.cancellations, vec![first]);
    assert_eq!(machine.usage().in_flight, 2);
    assert_eq!(second.generation(), 2);
}

#[test]
fn shrinking_active_budgets_cancels_work_in_deterministic_pressure_order() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let coarse = issue(
        &mut machine,
        request(1, ResidencyDetail::Domain, ResidencyClass::Hot),
        &mut output,
    );
    let fine = issue(
        &mut machine,
        request(2, ResidencyDetail::Atom, ResidencyClass::Warm),
        &mut output,
    );
    let mut limits = budget();
    limits.in_flight = 2;
    machine.set_budget_into(limits, &mut output);
    assert_eq!(output.cancellations, vec![fine]);
    assert_eq!(
        machine.snapshot(fine.key).failure,
        Some(FailureReason::BudgetExceeded)
    );
    assert_eq!(
        machine.snapshot(coarse.key).phase,
        ResidencyPhase::Requested
    );
    assert_eq!(machine.usage().in_flight, 2);
}

#[test]
fn exhausted_generations_are_a_typed_error_without_replacing_the_entry() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let request = request(1, ResidencyDetail::Atom, ResidencyClass::Hot);
    machine.entries.insert(
        request.key,
        Entry {
            generation: u64::MAX,
            phase: ResidencyPhase::Absent,
            footprint: request.footprint,
            class: request.class,
            priority: request.priority,
            failure: None,
        },
    );
    assert_eq!(
        machine.request_into(request, &mut output),
        Err(ResidencyError::GenerationExhausted)
    );
    assert_eq!(machine.snapshot(request.key).generation, u64::MAX);
}

#[test]
fn cancelled_ticket_completions_are_reported_as_stale() {
    let mut machine = ResidencyMachine::new(budget());
    let mut output = ResidencyOutput::default();
    let ticket = issue(
        &mut machine,
        request(1, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    assert!(machine.cancel_into(ticket, &mut output));
    assert_eq!(machine.ready_cpu_into(ticket, &mut output), Ok(false));
    assert_eq!(output.stale[0].current_phase, ResidencyPhase::Absent);
}

#[test]
fn failed_cpu_admission_does_not_release_another_in_flight_request() {
    let mut limits = budget();
    limits.cpu = 3;
    let mut machine = ResidencyMachine::new(limits);
    let mut output = ResidencyOutput::default();
    let failing = issue(
        &mut machine,
        request(1, ResidencyDetail::Atom, ResidencyClass::Hot),
        &mut output,
    );
    let waiting = issue(
        &mut machine,
        request(2, ResidencyDetail::Residue, ResidencyClass::Hot),
        &mut output,
    );
    assert_eq!(
        machine.ready_cpu_into(failing, &mut output),
        Err(ResidencyError::BudgetExceeded { tier: "cpu" })
    );
    assert_eq!(machine.usage().in_flight, 2);
    assert_eq!(
        machine.snapshot(waiting.key).phase,
        ResidencyPhase::Requested
    );
}
