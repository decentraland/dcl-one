//! Profile fields exposed by the social RPC, resolved from the active profile
//! entity in the shared content database. The first avatar must have a userId,
//! name (or unclaimedName) and face256; the stored face256 is returned verbatim.
//! List procedures omit unavailable profiles, while block/unblock responses and
//! connectivity may represent an unavailable profile with empty display fields.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::rpc::proto::common::Color3;
use crate::rpc::proto::v2::{BlockedUserProfile, FriendProfile};
use crate::shared::cache::TtlMap;

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
    pub name: String,
    pub profile_picture_url: String,
    pub has_claimed_name: bool,
    pub name_color: Option<NameColor>,
}

const CACHE_TTL: Duration = Duration::from_secs(600);
const CACHE_MAX_ENTRIES: usize = 100_000;

/// Preserves first-seen order without a quadratic scan, so a 10^3-friend
/// page stays linear.
fn dedup_lowercased(addresses: &[String]) -> Vec<String> {
    crate::communities::profiles::first_seen(addresses.iter().map(|a| a.to_lowercase()))
}

/// `active_pointers` names the profile entity of each wallet (the pointer is
/// the lowercase address); the metadata read is one `= ANY` over the
/// `deployments_entity_id_key` probe of those ids. The column wraps the
/// deployed metadata as `{"v": ...}` (content::queries).
const PROFILE_SQL: &str = "\
    SELECT ap.pointer, d.entity_metadata::jsonb #> '{v,avatars,0}' AS avatar \
    FROM content.active_pointers ap \
    JOIN content.deployments d ON d.entity_id = ap.entity_id \
    WHERE ap.pointer = ANY($1::text[]) AND d.entity_type = 'profile'";

#[derive(Clone)]
pub struct Profiles {
    pool: Option<PgPool>,
    cache: Arc<TtlMap<String, Option<ProfileInfo>>>,
}

impl Profiles {
    /// A `None` pool resolves no profiles.
    pub fn new(pool: Option<PgPool>) -> Self {
        Self {
            pool,
            cache: Arc::new(TtlMap::bounded(CACHE_TTL, CACHE_MAX_ENTRIES)),
        }
    }

