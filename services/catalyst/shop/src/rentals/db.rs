//! The rentals listings queries over `signatures.*` (schemas/32_signatures.sql).
//! What changed from the
//! old crate's SQL: `metadata` is `rental_assets` keyed by
//! `(contract_address, token_id numeric)` with the wire's `nft_id` as a
//! UNIQUE (so a listing's asset is the chain identity, not a string), the
//! asset row is refreshed on conflict (the reader just fetched it), every
//! row-stamping clock is a bound parameter (`now`), never `now()`, and the
//! status values and numeric filters are validated before they reach the
//! statement (the old crate let Postgres answer 500 on a bad `status` or
//! `tokenId`).

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use super::http::PaginatedResponse;
use super::types::{RentalListing, RentalListingCreation, RentalListingPeriod};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

#[derive(Debug, Default)]
pub struct ListingFilters {
    pub category: Option<String>,
    pub text: Option<String>,
    pub lessor: Option<String>,
    pub tenant: Option<String>,
    pub status: Vec<String>,
    pub token_id: Option<String>,
    pub contract_addresses: Vec<String>,
    pub nft_ids: Vec<String>,
    pub network: Option<String>,
    pub updated_after: Option<i64>,
    pub target: Option<String>,
    pub min_price_per_day: Option<String>,
    pub max_price_per_day: Option<String>,
    pub min_distance_to_plaza: Option<i32>,
    pub max_distance_to_plaza: Option<i32>,
    pub min_estate_size: Option<i32>,
    pub max_estate_size: Option<i32>,
    pub adjacent_to_road: Option<bool>,
    pub rental_days: Vec<i32>,
}

pub struct ListingQuery {
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub offset: i64,
    pub limit: i64,
    pub filter: ListingFilters,
    pub history: bool,
}

#[derive(Debug, Default)]
pub struct PriceFilters {
    pub category: Option<String>,
    pub adjacent_to_road: Option<bool>,
    pub min_distance_to_plaza: Option<i32>,
    pub max_distance_to_plaza: Option<i32>,
    pub min_estate_size: Option<i32>,
    pub max_estate_size: Option<i32>,
    pub rental_days: Vec<i32>,
}

/// The asset columns the reader supplies (the old `metadata` row).
pub struct ListingMetadata<'a> {
    pub category: &'a str,
    pub search_text: &'a str,
    pub distance_to_plaza: Option<i32>,
    pub adjacent_to_road: Option<bool>,
    pub estate_size: Option<i32>,
    /// The subgraph's `updatedAt`.
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    OpenListing,
    Signature,
}

impl Conflict {
    pub fn message(self) -> &'static str {
        match self {
            Self::OpenListing => "There is already an open rental listing for the asset",
            Self::Signature => "There is already a rental listing with this signature",
        }
    }
}

pub const STATUS_VALUES: [&str; 4] = ["open", "executed", "cancelled", "claimed"];

pub fn is_decimal(s: &str) -> bool {
    !s.is_empty() && s.len() <= 78 && s.bytes().all(|b| b.is_ascii_digit())
}

fn to_millis(ndt: NaiveDateTime) -> i64 {
    DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc).timestamp_millis()
}

fn millis_to_naive(ms: i64, fallback: NaiveDateTime) -> NaiveDateTime {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.naive_utc())
        .unwrap_or(fallback)
}

/// The per-listing inner select: the rental, its parties and the periods
/// aggregate, grouped by the rental. `{DISTINCT}` is `DISTINCT ON (asset)`
/// for the current-listing-per-asset view, empty for the history.
const LISTING_INNER_SELECT: &str = "SELECT {DISTINCT} r.id, r.contract_address, r.token_id, r.network, r.chain_id, r.expiration, \
     r.nonces, r.signature, r.rental_contract_address, r.status, r.target, r.rented_days, r.period_chosen, \
     r.created_at, r.updated_at, r.started_at, l.lessor, l.tenant, \
     jsonb_agg(jsonb_build_array(p.min_days::text, p.max_days::text, p.price_per_day::text) ORDER BY p.min_days) AS periods, \
     min(p.price_per_day) AS min_price_per_day, max(p.price_per_day) AS max_price_per_day \
     FROM signatures.rentals r \
     JOIN signatures.rental_listings l ON l.id = r.id \
     JOIN signatures.rental_periods p ON p.rental_id = r.id ";

