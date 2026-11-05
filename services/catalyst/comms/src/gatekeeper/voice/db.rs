//! Voice presence over `social.private_voice_chats` +
//! `social.private_voice_chat_participants` (a call and its presence share one
//! lifecycle; the room is `voice-chat-private-<call id>`) and
//! `social.community_voice_chat_users` keyed by `(community_id, address)` (the
//! room is `voice-chat-community-<community id>`). Presence is not
//! permission: the join paths re-check the platform ban before a row lands.
//! Liveness is `status`/`joined_at`/`status_updated_at` against a TTL, and
//! every cutoff is computed here and bound, never spelled into the statement.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDateTime};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{Address, CommunityId};

use crate::shared::now_utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceChatUserStatus {
    Connected,
    ConnectionInterrupted,
    Disconnected,
    NotConnected,
}

impl VoiceChatUserStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            VoiceChatUserStatus::Connected => "connected",
            VoiceChatUserStatus::ConnectionInterrupted => "connection_interrupted",
            VoiceChatUserStatus::Disconnected => "disconnected",
            VoiceChatUserStatus::NotConnected => "not_connected",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceDbConfig {
    pub connection_interrupted_ttl_ms: i64,
    pub initial_connection_ttl_ms: i64,
    pub community_no_moderator_ttl_ms: i64,
    pub expired_batch_size: i64,
}

impl Default for VoiceDbConfig {
    fn default() -> Self {
        Self {
            connection_interrupted_ttl_ms: 300_000,
            initial_connection_ttl_ms: 300_000,
            community_no_moderator_ttl_ms: 300_000,
            expired_batch_size: 50,
        }
    }
}

/// The cutoffs of one statement, computed once from `now`.
struct Cutoffs {
    interrupted: NaiveDateTime,
    initial: NaiveDateTime,
    no_moderator: NaiveDateTime,
}

impl VoiceDbConfig {
    /// The comms liveness predicate for a caller's SQL alias and cutoff bind positions.
    /// Arguments are SQL identifiers supplied by trusted in-tree query builders.
    pub fn connected_sql(alias: &str, interrupted: &str, initial: &str) -> String {
        let prefix = if alias.is_empty() {
            String::new()
        } else {
            format!("{alias}.")
        };
        CONNECTED_SQL
            .replace("status_updated_at", &format!("{prefix}status_updated_at"))
            .replace("joined_at", &format!("{prefix}joined_at"))
            .replace("status =", &format!("{prefix}status ="))
            .replace("$1", "__INTERRUPTED_CUTOFF__")
            .replace("$2", initial)
            .replace("__INTERRUPTED_CUTOFF__", interrupted)
    }

    pub fn sql_cutoffs(&self) -> (NaiveDateTime, NaiveDateTime) {
        let values = self.cutoffs();
        (values.interrupted, values.initial)
    }

    fn cutoffs(&self) -> Cutoffs {
        let now = now_utc();
        Cutoffs {
            interrupted: now - Duration::milliseconds(self.connection_interrupted_ttl_ms),
            initial: now - Duration::milliseconds(self.initial_connection_ttl_ms),
            no_moderator: now - Duration::milliseconds(self.community_no_moderator_ttl_ms),
        }
    }
}

/// The "still counts as present" predicate over `$1` = the interrupted
/// cutoff and `$2` = the initial-connection cutoff.
const CONNECTED_SQL: &str = "(status = 'connected' \
     OR (status = 'connection_interrupted' AND status_updated_at > $1) \
     OR (status = 'not_connected' AND joined_at > $2))";

