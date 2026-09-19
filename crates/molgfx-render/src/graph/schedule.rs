//! Deterministic pass ordering.
//!
//! Topological sort over read/write dependencies. The tie-break is pass
//! declaration order — a stable key, never a hash-map iteration — so the
//! same graph schedules identically on every run, thread count and machine.
//! `O(passes² · resources)` at graph build time, which happens once per
//! graph change, never per frame.

use crate::error::RenderError;
use crate::graph::node::PassNode;
use molgfx_gpu::Device;

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;

/// Orders passes so every write lands before the reads that depend on it,
/// and writes to the same resource keep their declaration order.
///
/// # Errors
///
/// A dependency cycle.
pub(crate) fn schedule<D: Device>(passes: &[PassNode<D>]) -> Result<Vec<usize>, RenderError> {
    let n = passes.len();
    // after[i] holds passes that must run after pass i.
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree = vec![0usize; n];
    let add_edge = |after: &mut Vec<Vec<usize>>, in_degree: &mut Vec<usize>, i: usize, j: usize| {
        if !after[i].contains(&j) {
            after[i].push(j);
            in_degree[j] += 1;
        }
    };

    for (i, writer) in passes.iter().enumerate() {
        for (j, other) in passes.iter().enumerate() {
            if i == j {
                continue;
            }
            // A reader always follows the pass that writes what it reads,
            // wherever each was declared.
            if writer.writes.iter().any(|w| other.reads.contains(w)) {
                add_edge(&mut after, &mut in_degree, i, j);
            }
            // Two writers of one resource keep their declaration order.
            if i < j && writer.writes.iter().any(|w| other.writes.contains(w)) {
                add_edge(&mut after, &mut in_degree, i, j);
            }
        }
    }

    // Kahn's algorithm, always taking the lowest-index ready pass: the
    // declaration-order tie-break.
    let mut order = Vec::with_capacity(n);
    let mut ready: Vec<bool> = in_degree.iter().map(|&d| d == 0).collect();
    while order.len() < n {
        let Some(next) = ready.iter().position(|&r| r) else {
            let stuck = in_degree
                .iter()
                .position(|&d| d > 0 && d != usize::MAX)
                .and_then(|i| passes.get(i))
                .map_or("<unknown>", |p| p.name);
            return Err(RenderError::GraphCycle { pass: stuck });
        };
        ready[next] = false;
        in_degree[next] = usize::MAX; // consumed
        order.push(next);
        for &j in &after[next] {
            if in_degree[j] != usize::MAX {
                in_degree[j] -= 1;
                if in_degree[j] == 0 {
                    ready[j] = true;
                }
            }
        }
    }
    Ok(order)
}
