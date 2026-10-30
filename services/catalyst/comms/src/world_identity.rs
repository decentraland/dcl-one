//! The shared world identity vocabulary: the key types every worlds module,
//! the room resolver and `comms` bind to and decode from `worlds.*`.
//! Types only, no I/O.
//!
//! - [`WorldName`]: a world as a request, a deployment or a peer named it:
//!   the lowercase key every `worlds.*` predicate binds (`name = $key`) beside the
//!   display spelling the row stores as first seen.
//! - [`SceneId`]: a scene entity id (the content hash `world_scenes.entity_id`
//!   / `content.deployments.entity_id`), never a storage scope.
//! - [`RealmId`]: a world realm (`<name>.eth`, the comms contract) or a
//!   shared realm (Genesis City and any other non-world realm name).
//! - [`Parcel`] / [`ParcelSet`]: canonical `x,y` coordinates, the unit of
//!   parcel-scoped grants and of the base-parcel rule.
//! - [`SceneNamespace`]: the stable storage scope (a world scene's identity,
//!   the base parcel for the shared realm), distinct from any deployment
//!   hash, minted by the room resolver.
//! - [`PlayerAddress`]: `social::Address` (lowercase `0x` + 40 hex), which is
//!   how `player_storage.player_address` and every permission row store it.

use std::collections::BTreeSet;
use std::fmt;

use crate::auth::pointer::parse_pointer;
use crate::shared::identity::Address;

/// A player's wallet, exactly `social::Address`: the schema keys the two
/// identically and a permission answer never depends on which module parsed it.
pub type PlayerAddress = Address;

pub const DCL_ETH_SUFFIX: &str = ".dcl.eth";

/// The storage realm every shared (non-world) realm folds into: the old
/// world-storage `GENESIS_CITY_REALM`, so a scene at `10,20` keeps one
/// namespace whichever Genesis realm a player connected through.
pub const GENESIS_CITY_REALM: &str = "main";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorldNameError {
    #[error("world name is empty")]
    Empty,
    #[error("world name contains whitespace, a slash or a control character: {0:?}")]
    Malformed(String),
}

/// A world name. `key()` is the lowercase form every `worlds.*` row stores
/// and every statement binds (`worlds.name`, `world_permissions.world_name`,
/// the storage realm, the worlds service's room names); `display()` is the
/// spelling the caller used. World rooms use the key; wire display fields
/// can retain the caller's spelling.
///
/// Two names are equal when their keys are: `Foo.dcl.eth` and `foo.dcl.eth`
/// are one world (the schema refuses two rows differing only by case).
#[derive(Debug, Clone)]
pub struct WorldName {
    key: String,
    display: String,
}

