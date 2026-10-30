use std::io;

use async_trait::async_trait;

pub type TransportMessage = Vec<u8>;

#[derive(Debug)]
pub enum TransportError {
    /// Opaque transient eligibility/expiry outcome: end the affected stream cleanly for replay.
    Resync,
    Internal(Box<dyn std::error::Error + Send + Sync>),
    Closed,
    NotBinaryMessage,
}

impl From<io::Error> for TransportError {
    fn from(value: io::Error) -> Self {
        TransportError::Internal(Box::new(value))
    }
}

/// Opaque application eligibility evaluated at the transport's final queue handoff.
pub type DeliveryGuard = std::sync::Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, ()>> + Send>>
        + Send
        + Sync,
>;
pub const MAX_GUARDED_MESSAGE_BYTES: usize = 16 * 1024;
pub struct GuardedDelivery {
    pub guard: DeliveryGuard,
    pub expires_at: tokio::time::Instant,
    pub cancel: tokio_util::sync::CancellationToken,
}
#[derive(Debug, PartialEq, Eq)]
pub enum GuardedSend {
    Sent,
    Suppressed,
    Unsupported,
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn receive(&self) -> Result<TransportMessage, TransportError>;
    async fn send(&self, message: TransportMessage) -> Result<(), TransportError>;
    /// Explicitly unsupported unless the implementation retains the fence through its final queue.
    /// Sent confirms physical writer completion; Suppressed confirms no handoff. Preserve
    /// absolute expiry/cancellation and retire a possibly partial sink on interruption.
    /// Resync is transient and requires a confirmed clean terminal, not a transport RemoteError.
    async fn send_guarded(
        &self,
        _message: TransportMessage,
        _delivery: GuardedDelivery,
    ) -> Result<GuardedSend, TransportError> {
        Ok(GuardedSend::Unsupported)
    }
    async fn close(&self);
}

/// In-process transport over a pair of bounded channels: the unit tests drive
/// the server through it without a socket.
#[cfg(any(test, feature = "test-support"))]
pub mod memory {
    use super::{Transport, TransportError, TransportMessage};
    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc::{Receiver, Sender, channel};

    pub struct MemoryTransport {
        sender: Sender<Vec<u8>>,
        receiver: Mutex<Receiver<Vec<u8>>>,
    }

    impl MemoryTransport {
        fn new(sender: Sender<Vec<u8>>, receiver: Receiver<Vec<u8>>) -> Self {
            Self {
                sender,
                receiver: Mutex::new(receiver),
            }
        }

        /// Returns the client end first, the server end second.
        pub fn create() -> (Self, Self) {
            let (client_sender, server_receiver) = channel::<Vec<u8>>(32);
            let (server_sender, client_receiver) = channel::<Vec<u8>>(32);
            (
                Self::new(client_sender, client_receiver),
                Self::new(server_sender, server_receiver),
            )
        }
    }

    #[async_trait]
    impl Transport for MemoryTransport {
        async fn receive(&self) -> Result<TransportMessage, TransportError> {
            match self.receiver.lock().await.recv().await {
                Some(message) => Ok(message),
                None => Err(TransportError::Closed),
            }
        }

        async fn send(&self, message: Vec<u8>) -> Result<(), TransportError> {
            self.sender
                .send(message)
                .await
                .map_err(|_| TransportError::Closed)
        }

        async fn send_guarded(
            &self,
            message: Vec<u8>,
            delivery: super::GuardedDelivery,
        ) -> Result<super::GuardedSend, TransportError> {
            let _cancel_on_drop = delivery.cancel.clone().drop_guard();
            if message.len() > super::MAX_GUARDED_MESSAGE_BYTES {
                return Ok(super::GuardedSend::Suppressed);
            }
            let send = async {
                let permit = self
                    .sender
                    .reserve()
                    .await
                    .map_err(|_| TransportError::Closed)?;
                if delivery.cancel.is_cancelled() {
                    return Ok(super::GuardedSend::Suppressed);
                }
                match (delivery.guard)().await {
                    Ok(true) => {}
                    Ok(false) => return Ok(super::GuardedSend::Suppressed),
                    Err(()) => return Err(TransportError::Resync),
                }
                permit.send(message);
                Ok(super::GuardedSend::Sent)
            };
            tokio::select! {
                _ = delivery.cancel.cancelled() => Ok(super::GuardedSend::Suppressed),
                result = tokio::time::timeout_at(delivery.expires_at, send) => result.unwrap_or(Err(TransportError::Resync)),
            }
        }
        async fn close(&self) {
            self.receiver.lock().await.close();
        }
    }
}
