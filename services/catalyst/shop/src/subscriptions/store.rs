// from: shop-server@dc1a8be96:src/ports/notify/component.ts (upsertRequest, getRequest, getPendingForItem, markNotified, unsubscribe)
//! The waitlist rows (`shop.item_notify_requests`): the signer's upsert,
//! the read, the keyset page of an item's pending subscribers and the
//! opt-out. Marking a row notified happens in the delivery's own statement
//! (`delivery::record`), together with its outbox row.

use chrono::NaiveDateTime;
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyStatus {
    Pending,
    Notified,
    Unsubscribed,
}

impl NotifyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Notified => "notified",
            Self::Unsubscribed => "unsubscribed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "notified" => Some(Self::Notified),
            "unsubscribed" => Some(Self::Unsubscribed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyRequest {
    pub id: i64,
    pub address: String,
    pub email: String,
    pub contract_address: String,
    pub item_id: String,
    pub chain_id: i32,
    pub status: NotifyStatus,
    pub created_at: NaiveDateTime,
    pub notified_at: Option<NaiveDateTime>,
}

type Row = (
    i64,
    String,
    String,
    String,
    String,
    i32,
    String,
    NaiveDateTime,
    Option<NaiveDateTime>,
);

const COLUMNS: &str =
    "id, address, email, contract_address, item_id, chain_id, status, created_at, notified_at";

fn to_request(r: Row) -> NotifyRequest {
    NotifyRequest {
        id: r.0,
        address: r.1,
        email: r.2,
        contract_address: r.3,
        item_id: r.4,
        chain_id: r.5,
        status: NotifyStatus::parse(&r.6).unwrap_or(NotifyStatus::Unsubscribed),
        created_at: r.7,
        notified_at: r.8,
    }
}

#[derive(Clone)]
pub struct Subscriptions {
    pool: PgPool,
}

impl Subscriptions {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Records or refreshes a subscription: a repeat call updates the email
    /// and chain and resets the row to pending (a buyer re-subscribing after
    /// a notification is queued again). `address` and `contract` lowercase.
    pub async fn upsert(
        &self,
        address: &str,
        email: &str,
        contract: &str,
        item_id: &str,
        chain_id: i32,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO shop.item_notify_requests (address, email, contract_address, item_id, chain_id, status) \
             VALUES ($1, $2, $3, $4, $5, 'pending') \
             ON CONFLICT (address, contract_address, item_id) DO UPDATE \
             SET email = EXCLUDED.email, chain_id = EXCLUDED.chain_id, status = 'pending', notified_at = NULL, \
                 updated_at = timezone('UTC', now())",
        )
        .bind(address)
        .bind(email)
        .bind(contract)
        .bind(item_id)
        .bind(chain_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        address: &str,
        contract: &str,
        item_id: &str,
    ) -> sqlx::Result<Option<NotifyRequest>> {
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM shop.item_notify_requests WHERE address = $1 AND contract_address = $2 AND item_id = $3"
        )))
        .bind(address)
        .bind(contract)
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(to_request))
    }

    /// A page of an item's pending subscribers in `id` order, after `after_id`.
    pub async fn pending_for_item(
        &self,
        contract: &str,
        item_id: &str,
        after_id: i64,
        limit: i64,
    ) -> sqlx::Result<Vec<NotifyRequest>> {
        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM shop.item_notify_requests \
             WHERE contract_address = $1 AND item_id = $2 AND status = 'pending' AND id > $3 ORDER BY id LIMIT $4"
        )))
        .bind(contract)
        .bind(item_id)
        .bind(after_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(to_request).collect())
    }

    /// Opt-out: one item when `scope` is given, every row of the account
    /// otherwise; already-unsubscribed rows stay put. The count changed.
    pub async fn unsubscribe(
        &self,
        address: &str,
        scope: Option<(&str, &str)>,
    ) -> sqlx::Result<u64> {
        let res = match scope {
            Some((contract, item_id)) => {
                sqlx::query(
                    "UPDATE shop.item_notify_requests SET status = 'unsubscribed', updated_at = timezone('UTC', now()) \
                     WHERE address = $1 AND status <> 'unsubscribed' AND contract_address = $2 AND item_id = $3",
                )
                .bind(address)
                .bind(contract)
                .bind(item_id)
                .execute(&self.pool)
                .await?
            }
            None => {
                sqlx::query(
                    "UPDATE shop.item_notify_requests SET status = 'unsubscribed', updated_at = timezone('UTC', now()) \
                     WHERE address = $1 AND status <> 'unsubscribed'",
                )
                .bind(address)
                .execute(&self.pool)
                .await?
            }
        };
        Ok(res.rows_affected())
    }
}
