//! Weak scene subscriptions publish only after releasing the authoritative scene borrow.
use crate::scene_binding::PyScene;
use pyo3::prelude::*;

pub(super) fn publish(
    slf: &Bound<'_, PyScene>,
    py: Python<'_>,
    encoded: &str,
    runtime: bool,
) -> PyResult<()> {
    let subscribers: Vec<_> = {
        let scene = slf.borrow();
        let subscribers = if runtime {
            &scene.runtime_subscribers
        } else {
            &scene.subscribers
        };
        subscribers
            .iter()
            .map(|reference| reference.clone_ref(py))
            .collect()
    };
    for reference in subscribers {
        let attached = {
            let scene = slf.borrow();
            let subscribers = if runtime {
                &scene.runtime_subscribers
            } else {
                &scene.subscribers
            };
            subscribers
                .iter()
                .any(|current| current.bind(py).is(reference.bind(py)))
        };
        if !attached {
            continue;
        }
        let callback = reference.call0(py)?;
        if callback.bind(py).is_none() {
            let mut scene = slf.borrow_mut();
            let subscribers = if runtime {
                &mut scene.runtime_subscribers
            } else {
                &mut scene.subscribers
            };
            subscribers.retain(|current| !current.bind(py).is(reference.bind(py)));
        } else {
            let _ = callback.call1(py, (encoded,))?;
        }
    }
    Ok(())
}

#[pymethods]
impl PyScene {
    /// Registers a weak callable receiving runtime coordinate sample events as JSON.
    fn subscribe_runtime(&mut self, subscriber: Py<PyAny>) {
        self.runtime_subscribers.push(subscriber);
    }

    /// Detaches the exact runtime subscription without waiting for another sample.
    fn unsubscribe_runtime(&mut self, py: Python<'_>, subscriber: &Bound<'_, PyAny>) {
        self.runtime_subscribers
            .retain(|reference| !reference.bind(py).is(subscriber));
    }
}
