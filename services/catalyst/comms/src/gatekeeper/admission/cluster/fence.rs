use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;

const REALM_LANE: &str = "realm";
const OWNER_LAPSE_SECONDS: i32 = 90;
const MAX_ASSIGNMENT_BYTES: usize = 64 * 1024 - 2048;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectedRoomOrigin {
    #[default]
    Unknown,
    Native,
    LegacyOwnerV1,
    LegacyIntentV2,
}

impl SelectedRoomOrigin {
    fn wire(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::Native => 1,
            Self::LegacyOwnerV1 => 2,
            Self::LegacyIntentV2 => 3,
        }
    }
    pub(crate) fn from_wire(value: i32) -> Self {
        match value {
            1 => Self::Native,
            2 => Self::LegacyOwnerV1,
            3 => Self::LegacyIntentV2,
            _ => Self::Unknown,
        }
    }
}

/// Internal credential authority retained through SQL visibility and socket queue waits.
/// This is never projected into a public client packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PulseAdmissionStamp {
    #[serde(default)]
    pub selected_origin: SelectedRoomOrigin,
    #[serde(default)]
    pub selected_identity: u64,
    pub wallet: String,
    pub session: String,
    pub cluster_id: String,
    pub realm: String,
    pub source_incarnation: String,
    pub assignment_version: u64,
    pub token_not_before: u64,
    pub credential_not_before: u64,
    #[serde(default)]
    pub safety_generation: u64,
    #[serde(default)]
    pub safety_token_floor: u64,
}

