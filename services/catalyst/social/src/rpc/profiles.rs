//! Profile fields exposed by the social RPC, resolved from the active profile
//! entity in the shared content database. The first avatar must have a userId,
//! name (or unclaimedName) and face256; the stored face256 is returned verbatim.
//! List procedures omit unavailable profiles; block/unblock keep address-only
//! fallbacks. Connectivity subscriptions skip missing profiles. Public identity
//! and source cache facts are shared with communities, separate from actor keys.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;

use crate::communities::profiles::{RegistryError, RegistryProfile, RegistryReader};
use crate::rpc::proto::common::Color3;
use crate::rpc::proto::v2::{BlockedUserProfile, FriendProfile};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NameColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl From<NameColor> for Color3 {
    fn from(c: NameColor) -> Self {
        Color3 {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// Public profile identity; authorization continues to use the caller/pointer.
    pub address: String,
    pub name: String,
    pub profile_picture_url: String,
    pub has_claimed_name: bool,
    pub name_color: Option<NameColor>,
}

/// Preserves first-seen order without a quadratic scan.
#[cfg(test)]
fn dedup_lowercased(addresses: &[String]) -> Vec<String> {
    crate::communities::profiles::first_seen(addresses.iter().map(|address| address.to_lowercase()))
}

#[derive(Clone)]
pub struct Profiles {
    registry: Arc<RegistryReader>,
}
impl Profiles {
    pub fn new(pool: Option<PgPool>) -> Self {
        Self::from_registry(Arc::new(RegistryReader::new(pool)))
    }
    pub fn from_registry(registry: Arc<RegistryReader>) -> Self {
        Self { registry }
    }
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        self.registry.cleanup_expired().await
    }
    pub async fn get_profiles(&self, addresses: &[String]) -> HashMap<String, ProfileInfo> {
        self.get_profiles_result(addresses)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error,"profile source unavailable");
                HashMap::new()
            })
    }
    pub(crate) async fn get_profiles_result(
        &self,
        addresses: &[String],
    ) -> Result<HashMap<String, ProfileInfo>, sqlx::Error> {
        Ok(self
            .get_profiles_list_result(addresses)
            .await?
            .into_iter()
            .map(|profile| (profile.address.clone(), profile))
            .collect())
    }
    pub(crate) async fn get_profiles_list_result(
        &self,
        addresses: &[String],
    ) -> Result<Vec<ProfileInfo>, sqlx::Error> {
        Ok(self
            .get_profiles_with_actors_result(addresses)
            .await?
            .into_iter()
            .map(|(_, profile)| profile)
            .collect())
    }
    /// Requested actor remains separate from the public profile for guards.
    pub(crate) async fn get_profiles_with_actors_result(
        &self,
        addresses: &[String],
    ) -> Result<Vec<(String, ProfileInfo)>, sqlx::Error> {
        Ok(self
            .registry
            .batch(addresses)
            .await?
            .into_iter()
            .map(|(actor, profile)| (actor, rpc_profile(profile)))
            .collect())
    }
    pub(crate) async fn get_profile_result(
        &self,
        address: &str,
    ) -> Result<Option<ProfileInfo>, sqlx::Error> {
        match self.registry.single(address).await {
            Ok(profile) => Ok(Some(rpc_profile(profile))),
            Err(RegistryError::Database(error)) => Err(error),
            Err(RegistryError::NotFound(_) | RegistryError::Invalid(_)) => Ok(None),
        }
    }
    pub async fn get_profile(&self, address: &str) -> Option<ProfileInfo> {
        self.get_profile_result(address)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error,"single profile enrichment unavailable");
                None
            })
    }

    pub async fn friend_profile(&self, address: &str) -> FriendProfile {
        friend_profile(address, self.get_profile(address).await.as_ref())
    }

    /// Whether a profile source is attached. With one, a wallet without a
    /// usable profile is left out where upstream's registry drops it (a
    /// friend, a request, a friendship update, a connectivity update) and
    /// fails a friendship upsert; without one (a node with no content, the
    /// tests' pool-less reader) every wallet is the placeholder.
    pub fn requires_profiles(&self) -> bool {
        self.registry.attached()
    }

    /// `address`'s profile, or the placeholder; `None` only when a profile
    /// source is attached and holds no usable profile for it.
    pub async fn existing_friend_profile(&self, address: &str) -> Option<FriendProfile> {
        match self.get_profile_result(address).await {
            Ok(Some(info)) => Some(friend_profile(address, Some(&info))),
            Ok(None) if self.requires_profiles() => None,
            Ok(None) => Some(friend_profile(address, None)),
            Err(error) => {
                tracing::warn!(%error,"profile source unavailable; retaining connectivity placeholder");
                Some(friend_profile(address, None))
            }
        }
    }

    /// Resolved profiles in registry order: cached values before fresh values.
    /// Unavailable profiles are omitted; public aliases retain avatar.userId.
    pub async fn friend_profiles(
        &self,
        addresses: &[String],
    ) -> Result<Vec<FriendProfile>, sqlx::Error> {
        Ok(self
            .get_profiles_list_result(addresses)
            .await?
            .iter()
            .map(|profile| friend_profile(&profile.address, Some(profile)))
            .collect())
    }

    pub async fn blocked_profile(
        &self,
        address: &str,
        blocked_at_ms: Option<i64>,
    ) -> BlockedUserProfile {
        blocked_profile(
            address,
            self.get_profile(address).await.as_ref(),
            blocked_at_ms,
        )
    }
}

