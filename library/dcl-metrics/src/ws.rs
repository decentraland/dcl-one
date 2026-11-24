//! WebSocket transport metrics. Text and binary payloads are counted once on read
//! or on flush. Control frames and application queues have separate semantics.

use crate::{CounterFamily, Descriptor, GaugeFamily, HistogramFamily};
use axum::extract::ws::{Message, WebSocket};
use futures::{Sink, Stream};
use metrics::{Counter, Gauge, Histogram, Unit};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

const ACTIVE: GaugeFamily<1> = GaugeFamily::new(
    "dcl_ws_active_connections",
    "Established WebSocket transports.",
    None,
    &["protocol"],
);
const DURATION: HistogramFamily<1> = HistogramFamily::new(
    "dcl_ws_connection_duration_seconds",
    "Established WebSocket transport lifetime.",
    Some(Unit::Seconds),
    &["protocol"],
    &[
        1., 5., 10., 30., 60., 300., 600., 1800., 3600., 7200., 14400.,
    ],
);
const MESSAGE_LABELS: &[&str; 3] = &["protocol", "direction", "outcome"];
const MESSAGES: CounterFamily<3> = CounterFamily::new(
    "dcl_ws_messages_total",
    "Application WebSocket messages by transport outcome.",
    None,
    MESSAGE_LABELS,
);
const BYTES: CounterFamily<3> = CounterFamily::new(
    "dcl_ws_message_bytes_total",
    "Application WebSocket payload bytes by transport outcome.",
    Some(Unit::Bytes),
    MESSAGE_LABELS,
);
const ERRORS: CounterFamily<2> = CounterFamily::new(
    "dcl_ws_transport_errors_total",
    "WebSocket transport errors by operation.",
    None,
    &["protocol", "operation"],
);
const REJECTIONS: CounterFamily<2> = CounterFamily::new(
    "dcl_ws_admission_rejections_total",
    "WebSocket admissions refused by the server.",
    None,
    &["protocol", "reason"],
);
pub const FAMILIES: &[Descriptor] = &[
    ACTIVE.descriptor,
    DURATION.descriptor,
    MESSAGES.descriptor,
    BYTES.descriptor,
    ERRORS.descriptor,
    REJECTIONS.descriptor,
];

/// Record an application admission decision. Labels must come from constants.
pub fn rejected(protocol: &'static str, reason: &'static str) {
    REJECTIONS.handle([protocol, reason]).increment(1);
}

struct Traffic {
    messages: Counter,
    bytes: Counter,
}
impl Traffic {
    fn new(protocol: &'static str, direction: &'static str, outcome: &'static str) -> Self {
        let labels = [protocol, direction, outcome];
        Self {
            messages: MESSAGES.handle(labels),
            bytes: BYTES.handle(labels),
        }
    }
    fn record(&self, count: u64, bytes: u64) {
        if count > 0 {
            self.messages.increment(count);
            self.bytes.increment(bytes);
        }
    }
}

fn payload_len(message: &Message) -> Option<u64> {
    match message {
        Message::Text(text) => Some(text.len() as u64),
        Message::Binary(bytes) => Some(bytes.len() as u64),
        _ => None,
    }
}

/// Wrap the socket once, inside the successful upgrade callback, before splitting
/// its read and write halves. Outbound successes require a successful flush.
/// A dropped transport reports unconfirmed buffered messages as abandoned.
pub struct Socket<S = WebSocket> {
    inner: S,
    started: Instant,
    active: Gauge,
    duration: Histogram,
    received: Traffic,
    flushed: Traffic,
    failed: Traffic,
    abandoned: Traffic,
    pending_messages: u64,
    pending_bytes: u64,
    protocol: &'static str,
}

impl<S> Socket<S> {
    pub fn new(inner: S, protocol: &'static str) -> Self {
        let active = ACTIVE.handle([protocol]);
        active.increment(1.);
        Self {
            inner,
            protocol,
            active,
            started: Instant::now(),
            duration: DURATION.handle([protocol]),
            received: Traffic::new(protocol, "in", "received"),
            flushed: Traffic::new(protocol, "out", "flushed"),
            failed: Traffic::new(protocol, "out", "failed"),
            abandoned: Traffic::new(protocol, "out", "abandoned"),
            pending_messages: 0,
            pending_bytes: 0,
        }
    }

