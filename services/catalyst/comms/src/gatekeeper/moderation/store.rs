//! The moderation relations of `social` the gatekeeper reads and writes:
//! `scene_admins`, `scene_bans` (place-scoped), `user_bans`, `user_warnings`,
//! `player_connections` (platform-scoped) and the private-messages privacy
//! column of `user_settings`. Schema-qualified SQL on the ONE pool; every
//! "now" is a bind (`user_bans` expiry is time-dependent, so no index
//! predicate carries it), and a ban probe answers a [`PermissionDecision`]:
//! a database fault is "could not tell" (503), never "not banned".

use crate::shared::now_utc;
use chrono::{DateTime, Duration, NaiveDateTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::interfaces::{
    ModerationNotification, ModerationNotificationKind as Notice, SocialDirectory,
};

use crate::{Address, PermissionDecision};

use crate::gatekeeper::errors::ApiError;

pub const PLATFORM_BANNED_MSG: &str = "Access denied, platform-banned user";
pub const SCENE_BANNED_MSG: &str = "User is banned from this scene";

/// Boolean expression over `$1` (the lowercase address), `$2` (the device id
/// the caller sent, or NULL to fall back to the device recorded for the
/// address) and `$3` (now).
pub(crate) const CONNECTION_BAN_EXISTS: &str = "EXISTS (SELECT 1 FROM social.user_bans ub \
     WHERE (ub.banned_address = $1 \
            OR ub.banned_device_id = COALESCE($2::text, (SELECT NULLIF(p.device_id, '') \
                                                         FROM social.player_connections p \
                                                         WHERE p.address = $1))) \
       AND ub.lifted_at IS NULL \
       AND (ub.expires_at IS NULL OR ub.expires_at > $3))";

pub(crate) fn sent_device_id(device_id: Option<&str>) -> Option<String> {
    device_id
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(String::from)
}

pub mod ms_iso {
    use super::{DateTime, SecondsFormat, Utc};
    use serde::Serializer;

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&dt.to_rfc3339_opts(SecondsFormat::Millis, true))
    }

    pub mod option {
        use super::super::{DateTime, SecondsFormat, Utc};
        use serde::Serializer;

        pub fn serialize<S: Serializer>(
            dt: &Option<DateTime<Utc>>,
            s: S,
        ) -> Result<S::Ok, S::Error> {
            match dt {
                Some(dt) => s.serialize_str(&dt.to_rfc3339_opts(SecondsFormat::Millis, true)),
                None => s.serialize_none(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneAdminRow {
    /// The old table minted a uuid per add; the key is `(place_id, admin)`
    /// now, so the wire's `id` is derived from it (uuid v5) and stable.
    pub id: Uuid,
    pub place_id: String,
    pub admin: String,
    pub added_by: String,
    pub created_at: i64,
    pub active: bool,
}

const SCENE_ADMIN_NS: Uuid = Uuid::from_bytes([
    0x5c, 0xe9, 0xe1, 0xd0, 0x7a, 0x4d, 0x4c, 0x5a, 0x9a, 0x2f, 0x1b, 0x6e, 0x3d, 0x2c, 0x8f, 0x01,
]);

fn scene_admin_id(place_id: &str, admin: &str) -> Uuid {
    Uuid::new_v5(&SCENE_ADMIN_NS, format!("{place_id}\n{admin}").as_bytes())
}

fn scene_source(place_id: &str, version: NaiveDateTime) -> String {
    serde_json::to_string(&(place_id, version.and_utc().timestamp_micros()))
        .expect("scene source identity contains a string and integer")
}

#[derive(Clone)]
pub struct Moderation {
    pool: PgPool,
    social: Option<Arc<dyn SocialDirectory>>,
}

impl Moderation {
    /// Read-only moderation probes, including the worlds permission core.
    /// Ban/warning transitions require `with_social` and fail closed without it.
    pub fn new(pool: PgPool) -> Self {
        Self { pool, social: None }
    }

    pub fn with_social(pool: PgPool, social: Arc<dyn SocialDirectory>) -> Self {
        Self {
            pool,
            social: Some(social),
        }
    }

    fn social(&self) -> Result<&dyn SocialDirectory, sqlx::Error> {
        self.social.as_deref().ok_or_else(|| {
            sqlx::Error::Protocol("moderation notification persistence is not configured".into())
        })
    }

    pub async fn list_active_admins(
        &self,
        place_id: &str,
        admin: Option<&Address>,
    ) -> Result<Vec<SceneAdminRow>, ApiError> {
        let rows: Vec<(String, String, String, NaiveDateTime, bool)> = sqlx::query_as(
            "SELECT place_id, admin_address, added_by, added_at, active \
             FROM social.scene_admins \
             WHERE active AND place_id = $1 AND ($2::text IS NULL OR admin_address = $2) \
             ORDER BY added_at, admin_address",
        )
        .bind(place_id)
        .bind(admin.map(Address::as_str))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(place_id, admin, added_by, added_at, active)| SceneAdminRow {
                    id: scene_admin_id(&place_id, &admin),
                    place_id,
                    admin,
                    added_by,
                    created_at: added_at.and_utc().timestamp_millis(),
                    active,
                },
            )
            .collect())
    }

    /// Re-adding a removed admin flips the same row back to active.
    pub async fn add_admin(
        &self,
        place_id: &str,
        admin: &Address,
        added_by: &Address,
    ) -> Result<Uuid, ApiError> {
        sqlx::query(
            "INSERT INTO social.scene_admins (place_id, admin_address, added_by, added_at, active, removed_at) \
             VALUES ($1, $2, $3, $4, TRUE, NULL) \
             ON CONFLICT (place_id, admin_address) DO UPDATE \
               SET added_by = EXCLUDED.added_by, added_at = EXCLUDED.added_at, active = TRUE, removed_at = NULL",
        )
        .bind(place_id)
        .bind(admin)
        .bind(added_by)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(scene_admin_id(place_id, admin.as_str()))
    }

    pub async fn remove_admin(&self, place_id: &str, admin: &Address) -> Result<u64, ApiError> {
        let res = sqlx::query(
            "UPDATE social.scene_admins SET active = FALSE, removed_at = $3 \
             WHERE place_id = $1 AND admin_address = $2 AND active",
        )
        .bind(place_id)
        .bind(admin)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    /// A database fault is an undetermined authority, not "not an admin".
    pub async fn admin_decision(&self, place_id: &str, admin: &Address) -> PermissionDecision {
        let found: Result<bool, sqlx::Error> =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM social.scene_admins WHERE place_id = $1 AND admin_address = $2 AND active)")
                .bind(place_id)
                .bind(admin)
                .fetch_one(&self.pool)
                .await;
        match found {
            Ok(true) => PermissionDecision::Allowed,
            Ok(false) => {
                PermissionDecision::refused("signer is not an owner or admin of this scene")
            }
            Err(e) => PermissionDecision::undetermined("social.scene_admins", &e),
        }
    }

    pub async fn count_scene_bans(&self, place_id: &str) -> Result<i64, ApiError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM social.scene_bans WHERE place_id = $1")
                .bind(place_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Page plus total in one statement (`scene_bans_place_time_idx`); an
    /// empty page past the start falls back to a COUNT so the total is exact.
    pub async fn list_scene_ban_addresses(
        &self,
        place_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<String>, i64), ApiError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT banned_address, count(*) OVER () FROM social.scene_bans WHERE place_id = $1 \
             ORDER BY banned_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(place_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total = match rows.first() {
            Some((_, total)) => *total,
            None if offset == 0 => 0,
            None => self.count_scene_bans(place_id).await?,
        };
        Ok((rows.into_iter().map(|(addr, _)| addr).collect(), total))
    }

    pub async fn scene_ban(
        &self,
        place_id: &str,
        banned: &Address,
        banned_by: &Address,
        rooms: &[String],
        place_title: &str,
    ) -> Result<(), ApiError> {
        let mut txn = self.pool.begin().await?;
        let version:NaiveDateTime=sqlx::query_scalar("INSERT INTO social.scene_bans (place_id,banned_address,banned_by,banned_at) VALUES ($1,$2,$3,$4) ON CONFLICT (place_id,banned_address) DO UPDATE SET banned_by=EXCLUDED.banned_by,banned_at=EXCLUDED.banned_at RETURNING banned_at")
            .bind(place_id).bind(banned).bind(banned_by).bind(now_utc()).fetch_one(&mut *txn).await?;
        super::super::removal::scene(&mut txn, place_id, banned.as_str(), version, rooms).await?;
        self.social()?
            .notify_moderation_notification(
                &mut txn,
                &ModerationNotification {
                    source_id: scene_source(place_id, version),
                    recipient: banned.clone(),
                    occurred_at: version,
                    event: Notice::UserBannedFromScene {
                        place_title: place_title.into(),
                    },
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn scene_unban(
        &self,
        place_id: &str,
        banned: &Address,
        place_title: &str,
    ) -> Result<u64, ApiError> {
        let mut txn = self.pool.begin().await?;
        let version:Option<NaiveDateTime>=sqlx::query_scalar("DELETE FROM social.scene_bans WHERE place_id=$1 AND banned_address=$2 RETURNING banned_at")
            .bind(place_id).bind(banned).fetch_optional(&mut *txn).await?;
        if let Some(version) = version {
            super::super::removal::cancel_scene(&mut txn, place_id, banned.as_str(), version)
                .await?;
            self.social()?
                .notify_moderation_notification(
                    &mut txn,
                    &ModerationNotification {
                        source_id: scene_source(place_id, version),
                        recipient: banned.clone(),
                        occurred_at: now_utc(),
                        event: Notice::UserUnbannedFromScene {
                            place_title: place_title.into(),
                        },
                    },
                )
                .await?;
        }
        txn.commit().await?;
        Ok(u64::from(version.is_some()))
    }

    /// Exact retry authority owned beside its ban rows; unknown stays an error.
    pub(crate) async fn platform_removal_applies(
        &self,
        id: Uuid,
        actor: &Address,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM social.user_bans ub WHERE ub.id=$1 AND (ub.banned_address=$2 OR ub.banned_device_id=(SELECT NULLIF(p.device_id,'') FROM social.player_connections p WHERE p.address=$2)) AND ub.lifted_at IS NULL AND (ub.expires_at IS NULL OR ub.expires_at>$3))")
            .bind(id).bind(actor).bind(now_utc()).fetch_one(&self.pool).await
    }

    pub(crate) async fn scene_removal_applies(
        &self,
        place: &str,
        actor: &Address,
        version: NaiveDateTime,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM social.scene_bans WHERE place_id=$1 AND banned_address=$2 AND banned_at=$3)")
            .bind(place).bind(actor).bind(version).fetch_one(&self.pool).await
    }

    /// `Ok(true)` banned, `Ok(false)` not, `Err` could not tell: collapsing
    /// the fault to `false` silently un-banned during any transient hiccup.
    pub async fn is_scene_banned(
        &self,
        place_id: &str,
        address: &Address,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM social.scene_bans WHERE place_id = $1 AND banned_address = $2)")
            .bind(place_id)
            .bind(address)
            .fetch_one(&self.pool)
            .await
    }

    /// Comms-owned platform and scene bans, in that order; operator blocks live in worlds.
    pub async fn ban(
        &self,
        actor: &Address,
        scene: Option<&crate::access::SceneBanKey>,
        device: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Option<crate::access::Ban>, crate::AuthorityNotEstablished> {
        self.ban_at(
            actor,
            scene.map(crate::access::SceneBanKey::as_str),
            device,
            now,
        )
        .await
        .map_err(|e| crate::AuthorityNotEstablished::store_unavailable("social.user_bans", &e))
    }

    async fn ban_at(
        &self,
        actor: &Address,
        scene: Option<&str>,
        device: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Option<crate::access::Ban>, sqlx::Error> {
        let (platform, scene): (bool, bool) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {CONNECTION_BAN_EXISTS}, ($4::text IS NOT NULL AND EXISTS (SELECT 1 FROM social.scene_bans WHERE place_id = $4 AND banned_address = $1))"
        )))
        .bind(actor).bind(sent_device_id(device)).bind(now).bind(scene)
        .fetch_one(&self.pool).await?;
        Ok(if platform {
            Some(crate::access::Ban::Platform)
        } else if scene {
            Some(crate::access::Ban::Scene)
        } else {
            None
        })
    }

    /// Upstream's `getActiveBanForConnection`: the device the caller sent,
    /// else the one recorded for the address, matched alongside the address.
    pub async fn is_banned_for_connection(
        &self,
        address: &Address,
        device_id: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        self.ban_at(address, None, device_id, now_utc())
            .await
            .map(|ban| ban.is_some())
    }

    /// Platform ban and scene ban for one connection in a single statement,
    /// as one decision: platform ban first, then the scene ban.
    pub async fn connection_gate(
        &self,
        address: &Address,
        device_id: Option<&str>,
        place_id: &str,
    ) -> PermissionDecision {
        match self
            .ban_at(address, Some(place_id), device_id, now_utc())
            .await
        {
            Ok(Some(crate::access::Ban::Platform)) => {
                PermissionDecision::refused(PLATFORM_BANNED_MSG)
            }
            Ok(Some(_)) => PermissionDecision::refused(SCENE_BANNED_MSG),
            Ok(None) => PermissionDecision::Allowed,
            Err(e) => PermissionDecision::undetermined("social.user_bans", &e),
        }
    }

    /// The platform-ban decision for a set of wallets (a private call refuses
    /// when either side is banned): the first banned one, in the given order.
    pub async fn platform_ban_decision(&self, addresses: &[Address]) -> PermissionDecision {
        if addresses.is_empty() {
            return PermissionDecision::Allowed;
        }
        let lowered: Vec<&str> = addresses.iter().map(Address::as_str).collect();
        let idx: Result<Option<i64>, sqlx::Error> = sqlx::query_scalar(
            "SELECT a.ord - 1 FROM unnest($1::text[]) WITH ORDINALITY AS a(address, ord) \
             WHERE EXISTS (SELECT 1 FROM social.user_bans ub \
                 WHERE (ub.banned_address = a.address \
                        OR ub.banned_device_id = (SELECT NULLIF(p.device_id, '') \
                                                  FROM social.player_connections p \
                                                  WHERE p.address = a.address)) \
                   AND ub.lifted_at IS NULL \
                   AND (ub.expires_at IS NULL OR ub.expires_at > $2)) \
             ORDER BY a.ord LIMIT 1",
        )
        .bind(&lowered)
        .bind(now_utc())
        .fetch_optional(&self.pool)
        .await;
        match idx {
            Ok(None) => PermissionDecision::Allowed,
            Ok(Some(i)) => {
                tracing::warn!(address = %addresses[i as usize], "rejected voice credentials for a platform-banned user");
                PermissionDecision::refused(PLATFORM_BANNED_MSG)
            }
            Err(e) => PermissionDecision::undetermined("social.user_bans", &e),
        }
    }

    pub async fn ban_status(&self, address: &Address) -> Result<BanStatus, ApiError> {
        let row = sqlx::query_as::<_, BanRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {BAN_SELECT_FIELDS} FROM social.user_bans \
             WHERE banned_address = $1 AND lifted_at IS NULL AND (expires_at IS NULL OR expires_at > $2) \
             ORDER BY banned_at DESC LIMIT 1"
        )))
        .bind(address)
        .bind(now_utc())
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            Some(row) => BanStatus {
                is_banned: true,
                ban: Some(ban_from_row(row).into()),
            },
            None => BanStatus {
                is_banned: false,
                ban: None,
            },
        })
    }

    pub async fn create_ban(&self, input: CreateBan) -> Result<UserBan, BanWriteError> {
        self.insert_ban(input, false).await
    }

    /// `create_ban` that snapshots the player's recorded device id when the
    /// caller sends none.
    pub async fn create_ban_with_recorded_device(
        &self,
        input: CreateBan,
    ) -> Result<UserBan, BanWriteError> {
        self.insert_ban(input, true).await
    }

    /// One in-force ban per address: the probe and the insert run under
    /// `pg_advisory_xact_lock(hashtext(address))` because the expiry half of
    /// "in force" is time-dependent and no unique index can express it.
    async fn insert_ban(
        &self,
        input: CreateBan,
        snapshot_device: bool,
    ) -> Result<UserBan, BanWriteError> {
        let now = now_utc();
        let expires_at = input.duration_ms.map(|d| now + Duration::milliseconds(d));
        let banned_device_id = input.banned_device_id.filter(|s| !s.is_empty());
        let mut txn = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(&input.banned_address)
            .execute(&mut *txn)
            .await?;
        let row = sqlx::query_as::<_, BanRow>(sqlx::AssertSqlSafe(format!(
            "WITH existing AS ( \
                SELECT count(*) AS n FROM social.user_bans \
                WHERE banned_address = $1 AND lifted_at IS NULL AND (expires_at IS NULL OR expires_at > $8) \
             ), ins AS ( \
                INSERT INTO social.user_bans \
                  (banned_address, banned_by, reason, custom_message, banned_device_id, expires_at, banned_at, created_at) \
                SELECT $1, $2, $3, $4, \
                       CASE WHEN $7::bool \
                            THEN COALESCE($5::text, (SELECT NULLIF(p.device_id, '') \
                                                     FROM social.player_connections p WHERE p.address = $1)) \
                            ELSE $5::text END, \
                       $6, $8, $8 \
                WHERE (SELECT n FROM existing) = 0 \
                RETURNING {BAN_SELECT_FIELDS} \
             ) SELECT {BAN_SELECT_FIELDS} FROM ins"
        )))
        .bind(&input.banned_address)
        .bind(&input.banned_by)
        .bind(&input.reason)
        .bind(&input.custom_message)
        .bind(&banned_device_id)
        .bind(expires_at)
        .bind(snapshot_device)
        .bind(now)
        .fetch_optional(&mut *txn)
        .await?;
        let Some(row) = row else {
            return Err(BanWriteError::AlreadyBanned(
                input.banned_address.into_string(),
            ));
        };
        super::super::removal::platform(&mut txn, row.0, &row.1).await?;
        self.social()?
            .notify_moderation_notification(
                &mut txn,
                &ModerationNotification {
                    source_id: row.0.to_string(),
                    recipient: input.banned_address,
                    occurred_at: row.6,
                    event: Notice::Banned {
                        reason: row.3.clone(),
                        banned_at: row.6.and_utc().timestamp_millis(),
                        expires_at: row.7.map(|time| time.and_utc().timestamp_millis()),
                        custom_message: row.4.clone(),
                    },
                },
            )
            .await?;
        txn.commit().await?;
        Ok(ban_from_row(row))
    }

    pub async fn lift_ban(&self, address: &Address, lifted_by: &str) -> Result<UserBan, LiftError> {
        let now = now_utc();
        let mut txn = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(address)
            .execute(&mut *txn)
            .await?;
        let row = sqlx::query_as::<_, BanRow>(sqlx::AssertSqlSafe(format!(
            "UPDATE social.user_bans SET lifted_at = $3, lifted_by = $2 \
             WHERE banned_address = $1 AND lifted_at IS NULL AND (expires_at IS NULL OR expires_at > $3) \
             RETURNING {BAN_SELECT_FIELDS}"
        )))
        .bind(address)
        .bind(lifted_by.to_lowercase())
        .bind(now)
        .fetch_optional(&mut *txn)
        .await?;
        let row = row.ok_or_else(|| LiftError::NotFound(address.as_str().to_string()))?;
        super::super::removal::cancel_platform(&mut txn, row.0).await?;
        self.social()?
            .notify_moderation_notification(
                &mut txn,
                &ModerationNotification {
                    source_id: row.0.to_string(),
                    recipient: address.clone(),
                    occurred_at: now,
                    event: Notice::BanLifted {
                        lifted_at: now.and_utc().timestamp_millis(),
                    },
                },
            )
            .await?;
        txn.commit().await?;
        Ok(ban_from_row(row))
    }

    /// The admin listing: every ban in force, newest first
    /// (`user_bans_open_by_address_idx` is per address; this scans the open set).
    pub async fn active_bans(&self) -> Result<Vec<UserBan>, ApiError> {
        let rows = sqlx::query_as::<_, BanRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {BAN_SELECT_FIELDS} FROM social.user_bans \
             WHERE lifted_at IS NULL AND (expires_at IS NULL OR expires_at > $1) ORDER BY banned_at DESC"
        )))
        .bind(now_utc())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(ban_from_row).collect())
    }

    pub async fn create_warning(&self, input: CreateWarning) -> Result<UserWarning, ApiError> {
        let mut txn = self.pool.begin().await?;
        let row = sqlx::query_as::<_, WarningRow>(
            "INSERT INTO social.user_warnings (warned_address, warned_by, reason, warned_at) VALUES ($1, $2, $3, $4) \
             RETURNING id, warned_address, warned_by, reason, warned_at",
        )
        .bind(&input.warned_address)
        .bind(input.warned_by.to_lowercase())
        .bind(&input.reason)
        .bind(now_utc())
        .fetch_one(&mut *txn)
        .await?;
        self.social()?
            .notify_moderation_notification(
                &mut txn,
                &ModerationNotification {
                    source_id: row.0.to_string(),
                    recipient: input.warned_address,
                    occurred_at: row.4,
                    event: Notice::BanWarning {
                        reason: row.3.clone(),
                        warned_at: row.4.and_utc().timestamp_millis(),
                    },
                },
            )
            .await?;
        txn.commit().await?;
        Ok(warning_from_row(row))
    }

    pub async fn warnings(&self, address: &Address) -> Result<Vec<UserWarning>, ApiError> {
        let rows = sqlx::query_as::<_, WarningRow>(
            "SELECT id, warned_address, warned_by, reason, warned_at FROM social.user_warnings \
             WHERE warned_address = $1 ORDER BY warned_at DESC",
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(warning_from_row).collect())
    }

    /// Best-effort on every token mint: the latest ip/device per wallet, a
    /// missing value never overwriting a recorded one.
    pub async fn upsert_connection(
        &self,
        input: UpsertPlayerConnection,
    ) -> Result<(), sqlx::Error> {
        let ip_address = input.ip_address.filter(|s| !s.is_empty());
        let device_id = input.device_id.filter(|s| !s.is_empty());
        sqlx::query(
            "INSERT INTO social.player_connections (address, ip_address, device_id, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $4) \
             ON CONFLICT (address) DO UPDATE SET \
               ip_address = COALESCE(EXCLUDED.ip_address, social.player_connections.ip_address), \
               device_id = COALESCE(EXCLUDED.device_id, social.player_connections.device_id), \
               updated_at = EXCLUDED.updated_at",
        )
        .bind(&input.address)
        .bind(&ip_address)
        .bind(&device_id)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "comms/", rename_all = "camelCase")
)]
pub struct UserBan {
    pub id: String,
    #[serde(rename = "bannedAddress")]
    pub banned_address: String,
    #[serde(rename = "bannedBy")]
    pub banned_by: String,
    pub reason: String,
    #[serde(rename = "customMessage")]
    pub custom_message: Option<String>,
    #[serde(rename = "bannedDeviceId")]
    pub banned_device_id: Option<String>,
    #[serde(rename = "bannedAt", with = "ms_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub banned_at: DateTime<Utc>,
    #[serde(rename = "expiresAt", with = "ms_iso::option")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(rename = "liftedAt", with = "ms_iso::option")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub lifted_at: Option<DateTime<Utc>>,
    #[serde(rename = "liftedBy")]
    pub lifted_by: Option<String>,
    #[serde(rename = "createdAt", with = "ms_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: DateTime<Utc>,
}