#[derive(Debug, Clone)]
pub struct VoiceChatUserRow {
    pub address: String,
    pub status: String,
    pub joined_at: i64,
    pub status_updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct CommunityVoiceChatUserRow {
    pub address: String,
    pub is_moderator: bool,
    pub status: String,
    pub joined_at: i64,
    pub status_updated_at: i64,
    pub sid: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActiveCommunityVoiceChat {
    pub community_id: CommunityId,
    pub participant_count: i64,
    pub moderator_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateJoin {
    Inactive,
    NotInRoom,
    Joined { old_call: Uuid },
}

/// An accepted call that was ended: its row, and who was in it.
#[derive(Debug, Clone)]
pub struct EndedPrivateCall {
    pub call: PrivateVoiceChatRow,
    pub users: Vec<String>,
}

#[derive(Debug)]
pub enum DeleteRoomError {
    RoomDoesNotExist,
    Db(sqlx::Error),
}

#[derive(Debug)]
pub enum CreatePrivateCallError {
    /// The room exists for another party pair: its id cannot admit these wallets.
    PartiesDiffer,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for CreatePrivateCallError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

#[derive(Debug)]
pub enum CommunityJoinError {
    /// `community_voice_chat_users.community_id` references `communities`:
    /// a room can only exist for a community this database holds.
    CommunityNotFound,
    Db(sqlx::Error),
}

fn ms(t: NaiveDateTime) -> i64 {
    t.and_utc().timestamp_millis()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateVoiceChatRow {
    pub id: Uuid,
    pub caller_address: String,
    pub callee_address: String,
}

pub fn voice_row(r: &sqlx::postgres::PgRow) -> PrivateVoiceChatRow {
    PrivateVoiceChatRow {
        id: r.get::<Uuid, _>("id"),
        caller_address: r.get::<String, _>("caller_address"),
        callee_address: r.get::<String, _>("callee_address"),
    }
}

/// The outcome of creating a pending private call.
#[derive(Debug, Default)]
pub struct StartOutcome {
    /// The new call, or `None` when a participant is busy.
    pub started: Option<Uuid>,
}

/// A pending call to delete: one nobody accepted, so it has no participant
/// rows. An accepted call ends through `end_private_call` instead, which
/// deletes its room.
#[derive(Debug, Clone)]
pub enum PendingPrivateCall {
    Id(Uuid),
    /// The latest pending call of a disconnecting address.
    Address(String),
    /// Rejected: only by its callee (upstream `voice.ts:156-157`).
    Rejected {
        id: Uuid,
        callee: String,
    },
    /// Ended: only by one of its parties (upstream `voice.ts:189-191`).
    Ended {
        id: Uuid,
        party: String,
    },
}

/// Locks the source rows until required media cleanup has succeeded. Dropping
/// it rolls back without losing the old-call identity needed by a retry.
pub(super) struct PreparedPrivateJoin<'a> {
    txn: sqlx::Transaction<'a, sqlx::Postgres>,
    pub outcome: PrivateJoin,
    address: String,
    call_id: Uuid,
}

impl PreparedPrivateJoin<'_> {
    pub async fn commit(mut self) -> Result<PrivateJoin, sqlx::Error> {
        if let PrivateJoin::Joined { old_call } = &self.outcome {
            sqlx::query("UPDATE social.private_voice_chat_participants SET status = CASE WHEN call_id = $1 THEN 'connected' ELSE 'disconnected' END, status_updated_at = $4 WHERE address = $2 AND call_id IN ($1, $3)")
                .bind(self.call_id).bind(&self.address).bind(old_call).bind(now_utc())
                .execute(&mut *self.txn).await?;
        }
        self.txn.commit().await?;
        Ok(self.outcome)
    }
}

#[derive(Clone)]
pub struct VoiceDb {
    pool: PgPool,
    cfg: VoiceDbConfig,
}

impl VoiceDb {
    pub fn new(pool: PgPool, cfg: VoiceDbConfig) -> Self {
        Self { pool, cfg }
    }

    /// Test setup only: insert a pending call without the busy check.
    #[cfg(any(test, feature = "test-support"))]
    pub async fn seed_pending_private_call(
        &self,
        caller: &str,
        callee: &str,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO social.private_voice_chats (id, caller_address, callee_address) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(caller.to_lowercase())
            .bind(callee.to_lowercase())
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    /// Starts the call unless a participant is busy. Sorted per-participant
    /// advisory locks serialize overlapping starts. Pending calls remain busy
    /// until the expiry sweep deletes them, including calls older than its TTL.
    pub async fn create_pending_private_call(
        &self,
        caller: &str,
        callee: &str,
        _expiration_ms: i64,
    ) -> Result<StartOutcome, sqlx::Error> {
        let caller = caller.to_lowercase();
        let callee = callee.to_lowercase();
        let mut participants = vec![caller.clone(), callee.clone()];
        participants.sort();

        let mut tx = self.pool.begin().await?;
        // The fresh probe after waiting for a participant lock must see the
        // previous winner even when the role default is REPEATABLE READ.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('private-voice:' || p)) FROM unnest($1::text[]) AS t(p)")
            .bind(&participants)
            .execute(&mut *tx)
            .await?;
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO social.private_voice_chats (id, caller_address, callee_address) \
             SELECT $1::uuid, $2::text, $3::text \
             WHERE NOT EXISTS ( \
               SELECT 1 FROM social.private_voice_chats \
               WHERE (caller_address = ANY($4) OR callee_address = ANY($4)) \
                 AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = private_voice_chats.id) \
             ) \
             RETURNING id",
        )
        .bind(Uuid::new_v4())
        .bind(&caller)
        .bind(&callee)
        .bind(&participants)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(StartOutcome { started: inserted })
    }

    /// One bounded batch of expired pending calls, oldest first, deleted and
    /// returned. An accepted call (it has participant rows) never expires.
    pub async fn expire_pending_private_calls(
        &self,
        expiration_ms: i64,
        limit: i64,
    ) -> Result<Vec<PrivateVoiceChatRow>, sqlx::Error> {
        let mut txn = self.pool.begin().await?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM social.private_voice_chats WHERE created_at < $1 \
             AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id=private_voice_chats.id) \
             ORDER BY created_at, id LIMIT $2 FOR UPDATE SKIP LOCKED"
        ).bind(now_utc() - Duration::milliseconds(expiration_ms)).bind(limit)
            .fetch_all(&mut *txn).await?;
        let rows = sqlx::query(
            "DELETE FROM social.private_voice_chats WHERE id=ANY($1) \
             AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id=private_voice_chats.id) \
             RETURNING id, caller_address, callee_address"
        ).bind(ids).fetch_all(&mut *txn).await?;
        txn.commit().await?;
        Ok(rows.iter().map(voice_row).collect())
    }

