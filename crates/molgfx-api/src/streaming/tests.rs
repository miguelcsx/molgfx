use crate::streaming::{
    Cancellation, Chunk, DataSource, Limits, Metadata, Priority, Request, Scheduler, SourceError,
};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

fn complete<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test future unexpectedly remained pending"),
    }
}

fn request(chunks: Vec<u64>) -> Request {
    Request {
        chunks,
        priority: Priority::Visible,
    }
}

fn chunk(id: u64) -> Chunk {
    Chunk {
        id,
        bytes: Arc::from([id.to_le_bytes()[0]]),
    }
}

#[test]
fn cancellation_is_shared_and_monotonic() {
    let cancellation = Cancellation::default();
    let observer = cancellation.clone();
    assert!(!observer.is_cancelled());
    cancellation.cancel();
    assert!(observer.is_cancelled());
}

#[test]
fn zero_sized_stream_batches_are_rejected() {
    struct Source;
    impl crate::streaming::DataSource for Source {
        fn metadata(
            &self,
        ) -> impl Future<Output = Result<crate::streaming::Metadata, crate::streaming::SourceError>> + Send
        {
            std::future::ready(Err(SourceError::Shutdown))
        }

        fn request(
            &self,
            _: crate::streaming::Request,
            _: Cancellation,
        ) -> impl Future<Output = Result<Vec<crate::streaming::Chunk>, SourceError>> + Send
        {
            std::future::ready(Err(SourceError::Shutdown))
        }

        fn shutdown(&self) -> impl Future<Output = Result<(), SourceError>> + Send {
            std::future::ready(Ok(()))
        }
    }

    assert!(matches!(
        crate::streaming::Scheduler::new(
            Source,
            crate::streaming::Limits {
                maximum_batch: 0,
                maximum_in_flight: 1,
            }
        ),
        Err(SourceError::InvalidBatch(_))
    ));
}

#[test]
fn provider_order_and_cardinality_are_validated() {
    struct Reversed;
    impl DataSource for Reversed {
        fn metadata(&self) -> impl Future<Output = Result<Metadata, SourceError>> + Send {
            std::future::ready(Err(SourceError::Shutdown))
        }

        fn request(
            &self,
            request: Request,
            _: Cancellation,
        ) -> impl Future<Output = Result<Vec<Chunk>, SourceError>> + Send {
            std::future::ready(Ok(request.chunks.into_iter().rev().map(chunk).collect()))
        }

        fn shutdown(&self) -> impl Future<Output = Result<(), SourceError>> + Send {
            std::future::ready(Ok(()))
        }
    }

    let scheduler = Scheduler::new(Reversed, Limits::default())
        .unwrap_or_else(|error| panic!("valid scheduler: {error}"));
    assert!(matches!(
        complete(scheduler.request(request(vec![1, 2]), Cancellation::default())),
        Err(SourceError::InvalidBatch(_))
    ));
}

#[test]
fn cancellation_after_provider_work_prevents_publication() {
    struct Cancelling;
    impl DataSource for Cancelling {
        fn metadata(&self) -> impl Future<Output = Result<Metadata, SourceError>> + Send {
            std::future::ready(Err(SourceError::Shutdown))
        }

        fn request(
            &self,
            request: Request,
            cancellation: Cancellation,
        ) -> impl Future<Output = Result<Vec<Chunk>, SourceError>> + Send {
            cancellation.cancel();
            std::future::ready(Ok(request.chunks.into_iter().map(chunk).collect()))
        }

        fn shutdown(&self) -> impl Future<Output = Result<(), SourceError>> + Send {
            std::future::ready(Ok(()))
        }
    }

    let scheduler = Scheduler::new(Cancelling, Limits::default())
        .unwrap_or_else(|error| panic!("valid scheduler: {error}"));
    assert_eq!(
        complete(scheduler.request(request(vec![1]), Cancellation::default())),
        Err(SourceError::Cancelled)
    );
}

#[test]
fn in_flight_requests_apply_backpressure() {
    struct Pending;
    impl DataSource for Pending {
        fn metadata(&self) -> impl Future<Output = Result<Metadata, SourceError>> + Send {
            std::future::ready(Err(SourceError::Shutdown))
        }

        async fn request(&self, _: Request, _: Cancellation) -> Result<Vec<Chunk>, SourceError> {
            std::future::pending().await
        }

        fn shutdown(&self) -> impl Future<Output = Result<(), SourceError>> + Send {
            std::future::ready(Ok(()))
        }
    }

    let scheduler = Scheduler::new(
        Pending,
        Limits {
            maximum_batch: 2,
            maximum_in_flight: 1,
        },
    )
    .unwrap_or_else(|error| panic!("valid scheduler: {error}"));
    let mut active = Box::pin(scheduler.request(request(vec![1]), Cancellation::default()));
    let mut context = Context::from_waker(Waker::noop());
    assert!(active.as_mut().poll(&mut context).is_pending());
    assert_eq!(
        complete(scheduler.request(request(vec![2]), Cancellation::default())),
        Err(SourceError::Backpressure)
    );
}

#[test]
fn shutdown_is_idempotent_and_rejects_new_work() {
    struct Source(Arc<AtomicUsize>);
    impl DataSource for Source {
        fn metadata(&self) -> impl Future<Output = Result<Metadata, SourceError>> + Send {
            std::future::ready(Ok(Metadata {
                identity: "test".into(),
                chunk_count: Some(1),
                byte_length: Some(1),
            }))
        }

        fn request(
            &self,
            request: Request,
            _: Cancellation,
        ) -> impl Future<Output = Result<Vec<Chunk>, SourceError>> + Send {
            std::future::ready(Ok(request.chunks.into_iter().map(chunk).collect()))
        }

        fn shutdown(&self) -> impl Future<Output = Result<(), SourceError>> + Send {
            let _ = self.0.fetch_add(1, Ordering::Relaxed);
            std::future::ready(Ok(()))
        }
    }

    let shutdowns = Arc::new(AtomicUsize::new(0));
    let scheduler = Scheduler::new(Source(Arc::clone(&shutdowns)), Limits::default())
        .unwrap_or_else(|error| panic!("valid scheduler: {error}"));
    assert_eq!(complete(scheduler.shutdown()), Ok(()));
    assert_eq!(complete(scheduler.shutdown()), Ok(()));
    assert_eq!(shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(complete(scheduler.metadata()), Err(SourceError::Shutdown));
    assert_eq!(
        complete(scheduler.request(request(vec![1]), Cancellation::default())),
        Err(SourceError::Shutdown)
    );
}
