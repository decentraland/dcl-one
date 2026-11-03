//! Opt-in ordinary-message queue for the existing RPC writer.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::sync::{Notify, mpsc::UnboundedSender, oneshot};
use tokio::time::Instant;

use super::{ByteBudget, OrdinaryOutbound, RPC_TRANSPORT_CLOSED, SocketClose};

/// Source UWebSocketTransport numeric settings. Other users of the shared
/// transport keep the legacy writer unless they explicitly supply these.
#[derive(Clone, Copy, Debug)]
pub struct OrdinaryQueueConfig {
    pub max_queue_size: f64,
    pub min_queue_size: f64,
    pub max_queue_size_limit: f64,
    pub estimated_message_size: f64,
    pub retry_delay_ms: f64,
    pub max_retry_attempts: f64,
    pub max_backoff_delay_ms: f64,
}
impl Default for OrdinaryQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 1000.0,
            min_queue_size: 100.0,
            max_queue_size_limit: 5000.0,
            estimated_message_size: 1024.0,
            retry_delay_ms: 1000.0,
            max_retry_attempts: 5.0,
            max_backoff_delay_ms: 30000.0,
        }
    }
}
impl OrdinaryQueueConfig {
    pub fn capacity(self, max_backpressure: f64) -> Result<f64, &'static str> {
        native_backpressure_limit(max_backpressure)?;
        let estimate = (max_backpressure / self.estimated_message_size).ceil();
        let adaptive = js_max(
            self.min_queue_size,
            js_min(self.max_queue_size_limit, estimate),
        );
        let capacity = js_min(self.max_queue_size, adaptive);
        if capacity.is_finite() {
            Ok(capacity)
        } else {
            Err("WS_TRANSPORT settings must produce a finite ordinary message queue capacity")
        }
    }
}
/// uWS assigns Int32Value to its unsigned-32-bit maxBackpressure field. The
/// ordinary queue separately consumes the original number before conversion.
pub fn native_backpressure_limit(number: f64) -> Result<usize, &'static str> {
    if !number.is_finite() {
        return Err("WS_MAX_BACKPRESSURE must be finite for the bounded RPC writer");
    }
    let bytes = number.trunc().rem_euclid(4294967296.0) as u32;
    if bytes == 0 {
        return Err(
            "WS_MAX_BACKPRESSURE resolves to zero; zero disables the source native buffer limit and is unsupported",
        );
    }
    Ok(bytes as usize)
}
fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}
fn timer_delay(ms: f64) -> Duration {
    Duration::from_millis(if (1.0..=i32::MAX as f64).contains(&ms) {
        ms.trunc() as u64
    } else {
        1
    })
}

