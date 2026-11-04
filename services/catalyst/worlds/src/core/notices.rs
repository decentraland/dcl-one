//! The user notices worlds produces, with the titles, descriptions and links
//! of the old service's events as the notifications service stored them:
//! `worlds_permission_granted` / `worlds_permission_revoked` (one per
//! address a grant edit actually added or removed) and the storage-quota
//! trio `worlds_missing_resources`, `worlds_access_restricted`,
//! `worlds_access_restored`. Rows are written through social's
//! [`SocialDirectory`] in the caller's transaction.

use std::sync::Arc;

use chrono::NaiveDateTime;
use serde_json::json;
use uuid::Uuid;

use crate::comms::access::QUOTA_BLOCK_GRACE;
use crate::comms::interfaces::{SocialDirectory, UserNotification};
use crate::comms::world_identity::WorldName;
use crate::policy::Permission;
use crate::shared::identity::Address;

/// WCS `BUILDER_URL`'s default.
pub const DEFAULT_BUILDER_URL: &str = "https://decentraland.org/builder";

pub const PERMISSION_GRANTED: &str = "worlds_permission_granted";
pub const PERMISSION_REVOKED: &str = "worlds_permission_revoked";
pub const MISSING_RESOURCES: &str = "worlds_missing_resources";
pub const ACCESS_RESTRICTED: &str = "worlds_access_restricted";
pub const ACCESS_RESTORED: &str = "worlds_access_restored";

/// Namespace of the quota notices' ids (v5 over kind, event key and wallet).
const QUOTA_NS: Uuid = Uuid::from_bytes([
    0x5b, 0x0e, 0x7c, 0x31, 0x9a, 0x4d, 0x4f, 0x8e, 0xa1, 0x52, 0x3c, 0x77, 0x08, 0xd4, 0x6e, 0x19,
]);

pub struct Notices {
    social: Option<Arc<dyn SocialDirectory>>,
    builder_url: String,
}

impl Default for Notices {
    fn default() -> Self {
        Self::new(None, DEFAULT_BUILDER_URL)
    }
}

impl Notices {
    /// `social: None` (tests, tools) drops every notice.
    pub fn new(social: Option<Arc<dyn SocialDirectory>>, builder_url: &str) -> Self {
        Self {
            social,
            builder_url: builder_url.trim_end_matches('/').to_string(),
        }
    }

    fn worlds_url(&self) -> String {
        format!("{}/worlds?tab=dcl", self.builder_url)
    }

    /// Writes `notices` in `transaction`; a no-op without a directory.
    pub async fn send(
        &self,
        transaction: &mut sqlx::PgConnection,
        notices: &[UserNotification],
    ) -> Result<(), sqlx::Error> {
        match (&self.social, notices.is_empty()) {
            (Some(social), false) => social.notify_user_notifications(transaction, notices).await,
            _ => Ok(()),
        }
    }

    /// One notice per address whose `permission` grant on `world` was added
    /// (`granted`) or removed.
    pub fn permission_changed(
        &self,
        granted: bool,
        world: &WorldName,
        permission: Permission,
        addresses: &[String],
        now: NaiveDateTime,
    ) -> Vec<UserNotification> {
        let (kind, title) = if granted {
            (PERMISSION_GRANTED, "World Permission Granted")
        } else {
            (PERMISSION_REVOKED, "World Permission Revoked")
        };
        let (world, permission) = (world.key(), permission.as_str());
        let description = if granted {
            format!("You have been granted {permission} permission for world {world}.")
        } else {
            format!("Your {permission} permission for world {world} has been revoked.")
        };
        addresses
            .iter()
            .filter_map(|a| Address::parse(a).ok())
            .map(|recipient| UserNotification {
                id: Uuid::new_v4(),
                recipient,
                kind,
                metadata: json!({
                    "title": title,
                    "description": description,
                    "world": world,
                    "permissions": [permission],
                    "url": self.worlds_url(),
                }),
                occurred_at: now,
            })
            .collect()
    }

    /// The block of `wallet` created at `created_at`, refreshed at
    /// `updated_at`: a warning inside the grace, the restriction after it.
    pub fn quota_block(
        &self,
        wallet: &Address,
        created_at: NaiveDateTime,
        updated_at: NaiveDateTime,
    ) -> UserNotification {
        let ends = created_at + QUOTA_BLOCK_GRACE;
        let when = ends.and_utc().timestamp_millis();
        if updated_at - created_at < QUOTA_BLOCK_GRACE {
            quota_notice(
                MISSING_RESOURCES,
                wallet,
                created_at,
                created_at,
                json!({
                    "title": "Missing Resources",
                    "description": "World access at risk in 48hs. Rectify now to prevent disruption.",
                    "url": self.worlds_url(),
                    "when": when,
                }),
            )
        } else {
            quota_notice(
                ACCESS_RESTRICTED,
                wallet,
                created_at,
                ends,
                json!({
                    "title": "Worlds restricted",
                    "description": "Access to your Worlds has been restricted due to insufficient resources.",
                    "when": when,
                }),
            )
        }
    }

