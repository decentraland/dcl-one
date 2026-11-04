//! Profile enrichment for member, request, ban and friend rows: name,
//! picture, claimed-name flag and name colour read from the active profile
//! entity in `content.deployments`. The picture is the face the lambdas'
//! snapshots name, preserving the stored `face256` value. The old service reached the content
//! database through a registry; reads here use the ONE pool and one shared
//! SQL registry cache with the source600-second key lifetime. Public profile
//! identity is separate from requested actor/authentication. Batch decoration
//! can omit unavailable data; single owner lookups preserve source errors.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;

/// A profile's `nameColor` as production writes it: upstream passes any
/// truthy value through as stored (`src/logic/profiles.ts` at `fb3dcb8`,
/// `...(nameColor && { nameColor })`), and `JSON.stringify` writes each number
/// in JavaScript's form (`1` for a stored `1.0`, `0.6801473` as stored, see
/// [`js_number`]); the usual shape is `{r, g, b}` with `a` when the profile has
/// one (capture `social/community-members`). The type names that usual shape;
/// a stored value of any other truthy shape is passed through unchanged.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "communities/")
)]
pub struct NameColor(
    #[cfg_attr(
        feature = "ts",
        ts(type = "{ r: number, g: number, b: number, a?: number, }")
    )]
    serde_json::Value,
);

impl NameColor {
    /// The stored value when JavaScript reads it as truthy (anything but
    /// `null`, `false`, `0` and `""`), its numbers in `JSON.stringify`'s form.
    pub fn from_profile(value: &serde_json::Value) -> Option<Self> {
        use serde_json::Value;
        let truthy = match value {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64() != Some(0.0),
            Value::String(s) => !s.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        };
        truthy.then(|| Self(js_numbers(value)))
    }
}

/// An object writes `r`, `g`, `b` and `a` first, in the order a profile
/// stores them, then any other key; any other value as it is.
impl Serialize for NameColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let serde_json::Value::Object(map) = &self.0 else {
            return self.0.serialize(serializer);
        };
        const FIRST: [&str; 4] = ["r", "g", "b", "a"];
        let mut out = serializer.serialize_map(Some(map.len()))?;
        for key in FIRST {
            if let Some(v) = map.get(key) {
                out.serialize_entry(key, v)?;
            }
        }
        for (k, v) in map.iter().filter(|(k, _)| !FIRST.contains(&k.as_str())) {
            out.serialize_entry(k, v)?;
        }
        out.end()
    }
}