pub(super) enum SendOutcome {
    Code(i32),
    Failure(String),
}
struct Pending {
    bytes: Vec<u8>,
    completion: oneshot::Sender<Result<(), String>>,
    attempts: u32,
}
struct Queue {
    config: OrdinaryQueueConfig,
    capacity: f64,
    pending: VecDeque<Pending>,
    retry: Option<Instant>,
    active: bool,
    errors: Vec<String>,
    #[cfg(test)]
    metrics: Vec<serde_json::Value>,
}
impl Queue {
    fn new(config: OrdinaryQueueConfig, max_backpressure: f64) -> Self {
        Self {
            config,
            capacity: config
                .capacity(max_backpressure)
                .expect("validated ordinary queue"),
            pending: VecDeque::new(),
            retry: None,
            active: true,
            errors: Vec::new(),
            #[cfg(test)]
            metrics: Vec::new(),
        }
    }
    fn counter(&mut self, name: &'static str, result: Option<&'static str>) {
        if let Some(result) = result {
            metrics::counter!(name,"result"=>result).increment(1);
        } else {
            metrics::counter!(name).increment(1);
        }
        #[cfg(test)]
        self.metrics.push(serde_json::json!({"name":name,"labels":result.map(|result|serde_json::json!({"result":result})).unwrap_or_else(||serde_json::json!({}))}));
    }
    fn close(&mut self) {
        self.active = false;
        self.retry = None;
        for item in self.pending.drain(..) {
            let _ = item.completion.send(Err("Connection closed".into()));
        }
    }
    fn fail(&mut self, error: &str) {
        self.errors.push(error.into());
        self.close();
    }
    fn enqueue(
        &mut self,
        bytes: Vec<u8>,
        connected: bool,
    ) -> oneshot::Receiver<Result<(), String>> {
        let (completion, receiver) = oneshot::channel();
        if !self.active || !connected {
            let _ = completion.send(Ok(()));
        } else if self.pending.len() as f64 >= self.capacity {
            self.fail("Message queue size limit reached");
            // The overflowing source send returns an already resolved promise.
            let _ = completion.send(Ok(()));
        } else {
            self.pending.push_back(Pending {
                bytes,
                completion,
                attempts: 0,
            });
        }
        receiver
    }
    fn process(
        &mut self,
        now: Instant,
        mut send: impl FnMut(&mut Vec<u8>) -> SendOutcome,
        mut buffered: impl FnMut() -> usize,
    ) {
        if !self.active || self.retry.is_some_and(|retry| now < retry) {
            return;
        }
        self.retry = None;
        while !self.pending.is_empty() {
            let queued = self.pending.len();
            let item = self.pending.front_mut().unwrap();
            if item.attempts as f64 >= self.config.max_retry_attempts {
                self.counter("ws_backpressure_events", Some("max_retries"));
                self.fail("Message not deliverable after max retries");
                return;
            }
            let size = item.bytes.len();
            let outcome = send(&mut item.bytes);
            if matches!(outcome, SendOutcome::Code(_)) {
                let pending_bytes = buffered();
                if pending_bytes > 0 {
                    metrics::gauge!("ws_queue_vs_backpressure_ratio").set(
                        queued as f64 / (pending_bytes as f64 / self.config.estimated_message_size),
                    );
                }
            }
            if let SendOutcome::Code(code @ (0..=2)) = &outcome {
                let result = match code {
                    0 => "backpressure",
                    1 => "success",
                    _ => "dropped",
                };
                metrics::histogram!("ws_message_size_bytes","result"=>result).record(size as f64);
            }
            let attempts = match outcome {
                SendOutcome::Code(1) => {
                    let item = self.pending.pop_front().unwrap();
                    let _ = item.completion.send(Ok(()));
                    self.counter("ws_messages_sent", None);
                    continue;
                }
                SendOutcome::Code(0) => {
                    let item = self.pending.pop_front().unwrap();
                    let _ = item.completion.send(Ok(()));
                    self.counter("ws_messages_sent", None);
                    self.counter("ws_backpressure_events", Some("backpressure"));
                    0
                }
                SendOutcome::Code(2) => {
                    item.attempts = item.attempts.saturating_add(1);
                    let attempts = item.attempts;
                    self.counter("ws_backpressure_events", Some("dropped"));
                    attempts
                }
                SendOutcome::Code(_) => {
                    let attempts = item.attempts;
                    self.counter("ws_unexpected_send_result_events", None);
                    attempts
                }
                SendOutcome::Failure(error) => {
                    let message = format!("Failed to send message: {error}");
                    let item = self.pending.pop_front().unwrap();
                    let _ = item.completion.send(Err(message.clone()));
                    self.fail(&message);
                    return;
                }
            };
            let ms = js_min(
                self.config.retry_delay_ms * 2.0_f64.powf(attempts as f64),
                self.config.max_backoff_delay_ms,
            );
            self.retry = Some(now + timer_delay(ms));
            return;
        }
    }
}

