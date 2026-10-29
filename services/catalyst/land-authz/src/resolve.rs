//! Reads over the folded index. The old store joined `squid_marketplace.parcel`
//! and `.estate` for coordinates, ownership and estate membership; here the
//! coordinates are `token_right.x/y` (the token id decoded by the fold), the
//! owner is `token_right.owner` (the latest `Transfer`) and membership is
//! `estate_land`. The reading of an estate parcel is the land-manager
//! subgraph's: the estate's owner and operator, but a per-parcel update
//! operator still wins over the estate's.

use sqlx::PgPool;

use super::{
    KIND_APPROVED_FOR_ALL, KIND_UPDATE_MANAGER, LandOperators, ParcelSubject, UpdatableParcel,
};

/// What one parcel resolves to: its subject and the four operator legs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParcelRights {
    pub subject: ParcelSubject,
    pub operators: LandOperators,
}

/// The five legs a LAND right can come from; every one grants a deploy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "validator/")
)]
pub enum LandRightLeg {
    Owner,
    Operator,
    UpdateOperator,
    UpdateManager,
    ApprovedForAll,
}

pub const DEPLOY_GRANTING_LEGS: [LandRightLeg; 5] = [
    LandRightLeg::Owner,
    LandRightLeg::Operator,
    LandRightLeg::UpdateOperator,
    LandRightLeg::UpdateManager,
    LandRightLeg::ApprovedForAll,
];

/// The wire shape of `/lambdas/users/{address}/parcels/{x}/{y}/permissions`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "validator/")
)]
pub struct ParcelPermissionFlags {
    pub owner: bool,
    pub operator: bool,
    pub update_operator: bool,
    pub update_manager: bool,
    pub approved_for_all: bool,
}

impl ParcelPermissionFlags {
    pub fn leg(&self, leg: LandRightLeg) -> bool {
        match leg {
            LandRightLeg::Owner => self.owner,
            LandRightLeg::Operator => self.operator,
            LandRightLeg::UpdateOperator => self.update_operator,
            LandRightLeg::UpdateManager => self.update_manager,
            LandRightLeg::ApprovedForAll => self.approved_for_all,
        }
    }

    /// The first granting leg in [`DEPLOY_GRANTING_LEGS`] order, if any.
    pub fn granting_leg(&self) -> Option<LandRightLeg> {
        DEPLOY_GRANTING_LEGS
            .iter()
            .copied()
            .find(|&leg| self.leg(leg))
    }

    pub fn grants_deploy(&self) -> bool {
        self.granting_leg().is_some()
    }
}

fn addresses_match(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn address_in_list(address: &str, list: &[String]) -> bool {
    list.iter().any(|a| addresses_match(address, a))
}

/// The four operator legs of `address` over `operators`; `owner` stays false
/// (the caller settles it from the subject).
pub fn operator_flags(address: &str, operators: &LandOperators) -> ParcelPermissionFlags {
    let matches = |o: &Option<String>| o.as_deref().is_some_and(|v| addresses_match(address, v));
    ParcelPermissionFlags {
        owner: false,
        operator: matches(&operators.operator),
        update_operator: matches(&operators.update_operator),
        update_manager: address_in_list(address, &operators.update_managers),
        approved_for_all: address_in_list(address, &operators.approved_for_all),
    }
}

impl ParcelRights {
    /// Every leg of `address` over this parcel: the owner leg from the
    /// subject (the estate's owner when the parcel sits in one), the rest
    /// from the operators.
    pub fn flags_for(&self, address: &str) -> ParcelPermissionFlags {
        let mut flags = operator_flags(address, &self.operators);
        flags.owner =
            !self.subject.owner.is_empty() && addresses_match(address, &self.subject.owner);
        flags
    }
}

type ParcelSubjectRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
);

type ParcelRightsRow = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
    Vec<String>,
    Vec<String>,
);

/// The reads over `land_authz.*` for one network's two registries.
#[derive(Clone)]
pub struct Store {
    pool: PgPool,
    land_registry: String,
    estate_registry: String,
}