/// `value` with every number in [`js_number`]'s form.
fn js_numbers(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Number(n) => Value::Number(js_number(n)),
        Value::Array(items) => Value::Array(items.iter().map(js_numbers).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), js_numbers(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// A JSON number as `JSON.stringify` writes it: a whole value within the
/// safe-integer range as an integer (`1.0` and `1.00` read `1`); any other
/// keeps its `f64`, whose shortest form is JavaScript's in that range.
fn js_number(n: &serde_json::Number) -> serde_json::Number {
    const SAFE: f64 = 9_007_199_254_740_992.0; // 2^53
    match n.as_f64() {
        Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < SAFE => {
            serde_json::Number::from(f as i64)
        }
        _ => n.clone(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileInfo {
    pub name: String,
    #[serde(rename = "profilePictureUrl")]
    pub profile_picture_url: String,
    #[serde(rename = "hasClaimedName")]
    pub has_claimed_name: bool,
    #[serde(rename = "nameColor", skip_serializing_if = "Option::is_none")]
    pub name_color: Option<NameColor>,
}

/// Each key once, in first-seen order: one set probe per key rather than a
/// `Vec::contains` scan of the kept ones, which made this shared primitive
/// quadratic in the key count.
pub(crate) fn first_seen<K: Eq + std::hash::Hash + Clone>(
    keys: impl ExactSizeIterator<Item = K>,
) -> Vec<K> {
    let mut seen = HashSet::with_capacity(keys.len());
    let mut wanted = Vec::with_capacity(keys.len());
    for key in keys {
        if seen.insert(key.clone()) {
            wanted.push(key);
        }
    }
    wanted
}

fn dedup_requested(addresses: &[String]) -> Vec<String> {
    first_seen(addresses.iter().cloned())
}

/// The key comparisons [`first_seen`] makes deduplicating exact requested keys:
/// one per repeat (and the odd hash collision) for a set probe, the kept count per
/// key for a scan.
#[cfg(test)]
pub(crate) fn first_seen_comparisons(addresses: &[String]) -> usize {
    thread_local! {
        static COMPARED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    #[derive(Clone)]
    struct Counted(String);
    impl PartialEq for Counted {
        fn eq(&self, other: &Self) -> bool {
            COMPARED.set(COMPARED.get() + 1);
            self.0 == other.0
        }
    }
    impl Eq for Counted {}
    impl std::hash::Hash for Counted {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
    }
    COMPARED.set(0);
    let kept = first_seen(addresses.iter().map(|a| Counted(a.clone())));
    assert_eq!(kept.len(), dedup_requested(addresses).len());
    COMPARED.get()
}

/// Where a profile's face is served, as the lambdas' snapshots name it
/// (`market::profiles::rewrite_snapshot_urls`): `<base>entities/<profile
/// entity id>/face.png` under `PROFILE_CDN_BASE_URL`, which serves every
/// profile, or under this node's `/profile-images/`, which serves an uploaded
/// face it holds and, when it `derives` (an origin or a renderer is
/// configured), the face of any profile.
#[derive(Clone, Debug, Default)]
pub struct FaceUrls {
    base: String,
    derives: bool,
}

impl FaceUrls {
    pub fn new(cdn: Option<&str>, profile_images: &str, derives: bool) -> Self {
        let base = cdn.unwrap_or(profile_images).trim_end_matches('/');
        Self {
            base: format!("{base}/"),
            derives: derives || cdn.is_some(),
        }
    }

    /// The picture of the profile entity `entity_id`; `None` when it uploaded
    /// no face and nothing here derives one.
    pub fn picture(&self, entity_id: &str, face256: Option<&str>) -> Option<String> {
        let uploaded = face256.is_some_and(|f| !f.is_empty());
        (uploaded || self.derives).then(|| format!("{}entities/{entity_id}/face.png", self.base))
    }
}

/// Minimal public profile facts in the legacy registry cache. `address` is the
/// avatar userId, independent of the requested/authenticated wallet. Community
/// nameColor keeps its raw JSON; the RPC DTO narrows it to Color3 separately.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryProfile {
    pub address: String,
    pub name: String,
    pub profile_picture_url: String,
    pub has_claimed_name: bool,
    pub name_color: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("Profile not found: {0}")]
    NotFound(String),
    #[error("Invalid profile received from registry: {0}")]
    Invalid(String),
}

const PROFILE_SQL: &str = "\
    SELECT ap.pointer, d.entity_metadata::jsonb #> '{v,avatars,0}' AS avatar \
    FROM content.active_pointers ap \
    JOIN content.deployments d ON d.entity_id=ap.entity_id \
    WHERE ap.pointer=ANY($1::text[]) AND d.entity_type='profile' \
    ORDER BY array_position($1::text[],ap.pointer)";

const CACHE_WRITE_SQL: &str = "INSERT INTO social.registry_profile_cache(profile_key,profile,expires_at) SELECT entry.profile_key,entry.profile,timezone('UTC',clock_timestamp())+interval '600 seconds' FROM unnest($1::text[],$2::jsonb[]) AS entry(profile_key,profile) ON CONFLICT(profile_key) DO UPDATE SET profile=EXCLUDED.profile,expires_at=timezone('UTC',clock_timestamp())+interval '600 seconds'";

/// Shared source registry cache over the existing pool. Each reader instance
/// observes the same key/TTL facts; no native content cache is invalidated.
#[derive(Debug)]
pub struct RegistryReader {
    pool: Option<PgPool>,
    #[cfg(test)]
    seen: std::sync::Mutex<HashSet<String>>,
}
impl RegistryReader {
    pub fn new(pool: Option<PgPool>) -> Self {
        Self {
            pool,
            #[cfg(test)]
            seen: Default::default(),
        }
    }
    pub fn attached(&self) -> bool {
        self.pool.is_some()
    }

    async fn cached(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, RegistryProfile>, sqlx::Error> {
        #[cfg(test)]
        self.seen.lock().unwrap().extend(keys.iter().cloned());
        let Some(pool) = &self.pool else {
            return Ok(HashMap::new());
        };
        let rows=sqlx::query_as::<_,(String,sqlx::types::Json<RegistryProfile>)>("SELECT profile_key,profile FROM social.registry_profile_cache WHERE profile_key=ANY($1::text[]) AND expires_at>timezone('UTC',clock_timestamp())")
            .bind(keys).fetch_all(pool).await?;
        Ok(rows
            .into_iter()
            .map(|(key, profile)| (key, profile.0))
            .collect())
    }
    async fn put(&self, key: &str, profile: &RegistryProfile) {
        self.put_many(&[(key.to_string(), profile.clone())]).await;
    }
    async fn put_many(&self, profiles: &[(String, RegistryProfile)]) {
        let Some(pool) = &self.pool else { return };
        if profiles.is_empty() {
            return;
        }
        // One write per source key; later returned entries win, while the
        // caller's returned list retains every cached/fetched duplicate.
        let mut last = HashMap::new();
        for (key, profile) in profiles {
            last.insert(key.clone(), profile);
        }
        let mut keys = Vec::with_capacity(last.len());
        let mut values = Vec::with_capacity(last.len());
        for (key, profile) in last {
            keys.push(key);
            values.push(serde_json::to_value(profile).expect("minimal JSON profile"));
        }
        #[cfg(test)]
        self.seen.lock().unwrap().extend(keys.iter().cloned());
        if let Err(error) = sqlx::query(CACHE_WRITE_SQL)
            .bind(&keys)
            .bind(&values)
            .execute(pool)
            .await
        {
            if keys.len() == 1 {
                tracing::warn!(%error,"minimal profile cache write failed");
            } else {
                // A failed key must not prevent other source cache writes.
                // The normal path remains one bounded set operation.
                for (key, profile) in keys.iter().zip(&values) {
                    if let Err(error) = sqlx::query(CACHE_WRITE_SQL)
                        .bind(std::slice::from_ref(key))
                        .bind(std::slice::from_ref(profile))
                        .execute(pool)
                        .await
                    {
                        tracing::warn!(%error,profile_key=%key,"minimal profile cache write failed");
                    }
                }
            }
        }
    }
    async fn avatars(
        &self,
        keys: &[String],
    ) -> Result<Vec<(String, Option<serde_json::Value>)>, sqlx::Error> {
        let Some(pool) = &self.pool else {
            return Ok(Vec::new());
        };
        // Keep the source's requested cache key separate from the local
        // content pointer, whose storage is case-normalized. The local profile
        // registry resolves one document per normalized pointer.
        let mut actors = HashMap::new();
        for key in keys {
            actors
                .entry(key.to_lowercase())
                .or_insert_with(|| key.clone());
        }
        let pointers = first_seen(keys.iter().map(|key| key.to_lowercase()));
        let rows: Vec<(String, Option<serde_json::Value>)> = sqlx::query_as(PROFILE_SQL)
            .bind(&pointers)
            .fetch_all(pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(pointer, avatar)| (actors.remove(&pointer).expect("requested pointer"), avatar))
            .collect())
    }
    pub async fn single(&self, address: &str) -> Result<RegistryProfile, RegistryError> {
        let key = address.to_string();
        if let Some(profile) = self.cached(std::slice::from_ref(&key)).await?.remove(&key) {
            return Ok(profile);
        }
        let Some((_, avatar)) = self
            .avatars(std::slice::from_ref(&key))
            .await?
            .into_iter()
            .next()
        else {
            return Err(RegistryError::NotFound(key));
        };
        let profile = avatar
            .as_ref()
            .and_then(reduce_registry_profile)
            .ok_or_else(|| RegistryError::Invalid(key.clone()))?;
        self.put(&key, &profile).await;
        Ok(profile)
    }
    pub async fn batch(
        &self,
        addresses: &[String],
    ) -> Result<Vec<(String, RegistryProfile)>, sqlx::Error> {
        if addresses.is_empty() {
            return Ok(Vec::new());
        }
        let wanted = dedup_requested(addresses);
        let mut cached = self.cached(&wanted).await?;
        let mut profiles = wanted
            .iter()
            .filter_map(|key| cached.remove(key).map(|profile| (key.clone(), profile)))
            .collect::<Vec<_>>();
        let identities = profiles
            .iter()
            .map(|(_, profile)| profile.address.clone())
            .collect::<HashSet<_>>();
        let missing = wanted
            .into_iter()
            .filter(|key| !identities.contains(&key.to_lowercase()))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let mut to_cache = Vec::new();
            for (actor, avatar) in self.avatars(&missing).await? {
                let Some(profile) = avatar.as_ref().and_then(reduce_registry_profile) else {
                    continue;
                };
                to_cache.push((profile.address.clone(), profile.clone()));
                profiles.push((actor, profile));
            }
            self.put_many(&to_cache).await;
        }
        Ok(profiles)
    }
    /// Bounded expiry work, using the same shared job lifecycle as other social facts.
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let Some(pool) = &self.pool else { return Ok(0) };
        Ok(sqlx::query("DELETE FROM social.registry_profile_cache WHERE ctid=ANY(ARRAY(SELECT ctid FROM social.registry_profile_cache WHERE expires_at<=timezone('UTC',statement_timestamp()) ORDER BY expires_at,profile_key LIMIT 1000))").execute(pool).await?.rows_affected())
    }
    #[cfg(test)]
    async fn clear_seen(&self) {
        let keys = self.seen.lock().unwrap().drain().collect::<Vec<_>>();
        if let Some(pool) = &self.pool {
            sqlx::query(
                "DELETE FROM social.registry_profile_cache WHERE profile_key=ANY($1::text[])",
            )
            .bind(keys)
            .execute(pool)
            .await
            .unwrap();
        }
    }
}
fn reduce_registry_profile(avatar: &serde_json::Value) -> Option<RegistryProfile> {
    let nonempty = |key| {
        avatar
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
    };
    Some(RegistryProfile {
        address: nonempty("userId")?.to_lowercase(),
        name: nonempty("name")
            .or_else(|| nonempty("unclaimedName"))?
            .to_string(),
        profile_picture_url: avatar
            .pointer("/avatar/snapshots/face256")?
            .as_str()
            .filter(|value| !value.is_empty())?
            .to_string(),
        has_claimed_name: avatar
            .get("hasClaimedName")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        name_color: avatar.get("nameColor").cloned(),
    })
}

#[derive(Clone)]
pub struct ProfilesCache {
    registry: Arc<RegistryReader>,
}
impl ProfilesCache {
    pub fn new(pool: PgPool, _faces: FaceUrls) -> Self {
        Self {
            registry: Arc::new(RegistryReader::new(Some(pool))),
        }
    }
    pub fn registry(&self) -> Arc<RegistryReader> {
        self.registry.clone()
    }
    #[cfg(test)]
    pub(crate) async fn clear(&self) {
        self.registry.clear_seen().await;
    }
    /// Best-effort reader retained for consumers whose source permits missing
    /// enrichment. HTTP community profile boundaries use the fallible reader.
    pub async fn get_profiles(&self, addresses: &[String]) -> HashMap<String, ProfileInfo> {
        self.try_get_profiles(addresses)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error,"community profile source unavailable");
                HashMap::new()
            })
    }
    pub async fn try_get_profiles(
        &self,
        addresses: &[String],
    ) -> sqlx::Result<HashMap<String, ProfileInfo>> {
        Ok(self
            .registry
            .batch(addresses)
            .await?
            .into_iter()
            .map(|(_, profile)| (profile.address.clone(), community_profile(profile)))
            .collect())
    }
    pub async fn get_owner_names(
        &self,
        addresses: &[String],
    ) -> sqlx::Result<HashMap<String, String>> {
        Ok(self
            .try_get_profiles(addresses)
            .await?
            .into_iter()
            .map(|(address, profile)| (address, profile.name))
            .collect())
    }
    pub async fn get_owner_name(&self, address: &str) -> Result<String, RegistryError> {
        Ok(self.registry.single(address).await?.name)
    }
}
fn community_profile(profile: RegistryProfile) -> ProfileInfo {
    ProfileInfo {
        name: profile.name,
        profile_picture_url: profile.profile_picture_url,
        has_claimed_name: profile.has_claimed_name,
        name_color: profile
            .name_color
            .as_ref()
            .and_then(NameColor::from_profile),
    }
}

