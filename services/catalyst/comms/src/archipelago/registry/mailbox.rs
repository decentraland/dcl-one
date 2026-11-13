use super::SocketEvent;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Notify, mpsc::error::TryRecvError};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub const MAX_ASSIGNMENT_BYTES: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AssignmentSend {
    Queued,
    Full,
    Closed,
    TooLarge,
}

pub(crate) const OUTPUT_CAPACITY: usize = 64;
pub(crate) const FULL_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Occupancy {
    used: usize,
    full_since: Option<Instant>,
}

/// One credit follows each accepted item through validation and physical flush.
#[derive(Default)]
pub(crate) struct OutputBudget {
    occupancy: Mutex<Occupancy>,
    pub(crate) changed: Notify,
}

impl OutputBudget {
    #[cfg(feature = "test-support")]
    pub(crate) fn available(&self) -> usize {
        OUTPUT_CAPACITY - self.occupancy.lock().used
    }

    pub(crate) fn reserve(self: &Arc<Self>) -> Option<OutputCredit> {
        let mut occupancy = self.occupancy.lock();
        if occupancy.used == OUTPUT_CAPACITY {
            return None;
        }
        occupancy.used += 1;
        if occupancy.used == OUTPUT_CAPACITY {
            occupancy.full_since = Some(Instant::now());
        }
        drop(occupancy);
        self.changed.notify_waiters();
        Some(OutputCredit(self.clone()))
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.occupancy
            .lock()
            .full_since
            .map(|at| at + FULL_DEADLINE)
    }

    pub(crate) fn full_expired(&self, now: Instant) -> bool {
        self.deadline().is_some_and(|at| now >= at)
    }
}

pub(crate) struct OutputCredit(Arc<OutputBudget>);

impl Drop for OutputCredit {
    fn drop(&mut self) {
        let mut occupancy = self.0.occupancy.lock();
        occupancy.used -= 1;
        occupancy.full_since = None;
        drop(occupancy);
        self.0.changed.notify_waiters();
    }
}

pub(crate) struct ReceivedSocketEvent {
    pub(crate) event: SocketEvent,
    pub(crate) credit: Option<OutputCredit>,
}

#[derive(Default)]
struct Pending {
    assignments: VecDeque<(
        Box<[u8]>,
        Option<crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
        OutputCredit,
    )>,
    kicked: bool,
}

#[derive(Default)]
struct Mailbox {
    pending: Mutex<Pending>,
    budget: Arc<OutputBudget>,
    changed: Notify,
    closed: CancellationToken,
    kicked: std::sync::atomic::AtomicBool,
}

impl Mailbox {
    fn close(&self) {
        let mut pending = self.pending.lock();
        let assignments = std::mem::take(&mut pending.assignments);
        self.closed.cancel();
        drop(pending);
        drop(assignments);
        self.changed.notify_one();
    }
}

pub(super) struct MailboxSender(Arc<Mailbox>);

impl MailboxSender {
    pub(super) fn channel(budget: Arc<OutputBudget>) -> (Arc<Self>, SocketEvents) {
        let shared = Arc::new(Mailbox {
            budget,
            ..Mailbox::default()
        });
        (Arc::new(Self(shared.clone())), SocketEvents(shared))
    }

    pub(super) fn assignment(
        &self,
        bytes: &[u8],
        stamp: Option<crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
    ) -> AssignmentSend {
        if bytes.len() > MAX_ASSIGNMENT_BYTES {
            return AssignmentSend::TooLarge;
        }
        let mut pending = self.0.pending.lock();
        if self.is_closed() {
            return AssignmentSend::Closed;
        }
        let Some(credit) = self.0.budget.reserve() else {
            return AssignmentSend::Full;
        };
        pending.assignments.push_back((bytes.into(), stamp, credit));
        drop(pending);
        self.0.changed.notify_one();
        AssignmentSend::Queued
    }

    pub(super) fn kick(&self) -> bool {
        let mut pending = self.0.pending.lock();
        if self.is_closed() {
            return false;
        }
        pending.kicked = true;
        self.0
            .kicked
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let assignments = std::mem::take(&mut pending.assignments);
        self.0.closed.cancel();
        drop(pending);
        drop(assignments);
        self.0.changed.notify_one();
        true
    }

    pub(super) fn was_kicked(&self) -> bool {
        self.0.kicked.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(super) fn is_closed(&self) -> bool {
        self.0.closed.is_cancelled()
    }

    pub(super) fn while_open(&self, apply: impl FnOnce()) -> bool {
        let _pending = self.0.pending.lock();
        if self.is_closed() {
            return false;
        }
        apply();
        true
    }

    pub(super) fn close(&self) {
        self.0.close();
    }

    pub(super) async fn cancelled(&self) {
        self.0.closed.cancelled().await;
    }
}

impl Drop for MailboxSender {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Bounded accepted assignments in arrival order, with a separate terminal kick.
/// Taking an event is cancellation-safe: no state is consumed until a poll returns it.
pub struct SocketEvents(Arc<Mailbox>);

impl SocketEvents {
    pub fn try_recv(&mut self) -> Result<SocketEvent, TryRecvError> {
        self.try_recv_credited().map(|received| received.event)
    }

    fn try_recv_credited(&mut self) -> Result<ReceivedSocketEvent, TryRecvError> {
        let mut pending = self.0.pending.lock();
        if std::mem::take(&mut pending.kicked) {
            return Ok(ReceivedSocketEvent {
                event: SocketEvent::Kicked,
                credit: None,
            });
        }
        if self.0.closed.is_cancelled() {
            return Err(TryRecvError::Disconnected);
        }
        match pending.assignments.pop_front() {
            Some((bytes, stamp, credit)) => Ok(ReceivedSocketEvent {
                event: SocketEvent::IslandChanged(bytes.into_vec(), stamp),
                credit: Some(credit),
            }),
            None => Err(TryRecvError::Empty),
        }
    }