    /// The block of `wallet` created at `created_at` was lifted at `now`.
    pub fn quota_restored(
        &self,
        wallet: &Address,
        created_at: NaiveDateTime,
        now: NaiveDateTime,
    ) -> UserNotification {
        quota_notice(
            ACCESS_RESTORED,
            wallet,
            created_at,
            now,
            json!({
                "title": "Worlds available",
                "description": "Access to your Worlds has been restored.",
                "url": self.worlds_url(),
            }),
        )
    }
}

/// The old event key `detected-<block day>` per kind and wallet, as the
/// notifications service's (event_key, type, address) dedupe.
fn quota_notice(
    kind: &'static str,
    wallet: &Address,
    created_at: NaiveDateTime,
    occurred_at: NaiveDateTime,
    metadata: serde_json::Value,
) -> UserNotification {
    let key = format!("detected-{}", created_at.format("%Y-%m-%d"));
    let identity = format!("{kind}\n{key}\n{}", wallet.as_str());
    UserNotification {
        id: Uuid::new_v5(&QUOTA_NS, identity.as_bytes()),
        recipient: wallet.clone(),
        kind,
        metadata,
        occurred_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALLET: &str = "0x1111111111111111111111111111111111111111";

    fn at(secs: i64) -> NaiveDateTime {
        chrono::DateTime::from_timestamp(secs, 0)
            .unwrap()
            .naive_utc()
    }

    #[test]
    fn permission_notices_carry_the_old_titles_and_link() {
        let notices = Notices::new(None, "https://builder.example/");
        let world = WorldName::parse("Foo.dcl.eth").unwrap();
        let out = notices.permission_changed(
            true,
            &world,
            Permission::Deployment,
            &[WALLET.to_string()],
            at(0),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, PERMISSION_GRANTED);
        assert_eq!(
            out[0].metadata,
            json!({
                "title": "World Permission Granted",
                "description": "You have been granted deployment permission for world foo.dcl.eth.",
                "world": "foo.dcl.eth",
                "permissions": ["deployment"],
                "url": "https://builder.example/worlds?tab=dcl",
            })
        );
        let out = notices.permission_changed(
            false,
            &world,
            Permission::Streaming,
            &[WALLET.to_string()],
            at(0),
        );
        assert_eq!(out[0].kind, PERMISSION_REVOKED);
        assert_eq!(out[0].metadata["title"], "World Permission Revoked");
        assert_eq!(
            out[0].metadata["description"],
            "Your streaming permission for world foo.dcl.eth has been revoked."
        );
    }

    #[test]
    fn a_block_warns_inside_the_grace_and_restricts_after_it() {
        let notices = Notices::new(None, "https://b.example");
        let wallet = Address::parse(WALLET).unwrap();
        let created = at(1_800_000_000);
        let ends = created + QUOTA_BLOCK_GRACE;
        let warn = notices.quota_block(&wallet, created, created);
        assert_eq!(warn.kind, MISSING_RESOURCES);
        assert_eq!(warn.occurred_at, created);
        assert_eq!(
            warn.metadata,
            json!({
                "title": "Missing Resources",
                "description": "World access at risk in 48hs. Rectify now to prevent disruption.",
                "url": "https://b.example/worlds?tab=dcl",
                "when": ends.and_utc().timestamp_millis(),
            })
        );
        // The same block re-detected keeps its id; the restriction is another notice.
        let again = notices.quota_block(&wallet, created, created + chrono::TimeDelta::hours(12));
        assert_eq!(again.id, warn.id);
        let restricted = notices.quota_block(&wallet, created, ends);
        assert_eq!(restricted.kind, ACCESS_RESTRICTED);
        assert_eq!(restricted.occurred_at, ends);
        assert_ne!(restricted.id, warn.id);
        assert_eq!(restricted.metadata.get("url"), None);
        let restored = notices.quota_restored(&wallet, created, ends);
        assert_eq!(restored.kind, ACCESS_RESTORED);
        assert_eq!(
            restored.metadata,
            json!({
                "title": "Worlds available",
                "description": "Access to your Worlds has been restored.",
                "url": "https://b.example/worlds?tab=dcl",
            })
        );
    }
}
