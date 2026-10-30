//! A wallet's standing in one community, read from `social.community_members`
//! with the soft-delete guard (a member row of an inactive community grants
//! nothing, upstream #460), and the moderation authority over one member
//! derived from two standings. A query failure is "we could not tell"
//! ([`AuthorityNotEstablished::UndeterminedStoreUnavailable`]), never "no"
//! (the BC-1 fix: the superseded loader read a SQL fault as "not a member",
//! fail-open at the *target* of a moderation action).
//!
//! The federation path's twin table (`community_role_current`) is not
//! recreated in v2: `community_members` is the one projection.

use sqlx::PgPool;

use crate::identity::can_act_on_member;
use crate::{
    Address, AuthorityNotEstablished, CommunityId, CommunityPermission, CommunityStanding,
};

const STORE: &str = "social.community_members";

#[derive(Debug, Clone)]
pub struct Standing {
    community_id: CommunityId,
    address: Address,
    tier: CommunityStanding,
    /// `None` when no row existed, distinct from a row holding something
    /// unrecognised.
    stored_role: Option<String>,
}

impl Standing {
    fn from_row(community_id: CommunityId, address: Address, stored_role: Option<String>) -> Self {
        let tier = CommunityStanding::from_rows(false, stored_role.as_deref());
        if let Some(raw) = stored_role.as_deref()
            && CommunityStanding::parse_supplied(raw).is_none()
        {
            tracing::warn!(
                table = STORE,
                community = %community_id,
                wallet = %address,
                stored_role_text = %raw,
                "community membership row holds a role this service does not recognise; treating it as not a member"
            );
        }
        Self {
            community_id,
            address,
            tier,
            stored_role,
        }
    }

    pub fn tier(&self) -> CommunityStanding {
        self.tier
    }

    pub fn community_id(&self) -> CommunityId {
        self.community_id
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Deliberately does **not** consult the tier: a row holding an
    /// unrecognised string passes.
    pub fn row_exists(&self) -> bool {
        self.stored_role.is_some()
    }

    pub fn stored_role_or_none(&self) -> &str {
        self.stored_role.as_deref().unwrap_or("none")
    }

    pub fn is_at_least(&self, minimum: CommunityStanding) -> bool {
        self.tier >= minimum
    }

    pub fn has(&self, permission: CommunityPermission) -> bool {
        self.tier.has(permission)
    }

    pub fn is_member(&self) -> bool {
        self.tier.is_member()
    }

    #[cfg(test)]
    pub(crate) fn for_tests(tier_text: &str, row_exists: bool) -> Self {
        Self::from_row(
            CommunityId::new(),
            Address::parse("0x00000000000000000000000000000000000000aa").unwrap(),
            row_exists.then(|| tier_text.to_string()),
        )
    }
}

/// Reads `community_members` for one wallet. A missing row means not a
/// member; a query failure propagates as undetermined.
pub async fn load_standing(
    pool: &PgPool,
    community_id: CommunityId,
    address: &Address,
) -> Result<Standing, AuthorityNotEstablished> {
    let row: Option<String> = sqlx::query_scalar(
        "SELECT role FROM social.community_members \
         WHERE community_id = $1 AND member_address = $2 \
           AND EXISTS (SELECT 1 FROM social.communities c WHERE c.id = $1 AND c.active = true)",
    )
    .bind(community_id)
    .bind(address)
    .fetch_optional(pool)
    .await
    .map_err(|e| AuthorityNotEstablished::store_unavailable(STORE, &e))?;
    Ok(Standing::from_row(community_id, address.clone(), row))
}

/// Both wallets' standings from one read; a missing row is "not a member",
/// exactly as [`load_standing`] reports it.
pub async fn load_standing_pair(
    pool: &PgPool,
    community_id: CommunityId,
    first: &Address,
    second: &Address,
) -> Result<(Standing, Standing), AuthorityNotEstablished> {
    let rows: Vec<(Address, String)> = sqlx::query_as(
        "SELECT member_address, role FROM social.community_members \
         WHERE community_id = $1 AND member_address IN ($2, $3) \
           AND EXISTS (SELECT 1 FROM social.communities c WHERE c.id = $1 AND c.active = true)",
    )
    .bind(community_id)
    .bind(first)
    .bind(second)
    .fetch_all(pool)
    .await
    .map_err(|e| AuthorityNotEstablished::store_unavailable(STORE, &e))?;
    let role_of = |wallet: &Address| {
        rows.iter()
            .find(|(a, _)| a == wallet)
            .map(|(_, r)| r.clone())
    };
    Ok((
        Standing::from_row(community_id, first.clone(), role_of(first)),
        Standing::from_row(community_id, second.clone(), role_of(second)),
    ))
}

/// THE predicate behind both moderation authorities: the acting party holds
/// `BanPlayers`, and either outranks the target or the target is not a
/// member at all.
fn decide_whether_the_acting_party_outranks_the_target(
    actor: &Standing,
    target: &Standing,
    what: &'static str,
) -> Result<(), AuthorityNotEstablished> {
    if !actor.has(CommunityPermission::BanPlayers) {
        return Err(AuthorityNotEstablished::RefusedLacksAuthority {
            detail: format!(
                "{} holds the {} tier in community {}, which cannot {} in it",
                actor.address(),
                actor.tier().as_wire_text(),
                actor.community_id(),
                what,
            ),
        });
    }
    if !can_act_on_member(actor.tier(), target.tier()) && target.is_member() {
        return Err(AuthorityNotEstablished::RefusedLacksAuthority {
            detail: format!(
                "{} may not {} {}, who holds the {} tier in community {}",
                actor.address(),
                what,
                target.address(),
                target.tier().as_wire_text(),
                actor.community_id(),
            ),
        });
    }
    Ok(())
}

/// A value exists only downstream of the predicate above; holding one is
/// the proof, the write that follows does not re-derive anything.
#[derive(Debug)]
pub struct ModerationAuthority {
    actor: Standing,
    target: Standing,
}

impl ModerationAuthority {
    async fn resolve(
        pool: &PgPool,
        actor: &Address,
        community_id: CommunityId,
        target: &Address,
        what: &'static str,
    ) -> Result<Self, AuthorityNotEstablished> {
        let (actor, target) = load_standing_pair(pool, community_id, actor, target).await?;
        decide_whether_the_acting_party_outranks_the_target(&actor, &target, what)?;
        Ok(Self { actor, target })
    }