    pub async fn get_profiles(&self, addresses: &[String]) -> HashMap<String, ProfileInfo> {
        self.get_profiles_result(addresses)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "profile enrichment query failed; serving placeholders");
                addresses
                    .iter()
                    .filter_map(|address| {
                        self.cache
                            .get_fresh(&address.to_lowercase())
                            .flatten()
                            .map(|profile| (address.to_lowercase(), profile))
                    })
                    .collect()
            })
    }

    pub(crate) async fn get_profiles_result(
        &self,
        addresses: &[String],
    ) -> Result<HashMap<String, ProfileInfo>, sqlx::Error> {
        let mut out: HashMap<String, ProfileInfo> = HashMap::new();
        if addresses.is_empty() {
            return Ok(out);
        }
        let wanted = dedup_lowercased(addresses);
        let mut misses: Vec<String> = Vec::new();
        for addr in &wanted {
            match self.cache.get_fresh(addr) {
                Some(Some(info)) => {
                    out.insert(addr.clone(), info);
                }
                Some(None) => {}
                None => misses.push(addr.clone()),
            }
        }
        if misses.is_empty() {
            return Ok(out);
        }
        let Some(pool) = &self.pool else {
            return Ok(out);
        };

        let mut resolved: HashMap<String, ProfileInfo> = HashMap::new();
        match sqlx::query_as::<_, (String, Option<serde_json::Value>)>(PROFILE_SQL)
            .bind(&misses)
            .fetch_all(pool)
            .await
        {
            Ok(rows) => {
                for (addr, avatar) in rows {
                    let Some(avatar) = avatar else { continue };
                    let nonempty = |key| {
                        avatar
                            .get(key)
                            .and_then(serde_json::Value::as_str)
                            .filter(|v| !v.is_empty())
                    };
                    let Some(user_id) = nonempty("userId") else {
                        continue;
                    };
                    // Upstream keys the profile map by avatar.userId, not by
                    // the requested pointer; a mismatched profile cannot enrich it.
                    if user_id.to_lowercase() != addr {
                        continue;
                    }
                    let Some(name) = nonempty("name").or_else(|| nonempty("unclaimedName")) else {
                        continue;
                    };
                    let Some(picture) = avatar
                        .pointer("/avatar/snapshots/face256")
                        .and_then(serde_json::Value::as_str)
                        .filter(|v| !v.is_empty())
                    else {
                        continue;
                    };
                    let name_color = avatar.get("nameColor").and_then(|color| {
                        Some(NameColor {
                            r: color.get("r")?.as_f64()? as f32,
                            g: color.get("g")?.as_f64()? as f32,
                            b: color.get("b")?.as_f64()? as f32,
                        })
                    });
                    resolved.insert(
                        addr.to_lowercase(),
                        ProfileInfo {
                            name: name.to_string(),
                            profile_picture_url: picture.to_string(),
                            has_claimed_name: avatar
                                .get("hasClaimedName")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                            name_color,
                        },
                    );
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
        for addr in misses {
            let info = resolved.get(&addr).cloned();
            if let Some(info) = &info {
                out.insert(addr.clone(), info.clone());
            }
            // Upstream only caches resolved profiles. A missing profile may be
            // deployed immediately after this read and must be retried.
            if info.is_some() {
                self.cache.insert(addr, info);
            }
        }
        Ok(out)
    }

    pub async fn get_profile(&self, address: &str) -> Option<ProfileInfo> {
        self.get_profiles(std::slice::from_ref(&address.to_string()))
            .await
            .remove(&address.to_lowercase())
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
        self.pool.is_some()
    }

    /// The profiles found, and whether a missing one means "no profile": a
    /// failed read only says the source could not answer, so it leaves
    /// nobody out (the cached profiles and placeholders, as [`Self::get_profiles`]).
    pub async fn known_profiles(
        &self,
        addresses: &[String],
    ) -> (HashMap<String, ProfileInfo>, bool) {
        if !self.requires_profiles() {
            return (self.get_profiles(addresses).await, false);
        }
        match self.get_profiles_result(addresses).await {
            Ok(map) => (map, true),
            Err(error) => {
                tracing::warn!(%error, "profile query failed; keeping every wallet");
                (self.get_profiles(addresses).await, false)
            }
        }
    }

    /// `address`'s profile, or the placeholder; `None` only when a profile
    /// source is attached and holds no usable profile for it.
    pub async fn existing_friend_profile(&self, address: &str) -> Option<FriendProfile> {
        let (mut map, strict) = self
            .known_profiles(std::slice::from_ref(&address.to_string()))
            .await;
        let info = map.remove(&address.to_lowercase());
        if info.is_none() && strict {
            return None;
        }
        Some(friend_profile(address, info.as_ref()))
    }

    /// [`Self::friend_profiles`] without the wallets
    /// [`Self::existing_friend_profile`] would leave out, in input order
    /// (upstream `parseProfilesToFriends` over the registry's answer).
    pub async fn existing_friend_profiles(&self, addresses: &[String]) -> Vec<FriendProfile> {
        let (map, strict) = self.known_profiles(addresses).await;
        addresses
            .iter()
            .filter_map(|a| {
                let info = map.get(&a.to_lowercase());
                (info.is_some() || !strict).then(|| friend_profile(a, info))
            })
            .collect()
    }

    /// Resolved profiles in input order. Upstream's profile-returning list
    /// methods omit addresses whose profiles cannot be reduced.
    pub async fn friend_profiles(
        &self,
        addresses: &[String],
    ) -> Result<Vec<FriendProfile>, sqlx::Error> {
        let map = self.get_profiles_result(addresses).await?;
        Ok(addresses
            .iter()
            .filter_map(|a| {
                map.get(&a.to_lowercase())
                    .map(|p| friend_profile(a, Some(p)))
            })
            .collect())
    }

    /// A strict bounded initial replay batch; read failure retires this stream rather
    /// than certifying initialization. Missing profiles retain the established representation.
    pub async fn connectivity_profiles(
        &self,
        addresses: &[String],
    ) -> Result<Vec<FriendProfile>, sqlx::Error> {
        let map = self.get_profiles_result(addresses).await?;
        Ok(addresses
            .iter()
            .map(|address| {
                connectivity_profile(friend_profile(address, map.get(&address.to_lowercase())))
            })
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

/// Reuse the existing enrichment-unavailable profile when optional display metadata
/// cannot fit connectivity's item budget. Identity/status/authorization are never trimmed.
pub(crate) fn connectivity_profile(profile: FriendProfile) -> FriendProfile {
    use prost::Message as _;
    if profile.encoded_len() > crate::rpc::drpc::transport::MAX_GUARDED_MESSAGE_BYTES - 512 {
        tracing::warn!(address = %profile.address, "connectivity enrichment exceeds item budget; using existing unavailable-profile representation");
        friend_profile(&profile.address, None)
    } else {
        profile
    }
}

pub fn friend_profile(address: &str, info: Option<&ProfileInfo>) -> FriendProfile {
    match info {
        Some(info) => FriendProfile {
            address: address.to_lowercase(),
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
            address: address.to_lowercase(),
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
        cases.push((wrong_identity, None));
        cases.push((serde_json::json!([]), None));
        for (index, (avatar, expected_name)) in cases.into_iter().enumerate() {
            let entity = format!("bafkreiprofile{tag}{index}");
            let metadata = serde_json::json!({"v":{"avatars":[avatar, valid.clone()]}});
            sqlx::query("INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) VALUES ($1,'v3','profile',$2,$3,now(),ARRAY[$1],now(),'[]'::json)")
                .bind(&address).bind(&entity).bind(metadata).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile') ON CONFLICT(pointer) DO UPDATE SET entity_id=EXCLUDED.entity_id")
                .bind(&address).bind(&entity).execute(&pool).await.unwrap();
            let profiles = Profiles::new(Some(pool.clone()));
            let result = profiles
                .friend_profiles(&[address.to_uppercase()])
                .await
                .unwrap();
            if let Some(name) = expected_name {
                assert_eq!(result.len(), 1);
                assert_eq!(result[0].address, address);
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