impl WorldName {
    /// Trims; refuses empty, whitespace, `/` and control characters (a
    /// name reaches URL paths and LiveKit room names verbatim). Any other
    /// spelling is accepted: the old routes took any path segment.
    pub fn parse(raw: &str) -> Result<Self, WorldNameError> {
        let display = raw.trim();
        if display.is_empty() {
            return Err(WorldNameError::Empty);
        }
        if display
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c.is_control())
        {
            return Err(WorldNameError::Malformed(
                display.chars().take(64).collect(),
            ));
        }
        Ok(Self {
            key: display.to_lowercase(),
            display: display.to_string(),
        })
    }

    /// The personal world of `address`: `<lowercase address>.dcl.eth`.
    pub fn personal(address: &Address) -> Self {
        let display = format!("{}{DCL_ETH_SUFFIX}", address.as_str());
        Self {
            key: display.clone(),
            display,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    /// The NAME label the name authority is asked about: the key without
    /// `.dcl.eth` (a name without the suffix is its own label).
    pub fn label(&self) -> &str {
        self.key.strip_suffix(DCL_ETH_SUFFIX).unwrap_or(&self.key)
    }

    pub fn is_dcl_eth(&self) -> bool {
        self.key.ends_with(DCL_ETH_SUFFIX)
    }

    /// A DCL NAME's world or a personal world, in the form a worlds content
    /// server holds: one alphanumeric label under `.dcl.eth` (a NAME, or the
    /// address of a personal world). A bare address and ENS (`*.eth`) names
    /// are not.
    pub fn is_dcl_world_name(&self) -> bool {
        self.key.strip_suffix(DCL_ETH_SUFFIX).is_some_and(|label| {
            !label.is_empty()
                && label.len() <= 64
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
    }

    /// The world every signed-in account may publish to without owning a
    /// NAME: `<address>.dcl.eth`. NAME labels are 2-15 alphanumerics, so a
    /// 42-character label can never be minted on chain and the owner is the
    /// address itself. Accepts the bare label or the full `.dcl.eth` name.
    pub fn personal_owner(&self) -> Option<Address> {
        Address::parse(self.label())
            .ok()
            .filter(|_| self.label().len() == 42)
    }
}

impl PartialEq for WorldName {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for WorldName {}

impl std::hash::Hash for WorldName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl PartialOrd for WorldName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WorldName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl fmt::Display for WorldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display)
    }
}

impl std::str::FromStr for WorldName {
    type Err = WorldNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneIdError {
    #[error("scene id is empty")]
    Empty,
    #[error("scene id contains whitespace, a slash or a control character: {0:?}")]
    Malformed(String),
}

/// A scene entity id: the content hash of one deployment. It identifies a
/// version of a scene, so it names rooms and moderation rows (comms keys
/// `social.scene_admins`/`scene_bans` on it) but never a storage scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, sqlx::Type)]
#[sqlx(transparent)]
pub struct SceneId(String);

impl SceneId {
    pub fn parse(raw: &str) -> Result<Self, SceneIdError> {
        let id = raw.trim();
        if id.is_empty() {
            return Err(SceneIdError::Empty);
        }
        if id
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c.is_control())
        {
            return Err(SceneIdError::Malformed(id.chars().take(64).collect()));
        }
        Ok(Self(id.to_string()))
    }

    /// A value read back from `world_scenes.entity_id` /
    /// `content.deployments.entity_id`, which the write paths validated.
    /// Stored values only: anything that comes from a request, a peer or a
    /// scene goes through [`SceneId::parse`].
    pub fn from_stored(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    /// The old comms gatekeeper received the world name where a scene id was
    /// expected (`sceneId: "foo.dcl.eth"`) and resolved it to the world's
    /// active scene; the resolver keeps that reading.
    pub fn is_world_shaped(&self) -> bool {
        is_world_realm_name(&self.0)
    }
}

impl fmt::Display for SceneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The comms contract for "this realm is a world": the realm name ends with
/// `.eth`, case-insensitively. The old world-storage
/// service used `.dcl.eth`; the wider reading is kept so both agree on every
/// name the old services accepted.
pub fn is_world_realm_name(realm_name: &str) -> bool {
    realm_name
        .get(realm_name.len().saturating_sub(4)..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".eth"))
}

/// Where a request stands: a world realm or a shared one. `as_str()` is the
/// parsed display spelling (shared scene rooms preserve it);
/// `storage_realm()` is the storage scope: the world key, or
/// [`GENESIS_CITY_REALM`] for every shared realm.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RealmId {
    World(WorldName),
    Shared(String),
}

impl RealmId {
    pub fn parse(raw: &str) -> Result<Self, WorldNameError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(WorldNameError::Empty);
        }
        if is_world_realm_name(raw) {
            return Ok(Self::World(WorldName::parse(raw)?));
        }
        Ok(Self::Shared(raw.to_string()))
    }

    pub fn world(&self) -> Option<&WorldName> {
        match self {
            Self::World(w) => Some(w),
            Self::Shared(_) => None,
        }
    }

    pub fn is_world(&self) -> bool {
        matches!(self, Self::World(_))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::World(w) => w.display(),
            Self::Shared(s) => s,
        }
    }

    /// The storage realm scope, lowercase: `world_storage.realm` and the
    /// quota lock key.
    pub fn storage_realm(&self) -> &str {
        match self {
            Self::World(w) => w.key(),
            Self::Shared(_) => GENESIS_CITY_REALM,
        }
    }
}

