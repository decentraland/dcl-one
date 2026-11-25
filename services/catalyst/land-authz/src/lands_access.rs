//! The Creator Hub's LAND list: every parcel and estate a wallet holds a
//! role on, with the account-level authorizations around it, read from the
//! land-authz index in the shapes the land-manager subgraph answered the
//! Creator Hub's `Land` and `GrantorLand` queries with
//! (`apps/creator-hub/.../renderer/src/lib/land.ts`).
//!
//! The lists, as the subgraph filtered them:
//! - `owner*`: parcels outside any estate and estates whose owner is the
//!   wallet (a parcel inside an estate is owned by the Estate registry);
//! - `updateOperator*`, `approved*`: tokens whose update operator or
//!   approved address (`approve`) is the wallet, inside an estate or not;
//! - `tenant*`, `lessor*`: the token ids the caller names (its rentals);
//! - `grantor*`: the land of every owner whose current `UpdateManager` or
//!   `ApprovalForAll` authorization to the wallet is approved, per registry;
//! - `ownerAuthorizations`: the wallet's own `UpdateManager` grants;
//!   `operatorAuthorizations`: the `UpdateManager` and `ApprovalForAll`
//!   grants naming the wallet as operator.
//!
//! The index keeps the current state of each authorization, not its event
//! history, so each list holds one entry per counterparty, registry and kind
//! (what the client reduced the subgraph's history to); its `timestamp` is
//! `"0"` where the index has no block time for it. Parcel names come
//! from the shop's `land_data` (the nft index's LAND `Update` data), estate
//! names from the Estate registry's data events; both are display only.

use serde::Serialize;
use serde_json::Value;
use sqlx::PgConnection;

/// The token ids one request may name per rental list.
pub const MAX_RENTAL_TOKEN_IDS: usize = 1000;