/// The parcel's subject from one row of the parcel/estate join.
fn subject_of(
    land_registry: &str,
    estate_registry: &str,
    (parcel_owner, estate_owner, operator, update_operator, belongs_to_estate): ParcelSubjectRow,
) -> ParcelSubject {
    let owner = if belongs_to_estate {
        estate_owner.or(parcel_owner)
    } else {
        parcel_owner
    };
    ParcelSubject {
        owner: owner.unwrap_or_default().to_lowercase(),
        registry: if belongs_to_estate {
            estate_registry.to_string()
        } else {
            land_registry.to_string()
        },
        operator: operator.map(|o| o.to_lowercase()),
        update_operator: update_operator.map(|o| o.to_lowercase()),
        belongs_to_estate,
    }
}

/// The subject columns shared by `parcel_subject` and `rights_batch`: `pt`
/// is the parcel's `token_right`, `el` its `estate_land`, `et` the estate's
/// `token_right`.
const SUBJECT_COLUMNS: &str = "pt.owner AS parcel_owner, \
     et.owner AS estate_owner, \
     CASE WHEN el.estate_token_id IS NULL THEN pt.operator ELSE et.operator END AS operator, \
     COALESCE(pt.update_operator, CASE WHEN el.estate_token_id IS NULL THEN NULL ELSE et.update_operator END) AS update_operator, \
     (el.estate_token_id IS NOT NULL) AS belongs_to_estate";

impl Store {
    pub fn new(pool: PgPool, land_registry: &str, estate_registry: &str) -> Self {
        Self {
            pool,
            land_registry: land_registry.to_lowercase(),
            estate_registry: estate_registry.to_lowercase(),
        }
    }

    pub fn land_registry(&self) -> &str {
        &self.land_registry
    }

    pub fn estate_registry(&self) -> &str {
        &self.estate_registry
    }

