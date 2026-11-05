//! The access vocabulary of a world join: what `worlds::policy::Policy`
//! returns ([`Access`] with its [`AccessGrant`] and [`Ban`]) and the settings and
//! documents it decides over ([`AccessType`],
//! [`SceneRef`], [`LandOperators`]). Types only, no I/O: the decisions stay in
//! worlds, the vocabulary lives here so `comms` can name them without
//! depending on it.

use crate::shared::identity::Address;
use crate::world_identity::{ParcelSet, SceneId, WorldName};

/// `worlds.worlds.access_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    Unrestricted,
    SharedSecret,
    NftOwnership,
    AllowList,
}

impl AccessType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unrestricted => "unrestricted",
            Self::SharedSecret => "shared-secret",
            Self::NftOwnership => "nft-ownership",
            Self::AllowList => "allow-list",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "unrestricted" => Some(Self::Unrestricted),
            "shared-secret" => Some(Self::SharedSecret),
            "nft-ownership" => Some(Self::NftOwnership),
            "allow-list" => Some(Self::AllowList),
            _ => None,
        }
    }
}

/// How long a quota-blocked owner's worlds stay open (WCS `TWO_DAYS_IN_MS`).
pub const QUOTA_BLOCK_GRACE: chrono::TimeDelta = chrono::TimeDelta::days(2);

/// The one "world refused" predicate `/about` and the comms mints share: an
/// operator disable (`worlds.blocked_since`) applies at once, the owner's
/// quota block (`worlds.quota_blocks.created_at`) after its grace.
pub fn world_blocked(
    disabled_since: Option<chrono::NaiveDateTime>,
    quota_blocked_since: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
) -> bool {
    disabled_since.is_some()
        || quota_blocked_since.is_some_and(|since| now - since > QUOTA_BLOCK_GRACE)
}

/// Whether a world can be joined at all, before any actor is considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldAdmission {
    /// No `worlds.worlds` row.
    Unknown,
    /// Disabled, or its owner's quota block is past [`QUOTA_BLOCK_GRACE`].
    Blocked,
    Open(AccessType),
}

/// Why a world join is refused before any access setting is consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ban {
    /// `worlds.blocked_wallets` (the operator's block list).
    Blocked,
    /// An in-force `social.user_bans` row (address or recorded device).
    Platform,
    /// A `social.scene_bans` row for the resolved scene.
    Scene,
}

/// How a world admitted the actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessGrant {
    Owner,
    Unrestricted,
    AllowListed,
    /// A member of a community on the world's allow-list.
    Community,
    SharedSecret,
}

/// The answer of `worlds::policy::Policy::can_access`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    Allowed(AccessGrant),
    Banned(Ban),
    /// A shared-secret world and the presented secret did not verify (or none
    /// was presented); the route rate-limits and answers 403.
    InvalidSecret,
    /// Not the owner and not admitted by the access setting (allow-list miss,
    /// nft-ownership); the route answers 401.
    NotAllowed,
}

/// Which scene a moderation question is about.
#[derive(Debug, Clone)]
pub enum SceneRef<'a> {
    World {
        world: &'a WorldName,
        scene: &'a SceneId,
    },
    Genesis {
        scene: &'a SceneId,
        parcels: &'a ParcelSet,
    },
}

impl SceneRef<'_> {
    pub fn scene_id(&self) -> &SceneId {
        match self {
            Self::World { scene, .. } | Self::Genesis { scene, .. } => scene,
        }
    }
}

/// The LAND manager document of a Genesis parcel (`GET <base>/parcels/{x}/
/// {y}/operators`, the lambdas): every wallet that may manage the parcel.
/// It answers the old world-storage permission matrix
/// (`LandsParcelPermissions::has_any()`) and comms' protected set for a
/// Genesis place.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub struct LandOperators {
    pub owner: String,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default, rename = "updateOperator")]
    pub update_operator: Option<String>,
    #[serde(default, rename = "updateManagers")]
    pub update_managers: Vec<String>,
    #[serde(default, rename = "approvedForAll")]
    pub approved_for_all: Vec<String>,
}

impl LandOperators {
    /// Every address of the document, lowercase.
    pub fn addresses(&self) -> impl Iterator<Item = String> + '_ {
        std::iter::once(&self.owner)
            .chain(self.operator.iter())
            .chain(self.update_operator.iter())
            .chain(self.update_managers.iter())
            .chain(self.approved_for_all.iter())
            .map(|a| a.to_lowercase())
    }

    /// The old `has_any()`: owner, operator, updateOperator, updateManager
    /// or approvedForAll.
    pub fn manages(&self, actor: &Address) -> bool {
        self.addresses().any(|a| a == actor.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_operator_disable_applies_at_once_and_a_quota_block_after_two_days() {
        let now = chrono::DateTime::from_timestamp(1_800_000_000, 0)
            .unwrap()
            .naive_utc();
        let ago = |s: i64| Some(now - chrono::TimeDelta::seconds(s));
        assert!(!world_blocked(None, None, now));
        assert!(world_blocked(Some(now), None, now));
        assert!(!world_blocked(None, Some(now), now));
        assert!(!world_blocked(None, ago(2 * 86_400), now));
        assert!(world_blocked(None, ago(2 * 86_400 + 1), now));
    }
}
