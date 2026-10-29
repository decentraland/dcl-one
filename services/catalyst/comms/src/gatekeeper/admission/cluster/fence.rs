use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;

const REALM_LANE: &str = "realm";
const OWNER_LAPSE_SECONDS: i32 = 90;
const MAX_ASSIGNMENT_BYTES: usize = 64 * 1024 - 2048;

/// Internal credential authority retained through SQL visibility and socket queue waits.
/// This is never projected into a public client packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PulseAdmissionStamp {
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
        if !snapshot.pending_takeovers.is_empty() {
            return None;
        }
        Some(Self {
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
        super::subscriber::is_session_key(&self.wallet)
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
/// `comms::archipelago::control_v4` claims for configured legacy owners), read and
/// committed over the ONE pool. Every call is bounded by
/// [`FENCE_CALL_TIMEOUT`]: a slow database answers `Unavailable`, never a
/// stalled feed.
pub struct PgAssignmentFence {
    pool: PgPool,
    audience: String,
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
        validate_audience(&audience)?;
        Ok(Self { pool, audience })
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
    async fn lock_realm(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError> {
        tokio::time::timeout(FENCE_CALL_TIMEOUT, self.lock(wallet, session))
            .await
            .unwrap_or(Err(FenceError::Unavailable))
    }
}

struct PgRealmLease {
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
    async fn lock(
        &self,
        wallet: &str,
        session: &str,
    ) -> Result<Box<dyn RealmAssignmentLease>, FenceError> {
        // Read an optimistic immutable owner snapshot in a short transaction.
        // The persistent wallet key always precedes the assignment row lock;
        // no ownership transaction survives into a broker or SFU await.
        let safety = super::safety::SafetyStore::new(self.pool.clone());
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
             SELECT *, updated_at < clock_timestamp() - $4 * interval '1 second' AS owner_lapsed
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
                    updated_at < clock_timestamp() - $4 * interval '1 second' AS owner_lapsed
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
            owner_epoch: owner_epoch as u64,
            assignment_revision: assignment_revision as u64,
            fencing_token: fencing_token as u64,
            island_id,
        })
    }
}

#[async_trait]
impl RealmAssignmentLease for PgRealmLease {
    fn previous_pulse_admission(&self) -> Option<PulseAdmissionStamp> {
        self.previous_pulse_admission.clone()
    }
    fn token_fence(&self) -> TokenFence {
        TokenFence {
            audience: self.audience.clone(),
            lane: REALM_LANE.to_string(),
            owner_session: self.session.clone(),
            owner_epoch: self.owner_epoch as u64,
            assignment_revision: (self.assignment_revision + 1) as u64,
            fencing_token: self.fencing_token as u64,
        }
    }

    fn previous_island(&self) -> Option<String> {
        self.previous_island.clone()
    }

    fn previous_token_expires_at_unix(&self) -> Option<u64> {
        self.previous_token_expires_at_unix
    }

    async fn commit(self: Box<Self>, assignment: AssignmentDocument) -> Result<(), FenceError> {
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
        let safety = super::safety::SafetyStore::new(self.pool.clone());
        let mut lease = safety
            .authorize_stamp(stamp, Some(&self.audience), None, false)
            .await
            .map_err(|_| FenceError::Conflict)?;
        let assignment = encode_assignment(assignment)?;
        let updated = sqlx::query(
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
               AND updated_at >= clock_timestamp() - $8 * interval '1 second'
             RETURNING assignment_revision",
        )
        .bind(&self.audience)
        .bind(&self.wallet)
        .bind(REALM_LANE)
        .bind(&self.session)
        .bind(self.owner_epoch)
        .bind(self.fencing_token)
        .bind(assignment)
        .bind(OWNER_LAPSE_SECONDS)
        .bind(self.assignment_revision)
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
        lease.commit().await.map_err(|_| FenceError::Unavailable)
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
