//! A WebSocket as a dRPC [`Transport`]: binary frames in and out, closed by
//! `close` or a kill handle.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::rpc::drpc::transport::{GuardedDelivery, GuardedSend, MAX_GUARDED_MESSAGE_BYTES};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::{
    Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel,
};
use tokio::sync::{Mutex, Notify};

use crate::rpc::drpc::transport::{Transport, TransportError, TransportMessage};

/// The socket as a [`Transport`]: a reader task feeds binary frames into an
/// unbounded queue (text, ping and pong are dropped), a writer task drains
/// the outbound queue; `close` (or the admin disconnect) fires `kill`, which
/// ends both.
pub struct AxumWsTransport {
    inbound_rx: Mutex<UnboundedReceiver<TransportMessage>>,
    outbound_tx: UnboundedSender<TransportMessage>,
    guarded_tx: Sender<GuardedOutbound>,
    closed: AtomicBool,
    kill: Arc<Notify>,
    address: String,
}

impl AxumWsTransport {
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn kill_handle(&self) -> Arc<Notify> {
        self.kill.clone()
    }

    pub fn spawn(socket: WebSocket, address: String) -> Self {
        let (mut sink, mut stream) = socket.split();
        let (inbound_tx, inbound_rx) = unbounded_channel::<TransportMessage>();
        let (outbound_tx, outbound_rx) = unbounded_channel::<TransportMessage>();
        let (guarded_tx, guarded_rx) = channel(GUARDED_CAPACITY);
        let kill = Arc::new(Notify::new());

        let reader_kill = kill.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = reader_kill.notified() => break,
                    msg = stream.next() => match msg {
                        Some(Ok(Message::Binary(b))) => {
                            if inbound_tx.send(b.to_vec()).is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Text(_))) | Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    }
                }
            }
        });

        let writer_kill = kill.clone();
        tokio::spawn(async move {
            run_writer(&mut sink, outbound_rx, guarded_rx, writer_kill).await;
        });

        Self {
            inbound_rx: Mutex::new(inbound_rx),
            outbound_tx,
            guarded_tx,
            closed: AtomicBool::new(false),
            kill,
            address,
        }
    }
}

#[async_trait::async_trait]
impl Transport for AxumWsTransport {
    async fn receive(&self) -> Result<TransportMessage, TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::Closed);
        }
        let mut rx = self.inbound_rx.lock().await;
        let bytes = tokio::select! {
            _ = self.kill.notified() => None,
            recv = rx.recv() => recv,
        };
        match bytes {
            Some(bytes) => Ok(bytes),
            None => {
                self.closed.store(true, Ordering::Release);
                Err(TransportError::Closed)
            }
        }
    }

    async fn send(&self, message: TransportMessage) -> Result<(), TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::Closed);
        }
        self.outbound_tx
            .send(message)
            .map_err(|_| TransportError::Closed)
    }

    async fn send_guarded(
        &self,
        message: TransportMessage,
        delivery: GuardedDelivery,
    ) -> Result<GuardedSend, TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::Closed);
        }
        if message.len() > MAX_GUARDED_MESSAGE_BYTES {
            return Ok(GuardedSend::Suppressed);
        }
        let expires_at = delivery.expires_at;
        let _cancel_on_drop = delivery.cancel.clone().drop_guard();
        let (completion, receiver) = tokio::sync::oneshot::channel();
        let envelope = GuardedOutbound {
            bytes: message,
            delivery,
            completion,
        };
        let exchange = async {
            self.guarded_tx
                .send(envelope)
                .await
                .map_err(|_| TransportError::Closed)?;
            receiver.await.map_err(|_| TransportError::Closed)?
        };
        tokio::select! {
            _ = self.kill.notified() => Err(TransportError::Closed),
            result = tokio::time::timeout_at(expires_at, exchange) => result.unwrap_or(Err(TransportError::Closed)),
        }
    }
    async fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.kill.notify_waiters();
    }
}

