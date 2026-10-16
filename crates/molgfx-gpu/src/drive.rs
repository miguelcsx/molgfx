//! Driving a GPU future to completion on a thread that owns the device.

use crate::device::{Device, PollOutcome};
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Runs `future` to completion, polling `device` whenever it is pending.
///
/// GPU futures resolve only as the device is polled, so a plain executor would
/// wait forever. This is the blocking bridge for callers with no event loop of
/// their own: it spends the thread, where an embedder with a frame loop calls
/// [`Device::poll`] once per frame instead. A poll that fails is not reported
/// here; the future observes the same failure and resolves with it.
pub fn block_on_polling<D: Device, F: Future>(device: &D, future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        if !matches!(device.poll(), Ok(PollOutcome::Idle)) {
            std::thread::yield_now();
        }
    }
}