/// `UserBan` minus `banned_device_id`: the device id is a stable cross-wallet
/// machine identifier, publicly linkable, so only the moderator-gated
/// `GET /bans` shows it. The unauthenticated status route uses this type.
#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "comms/", rename_all = "camelCase")
)]
pub struct PublicUserBan {
    pub id: String,
    #[serde(rename = "bannedAddress")]
    pub banned_address: String,
    #[serde(rename = "bannedBy")]
    pub banned_by: String,
    pub reason: String,
    #[serde(rename = "customMessage")]
    pub custom_message: Option<String>,
    #[serde(rename = "bannedAt", with = "ms_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub banned_at: DateTime<Utc>,
    #[serde(rename = "expiresAt", with = "ms_iso::option")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(rename = "liftedAt", with = "ms_iso::option")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub lifted_at: Option<DateTime<Utc>>,
    #[serde(rename = "liftedBy")]
    pub lifted_by: Option<String>,
    #[serde(rename = "createdAt", with = "ms_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: DateTime<Utc>,
}

impl From<UserBan> for PublicUserBan {
    fn from(ban: UserBan) -> Self {
        Self {
            id: ban.id,
            banned_address: ban.banned_address,
            banned_by: ban.banned_by,
            reason: ban.reason,
            custom_message: ban.custom_message,
            banned_at: ban.banned_at,
            expires_at: ban.expires_at,
            lifted_at: ban.lifted_at,
            lifted_by: ban.lifted_by,
            created_at: ban.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "comms/", rename_all = "camelCase")
)]
pub struct UserWarning {
    pub id: String,
    #[serde(rename = "warnedAddress")]
    pub warned_address: String,
    #[serde(rename = "warnedBy")]
    pub warned_by: String,
    pub reason: String,
    #[serde(rename = "warnedAt", with = "ms_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub warned_at: DateTime<Utc>,
    /// The old table had a `created_at` beside `warned_at`, always equal; the
    /// wire keeps the field.
    #[serde(rename = "createdAt", with = "ms_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "comms/", rename_all = "camelCase")
)]
pub struct BanStatus {
    #[serde(rename = "isBanned")]
    pub is_banned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ban: Option<PublicUserBan>,
}