    fn error(&self, operation: &'static str) {
        ERRORS.handle([self.protocol, operation]).increment(1);
    }
    fn finish_pending(&mut self, success: bool) {
        let traffic = if success { &self.flushed } else { &self.failed };
        traffic.record(self.pending_messages, self.pending_bytes);
        self.pending_messages = 0;
        self.pending_bytes = 0;
    }
}

impl<S> Drop for Socket<S> {
    fn drop(&mut self) {
        self.abandoned
            .record(self.pending_messages, self.pending_bytes);
        self.duration.record(self.started.elapsed().as_secs_f64());
        self.active.decrement(1.);
    }
}

impl<S, E> Stream for Socket<S>
where
    S: Stream<Item = Result<Message, E>> + Unpin,
{
    type Item = Result<Message, E>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.inner).poll_next(cx);
        match &result {
            Poll::Ready(Some(Ok(message))) => {
                if let Some(bytes) = payload_len(message) {
                    self.received.record(1, bytes);
                }
            }
            Poll::Ready(Some(Err(_))) => self.error("read"),
            _ => {}
        }
        result
    }
}

impl<S: Sink<Message> + Unpin> Sink<Message> for Socket<S> {
    type Error = S::Error;
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let result = Pin::new(&mut self.inner).poll_ready(cx);
        if let Poll::Ready(Err(_)) = &result {
            self.error("ready");
            self.finish_pending(false);
        }
        result
    }
    fn start_send(mut self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
        let bytes = payload_len(&message);
        let result = Pin::new(&mut self.inner).start_send(message);
        if let Some(bytes) = bytes {
            self.pending_messages += 1;
            self.pending_bytes += bytes;
        }
        if result.is_err() {
            self.error("send");
            self.finish_pending(false);
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        if let Poll::Ready(outcome) = &result {
            if outcome.is_err() {
                self.error("flush");
            }
            self.finish_pending(outcome.is_ok());
        }
        result
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let result = Pin::new(&mut self.inner).poll_close(cx);
        if let Poll::Ready(outcome) = &result {
            if outcome.is_err() {
                self.error("close");
            }
            self.finish_pending(outcome.is_ok());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt, executor::block_on, sink, stream};
    use metrics_exporter_prometheus::PrometheusBuilder;

    #[test]
    fn counts_only_application_data_and_records_the_final_write_outcome() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            block_on(async {
                let reader = stream::iter(vec![
                    Ok::<_, ()>(Message::text("hello")),
                    Ok(Message::Ping(Default::default())),
                ]);
                let writer = sink::unfold((), |(), frame: Message| async move {
                    if frame == Message::text("fail") {
                        Err(())
                    } else {
                        Ok(())
                    }
                });
                let mut incoming = Socket::new(reader, "test");
                let mut socket = Socket::new(Box::pin(writer), "test");
                assert!(incoming.next().await.is_some());
                assert!(incoming.next().await.is_some());
                socket.feed(Message::text("sent")).await.unwrap();
                socket
                    .send(Message::Pong(Default::default()))
                    .await
                    .unwrap();
                assert!(socket.send(Message::text("fail")).await.is_err());
            })
        });
        metrics::with_local_recorder(&recorder, || {
            block_on(async {
                let mut socket = Socket::new(sink::drain(), "test");
                socket.feed(Message::text("pending")).await.unwrap();
            })
        });
        let output = handle.render();
        let sample = |family: &str, labels: &[&str], value: u64| {
            assert!(
                output
                    .lines()
                    .any(|line| line.starts_with(&format!("{family}{{"))
                        && labels.iter().all(|label| line.contains(label))
                        && line.ends_with(&format!(" {value}"))),
                "missing {family} {labels:?} {value} in {output}"
            );
        };
        for (outcome, bytes) in [
            ("received", 5),
            ("flushed", 4),
            ("failed", 4),
            ("abandoned", 7),
        ] {
            let label = format!("outcome=\"{outcome}\"");
            sample(MESSAGES.descriptor.name, &[&label], 1);
            sample(BYTES.descriptor.name, &[&label], bytes);
        }
        sample(ERRORS.descriptor.name, &["operation=\"flush\""], 1);
        sample(ACTIVE.descriptor.name, &[], 0);
        sample("dcl_ws_connection_duration_seconds_count", &[], 3);
    }
}