    /// Deletes the pending call and returns the facts social needs for
    /// signalling; `None` when there is no such pending call (an accepted one
    /// is never touched here).
    pub async fn finish_pending_private_call(
        &self,
        call: PendingPrivateCall,
    ) -> Result<Option<PrivateVoiceChatRow>, sqlx::Error> {
        let mut txn = self.pool.begin().await?;
        match &call {
            PendingPrivateCall::Address(address) => {
                let _: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM social.private_voice_chats WHERE caller_address=$1 OR callee_address=$1 ORDER BY id FOR UPDATE")
                    .bind(address.to_lowercase()).fetch_all(&mut *txn).await?;
            }
            PendingPrivateCall::Id(id)
            | PendingPrivateCall::Rejected { id, .. }
            | PendingPrivateCall::Ended { id, .. } => {
                let _: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM social.private_voice_chats WHERE id=$1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut *txn)
                .await?;
            }
        }
        let row = match call {
            PendingPrivateCall::Id(id) => sqlx::query(
                "DELETE FROM social.private_voice_chats WHERE id = $1 \
                   AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = private_voice_chats.id) \
                 RETURNING id, caller_address, callee_address",
            ).bind(id).fetch_optional(&mut *txn).await?,
            PendingPrivateCall::Rejected { id, callee } => sqlx::query(
                "DELETE FROM social.private_voice_chats WHERE id = $1 AND callee_address = $2 \
                   AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = private_voice_chats.id) \
                 RETURNING id, caller_address, callee_address",
            ).bind(id).bind(callee.to_lowercase()).fetch_optional(&mut *txn).await?,
            PendingPrivateCall::Ended { id, party } => sqlx::query(
                "DELETE FROM social.private_voice_chats WHERE id = $1 AND (caller_address = $2 OR callee_address = $2) \
                   AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = private_voice_chats.id) \
                 RETURNING id, caller_address, callee_address",
            ).bind(id).bind(party.to_lowercase()).fetch_optional(&mut *txn).await?,
            PendingPrivateCall::Address(address) => sqlx::query(
                "DELETE FROM social.private_voice_chats WHERE id = ( \
                   SELECT id FROM social.private_voice_chats \
                   WHERE (caller_address = $1 OR callee_address = $1) \
                     AND NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = private_voice_chats.id) \
                   ORDER BY created_at DESC LIMIT 1 \
                 ) RETURNING id, caller_address, callee_address",
            ).bind(address.to_lowercase()).fetch_optional(&mut *txn).await?,
        };
        txn.commit().await?;
        Ok(row.as_ref().map(voice_row))
    }

    pub fn config(&self) -> VoiceDbConfig {
        self.cfg
    }

    /// Accept only an existing pending intent. Holding its row lock makes acceptance and
    /// rejection mutually exclusive and prevents a late credentials request resurrecting a call.
    pub async fn accept_pending_private_call(
        &self,
        call_id: Uuid,
        caller: &Address,
        callee: &Address,
    ) -> Result<bool, sqlx::Error> {
        let mut txn = self.pool.begin().await?;
        let pending: Option<(String, String)> = sqlx::query_as(
            "SELECT caller_address, callee_address FROM social.private_voice_chats WHERE id=$1 FOR UPDATE"
        ).bind(call_id).fetch_optional(&mut *txn).await?;
        if pending
            .as_ref()
            .is_none_or(|(a, b)| a != caller.as_str() || b != callee.as_str())
        {
            return Ok(false);
        }
        let accepted: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.private_voice_chat_participants WHERE call_id=$1)",
        )
        .bind(call_id)
        .fetch_one(&mut *txn)
        .await?;
        if accepted {
            return Ok(false);
        }
        let now = now_utc();
        sqlx::query(
            "INSERT INTO social.private_voice_chat_participants (call_id, address, status, joined_at, status_updated_at) \
             SELECT $1, address, 'not_connected', $3, $3 FROM unnest($2::text[]) AS address"
        ).bind(call_id).bind(vec![caller.as_str(), callee.as_str()]).bind(now)
            .execute(&mut *txn).await?;
        txn.commit().await?;
        Ok(true)
    }

    /// Opens (or re-opens) this media call with both participants atomically.
    /// Issuing credentials does not displace other pending intents or media;
    /// the actual participant join/webhook and expiry paths own that lifecycle.
    /// The existing return type is retained; no room is displaced here. An
    /// existing room of another party pair is refused, in either order of
    /// the two wallets, so its id never admits unrelated wallets.
    pub async fn create_private_call(
        &self,
        call_id: Uuid,
        caller: &Address,
        callee: &Address,
    ) -> Result<Vec<Uuid>, CreatePrivateCallError> {
        let now = now_utc();
        let mut txn = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *txn)
            .await?;
        // Same-id credential writers serialize before reading the immutable
        // party pair. The two-key advisory namespace is separate from the
        // existing single-key wallet locks. Never hold a row lock while waiting
        // for wallets: pending/expiry paths may already be waiting on that row.
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtext('private-voice-call'),hashtext($1::text))",
        )
        .bind(call_id)
        .execute(&mut *txn)
        .await?;
        let old: Option<(String, String)> = sqlx::query_as(
            "SELECT caller_address,callee_address FROM social.private_voice_chats WHERE id=$1",
        )
        .bind(call_id)
        .fetch_optional(&mut *txn)
        .await?;
        let mut wallets = vec![caller.as_str().to_owned(), callee.as_str().to_owned()];
        wallets.sort();
        if let Some((a, b)) = old {
            let mut parties = vec![a.to_lowercase(), b.to_lowercase()];
            parties.sort();
            if parties != wallets {
                return Err(CreatePrivateCallError::PartiesDiffer);
            }
        }
        wallets.dedup();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('private-voice:' || p)) FROM unnest($1::text[]) AS t(p)")
            .bind(&wallets).execute(&mut *txn).await?;
        sqlx::query(
            "INSERT INTO social.private_voice_chats (id, caller_address, callee_address, created_at) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id",
        )
        .bind(call_id)
        .bind(caller)
        .bind(callee)
        .bind(now)
        .execute(&mut *txn)
        .await?;
        sqlx::query(
            "INSERT INTO social.private_voice_chat_participants (call_id, address, status, joined_at, status_updated_at) \
             SELECT $1, a.address, $3, $4, $4 FROM unnest($2::text[]) AS a(address) \
             ON CONFLICT (call_id, address) DO UPDATE SET status = $3, joined_at = $4, status_updated_at = $4",
        )
        .bind(call_id)
        .bind(vec![caller.as_str(), callee.as_str()])
        .bind(VoiceChatUserStatus::NotConnected.as_str())
        .bind(now)
        .execute(&mut *txn)
        .await?;
        txn.commit().await?;
        Ok(Vec::new())
    }

    pub async fn participants_in(
        &self,
        call_id: Uuid,
    ) -> Result<Vec<VoiceChatUserRow>, sqlx::Error> {
        let rows: Vec<(String, String, NaiveDateTime, NaiveDateTime)> = sqlx::query_as(
            "SELECT address, status, joined_at, status_updated_at FROM social.private_voice_chat_participants WHERE call_id = $1",
        )
        .bind(call_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(address, status, joined_at, status_updated_at)| VoiceChatUserRow {
                    address,
                    status,
                    joined_at: ms(joined_at),
                    status_updated_at: ms(status_updated_at),
                },
            )
            .collect())
    }

    pub async fn is_private_room_active(&self, call_id: Uuid) -> Result<bool, sqlx::Error> {
        let users = self.participants_in(call_id).await?;
        Ok(is_private_room_active(&self.cfg, &users, ms(now_utc())))
    }

    async fn update_participant_status(
        &self,
        call_id: Uuid,
        address: &str,
        status: VoiceChatUserStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE social.private_voice_chat_participants SET status = $1, status_updated_at = $4 WHERE call_id = $2 AND address = $3",
        )
        .bind(status.as_str())
        .bind(call_id)
        .bind(address)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_participant_disconnected(
        &self,
        call_id: Uuid,
        address: &str,
    ) -> Result<(), sqlx::Error> {
        self.update_participant_status(call_id, address, VoiceChatUserStatus::Disconnected)
            .await
    }

    pub async fn mark_participant_interrupted(
        &self,
        call_id: Uuid,
        address: &str,
    ) -> Result<(), sqlx::Error> {
        self.update_participant_status(call_id, address, VoiceChatUserStatus::ConnectionInterrupted)
            .await
    }

    /// The call `address` is currently present in, if any.
    pub async fn active_call_of(&self, address: &Address) -> Result<Option<Uuid>, sqlx::Error> {
        let c = self.cfg.cutoffs();
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT call_id FROM social.private_voice_chat_participants WHERE address = $3 AND ( \
                status = 'connected' \
                OR (status = 'connection_interrupted' AND status_updated_at > $1) \
                OR (status = 'not_connected' AND joined_at > $2) \
             ) \
             ORDER BY CASE status WHEN 'connected' THEN 1 WHEN 'connection_interrupted' THEN 2 WHEN 'not_connected' THEN 3 ELSE 4 END, \
                      status_updated_at DESC \
             LIMIT 1",
        )
        .bind(c.interrupted)
        .bind(c.initial)
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Shared SQL owner for a direct join and the webhook's cleanup-before-
    /// commit join. Existing direct consumers do not perform media cleanup.
    pub async fn join_user_to_active_call(
        &self,
        address: &str,
        call_id: Uuid,
    ) -> Result<PrivateJoin, sqlx::Error> {
        self.prepare_private_join(address, call_id)
            .await?
            .commit()
            .await
    }

    /// Read active membership/current call after locking involved call and
    /// participant rows deterministically. Media deletion may hold these locks
    /// for at most its five-second timeout; no status transition commits first.
    pub(super) async fn prepare_private_join(
        &self,
        address: &str,
        call_id: Uuid,
    ) -> Result<PreparedPrivateJoin<'_>, sqlx::Error> {
        for _ in 0..3 {
            let mut txn = self.pool.begin().await?;
            sqlx::query("SET LOCAL lock_timeout = '5s'")
                .execute(&mut *txn)
                .await?;
            sqlx::query("SET LOCAL statement_timeout = '5s'")
                .execute(&mut *txn)
                .await?;
            let locked: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM social.private_voice_chats WHERE id = $1 OR id IN (SELECT call_id FROM social.private_voice_chat_participants WHERE address = $2) ORDER BY id FOR UPDATE")
                .bind(call_id).bind(address).fetch_all(&mut *txn).await?;
            // Lock every participant contributing to the target's active
            // predicate, rather than only the wallet whose status will change.
            let _: Vec<(Uuid, String)> = sqlx::query_as("SELECT call_id, address FROM social.private_voice_chat_participants WHERE call_id = ANY($1::uuid[]) ORDER BY call_id, address FOR UPDATE")
                .bind(&locked).fetch_all(&mut *txn).await?;
            let c = self.cfg.cutoffs();
            let row: (bool, Option<Uuid>, bool) = sqlx::query_as(
                "WITH room AS (SELECT count(*) >= 2 AND NOT bool_or(status = 'disconnected' OR (status = 'connection_interrupted' AND status_updated_at < $1) OR (status = 'not_connected' AND joined_at < $2)) AS active FROM social.private_voice_chat_participants WHERE call_id = $4), cur AS (SELECT call_id FROM social.private_voice_chat_participants WHERE address = $3 AND (status = 'connected' OR (status = 'connection_interrupted' AND status_updated_at > $1) OR (status = 'not_connected' AND joined_at > $2)) ORDER BY CASE status WHEN 'connected' THEN 1 WHEN 'connection_interrupted' THEN 2 WHEN 'not_connected' THEN 3 ELSE 4 END, status_updated_at DESC LIMIT 1) SELECT COALESCE((SELECT active FROM room), false), (SELECT call_id FROM cur), EXISTS(SELECT 1 FROM social.private_voice_chat_participants WHERE call_id = $4 AND address = $3)"
            ).bind(c.interrupted).bind(c.initial).bind(address).bind(call_id).fetch_one(&mut *txn).await?;
            // A call created while the first locking statement waited may be
            // visible only to the second snapshot. Retry with that call locked.
            if (row.0 && !locked.contains(&call_id))
                || row.1.is_some_and(|id| !locked.contains(&id))
            {
                txn.rollback().await?;
                continue;
            }
            let outcome = match row {
                (false, _, _) => PrivateJoin::Inactive,
                (true, _, false) | (true, None, _) => PrivateJoin::NotInRoom,
                (true, Some(old_call), true) => PrivateJoin::Joined { old_call },
            };
            return Ok(PreparedPrivateJoin {
                txn,
                outcome,
                address: address.to_owned(),
                call_id,
            });
        }
        Err(sqlx::Error::Protocol(
            "private call changed during join preparation".into(),
        ))
    }

    /// Ends the call `address` is (or was) part of, `call_id` or else its
    /// latest, returning the call and who was in it; the participant rows go
    /// with the call (FK cascade).
    pub async fn delete_private_call_if_member(
        &self,
        call_id: Option<Uuid>,
        address: &Address,
    ) -> Result<EndedPrivateCall, DeleteRoomError> {
        let rows = sqlx::query(
            "WITH target AS ( \
                SELECT call_id FROM social.private_voice_chat_participants \
                WHERE address = $2 AND ($1::uuid IS NULL OR call_id = $1) \
                ORDER BY joined_at DESC LIMIT 1 \
             ), members AS ( \
                SELECT p.address FROM social.private_voice_chat_participants p JOIN target t ON p.call_id = t.call_id \
             ), del AS ( \
                DELETE FROM social.private_voice_chats WHERE id = (SELECT call_id FROM target) \
                RETURNING id, caller_address, callee_address \
             ) \
             SELECT d.id, d.caller_address, d.callee_address, m.address FROM del d CROSS JOIN members m",
        )
        .bind(call_id)
        .bind(address)
        .fetch_all(&self.pool)
        .await
        .map_err(DeleteRoomError::Db)?;
        let Some(first) = rows.first() else {
            return Err(DeleteRoomError::RoomDoesNotExist);
        };
        Ok(EndedPrivateCall {
            call: voice_row(first),
            users: rows.iter().map(|r| r.get::<String, _>("address")).collect(),
        })
    }

    pub async fn delete_private_call(&self, call_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM social.private_voice_chats WHERE id = $1")
            .bind(call_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The bounded sweep: calls with a participant that never connected in
    /// time, stayed interrupted past the TTL, or left. Returns the calls whose
    /// LiveKit room must be torn down (the ones a participant never left
    /// voluntarily).
    pub async fn delete_expired_private_calls(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        let c = self.cfg.cutoffs();
        let rows: Vec<(Uuid, bool)> = sqlx::query_as(
            "WITH expired AS ( \
                SELECT call_id, bool_or(status IN ('not_connected', 'connection_interrupted')) AS should_destroy_room \
                FROM social.private_voice_chat_participants \
                WHERE (status = 'not_connected' AND joined_at <= $2) \
                   OR (status = 'connection_interrupted' AND status_updated_at <= $1) \
                   OR status = 'disconnected' \
                GROUP BY call_id LIMIT $3 \
             ), del AS ( \
                DELETE FROM social.private_voice_chats c USING expired WHERE c.id = expired.call_id RETURNING c.id \
             ) \
             SELECT e.call_id, e.should_destroy_room FROM expired e",
        )
        .bind(c.interrupted)
        .bind(c.initial)
        .bind(self.cfg.expired_batch_size)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter(|(_, destroy)| *destroy)
            .map(|(id, _)| id)
            .collect())
    }

    pub async fn join_user_to_community_room(
        &self,
        community_id: CommunityId,
        address: &Address,
        is_moderator: bool,
    ) -> Result<(), CommunityJoinError> {
        let now = now_utc();
        sqlx::query(
            "INSERT INTO social.community_voice_chat_users \
               (community_id, address, is_moderator, status, sid, joined_at, status_updated_at, created_at) \
             VALUES ($1, $2, $3, $4, NULL, $5, $5, $5) \
             ON CONFLICT (community_id, address) DO UPDATE SET \
               status = $4, status_updated_at = $5, is_moderator = $3, sid = NULL",
        )
        .bind(community_id)
        .bind(address)
        .bind(is_moderator)
        .bind(VoiceChatUserStatus::NotConnected.as_str())
        .bind(now)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| match &e {
            sqlx::Error::Database(db) if db.code().as_deref() == Some("23503") => CommunityJoinError::CommunityNotFound,
            _ => CommunityJoinError::Db(e),
        })
    }

    pub async fn update_community_user_status(
        &self,
        community_id: CommunityId,
        address: &str,
        status: VoiceChatUserStatus,
        sid: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE social.community_voice_chat_users \
             SET status = $1, status_updated_at = $5, sid = COALESCE($4, sid) \
             WHERE community_id = $2 AND address = $3",
        )
        .bind(status.as_str())
        .bind(community_id)
        .bind(address)
        .bind(sid)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_community_user(
        &self,
        community_id: CommunityId,
        address: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM social.community_voice_chat_users WHERE community_id = $1 AND address = $2")
            .bind(community_id)
            .bind(address)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn community_users_in_room(
        &self,
        community_id: CommunityId,
    ) -> Result<Vec<CommunityVoiceChatUserRow>, sqlx::Error> {
        let rows: Vec<(
            String,
            bool,
            String,
            NaiveDateTime,
            NaiveDateTime,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT address, is_moderator, status, joined_at, status_updated_at, sid \
             FROM social.community_voice_chat_users WHERE community_id = $1",
        )
        .bind(community_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(address, is_moderator, status, joined_at, status_updated_at, sid)| {
                    CommunityVoiceChatUserRow {
                        address,
                        is_moderator,
                        status,
                        joined_at: ms(joined_at),
                        status_updated_at: ms(status_updated_at),
                        sid,
                    }
                },
            )
            .collect())
    }

    pub fn is_active_community_user(&self, user: &CommunityVoiceChatUserRow, now_ms: i64) -> bool {
        is_active_community_user(&self.cfg, user, now_ms)
    }

    /// Deletes the room's rows and returns how many there were (the
    /// participant count the ended event reports).
    pub async fn delete_community_voice_chat(
        &self,
        community_id: CommunityId,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "WITH d AS (DELETE FROM social.community_voice_chat_users WHERE community_id = $1 RETURNING 1) SELECT count(*) FROM d",
        )
        .bind(community_id)
        .fetch_one(&self.pool)
        .await
    }

    /// The bounded sweep: rooms with no moderator row, or whose moderators
    /// have all been inactive past `community_no_moderator_ttl_ms`.
    pub async fn delete_expired_community_voice_chats(
        &self,
    ) -> Result<Vec<CommunityId>, sqlx::Error> {
        let c = self.cfg.cutoffs();
        let rows: Vec<(CommunityId,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "WITH room_moderator_status AS ( \
                SELECT community_id, \
                    COUNT(CASE WHEN is_moderator THEN 1 END) AS moderator_count, \
                    COUNT(CASE WHEN is_moderator AND {CONNECTED_SQL} THEN 1 END) AS active_moderator_count, \
                    MAX(CASE WHEN is_moderator THEN status_updated_at END) AS last_moderator_activity \
                FROM social.community_voice_chat_users GROUP BY community_id \
             ), expired AS ( \
                SELECT community_id FROM room_moderator_status \
                WHERE moderator_count = 0 \
                   OR (active_moderator_count = 0 AND last_moderator_activity <= $3) \
                LIMIT $4 \
             ) \
             DELETE FROM social.community_voice_chat_users u USING expired \
             WHERE u.community_id = expired.community_id RETURNING expired.community_id"
        )))
        .bind(c.interrupted)
        .bind(c.initial)
        .bind(c.no_moderator)
        .bind(self.cfg.expired_batch_size)
        .fetch_all(&self.pool)
        .await?;
        let mut out: Vec<CommunityId> = Vec::new();
        for (id,) in rows {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Rooms with a present moderator, with their present participant and
    /// moderator counts.
    pub async fn all_active_community_voice_chats(
        &self,
    ) -> Result<Vec<ActiveCommunityVoiceChat>, sqlx::Error> {
        let c = self.cfg.cutoffs();
        let rows: Vec<(CommunityId, i64, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT community_id, \
                COUNT(CASE WHEN {CONNECTED_SQL} THEN 1 END) AS participant_count, \
                COUNT(CASE WHEN is_moderator AND {CONNECTED_SQL} THEN 1 END) AS moderator_count \
             FROM social.community_voice_chat_users GROUP BY community_id \
             HAVING COUNT(CASE WHEN is_moderator AND {CONNECTED_SQL} THEN 1 END) > 0"
        )))
        .bind(c.interrupted)
        .bind(c.initial)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(community_id, participant_count, moderator_count)| ActiveCommunityVoiceChat {
                    community_id,
                    participant_count,
                    moderator_count,
                },
            )
            .collect())
    }

    pub async fn is_user_in_any_community_voice_chat(
        &self,
        address: &Address,
    ) -> Result<bool, sqlx::Error> {
        let c = self.cfg.cutoffs();
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM social.community_voice_chat_users WHERE address = $3 AND {CONNECTED_SQL})"
        )))
        .bind(c.interrupted)
        .bind(c.initial)
        .bind(address)
        .fetch_one(&self.pool)
        .await
    }

    /// Row count per room that still has a present moderator (what the
    /// ended event reports for rooms the sweep is about to delete).
    pub async fn active_community_participant_counts(
        &self,
    ) -> Result<BTreeMap<CommunityId, i64>, sqlx::Error> {
        let c = self.cfg.cutoffs();
        let rows: Vec<(CommunityId, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT community_id, COUNT(*) FROM social.community_voice_chat_users GROUP BY community_id \
             HAVING COUNT(CASE WHEN is_moderator AND {CONNECTED_SQL} THEN 1 END) > 0"
        )))
        .bind(c.interrupted)
        .bind(c.initial)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn bulk_community_status(
        &self,
        ids: &[CommunityId],
    ) -> Result<BTreeMap<CommunityId, (i64, i64)>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Default::default());
        }
        let c = self.cfg.cutoffs();
        let raw: Vec<Uuid> = ids.iter().map(CommunityId::as_uuid).collect();
        let rows: Vec<(CommunityId, i64, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT community_id, \
                COUNT(CASE WHEN {CONNECTED_SQL} THEN 1 END) AS participant_count, \
                COUNT(CASE WHEN is_moderator AND {CONNECTED_SQL} THEN 1 END) AS moderator_count \
             FROM social.community_voice_chat_users WHERE community_id = ANY($3) GROUP BY community_id"
        )))
        .bind(c.interrupted)
        .bind(c.initial)
        .bind(&raw)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id, p, m)| (id, (p, m))).collect())
    }
}

