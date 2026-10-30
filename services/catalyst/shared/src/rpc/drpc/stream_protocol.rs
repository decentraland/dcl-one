use std::{future::Future, sync::Arc};

use prost::Message;
use tracing::debug;

use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use tokio_util::sync::CancellationToken;

use super::{
    CommonError,
    rpc_protocol::{RpcMessageTypes, StreamMessage, parse::build_message_identifier},
    transport::{Transport, TransportError},
};

pub struct StreamProtocol<T: Transport + ?Sized> {
    port_id: u32,
    /// Number of the message that opened the stream.
    message_number: u32,
    last_received_sequence_id: u32,
    is_remote_closed: bool,
    /// Whether the remote half was still open as of the last message.
    was_open: bool,
    generator: (Generator<Vec<u8>>, GeneratorYielder<Vec<u8>>),
    transport: Arc<T>,
    process_cancellation_token: CancellationToken,
}

impl<T: Transport + ?Sized + 'static> StreamProtocol<T> {
    pub(crate) fn new(transport: Arc<T>, port_id: u32, message_number: u32) -> Self {
        Self {
            last_received_sequence_id: 0,
            is_remote_closed: false,
            was_open: false,
            generator: Generator::create(),
            transport,
            message_number,
            port_id,
            process_cancellation_token: CancellationToken::new(),
        }
    }

    async fn next(&mut self) -> Option<Vec<u8>> {
        select! {
            _ = self.process_cancellation_token.cancelled() =>  {
                self.generator.0.close();
                self.is_remote_closed = true;
                None
            }
            message = self.generator.0.next() => {
                match message {
                    Some(msg) => {
                        self.last_received_sequence_id += 1;
                        self.was_open = true;
                        let stream_message = StreamMessage {
                            port_id: self.port_id,
                            sequence_id: self.last_received_sequence_id,
                            message_identifier: build_message_identifier(
                                RpcMessageTypes::StreamAck as u32,
                                self.message_number,
                            ),
                            payload: vec![],
                            closed: false,
                            ack: true,
                        };

                        if let Err(err) = self.transport
                            .send(stream_message.encode_to_vec())
                            .await {
                            tracing::error!("> StreamProtocol > next > Error while sending the ACK StreamMessage through the transport: {err:?}");
                            self.is_remote_closed = true;
                            return None;
                        }

                        Some(msg)
                    }
                    None => {
                        self.is_remote_closed = true;
                        None
                    }
                }
            }
        }
    }

    /// The transform runs in a background task; an item it maps to `None` is dropped.
    pub fn to_generator<
        O: Send + Sync + 'static,
        F: Fn(Vec<u8>) -> Option<O> + Send + Sync + 'static,
    >(
        mut self,
        transformer: F,
    ) -> Generator<O> {
        let (generator, generator_yielder) = Generator::create();
        tokio::spawn(async move {
            while let Some(item) = self.next().await {
                if let Some(item_decoded) = transformer(item)
                    && generator_yielder.r#yield(item_decoded).await.is_err()
                {
                    tracing::error!(
                        "> StreamProtocol > to_generator > Generator > error r#yield, probably it's closed"
                    );
                    tracing::debug!(
                        "> StreamProtocol > to_generator > Generator > breaking to stop yielding"
                    );
                    break;
                }
            }
        });

        generator
    }

    pub(crate) async fn acknowledge_open(&self) -> Result<(), TransportError> {
        let stream_message = StreamMessage {
            port_id: self.port_id,
            sequence_id: self.last_received_sequence_id,
            message_identifier: build_message_identifier(
                RpcMessageTypes::StreamAck as u32,
                self.message_number,
            ),
            payload: vec![],
            closed: false,
            ack: true,
        };

        self.transport.send(stream_message.encode_to_vec()).await
    }

    pub fn close(&mut self) {
        self.generator.0.close();
        self.process_cancellation_token.cancel();
    }

    /// The returned listener is meant to be registered in
    /// [`super::messages_handlers::ServerMessagesHandler`] or
    /// [`super::messages_handlers::ClientMessagesHandler`], and the callback is what
    /// unregisters it once processing finishes.
    pub(crate) async fn start_processing<
        F: Future + Send,
        Callback: FnOnce() -> F + Send + 'static,
    >(
        &self,
        callback: Callback,
    ) -> Result<UnboundedSender<(RpcMessageTypes, u32, StreamMessage)>, CommonError> {
        if self.acknowledge_open().await.is_err() {
            return Err(CommonError::TransportError);
        }
        Ok(self.listen(callback))
    }

    /// [`start_processing`](Self::start_processing) without the acknowledgement: the
    /// remote sends its first message on the acknowledgement, so the listener has to
    /// be registered before [`acknowledge_open`](Self::acknowledge_open).
    pub(crate) fn listen<F: Future + Send, Callback: FnOnce() -> F + Send + 'static>(
        &self,
        callback: Callback,
    ) -> UnboundedSender<(RpcMessageTypes, u32, StreamMessage)> {
        let token = self.process_cancellation_token.clone();
        let (messages_listener, messages_processor) = unbounded_channel();
        let internal_channel = self.generator.1.clone();
        tokio::spawn(async move {
            let token_cloned = token.clone();
            select! {
                _ = token.cancelled() => {
                    debug!("> StreamProtocol cancelled!");
                    callback().await;
                },
                _ = Self::process_messages(messages_processor, internal_channel, token_cloned) => {
                    callback().await;
                }
            }
        });

        messages_listener
    }

    async fn process_messages(
        mut messages_processor: UnboundedReceiver<(RpcMessageTypes, u32, StreamMessage)>,
        internal_channel_sender: GeneratorYielder<Vec<u8>>,
        cancellation_token: CancellationToken,
    ) {
        while let Some((message_type, _, stream_message)) = messages_processor.recv().await {
            if matches!(message_type, RpcMessageTypes::StreamMessage) {
                if stream_message.closed {
                    cancellation_token.cancel();
                    messages_processor.close();
                } else if internal_channel_sender
                    .r#yield(stream_message.payload)
                    .await
                    .is_err()
                {
                    tracing::error!(
                        "> StreamProtocol > process_messages > Error on sending through the Generator, seems to be dropped"
                    )
                }
            } else if matches!(message_type, RpcMessageTypes::RemoteErrorResponse) {
                cancellation_token.cancel();
                messages_processor.close();
            }
        }
    }
}