/// `name`, `profilePictureUrl`, `hasClaimedName` (and `nameColor` when known)
/// on rows whose `addr_field` resolved; unresolved rows are omitted.
pub async fn enrich_with_profiles(
    profiles: &ProfilesCache,
    rows: &mut Vec<serde_json::Value>,
    addr_field: &str,
) -> sqlx::Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let addresses: Vec<String> = rows
        .iter()
        .filter_map(|r| {
            r.get(addr_field)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    let map = profiles.try_get_profiles(&addresses).await?;
    // The V1 member mapper omits unresolved profiles without changing the
    // database total or pagination. V2 callers do not use this mapper.
    rows.retain(|row| {
        row.get(addr_field)
            .and_then(|value| value.as_str())
            .is_some_and(|address| map.contains_key(&address.to_lowercase()))
    });
    for row in rows.iter_mut() {
        let Some(obj) = row.as_object_mut() else {
            continue;
        };
        let addr = obj
            .get(addr_field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        let info = map.get(&addr);
        let (name, pic, claimed) = match info {
            Some(info) => (
                info.name.clone(),
                info.profile_picture_url.clone(),
                info.has_claimed_name,
            ),
            None => (String::new(), String::new(), false),
        };
        obj.insert("name".to_string(), serde_json::Value::String(name));
        obj.insert(
            "profilePictureUrl".to_string(),
            serde_json::Value::String(pic),
        );
        obj.insert(
            "hasClaimedName".to_string(),
            serde_json::Value::Bool(claimed),
        );
        if let Some(nc) = info.and_then(|i| i.name_color.as_ref())
            && let Ok(v) = serde_json::to_value(nc)
        {
            obj.insert("nameColor".to_string(), v);
        }
    }
    Ok(())
}

/// The post author fields; an unresolved author falls back to the address
/// as its name (the old shape).
pub async fn enrich_posts_with_authors(
    profiles: &ProfilesCache,
    rows: &mut [serde_json::Value],
    addr_field: &str,
) -> sqlx::Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let addresses: Vec<String> = rows
        .iter()
        .filter_map(|r| {
            r.get(addr_field)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    let map = profiles.try_get_profiles(&addresses).await?;
    for row in rows.iter_mut() {
        let Some(obj) = row.as_object_mut() else {
            continue;
        };
        let addr = obj
            .get(addr_field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        let (name, pic, claimed) = match map.get(&addr) {
            Some(info) => (
                info.name.clone(),
                info.profile_picture_url.clone(),
                info.has_claimed_name,
            ),
            None => (addr.clone(), String::new(), false),
        };
        obj.insert("authorName".to_string(), serde_json::Value::String(name));
        obj.insert(
            "authorProfilePictureUrl".to_string(),
            serde_json::Value::String(pic),
        );
        obj.insert(
            "authorHasClaimedName".to_string(),
            serde_json::Value::Bool(claimed),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn deploy_registry_profile(
        pool: &PgPool,
        pointer: &str,
        user_id: &str,
        name: &str,
    ) -> String {
        let entity = format!("bafkreiregistry{}", uuid::Uuid::new_v4().simple());
        let metadata = serde_json::json!({"v":{"avatars":[{"userId":user_id,"name":name,"hasClaimedName":true,
            "nameColor":{"r":0.5,"g":0.25,"b":1.0,"a":0.75,"custom":[2.0]},
            "avatar":{"snapshots":{"face256":"https://images.example.org/registry.png"}}}]}});
        sqlx::query("INSERT INTO content.deployments(deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES($1,'v3','profile',$2,$3,to_timestamp(1451606400),ARRAY[$1],now(),'[]'::json)")
            .bind(pointer).bind(&entity).bind(metadata).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile') ON CONFLICT(pointer) DO UPDATE SET entity_id=EXCLUDED.entity_id")
            .bind(pointer).bind(&entity).execute(pool).await.unwrap();
        entity
    }

    #[tokio::test]
    async fn shared_registry_cache_preserves_alias_order_across_readers_and_content_updates() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let key = uuid::Uuid::new_v4().simple().to_string();
        let pointer = format!("0x{}", &format!("{key}{key}")[..40]);
        let key2 = uuid::Uuid::new_v4().simple().to_string();
        let alias = format!("0x{}", &format!("{key2}{key2}")[..40]);
        let old = deploy_registry_profile(&pool, &pointer, &alias, "Old public name").await;
        let a = RegistryReader::new(Some(pool.clone()));
        let b = RegistryReader::new(Some(pool.clone()));
        let first = a.batch(std::slice::from_ref(&pointer)).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!((&first[0].0, &first[0].1.address), (&pointer, &alias));
        assert_eq!(b.single(&pointer).await.unwrap().address, alias);
        let third = a.batch(std::slice::from_ref(&pointer)).await.unwrap();
        assert_eq!(
            third.len(),
            2,
            "source cached-under-requested alias plus newly fetched alias"
        );
        assert!(
            third
                .iter()
                .all(|(actor, profile)| actor == &pointer && profile.address == alias)
        );
        let community = ProfilesCache::new(pool.clone(), FaceUrls::default());
        assert_eq!(
            community.get_owner_name(&pointer).await.unwrap(),
            "Old public name"
        );
        let map = community.get_profiles(std::slice::from_ref(&pointer)).await;
        assert!(!map.contains_key(&pointer));
        let color = serde_json::to_value(map[&alias].name_color.as_ref().unwrap()).unwrap();
        assert_eq!(
            color,
            serde_json::json!({"r":0.5,"g":0.25,"b":1,"a":0.75,"custom":[2]})
        );
        let rpc = crate::rpc::profiles::Profiles::new(Some(pool.clone()));
        let public = rpc.get_profile_result(&pointer).await.unwrap().unwrap();
        assert_eq!(public.address, alias);
        assert_eq!(public.name_color.unwrap().r, 0.5);
        let ttl:f64=sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM expires_at-timezone('UTC',clock_timestamp()))::float8 FROM social.registry_profile_cache WHERE profile_key=$1")
            .bind(&pointer).fetch_one(&pool).await.unwrap();
        assert!((598.0..=600.0).contains(&ttl), "{ttl}");
        let fresh = deploy_registry_profile(&pool, &pointer, &pointer, "Fresh valid name").await;
        assert_eq!(
            b.single(&pointer).await.unwrap().name,
            "Old public name",
            "source retains the old cache until expiry after a valid content update"
        );
        sqlx::query("UPDATE social.registry_profile_cache SET expires_at=timezone('UTC',now())-interval '1 second' WHERE profile_key=$1")
            .bind(&pointer).execute(&pool).await.unwrap();
        assert_eq!(a.single(&pointer).await.unwrap().name, "Fresh valid name");
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer=$1")
            .bind(&pointer)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id=ANY($1::text[])")
            .bind(vec![old, fresh])
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM social.registry_profile_cache WHERE profile_key=ANY($1::text[])")
            .bind(vec![pointer, alias])
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn shared_registry_distinguishes_absent_invalid_and_database_failures_without_negative_cache()
     {
        let Some(pool) = crate::db::scratch::pool(3).await else {
            return;
        };
        let reader = RegistryReader::new(Some(pool.clone()));
        let pointer = format!("registry-errors-{}", uuid::Uuid::new_v4());
        assert!(matches!(
            reader.single(&pointer).await,
            Err(RegistryError::NotFound(_))
        ));
        let entity = deploy_registry_profile(&pool, &pointer, &pointer, "Valid after miss").await;
        sqlx::query("UPDATE content.deployments SET entity_metadata=json_build_object('v',json_build_object('avatars',json_build_array(json_build_object('userId',$1::text)))) WHERE entity_id=$2")
            .bind(&pointer).bind(&entity).execute(&pool).await.unwrap();
        assert!(matches!(
            reader.single(&pointer).await,
            Err(RegistryError::Invalid(_))
        ));
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.registry_profile_cache WHERE profile_key=$1",
        )
        .bind(&pointer)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            count, 0,
            "missing/invalid profiles must not create negative cache facts"
        );
        let closed = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test.invalid/unreachable")
            .unwrap();
        closed.close().await;
        assert!(matches!(
            RegistryReader::new(Some(closed)).single(&pointer).await,
            Err(RegistryError::Database(sqlx::Error::PoolClosed))
        ));
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer=$1")
            .bind(&pointer)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id=$1")
            .bind(&entity)
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn shared_registry_canonical_upgrade_preserves_populated_state_and_replays() {
        let Some(pool) = crate::db::scratch::pool(3).await else {
            return;
        };
        let schema = format!("registry_upgrade_{}", uuid::Uuid::new_v4().simple());
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}; CREATE TABLE {schema}.kept(id integer PRIMARY KEY); INSERT INTO {schema}.kept VALUES(42)"))).execute(&pool).await.unwrap();
        let source = include_str!(concat!(env!("DCL_ONE_ROOT"), "/schemas/40_social.sql"));
        let tail = &source[source
            .find("CREATE TABLE social.registry_profile_cache")
            .unwrap()..];
        let end = tail
            .find("ON social.registry_profile_cache(expires_at,profile_key);")
            .unwrap()
            + "ON social.registry_profile_cache(expires_at,profile_key);".len();
        let canonical = format!(
            "CREATE SCHEMA {schema};\n{}",
            tail[..end].replace("social.", &format!("{schema}."))
        );
        let first = crate::db::schema_migrate::migrate(&pool, &canonical, "")
            .await
            .unwrap();
        assert_eq!(first.created.len(), 2);
        assert!(first.steps.is_empty());
        sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO {schema}.registry_profile_cache VALUES('kept-key','{{\"name\":\"kept value\"}}',timezone('UTC',now())+interval '600 seconds')"))).execute(&pool).await.unwrap();
        assert!(
            crate::db::schema_migrate::migrate(&pool, &canonical, "")
                .await
                .unwrap()
                .is_empty()
        );
        let kept: i32 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT id FROM {schema}.kept")))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(kept, 42);
        let profile: serde_json::Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT profile FROM {schema}.registry_profile_cache WHERE profile_key='kept-key'"
        )))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(profile["name"], "kept value");
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn shared_registry_expiry_cleanup_is_bounded_and_retains_live_facts() {
        let Some(pool) = crate::db::scratch::pool(3).await else {
            return;
        };
        let prefix = format!("registry-cleanup-{}-", uuid::Uuid::new_v4());
        sqlx::query("INSERT INTO social.registry_profile_cache(profile_key,profile,expires_at) SELECT $1||i::text,'{}'::jsonb,timezone('UTC',now())-interval '1 second' FROM generate_series(1,1200)i").bind(&prefix).execute(&pool).await.unwrap();
        let live = format!("{prefix}live");
        sqlx::query("INSERT INTO social.registry_profile_cache VALUES($1,'{}'::jsonb,timezone('UTC',now())+interval '600 seconds')").bind(&live).execute(&pool).await.unwrap();
        let reader = RegistryReader::new(Some(pool.clone()));
        assert_eq!(reader.cleanup_expired().await.unwrap(), 1000);
        assert!(reader.cleanup_expired().await.unwrap() <= 1000);
        let expired:i64=sqlx::query_scalar("SELECT count(*) FROM social.registry_profile_cache WHERE profile_key LIKE $1 AND expires_at<=timezone('UTC',now())").bind(format!("{prefix}%")).fetch_one(&pool).await.unwrap();
        assert_eq!(expired, 0);
        let retained: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.registry_profile_cache WHERE profile_key=$1)",
        )
        .bind(&live)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(retained);
        sqlx::query("DELETE FROM social.registry_profile_cache WHERE profile_key=$1")
            .bind(live)
            .execute(&pool)
            .await
            .unwrap();
    }

    /// What `JSON.stringify` writes for a profile's colour numbers.
    #[test]
    fn a_name_color_is_written_as_javascript_writes_it() {
        let written = |raw: &str| {
            let value: serde_json::Value = serde_json::from_str(raw).unwrap();
            serde_json::to_string(&NameColor::from_profile(&value).unwrap()).unwrap()
        };
        assert_eq!(
            written(r#"{"r":1.0,"g":1.50,"b":0.6801473,"a":1}"#),
            r#"{"r":1,"g":1.5,"b":0.6801473,"a":1}"#
        );
        assert_eq!(
            written(r#"{"r":0.6801472902297974,"g":-0.0,"b":0}"#),
            r#"{"r":0.6801472902297974,"g":0,"b":0}"#
        );
        // any truthy value passes through as stored; a falsy one is no colour
        assert_eq!(
            written(r#"{"r":1,"g":"1","b":1.0,"x":[2.0]}"#),
            r#"{"r":1,"g":"1","b":1,"x":[2]}"#
        );
        assert_eq!(written(r#""red""#), r#""red""#);
        for falsy in ["null", "false", "0", "0.0", r#""""#] {
            let value: serde_json::Value = serde_json::from_str(falsy).unwrap();
            assert!(NameColor::from_profile(&value).is_none(), "{falsy}");
        }
    }

    #[test]
    fn dedup_requested_preserves_first_seen_order() {
        let mixed: Vec<String> = ["0xAbC", "0xabc", "0xDEF", "0xdef", "0xABC", "0xghi"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(dedup_requested(&mixed), mixed);
        assert!(dedup_requested(&[]).is_empty());
        let dups: Vec<String> = vec!["0xZ".to_string(); 8];
        assert_eq!(dedup_requested(&dups), vec!["0xZ"]);
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
        assert_eq!(dedup_requested(&addrs).len(), 5_000);
        let compared = first_seen_comparisons(&addrs);
        assert!(
            compared < 2 * addrs.len(),
            "{compared} key comparisons for {} addresses: the kept keys are scanned, not probed",
            addrs.len()
        );
    }

    #[test]
    fn faces_are_the_profile_images_route_or_the_cdn_never_contents() {
        let node = FaceUrls::new(None, "https://node/profile-images/", false);
        assert_eq!(
            node.picture("bafyprofile", Some("bafyface")).as_deref(),
            Some("https://node/profile-images/entities/bafyprofile/face.png")
        );
        assert_eq!(node.picture("bafyprofile", None), None);
        assert_eq!(node.picture("bafyprofile", Some("")), None);
        let deriving = FaceUrls::new(None, "https://node/profile-images", true);
        assert_eq!(
            deriving.picture("bafyprofile", None).as_deref(),
            Some("https://node/profile-images/entities/bafyprofile/face.png")
        );
        let cdn = FaceUrls::new(Some("https://cdn/"), "https://node/profile-images/", false);
        assert_eq!(
            cdn.picture("bafyprofile", None).as_deref(),
            Some("https://cdn/entities/bafyprofile/face.png")
        );
    }

    #[tokio::test]
    async fn unresolved_members_are_omitted_while_posts_keep_the_author_fallback() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let profiles = ProfilesCache::new(
            pool,
            FaceUrls::new(None, "https://node/profile-images/", false),
        );
        let mut rows = vec![serde_json::json!({
            "communityId": "c1",
            "memberAddress": "0x00000000000000000000000000000000000000ab",
            "role": "member",
            "joinedAt": "2024-01-01T00:00:00",
        })];
        enrich_with_profiles(&profiles, &mut rows, "memberAddress")
            .await
            .unwrap();
        assert!(
            rows.is_empty(),
            "source V1 member mapping omits an unresolved profile"
        );

        let mut rows =
            vec![serde_json::json!({ "id": "p1", "authorAddress": "0xABC", "content": "hi" })];
        enrich_posts_with_authors(&profiles, &mut rows, "authorAddress")
            .await
            .unwrap();
        let m = rows[0].as_object().unwrap();
        for key in [
            "authorName",
            "authorProfilePictureUrl",
            "authorHasClaimedName",
        ] {
            assert!(m.contains_key(key), "post missing {key}");
        }
        assert_eq!(m["authorName"], "0xabc");
    }
    #[tokio::test]
    async fn registry_requested_key_case_matches_executed_source_adapter() {
        let Some(pool) = crate::db::scratch::pool(3).await else {
            return;
        };
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/source-contracts/profile-keys.json"
    )))
    .unwrap();
        let pointer = fixture["pointer"].as_str().unwrap();
        let entity = format!("bafkreiprofilecase{}", uuid::Uuid::new_v4().simple());
        sqlx::query("INSERT INTO content.deployments(deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES($1,'v3','profile',$2,$3,to_timestamp(1451606400),ARRAY[$1],now(),'[]'::json)")
            .bind(pointer).bind(&entity).bind(serde_json::json!({"v":fixture["document"]})).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile')")
            .bind(pointer).bind(&entity).execute(&pool).await.unwrap();
        let a = RegistryReader::new(Some(pool.clone()));
        let b = RegistryReader::new(Some(pool.clone()));
        let present = |profile: RegistryProfile| serde_json::json!({"name":profile.name,"userId":profile.address,"hasClaimedName":profile.has_claimed_name,"profilePictureUrl":profile.profile_picture_url});
        for (index, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
            let addresses = step["addresses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<Vec<_>>();
            let reader = if index % 2 == 0 { &a } else { &b };
            match step["operation"].as_str().unwrap() {
                "single" => match reader.single(&addresses[0]).await {
                    Ok(profile) => {
                        assert_eq!(serde_json::json!([present(profile)]), step["profiles"])
                    }
                    Err(error) => assert_eq!(error.to_string(), step["error"].as_str().unwrap()),
                },
                "batch" => assert_eq!(
                    serde_json::to_value(
                        reader
                            .batch(&addresses)
                            .await
                            .unwrap()
                            .into_iter()
                            .map(|(_, p)| present(p))
                            .collect::<Vec<_>>()
                    )
                    .unwrap(),
                    step["profiles"]
                ),
                "expire" => {
                    sqlx::query("DELETE FROM social.registry_profile_cache WHERE profile_key=$1")
                        .bind(&addresses[0])
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                other => panic!("unknown source operation {other}"),
            }
            let mut keys: Vec<String> = sqlx::query_scalar(
                "SELECT profile_key FROM social.registry_profile_cache WHERE lower(profile_key)=$1",
            )
            .bind(pointer)
            .fetch_all(&pool)
            .await
            .unwrap();
            // The capture sorts its cache-key inventory with JavaScript's
            // default sort, not the database's locale-dependent collation.
            // Only this ASCII key inventory is unordered; profiles above
            // still compare the source's returned order and duplicates.
            keys.sort_unstable();
            let source = step["cache_keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| {
                    key.as_str()
                        .unwrap()
                        .strip_prefix("catalyst:minimal:profile:")
                        .unwrap()
                        .to_string()
                })
                .collect::<Vec<_>>();
            assert_eq!(keys, source, "step {index}");
        }
        sqlx::query("DELETE FROM content.active_pointers WHERE entity_id=$1")
            .bind(&entity)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id=$1")
            .bind(&entity)
            .execute(&pool)
            .await
            .unwrap();
    }
}