// Opt-in lane shares the existing writer; ordinary multiplexed procedure behavior stays intact.
const GUARDED_CAPACITY: usize = 16;
struct GuardedOutbound {
    bytes: Vec<u8>,
    delivery: GuardedDelivery,
    completion: tokio::sync::oneshot::Sender<Result<GuardedSend, TransportError>>,
}
enum Outbound {
    Ordinary(Vec<u8>),
    Guarded(GuardedOutbound),
}
async fn run_writer<S>(
    sink: &mut S,
    mut ordinary: UnboundedReceiver<Vec<u8>>,
    mut guarded: Receiver<GuardedOutbound>,
    kill: Arc<Notify>,
) where
    S: futures::Sink<Message> + Unpin,
{
    let mut prefer_guarded = true;
    loop {
        // Alternate priority when both lanes are ready: neither can indefinitely starve the other.
        let next = if prefer_guarded {
            tokio::select! { biased;
                _ = kill.notified() => None,
                item = guarded.recv(), if !guarded.is_closed() || !guarded.is_empty() => item.map(Outbound::Guarded),
                item = ordinary.recv() => item.map(Outbound::Ordinary),
            }
        } else {
            tokio::select! { biased;
                _ = kill.notified() => None,
                item = ordinary.recv(), if !ordinary.is_closed() || !ordinary.is_empty() => item.map(Outbound::Ordinary),
                item = guarded.recv() => item.map(Outbound::Guarded),
            }
        };
        let Some(next) = next else {
            break;
        };
        prefer_guarded = !prefer_guarded;
        match next {
            Outbound::Ordinary(bytes) => {
                let sent = tokio::select! { _ = kill.notified() => false, result = sink.send(Message::Binary(bytes.into())) => result.is_ok() };
                if !sent {
                    break;
                }
            }
            Outbound::Guarded(envelope) => {
                if !write_guarded(sink, envelope, &kill).await {
                    break;
                }
            }
        }
    }
    // A partial/uncertain guarded sink write poisons this socket and all queued envelopes.
    kill.notify_waiters();
    drop(guarded);
    let _ = tokio::time::timeout(std::time::Duration::from_millis(100), sink.close()).await;
}

async fn write_guarded<S>(sink: &mut S, envelope: GuardedOutbound, kill: &Notify) -> bool
where
    S: futures::Sink<Message> + Unpin,
{
    let GuardedOutbound {
        bytes,
        delivery,
        completion,
    } = envelope;
    if completion.is_closed() || delivery.cancel.is_cancelled() {
        let _ = completion.send(Ok(GuardedSend::Suppressed));
        return true;
    }
    if tokio::time::Instant::now() >= delivery.expires_at {
        let _ = completion.send(Err(TransportError::Resync));
        return true;
    }
    let eligibility = tokio::select! {
        _ = kill.notified() => None,
        _ = delivery.cancel.cancelled() => None,
        result = tokio::time::timeout_at(delivery.expires_at, (delivery.guard)()) => Some(result.unwrap_or(Err(()))),
    };
    if matches!(eligibility, Some(Err(()))) {
        let _ = completion.send(Err(TransportError::Resync));
        return true;
    }
    if tokio::time::Instant::now() >= delivery.expires_at {
        let _ = completion.send(Err(TransportError::Resync));
        return true;
    }
    if eligibility != Some(Ok(true)) || completion.is_closed() || delivery.cancel.is_cancelled() {
        let _ = completion.send(Ok(GuardedSend::Suppressed));
        return true;
    }
    // This is the irrevocable application handoff. A concurrent change after it cannot
    // revoke bytes already given to the sink/network. Any interrupted write retires it.
    let sent = tokio::select! {
        _ = kill.notified() => false,
        _ = delivery.cancel.cancelled() => false,
        result = tokio::time::timeout_at(delivery.expires_at, sink.send(Message::Binary(bytes.into()))) => matches!(result, Ok(Ok(()))),
    };
    let _ = completion.send(if sent {
        Ok(GuardedSend::Sent)
    } else {
        Err(TransportError::Closed)
    });
    sent
}
#[cfg(test)]
mod guarded_tests {
    use super::*;
    use crate::rpc::drpc::{
        messages_handlers::StreamsHandler, rpc_protocol::StreamMessage, stream_protocol::Generator,
        transport::DeliveryGuard,
    };
    use futures::task::AtomicWaker;
    use prost::Message as ProstMessage;
    use std::time::Duration;
    use std::{
        pin::Pin,
        sync::atomic::AtomicUsize,
        task::{Context, Poll},
    };