pub(crate) fn is_active_community_user(
    cfg: &VoiceDbConfig,
    user: &CommunityVoiceChatUserRow,
    now: i64,
) -> bool {
    let status = user.status.as_str();
    status == VoiceChatUserStatus::Connected.as_str()
        || (status == VoiceChatUserStatus::ConnectionInterrupted.as_str()
            && user.status_updated_at + cfg.connection_interrupted_ttl_ms > now)
        || (status == VoiceChatUserStatus::NotConnected.as_str()
            && user.joined_at + cfg.initial_connection_ttl_ms > now)
}

pub(crate) fn is_private_room_active(
    cfg: &VoiceDbConfig,
    users: &[VoiceChatUserRow],
    now: i64,
) -> bool {
    let interrupted = VoiceChatUserStatus::ConnectionInterrupted.as_str();
    let not_connected = VoiceChatUserStatus::NotConnected.as_str();
    let disconnected = VoiceChatUserStatus::Disconnected.as_str();
    let has_inactive_user = users.iter().any(|user| {
        (user.status == interrupted
            && user.status_updated_at + cfg.connection_interrupted_ttl_ms < now)
            || (user.status == not_connected
                && user.joined_at + cfg.initial_connection_ttl_ms < now)
            || user.status == disconnected
    });
    !has_inactive_user && users.len() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn waiting_on(pool: &PgPool, blocker: i32) -> i32 {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let pid: Option<i32> = sqlx::query_scalar(
                    "SELECT pid FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) LIMIT 1",
                )
                .bind(blocker)
                .fetch_optional(pool)
                .await
                .unwrap();
                if let Some(pid) = pid {
                    return pid;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the production writer must reach the database barrier")
    }

    #[tokio::test]
    async fn private_conflicts_preserve_parties_and_wait_for_wallets_before_rows() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let db = VoiceDb::new(pool.clone(), VoiceDbConfig::default());
        let wallet = || Address::parse(&format!("0x{}00000000", Uuid::new_v4().simple())).unwrap();
        let (a, b, c, d, e, f, h) = (
            wallet(),
            wallet(),
            wallet(),
            wallet(),
            wallet(),
            wallet(),
            wallet(),
        );
        let call = db
            .seed_pending_private_call(a.as_str(), b.as_str())
            .await
            .unwrap();
        let unrelated = db
            .seed_pending_private_call(c.as_str(), d.as_str())
            .await
            .unwrap();
        let before: (String,String,NaiveDateTime) = sqlx::query_as("SELECT caller_address,callee_address,created_at FROM social.private_voice_chats WHERE id=$1")
            .bind(call).fetch_one(&pool).await.unwrap();
        let unrelated_before: String = sqlx::query_scalar(
            "SELECT to_jsonb(c)::text FROM social.private_voice_chats c WHERE id=$1",
        )
        .bind(unrelated)
        .fetch_one(&pool)
        .await
        .unwrap();
        let mut barrier = pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *barrier)
            .await
            .unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('private-voice:' || $1))")
            .bind(&a)
            .execute(&mut *barrier)
            .await
            .unwrap();
        let writer = db.clone();
        let (a1, b1) = (a.clone(), b.clone());
        let first = tokio::spawn(async move { writer.create_private_call(call, &a1, &b1).await });
        let first_pid = waiting_on(&pool, pid).await;
        // Acceptance takes this call's row lock. It must finish while the
        // credential writer waits for a wallet, proving there is no row→wallet
        // inversion. Existing callers/callees are retained on the later conflict.
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                db.accept_pending_private_call(call, &a, &b)
            )
            .await
            .unwrap()
            .unwrap()
        );
        let writer = db.clone();
        let (a1, b1) = (a.clone(), b.clone());
        // The same pair in the other order is the same room.
        let second = tokio::spawn(async move { writer.create_private_call(call, &b1, &a1).await });
        waiting_on(&pool, first_pid).await;
        let pending_db = db.clone();
        let (a1, h2) = (a.to_string(), h.to_string());
        let pending = tokio::spawn(async move {
            pending_db
                .create_pending_private_call(&a1, &h2, 60_000)
                .await
        });
        barrier.commit().await.unwrap();
        for writer in [first, second] {
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(10), writer)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()
                    .is_empty()
            );
        }
        let pending = tokio::time::timeout(std::time::Duration::from_secs(10), pending)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            pending.started.is_some(),
            "accepted media and a new pending intent coexist"
        );
        let after: (String,String,NaiveDateTime) = sqlx::query_as("SELECT caller_address,callee_address,created_at FROM social.private_voice_chats WHERE id=$1")
            .bind(call).fetch_one(&pool).await.unwrap();
        assert_eq!(
            after, before,
            "conflicts preserve the original party pair and creation time"
        );
        let parties: Vec<Address> = sqlx::query_scalar("SELECT address FROM social.private_voice_chat_participants WHERE call_id=$1 ORDER BY address")
            .bind(call).fetch_all(&pool).await.unwrap();
        let mut expected = vec![a.clone(), b.clone()];
        expected.sort();
        assert_eq!(parties, expected, "credentials keep the room's party pair");
        for (x, y) in [(&e, &f), (&a, &e), (&e, &b)] {
            assert!(
                matches!(
                    db.create_private_call(call, x, y).await,
                    Err(CreatePrivateCallError::PartiesDiffer)
                ),
                "another party pair cannot take this room"
            );
        }
        let parties: Vec<Address> = sqlx::query_scalar("SELECT address FROM social.private_voice_chat_participants WHERE call_id=$1 ORDER BY address")
            .bind(call).fetch_all(&pool).await.unwrap();
        assert_eq!(parties, expected, "a refused pair adds no participant");
        let unrelated_after: String = sqlx::query_scalar(
            "SELECT to_jsonb(c)::text FROM social.private_voice_chats c WHERE id=$1",
        )
        .bind(unrelated)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unrelated_after, unrelated_before);
    }

    #[tokio::test]
    async fn private_credential_writer_does_not_lock_rows_while_waiting_behind_expiry() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let db = VoiceDb::new(pool.clone(), VoiceDbConfig::default());
        let wallet = || Address::parse(&format!("0x{}00000000", Uuid::new_v4().simple())).unwrap();
        let (a, b) = (wallet(), wallet());
        let call = db
            .seed_pending_private_call(a.as_str(), b.as_str())
            .await
            .unwrap();
        sqlx::query("UPDATE social.private_voice_chats SET created_at=created_at-interval '1 day' WHERE id=$1").bind(call).execute(&pool).await.unwrap();
        let mut barrier = pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *barrier)
            .await
            .unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('private-voice:' || $1))")
            .bind(&a)
            .execute(&mut *barrier)
            .await
            .unwrap();
        let writer = db.clone();
        let (a1, b1) = (a.clone(), b.clone());
        let media = tokio::spawn(async move { writer.create_private_call(call, &b1, &a1).await });
        waiting_on(&pool, pid).await;
        let expired = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            db.expire_pending_private_calls(60 * 60 * 1000, 100),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            expired.iter().any(|row| row.id == call),
            "expiry can acquire the pending row while media waits for wallets"
        );
        barrier.commit().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), media)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        // The pending fact expired before media creation; the direct credential
        // service still creates its requested media, without restoring an intent.
        let parties: (String, String) = sqlx::query_as(
            "SELECT caller_address,callee_address FROM social.private_voice_chats WHERE id=$1",
        )
        .bind(call)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(parties, (b.to_string(), a.to_string()));
        assert!(
            db.finish_pending_private_call(PendingPrivateCall::Id(call))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn prepared_join_serializes_rows_and_rollback_preserves_replay_source() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let db = VoiceDb::new(pool, VoiceDbConfig::default());
        let a = Address::parse(&format!("0x{}00000000", Uuid::new_v4().simple())).unwrap();
        let b = Address::parse(&format!("0x{}00000000", Uuid::new_v4().simple())).unwrap();
        let call = Uuid::new_v4();
        db.create_private_call(call, &a, &b).await.unwrap();
        let first = db.prepare_private_join(a.as_str(), call).await.unwrap();
        assert_eq!(first.outcome, PrivateJoin::Joined { old_call: call });
        let mut second = Box::pin(db.prepare_private_join(a.as_str(), call));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        assert!(
            db.participants_in(call)
                .await
                .unwrap()
                .iter()
                .all(|row| row.status == "not_connected")
        );
        drop(first);
        let second = second.await.unwrap();
        assert_eq!(second.outcome, PrivateJoin::Joined { old_call: call });
        second.commit().await.unwrap();
        assert_eq!(db.active_call_of(&a).await.unwrap(), Some(call));
        db.delete_private_call(call).await.unwrap();
    }

    #[tokio::test]
    async fn pending_calls_serialize_shared_participants_and_return_deleted_rows() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let db = VoiceDb::new(pool, VoiceDbConfig::default());
        let caller = format!("0x{}00000000", Uuid::new_v4().simple());
        let callee = format!("0x{}00000000", Uuid::new_v4().simple());
        let other = format!("0x{}00000000", Uuid::new_v4().simple());
        let uppercase_caller = caller.to_uppercase();
        let (a, b) = tokio::join!(
            db.create_pending_private_call(&uppercase_caller, &callee, 60_000),
            db.create_pending_private_call(&other, &callee, 60_000),
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_ne!(a.started.is_some(), b.started.is_some());
        let id = a.started.or(b.started).unwrap();
        let ended = db
            .finish_pending_private_call(PendingPrivateCall::Address(callee.clone()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ended.id, id);
        assert_eq!(ended.callee_address, callee);
        assert_eq!(
            ended.caller_address,
            if a.started.is_some() { caller } else { other }
        );
        assert!(
            db.finish_pending_private_call(PendingPrivateCall::Id(id))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn busy_start_preserves_stale_pending_calls() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let db = VoiceDb::new(pool.clone(), VoiceDbConfig::default());
        let caller = format!("0x{}00000000", Uuid::new_v4().simple());
        let callee = format!("0x{}00000000", Uuid::new_v4().simple());
        let old = db
            .seed_pending_private_call(&caller, &callee)
            .await
            .unwrap();
        sqlx::query("UPDATE social.private_voice_chats SET created_at = created_at - interval '1 day' WHERE id = $1")
            .bind(old).execute(&pool).await.unwrap();
        let next = format!("0x{}00000000", Uuid::new_v4().simple());
        assert!(
            db.create_pending_private_call(&caller, &next, 60_000)
                .await
                .unwrap()
                .started
                .is_none()
        );
        let restored = db
            .finish_pending_private_call(PendingPrivateCall::Id(old))
            .await
            .unwrap()
            .expect("busy start must preserve the stale pending call");
        assert_eq!(restored.caller_address, caller);
        assert_eq!(restored.callee_address, callee);
    }

    #[tokio::test]
    async fn pending_starts_use_fresh_snapshots_with_repeatable_read_role_defaults() {
        let Some(pool) = crate::db::scratch::pool(6).await else {
            return;
        };
        let name = format!("voice-isolation-{}", Uuid::new_v4().simple());
        let options = (*pool.connect_options()).clone().application_name(&name);
        let writers = sqlx::postgres::PgPoolOptions::new().max_connections(2)
            .after_connect(|connection, _| Box::pin(async move {
                sqlx::query("SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                    .execute(connection).await?;
                Ok(())
            })).connect_with(options).await.unwrap();
        let isolation: String = sqlx::query_scalar("SHOW default_transaction_isolation")
            .fetch_one(&writers)
            .await
            .unwrap();
        assert_eq!(isolation, "repeatable read");
        let wallet = |prefix: &str| format!("0x{prefix}{}000000", Uuid::new_v4().simple());
        let (a, b, c, d) = (wallet("01"), wallet("02"), wallet("03"), wallet("04"));
        let db = VoiceDb::new(pool.clone(), VoiceDbConfig::default());
        let old = Uuid::new_v4();
        db.create_private_call(
            old,
            &Address::parse(&a).unwrap(),
            &Address::parse(&d).unwrap(),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE social.private_voice_chat_participants SET joined_at=joined_at-interval '1 day' WHERE call_id=$1").bind(old).execute(&pool).await.unwrap();
        let mut blocker = pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('private-voice:' || $1))")
            .bind(&a)
            .execute(&mut *blocker)
            .await
            .unwrap();
        let first = VoiceDb::new(writers.clone(), VoiceDbConfig::default());
        let second = first.clone();
        let (upper_a, lower_a, first_b, second_c) =
            (a.to_uppercase(), a.clone(), b.clone(), c.clone());
        let first = tokio::spawn(async move {
            first
                .create_pending_private_call(&upper_a, &first_b, 60_000)
                .await
        });
        let second = tokio::spawn(async move {
            second
                .create_pending_private_call(&second_c, &lower_a, 60_000)
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let waiting:i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE application_name=$1 AND wait_event='advisory'").bind(&name).fetch_one(&pool).await.unwrap();
                if waiting == 2 { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("both role-default REPEATABLE READ writers must wait behind the same participant lock");
        blocker.commit().await.unwrap();
        let (first, second) = (
            first.await.unwrap().unwrap(),
            second.await.unwrap().unwrap(),
        );
        assert_ne!(
            first.started.is_some(),
            second.started.is_some(),
            "one pending wins across caller/callee roles and case"
        );
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM social.private_voice_chats c WHERE (caller_address=$1 OR callee_address=$1) AND NOT EXISTS(SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id=c.id)").bind(&a).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            db.participants_in(old).await.unwrap().len(),
            2,
            "the pending winner preserves historical media"
        );
        sqlx::query(
            "DELETE FROM social.private_voice_chats WHERE caller_address=$1 OR callee_address=$1",
        )
        .bind(&a)
        .execute(&pool)
        .await
        .unwrap();
        writers.close().await;
    }

    #[test]
    fn status_wire_values_and_default_ttls_match_upstream() {
        assert_eq!(VoiceChatUserStatus::Connected.as_str(), "connected");
        assert_eq!(
            VoiceChatUserStatus::ConnectionInterrupted.as_str(),
            "connection_interrupted"
        );
        assert_eq!(VoiceChatUserStatus::Disconnected.as_str(), "disconnected");
        assert_eq!(VoiceChatUserStatus::NotConnected.as_str(), "not_connected");
        let cfg = VoiceDbConfig::default();
        assert_eq!(
            (
                cfg.connection_interrupted_ttl_ms,
                cfg.initial_connection_ttl_ms,
                cfg.community_no_moderator_ttl_ms,
                cfg.expired_batch_size
            ),
            (300_000, 300_000, 300_000, 50)
        );
    }

    fn community_user(
        status: VoiceChatUserStatus,
        joined_at: i64,
        updated_at: i64,
    ) -> CommunityVoiceChatUserRow {
        CommunityVoiceChatUserRow {
            address: "0xabc".into(),
            is_moderator: true,
            status: status.as_str().into(),
            joined_at,
            status_updated_at: updated_at,
            sid: None,
        }
    }

    fn private_user(
        status: VoiceChatUserStatus,
        joined_at: i64,
        updated_at: i64,
    ) -> VoiceChatUserRow {
        VoiceChatUserRow {
            address: "0xabc".into(),
            status: status.as_str().into(),
            joined_at,
            status_updated_at: updated_at,
        }
    }

    #[test]
    fn active_community_user_rules() {
        let cfg = VoiceDbConfig::default();
        let now = 10_000_000;
        assert!(is_active_community_user(
            &cfg,
            &community_user(VoiceChatUserStatus::Connected, 0, 0),
            now
        ));
        assert!(is_active_community_user(
            &cfg,
            &community_user(
                VoiceChatUserStatus::ConnectionInterrupted,
                0,
                now - cfg.connection_interrupted_ttl_ms + 1
            ),
            now
        ));
        assert!(!is_active_community_user(
            &cfg,
            &community_user(
                VoiceChatUserStatus::ConnectionInterrupted,
                0,
                now - cfg.connection_interrupted_ttl_ms
            ),
            now
        ));
        assert!(is_active_community_user(
            &cfg,
            &community_user(
                VoiceChatUserStatus::NotConnected,
                now - cfg.initial_connection_ttl_ms + 1,
                0
            ),
            now
        ));
        assert!(!is_active_community_user(
            &cfg,
            &community_user(
                VoiceChatUserStatus::NotConnected,
                now - cfg.initial_connection_ttl_ms,
                0
            ),
            now
        ));
        assert!(!is_active_community_user(
            &cfg,
            &community_user(VoiceChatUserStatus::Disconnected, now, now),
            now
        ));
    }

    #[test]
    fn private_room_active_requires_two_active_users() {
        let cfg = VoiceDbConfig::default();
        let now = 10_000_000;
        let users = vec![
            private_user(VoiceChatUserStatus::Connected, now, now),
            private_user(VoiceChatUserStatus::Connected, now, now),
        ];
        assert!(is_private_room_active(&cfg, &users, now));
        assert!(!is_private_room_active(&cfg, &users[..1], now));
        let left = vec![
            private_user(VoiceChatUserStatus::Connected, now, now),
            private_user(VoiceChatUserStatus::Disconnected, now, now),
        ];
        assert!(!is_private_room_active(&cfg, &left, now));
        let stale = vec![
            private_user(VoiceChatUserStatus::Connected, now, now),
            private_user(
                VoiceChatUserStatus::NotConnected,
                now - cfg.initial_connection_ttl_ms - 1,
                now,
            ),
        ];
        assert!(!is_private_room_active(&cfg, &stale, now));
        let interrupted_ok = vec![
            private_user(VoiceChatUserStatus::Connected, now, now),
            private_user(
                VoiceChatUserStatus::ConnectionInterrupted,
                now,
                now - cfg.connection_interrupted_ttl_ms + 1,
            ),
        ];
        assert!(is_private_room_active(&cfg, &interrupted_ok, now));
    }
}