    pub fn actor(&self) -> &Standing {
        &self.actor
    }

    pub fn target(&self) -> &Standing {
        &self.target
    }
}

/// The authority to ban one wallet in one community, proven for the
/// end user's ADR-44 signed fetch.
#[derive(Debug)]
pub struct CommunityBanAuthority(ModerationAuthority);

impl CommunityBanAuthority {
    pub async fn resolve(
        pool: &PgPool,
        banning: &Address,
        community_id: CommunityId,
        target: &Address,
    ) -> Result<Self, AuthorityNotEstablished> {
        ModerationAuthority::resolve(pool, banning, community_id, target, "ban")
            .await
            .map(Self)
    }

    pub fn inner(&self) -> &ModerationAuthority {
        &self.0
    }
}

/// A separate type so a witness of one cannot be used as a witness of the
/// other; the predicate is shared on purpose, the names are not.
#[derive(Debug)]
pub struct CommunityUnbanAuthority(ModerationAuthority);

impl CommunityUnbanAuthority {
    pub async fn resolve(
        pool: &PgPool,
        unbanning: &Address,
        community_id: CommunityId,
        target: &Address,
    ) -> Result<Self, AuthorityNotEstablished> {
        ModerationAuthority::resolve(pool, unbanning, community_id, target, "unban")
            .await
            .map(Self)
    }

    pub fn inner(&self) -> &ModerationAuthority {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommunityStanding as Tier;

    const EVERY_TIER_TEXT: &[&str] = &["banned", "none", "member", "mod", "owner"];

    fn decide(actor_text: &str, target_text: &str) -> bool {
        let actor = Standing::for_tests(actor_text, true);
        let target = Standing::for_tests(target_text, true);
        decide_whether_the_acting_party_outranks_the_target(&actor, &target, "ban").is_ok()
    }

    fn tier(text: &str) -> Tier {
        Tier::from_rows(false, Some(text))
    }

    #[test]
    fn the_shared_predicate_matches_the_client_paths_former_inline_predicate() {
        for actor_text in EVERY_TIER_TEXT {
            for target_text in EVERY_TIER_TEXT {
                let (actor, target) = (tier(actor_text), tier(target_text));
                let former = actor.has(CommunityPermission::BanPlayers)
                    && !(!can_act_on_member(actor, target) && target.is_member());
                assert_eq!(
                    decide(actor_text, target_text),
                    former,
                    "actor {actor_text} target {target_text}"
                );
            }
        }
    }

    #[test]
    fn the_shared_predicate_matches_the_federation_paths_former_inline_predicate() {
        for actor_text in EVERY_TIER_TEXT {
            for target_text in EVERY_TIER_TEXT {
                let (actor, target) = (tier(actor_text), tier(target_text));
                let former = actor != Tier::Banned && actor >= Tier::Moderator && target < actor;
                assert_eq!(
                    decide(actor_text, target_text),
                    former,
                    "actor {actor_text} target {target_text}"
                );
            }
        }
    }

    #[test]
    fn a_missing_row_is_distinguishable_from_a_row_holding_an_unrecognised_value() {
        let no_row = Standing::for_tests("", false);
        let junk_row = Standing::for_tests("wat", true);
        assert!(!no_row.row_exists());
        assert!(junk_row.row_exists());
        assert_eq!(no_row.tier(), junk_row.tier());
        assert_eq!(no_row.stored_role_or_none(), "none");
        assert_eq!(junk_row.stored_role_or_none(), "wat");
        assert_eq!(Standing::for_tests("mod", true).tier(), Tier::Moderator);
        assert_eq!(Standing::for_tests("admin", true).tier(), Tier::NotAMember);
    }

    #[test]
    fn a_backing_store_failure_is_never_a_refusal() {
        let actor = Standing::for_tests("owner", true);
        let target = Standing::for_tests("owner", true);
        let refusal = decide_whether_the_acting_party_outranks_the_target(&actor, &target, "ban")
            .unwrap_err();
        assert_eq!(refusal.http_status(), 403);
        let undetermined =
            AuthorityNotEstablished::store_unavailable(STORE, &sqlx::Error::PoolTimedOut);
        assert_eq!(undetermined.http_status(), 503);
        assert!(matches!(
            undetermined,
            AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store: "social.community_members",
                ..
            }
        ));
    }

    /// BC-1 regression. Needs no database: an unreachable pool is the fault.
    #[tokio::test]
    async fn a_query_fault_is_still_undetermined_and_never_a_silent_not_a_member() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(250))
            .connect_lazy("postgres://postgres@127.0.0.1:1/unreachable")
            .expect("lazy pool");
        let owner = Address::parse("0x00000000000000000000000000000000d31e7ed0").unwrap();
        let refusal = load_standing(&pool, CommunityId::new(), &owner)
            .await
            .expect_err("an unreachable store must not resolve to a standing");
        match refusal {
            AuthorityNotEstablished::UndeterminedStoreUnavailable { store, .. } => {
                assert_eq!(store, STORE)
            }
            other => panic!("a store fault must render as Undetermined, got {other:?}"),
        }
    }
}
