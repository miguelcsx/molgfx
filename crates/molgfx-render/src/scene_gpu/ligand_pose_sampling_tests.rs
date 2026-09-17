use super::*;

fn sample(key: u64, opaque: u32, translucent: u32, cost: u32) -> PoseBatchSample {
    let Ok(plan_index) = usize::try_from(key) else {
        panic!("test key fits usize");
    };
    PoseBatchSample::new(plan_index, key, opaque, translucent, cost, 2)
}

fn selection(samples: &[PoseBatchSample]) -> Vec<(u64, u32, u32)> {
    let mut result = samples
        .iter()
        .map(|sample| {
            (
                sample.stable_key,
                sample.selected_opaque,
                sample.selected_translucent,
            )
        })
        .collect::<Vec<_>>();
    result.sort_unstable_by_key(|row| row.0);
    result
}

#[test]
fn realtime_keeps_every_complete_candidate_when_both_budgets_fit() {
    let mut samples = [sample(1, 400, 0, 18), sample(2, 40, 40, 11)];

    select_pose_batches(&mut samples, false);

    assert_eq!(selection(&samples), vec![(1, 400, 0), (2, 40, 40)]);
}

#[test]
fn heterogeneous_topologies_spend_the_instance_budget_on_more_candidates() {
    let mut samples = [sample(1, 10_000, 0, 2), sample(2, 10_000, 0, 100)];

    select_pose_batches(&mut samples, false);

    assert_eq!(selection(&samples), vec![(1, 4_096, 0), (2, 0, 0)]);
    let instances = samples
        .iter()
        .map(|sample| u64::from(sample.selected()) * u64::from(sample.primitive_cost))
        .sum::<u64>();
    assert_eq!(instances, REALTIME_POSE_PRIMITIVES);
}

#[test]
fn activated_batches_allocate_candidates_cheapest_first() {
    let mut samples = [sample(1, 10_000, 0, 2), sample(2, 100, 0, 1)];

    select_pose_batches(&mut samples, false);

    assert_eq!(selection(&samples), vec![(1, 4_046, 0), (2, 100, 0)]);
    assert_eq!(
        samples.iter().map(PoseBatchSample::selected).sum::<u32>(),
        4_146
    );
}

#[test]
fn allocation_is_identical_after_input_order_is_permuted() {
    let input = [
        sample(40, 20_000, 0, 3),
        sample(10, 500, 500, 2),
        sample(30, 20_000, 0, 7),
        sample(20, 2, 0, 1),
    ];
    let mut forward = input;
    let mut permuted = [input[2], input[0], input[3], input[1]];

    select_pose_batches(&mut forward, false);
    select_pose_batches(&mut permuted, false);

    assert_eq!(selection(&forward), selection(&permuted));
}

#[test]
fn mixed_opacity_is_one_batch_share_not_two_fairness_competitors() {
    let mut mixed = [sample(1, 10_000, 10_000, 1)];
    let mut opaque = [sample(1, 20_000, 0, 1)];

    select_pose_batches(&mut mixed, false);
    select_pose_batches(&mut opaque, false);

    assert_eq!(mixed[0].selected(), opaque[0].selected());
    assert_eq!(mixed[0].selected_opaque, 4_096);
    assert_eq!(mixed[0].selected_translucent, 4_096);
}

#[test]
fn many_tiny_batches_stop_at_the_fixed_draw_activation_ceiling() {
    let mut samples = (0..20_000)
        .map(|key| PoseBatchSample::new(key, key as u64, 1, 0, 1, 1))
        .collect::<Vec<_>>();

    select_pose_batches(&mut samples, false);

    let groups = samples
        .iter()
        .map(PoseBatchSample::draw_groups)
        .sum::<u32>();
    assert_eq!(groups, REALTIME_POSE_DRAW_GROUPS);
    assert_eq!(
        samples.iter().map(PoseBatchSample::selected).sum::<u32>(),
        128
    );
    assert_eq!(REALTIME_POSE_RASTER_DRAWS, 256);
}

#[test]
fn quality_mode_preserves_every_visible_source_pose() {
    let mut samples = [sample(1, u32::MAX, u32::MAX, u32::MAX), sample(2, 17, 3, 3)];

    select_pose_batches(&mut samples, true);

    assert_eq!(
        selection(&samples),
        vec![(1, u32::MAX, u32::MAX), (2, 17, 3)]
    );
}

#[test]
fn ben_scale_topology_samples_large_batches_to_the_same_stable_ceiling() {
    let mut ten_thousand = [sample(1, 10_000, 0, 18)];
    let mut fifty_thousand = [sample(1, 50_000, 0, 18)];

    select_pose_batches(&mut ten_thousand, false);
    select_pose_batches(&mut fifty_thousand, false);

    assert_eq!(ten_thousand[0].selected(), 455);
    assert_eq!(fifty_thousand[0].selected(), 455);
}

#[test]
fn realtime_raster_work_has_an_explicit_beauty_plus_shadow_ceiling() {
    let beauty = REALTIME_POSE_PRIMITIVES;
    let shadow = realtime_shadow_instances(REALTIME_POSE_SHADOW_PRIMITIVES);

    assert_eq!(beauty + shadow, REALTIME_POSE_RASTER_PRIMITIVES);
    assert_eq!(REALTIME_POSE_RASTER_PRIMITIVES, 12 * 1_024);
    assert_eq!(realtime_shadow_instances(4_097), 0);
}

#[test]
fn sampled_indices_are_deterministic_distributed_and_include_both_ends() {
    let first = (0..5)
        .map(|ordinal| distributed_source_index(ordinal, 101, 5))
        .collect::<Vec<_>>();
    let second = (0..5)
        .map(|ordinal| distributed_source_index(ordinal, 101, 5))
        .collect::<Vec<_>>();

    assert_eq!(first, vec![0, 25, 50, 75, 100]);
    assert_eq!(first, second);
}