    pub async fn recv(&mut self) -> Option<SocketEvent> {
        self.recv_credited().await.map(|received| received.event)
    }

    pub(crate) async fn recv_credited(&mut self) -> Option<ReceivedSocketEvent> {
        loop {
            let shared = self.0.clone();
            let changed = shared.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match self.try_recv_credited() {
                Ok(event) => return Some(event),
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => changed.await,
            }
        }
    }
}

impl Drop for SocketEvents {
    fn drop(&mut self) {
        self.0.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_follows_accepted_assignments_through_validation_until_drop() {
        let (send, mut receive) = MailboxSender::channel(Arc::default());
        for value in 0..OUTPUT_CAPACITY {
            assert_eq!(
                send.assignment(&[value as u8], None),
                AssignmentSend::Queued
            );
        }
        assert_eq!(send.assignment(b"overflow", None), AssignmentSend::Full);
        let first = receive.try_recv_credited().unwrap();
        assert_eq!(send.assignment(b"overflow", None), AssignmentSend::Full);
        drop(first);
        assert_eq!(send.assignment(b"overflow", None), AssignmentSend::Queued);
        for value in 1..OUTPUT_CAPACITY {
            let SocketEvent::IslandChanged(bytes, _) = receive.try_recv().unwrap() else {
                panic!("assignment")
            };
            assert_eq!(bytes, [value as u8]);
            assert_eq!(bytes.capacity(), 1);
        }
        assert!(
            matches!(receive.try_recv(), Ok(SocketEvent::IslandChanged(bytes, _)) if bytes == b"overflow")
        );
        assert!(send.0.budget.deadline().is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn occupancy_deadline_survives_handoff_and_resets_only_after_release() {
        let budget = Arc::new(OutputBudget::default());
        let mut credits: Vec<_> = (0..OUTPUT_CAPACITY)
            .map(|_| budget.reserve().unwrap())
            .collect();
        let deadline = budget.deadline().unwrap();
        tokio::time::advance(FULL_DEADLINE - Duration::from_nanos(1)).await;
        assert!(!budget.full_expired(Instant::now()));
        assert!(budget.reserve().is_none());
        assert_eq!(budget.deadline(), Some(deadline));
        tokio::time::advance(Duration::from_nanos(1)).await;
        assert!(budget.full_expired(Instant::now()));
        credits.pop();
        assert_eq!(budget.deadline(), None);
        credits.push(budget.reserve().unwrap());
        assert_eq!(budget.deadline(), Some(Instant::now() + FULL_DEADLINE));
        drop(credits);
        assert_eq!(budget.occupancy.lock().used, 0);
    }

    #[test]
    fn byte_limit_rejects_without_replacing_a_valid_pending_assignment() {
        let (send, mut receive) = MailboxSender::channel(Arc::default());
        assert_eq!(
            send.assignment(&vec![1; MAX_ASSIGNMENT_BYTES], None),
            AssignmentSend::Queued
        );
        assert_eq!(
            send.assignment(&vec![2; MAX_ASSIGNMENT_BYTES + 1], None),
            AssignmentSend::TooLarge
        );
        assert!(
            matches!(receive.try_recv(), Ok(SocketEvent::IslandChanged(bytes, _)) if bytes == vec![1; MAX_ASSIGNMENT_BYTES])
        );
    }

    #[tokio::test]
    async fn kick_is_terminal_discards_backlog_and_wakes_the_writer() {
        let (send, mut receive) = MailboxSender::channel(Arc::default());
        send.assignment(b"old", None);
        assert!(send.kick());
        send.close();
        assert!(send.is_closed());
        assert!(!send.kick());
        assert_eq!(send.assignment(b"late", None), AssignmentSend::Closed);
        send.cancelled().await;
        assert!(matches!(receive.recv().await, Some(SocketEvent::Kicked)));
        assert!(receive.recv().await.is_none());
        assert!(send.0.pending.lock().assignments.is_empty());
    }

    #[tokio::test]
    async fn close_drop_and_receiver_loss_release_pending_bytes() {
        let (send, mut receive) = MailboxSender::channel(Arc::default());
        send.assignment(b"old", None);
        send.close();
        assert!(receive.recv().await.is_none());
        assert_eq!(send.assignment(b"late", None), AssignmentSend::Closed);
        let (send, mut receive) = MailboxSender::channel(Arc::default());
        let clone = send.clone();
        drop(send);
        assert!(matches!(receive.try_recv(), Err(TryRecvError::Empty)));
        drop(clone);
        assert!(receive.recv().await.is_none());
        let (send, receive) = MailboxSender::channel(Arc::default());
        send.assignment(b"old", None);
        drop(receive);
        assert!(send.is_closed());
        assert!(send.0.pending.lock().assignments.is_empty());
        assert_eq!(send.assignment(b"late", None), AssignmentSend::Closed);
    }

    #[tokio::test]
    async fn cancelling_a_pending_receive_does_not_lose_the_next_notification() {
        let (send, mut receive) = MailboxSender::channel(Arc::default());
        {
            let mut waiting = Box::pin(receive.recv());
            assert!(futures::poll!(&mut waiting).is_pending());
        }
        assert_eq!(send.assignment(b"next", None), AssignmentSend::Queued);
        assert!(
            matches!(receive.recv().await, Some(SocketEvent::IslandChanged(bytes, _)) if bytes == b"next")
        );
        {
            let mut waiting = Box::pin(receive.recv());
            assert!(futures::poll!(&mut waiting).is_pending());
            assert!(send.kick());
            assert!(matches!(waiting.await, Some(SocketEvent::Kicked)));
        }
    }
}