impl fmt::Display for RealmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParcelError {
    #[error("parcel must be two comma-separated integer coordinates, e.g. \"10,-25\", got {0:?}")]
    Malformed(String),
    #[error("parcel must match ^-?\\d+,-?\\d+$, got {0:?}")]
    NotStrict(String),
}

/// One parcel; `to_string()` is the canonical `x,y` (no leading zeros, no
/// whitespace, no `-0`) that `world_permission_parcels.parcel` and
/// `world_scenes.parcels` store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Parcel {
    pub x: i32,
    pub y: i32,
}

impl Parcel {
    /// Accepts what the old `canonicalize_pointer` did (` 01 , -002 `) and
    /// refuses anything that does not parse into two i32 coordinates.
    pub fn parse(raw: &str) -> Result<Self, ParcelError> {
        let (x, y) = parse_pointer(raw)
            .ok_or_else(|| ParcelError::Malformed(raw.chars().take(64).collect()))?;
        match (i32::try_from(x), i32::try_from(y)) {
            (Ok(x), Ok(y)) => Ok(Self { x, y }),
            _ => Err(ParcelError::Malformed(raw.chars().take(64).collect())),
        }
    }

    /// [`Parcel::parse`] behind the request-schema pattern `^-?\d+,-?\d+$`
    /// (no whitespace, no `+`); out-of-range values stay `Malformed`.
    pub fn parse_strict(raw: &str) -> Result<Self, ParcelError> {
        let coordinate = |c: &str| {
            let digits = c.strip_prefix('-').unwrap_or(c);
            !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
        };
        let strict = raw
            .split_once(',')
            .is_some_and(|(x, y)| coordinate(x) && coordinate(y));
        if !strict {
            return Err(ParcelError::NotStrict(raw.chars().take(64).collect()));
        }
        Self::parse(raw)
    }