type BanRow = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    NaiveDateTime,
    Option<NaiveDateTime>,
    Option<NaiveDateTime>,
    Option<String>,
    NaiveDateTime,
);

type WarningRow = (Uuid, String, String, String, NaiveDateTime);

const BAN_SELECT_FIELDS: &str = "id, banned_address, banned_by, reason, custom_message, banned_device_id, banned_at, expires_at, lifted_at, lifted_by, created_at";

fn ban_from_row(row: BanRow) -> UserBan {
    let (
        id,
        banned_address,
        banned_by,
        reason,
        custom_message,
        banned_device_id,
        banned_at,
        expires_at,
        lifted_at,
        lifted_by,
        created_at,
    ) = row;
    UserBan {
        id: id.to_string(),
        banned_address,
        banned_by,
        reason,
        custom_message,
        banned_device_id,
        banned_at: banned_at.and_utc(),
        expires_at: expires_at.map(|t| t.and_utc()),
        lifted_at: lifted_at.map(|t| t.and_utc()),
        lifted_by,
        created_at: created_at.and_utc(),
    }
}

fn warning_from_row(row: WarningRow) -> UserWarning {
    let (id, warned_address, warned_by, reason, warned_at) = row;
    UserWarning {
        id: id.to_string(),
        warned_address,
        warned_by,
        reason,
        warned_at: warned_at.and_utc(),
        created_at: warned_at.and_utc(),
    }
}

