use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub const OWNER_RENEWAL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_LANES_PER_SOCKET: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LaneKey(String);

impl LaneKey {
    pub fn parse(raw: &str) -> Result<Self, V4Error> {
        if raw == "realm" {
            return Ok(Self(raw.to_string()));
        }
        let Some((kind, id)) = raw.split_once(':') else {
            return Err(V4Error::invalid_lane());
        };
        if !matches!(kind, "scene" | "voice" | "world")
            || id.is_empty()
            || id.len() > 96
            || !id
                .as_bytes()
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'/'))
        {
            return Err(V4Error::invalid_lane());
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct V4Owner {
    pub address: String,
    pub session: String,
    pub epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssignmentPayload {
    #[serde(default, skip_serializing)]
    pub pulse_admission: Option<crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp>,
    #[serde(default, skip_serializing)]
    pub token_expires_at_unix: Option<u64>,
    pub island_id: String,
    pub connection_string: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_island_id: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub peers: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentState {
    pub lane: String,
    pub authority_incarnation: String,
    pub assignment_revision: u64,
    pub realm_revision: u64,
    pub owner_session: String,
    pub owner_epoch: u64,
    pub fencing_token: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignment: Option<AssignmentPayload>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RetryClass {
    Never,
    AfterReconnect,
    Later,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicError {
    pub code: u16,
    pub name: &'static str,
    pub retry: RetryClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Error)]
pub enum V4Error {
    #[error("invalid frame")]
    InvalidFrame,
    #[error("protocol violation")]
    Protocol,
    #[error("authentication failed")]
    AuthFailed,
    #[error("stale connection epoch")]
    StaleEpoch,
    #[error("authority unavailable")]
    AuthorityUnavailable,
    #[error("authority conflict")]
    AuthorityConflict,
    #[error("gap requires snapshot")]
    GapRequiresSnapshot,
    #[error("limit exceeded")]
    LimitExceeded,
    #[error("invalid lane")]
    InvalidLane,
}

impl V4Error {
    pub fn public(&self) -> PublicError {
        match self {
            Self::InvalidFrame => PublicError {
                code: 1001,
                name: "invalid_frame",
                retry: RetryClass::Never,
                detail: None,
            },
            Self::Protocol => PublicError {
                code: 1002,
                name: "protocol_violation",
                retry: RetryClass::Never,
                detail: None,
            },
            Self::AuthFailed => PublicError {
                code: 1003,
                name: "auth_failed",
                retry: RetryClass::Never,
                detail: None,
            },
            Self::StaleEpoch => PublicError {
                code: 1004,
                name: "stale_epoch",
                retry: RetryClass::AfterReconnect,
                detail: None,
            },
            Self::AuthorityUnavailable => PublicError {
                code: 1005,
                name: "authority_unavailable",
                retry: RetryClass::Later,
                detail: None,
            },
            Self::AuthorityConflict => PublicError {
                code: 1006,
                name: "authority_conflict",
                retry: RetryClass::AfterReconnect,
                detail: None,
            },
            Self::GapRequiresSnapshot => PublicError {
                code: 1007,
                name: "gap_requires_snapshot",
                retry: RetryClass::Never,
                detail: None,
            },
            Self::LimitExceeded => PublicError {
                code: 1008,
                name: "limit_exceeded",
                retry: RetryClass::Never,
                detail: None,
            },
            Self::InvalidLane => PublicError {
                code: 1009,
                name: "invalid_lane",
                retry: RetryClass::Never,
                detail: None,
            },
        }
    }

    fn invalid_lane() -> Self {
        Self::InvalidLane
    }
}

#[derive(Clone)]
pub enum AssignmentAuthority {
    Pg(Arc<PgAssignmentAuthority>),
    Unavailable { incarnation: String },
}

impl AssignmentAuthority {
    pub async fn pg(pool: PgPool) -> Result<Arc<Self>, sqlx::Error> {
        Self::pg_with_namespace(pool, "default").await
    }

    pub async fn pg_with_namespace(
        pool: PgPool,
        namespace: impl Into<String>,
    ) -> Result<Arc<Self>, sqlx::Error> {
        Ok(Arc::new(Self::Pg(Arc::new(
            PgAssignmentAuthority::prepare(pool, namespace.into()).await?,
        ))))
    }

    pub fn unavailable(replica_id: &str) -> Arc<Self> {
        Arc::new(Self::Unavailable {
            incarnation: format!("unavailable:{replica_id}"),
        })
    }

    pub fn incarnation(&self) -> &str {
        match self {
            Self::Pg(pg) => &pg.incarnation,
            Self::Unavailable { incarnation } => incarnation,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Pg(_))
    }

    pub async fn claim_lanes(
        &self,
        owner: &V4Owner,
        lanes: &[LaneKey],
    ) -> Result<Vec<AssignmentState>, V4Error> {
        match self {
            Self::Pg(pg) => pg.claim_lanes(owner, lanes).await,
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }

    pub async fn next_connection_epoch(&self) -> Result<u64, V4Error> {
        match self {
            Self::Pg(pg) => pg
                .next_connection_epoch()
                .await
                .map_err(|_| V4Error::AuthorityUnavailable),
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }

    pub async fn publish_assignment(
        &self,
        owner: &V4Owner,
        lane: &LaneKey,
        assignment: AssignmentPayload,
    ) -> Result<AssignmentState, V4Error> {
        match self {
            Self::Pg(pg) => pg.publish_assignment(owner, lane, assignment).await,
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }

    pub async fn snapshot(
        &self,
        owner: &V4Owner,
        lane: &LaneKey,
    ) -> Result<AssignmentState, V4Error> {
        match self {
            Self::Pg(pg) => pg.snapshot(owner, lane).await,
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }

    pub async fn ack(
        &self,
        owner: &V4Owner,
        lane: &LaneKey,
        revision: u64,
    ) -> Result<AssignmentState, V4Error> {
        match self {
            Self::Pg(pg) => pg.ack(owner, lane, revision).await,
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }

    pub async fn renew(&self, owner: &V4Owner) -> Result<(), V4Error> {
        match self {
            Self::Pg(pg) => pg.set_renewal(owner, "clock_timestamp()").await,
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }

    pub async fn release(&self, owner: &V4Owner) -> Result<(), V4Error> {
        match self {
            Self::Pg(pg) => pg.set_renewal(owner, "'-infinity'").await,
            Self::Unavailable { .. } => Err(V4Error::AuthorityUnavailable),
        }
    }
}

pub struct PgAssignmentAuthority {
    pool: PgPool,
    incarnation: String,
    namespace: String,
}

impl PgAssignmentAuthority {
    /// Binds to the fence table (`social.archipelago_v4_*`, created by `schema.sql`; the old
    /// service created it at boot, v2 never runs DDL from the runtime role, so an unreadable
    /// table fails this constructor and the caller keeps the v4 control authority unavailable).
    async fn prepare(pool: PgPool, namespace: String) -> Result<Self, sqlx::Error> {
        let minted = Uuid::new_v4().to_string();
        let incarnation: String = sqlx::query_scalar(
            "INSERT INTO social.archipelago_v4_meta (key, value)
             VALUES ('authority_incarnation', $1)
             ON CONFLICT (key) DO UPDATE SET value = social.archipelago_v4_meta.value
             RETURNING value",
        )
        .bind(minted)
        .fetch_one(&pool)
        .await?;
        Ok(Self {
            pool,
            incarnation,
            namespace,
        })
    }

    async fn set_renewal(&self, owner: &V4Owner, renewed_at: &'static str) -> Result<(), V4Error> {
        checked_epoch(owner.epoch)?;
        let changed = sqlx::query(sqlx::AssertSqlSafe(format!(
            "WITH held AS MATERIALIZED (
                SELECT lane_key, updated_at FROM social.archipelago_v4_assignments
                WHERE deployment_audience = $1 AND owner_address = $2
                  AND owner_session = $3 AND owner_epoch = $4
                ORDER BY lane_key FOR UPDATE
             )
             UPDATE social.archipelago_v4_assignments AS current SET updated_at = {renewed_at}
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND owner_session = $3
               AND owner_epoch = $4
               AND EXISTS (SELECT 1 FROM held WHERE held.lane_key = current.lane_key
                 AND held.updated_at >= clock_timestamp() - interval '90 seconds')"
        )))
        .bind(&self.namespace)
        .bind(&owner.address)
        .bind(&owner.session)
        .bind(owner.epoch as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| V4Error::AuthorityUnavailable)?;
        if changed.rows_affected() == 0 {
            Err(V4Error::AuthorityConflict)
        } else {
            Ok(())
        }
    }

    async fn next_connection_epoch(&self) -> Result<u64, sqlx::Error> {
        let value: i64 =
            sqlx::query_scalar("SELECT nextval('social.archipelago_v4_connection_epoch')")
                .fetch_one(&self.pool)
                .await?;
        Ok(value.max(1) as u64)
    }

    async fn claim_lanes(
        &self,
        owner: &V4Owner,
        lanes: &[LaneKey],
    ) -> Result<Vec<AssignmentState>, V4Error> {
        checked_epoch(owner.epoch)?;
        if lanes.is_empty() || lanes.len() > MAX_LANES_PER_SOCKET {
            return Err(V4Error::LimitExceeded);
        }
        let mut lanes = lanes.to_vec();
        lanes.sort_unstable();
        if lanes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(V4Error::InvalidLane);
        }
        let mut tx = if lanes.iter().any(|lane| lane.as_str() == "realm") {
            crate::gatekeeper::admission::cluster::safety::SafetyStore::new(self.pool.clone())
                .lock(&owner.address)
                .await
                .map_err(|_| V4Error::AuthorityUnavailable)?
                .tx
        } else {
            self.pool
                .begin()
                .await
                .map_err(|_| V4Error::AuthorityUnavailable)?
        };
        let mut out = Vec::with_capacity(lanes.len());
        for lane in &lanes {
            out.push(self.claim_lane(&mut tx, owner, lane).await?);
        }
        tx.commit()
            .await
            .map_err(|_| V4Error::AuthorityUnavailable)?;
        Ok(out)
    }

    async fn claim_lane(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &V4Owner,
        lane: &LaneKey,
    ) -> Result<AssignmentState, V4Error> {
        let row = sqlx::query(
            "INSERT INTO social.archipelago_v4_assignments (
                deployment_audience, owner_address, lane_key, authority_incarnation, assignment_revision,
                owner_session, owner_epoch, fencing_token
             )
             VALUES ($1, $2, $3, $4, 1, $5, $6, 1)
             ON CONFLICT (deployment_audience, owner_address, lane_key) DO UPDATE SET
                assignment_revision = CASE
                    WHEN social.archipelago_v4_assignments.owner_session = EXCLUDED.owner_session
                     AND social.archipelago_v4_assignments.owner_epoch = EXCLUDED.owner_epoch
                    THEN social.archipelago_v4_assignments.assignment_revision
                    ELSE social.archipelago_v4_assignments.assignment_revision + 1
                END,
                owner_address = EXCLUDED.owner_address,
                owner_session = EXCLUDED.owner_session,
                owner_epoch = EXCLUDED.owner_epoch,
                fencing_token = CASE
                    WHEN social.archipelago_v4_assignments.owner_session = EXCLUDED.owner_session
                     AND social.archipelago_v4_assignments.owner_epoch = EXCLUDED.owner_epoch
                    THEN social.archipelago_v4_assignments.fencing_token
                    ELSE social.archipelago_v4_assignments.fencing_token + 1
                END,
                assignment_json = CASE
                    WHEN social.archipelago_v4_assignments.owner_session = EXCLUDED.owner_session
                     AND social.archipelago_v4_assignments.owner_epoch = EXCLUDED.owner_epoch
                    THEN social.archipelago_v4_assignments.assignment_json
                    ELSE NULL
                END,
                acknowledged_revision = CASE
                    WHEN social.archipelago_v4_assignments.owner_session = EXCLUDED.owner_session
                     AND social.archipelago_v4_assignments.owner_epoch = EXCLUDED.owner_epoch
                    THEN social.archipelago_v4_assignments.acknowledged_revision
                    ELSE 0
                END,
                updated_at = clock_timestamp()
             WHERE EXCLUDED.owner_epoch > social.archipelago_v4_assignments.owner_epoch
                OR (
                    EXCLUDED.owner_epoch = social.archipelago_v4_assignments.owner_epoch
                    AND EXCLUDED.owner_session = social.archipelago_v4_assignments.owner_session
                    AND social.archipelago_v4_assignments.updated_at >= clock_timestamp() - interval '90 seconds'
                )
             RETURNING lane_key, authority_incarnation, assignment_revision, owner_session,
                       owner_epoch, fencing_token, assignment_json",
        )
        .bind(&self.namespace)
        .bind(&owner.address)
        .bind(lane.as_str())
        .bind(&self.incarnation)
        .bind(&owner.session)
        .bind(owner.epoch as i64)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| V4Error::AuthorityUnavailable)?;
        row.ok_or(V4Error::AuthorityConflict)
            .and_then(state_from_row)
    }

    async fn publish_assignment(
        &self,
        owner: &V4Owner,
        lane: &LaneKey,
        assignment: AssignmentPayload,
    ) -> Result<AssignmentState, V4Error> {
        checked_epoch(owner.epoch)?;
        let stamp = assignment.pulse_admission.clone();
        let mut assignment = serde_json::to_value(assignment).map_err(|_| V4Error::InvalidFrame)?;
        if let Some(stamp) = stamp.as_ref() {
            if !stamp.valid()
                || stamp.wallet != owner.address
                || stamp.session != owner.session
                || crate::rooms::island_room_name(&stamp.cluster_id) != assignment["islandId"]
            {
                return Err(V4Error::InvalidFrame);
            }
            assignment["pulseAdmission"] =
                serde_json::to_value(stamp).map_err(|_| V4Error::InvalidFrame)?;
        }
        if serde_json::to_vec(&assignment)
            .map_err(|_| V4Error::InvalidFrame)?
            .len()
            > MAX_FRAME_BYTES - 2048
        {
            return Err(V4Error::LimitExceeded);
        }
        // Stamped island candidates share the same wallet-before-owner
        // transaction as gatekeeper publication. The original stamp cannot be
        // relabeled after a cleanup generation/floor changes.
        let mut safety = if let Some(stamp) = stamp.as_ref() {
            Some(
                crate::gatekeeper::admission::cluster::safety::SafetyStore::new(self.pool.clone())
                    .authorize_stamp(stamp, Some(&self.namespace), None, false)
                    .await
                    .map_err(|_| V4Error::AuthorityUnavailable)?,
            )
        } else {
            None
        };
        let query = sqlx::query(
            "WITH held AS MATERIALIZED (
                SELECT updated_at FROM social.archipelago_v4_assignments
                WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = $3
                  AND owner_session = $4 AND owner_epoch = $5 FOR UPDATE
             )
             UPDATE social.archipelago_v4_assignments
             SET assignment_revision = assignment_revision + 1,
                 assignment_json = $6,
                 updated_at = clock_timestamp()
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND lane_key = $3
               AND owner_session = $4
               AND owner_epoch = $5
               AND (SELECT updated_at FROM held) >= clock_timestamp() - interval '90 seconds'
             RETURNING lane_key, authority_incarnation, assignment_revision, owner_session,
                       owner_epoch, fencing_token, assignment_json",
        )
        .bind(&self.namespace)
        .bind(&owner.address)
        .bind(lane.as_str())
        .bind(&owner.session)
        .bind(owner.epoch as i64)
        .bind(assignment);
        let row = if let Some(lease) = safety.as_mut() {
            query.fetch_optional(&mut *lease.tx).await
        } else {
            query.fetch_optional(&self.pool).await
        }
        .map_err(|_| V4Error::AuthorityUnavailable)?;
        if let Some(lease) = safety {
            lease
                .commit()
                .await
                .map_err(|_| V4Error::AuthorityUnavailable)?;
        }
        row.ok_or(V4Error::AuthorityConflict)
            .and_then(state_from_row)
    }

    async fn snapshot(&self, owner: &V4Owner, lane: &LaneKey) -> Result<AssignmentState, V4Error> {
        checked_epoch(owner.epoch)?;
        let row = sqlx::query(
            "SELECT lane_key, authority_incarnation, assignment_revision, owner_session,
                    owner_epoch, fencing_token, assignment_json,
                    updated_at >= clock_timestamp() - interval '90 seconds' AS live
             FROM social.archipelago_v4_assignments
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND lane_key = $3
               AND owner_session = $4
               AND owner_epoch = $5",
        )
        .bind(&self.namespace)
        .bind(&owner.address)
        .bind(lane.as_str())
        .bind(&owner.session)
        .bind(owner.epoch as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| V4Error::AuthorityUnavailable)?;
        let row = row.ok_or(V4Error::AuthorityConflict)?;
        if !row
            .try_get::<bool, _>("live")
            .map_err(|_| V4Error::AuthorityUnavailable)?
        {
            return Err(V4Error::AuthorityUnavailable);
        }
        state_from_row(row)
    }

    async fn ack(
        &self,
        owner: &V4Owner,
        lane: &LaneKey,
        revision: u64,
    ) -> Result<AssignmentState, V4Error> {
        checked_epoch(owner.epoch)?;
        let revision = checked_i64(revision)?;
        let row = sqlx::query(
            "WITH held AS MATERIALIZED (
                SELECT updated_at FROM social.archipelago_v4_assignments
                WHERE deployment_audience = $1 AND owner_address = $2 AND lane_key = $3
                  AND owner_session = $4 AND owner_epoch = $5 FOR UPDATE
             )
             UPDATE social.archipelago_v4_assignments
             SET acknowledged_revision = GREATEST(acknowledged_revision, $6),
                 updated_at = clock_timestamp()
             WHERE deployment_audience = $1
               AND owner_address = $2
               AND lane_key = $3
               AND owner_session = $4
               AND owner_epoch = $5
               AND assignment_revision >= $6
               AND (SELECT updated_at FROM held) >= clock_timestamp() - interval '90 seconds'
             RETURNING lane_key, authority_incarnation, assignment_revision, owner_session,
                       owner_epoch, fencing_token, assignment_json",
        )
        .bind(&self.namespace)
        .bind(&owner.address)
        .bind(lane.as_str())
        .bind(&owner.session)
        .bind(owner.epoch as i64)
        .bind(revision)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| V4Error::AuthorityUnavailable)?;
        row.ok_or(V4Error::AuthorityConflict)
            .and_then(state_from_row)
    }
}

fn state_from_row(row: sqlx::postgres::PgRow) -> Result<AssignmentState, V4Error> {
    let failure = |_| V4Error::AuthorityUnavailable;
    let revision: i64 = row.try_get("assignment_revision").map_err(failure)?;
    let owner_epoch: i64 = row.try_get("owner_epoch").map_err(failure)?;
    let fencing_token: i64 = row.try_get("fencing_token").map_err(failure)?;
    let assignment: Option<serde_json::Value> = row.try_get("assignment_json").map_err(failure)?;
    if revision <= 0 || owner_epoch <= 0 || fencing_token <= 0 {
        return Err(V4Error::AuthorityUnavailable);
    }
    Ok(AssignmentState {
        lane: row.try_get("lane_key").map_err(failure)?,
        authority_incarnation: row.try_get("authority_incarnation").map_err(failure)?,
        assignment_revision: revision as u64,
        realm_revision: revision as u64,
        owner_session: row.try_get("owner_session").map_err(failure)?,
        owner_epoch: owner_epoch as u64,
        fencing_token: fencing_token as u64,
        assignment: assignment
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| V4Error::AuthorityUnavailable)?,
    })
}

fn checked_i64(value: u64) -> Result<i64, V4Error> {
    i64::try_from(value).map_err(|_| V4Error::LimitExceeded)
}

fn checked_epoch(value: u64) -> Result<i64, V4Error> {
    if value == 0 {
        return Err(V4Error::StaleEpoch);
    }
    checked_i64(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_assignment_expiry_is_not_part_of_the_client_payload() {
        let document = serde_json::json!({
            "islandId": "island-current",
            "connectionString": "livekit:wss://example.invalid/current",
            "tokenExpiresAtUnix": 2_000_000_000_u64
        });
        let assignment: AssignmentPayload = serde_json::from_value(document).unwrap();
        assert_eq!(
            serde_json::to_value(assignment).unwrap(),
            serde_json::json!({
                "islandId": "island-current",
                "connectionString": "livekit:wss://example.invalid/current"
            })
        );
    }

    #[test]
    fn lanes_are_typed_and_independent() {
        assert!(LaneKey::parse("realm").is_ok());
        assert!(LaneKey::parse("scene:abc").is_ok());
        assert!(LaneKey::parse("voice:room-1").is_ok());
        assert!(LaneKey::parse("world:world.example").is_ok());
        assert!(LaneKey::parse("realm:scene").is_err());
        assert!(LaneKey::parse("scene:").is_err());
    }
    #[tokio::test]
    async fn durable_stamped_publication_checks_wallet_generation_floor_hazard_and_missing_stamp_generation()
     {
        use crate::gatekeeper::admission::cluster::{
            fence::PulseAdmissionStamp, safety::SafetyStore,
        };
        use crate::protocol::pulse::decentraland::pulse::{PeerClusterSnapshot, PendingTakeover};
        if std::env::var("CATALYST_TEST_DATABASE_URL")
            .ok()
            .is_none_or(|value| value.trim().is_empty())
        {
            assert_ne!(
                std::env::var("GITHUB_ACTIONS").as_deref(),
                Ok("true"),
                "mandatory PostgreSQL in CI"
            );
            eprintln!(
                "local skip: durable publication regression requires CATALYST_TEST_DATABASE_URL"
            );
            return;
        }
        let pool = crate::db::scratch::pool(4).await.unwrap();
        let authority = AssignmentAuthority::pg_with_namespace(
            pool.clone(),
            format!("publish-{}", Uuid::new_v4()),
        )
        .await
        .unwrap();
        let lane = LaneKey::parse("realm").unwrap();
        for change in [
            "unchanged",
            "generation",
            "floor",
            "hazard",
            "generation-zero",
        ] {
            let owner = V4Owner {
                address: format!("0x00000000{}", Uuid::new_v4().simple()),
                session: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                epoch: authority.next_connection_epoch().await.unwrap(),
            };
            authority
                .claim_lanes(&owner, &[lane.clone()])
                .await
                .unwrap();
            let store = SafetyStore::new(pool.clone());
            let safety = store.snapshot(&owner.address).await.unwrap();
            let plan = PeerClusterSnapshot {
                wallet: owner.address.clone(),
                session: owner.session.clone(),
                cluster_id: "current".into(),
                realm: "realm".into(),
                source_incarnation: "publication-test".into(),
                assignment_version: 1,
                token_not_before: Some(0),
                ..Default::default()
            };
            let mut stamp = PulseAdmissionStamp::from_snapshot(&plan, 0).unwrap();
            stamp.safety_generation = safety.generation;
            stamp.safety_token_floor = safety.token_floor;
            match change {
                "generation" => {
                    sqlx::query("UPDATE social.island_admission_fences SET generation=generation+1 WHERE wallet=$1").bind(&owner.address).execute(&pool).await.unwrap();
                }
                "floor" => {
                    sqlx::query(
                        "UPDATE social.island_admission_fences SET token_floor=1 WHERE wallet=$1",
                    )
                    .bind(&owner.address)
                    .execute(&pool)
                    .await
                    .unwrap();
                }
                "hazard" => {
                    let op = PendingTakeover {
                        takeover_id: "publication".into(),
                        displaced_session: owner.session.clone(),
                        displaced_cluster_id: "old".into(),
                        displaced_realm: "realm".into(),
                    };
                    store
                        .prepare_takeover(&owner.address, "other-source", &owner.session, &op)
                        .await
                        .unwrap();
                    store
                        .begin_takeover(&owner.address, "other-source", &op, 0)
                        .await
                        .unwrap()
                        .unwrap();
                }
                "generation-zero" => stamp.safety_generation = 0,
                _ => {}
            }
            let result = authority
                .publish_assignment(
                    &owner,
                    &lane,
                    AssignmentPayload {
                        pulse_admission: Some(stamp),
                        island_id: "island-current".into(),
                        connection_string: "credential".into(),
                        from_island_id: None,
                        token_expires_at_unix: None,
                        peers: Default::default(),
                    },
                )
                .await;
            assert_eq!(result.is_ok(), change == "unchanged", "{change}");
            let current = authority.snapshot(&owner, &lane).await.unwrap();
            assert_eq!(current.assignment.is_some(), change == "unchanged");
        }
    }
}