    /// `None` means no such parcel is indexed (never minted, or the index
    /// has not seen its mint).
    pub async fn parcel_subject(
        &self,
        x: i32,
        y: i32,
    ) -> Result<Option<ParcelSubject>, sqlx::Error> {
        let row: Option<ParcelSubjectRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {SUBJECT_COLUMNS} \
             FROM land_authz.token_right pt \
             LEFT JOIN land_authz.estate_land el ON el.land_token_id = pt.token_id \
             LEFT JOIN land_authz.token_right et ON et.token_address = $4 AND et.token_id = el.estate_token_id \
             WHERE pt.token_address = $3 AND pt.x = $1 AND pt.y = $2"
        )))
        .bind(x)
        .bind(y)
        .bind(&self.land_registry)
        .bind(&self.estate_registry)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| subject_of(&self.land_registry, &self.estate_registry, row)))
    }

    pub async fn account_grants(
        &self,
        registry: &str,
        owner: &str,
        kind: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT operator FROM land_authz.account_right \
             WHERE token_address = $1 AND account = lower($2) AND kind = $3 AND is_approved \
             ORDER BY operator",
        )
        .bind(registry)
        .bind(owner)
        .bind(kind)
        .fetch_all(&self.pool)
        .await
    }

    /// Subject plus both grant legs per parcel in one round trip,
    /// positional; `None` = not indexed.
    pub async fn rights_batch(
        &self,
        parcels: &[(i32, i32)],
    ) -> Result<Vec<Option<ParcelRights>>, sqlx::Error> {
        if parcels.is_empty() {
            return Ok(Vec::new());
        }
        let xs: Vec<i32> = parcels.iter().map(|p| p.0).collect();
        let ys: Vec<i32> = parcels.iter().map(|p| p.1).collect();
        let rows: Vec<ParcelRightsRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT t.i, {SUBJECT_COLUMNS}, um.operators AS update_managers, af.operators AS approved_for_all \
             FROM unnest($1::int[], $2::int[]) WITH ORDINALITY AS t(x, y, i) \
             JOIN land_authz.token_right pt ON pt.token_address = $3 AND pt.x = t.x AND pt.y = t.y \
             LEFT JOIN land_authz.estate_land el ON el.land_token_id = pt.token_id \
             LEFT JOIN land_authz.token_right et ON et.token_address = $4 AND et.token_id = el.estate_token_id \
             CROSS JOIN LATERAL ( \
                 SELECT CASE WHEN el.estate_token_id IS NULL THEN $3::text ELSE $4::text END AS registry, \
                        lower(COALESCE(CASE WHEN el.estate_token_id IS NOT NULL THEN et.owner END, pt.owner, '')) AS account \
             ) s \
             LEFT JOIN LATERAL ( \
                 SELECT COALESCE(array_agg(ar.operator ORDER BY ar.operator), '{{}}'::text[]) AS operators \
                 FROM land_authz.account_right ar \
                 WHERE ar.token_address = s.registry AND ar.account = s.account AND ar.kind = $5 AND ar.is_approved \
             ) um ON true \
             LEFT JOIN LATERAL ( \
                 SELECT COALESCE(array_agg(ar.operator ORDER BY ar.operator), '{{}}'::text[]) AS operators \
                 FROM land_authz.account_right ar \
                 WHERE ar.token_address = s.registry AND ar.account = s.account AND ar.kind = $6 AND ar.is_approved \
             ) af ON true"
        )))
        .bind(&xs)
        .bind(&ys)
        .bind(&self.land_registry)
        .bind(&self.estate_registry)
        .bind(KIND_UPDATE_MANAGER)
        .bind(KIND_APPROVED_FOR_ALL)
        .fetch_all(&self.pool)
        .await?;

        let mut out: Vec<Option<ParcelRights>> = vec![None; parcels.len()];
        for (
            i,
            parcel_owner,
            estate_owner,
            operator,
            update_operator,
            belongs_to_estate,
            update_managers,
            approved_for_all,
        ) in rows
        {
            let Some(slot) = usize::try_from(i - 1).ok().and_then(|i| out.get_mut(i)) else {
                continue;
            };
            let subject = subject_of(
                &self.land_registry,
                &self.estate_registry,
                (
                    parcel_owner,
                    estate_owner,
                    operator,
                    update_operator,
                    belongs_to_estate,
                ),
            );
            let operators = LandOperators {
                operator: subject.operator.clone(),
                update_operator: subject.update_operator.clone(),
                update_managers,
                approved_for_all,
            };
            *slot = Some(ParcelRights { subject, operators });
        }
        Ok(out)
    }

    pub async fn rights(&self, x: i32, y: i32) -> Result<Option<ParcelRights>, sqlx::Error> {
        Ok(self.rights_batch(&[(x, y)]).await?.pop().flatten())
    }

    pub async fn operators(&self, x: i32, y: i32) -> Result<Option<LandOperators>, sqlx::Error> {
        Ok(self.rights(x, y).await?.map(|r| r.operators))
    }

    pub async fn parcel_owner(&self, x: i32, y: i32) -> Result<Option<String>, sqlx::Error> {
        Ok(self.parcel_subject(x, y).await?.map(|s| s.owner))
    }

    /// Whether `owner` holds any parcel at `coords`, directly or through the
    /// estate it sits in (the LAND leg of `worlds::policy::is_admin`).
    pub async fn owns_land_at(
        &self,
        owner: &str,
        coords: &[(i32, i32)],
    ) -> Result<bool, sqlx::Error> {
        if coords.is_empty() || !super::is_address(owner) {
            return Ok(false);
        }
        let xs: Vec<i32> = coords.iter().map(|p| p.0).collect();
        let ys: Vec<i32> = coords.iter().map(|p| p.1).collect();
        sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM unnest($1::int[], $2::int[]) AS t(x, y) \
                 JOIN land_authz.token_right pt ON pt.token_address = $3 AND pt.x = t.x AND pt.y = t.y \
                 LEFT JOIN land_authz.estate_land el ON el.land_token_id = pt.token_id \
                 LEFT JOIN land_authz.token_right et ON et.token_address = $4 AND et.token_id = el.estate_token_id \
                 WHERE pt.owner = lower($5) OR et.owner = lower($5))",
        )
        .bind(&xs)
        .bind(&ys)
        .bind(&self.land_registry)
        .bind(&self.estate_registry)
        .bind(owner)
        .fetch_one(&self.pool)
        .await
    }

    /// The parcels `owner` holds, directly (`token_right_owner_idx`) or
    /// through an estate; `via_estate` says which. Bounded by `limit`.
    pub async fn parcels_owned_by(
        &self,
        owner: &str,
        limit: i64,
    ) -> Result<Vec<UpdatableParcel>, sqlx::Error> {
        let rows: Vec<(String, i32, i32, Option<String>, bool)> = sqlx::query_as(
            "SELECT tr.token_id::text, tr.x, tr.y, tr.owner, false \
             FROM land_authz.token_right tr \
             WHERE tr.token_address = $1 AND tr.owner = lower($2) AND tr.x IS NOT NULL \
             UNION ALL \
             SELECT pt.token_id::text, pt.x, pt.y, et.owner, true \
             FROM land_authz.token_right et \
             JOIN land_authz.estate_land el ON el.estate_token_id = et.token_id \
             JOIN land_authz.token_right pt ON pt.token_address = $1 AND pt.token_id = el.land_token_id \
             WHERE et.token_address = $3 AND et.owner = lower($2) \
             ORDER BY 2, 3 \
             LIMIT $4",
        )
        .bind(&self.land_registry)
        .bind(owner)
        .bind(&self.estate_registry)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(updatable).collect())
    }

    /// One page of the direct reverse lookup with its total (`count(*) OVER
    /// ()`, a bare COUNT when the page is past the end).
    pub async fn parcels_with_update_operator_page(
        &self,
        address: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<UpdatableParcel>, i64), sqlx::Error> {
        let rows: Vec<(String, i32, i32, Option<String>, i64)> = sqlx::query_as(
            "SELECT tr.token_id::text, tr.x, tr.y, tr.owner, count(*) OVER () \
             FROM land_authz.token_right tr \
             WHERE tr.token_address = $1 AND tr.update_operator = lower($2) \
             ORDER BY tr.token_id \
             LIMIT $3 OFFSET $4",
        )
        .bind(&self.land_registry)
        .bind(address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total = match rows.first() {
            Some(row) => row.4,
            None => {
                sqlx::query_scalar(
                    "SELECT count(*) FROM land_authz.token_right tr \
                     WHERE tr.token_address = $1 AND tr.update_operator = lower($2)",
                )
                .bind(&self.land_registry)
                .bind(address)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok((
            rows.into_iter()
                .map(|(token_id, x, y, owner, _)| updatable((token_id, x, y, owner, false)))
                .collect(),
            total,
        ))
    }

    /// The direct reverse lookup the lands-permissions route answers: parcels
    /// whose own update operator is this address. Served by the partial index
    /// on `token_right.update_operator`, not by a scan.
    pub async fn parcels_with_update_operator(
        &self,
        address: &str,
    ) -> Result<Vec<UpdatableParcel>, sqlx::Error> {
        let rows: Vec<(String, i32, i32, Option<String>)> = sqlx::query_as(
            "SELECT tr.token_id::text, tr.x, tr.y, tr.owner \
             FROM land_authz.token_right tr \
             WHERE tr.token_address = $1 AND tr.update_operator = lower($2) \
             ORDER BY tr.token_id",
        )
        .bind(&self.land_registry)
        .bind(address)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(token_id, x, y, owner)| updatable((token_id, x, y, owner, false)))
            .collect())
    }

    /// Every parcel this address may update, including the ones it reaches
    /// only through an estate-level grant. Strictly a superset of the direct
    /// lookup, kept separate so the route's subgraph-parity answer stays
    /// exactly that.
    pub async fn parcels_updatable_by(
        &self,
        address: &str,
    ) -> Result<Vec<UpdatableParcel>, sqlx::Error> {
        let rows: Vec<(String, i32, i32, Option<String>, bool)> = sqlx::query_as(
            "SELECT tr.token_id::text, tr.x, tr.y, tr.owner, false \
             FROM land_authz.token_right tr \
             WHERE tr.token_address = $1 AND tr.update_operator = lower($2) \
             UNION \
             SELECT pt.token_id::text, pt.x, pt.y, et.owner, true \
             FROM land_authz.token_right et \
             JOIN land_authz.estate_land el ON el.estate_token_id = et.token_id \
             JOIN land_authz.token_right pt ON pt.token_address = $1 AND pt.token_id = el.land_token_id \
             WHERE et.token_address = $3 AND et.update_operator = lower($2) \
               AND pt.update_operator IS NULL \
             ORDER BY 2, 3",
        )
        .bind(&self.land_registry)
        .bind(address)
        .bind(&self.estate_registry)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(updatable).collect())
    }
}

