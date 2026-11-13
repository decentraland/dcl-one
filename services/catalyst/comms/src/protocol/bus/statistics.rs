//! Internal legacy statistics observations and atomic full-state transfers.
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

pub const OBSERVATIONS_SUBJECT: &str = "archipelago.statistics.observations.v2";
pub const PRODUCER_SUBJECT: &str = "archipelago.statistics.producer.v2";
pub const SNAPSHOTS_SUBJECT: &str = "archipelago.statistics.snapshots.v1";
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Peer {
    pub address: String,
    #[serde(with = "position_bits")]
    pub position: [f64; 3],
    pub time: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Event {
    Heartbeat {
        address: String,
        #[serde(with = "position_bits")]
        position: [f64; 3],
    },
    Disconnect {
        address: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub version: u8,
    pub incarnation: uuid::Uuid,
    pub generation: u64,
    /// One random UUID per connector process lifetime, not a wallet or room authority.
    pub producer: uuid::Uuid,
    /// Strictly increasing across all events from this producer; gaps are allowed.
    pub sequence: u64,
    pub event: Event,
}
impl Observation {
    pub fn encode(&self) -> Option<Vec<u8>> {
        if self.version != 2 || self.generation == 0 || self.sequence == 0 {
            return None;
        }
        let bytes = serde_json::to_vec(self).ok()?;
        (bytes.len() <= MAX_FRAME_BYTES).then_some(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_FRAME_BYTES {
            return None;
        }
        let message: Self = serde_json::from_slice(bytes).ok()?;
        (message.version == 2 && message.generation > 0 && message.sequence > 0).then_some(message)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerGrant {
    pub incarnation: uuid::Uuid,
    pub producer: uuid::Uuid,
    pub generation: u64,
    pub challenge: uuid::Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProducerRequest {
    Status,
    Begin {
        incarnation: uuid::Uuid,
        producer: uuid::Uuid,
    },
    Confirm(ProducerGrant),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProducerReply {
    Status { incarnation: uuid::Uuid },
    Grant(ProducerGrant),
    Active(ProducerGrant),
    Rejected { incarnation: uuid::Uuid },
}
impl ProducerRequest {
    pub fn encode(&self) -> Option<Vec<u8>> {
        bounded_encode(self)
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        bounded_decode(bytes)
    }
}
impl ProducerReply {
    pub fn encode(&self) -> Option<Vec<u8>> {
        bounded_encode(self)
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        bounded_decode(bytes)
    }
}
fn bounded_encode(value: &impl Serialize) -> Option<Vec<u8>> {
    let bytes = serde_json::to_vec(value).ok()?;
    (bytes.len() <= MAX_FRAME_BYTES).then_some(bytes)
}
fn bounded_decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    (bytes.len() <= MAX_FRAME_BYTES)
        .then(|| serde_json::from_slice(bytes).ok())
        .flatten()
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub peers: Vec<Peer>,
    /// Exact protobuf topology, including order, unknown peers and geometry.
    pub islands: Vec<u8>,
    /// Exact protobuf discovery; no receipt-time substitution.
    pub discovery: Vec<u8>,
}
/// A recovery transfer is all-or-nothing above this supported byte budget.
pub const MAX_TRANSFER_BYTES: usize = 64 * 1024 * 1024;
/// Retired Pulse snapshot incarnation fences per reader; the oldest is forgotten at
/// capacity. Connector generations are separate.
pub const MAX_PRODUCERS: usize = 4096;
pub const TRANSFER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
pub const PRODUCER_QUIET: std::time::Duration = std::time::Duration::from_secs(15);
const HEADER_BYTES: usize = 44;
const MAGIC: &[u8; 4] = b"STS2";

/// A source hint only; callers must obtain authority from a correlated control reply.
pub fn frame_producer(frame: &[u8]) -> Option<uuid::Uuid> {
    if frame.len() <= HEADER_BYTES || frame.len() > MAX_FRAME_BYTES || &frame[..4] != MAGIC {
        return None;
    }
    uuid::Uuid::from_slice(&frame[4..20]).ok()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Change {
    Peer(Peer),
    Remove(String),
    Islands(Vec<u8>),
    Discovery(Vec<u8>),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Update {
    Snapshot { revision: u64, state: Snapshot },
    Change { revision: u64, change: Change },
}
impl Update {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Snapshot { revision, .. } | Self::Change { revision, .. } => *revision,
        }
    }
    fn valid(&self) -> bool {
        use crate::protocol::pulse::decentraland::kernel::comms::v3::{
            IslandStatusMessage, ServiceDiscoveryMessage,
        };
        use prost::Message;
        match self {
            Self::Snapshot { state, .. } => {
                IslandStatusMessage::decode(state.islands.as_slice()).is_ok()
                    && ServiceDiscoveryMessage::decode(state.discovery.as_slice()).is_ok()
            }
            Self::Change {
                change: Change::Islands(bytes),
                ..
            } => IslandStatusMessage::decode(bytes.as_slice()).is_ok(),
            Self::Change {
                change: Change::Discovery(bytes),
                ..
            } => ServiceDiscoveryMessage::decode(bytes.as_slice()).is_ok(),
            _ => true,
        }
    }
}

/// Own one encoded payload and produce one bounded frame at a time.
pub struct Frames {
    payload: Vec<u8>,
    producer: uuid::Uuid,
    sequence: u64,
    size: usize,
    index: usize,
}
struct Payload(Vec<u8>);
fn append_payload(buffer: &mut Vec<u8>, bytes: &[u8]) -> bool {
    let needed = buffer.len().saturating_add(bytes.len());
    if needed > MAX_TRANSFER_BYTES {
        return false;
    }
    if needed > buffer.capacity() {
        let capacity = buffer
            .capacity()
            .saturating_mul(2)
            .max(needed)
            .min(MAX_TRANSFER_BYTES);
        buffer.reserve_exact(capacity - buffer.len());
    }
    buffer.extend_from_slice(bytes);
    true
}
impl std::io::Write for Payload {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if !append_payload(&mut self.0, bytes) {
            return Err(std::io::Error::other(
                "statistics transfer capacity exceeded",
            ));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Frames {
    pub fn new(update: &Update, producer: uuid::Uuid, sequence: u64, limit: usize) -> Option<Self> {
        let size = limit.min(MAX_FRAME_BYTES).checked_sub(HEADER_BYTES)?;
        if size == 0 || sequence == 0 {
            return None;
        }
        let mut payload = Payload(Vec::new());
        serde_json::to_writer(&mut payload, update).ok()?;
        Some(Self {
            payload: payload.0,
            producer,
            sequence,
            size,
            index: 0,
        })
    }
}
impl Iterator for Frames {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.payload.chunks(self.size).nth(self.index)?;
        let mut bytes = Vec::with_capacity(HEADER_BYTES + chunk.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(self.producer.as_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&(self.index as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.payload.len().div_ceil(self.size) as u64).to_le_bytes());
        bytes.extend_from_slice(chunk);
        self.index += 1;
        Some(bytes)
    }
}
/// Fixture/test helper. Production streams `Frames` without collecting a second payload.
#[cfg(any(test, feature = "test-support"))]
pub fn frames(
    snapshot: &Snapshot,
    producer: uuid::Uuid,
    sequence: u64,
    limit: usize,
) -> Option<Vec<Vec<u8>>> {
    Some(
        Frames::new(
            &Update::Snapshot {
                revision: sequence,
                state: snapshot.clone(),
            },
            producer,
            sequence,
            limit,
        )?
        .collect(),
    )
}
struct Transfer {
    producer: uuid::Uuid,
    sequence: u64,
    count: u64,
    next: u64,
    bytes: Vec<u8>,
    started: std::time::Instant,
}

/// Two bounded slots isolate an incomplete replacement from the working producer.
/// Retired fences are kept for the last [`MAX_PRODUCERS`] replacements; beyond that the
/// oldest is forgotten, so a new Pulse incarnation is never refused for the reader's lifetime.
#[derive(Default)]
pub struct Assembler {
    producer: Option<uuid::Uuid>,
    retired: HashSet<uuid::Uuid>,
    retired_order: VecDeque<uuid::Uuid>,
    sequence: u64,
    last_valid: Option<std::time::Instant>,
    pending: [Option<Transfer>; 2],
}
impl Assembler {
    pub fn expire(&mut self, now: std::time::Instant) {
        for slot in &mut self.pending {
            if slot
                .as_ref()
                .is_some_and(|t| now.saturating_duration_since(t.started) >= TRANSFER_TIMEOUT)
            {
                *slot = None;
            }
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn receive(&mut self, frame: &[u8]) -> Option<Snapshot> {
        match self.receive_update(frame, std::time::Instant::now())?.1 {
            Update::Snapshot { state, .. } => Some(state),
            _ => None,
        }
    }
    pub fn receive_update(
        &mut self,
        frame: &[u8],
        now: std::time::Instant,
    ) -> Option<(uuid::Uuid, Update)> {
        self.expire(now);
        if frame.len() <= HEADER_BYTES || frame.len() > MAX_FRAME_BYTES || &frame[..4] != MAGIC {
            return None;
        }
        let producer = uuid::Uuid::from_slice(&frame[4..20]).ok()?;
        let sequence = u64::from_le_bytes(frame[20..28].try_into().ok()?);
        let index = u64::from_le_bytes(frame[28..36].try_into().ok()?);
        let count = u64::from_le_bytes(frame[36..44].try_into().ok()?);
        if sequence == 0
            || count == 0
            || count > MAX_TRANSFER_BYTES as u64
            || index >= count
            || self.retired.contains(&producer)
        {
            return None;
        }
        let active = self.producer == Some(producer);
        if active && sequence <= self.sequence {
            return None;
        }
        if !active
            && self.producer.is_some()
            && self
                .last_valid
                .is_some_and(|at| now.saturating_duration_since(at) < PRODUCER_QUIET)
        {
            return None;
        }
        let slot = &mut self.pending[usize::from(!active && self.producer.is_some())];
        let same = slot
            .as_ref()
            .is_some_and(|t| t.producer == producer && t.sequence == sequence);
        if !same {
            if index != 0
                || slot
                    .as_ref()
                    .is_some_and(|t| t.producer == producer && t.sequence > sequence)
            {
                return None;
            }
            *slot = Some(Transfer {
                producer,
                sequence,
                count,
                next: 0,
                bytes: Vec::new(),
                started: now,
            });
        }
        let transfer = slot.as_mut()?;
        if count != transfer.count
            || index > transfer.next
            || transfer
                .bytes
                .len()
                .saturating_add(frame.len() - HEADER_BYTES)
                > MAX_TRANSFER_BYTES
        {
            *slot = None;
            return None;
        }
        if index < transfer.next {
            return None;
        }
        if !append_payload(&mut transfer.bytes, &frame[HEADER_BYTES..]) {
            *slot = None;
            return None;
        }
        transfer.next += 1;
        if transfer.next != count {
            return None;
        }
        let completed = slot.take()?;
        let update: Update = serde_json::from_slice(&completed.bytes).ok()?;
        if !update.valid() || (!active && !matches!(update, Update::Snapshot { .. })) {
            return None;
        }
        if !active {
            if let Some(previous) = self.producer.replace(producer) {
                self.retire(previous);
            }
            self.pending = [None, None];
        }
        self.sequence = sequence;
        self.last_valid = Some(now);
        Some((producer, update))
    }
    fn retire(&mut self, producer: uuid::Uuid) {
        if !self.retired.insert(producer) {
            return;
        }
        self.retired_order.push_back(producer);
        while self.retired_order.len() > MAX_PRODUCERS {
            if let Some(oldest) = self.retired_order.pop_front() {
                self.retired.remove(&oldest);
            }
        }
    }
}

// JSON's null cannot round-trip a non-finite float. This private wire preserves
// all IEEE bits; the HTTP renderer remains responsible for JSON number semantics.
mod position_bits {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(position: &[f64; 3], serializer: S) -> Result<S::Ok, S::Error> {
        position.map(f64::to_bits).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[f64; 3], D::Error> {
        Ok(<[u64; 3]>::deserialize(deserializer)?.map(f64::from_bits))
    }
}
#[cfg(test)]
#[path = "statistics_tests.rs"]
mod tests;