fn rpc_profile(profile: RegistryProfile) -> ProfileInfo {
    let name_color = profile.name_color.as_ref().and_then(|color| {
        Some(NameColor {
            r: color.get("r")?.as_f64()? as f32,
            g: color.get("g")?.as_f64()? as f32,
            b: color.get("b")?.as_f64()? as f32,
        })
    });
    ProfileInfo {
        address: profile.address,
        name: profile.name,
        profile_picture_url: profile.profile_picture_url,
        has_claimed_name: profile.has_claimed_name,
        name_color,
    }
}

pub fn friend_profile(address: &str, info: Option<&ProfileInfo>) -> FriendProfile {
    match info {
        Some(info) => FriendProfile {
            address: info.address.clone(),
            name: info.name.clone(),
            has_claimed_name: info.has_claimed_name,
            profile_picture_url: info.profile_picture_url.clone(),
            name_color: info.name_color.map(Into::into),
        },
        None => FriendProfile {
            address: address.to_lowercase(),
            name: String::new(),
            has_claimed_name: false,
            profile_picture_url: String::new(),
            name_color: None,
        },
    }
}

pub fn blocked_profile(
    address: &str,
    info: Option<&ProfileInfo>,
    blocked_at_ms: Option<i64>,
) -> BlockedUserProfile {
    match info {
        Some(info) => BlockedUserProfile {
            address: info.address.clone(),
            name: info.name.clone(),
            has_claimed_name: info.has_claimed_name,
            profile_picture_url: info.profile_picture_url.clone(),
            blocked_at: blocked_at_ms,
            name_color: info.name_color.map(Into::into),
        },
        None => BlockedUserProfile {
            address: address.to_lowercase(),
            name: String::new(),
            has_claimed_name: false,
            profile_picture_url: String::new(),
            blocked_at: blocked_at_ms,
            name_color: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(addrs: &[String]) -> Vec<String> {
        let mut w = Vec::new();
        for a in addrs {
            let lc = a.to_lowercase();
            if !w.contains(&lc) {
                w.push(lc);
            }
        }
        w
    }

    #[test]
    fn dedup_lowercased_preserves_first_seen_order_and_matches_reference() {
        let mixed: Vec<String> = ["0xAbC", "0xabc", "0xDEF", "0xdef", "0xABC", "0xghi"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(dedup_lowercased(&mixed), vec!["0xabc", "0xdef", "0xghi"]);
        assert_eq!(dedup_lowercased(&mixed), reference(&mixed));
        assert_eq!(dedup_lowercased(&[]), reference(&[]));
        let dups: Vec<String> = vec!["0xZ".to_string(); 8];
        assert_eq!(dedup_lowercased(&dups), reference(&dups));
    }

    #[test]
    fn dedup_of_twenty_thousand_addresses_is_not_quadratic() {
        let addrs: Vec<String> = (0..20_000)
            .map(|i| {
                let v = i % 5_000;
                if i % 2 == 0 {
                    format!("0xADDR{v}")
                } else {
                    format!("0xaddr{v}")
                }
            })
            .collect();
        let kept = dedup_lowercased(&addrs);
        assert_eq!(kept.len(), 5_000);
        let shared =
            crate::communities::profiles::first_seen(addrs.iter().map(|a| a.to_lowercase()));
        assert_eq!(kept, shared, "the dedup is the shared first-seen pass");
        let compared = crate::communities::profiles::first_seen_comparisons(&addrs);
        assert!(
            compared < 2 * addrs.len(),
            "{compared} key comparisons for {} addresses: the kept keys are scanned, not probed",
            addrs.len()
        );
    }

    #[tokio::test]
    async fn absent_profiles_are_omitted_from_lists_and_block_responses_keep_the_address() {
        let profiles = Profiles::new(None);
        assert!(
            profiles
                .friend_profiles(&["0xB".into(), "0xa".into()])
                .await
                .unwrap()
                .is_empty()
        );
        let blocked = profiles.blocked_profile("0xA", Some(7)).await;
        assert_eq!(
            (blocked.address.as_str(), blocked.blocked_at),
            ("0xa", Some(7))
        );
    }

    #[tokio::test]
    async fn first_avatar_requires_identity_name_and_face_and_preserves_snapshot_url() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let address = format!("0x{}", &format!("{tag}{tag}")[..40]);
        let valid = serde_json::json!({"userId": address.to_uppercase(), "name": "Alice", "hasClaimedName": true,
            "nameColor": {"r": 0.5, "g": 0.25, "b": 1.0},
            "avatar": {"snapshots": {"face256": "https://example.test/stored-face.png"}}});
        let mut fallback = valid.clone();
        fallback["name"] = serde_json::json!("");
        fallback["unclaimedName"] = serde_json::json!("Unclaimed");
        let mut cases = vec![
            (valid.clone(), Some("Alice")),
            (fallback, Some("Unclaimed")),
        ];
        for key in ["userId", "name", "avatar"] {
            let mut avatar = valid.clone();
            avatar.as_object_mut().unwrap().remove(key);
            cases.push((avatar, None));
        }
        let mut empty_face = valid.clone();
        empty_face["avatar"]["snapshots"]["face256"] = serde_json::json!("");
        cases.push((empty_face, None));
        let mut wrong_identity = valid.clone();
        wrong_identity["userId"] = serde_json::json!("different-user");
        cases.push((wrong_identity, Some("Alice")));
        cases.push((serde_json::json!([]), None));
        for (index, (avatar, expected_name)) in cases.into_iter().enumerate() {
            let public_address = avatar
                .get("userId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_lowercase);
            let entity = format!("bafkreiprofile{tag}{index}");
            let metadata = serde_json::json!({"v":{"avatars":[avatar, valid.clone()]}});
            sqlx::query("INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) VALUES ($1,'v3','profile',$2,$3,to_timestamp(1451606400),ARRAY[$1],now(),'[]'::json)")
                .bind(&address).bind(&entity).bind(metadata).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile') ON CONFLICT(pointer) DO UPDATE SET entity_id=EXCLUDED.entity_id")
                .bind(&address).bind(&entity).execute(&pool).await.unwrap();
            // Each reducer case models a fresh source Redis cache; a separate
            // regression covers shared retention across actual content updates.
            sqlx::query("DELETE FROM social.registry_profile_cache WHERE profile_key=$1")
                .bind(&address)
                .execute(&pool)
                .await
                .unwrap();
            let profiles = Profiles::new(Some(pool.clone()));
            let result = profiles
                .friend_profiles(&[address.to_uppercase()])
                .await
                .unwrap();
            if let Some(name) = expected_name {
                assert_eq!(result.len(), 1);
                assert_eq!(Some(result[0].address.as_str()), public_address.as_deref());
                assert_eq!(result[0].name, name);
                assert_eq!(
                    result[0].profile_picture_url,
                    "https://example.test/stored-face.png"
                );
                assert!(result[0].has_claimed_name);
                assert_eq!(
                    result[0].name_color,
                    Some(Color3 {
                        r: 0.5,
                        g: 0.25,
                        b: 1.0
                    })
                );
            } else {
                assert!(result.is_empty(), "case {index}: {result:?}");
            }
        }
    }
}