/// The outer projection: the wire columns of the inner row plus the asset's.
const LISTING_OUTER_COLUMNS: &str = "rentals.id, rentals.contract_address, rentals.token_id::text AS token_id, rentals.network, rentals.chain_id, \
     rentals.expiration, rentals.nonces, rentals.signature, rentals.rental_contract_address, rentals.status::text AS status, \
     rentals.target, rentals.rented_days, rentals.created_at, rentals.updated_at, rentals.started_at, \
     rentals.lessor, rentals.tenant, rentals.periods, \
     a.nft_id, a.category, a.search_text, a.nft_created_at AS metadata_created_at";

fn push_rental_days(qb: &mut QueryBuilder<Postgres>, days: &[i32]) {
    qb.push(", (SELECT DISTINCT rental_id FROM signatures.rental_periods WHERE ");
    let mut sep = qb.separated(" OR ");
    for d in days {
        sep.push("(min_days <= ");
        sep.push_bind_unseparated(*d);
        sep.push_unseparated(" AND max_days >= ");
        sep.push_bind_unseparated(*d);
        sep.push_unseparated(")");
    }
    qb.push(") AS rental_days_periods ");
}

impl Database {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The asset upsert, the rental, its listing row and its periods in one
    /// statement: a conflict on the one-open-listing index rolls every row
    /// back. `now` stamps `fetched_at`, `created_at` and `updated_at`.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_listing(
        &self,
        nft_id: &str,
        metadata: &ListingMetadata<'_>,
        nft_created_at: NaiveDateTime,
        rental: &RentalListingCreation,
        lessor: &str,
        now: NaiveDateTime,
    ) -> Result<RentalListing, sqlx::Error> {
        let expiration = millis_to_naive(rental.expiration, now);
        let min_days: Vec<i32> = rental.periods.iter().map(|p| p.min_days as i32).collect();
        let max_days: Vec<i32> = rental.periods.iter().map(|p| p.max_days as i32).collect();
        let prices: Vec<String> = rental
            .periods
            .iter()
            .map(|p| p.price_per_day.clone())
            .collect();

        let row = sqlx::query(INSERT_LISTING_SQL)
            .bind(rental.contract_address.to_lowercase())
            .bind(&rental.token_id)
            .bind(nft_id)
            .bind(&rental.network)
            .bind(rental.chain_id as i32)
            .bind(metadata.category)
            .bind(metadata.search_text)
            .bind(metadata.distance_to_plaza)
            .bind(metadata.adjacent_to_road)
            .bind(metadata.estate_size)
            .bind(nft_created_at)
            .bind(metadata.updated_at)
            .bind(now)
            .bind(expiration)
            .bind(&rental.nonces)
            .bind(&rental.signature)
            .bind(rental.rental_contract_address.to_lowercase())
            .bind(rental.target.to_lowercase())
            .bind(lessor.to_lowercase())
            .bind(min_days)
            .bind(max_days)
            .bind(prices)
            .fetch_one(&self.pool)
            .await?;
        Ok(row_to_listing(&row))
    }

    /// The 409s of the create route: `rentals_open_asset_key` (a second open
    /// listing of the asset) and `rentals_signature_key` (the same signed
    /// offer posted twice; the contract would refuse its nonces anyway).
    pub fn conflict(err: &sqlx::Error) -> Option<Conflict> {
        let sqlx::Error::Database(db) = err else {
            return None;
        };
        match db.constraint() {
            Some("rentals_open_asset_key") => Some(Conflict::OpenListing),
            Some("rentals_signature_key") => Some(Conflict::Signature),
            _ => None,
        }
    }

    pub fn is_open_conflict(err: &sqlx::Error) -> bool {
        Self::conflict(err) == Some(Conflict::OpenListing)
    }

    pub async fn listing_asset(&self, id: &str) -> Result<Option<(String, String)>, sqlx::Error> {
        let Ok(uuid) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        sqlx::query_as(
            "SELECT contract_address, token_id::text FROM signatures.rentals WHERE id = $1",
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await
    }

    /// Updates the listing's asset row from a fresh reader row and returns
    /// the refreshed listing in one statement; `None` when the rental (or
    /// its periods) is missing.
    pub async fn refresh_metadata_for_rental(
        &self,
        rental_id: &str,
        metadata: &ListingMetadata<'_>,
        now: NaiveDateTime,
    ) -> Result<Option<RentalListing>, sqlx::Error> {
        let Ok(uuid) = Uuid::parse_str(rental_id) else {
            return Ok(None);
        };
        let row = sqlx::query(REFRESH_LISTING_SQL)
            .bind(uuid)
            .bind(metadata.category)
            .bind(metadata.search_text)
            .bind(metadata.distance_to_plaza)
            .bind(metadata.adjacent_to_road)
            .bind(metadata.estate_size)
            .bind(metadata.updated_at)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| row_to_listing(&r)))
    }

    pub async fn get_listing_by_id(&self, id: &str) -> Result<Option<RentalListing>, sqlx::Error> {
        let Ok(uuid) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        let row = sqlx::query(LISTING_BY_ID_SQL)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| row_to_listing(&r)))
    }

    pub async fn get_listings(
        &self,
        q: &ListingQuery,
    ) -> Result<PaginatedResponse<RentalListing>, sqlx::Error> {
        let f = &q.filter;
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(format!(
            "SELECT {LISTING_OUTER_COLUMNS}, COUNT(*) OVER() AS rentals_listings_count \
             FROM signatures.rental_assets a JOIN ("
        ));
        qb.push(LISTING_INNER_SELECT.replace(
            "{DISTINCT}",
            if q.history {
                ""
            } else {
                "DISTINCT ON (r.contract_address, r.token_id)"
            },
        ));
        if !f.rental_days.is_empty() {
            push_rental_days(&mut qb, &f.rental_days);
        }
        qb.push("WHERE TRUE ");
        if !f.rental_days.is_empty() {
            qb.push("AND rental_days_periods.rental_id = r.id ");
        }
        if !f.status.is_empty() {
            qb.push("AND r.status = ANY(")
                .push_bind(f.status.clone())
                .push("::signatures.rental_status[]) ");
        }
        if let Some(t) = &f.target {
            qb.push("AND r.target = ")
                .push_bind(t.to_lowercase())
                .push(" ");
        }
        if let Some(ua) = f.updated_after {
            let dt = millis_to_naive(ua, Utc::now().naive_utc());
            qb.push("AND r.updated_at > ").push_bind(dt).push(" ");
        }
        if let Some(t) = &f.token_id {
            qb.push("AND r.token_id = ")
                .push_bind(t.clone())
                .push("::numeric ");
        }
        if !f.contract_addresses.is_empty() {
            let lowered: Vec<String> = f
                .contract_addresses
                .iter()
                .map(|c| c.to_lowercase())
                .collect();
            qb.push("AND r.contract_address = ANY(")
                .push_bind(lowered)
                .push(") ");
        }
        if let Some(n) = &f.network {
            qb.push("AND r.network = ").push_bind(n.clone()).push(" ");
        }
        if let Some(l) = &f.lessor {
            qb.push("AND l.lessor = ")
                .push_bind(l.to_lowercase())
                .push(" ");
        }
        if let Some(t) = &f.tenant {
            qb.push("AND l.tenant = ")
                .push_bind(t.to_lowercase())
                .push(" ");
        }
        if !f.nft_ids.is_empty() {
            let lowered: Vec<String> = f.nft_ids.iter().map(|n| n.to_lowercase()).collect();
            qb.push(
                "AND (r.contract_address, r.token_id) IN \
                 (SELECT contract_address, token_id FROM signatures.rental_assets WHERE nft_id = ANY(",
            )
            .push_bind(lowered)
            .push(")) ");
        }

        qb.push("GROUP BY r.id, l.id ");

        let mut having_started = false;
        if let Some(min) = &f.min_price_per_day {
            qb.push(" HAVING max(p.price_per_day) >= ")
                .push_bind(min.clone())
                .push("::numeric ");
            having_started = true;
        }
        if let Some(max) = &f.max_price_per_day {
            qb.push(if having_started { " AND " } else { " HAVING " })
                .push("min(p.price_per_day) <= ")
                .push_bind(max.clone())
                .push("::numeric ");
        }

        qb.push(
            "ORDER BY r.contract_address, r.token_id, r.created_at DESC) AS rentals \
             ON a.contract_address = rentals.contract_address AND a.token_id = rentals.token_id \
             WHERE TRUE ",
        );

        if let Some(c) = &f.category {
            qb.push("AND a.category = ").push_bind(c.clone()).push(" ");
        }
        if let Some(text) = &f.text {
            qb.push("AND a.search_text ILIKE '%' || ")
                .push_bind(text.clone())
                .push(" || '%' ");
        }
        if let Some(min) = f.min_distance_to_plaza {
            qb.push("AND a.distance_to_plaza >= ")
                .push_bind(min)
                .push(" ");
        }
        if let Some(max) = f.max_distance_to_plaza {
            if f.min_distance_to_plaza.is_none() {
                qb.push("AND a.distance_to_plaza >= 0 ");
            }
            qb.push("AND a.distance_to_plaza <= ")
                .push_bind(max)
                .push(" ");
        }
        if let Some(adj) = f.adjacent_to_road {
            qb.push("AND a.adjacent_to_road = ")
                .push_bind(adj)
                .push(" ");
        }
        if let Some(min) = f.min_estate_size
            && min >= 0
        {
            qb.push("AND a.estate_size >= ").push_bind(min).push(" ");
        }
        if let Some(max) = f.max_estate_size {
            qb.push("AND a.estate_size <= ").push_bind(max).push(" ");
        }

        let dir = match q.sort_direction.as_deref() {
            Some("desc") => "desc",
            _ => "asc",
        };
        let order = match q.sort_by.as_deref() {
            Some("land_creation_date") => format!("ORDER BY a.nft_created_at {dir} "),
            Some("name") => format!("ORDER BY a.search_text {dir} "),
            Some("max_rental_price") => format!("ORDER BY rentals.max_price_per_day {dir} "),
            Some("min_rental_price") => format!("ORDER BY rentals.min_price_per_day {dir} "),
            _ => format!("ORDER BY rentals.created_at {dir} "),
        };
        qb.push(order);
        qb.push("LIMIT ")
            .push_bind(q.limit)
            .push(" OFFSET ")
            .push_bind(q.offset);

        let rows = qb.build().fetch_all(&self.pool).await?;
        let total: i64 = rows
            .first()
            .map(|r| r.try_get::<i64, _>("rentals_listings_count").unwrap_or(0))
            .unwrap_or(0);
        let results: Vec<RentalListing> = rows.iter().map(row_to_listing).collect();
        Ok(PaginatedResponse::new(results, total, q.limit, q.offset))
    }

    /// The price histogram of the open listings: `price_per_day` -> listings.
    pub async fn get_prices(&self, f: &PriceFilters) -> Result<Vec<(String, i64)>, sqlx::Error> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT p.price_per_day::text AS price_per_day, COUNT(DISTINCT r.id) AS count \
             FROM signatures.rentals r \
             JOIN signatures.rental_listings l ON l.id = r.id \
             JOIN signatures.rental_periods p ON p.rental_id = r.id \
             JOIN signatures.rental_assets a ON a.contract_address = r.contract_address AND a.token_id = r.token_id ",
        );
        if !f.rental_days.is_empty() {
            push_rental_days(&mut qb, &f.rental_days);
        }
        qb.push("WHERE r.status = 'open' ");
        if !f.rental_days.is_empty() {
            qb.push("AND rental_days_periods.rental_id = r.id ");
        }
        if let Some(c) = &f.category {
            qb.push("AND a.category = ").push_bind(c.clone()).push(" ");
        }
        if let Some(adj) = f.adjacent_to_road {
            qb.push("AND a.adjacent_to_road = ")
                .push_bind(adj)
                .push(" ");
        }
        if let Some(min) = f.min_distance_to_plaza {
            qb.push("AND a.distance_to_plaza >= ")
                .push_bind(min)
                .push(" ");
        }
        if let Some(max) = f.max_distance_to_plaza {
            qb.push("AND a.distance_to_plaza <= ")
                .push_bind(max)
                .push(" ");
        }
        if let Some(min) = f.min_estate_size {
            qb.push("AND a.estate_size >= ").push_bind(min).push(" ");
        }
        if let Some(max) = f.max_estate_size {
            qb.push("AND a.estate_size <= ").push_bind(max).push(" ");
        }
        qb.push("GROUP BY p.price_per_day");

        let rows = qb.build().fetch_all(&self.pool).await?;
        Ok(rows
            .iter()
            .map(|r| {
                let p: String = r.try_get("price_per_day").unwrap_or_default();
                let c: i64 = r.try_get("count").unwrap_or(0);
                (p, c)
            })
            .collect())
    }
}