fn updatable(
    (token_id, x, y, owner, via_estate): (String, i32, i32, Option<String>, bool),
) -> UpdatableParcel {
    UpdatableParcel {
        token_id,
        x,
        y,
        owner: owner.unwrap_or_default().to_lowercase(),
        via_estate,
    }
}

#[cfg(all(test, feature = "ts"))]
mod ts_export {
    use super::DEPLOY_GRANTING_LEGS;

    #[test]
    fn export_bindings_deploy_granting_legs() {
        let base = std::env::var("TS_RS_EXPORT_DIR").unwrap_or_else(|_| "./bindings".to_string());
        let dir = std::path::Path::new(&base).join("validator");
        std::fs::create_dir_all(&dir).unwrap();
        let legs: Vec<String> = DEPLOY_GRANTING_LEGS
            .iter()
            .map(|leg| {
                format!(
                    "\"{}\"",
                    serde_json::to_value(leg).unwrap().as_str().unwrap()
                )
            })
            .collect();
        assert!(
            !legs.is_empty(),
            "DEPLOY_GRANTING_LEGS is empty: the exported predicate would grant nothing"
        );
        let body = format!(
            "// This file was generated by `export_bindings_deploy_granting_legs` (src/resolve.rs). Do not edit this file manually.\nimport type {{ LandRightLeg }} from \"./LandRightLeg\";\n\nexport const DEPLOY_GRANTING_LEGS = [{}] as const satisfies readonly LandRightLeg[];\n",
            legs.join(", ")
        );
        std::fs::write(dir.join("deployGrantingLegs.ts"), body).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_flags_each_leg_case_insensitively() {
        let ops = LandOperators {
            operator: Some("0xAAA1".into()),
            update_operator: Some("0xbbb2".into()),
            update_managers: vec!["0xccc3".into()],
            approved_for_all: vec!["0xDDD4".into()],
        };
        assert_eq!(
            operator_flags("0xaaa1", &ops),
            ParcelPermissionFlags {
                operator: true,
                ..Default::default()
            }
        );
        assert_eq!(
            operator_flags("0xBBB2", &ops),
            ParcelPermissionFlags {
                update_operator: true,
                ..Default::default()
            }
        );
        assert_eq!(
            operator_flags("0xccc3", &ops).granting_leg(),
            Some(LandRightLeg::UpdateManager)
        );
        assert_eq!(
            operator_flags("0xddd4", &ops).granting_leg(),
            Some(LandRightLeg::ApprovedForAll)
        );
        assert!(!operator_flags("0xeee5", &ops).grants_deploy());
    }

    #[test]
    fn rights_flags_settle_the_owner_leg_from_the_subject() {
        let rights = ParcelRights {
            subject: ParcelSubject {
                owner: "0xaaaa000000000000000000000000000000000001".into(),
                registry: String::new(),
                operator: None,
                update_operator: None,
                belongs_to_estate: false,
            },
            operators: LandOperators::default(),
        };
        let flags = rights.flags_for("0xAAAA000000000000000000000000000000000001");
        assert_eq!(
            (flags.owner, flags.granting_leg()),
            (true, Some(LandRightLeg::Owner))
        );
        let unowned = ParcelRights {
            subject: ParcelSubject {
                owner: String::new(),
                ..rights.subject.clone()
            },
            ..rights
        };
        assert!(
            !unowned.flags_for("").owner,
            "an unowned parcel never matches the empty address"
        );
        assert_eq!(
            serde_json::to_value(flags).unwrap()["updateOperator"],
            false
        );
    }
}
