//! Ordered statistical observations, independent of session ownership.
use crate::gatekeeper::bus::{NatsBus, NatsSubscription, PublishOutcome};
use crate::jobs::Jobs;
use crate::protocol::bus::statistics::{
    Event, OBSERVATIONS_SUBJECT, Observation, ProducerGrant, ProducerReply, ProducerRequest,
    SNAPSHOTS_SUBJECT, frame_producer,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct QueuedObservation {
    event: Event,
    deadline: Option<tokio::time::Instant>,
}

#[derive(Default)]
struct ProducerSession {
    grant: Option<ProducerGrant>,
    confirmed: bool,
    sequence: u64,
}
impl ProducerSession {
    fn adopt(&mut self, grant: ProducerGrant, confirmed: bool) {
        if self.grant.as_ref().is_none_or(|old| {
            old.incarnation != grant.incarnation || old.generation != grant.generation
        }) {
            self.sequence = 0;
        }
        self.grant = Some(grant);
        self.confirmed = confirmed;
    }
    async fn prepare(
        &mut self,
        bus: &dyn NatsBus,
        producer: uuid::Uuid,
        hint: Option<uuid::Uuid>,
    ) -> bool {
        if self.confirmed
            && self
                .grant
                .as_ref()
                .is_some_and(|grant| hint.is_none_or(|source| source == grant.incarnation))
        {
            return true;
        }
        let Some(ProducerReply::Status { incarnation }) =
            control(bus, ProducerRequest::Status).await
        else {
            return false;
        };
        if self.confirmed
            && self
                .grant
                .as_ref()
                .is_some_and(|grant| grant.incarnation == incarnation)
        {
            return true;
        }
        if self
            .grant
            .as_ref()
            .is_none_or(|grant| grant.incarnation != incarnation)
        {
            let reply = control(
                bus,
                ProducerRequest::Begin {
                    incarnation,
                    producer,
                },
            )
            .await;
            match reply {
                Some(ProducerReply::Grant(grant))
                    if grant.incarnation == incarnation
                        && grant.producer == producer
                        && grant.generation > 0 =>
                {
                    self.adopt(grant, false);
                }
                Some(ProducerReply::Active(grant))
                    if grant.incarnation == incarnation
                        && grant.producer == producer
                        && grant.generation > 0 =>
                {
                    self.adopt(grant, true)
                }
                _ => return false,
            }
        }
        if self.confirmed {
            return true;
        }
        let grant = self.grant.clone().unwrap();
        match control(bus, ProducerRequest::Confirm(grant.clone())).await {
            Some(ProducerReply::Active(active)) if active == grant => {
                self.confirmed = true;
                true
            }
            Some(ProducerReply::Rejected { .. }) => {
                self.grant = None;
                false
            }
            _ => false,
        }
    }
}
async fn control(bus: &dyn NatsBus, request: ProducerRequest) -> Option<ProducerReply> {
    ProducerReply::decode(&bus.statistics_control(request.encode()?).await.ok()?)
}
async fn submit_observation(
    bus: &dyn NatsBus,
    producer: uuid::Uuid,
    session: &mut ProducerSession,
    source: &parking_lot::Mutex<Option<uuid::Uuid>>,
    event: Event,
    deadline: tokio::time::Instant,
) -> bool {
    let hint = *source.lock();
    if !session.prepare(bus, producer, hint).await || tokio::time::Instant::now() >= deadline {
        return false;
    }
    let grant = session.grant.as_ref().unwrap();
    {
        let mut current = source.lock();
        if *current == hint {
            *current = Some(grant.incarnation);
        }
    }
    let Some(sequence) = session.sequence.checked_add(1) else {
        return false;
    };
    session.sequence = sequence;
    let terminal = matches!(&event, Event::Disconnect { .. });
    let Some(bytes) = (Observation {
        version: 2,
        incarnation: grant.incarnation,
        generation: grant.generation,
        producer,
        sequence,
        event,
    })
    .encode() else {
        return false;
    };
    if terminal {
        bus.publish_statistical_disconnect(bytes, deadline).await == PublishOutcome::Submitted
    } else {
        bus.publish(OBSERVATIONS_SUBJECT, bytes).await == PublishOutcome::Submitted
    }
}

/// Admission is bounded and never promises broker delivery or replay.
pub struct Observations {
    bus: Arc<dyn NatsBus>,
    tx: tokio::sync::mpsc::Sender<QueuedObservation>,
    dropped: Arc<AtomicU64>,
    stop: tokio_util::sync::CancellationToken,
    close: tokio_util::sync::CancellationToken,
    stopped: tokio_util::sync::CancellationToken,
    _source: Option<NatsSubscription>,
}
impl Observations {
    pub fn new(bus: Arc<dyn NatsBus>, jobs: &Jobs) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
        let dropped = Arc::new(AtomicU64::new(0));
        let failures = dropped.clone();
        let publishing = bus.clone();
        let stop = tokio_util::sync::CancellationToken::new();
        let close = tokio_util::sync::CancellationToken::new();
        let stopped = tokio_util::sync::CancellationToken::new();
        let stopping = stop.clone();
        let closing = close.clone();
        let completed = stopped.clone();
        let source_hint = Arc::new(parking_lot::Mutex::new(None));
        let hint = source_hint.clone();
        let source = bus.subscribe(
            SNAPSHOTS_SUBJECT,
            None,
            Arc::new(move |_, bytes| {
                if let Some(producer) = frame_producer(bytes) {
                    *hint.lock() = Some(producer);
                }
            }),
        );
        jobs.spawn(
            "archipelago-statistics-observations",
            move |ctx| async move {
                let producer = uuid::Uuid::new_v4();
                let mut session = ProducerSession::default();
                let mut deadline = None;
                let mut warnings = tokio::time::interval(std::time::Duration::from_secs(10));
                warnings.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                let mut reported = 0;
                'events: loop {
                    let event = tokio::select! {
                        biased;
                        _ = async { match deadline { Some(at) => tokio::time::sleep_until(at).await, None => std::future::pending().await } } => {
                            rx.close();
                            break;
                        }
                        _ = stopping.cancelled(), if deadline.is_none() => {
                            deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5)); continue;
                        }
                        _ = ctx.cancel.cancelled(), if deadline.is_none() => {
                            rx.close(); deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5)); continue;
                        }
                        _ = closing.cancelled(), if !rx.is_closed() => { rx.close(); continue; }
                        _ = warnings.tick() => {
                            warn_dropped(&failures, &mut reported);
                            continue;
                        }
                        event = rx.recv() => match event { Some(event) => event, None => break },
                    };
                    let QueuedObservation { event, deadline: publication_deadline } = event;
                    if publication_deadline.is_some_and(|at| tokio::time::Instant::now() >= at) {
                        failures.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let terminal = matches!(&event, Event::Disconnect { .. });
                    let publication_deadline = publication_deadline.unwrap_or_else(|| tokio::time::Instant::now() + crate::protocol::bus::REQUEST_DEADLINE);
                    let submission = submit_observation(&*publishing, producer, &mut session, &source_hint, event, publication_deadline);
                    tokio::pin!(submission);
                    let submitted =
                        if terminal {
                            let outcome = loop {
                                tokio::select! {
                                    biased;
                                    _ = async { match deadline { Some(at) => tokio::time::sleep_until(at).await, None => std::future::pending().await } } => {
                                        rx.close();
                                        failures.fetch_add(1, Ordering::Relaxed);
                                        break 'events;
                                    }
                                    _ = stopping.cancelled(), if deadline.is_none() => {
                                        deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
                                    }
                                    _ = ctx.cancel.cancelled(), if deadline.is_none() => {
                                        rx.close(); deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
                                    }
                                    _ = closing.cancelled(), if !rx.is_closed() => rx.close(),
                                    _ = tokio::time::sleep_until(publication_deadline) => break false,
                                    _ = warnings.tick() => warn_dropped(&failures, &mut reported),
                                    outcome = &mut submission => break outcome,
                                }
                            };
                            outcome
                        } else { tokio::time::timeout_at(publication_deadline, &mut submission).await.unwrap_or(false) };
                    if !submitted {
                        failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
                // Outstanding send reservations can still commit after close.
                // In-flight submission is dropped before this discard-only drain.
                rx.close();
                while rx.recv().await.is_some() {
                    failures.fetch_add(1, Ordering::Relaxed);
                }
                warn_dropped(&failures, &mut reported);
                if !matches!(tokio::time::timeout(std::time::Duration::from_secs(2), publishing.flush()).await, Ok(true)) {
                    tracing::warn!("statistics shutdown broker flush unconfirmed");
                }
                completed.cancel();
                Ok(())
            },
        );
        Self {
            bus,
            tx,
            dropped,
            stop,
            close,
            stopped,
            _source: Some(source),
        }
    }
    pub fn publish(&self, event: Event) -> bool {
        if !self.bus.is_enabled()
            || !self.bus.is_ready()
            || self
                .tx
                .try_send(QueuedObservation {
                    event,
                    deadline: None,
                })
                .is_err()
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }
    /// One existing publication budget covers enqueue, queue age and submission.
    /// Socket teardown retains its handler permit only through bounded admission.
    /// Terminal events share the heartbeat FIFO, so they cannot overtake older input.
    pub async fn disconnect(&self, address: String) -> bool {
        let deadline = tokio::time::Instant::now() + crate::protocol::bus::REQUEST_DEADLINE;
        if !self.bus.is_enabled()
            || !matches!(
                tokio::time::timeout_at(
                    deadline,
                    self.tx.send(QueuedObservation {
                        event: Event::Disconnect { address },
                        deadline: Some(deadline),
                    })
                )
                .await,
                Ok(Ok(()))
            )
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }
    pub async fn shutdown(&self) {
        self.shutdown_after(std::future::ready(())).await;
    }
    /// Begin the existing drain budget now, allowing handlers to finish producing
    /// until they complete or that budget expires.
    pub async fn shutdown_after(&self, handlers_done: impl std::future::Future<Output = ()>) {
        self.stop.cancel();
        let completion = async {
            tokio::select! {
                _ = self.stopped.cancelled() => return,
                _ = handlers_done => self.close.cancel(),
            }
            self.stopped.cancelled().await;
        };
        if tokio::time::timeout(std::time::Duration::from_secs(10), completion)
            .await
            .is_err()
        {
            tracing::warn!("statistics publisher shutdown deadline exceeded");
        }
        tracing::info!(dropped = self.dropped(), "statistics publisher stopped");
    }
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}
fn warn_dropped(failures: &AtomicU64, reported: &mut u64) {
    let total_dropped = failures.load(Ordering::Relaxed);
    if total_dropped != *reported {
        tracing::warn!(
            dropped = total_dropped.saturating_sub(*reported),
            total_dropped,
            "statistics observations lost; no replay"
        );
        *reported = total_dropped;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatekeeper::bus::InProcessBus;
    use crate::protocol::bus::statistics::{OBSERVATIONS_SUBJECT, Observation};
    #[derive(Default)]
    struct BlockedPublicationBus {
        inner: InProcessBus,
        block_control: std::sync::atomic::AtomicBool,
        publication_started: tokio::sync::Notify,
        publication_finished: tokio::sync::Notify,
        flush_started: tokio::sync::Notify,
        finish_flush: tokio::sync::Notify,
    }
    struct PublicationFinished<'a>(&'a tokio::sync::Notify);
    impl Drop for PublicationFinished<'_> {
        fn drop(&mut self) {
            self.0.notify_one();
        }
    }
    #[async_trait::async_trait]
    impl NatsBus for BlockedPublicationBus {
        fn is_enabled(&self) -> bool {
            true
        }
        fn is_connected(&self) -> bool {
            true
        }
        fn is_ready(&self) -> bool {
            true
        }
        fn connect(&self) {}
        fn subscribe(
            &self,
            subject: &str,
            queue: Option<&str>,
            handler: crate::gatekeeper::bus::NatsHandler,
        ) -> crate::gatekeeper::bus::NatsSubscription {
            self.inner.subscribe(subject, queue, handler)
        }
        async fn publish(&self, _: &str, payload: Vec<u8>) -> PublishOutcome {
            self.publication_started.notify_one();
            if matches!(Observation::decode(&payload).unwrap().event, Event::Heartbeat { address, .. } if address == "probe")
            {
                return PublishOutcome::Submitted;
            }
            let _finished = PublicationFinished(&self.publication_finished);
            std::future::pending().await
        }
        async fn request(
            &self,
            _: &str,
            _: Vec<u8>,
        ) -> Result<Vec<u8>, crate::gatekeeper::bus::RequestError> {
            Err(crate::gatekeeper::bus::RequestError::NoResponders)
        }
        async fn statistics_control(
            &self,
            payload: Vec<u8>,
        ) -> Result<Vec<u8>, crate::gatekeeper::bus::RequestError> {
            if self.block_control.load(Ordering::Relaxed) {
                return std::future::pending().await;
            }
            self.inner.statistics_control(payload).await
        }
        async fn flush(&self) -> bool {
            self.flush_started.notify_one();
            self.finish_flush.notified().await;
            true
        }
    }
    #[tokio::test(start_paused = true)]
    async fn shutdown_deadline_cancels_unsubmitted_disconnect_and_counts_all_forced_loss_once() {
        for (handlers_finish, block_control) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let bus = Arc::new(BlockedPublicationBus::default());
            bus.block_control.store(block_control, Ordering::Relaxed);
            bus.finish_flush.notify_one();
            let jobs = Jobs::new();
            let out = Observations::new(bus, &jobs);
            assert!(out.disconnect("first".into()).await);
            assert!(out.disconnect("queued".into()).await);
            let started = tokio::time::Instant::now();
            if handlers_finish {
                out.shutdown().await;
            } else {
                out.shutdown_after(std::future::pending::<()>()).await;
            }
            assert_eq!(
                started.elapsed(),
                if handlers_finish {
                    crate::protocol::bus::REQUEST_DEADLINE
                } else {
                    std::time::Duration::from_secs(5)
                }
            );
            assert_eq!(
                out.dropped(),
                2,
                "one in-flight and one queued terminal were forced out"
            );
            assert!(!out.disconnect("closed".into()).await);
            assert_eq!(out.dropped(), 3);
            assert!(
                jobs.shutdown(std::time::Duration::from_secs(1))
                    .await
                    .is_empty()
            );
        }
        for (terminal, write_reserved) in [(false, false), (true, true)] {
            let bus = Arc::new(BlockedPublicationBus::default());
            let jobs = Jobs::new();
            let out = Observations::new(bus.clone(), &jobs);
            let drain = out.shutdown_after(std::future::pending::<()>());
            tokio::pin!(drain);
            assert!(futures::poll!(drain.as_mut()).is_pending());
            assert!(out.publish(Event::Heartbeat {
                address: "probe".into(),
                position: [0.; 3]
            }));
            // Biased stop handling establishes the drain deadline before this probe.
            bus.publication_started.notified().await;
            tokio::time::advance(std::time::Duration::from_secs(4)).await;
            let beat = || Event::Heartbeat {
                address: "wallet".into(),
                position: [0.; 3],
            };
            let terminal_deadline =
                tokio::time::Instant::now() + crate::protocol::bus::REQUEST_DEADLINE;
            if terminal {
                assert!(out.disconnect("in-flight".into()).await);
            } else {
                assert!(out.publish(beat()));
            }
            bus.publication_started.notified().await;
            assert!(out.publish(beat()));
            let reserved = out.tx.reserve().await.unwrap();
            tokio::time::advance(if terminal {
                crate::protocol::bus::REQUEST_DEADLINE / 2
            } else {
                crate::protocol::bus::REQUEST_DEADLINE
            })
            .await;
            bus.publication_finished.notified().await;
            if terminal {
                assert!(
                    tokio::time::Instant::now() < terminal_deadline,
                    "global cutoff must cancel the in-flight terminal before its own deadline"
                );
            }
            let flush = bus.flush_started.notified();
            tokio::pin!(flush);
            assert!(
                futures::poll!(flush.as_mut()).is_pending(),
                "flush must wait for the outstanding reserved slot"
            );
            if write_reserved {
                reserved.send(QueuedObservation {
                    event: Event::Disconnect {
                        address: "reserved".into(),
                    },
                    deadline: Some(terminal_deadline),
                });
            } else {
                drop(reserved);
            }
            flush.await;
            let cutoff_losses = if write_reserved { 3 } else { 2 };
            assert_eq!(
                out.dropped(),
                cutoff_losses,
                "global cutoff owns the in-flight and queued losses once"
            );
            assert!(
                !out.disconnect("during-flush".into()).await,
                "global cutoff must close admission before flush"
            );
            assert!(!out.publish(beat()));
            assert_eq!(
                out.dropped(),
                cutoff_losses + 2,
                "late terminal and heartbeat refusals are counted"
            );
            bus.finish_flush.notify_one();
            drain.await;
            assert_eq!(out.dropped(), cutoff_losses + 2);
            assert!(
                jobs.shutdown(std::time::Duration::from_secs(1))
                    .await
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn saturated_heartbeats_cannot_drop_or_overtake_a_waiting_disconnect() {
        let bus = Arc::new(InProcessBus::new());
        let jobs = Jobs::new();
        let out = Observations::new(bus.clone(), &jobs);
        let heartbeat = || Event::Heartbeat {
            address: "wallet".into(),
            position: [1.; 3],
        };
        for _ in 0..1024 {
            assert!(out.publish(heartbeat()));
        }
        let disconnect = out.disconnect("wallet".into());
        tokio::pin!(disconnect);
        assert!(futures::poll!(disconnect.as_mut()).is_pending());
        assert_eq!(out.dropped(), 0);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(3), disconnect)
                .await
                .unwrap()
        );
        assert!(out.publish(heartbeat()));
        out.shutdown().await;
        let observations: Vec<_> = bus
            .published()
            .iter()
            .map(|(_, bytes)| Observation::decode(bytes).unwrap())
            .collect();
        assert_eq!(observations.len(), 1026);
        assert!(
            observations[..1024]
                .iter()
                .all(|observation| matches!(observation.event, Event::Heartbeat { .. }))
        );
        assert!(matches!(observations[1024].event, Event::Disconnect { .. }));
        assert!(matches!(observations[1025].event, Event::Heartbeat { .. }));
        assert_eq!(out.dropped(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn disconnect_waiters_retain_admission_credit_across_repeated_churn() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let out = Arc::new(Observations {
            bus: Arc::new(InProcessBus::new()),
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
            stop: Default::default(),
            close: Default::default(),
            stopped: Default::default(),
            _source: None,
        });
        for _ in 0..2 {
            assert!(out.publish(Event::Heartbeat {
                address: "earlier".into(),
                position: [0.; 3]
            }));
        }
        let handlers = Arc::new(tokio::sync::Semaphore::new(3));
        let mut closing = tokio::task::JoinSet::new();
        for id in 0..30 {
            let handlers = handlers.clone();
            let out = out.clone();
            closing.spawn(async move {
                let _handler = handlers.acquire_owned().await.unwrap();
                assert!(
                    !out.disconnect(format!("wallet-{id}")).await,
                    "stalled admission must expire"
                );
            });
        }
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while handlers.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(
            closing.try_join_next().is_none(),
            "a saturated queue cannot release teardown admission credits"
        );
        tokio::time::advance(crate::protocol::bus::REQUEST_DEADLINE).await;
        while let Some(result) = closing.join_next().await {
            result.unwrap();
        }
        assert_eq!(handlers.available_permits(), 3);
        assert_eq!(
            rx.len(),
            2,
            "timed-out terminal senders cannot enqueue later"
        );
        assert_eq!(out.dropped(), 30, "each failed close is counted once");
        for _ in 0..2 {
            rx.recv().await.unwrap();
        }
        assert!(out.disconnect("recovered".into()).await);
        assert!(
            matches!(rx.recv().await.unwrap().event, Event::Disconnect { address } if address == "recovered")
        );
    }

    #[tokio::test]
    async fn disconnect_admission_ignores_link_readiness_but_counts_bus_and_shutdown_loss() {
        let bus = Arc::new(InProcessBus::new());
        bus.set_connected(false);
        let out = Observations::new(bus.clone(), &Jobs::new());
        assert!(out.disconnect("wallet".into()).await);
        assert_eq!(
            out.dropped(),
            0,
            "link state must not reject terminal queue admission"
        );
        out.shutdown().await;
        assert_eq!(
            out.dropped(),
            1,
            "bus-level refusal remains an explicit loss"
        );
        assert!(bus.published().is_empty());
        assert!(!out.disconnect("later".into()).await);
        assert_eq!(
            out.dropped(),
            2,
            "closed queues count rejected terminal sends"
        );
    }

    #[tokio::test]
    async fn refused_observations_count_every_loss_without_logging_on_the_socket_path() {
        use tracing_subscriber::layer::SubscriberExt;
        struct Warnings {
            all: Arc<AtomicU64>,
            aggregate: Arc<AtomicU64>,
        }
        impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _: tracing_subscriber::layer::Context<'_, S>,
            ) {
                if *event.metadata().level() == tracing::Level::WARN {
                    self.all.fetch_add(1, Ordering::Relaxed);
                    if event.metadata().fields().field("total_dropped").is_some() {
                        self.aggregate.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        let all = Arc::new(AtomicU64::new(0));
        let aggregate = Arc::new(AtomicU64::new(0));
        let _subscriber =
            tracing::subscriber::set_default(tracing_subscriber::registry().with(Warnings {
                all: all.clone(),
                aggregate: aggregate.clone(),
            }));
        let jobs = Jobs::new();
        let out = Observations::new(Arc::new(crate::gatekeeper::bus::DisabledBus), &jobs);
        for _ in 0..10_000 {
            assert!(!out.publish(Event::Disconnect {
                address: "wallet".into()
            }));
        }
        assert_eq!(out.dropped(), 10_000);
        assert_eq!(
            all.load(Ordering::Relaxed),
            0,
            "socket callers must not emit per-refusal warnings"
        );
        out.shutdown().await;
        assert_eq!(
            aggregate.load(Ordering::Relaxed),
            1,
            "shutdown reports the lost delta once"
        );
    }

    #[tokio::test]
    async fn observations_preserve_every_heartbeat_disconnect_and_reinsert_in_queue_order() {
        let bus = Arc::new(InProcessBus::new());
        let out = Observations::new(bus.clone(), &Jobs::new());
        let events = vec![
            Event::Heartbeat {
                address: "MixedCase".into(),
                position: [1., 2., 3.],
            },
            Event::Heartbeat {
                address: "MixedCase".into(),
                position: [4., 5., 6.],
            },
            Event::Disconnect {
                address: "MixedCase".into(),
            },
            Event::Heartbeat {
                address: "MixedCase".into(),
                position: [-0., f64::INFINITY, 9.],
            },
        ];
        for event in &events {
            assert!(
                out.publish(event.clone()),
                "observation must enter bounded queue"
            );
        }
        out.shutdown().await;
        assert_eq!(out.dropped(), 0);
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while bus.published().len() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let messages: Vec<_> = bus
            .published()
            .into_iter()
            .map(|(subject, data)| {
                assert_eq!(subject, OBSERVATIONS_SUBJECT);
                Observation::decode(&data).unwrap()
            })
            .collect();
        for (index, message) in messages.iter().enumerate() {
            assert_eq!(message.sequence, index as u64 + 1);
            assert_eq!(message.producer, messages[0].producer);
            assert_eq!(message.event, events[index]);
        }
    }
    struct LifecycleBus {
        inner: InProcessBus,
        state: parking_lot::Mutex<crate::pulse::statistics::Statistics>,
        requests: parking_lot::Mutex<Vec<ProducerRequest>>,
        lose_confirmation: std::sync::atomic::AtomicBool,
        hold_confirmation: std::sync::atomic::AtomicBool,
        confirmation_ready: tokio::sync::Notify,
        finish_confirmation: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl NatsBus for LifecycleBus {
        fn is_enabled(&self) -> bool {
            true
        }
        fn is_connected(&self) -> bool {
            true
        }
        fn is_ready(&self) -> bool {
            true
        }
        fn connect(&self) {}
        fn subscribe(
            &self,
            subject: &str,
            queue: Option<&str>,
            handler: crate::gatekeeper::bus::NatsHandler,
        ) -> NatsSubscription {
            self.inner.subscribe(subject, queue, handler)
        }
        async fn publish(&self, subject: &str, payload: Vec<u8>) -> PublishOutcome {
            self.inner.publish(subject, payload).await
        }
        async fn request(
            &self,
            _: &str,
            _: Vec<u8>,
        ) -> Result<Vec<u8>, crate::gatekeeper::bus::RequestError> {
            Err(crate::gatekeeper::bus::RequestError::NoResponders)
        }
        async fn statistics_control(
            &self,
            payload: Vec<u8>,
        ) -> Result<Vec<u8>, crate::gatekeeper::bus::RequestError> {
            let request = ProducerRequest::decode(&payload).unwrap();
            self.requests.lock().push(request.clone());
            let confirmation = matches!(request, ProducerRequest::Confirm(_));
            let reply = self.state.lock().control(request);
            if confirmation && self.hold_confirmation.swap(false, Ordering::Relaxed) {
                self.confirmation_ready.notify_one();
                self.finish_confirmation.notified().await;
            }
            if confirmation && self.lose_confirmation.swap(false, Ordering::Relaxed) {
                return Err(crate::gatekeeper::bus::RequestError::TimedOut);
            }
            Ok(reply.encode().unwrap())
        }
        async fn flush(&self) -> bool {
            true
        }
    }
    #[tokio::test]
    async fn producer_registration_is_lazy_and_recovers_restart_without_replaying_events() {
        use crate::protocol::bus::statistics::{Snapshot, frames};
        let bus = Arc::new(LifecycleBus {
            inner: InProcessBus::new(),
            state: Default::default(),
            requests: Default::default(),
            lose_confirmation: Default::default(),
            hold_confirmation: Default::default(),
            confirmation_ready: Default::default(),
            finish_confirmation: Default::default(),
        });
        let previous = uuid::Uuid::new_v4();
        let ProducerReply::Grant(previous) = bus.state.lock().control(ProducerRequest::Begin {
            incarnation: uuid::Uuid::nil(),
            producer: previous,
        }) else {
            panic!("grant")
        };
        bus.state
            .lock()
            .control(ProducerRequest::Confirm(previous.clone()));
        let jobs = Jobs::new();
        let out = Observations::new(bus.clone(), &jobs);
        let hint = |incarnation| {
            for frame in frames(&Snapshot::default(), incarnation, 1, 1024).unwrap() {
                bus.inner.inject(SNAPSHOTS_SUBJECT, &frame);
            }
        };
        hint(uuid::Uuid::new_v4());
        tokio::task::yield_now().await;
        assert!(
            bus.requests.lock().is_empty(),
            "passive replicas never claim on construction or source hints"
        );
        let heartbeat = || Event::Heartbeat {
            address: "wallet".into(),
            position: [0.; 3],
        };
        let observed = &bus;
        let wait = |count| async move {
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while observed.inner.published().len() < count {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        };
        assert!(out.publish(heartbeat()));
        wait(1).await;
        let first = Observation::decode(&bus.inner.published()[0].1).unwrap();
        assert!(first.generation > previous.generation);
        assert_eq!(first.sequence, 1);
        let count = bus.requests.lock().len();
        hint(first.incarnation);
        assert!(out.publish(heartbeat()));
        wait(2).await;
        assert_eq!(bus.requests.lock().len(), count);
        hint(uuid::Uuid::new_v4());
        assert!(out.publish(heartbeat()));
        wait(3).await;
        assert_eq!(
            &bus.requests.lock()[count..],
            &[ProducerRequest::Status],
            "a stale hint cannot reset the sequence or reclaim authority"
        );
        assert_eq!(
            Observation::decode(&bus.inner.published()[2].1)
                .unwrap()
                .sequence,
            3
        );
        let restarted = uuid::Uuid::new_v4();
        *bus.state.lock() = crate::pulse::statistics::Statistics::with_incarnation(restarted);
        hint(restarted);
        bus.lose_confirmation.store(true, Ordering::Relaxed);
        assert!(out.publish(heartbeat()));
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while out.dropped() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            bus.inner.published().len(),
            3,
            "uncertain registration does not submit or retain the event"
        );
        let count = bus.requests.lock().len();
        assert!(out.disconnect("wallet".into()).await);
        out.shutdown().await;
        let messages = bus.inner.published();
        assert_eq!(messages.len(), 4);
        let last = Observation::decode(&messages[3].1).unwrap();
        assert_eq!(last.incarnation, restarted);
        assert_eq!(last.sequence, 1);
        assert!(matches!(last.event, Event::Disconnect { .. }));
        assert!(
            matches!(
                &bus.requests.lock()[count..],
                [ProducerRequest::Status, ProducerRequest::Confirm(_)]
            ),
            "lost confirmation retries the exact candidate, without a new generation"
        );
        assert_eq!(out.dropped(), 1);
        // A restart hint arriving while an old confirmation is held must survive
        // that confirmation; the next local event must probe the new source.
        let out = Observations::new(bus.clone(), &jobs);
        bus.hold_confirmation.store(true, Ordering::Relaxed);
        hint(restarted);
        assert!(out.publish(heartbeat()));
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            bus.confirmation_ready.notified(),
        )
        .await
        .unwrap();
        let next_source = uuid::Uuid::new_v4();
        *bus.state.lock() = crate::pulse::statistics::Statistics::with_incarnation(next_source);
        hint(next_source);
        bus.finish_confirmation.notify_one();
        wait(5).await;
        assert_eq!(
            Observation::decode(&bus.inner.published()[4].1)
                .unwrap()
                .incarnation,
            restarted
        );
        let count = bus.requests.lock().len();
        assert!(out.publish(heartbeat()));
        wait(6).await;
        let recovered = Observation::decode(&bus.inner.published()[5].1).unwrap();
        assert_eq!(
            recovered.incarnation, next_source,
            "the new hint cannot be overwritten by an earlier control result"
        );
        assert_eq!(recovered.sequence, 1);
        assert!(matches!(
            &bus.requests.lock()[count..],
            [
                ProducerRequest::Status,
                ProducerRequest::Begin { .. },
                ProducerRequest::Confirm(_)
            ]
        ));
        out.shutdown().await;
        assert_eq!(
            out.dropped(),
            0,
            "SDK submission still does not acknowledge Pulse acceptance"
        );
        assert!(
            jobs.shutdown(std::time::Duration::from_secs(1))
                .await
                .is_empty()
        );
    }
    #[tokio::test]
    async fn unavailable_link_refuses_observations_without_replay() {
        let out = Observations::new(Arc::new(crate::gatekeeper::bus::DisabledBus), &Jobs::new());
        assert!(!out.publish(Event::Disconnect {
            address: "wallet".into()
        }));
    }
    #[tokio::test]
    async fn queue_saturation_and_unavailable_shutdown_count_every_lost_observation() {
        let bus = Arc::new(InProcessBus::new());
        let jobs = Jobs::new();
        let out = Observations::new(bus.clone(), &jobs);
        for _ in 0..1024 {
            assert!(out.publish(Event::Disconnect {
                address: "wallet".into()
            }));
        }
        assert!(!out.publish(Event::Disconnect {
            address: "wallet".into()
        }));
        bus.set_connected(false);
        tokio::time::timeout(std::time::Duration::from_secs(3), out.shutdown())
            .await
            .unwrap();
        assert_eq!(out.dropped(), 1025);
        assert!(
            jobs.shutdown(std::time::Duration::from_secs(1))
                .await
                .is_empty()
        );
    }
}