const LISTING_BY_ID_SQL: &str = "\
SELECT rentals.id, rentals.contract_address, rentals.token_id::text AS token_id, rentals.network, rentals.chain_id, \
       rentals.expiration, rentals.nonces, rentals.signature, rentals.rental_contract_address, rentals.status::text AS status, \
       rentals.target, rentals.rented_days, rentals.created_at, rentals.updated_at, rentals.started_at, \
       rentals.lessor, rentals.tenant, rentals.periods, \
       a.nft_id, a.category, a.search_text, a.nft_created_at AS metadata_created_at \
FROM signatures.rental_assets a JOIN \
  (SELECT r.id, r.contract_address, r.token_id, r.network, r.chain_id, r.expiration, r.nonces, r.signature, \
          r.rental_contract_address, r.status, r.target, r.rented_days, r.created_at, r.updated_at, r.started_at, \
          l.lessor, l.tenant, \
          jsonb_agg(jsonb_build_array(p.min_days::text, p.max_days::text, p.price_per_day::text) ORDER BY p.min_days) AS periods \
   FROM signatures.rentals r \
   JOIN signatures.rental_listings l ON l.id = r.id \
   JOIN signatures.rental_periods p ON p.rental_id = r.id \
   WHERE r.id = $1 \
   GROUP BY r.id, l.id) AS rentals \
ON a.contract_address = rentals.contract_address AND a.token_id = rentals.token_id";