impl PulseAdmissionStamp {
    pub fn from_snapshot(
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
        credential_not_before: u64,
    ) -> Option<Self> {
        if !crate::protocol::bus::cluster::active_selection(snapshot)
            || !snapshot.pending_takeovers.is_empty()
        {
            return None;
        }
        Some(Self {
            selected_origin: SelectedRoomOrigin::from_wire(snapshot.selected_origin),
            selected_identity: snapshot.selected_identity?,
            wallet: snapshot.wallet.clone(),
            session: snapshot.session.clone(),
            cluster_id: snapshot.cluster_id.clone(),
            realm: snapshot.realm.clone(),
            source_incarnation: snapshot.source_incarnation.clone(),
            assignment_version: snapshot.assignment_version,
            token_not_before: snapshot.token_not_before?,
            credential_not_before,
            safety_generation: 0,
            safety_token_floor: 0,
        })
    }
    pub fn valid(&self) -> bool {
        self.selected_identity > 0
            && self.selected_origin != SelectedRoomOrigin::Unknown
            && (self.selected_origin != SelectedRoomOrigin::LegacyIntentV2
                || self.selected_identity <= i64::MAX as u64)
            && super::subscriber::is_session_key(&self.wallet)
            && super::subscriber::is_session_key(&self.session)
            && !self.cluster_id.is_empty()
            && self.cluster_id.len() <= crate::protocol::bus::cluster::MAX_REPLY_BYTES
            && self.realm.len() <= crate::protocol::bus::cluster::MAX_REPLY_BYTES
            && !self.source_incarnation.is_empty()
            && self.source_incarnation.len() <= 256
            && self.assignment_version > 0
            && self.safety_token_floor <= self.credential_not_before
            && self.token_not_before <= self.credential_not_before
            && self.credential_not_before <= crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS
    }
    pub fn matches_snapshot(
        &self,
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
    ) -> bool {
        self.valid()
            && Self::from_snapshot(snapshot, self.credential_not_before).is_some_and(
                |mut expected| {
                    expected.safety_generation = self.safety_generation;
                    expected.safety_token_floor = self.safety_token_floor;
                    expected == *self
                },
            )
    }
    pub fn safety_snapshot(&self) -> Option<super::safety::SafetySnapshot> {
        (self.safety_generation > 0).then_some(super::safety::SafetySnapshot {
            generation: self.safety_generation,
            token_floor: self.safety_token_floor,
        })
    }
    pub fn snapshot(&self) -> crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
        crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
            selected_origin: self.selected_origin.wire(),
            selected_identity: Some(self.selected_identity),
            assignment_state: if self.valid() { 1 } else { 0 },
            wallet: self.wallet.clone(),
            session: self.session.clone(),
            cluster_id: self.cluster_id.clone(),
            realm: self.realm.clone(),
            source_incarnation: self.source_incarnation.clone(),
            assignment_version: self.assignment_version,
            token_not_before: Some(self.token_not_before),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenFence {
    pub audience: String,
    pub lane: String,
    pub owner_session: String,
    pub owner_epoch: u64,
    pub assignment_revision: u64,
    pub fencing_token: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse_admission: Option<PulseAdmissionStamp>,
    pub island_id: String,
    pub connection_string: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_island_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_expires_at_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub peers: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealmAssignmentSnapshot {
    pub pulse_admission: Option<PulseAdmissionStamp>,
    pub audience: String,
    pub wallet: String,
    pub lane: String,
    pub authority_incarnation: String,
    pub owner_session: String,
    pub owner_epoch: u64,
    pub assignment_revision: u64,
    pub fencing_token: u64,
    pub island_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredAssignmentDocument {
    #[serde(default)]
    pulse_admission: Option<PulseAdmissionStamp>,
    island_id: String,
    #[serde(rename = "connectionString")]
    _connection_string: String,
    #[serde(default, rename = "fromIslandId")]
    _from_island_id: Option<String>,
    #[serde(default, rename = "tokenExpiresAtUnix")]
    _token_expires_at_unix: Option<u64>,
    #[serde(default, rename = "peers")]
    _peers: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FenceError {
    #[error("assignment authority unavailable")]
    Unavailable,
    #[error("assignment owner changed")]
    Conflict,
    #[error("wallet has no fenced assignment owner")]
    Unowned,
    #[error("assignment authority data is invalid")]
    Invalid,
}

#[async_trait]
pub trait RealmAssignmentLease: Send {
    fn read_only(&self) -> bool {
        false
    }
    fn previous_pulse_admission(&self) -> Option<PulseAdmissionStamp> {
        None
    }
    fn token_fence(&self) -> TokenFence;
    fn previous_island(&self) -> Option<String>;
    fn previous_token_expires_at_unix(&self) -> Option<u64> {
        None
    }
    async fn commit(self: Box<Self>, assignment: AssignmentDocument) -> Result<(), FenceError>;
}

#[async_trait]
pub trait AssignmentFence: Send + Sync {
    async fn lock_selected(
        &self,
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError>;
    fn requires_generation(&self) -> bool {
        false
    }
    async fn generation_binding(&self) -> Option<crate::interfaces::PulseGenerationBinding> {
        None
    }
    async fn accept_selected_hint(
        &self,
        wallet: &str,
        change: &crate::protocol::pulse::decentraland::pulse::PeerClusterChange,
    ) -> bool {
        self.observe_selected_hint(wallet, change);
        true
    }
    async fn complete_generation_view(
        &self,
        _binding: &crate::interfaces::PulseGenerationBinding,
    ) -> bool {
        false
    }
    async fn retire_obsolete_generation(
        &self,
        _binding: &crate::interfaces::PulseGenerationBinding,
        _limit: usize,
        _current_targets: &[crate::protocol::bus::selection_view::SelectionTarget],
    ) {
    }
    async fn recover_selected_target(
        &self,
        _binding: &crate::interfaces::PulseGenerationBinding,
        _target: &crate::protocol::bus::selection_view::SelectionTarget,
    ) -> bool {
        false
    }
    fn selected_reconciliation_targets(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    async fn selected_reconciliation_batch(
        &self,
        limit: usize,
        cold_limit: usize,
    ) -> Vec<(String, String)> {
        self.selected_reconciliation_targets()
            .into_iter()
            .take(limit.min(cold_limit))
            .collect()
    }
    fn selected_maintenance_urgent(&self, _wallet: &str) -> bool {
        false
    }
    fn observe_selected_hint(
        &self,
        _wallet: &str,
        _change: &crate::protocol::pulse::decentraland::pulse::PeerClusterChange,
    ) {
    }

    fn selected_retirement_target(
        &self,
        _wallet: &str,
        _session: &str,
    ) -> Option<SelectedRetirementTarget> {
        None
    }
    async fn retire_selected(
        &self,
        _target: SelectedRetirementTarget,
        _reply: crate::protocol::bus::cluster::SelectionRetirementReply,
    ) {
    }

    async fn settle_idle(&self, _wallet: &str) {}
    fn retained_work_credit(
        &self,
        _wallet: &str,
    ) -> Option<std::sync::Arc<super::subscriber::wallet_queue::WorkCredit>> {
        None
    }
    fn unresolved_mutations(&self) -> usize {
        0
    }

    async fn lock_realm(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError>;
}

#[async_trait]
pub trait RealmAssignmentReader: Send + Sync {
    async fn current_realm_assignment(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<RealmAssignmentSnapshot, FenceError>;
}

/// The realm-lane fence over `social.archipelago_v4_assignments` (the rows
/// configured selected room ownership shares with the additive control API), read
/// and committed over the ONE pool. Every call is bounded by
/// [`FENCE_CALL_TIMEOUT`]: a slow database answers `Unavailable`, never a
/// stalled feed.
pub struct PgAssignmentFence {
    pool: PgPool,
    audience: String,
    selected: std::sync::Arc<
        parking_lot::Mutex<std::collections::BTreeMap<String, std::sync::Arc<SelectedWallet>>>,
    >,
    /// Exact owned rows of retired slots; slots and these rows share the selected bound.
    /// The wallet's next selection replaces its own fresh row. Retirement writes no row.
    retired: parking_lot::Mutex<std::collections::BTreeMap<String, RealmRow>>,
    incarnation: String,
    selected_limit: usize,
    selected_cursor: tokio::sync::Mutex<[Option<String>; 2]>,
    obsolete_cursor: tokio::sync::Mutex<Option<String>>,
    direct_credits: std::sync::Arc<tokio::sync::Semaphore>,
    generation_required: bool,
    completed_generation: parking_lot::Mutex<
        Option<(
            crate::interfaces::PulseGenerationBinding,
            tokio::time::Instant,
        )>,
    >,
}

pub const FENCE_CALL_TIMEOUT: Duration = Duration::from_secs(2);

fn validate_audience(audience: &str) -> Result<(), FenceError> {
    if audience.is_empty() || audience.len() > 256 {
        Err(FenceError::Invalid)
    } else {
        Ok(())
    }
}

impl PgAssignmentFence {
    pub fn new(pool: PgPool, audience: String) -> Result<Self, FenceError> {
        Self::with_limit(pool, audience, super::peer_state::DEFAULT_PEER_STATE_MAX)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_selected_limit_for_test(
        pool: PgPool,
        audience: String,
        limit: usize,
    ) -> Result<Self, FenceError> {
        Self::with_limit(pool, audience, limit)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn unfenced_test(pool: PgPool, audience: String) -> Result<Self, FenceError> {
        Self::unfenced_test_with_limit(pool, audience, super::peer_state::DEFAULT_PEER_STATE_MAX)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn unfenced_test_with_limit(
        pool: PgPool,
        audience: String,
        limit: usize,
    ) -> Result<Self, FenceError> {
        let mut fence = Self::with_limit(pool, audience, limit)?;
        fence.generation_required = false;
        Ok(fence)
    }
    pub fn generation_required(&self) -> bool {
        self.generation_required
    }
    pub async fn current_generation(
        &self,
    ) -> Result<crate::interfaces::PulseGenerationBinding, FenceError> {
        if !self.generation_required {
            return Err(FenceError::Unavailable);
        }
        super::generation::current(&self.pool, &self.audience).await
    }
    fn safety_store(&self) -> super::safety::SafetyStore {
        #[cfg(any(test, feature = "test-support"))]
        if !self.generation_required {
            return super::safety::SafetyStore::unfenced_test(self.pool.clone());
        }
        super::safety::SafetyStore::new(self.pool.clone())
    }
    async fn register_source(&self, source: &str) -> Result<Option<u64>, FenceError> {
        if !self.generation_required {
            return Ok(None);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| FenceError::Unavailable)?;
        sqlx::query("SET LOCAL lock_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let binding = super::generation::lock(&mut tx, &self.audience, source).await?;
        tx.commit().await.map_err(|_| FenceError::Unavailable)?;
        Ok(Some(binding.epoch))
    }
    pub(crate) fn with_limit(
        pool: PgPool,
        audience: String,
        limit: usize,
    ) -> Result<Self, FenceError> {
        validate_audience(&audience)?;
        Ok(Self {
            pool,
            audience,
            selected: Default::default(),
            retired: Default::default(),
            incarnation: uuid::Uuid::new_v4().to_string(),
            selected_limit: limit.max(1),
            selected_cursor: Default::default(),
            generation_required: true,
            completed_generation: Default::default(),
            obsolete_cursor: Default::default(),
            direct_credits: std::sync::Arc::new(tokio::sync::Semaphore::new(
                super::subscriber::wallet_queue::MAX_TOTAL_WORK,
            )),
        })
    }
}

#[async_trait]
impl RealmAssignmentReader for PgAssignmentFence {
    async fn current_realm_assignment(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<RealmAssignmentSnapshot, FenceError> {
        tokio::time::timeout(FENCE_CALL_TIMEOUT, self.read_current(wallet, session))
            .await
            .unwrap_or(Err(FenceError::Unavailable))
    }
}

#[async_trait]
impl AssignmentFence for PgAssignmentFence {
    async fn lock_selected(
        &self,
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError> {
        tokio::time::timeout(FENCE_CALL_TIMEOUT, self.selected_lease(snapshot))
            .await
            .unwrap_or(Err(FenceError::Unavailable))
    }
    fn requires_generation(&self) -> bool {
        self.generation_required
    }
    async fn generation_binding(&self) -> Option<crate::interfaces::PulseGenerationBinding> {
        self.current_generation().await.ok()
    }
    async fn accept_selected_hint(
        &self,
        wallet: &str,
        change: &crate::protocol::pulse::decentraland::pulse::PeerClusterChange,
    ) -> bool {
        let Ok(epoch) = self.register_source(&change.source_incarnation).await else {
            return false;
        };
        self.observe_bound_hint(wallet, change, epoch);
        true
    }
    fn selected_reconciliation_targets(&self) -> Vec<(String, String)> {
        let mut targets = Vec::new();
        for (wallet, slot) in self.selected.lock().iter() {
            let desired = slot.desired.lock().session.clone();
            if !desired.is_empty() {
                targets.push((wallet.clone(), desired.clone()));
            }
            if let Ok(state) = slot.state.try_lock()
                && let Some(owned) = &state.owned
                && owned.session != desired
            {
                targets.push((wallet.clone(), owned.session.clone()));
            }
        }
        targets
    }
    async fn complete_generation_view(
        &self,
        binding: &crate::interfaces::PulseGenerationBinding,
    ) -> bool {
        if self.current_generation().await.ok().as_ref() != Some(binding) {
            return false;
        }
        let mut complete = self.completed_generation.lock();
        if complete.as_ref().is_none_or(|(old, _)| old != binding) {
            *complete = Some((binding.clone(), tokio::time::Instant::now()));
        }
        true
    }
    async fn retire_obsolete_generation(
        &self,
        binding: &crate::interfaces::PulseGenerationBinding,
        limit: usize,
        current_targets: &[crate::protocol::bus::selection_view::SelectionTarget],
    ) {
        let _ = tokio::time::timeout(
            FENCE_CALL_TIMEOUT,
            self.retire_obsolete(binding, limit.min(128), current_targets),
        )
        .await;
    }
    async fn recover_selected_target(
        &self,
        binding: &crate::interfaces::PulseGenerationBinding,
        target: &crate::protocol::bus::selection_view::SelectionTarget,
    ) -> bool {
        if !target.valid()
            || self
                .register_source(&binding.source_incarnation)
                .await
                .ok()
                .flatten()
                != Some(binding.epoch)
        {
            return false;
        }
        self.observe_bound_hint(
            &target.wallet,
            &crate::protocol::pulse::decentraland::pulse::PeerClusterChange {
                session: target.session.clone(),
                source_incarnation: binding.source_incarnation.clone(),
                assignment_version: target.assignment_version,
                ..Default::default()
            },
            Some(binding.epoch),
        );
        self.selected
            .lock()
            .get(&target.wallet)
            .is_some_and(|slot| {
                let desired = slot.desired.lock();
                desired.epoch == Some(binding.epoch) && desired.source == binding.source_incarnation
            })
    }
    async fn selected_reconciliation_batch(
        &self,
        limit: usize,
        cold_limit: usize,
    ) -> Vec<(String, String)> {
        use std::ops::Bound::{Excluded, Unbounded};
        let mut targets = Vec::new();
        if limit == 0 {
            return targets;
        }
        let mut cursors = self.selected_cursor.lock().await;
        for (phase, cursor) in cursors.iter_mut().enumerate() {
            let urgent_phase = phase == 0;
            let phase_limit = if urgent_phase {
                limit
            } else {
                limit.min(targets.len().saturating_add(cold_limit))
            };
            let mut remaining = self.selected.lock().len();
            while remaining > 0 && targets.len() < phase_limit {
                // Copy only a bounded page; no map/desired lock spans a yield or SQL await.
                let page: Vec<_> = {
                    let slots = self.selected.lock();
                    let start = cursor
                        .as_ref()
                        .map_or(Unbounded, |key| Excluded(key.clone()));
                    slots
                        .range((start, Unbounded))
                        .take(remaining.min(256))
                        .map(|(wallet, slot)| (wallet.clone(), slot.clone()))
                        .collect()
                };
                if page.is_empty() {
                    if self.selected.lock().is_empty() {
                        break;
                    }
                    *cursor = None;
                    continue;
                }
                for (wallet, slot) in page {
                    *cursor = Some(wallet.clone());
                    remaining -= 1;
                    let now = tokio::time::Instant::now();
                    let mut retry_at = slot.retry_at.lock();
                    if now < *retry_at {
                        continue;
                    }
                    let Ok(state) = slot.state.try_lock() else {
                        continue;
                    };
                    let urgent = state.owned.is_some() || state.attempt.is_some();
                    if urgent != urgent_phase {
                        continue;
                    }
                    let desired = slot.desired.lock();
                    let healthy = state.attempt.is_none()
                        && state
                            .owned
                            .as_ref()
                            .is_some_and(|owned| owned.session == desired.session)
                        && state.key.as_ref().is_some_and(|key| {
                            key.session == desired.session
                                && key.source == desired.source
                                && key.version == desired.version
                        })
                        && state
                            .renewed_at
                            .is_some_and(|at| now.duration_since(at) < Duration::from_secs(30));
                    if healthy {
                        continue;
                    }
                    *retry_at = now + Duration::from_secs(5);
                    let previous = state
                        .owned
                        .as_ref()
                        .filter(|owned| owned.session != desired.session);
                    let owned_first = previous.is_some()
                        && slot
                            .owned_first
                            .fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
                    if owned_first && let Some(owned) = previous {
                        targets.push((wallet.clone(), owned.session.clone()));
                    }
                    if targets.len() < phase_limit && !desired.session.is_empty() {
                        targets.push((wallet.clone(), desired.session.clone()));
                    }
                    if targets.len() < phase_limit
                        && !owned_first
                        && let Some(owned) = previous
                    {
                        targets.push((wallet, owned.session.clone()));
                    }
                    if targets.len() >= phase_limit {
                        break;
                    }
                }
                tokio::task::yield_now().await;
            }
        }
        targets
    }
    fn selected_maintenance_urgent(&self, wallet: &str) -> bool {
        let Some(slot) = self.selected.lock().get(wallet).cloned() else {
            return false;
        };
        slot.state.try_lock().map_or(true, |state| {
            state.owned.is_some() || state.attempt.is_some()
        })
    }
    fn observe_selected_hint(
        &self,
        wallet: &str,
        change: &crate::protocol::pulse::decentraland::pulse::PeerClusterChange,
    ) {
        if !self.generation_required {
            self.observe_bound_hint(wallet, change, None);
        }
    }
    fn selected_retirement_target(
        &self,
        wallet: &str,
        session: &str,
    ) -> Option<SelectedRetirementTarget> {
        let slot = self.selected.lock().get(wallet).cloned()?;
        let desired = slot.desired.lock();
        if desired.session != session {
            return None;
        }
        let lookup = crate::protocol::bus::cluster::SelectionRetirementLookup {
            wallet: wallet.into(),
            session: desired.session.clone(),
            source_incarnation: desired.source.clone(),
            assignment_version: desired.version,
            selected: desired.selected.clone(),
        };
        if !lookup.valid() {
            return None;
        }
        let generation = desired.generation;
        let epoch = desired.epoch;
        drop(desired);
        Some(SelectedRetirementTarget {
            epoch,
            lookup,
            generation,
            slot,
        })
    }
    async fn retire_selected(
        &self,
        target: SelectedRetirementTarget,
        reply: crate::protocol::bus::cluster::SelectionRetirementReply,
    ) {
        let _ = tokio::time::timeout(
            FENCE_CALL_TIMEOUT,
            self.retire_selected_target(target, reply),
        )
        .await;
    }
    async fn settle_idle(&self, wallet: &str) {
        let _ = tokio::time::timeout(FENCE_CALL_TIMEOUT, self.settle_selected_idle(wallet)).await;
    }
    fn retained_work_credit(
        &self,
        wallet: &str,
    ) -> Option<std::sync::Arc<super::subscriber::wallet_queue::WorkCredit>> {
        let slot = self.selected.lock().get(wallet).cloned()?;
        let slot = slot.state.try_lock().ok()?;
        slot.attempt.as_ref().map(|attempt| attempt._credit.clone())
    }
    fn unresolved_mutations(&self) -> usize {
        self.selected
            .lock()
            .values()
            .filter(|slot| {
                slot.state
                    .try_lock()
                    .map_or(true, |slot| slot.attempt.is_some())
            })
            .count()
    }
    async fn lock_realm(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError> {
        tokio::time::timeout(FENCE_CALL_TIMEOUT, self.lock(wallet, session))
            .await
            .unwrap_or(Err(FenceError::Unavailable))
            .map(|lease| lease as Box<dyn RealmAssignmentLease>)
    }
}

// Slots are retained capacity: cancellation never drops the only description of a
// dispatched SQL mutation. The bound matches the subscriber's total work envelope.

#[derive(Clone, Debug)]
struct RealmRow {
    session: String,
    epoch: i64,
    revision: i64,
    token: i64,
    incarnation: String,
    assignment: Option<serde_json::Value>,
    assignment_hash: Option<[u8; 32]>,
    renewed: Option<chrono::NaiveDateTime>,
    lapsed: bool,
    renewal_due: bool,
}
impl RealmRow {
    fn compact(mut self) -> Self {
        self.assignment = None;
        self
    }
    fn document_hash(value: &serde_json::Value) -> Result<[u8; 32], FenceError> {
        use sha2::Digest;
        let bytes = serde_json::to_vec(value).map_err(|_| FenceError::Invalid)?;
        if bytes.len() > MAX_ASSIGNMENT_BYTES {
            return Err(FenceError::Invalid);
        }
        Ok(sha2::Sha256::digest(bytes).into())
    }
    fn same_fact(&self, other: &Self) -> bool {
        self.session == other.session
            && self.epoch == other.epoch
            && self.revision == other.revision
            && self.token == other.token
            && self.incarnation == other.incarnation
            && self.assignment_hash == other.assignment_hash
            && self.renewed == other.renewed
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectedKey {
    session: String,
    source: String,
    version: u64,
    origin: i32,
    identity: u64,
}
impl SelectedKey {
    fn from_snapshot(
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
    ) -> Result<Self, FenceError> {
        let valid = snapshot
            .token_not_before
            .and_then(|floor| PulseAdmissionStamp::from_snapshot(snapshot, floor))
            .is_some_and(|stamp| stamp.valid());
        if !valid {
            return Err(FenceError::Invalid);
        }
        Ok(Self {
            session: snapshot.session.clone(),
            source: snapshot.source_incarnation.clone(),
            version: snapshot.assignment_version,
            origin: snapshot.selected_origin,
            identity: snapshot.selected_identity.ok_or(FenceError::Invalid)?,
        })
    }
}
struct RealmAttempt {
    before: Option<RealmRow>,
    after: RealmRow,
    key: Option<SelectedKey>,
    _credit: std::sync::Arc<super::subscriber::wallet_queue::WorkCredit>,
}
#[derive(Default)]
struct SelectedDesire {
    epoch: Option<u64>,
    session: String,
    source: String,
    version: u64,
    selected: Option<crate::protocol::bus::cluster::SelectedIdentity>,
    generation: u64,
}
impl SelectedDesire {
    fn observe(
        &mut self,
        session: &str,
        source: &str,
        version: u64,
        selected: Option<crate::protocol::bus::cluster::SelectedIdentity>,
        epoch: Option<u64>,
    ) {
        let same = self.session == session
            && self.source == source
            && self.version == version
            && self.epoch == epoch;
        let selected = selected.or_else(|| same.then(|| self.selected.clone()).flatten());
        if same && self.selected == selected {
            return;
        }
        let Some(generation) = self.generation.checked_add(1) else {
            return;
        };
        *self = Self {
            epoch,
            session: session.into(),
            source: source.into(),
            version,
            selected,
            generation,
        };
    }
}
struct SelectedWallet {
    owned_first: std::sync::atomic::AtomicBool,
    retry_at: parking_lot::Mutex<tokio::time::Instant>,
    desired: parking_lot::Mutex<SelectedDesire>,
    state: tokio::sync::Mutex<SelectedBinding>,
}
/// Captures the exact local obligation across the negative authority lookup.
/// Private slot identity and generation cannot be constructed by a caller.
pub struct SelectedRetirementTarget {
    epoch: Option<u64>,
    pub lookup: crate::protocol::bus::cluster::SelectionRetirementLookup,
    generation: u64,
    slot: std::sync::Arc<SelectedWallet>,
}
impl SelectedRetirementTarget {
    fn matches(&self, desired: &SelectedDesire) -> bool {
        self.epoch == desired.epoch
            && self.generation == desired.generation
            && self.lookup.session == desired.session
            && self.lookup.source_incarnation == desired.source
            && self.lookup.assignment_version == desired.version
            && self.lookup.selected == desired.selected
    }
    fn permits_key(&self, key: &SelectedKey) -> bool {
        key.source == self.lookup.source_incarnation
            && key.version <= self.lookup.assignment_version
            && (key.version < self.lookup.assignment_version
                || (key.session == self.lookup.session
                    && self.lookup.selected.as_ref().is_none_or(|selected| {
                        selected.origin == key.origin && selected.identity == key.identity
                    })))
    }
}
#[derive(Default)]
struct SelectedBinding {
    renewed_at: Option<tokio::time::Instant>,
    allocator_below: Option<i64>,
    key: Option<SelectedKey>,
    owned: Option<RealmRow>,
    attempt: Option<RealmAttempt>,
}
#[derive(Clone)]
struct SelectedLease {
    direct_credits: std::sync::Arc<tokio::sync::Semaphore>,
    slot: std::sync::Arc<SelectedWallet>,
    row: RealmRow,
}
#[derive(Debug, PartialEq, Eq)]
enum RealmTransition {
    Own,
    Borrow,
    Conflict,
}
fn realm_transition(
    row: Option<&RealmRow>,
    owned: Option<&RealmRow>,
    session: &str,
) -> RealmTransition {
    match row {
        None => RealmTransition::Own,
        Some(row) if row.lapsed || owned.is_some_and(|owned| owned.same_fact(row)) => {
            RealmTransition::Own
        }
        Some(row) if row.session == session => RealmTransition::Borrow,
        _ => RealmTransition::Conflict,
    }
}
impl PgAssignmentFence {
    fn observe_bound_hint(
        &self,
        wallet: &str,
        change: &crate::protocol::pulse::decentraland::pulse::PeerClusterChange,
        epoch: Option<u64>,
    ) {
        if !super::subscriber::is_session_key(wallet)
            || !super::subscriber::is_session_key(&change.session)
            || change.assignment_version == 0
            || change.source_incarnation.is_empty()
            || change.source_incarnation.len() > 256
        {
            return;
        }
        if let Ok(slot) = self.selected_slot(wallet, &change.session) {
            let mut desired = slot.desired.lock();
            if desired.source.is_empty()
                || desired
                    .epoch
                    .is_some_and(|old| epoch.is_some_and(|new| new > old))
                || (desired.epoch == epoch
                    && (desired.source != change.source_incarnation
                        || change.assignment_version >= desired.version))
            {
                let generation = desired.generation;
                desired.observe(
                    &change.session,
                    &change.source_incarnation,
                    change.assignment_version,
                    None,
                    epoch,
                );
                let changed = generation != desired.generation;
                drop(desired);
                if changed {
                    *slot.retry_at.lock() = tokio::time::Instant::now();
                }
            }
        }
    }
    /// Changes only fixture scheduling; the production runtime still performs every lookup,
    /// settlement and retirement using the actual authority and SQL paths.
    #[cfg(any(test, feature = "test-support"))]
    pub async fn reconcile_selected_now_for_test(&self, wallet: &str) {
        let slot = self.selected.lock().get(wallet).cloned();
        if let Some(slot) = slot {
            slot.state.lock().await.renewed_at = None;
            *slot.retry_at.lock() = tokio::time::Instant::now();
        }
    }

    fn mutation_credit(
        &self,
    ) -> Result<std::sync::Arc<super::subscriber::wallet_queue::WorkCredit>, FenceError> {
        super::subscriber::wallet_queue::current_work_credit()
            .or_else(|| super::subscriber::wallet_queue::WorkCredit::reserve(&self.direct_credits))
            .ok_or(FenceError::Unavailable)
    }
    fn selected_slot(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<std::sync::Arc<SelectedWallet>, FenceError> {
        let mut slots = self.selected.lock();
        if let Some(slot) = slots.get(wallet) {
            return Ok(slot.clone());
        }
        if slots.len() >= self.selected_limit {
            super::metrics::selected_slot_refused();
            return Err(FenceError::Unavailable);
        }
        let mut retired = self.retired.lock();
        let owned = retired.remove(wallet);
        if slots.len() + retired.len() >= self.selected_limit {
            retired.pop_first();
        }
        let slot = std::sync::Arc::new(SelectedWallet {
            owned_first: std::sync::atomic::AtomicBool::new(false),
            retry_at: parking_lot::Mutex::new(tokio::time::Instant::now()),
            desired: parking_lot::Mutex::new(SelectedDesire {
                session: session.into(),
                ..Default::default()
            }),
            state: tokio::sync::Mutex::new(SelectedBinding {
                owned,
                ..Default::default()
            }),
        });
        slots.insert(wallet.into(), slot.clone());
        super::metrics::selected_slot_occupancy(slots.len());
        Ok(slot)
    }
    async fn realm_row(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        wallet: &str,
    ) -> Result<Option<RealmRow>, FenceError> {
        let row = sqlx::query("SELECT owner_session, owner_epoch, assignment_revision, fencing_token, authority_incarnation, assignment_json,
            CASE WHEN isfinite(updated_at) THEN updated_at END AS renewed,
            updated_at < timezone('UTC',clock_timestamp())-interval '90 seconds' AS lapsed,
            updated_at < timezone('UTC',clock_timestamp())-interval '30 seconds' AS renewal_due
            FROM social.archipelago_v4_assignments WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm' FOR UPDATE")
            .bind(&self.audience).bind(wallet).fetch_optional(&mut **tx).await.map_err(|_| FenceError::Unavailable)?;
        row.map(|row| {
            let assignment: Option<serde_json::Value> = row
                .try_get("assignment_json")
                .map_err(|_| FenceError::Invalid)?;
            let assignment_hash = assignment
                .as_ref()
                .map(RealmRow::document_hash)
                .transpose()?;
            let fact = RealmRow {
                session: row
                    .try_get("owner_session")
                    .map_err(|_| FenceError::Invalid)?,
                epoch: row
                    .try_get("owner_epoch")
                    .map_err(|_| FenceError::Invalid)?,
                revision: row
                    .try_get("assignment_revision")
                    .map_err(|_| FenceError::Invalid)?,
                token: row
                    .try_get("fencing_token")
                    .map_err(|_| FenceError::Invalid)?,
                incarnation: row
                    .try_get("authority_incarnation")
                    .map_err(|_| FenceError::Invalid)?,
                assignment,
                assignment_hash,
                renewed: row.try_get("renewed").map_err(|_| FenceError::Invalid)?,
                lapsed: row.try_get("lapsed").map_err(|_| FenceError::Invalid)?,
                renewal_due: row
                    .try_get("renewal_due")
                    .map_err(|_| FenceError::Invalid)?,
            };
            if fact.epoch <= 0
                || fact.revision <= 0
                || fact.token <= 0
                || (fact.renewed.is_none() && !fact.lapsed)
            {
                return Err(FenceError::Invalid);
            }
            Ok(fact)
        })
        .transpose()
    }
    fn reconcile_attempt(slot: &mut SelectedBinding, current: Option<&RealmRow>) {
        if let Some(attempt) = slot.attempt.take() {
            if current.is_some_and(|row| row.same_fact(&attempt.after)) {
                slot.owned = Some(attempt.after.compact());
                if let Some(key) = attempt.key {
                    slot.key = Some(key);
                }
            } else if !(match (&attempt.before, current) {
                (None, None) => true,
                (Some(before), Some(current)) => before.same_fact(current),
                _ => false,
            }) {
                // Another exact owner won. It is never restored, renewed or released.
                slot.owned = None;
                slot.key = None;
            }
        }
    }
    async fn selected_lease(
        &self,
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError> {
        let key = SelectedKey::from_snapshot(snapshot)?;
        let epoch = self.register_source(&key.source).await?;
        let slot_ref = self.selected_slot(&snapshot.wallet, &snapshot.session)?;
        let mut slot = slot_ref.state.lock().await;
        {
            let mut desired = slot_ref.desired.lock();
            if desired.source.is_empty()
                || desired
                    .epoch
                    .is_some_and(|old| epoch.is_some_and(|new| new > old))
                || (desired.epoch == epoch
                    && desired.source == key.source
                    && desired.version <= key.version)
            {
                desired.observe(
                    &snapshot.session,
                    &key.source,
                    key.version,
                    Some(crate::protocol::bus::cluster::SelectedIdentity {
                        origin: key.origin,
                        identity: key.identity,
                    }),
                    epoch,
                );
            }
        }
        let lease = self
            .safety_store()
            .authorize_source(
                &snapshot.wallet,
                None,
                crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS,
                self.generation_required
                    .then_some((self.audience.as_str(), snapshot.source_incarnation.as_str())),
            )
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let mut tx = lease.tx;
        // These server limits bound backend work after a cancelled waiter.
        sqlx::query("SET LOCAL statement_timeout='2000ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        sqlx::query("SET LOCAL lock_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let current = self.realm_row(&mut tx, &snapshot.wallet).await?;
        Self::reconcile_attempt(&mut slot, current.as_ref());
        if slot
            .owned
            .as_ref()
            .is_some_and(|owned| current.as_ref().is_none_or(|row| !owned.same_fact(row)))
        {
            slot.owned = None;
            slot.key = None;
        }
        match realm_transition(current.as_ref(), slot.owned.as_ref(), &snapshot.session) {
            // Borrowing or refusing an external row does not end Pulse's selected
            // retry obligation. The same native peer may have no signaling socket.
            RealmTransition::Conflict => return Err(FenceError::Conflict),
            RealmTransition::Borrow => {
                tx.commit().await.map_err(|_| FenceError::Unavailable)?;
                drop(slot);
                let mut lease = self.lock(&snapshot.wallet, &snapshot.session).await?;
                lease.read_only = true;
                return Ok(lease);
            }
            RealmTransition::Own => {}
        }
        if slot
            .allocator_below
            .is_some_and(|epoch| current.as_ref().is_some_and(|row| row.epoch == epoch))
        {
            return Err(FenceError::Conflict);
        }
        slot.allocator_below = None;
        let unchanged = current
            .as_ref()
            .is_some_and(|row| !row.lapsed && row.session == snapshot.session)
            && slot.key.as_ref() == Some(&key);
        if !unchanged || current.as_ref().is_none_or(|row| row.renewal_due) {
            let renewed = sqlx::query_scalar("SELECT timezone('UTC',clock_timestamp())")
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| FenceError::Unavailable)?;
            let after = if let Some(row) = current.as_ref().filter(|_| unchanged) {
                let mut after = row.clone();
                after.renewed = Some(renewed);
                after.lapsed = false;
                after.renewal_due = false;
                after
            } else {
                let epoch: i64 =
                    sqlx::query_scalar("SELECT nextval('social.archipelago_v4_connection_epoch')")
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|_| FenceError::Unavailable)?;
                if epoch <= 0 || current.as_ref().is_some_and(|row| epoch <= row.epoch) {
                    slot.allocator_below = current.as_ref().map(|row| row.epoch);
                    tracing::warn!(wallet=%snapshot.wallet,allocated_epoch=epoch,"selected realm epoch allocator does not follow the observed owner; admission withheld without allocation retries");
                    return Err(FenceError::Conflict);
                }
                RealmRow {
                    session: snapshot.session.clone(),
                    epoch,
                    revision: current
                        .as_ref()
                        .map_or(Some(1), |row| row.revision.checked_add(1))
                        .ok_or(FenceError::Invalid)?,
                    token: current
                        .as_ref()
                        .map_or(Some(1), |row| row.token.checked_add(1))
                        .ok_or(FenceError::Invalid)?,
                    incarnation: self.incarnation.clone(),
                    assignment: None,
                    assignment_hash: None,
                    renewed: Some(renewed),
                    lapsed: false,
                    renewal_due: false,
                }
            };
            slot.attempt = Some(RealmAttempt {
                before: current.clone(),
                after: after.clone(),
                key: Some(key.clone()),
                _credit: self.mutation_credit()?,
            });
            let changed = if let Some(before) = &current {
                sqlx::query("UPDATE social.archipelago_v4_assignments SET owner_session=$3, owner_epoch=$4, assignment_revision=$5,
                    fencing_token=$6, authority_incarnation=$7, assignment_json=$8, updated_at=$9,
                    acknowledged_revision=CASE WHEN owner_epoch=$4 THEN acknowledged_revision ELSE 0 END
                    WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'
                    AND owner_session=$10 AND owner_epoch=$11 AND assignment_revision=$12 AND fencing_token=$13
                    AND authority_incarnation=$14 AND assignment_json IS NOT DISTINCT FROM $15
                    AND (CASE WHEN isfinite(updated_at) THEN updated_at END) IS NOT DISTINCT FROM $16")
                    .bind(&self.audience).bind(&snapshot.wallet).bind(&after.session).bind(after.epoch).bind(after.revision)
                    .bind(after.token).bind(&after.incarnation).bind(&after.assignment).bind(after.renewed)
                    .bind(&before.session).bind(before.epoch).bind(before.revision).bind(before.token).bind(&before.incarnation)
                    .bind(&before.assignment).bind(before.renewed).execute(&mut *tx).await
            } else {
                sqlx::query("INSERT INTO social.archipelago_v4_assignments(deployment_audience,owner_address,lane_key,authority_incarnation,
                    assignment_revision,owner_session,owner_epoch,fencing_token,updated_at) VALUES($1,$2,'realm',$3,$4,$5,$6,$7,$8)
                    ON CONFLICT(deployment_audience,owner_address,lane_key) DO NOTHING")
                    .bind(&self.audience).bind(&snapshot.wallet).bind(&after.incarnation).bind(after.revision).bind(&after.session)
                    .bind(after.epoch).bind(after.token).bind(after.renewed).execute(&mut *tx).await
            }.map_err(|_| FenceError::Unavailable)?;
            if changed.rows_affected() != 1 {
                return Err(FenceError::Conflict);
            }
            tx.commit().await.map_err(|_| FenceError::Unavailable)?;
            slot.renewed_at = Some(tokio::time::Instant::now());
            slot.owned = Some(after.compact());
            slot.attempt = None;
            slot.key = Some(key);
        } else {
            tx.commit().await.map_err(|_| FenceError::Unavailable)?;
        }
        let owned = slot.owned.clone().ok_or(FenceError::Conflict)?;
        drop(slot);
        let mut lease = self.lock(&snapshot.wallet, &snapshot.session).await?;
        if lease.owner_epoch != owned.epoch
            || lease.fencing_token != owned.token
            || lease.assignment_revision != owned.revision
        {
            return Err(FenceError::Conflict);
        }
        lease.selected = Some(SelectedLease {
            direct_credits: self.direct_credits.clone(),
            slot: slot_ref,
            row: owned,
        });
        Ok(lease)
    }
    async fn settle_selected_idle(&self, wallet: &str) -> Result<(), FenceError> {
        let Some(slot_ref) = self.selected.lock().get(wallet).cloned() else {
            return Ok(());
        };
        let mut slot = slot_ref.state.lock().await;
        let lease = self
            .safety_store()
            .lock(wallet)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let mut tx = lease.tx;
        let current = self.realm_row(&mut tx, wallet).await?;
        Self::reconcile_attempt(&mut slot, current.as_ref());
        tx.commit().await.map_err(|_| FenceError::Unavailable)?;
        if current.as_ref().is_none_or(|row| {
            row.lapsed
                || !slot
                    .owned
                    .as_ref()
                    .is_some_and(|owned| owned.same_fact(row))
        }) {
            // A failed lookup is unknown, including after lease lapse. Retire
            // settled SQL ownership facts, but keep the bounded desired retry.
            slot.owned = None;
            slot.key = None;
        }
        Ok(())
    }

    async fn retire_obsolete(
        &self,
        binding: &crate::interfaces::PulseGenerationBinding,
        limit: usize,
        current_targets: &[crate::protocol::bus::selection_view::SelectionTarget],
    ) -> Result<(), FenceError> {
        if !self
            .completed_generation
            .lock()
            .as_ref()
            .is_some_and(|(complete, at)| {
                complete == binding && at.elapsed() >= super::generation::OPERATION_VALIDITY
            })
        {
            return Ok(());
        }
        let mut cursor = self.obsolete_cursor.lock().await;
        let page: Vec<_> = {
            use std::ops::Bound::{Excluded, Unbounded};
            let slots = self.selected.lock();
            slots
                .range((
                    cursor
                        .as_ref()
                        .map_or(Unbounded, |key| Excluded(key.clone())),
                    Unbounded,
                ))
                .take(limit)
                .map(|(wallet, slot)| (wallet.clone(), slot.clone()))
                .collect()
        };
        if page.is_empty() {
            *cursor = None;
            return Ok(());
        }
        for (wallet, slot) in page {
            *cursor = Some(wallet.clone());
            // The complete view is strictly wallet-sorted. Retain a still-selected
            // predecessor until ordinary reseeding advances this exact slot in place.
            if current_targets
                .binary_search_by(|target| target.wallet.as_str().cmp(wallet.as_str()))
                .is_ok()
            {
                continue;
            }
            let (old_epoch, source, generation) = {
                let desired = slot.desired.lock();
                (desired.epoch, desired.source.clone(), desired.generation)
            };
            // Unregistered historical UUIDs provide no supersession evidence.
            if !old_epoch.is_some_and(|epoch| epoch < binding.epoch) {
                continue;
            }
            let Ok(mut state) = slot.state.try_lock() else {
                continue;
            };
            let lease = self
                .safety_store()
                .lock_with_source(&wallet, Some((&self.audience, &binding.source_incarnation)))
                .await
                .map_err(|_| FenceError::Unavailable)?;
            let mut tx = lease.tx;
            // Settle the exact uncertain SQL attempt; no assignment writes or foreign cleanup.
            // Cleanup/takeover obligations remain in their existing independent durable owners.
            let current = self.realm_row(&mut tx, &wallet).await?;
            tx.commit().await.map_err(|_| FenceError::Unavailable)?;
            Self::reconcile_attempt(&mut state, current.as_ref());
            let mut slots = self.selected.lock();
            let desired = slot.desired.lock();
            if desired.epoch == old_epoch
                && desired.source == source
                && desired.generation == generation
                && state.attempt.is_none()
                && state.key.as_ref().is_none_or(|key| key.source == source)
                && slots
                    .get(&wallet)
                    .is_some_and(|stored| std::sync::Arc::ptr_eq(stored, &slot))
                && std::sync::Arc::strong_count(&slot) == 2
            {
                slots.remove(&wallet);
                if let Some(owned) = state.owned.take() {
                    self.retired.lock().insert(wallet, owned);
                }
                super::metrics::selected_slot_occupancy(slots.len());
            }
        }
        Ok(())
    }

    async fn retire_selected_target(
        &self,
        target: SelectedRetirementTarget,
        reply: crate::protocol::bus::cluster::SelectionRetirementReply,
    ) -> Result<(), FenceError> {
        if reply.lookup != target.lookup || reply.encode().is_none() {
            return Err(FenceError::Invalid);
        }
        let mut state = target.slot.state.lock().await;
        if !target.matches(&target.slot.desired.lock())
            || state
                .key
                .as_ref()
                .is_some_and(|key| !target.permits_key(key))
            || state
                .attempt
                .as_ref()
                .and_then(|attempt| attempt.key.as_ref())
                .is_some_and(|key| !target.permits_key(key))
        {
            return Ok(());
        }
        let lease = self
            .safety_store()
            .lock_with_source(
                &target.lookup.wallet,
                self.generation_required.then_some((
                    self.audience.as_str(),
                    target.lookup.source_incarnation.as_str(),
                )),
            )
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let mut tx = lease.tx;
        let current = self.realm_row(&mut tx, &target.lookup.wallet).await?;
        // A failed settlement read/commit retains the exact attempt and credit.
        // No realm write, renewal, release or credential issuance occurs here.
        tx.commit().await.map_err(|_| FenceError::Unavailable)?;
        Self::reconcile_attempt(&mut state, current.as_ref());
        let mut slots = self.selected.lock();
        let desired = target.slot.desired.lock();
        if !target.matches(&desired)
            || !slots
                .get(&target.lookup.wallet)
                .is_some_and(|slot| std::sync::Arc::ptr_eq(slot, &target.slot))
            || state.attempt.is_some()
            || state
                .key
                .as_ref()
                .is_some_and(|key| !target.permits_key(key))
            || std::sync::Arc::strong_count(&target.slot) != 2
        {
            return Ok(());
        }
        // Only this map and this retirement operation still reference the slot.
        // Settled owned facts cease renewal; the existing finite SQL lease stays.
        slots.remove(&target.lookup.wallet);
        if let Some(owned) = state.owned.take() {
            self.retired.lock().insert(target.lookup.wallet, owned);
        }
        super::metrics::selected_slot_occupancy(slots.len());
        Ok(())
    }
}

struct PgRealmLease {
    generation_required: bool,
    selected: Option<SelectedLease>,
    read_only: bool,
    previous_pulse_admission: Option<PulseAdmissionStamp>,
    pool: PgPool,
    audience: String,
    wallet: String,
    session: String,
    owner_epoch: i64,
    assignment_revision: i64,
    fencing_token: i64,
    previous_island: Option<String>,
    previous_token_expires_at_unix: Option<u64>,
}

impl PgAssignmentFence {
    async fn lock(&self, wallet: &str, session: &str) -> Result<Box<PgRealmLease>, FenceError> {
        // Read an optimistic immutable owner snapshot in a short transaction.
        // The persistent wallet key always precedes the assignment row lock;
        // no ownership transaction survives into a broker or SFU await.
        let safety = self.safety_store();
        let lease = safety
            .lock(wallet)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let mut tx = lease.tx;
        for statement in [
            "SET LOCAL lock_timeout = '3000ms'",
            "SET LOCAL statement_timeout = '5000ms'",
            "SET LOCAL idle_in_transaction_session_timeout = '30000ms'",
        ] {
            sqlx::query(statement)
                .execute(&mut *tx)
                .await
                .map_err(|_| FenceError::Unavailable)?;
        }
        let row = sqlx::query(
            "WITH locked AS MATERIALIZED (
                 SELECT owner_session, owner_epoch, assignment_revision, fencing_token,
                        assignment_json, updated_at
                 FROM social.archipelago_v4_assignments
                 WHERE deployment_audience = $1
                   AND owner_address = $2
                   AND lane_key = $3
                 FOR UPDATE
             )
             SELECT *, updated_at < timezone('UTC',clock_timestamp()) - $4 * interval '1 second' AS owner_lapsed
             FROM locked",
        )
        .bind(&self.audience)
        .bind(wallet)
        .bind(REALM_LANE)
        .bind(OWNER_LAPSE_SECONDS)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| FenceError::Unavailable)?
        .ok_or(FenceError::Unowned)?;
        let owner_session: String = row
            .try_get("owner_session")
            .map_err(|_| FenceError::Invalid)?;
        let owner_lapsed: bool = row
            .try_get("owner_lapsed")
            .map_err(|_| FenceError::Invalid)?;
        if owner_session != session || owner_lapsed {
            return Err(FenceError::Conflict);
        }
        let owner_epoch: i64 = row
            .try_get("owner_epoch")
            .map_err(|_| FenceError::Invalid)?;
        let assignment_revision: i64 = row
            .try_get("assignment_revision")
            .map_err(|_| FenceError::Invalid)?;
        let fencing_token: i64 = row
            .try_get("fencing_token")
            .map_err(|_| FenceError::Invalid)?;
        if owner_epoch <= 0
            || assignment_revision <= 0
            || assignment_revision == i64::MAX
            || fencing_token <= 0
        {
            return Err(FenceError::Invalid);
        }
        let previous: Option<serde_json::Value> = row
            .try_get("assignment_json")
            .map_err(|_| FenceError::Invalid)?;
        let previous_island = previous
            .as_ref()
            .and_then(|value| value.get("islandId"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let previous_token_expires_at_unix = previous
            .as_ref()
            .and_then(|value| value.get("tokenExpiresAtUnix"))
            .and_then(serde_json::Value::as_u64)
            .filter(|expires_at| *expires_at > 0);
        let previous_pulse_admission = previous
            .as_ref()
            .and_then(|value| value.get("pulseAdmission"))
            .and_then(|value| serde_json::from_value::<PulseAdmissionStamp>(value.clone()).ok())
            .filter(PulseAdmissionStamp::valid);
        tx.commit().await.map_err(|_| FenceError::Unavailable)?;
        Ok(Box::new(PgRealmLease {
            generation_required: self.generation_required,
            selected: None,
            read_only: false,
            previous_pulse_admission,
            pool: self.pool.clone(),
            audience: self.audience.clone(),
            wallet: wallet.to_string(),
            session: owner_session,
            owner_epoch,
            assignment_revision,
            fencing_token,
            previous_island,
            previous_token_expires_at_unix,
        }))
    }

    async fn read_current(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<RealmAssignmentSnapshot, FenceError> {
        let row = sqlx::query(
            "SELECT deployment_audience, owner_address, lane_key, authority_incarnation,
                    owner_session, owner_epoch, assignment_revision, fencing_token,
                    assignment_json,
                    updated_at < timezone('UTC',clock_timestamp()) - $4 * interval '1 second' AS owner_lapsed
             FROM social.archipelago_v4_assignments
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND lane_key = $3",
        )
        .bind(&self.audience)
        .bind(wallet)
        .bind(REALM_LANE)
        .bind(OWNER_LAPSE_SECONDS)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| FenceError::Unavailable)?
        .ok_or(FenceError::Unowned)?;

        let audience: String = row
            .try_get("deployment_audience")
            .map_err(|_| FenceError::Invalid)?;
        let wallet: String = row
            .try_get("owner_address")
            .map_err(|_| FenceError::Invalid)?;
        let lane: String = row.try_get("lane_key").map_err(|_| FenceError::Invalid)?;
        let authority_incarnation: String = row
            .try_get("authority_incarnation")
            .map_err(|_| FenceError::Invalid)?;
        let owner_session: String = row
            .try_get("owner_session")
            .map_err(|_| FenceError::Invalid)?;
        let owner_lapsed: bool = row
            .try_get("owner_lapsed")
            .map_err(|_| FenceError::Invalid)?;
        if owner_lapsed {
            return Err(FenceError::Unowned);
        }
        if owner_session != session {
            return Err(FenceError::Conflict);
        }
        let owner_epoch: i64 = row
            .try_get("owner_epoch")
            .map_err(|_| FenceError::Invalid)?;
        let assignment_revision: i64 = row
            .try_get("assignment_revision")
            .map_err(|_| FenceError::Invalid)?;
        let fencing_token: i64 = row
            .try_get("fencing_token")
            .map_err(|_| FenceError::Invalid)?;
        if audience != self.audience
            || audience.is_empty()
            || audience.len() > 256
            || wallet.is_empty()
            || wallet.len() > 256
            || lane != REALM_LANE
            || authority_incarnation.is_empty()
            || authority_incarnation.len() > 256
            || owner_session.is_empty()
            || owner_session.len() > 256
            || owner_epoch <= 0
            || assignment_revision <= 0
            || fencing_token <= 0
        {
            return Err(FenceError::Invalid);
        }
        let assignment: serde_json::Value = row
            .try_get::<Option<serde_json::Value>, _>("assignment_json")
            .map_err(|_| FenceError::Invalid)?
            .ok_or(FenceError::Invalid)?;
        let encoded = serde_json::to_vec(&assignment).map_err(|_| FenceError::Invalid)?;
        if encoded.len() > MAX_ASSIGNMENT_BYTES {
            return Err(FenceError::Invalid);
        }
        let assignment: StoredAssignmentDocument =
            serde_json::from_slice(&encoded).map_err(|_| FenceError::Invalid)?;
        let island_id = assignment.island_id;
        if island_id.is_empty() {
            return Err(FenceError::Invalid);
        }
        let pulse_admission = assignment.pulse_admission;
        if pulse_admission.as_ref().is_some_and(|stamp| {
            !stamp.valid()
                || stamp.wallet != wallet
                || stamp.session != owner_session
                || crate::rooms::island_room_name(&stamp.cluster_id) != island_id
        }) {
            return Err(FenceError::Invalid);
        }

        Ok(RealmAssignmentSnapshot {
            pulse_admission,
            audience,
            wallet,
            lane,
            authority_incarnation,
            owner_session,
            owner_epoch: crate::casts::non_negative(owner_epoch),
            assignment_revision: crate::casts::non_negative(assignment_revision),
            fencing_token: crate::casts::non_negative(fencing_token),
            island_id,
        })
    }
}

#[async_trait]
impl RealmAssignmentLease for PgRealmLease {
    fn read_only(&self) -> bool {
        self.read_only
    }
    fn previous_pulse_admission(&self) -> Option<PulseAdmissionStamp> {
        self.previous_pulse_admission.clone()
    }
    fn token_fence(&self) -> TokenFence {
        TokenFence {
            audience: self.audience.clone(),
            lane: REALM_LANE.to_string(),
            owner_session: self.session.clone(),
            owner_epoch: crate::casts::non_negative(self.owner_epoch),
            assignment_revision: crate::casts::non_negative(self.assignment_revision + 1),
            fencing_token: crate::casts::non_negative(self.fencing_token),
        }
    }

    fn previous_island(&self) -> Option<String> {
        self.previous_island.clone()
    }

    fn previous_token_expires_at_unix(&self) -> Option<u64> {
        self.previous_token_expires_at_unix
    }

    async fn commit(self: Box<Self>, assignment: AssignmentDocument) -> Result<(), FenceError> {
        if self.selected.is_some() {
            tokio::time::timeout(FENCE_CALL_TIMEOUT, self.commit_assignment(assignment))
                .await
                .unwrap_or(Err(FenceError::Unavailable))
        } else {
            self.commit_assignment(assignment).await
        }
    }
}

impl PgRealmLease {
    async fn commit_assignment(
        self: Box<Self>,
        assignment: AssignmentDocument,
    ) -> Result<(), FenceError> {
        if self.read_only {
            return Err(FenceError::Conflict);
        }
        if assignment
            .pulse_admission
            .as_ref()
            .is_some_and(|stamp| stamp.wallet != self.wallet || stamp.session != self.session)
        {
            return Err(FenceError::Invalid);
        }
        let stamp = assignment
            .pulse_admission
            .as_ref()
            .ok_or(FenceError::Invalid)?;
        let mut selected_guard = if let Some(selected) = &self.selected {
            Some(selected.slot.state.lock().await)
        } else {
            None
        };
        let safety = {
            #[cfg(any(test, feature = "test-support"))]
            if !self.generation_required {
                super::safety::SafetyStore::unfenced_test(self.pool.clone())
            } else {
                super::safety::SafetyStore::new(self.pool.clone())
            }
            #[cfg(not(any(test, feature = "test-support")))]
            {
                super::safety::SafetyStore::new(self.pool.clone())
            }
        };
        let mut lease = safety
            .authorize_stamp(stamp, Some(&self.audience), None, false)
            .await
            .map_err(|_| FenceError::Conflict)?;
        let assignment = encode_assignment(assignment)?;
        if let (Some(selected), Some(slot)) = (&self.selected, selected_guard.as_mut()) {
            if slot.attempt.is_some()
                || !slot
                    .owned
                    .as_ref()
                    .is_some_and(|row| row.same_fact(&selected.row))
            {
                return Err(FenceError::Conflict);
            }
            let mut after = selected.row.clone();
            after.revision = self.assignment_revision + 1;
            after.assignment = Some(assignment.clone());
            after.assignment_hash = Some(RealmRow::document_hash(&assignment)?);
            after.renewed = Some(
                sqlx::query_scalar("SELECT timezone('UTC',clock_timestamp())")
                    .fetch_one(&mut *lease.tx)
                    .await
                    .map_err(|_| FenceError::Unavailable)?,
            );
            after.lapsed = false;
            after.renewal_due = false;
            let key = slot.key.clone();
            slot.attempt = Some(RealmAttempt {
                before: Some(selected.row.clone()),
                after,
                key,
                _credit: super::subscriber::wallet_queue::current_work_credit()
                    .or_else(|| {
                        super::subscriber::wallet_queue::WorkCredit::reserve(
                            &selected.direct_credits,
                        )
                    })
                    .ok_or(FenceError::Unavailable)?,
            });
        }
        let mutation = if selected_guard.is_some() {
            "UPDATE social.archipelago_v4_assignments
             SET assignment_revision = assignment_revision + 1,
                 assignment_json = $7,
                 updated_at = COALESCE($10::timestamp, timezone('UTC',now()))
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND lane_key = $3
               AND owner_session = $4
               AND owner_epoch = $5
               AND fencing_token = $6
               AND assignment_revision = $9
               AND updated_at >= timezone('UTC',clock_timestamp()) - $8 * interval '1 second'
             RETURNING assignment_revision"
        } else {
            "UPDATE social.archipelago_v4_assignments
             SET assignment_revision = assignment_revision + 1,
                 assignment_json = $7,
                 updated_at = now()
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND lane_key = $3
               AND owner_session = $4
               AND owner_epoch = $5
               AND fencing_token = $6
               AND assignment_revision = $9
               AND updated_at >= timezone('UTC',clock_timestamp()) - $8 * interval '1 second'
             RETURNING assignment_revision"
        };
        let query = sqlx::query(mutation)
            .bind(&self.audience)
            .bind(&self.wallet)
            .bind(REALM_LANE)
            .bind(&self.session)
            .bind(self.owner_epoch)
            .bind(self.fencing_token)
            .bind(assignment)
            .bind(OWNER_LAPSE_SECONDS)
            .bind(self.assignment_revision);
        let query = if let Some(slot) = selected_guard.as_ref() {
            query.bind(
                slot.attempt
                    .as_ref()
                    .and_then(|attempt| attempt.after.renewed),
            )
        } else {
            query
        };
        let updated = query
            .fetch_optional(&mut *lease.tx)
            .await
            .map_err(|_| FenceError::Unavailable)?
            .ok_or(FenceError::Conflict)?;
        let revision: i64 = updated
            .try_get("assignment_revision")
            .map_err(|_| FenceError::Invalid)?;
        if revision != self.assignment_revision + 1 {
            return Err(FenceError::Conflict);
        }
        lease.commit().await.map_err(|_| FenceError::Unavailable)?;
        if let Some(slot) = selected_guard.as_mut()
            && let Some(attempt) = slot.attempt.take()
        {
            slot.owned = Some(attempt.after.compact());
        }
        Ok(())
    }
}

fn encode_assignment(assignment: AssignmentDocument) -> Result<serde_json::Value, FenceError> {
    if assignment.pulse_admission.as_ref().is_some_and(|stamp| {
        !stamp.valid() || crate::rooms::island_room_name(&stamp.cluster_id) != assignment.island_id
    }) {
        return Err(FenceError::Invalid);
    }
    let bytes = serde_json::to_vec(&assignment).map_err(|_| FenceError::Invalid)?;
    if bytes.len() > MAX_ASSIGNMENT_BYTES {
        return Err(FenceError::Invalid);
    }
    serde_json::from_slice(&bytes).map_err(|_| FenceError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn selected_hints_retain_one_bounded_latest_desire_without_claiming_authority() {
        use crate::protocol::pulse::decentraland::pulse::PeerClusterChange;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap();
        let fence =
            PgAssignmentFence::unfenced_test_with_limit(pool, "hint-test".into(), 1).unwrap();
        let wallet = "0x1111111111111111111111111111111111111111";
        let first = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let second = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let mut hint = PeerClusterChange {
            session: second.into(),
            source_incarnation: "source".into(),
            assignment_version: 2,
            ..Default::default()
        };
        fence.observe_selected_hint(wallet, &hint);
        hint.session = first.into();
        hint.assignment_version = 1;
        fence.observe_selected_hint(wallet, &hint);
        assert_eq!(
            fence.selected_reconciliation_targets(),
            vec![(wallet.into(), second.into())]
        );
        fence.observe_selected_hint("0x2222222222222222222222222222222222222222", &hint);
        assert_eq!(fence.selected.lock().len(), 1);
        let slot = fence.selected.lock().get(wallet).unwrap().clone();
        let state = slot.state.lock().await;
        assert!(state.owned.is_none());
        assert!(state.attempt.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn selected_replay_rotates_beyond_queue_capacity_without_a_global_tick_and_retains_old_sources()
     {
        use crate::protocol::pulse::decentraland::pulse::PeerClusterChange;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap();
        let fence =
            PgAssignmentFence::unfenced_test_with_limit(pool, "replay-test".into(), 1400).unwrap();
        for index in 0..1400 {
            fence.observe_selected_hint(
                &format!("0x{index:040x}"),
                &PeerClusterChange {
                    session: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                    source_incarnation: "old-pulse".into(),
                    assignment_version: 1,
                    ..Default::default()
                },
            );
        }
        let started = tokio::time::Instant::now();
        let mut wallets = std::collections::BTreeSet::new();
        loop {
            let page = fence.selected_reconciliation_batch(128, 128).await;
            assert!(page.len() <= 128);
            if page.is_empty() {
                break;
            }
            for (wallet, _) in page {
                assert!(wallets.insert(wallet));
            }
        }
        assert_eq!(wallets.len(), 1400);
        assert_eq!(
            tokio::time::Instant::now(),
            started,
            "refill cannot wait for a five-second sweep"
        );
        assert_eq!(
            fence.selected.lock().len(),
            1400,
            "unknown old-source obligations never expire"
        );
        let positive = crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
            wallet: "0xffffffffffffffffffffffffffffffffffffffff".into(),
            session: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            cluster_id: "selected".into(),
            source_incarnation: "new-pulse".into(),
            assignment_version: 1,
            assignment_state: 1,
            selected_origin: 1,
            selected_identity: Some(1),
            token_not_before: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            fence.lock_selected(&positive).await,
            Err(FenceError::Unavailable)
        ));
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(
            fence.selected_reconciliation_batch(128, 128).await.len(),
            128
        );
        assert_eq!(
            fence.selected_reconciliation_targets().len(),
            1400,
            "shutdown enumeration is never due-filtered"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn healthy_owned_replay_waits_for_renewal_but_a_new_desire_is_due_immediately() {
        use crate::protocol::pulse::decentraland::pulse::PeerClusterChange;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap();
        let fence =
            PgAssignmentFence::unfenced_test_with_limit(pool, "due-test".into(), 2).unwrap();
        let wallet = "0x1111111111111111111111111111111111111111";
        let hint = PeerClusterChange {
            session: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            source_incarnation: "pulse".into(),
            assignment_version: 1,
            ..Default::default()
        };
        fence.observe_selected_hint(wallet, &hint);
        let slot = fence.selected.lock().get(wallet).unwrap().clone();
        {
            let mut state = slot.state.lock().await;
            state.key = Some(SelectedKey {
                session: hint.session.clone(),
                source: "pulse".into(),
                version: 1,
                origin: 1,
                identity: 1,
            });
            state.owned = Some(RealmRow {
                session: hint.session.clone(),
                epoch: 1,
                revision: 1,
                token: 1,
                incarnation: "gatekeeper".into(),
                assignment: None,
                assignment_hash: None,
                renewed: Some(chrono::Utc::now().naive_utc()),
                lapsed: false,
                renewal_due: false,
            });
            state.renewed_at = Some(tokio::time::Instant::now());
        }
        assert!(
            fence
                .selected_reconciliation_batch(128, 128)
                .await
                .is_empty()
        );
        tokio::time::advance(Duration::from_secs(29)).await;
        assert!(
            fence
                .selected_reconciliation_batch(128, 128)
                .await
                .is_empty()
        );
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(
            fence.selected_reconciliation_batch(128, 128).await,
            vec![(wallet.into(), hint.session.clone())]
        );
        fence.observe_selected_hint(
            wallet,
            &PeerClusterChange {
                assignment_version: 2,
                ..hint.clone()
            },
        );
        assert_eq!(
            fence.selected_reconciliation_batch(128, 128).await,
            vec![(wallet.into(), hint.session.clone())]
        );
        let cold = "0x0000000000000000000000000000000000000000";
        fence.observe_selected_hint(cold, &hint);
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(fence.selected_maintenance_urgent(wallet));
        assert!(!fence.selected_maintenance_urgent(cold));
        assert_eq!(
            fence.selected_reconciliation_batch(1, 1).await,
            vec![(wallet.into(), hint.session.clone())],
            "owned work precedes a lexically earlier cold target"
        );
        assert_eq!(
            fence.selected_reconciliation_batch(1, 1).await,
            vec![(cold.into(), hint.session.clone())]
        );
        {
            let mut state = slot.state.lock().await;
            state.owned = None;
            state.key = None;
        }
        assert!(
            !fence.selected_maintenance_urgent(wallet),
            "settled ownership loss moves retry work into cold class"
        );
    }

    #[tokio::test]
    async fn selected_realm_transitions_require_exact_owned_or_lapsed_predecessors() {
        let row = RealmRow {
            session: "first".into(),
            epoch: 10,
            revision: 3,
            token: 2,
            incarnation: "authority".into(),
            assignment: None,
            assignment_hash: None,
            renewed: Some(chrono::Utc::now().naive_utc()),
            lapsed: false,
            renewal_due: false,
        };
        assert_eq!(
            realm_transition(Some(&row), None, "second"),
            RealmTransition::Conflict
        );
        assert_eq!(
            realm_transition(Some(&row), None, "first"),
            RealmTransition::Borrow
        );
        assert_eq!(
            realm_transition(Some(&row), Some(&row), "second"),
            RealmTransition::Own
        );
        let mut changed = row.clone();
        changed.revision += 1;
        assert_eq!(
            realm_transition(Some(&changed), Some(&row), "second"),
            RealmTransition::Conflict
        );
        let mut lapsed = row.clone();
        lapsed.lapsed = true;
        assert_eq!(
            realm_transition(Some(&lapsed), None, "second"),
            RealmTransition::Own
        );
        assert_eq!(realm_transition(None, None, "second"), RealmTransition::Own);
        let mut attempt = row.clone();
        attempt.epoch = 11;
        assert!(!attempt.same_fact(&row));
        let mut foreign = attempt.clone();
        foreign.token += 1;
        assert!(!attempt.same_fact(&foreign));
        // A retired slot's owned row waits inside the selected bound for the wallet's next slot.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap();
        let fence = PgAssignmentFence::unfenced_test_with_limit(pool, "retired".into(), 2).unwrap();
        for wallet in ["a", "b"] {
            fence.retired.lock().insert(wallet.into(), row.clone());
        }
        let slot = fence.selected_slot("b", "second").unwrap();
        let owned = slot.state.lock().await.owned.clone();
        assert!(owned.unwrap().same_fact(&row));
        // A live slot is never refused for a kept row: the kept row gives its place.
        let slot = fence.selected_slot("d", "second").unwrap();
        assert!(slot.state.lock().await.owned.is_none());
        assert!(fence.retired.lock().is_empty());
        assert!(fence.selected_slot("a", "second").is_err());
    }

    #[test]
    fn selected_wire_vectors_preserve_exact_u64_json_and_refuse_unknown_authority() {
        use crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot;
        use prost::Message;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/pulse_selected_recovery.json"
        )))
        .unwrap();
        for vector in fixture["reply_vectors"]
            .as_array()
            .unwrap()
            .iter()
            .chain(fixture["identity_boundary_vectors"].as_array().unwrap())
        {
            let bytes = hex::decode(vector["wire_hex"].as_str().unwrap()).unwrap();
            let snapshot = PeerClusterSnapshot::decode(bytes.as_slice()).unwrap();
            assert_eq!(snapshot.encode_to_vec(), bytes, "{}", vector["id"]);
            assert_eq!(
                crate::protocol::bus::cluster::selected_evidence(&snapshot),
                vector["expected_selection_evidence_valid"]
                    .as_bool()
                    .unwrap(),
                "{}",
                vector["id"]
            );
            let stamp = PulseAdmissionStamp::from_snapshot(&snapshot, 0);
            if let Some(want) = vector["expected_eligible_to_construct_stamp"].as_bool() {
                assert_eq!(stamp.is_some(), want, "{}", vector["id"]);
            }
            if let Some(stamp) = stamp {
                assert!(stamp.valid());
                let json = serde_json::to_string(&stamp).unwrap();
                let restored: PulseAdmissionStamp = serde_json::from_str(&json).unwrap();
                assert_eq!(restored, stamp);
                assert_eq!(
                    restored.snapshot().selected_identity,
                    snapshot.selected_identity
                );
                assert!(restored.matches_snapshot(&snapshot));
                let mut changed = snapshot.clone();
                changed.selected_origin = if snapshot.selected_origin == 1 { 2 } else { 1 };
                assert!(!stamp.matches_snapshot(&changed));
                let mut changed = snapshot.clone();
                changed.selected_identity =
                    Some(snapshot.selected_identity.unwrap().saturating_sub(1));
                assert!(!stamp.matches_snapshot(&changed));
                let mut changed = snapshot.clone();
                changed.assignment_state = 2;
                assert!(!stamp.matches_snapshot(&changed));
                let mut advanced = snapshot.clone();
                advanced.pass += 1;
                assert!(stamp.matches_snapshot(&advanced));
                let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
                old.as_object_mut().unwrap().remove("selectedOrigin");
                old.as_object_mut().unwrap().remove("selectedIdentity");
                let old: PulseAdmissionStamp = serde_json::from_value(old).unwrap();
                assert!(!old.valid());
            }
        }
    }

    #[test]
    fn old_unknown_and_cleanup_only_snapshots_cannot_create_admission_stamps() {
        use prost::Message;
        let old = crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
            wallet: format!("0x{}", "01".repeat(20)),
            session: format!("0x{}", "02".repeat(20)),
            cluster_id: "c".into(),
            realm: "r".into(),
            source_incarnation: "source".into(),
            pass: 1,
            assignment_version: 1,
            token_not_before: Some(0),
            ..Default::default()
        };
        assert!(
            PulseAdmissionStamp::from_snapshot(&old, 0).is_none(),
            "old reply has no selection authority"
        );
        let mut bytes = old.encode_to_vec();
        bytes.extend([0x50, 1, 0x58, 1, 0x60, 2]);
        let cleanup = crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot::decode(
            bytes.as_slice(),
        )
        .unwrap();
        assert!(
            PulseAdmissionStamp::from_snapshot(&cleanup, 0).is_none(),
            "cleanup-only cannot mint even with no pending entries"
        );
    }

    #[test]
    fn pulse_stamp_is_strict_internal_data_and_preserves_actual_credential_boundary() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/pulse_retained_recovery.json"
        )))
        .unwrap();
        let stamp: PulseAdmissionStamp =
            serde_json::from_value(fixture["internal_assignment_stamp"].clone()).unwrap();
        assert!(stamp.valid());
        assert!(stamp.matches_snapshot(&stamp.snapshot()));
        let mut changed = stamp.clone();
        changed.assignment_version += 1;
        assert!(!changed.matches_snapshot(&stamp.snapshot()));
        let mut invalid = fixture["internal_assignment_stamp"].clone();
        invalid["extra"] = true.into();
        assert!(serde_json::from_value::<PulseAdmissionStamp>(invalid).is_err());
        let mut missing = fixture["internal_assignment_stamp"].clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("credentialNotBefore");
        assert!(serde_json::from_value::<PulseAdmissionStamp>(missing).is_err());
    }

    #[test]
    fn assignment_document_uses_the_shared_authority_shape() {
        let value = serde_json::to_value(AssignmentDocument {
            island_id: "island-a".into(),
            connection_string: "livekit:wss://example".into(),
            from_island_id: Some("island-old".into()),
            token_expires_at_unix: Some(1234),
            peers: Default::default(),
            pulse_admission: None,
        })
        .unwrap();
        assert_eq!(value["islandId"], "island-a");
        assert_eq!(value["connectionString"], "livekit:wss://example");
        assert_eq!(value["fromIslandId"], "island-old");
        assert_eq!(value["tokenExpiresAtUnix"], 1234);
        assert!(value.get("peers").is_none());
    }

    #[test]
    fn assignment_document_must_fit_the_authority_frame_budget() {
        let assignment = AssignmentDocument {
            island_id: "island-a".into(),
            connection_string: "x".repeat(MAX_ASSIGNMENT_BYTES),
            from_island_id: None,
            token_expires_at_unix: None,
            peers: Default::default(),
            pulse_admission: None,
        };
        assert_eq!(encode_assignment(assignment), Err(FenceError::Invalid));
    }
}