    pub fn canonical(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Parcel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

/// A set of parcels in canonical order: a scene's footprint, a grant's
/// scope, the parcels a deploy must be authorized over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParcelSet(BTreeSet<Parcel>);

impl ParcelSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every entry must parse; a scene's stored parcels and a grant's parcels
    /// were validated on their way in, so a failure here is a data fault.
    pub fn parse<S: AsRef<str>>(raw: &[S]) -> Result<Self, ParcelError> {
        raw.iter()
            .map(|s| Parcel::parse(s.as_ref()))
            .collect::<Result<BTreeSet<_>, _>>()
            .map(Self)
    }

    /// [`ParcelSet::parse`] with [`Parcel::parse_strict`] per entry.
    pub fn parse_strict<S: AsRef<str>>(raw: &[S]) -> Result<Self, ParcelError> {
        raw.iter()
            .map(|s| Parcel::parse_strict(s.as_ref()))
            .collect::<Result<BTreeSet<_>, _>>()
            .map(Self)
    }

    pub fn insert(&mut self, parcel: Parcel) -> bool {
        self.0.insert(parcel)
    }

    pub fn extend<I: IntoIterator<Item = Parcel>>(&mut self, parcels: I) {
        self.0.extend(parcels);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, parcel: &Parcel) -> bool {
        self.0.contains(parcel)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Parcel> {
        self.0.iter()
    }

    /// The canonical strings, in order: the `text[]` bind of every statement.
    pub fn canonical(&self) -> Vec<String> {
        self.0.iter().map(Parcel::canonical).collect()
    }

    pub fn overlaps(&self, other: &ParcelSet) -> bool {
        self.0.iter().any(|p| other.0.contains(p))
    }

    pub fn is_subset_of(&self, other: &ParcelSet) -> bool {
        self.0.is_subset(&other.0)
    }

    /// The canonical parcel used as a scene's downstream identity (room and
    /// moderation keys, the storage namespace). The declared
    /// `metadata.scene.base` is trusted only when it belongs to the scene's
    /// footprint; metadata naming a base outside it falls back to the first
    /// canonical parcel rather than resolving to some other scene's identity.
    /// `None` only for an empty footprint.
    pub fn effective_base(&self, declared: Option<&str>) -> Option<Parcel> {
        if let Some(base) = declared.map(str::trim).filter(|b| !b.is_empty())
            && let Ok(base) = Parcel::parse(base)
            && self.0.contains(&base)
        {
            return Some(base);
        }
        self.0.iter().next().copied()
    }
}

impl FromIterator<Parcel> for ParcelSet {
    fn from_iter<I: IntoIterator<Item = Parcel>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// The stable storage scope of a scene, what `world_storage`,
/// `player_storage` and `env_variables` key on, so a new scene version cannot
/// orphan player data:
///
/// - a world realm: the scene's active `explore.world_scene_place`, the
///   uuid the old world-storage keyed rows on as the Places id (one place
///   per world scene). It follows Places' overlap rule: a redeploy whose
///   footprint overlaps exactly one active scene keeps it, a moved base
///   included; one overlapping none or several starts a new one. The
///   scenes of a world never share one;
/// - the shared realm (Genesis City, [`GENESIS_CITY_REALM`]): the canonical
///   base parcel of the active deployment, the LAND place the old Places id
///   stood for. A redeploy keeping its base keeps the scope; one that moves
///   its base is a new place, as it was.
///
/// Minted by the room resolver from the active deployment (with the world
/// scene's identity), or explicitly by a caller that already resolved one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SceneNamespace {
    realm: String,
    scope: String,
}

impl SceneNamespace {
    /// `identity` is a world scene's active identity: `None` for a world
    /// scene without one leaves it no scope (storage refuses it); a shared
    /// realm's scope is `base`.
    pub fn new(realm: &RealmId, base: Parcel, identity: Option<uuid::Uuid>) -> Option<Self> {
        let scope = if realm.is_world() {
            identity?.to_string()
        } else {
            base.canonical()
        };
        Some(Self {
            realm: realm.storage_realm().to_string(),
            scope,
        })
    }

    /// `world_storage.realm`.
    pub fn realm(&self) -> &str {
        &self.realm
    }

    /// `world_storage.scene_namespace`.
    pub fn scene_namespace(&self) -> String {
        self.scope.clone()
    }

    /// Whether the realm scope is a shared realm, where quotas are per scene
    /// namespace (a world's quotas span all its scenes).
    pub fn is_shared_realm(&self) -> bool {
        !self.realm.ends_with(DCL_ETH_SUFFIX) && !self.realm.ends_with(".eth")
    }
}

impl fmt::Display for SceneNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.realm, self.scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDR: &str = "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2";

    #[test]
    fn a_world_name_keys_on_its_lowercase_and_keeps_its_display() {
        let a = WorldName::parse(" Foo.DCL.eth ").unwrap();
        assert_eq!(a.key(), "foo.dcl.eth");
        assert_eq!(a.display(), "Foo.DCL.eth");
        assert_eq!(a.label(), "foo");
        assert!(a.is_dcl_eth());
        assert_eq!(a, WorldName::parse("foo.dcl.eth").unwrap());
        assert_eq!(a.to_string(), "Foo.DCL.eth");
        let b = WorldName::parse("my-world").unwrap();
        assert_eq!(b.label(), "my-world");
        assert!(!b.is_dcl_eth());
        for bad in ["", "   ", "foo bar", "a/b", "x\ny"] {
            assert!(WorldName::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_dcl_name_and_personal_worlds_are_dcl_world_names() {
        for good in [
            "rt.dcl.eth",
            "RT.dcl.ETH",
            "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2.dcl.eth",
        ] {
            assert!(
                WorldName::parse(good).unwrap().is_dcl_world_name(),
                "{good}"
            );
        }
        for bad in [
            "rt.dcl.eth\\about?0",
            "rt.dcl.eth?x",
            "rt.dcl.eth#x",
            "a.b.dcl.eth",
            "my-world.dcl.eth",
            "..",
            "rt..eth",
            ".eth",
            "eth",
            "my-world",
            "a%2fb.eth",
            "café.dcl.eth",
            "rtens.eth",
            "my-name.eth",
            "sub.my-name.eth",
            "dcl.eth",
            ".dcl.eth",
            "0x4d02.dcl.eth.eth",
            "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2",
        ] {
            assert!(!WorldName::parse(bad).unwrap().is_dcl_world_name(), "{bad}");
        }
    }

    #[test]
    fn the_personal_world_is_owned_by_its_address() {
        let owner = Address::parse(ADDR).unwrap();
        assert_eq!(
            WorldName::parse(&format!("{ADDR}.dcl.eth"))
                .unwrap()
                .personal_owner(),
            Some(owner.clone())
        );
        assert_eq!(
            WorldName::parse(ADDR).unwrap().personal_owner(),
            Some(owner.clone())
        );
        assert_eq!(
            WorldName::parse(&format!("{}.DCL.ETH", ADDR.to_ascii_uppercase()))
                .unwrap()
                .personal_owner(),
            Some(owner.clone())
        );
        let name = WorldName::personal(&owner);
        assert_eq!(name.key(), format!("{ADDR}.dcl.eth"));
        assert_eq!(name.personal_owner(), Some(owner));
    }

    #[test]
    fn a_name_label_is_never_a_personal_world() {
        for name in [
            "boedo.dcl.eth",
            "0x4d02.dcl.eth",
            "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf.dcl.eth",
            "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2z.dcl.eth",
            "0xzz02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2.dcl.eth",
        ] {
            assert_eq!(
                WorldName::parse(name).unwrap().personal_owner(),
                None,
                "{name:?}"
            );
        }
    }

    #[test]
    fn a_scene_id_is_any_non_blank_token() {
        let id = SceneId::parse(" bafkreiabc ").unwrap();
        assert_eq!(id.as_str(), "bafkreiabc");
        assert!(!id.is_world_shaped());
        assert!(SceneId::parse("foo.dcl.eth").unwrap().is_world_shaped());
        assert!(SceneId::parse("").is_err());
        assert!(SceneId::parse("a b").is_err());
    }

    #[test]
    fn realms_fold_shared_names_into_genesis_and_keep_worlds_by_key() {
        let w = RealmId::parse("Foo.dcl.eth").unwrap();
        assert!(w.is_world());
        assert_eq!(w.as_str(), "Foo.dcl.eth");
        assert_eq!(w.storage_realm(), "foo.dcl.eth");
        assert_eq!(w.world().unwrap().label(), "foo");
        for shared in ["main", "artemis", "localpreview", "PREVIEW"] {
            let r = RealmId::parse(shared).unwrap();
            assert!(!r.is_world());
            assert_eq!(r.as_str(), shared);
            assert_eq!(r.storage_realm(), GENESIS_CITY_REALM);
        }
        assert!(is_world_realm_name("bar.eth") && !is_world_realm_name("main"));
        assert!(RealmId::parse("  ").is_err());
    }

    #[test]
    fn parcels_canonicalize_and_refuse_garbage() {
        for (raw, canon) in [
            ("0,0", "0,0"),
            (" 01 , -002 ", "1,-2"),
            ("-0,-0", "0,0"),
            ("-150,150", "-150,150"),
        ] {
            assert_eq!(Parcel::parse(raw).unwrap().canonical(), canon, "{raw:?}");
        }
        for bad in [
            "10",
            "10,",
            ",25",
            "10,25,3",
            "a,b",
            "10.5,2",
            "--1,2",
            "-,2",
            "1,2/../x",
            "",
            "99999999999,0",
        ] {
            assert!(Parcel::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn strict_parcels_follow_the_request_pattern() {
        for (raw, canon) in [("0,0", "0,0"), ("-1,-20", "-1,-20"), ("00,01", "0,1")] {
            assert_eq!(Parcel::parse_strict(raw).unwrap().canonical(), canon);
        }
        for bad in ["0, 0", "+1,0", "1,2,3", "1,", "0,0\n", "\u{0661},0", "a,b"] {
            assert!(
                matches!(Parcel::parse_strict(bad), Err(ParcelError::NotStrict(_))),
                "{bad:?}"
            );
        }
        assert!(matches!(
            Parcel::parse_strict("99999999999,0"),
            Err(ParcelError::Malformed(_))
        ));
        assert!(ParcelSet::parse_strict(&["0,0", " 1,1"]).is_err());
    }

    #[test]
    fn parcel_sets_compare_by_value_in_canonical_order() {
        let a = ParcelSet::parse(&["1,1", "0,0", "01,001"]).unwrap();
        assert_eq!(a.canonical(), vec!["0,0", "1,1"]);
        let b = ParcelSet::parse(&["1,1", "2,2"]).unwrap();
        assert!(a.overlaps(&b));
        assert!(!a.is_subset_of(&b));
        assert!(ParcelSet::parse(&["1,1"]).unwrap().is_subset_of(&b));
        assert!(ParcelSet::parse(&["x"]).is_err());
        assert!(ParcelSet::new().is_empty());
    }

    #[test]
    fn effective_base_trusts_declared_base_only_when_a_member() {
        let parcels = ParcelSet::parse(&["0,0", "0,1"]).unwrap();
        assert_eq!(
            parcels.effective_base(Some("0,1")).unwrap().canonical(),
            "0,1"
        );
        assert_eq!(
            parcels
                .effective_base(Some(" 00 , 01 "))
                .unwrap()
                .canonical(),
            "0,1"
        );
        assert_eq!(
            parcels.effective_base(Some("9,9")).unwrap().canonical(),
            "0,0"
        );
        assert_eq!(parcels.effective_base(None).unwrap().canonical(), "0,0");
        assert_eq!(parcels.effective_base(Some("")).unwrap().canonical(), "0,0");
        assert_eq!(ParcelSet::new().effective_base(Some("0,0")), None);
    }

    #[test]
    fn a_scene_namespace_is_the_world_scene_identity_or_the_genesis_base_parcel() {
        let realm = RealmId::parse("Foo.dcl.eth").unwrap();
        let id = uuid::Uuid::from_u128(7);
        let world = SceneNamespace::new(&realm, Parcel { x: 0, y: 0 }, Some(id)).unwrap();
        assert_eq!(
            (world.realm(), world.scene_namespace()),
            ("foo.dcl.eth", id.to_string())
        );
        assert_eq!(
            Some(world.clone()),
            SceneNamespace::new(&realm, Parcel { x: 5, y: 5 }, Some(id)),
            "the identity, not the base, is the scope"
        );
        assert_ne!(
            Some(world.clone()),
            SceneNamespace::new(
                &realm,
                Parcel { x: 0, y: 0 },
                Some(uuid::Uuid::from_u128(8))
            ),
            "another scene of the world is another scope"
        );
        assert_eq!(
            SceneNamespace::new(&realm, Parcel { x: 0, y: 0 }, None),
            None,
            "a world scene without an identity has no scope"
        );
        assert_eq!(
            world.to_string(),
            "foo.dcl.eth:00000000-0000-0000-0000-000000000007"
        );
        assert!(!world.is_shared_realm());
        let genesis = SceneNamespace::new(
            &RealmId::parse("artemis").unwrap(),
            Parcel { x: 10, y: -25 },
            Some(id),
        )
        .unwrap();
        assert_eq!(
            (genesis.realm(), genesis.scene_namespace().as_str()),
            ("main", "10,-25")
        );
        assert!(genesis.is_shared_realm());
        assert_eq!(genesis.to_string(), "main:10,-25");
    }
}