const INSERT_LISTING_SQL: &str = "\
WITH a AS ( \
  INSERT INTO signatures.rental_assets \
    (contract_address, token_id, nft_id, network, chain_id, category, search_text, distance_to_plaza, \
     adjacent_to_road, estate_size, nft_created_at, nft_updated_at, fetched_at) \
  VALUES ($1, $2::numeric, $3, $4, $5, $6, $7, $8::smallint, $9, $10::smallint, $11, $12, $13) \
  ON CONFLICT (contract_address, token_id) DO UPDATE SET \
    category = EXCLUDED.category, search_text = EXCLUDED.search_text, \
    distance_to_plaza = EXCLUDED.distance_to_plaza, adjacent_to_road = EXCLUDED.adjacent_to_road, \
    estate_size = EXCLUDED.estate_size, nft_updated_at = EXCLUDED.nft_updated_at, fetched_at = EXCLUDED.fetched_at \
  RETURNING contract_address, token_id, nft_id, category, search_text, nft_created_at \
), r AS ( \
  INSERT INTO signatures.rentals \
    (contract_address, token_id, network, chain_id, expiration, nonces, signature, rental_contract_address, \
     status, target, created_at, updated_at) \
  SELECT a.contract_address, a.token_id, $4, $5, $14, $15, $16, $17, 'open', $18, $13, $13 FROM a \
  RETURNING * \
), l AS ( \
  INSERT INTO signatures.rental_listings (id, lessor) SELECT id, $19 FROM r \
  RETURNING id, lessor, tenant \
), p AS ( \
  INSERT INTO signatures.rental_periods (rental_id, min_days, max_days, price_per_day) \
  SELECT r.id, u.min_days, u.max_days, u.price_per_day::numeric \
  FROM r, UNNEST($20::int[], $21::int[], $22::text[]) AS u(min_days, max_days, price_per_day) \
  RETURNING min_days, max_days, price_per_day \
) \
SELECT r.id, r.contract_address, r.token_id::text AS token_id, r.network, r.chain_id, r.expiration, r.nonces, \
       r.signature, r.rental_contract_address, r.status::text AS status, r.target, r.rented_days, \
       r.created_at, r.updated_at, r.started_at, l.lessor, l.tenant, \
       a.nft_id, a.category, a.search_text, a.nft_created_at AS metadata_created_at, \
       (SELECT jsonb_agg(jsonb_build_array(p.min_days::text, p.max_days::text, p.price_per_day::text) ORDER BY p.min_days) FROM p) AS periods \
FROM r, l, a";