#[derive(Debug)]
pub enum GeneratorError {
    /// The receiving half is gone; stop yielding.
    UnableToInsert,
}

/// An opt-in delivery fence propagated through mapping to the final transport consumer.
pub use super::transport::DeliveryGuard;
use super::transport::{GuardedDelivery, MAX_GUARDED_MESSAGE_BYTES};
#[derive(Debug, PartialEq, Eq)]
pub enum DeliveryCompletion {
    Acknowledged,
    Suppressed,
}
pub(crate) struct Item<M> {
    pub value: M,
    pub delivery: Option<GuardedDelivery>,
    pub completion: Option<tokio::sync::oneshot::Sender<DeliveryCompletion>>,
}
enum Receiver<M> {
    Unbounded(UnboundedReceiver<Item<M>>),
    Bounded(tokio::sync::mpsc::Receiver<Item<M>>),
}
enum Sender<M> {
    Unbounded(UnboundedSender<Item<M>>),
    Bounded(tokio::sync::mpsc::Sender<Item<M>>),
}
pub struct Generator<M> {
    receiver: Receiver<M>,
    policy: Option<(usize, std::time::Duration)>,
}

impl<M: Send + Sync + 'static> Generator<M> {
    pub fn create() -> (Self, GeneratorYielder<M>) {
        Self::with_policy(None)
    }

    /// Bounded staging at every mapping hop, plus a total transport send/ACK deadline.
    pub fn bounded(capacity: usize, deadline: std::time::Duration) -> (Self, GeneratorYielder<M>) {
        Self::with_policy(Some((capacity.max(1), deadline)))
    }
    fn with_policy(policy: Option<(usize, std::time::Duration)>) -> (Self, GeneratorYielder<M>) {
        let (receiver, sender) = if let Some((capacity, _)) = policy {
            let (tx, rx) = tokio::sync::mpsc::channel(capacity);
            (Receiver::Bounded(rx), Sender::Bounded(tx))
        } else {
            let (tx, rx) = unbounded_channel();
            (Receiver::Unbounded(rx), Sender::Unbounded(tx))
        };
        (
            Self { receiver, policy },
            GeneratorYielder {
                sender,
                deadline: policy.map(|(_, deadline)| deadline),
            },
        )
    }
    pub(crate) fn delivery_deadline(&self) -> Option<std::time::Duration> {
        self.policy.map(|(_, deadline)| deadline)
    }

    /// Mapping preserves boundedness and the original item's final delivery fence.
    pub fn from_generator<
        O: Send + Sync + 'static,
        F: Fn(M) -> Option<O> + Send + Sync + 'static,
    >(
        mut old_generator: Generator<M>,
        transformer: F,
    ) -> Generator<O> {
        let (generator, yielder) = Generator::with_policy(old_generator.policy);
        tokio::spawn(async move {
            loop {
                let item = tokio::select! { _ = yielder.closed() => break, item = old_generator.next_item() => item };
                let Some(item) = item else {
                    break;
                };
                if let Some(value) = transformer(item.value)
                    && yielder
                        .yield_item(Item {
                            value,
                            delivery: item.delivery,
                            completion: item.completion,
                        })
                        .await
                        .is_err()
                {
                    break;
                }
            }
        });
        generator
    }
    pub(crate) async fn next_item(&mut self) -> Option<Item<M>> {
        match &mut self.receiver {
            Receiver::Unbounded(rx) => rx.recv().await,
            Receiver::Bounded(rx) => rx.recv().await,
        }
    }
    pub async fn next(&mut self) -> Option<M> {
        while let Some(item) = self.next_item().await {
            if let Some(delivery) = item.delivery {
                let result = tokio::time::timeout_at(delivery.expires_at, (delivery.guard)()).await;
                if delivery.cancel.is_cancelled() {
                    self.close();
                    return None;
                }
                match result {
                    Ok(Ok(true)) => {}
                    Ok(Ok(false)) => {
                        if let Some(completion) = item.completion {
                            let _ = completion.send(DeliveryCompletion::Suppressed);
                        }
                        continue;
                    }
                    _ => {
                        self.close();
                        return None;
                    }
                }
            }
            if let Some(completion) = item.completion {
                let _ = completion.send(DeliveryCompletion::Acknowledged);
            }
            return Some(item.value);
        }
        None
    }
    pub fn close(&mut self) {
        match &mut self.receiver {
            Receiver::Unbounded(rx) => rx.close(),
            Receiver::Bounded(rx) => rx.close(),
        }
    }
}