#[derive(Debug, Serialize, PartialEq)]
pub struct Owner {
    pub address: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct LandData {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// The subgraph's `Parcel`, as the Creator Hub's `parcelFields` read it.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParcelFields {
    pub x: String,
    pub y: String,
    pub token_id: String,
    pub owner: Owner,
    pub operator: Option<String>,
    pub update_operator: Option<String>,
    pub data: Option<LandData>,
}

/// The subgraph's `Estate`, as the Creator Hub's `estateFields` read it.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EstateFields {
    pub id: String,
    pub owner: Owner,
    pub operator: Option<String>,
    pub update_operator: Option<String>,
    pub size: i64,
    pub parcels: Value,
    pub data: Option<LandData>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnerAuthorization {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub operator: String,
    pub is_approved: bool,
    pub token_address: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperatorAuthorization {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub owner: Owner,
    pub is_approved: bool,
    pub token_address: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LandsAccess {
    pub owner_parcels: Vec<ParcelFields>,
    pub owner_estates: Vec<EstateFields>,
    pub update_operator_parcels: Vec<ParcelFields>,
    pub update_operator_estates: Vec<EstateFields>,
    pub approved_parcels: Vec<ParcelFields>,
    pub approved_estates: Vec<EstateFields>,
    pub tenant_parcels: Vec<ParcelFields>,
    pub tenant_estates: Vec<EstateFields>,
    pub lessor_parcels: Vec<ParcelFields>,
    pub lessor_estates: Vec<EstateFields>,
    pub grantor_parcels: Vec<ParcelFields>,
    pub grantor_estates: Vec<EstateFields>,
    pub owner_authorizations: Vec<OwnerAuthorization>,
    pub operator_authorizations: Vec<OperatorAuthorization>,
}

/// Which tokens one list holds; the parameter is bound as `$2`.
enum Pick<'a> {
    Owner(&'a str),
    UpdateOperator(&'a str),
    Operator(&'a str),
    TokenIds(&'a [String]),
    /// Tokens of the owners whose authorization to `$2` on this registry is
    /// approved.
    Grantors(&'a str),
}

impl Pick<'_> {
    fn predicate(&self, alias: &str) -> String {
        match self {
            Pick::Owner(_) => format!("{alias}.owner = $2"),
            Pick::UpdateOperator(_) => format!("{alias}.update_operator = $2"),
            Pick::Operator(_) => format!("{alias}.operator = $2"),
            Pick::TokenIds(_) => format!("{alias}.token_id = ANY($2::numeric[])"),
            Pick::Grantors(_) => format!(
                "{alias}.owner IN (SELECT ar.account FROM land_authz.account_right ar \
                 WHERE ar.token_address = {alias}.token_address AND ar.operator = $2 \
                 AND ar.kind IN ('update_manager', 'approved_for_all') AND ar.is_approved)"
            ),
        }
    }

    fn bind<'q>(
        &self,
        query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match self {
            Pick::Owner(a) | Pick::UpdateOperator(a) | Pick::Operator(a) | Pick::Grantors(a) => {
                query.bind(a.to_string())
            }
            Pick::TokenIds(ids) => query.bind(ids.to_vec()),
        }
    }
}

/// Every list for `address` (lowercase), on the caller's snapshot.
pub async fn lands_access_on(
    conn: &mut PgConnection,
    land_registry: &str,
    estate_registry: &str,
    address: &str,
    tenant_token_ids: &[String],
    lessor_token_ids: &[String],
) -> Result<LandsAccess, sqlx::Error> {
    let parcels = |pick| parcels_on(land_registry, pick);
    let estates = |pick| estates_on(land_registry, estate_registry, pick);
    Ok(LandsAccess {
        owner_parcels: parcels(Pick::Owner(address)).run(conn).await?,
        owner_estates: estates(Pick::Owner(address)).run(conn).await?,
        update_operator_parcels: parcels(Pick::UpdateOperator(address)).run(conn).await?,
        update_operator_estates: estates(Pick::UpdateOperator(address)).run(conn).await?,
        approved_parcels: parcels(Pick::Operator(address)).run(conn).await?,
        approved_estates: estates(Pick::Operator(address)).run(conn).await?,
        tenant_parcels: parcels(Pick::TokenIds(tenant_token_ids)).run(conn).await?,
        tenant_estates: estates(Pick::TokenIds(tenant_token_ids)).run(conn).await?,
        lessor_parcels: parcels(Pick::TokenIds(lessor_token_ids)).run(conn).await?,
        lessor_estates: estates(Pick::TokenIds(lessor_token_ids)).run(conn).await?,
        grantor_parcels: parcels(Pick::Grantors(address)).run(conn).await?,
        grantor_estates: estates(Pick::Grantors(address)).run(conn).await?,
        owner_authorizations: owner_authorizations_on(conn, address).await?,
        operator_authorizations: operator_authorizations_on(conn, address).await?,
    })
}

struct ParcelList<'a> {
    land_registry: &'a str,
    pick: Pick<'a>,
}

fn parcels_on<'a>(land_registry: &'a str, pick: Pick<'a>) -> ParcelList<'a> {
    ParcelList {
        land_registry,
        pick,
    }
}

impl ParcelList<'_> {
    async fn run(self, conn: &mut PgConnection) -> Result<Vec<ParcelFields>, sqlx::Error> {
        if matches!(self.pick, Pick::TokenIds(ids) if ids.is_empty()) {
            return Ok(Vec::new());
        }
        // The owner and grantor lists hold parcels outside any estate, as the
        // subgraph's `estate: null` did: a parcel inside one is the estate's
        // (owned by the Estate registry, which is an address too). The
        // operator, approved and rental lists admit estate members.
        let outside_estates = if matches!(self.pick, Pick::Owner(_) | Pick::Grantors(_)) {
            " AND NOT EXISTS (SELECT 1 FROM land_authz.estate_land el WHERE el.land_token_id = pt.token_id)"
        } else {
            ""
        };
        let sql = format!(
            "SELECT pt.token_id::text, pt.x, pt.y, pt.owner, pt.operator, pt.update_operator, \
                    ld.name, ld.description, ld.token_id IS NOT NULL \
             FROM land_authz.token_right pt \
             LEFT JOIN shop.land_data ld ON ld.token_address = pt.token_address AND ld.token_id = pt.token_id \
             WHERE pt.token_address = $1 AND pt.x IS NOT NULL AND pt.y IS NOT NULL AND {}{outside_estates} \
             ORDER BY pt.token_id",
            self.pick.predicate("pt")
        );
        let query = self
            .pick
            .bind(sqlx::query(sqlx::AssertSqlSafe(sql)).bind(self.land_registry));
        let rows = query.fetch_all(&mut *conn).await?;
        use sqlx::Row;
        Ok(rows
            .into_iter()
            .map(|r| ParcelFields {
                token_id: r.get(0),
                x: r.get::<i32, _>(1).to_string(),
                y: r.get::<i32, _>(2).to_string(),
                owner: Owner {
                    address: r.get::<Option<String>, _>(3).unwrap_or_default(),
                },
                operator: r.get(4),
                update_operator: r.get(5),
                data: r.get::<bool, _>(8).then(|| LandData {
                    name: r.get(6),
                    description: r.get(7),
                }),
            })
            .collect())
    }
}

struct EstateList<'a> {
    land_registry: &'a str,
    estate_registry: &'a str,
    pick: Pick<'a>,
}

fn estates_on<'a>(
    land_registry: &'a str,
    estate_registry: &'a str,
    pick: Pick<'a>,
) -> EstateList<'a> {
    EstateList {
        land_registry,
        estate_registry,
        pick,
    }
}