const REFRESH_LISTING_SQL: &str = "\
WITH upd AS ( \
  UPDATE signatures.rental_assets a SET \
    category = $2, search_text = $3, distance_to_plaza = $4::smallint, \
    adjacent_to_road = $5, estate_size = $6::smallint, nft_updated_at = $7, fetched_at = $8 \
  FROM signatures.rentals r \
  WHERE r.id = $1 AND a.contract_address = r.contract_address AND a.token_id = r.token_id \
  RETURNING a.contract_address, a.token_id, a.nft_id, a.category, a.search_text, a.nft_created_at \
) \
SELECT rentals.id, rentals.contract_address, rentals.token_id::text AS token_id, rentals.network, rentals.chain_id, \
       rentals.expiration, rentals.nonces, rentals.signature, rentals.rental_contract_address, rentals.status::text AS status, \
       rentals.target, rentals.rented_days, rentals.created_at, rentals.updated_at, rentals.started_at, \
       rentals.lessor, rentals.tenant, rentals.periods, \
       upd.nft_id, upd.category, upd.search_text, upd.nft_created_at AS metadata_created_at \
FROM upd JOIN \
  (SELECT r.id, r.contract_address, r.token_id, r.network, r.chain_id, r.expiration, r.nonces, r.signature, \
          r.rental_contract_address, r.status, r.target, r.rented_days, r.created_at, r.updated_at, r.started_at, \
          l.lessor, l.tenant, \
          jsonb_agg(jsonb_build_array(p.min_days::text, p.max_days::text, p.price_per_day::text) ORDER BY p.min_days) AS periods \
   FROM signatures.rentals r \
   JOIN signatures.rental_listings l ON l.id = r.id \
   JOIN signatures.rental_periods p ON p.rental_id = r.id \
   WHERE r.id = $1 \
   GROUP BY r.id, l.id) AS rentals \
ON upd.contract_address = rentals.contract_address AND upd.token_id = rentals.token_id";