    struct SinkState {
        flush: AtomicBool,
        frames: std::sync::Mutex<Vec<Vec<u8>>>,
        wake: AtomicWaker,
        retired: AtomicBool,
    }
    impl SinkState {
        fn new(flush: bool) -> Arc<Self> {
            Arc::new(Self {
                flush: AtomicBool::new(flush),
                frames: std::sync::Mutex::new(Vec::new()),
                wake: AtomicWaker::new(),
                retired: AtomicBool::new(false),
            })
        }
        fn release(&self) {
            self.flush.store(true, Ordering::SeqCst);
            self.wake.wake();
        }
    }
    struct TestSink(Arc<SinkState>);
    impl futures::Sink<Message> for TestSink {
        type Error = std::io::Error;
        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            if let Message::Binary(bytes) = item {
                self.0.frames.lock().unwrap().push(bytes.to_vec());
            }
            Ok(())
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.0.wake.register(cx.waker());
            if self.0.flush.load(Ordering::SeqCst) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.0.retired.store(true, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
    fn transport(state: Arc<SinkState>) -> Arc<AxumWsTransport> {
        let (_, inbound_rx) = unbounded_channel();
        let (outbound_tx, ordinary) = unbounded_channel();
        let (guarded_tx, guarded) = channel(GUARDED_CAPACITY);
        let kill = Arc::new(Notify::new());
        let writer_kill = kill.clone();
        tokio::spawn(async move {
            run_writer(&mut TestSink(state), ordinary, guarded, writer_kill).await;
        });
        Arc::new(AxumWsTransport {
            inbound_rx: Mutex::new(inbound_rx),
            outbound_tx,
            guarded_tx,
            closed: AtomicBool::new(false),
            kill,
            address: String::new(),
        })
    }
    fn guard(allowed: Arc<AtomicBool>, checked: Arc<AtomicUsize>) -> DeliveryGuard {
        Arc::new(move || {
            let allowed = allowed.clone();
            let checked = checked.clone();
            Box::pin(async move {
                checked.fetch_add(1, Ordering::SeqCst);
                Ok(allowed.load(Ordering::SeqCst))
            })
        })
    }
    fn value(n: u8) -> StreamMessage {
        StreamMessage {
            payload: vec![n],
            ..Default::default()
        }
    }
    async fn settle() {
        for _ in 0..40 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ack_paced_replay_reaches_tail_across_pages_under_slow_healthy_ack() {
        let sink = SinkState::new(true);
        let output = transport(sink.clone());
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(output, encoded, 20, 20)
                .await
        });
        let completed = Arc::new(AtomicUsize::new(0));
        let count = completed.clone();
        let producer = tokio::spawn(async move {
            let rows: Vec<u32> = (0..300).collect();
            for page in rows.chunks(256) {
                for row in page {
                    let item = StreamMessage {
                        payload: row.to_le_bytes().to_vec(),
                        ..Default::default()
                    };
                    assert_eq!(
                        yielder
                            .yield_confirmed(
                                item,
                                guard(
                                    Arc::new(AtomicBool::new(true)),
                                    Arc::new(AtomicUsize::new(0))
                                )
                            )
                            .await
                            .unwrap(),
                        crate::rpc::drpc::stream_protocol::DeliveryCompletion::Acknowledged
                    );
                    count.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        for n in 0..300 {
            settle().await;
            assert_eq!(
                sink.frames.lock().unwrap().len(),
                n + 1,
                "one admitted replay item, no expiring34-item burst"
            );
            let bytes = sink.frames.lock().unwrap()[n].clone();
            let outer = StreamMessage::decode(bytes.as_slice()).unwrap();
            let inner = StreamMessage::decode(outer.payload.as_slice()).unwrap();
            assert_eq!(
                u32::from_le_bytes(inner.payload.try_into().unwrap()),
                n as u32
            );
            tokio::time::advance(Duration::from_secs(1)).await;
            handler.complete_test_ack(
                20,
                20,
                n as u32 + 1,
                StreamMessage {
                    ack: true,
                    ..Default::default()
                }
                .encode_to_vec(),
            );
        }
        settle().await;
        assert_eq!(completed.load(Ordering::SeqCst), 300);
        assert!(producer.is_finished());
        assert!(task.await.unwrap().is_ok());
        let terminal = sink.frames.lock().unwrap().last().unwrap().clone();
        assert!(StreamMessage::decode(terminal.as_slice()).unwrap().closed);
    }

    #[tokio::test(start_paused = true)]
    async fn clean_terminal_retries_pinned_client_loop_and_replay_repairs_lost_offline_on_same_socket()
     {
        let sink = SinkState::new(true);
        let output = transport(sink.clone());
        let handler = Arc::new(StreamsHandler::new());
        let mut retained_online = true;
        let mut calls = 0;
        // Unity's source-pinned loop retries clean end, but its tracker does not Reset.
        for unknown in [true, false] {
            calls += 1;
            let start = sink.frames.lock().unwrap().len();
            let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
            let encoded =
                Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
            let sender = handler.clone();
            let connection = output.clone();
            let task = tokio::spawn(async move {
                sender
                    .send_streams_through_transport(connection, encoded, 21, 21)
                    .await
            });
            let eligibility: DeliveryGuard = if unknown {
                Arc::new(|| Box::pin(async { Err(()) }))
            } else {
                guard(
                    Arc::new(AtomicBool::new(true)),
                    Arc::new(AtomicUsize::new(0)),
                )
            };
            yielder.yield_guarded(value(1), eligibility).await.unwrap();
            settle().await;
            if !unknown {
                let frame = sink.frames.lock().unwrap()[start].clone();
                let frame = StreamMessage::decode(frame.as_slice()).unwrap();
                let offline = StreamMessage::decode(frame.payload.as_slice()).unwrap();
                retained_online = offline.payload[0] == 0; // existing enum ONLINE=0, OFFLINE=1
                handler.complete_test_ack(
                    21,
                    21,
                    1,
                    StreamMessage {
                        ack: true,
                        ..Default::default()
                    }
                    .encode_to_vec(),
                );
                drop(yielder);
                settle().await;
            }
            assert!(
                task.await.unwrap().is_ok(),
                "no generic Transport RemoteError on retryable failure"
            );
            let terminal = sink.frames.lock().unwrap().last().unwrap().clone();
            assert!(
                StreamMessage::decode(terminal.as_slice()).unwrap().closed,
                "pinned KeepServerStreamOpen loop calls again only after clean end"
            );
            assert!(
                !output.closed.load(Ordering::Acquire),
                "same healthy socket, no implicit tracker reset"
            );
        }
        assert_eq!(calls, 2);
        assert!(!retained_online);
        assert_eq!(handler.pending_ack_count(), 0);
        output.close().await;
    }

    #[tokio::test(start_paused = true)]
    async fn unconfirmed_terminal_writer_stall_retires_socket_instead_of_stranding_subscription() {
        let sink = SinkState::new(false);
        let output = transport(sink.clone());
        output.send(vec![99]).await.unwrap();
        settle().await;
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let connection = output.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(connection, encoded, 22, 22)
                .await
        });
        drop(yielder);
        settle().await;
        tokio::time::advance(Duration::from_millis(100)).await;
        settle().await;
        assert!(task.await.unwrap().is_ok());
        assert!(output.closed.load(Ordering::Acquire));
        assert!(sink.retired.load(Ordering::Acquire));
        assert_eq!(handler.pending_ack_count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn already_expired_or_guard_timeout_requires_resync_not_silent_suppression() {
        for expired in [true, false] {
            let state = SinkState::new(true);
            let mut sink = TestSink(state.clone());
            let (completion, receiver) = tokio::sync::oneshot::channel();
            let envelope = GuardedOutbound {
                bytes: vec![1],
                delivery: GuardedDelivery {
                    guard: Arc::new(|| Box::pin(std::future::pending())),
                    expires_at: tokio::time::Instant::now()
                        + if expired {
                            Duration::ZERO
                        } else {
                            Duration::from_secs(5)
                        },
                    cancel: tokio_util::sync::CancellationToken::new(),
                },
                completion,
            };
            assert!(write_guarded(&mut sink, envelope, &Notify::new()).await);
            assert!(
                receiver.await.unwrap().is_err(),
                "expired/unknown delivery ends affected stream for resync"
            );
            assert!(state.frames.lock().unwrap().is_empty());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn unknown_final_guard_ends_stream_for_resync_instead_of_losing_one_status() {
        let sink = SinkState::new(true);
        let output = transport(sink.clone());
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(output, encoded, 6, 6)
                .await
        });
        yielder
            .yield_guarded(value(1), Arc::new(|| Box::pin(async { Err(()) })))
            .await
            .unwrap();
        settle().await;
        assert!(
            task.await.unwrap().is_ok(),
            "retryable failure is a clean end, not Transport RemoteError"
        );
        let frames = sink.frames.lock().unwrap();
        assert_eq!(frames.len(), 1);
        assert!(
            StreamMessage::decode(frames[0].as_slice()).unwrap().closed,
            "pinned client retries this terminal frame"
        );
        assert_eq!(handler.pending_ack_count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn many_streams_share_sixteen_guarded_slots_and_encoded_byte_cap() {
        let sink = SinkState::new(false);
        let output = transport(sink.clone());
        output.send(vec![99]).await.unwrap();
        settle().await;
        let mut tasks = Vec::new();
        for _ in 0..64 {
            let output = output.clone();
            tasks.push(tokio::spawn(async move {
                output
                    .send_guarded(
                        vec![1],
                        GuardedDelivery {
                            guard: guard(
                                Arc::new(AtomicBool::new(true)),
                                Arc::new(AtomicUsize::new(0)),
                            ),
                            expires_at: tokio::time::Instant::now() + Duration::from_secs(5),
                            cancel: tokio_util::sync::CancellationToken::new(),
                        },
                    )
                    .await
            }));
        }
        settle().await;
        assert_eq!(output.guarded_tx.capacity(), 0);
        assert_eq!(output.guarded_tx.max_capacity(), 16);
        let (_, yielder) = Generator::bounded(16, Duration::from_secs(5));
        assert!(
            yielder
                .yield_guarded(
                    StreamMessage {
                        payload: vec![0; MAX_GUARDED_MESSAGE_BYTES],
                        ..Default::default()
                    },
                    guard(
                        Arc::new(AtomicBool::new(true)),
                        Arc::new(AtomicUsize::new(0))
                    )
                )
                .await
                .is_err()
        );
        tokio::time::advance(Duration::from_secs(5)).await;
        settle().await;
        assert!(tasks.iter().all(|task| task.is_finished()));
        sink.release();
        settle().await;
        assert_eq!(
            *sink.frames.lock().unwrap(),
            vec![vec![99]],
            "cancelled expired queue entries cannot write later"
        );
        output.close().await;
    }

    #[tokio::test(start_paused = true)]
    async fn fast_ack_and_ack_before_close_precedence_are_unchanged_for_both_policies() {
        for bounded in [false, true] {
            let sink = SinkState::new(true);
            let output = transport(sink.clone());
            let (source, yielder) = if bounded {
                Generator::bounded(16, Duration::from_secs(5))
            } else {
                Generator::create()
            };
            let encoded =
                Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
            let handler = Arc::new(StreamsHandler::new());
            let sender = handler.clone();
            let task = tokio::spawn(async move {
                sender
                    .send_streams_through_transport(output, encoded, 4, 4)
                    .await
            });
            for n in 1..=2 {
                if bounded {
                    yielder
                        .yield_guarded(
                            value(n),
                            guard(
                                Arc::new(AtomicBool::new(true)),
                                Arc::new(AtomicUsize::new(0)),
                            ),
                        )
                        .await
                        .unwrap();
                } else {
                    yielder.r#yield(value(n)).await.unwrap();
                }
            }
            settle().await;
            handler.complete_test_ack(
                4,
                4,
                1,
                StreamMessage {
                    ack: true,
                    closed: true,
                    ..Default::default()
                }
                .encode_to_vec(),
            );
            settle().await;
            assert_eq!(
                sink.frames.lock().unwrap().len(),
                2,
                "ACK takes precedence over closed=true"
            );
            handler.complete_test_ack(
                4,
                4,
                2,
                StreamMessage {
                    ack: false,
                    closed: true,
                    ..Default::default()
                }
                .encode_to_vec(),
            );
            settle().await;
            assert!(task.is_finished());
            assert_eq!(handler.pending_ack_count(), 0);
            assert!(task.await.unwrap().is_ok());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn malformed_ack_releases_guarded_reservation() {
        let sink = SinkState::new(true);
        let output = transport(sink);
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(output, encoded, 5, 5)
                .await
        });
        yielder
            .yield_guarded(
                value(1),
                guard(
                    Arc::new(AtomicBool::new(true)),
                    Arc::new(AtomicUsize::new(0)),
                ),
            )
            .await
            .unwrap();
        settle().await;
        handler.complete_test_ack(5, 5, 1, vec![0xff]);
        settle().await;
        assert!(task.await.unwrap().is_err());
        assert_eq!(handler.pending_ack_count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn mapped_connectivity_rechecks_revocation_after_stalled_physical_writer_dequeue() {
        let sink = SinkState::new(false);
        let transport = transport(sink.clone());
        transport.send(vec![99]).await.unwrap();
        settle().await; // ordinary write holds the same physical writer
        let allowed = Arc::new(AtomicBool::new(true));
        let checked = Arc::new(AtomicUsize::new(0));
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let output = transport.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(output, encoded, 1, 1)
                .await
        });
        yielder
            .yield_guarded(value(1), guard(allowed.clone(), checked.clone()))
            .await
            .unwrap();
        settle().await;
        assert_eq!(
            checked.load(Ordering::SeqCst),
            0,
            "encoder cannot consume the final fence"
        );
        allowed.store(false, Ordering::SeqCst); // models committed block or replaced topology before dequeue
        sink.release();
        settle().await;
        assert_eq!(checked.load(Ordering::SeqCst), 1);
        assert_eq!(
            *sink.frames.lock().unwrap(),
            vec![vec![99]],
            "revoked staged ONLINE never reaches sink"
        );
        assert_eq!(
            handler.pending_ack_count(),
            0,
            "suppression releases reservation without impossible ACK wait"
        );
        task.abort();
        settle().await;
        transport.close().await;
    }

    #[tokio::test(start_paused = true)]
    async fn stopped_ack_bounds_mapping_and_cleans_listener_and_staged_values() {
        let sink = SinkState::new(true);
        let transport = transport(sink.clone());
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let output = transport.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(output, encoded, 2, 2)
                .await
        });
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        let producer = tokio::spawn(async move {
            for _ in 0..1000 {
                if yielder
                    .yield_guarded(
                        value(1),
                        guard(
                            Arc::new(AtomicBool::new(true)),
                            Arc::new(AtomicUsize::new(0)),
                        ),
                    )
                    .await
                    .is_err()
                {
                    break;
                }
                count.fetch_add(1, Ordering::SeqCst);
            }
        });
        settle().await;
        assert!(
            accepted.load(Ordering::SeqCst) <= 35,
            "typed16 + mapper1 + encoded16 + sender1 are bounded"
        );
        assert_eq!(handler.pending_ack_count(), 1);
        tokio::time::advance(Duration::from_secs(5)).await;
        settle().await;
        assert!(task.is_finished());
        assert_eq!(handler.pending_ack_count(), 0);
        assert!(
            producer.is_finished(),
            "mapper and source release after downstream deadline"
        );
        transport.close().await;
    }

    #[tokio::test(start_paused = true)]
    async fn interrupted_physical_send_retires_socket_and_cannot_replay_queued_value() {
        let sink = SinkState::new(false);
        let transport = transport(sink.clone());
        let first = transport.clone();
        let second = transport.clone();
        let delivery = || GuardedDelivery {
            guard: guard(
                Arc::new(AtomicBool::new(true)),
                Arc::new(AtomicUsize::new(0)),
            ),
            expires_at: tokio::time::Instant::now() + Duration::from_secs(5),
            cancel: tokio_util::sync::CancellationToken::new(),
        };
        let a = tokio::spawn(async move { first.send_guarded(vec![1], delivery()).await });
        settle().await;
        let b = tokio::spawn(async move { second.send_guarded(vec![2], delivery()).await });
        settle().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        settle().await;
        sink.release();
        settle().await;
        assert!(sink.retired.load(Ordering::SeqCst));
        assert_eq!(
            *sink.frames.lock().unwrap(),
            vec![vec![1]],
            "possibly partial first frame poisons sink before second"
        );
        assert!(a.is_finished() && b.is_finished());
        assert!(transport.guarded_tx.is_closed());
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_send_task_releases_ack_reservation() {
        let sink = SinkState::new(true);
        let transport = transport(sink);
        let (source, yielder) = Generator::bounded(16, Duration::from_secs(5));
        let encoded = Generator::from_generator(source, |v: StreamMessage| Some(v.encode_to_vec()));
        let handler = Arc::new(StreamsHandler::new());
        let sender = handler.clone();
        let task = tokio::spawn(async move {
            sender
                .send_streams_through_transport(transport, encoded, 3, 3)
                .await
        });
        yielder
            .yield_guarded(
                value(1),
                guard(
                    Arc::new(AtomicBool::new(true)),
                    Arc::new(AtomicUsize::new(0)),
                ),
            )
            .await
            .unwrap();
        settle().await;
        assert_eq!(handler.pending_ack_count(), 1);
        task.abort();
        settle().await;
        assert_eq!(handler.pending_ack_count(), 0);
    }
}