pub struct CreateBan {
    pub banned_address: Address,
    pub banned_by: String,
    pub reason: String,
    pub custom_message: Option<String>,
    pub banned_device_id: Option<String>,
    pub duration_ms: Option<i64>,
}

pub struct CreateWarning {
    pub warned_address: Address,
    pub warned_by: String,
    pub reason: String,
}

#[derive(Debug)]
pub enum BanWriteError {
    AlreadyBanned(String),
    Db(ApiError),
}

#[derive(Debug)]
pub enum LiftError {
    NotFound(String),
    Db(ApiError),
}

impl From<sqlx::Error> for BanWriteError {
    fn from(e: sqlx::Error) -> Self {
        BanWriteError::Db(ApiError::from(e))
    }
}

impl From<sqlx::Error> for LiftError {
    fn from(e: sqlx::Error) -> Self {
        LiftError::Db(ApiError::from(e))
    }
}

pub struct UpsertPlayerConnection {
    pub address: Address,
    pub ip_address: Option<String>,
    pub device_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ban_at(
        banned: DateTime<Utc>,
        expires: Option<DateTime<Utc>>,
        device: Option<&str>,
    ) -> UserBan {
        UserBan {
            id: "00000000-0000-0000-0000-000000000001".into(),
            banned_address: "0xabc".into(),
            banned_by: "0xdef".into(),
            reason: "test".into(),
            custom_message: None,
            banned_device_id: device.map(String::from),
            banned_at: banned,
            expires_at: expires,
            lifted_at: None,
            lifted_by: None,
            created_at: banned,
        }
    }