pub(crate) fn row_to_listing(r: &PgRow) -> RentalListing {
    let periods_raw: serde_json::Value = r.try_get("periods").unwrap_or(serde_json::Value::Null);
    let periods = periods_raw
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    if p.len() < 3 {
                        return None;
                    }
                    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
                    Some(RentalListingPeriod {
                        min_days: s(&p[0]).parse().unwrap_or(0),
                        max_days: s(&p[1]).parse().unwrap_or(0),
                        price_per_day: s(&p[2]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let started_at: Option<NaiveDateTime> = r.try_get("started_at").ok().flatten();
    let rented_days: Option<i32> = r.try_get("rented_days").ok().flatten();

    RentalListing {
        id: r.get::<Uuid, _>("id").to_string(),
        nft_id: r.try_get("nft_id").unwrap_or_default(),
        category: r.try_get("category").unwrap_or_default(),
        search_text: r.try_get("search_text").unwrap_or_default(),
        network: r.try_get("network").unwrap_or_default(),
        chain_id: r.try_get::<i32, _>("chain_id").unwrap_or(0) as i64,
        expiration: r
            .try_get::<NaiveDateTime, _>("expiration")
            .map(to_millis)
            .unwrap_or(0),
        signature: r.try_get("signature").unwrap_or_default(),
        nonces: r.try_get("nonces").unwrap_or_default(),
        token_id: r.try_get("token_id").unwrap_or_default(),
        contract_address: r.try_get("contract_address").unwrap_or_default(),
        rental_contract_address: r.try_get("rental_contract_address").unwrap_or_default(),
        lessor: r.try_get("lessor").ok().flatten(),
        tenant: r.try_get("tenant").ok().flatten(),
        status: r.try_get("status").unwrap_or_else(|_| "open".to_string()),
        created_at: r
            .try_get::<NaiveDateTime, _>("created_at")
            .map(to_millis)
            .unwrap_or(0),
        updated_at: r
            .try_get::<NaiveDateTime, _>("updated_at")
            .map(to_millis)
            .unwrap_or(0),
        started_at: started_at.map(to_millis),
        periods,
        target: r.try_get("target").unwrap_or_default(),
        rented_days: rented_days.map(|d| d as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_guard_admits_uint256_strings_only() {
        assert!(is_decimal("0"));
        assert!(is_decimal(&"9".repeat(78)));
        assert!(!is_decimal(""));
        assert!(!is_decimal("4_2"));
        assert!(!is_decimal("-1"));
        assert!(!is_decimal("0x2a"));
        assert!(!is_decimal(&"9".repeat(79)));
    }

    #[test]
    fn the_status_values_are_the_enum_labels() {
        let ddl = crate::db::SCHEMA_SQL;
        let labels: Vec<String> = STATUS_VALUES.iter().map(|s| format!("'{s}'")).collect();
        assert!(ddl.contains(&format!(
            "CREATE TYPE signatures.rental_status AS ENUM ({})",
            labels.join(", ")
        )));
        assert!(
            ddl.contains("CREATE UNIQUE INDEX rentals_open_asset_key"),
            "is_open_conflict names this index"
        );
    }
}
