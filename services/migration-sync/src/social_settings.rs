//! A scoped projection of committed social settings, never provider enforcement.
//!
//! Missing source rows remain missing: source and target readers use an absent-row
//! privacy default different from the inserted-row database default.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, Row};

pub const SOURCE: &str = "social-settings";
pub const ADAPTER: &str = "social_settings_outbox_v1";
pub const SOURCE_SQL: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/migration-sync/source/social-settings.sql"
));
pub const SOURCE_SCHEMA: &str = "migration_capture_social_settings";
pub const LOCK_KEY: (i32, i32) = (179321, 3);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateMessagesPrivacy {
    OnlyFriends,
    All,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedMessagesVisibility {
    ShowMessages,
    DoNotShowMessages,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SituationReactions {
    Show,
    Hide,
}

impl PrivateMessagesPrivacy {
    fn as_str(self) -> &'static str {
        match self {
            Self::OnlyFriends => "only_friends",
            Self::All => "all",
        }
    }
}
impl BlockedMessagesVisibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::ShowMessages => "show_messages",
            Self::DoNotShowMessages => "do_not_show_messages",
        }
    }
}
impl SituationReactions {
    fn as_str(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Hide => "hide",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    SocialSettingsUpsert {
        address: String,
        private_messages_privacy: PrivateMessagesPrivacy,
        blocked_users_messages_visibility: BlockedMessagesVisibility,
        show_situation_reactions: SituationReactions,
    },
    SocialSettingsDelete {
        address: String,
    },
}
impl Change {
    pub fn key(&self) -> &str {
        match self {
            Self::SocialSettingsUpsert { address, .. } | Self::SocialSettingsDelete { address } => {
                address
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        validate_address(self.key())
    }
    pub fn deleted(&self) -> bool {
        matches!(self, Self::SocialSettingsDelete { .. })
    }
}

pub fn validate_address(address: &str) -> Result<()> {
    ensure!(
        address.len() == 42
            && address.starts_with("0x")
            && address[2..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "social settings address must be canonical lowercase hex"
    );
    Ok(())
}

/// Caller owns the source-order transaction and durable checkpoint. No callback,
/// job, notification or Gatekeeper request runs when applying these facts.
pub async fn apply(connection: &mut PgConnection, change: &Change) -> Result<()> {
    change.validate()?;
    match change {
        Change::SocialSettingsUpsert {
            address,
            private_messages_privacy,
            blocked_users_messages_visibility,
            show_situation_reactions,
        } => {
            sqlx::query("INSERT INTO social.user_settings AS current(address,private_messages_privacy,blocked_users_messages_visibility,show_situation_reactions) VALUES($1,$2,$3,$4)
                ON CONFLICT(address) DO UPDATE SET private_messages_privacy=EXCLUDED.private_messages_privacy,
                blocked_users_messages_visibility=EXCLUDED.blocked_users_messages_visibility,
                show_situation_reactions=EXCLUDED.show_situation_reactions,
                updated_at=CASE WHEN (current.private_messages_privacy,current.blocked_users_messages_visibility,current.show_situation_reactions)
                  IS DISTINCT FROM (EXCLUDED.private_messages_privacy,EXCLUDED.blocked_users_messages_visibility,EXCLUDED.show_situation_reactions)
                  THEN timezone('UTC',now()) ELSE current.updated_at END")
                .bind(address).bind(private_messages_privacy.as_str()).bind(blocked_users_messages_visibility.as_str()).bind(show_situation_reactions.as_str())
                .execute(connection).await?;
        }
        Change::SocialSettingsDelete { address } => {
            sqlx::query("DELETE FROM social.user_settings WHERE address=$1")
                .bind(address)
                .execute(connection)
                .await?;
        }
    }
    Ok(())
}

/// Export only present scoped rows. Reject case variants rather than silently
/// choosing a winner whose deletion might later expose a conflicting row.
pub async fn read_snapshot(connection: &mut PgConnection) -> Result<Vec<Change>> {
    let scope: Vec<String> = sqlx::query_scalar(
        "SELECT address FROM migration_capture_social_settings.scope ORDER BY address",
    )
    .fetch_all(&mut *connection)
    .await?;
    for address in &scope {
        validate_address(address)?;
    }
    let rows=sqlx::query("SELECT address,private_messages_privacy,blocked_users_messages_visibility,show_situation_reactions FROM public.social_settings WHERE lower(address)=ANY($1) ORDER BY address LIMIT 100001")
        .bind(&scope).fetch_all(connection).await?;
    ensure!(
        rows.len() <= 100_000,
        "social settings baseline exceeds bounded export size"
    );
    rows.into_iter().map(|row| {
        let change: Change = serde_json::from_value(serde_json::json!({
            "op":"social_settings_upsert", "address":row.try_get::<String,_>("address")?,
            "private_messages_privacy":row.try_get::<String,_>("private_messages_privacy")?,
            "blocked_users_messages_visibility":row.try_get::<String,_>("blocked_users_messages_visibility")?,
            "show_situation_reactions":row.try_get::<String,_>("show_situation_reactions")?
        })).map_err(|_|anyhow::anyhow!("unsupported social settings value"))?;
        change.validate()?;
        Ok(change)
    }).collect()
}

pub async fn canonical_target(
    connection: &mut PgConnection,
    scope: &[String],
) -> Result<Vec<Value>> {
    for address in scope {
        validate_address(address)?;
    }
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('op','social_settings_upsert','address',address,'private_messages_privacy',private_messages_privacy,'blocked_users_messages_visibility',blocked_users_messages_visibility,'show_situation_reactions',show_situation_reactions) FROM social.user_settings WHERE address=ANY($1) ORDER BY address")
        .bind(scope).fetch_all(connection).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_require_all_fields_and_known_values() {
        let good = serde_json::json!({"op":"social_settings_upsert","address":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","private_messages_privacy":"all","blocked_users_messages_visibility":"show_messages","show_situation_reactions":"hide"});
        let change: Change = serde_json::from_value(good.clone()).unwrap();
        change.validate().unwrap();
        let mut invalid = good.clone();
        invalid["private_messages_privacy"] = serde_json::json!("unknown");
        assert!(serde_json::from_value::<Change>(invalid).is_err());
        let mut missing = good;
        missing
            .as_object_mut()
            .unwrap()
            .remove("show_situation_reactions");
        assert!(serde_json::from_value::<Change>(missing).is_err());
        assert!(validate_address("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_err());
    }
}