    #[test]
    fn ban_timestamps_serialize_fixed_3_digit_millis_iso() {
        let banned = Utc.timestamp_opt(1_718_900_000, 0).unwrap();
        let expires = Utc.timestamp_opt(1_718_900_500, 7_000_000).unwrap();
        let v = serde_json::to_value(ban_at(banned, Some(expires), None)).unwrap();
        assert_eq!(v["bannedAt"], "2024-06-20T16:13:20.000Z");
        assert_eq!(v["createdAt"], "2024-06-20T16:13:20.000Z");
        assert_eq!(v["expiresAt"], "2024-06-20T16:21:40.007Z");
        let v = serde_json::to_value(ban_at(banned, None, None)).unwrap();
        assert!(v["expiresAt"].is_null() && v["liftedAt"].is_null());
        let v = serde_json::to_value(ban_at(
            Utc.timestamp_opt(1_718_900_000, 123_456_789).unwrap(),
            None,
            None,
        ))
        .unwrap();
        assert_eq!(v["bannedAt"], "2024-06-20T16:13:20.123Z");
    }

    #[test]
    fn the_public_status_shape_never_carries_the_device_id_and_the_moderator_list_keeps_it() {
        let at = Utc.timestamp_opt(1_718_900_000, 0).unwrap();
        let status = BanStatus {
            is_banned: true,
            ban: Some(ban_at(at, None, Some("dev-1")).into()),
        };
        let v = serde_json::to_value(status).unwrap();
        assert_eq!(v["isBanned"], true);
        let ban = &v["ban"];
        assert!(ban.get("bannedDeviceId").is_none());
        assert_eq!(ban["bannedAddress"], "0xabc");
        assert_eq!(ban.as_object().unwrap().len(), 10);
        let list = serde_json::to_value(vec![ban_at(at, None, Some("dev-1"))]).unwrap();
        assert_eq!(list[0]["bannedDeviceId"], "dev-1");
        assert_eq!(list[0].as_object().unwrap().len(), 11);
        let none = serde_json::to_value(BanStatus {
            is_banned: false,
            ban: None,
        })
        .unwrap();
        assert_eq!(none["isBanned"], false);
        assert!(none.get("ban").is_none());
    }