impl EstateList<'_> {
    async fn run(self, conn: &mut PgConnection) -> Result<Vec<EstateFields>, sqlx::Error> {
        if matches!(self.pick, Pick::TokenIds(ids) if ids.is_empty()) {
            return Ok(Vec::new());
        }
        // `$1` the estate registry and `$2` the pick, as for parcels; the
        // LAND registry is `$3`.
        let sql = format!(
            "SELECT et.token_id::text, et.owner, et.operator, et.update_operator, \
                    d.name, d.description, d.estate_id IS NOT NULL, \
                    COALESCE(p.size, 0), COALESCE(p.parcels, '[]'::jsonb) \
             FROM land_authz.token_right et \
             LEFT JOIN LATERAL (SELECT x.estate_id, x.name, x.description FROM land_authz.estate_data_event x \
                                WHERE x.token_address = et.token_address AND x.estate_id = et.token_id \
                                ORDER BY x.block_number DESC, x.log_index DESC LIMIT 1) d ON true \
             LEFT JOIN LATERAL (SELECT count(*) AS size, \
                                       jsonb_agg(jsonb_build_object('x', pt.x::text, 'y', pt.y::text, \
                                                 'tokenId', pt.token_id::text, 'updateOperator', pt.update_operator) \
                                                 ORDER BY pt.token_id) AS parcels \
                                FROM land_authz.estate_land el \
                                JOIN land_authz.token_right pt ON pt.token_address = $3 AND pt.token_id = el.land_token_id \
                                WHERE el.estate_token_id = et.token_id AND pt.x IS NOT NULL AND pt.y IS NOT NULL) p ON true \
             WHERE et.token_address = $1 AND {} \
             ORDER BY et.token_id",
            self.pick.predicate("et")
        );
        let query = self
            .pick
            .bind(sqlx::query(sqlx::AssertSqlSafe(sql)).bind(self.estate_registry))
            .bind(self.land_registry);
        let rows = query.fetch_all(&mut *conn).await?;
        use sqlx::Row;
        Ok(rows
            .into_iter()
            .map(|r| EstateFields {
                id: r.get(0),
                owner: Owner {
                    address: r.get::<Option<String>, _>(1).unwrap_or_default(),
                },
                operator: r.get(2),
                update_operator: r.get(3),
                data: r.get::<bool, _>(6).then(|| LandData {
                    name: r.get(4),
                    description: r.get(5),
                }),
                size: r.get(7),
                parcels: r.get(8),
            })
            .collect())
    }
}

/// The subgraph's authorization type for an index kind.
fn authorization_type(kind: &str) -> &'static str {
    match kind {
        "update_manager" => "UpdateManager",
        _ => "ApprovalForAll",
    }
}

/// The subgraph's ordering key: the authorization's block time in
/// microseconds plus its log index. `"0"` when the index has no block time
/// for it (no event row, or one whose time the timestamp repair has not
/// filled yet): the lists hold the current state, one entry per
/// counterparty, registry and kind, so nothing is ordered by it.
const TIMESTAMP_SQL: &str =
    "(CASE WHEN e.block_time > 0 THEN e.block_time * 1000000 + ar.updated_log ELSE 0 END)::text";

async fn owner_authorizations_on(
    conn: &mut PgConnection,
    address: &str,
) -> Result<Vec<OwnerAuthorization>, sqlx::Error> {
    let rows: Vec<(String, String, bool, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT ar.kind, ar.operator, ar.is_approved, ar.token_address, {TIMESTAMP_SQL} \
         FROM land_authz.account_right ar \
         LEFT JOIN land_authz.authz_event e ON e.block_number = ar.updated_block AND e.log_index = ar.updated_log \
         WHERE ar.account = $1 AND ar.kind = 'update_manager' \
         ORDER BY ar.token_address, ar.operator"
    )))
    .bind(address)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(kind, operator, is_approved, token_address, timestamp)| OwnerAuthorization {
                kind: authorization_type(&kind),
                operator,
                is_approved,
                token_address,
                timestamp,
            },
        )
        .collect())
}

async fn operator_authorizations_on(
    conn: &mut PgConnection,
    address: &str,
) -> Result<Vec<OperatorAuthorization>, sqlx::Error> {
    let rows: Vec<(String, String, bool, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT ar.kind, ar.account, ar.is_approved, ar.token_address, {TIMESTAMP_SQL} \
         FROM land_authz.account_right ar \
         LEFT JOIN land_authz.authz_event e ON e.block_number = ar.updated_block AND e.log_index = ar.updated_log \
         WHERE ar.operator = $1 AND ar.kind IN ('update_manager', 'approved_for_all') \
         ORDER BY ar.token_address, ar.account, ar.kind"
    )))
    .bind(address)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(kind, account, is_approved, token_address, timestamp)| OperatorAuthorization {
                kind: authorization_type(&kind),
                owner: Owner { address: account },
                is_approved,
                token_address,
                timestamp,
            },
        )
        .collect())
}