/// Only pending messages remain here; accepted bytes use the existing physical
/// writer and retain a permit until its flush completes. No forwarding queue
/// exists before this admission check.
pub(super) struct OrdinaryLane {
    queue: Mutex<Queue>,
    changed: Arc<Notify>,
    output: UnboundedSender<OrdinaryOutbound>,
    budget: Arc<ByteBudget>,
    close: Arc<SocketClose>,
}
impl OrdinaryLane {
    pub(super) fn new(
        config: OrdinaryQueueConfig,
        output: UnboundedSender<OrdinaryOutbound>,
        budget: Arc<ByteBudget>,
        close: Arc<SocketClose>,
        max_backpressure: f64,
    ) -> Arc<Self> {
        let lane = Arc::new(Self {
            queue: Mutex::new(Queue::new(config, max_backpressure)),
            changed: Arc::new(Notify::new()),
            output,
            budget,
            close,
        });
        tokio::spawn(Self::run(
            Arc::downgrade(&lane),
            lane.changed.clone(),
            lane.budget.clone(),
            lane.close.clone(),
        ));
        lane
    }
    fn process(&self, queue: &mut Queue) {
        if self.close.is_ended() {
            queue.close();
            return;
        }
        // A flush can precede the worker's first poll. The empty-budget
        // snapshot also supplies that drain, so a lost Notify cannot delay
        // a healthy socket until its previous backoff timer.
        if self.budget.used() == 0 {
            queue.retry = None;
        }
        queue.process(
            Instant::now(),
            |bytes| {
                // uWS checks bytes already buffered, allowing one frame to cross
                // the threshold. A zero-byte frame still costs one admission byte.
                let Some((permit, crossing)) = self.budget.try_reserve_buffered(bytes.len().max(1))
                else {
                    return SendOutcome::Code(2);
                };
                let envelope = OrdinaryOutbound {
                    bytes: std::mem::take(bytes),
                    _permit: Some(permit),
                };
                if self.output.send(envelope).is_err() {
                    return SendOutcome::Failure("writer closed".into());
                }
                SendOutcome::Code(if crossing { 0 } else { 1 })
            },
            || self.budget.used(),
        );
        if !queue.active {
            for error in queue.errors.drain(..) {
                tracing::warn!(%error, "ordinary RPC transport queue closed");
            }
            self.close
                .end(RPC_TRANSPORT_CLOSED.0, RPC_TRANSPORT_CLOSED.1);
        }
    }
    pub(super) fn send(&self, bytes: Vec<u8>) -> oneshot::Receiver<Result<(), String>> {
        let mut queue = self.queue.lock().unwrap();
        let receiver = queue.enqueue(bytes, !self.close.is_ended());
        self.process(&mut queue);
        self.changed.notify_one();
        receiver
    }
    pub(super) fn close(&self) {
        self.queue.lock().unwrap().close();
        self.changed.notify_one();
    }
    async fn run(
        weak: Weak<Self>,
        changed: Arc<Notify>,
        budget: Arc<ByteBudget>,
        close: Arc<SocketClose>,
    ) {
        loop {
            // Register drain before checking state so a physical flush cannot
            // be lost between observing bytes and waiting for its notification.
            let drained = budget.changed();
            tokio::pin!(drained);
            drained.as_mut().enable();
            let Some(lane) = weak.upgrade() else {
                return;
            };
            let deadline = {
                let mut queue = lane.queue.lock().unwrap();
                lane.process(&mut queue);
                if !queue.active {
                    return;
                }
                queue.retry
            };
            drop(lane);
            tokio::select! {
                _ = close.ended() => { if let Some(lane) = weak.upgrade() { lane.close(); } return; },
                _ = changed.notified() => {},
                _ = &mut drained => {
                    metrics::counter!("ws_drain_events").increment(1);
                    if let Some(lane) = weak.upgrade() { lane.queue.lock().unwrap().retry = None; }
                },
                _ = async { if let Some(deadline) = deadline { tokio::time::sleep_until(deadline).await; } else { std::future::pending::<()>().await; } } => {},
            }
        }
    }
}
impl Drop for OrdinaryLane {
    fn drop(&mut self) {
        self.queue.get_mut().unwrap().close();
        self.changed.notify_one();
    }
}

#[cfg(test)]
mod tests;