    #[test]
    fn warning_timestamps_serialize_fixed_3_digit_millis_iso() {
        let at = Utc.timestamp_opt(1_718_900_000, 0).unwrap();
        let w = UserWarning {
            id: "2".into(),
            warned_address: "0xabc".into(),
            warned_by: "0xdef".into(),
            reason: "test".into(),
            warned_at: at,
            created_at: at,
        };
        let v = serde_json::to_value(w).unwrap();
        assert_eq!(v["warnedAt"], "2024-06-20T16:13:20.000Z");
        assert_eq!(v["createdAt"], "2024-06-20T16:13:20.000Z");
    }

    #[test]
    fn the_derived_scene_admin_id_is_stable_per_key() {
        assert_eq!(scene_admin_id("p", "0xa"), scene_admin_id("p", "0xa"));
        assert_ne!(scene_admin_id("p", "0xa"), scene_admin_id("p", "0xb"));
        assert_ne!(scene_admin_id("p", "0xa"), scene_admin_id("q", "0xa"));
        assert_eq!(sent_device_id(Some("  ")), None);
        assert_eq!(sent_device_id(Some(" d ")).as_deref(), Some("d"));
    }
}

#[async_trait::async_trait]
impl super::BanStatus for Moderation {
    async fn ban(
        &self,
        actor: &Address,
        scene: Option<&crate::access::SceneBanKey>,
        device: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Option<crate::access::Ban>, crate::AuthorityNotEstablished> {
        Moderation::ban(self, actor, scene, device, now).await
    }
}