pub struct GeneratorYielder<M> {
    sender: Sender<M>,
    deadline: Option<std::time::Duration>,
}
impl<M> GeneratorYielder<M> {
    pub async fn closed(&self) {
        match &self.sender {
            Sender::Unbounded(tx) => tx.closed().await,
            Sender::Bounded(tx) => tx.closed().await,
        }
    }
    async fn yield_item(&self, item: Item<M>) -> Result<(), GeneratorError> {
        match &self.sender {
            Sender::Unbounded(tx) => tx.send(item).map_err(|_| GeneratorError::UnableToInsert),
            Sender::Bounded(tx) => {
                let deadline = item
                    .delivery
                    .as_ref()
                    .map(|d| d.expires_at)
                    .unwrap_or_else(|| {
                        tokio::time::Instant::now() + self.deadline.expect("bounded policy")
                    });
                let send = tx.send(item);
                match tokio::time::timeout_at(deadline, send).await {
                    Ok(Ok(())) => Ok(()),
                    _ => Err(GeneratorError::UnableToInsert),
                }
            }
        }
    }
    pub async fn r#yield(&self, item: M) -> Result<(), GeneratorError> {
        self.yield_item(Item {
            value: item,
            delivery: None,
            completion: None,
        })
        .await
    }
    /// Initial replay admits its next item only after this item is ACKed or safely suppressed.
    pub async fn yield_confirmed(
        &self,
        item: M,
        guard: DeliveryGuard,
    ) -> Result<DeliveryCompletion, GeneratorError>
    where
        M: prost::Message,
    {
        if item.encoded_len() > MAX_GUARDED_MESSAGE_BYTES - 256 {
            return Err(GeneratorError::UnableToInsert);
        }
        let expires_at =
            tokio::time::Instant::now() + self.deadline.ok_or(GeneratorError::UnableToInsert)?;
        let (completion, receiver) = tokio::sync::oneshot::channel();
        self.yield_item(Item {
            value: item,
            delivery: Some(GuardedDelivery {
                guard,
                expires_at,
                cancel: CancellationToken::new(),
            }),
            completion: Some(completion),
        })
        .await?;
        tokio::time::timeout_at(expires_at, receiver)
            .await
            .map_err(|_| GeneratorError::UnableToInsert)?
            .map_err(|_| GeneratorError::UnableToInsert)
    }

    pub async fn yield_guarded(&self, item: M, guard: DeliveryGuard) -> Result<(), GeneratorError>
    where
        M: prost::Message,
    {
        if item.encoded_len() > MAX_GUARDED_MESSAGE_BYTES - 256 {
            return Err(GeneratorError::UnableToInsert);
        }
        let deadline = self.deadline.ok_or(GeneratorError::UnableToInsert)?;
        self.yield_item(Item {
            value: item,
            delivery: Some(GuardedDelivery {
                guard,
                expires_at: tokio::time::Instant::now() + deadline,
                cancel: CancellationToken::new(),
            }),
            completion: None,
        })
        .await
    }
}
impl<M> Clone for GeneratorYielder<M> {
    fn clone(&self) -> Self {
        let sender = match &self.sender {
            Sender::Unbounded(tx) => Sender::Unbounded(tx.clone()),
            Sender::Bounded(tx) => Sender::Bounded(tx.clone()),
        };
        Self {
            sender,
            deadline: self.deadline,
        }
    }
}
